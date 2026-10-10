//! The well: the strike shattering the ground, the field holding (slabs lifted and
//! grinding, shards of rock circling, lightning crawling over the ground and onto what it
//! crushes), aircraft dragged down out of it, and the field let go.

use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::engine::{fx_lamp, BOLT};
use super::slabs::{shard_model, slab_model, Motion, Piece};
use super::{Engine, LILAC, VIOLET, WHITE};
use crate::camera::Camera;
use crate::gpu_consts::{effect, puff, rift};
use crate::renderer::lens_flare::Flare;
use crate::renderer::stun_fx::Flash;
use crate::renderer::{Renderer, PUFF_CLOD, PUFF_DUST, PUFF_SHOCK_DUST};
use mc_sim::mirror::{RenderFrame, WellView, KIND_GHOST, KIND_PROP, KIND_WRECK};

const MOTE: f32 = puff::WARP_MOTE as f32;
/// Slabs lifted over a well and shards circling it: a deliberate cosmetic cap.
const WELL_SLABS: usize = 95;
const WELL_SHARDS: usize = 60;
/// Units in a well given arcs of their own each tick, at most (cosmetic).
const MOST_CRUSHED: usize = 12;

impl Renderer {
    /// A blow through the ground: dust driven out over it in a ring and the trees pushed
    /// over by it, but no sphere of bent air (that reads as a bubble round it).
    pub(super) fn gravity_wave(
        &mut self,
        at: Vec3,
        time: f32,
        radius: f32,
        life: f32,
        strength: f32,
    ) {
        self.tree_blasts.record(at, time, radius, strength, false);
        self.shockwave_ground_dust(at, time, radius, life, strength, Vec3::ZERO);
    }

    /// The rift strikes `pos`: the ground shatters `radius` round and the well takes hold.
    pub(super) fn well_struck(&mut self, unit: u32, pos: Vec3, radius: f32, time: f32) {
        let r = radius.max(20.0);
        let centre = pos.truncate();
        let ground = centre.extend(self.ground_height(centre).max(pos.z));
        self.gravitic_engine_fx.ground.well(unit, centre, r, time);
        // A hard violet-white flash, gone in a breath; the light lingers violet in the cracks.
        let heart = ground + Vec3::Z * r * 0.12;
        self.push_effect(
            heart.to_array(),
            time,
            r * 0.35,
            0.45,
            effect::GRAVITIC as f32,
            0.0,
        );
        self.push_effect(
            heart.to_array(),
            time + 0.05,
            r * 0.2,
            0.6,
            effect::GRAVITIC as f32,
            0.0,
        );
        self.lens_flares.flash(
            heart,
            Flare {
                color: WHITE * 20.0,
                size: r * 0.9,
            },
            time,
            0.7,
        );
        self.emp_fx.flashes.push(Flash {
            pos: heart,
            start: time,
            life: 0.9,
            color: WHITE.lerp(LILAC, 0.5) * 5.0e4,
            range: (r * 3.0).min(1200.0),
            flicker: 0.2,
        });
        self.gravity_wave(ground + Vec3::Z * 3.0, time, r * 1.5, 1.8, 1.0);
        // Dust driven out over the ground from the shattering plates.
        for i in 0..56 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.6) / 56.0;
            let dir = Vec2::new(a.cos(), a.sin());
            for band in 0..2 {
                let at =
                    centre + dir * r * (0.25 + 0.45 * band as f32 + 0.15 * self.scatter.unit());
                let at = at.extend(self.ground_height(at));
                let born = time + at.truncate().distance(centre) / (r * 3.0);
                let speed = r * (0.35 + 0.3 * self.scatter.unit());
                let life = 4.0 + 2.0 * self.scatter.unit();
                self.push_puff(
                    if band == 0 {
                        PUFF_DUST
                    } else {
                        PUFF_SHOCK_DUST
                    },
                    at,
                    dir.extend(0.08) * speed,
                    born,
                    life,
                    (r * 0.05, r * 0.16),
                );
            }
        }
        // Earth and rock thrown up off the plates as they break.
        for _ in 0..48 {
            let at = centre + Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.7;
            let at = at.extend(self.ground_height(at));
            let vel = Vec3::new(
                self.scatter.signed() * 10.0,
                self.scatter.signed() * 10.0,
                25.0 + 30.0 * self.scatter.unit(),
            );
            let born = time + at.truncate().distance(centre) / (r * 3.0);
            let size = 1.5 + self.scatter.unit();
            self.push_puff(PUFF_CLOD, at, vel, born, 3.0, (size, 0.8));
        }
        // Lightning run out over the breaking ground.
        for _ in 0..18 {
            let a = TAU * self.scatter.unit();
            let to = centre + Vec2::new(a.cos(), a.sin()) * r * (0.5 + 0.5 * self.scatter.unit());
            let to = to.extend(self.ground_height(to) + 1.0);
            let side = (to - ground).cross(Vec3::Z).normalize_or(Vec3::X);
            let start = time + self.scatter.unit() * 0.25;
            self.emp_kinks(
                ground + Vec3::Z * 2.0,
                to,
                r * 0.05,
                side,
                Vec3::Z * 0.3,
                start,
                0.3,
                2.2,
                12,
                BOLT,
            );
        }
        self.well_rock(unit, centre, ground.z, r, time);
    }

    /// The well's plates lifting as the shatter passes them from `time`, and shards of rock
    /// rising and starting round.
    pub(super) fn well_rock(&mut self, unit: u32, centre: Vec2, ground_z: f32, r: f32, time: f32) {
        let ground = centre.extend(ground_z);
        for i in 0..WELL_SLABS {
            let d =
                r * (0.03 + 0.95 * ((i as f32 + self.scatter.unit()) / WELL_SLABS as f32).sqrt());
            let a = TAU * self.scatter.unit();
            let at = centre + Vec2::new(a.cos(), a.sin()) * d;
            let inner = 1.0 - d / r;
            // Mostly middling plates, a few great ones and a scatter of small.
            let scale = 0.6 + 3.6 * self.scatter.unit().powi(2);
            let steep = if self.scatter.unit() < 0.2 { 1.1 } else { 0.45 };
            let piece = Piece {
                ground: ground.z,
                engine: unit,
                model: slab_model(self.scatter.unit()),
                at,
                heading: TAU * self.scatter.unit(),
                scale,
                born: time + d / (r * 3.0),
                rise: 1.5 + 2.5 * self.scatter.unit(),
                lift: 1.5 + 22.0 * inner * self.scatter.unit().powi(2) + 3.0 * self.scatter.unit(),
                pitch: steep * self.scatter.signed() * (0.4 + inner),
                roll: steep * self.scatter.signed() * (0.4 + inner),
                motion: Motion::Hover,
                seed: self.scatter.unit(),
            };
            self.gravitic_engine_fx.slabs.add(piece);
        }
        for _ in 0..WELL_SHARDS {
            let d = r * (0.1 + 0.85 * self.scatter.unit());
            let a = TAU * self.scatter.unit();
            let piece = Piece {
                ground: ground.z,
                engine: unit,
                model: shard_model(self.scatter.unit()),
                at: centre + Vec2::new(a.cos(), a.sin()) * d,
                heading: TAU * self.scatter.unit(),
                scale: 0.5 + 1.8 * self.scatter.unit().powi(2),
                born: time + d / (r * 3.0),
                rise: 3.0 + 5.0 * self.scatter.unit(),
                lift: 8.0 + 70.0 * self.scatter.unit().powi(2),
                pitch: self.scatter.signed(),
                roll: self.scatter.signed(),
                motion: Motion::Orbit {
                    centre,
                    rate: (0.03 + 0.07 * self.scatter.unit()) * (1.0 - 0.5 * d / r),
                },
                seed: self.scatter.unit(),
            };
            self.gravitic_engine_fx.slabs.add(piece);
        }
    }

    /// A tick of the well holding.
    pub(super) fn well_holding(
        &mut self,
        e: &Engine,
        v: &WellView,
        frame: &RenderFrame,
        time: f32,
        camera: &Camera,
    ) {
        let r = v.radius.max(20.0);
        let centre = Vec3::from(v.mark).truncate();
        let powered = v.powered;
        let flicker = if powered {
            1.0
        } else {
            0.2 + 0.8 * self.scatter.unit().powi(3)
        };
        let fx = &mut self.gravitic_engine_fx;
        let dim = if powered { 1.0 } else { 0.3 } * flicker;
        fx.ground.feed(
            e.id,
            rift::WELL,
            [0.6 * dim, 0.0, 0.6, 0.0, if powered { 1.0 } else { 0.6 }],
        );
        fx.ground.feed(
            e.id,
            rift::LINE,
            [0.45 * dim, if powered { 1.0 } else { 0.2 }, 0.3, 0.0, 0.8],
        );
        let ground = centre.extend(self.ground_height(centre).max(v.mark[2]));
        fx_lamp(
            self,
            ground + Vec3::Z * r * 0.25,
            VIOLET * 60_000.0 * dim,
            r * 1.3,
        );
        if ground.truncate().distance(camera.focus.truncate()) > camera.distance * 2.5 + 2000.0 {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        // Lightning crawling over the ground, hugging it.
        let arcs = ((1.0 + 3.0 * self.scatter.unit()) * dim) as usize;
        for _ in 0..arcs {
            let a = TAU * self.scatter.unit();
            let from = centre + Vec2::new(a.cos(), a.sin()) * r * self.scatter.unit().sqrt();
            let b = TAU * self.scatter.unit();
            let to = from + Vec2::new(b.cos(), b.sin()) * r * (0.1 + 0.2 * self.scatter.unit());
            let n = 8;
            let start = time + self.scatter.unit() * dt;
            let life = 0.12 + 0.15 * self.scatter.unit();
            let mut last = from.extend(self.ground_height(from) + 1.0);
            for k in 1..=n {
                let t = k as f32 / n as f32;
                let p = from.lerp(to, t)
                    + Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.02;
                let next = p.extend(self.ground_height(p) + 0.6 + 2.0 * self.scatter.unit());
                self.emp_kinks(last, next, 0.8, Vec3::X, Vec3::Y, start, life, 1.1, 2, BOLT);
                last = next;
            }
            self.emp_fx.flashes.push(Flash {
                pos: last + Vec3::Z * 3.0,
                start,
                life: life * 1.5,
                color: LILAC * 2500.0 * dim,
                range: r * 0.35,
                flicker: 1.0,
            });
        }
        // Violet motes rising off the cracks, and a low haze of dust stirred off the plates.
        for _ in 0..(3.0 * dim) as usize {
            let at = centre + Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.8;
            let at = at.extend(self.ground_height(at) + 1.0);
            let rise = Vec3::Z * (8.0 + 10.0 * self.scatter.unit());
            let born = time + self.scatter.unit() * dt;
            self.push_lit(MOTE, at, rise, born, 1.5, (0.8, 0.4), VIOLET * 3.0, 0.0);
        }
        if self.scatter.unit() < 0.5 {
            let at = centre + Vec2::new(self.scatter.signed(), self.scatter.signed()) * r * 0.8;
            let at = at.extend(self.ground_height(at));
            self.push_puff(
                PUFF_DUST,
                at,
                Vec3::Z * 1.5,
                time,
                6.0,
                (r * 0.05, r * 0.15),
            );
        }
        self.crushed_arcs(e, centre, r, frame, dim, time);
    }

    /// Faint violet arcs onto the enemy units the well crushes, near the eye.
    fn crushed_arcs(
        &mut self,
        e: &Engine,
        centre: Vec2,
        r: f32,
        frame: &RenderFrame,
        dim: f32,
        time: f32,
    ) {
        let dt = self.tick_seconds.max(0.02);
        let hidden = KIND_WRECK | KIND_PROP | KIND_GHOST;
        let mut crushed: Vec<(Vec3, f32)> = frame
            .units
            .iter()
            .filter(|u| u.owner_flags & hidden == 0 && (u.owner_flags & 0xFF) as u8 != e.owner)
            .filter(|u| Vec3::from(u.pos).truncate().distance(centre) < r)
            // On the ground only: an arc up to an aircraft would be a long beam, and the
            // frame cannot tell one from a spacecraft, which the well never touches.
            .filter(|u| {
                u.pos[2] - self.ground_height(Vec2::new(u.pos[0], u.pos[1])) < u.radius * 3.0 + 4.0
            })
            .map(|u| (Vec3::from(u.pos), u.radius.max(2.0)))
            .collect();
        // A deliberate cap (cosmetic): the first few in the list get arcs.
        crushed.truncate(MOST_CRUSHED);
        for (pos, size) in crushed {
            if self.scatter.unit() > 0.4 * dim {
                continue;
            }
            let foot = pos.truncate()
                + Vec2::new(self.scatter.signed(), self.scatter.signed()) * size * 2.0;
            let foot = foot.extend(self.ground_height(foot));
            let top = pos + Vec3::Z * size * (0.4 + 0.6 * self.scatter.unit());
            let start = time + self.scatter.unit() * dt;
            self.emp_kinks(
                foot,
                top,
                size * 0.3,
                Vec3::X,
                Vec3::Y,
                start,
                0.1,
                0.4 + size * 0.05,
                4,
                BOLT,
            );
        }
    }

    /// The engine let go: its well collapses (`radius` over zero), or its rift or its
    /// gathering dies (`radius` zero).
    pub(super) fn well_released(&mut self, unit: u32, pos: Vec3, radius: f32, time: f32) {
        let fx = &mut self.gravitic_engine_fx;
        fx.ground.release(unit, time);
        fx.slabs.release(unit, time);
        if radius <= 0.0 {
            return;
        }
        // The field goes out with a last dull pulse; the dust of the falling plates rises.
        let centre = pos.truncate();
        let ground = centre.extend(self.ground_height(centre).max(pos.z));
        self.emp_fx.flashes.push(Flash {
            pos: ground + Vec3::Z * radius * 0.2,
            start: time,
            life: 1.2,
            color: VIOLET * 2.0e4,
            range: radius * 1.5,
            flicker: 0.5,
        });
        for _ in 0..40 {
            let at =
                centre + Vec2::new(self.scatter.signed(), self.scatter.signed()) * radius * 0.85;
            let at = at.extend(self.ground_height(at));
            let born = time + 0.4 + 0.8 * self.scatter.unit();
            self.push_puff(
                PUFF_DUST,
                at,
                Vec3::Z * 3.0,
                born,
                5.0,
                (radius * 0.04, radius * 0.14),
            );
        }
    }
}
