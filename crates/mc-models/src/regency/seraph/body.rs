//! The body: legs hanging from the hips with their toes pointed, a narrow waist under a
//! deep plated chest, high pauldrons, a horned helm. Every piece a faceted solid under
//! dark plates laid back over graphite machinery.

use glam::{Affine3A, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::gpu_consts::flight;
use crate::material::*;
use crate::pattern;

use super::super::commander::form::{ball, blade, ring, sleeve, ARCH, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, swept, Frame};
use super::{Pose, NECK, SHOULDER, WAIST};

/// A bone from `a` to `c` in the leg's plane: its direction, and the normal out of its
/// front (forward).
fn bone(a: Vec3, c: Vec3) -> (Vec3, Vec3) {
    let u = (c - a).normalize();
    let n = Vec3::new(-u.z, 0.0, u.x);
    (u, if n.x < 0.0 { -n } else { n })
}

/// A leg on the left side (the right is built as a left one and mirrored), fixed in its
/// hanging pose.
/// The thigh swings about the hip as it trails, the shin and foot about the knee
/// (`gpu_consts::flight`).
pub(super) fn leg(b: &mut MeshBuilder, p: Pose) {
    b.with_flight(flight::THIGH, |b| thigh(b, p.hip, p.knee));
    b.with_flight(flight::SHIN, |b| {
        shin(b, p.knee, p.ankle);
        foot(b, p.ankle, p.toe);
    });
}

fn thigh(b: &mut MeshBuilder, hip: Vec3, knee: Vec3) {
    let (u, n) = bone(hip, knee);
    let at = |t: f32, fwd: f32| hip.lerp(knee, t) + n * fwd;
    metal(b);
    ball(b, hip, 0.95);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(0.05, 0.0), n, 0.95, 0.95),
            ring(at(0.25, 0.12), n, 1.12, 1.18),
            ring(at(0.6, 0.12), n, 1.0, 1.08),
            ring(at(0.94, 0.05), n, 0.72, 0.8),
        ],
        &KEEL,
    );
    // The cuisse down the outside, swept up past the hip into a point.
    blade(
        b,
        at(0.78, -0.05) + Vec3::Y * 1.02,
        -u - n * 0.2,
        Vec3::Y,
        3.5,
        0.82,
        -0.55,
        0.26,
    );
    // The front plate, its point up at the hip.
    blade(b, at(0.82, 1.15), -u + n * 0.06, n, 3.0, 0.78, 0.0, 0.28);
}

fn shin(b: &mut MeshBuilder, knee: Vec3, ankle: Vec3) {
    let (u, n) = bone(knee, ankle);
    let at = |t: f32, fwd: f32| knee.lerp(ankle, t) + n * fwd;
    metal(b);
    b.cylinder_between(
        knee - Vec3::Y * 0.7,
        knee + Vec3::Y * 0.7,
        0.55,
        0.55,
        b.sides(10),
    );
    dark_plate(b);
    // The shin, flaring into a greave over the ankle, the calf deep behind.
    sleeve(
        b,
        &[
            ring(at(0.08, 0.0), n, 0.8, 0.85),
            ring(at(0.35, -0.1), n, 0.82, 1.0),
            ring(at(0.68, -0.2), n, 0.95, 1.22),
            ring(at(0.92, -0.14), n, 0.9, 1.1),
            ring(at(1.02, -0.05), n, 0.7, 0.8),
        ],
        &OCT,
    );
    // The knee guard, thrust up and forward into a point.
    sleeve(
        b,
        &[
            ring(at(0.24, 0.62), n, 0.92, 0.5),
            ring(at(0.05, 0.92), n, 0.98, 0.66),
            ring(at(-0.12, 1.1), n, 0.7, 0.44),
            ring(at(-0.22, 1.5), n, 0.06, 0.06),
        ],
        &KEEL,
    );
    // The greave down the front, its point up under the guard.
    blade(b, at(0.95, 1.12), -u + n * 0.1, n, 2.9, 1.0, 0.0, 0.3);
    // Down the outside, swept up and back.
    blade(
        b,
        at(0.92, -0.25) + Vec3::Y * 1.12,
        -u - n * 0.1,
        Vec3::Y,
        2.6,
        0.85,
        -0.5,
        0.22,
    );
    // The calf's vent: the lift's heat let out through a slotted plate.
    b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
    armour(
        b,
        &Frame::new(at(0.62, -1.42), -u, -n),
        &[[-1.0, -0.42], [1.0, -0.42], [1.0, 0.42], [-1.0, 0.42]],
        0.14,
    );
    if !b.fine() {
        return;
    }
    dark_plate(b);
    // A fin swept back off the calf.
    blade(
        b,
        at(0.4, -1.32),
        -n * 0.7 - u * 0.7,
        Vec3::Y,
        2.2,
        0.55,
        0.5,
        0.22,
    );
}

/// The foot, hanging toe down: a long pointed sabot from the ankle to its toe, armoured
/// over the instep, a spur behind the heel.
fn foot(b: &mut MeshBuilder, ankle: Vec3, toe: Vec3) {
    let (u, n) = bone(ankle, toe);
    let at = |t: f32, fwd: f32| ankle.lerp(toe, t) + n * fwd;
    metal(b);
    ball(b, ankle, 0.7);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(0.0, -0.15), n, 0.62, 0.78),
            ring(at(0.3, 0.05), n, 0.72, 0.82),
            ring(at(0.7, 0.08), n, 0.45, 0.48),
            ring(at(1.0, 0.0), n, 0.08, 0.1),
        ],
        &OCT,
    );
    // The instep plate, from the toe laid back up toward the greave.
    blade(b, at(0.95, 0.12), -u + n * 0.15, n, 2.9, 0.82, 0.0, 0.26);
    // The heel spur, swept back and down.
    blade(b, at(0.15, -0.9), u * 0.4 - n, Vec3::Y, 1.6, 0.45, 0.0, 0.2);
}

/// The pelvis under the waist ring: a faceted block, a keeled fauld down the front, a
/// plate hanging over each hip and one behind.
pub(super) fn pelvis(b: &mut MeshBuilder) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [1.3, 0.0],
                [1.0, 1.35],
                [0.0, 1.9],
                [-1.1, 1.5],
                [-1.4, 0.0],
                [-1.1, -1.5],
                [0.0, -1.9],
                [1.0, -1.35],
            ],
            &[
                Section::scaled(12.0, 0.7, 0.8),
                Section::new(13.1, 1.0),
                Section::new(WAIST - 0.1, 0.92),
            ],
        );
    });
    metal(b);
    b.cylinder_between(
        Vec3::Z * (WAIST - 0.3),
        Vec3::Z * (WAIST + 0.3),
        1.5,
        1.5,
        b.sides(12),
    );
    dark_plate(b);
    // The fauld: a keeled plate down the front to a point between the thighs.
    sleeve(
        b,
        &[
            ring(v3(1.35, 0.0, 14.2), Vec3::X, 0.95, 0.3),
            ring(v3(1.45, 0.0, 12.9), Vec3::X, 0.75, 0.34),
            ring(v3(1.2, 0.0, 11.3), Vec3::X, 0.08, 0.1),
        ],
        &KEEL,
    );
    // The hip plates, hung out over the thighs and swept down.
    b.mirror_y(|b| {
        armour(
            b,
            &Frame::new(
                v3(0.25, 1.95, 14.2),
                v3(0.0, 0.32, -1.0),
                v3(0.15, 1.0, 0.3),
            ),
            &swept(3.4, 1.15, 0.2, 0.55),
            0.24,
        );
    });
    // Behind: a plate over the seat, swept down and back.
    armour(
        b,
        &Frame::new(
            v3(-1.45, 0.0, 14.2),
            v3(-0.3, 0.0, -1.0),
            Vec3::new(-1.0, 0.0, 0.2),
        ),
        &swept(2.6, 1.2, 0.0, 0.5),
        0.24,
    );
}

/// The torso above the waist ring: a narrow banded abdomen, the chest broadening to the
/// shoulders under a keeled prow, vents either side of it, the collar and pauldrons.
pub(super) fn torso(b: &mut MeshBuilder) {
    // The abdomen: banded graphite between plates.
    metal(b);
    b.with_facets(|b| {
        b.loft_z(
            &OCT_PLAN,
            &[
                Section::scaled(WAIST, 1.0, 1.1),
                Section::scaled(16.3, 1.1, 1.3),
            ],
        );
    });
    dark_plate(b);
    for z in [15.0f32, 15.75] {
        b.with_facets(|b| {
            b.loft_z(
                &OCT_PLAN,
                &[
                    Section::scaled(z, 1.18, 1.32),
                    Section::scaled(z + 0.45, 1.26, 1.42),
                ],
            );
        });
    }
    // The chest, broad and deep, swept in at the back.
    b.with_facets(|b| {
        b.loft_z(
            &CHEST,
            &[
                Section::scaled(16.2, 0.72, 0.62),
                Section::scaled(17.4, 0.95, 0.88),
                Section::scaled(19.2, 1.0, 1.0),
                Section::scaled(20.3, 0.82, 0.7).shifted(-0.15, 0.0),
            ],
        );
    });
    // The prow: a keel down the breastbone, plates laid back from it either side.
    sleeve(
        b,
        &[
            ring(v3(1.62, 0.0, 20.0), Vec3::X, 0.5, 0.22),
            ring(v3(1.82, 0.0, 18.6), Vec3::X, 0.62, 0.3),
            ring(v3(1.4, 0.0, 16.6), Vec3::X, 0.08, 0.1),
        ],
        &KEEL,
    );
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(1.72, 0.45, 19.3),
            v3(-0.35, 1.0, -0.15),
            v3(1.0, 0.3, 0.35),
        );
        armour(b, &f, &swept(2.3, 0.9, -0.3, 0.6), 0.22);
        // The breast vent beside the keel: its heat in the plate.
        b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
        armour(
            b,
            &Frame::new(v3(1.55, 0.95, 17.6), v3(-0.2, 1.0, 0.0), v3(1.0, 0.25, 0.1)),
            &[[0.0, -0.32], [1.3, -0.32], [1.15, 0.32], [0.0, 0.32]],
            0.12,
        );
        dark_plate(b);
        pauldron(b);
    });
    // The collar round the neck.
    seam(b);
    b.with_facets(|b| {
        b.loft_z(
            &OCT_PLAN,
            &[
                Section::scaled(20.2, 0.9, 1.1),
                Section::scaled(20.75, 0.7, 0.85),
            ],
        );
    });
}

/// An octagon an armour band is lofted through (x forward, y across).
const OCT_PLAN: [[f32; 2]; 8] = [
    [1.1, 0.45],
    [0.5, 1.15],
    [-0.5, 1.15],
    [-1.0, 0.45],
    [-1.0, -0.45],
    [-0.5, -1.15],
    [0.5, -1.15],
    [1.1, -0.45],
];

/// The chest's plan: a prow in front, broad at the shoulders, its back cut in for the
/// wing roots.
const CHEST: [[f32; 2]; 10] = [
    [1.8, 0.0],
    [1.5, 1.4],
    [0.6, 2.85],
    [-0.8, 2.75],
    [-1.6, 1.4],
    [-1.75, 0.0],
    [-1.6, -1.4],
    [-0.8, -2.75],
    [0.6, -2.85],
    [1.5, -1.4],
];

/// The left pauldron: an arch over the shoulder ball swept back into a blade, a second
/// plate lapped under it.
fn pauldron(b: &mut MeshBuilder) {
    let s = SHOULDER;
    metal(b);
    ball(b, s, 0.9);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(s + v3(1.45, 0.3, 0.2), Vec3::Z, 1.05, 1.05),
            ring(s + v3(0.4, 0.55, 0.55), Vec3::Z, 1.35, 1.25),
            ring(s + v3(-0.8, 0.5, 0.45), Vec3::Z, 1.25, 1.15),
            ring(s + v3(-2.4, 0.35, 0.9), Vec3::Z, 0.15, 0.2),
        ],
        &ARCH,
    );
    // A plate lapped under its rim, swept down over the upper arm.
    armour(
        b,
        &Frame::new(
            s + v3(0.6, 1.5, -0.2),
            v3(-0.3, 0.25, -1.0),
            v3(0.0, 1.0, 0.0),
        ),
        &swept(2.0, 0.95, 0.3, 0.55),
        0.2,
    );
}

/// The helm on its neck, idling about `NECK`: a narrow skull swept back into a crest, a
/// red visor slit across its face, two horns raked up and out off the brow, a chin guard.
pub(super) fn head(b: &mut MeshBuilder) {
    metal(b);
    b.cylinder_between(
        NECK - v3(0.2, 0.0, 0.5),
        NECK + v3(0.0, 0.0, 0.4),
        0.45,
        0.4,
        b.sides(8),
    );
    let grow = Affine3A::from_translation(NECK)
        * Affine3A::from_scale(Vec3::splat(1.1))
        * Affine3A::from_translation(-NECK);
    b.with(grow, |b| {
        b.with_head(NECK, |b| {
            let h = |dx: f32, dy: f32, dz: f32| NECK + v3(dx, dy, dz);
            // The face behind the visor.
            seam(b);
            b.with_facets(|b| {
                b.loft_z(
                    &[
                        [0.75, 0.0],
                        [0.55, 0.5],
                        [-0.5, 0.55],
                        [-0.6, 0.0],
                        [-0.5, -0.55],
                        [0.55, -0.5],
                    ],
                    &[
                        Section::scaled(NECK.z + 0.2, 0.7, 0.7).shifted(NECK.x + 0.1, 0.0),
                        Section::new(NECK.z + 1.1, 1.0).shifted(NECK.x, 0.0),
                        Section::scaled(NECK.z + 1.7, 0.9, 0.9).shifted(NECK.x - 0.05, 0.0),
                    ],
                );
            });
            // The visor: a red slit across the face, the light in the plate.
            b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
            armour(
                b,
                &Frame::new(h(0.72, -0.5, 1.12), Vec3::Y, v3(1.0, 0.0, 0.1)),
                &[[0.0, -0.13], [1.0, -0.13], [1.0, 0.13], [0.0, 0.13]],
                0.08,
            );
            // The helm: arched over the skull from the brow, swept back into a crest.
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(h(0.85, 0.0, 1.45), Vec3::Z, 0.62, 0.45),
                    ring(h(0.1, 0.0, 1.75), Vec3::Z, 0.72, 0.6),
                    ring(h(-0.7, 0.0, 1.55), Vec3::Z, 0.6, 0.5),
                    ring(h(-1.6, 0.0, 1.9), Vec3::Z, 0.06, 0.08),
                ],
                &ARCH,
            );
            // The chin guard, a keeled wedge to a point under the visor.
            sleeve(
                b,
                &[
                    ring(h(0.55, 0.0, 0.85), Vec3::X, 0.48, 0.2),
                    ring(h(0.85, 0.0, 0.5), Vec3::X, 0.3, 0.2),
                    ring(h(0.9, 0.0, 0.15), Vec3::X, 0.04, 0.05),
                ],
                &KEEL,
            );
            // The horns: a long blade raked up and out off each side of the brow, the
            // knight's crest, and a short one under it.
            b.mirror_y(|b| {
                blade(
                    b,
                    h(0.85, 0.12, 1.5),
                    v3(0.05, 0.75, 1.0),
                    v3(1.0, 0.0, -0.15),
                    2.4,
                    0.32,
                    0.6,
                    0.12,
                );
                blade(
                    b,
                    h(0.8, 0.35, 1.15),
                    v3(-0.3, 1.0, 0.25),
                    v3(1.0, 0.0, 0.0),
                    1.0,
                    0.2,
                    0.5,
                    0.1,
                );
            });
            // A small keel on the brow between the horns.
            b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
            blade(
                b,
                h(0.95, 0.0, 1.35),
                v3(-0.25, 0.0, 1.0),
                Vec3::X,
                0.9,
                0.18,
                0.0,
                0.1,
            );
        });
    });
}
