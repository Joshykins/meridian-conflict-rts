//! Design A: a long pointed wedge of a hull, the lance carried low between two tall split
//! cheeks, a pair of flat tines either side of the charge, lit red inside.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::{part, rig};

use super::super::commander::form::{blade, ring, sleeve, OCT};
use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::{armour, collar, swept, Frame};
use super::{coarse, hull, lifts, rig as set_rig, slot, team, BREECH, DECK, PIVOT};

/// The hull's plan: a pointed wedge, its stern cut in between two trailing corners.
const HULL: [[f32; 2]; 10] = [
    [7.0, 0.0],
    [5.6, 2.0],
    [2.0, 3.6],
    [-4.4, 3.9],
    [-6.4, 2.7],
    [-5.8, 0.0],
    [-6.4, -2.7],
    [-4.4, -3.9],
    [2.0, -3.6],
    [5.6, -2.0],
];
const LIFTS: [(f32, f32); 2] = [(4.0, 2.4), (-4.0, 2.8)];
/// The cheeks' inner faces either side of the lance.
const GAP: f32 = 0.6;

pub(in crate::regency) fn split(b: &mut MeshBuilder, _tech: u8) {
    set_rig(b);
    if b.coarse() {
        coarse(
            b,
            &[[7.0, 0.0], [-1.0, 3.9], [-6.2, 0.0], [-1.0, -3.9]],
            false,
        );
        return;
    }
    hull(b, &HULL);
    nose(b);
    b.mirror_y(flank);
    lifts(b, &LIFTS, 1.0);
    b.with_part(part::TURRET, |b| {
        house(b);
        b.with_limb(rig::ARM_GUN, lance);
    });
}

/// A glacis plate over the nose, and a red optic either side of it.
fn nose(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(6.6, 0.0, 1.8),
        v3(-1.0, 0.0, 0.26),
        v3(0.3, 0.0, 1.0),
        4.2,
        1.5,
        0.0,
        0.32,
    );
    b.mirror_y(|b| {
        slot(
            b,
            v3(4.6, 2.62, 1.5),
            v3(0.5, 1.0, 0.0),
            v3(-1.0, 0.5, 0.0),
            1.4,
            0.16,
        );
    });
}

/// The left flank's plates: two courses swept back, the second running out past the
/// stern into a spike.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(4.6, 3.0, 1.95),
        v3(-1.0, 0.1, -0.05),
        v3(0.0, 1.0, 0.75),
        5.4,
        0.8,
        0.2,
        0.32,
    );
    blade(
        b,
        v3(0.6, 3.75, 1.8),
        v3(-1.0, 0.06, -0.04),
        v3(0.0, 1.0, 0.7),
        7.4,
        0.85,
        0.4,
        0.34,
    );
}

/// The turret: a bronze race, a low housing aft, and two tall cheeks either side of the
/// lance, each lapped in a plate swept back into a spike, a slot let into it.
fn house(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.4, 2.35, 0.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [-1.4, 0.0],
                [-1.7, 1.9],
                [-3.3, 1.7],
                [-3.9, 0.0],
                [-3.3, -1.7],
                [-1.7, -1.9],
            ],
            &[
                crate::builder::Section::new(DECK + 0.2, 1.0),
                crate::builder::Section::new(DECK + 0.75, 0.94),
                crate::builder::Section::new(DECK + 1.0, 0.7).shifted(-0.3, 0.0),
            ],
        );
    });
    team(b, -1.5, DECK + 1.0);
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [-2.4, DECK + 0.2],
                    [1.9, DECK + 0.2],
                    [1.6, PIVOT.z + 0.7],
                    [0.2, PIVOT.z + 1.6],
                    [-1.0, PIVOT.z + 1.5],
                    [-2.2, PIVOT.z + 0.4],
                ],
                GAP,
                1.75,
            );
        });
        let f = Frame::new(v3(1.9, 1.78, DECK + 0.7), v3(-1.0, 0.0, 0.55), Vec3::Y);
        armour(b, &f, &swept(4.8, 0.62, -0.3, 0.55), 0.2);
        if b.fine() {
            slot(b, f.at(1.8, 0.0, 0.2), Vec3::Y, f.u, 1.8, 0.16);
        }
    });
}

/// The lance, in the turret's frame (it pitches about `PIVOT`): a tapering plated bore
/// through a bronze trunnion, bronze pinch bands round it, a plate down its back, a slot
/// down each side, and two flat tines reaching past its mouth either side of the charge,
/// lit red inside.
fn lance(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    collar(b, PIVOT, Vec3::Y, 0.44, GAP * 2.0 + 0.1);
    let w = |x: f32| -> f32 {
        if x < 1.4 {
            0.45
        } else {
            0.45 - (x - 1.4) / 5.2 * 0.17
        }
    };
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(BREECH, 0.0, z), Vec3::Z, 0.3, 0.3),
            ring(v3(1.4, 0.0, z), Vec3::Z, w(1.4), w(1.4)),
            ring(v3(6.6, 0.0, z), Vec3::Z, w(6.6), w(6.6)),
            ring(v3(7.0, 0.0, z), Vec3::Z, 0.24, 0.24),
        ],
        &OCT,
    );
    if b.fine() {
        for x in [2.6f32, 4.0, 5.4] {
            metal(b);
            let r = w(x) + 0.07;
            sleeve(
                b,
                &[
                    ring(v3(x - 0.16, 0.0, z), Vec3::Z, r, r),
                    ring(v3(x + 0.16, 0.0, z), Vec3::Z, r, r),
                ],
                &OCT,
            );
        }
    }
    dark_plate(b);
    blade(
        b,
        v3(6.4, 0.0, z + w(6.4) - 0.02),
        v3(-1.0, 0.0, -0.035),
        Vec3::Z,
        5.0,
        0.36,
        0.0,
        0.16,
    );
    b.mirror_y(|b| {
        // A slot down the bore's side, on its flat.
        let (x0, x1) = (2.9, 5.1);
        let at = v3((x0 + x1) * 0.5, w((x0 + x1) * 0.5) + 0.005, z);
        let along = v3(x1 - x0, w(x1) - w(x0), 0.0);
        slot(b, at, Vec3::Y, along, x1 - x0, 0.14);
        tine(b, z);
    });
}

/// The left tine: a flat blade standing on edge beside the charge, tied to the mouth,
/// its inner face slotted red.
fn tine(b: &mut MeshBuilder, z: f32) {
    let inner = 0.72;
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(6.2, inner, z), Vec3::X, Vec3::Y),
        &swept(2.8, 0.44, 0.0, 0.6),
        0.2,
    );
    dark_plate(b);
    b.beam(
        v3(6.0, 0.2, z),
        v3(6.6, inner + 0.1, z),
        glam::Vec2::new(0.5, 0.3),
        glam::Vec2::new(0.5, 0.3),
    );
    if b.fine() {
        slot(b, v3(7.4, inner - 0.01, z), -Vec3::Y, Vec3::X, 1.3, 0.16);
    }
}
