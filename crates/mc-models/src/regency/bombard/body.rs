//! The hull: a long faceted body over the legs (authored low, raised by `lifted`), a keeled sensor head with red slit
//! optics under a plate brow, three plates lapped back over the top either side of the
//! ring, and bronze hip drums where the legs meet it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::{lifted, ANKLE, HIP, KNEE, RING, SOLE};

pub(super) fn hull(b: &mut MeshBuilder) {
    lifted(b, body);
    // The hip drums the legs turn in, bronze.
    b.mirror_y(|b| {
        let hip = HIP;
        let out = (ANKLE - hip).truncate().extend(0.0).normalize_or(Vec3::Y);
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(hip - out * 0.35, hip + out * 0.2, 0.42, 0.36, sides);
    });
    // The ring's bed: a bronze drum the turret ring sits on.
    metal(b);
    let sides = b.sides(12);
    b.cylinder_between(RING - Vec3::Z * 0.2, RING, 0.95, 0.95, sides);
}

fn body(b: &mut MeshBuilder) {
    // The body: a faceted octagon from the tail to the neck, deepest under the ring.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-2.45, 0.0, 3.05), Vec3::Z, 0.55, 0.38),
            ring(v3(-1.7, 0.0, 3.05), Vec3::Z, 1.05, 0.62),
            ring(v3(0.2, 0.0, 3.05), Vec3::Z, 1.18, 0.72),
            ring(v3(1.6, 0.0, 3.1), Vec3::Z, 1.0, 0.62),
            ring(v3(2.1, 0.0, 3.12), Vec3::Z, 0.7, 0.48),
        ],
        &OCT,
    );
    head(b);
    // The plates lapped back over the top, either side of the ring, each a keeled
    // blade whose trailing edge is swept back into a point past the one behind.
    b.mirror_y(|b| {
        for (i, x) in [1.65f32, 0.55, -0.6, -1.65].into_iter().enumerate() {
            let w = 1.15 - i as f32 * 0.08;
            dark_plate(b);
            blade(
                b,
                v3(x, 0.35, 3.72 - i as f32 * 0.03),
                v3(-1.0, 0.25, -0.18),
                v3(0.0, 0.45, 1.0),
                1.25,
                0.42,
                0.8,
                0.22,
            );
            if b.fine() {
                // A flank plate under it, hanging past the body's side.
                blade(
                    b,
                    v3(x + 0.1, w, 3.35),
                    v3(-1.0, 0.18, -0.25),
                    v3(0.0, 1.0, 0.35),
                    1.15,
                    0.32,
                    0.6,
                    0.18,
                );
            }
        }
    });
    // The owner's colour: a chevron across the front plates, ahead of the ring.
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.beam(
            v3(2.05, 0.08, 3.72),
            v3(1.6, 0.62, 3.62),
            Vec2::new(0.16, 0.03),
            Vec2::new(0.16, 0.03),
        )
    });
    // The belly: a flat seam plate under it all.
    seam(b);
    b.frustum(
        v3(-0.1, 0.0, 2.32),
        Vec2::new(3.2, 1.2),
        Vec2::new(3.8, 1.6),
        0.18,
        Vec2::ZERO,
    );
}

/// The head: a keeled wedge out front, a plate brow over three red slits a side, and a
/// bronze sensor bar under the chin.
fn head(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(1.95, 0.0, 3.15), Vec3::Z, 0.62, 0.5),
            ring(v3(2.6, 0.0, 3.12), Vec3::Z, 0.56, 0.42),
            ring(v3(3.3, 0.0, 3.0), Vec3::Z, 0.36, 0.28),
            ring(v3(3.95, 0.0, 2.88), Vec3::Z, 0.1, 0.08),
        ],
        &KEEL,
    );
    // The brow, swept back off the nose.
    blade(
        b,
        v3(3.7, 0.0, 3.08),
        v3(-1.0, 0.0, 0.22),
        v3(0.25, 0.0, 1.0),
        1.6,
        0.5,
        0.0,
        0.16,
    );
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        for (i, x) in [3.3f32, 3.02, 2.74].into_iter().enumerate() {
            if i > 0 && !b.fine() {
                continue;
            }
            let y = 0.2 + i as f32 * 0.1;
            b.beam(
                v3(x, y, 3.04),
                v3(x - 0.16, y + 0.05, 3.06),
                Vec2::new(0.05, 0.05),
                Vec2::new(0.05, 0.04),
            );
        }
    });
    metal(b);
    b.beam(
        v3(3.0, -0.3, 2.74),
        v3(3.0, 0.3, 2.74),
        Vec2::new(0.16, 0.14),
        Vec2::new(0.16, 0.14),
    );
}

/// Far off: a wedge of a body, flat legs that do not walk, the pod a box on the ring.
pub(super) fn coarse(b: &mut MeshBuilder) {
    lifted(b, |b| {
        dark_plate(b);
        b.frustum(
            v3(0.6, 0.0, 2.45),
            Vec2::new(6.4, 2.0),
            Vec2::new(3.6, 1.4),
            1.25,
            Vec2::new(-0.2, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(1.6, 0.0, 3.71),
            v3(1.0, 0.45, 3.71),
            v3(1.0, -0.45, 3.71),
        ]);
    });
    // Each leg bone a fin still posed by the rig, the boot a wedge, so it walks from afar.
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            let fin = |b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32| {
                let (a0, a1) = (a - Vec3::X * w, a + Vec3::X * w);
                b.face(&[a0, a1, c]);
                b.face(&[a1, a0, c]);
            };
            b.with_limb(rig::THIGH, |b| fin(b, HIP, KNEE, 0.4));
            b.with_limb(rig::SHIN, |b| fin(b, ANKLE, KNEE, 0.3));
            b.with_limb(rig::FOOT, |b| {
                let w = SOLE.2 * 0.5;
                b.face(&[
                    v3(ANKLE.x + SOLE.1, ANKLE.y, 0.02),
                    v3(ANKLE.x + SOLE.0, ANKLE.y + w, 0.7),
                    v3(ANKLE.x + SOLE.0, ANKLE.y - w, 0.7),
                ]);
            });
        });
    });
    super::pod::coarse(b);
}
