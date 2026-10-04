//! The Gravitic Seeker cells: two blocks of eight on the hump either side of the spine,
//! port block first (`space.ron` weapon 1's muzzles). Each is an armoured box stood on the
//! hull, its deck lidded cell by cell, each lid over a red rim and the round under it.

use glam::Vec2;

use crate::builder::{chamfered_rect, CellGrid, MeshBuilder, Section};
use crate::material::GLOW_LASER;
use crate::part;

use super::super::super::kit::{dark_plate, metal, seam};
use super::body::Body;

/// The blocks' middle along the hull, out from the spine, and the cells' spacing.
const X: f32 = -60.0;
const Y: f32 = 15.5;
const PITCH: f32 = 2.4;
/// Cells along and across.
const NX: u8 = 4;
const NY: u8 = 2;
/// The order the cells fire in, (along, across): spread over the block.
pub(super) const FIRE: [(u8, u8); 8] = [
    (0, 0),
    (2, 1),
    (1, 0),
    (3, 1),
    (0, 1),
    (2, 0),
    (1, 1),
    (3, 0),
];

/// How far under its cell's deck a seeker is launched from.
#[cfg(test)]
pub(super) const MUZZLE_DROP: f32 = 1.6;

fn half() -> Vec2 {
    Vec2::new(
        PITCH * NX as f32 * 0.5 + 0.45,
        PITCH * NY as f32 * 0.5 + 0.45,
    )
}

pub(super) fn cells(b: &mut MeshBuilder, body: &Body) {
    if b.coarse() {
        return;
    }
    for side in [1.0f32, -1.0] {
        block(b, body, side);
    }
}

fn block(b: &mut MeshBuilder, body: &Body, side: f32) {
    let h = half();
    let centre = Vec2::new(X, Y * side);
    // The deck clears the hull at the block's inner edge, where it stands highest; the
    // box runs down into the hull at its outer edge.
    let surface = |y: f32| {
        [X - h.x, X, X + h.x]
            .iter()
            .map(|&x| body.point(x, body.phi_up(x, y)).z)
            .fold(f32::MIN, f32::max)
    };
    let deck = surface(Y - h.y) + 1.6;
    let foot = surface(Y + h.y) - 1.5;
    let plan = chamfered_rect(h, 0.6);
    b.at(centre.extend(0.0), |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[Section::new(foot, 1.06), Section::new(deck - 0.3, 1.0)],
        );
        seam(b);
        b.loft_z(
            &plan,
            &[Section::new(deck - 0.3, 1.0), Section::new(deck, 0.97)],
        );
    });
    let grid = CellGrid {
        centre,
        deck,
        pitch: PITCH,
        half: PITCH * 0.42,
        nx: NX,
        ny: NY,
        hinge_y: true,
    };
    let fine = b.fine();
    let lid = PITCH * 0.8;
    for m in b.cell_block(grid, &FIRE) {
        if fine {
            b.paint(GLOW_LASER);
            b.plate(m.extend(deck), Vec2::splat(PITCH * 0.9), 0.04, 0.01);
        }
        b.with_part(part::CELL_HATCH, |b| {
            dark_plate(b);
            b.plate(m.extend(deck + 0.03), Vec2::splat(lid), 0.2, 0.06);
        });
        if fine {
            // The round: a bronze body under the contained charge's lit head.
            b.with_part(part::CELL_ROUND, |b| {
                let top = deck - 0.25;
                metal(b);
                b.cylinder_between(m.extend(top - 2.0), m.extend(top - 0.7), 0.42, 0.42, 6);
                b.paint(GLOW_LASER);
                b.cylinder_between(m.extend(top - 0.7), m.extend(top), 0.42, 0.12, 6);
            });
        }
    }
}
