//! The Gravitic Seeker Battery: its core on the race, and its two banks of cradles.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, collar, red_slot, swept, Frame};
use super::{CELL_X, CELL_Y, CELL_Z, PIVOT, RACE};

/// The core: a bronze race, a narrow faceted block between the banks with a red sensor
/// eye in its face and a plated bustle behind, the team colour on its roof.
pub(super) fn core(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, RACE - 0.05), sides, 1.4, 1.3, 0.2);
    dark_plate(b);
    let outline = [
        [1.5, 0.0],
        [1.1, 0.6],
        [-1.3, 0.72],
        [-2.1, 0.4],
        [-2.1, -0.4],
        [-1.3, -0.72],
        [1.1, -0.6],
    ];
    b.with_facets(|b| {
        b.loft_z(
            &outline,
            &[
                Section::new(RACE + 0.1, 1.0),
                Section::new(PIVOT.z + 0.3, 1.0),
                Section::new(PIVOT.z + 0.75, 0.75).shifted(-0.3, 0.0),
            ],
        )
    });
    red_slot(b, v3(1.32, 0.0, PIVOT.z + 0.1), Vec3::X, Vec3::Y, 0.4, 0.12);
    if b.fine() {
        red_slot(
            b,
            v3(-2.1, 0.0, PIVOT.z - 0.1),
            -Vec3::X,
            Vec3::Y,
            0.5,
            0.06,
        );
    }
    b.paint(TEAM);
    b.face(&[
        v3(-0.4, 0.0, PIVOT.z + 0.76),
        v3(-1.4, 0.35, PIVOT.z + 0.76),
        v3(-1.4, -0.35, PIVOT.z + 0.76),
    ]);
}

/// The banks, in the unit's frame at rest (they pitch about `PIVOT`): one trunnion through
/// the core, and either side a bank of three cradles.
pub(super) fn banks(b: &mut MeshBuilder) {
    collar(b, PIVOT, Vec3::Y, 0.36, 2.0 * (CELL_Y + 0.55));
    b.mirror_y(bank);
}

/// A bank: an armoured box open at the front, its walls plated and its hood swept back
/// into a spike, dividers between the cells, and in each cell a cradle: two bronze claws
/// over and under a charge of plasma held in gravity containment (the muzzle), red lenses
/// at their tips turned on it.
fn bank(b: &mut MeshBuilder) {
    let p = PIVOT;
    let (x0, x1) = (p.x - 0.6, p.x + CELL_X + 0.4);
    let (y0, y1) = (CELL_Y - 0.5, CELL_Y + 0.5);
    let (z0, z1) = (p.z - 0.85, p.z + 0.85);
    dark_plate(b);
    b.with_facets(|b| {
        // Inner and outer walls, their front edges raked back toward the bottom.
        for (ya, yb) in [(y0, y0 + 0.14), (y1 - 0.16, y1)] {
            b.extrude_y(
                &[
                    [x0, z0 + 0.1],
                    [x1 - 0.3, z0],
                    [x1, z0 + 0.4],
                    [x1, z1],
                    [x0 + 0.3, z1],
                    [x0, z1 - 0.2],
                ],
                ya,
                yb,
            );
        }
        // Back and floor.
        b.block(v3(x0, y0, z0 + 0.1), v3(x0 + 0.3, y1, z1 - 0.2));
        b.block(v3(x0 + 0.2, y0, z0), v3(x1 - 0.3, y1, z0 + 0.14));
    });
    // The hood, swept back past the bank into a spike.
    armour(
        b,
        &Frame::new(v3(x1 + 0.05, CELL_Y, z1), v3(-1.0, 0.0, 0.08), Vec3::Z),
        &swept(x1 - x0 + 0.9, 0.58, -0.6, 0.62),
        0.18,
    );
    if b.fine() {
        red_slot(
            b,
            v3(p.x + 0.8, y1 + 0.01, p.z + 0.5),
            Vec3::Y,
            Vec3::X,
            1.0,
            0.04,
        );
    }
    for (k, &dz) in CELL_Z.iter().enumerate() {
        let charge = p + v3(CELL_X, CELL_Y, dz);
        cradle(b, charge);
        if k > 0 && b.fine() {
            seam(b);
            let z = p.z + (CELL_Z[k - 1] + dz) * 0.5;
            b.block(
                v3(x0 + 0.3, y0 + 0.1, z - 0.03),
                v3(x1 - 0.2, y1 - 0.1, z + 0.03),
            );
        }
    }
}

/// A cradle round the charge at `c`: a claw over it and one under it, each a bronze bar
/// from the bank's back to a red lens just short of the charge, and the charge itself, a
/// small hot ball of plasma.
fn cradle(b: &mut MeshBuilder, c: Vec3) {
    for s in [1.0f32, -1.0] {
        let tip = c + v3(0.05, 0.0, s * 0.2);
        metal(b);
        b.beam(
            c + v3(-1.25, 0.0, s * 0.16),
            tip - v3(0.1, 0.0, 0.0),
            Vec2::new(0.16, 0.12),
            Vec2::new(0.12, 0.08),
        );
        if b.fine() {
            b.paint(GLOW_LASER);
            b.beam(
                tip - v3(0.1, 0.0, 0.0),
                tip + v3(0.08, 0.0, -s * 0.02),
                Vec2::new(0.13, 0.08),
                Vec2::new(0.1, 0.05),
            );
        }
    }
    b.paint(GLOW_LASER);
    let (sides, rings) = if b.fine() { (8, 4) } else { (4, 2) };
    b.spheroid(c, Vec3::splat(0.12), sides, rings);
}
