//! The Colossus's rocket battery, weapon 3: eight cells in two rows of four sunk in the
//! hull's back, under thin plated hatches that lie nearly flush and stand a little ajar, the
//! block low and sloped like a plate of the deck. The cells point 70 degrees up and a little
//! forward; the weapon turns the whole battery on a round turntable flush in the deck
//! (`MeshBuilder::with_house`) toward the mark, and the sim tips it the last degrees to the
//! unit file's `rake` before the rockets fire.
//!
//! The battery is drawn in its own frame: the origin on the pivot, +x down the cells, the
//! middle of their face at `(FACE, 0, 0)`, y across, z up the face.

use glam::{Affine3A, Vec3};

use super::detail::armor as dark_plate;
use crate::builder::MeshBuilder;

use crate::material::TEAM;

use super::super::kit::{metal, seam, v3};
use super::super::machine::hoop;
use super::{ROCKET_MUZZLE, ROCKET_PIVOT};

/// The cells' mouths across (y) and up the face (z), in the order the rockets fire: the
/// front row, outside pair first, then the rear row.
pub(super) const CELLS: [[f32; 2]; 8] = [
    [3.0, -1.3],
    [-3.0, -1.3],
    [1.0, -1.3],
    [-1.0, -1.3],
    [3.0, 1.3],
    [-3.0, 1.3],
    [1.0, 1.3],
    [-1.0, 1.3],
];
/// A cell's mouth radius.
pub(super) const CELL_R: f32 = 0.85;
/// How far the battery kicks back along its cells when it fires.
const KICK: f32 = 0.4;

/// The cells' axis: up and a little forward, from the pivot through the middle of the face.
pub(super) fn axis() -> (f32, f32) {
    let d = ROCKET_MUZZLE - ROCKET_PIVOT;
    (d.z.atan2(d.x), d.length())
}

/// Where each cell's mouth is at rest (model space), in firing order.
#[cfg(test)]
pub(super) fn mouths() -> [glam::Vec3; 8] {
    let (theta, face) = axis();
    let turn = glam::Mat3::from_rotation_y(-theta);
    CELLS.map(|[y, z]| ROCKET_PIVOT + SIZE * (turn * v3(face, y, z)))
}

/// The pack is drawn this many times its authored size about its hinge.
pub(super) const SIZE: f32 = 0.75;

pub(super) fn pack(b: &mut MeshBuilder) {
    let frame = Affine3A::from_translation(ROCKET_PIVOT)
        * Affine3A::from_scale(Vec3::splat(SIZE))
        * Affine3A::from_translation(-ROCKET_PIVOT);
    b.with(frame, drawn);
}

fn drawn(b: &mut MeshBuilder) {
    // The coaming: a low plated ring round the turntable, flush with the deck.
    dark_plate(b);
    let segs = if b.fine() { 28 } else { 12 };
    hoop(
        b,
        v3(ROCKET_PIVOT.x, 0.0, ROCKET_PIVOT.z + 1.5),
        6.15,
        0.8,
        0.7,
        segs,
    );
    b.with_house(3, ROCKET_PIVOT, KICK, |b| {
        // The turntable, a round plate flush with the deck, a seam round its edge.
        dark_plate(b);
        let sides = b.sides(20);
        b.prism(
            v3(ROCKET_PIVOT.x, 0.0, ROCKET_PIVOT.z + 1.1),
            sides,
            5.85,
            5.8,
            0.75,
        );
        // The owner's colour in a ring on the turntable.
        b.paint(TEAM);
        hoop(
            b,
            v3(ROCKET_PIVOT.x, 0.0, ROCKET_PIVOT.z + 1.86),
            5.2,
            0.5,
            0.05,
            sides,
        );
        if b.fine() {
            seam(b);
            b.prism(
                v3(ROCKET_PIVOT.x, 0.0, ROCKET_PIVOT.z + 1.5),
                sides,
                5.9,
                5.9,
                0.16,
            );
        }
        let (theta, _) = axis();
        b.pitched(ROCKET_PIVOT, theta, |b| b.with_recoil(battery));
    });
}

/// The battery in its own frame: a chamfered block of plate, the cells' dark mouths in its
/// face, a thin hatch over each hinged on its rear edge and standing a little ajar, a seam
/// across the face between the rows.
fn battery(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (_, face) = axis();
    let (half_y, half_z) = (5.25, 2.65);
    dark_plate(b);
    b.with_facets(|b| {
        b.chamfered_box(
            v3(face * 0.5 - 0.3, 0.0, 0.0),
            v3(face + 0.6, half_y * 2.0, half_z * 2.0),
            0.55,
        );
    });
    if !fine {
        return;
    }
    // The dark mouths.
    seam(b);
    let sides = b.sides(8);
    for [y, z] in CELLS {
        b.cylinder_between(
            v3(face - 0.05, y, z),
            v3(face + 0.07, y, z),
            CELL_R,
            CELL_R,
            sides,
        );
    }
    // A band of seam between the two rows of hatches.
    b.chamfered_box(
        v3(face + 0.02, 0.0, 0.0),
        v3(0.1, half_y * 2.0 - 0.8, 0.22),
        0.04,
    );
    // The hatches: a plate over each cell, its hinge on the face's rear edge of the cell.
    dark_plate(b);
    for [y, z] in CELLS {
        let hinge = v3(face + 0.16, y, z + 1.1);
        b.with(
            Affine3A::from_translation(hinge) * Affine3A::from_rotation_y(-0.2),
            |b| {
                b.with_facets(|b| {
                    b.chamfered_box(v3(0.08, 0.0, -1.05), v3(0.16, 1.95, 2.1), 0.05);
                });
            },
        );
    }
    // Bronze hinge pins along the rear edge of each row.
    metal(b);
    for z in [-0.2f32, 2.4] {
        b.cylinder_between(
            v3(face + 0.14, -4.2, z),
            v3(face + 0.14, 4.2, z),
            0.1,
            0.1,
            b.sides(6),
        );
    }
}
