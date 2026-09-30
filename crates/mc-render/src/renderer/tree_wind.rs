//! Trees answering the air. The wind comes straight from the weather in the
//! vertex shader (entity.wgsl `tree_air`); blasts are kept here as their
//! pressure fronts go out (`push_shockwave`) and handed over in
//! `Globals::tree_blasts`, the newest ones that can reach something on screen.
//! A shield stops both: the shader tests every blast against the live barriers
//! on its way to a tree, and a tree under a dome stands in still air.
//!
//! In a barrage there are more blasts than the shader takes. The ones handed over
//! are those still moving trees the most, not just the newest: a big blast whose
//! front is still running out must not be dropped for a spray of small shells, or
//! every tree it holds bent snaps upright at once. A blast shown last frame keeps
//! its place against one only a little stronger, so two alike never swap in and
//! out frame by frame.

use glam::{Vec3, Vec4};

/// Blasts the shader looks at, at most. Two vec4 each in `Globals`.
pub(super) const TREE_BLASTS: usize = 24;
/// Seconds a tree is still swinging after the front has passed.
const REMEMBER: f32 = 3.5;
/// Blasts kept waiting to be drawn, at most; the oldest go first.
const MOST: usize = 256;
/// How fast the push runs out through the trees, m/s.
pub(super) const FRONT_SPEED: f32 = 140.0;
/// How fast a tree's swing dies away once the front has passed, 1/s (cull.wgsl
/// `tree_blast`'s spring).
const SETTLE: f32 = 1.9;
/// How much stronger a blast must be to take a place from one shown last frame.
const KEEP: f32 = 1.5;

#[derive(Clone, Copy)]
struct Blast {
    at: Vec3,
    start: f32,
    /// How far out it bends trees, metres.
    range: f32,
    /// How far it throws a 10 m tree's top at point blank, metres.
    force: f32,
    /// How fast its front runs through the trees, m/s.
    speed: f32,
    /// Handed to the shader last frame.
    shown: bool,
}

impl Blast {
    /// How much it still moves trees at `time`: its full force while its front is
    /// still running out (or yet to go off), then dying away as the last trees it
    /// reached settle.
    fn stir(&self, time: f32) -> f32 {
        let past = time - self.start - self.range / self.speed;
        let keep = if self.shown { KEEP } else { 1.0 };
        self.force * (-SETTLE * past.max(0.0)).exp() * keep
    }
}

#[derive(Default)]
pub(super) struct TreeBlasts {
    live: Vec<Blast>,
    /// Scratch for `upload`: the blasts that could be on screen, strongest first.
    order: Vec<(f32, usize)>,
}

impl TreeBlasts {
    /// A pressure front of `reach` metres going out at `start`. A muzzle or
    /// motor blast (`directed`) is a narrow jet, not a sphere: it only stirs
    /// what is close.
    pub(super) fn record(
        &mut self,
        at: Vec3,
        start: f32,
        reach: f32,
        strength: f32,
        directed: bool,
    ) {
        let strength = strength.clamp(0.0, 1.0);
        if reach < 6.0 || strength < 0.05 {
            return;
        }
        let (range, force) = if directed {
            ((reach * 0.9).min(60.0), strength * 0.6)
        } else {
            (
                (reach * 2.6).min(900.0),
                strength * (reach / 30.0).clamp(0.5, 3.0) * 1.6,
            )
        };
        if self.live.len() >= MOST {
            self.live.remove(0);
        }
        self.live.push(Blast {
            at,
            start,
            range,
            force,
            speed: FRONT_SPEED,
            shown: false,
        });
    }

    /// A front far bigger than any gun's (a nuclear blast): `range` metres, `force` at a
    /// 10 m tree's top, running out at `speed` m/s, with none of `record`'s caps.
    pub(super) fn record_wide(&mut self, at: Vec3, start: f32, range: f32, force: f32, speed: f32) {
        if self.live.len() >= MOST {
            self.live.remove(0);
        }
        self.live.push(Blast {
            at,
            start,
            range,
            force,
            speed,
            shown: false,
        });
    }

    /// The blasts moving trees the most that could be on screen, packed for
    /// `Globals::tree_blasts`: `[x, y, z, start]`, `[range, force, speed, 0]`.
    pub(super) fn upload(
        &mut self,
        time: f32,
        frustum: &[Vec4; 6],
    ) -> (u32, [[f32; 4]; TREE_BLASTS * 2]) {
        self.live
            .retain(|b| time - b.start < REMEMBER + b.range / b.speed);
        self.order.clear();
        for (i, b) in self.live.iter().enumerate() {
            let seen = b.start <= time + 2.0
                && frustum
                    .iter()
                    .take(5)
                    .all(|p| p.truncate().dot(b.at) + p.w > -b.range);
            if seen {
                self.order.push((b.stir(time), i));
            }
        }
        // Strongest first; alike ones newest first.
        self.order
            .sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
        for b in &mut self.live {
            b.shown = false;
        }
        let mut out = [[0.0; 4]; TREE_BLASTS * 2];
        let n = self.order.len().min(TREE_BLASTS);
        for (slot, &(_, i)) in self.order.iter().take(n).enumerate() {
            let b = &mut self.live[i];
            b.shown = true;
            out[slot * 2] = [b.at.x, b.at.y, b.at.z, b.start];
            out[slot * 2 + 1] = [b.range, b.force, b.speed, 0.0];
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
    fn a_big_front_still_running_outlasts_a_spray_of_shells() {
        let mut t = TreeBlasts::default();
        // A heavy blast whose front takes seconds to run out...
        t.record(Vec3::ZERO, 0.0, 300.0, 1.0, false);
        t.upload(0.0, &everywhere());
        // ...then more small shells than the shader takes, all newer.
        for i in 0..TREE_BLASTS + 5 {
            t.record(Vec3::new(i as f32, 0.0, 0.0), 1.0, 10.0, 0.3, false);
        }
        let (n, packed) = t.upload(1.2, &everywhere());
        assert_eq!(n as usize, TREE_BLASTS);
        assert!(packed.iter().step_by(2).any(|b| b[3] == 0.0));
    }

    #[test]
    fn a_shown_blast_keeps_its_place_against_one_alike() {
        let mut t = TreeBlasts::default();
        for i in 0..TREE_BLASTS {
            t.record(Vec3::new(i as f32, 0.0, 0.0), 0.0, 30.0, 1.0, false);
        }
        t.upload(0.1, &everywhere());
        // Newer and a little stronger, but not enough to push one out.
        t.record(Vec3::new(-1.0, 0.0, 0.0), 0.1, 30.0, 1.0, false);
        t.live.last_mut().unwrap().force *= 1.2;
        let (_, packed) = t.upload(0.2, &everywhere());
        assert!(packed.iter().step_by(2).all(|b| b[0] != -1.0));
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
