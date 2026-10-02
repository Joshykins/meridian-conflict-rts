//! The Reaper, the Regency's tech 3 beam assault craft (`regency_t3_assault_aircraft`): a
//! heavy craft on six lift bells that comes in over its mark and hangs there, sweeping a
//! Pinch-fusion Beam across the ground from a projector under its nose.
//!
//! The projector (`projector`) turns in a ball under the nose (`part::TURRET`) and pitches
//! in it (`rig::ARM_GUN`) about `PIVOT`; its red lens is the muzzle (the unit file's
//! `pivot` and `muzzle`).
//!
//! The shape is a scythe: a long plated hull under a high keel, and a crescent wing either
//! side swept back like a scythe's blade into a down-turned tip, three bells under each.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, collar, shaft, swept, Course, Frame};
use super::{body, head, plan, team_mark};

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_assault_aircraft",
    RADIUS,
    HEIGHT,
    scythe,
)];

pub(crate) const RADIUS: f32 = 14.0;
pub(crate) const HEIGHT: f32 = 6.0;

/// The projector's trunnion and its lens (the unit file's weapon `pivot` and `muzzle`).
pub(crate) const PIVOT: Vec3 = Vec3::new(4.2, 0.0, 0.6);
pub(crate) const MUZZLE: Vec3 = Vec3::new(6.4, 0.0, 0.2);

/// The beam projector in its ball under the nose: a plated housing, a bronze emitter tube
/// ringed with collars, and the red lens at its mouth.
fn projector(b: &mut MeshBuilder) {
    b.set_turret_pivot(PIVOT);
    b.set_arm_pivot(PIVOT);
    let dir = (MUZZLE - PIVOT).normalize();
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        if b.coarse() {
            // Far off: one tapered block from the ball to the lens.
            b.with_limb(rig::ARM_GUN, |b| {
                b.beam(PIVOT, MUZZLE, Vec2::new(0.9, 0.8), Vec2::new(0.3, 0.3));
            });
            return;
        }
        let sides = b.sides(10);
        b.spheroid(PIVOT, v3(0.62, 0.58, 0.5), sides, 5);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                PIVOT + dir * 0.2,
                PIVOT + dir * 1.4,
                Vec2::new(0.62, 0.56),
                Vec2::new(0.42, 0.4),
            );
            metal(b);
            let sides = b.sides(8);
            b.cylinder_between(PIVOT + dir * 1.3, MUZZLE - dir * 0.18, 0.15, 0.13, sides);
            for k in [1.55f32, 1.8] {
                collar(b, PIVOT + dir * k, dir, 0.22, 0.08);
            }
            b.paint(GLOW_LASER);
            b.cylinder_between(MUZZLE - dir * 0.2, MUZZLE, 0.21, 0.15, sides);
        });
    });
}

/// Far off: a plan's flat solid, the team colour on top, one dark face for the bells, and
/// the projector.
fn coarse(b: &mut MeshBuilder, outline: &[[f32; 2]], top: f32) {
    dark_plate(b);
    b.extrude_z(outline, 1.0, top);
    team_mark(b, -1.0, top, 2.4);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(4.0, 0.0, 0.3), v3(-5.0, -4.0, 0.3), v3(-5.0, 4.0, 0.3)]);
    });
    projector(b);
}

/// A plated keel along the back from `fore` to `aft`, `top` high, lapped in a course of
/// plates that runs past its end into a spike.
fn keel(b: &mut MeshBuilder, fore: f32, aft: f32, deck: f32, top: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan(&[[fore, 0.0], [fore - 1.2, 0.7], [aft + 1.0, 0.8], [aft, 0.0]]),
            &[Section::new(deck - 0.1, 1.0), Section::new(top, 0.7)],
        )
    });
    let len = fore - aft;
    let f = Frame::new(v3(fore - 0.6, 0.0, top), v3(-1.0, 0.0, -0.03), Vec3::Z);
    Course {
        count: if b.fine() { 4 } else { 2 },
        step: if b.fine() { len / 4.4 } else { len / 2.2 },
        len: len / 3.6,
        half: 0.55,
        tip: 0.0,
        thick: 0.18,
        tail: 2.0,
    }
    .lay(b, &f);
}

const SCYTHE_HULL: [[f32; 2]; 6] = [
    [8.6, 0.0],
    [6.6, 1.2],
    [3.0, 2.0],
    [-4.0, 2.1],
    [-8.2, 1.2],
    [-9.6, 0.0],
];

/// The crescent wing's outline, root to tip along the leading edge and back.
const SCYTHE_WING: [[f32; 2]; 9] = [
    [3.2, 1.8],
    [1.6, 5.0],
    [-1.4, 8.4],
    [-5.4, 11.4],
    [-9.8, 12.6],
    [-8.2, 10.2],
    [-6.6, 7.4],
    [-6.2, 4.4],
    [-6.6, 1.8],
];

fn scythe(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        let outline = plan(&[
            [8.6, 0.0],
            [3.2, 2.0],
            [-5.4, 11.4],
            [-9.8, 12.6],
            [-6.4, 4.0],
            [-9.6, 0.0],
        ]);
        coarse(b, &outline, 4.9);
        return;
    }
    body(b, &SCYTHE_HULL, 1.0, 3.8, 0.82);
    keel(b, 5.6, -8.6, 3.7, 5.0);
    head(b, 8.6, 2.6, 1.5);
    team_mark(b, -1.5, 5.18, 0.7);
    b.mirror_y(|b| {
        seam(b);
        b.with_facets(|b| b.extrude_z(&SCYTHE_WING, 1.35, 1.6));
        dark_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &SCYTHE_WING,
                &[Section::new(1.6, 1.0), Section::new(2.15, 1.0)],
            )
        });
        // Plates lapped back along the blade, the last a spike past the tip.
        let f = Frame::new(v3(1.4, 4.4, 2.15), v3(-0.62, 0.78, 0.0), Vec3::Z);
        Course {
            count: if b.fine() { 3 } else { 2 },
            step: if b.fine() { 3.1 } else { 6.2 },
            len: 3.4,
            half: 0.9,
            tip: -0.6,
            thick: 0.18,
            tail: 1.6,
        }
        .lay(b, &f);
        // The tip turned down into a blade.
        armour(
            b,
            &Frame::new(v3(-8.6, 11.6, 1.6), v3(-0.5, 0.3, -0.8), v3(0.2, 1.0, 0.1)),
            &swept(1.5, 0.6, 0.3, 0.5),
            0.16,
        );
        // The lift's machinery along the wing root, in bronze.
        if b.fine() {
            metal(b);
            shaft(b, v3(1.0, 2.2, 1.5), v3(-5.6, 2.3, 1.5), 0.16);
            collar(b, v3(-2.0, 2.25, 1.5), Vec3::X, 0.26, 0.2);
        }
        bell(b, v3(0.2, 4.6, 0.0), 1.0, 1.4);
        bell(b, v3(-3.3, 7.9, 0.0), 0.95, 1.4);
        bell(b, v3(-6.9, 10.6, 0.0), 0.85, 1.4);
    });
    projector(b);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_fits_the_librarys_checks() {
        for key in MODELS.iter().map(|d| d.key) {
            super::super::super::check(key, RADIUS, HEIGHT, None, &[MUZZLE.to_array()]);
            let model = crate::build_model(key).unwrap();
            assert_eq!(model.lifts.len(), 6, "{key}: six lift bells");
            assert_eq!(Vec3::from(model.turret_pivot), PIVOT, "{key}: turret pivot");
        }
    }
}
