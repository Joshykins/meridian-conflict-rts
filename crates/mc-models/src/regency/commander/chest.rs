//! The chest: a deep core thrust forward from the waist to the collar, a keeled prow down
//! the middle like a jet's nose, and either side courses of thick plates laid from it up
//! and back toward the shoulders, lapped like the Anvil's feathered wings; the flanks
//! lapped back and blades up either side of the neck.

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};

use super::super::kit::{dark_plate, seam, v3};
use super::form::{blade, ring, sleeve, KEEL};

/// The chest's plan: a keel at the front, broad behind (y scaled by each section).
const PLAN: [[f32; 2]; 9] = [
    [2.3, 0.0],
    [2.0, 0.5],
    [1.0, 1.0],
    [-1.4, 1.0],
    [-2.1, 0.55],
    [-2.1, -0.55],
    [-1.4, -1.0],
    [1.0, -1.0],
    [2.0, -0.5],
];

pub(super) fn chest(b: &mut MeshBuilder) {
    seam(b);
    b.loft_z(
        &PLAN,
        &[
            Section::scaled(14.6, 0.8, 1.8),
            Section::scaled(16.3, 1.0, 2.6),
            Section::scaled(18.3, 1.08, 3.1),
            Section::scaled(19.7, 1.0, 2.9),
            Section::scaled(20.4, 0.7, 1.9).shifted(-0.2, 0.0),
        ],
    );
    prow(b);
    b.mirror_y(sides);
}

/// The flanks and the collar, whichever the front.
fn sides(b: &mut MeshBuilder) {
    dark_plate(b);
    for (k, z) in [16.2f32, 17.6].into_iter().enumerate() {
        blade(
            b,
            v3(0.9 - 0.4 * k as f32, 2.95 + 0.12 * k as f32, z),
            Vec3::new(-1.0, 0.1, 0.1),
            Vec3::new(0.0, 1.0, 0.1),
            3.2,
            0.65,
            0.3,
            0.32,
        );
    }
    blade(
        b,
        v3(0.9, 1.45, 20.0),
        Vec3::new(0.1, 0.3, 1.0),
        Vec3::new(0.8, 0.6, 0.0),
        2.1,
        0.55,
        0.5,
        0.3,
    );
}

/// A keeled prow down the middle and courses of plates laid from it toward the
/// shoulders.
fn prow(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(1.9, 0.0, 14.3), Vec3::X, 0.3, 0.2),
            ring(v3(2.55, 0.0, 15.6), Vec3::X, 0.75, 0.45),
            ring(v3(2.9, 0.0, 17.6), Vec3::X, 0.9, 0.55),
            ring(v3(2.65, 0.0, 19.5), Vec3::X, 0.8, 0.45),
            ring(v3(1.9, 0.0, 20.6), Vec3::X, 0.4, 0.22),
        ],
        &KEEL,
    );
    b.mirror_y(|b| {
        // Lowest first, so each course laps over the one under it.
        for k in 0..3 {
            let k = k as f32;
            blade(
                b,
                v3(2.55 - 0.05 * k, 0.6, 15.3 + 1.6 * k),
                Vec3::new(-0.6, 1.0, 0.25 + 0.15 * k),
                Vec3::new(0.85, 0.5, 0.1),
                3.2 + 0.35 * k,
                0.78,
                -0.35,
                0.4,
            );
        }
    });
}
