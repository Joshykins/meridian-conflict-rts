//! Design C: a slim hull with a raked dorsal keel aft, the lance sheathed in plates lapped
//! back like feathers, its mouth three claws closed round the charge, carried in a low
//! house between two raked fins.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::{part, rig};

use super::super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{armour, collar, swept, Course, Frame};
use super::{coarse, hull, lifts, rig as set_rig, slot, team, DECK, PIVOT};

/// The hull's plan: a slim lozenge, pointed fore, its stern cut square between two corners.
const HULL: [[f32; 2]; 9] = [
    [7.2, 0.0],
    [5.0, 2.4],
    [0.0, 3.3],
    [-5.6, 3.0],
    [-6.6, 1.4],
    [-6.6, -1.4],
    [-5.6, -3.0],
    [0.0, -3.3],
    [5.0, -2.4],
];
const LIFTS: [(f32, f32); 2] = [(4.0, 2.1), (-4.4, 2.4)];
/// The fins' inner faces either side of the lance.
const GAP: f32 = 0.66;
/// The sheathed lance's breech: shorter than the others', its sheath being thicker.
const BREECH: f32 = PIVOT.x - 0.45;

pub(in crate::regency) fn sheath(b: &mut MeshBuilder, _tech: u8) {
    set_rig(b);
    if b.coarse() {
        coarse(
            b,
            &[[7.2, 0.0], [0.0, 3.4], [-6.6, 0.0], [0.0, -3.4]],
            false,
        );
        return;
    }
    hull(b, &HULL);
    nose(b);
    b.mirror_y(flank);
    keel(b);
    lifts(b, &LIFTS, 0.95);
    b.with_part(part::TURRET, |b| {
        house(b);
        b.with_limb(rig::ARM_GUN, lance);
    });
}

/// A long glacis plate down the nose, a red optic either side of it.
fn nose(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(6.8, 0.0, 1.75),
        v3(-1.0, 0.0, 0.22),
        v3(0.25, 0.0, 1.0),
        4.6,
        1.3,
        0.0,
        0.3,
    );
    b.mirror_y(|b| {
        slot(
            b,
            v3(5.2, 2.2, 1.5),
            v3(0.45, 1.0, 0.0),
            v3(-1.0, 0.45, 0.0),
            1.2,
            0.15,
        );
    });
}

/// The left flank: one long plate swept back to a spike past the stern.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(3.6, 3.05, 1.9),
        v3(-1.0, 0.04, -0.05),
        v3(0.0, 1.0, 0.7),
        9.4,
        0.85,
        0.4,
        0.32,
    );
}

/// The dorsal keel aft: a keeled ridge raked up toward the stern, a plate lapped down
/// either side of it, a slot let into each.
fn keel(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-2.8, 0.0, DECK - 0.1), Vec3::Z, 0.9, 0.3),
            ring(v3(-4.6, 0.0, DECK + 0.6), Vec3::Z, 0.7, 0.6),
            ring(v3(-6.3, 0.0, DECK + 1.2), Vec3::Z, 0.03, 0.03),
        ],
        &KEEL,
    );
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(-2.6, 0.75, DECK + 0.1),
            v3(-1.0, 0.0, 0.32),
            v3(0.0, 1.0, 1.1),
        );
        dark_plate(b);
        armour(b, &f, &swept(4.2, 0.5, 0.4, 0.55), 0.2);
        if b.fine() {
            slot(b, f.at(1.6, 0.0, 0.2), f.n, f.u, 1.6, 0.14);
        }
    });
}

/// The turret: a bronze race, a low keeled house swept back to a point, and a raked fin
/// either side of the lance holding its trunnion, a slot let into each.
fn house(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.3, 2.25, 0.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [2.2, 0.0],
                [1.4, 1.8],
                [-1.6, 2.1],
                [-2.6, 0.9],
                [-2.6, -0.9],
                [-1.6, -2.1],
                [1.4, -1.8],
            ],
            &[
                Section::new(DECK + 0.15, 1.0),
                Section::new(DECK + 0.3, 0.92),
                Section::new(DECK + 0.42, 0.75).shifted(-0.3, 0.0),
            ],
        );
    });
    team(b, -0.9, DECK + 0.42);
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [-1.6, DECK + 0.3],
                    [1.4, DECK + 0.3],
                    [1.7, PIVOT.z + 0.4],
                    [1.2, PIVOT.z + 1.7],
                    [-0.4, PIVOT.z + 0.9],
                ],
                GAP,
                GAP + 0.45,
            );
        });
        if b.fine() {
            slot(
                b,
                v3(0.9, GAP + 0.46, PIVOT.z + 0.4),
                Vec3::Y,
                v3(0.25, 0.0, 1.0),
                1.2,
                0.15,
            );
        }
    });
}

/// The lance, in the turret's frame (it pitches about `PIVOT`): a plated sheath through
/// a bronze trunnion, plates lapped back down its back and flanks like feathers, a slot
/// either side, and three keeled claws round its mouth, closing in on the charge.
fn lance(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    collar(b, PIVOT, Vec3::Y, 0.42, GAP * 2.0 + 0.1);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(BREECH, 0.0, z), Vec3::Z, 0.42, 0.42),
            ring(v3(1.6, 0.0, z), Vec3::Z, 0.52, 0.5),
            ring(v3(6.4, 0.0, z), Vec3::Z, 0.36, 0.34),
            ring(v3(6.8, 0.0, z), Vec3::Z, 0.28, 0.26),
        ],
        &OCT,
    );
    let course = Course {
        count: if b.fine() { 3 } else { 2 },
        step: 1.6,
        len: 2.2,
        half: 0.32,
        tip: 0.0,
        thick: 0.14,
        tail: 0.6,
    };
    dark_plate(b);
    course.lay(b, &Frame::new(v3(6.0, 0.0, z + 0.32), -Vec3::X, Vec3::Z));
    b.mirror_y(|b| {
        dark_plate(b);
        course.lay(
            b,
            &Frame::new(v3(6.0, 0.3, z + 0.14), -Vec3::X, v3(0.0, 1.0, 0.8)),
        );
        if b.fine() {
            slot(b, v3(4.4, 0.42, z - 0.12), Vec3::Y, Vec3::X, 1.6, 0.12);
        }
    });
    for k in 0..3 {
        let a = (90.0 + 120.0 * k as f32).to_radians();
        let d = v3(0.0, a.cos(), a.sin());
        claw(b, v3(0.0, 0.0, z), d);
    }
}

/// A claw round the mouth on the side `d`: a keeled talon from the sheath's lip, out and
/// then in over the charge.
fn claw(b: &mut MeshBuilder, axis: Vec3, d: Vec3) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(axis + v3(6.3, 0.0, 0.0) + d * 0.32, d, 0.24, 0.14),
            ring(axis + v3(7.6, 0.0, 0.0) + d * 0.8, d, 0.2, 0.13),
            ring(axis + v3(9.0, 0.0, 0.0) + d * 0.6, d, 0.03, 0.03),
        ],
        &KEEL,
    );
    if b.fine() {
        metal(b);
        b.cylinder_between(
            axis + v3(6.5, 0.0, 0.0) + d * 0.3,
            axis + v3(6.5, 0.0, 0.0) + d * 0.5,
            0.12,
            0.12,
            6,
        );
    }
}
