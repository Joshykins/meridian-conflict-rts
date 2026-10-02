//! The Quiver, the Regency's tech 2 drone carrier (`regency_t2_drone_carrier`): a hull on
//! lift bells with six bays down its back, a Wick riding in each (the Wicks are units of
//! their own, drawn on their bays: `drone_sockets` in the unit file are `BAYS`). It carries
//! no gun. Each bay is a sunk cradle between two bronze rails, where the hull's nanites build
//! the spent Wick again.
//!
//! The shape is a manta: a broad flat delta, the bays sunk in its back, a sensor hump at
//! the nose, the trailing edge lapped in plates swept out to the tips, each tip turned down
//! into a blade, and three bells under it (one at the nose, one under each wing).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, hoop, swept, Course, Frame};
use super::{body, head, plan, team_mark};

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_drone_carrier",
    RADIUS,
    HEIGHT,
    manta,
)];

pub(crate) const RADIUS: f32 = 8.0;
pub(crate) const HEIGHT: f32 = 3.6;

/// The six bays, fore to aft, port first: where each Wick sits (the unit file's
/// `drone_sockets`).
pub(crate) const BAYS: [Vec3; 6] = [
    Vec3::new(2.3, 1.45, 2.05),
    Vec3::new(2.3, -1.45, 2.05),
    Vec3::new(-0.3, 1.6, 2.05),
    Vec3::new(-0.3, -1.6, 2.05),
    Vec3::new(-2.9, 1.45, 2.05),
    Vec3::new(-2.9, -1.45, 2.05),
];

/// The deck the bays are let into.
const DECK: f32 = 2.0;

/// One bay: a seam-dark cradle sunk in the deck, a bronze rail down each side, and at full
/// detail the bronze ring the nanites pour through.
fn bay(b: &mut MeshBuilder, at: Vec3) {
    seam(b);
    b.block(at - v3(1.25, 0.62, 0.22), at + v3(1.25, 0.62, 0.0));
    metal(b);
    let sides = b.sides(6);
    for y in [0.68f32, -0.68] {
        b.cylinder_between(
            at + v3(1.2, y, 0.06),
            at + v3(-1.2, y, 0.06),
            0.07,
            0.07,
            sides,
        );
    }
    if b.fine() {
        hoop(b, at + v3(0.0, 0.0, 0.01), 0.5, 0.06, 0.04, 10);
    }
}

fn bays(b: &mut MeshBuilder) {
    for at in BAYS {
        bay(b, at);
    }
}

/// Far off: a plan's flat solid, its bells as one dark face, the team colour on top.
fn coarse(b: &mut MeshBuilder, half: &[[f32; 2]], top: f32) {
    dark_plate(b);
    b.extrude_z(&plan(half), 0.5, top);
    team_mark(b, -0.5, top, 1.4);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(3.0, 0.0, 0.3), v3(-3.0, -2.0, 0.3), v3(-3.0, 2.0, 0.3)]);
    });
}

const MANTA_HULL: [[f32; 2]; 8] = [
    [6.8, 0.0],
    [4.6, 1.0],
    [-2.0, 5.6],
    [-4.6, 6.6],
    [-5.0, 5.8],
    [-3.8, 2.6],
    [-5.4, 1.0],
    [-5.6, 0.0],
];

/// The Quiver.
fn manta(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b, &MANTA_HULL, 3.0);
        return;
    }
    body(b, &MANTA_HULL, 0.95, DECK, 0.97);
    bays(b);
    // The sensor hump ahead of the bays, its head at the nose.
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan(&[[6.4, 0.0], [5.2, 0.7], [3.8, 0.9], [3.6, 0.0]]),
            &[
                crate::builder::Section::new(DECK - 0.1, 1.0),
                crate::builder::Section::new(3.1, 0.92).shifted(0.2, 0.0),
            ],
        )
    });
    head(b, 6.9, 1.9, 0.9);
    team_mark(b, 4.6, 3.1, 0.55);
    b.mirror_y(|b| {
        // The trailing edge lapped in plates swept out to the tips, and each tip turned
        // down into a blade.
        dark_plate(b);
        let f = Frame::new(v3(-0.8, 3.0, DECK - 0.02), v3(-0.75, 0.66, -0.04), Vec3::Z);
        Course {
            count: if b.fine() { 3 } else { 2 },
            step: if b.fine() { 1.2 } else { 2.4 },
            len: 2.2,
            half: 0.7,
            tip: 0.6,
            thick: 0.14,
            tail: 0.8,
        }
        .lay(b, &f);
        armour(
            b,
            &Frame::new(v3(-3.4, 5.6, 1.5), v3(-0.6, 0.35, -0.72), v3(0.2, 1.0, 0.0)),
            &swept(2.0, 0.55, 0.3, 0.5),
            0.16,
        );
        bell(b, v3(-2.6, 3.6, 0.0), 0.8, 1.0);
        if b.fine() {
            metal(b);
            let sides = b.sides(6);
            b.cylinder_between(v3(2.8, 0.9, 1.7), v3(-3.8, 2.3, 1.7), 0.09, 0.09, sides);
        }
    });
    bell(b, v3(3.6, 0.0, 0.0), 0.7, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;

    #[test]
    fn every_design_fits_the_librarys_checks_and_seats_the_wicks() {
        for key in MODELS.iter().map(|d| d.key) {
            super::super::super::check(key, RADIUS, HEIGHT, None, &[]);
            let model = build_model(key).unwrap();
            let mesh = &model.lods[0];
            for at in BAYS {
                // The bay's floor is under the Wick, and nothing stands over it.
                let over = mesh
                    .vertices
                    .iter()
                    .filter(|v| {
                        (v.pos[0] - at.x).abs() < 1.1
                            && (v.pos[1] - at.y).abs() < 0.55
                            && v.pos[2] > at.z + 0.1
                    })
                    .count();
                assert_eq!(over, 0, "{key}: something stands in the bay at {at}");
                let floor = mesh.vertices.iter().any(|v| {
                    (v.pos[0] - at.x).abs() < 1.3
                        && (v.pos[1] - at.y).abs() < 0.7
                        && (v.pos[2] - at.z).abs() < 0.05
                });
                assert!(floor, "{key}: no bay floor at {at}");
            }
            assert!(!model.lifts.is_empty(), "{key}: no lift bells");
        }
    }
}
