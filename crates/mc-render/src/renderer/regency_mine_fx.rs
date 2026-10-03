//! A Regency mine at work (`Model::excavation`, `models::regency::taproot`): it digs with a
//! beam, not a hammer (docs/STYLE.md "The Regency suite": plasma bound by gravity).
//!
//! - **The excavation beam**: a held red-white stream (sprites.wgsl fade beam 7, as the
//!   Regency's held weapons) from the emitter's lens straight down into the bore's mouth,
//!   wider at every tier; the model carries its core on down the molten shaft. From tech
//!   2 thinner pinch beams run into the mouth from the emitters round it.
//! - **The ore drawn up**: glowing motes lifted out of the funnel, spiralling in and
//!   climbing the column to the collector rings, white-hot to red (the reclaim puff, whose
//!   drift and climb it shares), more of them every tier; molten spatter where the beam
//!   bites.
//! - **The deep core's surge**: every few seconds the beam flares wide, a flash goes down
//!   the shaft and a gout of ore and spatter comes up it. Nothing runs out over the
//!   ground: no shockwave (user call, 2026-09-30).
//!
//! Presentation only; the renderer's own clock. A mine under construction, a wreck, a
//! ghost or a radar blip does nothing.

use glam::Vec3;
use mc_sim::mirror::{ProjectileInstance, UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

use super::clearing::PUFF_RECLAIM;
use super::plasma_fx::{held_instance, HELD_BEAM};
use super::{Renderer, PUFF_SPARK};
use crate::camera::Camera;
use crate::models::{Excavation, Pit};

/// A pinch beam's width against the main beam's.
const PINCH: f32 = 0.3;
/// How long a held beam drawn this tick lasts if the next tick does not renew it.
const HOLD: f32 = 0.3;

/// The beam each blueprint's model digs with, and its bore (`None`: it digs no bore).
pub(super) struct RegencyMineFx {
    beams: Vec<Option<(Excavation, Pit)>>,
}

impl RegencyMineFx {
    /// `beams`: per blueprint, in id order.
    pub(super) fn new(beams: Vec<Option<(Excavation, Pit)>>) -> Self {
        Self { beams }
    }
}

/// One working mine this tick, in the world.
struct Dig {
    emitter: Vec3,
    mouth: Vec3,
    pinches: Vec<Vec3>,
    width: f32,
    radius: f32,
    tech: u8,
    surge: f32,
    seed: f32,
}

impl Renderer {
    /// Once a sim tick, after the held beams: draws every working Regency mine's beams and
    /// sends up its ore.
    pub(super) fn excavation_tick(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        if camera.distance > 3000.0 || self.regency_mine_fx.beams.iter().all(Option::is_none) {
            return;
        }
        let hidden = KIND_WRECK
            | KIND_GHOST
            | STATE_RADAR
            | (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        let reach = camera.distance * 2.5 + 400.0;
        let focus = camera.focus.truncate();
        let close = camera.distance < 1400.0;
        let mut digs = Vec::new();
        for u in units {
            if u.owner_flags & hidden != 0 {
                continue;
            }
            let Some(Some((beam, pit))) = self.regency_mine_fx.beams.get(u.blueprint as usize)
            else {
                continue;
            };
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            let (s, c) = u.heading.sin_cos();
            let world =
                |p: [f32; 3]| at + Vec3::new(p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]);
            // Just under the opening, still over the ground drawn across the hole: the
            // model's core carries it on down the shaft.
            let mouth = at + Vec3::Z * (pit.open - 0.2);
            let bp = self
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16));
            digs.push(Dig {
                emitter: world(beam.emitter),
                mouth,
                pinches: beam.pinches.iter().map(|&p| world(p)).collect(),
                width: beam.width,
                radius: pit.radius,
                tech: bp.tech,
                surge: beam.surge,
                seed: (u.unit_id % 97) as f32 * 0.613,
            });
        }
        let mut out: Vec<ProjectileInstance> = Vec::new();
        for dig in &digs {
            self.dig(dig, time, close, &mut out);
        }
        self.push_projectiles(&out);
    }

    fn dig(&mut self, d: &Dig, time: f32, close: bool, out: &mut Vec<ProjectileInstance>) {
        let beam = |from: Vec3, width: f32, life: f32| {
            held_instance(HELD_BEAM, [from; 2], [d.mouth; 2], width, time, life)
        };
        out.push(beam(d.emitter, d.width, HOLD));
        for &p in &d.pinches {
            out.push(beam(p, d.width * PINCH, HOLD));
        }
        let previous = self.effect_origin;
        self.effect_origin = Some(d.mouth);
        // The ore drawn up the column: it leaves the mouth turning in round the beam and
        // climbs (the reclaim puff's drag and lift) to the collector as it cools.
        let climb = (d.emitter.z - d.mouth.z).max(1.0);
        let life = (climb / 1.6).sqrt().clamp(1.0, 4.0);
        let motes = if close {
            2 + 2 * d.tech as usize
        } else {
            1 + d.tech as usize / 2
        };
        for _ in 0..motes {
            self.ore_mote(d, time, life);
        }
        if close {
            // Molten spatter thrown up where it bites, more of it the hotter the bore.
            for _ in 0..1 + d.tech / 2 {
                self.spatter(d, time, 1.0);
            }
        }
        if d.surge > 0.0 {
            // The deep core surges on its own beat: the beam flares wide, a flash goes down
            // the shaft, and a small ring of pressure runs out over the ground.
            let t = time + d.seed;
            let tick = self.tick_seconds.max(0.02);
            if (t / d.surge).floor() != ((t - tick) / d.surge).floor() {
                out.push(beam(d.emitter, d.width * 2.4, 0.7));
                self.push_effect(d.mouth.to_array(), time, 14.0, 0.35, 1.0, 0.0);
                for _ in 0..18 {
                    self.ore_mote(d, time, life * 0.8);
                }
                for _ in 0..10 {
                    self.spatter(d, time, 1.6);
                }
            }
        }
        self.effect_origin = previous;
    }

    /// A fleck of molten rock thrown up out of the funnel, `kick` times the usual speed.
    fn spatter(&mut self, d: &Dig, time: f32, kick: f32) {
        let spray =
            Vec3::new(self.scatter.signed(), self.scatter.signed(), 2.2).normalize_or_zero();
        let speed = (7.0 + self.scatter.unit() * 8.0) * kick;
        self.push_puff(
            PUFF_SPARK,
            d.mouth + Vec3::Z * 0.3,
            spray * speed,
            time,
            0.6,
            (0.24, 0.1),
        );
    }

    /// One glowing mote of ore leaving the funnel, turning in round the beam as it climbs.
    fn ore_mote(&mut self, d: &Dig, time: f32, life: f32) {
        let a = self.scatter.unit() * std::f32::consts::TAU;
        let out = Vec3::new(a.cos(), a.sin(), 0.0);
        let round = Vec3::new(-a.sin(), a.cos(), 0.0);
        // From the funnel's lower walls, inside where the eye looks into it.
        let r = d.radius * (0.2 + 0.4 * self.scatter.unit());
        let vel = round * (2.0 + 2.0 * self.scatter.unit()) - out * 0.6 + Vec3::Z * 3.0;
        let size = 0.36 + 0.1 * d.tech as f32;
        let late = self.scatter.unit() * self.tick_seconds;
        let lasts = life * (0.8 + 0.3 * self.scatter.unit());
        self.push_puff(
            PUFF_RECLAIM,
            d.mouth + out * r + Vec3::Z * 0.3,
            vel,
            time + late,
            lasts,
            (size, size * 0.4),
        );
    }
}
