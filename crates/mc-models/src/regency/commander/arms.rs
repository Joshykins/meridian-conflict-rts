//! The forearms, each pitching about its elbow: the fusion cannon on the right
//! (`rig::ARM_GUN`) and the taloned hand that builds on the left (`rig::ARM_TOOL`), with
//! the engineering suites' kit.
//!
//! The cannon telescopes: a heavy faceted housing, then two sleeves each narrower than
//! the last, their mouths lapped over the next, and a split muzzle of two jaws with the
//! bore lit red between them. It is one weapon grown out of the arm, not a tube on it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::rig;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::plating::joint;
use super::form::ram;
use super::form::{blade, ring, sleeve, KEEL, OCT};
use super::{ELBOW, EMITTER, LANCE_TIP, MUZZLE};

/// Where the sleeves leave the housing; they kick back this far when it fires.
const BREECH: f32 = 3.4;
const KICK: f32 = 0.45;

/// A forearm's armoured housing along `y`, from behind the elbow to `front`, `w` either
/// side, with a blade laid back along its top past the elbow and one down its outside
/// (`out`: +1 left, -1 right).
fn housing(b: &mut MeshBuilder, y: f32, front: f32, w: f32, out: f32) {
    let z = ELBOW.z;
    let c = |x: f32| v3(x, y, z);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(c(-1.1), Vec3::Z, w * 0.75, 0.8),
            ring(c(0.3), Vec3::Z, w, 1.08),
            ring(c(front - 0.6), Vec3::Z, w * 0.95, 1.0),
            ring(c(front), Vec3::Z, w * 0.82, 0.86),
        ],
        &OCT,
    );
    blade(
        b,
        c(front - 0.1) + Vec3::Z * 0.95,
        Vec3::new(-1.0, 0.0, 0.14),
        Vec3::Z,
        front + 2.4,
        w * 0.72,
        -0.4 * out,
        0.3,
    );
    blade(
        b,
        c(front - 0.4) + Vec3::Y * (out * w * 0.95),
        Vec3::new(-1.0, 0.0, 0.2),
        Vec3::Y * out,
        front + 1.6,
        0.72,
        0.6,
        0.26,
    );
}

/// The right forearm: the fusion cannon.
pub(super) fn cannon(b: &mut MeshBuilder) {
    let (y, z) = (MUZZLE.y, MUZZLE.z);
    let c = |x: f32| v3(x, y, z);
    b.set_recoil(c(BREECH), MUZZLE, KICK);
    b.with_limb(rig::ARM_GUN, |b| {
        housing(b, y, BREECH, 1.12, -1.0);
        if b.fine() {
            cable(
                b,
                &[
                    c(0.2) - Vec3::Z * 0.95,
                    c(2.6) - Vec3::Z * 1.0,
                    c(BREECH) - Vec3::Z * 0.7,
                ],
                0.13,
            );
            ram(b, c(-0.6) + Vec3::Y * 1.0, c(2.6) + Vec3::Y * 0.9, 0.17);
        }
        b.with_recoil(|b| {
            // The first sleeve, its root inside the housing.
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(c(BREECH - 0.5), Vec3::Z, 0.82, 0.82),
                    ring(c(BREECH + 0.4), Vec3::Z, 0.86, 0.86),
                    ring(c(6.3), Vec3::Z, 0.66, 0.66),
                ],
                &OCT,
            );
            // Bronze showing where the second sleeve leaves the first.
            metal(b);
            b.cylinder_between(c(6.2), c(6.55), 0.52, 0.52, b.sides(8));
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(c(6.45), Vec3::Z, 0.6, 0.6),
                    ring(c(6.8), Vec3::Z, 0.62, 0.62),
                    ring(c(8.4), Vec3::Z, 0.46, 0.46),
                ],
                &OCT,
            );
            // The split muzzle: two keeled jaws above and below the bore.
            for up in [1.0f32, -1.0] {
                sleeve(
                    b,
                    &[
                        ring(c(8.1) + Vec3::Z * (up * 0.36), Vec3::Z * up, 0.42, 0.26),
                        ring(c(8.9) + Vec3::Z * (up * 0.4), Vec3::Z * up, 0.36, 0.22),
                        ring(MUZZLE + Vec3::Z * (up * 0.26), Vec3::Z * up, 0.12, 0.08),
                    ],
                    &KEEL,
                );
            }
            // Heat slits down the sleeves' sides.
            if b.fine() {
                b.paint(GLOW_LASER);
                for side in [1.0f32, -1.0] {
                    b.beam(
                        c(4.4) + Vec3::Y * (side * 0.8),
                        c(5.9) + Vec3::Y * (side * 0.64),
                        Vec2::new(0.04, 0.09),
                        Vec2::new(0.04, 0.07),
                    );
                }
            }
            // The bore, lit between the jaws.
            seam(b);
            b.cylinder_between(c(8.35), c(9.1), 0.3, 0.26, b.sides(8));
            b.paint(GLOW_LASER);
            b.cylinder_between(c(9.0), MUZZLE, 0.2, 0.14, b.sides(8));
        });
    });
}

/// The talons round the palm: (angle round the forearm's axis in degrees from up, reach).
const TALONS: [(f32, f32); 4] = [(0.0, 1.0), (95.0, 0.9), (185.0, 0.95), (265.0, 0.85)];

/// The left forearm: the taloned hand. The housing, a bronze wrist, the palm with the
/// violet nanite emitter in it (where the build beam leaves), and four long jointed
/// talons curled round it that open while it builds (`rig::WORK_BREATHE`).
pub(super) fn claw(b: &mut MeshBuilder) {
    let (y, z) = (EMITTER.y, EMITTER.z);
    let c = |x: f32| v3(x, y, z);
    b.with_limb(rig::ARM_TOOL, |b| {
        housing(b, y, 2.9, 1.0, 1.0);
        metal(b);
        b.cylinder_between(c(2.8), c(3.4), 0.62, 0.62, b.sides(10));
        seam(b);
        sleeve(
            b,
            &[
                ring(c(3.3), Vec3::Z, 0.7, 0.6),
                ring(c(4.2), Vec3::Z, 0.82, 0.55),
                ring(c(4.8), Vec3::Z, 0.55, 0.4),
            ],
            &OCT,
        );
        b.paint(GLOW_VIOLET);
        b.cylinder_between(c(4.6), EMITTER, 0.34, 0.22, b.sides(10));
        b.with_work(rig::WORK_BREATHE, |b| {
            for &(deg, reach) in &TALONS {
                let a = deg.to_radians();
                let dir = Vec3::new(0.0, -a.sin(), a.cos());
                let at = |x: f32, r: f32| c(x) + dir * r;
                let joints = [
                    at(3.9, 0.55),
                    at(5.4, 1.0),
                    at(5.4 + 1.3 * reach, 0.95),
                    at(5.4 + 2.2 * reach, 0.4),
                ];
                dark_plate(b);
                let talon = [
                    ring(joints[0], dir, 0.26, 0.28),
                    ring(joints[1], dir, 0.24, 0.26),
                    ring(joints[2], dir, 0.18, 0.2),
                    ring(joints[3], dir, 0.03, 0.03),
                ];
                if b.fine() {
                    sleeve(b, &talon, &KEEL);
                } else {
                    sleeve(b, &[talon[0], talon[1], talon[3]], &KEEL);
                }
                if b.fine() {
                    metal(b);
                    let hinge = dir.cross(Vec3::X) * 0.24;
                    for p in [joints[1], joints[2]] {
                        b.cylinder_between(p - hinge, p + hinge, 0.17, 0.17, 6);
                    }
                }
            }
        });
        suite_2(b, y, z);
        suite_3(b, y, z);
    });
}

/// Engineering Suite II: two emitters on the forearm, rocking while it builds, and two
/// bronze feed canisters along its outside piped to the palm.
fn suite_2(b: &mut MeshBuilder, y: f32, z: f32) {
    b.module("eng_2", 0.3, |b| {
        b.with_work(rig::WORK_TWIST, |b| {
            for dy in [-0.42f32, 0.42] {
                metal(b);
                b.cylinder_between(
                    v3(0.4, y + dy, z + 1.45),
                    v3(1.9, y + dy, z + 1.5),
                    0.22,
                    0.19,
                    b.sides(8),
                );
                b.paint(GLOW_VIOLET);
                b.cylinder_between(
                    v3(1.9, y + dy, z + 1.5),
                    v3(2.25, y + dy, z + 1.5),
                    0.17,
                    0.09,
                    b.sides(8),
                );
            }
        });
        metal(b);
        for dz in [-0.4f32, 0.35] {
            b.cylinder_between(
                v3(-0.4, y + 1.55, z + dz),
                v3(2.1, y + 1.55, z + dz),
                0.27,
                0.27,
                b.sides(8),
            );
        }
        if b.fine() {
            cable(
                b,
                &[
                    v3(2.1, y + 1.55, z),
                    v3(3.2, y + 1.0, z),
                    v3(4.0, y + 0.7, z),
                ],
                0.1,
            );
        }
    });
}

/// Engineering Suite III: a lance out of the palm that runs out while it builds (its tip
/// the unit file's `arm_emitter`), a ring of emitters round the wrist, and a feed drum on
/// a bracket over the elbow.
fn suite_3(b: &mut MeshBuilder, y: f32, z: f32) {
    b.module("eng_3", 0.3, |b| {
        b.with_work(rig::WORK_EXTEND, |b| {
            metal(b);
            b.cylinder_between(
                v3(3.9, y, z),
                v3(LANCE_TIP - 0.3, y, z),
                0.18,
                0.11,
                b.sides(8),
            );
            b.paint(GLOW_VIOLET);
            b.cylinder_between(
                v3(LANCE_TIP - 0.3, y, z),
                v3(LANCE_TIP, y, z),
                0.11,
                0.03,
                b.sides(8),
            );
        });
        b.paint(GLOW_VIOLET);
        for deg in [60.0f32, 180.0, 300.0] {
            let a = deg.to_radians();
            let at = |r: f32| v3(3.1, y - a.sin() * r, z + a.cos() * r);
            b.cylinder_between(at(0.64), at(0.94), 0.14, 0.09, 6);
        }
        joint(b, v3(-0.9, y + 0.2, z + 1.75), Vec3::X * 0.7, 0.48);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            v3(-0.25, y + 0.2, z + 1.75),
            v3(-0.1, y + 0.2, z + 1.75),
            0.5,
            0.5,
            b.sides(10),
        );
    });
}
