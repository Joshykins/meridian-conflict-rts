//! A dash as it is drawn (mc-sim `dash.rs`, the Regency Seraph): a first pass built from
//! the pieces the Regency's other light already uses, in its palette (red containment,
//! white hearts; no orange, no violet: nothing here is nanites).
//!
//! - **Windup** (`SimEvent::DashCharging`): a red charge gathering round the hull's
//!   middle, tightening onto a white heart as it is about to go.
//! - **Leaving** (`DashLanded`, at `from`): the light where it stood snaps in on itself and
//!   is gone, and the dust it stood in is pulled in after it.
//! - **Landing** (`DashShockwave`): a white flash and a fusion strike's nova at the heart,
//!   two pressure fronts out to the blast's reach, a ring of dust driven out over the
//!   ground and a red-glassed crater.
//!
//! Presentation only; the renderer's own clock.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::craters::CraterStyle;
use super::{Renderer, PUFF_DUST};
use crate::gpu_consts::puff;
use mc_sim::SimEvent;

const GLOW: f32 = puff::WARP_GLOW as f32;
const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);

impl Renderer {
    /// The dash's part of a sim event. Everything else about it is drawn as usual.
    pub(super) fn dash_event(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::DashCharging {
                from,
                ticks,
                blueprint,
                ..
            } => {
                let (r, h) = self.dash_hull(*blueprint);
                let at = Vec3::from(from.to_f32()) + Vec3::Z * h * 0.5;
                let seconds = *ticks as f32 * self.tick_seconds.max(0.02);
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    seconds,
                    (r * 1.4, r * 0.4),
                    RED * 3.0,
                    0.0,
                );
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time + seconds * 0.5,
                    seconds * 0.5,
                    (r * 0.3, r * 0.7),
                    HOT * 4.0,
                    0.0,
                );
            }
            SimEvent::DashLanded {
                from, blueprint, ..
            } => {
                let (r, h) = self.dash_hull(*blueprint);
                let ground = Vec3::from(from.to_f32());
                let at = ground + Vec3::Z * h * 0.5;
                // The light where it stood collapses to nothing.
                self.push_effect(at.to_array(), time, r * 1.2, 0.2, 9.0, 0.0);
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.25,
                    (r * 1.6, r * 0.05),
                    WHITE * 8.0,
                    0.0,
                );
                self.push_lit(
                    GLOW,
                    at,
                    Vec3::ZERO,
                    time,
                    0.4,
                    (r * 1.2, r * 0.1),
                    RED * 5.0,
                    0.0,
                );
                // The dust it stood in drawn in after it.
                for i in 0..16 {
                    let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 16.0;
                    let dir = Vec3::new(a.cos(), a.sin(), 0.0);
                    let out = r * (1.6 + 0.6 * self.scatter.unit());
                    self.push_puff(
                        PUFF_DUST,
                        ground + dir * out,
                        -dir * out * 1.6,
                        time,
                        0.6,
                        (r * 0.3, r * 0.15),
                    );
                }
            }
            SimEvent::DashShockwave { pos, radius, .. } => {
                self.dash_shockwave(Vec3::from(pos.to_f32()), radius.to_f32(), time);
            }
            _ => {}
        }
    }

    /// Radius and height of the hull that dashed.
    fn dash_hull(&self, blueprint: mc_data::BlueprintId) -> (f32, f32) {
        let bp = self.blueprints.unit(blueprint);
        (bp.radius.to_f32(), bp.height.to_f32())
    }

    /// The landing at `at`, its blow reaching `reach` metres.
    fn dash_shockwave(&mut self, at: Vec3, reach: f32, time: f32) {
        let ground = at
            .truncate()
            .extend(self.ground_height(at.truncate()).max(at.z));
        self.push_effect(
            (ground + Vec3::Z * 20.0).to_array(),
            time,
            reach * 0.6,
            0.35,
            9.0,
            0.0,
        );
        self.fusion_nova(ground, 1.0, reach * 0.15, reach * 0.25, true, time);
        self.push_shockwave(
            (ground + Vec3::Z * 4.0).to_array(),
            time,
            reach * 1.3,
            1.1,
            1.0,
            1.0,
            Vec3::ZERO,
        );
        self.push_shockwave(
            (ground + Vec3::Z * 2.0).to_array(),
            time + 0.15,
            reach * 0.8,
            1.6,
            0.8,
            1.0,
            Vec3::ZERO,
        );
        // Dust driven out over the ground in a ring.
        for i in 0..32 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 32.0;
            let dir = Vec2::new(a.cos(), a.sin()).extend(0.08);
            let from = ground + dir * reach * 0.15;
            let speed = reach * (0.9 + 0.5 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                from,
                dir * speed,
                time,
                1.6,
                (reach * 0.08, reach * 0.22),
            );
        }
        self.add_crater_styled(
            ground.truncate(),
            reach * 0.3,
            0.6,
            time,
            CraterStyle::Plasma,
        );
    }
}
