//! A walker's feet coming down: a print the size of the sole and dust from under each,
//! kept in time with the stride `entity.wgsl` walks (`walk_leg`, `crawl_leg`).
//!
//! Presentation only; the renderer's own clock.

use super::{titan_fx, Renderer, TrackMark, PUFF_DUST, TRACK_MARK_LIFE};
use crate::models::{self, Legs};
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

impl Renderer {
    /// A walker's foot coming down: a print the size of the sole, and dust from
    /// under it. The shader plants the left foot as the stride's cycle wraps
    /// and the right half a cycle later; this keeps the same time.
    pub(super) fn footfall(
        &mut self,
        u: &UnitInstance,
        legs: &Legs,
        time: f32,
        mark: bool,
        dust: bool,
    ) {
        if let Some(crawl) = legs.crawl {
            self.crawl_footfalls(u, legs, &crawl, time, dust);
            return;
        }
        let cycle = |ground: f32| (ground / legs.stride * 2.0).floor();
        let (before, now) = (cycle(u.gait[0] - u.gait[1]), cycle(u.gait[0]));
        // A giant wades: its feet still come down in the shallows (`titan_fx`).
        let giant = titan_fx::strides(&self.blueprints, u.blueprint);
        if u.gait[1] <= 0.0
            || before == now
            || (u.pos[2] < self.map_info.water_level.to_f32() && !giant)
        {
            return;
        }
        let side = if now.rem_euclid(2.0) < 1.0 { 1.0 } else { -1.0 };
        let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        // Set down at the front of its reach, in under the hips unless the leg bends back
        // (`entity.wgsl` `stride_ankle_y`).
        let out = if legs.hock.is_some() {
            legs.ankle[1]
        } else {
            legs.hip[1]
        };
        let plant = Vec3::from(u.pos)
            + forward * (legs.ankle[0] + legs.stride * legs.stance * 0.5)
            + left * (side * out);
        if mark && legs.foot[2] > 0.0 && giant {
            self.giant_print(plant, forward, legs, time + self.tick_seconds * 0.5);
        } else if mark && legs.foot[2] > 0.0 {
            let heel = [
                plant.x + forward.x * legs.foot[0],
                plant.y + forward.y * legs.foot[0],
            ];
            let toe = [
                plant.x + forward.x * legs.foot[1],
                plant.y + forward.y * legs.foot[1],
            ];
            self.push_mark(TrackMark {
                start_xy: heel,
                end_xy: toe,
                half_gauge: 0.0,
                width: legs.foot[2],
                start: time + self.tick_seconds * 0.5,
                life: TRACK_MARK_LIFE,
            });
        }
        if giant {
            // When in the tick the foot lands, as the game times its sound.
            let pace = legs.stride * 0.5;
            let landing = now * pace;
            let share = ((landing - (u.gait[0] - u.gait[1])) / u.gait[1]).clamp(0.0, 1.0);
            self.giant_footfall(plant, forward, legs.foot, time + share * self.tick_seconds);
            return;
        }
        if !dust {
            return;
        }
        let at = plant + Vec3::Z * 0.3;
        let weight = legs.hip[2];
        for _ in 0..4 {
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.15).normalize_or_zero();
            let life = 0.9 + self.scatter.unit() * 0.7;
            self.push_puff(
                PUFF_DUST,
                at + out * weight * 0.12,
                out * (2.0 + weight * 0.5) + Vec3::Z * 0.8,
                time + self.tick_seconds * 0.5,
                life,
                (weight * 0.12, weight * 0.42),
            );
        }
    }

    /// A many-legged walker's feet coming down (`models::Crawl`): no prints, a little dust
    /// kicked up where each pointed foot lands, as `entity.wgsl` `crawl_leg` plants them.
    fn crawl_footfalls(
        &mut self,
        u: &UnitInstance,
        legs: &Legs,
        crawl: &models::Crawl,
        time: f32,
        dust: bool,
    ) {
        if !dust || u.gait[1] <= 0.0 || u.pos[2] < self.map_info.water_level.to_f32() {
            return;
        }
        let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        let (before, now) = (
            (u.gait[0] - u.gait[1]) / legs.stride,
            u.gait[0] / legs.stride,
        );
        for i in 0..crawl.pairs {
            let [_, _, ankle] = crawl.joints[i];
            for side in [1.0f32, -1.0] {
                // A foot lands when its cycle passes the stance's start (`crawl_leg`: phase 0).
                let offset = crawl.phase[i] + if side < 0.0 { 0.5 } else { 0.0 };
                if (now - offset).floor() == (before - offset).floor() {
                    continue;
                }
                let plant = Vec3::from(u.pos)
                    + forward * (ankle[0] + legs.stride * legs.stance * 0.5)
                    + left * (side * ankle[1])
                    + Vec3::Z * 0.2;
                for _ in 0..2 {
                    let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.2)
                        .normalize_or_zero();
                    let life = 0.7 + self.scatter.unit() * 0.5;
                    self.push_puff(
                        PUFF_DUST,
                        plant + out * 0.3,
                        out * 2.2 + Vec3::Z * 0.7,
                        time + self.tick_seconds * 0.5,
                        life,
                        (0.35, 1.3),
                    );
                }
            }
        }
    }
}
