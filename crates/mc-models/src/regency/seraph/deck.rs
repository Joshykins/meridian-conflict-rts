//! The launch decks: two plated humps on the upper back between the wing roots, eight
//! cells in each open to the sky. The cased seekers leave straight up out of them, climb
//! and fall on everything round the Seraph (the unit file's `muzzles`, `CELLS` times
//! `SCALE`).

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

use super::super::kit::{dark_plate, seam, v3};

/// The decks' height.
pub(super) const DECK: f32 = 21.3;
/// The cells (x along the back, y across) in the order they fire: left and right decks
/// in turn, front to back, inner before outer.
pub(super) const CELLS: [[f32; 2]; 16] = [
    [-1.55, 1.15],
    [-1.55, -1.15],
    [-1.55, 1.85],
    [-1.55, -1.85],
    [-2.2, 1.15],
    [-2.2, -1.15],
    [-2.2, 1.85],
    [-2.2, -1.85],
    [-2.85, 1.15],
    [-2.85, -1.15],
    [-2.85, 1.85],
    [-2.85, -1.85],
    [-3.5, 1.15],
    [-3.5, -1.15],
    [-3.5, 1.85],
    [-3.5, -1.85],
];
/// A cell's mouth across.
pub(super) const CELL_R: f32 = 0.24;

/// Both decks and their cells.
pub(super) fn decks(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &[
                    [-0.95, 1.5],
                    [-1.2, 2.35],
                    [-3.4, 2.4],
                    [-4.05, 1.9],
                    [-4.0, 0.0],
                    [-0.9, 0.0],
                ],
                &[
                    Section::new(18.4, 0.9).shifted(-0.25, 0.15),
                    Section::new(20.4, 1.0),
                    Section::new(DECK, 1.0),
                ],
            );
        });
        // A red line down the deck's outer flank: the battery armed.
        b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
        b.face(&[
            v3(-1.3, 2.37, 20.2),
            v3(-3.3, 2.42, 20.2),
            v3(-3.3, 2.42, 20.9),
            v3(-1.3, 2.37, 20.9),
        ]);
    });
    for [x, y] in CELLS {
        let mouth = v3(x, y, DECK);
        dark_plate(b);
        let sides = b.sides(8);
        b.cylinder_between(
            mouth - Vec3::Z * 0.1,
            mouth + Vec3::Z * 0.08,
            CELL_R + 0.06,
            CELL_R + 0.04,
            sides,
        );
        // The cell's lid, its red gasket in the plate.
        if fine {
            b.paint(ACCENT).pattern(pattern::EMBER_RIM);
        } else {
            seam(b);
        }
        b.cylinder_between(
            mouth + Vec3::Z * 0.06,
            mouth + Vec3::Z * 0.1,
            CELL_R * 0.86,
            CELL_R * 0.86,
            sides,
        );
    }
    // The team colour down the spine between the decks.
    b.paint(TEAM);
    b.face(&[
        v3(-1.1, -0.32, DECK + 0.01),
        v3(-1.1, 0.32, DECK + 0.01),
        v3(-3.8, 0.24, DECK + 0.01),
        v3(-3.8, -0.24, DECK + 0.01),
    ]);
}
