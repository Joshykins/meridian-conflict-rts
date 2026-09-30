//! The commander's head: sunk between the shoulders, it looks about while the
//! commander stands idle (`MeshBuilder::with_head`).

use glam::{Affine3A, Vec3};

use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// Where the head is drawn: authored full size about its base, then shrunk to sit
/// between the shoulders.
const BASE: Vec3 = Vec3::new(0.4, 0.0, 13.15);
const SCALE: f32 = 0.88;

/// The head in plan: flat cheeks running forward to a prow down the middle of
/// the face, a clipped back.
const PLAN: [[f32; 2]; 7] = [
    [-0.8, -0.55],
    [-0.5, -0.95],
    [0.45, -0.95],
    [1.1, 0.0],
    [0.45, 0.95],
    [-0.5, 0.95],
    [-0.8, 0.55],
];

/// Builds the head about the neck at `neck` (model space).
pub(super) fn head(b: &mut MeshBuilder, neck: Vec3) {
    b.with_head(neck, |b| {
        b.with(
            Affine3A::from_translation(BASE)
                * Affine3A::from_scale(Vec3::splat(SCALE))
                * Affine3A::from_translation(Vec3::new(0.0, 0.0, -BASE.z)),
            wedge,
        )
    });
}

/// A faceted wedge, wider than tall: the brow rakes back hard to a low crown and
/// one thin visor slit wraps the two front facets under a jutting lip.
fn wedge(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.loft_z(
        &PLAN,
        &[
            Section::scaled(13.15, 0.66, 0.72).shifted(-0.1, 0.0),
            Section::new(13.55, 1.0),
            Section::new(13.92, 1.0),
        ],
    );
    b.loft_z(
        &PLAN,
        &[
            Section::new(14.06, 1.0),
            Section::scaled(14.3, 0.9, 0.95).shifted(-0.08, 0.0),
            Section::scaled(14.5, 0.55, 0.72).shifted(-0.3, 0.0),
        ],
    );
    // The slit: mirror glass across the face, dark metal round the back.
    b.paint(VISOR).pattern(pattern::PLAIN);
    b.loft_z(
        &[[0.4, -0.96], [1.12, 0.0], [0.4, 0.96]],
        &[Section::new(13.92, 1.0), Section::new(14.06, 1.0)],
    );
    b.paint(ACCENT);
    b.loft_z(
        &[
            [-0.8, -0.55],
            [-0.5, -0.95],
            [0.4, -0.95],
            [0.4, 0.95],
            [-0.5, 0.95],
            [-0.8, 0.55],
        ],
        &[Section::new(13.92, 0.97), Section::new(14.06, 0.97)],
    );
    // Brow lip over the slit and a chin guard under the prow, in light metal.
    b.paint(PLATING);
    b.loft_z(
        &[[0.3, -0.98], [1.15, 0.0], [0.3, 0.98]],
        &[Section::new(14.06, 1.0), Section::new(14.11, 0.99)],
    );
    b.loft_z(
        &[[0.3, -0.55], [0.95, 0.0], [0.3, 0.55]],
        &[Section::new(13.25, 0.95), Section::new(13.5, 1.2)],
    );
    // Cheek plates, raked to follow the brow.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_y(
            &[[-0.55, 13.4], [0.35, 13.45], [0.45, 13.88], [-0.45, 14.12]],
            0.94,
            1.03,
        );
    });
    if !b.fine() {
        return;
    }
    // Vents cut in the cheek plates.
    b.paint(ACCENT);
    b.mirror_y(|b| {
        for z in [13.56, 13.7, 13.84] {
            b.block(v3(-0.36, 1.02, z), v3(0.12, 1.05, z + 0.06));
        }
    });
    // A keel down the crown from the brow to the nape.
    b.paint(PLATING);
    b.extrude_y(
        &[
            [-0.72, 14.44],
            [0.3, 14.44],
            [0.9, 14.25],
            [0.92, 14.32],
            [0.3, 14.55],
            [-0.72, 14.55],
        ],
        -0.06,
        0.06,
    );
    // One sensor pod on the left of the crown, its lens looking forward.
    b.paint(ACCENT);
    b.block(v3(-0.62, 0.42, 14.28), v3(-0.08, 0.68, 14.5));
    b.paint(VISOR).pattern(pattern::PLAIN);
    b.block(v3(-0.08, 0.47, 14.33), v3(-0.05, 0.63, 14.45));
    // Neck guard: a skirt of plate hanging off the back.
    b.paint(ACCENT);
    b.extrude_y(
        &[[-0.98, 13.2], [-0.74, 13.2], [-0.74, 13.75], [-0.86, 13.75]],
        -0.6,
        0.6,
    );
}
