//! Band-limited oscillators (PolyBLEP), so a high saw does not alias into a whine.

use super::sin_cycles;
use crate::patch::Wave;
use std::f32::consts::TAU;

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// One oscillator's running state. `phase` is 0..1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Phase {
    pub phase: f32,
    /// The FM modulator's phase.
    pub mod_phase: f32,
    /// Leaky integrator state for the triangle.
    tri: f32,
}

impl Phase {
    pub fn with(phase: f32) -> Phase {
        let tri = if phase < 0.5 {
            -1.0 + 4.0 * phase
        } else {
            3.0 - 4.0 * phase
        };
        Phase {
            phase,
            mod_phase: 0.0,
            tri,
        }
    }

    /// One sample of `wave` at `dt` cycles per sample. `shape` is 0..1; `fm_depth` scales
    /// the FM index; `noise` is a fresh -1..1 value used by `Noise`.
    #[inline]
    pub fn next(
        &mut self,
        wave: Wave,
        dt: f32,
        shape: f32,
        ratio: f32,
        fm_depth: f32,
        noise: f32,
    ) -> f32 {
        let dt = dt.clamp(0.0, 0.45);
        let t = self.phase;
        let out = match wave {
            Wave::Sine => sin_cycles(t),
            Wave::Saw => 2.0 * t - 1.0 - poly_blep(t, dt),
            Wave::Square | Wave::Pulse => {
                let w = if wave == Wave::Square {
                    0.5
                } else {
                    shape.clamp(0.03, 0.97)
                };
                let mut v = if t < w { 1.0 } else { -1.0 };
                v += poly_blep(t, dt);
                let t2 = (t + 1.0 - w) % 1.0;
                v -= poly_blep(t2, dt);
                v
            }
            Wave::Triangle => {
                // A band-limited square integrated: no corners to alias.
                let mut sq = if t < 0.5 { 1.0 } else { -1.0 };
                sq += poly_blep(t, dt);
                sq -= poly_blep((t + 0.5) % 1.0, dt);
                self.tri = dt * sq * 4.0 + (1.0 - dt * 0.05) * self.tri;
                self.tri
            }
            Wave::Fm => {
                let m = sin_cycles(self.mod_phase);
                self.mod_phase += dt * ratio;
                self.mod_phase -= self.mod_phase.floor();
                let index = shape.clamp(0.0, 1.0) * 8.0 * fm_depth;
                sin_cycles(t + m * index / TAU)
            }
            Wave::Fold => {
                let drive = 1.0 + shape.clamp(0.0, 1.0) * 6.0 * fm_depth.max(0.25);
                let x = sin_cycles(t) * drive;
                // Triangle fold: bounce off ±1.
                let y = (x + 1.0).rem_euclid(4.0);
                (if y < 2.0 { y - 1.0 } else { 3.0 - y }) * 0.9
            }
            Wave::Noise => noise,
        };
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }

    /// Runs this oscillator over a block: `hz` per sample (cycles per sample after
    /// `ratio`), mixed into `l`/`r` at `place` gains. The wave is chosen once for the
    /// block, so the inner loops carry no branch on it.
    #[inline]
    pub fn run(
        &mut self,
        wave: Wave,
        hz: &[f32],
        ratio: f32,
        shape: f32,
        fm_ratio: f32,
        depth: &[f32],
        rng: &mut super::Rng,
        l: &mut [f32],
        r: &mut [f32],
        place: (f32, f32),
    ) {
        let n = hz.len();
        match wave {
            Wave::Saw => {
                for i in 0..n {
                    let dt = (hz[i] * ratio).clamp(0.0, 0.45);
                    let t = self.phase;
                    let v = 2.0 * t - 1.0 - poly_blep(t, dt);
                    self.phase += dt;
                    if self.phase >= 1.0 {
                        self.phase -= 1.0;
                    }
                    l[i] += v * place.0;
                    r[i] += v * place.1;
                }
            }
            Wave::Sine => {
                for i in 0..n {
                    let dt = (hz[i] * ratio).clamp(0.0, 0.45);
                    let v = sin_cycles(self.phase);
                    self.phase += dt;
                    if self.phase >= 1.0 {
                        self.phase -= 1.0;
                    }
                    l[i] += v * place.0;
                    r[i] += v * place.1;
                }
            }
            Wave::Noise => {
                for i in 0..n {
                    let v = rng.bipolar();
                    l[i] += v * place.0;
                    r[i] += v * place.1;
                }
            }
            _ => {
                for i in 0..n {
                    let v = self.next(wave, hz[i] * ratio, shape, fm_ratio, depth[i], 0.0);
                    l[i] += v * place.0;
                    r[i] += v * place.1;
                }
            }
        }
    }
}
