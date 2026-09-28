//! The stereo buffer every sound is built in, and the few filters its layers use.

use std::f32::consts::TAU;

/// A stereo buffer being built.
pub(crate) struct Buf {
    pub(crate) rate: f32,
    pub(crate) frames: Vec<[f32; 2]>,
}

/// White noise, deterministic so every run sounds the same.
pub(crate) struct Noise(pub(crate) u32);

impl Noise {
    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// A state-variable filter: band-pass and low-pass of the same input.
#[derive(Default)]
pub(crate) struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    /// Returns `(low, band)`. `q` around 0.5 is wide, 4 and up rings.
    pub(crate) fn step(&mut self, input: f32, cutoff: f32, q: f32, rate: f32) -> (f32, f32) {
        let f = (2.0 * (std::f32::consts::PI * cutoff / rate).sin()).min(0.9);
        self.low += f * self.band;
        let high = input - self.low - self.band / q;
        self.band += f * high;
        (self.low, self.band)
    }
}

/// A one-pole low-pass: 6 dB an octave. Two in a row after the band-pass take
/// the fizz off noise, which the band-pass's gentle skirt lets through.
#[derive(Default)]
pub(crate) struct OnePole(f32);

impl OnePole {
    pub(crate) fn step(&mut self, input: f32, cutoff: f32, rate: f32) -> f32 {
        self.0 += (1.0 - (-TAU * cutoff / rate).exp()) * (input - self.0);
        self.0
    }
}

/// Band-passed noise with the top rolled off above the band: soft air, no hiss.
#[derive(Default)]
pub(crate) struct Air {
    band: Svf,
    smooth: [OnePole; 2],
}

impl Air {
    pub(crate) fn step(&mut self, white: f32, cutoff: f32, q: f32, rate: f32) -> f32 {
        let b = self.band.step(white, cutoff, q, rate).1;
        let top = cutoff * 1.5;
        let once = self.smooth[0].step(b, top, rate);
        self.smooth[1].step(once, top, rate)
    }
}

/// Linear attack, exponential decay.
pub(crate) fn pluck(t: f32, attack: f32, decay: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

/// Equal-power pan, -1 left to 1 right.
pub fn pan(p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

impl Buf {
    pub(crate) fn new(rate: u32, seconds: f32) -> Buf {
        Buf {
            rate: rate as f32,
            frames: vec![[0.0; 2]; (rate as f32 * seconds) as usize],
        }
    }

    /// Adds `f(t)` (mono, `t` from `start`) at a pan position.
    pub(crate) fn add(&mut self, start: f32, position: f32, mut f: impl FnMut(f32) -> f32) {
        let (l, r) = pan(position);
        let first = (start * self.rate) as usize;
        for i in first..self.frames.len() {
            let s = f((i - first) as f32 / self.rate);
            self.frames[i][0] += s * l;
            self.frames[i][1] += s * r;
        }
    }

    /// A sine partial gliding from `f0` to `f1` over `glide` seconds, shaped by `pluck`.
    pub(crate) fn tone(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
    ) {
        let mut phase = 0.0f32;
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            // Ease out, so a glide settles into its note.
            let f = f0 + (f1 - f0) * (1.0 - (1.0 - k) * (1.0 - k));
            phase += TAU * f / rate;
            phase.sin() * pluck(t, attack, decay) * gain
        });
    }

    /// A sine shaken by another at `ratio` times its frequency, `index` deep at first and
    /// falling away over `fade`: bright at the start, a plain note by the end.
    pub(crate) fn fm(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        ratio: f32,
        index: f32,
        fade: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
    ) {
        let (mut carrier, mut shaker) = (0.0f32, 0.0f32);
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            let f = f0 + (f1 - f0) * (1.0 - (1.0 - k) * (1.0 - k));
            carrier = (carrier + TAU * f / rate) % TAU;
            shaker = (shaker + TAU * f * ratio / rate) % TAU;
            (carrier + index * (-t / fade).exp() * shaker.sin()).sin()
                * pluck(t, attack, decay)
                * gain
        });
    }

    /// A burst of band-passed noise.
    pub(crate) fn hiss(
        &mut self,
        start: f32,
        cutoff: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut air) = (Noise(seed), Air::default());
        let rate = self.rate;
        self.add(start, position, |t| {
            air.step(noise.next(), cutoff, q, rate) * pluck(t, attack, decay) * gain
        });
    }

    /// A burst of wide noise for the crack and the bark of a gun: a gentle corner
    /// below, a steep one (18 dB an octave) above. Anything left over `high_cut`
    /// is heard as static, not as a bang, so keep that low and the burst short.
    pub(crate) fn burst(
        &mut self,
        start: f32,
        low_cut: f32,
        high_cut: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut top, mut bottom) = (
            Noise(seed),
            [OnePole::default(), OnePole::default(), OnePole::default()],
            OnePole::default(),
        );
        let rate = self.rate;
        self.add(start, position, |t| {
            let mut bright = noise.next();
            for pole in &mut top {
                bright = pole.step(bright, high_cut, rate);
            }
            (bright - bottom.step(bright, low_cut, rate)) * pluck(t, attack, decay) * gain
        });
    }

    /// Noise through a band that glides from `f0` to `f1` over `glide` seconds:
    /// a report rolling away over the ground, or a shell whistling off.
    pub(crate) fn sweep(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut band, mut smooth) = (Noise(seed), Svf::default(), OnePole::default());
        let rate = self.rate;
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            // Falls fast at first, like a pitch heard going away.
            let cutoff = f1 + (f0 - f1) * (1.0 - k) * (1.0 - k);
            let b = band.step(noise.next(), cutoff, q, rate).1;
            smooth.step(b, cutoff * 2.0, rate) * pluck(t, attack, decay) * gain
        });
    }

    /// Low-passed noise under a cutoff gliding from `f0` to `f1`, its level swelling and
    /// sagging at random about `swell` times a second, `depth` deep: rolling thunder, the
    /// ground shaking. Below 22 Hz is taken out, which is felt as nothing and costs headroom.
    pub(crate) fn roll(
        &mut self,
        start: f32,
        f0: f32,
        f1: f32,
        glide: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        swell: f32,
        depth: f32,
        position: f32,
        seed: u32,
    ) {
        let (mut noise, mut band, mut smooth, mut floor) = (
            Noise(seed),
            Svf::default(),
            OnePole::default(),
            OnePole::default(),
        );
        let rate = self.rate;
        // Smooth random level: a value at every knot, eased between knots.
        let knot = |k: i64| {
            let mut n =
                Noise((seed ^ (k as u32).wrapping_mul(0x9E37_79B9)).wrapping_add(0x6D2B_79F5) | 1);
            n.next();
            n.next();
            n.next() * 0.5 + 0.5
        };
        let wander = move |x: f32| {
            let (k, f) = (x.floor(), x - x.floor());
            let e = f * f * (3.0 - 2.0 * f);
            knot(k as i64) * (1.0 - e) + knot(k as i64 + 1) * e
        };
        let depth = depth.clamp(0.0, 1.0);
        self.add(start, position, |t| {
            let k = (t / glide).min(1.0);
            let cutoff = f1 + (f0 - f1) * (1.0 - k) * (1.0 - k);
            let low = band.step(noise.next(), cutoff, 0.7, rate).0;
            let low = smooth.step(low, cutoff * 1.5, rate);
            let body = (low - floor.step(low, 22.0, rate)) * (rate / (4.0 * cutoff)).sqrt();
            // Two rates of wandering, the faster one lighter, raised to the fourth power so the
            // swells stand well out of the troughs (about 1 on average).
            let v = 0.62 * wander(t * swell) + 0.38 * wander(t * swell * 2.37 + 17.0);
            let level = 1.0 - depth + depth * 7.5 * (v * v) * (v * v);
            body * level * pluck(t, attack, decay) * gain
        });
    }

    /// The loudest sample so far.
    pub(crate) fn top(&self) -> f32 {
        self.frames
            .iter()
            .flat_map(|f| f.iter())
            .fold(1e-6f32, |m, s| m.max(s.abs()))
    }

    /// Soft saturation of the whole buffer, `amount` around 1 to 3: the loud
    /// part is squashed against the ceiling and everything under it comes up,
    /// which is most of what makes a bang sound like a bang and not like a drum.
    /// `top` is the ceiling: the buffer's own loudest sample, or the whole
    /// sound's when one layer is being heard alone.
    pub(crate) fn drive(&mut self, amount: f32, top: f32) {
        for f in &mut self.frames {
            for s in f {
                *s = (*s / top * amount).tanh() / amount.tanh();
            }
        }
    }

    /// Early reflections: a few cross-fed echoes that put a dry blip in a room.
    pub(crate) fn space(&mut self, amount: f32) {
        let n = self.frames.len();
        for (seconds, gain, swap) in [
            (0.043, 0.30, true),
            (0.089, 0.22, false),
            (0.137, 0.16, true),
            (0.211, 0.11, false),
            (0.307, 0.07, true),
        ] {
            let d = (seconds * self.rate) as usize;
            // Every tap echoes the signal as the previous taps left it.
            let dry = self.frames.clone();
            for i in d..n {
                let f = dry[i - d];
                let (a, b) = if swap { (f[1], f[0]) } else { (f[0], f[1]) };
                self.frames[i][0] += a * gain * amount;
                self.frames[i][1] += b * gain * amount;
            }
        }
    }

    /// Folds the last `fold` seconds back over the first, so the buffer can
    /// play round and round without a seam.
    pub(crate) fn fold_loop(&mut self, fold: f32) {
        let n = (fold * self.rate) as usize;
        let keep = self.frames.len().saturating_sub(n).max(2);
        for i in 0..n.min(keep) {
            let x = i as f32 / n as f32;
            for c in 0..2 {
                self.frames[i][c] = self.frames[i][c] * x + self.frames[keep + i][c] * (1.0 - x);
            }
        }
        self.frames.truncate(keep);
    }

    /// Scales to a peak level, and fades the last few milliseconds so a tail
    /// cut short by the buffer's end cannot click.
    pub(crate) fn finish(self, peak: f32) -> Vec<[f32; 2]> {
        let gain = peak / self.top();
        self.scaled(gain, true)
    }

    /// Multiplies by `gain`; `fade` takes the last few milliseconds down to
    /// nothing (a loop's end runs into its start instead).
    pub(crate) fn scaled(mut self, gain: f32, fade: bool) -> Vec<[f32; 2]> {
        let edge = if fade {
            (0.012 * self.rate) as usize
        } else {
            0
        };
        let n = self.frames.len();
        for (i, f) in self.frames.iter_mut().enumerate() {
            let e = if edge > 0 {
                ((n - 1 - i) as f32 / edge as f32).min(1.0)
            } else {
                1.0
            };
            f[0] *= gain * e;
            f[1] *= gain * e;
        }
        self.frames
    }
}
