//! Filters: a zero-delay-feedback state-variable filter for synth voices and
//! sweeps (stable under fast modulation), and RBJ biquads for the EQ.

use crate::patch::FilterMode;
use std::f32::consts::PI;

/// Andrew Simper's trapezoidal SVF. Cutoff can move every sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl Svf {
    /// `resonance` 0..1 maps to damping 2 (none) .. 0.05 (near self-oscillation).
    #[inline]
    pub fn set(&mut self, cutoff: f32, resonance: f32, rate: f32) {
        let fc = cutoff.clamp(16.0, rate * 0.49);
        self.g = (PI * fc / rate).tan();
        self.k = 2.0 - 1.95 * resonance.clamp(0.0, 1.0);
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    /// Low, band and high outputs at once.
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        let high = x - self.k * v1 - v2;
        (v2, v1, high)
    }

    #[inline]
    pub fn process(&mut self, x: f32, mode: FilterMode) -> f32 {
        let (low, band, high) = self.tick(x);
        match mode {
            FilterMode::LowPass | FilterMode::LowPass4 => low,
            FilterMode::BandPass => band * self.k,
            FilterMode::HighPass => high,
            FilterMode::Notch => low + high,
            FilterMode::Formant => band * self.k,
        }
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    /// Guards a filter that has been driven into denormals or blown up by NaN.
    #[inline]
    pub fn sanitise(&mut self) {
        if !self.ic1.is_finite() || !self.ic2.is_finite() {
            self.reset();
        }
        if self.ic1.abs() < 1e-20 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-20 {
            self.ic2 = 0.0;
        }
    }
}

/// A filter with its mode, as the synth and the effects use it: two stages for `LowPass4`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModeFilter {
    a: Svf,
    b: Svf,
    /// The voice filter's three formants.
    formants: [Svf; 3],
    formant_gain: [f32; 3],
    /// Vowel for `Formant`, 0..1 (see `patch::Filter::vowel`).
    pub vowel: f32,
}

/// A man's formants (Hz) and their levels, for "ah", "eh", "ee", "oh", "oo".
const VOWELS: [([f32; 3], [f32; 3]); 5] = [
    ([700.0, 1150.0, 2600.0], [1.0, 0.63, 0.25]),
    ([450.0, 1700.0, 2500.0], [1.0, 0.45, 0.3]),
    ([290.0, 2000.0, 2700.0], [1.0, 0.2, 0.25]),
    ([450.0, 800.0, 2600.0], [1.0, 0.5, 0.15]),
    ([330.0, 700.0, 2450.0], [1.0, 0.3, 0.1]),
];

impl ModeFilter {
    #[inline]
    pub fn set(&mut self, mode: FilterMode, cutoff: f32, resonance: f32, rate: f32) {
        if mode == FilterMode::Formant {
            let v = self.vowel.clamp(0.0, 1.0) * 4.0;
            let i = (v as usize).min(3);
            let t = v - i as f32;
            let (fa, ga) = VOWELS[i];
            let (fb, gb) = VOWELS[i + 1];
            let size = (cutoff / 1000.0).clamp(0.4, 3.0);
            // Formants stay narrow however the resonance is set: that is what makes it a vowel.
            let res = 0.75 + 0.24 * resonance.clamp(0.0, 1.0);
            for k in 0..3 {
                let f = (fa[k] + (fb[k] - fa[k]) * t) * size;
                self.formants[k].set(f, res, rate);
                self.formant_gain[k] = ga[k] + (gb[k] - ga[k]) * t;
            }
            return;
        }
        if mode == FilterMode::LowPass4 {
            // The resonance lives on the second stage; the first stays flat so the peak stays musical.
            self.a.set(cutoff, 0.0, rate);
            self.b.set(cutoff, resonance, rate);
        } else {
            self.a.set(cutoff, resonance, rate);
        }
    }
    #[inline]
    pub fn process(&mut self, x: f32, mode: FilterMode) -> f32 {
        if mode == FilterMode::Formant {
            let mut y = 0.0;
            for k in 0..3 {
                y += self.formants[k].process(x, FilterMode::BandPass) * self.formant_gain[k];
            }
            return y * 1.6;
        }
        let y = self.a.process(x, mode);
        if mode == FilterMode::LowPass4 {
            self.b.process(y, FilterMode::LowPass)
        } else {
            y
        }
    }
    pub fn sanitise(&mut self) {
        self.a.sanitise();
        self.b.sanitise();
        for f in self.formants.iter_mut() {
            f.sanitise();
        }
    }
    pub fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }
}

/// RBJ cookbook biquad, transposed direct form II.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Biquad {
        Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiquadKind {
    LowShelf,
    HighShelf,
    Peak,
    HighPass,
    LowPass,
}

impl Biquad {
    pub fn set(&mut self, kind: BiquadKind, freq: f32, q: f32, db: f32, rate: f32) {
        let w0 = 2.0 * PI * freq.clamp(10.0, rate * 0.49) / rate;
        let (sin, cos) = w0.sin_cos();
        let q = q.max(0.05);
        let alpha = sin / (2.0 * q);
        let a = 10f32.powf(db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::Peak => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            BiquadKind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - s),
                    (a + 1.0) + (a - 1.0) * cos + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - s,
                )
            }
            BiquadKind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - s),
                    (a + 1.0) - (a - 1.0) * cos + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - s,
                )
            }
            BiquadKind::HighPass => (
                (1.0 + cos) / 2.0,
                -(1.0 + cos),
                (1.0 + cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
            BiquadKind::LowPass => (
                (1.0 - cos) / 2.0,
                1.0 - cos,
                (1.0 - cos) / 2.0,
                1.0 + alpha,
                -2.0 * cos,
                1.0 - alpha,
            ),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Magnitude response at `freq`, for drawing the EQ curve.
    pub fn magnitude(&self, freq: f32, rate: f32) -> f32 {
        let w = 2.0 * PI * freq / rate;
        let (c1, s1) = (w.cos(), -w.sin());
        let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        ((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-12)).sqrt()
    }

    pub fn sanitise(&mut self) {
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
    }
}

/// One-pole low-pass, for tone controls and smoothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    pub z: f32,
    a: f32,
}

impl OnePole {
    pub fn set(&mut self, cutoff: f32, rate: f32) {
        self.a = 1.0 - (-2.0 * PI * cutoff.clamp(1.0, rate * 0.49) / rate).exp();
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.z += (x - self.z) * self.a;
        self.z
    }
}

/// Blocks DC with a pole at ~10 Hz.
#[derive(Clone, Copy, Debug, Default)]
pub struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + 0.9987 * self.y1;
        self.x1 = x;
        self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        y
    }
}
