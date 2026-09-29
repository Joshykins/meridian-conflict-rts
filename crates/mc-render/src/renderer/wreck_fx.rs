//! Smoke off a wreck, that the wind carries away from it in a long trail. It
//! thins as the wreck gets old and stops altogether. (Craters are dug only where
//! a blast struck the ground: impact_craters.rs.)
//!
//! Presentation only. The sim does not know how old a wreck is, so a wreck's
//! clock starts when the renderer first sees it.

use super::{Renderer, PUFF_TREE_SMOKE};
use crate::camera::Camera;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{UnitInstance, KIND_WRECK, WRECK_INNER};
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

/// Zero to one, fixed for a wreck and a salt.
fn hash(id: u32, salt: u32) -> f32 {
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
            let (size, height) = match self.burn_sites.get(u.blueprint as usize) {
                Some(site) => (site.reach, site.height),
                None => (u.radius, u.radius),
            };
            let size = size.max(1.0);

            let age = time - first;
            let burning = 1.0 - smoothstep(SMOKE_FULL, SMOKE_OUT, age);
            // Most of the hull reclaimed: little left to burn.
            let burning = burning * (0.35 + 0.65 * u.health.clamp(0.0, 1.0));
            if burning <= 0.01 || budget == 0 || at.distance(camera.focus) > reach {
                continue;
            }
            // A few sources over a big wreck, so a factory smokes from several places.
            let sources = ((size / 5.0).ceil() as usize).clamp(1, 4);
            for k in 0..sources {
                if budget == 0 || self.scatter.unit() > 0.25 + 0.55 * burning {
                    continue;
                }
                let angle = (hash(u.unit_id, 11 + k as u32) + self.scatter.unit() * 0.08)
                    * std::f32::consts::TAU;
                let out = size * 0.45 * hash(u.unit_id, 17 + k as u32).sqrt();
                let foot = at.truncate() + Vec2::from_angle(angle) * out;
                let ground = self.ground_height(foot).max(at.z);
                let from = foot.extend(ground + height * (0.25 + 0.35 * self.scatter.unit()));
                if self.under_sea(from) {
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
        self.wreck_fx.seen.retain(|_, seen| time - seen.1 < FORGET);
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
