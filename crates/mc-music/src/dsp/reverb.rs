//! A hall: input diffusion, then an eight-line feedback delay network with a
//! Householder mix, per-line damping and slow modulation so long tails do not
//! ring metallic.

use super::effects::Line;
use super::filter::{Biquad, BiquadKind, OnePole};
use std::f32::consts::TAU;

const LINES: usize = 8;
/// Line lengths in ms at size 1, mutually prime-ish.
const LENGTHS_MS: [f32; LINES] = [47.3, 53.9, 61.7, 68.3, 77.1, 83.9, 91.3, 101.7];
const DIFFUSERS_MS: [f32; 4] = [4.7, 6.1, 8.3, 11.9];

struct Allpass {
    line: Line,
    delay: usize,
    g: f32,
}

impl Allpass {
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let d = self.line.read_int(self.delay);
        let v = x + d * self.g;
        self.line.push(v);
        d - v * self.g
    }
}

pub struct Hall {
    rate: f32,
    lines: Vec<Line>,
    lengths: [f32; LINES],
    gains: [f32; LINES],
    damp: [OnePole; LINES],
    diffusers: [Vec<Allpass>; 2],
    pre: [Line; 2],
    pre_delay: usize,
    lowcut: [Biquad; 2],
    /// The modulation LFO as a unit phasor (cos, sin), turned a step each sample.
    lfo: (f32, f32),
    /// Each line's phase offset, as (cos, sin).
    offsets: [(f32, f32); LINES],
    samples: u32,
}

impl Hall {
    pub fn new(rate: f32) -> Hall {
        let max = (rate * 0.21) as usize;
        let mk = |side: usize| {
            DIFFUSERS_MS
                .iter()
                .enumerate()
                .map(|(i, ms)| {
                    let d = (ms * (1.0 + 0.07 * side as f32 + 0.013 * i as f32) * 0.001 * rate)
                        as usize;
                    Allpass {
                        line: Line::new(d + 4),
                        delay: d.max(1),
                        g: 0.62,
                    }
                })
                .collect::<Vec<_>>()
        };
        Hall {
            rate,
            lines: (0..LINES).map(|_| Line::new(max)).collect(),
            lengths: [1000.0; LINES],
            gains: [0.0; LINES],
            damp: Default::default(),
            diffusers: [mk(0), mk(1)],
            pre: [
                Line::new((rate * 0.25) as usize),
                Line::new((rate * 0.25) as usize),
            ],
            pre_delay: 1,
            lowcut: Default::default(),
            lfo: (1.0, 0.0),
            offsets: std::array::from_fn(|i| {
                let a = i as f32 * 0.125 * TAU;
                (a.cos(), a.sin())
            }),
            samples: 0,
        }
    }

    pub fn set(&mut self, size: f32, decay: f32, damp: f32, predelay_ms: f32, lowcut: f32) {
        let scale = 0.35 + 1.65 * size.clamp(0.0, 1.0);
        let rt60 = decay.max(0.1);
        for (i, &ms) in LENGTHS_MS.iter().enumerate() {
            self.lengths[i] = ms * scale * 0.001 * self.rate;
            let secs = self.lengths[i] / self.rate;
            // Each pass through a line loses its share of 60 dB over rt60.
            self.gains[i] = 10f32.powf(-3.0 * secs / rt60);
            self.damp[i].set(damp, self.rate);
        }
        self.pre_delay =
            ((predelay_ms.max(0.0) * 0.001 * self.rate) as usize).clamp(1, self.pre[0].len() - 2);
        for c in 0..2 {
            self.lowcut[c].set(BiquadKind::HighPass, lowcut.max(20.0), 0.6, 0.0, self.rate);
        }
    }

    pub fn run(&mut self, buf: &mut [[f32; 2]], mix: f32) {
        let step = 0.13 / self.rate * TAU;
        let (sc, ss) = (step.cos(), step.sin());
        let depth = 0.0009 * self.rate;
        let mut outs = [0.0f32; LINES];
        for f in buf.iter_mut() {
            let (c, s) = self.lfo;
            self.lfo = (c * sc - s * ss, s * sc + c * ss);
            self.samples = self.samples.wrapping_add(1);
            if self.samples.is_multiple_of(4096) {
                // Keep the phasor on the unit circle.
                let n = (self.lfo.0 * self.lfo.0 + self.lfo.1 * self.lfo.1)
                    .sqrt()
                    .max(1e-6);
                self.lfo = (self.lfo.0 / n, self.lfo.1 / n);
            }
            let mut ins = [0.0f32; 2];
            for c in 0..2 {
                self.pre[c].push(f[c]);
                let mut x = self.lowcut[c].process(self.pre[c].read_int(self.pre_delay));
                for a in self.diffusers[c].iter_mut() {
                    x = a.process(x);
                }
                ins[c] = x;
            }
            for (i, o) in outs.iter_mut().enumerate() {
                let (oc, os) = self.offsets[i];
                let m = (self.lfo.1 * oc + self.lfo.0 * os) * depth * (1.0 + (i % 3) as f32 * 0.3);
                let v = self.lines[i].read_linear(self.lengths[i] + m);
                *o = self.damp[i].process(v) * self.gains[i];
            }
            // Householder reflection: x - (2/N) sum(x).
            let sum: f32 = outs.iter().sum::<f32>() * (2.0 / LINES as f32);
            for (i, &out) in outs.iter().enumerate() {
                let feed = out - sum;
                let input = if i % 2 == 0 { ins[0] } else { ins[1] };
                self.lines[i].push(feed + input * 0.5);
            }
            let wl = (outs[0] - outs[2] + outs[4] - outs[6]) * 0.6;
            let wr = (outs[1] - outs[3] + outs[5] - outs[7]) * 0.6;
            f[0] = f[0] * (1.0 - mix) + wl * mix;
            f[1] = f[1] * (1.0 - mix) + wr * mix;
        }
        for c in 0..2 {
            self.lowcut[c].sanitise();
        }
        for d in self.damp.iter_mut() {
            if !d.z.is_finite() || d.z.abs() < 1e-20 {
                d.z = 0.0;
            }
        }
    }
}
