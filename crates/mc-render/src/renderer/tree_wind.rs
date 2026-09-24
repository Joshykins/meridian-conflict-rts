//! Trees answering the air. The wind comes straight from the weather in the
//! vertex shader (entity.wgsl `tree_air`); blasts are kept here as their
//! pressure fronts go out (`push_shockwave`) and handed over in
//! `Globals::tree_blasts`, the newest ones that can reach something on screen.
//! A shield stops both: the shader tests every blast against the live barriers
//! on its way to a tree, and a tree under a dome stands in still air.

use glam::{Vec3, Vec4};

/// Blasts the shader looks at, at most. Two vec4 each in `Globals`.
pub const TREE_BLASTS: usize = 24;
/// Seconds a tree is still swinging after the front has passed.
const REMEMBER: f32 = 3.5;
/// Blasts kept waiting to be drawn, at most; the oldest go first.
const MOST: usize = 256;
/// How fast the push runs out through the trees, m/s.
pub const FRONT_SPEED: f32 = 140.0;

#[derive(Clone, Copy)]
struct Blast {
    at: Vec3,
    start: f32,
    /// How far out it bends trees, metres.
    range: f32,
    /// How far it throws a 10 m tree's top at point blank, metres.
    force: f32,
}

#[derive(Default)]
pub(super) struct TreeBlasts {
    live: Vec<Blast>,
}

impl TreeBlasts {
    /// A pressure front of `reach` metres going out at `start`. A muzzle or
    /// motor blast (`directed`) is a narrow jet, not a sphere: it only stirs
    /// what is close.
    pub(super) fn record(&mut self, at: Vec3, start: f32, reach: f32, strength: f32, directed: bool) {
        let strength = strength.clamp(0.0, 1.0);
        if reach < 6.0 || strength < 0.05 {
            return;
        }
        let (range, force) = if directed {
            ((reach * 0.9).min(60.0), strength * 0.6)
        } else {
            ((reach * 2.6).min(900.0), strength * (reach / 30.0).clamp(0.5, 3.0) * 1.6)
        };
        if self.live.len() >= MOST {
            self.live.remove(0);
        }
        self.live.push(Blast { at, start, range, force });
    }

    /// The newest blasts still moving trees that could be on screen, packed
    /// for `Globals::tree_blasts`: `[x, y, z, start]`, `[range, force, 0, 0]`.
    pub(super) fn upload(&mut self, time: f32, frustum: &[Vec4; 6]) -> (u32, [[f32; 4]; TREE_BLASTS * 2]) {
        self.live
            .retain(|b| time - b.start < REMEMBER + b.range / FRONT_SPEED);
        let mut out = [[0.0; 4]; TREE_BLASTS * 2];
        let mut n = 0;
        for b in self.live.iter().rev() {
            if n == TREE_BLASTS {
                break;
            }
            if b.start > time + 2.0 {
                continue;
            }
            let seen = frustum
                .iter()
                .take(5)
                .all(|p| p.truncate().dot(b.at) + p.w > -b.range);
            if !seen {
                continue;
            }
            out[n * 2] = [b.at.x, b.at.y, b.at.z, b.start];
            out[n * 2 + 1] = [b.range, b.force, 0.0, 0.0];
            n += 1;
        }
        (n as u32, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn everywhere() -> [Vec4; 6] {
        [Vec4::new(0.0, 0.0, 1.0, 1.0e6); 6]
    }

    #[test]
    fn small_puffs_are_ignored_and_old_blasts_expire() {
        let mut t = TreeBlasts::default();
        t.record(Vec3::ZERO, 0.0, 3.0, 1.0, false);
        t.record(Vec3::ZERO, 0.0, 40.0, 0.8, false);
        assert_eq!(t.upload(0.5, &everywhere()).0, 1);
        assert_eq!(t.upload(30.0, &everywhere()).0, 0);
    }

    #[test]
    fn newest_blasts_win_when_there_are_too_many() {
        let mut t = TreeBlasts::default();
        for i in 0..TREE_BLASTS + 10 {
            t.record(Vec3::new(i as f32, 0.0, 0.0), 1.0, 30.0, 1.0, false);
        }
        let (n, packed) = t.upload(1.0, &everywhere());
        assert_eq!(n as usize, TREE_BLASTS);
        assert_eq!(packed[0][0], (TREE_BLASTS + 9) as f32);
    }

    #[test]
    fn blasts_off_screen_are_skipped() {
        let mut t = TreeBlasts::default();
        t.record(Vec3::new(5000.0, 0.0, 0.0), 0.0, 20.0, 1.0, false);
        // Everything with x > 100 is outside.
        let mut planes = everywhere();
        planes[0] = Vec4::new(-1.0, 0.0, 0.0, 100.0);
        assert_eq!(t.upload(0.1, &planes).0, 0);
    }
}
