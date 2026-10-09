//! A winged flier's light (`models::Flight`): what its feathers shed. Each tick, at each
//! feather's tip where flight.wgsl draws it (the pose's main terms are shared through
//! `gpu_consts::flight`):
//!
//! - at rest, now and then an ember lets go of a feather and drifts down and away,
//!   dying as it falls;
//! - under way, a thread of red light streams back off every tip along the path, longer
//!   the faster it goes, and embers peel off in its wake;
//! - winding up a dash, light is drawn in to it from all round and the tips flare
//!   white-hot; at the landing every feather throws a burst of embers out.
//!
//! The puffs are the warp's (`warp_puffs.wgsl`: `WARP_GLOW`, `WARP_STREAK`, `WARP_MOTE`),
//! clean, coloured red. Presentation only.

use super::Renderer;
use crate::gpu_consts::{flight, puff};
use crate::models::Flight;
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

const PUFF_GLOW: f32 = puff::WARP_GLOW as f32;
const PUFF_STREAK: f32 = puff::WARP_STREAK as f32;
const PUFF_MOTE: f32 = puff::WARP_MOTE as f32;

/// The feathers' red, their white-hot charge, and the pink of the light drawn in.
const RED: Vec3 = Vec3::new(1.0, 0.1, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.62, 0.55);
const DRAWN: Vec3 = Vec3::new(1.0, 0.3, 0.42);
/// Puffs every flier together may lay in one tick: a deliberate cosmetic cap, so a few of
/// them in a melee cannot flush the shared puff ring of everything else's smoke and fire.
/// Past it the furthest-down units in the list go without for that tick.
const TICK_BUDGET: usize = 360;
/// The most one flier lays in a tick (a landing's burst from both wings).
const PER_UNIT: usize = 2 * flight::FEATHERS as usize * 3;

pub(super) struct WingFx {
    /// Per model slot (`UnitInstance::blueprint`): its flight rig, and its top speed in
    /// metres a tick.
    rigs: Vec<Option<(Flight, f32)>>,
    /// Puffs left in this tick's budget.
    left: usize,
}

impl WingFx {
    pub(super) fn new(rigs: Vec<Option<(Flight, f32)>>) -> Self {
        Self { rigs, left: 0 }
    }

    /// A new tick: the budget is full again.
    pub(super) fn new_tick(&mut self) {
        self.left = TICK_BUDGET;
    }

    /// The unit's model has wings.
    pub(super) fn flies(&self, blueprint: u32) -> bool {
        self.rigs
            .get(blueprint as usize)
            .is_some_and(Option::is_some)
    }
}

/// flight.wgsl's turns (`rot_x`, `rot_z`, `rot_xz`), the same way round.
fn rot_x(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c)
}

fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}

/// Where feather `i`'s tip is on the unit, `left` wing or right, at `speed` (a share of its
/// top) and `charge` (a dash's windup): flight.wgsl's pose by its main terms, the torso
/// turned `yaw` about the waist and the body leaned. In the unit's frame (x forward).
fn tip(rig: &Flight, i: usize, left: bool, speed: f32, charge: f32, yaw: f32) -> Vec3 {
    let hub = Vec3::from(rig.hub);
    let close = flight::CLOSE_SPEED * speed + flight::CLOSE_CHARGE * charge;
    let fan = (i as f32 - 3.0) * flight::FAN_STEP * close;
    let sweep = flight::SWEEP_SPEED * speed + flight::SWEEP_CHARGE * charge;
    let lift = flight::LIFT_SPEED * speed + flight::LIFT_CHARGE * charge;
    let q = rot_z(
        rot_x(rot_x(Vec3::from(rig.tips[i]) - hub, fan), lift),
        sweep,
    ) + hub;
    let q = if left {
        q
    } else {
        q * Vec3::new(1.0, -1.0, 1.0)
    };
    let hips = Vec3::new(0.0, 0.0, rig.hip[2]);
    let lean = -(flight::LEAN_SPEED * speed + flight::LEAN_CHARGE * charge);
    rot_xz(rot_z(q, yaw) - hips, lean) + hips
}

impl Renderer {
    /// This tick's light off the unit's feathers.
    pub(super) fn wing_light(&mut self, u: &UnitInstance, time: f32) {
        let Some(Some((rig, top))) = self.wing_fx.rigs.get(u.blueprint as usize).copied() else {
            return;
        };
        if self.wing_fx.left < PER_UNIT {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
        let travel = to - from;
        let speed = (u.gait[1] / top.max(0.01)).clamp(0.0, 1.0);
        let charge = u.deploy.clamp(0.0, 1.0);
        let landed = u.prev_deploy >= 0.0 && u.deploy < 0.0;
        let (s, c) = u.heading.sin_cos();
        let world = |v: Vec3| Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
        let fwd = Vec3::new(c, s, 0.0);
        let mut laid = 0;
        for left in [true, false] {
            for i in 0..flight::FEATHERS as usize {
                let at = to + world(tip(&rig, i, left, speed, charge, u.turret_yaw));
                let k = self.scatter.unit();
                if landed {
                    // The blow throws a spray of embers off every feather.
                    for _ in 0..3 {
                        let out = (at - to).normalize_or(Vec3::Z);
                        let vel = (out
                            + Vec3::new(
                                self.scatter.signed(),
                                self.scatter.signed(),
                                self.scatter.unit(),
                            ) * 0.6)
                            * (14.0 + 18.0 * self.scatter.unit());
                        let life = 0.7 + 0.5 * self.scatter.unit();
                        self.push_warp(PUFF_MOTE, at, vel, time, life, (1.6, 0.55), HOT * 4.0, 0.0);
                        laid += 1;
                    }
                    continue;
                }
                if charge > 0.0 {
                    // The tip flares, and light is drawn in to it from out round the wing.
                    self.push_warp(
                        PUFF_GLOW,
                        at,
                        Vec3::ZERO,
                        time + k * dt,
                        dt * 1.1,
                        (0.8 + 1.6 * charge, 1.2 + 2.0 * charge),
                        RED.lerp(HOT, charge) * (2.0 + 3.0 * charge),
                        0.0,
                    );
                    let from_out = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    )
                    .normalize_or(Vec3::Z)
                        * (18.0 + 22.0 * self.scatter.unit());
                    let life = 0.3 + 0.15 * self.scatter.unit();
                    self.push_warp(
                        PUFF_MOTE,
                        at + from_out,
                        -from_out / life,
                        time + k * dt,
                        life,
                        (1.2, 0.4),
                        DRAWN * 4.0,
                        0.0,
                    );
                    laid += 2;
                    continue;
                }
                if speed > 0.15 {
                    // A thread of light streamed back off the tip along the path.
                    let length = travel.length() * (1.5 + 2.5 * speed) + 4.0;
                    self.push_warp(
                        PUFF_STREAK,
                        at - travel * k,
                        -fwd * length,
                        time + k * dt,
                        0.28 + 0.2 * speed,
                        (0.9, 0.9),
                        RED * (2.0 + 2.0 * speed),
                        0.0,
                    );
                    laid += 1;
                }
                // An ember now and then, more at speed, from somewhere along the feather.
                if self.scatter.unit() < 0.06 + 0.25 * speed {
                    let along = 0.5 + 0.5 * self.scatter.unit();
                    let root = to
                        + world(
                            Vec3::from(rig.hub)
                                * Vec3::new(1.0, if left { 1.0 } else { -1.0 }, 1.0),
                        );
                    let start = root.lerp(at, along) - travel * k;
                    let vel = -fwd * travel.length() / dt * 0.15
                        + Vec3::new(
                            self.scatter.signed() * 2.0,
                            self.scatter.signed() * 2.0,
                            -1.5 - 2.0 * self.scatter.unit(),
                        );
                    let tint = RED.lerp(HOT, self.scatter.unit() * 0.4) * 3.0;
                    let life = 1.2 + 0.9 * self.scatter.unit();
                    self.push_warp(
                        PUFF_MOTE,
                        start,
                        vel,
                        time + k * dt,
                        life,
                        (0.9, 0.35),
                        tint,
                        0.0,
                    );
                    laid += 1;
                }
            }
        }
        self.wing_fx.left = self.wing_fx.left.saturating_sub(laid);
    }
}
