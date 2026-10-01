//! Craters where a blast actually hit the ground: a shell, bomb or rocket with a real
//! splash that burst on the ground (not on a hull, a shield or in the air), and a
//! crashing aircraft where it slams in. Each lies where it struck, sized to the blast,
//! and shows up as the burst lands. The sim's charcoal scorch for the same hit lies
//! round it.
//!
//! Presentation only: craters are ground stains with `stain::CRATER` set (ground.wgsl
//! `fs_stain` draws a bowl, lip and spoil for them), uploaded after the sim's own
//! stains. Nothing is made up after the fact: a wreck first seen (map wreckage, or one
//! scrolled into view) gets none. Beams, plasma and the heavy rail mark the ground
//! their own way; fire weapons scorch, they do not dig.

use super::Renderer;
use crate::gpu_consts::stain;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{SimEvent, StainInstance};

/// Craters kept, at most; past that the oldest go first.
const MAX_CRATERS: usize = 1024;
/// Smallest splash, metres, that digs a crater. A tank gun's round or a light rocket
/// only scorches.
const MIN_SPLASH: f32 = 5.0;

#[derive(Default)]
pub(super) struct ImpactCraters {
    craters: Vec<StainInstance>,
    /// Craters for bursts still in flight: when each lands, and the crater.
    pending: Vec<(f32, StainInstance)>,
    /// Where the next crater goes once the ring is full.
    next: usize,
    seed: u32,
}

impl ImpactCraters {
    pub(super) fn craters(&self) -> &[StainInstance] {
        &self.craters
    }

    pub(super) fn clear(&mut self) {
        *self = ImpactCraters::default();
    }

    /// A crater of `radius` at `at` once `start` comes round.
    fn dig(&mut self, at: Vec2, radius: f32, start: f32) {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let strength = 150 + (self.seed >> 25) % 90;
        let seed = (self.seed >> 9) & 0x7FFF;
        self.pending.push((
            start,
            StainInstance {
                pos: at.to_array(),
                radius,
                strength_seed: strength | seed << 8 | stain::CRATER,
            },
        ));
    }

    /// Lays the craters whose burst has landed by `time`. Another shell into one
    /// already there widens it a little and churns it darker, instead of piling copies.
    pub(super) fn land(&mut self, time: f32) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 > time {
                i += 1;
                continue;
            }
            let (_, crater) = self.pending.swap_remove(i);
            let at = Vec2::from(crater.pos);
            if let Some(old) = self
                .craters
                .iter_mut()
                .find(|c| Vec2::from(c.pos).distance(at) < 0.5 * c.radius.max(crater.radius))
            {
                let r = old.radius.max(crater.radius);
                old.radius = r.max((r * 1.06).min(crater.radius * 1.4));
                let strength = ((old.strength_seed & 0xFF) + 20).min(255);
                old.strength_seed = old.strength_seed & !0xFF | strength;
                continue;
            }
            if self.craters.len() < MAX_CRATERS {
                self.craters.push(crater);
            } else {
                self.craters[self.next] = crater;
                self.next = (self.next + 1) % MAX_CRATERS;
            }
        }
    }
}

impl Renderer {
    /// Digs a crater for a blast that struck the ground.
    pub(super) fn impact_crater(&mut self, event: &SimEvent, time: f32) {
        let (at, radius, start) = match event {
            SimEvent::Impact {
                pos,
                splash,
                after,
                on_unit,
                on_shield,
                blueprint,
                weapon,
                ..
            } => {
                if *on_unit || *on_shield {
                    return;
                }
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                if w.beam
                    || w.plasma_grade.is_some()
                    || w.heavy_rail > 0.0
                    || w.great_gun > 0.0
                    || w.burn_ticks > 0
                {
                    return;
                }
                let splash = splash.to_f32();
                if splash < MIN_SPLASH {
                    return;
                }
                let at = Vec3::from(pos.to_f32());
                // An air burst (flak, a proximity fuse short of its target) leaves the
                // ground whole.
                if at.z - self.ground_height(at.truncate()) > 1.0 + splash * 0.25 {
                    return;
                }
                let start = if w.hitscan {
                    time
                } else {
                    time + after.to_f32() * self.tick_seconds
                };
                (at, (splash * 0.3).clamp(1.5, 16.0), start)
            }
            SimEvent::AircraftCrashed { pos, blueprint } => {
                let r = self.blueprints.unit(*blueprint).radius.to_f32();
                (Vec3::from(pos.to_f32()), (r * 0.8).clamp(1.5, 12.0), time)
            }
            _ => return,
        };
        let ground = self.ground_height(at.truncate());
        if self.under_sea(at.truncate().extend(ground + 0.1)) {
            return;
        }
        self.impact_craters.dig(at.truncate(), radius, start);
    }
}
