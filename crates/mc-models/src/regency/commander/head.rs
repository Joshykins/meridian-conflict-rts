//! The head, sunk forward and low between the pauldrons, turning about the neck while
//! idle: a long narrow face tapering to the chin under a helmet that sweeps back past the
//! skull, a heavy brow over two slit eyes, a slatted mouth plate, fins swept back from the
//! temples and a crest laid back along the crown.

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::form::{blade, ring, sleeve, ARCH, KEEL};
use super::NECK;

/// The face's plan, a wedge to the front (x forward from the head's centre, y across).
const FACE: [[f32; 2]; 7] = [
    [1.0, 0.0],
    [0.7, 0.45],
    [-0.2, 0.6],
    [-0.6, 0.35],
    [-0.6, -0.35],
    [-0.2, -0.6],
    [0.7, -0.45],
];

pub(super) fn head(b: &mut MeshBuilder) {
    let (x, z) = (NECK.x, NECK.z);
    metal(b);
    b.cylinder_between(
        NECK - v3(0.5, 0.0, 0.9),
        NECK + v3(0.2, 0.0, 0.5),
        0.62,
        0.52,
        b.sides(8),
    );
    // Authored at 1:1 and drawn a little larger about the neck.
    let grow = Affine3A::from_translation(NECK)
        * Affine3A::from_scale(Vec3::splat(1.15))
        * Affine3A::from_translation(-NECK);
    b.with(grow, |b| {
        b.with_head(NECK, |b| {
            // The face, narrowing from the cheeks to the chin.
            seam(b);
            b.loft_z(
                &FACE,
                &[
                    Section::scaled(z + 0.2, 0.55, 0.45).shifted(x + 0.45, 0.0),
                    Section::scaled(z + 1.0, 0.85, 0.85).shifted(x + 0.35, 0.0),
                    Section::scaled(z + 1.9, 0.95, 0.95).shifted(x + 0.2, 0.0),
                    Section::scaled(z + 2.3, 0.8, 0.8).shifted(x + 0.1, 0.0),
                ],
            );
            // The helmet: arched over the skull from the brow, swept back into a point.
            dark_plate(b);
            let h = |dx: f32, dz: f32| v3(x + dx, 0.0, z + dz);
            sleeve(
                b,
                &[
                    ring(h(1.25, 1.6), Vec3::Z, 0.62, 0.7),
                    ring(h(0.4, 1.75), Vec3::Z, 0.82, 0.95),
                    ring(h(-0.6, 1.8), Vec3::Z, 0.78, 0.9),
                    ring(h(-1.7, 2.1), Vec3::Z, 0.2, 0.25),
                ],
                &ARCH,
            );
            // The brow: a heavy ledge over the eyes, pointed at the front.
            blade(
                b,
                h(1.55, 1.62),
                Vec3::new(-1.0, 0.0, 0.25),
                Vec3::new(0.3, 0.0, 1.0),
                1.6,
                0.62,
                0.0,
                0.26,
            );
            // The crest, laid back along the crown and past it.
            sleeve(
                b,
                &[
                    ring(h(1.1, 2.35), Vec3::Z, 0.1, 0.12),
                    ring(h(0.2, 2.72), Vec3::Z, 0.16, 0.3),
                    ring(h(-1.3, 2.75), Vec3::Z, 0.12, 0.22),
                    ring(h(-2.3, 2.9), Vec3::Z, 0.02, 0.02),
                ],
                &KEEL,
            );
            // The mouth plate, pointed at the chin, slatted.
            blade(
                b,
                h(1.05, 1.05),
                Vec3::new(0.25, 0.0, -1.0),
                Vec3::new(1.0, 0.0, 0.2),
                0.9,
                0.4,
                0.0,
                0.14,
            );
            if b.fine() {
                metal(b);
                for dy in [-0.16f32, 0.0, 0.16] {
                    b.beam(
                        h(1.2, 0.95) + Vec3::Y * dy,
                        h(1.12, 0.5) + Vec3::Y * dy,
                        Vec2::new(0.06, 0.08),
                        Vec2::new(0.06, 0.08),
                    );
                }
            }
            b.mirror_y(|b| {
                // An eye: a slit slanted up and out under the brow.
                b.paint(GLOW_LASER);
                b.beam(
                    h(1.33, 1.37) + Vec3::Y * 0.1,
                    h(1.14, 1.47) + Vec3::Y * 0.42,
                    Vec2::new(0.08, 0.1),
                    Vec2::new(0.08, 0.06),
                );
                dark_plate(b);
                // A fin swept back from the temple past the helmet.
                blade(
                    b,
                    h(0.7, 1.35) + Vec3::Y * 0.7,
                    Vec3::new(-1.0, 0.3, 0.22),
                    Vec3::new(0.0, 1.0, 0.25),
                    2.2,
                    0.42,
                    0.55,
                    0.18,
                );
                // A cheek plate, swept back along the jaw.
                blade(
                    b,
                    h(1.0, 0.75) + Vec3::Y * 0.5,
                    Vec3::new(-1.0, 0.15, -0.25),
                    Vec3::new(0.2, 1.0, 0.0),
                    1.3,
                    0.32,
                    -0.4,
                    0.14,
                );
            });
        })
    });
}
