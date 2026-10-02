//! A cluster shot breaking into its sub-shots (`SimEvent::ClusterSplit`, `Weapon::cluster`):
//! the cage parting, a white heart over at once in a red flash, and sparks thrown out round
//! the way it was flying. Hard-edged and short-lived: no smoke, no mist. The shot and its
//! pieces are drawn in flight and where they land as any Gravitic Seeker (`gravitic_fx`).

use glam::Vec3;

use super::{rail_fx, Renderer, PUFF_SPARK};

/// The red flash kind (`push_effect`), as a gun whose tracers run red flashes.
const RED_FLASH: f32 = 8.0;

impl Renderer {
    /// A cluster shot breaking into `count` sub-shots at `at`, flying at `vel` (metres a
    /// second).
    pub(super) fn cluster_split(&mut self, at: Vec3, vel: Vec3, count: u8, time: f32) {
        self.push_effect(at.to_array(), time, 3.0, 0.08, rail_fx::RAIL_FLASH, 0.0);
        self.push_effect(at.to_array(), time, 7.0, 0.16, RED_FLASH, 0.0);
        // A deliberate cosmetic cap: a few sparks a sub-shot.
        for _ in 0..(count as usize * 3).min(48) {
            let spray = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or_zero();
            let speed = 15.0 + self.scatter.unit() * 25.0;
            self.push_puff(
                PUFF_SPARK,
                at,
                vel * 0.6 + spray * speed,
                time,
                0.6,
                (0.3, 0.1),
            );
        }
    }
}
