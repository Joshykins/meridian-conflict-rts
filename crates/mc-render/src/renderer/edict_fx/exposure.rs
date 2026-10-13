//! The Edict's hold on the scene's light, as the tone map's exposure (`TONEMAP`, frame.rs):
//! as a star blazes the scene round it dims a little; at the shot a short white-out (the
//! one thing allowed to white the screen out, approved 2026-10-13 against NUKES.md's
//! rule), full within a tenth of a second and gone within a third; then the day darkens
//! (to under half the exposure, about half as bright once tone-mapped) and comes back over
//! six seconds. Never a hard flash that hurts, never black. Each is weighed by how near
//! the camera's focus is to it. The shot's own light on the country is kept short
//! (`shot.rs`) so it does not light up the dark it leaves.

use glam::Vec3;

/// How far from the camera's focus (metres) a glare is half felt.
const HALF_FELT: f32 = 9000.0;
/// The white-out at its height, times the scene's exposure; how fast it comes up, how
/// long it holds and how fast it falls (seconds).
const WHITE: f32 = 7.0;
const RISE: f32 = 0.07;
const HOLD: f32 = 0.06;
const FALL: f32 = 0.09;
/// The darkness after it: the share of the light taken at its deepest, how long until it
/// is deepest and until it is gone.
const DARK: f32 = 0.62;
const DARKEST: f32 = 0.45;
const BACK: f32 = 6.5;
/// The dimming round a blazing star at its fullest.
const STAR_DIM: f32 = 0.18;
/// At most this many shots are remembered (a deliberate cap: past it the oldest is
/// forgotten; their light is long back by then).
const MOST: usize = 8;

/// A shot's glare: where, and when.
#[derive(Clone, Copy)]
struct Glare {
    at: Vec3,
    start: f32,
}

#[derive(Default)]
pub(in crate::renderer) struct Exposure {
    glares: Vec<Glare>,
    /// This frame's factor on the tone map's exposure.
    value: f32,
}

/// How much of a glare at `at` the camera looking at `focus` feels, zero to one.
fn felt(at: Vec3, focus: Vec3) -> f32 {
    let d = at.truncate().distance(focus.truncate()) / HALF_FELT;
    1.0 / (1.0 + d * d)
}

/// The exposure factor `age` seconds after a shot felt `w`.
fn after_shot(age: f32, w: f32) -> f32 {
    if age < 0.0 {
        return 1.0;
    }
    let up = (age / RISE).clamp(0.0, 1.0);
    let white = up * up * (3.0 - 2.0 * up) * (-(age - RISE - HOLD).max(0.0) / FALL).exp();
    // The dark comes in under the white and eases back.
    let k = (age / DARKEST).clamp(0.0, 1.0);
    let into = k * k * (3.0 - 2.0 * k);
    let back = ((age - DARKEST) / (BACK - DARKEST)).clamp(0.0, 1.0);
    let out = back * back * (3.0 - 2.0 * back);
    let dark = DARK * into * (1.0 - out);
    (1.0 + (WHITE - 1.0) * white * w) * (1.0 - dark * w)
}

impl Exposure {
    pub(in crate::renderer) fn clear(&mut self) {
        self.glares.clear();
    }

    /// A shot from `from` at `mark`, at `start`.
    pub(super) fn glare(&mut self, from: Vec3, mark: Vec3, start: f32) {
        if self.glares.len() >= MOST {
            self.glares.remove(0);
        }
        self.glares.push(Glare {
            at: (from + mark) * 0.5,
            start,
        });
    }

    /// This frame's factor, from the shots and the stars now gathering (where, how full).
    pub(super) fn frame(
        &mut self,
        time: f32,
        focus: Vec3,
        stars: impl Iterator<Item = (Vec3, f32)>,
    ) {
        self.glares.retain(|g| time - g.start < BACK + 1.0);
        let mut value = 1.0;
        for (at, share) in stars {
            value *= 1.0 - STAR_DIM * share.powi(3) * felt(at, focus);
        }
        for g in &self.glares {
            value *= after_shot(time - g.start, felt(g.at, focus));
        }
        self.value = value;
    }

    /// The factor on the tone map's exposure this frame: one when nothing is at work.
    pub(in crate::renderer) fn value(&self) -> f32 {
        if self.value > 0.0 {
            self.value
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_white_out_is_short_and_the_dark_never_black() {
        // Full within a tenth of a second, mostly gone by a third.
        assert!(after_shot(0.1, 1.0) > 5.0);
        assert!(after_shot(0.35, 1.0) < 1.5);
        let mut lowest = f32::MAX;
        let mut t = 0.0;
        while t < BACK + 1.0 {
            lowest = lowest.min(after_shot(t, 1.0));
            t += 0.01;
        }
        assert!((0.33..0.5).contains(&lowest), "darkest {lowest}");
        assert!((after_shot(BACK + 0.5, 1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn far_off_it_is_barely_felt() {
        assert!(felt(Vec3::ZERO, Vec3::new(40_000.0, 0.0, 0.0)) < 0.06);
        assert!(felt(Vec3::ZERO, Vec3::new(1000.0, 0.0, 0.0)) > 0.95);
    }
}
