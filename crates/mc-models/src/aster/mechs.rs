//! Aster walkers: the Paladin assault bot and the Lancer light bot (the commander is
//! `commander`).
//!
//! Legs and feet are `LOCOMOTION`, the pelvis is `HULL`, and everything above
//! the waist ring (torso, arms, head) is the `TURRET`, twisting about the
//! model's z axis so arm-mounted muzzles track the sim's muzzle offsets.

use glam::{Vec2, Vec3};

use super::bolt_rifle::{bolt_rifle, rifle_shoulder};
use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

/// One leg on the +y side, hip first. Each segment is a tapering armoured
/// beam (`girth`: start and end cross-section) between two joints.
struct Leg<'a> {
    joints: &'a [Vec3],
    girth: &'a [(Vec2, Vec2)],
    /// Foot outline relative to the last joint: (x rear, x front, height, width).
    foot: (f32, f32, f32, f32),
    /// Segments from this index on are bare frame rather than plating.
    bare_from: usize,
}

fn leg(b: &mut MeshBuilder, leg: &Leg) {
    let (hip, ankle) = (leg.joints[0], leg.joints[leg.joints.len() - 1]);
    let (rear, front, height, width) = leg.foot;
    b.with_part(part::LOCOMOTION, |b| {
        if b.coarse() {
            b.paint(PLATING);
            let top = leg.girth[0].0;
            b.frustum_open(
                v3(ankle.x + (rear + front) * 0.5, ankle.y, 0.0),
                v2(front - rear, width),
                v2(top.y, top.x),
                hip.z,
                v2(hip.x - ankle.x - (rear + front) * 0.5, hip.y - ankle.y),
            );
            return;
        }
        // Two bones for the shader's IK: the thigh, and everything from the knee
        // to the ankle as one rigid shank, however many joints it is drawn with.
        for (i, pair) in leg.joints.windows(2).enumerate() {
            b.paint(if i >= leg.bare_from { METAL } else { PLATING });
            b.with_limb(if i == 0 { rig::THIGH } else { rig::SHIN }, |b| {
                b.beam(pair[0], pair[1], leg.girth[i].0, leg.girth[i].1)
            });
        }
        b.with_limb(rig::SHIN, |b| {
            // Joint drums across the limb.
            b.paint(ACCENT);
            let drums = if b.fine() { leg.joints.len() - 1 } else { 1 };
            for (i, &joint) in leg.joints.iter().enumerate().skip(1).take(drums) {
                let end = leg.girth[i - 1].1;
                let half = Vec3::Y * (end.x * 0.56);
                b.cylinder_between(
                    joint - half,
                    joint + half,
                    end.y * 0.62,
                    end.y * 0.62,
                    if b.fine() { 8 } else { 4 },
                );
            }
            if b.fine() {
                // Knee guard on the first joint below the hip.
                b.paint(PLATING);
                let knee = leg.joints[1];
                let size = leg.girth[0].1;
                b.frustum(
                    knee + v3(size.y * 0.35, 0.0, -size.y * 0.5),
                    v2(size.y * 0.5, size.x * 1.05),
                    v2(size.y * 0.3, size.x * 0.7),
                    size.y * 1.1,
                    v2(size.y * 0.2, 0.0),
                );
            }
        });
        // Foot: dark sole, white toe cap.
        b.with_limb(rig::FOOT, |b| {
            let (x0, x1, h) = (ankle.x + rear, ankle.x + front, height);
            b.paint(TREAD);
            b.extrude_y(
                &[
                    [x0, 0.0],
                    [x1, 0.0],
                    [x1, 0.4 * h],
                    [x1 - 0.3 * (x1 - x0), h],
                    [x0 + 0.15 * (x1 - x0), h],
                    [x0, 0.55 * h],
                ],
                ankle.y - width * 0.5,
                ankle.y + width * 0.5,
            );
            if b.fine() {
                b.paint(PLATING);
                on_slope(b, [x1, 0.4 * h], [x1 - 0.3 * (x1 - x0), h], 0.5, |b| {
                    b.plate(
                        v3(0.0, ankle.y, 0.0),
                        v2(0.3 * (x1 - x0), width * 0.8),
                        0.1 * h,
                        0.04 * h,
                    )
                });
            }
        });
    });
}

/// Both legs of a walker, and the rig the shader walks them by: `stride` metres
/// of ground to a full cycle, feet lifted `lift` on the way forward.
fn walker_legs(b: &mut MeshBuilder, stance: &Leg, stride: f32, lift: f32) {
    b.set_legs(
        stance.joints[0],
        stance.joints[1],
        stance.joints[stance.joints.len() - 1],
        stride,
        0.6,
        lift,
    );
    let (rear, front, _, width) = stance.foot;
    b.set_foot(rear, front, width);
    b.mirror_y(|b| leg(b, stance));
}

// ---- Paladin: assault bot ----------------------------------------------------

/// The shoulder axle each arm pitches about (its y is the arm's own, mirrored): the unit
/// file's `pivot`, (-0.8, 0, 18.4) on the 24 m Paladin, over two.
const PALADIN_SHOULDER: Vec3 = Vec3::new(-0.4, 0.0, 9.2);
/// The left bolt rifle's breech and muzzle: the unit file's muzzles, (10, ±7.2, 18), over two.
const PALADIN_BREECH: Vec3 = Vec3::new(0.9, 3.6, 9.0);
const PALADIN_MUZZLE: Vec3 = Vec3::new(5.0, 3.6, 9.0);

pub(super) fn assault_bot(b: &mut MeshBuilder, _tech: u8) {
    let stance = Leg {
        joints: &[
            v3(-0.2, 2.3, 6.5),
            v3(1.5, 2.5, 4.3),
            v3(-1.3, 2.5, 2.3),
            v3(0.1, 2.5, 0.9),
        ],
        girth: &[
            (v2(1.5, 2.0), v2(1.2, 1.4)),
            (v2(1.1, 1.3), v2(0.9, 1.0)),
            (v2(0.7, 0.8), v2(0.9, 1.0)),
        ],
        foot: (-1.5, 2.2, 0.9, 2.1),
        bare_from: 2,
    };
    // Twice the hull, twice the ground: 16 m to a cycle, feet lifted clear of a
    // shuffle. The shader plants each foot, so the stride stays a power of two.
    walker_legs(b, &stance, 16.0, 1.4);
    // It walks the seabed: a sealed torpedo tube strapped to each shin, riding the shin bone.
    b.mirror_y(shin_tube);

    b.set_turret_pivot(v3(0.0, 0.0, 7.2));
    // Each shoulder pitches about its axle to point its gun up or down at what it aims at,
    // and each barrel kicks back on its own shots (`mirror::UNIT_TWIN_*`).
    b.set_arm_pivot(PALADIN_SHOULDER);
    b.set_recoil(PALADIN_BREECH, PALADIN_MUZZLE, 0.4);
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.2, 0.0, 6.3),
                v2(4.6, 5.0),
                v2(3.4, 5.2),
                4.2,
                v2(-0.6, 0.0),
            );
            team_panel(b, v3(-1.4, 0.0, 10.5), v2(1.4, 4.8));
            b.mirror_y(|b| {
                // Arm and rail in one tapered bar out to the muzzle.
                b.with_limb(rig::ARM_GUN, |b| {
                    b.paint(PLATING).beam(
                        v3(-2.6, 3.6, 9.0),
                        PALADIN_MUZZLE,
                        v2(1.9, 1.7),
                        v2(0.5, 0.5),
                    )
                });
            });
        });
        return;
    }

    b.paint(ACCENT);
    b.chamfered_box(v3(-0.2, 0.0, 6.4), v3(2.6, 3.4, 1.5), 0.6);
    b.cylinder_between(
        v3(-0.2, -3.0, 6.5),
        v3(-0.2, 3.0, 6.5),
        0.85,
        0.85,
        b.sides(8),
    );

    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(-0.2, 0.0, 7.0), 8, 1.7, 1.7, 0.5);
        // Low, broad hull-torso with a sunk sensor band instead of a head.
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[
                [-2.6, 7.4],
                [1.6, 7.4],
                [3.0, 8.4],
                [2.6, 9.6],
                [0.8, 10.6],
                [-2.4, 10.4],
                [-3.2, 9.0],
            ],
            2.6,
            0.7,
        );
        on_slope(b, [3.0, 8.4], [2.6, 9.6], 0.5, |b| {
            // The sensor band: a dark slit in a black frame, nothing lit.
            b.paint(ACCENT);
            b.plate(Vec3::ZERO, v2(0.8, 3.0), 0.1, 0.04);
            glow_strip(b, v3(0.0, 0.0, 0.1), v2(0.3, 2.4), TREAD);
        });
        on_slope(b, [0.8, 10.6], [-2.4, 10.4], 0.35, |b| {
            team_panel(b, Vec3::ZERO, v2(1.5, 3.0))
        });

        // Arms: a bolt rifle carried in each shoulder, the shoulder turning on its axle.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            let axle = PALADIN_SHOULDER.with_y(2.3);
            b.cylinder_between(axle, axle.with_y(3.0), 0.8, 0.8, b.sides(8));
            b.with_limb(rig::ARM_GUN, |b| {
                rifle_shoulder(b, PALADIN_BREECH, 0.44);
                bolt_rifle(b, PALADIN_BREECH, PALADIN_MUZZLE, 0.44);
            });
        });

        // Heat-sink fins on the back.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[-3.0, 9.6], [-1.6, 10.45], [-2.2, 11.6], [-3.9, 10.6]],
                0.7,
                1.0,
            );
            b.extrude_y(
                &[[-3.0, 9.6], [-1.6, 10.45], [-2.0, 11.3], [-3.7, 10.4]],
                1.6,
                1.9,
            );
        });
        if b.fine() {
            // Bare metal sink plates between the fins.
            b.mirror_y(|b| {
                b.paint(METAL);
                b.extrude_y(
                    &[[-2.9, 9.8], [-1.9, 10.4], [-2.3, 11.1], [-3.5, 10.4]],
                    1.0,
                    1.6,
                );
            });
            on_slope(b, [2.6, 9.6], [0.8, 10.6], 0.5, |b| {
                b.paint(GLASS);
                b.plate(Vec3::ZERO, v2(1.1, 2.2), 0.08, 0.04);
            });
            // Intake grilles low on the chest.
            on_slope(b, [1.6, 7.4], [3.0, 8.4], 0.5, |b| {
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.0, 0.0), v2(0.9, 0.6), TREAD))
            });
            // A whip antenna, its tip unlit.
            let (whip, tip) = (v3(-2.6, 0.0, 10.4), v3(-2.8, 0.0, 12.4));
            b.paint(ACCENT);
            b.cylinder_between(whip, tip, 0.074, 0.032, 4);
            b.paint(METAL);
            b.cuboid(tip, Vec3::splat(0.12));
        }
    });

    if b.fine() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(METAL);
                b.beam(
                    v3(1.0, 2.38, 5.75),
                    v3(1.9, 2.48, 4.6),
                    v2(0.5, 0.08),
                    v2(0.4, 0.08),
                );
                b.paint(TEAM);
                b.beam(
                    v3(0.15, 2.3, 6.95),
                    v3(1.0, 2.4, 5.85),
                    v2(1.1, 0.1),
                    v2(1.0, 0.1),
                );
            });
        });
    }
}

/// Where each shin tube's mouth is at rest (left leg; the right is its mirror): the unit
/// file's torpedo muzzles, (2.2, ±4.6, 3.0) on the 24 m Paladin, over two.
const PALADIN_TUBE_MOUTH: Vec3 = Vec3::new(1.1, 2.3, 1.5);

/// A short armoured pod strapped to the front of the lower shin, its mouth facing
/// forward: dark bore in a light cap, a black strap behind the cap.
/// It is part of the shin, so it strides with the leg.
fn shin_tube(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    let m = PALADIN_TUBE_MOUTH;
    let at = |x: f32| v3(x, m.y, m.z);
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::SHIN, |b| {
            let sides = b.sides(8);
            b.paint(PLATING);
            b.cylinder_between(at(-0.45), at(m.x - 0.16), 0.27, 0.29, sides);
            // Straps round the pod and back onto the shin.
            b.paint(ACCENT);
            for x in [-0.2, 0.42] {
                b.cylinder_between(at(x), at(x + 0.1), 0.31, 0.31, sides);
            }
            b.block(
                v3(-0.75, m.y - 0.22, m.z - 0.34),
                v3(-0.25, m.y + 0.22, m.z + 0.12),
            );
            // White cap round the mouth, the bore dark inside it.
            b.paint(PLATING);
            b.cylinder_between(at(m.x - 0.18), at(m.x - 0.01), 0.33, 0.31, sides);
            b.paint(ACCENT);
            b.cylinder_between(at(m.x - 0.05), at(m.x), 0.17, 0.17, sides);
            if b.fine() {
                b.paint(ACCENT);
                b.cylinder_between(at(m.x - 0.26), at(m.x - 0.2), 0.3, 0.3, sides);
            }
        });
    });
}

// ---- Lancer: light assault bot ---------------------------------------------

pub(super) fn bot_light(b: &mut MeshBuilder, _tech: u8) {
    let stance = Leg {
        joints: &[
            v3(-0.1, 1.1, 2.95),
            v3(0.8, 1.15, 1.95),
            v3(-0.6, 1.15, 1.05),
            v3(0.1, 1.15, 0.4),
        ],
        girth: &[
            (v2(0.6, 0.8), v2(0.48, 0.55)),
            (v2(0.42, 0.48), v2(0.36, 0.4)),
            (v2(0.3, 0.34), v2(0.38, 0.42)),
        ],
        foot: (-0.7, 1.15, 0.42, 0.85),
        bare_from: 2,
    };
    walker_legs(b, &stance, 4.0, 0.32);

    b.set_turret_pivot(v3(0.0, 0.0, 3.2));
    let (breech, muzzle) = (v3(0.3, 0.0, 4.2), v3(1.4, 0.0, 4.2));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.2, 0.0, 2.9),
                v2(1.7, 2.6),
                v2(1.2, 2.2),
                1.85,
                v2(-0.1, 0.0),
            );
            team_panel(b, v3(-0.5, 0.0, 4.75), v2(0.6, 1.8));
            b.paint(METAL);
            b.beam(breech, muzzle, v2(0.7, 0.26), v2(0.6, 0.2));
        });
        return;
    }

    b.paint(ACCENT);
    if b.fine() {
        b.chamfered_box(v3(-0.1, 0.0, 2.95), v3(0.9, 1.5, 0.6), 0.2);
        b.cylinder_between(v3(-0.1, -1.35, 2.95), v3(-0.1, 1.35, 2.95), 0.3, 0.3, 6);
    } else {
        b.cuboid(v3(-0.1, 0.0, 2.95), v3(0.9, 2.2, 0.6));
    }

    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(-0.1, 0.0, 3.2), b.sides(6), 0.55, 0.55, 0.2);
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[
                [-1.1, 3.35],
                [0.4, 3.35],
                [0.78, 3.75],
                [0.72, 4.45],
                [0.1, 4.8],
                [-0.9, 4.7],
                [-1.3, 4.0],
            ],
            0.8,
            0.24,
        );
        // Twin autocannon through the brow.
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(
                breech + Vec3::Y * 0.26,
                muzzle + Vec3::Y * 0.26,
                0.11,
                0.09,
                b.sides(6),
            );
            if b.fine() {
                b.paint(GLOW_ORANGE);
                b.cylinder_between(
                    muzzle + v3(-0.04, 0.26, 0.0),
                    muzzle + v3(0.02, 0.26, 0.0),
                    0.065,
                    0.065,
                    6,
                );
            }
        });
        b.paint(if b.fine() { ACCENT } else { GLOW_ORANGE });
        b.block(v3(0.55, -0.5, 4.0), v3(0.95, 0.5, 4.4));
        // Shoulder sensor pods give the bot its hunched silhouette.
        b.mirror_y(|b| {
            b.paint(PLATING);
            if b.fine() {
                b.at(v3(-0.25, 1.22, 0.0), |b| {
                    b.loft_z(
                        &chamfered_rect(v2(0.7, 0.42), 0.18),
                        &[
                            Section::new(3.8, 0.85),
                            Section::new(4.35, 1.0),
                            Section::scaled(4.65, 0.7, 0.6),
                        ],
                    )
                });
            } else {
                b.frustum(
                    v3(-0.25, 1.22, 3.8),
                    v2(1.3, 0.8),
                    v2(0.9, 0.5),
                    0.85,
                    v2(0.0, 0.0),
                );
            }
        });
        on_slope(b, [0.1, 4.8], [-0.9, 4.7], 0.45, |b| {
            team_panel(b, Vec3::ZERO, v2(0.6, 1.1))
        });
        if b.fine() {
            b.paint(ACCENT);
            b.mirror_y(|b| b.block(v3(1.15, 0.1, 4.05), v3(1.32, 0.42, 4.35)));
            on_slope(b, [0.78, 3.75], [0.72, 4.45], 0.3, |b| {
                glow_strip(b, Vec3::ZERO, v2(0.12, 0.9), GLOW)
            });
            b.mirror_y(|b| {
                b.paint(GLASS);
                b.block(v3(0.45, 1.0, 4.05), v3(0.5, 1.45, 4.25));
                glow_strip(b, v3(-0.25, 1.22, 4.65), v2(0.6, 0.12), GLOW);
            });
            // Ammunition drum and exhaust on the back.
            b.paint(ACCENT);
            b.cylinder_between(v3(-1.25, -0.55, 4.05), v3(-1.25, 0.55, 4.05), 0.36, 0.36, 6);
            b.paint(GLOW_ORANGE);
            b.block(v3(-1.64, -0.25, 3.98), v3(-1.6, 0.25, 4.1));
            antenna(b, v3(-0.8, -0.5, 4.65), 0.8, 0.2);
        }
    });
}
