//! The Edict's shot: the star let go down its line to the mark. A column of pinch-fusion
//! plasma there at once (`fade_beam::HELD_EDICT`: a white-hot core in a roiling sheath going
//! from the prism's pink to red, swelling from the hull to its full width at the ground, the
//! air round it shimmering), held a
//! second and a quarter, then gone,
//! leaving a red filament along its line that cools and breaks up (`PLASMA_TRAIL`). At the
//! hull, where it leaves: a blinding flash, and a hard ring of pinched plasma thrown out
//! across the shot with a pressure ring behind it. The deck is cut along its line and round its mark
//! (sky/cut.rs) and thrown back round both. The strike at the mark is the ground's.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_sim::mirror::{ProjectileInstance, PROJECTILE_FADE_BEAM};

use super::super::heat_haze::GpuHeatPlume;
use super::super::plasma_fx::{held_instance, HELD_BEAM};
use super::super::star_core_fx::{STAR, WHITE};
use super::super::stun_fx::Flash;
use super::super::Renderer;
use crate::gpu_consts::{fade_beam, puff};

const KNOT: f32 = puff::PINCH_KNOT as f32;
/// The hard ring thrown out across the shot at the hull: its knots, how long it runs out,
/// how far it gets (metres) and how thick it is.
const RING_KNOTS: usize = 72;
const RING_RUN: f32 = 0.4;
const RING_REACH: f32 = 1300.0;
const RING_THICK: f32 = 7.0;
/// The column's width at the ground (metres, across), sheath and all; its white core is
/// a third to a half of it.
const WIDTH: f32 = 260.0;
/// Seconds the column holds (its last quarter fading), and when the red filament it leaves
/// takes over and how long that burns.
const COLUMN: f32 = 1.6;
const FILAMENT_AT: f32 = 1.05;
const FILAMENT: f32 = 4.0;
/// The filament's width.
const THREAD: f32 = 28.0;
/// The cut in the deck: the lane's half width, and the clearing blown round the mark
/// against the blast's radius (a ring's radius, zero for a round clearing, and its half
/// width).
const LANE: f32 = 180.0;
const RING: (f32, f32) = (0.0, 1.1);

/// A shot, from its firing point to its mark, fired at `start`.
#[derive(Clone, Copy)]
pub(super) struct Shot {
    from: Vec3,
    mark: Vec3,
    start: f32,
}

impl Shot {
    pub(super) fn alive(&self, time: f32) -> bool {
        time < self.start + FILAMENT_AT + FILAMENT
    }

    /// The air shimmering round the column while it holds, for the tone map.
    pub(super) fn shimmer(&self, time: f32) -> Option<GpuHeatPlume> {
        let age = time - self.start;
        if !(0.0..COLUMN).contains(&age) {
            return None;
        }
        let line = self.mark - self.from;
        Some(GpuHeatPlume {
            port: self.from.to_array(),
            radius: WIDTH * 0.45,
            axis: line.normalize_or(Vec3::NEG_Z).to_array(),
            height: line.length(),
            strength: 1.0 - age / COLUMN,
            seed: self.start.fract() * 7.0,
            kind: crate::gpu_consts::haze::KIND_HEAT,
            _pad: 0.0,
        })
    }
}

impl Renderer {
    /// The shot leaves the hull at `from` for `mark` (a blast `radius` round it).
    pub(super) fn edict_fired(&mut self, from: Vec3, mark: Vec3, radius: f32, time: f32) {
        let shot = Shot {
            from,
            mark,
            start: time,
        };
        self.edict_fx.shots.push(shot);
        self.edict_fx.exposure.glare(from, mark, time);
        let dir = (mark - from).normalize_or(Vec3::NEG_Z);
        // The release: a blinding white flash where the star was, the light of it over
        // everything for kilometres, white going to rose.
        self.push_lit(
            STAR,
            from - Vec3::Z * 40.0,
            Vec3::ZERO,
            time,
            0.35,
            (520.0, 520.0),
            Vec3::splat(70.0),
            1.7,
        );
        self.push_effect(from.to_array(), time, 900.0, 0.3, 9.0, 0.0);
        self.emp_fx.flashes.push(Flash {
            pos: from - Vec3::Z * 60.0,
            start: time,
            life: 0.35,
            color: WHITE * 1.5e8,
            range: 9000.0,
            flicker: 0.0,
        });
        self.emp_fx.flashes.push(Flash {
            pos: from.lerp(mark, 0.5),
            start: time,
            life: 0.6,
            color: Vec3::new(1.0, 0.7, 0.82) * 6.0e7,
            range: 7000.0,
            flicker: 0.15,
        });
        self.lens_flares.flash(
            from,
            super::super::lens_flare::Flare {
                color: super::star::FLARE * 60.0,
                size: 2400.0,
            },
            time,
            1.4,
        );
        // A hard ring of pressure across the shot where it leaves the hull.
        self.push_shockwave(from.to_array(), time, 1500.0, 1.5, 1.0, 1.0, dir);
        // The deck cut along its line and round its mark, and thrown back round both.
        let (a, b) = (from.truncate(), mark.truncate());
        self.sky
            .cut(a, b, LANE, (radius * RING.0, radius * RING.1), time);
        self.sky
            .blast(mark.truncate().extend(from.z), radius * 3.0, 3.0, time);
        self.sky.blast(from, 900.0, 2.0, time);
    }

    /// The hard ring thrown out across the shot where it leaves the hull, `age` seconds on:
    /// a band of pinched plasma knots edge-on round the shot's line, white going to rose
    /// as it runs out and thins.
    fn edict_ring(&mut self, s: &Shot, age: f32, time: f32) {
        let k = age / RING_RUN;
        if !(0.0..1.0).contains(&k) {
            return;
        }
        let dir = (s.mark - s.from).normalize_or(Vec3::NEG_Z);
        let across = dir.any_orthonormal_vector();
        let other = dir.cross(across);
        let reach = 60.0 + RING_REACH * (1.0 - (-age * 3.2).exp());
        let half = reach * TAU / RING_KNOTS as f32 * 0.75;
        let glow = 2.2 * (1.0 - k).powf(2.5);
        let rgb = Vec3::new(1.0, 0.95, 0.97).lerp(Vec3::new(1.0, 0.45, 0.62), k) * glow;
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        for i in 0..RING_KNOTS {
            // Torn as it runs out: knots drop out of it, more the further it has gone.
            if self.scatter.unit() < 0.15 + 0.5 * k {
                continue;
            }
            let flat = Vec2::from_angle(i as f32 * TAU / RING_KNOTS as f32);
            let out = across * flat.x + other * flat.y;
            self.push_lit(
                KNOT,
                s.from + out * reach,
                out,
                time,
                life,
                (half, RING_THICK * (1.0 + 2.0 * k) / half),
                rgb,
                0.0,
            );
        }
    }

    /// Each tick a shot burns: its column while it holds, then its filament.
    pub(super) fn edict_shot_tick(&mut self, s: &Shot, time: f32) {
        let age = time - s.start;
        self.edict_ring(s, age, time);
        let mut out = Vec::new();
        if age < COLUMN {
            let mut column = held_instance(
                HELD_BEAM,
                [s.from, s.from],
                [s.mark, s.mark],
                WIDTH * 0.5,
                s.start,
                COLUMN,
            );
            column.aim[3] = fade_beam::HELD_EDICT;
            out.push(column);
        }
        if age >= FILAMENT_AT - 0.1 {
            // A Pinched bolt's trail (`aim.w` 0): red, cooling and breaking up.
            out.push(ProjectileInstance {
                prev_pos: s.from.to_array(),
                color: PROJECTILE_FADE_BEAM | fade_beam::PLASMA_TRAIL,
                pos: s.mark.to_array(),
                size: THREAD,
                wake: s.start + FILAMENT_AT,
                plasma: FILAMENT,
                _pad: [0.0; 2],
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
            });
        }
        self.push_projectiles(&out);
    }
}
