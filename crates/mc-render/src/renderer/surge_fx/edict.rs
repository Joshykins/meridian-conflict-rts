//! What the Edict's records lay on the ground as their fronts get there (`surge_tick`;
//! the strike itself is `nuke_fx/edict.rs`):
//!
//! - **The shock** (`EDICT_SHOCK`): a skirt of dark dust thrown out low off the ground
//!   along its foot, fast close in and slower as it runs out, settling within seconds.
//! - **The molten wave** (`EDICT_WAVE`): molten spatter and sparks flung forward off its
//!   crest, its red light running out with it round the ring, and embers rising off the
//!   ground it has rolled over, more near the mark where it burns hottest.
//!
//! The strike's lens (`EDICT_STRIKE`) lays nothing.

use super::super::regency_guns_fx::{GLOB, HOT, MOTE, RED, WHITE};
use super::super::{Renderer, PUFF_SHOCK_DUST};
use super::Surge;
use crate::gpu_consts::surge_fx::{EDICT_SHOCK, EDICT_WAVE};
use glam::Vec3;
use std::f32::consts::TAU;

/// Metres between the rings of an Edict record's foot laid at once.
pub(super) const RING: f32 = 30.0;
/// The share of the shock's radius inside which it throws up no dust.
const DUST_FROM: f32 = 0.35;
/// Embers let rise off the molten ground each tick, at most.
const EMBERS: usize = 40;

impl Renderer {
    /// Ring `k` of an Edict record's foot, as its front reaches it.
    pub(super) fn lay_edict_ring(&mut self, surge: &Surge, k: usize) {
        let r = super::ring_at(k, RING);
        match surge.look {
            EDICT_SHOCK => self.shock_ring(surge, r),
            EDICT_WAVE => self.wave_ring(surge, r, k),
            _ => {}
        }
    }

    /// Seconds after the strike the shock's front reaches `r` (`Surge::front` turned
    /// round).
    fn shock_reaches(surge: &Surge, r: f32) -> f32 {
        let t = ((r / surge.radius - 1.0 / 12.0) / (11.0 / 12.0)).clamp(0.0, 1.0);
        surge.roll() * t * t
    }

    /// The shock's foot at `r`: dark dust thrown out ahead of it, low and fast.
    fn shock_ring(&mut self, surge: &Surge, r: f32) {
        // Under the lens the ground is vaporised, not thrown up.
        if r < surge.radius * DUST_FROM {
            return;
        }
        let when = surge.start + Self::shock_reaches(surge, r);
        let water = self.map_info.water_level.to_f32();
        let out_of = (r / surge.radius).min(1.0);
        // A deliberate cosmetic cap per ring: past a few dozen puffs a ring the skirt
        // reads the same and costs the puff ring its other smoke.
        let kicks = ((TAU * r / 110.0).ceil() as usize).clamp(8, 72);
        let spin = self.scatter.unit();
        for n in 0..kicks {
            let turn = (n as f32 + 0.7 * self.scatter.unit() + spin) / kicks as f32;
            let (p, out) = self.surge_foot(surge, r, turn);
            if p.z <= water + 0.05 {
                continue;
            }
            let speed = (120.0 - 80.0 * out_of) * (0.6 + 0.5 * self.scatter.unit());
            let vel = (out * speed).extend(10.0 + 18.0 * self.scatter.unit());
            let size = (34.0 - 14.0 * out_of) * (0.7 + 0.6 * self.scatter.unit());
            let life = 1.6 + 1.6 * self.scatter.unit();
            self.push_puff(
                PUFF_SHOCK_DUST,
                p + Vec3::Z * (1.0 + size * 0.2),
                vel,
                when,
                life,
                (size, size * 3.2),
            );
        }
    }

    /// The molten wave's crest at ring `k` (`r` out): spatter and sparks thrown forward
    /// off it, and its light.
    fn wave_ring(&mut self, surge: &Surge, r: f32, k: usize) {
        let when = surge.start + r / surge.speed;
        // Hotter close in, a tenth of it at its rim (the sim's `EdictWave::edge`).
        let heat = 1.0 - 0.85 * (r / surge.radius).min(1.0);
        let spatter = ((TAU * r / 60.0).ceil() as usize).clamp(6, 64);
        let spin = self.scatter.unit();
        for n in 0..spatter {
            let turn = (n as f32 + self.scatter.unit() + spin) / spatter as f32;
            let (p, out) = self.surge_foot(surge, r, turn);
            let r0 = self.scatter.unit();
            if r0 < 0.5 {
                // A glob of melt flung forward and up off the crest, falling and cooling.
                let vel = (out * (14.0 + 22.0 * self.scatter.unit()))
                    .extend(10.0 + 16.0 * self.scatter.unit());
                let size = 1.6 + 2.2 * self.scatter.unit();
                let tone = RED.lerp(HOT, heat * self.scatter.unit()) * (3.0 + 4.0 * heat);
                self.push_lingering(
                    GLOB,
                    p + Vec3::Z * 2.0,
                    vel,
                    when,
                    1.2 + 1.2 * r0,
                    (size, size * 0.6),
                    tone,
                    0.0,
                    1,
                );
            } else {
                let vel = (out * (20.0 + 30.0 * self.scatter.unit()))
                    .extend(8.0 + 14.0 * self.scatter.unit());
                let life = 0.6 + 0.6 * self.scatter.unit();
                self.push_lit(
                    MOTE,
                    p + Vec3::Z * 1.5,
                    vel,
                    when,
                    life,
                    (0.9, 0.3),
                    HOT.lerp(WHITE, heat * 0.6) * (3.0 + 3.0 * heat),
                    0.0,
                );
            }
        }
        // Its light runs out with it, round the ring a few points at a time.
        if k.is_multiple_of(2) {
            let lights = ((TAU * r / 220.0).ceil() as usize).clamp(3, 14);
            for n in 0..lights {
                let (p, _) = self.surge_foot(surge, r, (n as f32 + spin) / lights as f32);
                self.plasma_fx.guns.flare(
                    p + Vec3::Z * 6.0,
                    RED.lerp(HOT, heat) * 900.0 * (0.3 + heat),
                    (TAU * r / lights as f32 * 0.7).clamp(60.0, 260.0),
                    when,
                    0.5,
                );
            }
        }
    }

    /// Embers rising off the molten ground behind the wave, more where it burns hottest,
    /// crimson, now and then a hotter one.
    pub(super) fn edict_embers(&mut self, surge: &Surge, front: f32, time: f32) {
        let left = 1.0 - ((time - surge.start - surge.roll()) / surge.linger).clamp(0.0, 1.0);
        let count = (EMBERS as f32 * (0.4 + 0.6 * left)) as usize;
        for _ in 0..count {
            // Bunched toward the mark, where the melt is hottest.
            let r = front * self.scatter.unit().powf(0.8);
            let heat = 1.0 - 0.85 * (r / surge.radius).min(1.0);
            let turn = self.scatter.unit();
            let (p, _) = self.surge_foot(surge, r, turn);
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 2.0;
            let vel = drift + Vec3::Z * (4.0 + 8.0 * self.scatter.unit() * heat);
            let tone = if self.scatter.unit() < 0.12 * heat {
                HOT
            } else {
                RED
            };
            let start = time + self.tick_seconds * self.scatter.unit();
            let life = 1.4 + 1.6 * self.scatter.unit();
            self.push_lit(
                MOTE,
                p + Vec3::Z * 0.6,
                vel,
                start,
                life,
                (0.8, 0.26),
                tone * (1.6 + 2.0 * heat),
                0.0,
            );
        }
    }
}
