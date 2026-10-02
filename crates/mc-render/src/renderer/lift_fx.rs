//! Plasma lift: red plasma under the bells of a craft that floats on gravity lift (the
//! Regency's Artificer; `models::Lift`, `MeshBuilder::add_lift`). Each tick, under each
//! bell near the eye:
//!
//! - a red glow swelling under the mouth;
//! - lightning crackling from the mouth out to the ground, one or two arcs at a time;
//! - plasma motes thrown off it, rising and dying.
//!
//! All of it is laid in the world where the bell was and left there, so as the craft moves
//! it trails a little behind: the faster it goes the further the arcs rake back and the
//! longer the glow smears out behind it. Standing still it only crackles. The ground under
//! it is lit by the unit file's own red lamp (`lights`), not from here.
//!
//! The puffs are the warp's (`warp_puffs.wgsl`: `WARP_GLOW`, `WARP_ARC`, `WARP_MOTE`),
//! clean (not torn), coloured red. Presentation only.

use super::Renderer;
use crate::gpu_consts::puff;
use crate::models::Lift;
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

const PUFF_GLOW: f32 = puff::WARP_GLOW as f32;
const PUFF_ARC: f32 = puff::WARP_ARC as f32;
const PUFF_MOTE: f32 = puff::WARP_MOTE as f32;

/// The lift's deep red, and the pink-white of its hottest arcs.
const RED: Vec3 = Vec3::new(1.0, 0.1, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.42, 0.32);
/// Mouth radius (m) from which a bell throws its motes at full speed; smaller ones throw
/// them in proportion.
const FULL_THROW: f32 = 0.5;
/// Speed (m/s) at which the plasma rakes back at its longest.
const FULL_TRAIL: f32 = 24.0;
/// Puffs every lift together may lay in one tick: a deliberate cosmetic cap, so a crowd
/// of engineers by the eye cannot flush the shared puff ring of everything else's smoke
/// and fire. Past it the furthest-down units in the list just go without for that tick.
const TICK_BUDGET: usize = 480;
/// The most one bell lays in a tick (a glow, two arcs, two motes).
const PER_BELL: usize = 5;

pub(super) struct LiftFx {
    /// Per model slot (`UnitInstance::blueprint`): its lift bells.
    bells: Vec<Vec<Lift>>,
    /// Puffs left in this tick's budget.
    left: usize,
}

impl LiftFx {
    pub(super) fn new(bells: Vec<Vec<Lift>>) -> Self {
        Self { bells, left: 0 }
    }

    /// A new tick: the budget is full again.
    pub(super) fn new_tick(&mut self) {
        self.left = TICK_BUDGET;
    }

    /// The unit's model has lift bells.
    pub(super) fn lifts(&self, blueprint: u32) -> bool {
        self.bells
            .get(blueprint as usize)
            .is_some_and(|b| !b.is_empty())
    }
}

impl Renderer {
    /// This tick's plasma under each of the unit's lift bells.
    pub(super) fn plasma_lift(&mut self, u: &UnitInstance, time: f32, moving: bool) {
        let slot = u.blueprint as usize;
        let count = self.lift_fx.bells.get(slot).map_or(0, Vec::len);
        let dt = self.tick_seconds.max(0.02);
        let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
        let speed = if moving {
            from.truncate().distance(to.truncate()) / dt
        } else {
            0.0
        };
        let trail = (speed / FULL_TRAIL).min(1.0);
        let (s, c) = u.heading.sin_cos();
        let fwd = Vec3::new(c, s, 0.0);
        for i in 0..count {
            if self.lift_fx.left < PER_BELL {
                return;
            }
            let bell = self.lift_fx.bells[slot][i];
            let local = Vec3::from(bell.at);
            let off = Vec3::new(
                local.x * c - local.y * s,
                local.x * s + local.y * c,
                local.z,
            );
            let r = bell.radius;
            // A small bell (a drone's) throws its motes in proportion, so its crackle stays
            // about its own size rather than a full bell's.
            let throw = (r / FULL_THROW).min(1.0);
            let mut laid = 0;

            // The glow under the mouth, smeared out behind when it moves.
            let k = self.scatter.unit();
            self.push_warp(
                PUFF_GLOW,
                from.lerp(to, k) + off - Vec3::Z * 0.05,
                -fwd * speed * 0.2,
                time + k * dt,
                0.2 + 0.12 * trail,
                (r * 1.8, r * (2.3 + 0.8 * trail)),
                RED * 1.4,
                0.0,
            );
            laid += 1;

            // Lightning from the mouth to the ground, raked back the faster it goes.
            let arcs = 1 + usize::from(self.scatter.unit() < 0.3 + 0.5 * trail);
            for _ in 0..arcs {
                let k = self.scatter.unit();
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let out = Vec3::new(a.cos(), a.sin(), 0.0);
                let start = from.lerp(to, k) + off + out * (r * 0.5 * self.scatter.unit());
                let reach = r * (1.2 + 1.4 * self.scatter.unit()) + trail * r * 2.5;
                let dir =
                    (out * (0.9 - 0.6 * trail) - fwd * (0.25 + 1.3 * trail)).normalize_or(-fwd);
                let color = if self.scatter.unit() < 0.25 { HOT } else { RED };
                let life = 0.14 + 0.1 * self.scatter.unit();
                self.push_warp(
                    PUFF_ARC,
                    start,
                    dir * reach - Vec3::Z * (off.z * 0.9),
                    time + k * dt,
                    life,
                    (r * 0.6, r * 0.6),
                    color * 5.0,
                    0.0,
                );
                laid += 1;
            }

            // Motes thrown off, rising, left behind as it goes.
            for _ in 0..1 + usize::from(moving) {
                let k = self.scatter.unit();
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let out = Vec3::new(a.cos(), a.sin(), 0.0);
                let start =
                    from.lerp(to, k) + off + out * (r * 0.6 * self.scatter.unit()) - Vec3::Z * 0.05;
                let vel = -fwd * speed * (0.15 + 0.2 * self.scatter.unit())
                    + (out * (0.6 + 1.6 * self.scatter.unit())
                        + Vec3::Z * (0.4 + 1.2 * self.scatter.unit()))
                        * throw;
                let color = RED.lerp(HOT, self.scatter.unit() * 0.5) * 3.0;
                let life = 0.35 + 0.3 * self.scatter.unit();
                self.push_warp(
                    PUFF_MOTE,
                    start,
                    vel,
                    time + k * dt,
                    life,
                    (r * 0.11, r * 0.04),
                    color,
                    0.0,
                );
                laid += 1;
            }
            self.lift_fx.left -= laid;
        }
    }
}
