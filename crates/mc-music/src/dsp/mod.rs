//! Signal processing building blocks: oscillators, filters, envelopes, and the
//! insert effects. Everything is per sample and allocation-free once built.

pub mod effects;
pub mod filter;
pub mod osc;
pub mod reverb;

pub use filter::{Biquad, Svf};

/// sin(2 pi x) for `x` in cycles, any value: a 9th-order polynomial on the folded quarter
/// wave, within 4e-6 of the real thing and several times cheaper than `f32::sin`.
#[inline]
pub fn sin_cycles(x: f32) -> f32 {
    let mut t = x - x.round();
    if t > 0.25 {
        t = 0.5 - t;
    } else if t < -0.25 {
        t = -0.5 - t;
    }
    let z = t * std::f32::consts::TAU;
    let z2 = z * z;
    z * (1.0 + z2 * (-1.0 / 6.0 + z2 * (1.0 / 120.0 + z2 * (-1.0 / 5040.0 + z2 * (1.0 / 362_880.0)))))
}

/// A small, fast, deterministic noise source.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed.max(1))
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// -1..1.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }
    /// 0..1.
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -96.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

#[inline]
pub fn gain_to_db(g: f32) -> f32 {
    if g <= 1e-6 {
        -120.0
    } else {
        20.0 * g.log10()
    }
}

#[inline]
pub fn midi_hz(key: f32) -> f32 {
    440.0 * 2f32.powf((key - 69.0) / 12.0)
}

/// Equal-power pan: -1 left .. 1 right, (left, right) gains, both 0.707 at centre.
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

/// One-pole smoothing coefficient for a time constant of `seconds`.
#[inline]
pub fn one_pole(seconds: f32, rate: f32) -> f32 {
    if seconds <= 0.0 {
        1.0
    } else {
        1.0 - (-1.0 / (seconds * rate)).exp()
    }
}

/// A soft saturator that is linear near zero and approaches ±1.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    // Rational tanh approximation, exact at 0, error < 0.3% and bounded.
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

/// Linear ADSR with exponential decay and release, like an analogue one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Adsr {
    pub level: f32,
    stage: Stage,
    attack_step: f32,
    decay_coef: f32,
    release_coef: f32,
    sustain: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Release,
}

impl Adsr {
    pub fn set(&mut self, env: &crate::patch::Env, rate: f32) {
        self.attack_step = 1.0 / (env.a.max(0.0005) * rate);
        // Decay and release reach about a thousandth in their stated time.
        self.decay_coef = (-6.9 / (env.d.max(0.001) * rate)).exp();
        self.release_coef = (-6.9 / (env.r.max(0.002) * rate)).exp();
        self.sustain = env.s.clamp(0.0, 1.0);
    }
    pub fn gate_on(&mut self, restart: bool) {
        if restart {
            self.level = 0.0;
        }
        self.stage = Stage::Attack;
    }
    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }
    /// Cut short fast (a stolen voice or a choke): release over a few ms.
    pub fn kill(&mut self, rate: f32) {
        self.release_coef = (-6.9 / (0.004 * rate)).exp();
        self.gate_off();
    }
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }
    #[inline]
    pub fn step(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.attack_step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coef;
            }
            Stage::Release => {
                self.level *= self.release_coef;
                if self.level < 1e-4 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}
