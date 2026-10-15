//! The commander's head: sunk between the shoulders, it looks about while the
//! commander stands idle (`MeshBuilder::with_head`).

use glam::Vec3;

use crate::aster::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The armoured head in plan (model space): a broad block, its front corners cut, sat
/// low in the collar.
const SLIT_PLAN: [[f32; 2]; 8] = [
    [-0.5, -0.6],
    [-0.3, -0.8],
    [0.85, -0.8],
    [1.2, -0.45],
    [1.2, 0.45],
    [0.85, 0.8],
    [-0.3, 0.8],
    [-0.5, 0.6],
];
/// Where the visor slit runs round the head's front, bottom and top.
const SLIT: (f32, f32) = (13.52, 13.68);

/// The armoured head: a broad black block sunk in the collar, one lit slit across its
/// face under a white brow, a white crown. Builds about the neck at `neck`.
pub(super) fn head(b: &mut MeshBuilder, neck: Vec3) {
    b.with_head(neck, |b| {
        b.paint(ACCENT);
        b.loft_z(
            &SLIT_PLAN,
            &[
                Section::scaled(13.1, 0.86, 0.9),
                Section::new(13.3, 1.0),
                Section::new(SLIT.0, 1.0),
            ],
        );
        b.loft_z(
            &SLIT_PLAN,
            &[
                Section::new(SLIT.1, 1.0),
                Section::new(13.88, 1.0),
                Section::scaled(14.02, 0.9, 0.92).shifted(-0.06, 0.0),
            ],
        );
        // The slit: glass across the front and round the cheeks, dark metal behind.
        b.paint(VISOR).pattern(pattern::PLAIN);
        b.loft_z(
            &[
                [0.35, -0.78],
                [0.85, -0.78],
                [1.18, -0.44],
                [1.18, 0.44],
                [0.85, 0.78],
                [0.35, 0.78],
            ],
            &[Section::new(SLIT.0, 1.0), Section::new(SLIT.1, 1.0)],
        );
        b.paint(ACCENT);
        b.loft_z(
            &[
                [-0.45, -0.6],
                [-0.3, -0.76],
                [0.35, -0.76],
                [0.35, 0.76],
                [-0.3, 0.76],
                [-0.45, 0.6],
            ],
            &[Section::new(SLIT.0, 1.0), Section::new(SLIT.1, 1.0)],
        );
        // The brow over the slit, standing a little proud of the face, and the crown.
        b.paint(PLATING);
        b.loft_z(
            &[
                [0.75, -0.84],
                [0.9, -0.84],
                [1.26, -0.47],
                [1.26, 0.47],
                [0.9, 0.84],
                [0.75, 0.84],
            ],
            &[Section::new(SLIT.1, 1.0), Section::new(SLIT.1 + 0.14, 1.0)],
        );
        b.plate(v3(0.2, 0.0, 14.02), v2(1.2, 1.15), 0.1, 0.04);
        if !b.fine() {
            return;
        }
        // A chin guard under the slit.
        b.loft_z(
            &[[0.9, -0.5], [1.24, -0.3], [1.24, 0.3], [0.9, 0.5]],
            &[Section::new(13.2, 1.0), Section::new(13.44, 1.0)],
        );
        // A grab bar over the crown, and a sensor pod on its left, its lens forward.
        b.paint(METAL);
        for x in [-0.25, 0.45] {
            b.cylinder_between(v3(x, -0.3, 14.1), v3(x, -0.3, 14.3), 0.05, 0.05, 6);
        }
        b.cylinder_between(v3(-0.3, -0.3, 14.3), v3(0.5, -0.3, 14.3), 0.05, 0.05, 6);
        b.paint(ACCENT);
        b.block(v3(-0.3, 0.3, 14.1), v3(0.3, 0.6, 14.32));
        b.paint(VISOR).pattern(pattern::PLAIN);
        b.block(v3(0.3, 0.36, 14.15), v3(0.33, 0.54, 14.27));
    });
}
