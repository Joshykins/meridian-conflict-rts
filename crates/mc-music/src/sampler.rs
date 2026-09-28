//! A recorded note at play: read from its zone at the note's pitch (cubic
//! interpolation), looped while held, shaped by the sampler's envelope and filter.

use crate::dsp::{Adsr, Svf};
use crate::patch::{FilterMode, Sampler};
use crate::samples::{SampleSet, Zone};
use std::sync::Arc;

pub(crate) struct SampleVoice {
    set: Arc<SampleSet>,
    zone: usize,
    /// Read position in frames, and frames advanced per output sample.
    pos: f64,
    step: f64,
    vel: f32,
    amp: Adsr,
    filter: [Svf; 2],
    cut_smooth: f32,
    pub(crate) stealing: bool,
    /// Past the recording's end (or silent after release).
    finished: bool,
    age: u32,
}

impl SampleVoice {
    /// A voice for `key`, or None if the set has nothing to play there. `round`
    /// picks among several takes of one note.
    pub(crate) fn new(
        s: &Sampler,
        set: &Arc<SampleSet>,
        key: u8,
        vel: f32,
        round: u32,
        rate: f32,
    ) -> Option<SampleVoice> {
        let takes = set.pick(key, vel);
        if takes.is_empty() {
            return None;
        }
        let take = &takes[round as usize % takes.len()];
        let zone = set
            .zones
            .iter()
            .position(|z| std::ptr::eq(z, take))
            .unwrap_or(0);
        let semis = if set.pitched {
            key as f32 + s.transpose as f32 - take.key as f32 - take.cents * 0.01
        } else {
            0.0
        };
        let step = 2f64.powf(semis as f64 / 12.0) * set.rate as f64 / rate as f64;
        let mut amp = Adsr::default();
        amp.set(&s.amp, rate);
        amp.gate_on(true);
        let start = take.speak as f64 + (s.offset.max(0.0) * set.rate) as f64;
        let limit = take.looped.map_or(take.len(), |(ls, _)| ls as usize) as f64;
        Some(SampleVoice {
            set: set.clone(),
            zone,
            pos: start.min((limit - 1.0).max(0.0)),
            step,
            vel,
            amp,
            filter: [Svf::default(); 2],
            cut_smooth: 0.0,
            stealing: false,
            finished: false,
            age: 0,
        })
    }

    /// Let go: a held set dies by the release; any other rings to its end.
    pub(crate) fn release(&mut self) {
        if self.set.held {
            self.amp.gate_off();
        }
    }

    /// Cut short over a few ms: a stolen voice, the same note struck again, or stop.
    pub(crate) fn steal(&mut self, rate: f32) {
        self.stealing = true;
        self.amp.kill(rate);
    }

    pub(crate) fn done(&self) -> bool {
        self.finished || (self.amp.is_idle() && self.age > 0)
    }

    pub(crate) fn render(&mut self, s: &Sampler, out: &mut [[f32; 2]], cutoff_oct: f32, rate: f32) {
        if !self.stealing {
            // Settings may have changed under a held note.
            self.amp.set(&s.amp, rate);
        }
        let set = self.set.clone();
        let z: &Zone = &set.zones[self.zone];
        let len = z.len();
        if len < 4 {
            self.finished = true;
            return;
        }
        let gain = s.gain * (1.0 - s.velocity.clamp(0.0, 1.0) * (1.0 - self.vel));
        // Quiet notes darker by `soften` octaves; the track's cutoff automation on top.
        let open = s.cutoff <= 0.0;
        let base = if open { rate * 0.45 } else { s.cutoff };
        let oct = cutoff_oct - s.soften * (1.0 - self.vel);
        let target = (base * 2f32.powf(oct)).clamp(20.0, rate * 0.45);
        let filtering = !open || oct < -0.01;
        if self.cut_smooth <= 0.0 {
            self.cut_smooth = target;
        }
        let stereo = z.channels >= 2;
        let ch = z.channels;
        let frame = |i: usize| -> (f32, f32) {
            let at = i * ch;
            let l = z.frames[at] as f32 * (1.0 / 32768.0);
            let r = if stereo {
                z.frames[at + 1] as f32 * (1.0 / 32768.0)
            } else {
                l
            };
            (l, r)
        };
        let looped = z.looped.map(|(a, b)| (a as f64, b as f64));
        for (n, o) in out.iter_mut().enumerate() {
            if n % 16 == 0 && filtering {
                self.cut_smooth += (target - self.cut_smooth) * 0.2;
                for f in self.filter.iter_mut() {
                    f.set(self.cut_smooth, 0.0, rate);
                }
            }
            let i = self.pos as usize;
            if i + 2 >= len {
                self.finished = true;
                break;
            }
            let t = (self.pos - i as f64) as f32;
            // Four points around the read position; at a loop's end the next ones come
            // from its start, so the seam reads straight through.
            let wrap = |k: usize| -> usize {
                match looped {
                    Some((a, b)) if k >= b as usize => a as usize + (k - b as usize),
                    _ => k.min(len - 1),
                }
            };
            let p0 = frame(if i == 0 { 0 } else { i - 1 });
            let p1 = frame(i);
            let p2 = frame(wrap(i + 1));
            let p3 = frame(wrap(i + 2));
            let mut l = hermite(p0.0, p1.0, p2.0, p3.0, t);
            let mut r = hermite(p0.1, p1.1, p2.1, p3.1, t);
            if filtering {
                l = self.filter[0].process(l, FilterMode::LowPass);
                r = self.filter[1].process(r, FilterMode::LowPass);
            }
            let g = self.amp.step() * gain;
            o[0] += l * g;
            o[1] += r * g;
            self.age = self.age.saturating_add(1);
            self.pos += self.step;
            if let Some((a, b)) = looped {
                if self.pos >= b {
                    self.pos -= b - a;
                }
            }
        }
        self.filter[0].sanitise();
        self.filter[1].sanitise();
    }
}

#[inline]
fn hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + y1
}
