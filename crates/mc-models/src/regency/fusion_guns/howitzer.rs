//! The tech 3 heavy artillery: a Pinch-fusion Howitzer. A long turret on a bronze race,
//! the gun's trunnions in plated cheeks near its breech, the caged fusion core standing
//! on the turret's back behind them.
//!
//! The bore is split down its length into two plated halves that part through the
//! charge, red light let into their inner faces lighting from the breech, a gravity lens
//! at each half's tip. It rides the tracked, skirted body (`chassis`).
//!
//! The unit file's pivot and muzzle are [`LINE`]'s (`regency_t3_artillery`,
//! `data/factions/regency/units/land_t3.ron`).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::SPREAD;
use crate::material::*;
use crate::{part, pattern};

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{collar, red_slot, Course, Frame};
use super::super::turrets::sunspear;
use super::chassis::{body, Hull};
use super::{coarse_gun, fusion_core, team_patch, turret_hull, yoke, Line};

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 11.0;
pub(in crate::regency) const HEIGHT: f32 = 9.5;

pub(super) const HULL: Hull = Hull {
    nose: 9.0,
    tail: -8.6,
    half: 4.8,
    deck: 3.0,
};
/// The gun's line: trunnions behind the turret's middle, laid 18 degrees up at rest.
pub(super) const LINE: Line = Line {
    pivot: Vec3::new(-2.2, 0.0, 5.8),
    pitch_deg: 18.0,
    len: 15.0,
};
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
pub(super) const HOLD: f32 = 1.6;
/// The turret roof.
const ROOF: f32 = 4.0;
/// Metres the charge gear's travels are drawn at (its halves part 0.56 m).
const GEAR: f32 = 0.8;

pub(super) fn draw(b: &mut MeshBuilder) {
    LINE.rig(b, HULL.deck, 1.2);
    body(b, &HULL);
    if b.coarse() {
        coarse_gun(b, &LINE, HULL.deck, ROOF, 4.0, 1.2);
        return;
    }
    b.with_part(part::TURRET, |b| {
        turret_hull(
            b,
            &[[2.6, 1.9], [0.6, 2.8], [-5.0, 2.8], [-7.6, 1.6]],
            HULL.deck,
            ROOF,
        );
        team_patch(b, 0.0, 1.8, 1.0, ROOF + 0.01);
        yoke(
            b,
            LINE.pivot,
            1.9,
            0.5,
            [-3.6, -0.8],
            [ROOF - 0.1, LINE.pivot.z + 0.75],
        );
        fusion_core(b, v3(-6.0, 0.0, ROOF), 0.42, GEAR);
        if b.fine() {
            b.mirror_y(|b| red_slot(b, v3(-4.2, 2.81, 3.5), Vec3::Y, Vec3::X, 2.4, 0.12));
        }
        LINE.gun(b, |b| {
            breech(b);
            split(b);
        });
    });
}

/// The breech round the trunnion: a bronze pin housing and a plated block behind the
/// bore, short so the gun can lob high without its breech meeting the roof.
fn breech(b: &mut MeshBuilder) {
    collar(b, Vec3::ZERO, Vec3::Y, 0.7, 3.2);
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(-0.8, 0.0, 0.0),
            v3(2.2, 0.0, 0.0),
            Vec2::new(2.3, 1.6),
            Vec2::new(1.9, 1.4),
        )
    });
}

/// A `GLOW_LASER` brush lit with the charge at `stage` (0 first, 6 last).
fn coil_light(b: &mut MeshBuilder, stage: u32) {
    b.paint(GLOW_LASER).pattern(pattern::COIL + stage);
}

/// A bore split down its length into two plated halves that part through the charge, a
/// red-lit strip down each inner face lighting from the breech, a plate course along each
/// half's top and a gravity lens at each tip.
fn split(b: &mut MeshBuilder) {
    let len = LINE.len;
    let fine = b.fine();
    // A half's section, out from its flat inner face (`y` from 0 out, `z` up).
    let shape: &[[f32; 2]] = &[
        [0.0, 1.0],
        [0.55, 0.9],
        [1.0, 0.4],
        [1.0, -0.4],
        [0.55, -0.9],
        [0.0, -1.0],
    ];
    let stations = [
        (1.2, 1.0, 0.95),
        (3.2, 1.0, 0.95),
        (12.8, 0.78, 0.72),
        (14.2, 0.62, 0.6),
    ];
    // The bore's core inside the halves, and its seat under them.
    metal(b);
    b.cylinder_between(v3(1.4, 0.0, 0.0), v3(12.4, 0.0, 0.0), 0.22, 0.2, b.sides(8));
    dark_plate(b);
    b.block(v3(2.0, -0.4, -1.25), v3(6.0, 0.4, -0.85));
    b.mirror_y(|b| {
        b.with_charge_gear(SPREAD, |b| {
            let rings: Vec<Vec<Vec3>> = stations
                .iter()
                .map(|&(x, w, h)| {
                    shape
                        .iter()
                        .map(|&[y, z]| v3(x, 0.12 + y * w, z * h))
                        .collect()
                })
                .collect();
            dark_plate(b);
            b.with_facets(|b| b.loft(&rings, true, true));
            // The inner face lit stage by stage from the breech.
            let strips: Vec<u32> = if fine { (0..7).collect() } else { vec![2, 5] };
            for i in strips {
                let x = 2.0 + 1.6 * i as f32;
                coil_light(b, i);
                b.block(v3(x, 0.08, -0.25), v3(x + 1.2, 0.12, 0.25));
            }
            if fine {
                metal(b);
                for x in [2.6f32, 7.0, 11.4] {
                    b.block(v3(x, 0.4, -1.0), v3(x + 0.5, 1.05, 1.0));
                }
                dark_plate(b);
                Course {
                    count: 3,
                    step: 3.6,
                    len: 4.2,
                    half: 0.45,
                    tip: -0.4,
                    thick: 0.18,
                    tail: 0.6,
                }
                .lay(
                    b,
                    &Frame::new(v3(13.0, 0.55, 0.78), v3(-1.0, 0.0, 0.03), v3(0.0, 0.3, 1.0)),
                );
            }
            for z in [-0.3f32, 0.3] {
                sunspear::lens(b, v3(14.25, 0.45, z), v3(len, 0.0, 0.0), 0.26);
            }
        });
    });
}
