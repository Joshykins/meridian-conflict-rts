//! What the Springald's turret carries on its back behind the trunnion: the fusion plant
//! that feeds its gun, three upright capacitor cells, one for each knot of its triune
//! charge, its light let out only in thin lit seams (no open star).
//!
//! Each cell stands in a seam-dark socket: a plated hexagonal column, ridged plates lapped
//! up three of its faces, graphite bands round it, a lit seam up its back that lights in
//! turn through the charge, and a swept cap that lifts off a hot grille with the heat after
//! the shot (`charge_gear::VENT`). A plated plinth carries the three, a spine behind them,
//! a machinery bay down its back and conduits forward to the cheeks.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, VENT};

use super::super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, hoop_on, swept, Frame};
use super::super::super::parts::{bay, ridged};
use super::super::bar_through;
use super::super::fusion_kit::coil_light;
use super::{CHEEK, LINE, ROOF};

/// The cells' row across the turret's back, and their tops.
const ROW: f32 = -14.0;
const TOP: f32 = ROOF + 9.0;
/// The cells across the back and the coil stage each lights at.
const CELLS: [(f32, u32); 3] = [(-4.2, 2), (0.0, 4), (4.2, 6)];
/// A cell's column, across its flats' corners.
const CELL_R: f32 = 2.0;

pub(super) fn draw(b: &mut MeshBuilder) {
    let fine = b.fine();
    dark_plate(b);
    b.with_facets(|b| b.chamfered_box(v3(ROW, 0.0, ROOF + 0.6), v3(8.2, 13.4, 1.4), 0.4));
    for (y, stage) in CELLS {
        cell(b, v3(ROW, y, ROOF + 1.3), stage);
    }
    dark_plate(b);
    bar_through(
        b,
        &[
            (v3(ROW - 4.6, 0.0, ROOF + 1.2), Vec2::new(1.8, 2.6)),
            (v3(ROW - 3.2, 0.0, TOP - 2.0), Vec2::new(1.5, 2.1)),
        ],
        Vec3::Y,
    );
    if !b.mid() {
        return;
    }
    // A bay either side of the spine in the plinth's back, where the plates part over its
    // machinery.
    b.mirror_y(|b| {
        bay(
            b,
            &Frame::new(v3(ROW - 4.12, 1.8, ROOF + 0.6), Vec3::Y, -Vec3::X),
            4.2,
            0.36,
        )
    });
    if fine {
        conduits(b);
    }
}

/// A capacitor cell standing from `c`.
fn cell(b: &mut MeshBuilder, c: Vec3, stage: u32) {
    let h = TOP - c.z;
    let sides = b.sides(12);
    seam(b);
    b.prism(c - Vec3::Z * 0.1, 6, CELL_R + 0.45, CELL_R + 0.35, 0.9);
    dark_plate(b);
    b.prism(c + Vec3::Z * 0.8, 6, CELL_R, CELL_R * 0.86, h - 2.2);
    metal(b);
    for z in [1.6, h * 0.55] {
        hoop_on(b, c + Vec3::Z * z, Vec3::Z, CELL_R + 0.1, 0.35, 0.6, sides);
    }
    // The lit seam up its back.
    coil_light(b, stage);
    b.block(
        v3(c.x - CELL_R * 0.98, c.y - 0.12, c.z + 2.2),
        v3(c.x - CELL_R * 0.86, c.y + 0.12, TOP - 2.0),
    );
    if b.mid() {
        // Ridged plates lapped up three of its faces, the back left bare for the seam.
        for k in [1.0f32, 3.0, 5.0] {
            let a = TAU * k / 6.0 + TAU / 12.0;
            let out = v3(a.cos(), a.sin(), 0.0);
            let flat = CELL_R * 0.93 * (TAU / 12.0).cos();
            dark_plate(b);
            ridged(
                b,
                &Frame::new(c + out * flat + Vec3::Z * 2.4, Vec3::Z, out),
                &[
                    [0.0, -0.8],
                    [h * 0.42, -0.75],
                    [h * 0.52, 0.0],
                    [h * 0.42, 0.75],
                    [0.0, 0.8],
                ],
                0.2,
                [h * 0.22, 0.0],
                0.15,
            );
        }
    }
    // The hot grille under its cap, dark until the shot.
    coil_light(b, HEAT_STAGE);
    b.prism(
        v3(c.x, c.y, TOP - 1.42),
        6,
        CELL_R * 0.8,
        CELL_R * 0.8,
        0.05,
    );
    // The cap: plated, swept back into a spike, lifting with the heat.
    b.with_charge_gear(VENT, |b| {
        dark_plate(b);
        b.prism(v3(c.x, c.y, TOP - 1.4), 6, CELL_R * 0.95, CELL_R * 0.7, 0.9);
        if b.mid() {
            armour(
                b,
                &Frame::new(
                    v3(c.x + 1.8, c.y, TOP - 0.45),
                    v3(-1.0, 0.0, -0.12),
                    Vec3::Z,
                ),
                &swept(5.8, 1.5, 0.0, 0.5),
                0.35,
            );
        }
    });
}

/// Graphite conduits from the plinth forward to the trunnion cheeks, either side.
fn conduits(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        metal(b);
        for (k, dz) in [(0.0f32, 0.0f32), (0.6, 0.8)] {
            let from = v3(ROW + 4.2, 5.4 - k, ROOF + 1.6 + dz);
            cable(
                b,
                &[
                    from,
                    from + v3(2.4, 0.4, -0.2),
                    v3(-6.4, CHEEK + 0.4 - k * 0.5, LINE.pivot.z - 3.0 + dz),
                ],
                0.4,
            );
        }
    });
}
