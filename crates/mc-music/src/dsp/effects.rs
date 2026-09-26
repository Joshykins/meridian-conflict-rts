//! Insert effects. Each `Unit` keeps the state of one `Effect` and is updated
//! in place when the effect's settings change, so editing a knob while the
//! song plays keeps delay lines and reverb tails going.

use super::filter::{Biquad, BiquadKind, DcBlock, ModeFilter, OnePole};
use super::reverb::Hall;
use super::{db_to_gain, soft_clip};
use crate::patch::{Curve, Effect, FilterMode};
use std::f32::consts::TAU;

pub struct Ctx<'a> {
    pub rate: f32,
    /// Samples per beat at the song's tempo.
    pub beat: f32,
    /// The key signal for a sidechained compressor, same length as the block.
    pub sidechain: Option<&'a [[f32; 2]]>,
}

/// Gain reduction a dynamics unit is applying, in dB (<= 0), for the meters.
pub type Reduction = f32;

pub enum Unit {
    Eq(EqUnit),
    Filter([ModeFilter; 2], FilterMode),
    Drive([OnePole; 2], [DcBlock; 2]),
    Chorus(ChorusUnit),
    Delay(DelayUnit),
    Reverb(Box<Hall>),
    Compressor(CompUnit),
    Limiter(LimiterUnit),
    Width,
    Crush(CrushUnit),
}

impl Unit {
    pub fn new(fx: &Effect, rate: f32) -> Unit {
        let mut u = match fx {
            Effect::Eq { .. } => Unit::Eq(EqUnit::default()),
            Effect::Filter { mode, .. } => Unit::Filter(Default::default(), *mode),
            Effect::Drive { .. } => Unit::Drive(Default::default(), Default::default()),
            Effect::Chorus { .. } => Unit::Chorus(ChorusUnit::new(rate)),
            Effect::Delay { .. } => Unit::Delay(DelayUnit::new(rate)),
            Effect::Reverb { .. } => Unit::Reverb(Box::new(Hall::new(rate))),
            Effect::Compressor { .. } => Unit::Compressor(CompUnit::default()),
            Effect::Limiter { .. } => Unit::Limiter(LimiterUnit::new(rate)),
            Effect::Width { .. } => Unit::Width,
            Effect::Crush { .. } => Unit::Crush(CrushUnit::default()),
        };
        u.update(fx, rate);
        u
    }

    /// Whether this unit's state can carry on under `fx` (same kind).
    pub fn fits(&self, fx: &Effect) -> bool {
        matches!(
            (self, fx),
            (Unit::Eq(_), Effect::Eq { .. })
                | (Unit::Filter(..), Effect::Filter { .. })
                | (Unit::Drive(..), Effect::Drive { .. })
                | (Unit::Chorus(_), Effect::Chorus { .. })
                | (Unit::Delay(_), Effect::Delay { .. })
                | (Unit::Reverb(_), Effect::Reverb { .. })
                | (Unit::Compressor(_), Effect::Compressor { .. })
                | (Unit::Limiter(_), Effect::Limiter { .. })
                | (Unit::Width, Effect::Width { .. })
                | (Unit::Crush(_), Effect::Crush { .. })
        )
    }

    /// Takes new settings without losing state.
    pub fn update(&mut self, fx: &Effect, rate: f32) {
        match (self, fx) {
            (
                Unit::Eq(u),
                Effect::Eq { low_db, low_hz, mid_db, mid_hz, mid_q, high_db, high_hz, cut_hz, .. },
            ) => {
                for c in 0..2 {
                    u.low[c].set(BiquadKind::LowShelf, *low_hz, 0.707, *low_db, rate);
                    u.mid[c].set(BiquadKind::Peak, *mid_hz, *mid_q, *mid_db, rate);
                    u.high[c].set(BiquadKind::HighShelf, *high_hz, 0.707, *high_db, rate);
                    u.cut[c].set(BiquadKind::HighPass, cut_hz.max(10.0), 0.707, 0.0, rate);
                }
                u.use_cut = *cut_hz > 0.0;
                u.flat = *low_db == 0.0 && *mid_db == 0.0 && *high_db == 0.0;
            }
            (Unit::Filter(f, m), Effect::Filter { mode, cutoff, resonance, .. }) => {
                *m = *mode;
                for x in f.iter_mut() {
                    x.set(*mode, *cutoff, *resonance, rate);
                }
            }
            (Unit::Drive(tone, _), Effect::Drive { tone: t, .. }) => {
                for p in tone.iter_mut() {
                    p.set(*t, rate);
                }
            }
            (Unit::Chorus(_), _) | (Unit::Delay(_), _) | (Unit::Width, _) | (Unit::Crush(_), _) => {}
            (Unit::Reverb(h), Effect::Reverb { size, decay, damp, predelay, lowcut, .. }) => {
                h.set(*size, *decay, *damp, *predelay, *lowcut);
            }
            (Unit::Compressor(c), Effect::Compressor { attack, release, .. }) => {
                c.attack = (-1.0 / (attack.max(0.05) * 0.001 * rate)).exp();
                c.release = (-1.0 / (release.max(1.0) * 0.001 * rate)).exp();
            }
            (Unit::Limiter(l), Effect::Limiter { release, .. }) => {
                l.release = (-1.0 / (release.max(1.0) * 0.001 * rate)).exp();
            }
            _ => {}
        }
    }

    /// Processes a block in place. Returns the gain reduction for dynamics, else 0.
    pub fn run(&mut self, fx: &Effect, buf: &mut [[f32; 2]], ctx: &Ctx) -> Reduction {
        if !fx.is_on() {
            return 0.0;
        }
        match (self, fx) {
            (Unit::Eq(u), _) => {
                if u.flat && !u.use_cut {
                    return 0.0;
                }
                for f in buf.iter_mut() {
                    for (c, s) in f.iter_mut().enumerate() {
                        let mut x = *s;
                        if u.use_cut {
                            x = u.cut[c].process(x);
                        }
                        if !u.flat {
                            x = u.high[c].process(u.mid[c].process(u.low[c].process(x)));
                        }
                        *s = x;
                    }
                }
                for c in 0..2 {
                    u.low[c].sanitise();
                    u.mid[c].sanitise();
                    u.high[c].sanitise();
                    u.cut[c].sanitise();
                }
                0.0
            }
            (Unit::Filter(fl, mode), _) => {
                for f in buf.iter_mut() {
                    for c in 0..2 {
                        f[c] = fl[c].process(f[c], *mode);
                    }
                }
                fl[0].sanitise();
                fl[1].sanitise();
                0.0
            }
            (Unit::Drive(tone, dc), Effect::Drive { amount, curve, mix, .. }) => {
                let pre = 1.0 + amount.clamp(0.0, 1.0) * 14.0;
                // Keep the level about where it was so the knob is about colour, not volume.
                let post = 1.0 / (1.0 + amount.clamp(0.0, 1.0) * 2.2);
                for f in buf.iter_mut() {
                    for c in 0..2 {
                        let x = f[c];
                        let d = x * pre;
                        let y = match curve {
                            Curve::Tape => soft_clip(d + 0.08 * d * d.abs().min(1.0)) ,
                            Curve::Clip => d.clamp(-1.0, 1.0),
                            Curve::Fold => {
                                let y = (d + 1.0).rem_euclid(4.0);
                                if y < 2.0 { y - 1.0 } else { 3.0 - y }
                            }
                        };
                        let y = dc[c].process(tone[c].process(y)) * post;
                        f[c] = x + (y - x) * mix;
                    }
                }
                0.0
            }
            (Unit::Chorus(u), Effect::Chorus { rate, depth, mix, .. }) => {
                u.run(buf, *rate, *depth, *mix, ctx.rate);
                0.0
            }
            (Unit::Delay(u), Effect::Delay { beats, feedback, mix, pingpong, tone, .. }) => {
                u.run(buf, beats * ctx.beat, *feedback, *mix, *pingpong, *tone, ctx.rate);
                0.0
            }
            (Unit::Reverb(h), Effect::Reverb { mix, .. }) => {
                h.run(buf, *mix);
                0.0
            }
            (Unit::Compressor(u), Effect::Compressor { threshold, ratio, makeup, .. }) => {
                u.run(buf, *threshold, *ratio, *makeup, ctx.sidechain)
            }
            (Unit::Limiter(u), Effect::Limiter { ceiling, gain, .. }) => u.run(buf, *ceiling, *gain),
            (Unit::Width, Effect::Width { amount, .. }) => {
                for f in buf.iter_mut() {
                    let m = (f[0] + f[1]) * 0.5;
                    let s = (f[0] - f[1]) * 0.5 * amount;
                    f[0] = m + s;
                    f[1] = m - s;
                }
                0.0
            }
            (Unit::Crush(u), Effect::Crush { bits, rate, mix, .. }) => {
                let levels = 2f32.powf(bits.clamp(1.0, 16.0) - 1.0);
                let step = (rate / ctx.rate).clamp(0.001, 1.0);
                for f in buf.iter_mut() {
                    u.acc += step;
                    if u.acc >= 1.0 {
                        u.acc -= 1.0;
                        for (held, &s) in u.held.iter_mut().zip(f.iter()) {
                            *held = (s * levels).round() / levels;
                        }
                    }
                    for (s, &held) in f.iter_mut().zip(&u.held) {
                        *s += (held - *s) * mix;
                    }
                }
                0.0
            }
            _ => 0.0,
        }
    }
}

#[derive(Default)]
pub struct EqUnit {
    low: [Biquad; 2],
    mid: [Biquad; 2],
    high: [Biquad; 2],
    cut: [Biquad; 2],
    use_cut: bool,
    flat: bool,
}

/// The EQ's response in dB at `freq`, for drawing.
pub fn eq_response(fx: &Effect, freq: f32, rate: f32) -> f32 {
    let mut u = EqUnit::default();
    let mut unit = Unit::Eq(EqUnit::default());
    unit.update(fx, rate);
    if let Unit::Eq(e) = unit {
        u = e;
    }
    let mut m = u.low[0].magnitude(freq, rate) * u.mid[0].magnitude(freq, rate) * u.high[0].magnitude(freq, rate);
    if u.use_cut {
        m *= u.cut[0].magnitude(freq, rate);
    }
    super::gain_to_db(m)
}

/// A power-of-two ring buffer read with linear or cubic interpolation.
pub struct Line {
    buf: Vec<f32>,
    mask: usize,
    at: usize,
}

impl Line {
    pub fn new(min_len: usize) -> Line {
        let n = min_len.next_power_of_two().max(16);
        Line { buf: vec![0.0; n], mask: n - 1, at: 0 }
    }
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.at = (self.at + 1) & self.mask;
        self.buf[self.at] = x;
    }
    /// The sample `delay` samples ago (fractional), Hermite-interpolated.
    #[inline]
    pub fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.mask - 3) as f32);
        let i = d as usize;
        let t = d - i as f32;
        let at = |k: usize| self.buf[(self.at.wrapping_sub(k)) & self.mask];
        let (y0, y1, y2, y3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c0 = y1;
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        ((c3 * t + c2) * t + c1) * t + c0
    }
    /// Linear interpolation: cheaper, for slowly modulated reads (the reverb) where the
    /// small high-frequency loss is inaudible.
    #[inline]
    pub fn read_linear(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.mask - 2) as f32);
        let i = d as usize;
        let t = d - i as f32;
        let a = self.buf[(self.at.wrapping_sub(i)) & self.mask];
        let b = self.buf[(self.at.wrapping_sub(i + 1)) & self.mask];
        a + (b - a) * t
    }
    #[inline]
    pub fn read_int(&self, delay: usize) -> f32 {
        self.buf[(self.at.wrapping_sub(delay)) & self.mask]
    }
    #[expect(clippy::len_without_is_empty, reason = "a delay line is never empty: len is its power-of-two capacity")]
    pub fn len(&self) -> usize {
        self.mask + 1
    }
    pub fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

pub struct ChorusUnit {
    lines: [Line; 2],
    phase: f32,
}

impl ChorusUnit {
    fn new(rate: f32) -> ChorusUnit {
        let n = (rate * 0.06) as usize;
        ChorusUnit { lines: [Line::new(n), Line::new(n)], phase: 0.0 }
    }
    fn run(&mut self, buf: &mut [[f32; 2]], lfo: f32, depth_ms: f32, mix: f32, rate: f32) {
        let base = 0.012 * rate;
        let depth = depth_ms.clamp(0.0, 20.0) * 0.001 * rate;
        let step = lfo / rate;
        for f in buf.iter_mut() {
            self.phase = (self.phase + step).fract();
            for (c, s) in f.iter_mut().enumerate() {
                self.lines[c].push(*s);
                // Two taps per side, a third of a cycle apart, the sides in quadrature.
                let p = self.phase + c as f32 * 0.25;
                let a = self.lines[c].read(base + depth * (0.5 + 0.5 * (p * TAU).sin()));
                let b = self.lines[c].read(base * 1.37 + depth * (0.5 + 0.5 * ((p + 0.333) * TAU).sin()));
                let wet = (a + b) * 0.5;
                *s = *s * (1.0 - mix * 0.5) + wet * mix;
            }
        }
    }
}

pub struct DelayUnit {
    lines: [Line; 2],
    tone: [OnePole; 2],
    hp: [Biquad; 2],
    /// Smoothed delay time, so a tempo change glides instead of clicking.
    time: f32,
    rate: f32,
}

impl DelayUnit {
    fn new(rate: f32) -> DelayUnit {
        let n = (rate * 4.0) as usize;
        let mut hp = [Biquad::default(); 2];
        for h in hp.iter_mut() {
            h.set(BiquadKind::HighPass, 150.0, 0.707, 0.0, rate);
        }
        DelayUnit {
            lines: [Line::new(n), Line::new(n)],
            tone: Default::default(),
            hp,
            time: 0.0,
            rate,
        }
    }
    fn run(&mut self, buf: &mut [[f32; 2]], samples: f32, feedback: f32, mix: f32, pingpong: bool, tone: f32, rate: f32) {
        let target = samples.clamp(1.0, (self.lines[0].len() - 8) as f32);
        if self.time <= 0.0 {
            self.time = target;
        }
        for t in self.tone.iter_mut() {
            t.set(tone, rate);
        }
        let fb = feedback.clamp(0.0, 0.97);
        let k = 1.0 - (-1.0 / (0.05 * self.rate)).exp();
        for f in buf.iter_mut() {
            self.time += (target - self.time) * k;
            let l = self.lines[0].read(self.time);
            let r = self.lines[1].read(self.time);
            let wl = self.hp[0].process(self.tone[0].process(l));
            let wr = self.hp[1].process(self.tone[1].process(r));
            if pingpong {
                // The input enters on the left only (mono sum); repeats cross sides.
                let input = (f[0] + f[1]) * 0.5;
                self.lines[0].push(soft_clip(input + wr * fb));
                self.lines[1].push(soft_clip(wl * fb));
            } else {
                self.lines[0].push(soft_clip(f[0] + wl * fb));
                self.lines[1].push(soft_clip(f[1] + wr * fb));
            }
            f[0] += l * mix;
            f[1] += r * mix;
        }
        self.hp[0].sanitise();
        self.hp[1].sanitise();
    }
}

#[derive(Default)]
pub struct CompUnit {
    env: f32,
    attack: f32,
    release: f32,
    /// The gain being applied, gliding to the one computed every few samples.
    gain: f32,
}

impl CompUnit {
    fn run(&mut self, buf: &mut [[f32; 2]], threshold: f32, ratio: f32, makeup: f32, key: Option<&[[f32; 2]]>) -> Reduction {
        let slope = 1.0 - 1.0 / ratio.max(1.0);
        let up = db_to_gain(makeup);
        let mut most = 0.0f32;
        let knee = 6.0;
        // Below this level nothing is cut, so the logs can be skipped.
        let floor = db_to_gain(threshold - knee / 2.0);
        if self.gain <= 0.0 {
            self.gain = 1.0;
        }
        let mut target = self.gain;
        for (i, f) in buf.iter_mut().enumerate() {
            let k = key.map(|k| k[i]).unwrap_or(*f);
            let level = k[0].abs().max(k[1].abs());
            let coef = if level > self.env { self.attack } else { self.release };
            self.env = level + (self.env - level) * coef;
            // The gain curve is worked out every 8 samples (6 kHz at 48 kHz, far faster than
            // any attack) and glided between, instead of a log and a power every sample.
            if i % 8 == 0 {
                let cut = if self.env <= floor {
                    0.0
                } else {
                    let over = super::gain_to_db(self.env) - threshold;
                    // Soft knee.
                    if over >= knee / 2.0 {
                        over * slope
                    } else {
                        slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
                    }
                };
                most = most.max(cut);
                target = if cut > 0.0 { db_to_gain(-cut) } else { 1.0 };
            }
            self.gain += (target - self.gain) * 0.25;
            let g = self.gain * up;
            f[0] *= g;
            f[1] *= g;
        }
        -most
    }
}

pub struct LimiterUnit {
    lines: [Line; 2],
    /// Peak-hold window over the lookahead, as a max of a sliding window.
    window: std::collections::VecDeque<(usize, f32)>,
    n: usize,
    look: usize,
    gain: f32,
    release: f32,
}

impl LimiterUnit {
    fn new(rate: f32) -> LimiterUnit {
        let look = (rate * 0.0015) as usize + 1;
        LimiterUnit {
            lines: [Line::new(look + 8), Line::new(look + 8)],
            window: std::collections::VecDeque::with_capacity(look + 2),
            n: 0,
            look,
            gain: 1.0,
            release: 0.999,
        }
    }
    fn run(&mut self, buf: &mut [[f32; 2]], ceiling: f32, input_db: f32) -> Reduction {
        let ceil = db_to_gain(ceiling.min(0.0));
        let pre = db_to_gain(input_db);
        let mut least = 1.0f32;
        // Attack reaches the needed gain within the lookahead.
        let attack = (-4.0 / self.look as f32).exp();
        for f in buf.iter_mut() {
            let (l, r) = (f[0] * pre, f[1] * pre);
            self.lines[0].push(l);
            self.lines[1].push(r);
            let peak = l.abs().max(r.abs());
            self.n += 1;
            while self.window.back().map(|b| b.1 <= peak).unwrap_or(false) {
                self.window.pop_back();
            }
            self.window.push_back((self.n, peak));
            while self.window.front().map(|a| a.0 + self.look < self.n).unwrap_or(false) {
                self.window.pop_front();
            }
            let held = self.window.front().map(|a| a.1).unwrap_or(0.0);
            let want = if held > ceil { ceil / held } else { 1.0 };
            let coef = if want < self.gain { attack } else { self.release };
            self.gain = want + (self.gain - want) * coef;
            let g = self.gain.min(want.max(self.gain));
            least = least.min(g);
            let dl = self.lines[0].read_int(self.look);
            let dr = self.lines[1].read_int(self.look);
            // A final safety clip: the smoothing can let the very edge of a peak through.
            f[0] = (dl * g).clamp(-ceil, ceil);
            f[1] = (dr * g).clamp(-ceil, ceil);
        }
        super::gain_to_db(least)
    }
}

#[derive(Default)]
pub struct CrushUnit {
    acc: f32,
    held: [f32; 2],
}
