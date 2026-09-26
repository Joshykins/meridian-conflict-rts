//! The commander's forearms, each pitching about its elbow: the plasmeric cannon on the
//! right (`rig::ARM_GUN`) and the clawed hand that builds on the left (`rig::ARM_TOOL`),
//! with the engineering suites' kit.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::rig;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::plating::{joint, plate, side_plate};
use super::{ELBOW, EMITTER, LANCE_TIP, MUZZLE};

/// Where the barrel leaves the housing; it kicks back this far when it fires.
const BREECH: f32 = 3.5;
const KICK: f32 = 0.4;

/// A forearm's housing, `y` its axis: a dark block from the elbow forward to `front`, a
/// plate on top drawn back past the elbow into a point, and a plate down the outside
/// (`out`: +1 on the left arm, -1 on the right).
fn housing(b: &mut MeshBuilder, y: f32, front: f32, half: f32, out: f32) {
    let z = ELBOW.z;
    metal(b);
    b.chamfered_box(
        v3((front - 0.4) * 0.5, y, z),
        v3(front + 0.4, half * 2.0, 1.9),
        0.4,
    );
    dark_plate(b);
    plate(
        b,
        &[
            v3(front + 0.1, y - half - 0.05, z + 0.92),
            v3(front + 0.1, y + half + 0.05, z + 0.92),
            v3(-0.3, y + half + 0.1, z + 0.98),
            v3(-1.9, y + out * 0.35, z + 1.25),
            v3(-0.3, y - half - 0.1, z + 0.98),
        ],
        Vec3::Z * 0.24,
    );
    let side = y + out * (half + 0.02);
    side_plate(
        b,
        &[
            [front, z + 0.88],
            [front + 0.25, z - 0.5],
            [front - 1.1, z - 1.05],
            [-0.4, z - 1.0],
            [-1.75, z + 0.1],
            [-0.3, z + 0.9],
        ],
        side.min(side + out * 0.26),
        side.max(side + out * 0.26),
    );
}

/// The right forearm: the cannon. A heavy housing, bronze containment rings round the
/// barrel's root, the barrel itself with rings along it and a muzzle block ported at the
/// sides, its bore lit red. The barrel and muzzle block kick back when it fires.
pub(super) fn cannon(b: &mut MeshBuilder) {
    let (y, z) = (MUZZLE.y, MUZZLE.z);
    b.set_recoil(v3(BREECH, y, z), MUZZLE, KICK);
    b.with_limb(rig::ARM_GUN, |b| {
        housing(b, y, BREECH, 1.15, -1.0);
        metal(b);
        let sides = b.sides(12);
        let rings = if b.fine() { 3 } else { 1 };
        for (i, x) in [BREECH, BREECH + 0.45, BREECH + 0.9]
            .into_iter()
            .enumerate()
            .take(rings)
        {
            let r = 0.95 - 0.09 * i as f32;
            b.cylinder_between(v3(x, y, z), v3(x + 0.3, y, z), r, r, sides);
        }
        if b.fine() {
            // Heat fins across the housing's top, behind the top plate's front edge.
            dark_plate(b);
            for x in [1.0f32, 1.5, 2.0, 2.5] {
                b.block(
                    v3(x - 0.07, y - 0.85, z + 1.15),
                    v3(x + 0.07, y + 0.85, z + 1.52),
                );
            }
            // Feed lines from the housing's underside into the rings.
            cable(
                b,
                &[
                    v3(0.6, y, z - 1.0),
                    v3(2.6, y, z - 1.1),
                    v3(BREECH + 0.3, y, z - 0.7),
                ],
                0.14,
            );
            // A ram along the inside of the housing.
            super::super::plating::piston(
                b,
                v3(-0.3, y + 1.1, z - 0.25),
                v3(2.8, y + 1.1, z - 0.3),
                0.19,
            );
        }
        b.with_recoil(|b| {
            dark_plate(b);
            let sides = b.sides(10);
            b.cylinder_between(v3(BREECH - 0.3, y, z), v3(8.35, y, z), 0.7, 0.62, sides);
            metal(b);
            for x in [5.3f32, 6.4, 7.5] {
                b.cylinder_between(v3(x, y, z), v3(x + 0.24, y, z), 0.76, 0.76, sides);
            }
            // The muzzle block: a dark block with the side ports cut as dark slots.
            dark_plate(b);
            b.chamfered_box(v3(8.85, y, z), v3(1.1, 1.45, 1.35), 0.3);
            if b.fine() {
                seam(b);
                for x in [8.6f32, 9.05] {
                    b.block(
                        v3(x - 0.13, y - 0.77, z - 0.36),
                        v3(x + 0.13, y + 0.77, z + 0.36),
                    );
                }
            }
            b.paint(GLOW_LASER);
            b.cylinder_between(
                v3(9.35, y, z),
                MUZZLE + Vec3::X * 0.02,
                0.4,
                0.36,
                b.sides(10),
            );
        });
    });
}

/// Where the claw's fingers root round the palm, their knuckles and tips: (angle round
/// the forearm's axis in degrees from up, root radius, knuckle radius, tip radius).
const FINGERS: [(f32, f32, f32, f32); 3] = [
    (0.0, 0.65, 1.0, 0.36),
    (125.0, 0.65, 0.95, 0.36),
    (235.0, 0.65, 0.95, 0.36),
];

/// The left forearm: the claw. A housing, a bronze wrist, the palm with the violet
/// nanite emitter in it (where the build beam leaves), and three plated talons round it
/// that open while it builds (`rig::WORK_BREATHE`). The suites' kit is on it.
pub(super) fn claw(b: &mut MeshBuilder) {
    let (y, z) = (EMITTER.y, EMITTER.z);
    b.with_limb(rig::ARM_TOOL, |b| {
        housing(b, y, 2.9, 0.9, 1.0);
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(v3(2.9, y, z), v3(3.45, y, z), 0.75, 0.75, sides);
        seam(b);
        b.cylinder_between(v3(3.45, y, z), v3(4.55, y, z), 0.85, 0.66, b.sides(8));
        b.paint(GLOW_VIOLET);
        b.cylinder_between(v3(4.5, y, z), EMITTER, 0.4, 0.26, sides);
        b.with_work(rig::WORK_BREATHE, |b| {
            for &(deg, root, knuckle, tip) in &FINGERS {
                let a = deg.to_radians();
                let at = |x: f32, r: f32| v3(x, y - a.sin() * r, z + a.cos() * r);
                let (p0, p1, p2) = (at(4.1, root), at(5.65, knuckle), at(7.1, tip));
                dark_plate(b);
                b.beam(p0, p1, Vec2::new(0.5, 0.42), Vec2::new(0.4, 0.34));
                b.beam(p1, p2, Vec2::new(0.38, 0.32), Vec2::new(0.07, 0.07));
                if b.fine() {
                    metal(b);
                    let hinge = (p1 - v3(p1.x, y, z)).normalize().cross(Vec3::X) * 0.24;
                    b.cylinder_between(p1 - hinge, p1 + hinge, 0.24, 0.24, 6);
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
                    v3(0.4, y + dy, z + 1.36),
                    v3(1.9, y + dy, z + 1.4),
                    0.22,
                    0.19,
                    b.sides(8),
                );
                b.paint(GLOW_VIOLET);
                b.cylinder_between(
                    v3(1.9, y + dy, z + 1.4),
                    v3(2.25, y + dy, z + 1.4),
                    0.17,
                    0.09,
                    b.sides(8),
                );
            }
        });
        metal(b);
        for dz in [-0.4f32, 0.35] {
            b.cylinder_between(
                v3(-0.4, y + 1.45, z + dz),
                v3(2.1, y + 1.45, z + dz),
                0.27,
                0.27,
                b.sides(8),
            );
        }
        if b.fine() {
            cable(
                b,
                &[
                    v3(2.1, y + 1.45, z),
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
            let at = |r: f32| v3(3.18, y - a.sin() * r, z + a.cos() * r);
            b.cylinder_between(at(0.72), at(1.02), 0.14, 0.09, 6);
        }
        joint(b, v3(-0.9, y + 0.2, z + 1.62), Vec3::X * 0.7, 0.48);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            v3(-0.25, y + 0.2, z + 1.62),
            v3(-0.1, y + 0.2, z + 1.62),
            0.5,
            0.5,
            b.sides(10),
        );
    });
}
