//! Design B: a broad blunt hull lapped in feathered plates down its flanks, the lance hung
//! in a tall keeled yoke over a capacitor housing, two rails above and below its bore
//! reaching past the mouth to hold the charge between them.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::{part, rig};

use super::super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{armour, collar, swept, Course, Frame};
use super::{coarse, hull, lifts, rig as set_rig, slot, team, BREECH, DECK, PIVOT};

/// The hull's plan: a broad blunt diamond, its stern cut square.
const HULL: [[f32; 2]; 10] = [
    [6.4, 0.0],
    [5.4, 1.9],
    [1.0, 4.3],
    [-3.6, 4.3],
    [-6.0, 2.4],
    [-6.2, 0.0],
    [-6.0, -2.4],
    [-3.6, -4.3],
    [1.0, -4.3],
    [5.4, -1.9],
];
const LIFTS: [(f32, f32); 3] = [(3.6, 2.5), (-0.6, 3.3), (-4.4, 2.6)];
/// The yoke's arms' inner faces either side of the lance.
const GAP: f32 = 0.62;

pub(in crate::regency) fn yoke(b: &mut MeshBuilder, _tech: u8) {
    set_rig(b);
    if b.coarse() {
        coarse(
            b,
            &[[6.4, 0.0], [-1.0, 4.4], [-6.2, 0.0], [-1.0, -4.4]],
            true,
        );
        return;
    }
    hull(b, &HULL);
    nose(b);
    b.mirror_y(flank);
    lifts(b, &LIFTS, 0.9);
    b.with_part(part::TURRET, |b| {
        house(b);
        b.with_limb(rig::ARM_GUN, lance);
    });
}

/// A wide glacis across the blunt nose, a chin plate under it, a red optic either side.
fn nose(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(6.1, 0.0, 1.9),
        v3(-1.0, 0.0, 0.3),
        v3(0.3, 0.0, 1.0),
        3.4,
        2.0,
        0.0,
        0.34,
    );
    blade(
        b,
        v3(5.4, 0.0, 1.0),
        v3(1.0, 0.0, 0.1),
        v3(0.3, 0.0, -1.0),
        1.4,
        1.6,
        0.0,
        0.26,
    );
    b.mirror_y(|b| {
        slot(
            b,
            v3(5.55, 1.75, 1.55),
            v3(0.9, 0.45, 0.0),
            v3(-0.45, 0.9, 0.0),
            0.9,
            0.16,
        );
    });
}

/// The left flank: three plates lapped back like feathers, the last out past the stern.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    Course {
        count: 3,
        step: 2.9,
        len: 3.6,
        half: 0.62,
        tip: 0.3,
        thick: 0.26,
        tail: 1.4,
    }
    .lay(
        b,
        &Frame::new(v3(4.2, 3.2, 1.95), v3(-1.0, 0.2, -0.05), v3(0.0, 1.0, 0.7)),
    );
}

/// The turret: a bronze race, a capacitor housing aft with a slot across its back, and
/// the yoke: a keeled arm either side rising to the trunnion, a plate lapped down its
/// outside and swept back into a spike.
fn house(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.6, 2.55, 0.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [-1.5, 0.0],
                [-1.9, 1.6],
                [-3.7, 1.4],
                [-4.2, 0.0],
                [-3.7, -1.4],
                [-1.9, -1.6],
            ],
            &[
                Section::new(DECK + 0.2, 1.0),
                Section::new(DECK + 0.9, 0.95),
                Section::new(DECK + 1.2, 0.7).shifted(-0.4, 0.0),
            ],
        );
    });
    team(b, -1.6, DECK + 1.2);
    slot(b, v3(-4.15, 0.0, DECK + 0.55), -Vec3::X, Vec3::Y, 1.6, 0.2);
    if b.fine() {
        // Capacitor drums out of the housing's back.
        b.mirror_y(|b| {
            metal(b);
            b.cylinder_between(
                v3(-3.4, 0.95, DECK + 0.9),
                v3(-4.6, 0.95, DECK + 0.8),
                0.3,
                0.28,
                10,
            );
        });
    }
    b.mirror_y(|b| {
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(-0.4, GAP + 0.35, DECK + 0.15), Vec3::Y, 1.2, 0.35),
                ring(v3(0.2, GAP + 0.35, PIVOT.z), Vec3::Y, 0.9, 0.35),
                ring(v3(0.5, GAP + 0.3, PIVOT.z + 1.0), Vec3::Y, 0.55, 0.3),
                ring(v3(0.3, GAP + 0.25, PIVOT.z + 1.7), Vec3::Y, 0.05, 0.05),
            ],
            &KEEL,
        );
        let f = Frame::new(
            v3(0.9, GAP + 0.8, PIVOT.z + 1.1),
            v3(-1.0, 0.0, -0.6),
            Vec3::Y,
        );
        armour(b, &f, &swept(4.0, 0.5, -0.4, 0.5), 0.2);
        if b.fine() {
            slot(b, f.at(1.4, 0.0, 0.2), Vec3::Y, f.u, 1.4, 0.15);
        }
    });
}

/// The lance, in the turret's frame (it pitches about `PIVOT`): a slim bore through a
/// bronze trunnion, and above and below it a keeled rail running out past its mouth to a
/// point, the charge held between the two; bronze bands bind rails and bore together, a
/// slot runs down each side of the bore.
fn lance(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    collar(b, PIVOT, Vec3::Y, 0.5, GAP * 2.0 + 0.1);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(BREECH, 0.0, z), Vec3::Z, 0.34, 0.3),
            ring(v3(1.2, 0.0, z), Vec3::Z, 0.36, 0.32),
            ring(v3(6.8, 0.0, z), Vec3::Z, 0.26, 0.24),
            ring(v3(7.1, 0.0, z), Vec3::Z, 0.2, 0.18),
        ],
        &OCT,
    );
    for s in [-1.0f32, 1.0] {
        let out = Vec3::Z * s;
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(BREECH + 0.2, 0.0, z + 0.44 * s), out, 0.34, 0.16),
                ring(v3(5.0, 0.0, z + 0.52 * s), out, 0.3, 0.17),
                ring(v3(7.6, 0.0, z + 0.68 * s), out, 0.26, 0.16),
                ring(v3(9.0, 0.0, z + 0.62 * s), out, 0.03, 0.03),
            ],
            &KEEL,
        );
    }
    let bands: &[f32] = if b.fine() {
        &[1.6, 3.4, 5.2, 6.7]
    } else {
        &[3.4, 6.7]
    };
    for &x in bands {
        metal(b);
        b.beam(
            v3(x - 0.16, 0.0, z),
            v3(x + 0.16, 0.0, z),
            Vec2::new(0.62, 1.55),
            Vec2::new(0.62, 1.55),
        );
    }
    if b.fine() {
        b.mirror_y(|b| {
            slot(b, v3(4.3, 0.31, z), Vec3::Y, Vec3::X, 1.4, 0.12);
        });
    }
}
