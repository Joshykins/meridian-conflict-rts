//! The commander's forearms, each pitching about its elbow: the plasmeric cannon on the
//! right (`rig::ARM_GUN`) and the clawed hand that builds on the left (`rig::ARM_TOOL`),
//! with the engineering suites' kit.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::rig;

use super::super::kit::{cable, hide, metal, under_hide, v3};
use super::super::machine::{joint, plate, side_plate};
use super::{ELBOW, EMITTER, LANCE_TIP, MUZZLE};

/// Where the barrel leaves the housing; it kicks back this far when it fires.
const BREECH: f32 = 3.2;
const KICK: f32 = 0.35;

/// A forearm's housing, `y` its axis: a dark block from the elbow forward to `front`, a
/// plate on top drawn back past the elbow into a point, and a plate down the outside
/// (`out`: +1 on the left arm, -1 on the right).
fn housing(b: &mut MeshBuilder, y: f32, front: f32, half: f32, out: f32) {
    let z = ELBOW.z;
    metal(b);
    b.chamfered_box(
        v3((front - 0.2) * 0.5, y, z),
        v3(front + 0.2, half * 2.0, 1.55),
        0.35,
    );
    hide(b);
    plate(
        b,
        &[
            v3(front + 0.1, y - half - 0.05, z + 0.75),
            v3(front + 0.1, y + half + 0.05, z + 0.75),
            v3(-0.2, y + half + 0.1, z + 0.8),
            v3(-1.6, y + out * 0.3, z + 1.05),
            v3(-0.2, y - half - 0.1, z + 0.8),
        ],
        Vec3::Z * 0.2,
    );
    let side = y + out * (half + 0.02);
    side_plate(
        b,
        &[
            [front, z + 0.7],
            [front + 0.2, z - 0.4],
            [front - 1.0, z - 0.85],
            [-0.3, z - 0.8],
            [-1.45, z + 0.1],
            [-0.2, z + 0.72],
        ],
        side.min(side + out * 0.22),
        side.max(side + out * 0.22),
    );
}

/// The right forearm: the cannon. A heavy housing, bronze containment rings round the
/// barrel's root, the barrel itself with rings along it and a muzzle block ported at the
/// sides, its bore lit red. The barrel and muzzle block kick back when it fires.
pub(super) fn cannon(b: &mut MeshBuilder) {
    let (y, z) = (MUZZLE.y, MUZZLE.z);
    b.set_recoil(v3(BREECH, y, z), MUZZLE, KICK);
    b.with_limb(rig::ARM_GUN, |b| {
        housing(b, y, BREECH, 0.9, -1.0);
        metal(b);
        let sides = b.sides(12);
        for (i, x) in [BREECH, BREECH + 0.45, BREECH + 0.9]
            .into_iter()
            .enumerate()
        {
            let r = 0.78 - 0.08 * i as f32;
            b.cylinder_between(v3(x, y, z), v3(x + 0.28, y, z), r, r, sides);
        }
        if b.fine() {
            // Heat fins across the housing's top, behind the top plate's front edge.
            hide(b);
            for x in [1.0f32, 1.5, 2.0, 2.5] {
                b.block(
                    v3(x - 0.06, y - 0.7, z + 0.95),
                    v3(x + 0.06, y + 0.7, z + 1.28),
                );
            }
            // Feed lines from the housing's underside into the rings.
            cable(
                b,
                &[
                    v3(0.6, y, z - 0.85),
                    v3(2.4, y, z - 0.95),
                    v3(BREECH + 0.3, y, z - 0.6),
                ],
                0.12,
            );
            // A ram along the inside of the housing.
            super::super::machine::piston(
                b,
                v3(-0.3, y + 0.85, z - 0.2),
                v3(2.6, y + 0.85, z - 0.25),
                0.16,
            );
        }
        b.with_recoil(|b| {
            hide(b);
            let sides = b.sides(10);
            b.cylinder_between(v3(BREECH - 0.3, y, z), v3(7.9, y, z), 0.56, 0.5, sides);
            metal(b);
            for x in [5.0f32, 6.1, 7.2] {
                b.cylinder_between(v3(x, y, z), v3(x + 0.2, y, z), 0.62, 0.62, sides);
            }
            // The muzzle block: a dark block with the side ports cut as dark slots.
            hide(b);
            b.chamfered_box(v3(8.4, y, z), v3(1.0, 1.2, 1.1), 0.25);
            if b.fine() {
                under_hide(b);
                for x in [8.15f32, 8.55] {
                    b.block(
                        v3(x - 0.12, y - 0.64, z - 0.3),
                        v3(x + 0.12, y + 0.64, z + 0.3),
                    );
                }
            }
            b.paint(GLOW_LASER);
            b.cylinder_between(
                v3(8.85, y, z),
                MUZZLE + Vec3::X * 0.02,
                0.32,
                0.3,
                b.sides(10),
            );
        });
    });
}

/// Where the claw's fingers root round the palm, their knuckles and tips: (angle round
/// the forearm's axis in degrees from up, root radius, knuckle radius, tip radius).
const FINGERS: [(f32, f32, f32, f32); 3] = [
    (0.0, 0.55, 0.85, 0.3),
    (125.0, 0.55, 0.8, 0.3),
    (235.0, 0.55, 0.8, 0.3),
];

/// The left forearm: the claw. A housing, a bronze wrist, the palm with the violet
/// nanite emitter in it (where the build beam leaves), and three plated talons round it
/// that open while it builds (`rig::WORK_BREATHE`). The suites' kit is on it.
pub(super) fn claw(b: &mut MeshBuilder) {
    let (y, z) = (EMITTER.y, EMITTER.z);
    b.with_limb(rig::ARM_TOOL, |b| {
        housing(b, y, 2.7, 0.7, 1.0);
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(v3(2.7, y, z), v3(3.2, y, z), 0.6, 0.6, sides);
        under_hide(b);
        b.cylinder_between(v3(3.2, y, z), v3(4.25, y, z), 0.7, 0.55, b.sides(8));
        b.paint(GLOW_VIOLET);
        b.cylinder_between(v3(4.2, y, z), EMITTER, 0.34, 0.22, sides);
        b.with_work(rig::WORK_BREATHE, |b| {
            for &(deg, root, knuckle, tip) in &FINGERS {
                let a = deg.to_radians();
                let at = |x: f32, r: f32| v3(x, y - a.sin() * r, z + a.cos() * r);
                let (p0, p1, p2) = (at(3.9, root), at(5.3, knuckle), at(6.6, tip));
                hide(b);
                b.beam(p0, p1, Vec2::new(0.4, 0.34), Vec2::new(0.32, 0.28));
                b.beam(p1, p2, Vec2::new(0.3, 0.26), Vec2::new(0.05, 0.05));
                if b.fine() {
                    metal(b);
                    let hinge = (p1 - v3(p1.x, y, z)).normalize().cross(Vec3::X) * 0.2;
                    b.cylinder_between(p1 - hinge, p1 + hinge, 0.2, 0.2, 6);
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
            for dy in [-0.35f32, 0.35] {
                metal(b);
                b.cylinder_between(
                    v3(0.4, y + dy, z + 1.05),
                    v3(1.9, y + dy, z + 1.1),
                    0.2,
                    0.17,
                    b.sides(8),
                );
                b.paint(GLOW_VIOLET);
                b.cylinder_between(
                    v3(1.9, y + dy, z + 1.1),
                    v3(2.2, y + dy, z + 1.1),
                    0.15,
                    0.08,
                    b.sides(8),
                );
            }
        });
        metal(b);
        for dz in [-0.35f32, 0.3] {
            b.cylinder_between(
                v3(-0.4, y + 1.05, z + dz),
                v3(2.1, y + 1.05, z + dz),
                0.27,
                0.27,
                b.sides(8),
            );
        }
        if b.fine() {
            cable(
                b,
                &[
                    v3(2.1, y + 1.05, z),
                    v3(3.0, y + 0.75, z),
                    v3(3.7, y + 0.55, z),
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
                v3(3.6, y, z),
                v3(LANCE_TIP - 0.3, y, z),
                0.16,
                0.1,
                b.sides(8),
            );
            b.paint(GLOW_VIOLET);
            b.cylinder_between(
                v3(LANCE_TIP - 0.3, y, z),
                v3(LANCE_TIP, y, z),
                0.1,
                0.03,
                b.sides(8),
            );
        });
        b.paint(GLOW_VIOLET);
        for deg in [60.0f32, 180.0, 300.0] {
            let a = deg.to_radians();
            let at = |r: f32| v3(2.95, y - a.sin() * r, z + a.cos() * r);
            b.cylinder_between(at(0.55), at(0.82), 0.12, 0.08, 6);
        }
        joint(b, v3(-0.9, y + 0.2, z + 1.35), Vec3::X * 0.7, 0.45);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            v3(-0.25, y + 0.2, z + 1.35),
            v3(-0.1, y + 0.2, z + 1.35),
            0.47,
            0.47,
            b.sides(10),
        );
    });
}
