//! A ship's hull going down (`WRECK_SINKING`): fire and smoke off the part of its
//! deck still out of the water, steam where the hot hull meets the sea, bubbles
//! off what is under it, and rings round it on the surface.
//!
//! Every point is on the hull as `entity.wgsl` draws it: the model's upper surface
//! (`models::burns::BurnGrid`) turned by the heading, then the sim's pitch and roll,
//! about the instance's origin (`wreck_fx::HullFrame`). So the fire stands on the deck it burns on, and a
//! stretch of deck under the water gets bubbles, never flame.

use super::{Renderer, PUFF_DROPLET, PUFF_STEAM};
use crate::camera::Camera;
use crate::renderer::wreck_fx::HullFrame;
use crate::renderer::{PUFF_FIRE, PUFF_SMOKE};
use glam::Vec3;
use mc_data::BlueprintId;
use mc_sim::mirror::UnitInstance;

impl Renderer {
    pub(super) fn sinking_hulls(&mut self, hulls: &[UnitInstance], time: f32, camera: &Camera) {
        let blueprints = self.blueprints.clone();
        let water = self.sea_level();
        let tick = self.tick_seconds.max(0.02);
        let reach = camera.distance * 2.5 + 400.0;
        let alive: Vec<u32> = hulls.iter().map(|u| u.unit_id).collect();
        self.water_fx.sinking.retain(|id, _| alive.contains(id));
        for u in hulls {
            let to = Vec3::from(u.pos);
            if to.distance(camera.focus) > reach {
                continue;
            }
            let bp = blueprints.unit(BlueprintId(u.blueprint as u16));
            let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
            // Along the hull as the model has it, and its deck at the middle.
            let (length, height) = match self.burn_sites.get(u.blueprint as usize) {
                Some(site) => (site.reach, site.height),
                None => (r, h),
            };
            let progress = u.health.clamp(0.0, 1.0);
            // Fire dies down as the hull goes; steam where it is still hot and meets the sea.
            let heat = (1.0 - progress * 1.6).clamp(0.0, 1.0);
            let mut crossing = false;
            for k in [-0.75f32, 0.0, 0.75] {
                let t = self.scatter.unit();
                let frame = HullFrame::of(u, t, true);
                let x = (k + self.scatter.signed() * 0.12) * length;
                let y = self.scatter.signed() * length * 0.1;
                let local = self
                    .burn_sites
                    .get(u.blueprint as usize)
                    .and_then(|site| site.grid.surface(x, y))
                    .map_or(Vec3::new(x, y, height * 0.55), |(p, _)| Vec3::from(p));
                let deck = frame.at(local);
                let above = deck.z - water;
                let start = time + t * tick;
                // A flame is only lit where all of it stands clear of the water: puffs
                // are drawn over the sea, so one reaching under it would burn on top
                // of the hull seen through the water.
                let flame = (r * 0.1).max(0.5);
                if above > flame + 0.6 && self.scatter.unit() < 0.7 * heat {
                    let rise = Vec3::new(0.5, 0.2, 2.0 + self.scatter.unit() * 2.0);
                    let grown = (r * 0.28).min(flame + above);
                    self.push_puff(PUFF_FIRE, deck, rise, start, 0.9, (flame, grown));
                }
                if above > 0.6 && self.scatter.unit() < 0.3 + 0.4 * heat {
                    let drift = Vec3::new(
                        1.4 + self.scatter.signed() * 0.6,
                        0.5 + self.scatter.signed() * 0.6,
                        2.6 + self.scatter.unit() * 1.5,
                    );
                    let size = (r * 0.15).min(above);
                    self.push_puff(
                        PUFF_SMOKE,
                        deck + Vec3::Z,
                        drift,
                        start,
                        4.2,
                        (size, r * 0.75),
                    );
                }
                // Steam where the hot deck is at the surface: not over a stretch of it
                // already deep under, which only bubbles.
                if above > -3.0 && above < h * 0.7 {
                    crossing = true;
                    let at = Vec3::new(deck.x, deck.y, water + 0.3);
                    if self.scatter.unit() < 0.25 + 0.6 * heat {
                        let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.5);
                        self.push_puff(PUFF_STEAM, at, drift, start, 2.6, (r * 0.1, r * 0.5));
                    }
                    if self.scatter.unit() < 0.5 {
                        let vel = self.scatter.upward(0.5) * (1.5 + self.scatter.unit() * 2.5);
                        self.push_puff(PUFF_DROPLET, at, vel, start, 1.2, (0.18, 0.35));
                    }
                }
                if above < -0.5 && self.scatter.unit() < 0.8 {
                    self.push_bubbles(deck, r * 0.25, 2, start, tick, 0.45);
                }
            }
            // Rings and foam round it while it is going through the surface, fainter as it goes.
            let last = self
                .water_fx
                .sinking
                .get(&u.unit_id)
                .copied()
                .unwrap_or(f32::MIN);
            let every = if crossing { 0.7 } else { 1.8 };
            if time - last >= every {
                self.water_fx.sinking.insert(u.unit_id, time);
                let foam = if crossing { 0.9 } else { 0.35 };
                let size = r * if crossing { 0.75 } else { 0.4 };
                self.push_ripple(
                    Vec3::new(to.x, to.y, water),
                    time,
                    size,
                    5.0,
                    0.0,
                    foam * (1.0 - progress * 0.6),
                );
                if !crossing {
                    // Air still escaping, reaching the surface in gulps.
                    self.push_bubbles(to + Vec3::Z * h * 0.3, r * 0.4, 6, time, 0.6, 0.6);
                }
            }
        }
    }
}
