//! The Seraph model against the library's checks.

use super::deck::{CELLS, CELL_R, DECK};
use super::*;
use crate::build_model;

const KEYS: [&str; 3] = [
    "regency_seraph",
    "regency_seraph~seraph",
    "regency_seraph~halo",
];

#[test]
fn fits_the_librarys_checks() {
    for key in KEYS {
        super::super::check(
            key,
            RADIUS,
            HEIGHT,
            None,
            &[MUZZLE.to_array(), (MUZZLE * v3(1.0, -1.0, 1.0)).to_array()],
        );
    }
}

/// The seekers leave straight up: nothing of the model stands over a cell's mouth.
#[test]
fn the_cells_are_open_to_the_sky() {
    for key in KEYS {
        let model = build_model(key).unwrap();
        for [x, y] in CELLS {
            let over = model.lods[0].vertices.iter().find(|v| {
                v.pos[2] > DECK + 0.2
                    && Vec2::new(v.pos[0] - x, v.pos[1] - y).length() < CELL_R * 2.0
            });
            assert!(over.is_none(), "{key}: {over:?} over the cell at {x},{y}");
        }
    }
}

/// It hangs in the air: its lowest point, the pointed toes, well off the ground.
#[test]
fn hangs_off_the_ground() {
    for key in KEYS {
        let model = build_model(key).unwrap();
        let low = model.lods[0]
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!(low > 1.0, "{key}: lowest point {low}");
        assert!(model.hover, "{key}: a hover hull");
    }
}
