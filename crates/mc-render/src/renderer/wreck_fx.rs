//! Smoke off a wreck, that the wind carries away from it in a long trail. It
//! thins as the wreck gets old and stops altogether. (Craters are dug only where
//! a blast struck the ground: impact_craters.rs.)
//!
//! Presentation only. The sim does not know how old a wreck is, so a wreck's
//! clock starts when the renderer first sees it.
//!
//! The smoke rises off the wreck's own metal: a point of the model's upper surface
//! within the stretch of hull the section keeps, posed as `entity.wgsl` poses it
//! (`HullFrame`). A wreck lying under the sea gives none from what is under it.

use super::{Renderer, PUFF_TREE_SMOKE};
use crate::camera::Camera;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{UnitInstance, KIND_WRECK, WRECK_COUNT_SHIFT, WRECK_INNER, WRECK_POSED};
use std::collections::HashMap;

/// Seconds a wreck smokes at full strength, and by when it has stopped.
const SMOKE_FULL: f32 = 30.0;
const SMOKE_OUT: f32 = 100.0;
/// Seconds a wreck out of sight is remembered, so it comes back into view as old as it was.
const FORGET: f32 = 300.0;
/// Smoke puffs all wrecks may lay in one tick, at most.
const PUFFS_PER_TICK: usize = 64;

#[derive(Default)]
pub(super) struct WreckFx {
    /// Wreck id to when it was first and last seen, seconds.
    seen: HashMap<u32, (f32, f32)>,
}

impl WreckFx {
    pub(super) fn clear(&mut self) {
        *self = WreckFx::default();
    }
}

/// A hull's drawn frame at one moment of the tick, as the entity shader poses a wreck
/// that is falling, sinking or posed: heading, then pitch (bow up) about the beam, then
/// roll about the pitched keel line, all about the instance's origin. (A settled
/// wreck's lean onto the slope and its slump into the ground are left out: small.)
pub(super) struct HullFrame {
    origin: Vec3,
    fwd: Vec3,
    left: Vec3,
    up: Vec3,
}

impl HullFrame {
    /// `tilted`: the instance's pitch (`arm_pitch.xy`) and roll (`_pad2`) are set.
    pub(super) fn of(u: &UnitInstance, t: f32, tilted: bool) -> HullFrame {
        let origin = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), t);
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let heading = u.prev_heading + turn * t;
        let (pitch, roll) = if tilted {
            (
                u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * t,
                u._pad2[0] + (u._pad2[1] - u._pad2[0]) * t,
            )
        } else {
            (0.0, 0.0)
        };
        let flat = Vec3::new(heading.cos(), heading.sin(), 0.0);
        let left = Vec3::new(-flat.y, flat.x, 0.0);
        let fwd = flat * pitch.cos() + Vec3::Z * pitch.sin();
        let up = Vec3::Z * pitch.cos() - flat * pitch.sin();
        HullFrame {
            origin,
            fwd,
            left: left * roll.cos() + up * roll.sin(),
            up: up * roll.cos() - left * roll.sin(),
        }
    }

    /// A point of the model, metres in its own frame, in the world.
    pub(super) fn at(&self, local: Vec3) -> Vec3 {
        self.origin + self.fwd * local.x + self.left * local.y + self.up * local.z
    }
}

/// Zero to one, fixed for an id (a wreck, a prop) and a salt.
pub(super) fn hash(id: u32, salt: u32) -> f32 {
    let mut n = id.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 15;
    n = n.wrapping_mul(0x2C1B_3C6D);
    n ^= n >> 12;
    n = n.wrapping_mul(0x297A_2D39);
    n ^= n >> 15;
    (n >> 8) as f32 / 16_777_216.0
}

impl Renderer {
    /// Settled wrecks smoke, downwind, for a while after they die.
    pub(super) fn wreck_smoke(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let wind = self.sky.wind_heading();
        let reach = camera.distance * 2.5 + 300.0;
        let mut budget = PUFFS_PER_TICK;
        // Held aside while the puffs go out, which needs the whole renderer.
        let sites = std::mem::take(&mut self.burn_sites);
        for u in units {
            // Settled salvage only: a falling or sinking hull has its own trail.
            // One smoke per piece a wreck broke into, not per piece's inside.
            if u.owner_flags & KIND_WRECK == 0
                || u.packed != 0
                || u.refit_modules & WRECK_INNER != 0
            {
                continue;
            }
            let at = Vec3::from(u.pos);
            let first = match self.wreck_fx.seen.get_mut(&u.unit_id) {
                Some(seen) => {
                    seen.1 = time;
                    seen.0
                }
                None => {
                    self.wreck_fx.seen.insert(u.unit_id, (time, time));
                    time
                }
            };
            let Some(site) = sites.get(u.blueprint as usize) else {
                continue;
            };
            let size = site.reach.max(1.0);
            // The stretch of the model this section keeps, and the middle it lies about
            // (`wreck.wgsl wreck_pose`): the whole hull when it lies in one piece.
            let posed = u.refit_modules & WRECK_POSED != 0;
            let count = (u.refit_modules >> WRECK_COUNT_SHIFT) & 15;
            let (lo, hi, centre) = if posed && count > 1 {
                let (lo, hi) = (u.arm_pitch[2] * site.bounds, u.arm_pitch[3] * site.bounds);
                (lo, hi, 0.5 * (lo.max(-site.bounds) + hi.min(site.bounds)))
            } else {
                (f32::MIN, f32::MAX, 0.0)
            };
            // Metres of it the section keeps, for how many places it smokes from.
            let kept = (hi.min(size) - lo.max(-size)).clamp(1.0, 2.0 * size);

            let age = time - first;
            let burning = 1.0 - smoothstep(SMOKE_FULL, SMOKE_OUT, age);
            // Most of the hull reclaimed: little left to burn.
            let burning = burning * (0.35 + 0.65 * u.health.clamp(0.0, 1.0));
            if burning <= 0.01 || budget == 0 || at.distance(camera.focus) > reach {
                continue;
            }
            // A few sources over a big wreck, so a factory smokes from several places.
            let sources = ((kept / 10.0).ceil() as usize).clamp(1, 4);
            for k in 0..sources {
                if budget == 0 || self.scatter.unit() > 0.25 + 0.55 * burning {
                    continue;
                }
                let angle = (hash(u.unit_id, 11 + k as u32) + self.scatter.unit() * 0.08)
                    * std::f32::consts::TAU;
                let out = size * 0.9 * hash(u.unit_id, 17 + k as u32).sqrt();
                let x = (angle.cos() * out).clamp(lo, hi);
                // On the metal under that point (walked in toward the middle if it is off
                // the hull), and within the stretch this section keeps.
                let Some((p, _)) = site.grid.surface(x, angle.sin() * out) else {
                    continue;
                };
                if p[0] < lo || p[0] > hi {
                    continue;
                }
                let local = Vec3::new(
                    p[0] - centre,
                    p[1],
                    p[2] * (0.6 + 0.3 * self.scatter.unit()),
                );
                let from = HullFrame::of(u, 1.0, posed).at(local);
                let from = from
                    .truncate()
                    .extend(from.z.max(self.ground_height(from.truncate())));
                // Under the sea: that part of it has nothing to burn. (A little margin for
                // the slump into the seabed the frame leaves out.)
                if self.under_sea(from - Vec3::Z * site.height * 0.1) {
                    continue;
                }
                let puff = (size * 0.2).clamp(0.6, 5.5) * (0.55 + 0.45 * burning);
                // Pushed hard downwind, so it lies back from the wreck in a trail
                // rather than standing over it.
                let carry = 2.4 + 1.4 * self.scatter.unit();
                let drift = (wind * carry
                    + Vec2::new(self.scatter.signed(), self.scatter.signed()) * 0.45)
                    .extend(0.4 + puff * 0.25);
                let life = (5.0 + puff * 1.6).min(13.0)
                    * (0.8 + 0.4 * self.scatter.unit())
                    * (0.55 + 0.45 * burning);
                let start = time + self.scatter.unit() * self.tick_seconds;
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    from,
                    drift,
                    start,
                    life,
                    (puff * 0.7, puff * (2.6 + 1.4 * burning)),
                );
                budget -= 1;
            }
        }
        self.burn_sites = sites;
        self.wreck_fx.seen.retain(|_, seen| time - seen.1 < FORGET);
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
