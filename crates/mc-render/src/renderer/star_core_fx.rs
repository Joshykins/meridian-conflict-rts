//! A Regency power generator's star while the plant runs (`Model::star_core`,
//! `models::regency::heart`): light, not a solid. Each tick it lays the star
//! (plasma_puffs.wgsl `star_core`: a ball of fusing plasma boiling in cells, white-hot at
//! the heart, the prism's pinks drifting over it, a ragged flickering corona), now and then
//! a filament of lightning cracking off it into the cage, and it lights the cage round it in
//! the prism's pink. When a burning star's cage is breached it goes supernova
//! (`supernova_fx.rs`).
//!
//! Presentation only; the renderer's own clock. A plant under construction, unpowered, a
//! wreck, a ghost or a radar blip has no star.

use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR, STATE_UNPOWERED};

use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::puff;

pub(super) const STAR: f32 = puff::STAR_CORE as f32;
pub(super) const ARC: f32 = puff::WARP_ARC as f32;
/// The star's light on its cage: the prism's rose.
pub(super) const ROSE: Vec3 = Vec3::new(1.0, 0.5, 0.78);
/// The arcs it throws: white into pale lavender.
pub(super) const WHITE: Vec3 = Vec3::new(1.0, 0.95, 1.0);
pub(super) const LAVENDER: Vec3 = Vec3::new(0.8, 0.62, 1.0);
/// How bright the star burns (`appearance.rgb` of its puff).
pub(super) const GLARE: f32 = 1.6;

/// A star burning this tick: where its plant stands, its middle, its face's radius and
/// its own seed.
#[derive(Clone, Copy)]
pub(super) struct Burning {
    pub(super) at: Vec3,
    pub(super) centre: Vec3,
    pub(super) r: f32,
    pub(super) seed: f32,
}

/// Each blueprint's star (`None`: it has none), the stars burning this tick, and the
/// ones going supernova.
pub(super) struct StarCoreFx {
    stars: Vec<Option<[f32; 4]>>,
    pub(super) burning: Vec<Burning>,
    pub(super) novae: Vec<super::supernova_fx::Nova>,
}

impl StarCoreFx {
    /// `stars`: per blueprint, in id order.
    pub(super) fn new(stars: Vec<Option<[f32; 4]>>) -> Self {
        Self {
            stars,
            burning: Vec::new(),
            novae: Vec::new(),
        }
    }
}

impl Renderer {
    /// Once a sim tick: lays every running Regency power generator's star and its arcs.
    pub(super) fn star_core_tick(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        self.star_core_fx.burning.clear();
        self.novae_tick(time);
        if self.star_core_fx.stars.iter().all(Option::is_none) {
            return;
        }
        let hidden = KIND_WRECK
            | KIND_GHOST
            | STATE_RADAR
            | STATE_UNPOWERED
            | (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        let reach = camera.distance * 2.5 + 600.0;
        let focus = camera.focus.truncate();
        let close = camera.distance < 1400.0;
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        for u in units {
            if u.owner_flags & hidden != 0 {
                continue;
            }
            let Some(Some(star)) = self.star_core_fx.stars.get(u.blueprint as usize) else {
                continue;
            };
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            let (s, c) = u.heading.sin_cos();
            let centre = at
                + Vec3::new(
                    star[0] * c - star[1] * s,
                    star[0] * s + star[1] * c,
                    star[2],
                );
            let r = star[3];
            let seed = (u.unit_id % 97) as f32 * 0.613;
            self.star_core_fx.burning.push(Burning {
                at,
                centre,
                r,
                seed,
            });
            // The quad holds the corona too: the face is the middle 0.42 of it.
            let across = r / 0.42;
            self.push_lit(
                STAR,
                centre,
                Vec3::ZERO,
                time,
                life,
                (across, across),
                Vec3::splat(GLARE),
                seed,
            );
            if close && self.scatter.unit() < 0.22 {
                // A filament of lightning cracking off the star into the cage.
                let out = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed() * 0.7,
                )
                .normalize_or(Vec3::Z);
                let tint = WHITE.lerp(LAVENDER, self.scatter.unit());
                let length = r * (0.9 + 1.2 * self.scatter.unit());
                let lasts = 0.07 + 0.06 * self.scatter.unit();
                self.push_lit(
                    ARC,
                    centre + out * r * 0.9,
                    out * length,
                    time,
                    lasts,
                    (r * 0.16, r * 0.16),
                    tint * 4.0,
                    0.0,
                );
            }
        }
    }

    /// Every frame: the burning stars light their cages, breathing with the star's beat.
    pub(super) fn star_core_lights(&mut self, time: f32) {
        for b in &self.star_core_fx.burning {
            let beat =
                0.85 + 0.1 * (time * 1.3 + b.seed).sin() + 0.05 * (time * 3.7 + b.seed * 2.0).sin();
            self.lights.lamp(
                b.centre,
                Vec3::NEG_Z,
                ROSE * (9.0 * b.r * beat),
                b.r * 7.0,
                180.0,
                1.0,
            );
        }
        self.novae_lights(time);
    }
}
