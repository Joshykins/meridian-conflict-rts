//! What a wreck leaves around it: a few craters blown into the ground next to
//! it, and smoke that the wind carries away from it in a long trail. The smoke
//! thins as the wreck gets old and stops altogether; the craters stay.
//!
//! Presentation only. The sim does not know how old a wreck is, so a wreck's
//! clock starts when the renderer first sees it. Craters are ground stains with
//! `STAIN_CRATER` set (ground.wgsl `fs_stain` draws a bowl and a rim for them),
//! uploaded after the sim's own stains. They outlive the wreck: reclaiming the
//! hull does not fill the ground in.

use super::{Renderer, PUFF_TREE_SMOKE};
use crate::camera::Camera;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{StainInstance, UnitInstance, KIND_WRECK};
use std::collections::HashMap;

/// Craters kept, at most; the oldest go first.
const MAX_CRATERS: usize = 1024;
/// `strength_seed` bit that makes a stain a crater.
const STAIN_CRATER: u32 = 1 << 31;
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
    craters: Vec<StainInstance>,
    /// The wreck each crater was blown for, so one seen again gets no more.
    owners: Vec<u32>,
    /// Where the next crater goes once the ring is full.
    next: usize,
}

impl WreckFx {
    pub(super) fn craters(&self) -> &[StainInstance] {
        &self.craters
    }

    pub(super) fn clear(&mut self) {
        *self = WreckFx::default();
    }

    fn push(&mut self, owner: u32, crater: StainInstance) {
        if self.craters.len() < MAX_CRATERS {
            self.craters.push(crater);
            self.owners.push(owner);
        } else {
            self.craters[self.next] = crater;
            self.owners[self.next] = owner;
            self.next = (self.next + 1) % MAX_CRATERS;
        }
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
    /// Settled wrecks smoke, downwind, for a while after they die, and blow
    /// craters into the ground round them the first time they are seen.
    pub(super) fn wreck_smoke(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let wind = self.sky.wind_heading();
        let reach = camera.distance * 2.5 + 300.0;
        let mut budget = PUFFS_PER_TICK;
        for u in units {
            // Settled salvage only: a falling or sinking hull has its own trail.
            if u.owner_flags & KIND_WRECK == 0 || u._pad != 0 {
                continue;
            }
            let at = Vec3::from(u.pos);
            let (first, fresh) = match self.wreck_fx.seen.get_mut(&u.unit_id) {
                Some(seen) => {
                    seen.1 = time;
                    (seen.0, false)
                }
                None => {
                    self.wreck_fx.seen.insert(u.unit_id, (time, time));
                    (time, true)
                }
            };
            let (size, height) = match self.burn_sites.get(u.blueprint as usize) {
                Some(site) => (site.reach, site.height),
                None => (u.radius, u.radius),
            };
            let size = size.max(1.0);
            if fresh && !self.wreck_fx.owners.contains(&u.unit_id) {
                self.blow_craters(u.unit_id, at.truncate(), size);
            }

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

    /// Three to six craters round a wreck: one the size of the blast that killed it,
    /// close in, and smaller ones scattered further out. Never under water.
    fn blow_craters(&mut self, id: u32, at: Vec2, size: f32) {
        let count = 3 + (hash(id, 1) * 2.99) as u32 + (size > 8.0) as u32;
        for k in 0..count {
            let big = k == 0;
            let angle = hash(id, 20 + k) * std::f32::consts::TAU;
            let out = size * if big { 0.5 + 0.5 * hash(id, 30 + k) } else { 0.9 + 1.4 * hash(id, 30 + k) };
            let radius = size
                * if big { 0.45 + 0.2 * hash(id, 40 + k) } else { 0.14 + 0.2 * hash(id, 40 + k) };
            let radius = radius.clamp(0.9, 14.0);
            let pos = at + Vec2::from_angle(angle) * out;
            if self.under_sea(pos.extend(self.ground_height(pos) + 0.1)) {
                continue;
            }
            let strength = (150.0 + 90.0 * hash(id, 50 + k)) as u32;
            let seed = (hash(id, 60 + k) * 32767.0) as u32;
            self.wreck_fx.push(
                id,
                StainInstance {
                    pos: pos.to_array(),
                    radius,
                    strength_seed: strength | seed << 8 | STAIN_CRATER,
                },
            );
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
