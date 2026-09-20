//! Aster walkers: the commander, the Paladin assault bot and the Lancer light bot.
//!
//! Legs and feet are `LOCOMOTION`, the pelvis is `HULL`, and everything above
//! the waist ring (torso, arms, head) is the `TURRET`, twisting about the
//! model's z axis so arm-mounted muzzles track the sim's muzzle offsets.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, rig};

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
    b.mirror_y(|b| leg(b, stance));
}

// ---- ARC Commander -----------------------------------------------------------
//
// A dark graphite frame carrying white armour: the plating colour is kept for
// what is armour (pauldrons, chest plates, greaves, forearm shrouds), so it
// reads as a machine in armour rather than a white statue. The right arm is
// the rail rifle, lit blue; the left is the construction projector, and
// everything that builds is marked in amber. Each engineering suite adds to
// the build arm and the fabricator pack; the next suite's pieces are emitted
// as upgrade pieces, which the shader raises during the refit.

/// Where the rifle ends and where the build beam leaves, as the unit file has them.
const COMMANDER_MUZZLE: Vec3 = Vec3::new(6.2, -3.0, 9.7);
const COMMANDER_EMITTER: Vec3 = Vec3::new(4.8, 3.0, 9.7);

/// Emits `f` if engineering suite `tier` is fitted at `tech`, as an upgrade
/// piece going up `at` of the way through the refit if it is the next one (full
/// detail only: a refit is watched from close by), and not at all beyond that.
fn suite(b: &mut MeshBuilder, tech: u8, tier: u8, at: f32, f: impl FnOnce(&mut MeshBuilder)) {
    if tier <= tech {
        f(b);
    } else if tier == tech + 1 && b.fine() {
        b.upgrade(at, f);
    }
}

fn commander_leg(b: &mut MeshBuilder, tech: u8, hip: Vec3, knee: Vec3, ankle: Vec3) {
    b.with_part(part::LOCOMOTION, |b| {
        if b.coarse() {
            // From this far the legs are their white greaves and cuisses.
            b.paint(PLATING);
            b.frustum_open(
                v3(ankle.x + 0.5, ankle.y, 0.0),
                v2(3.2, 1.75),
                v2(1.4, 1.5),
                hip.z,
                v2(hip.x - ankle.x - 0.5, hip.y - ankle.y),
            );
            return;
        }
        b.with_limb(rig::THIGH, |b| {
            b.paint(ACCENT);
            b.beam(hip, knee, v2(1.2, 1.4), v2(1.0, 1.15));
            b.cylinder_between(
                hip - Vec3::Y * 0.82,
                hip + Vec3::Y * 0.82,
                0.85,
                0.85,
                b.sides(8),
            );
            // Cuisse: a white plate over the front and outside of the thigh.
            b.paint(PLATING);
            b.beam(
                hip + v3(0.52, 0.15, -0.5),
                knee + v3(0.46, 0.15, 0.8),
                v2(1.55, 0.72),
                v2(1.25, 0.6),
            );
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(
                    hip + v3(-0.7, 0.0, -0.4),
                    knee + v3(-0.64, 0.0, 0.6),
                    0.18,
                    0.15,
                    6,
                );
            }
        });
        b.with_limb(rig::SHIN, |b| {
            b.paint(METAL);
            b.cylinder_between(
                knee - Vec3::Y * 0.75,
                knee + Vec3::Y * 0.75,
                0.72,
                0.72,
                b.sides(8),
            );
            b.paint(ACCENT);
            b.beam(knee, ankle, v2(0.95, 1.1), v2(0.82, 0.92));
            // Greave, with the knee cap standing proud of it.
            b.paint(PLATING);
            b.beam(
                knee + v3(0.34, 0.0, -0.7),
                ankle + v3(0.5, 0.0, 0.55),
                v2(1.5, 1.1),
                v2(1.18, 0.82),
            );
            b.paint(PLATING_DARK);
            b.frustum(
                knee + v3(0.7, 0.0, -0.6),
                v2(0.68, 1.3),
                v2(0.4, 0.88),
                1.35,
                v2(0.13, 0.0),
            );
            if b.fine() {
                b.paint(TEAM);
                b.beam(
                    knee + v3(0.24, 0.6, -1.3),
                    ankle + v3(0.4, 0.56, 1.2),
                    v2(0.62, 0.07),
                    v2(0.5, 0.07),
                );
                b.paint(METAL);
                b.cylinder_between(
                    knee + v3(-0.68, 0.0, -0.5),
                    ankle + v3(-0.54, 0.0, 0.5),
                    0.16,
                    0.13,
                    6,
                );
            }
            suite(b, tech, 3, 0.72, |b| {
                // Suite III: feed conduits down the greaves.
                b.paint(GLOW_AMBER);
                b.beam(
                    knee + v3(0.9, 0.0, -1.1),
                    ankle + v3(0.92, 0.0, 0.9),
                    v2(0.28, 0.07),
                    v2(0.22, 0.07),
                );
            });
        });
        b.with_limb(rig::FOOT, |b| {
            b.paint(METAL);
            b.cylinder_between(
                ankle - Vec3::Y * 0.65,
                ankle + Vec3::Y * 0.65,
                0.55,
                0.55,
                b.sides(8),
            );
            let (x0, x1, h) = (ankle.x - 1.2, ankle.x + 2.4, 1.05);
            b.paint(TREAD);
            b.extrude_y(
                &[
                    [x0, 0.0],
                    [x1, 0.0],
                    [x1, 0.35 * h],
                    [x1 - 1.1, h],
                    [x0 + 0.6, h],
                    [x0, 0.5 * h],
                ],
                ankle.y - 0.95,
                ankle.y + 0.95,
            );
            if b.fine() {
                b.paint(PLATING_DARK);
                on_slope(b, [x1, 0.35 * h], [x1 - 1.1, h], 0.5, |b| {
                    b.plate(v3(0.0, ankle.y, 0.0), v2(1.0, 1.6), 0.12, 0.05)
                });
                b.plate(v3(ankle.x + 0.2, ankle.y, h), v2(1.4, 1.55), 0.1, 0.04);
            }
        });
    });
}

pub fn commander(b: &mut MeshBuilder, tech: u8) {
    // Lean and long in the limb. It covers ground in long, high strides: a foot is planted
    // for under half the cycle, so there is a moment between steps with neither down.
    let (hip, knee, ankle) = (v3(-0.1, 1.75, 8.3), v3(1.5, 1.85, 5.0), v3(-0.5, 1.85, 1.3));
    b.set_legs(hip, knee, ankle, 16.0, 0.4, 1.9);
    // Sole as `commander_leg` draws it: 1.2 m behind the ankle, 2.4 m ahead, 1.9 m across.
    b.set_foot(-1.2, 2.4, 1.9);
    b.mirror_y(|b| commander_leg(b, tech, hip, knee, ankle));

    let (gun_y, tool_y, arm_z) = (COMMANDER_MUZZLE.y, COMMANDER_EMITTER.y, COMMANDER_MUZZLE.z);
    b.set_turret_pivot(v3(0.0, 0.0, 9.2));
    // The elbows: both forearms pitch about them to point at what they shoot or build.
    b.set_arm_pivot(v3(-0.3, tool_y, arm_z + 0.1));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING_DARK);
            b.frustum_open(
                v3(-0.1, 0.0, 9.0),
                v2(2.8, 3.6),
                v2(2.6, 4.2),
                4.4,
                v2(0.0, 0.0),
            );
            team_panel(b, v3(-0.3, 0.0, 13.45), v2(1.4, 3.8));
            b.paint(METAL);
            b.with_limb(rig::ARM_GUN, |b| {
                b.beam(
                    v3(-1.4, gun_y, arm_z),
                    COMMANDER_MUZZLE,
                    v2(1.4, 1.4),
                    v2(0.8, 0.65),
                )
            });
            b.paint(PLATING_DARK);
            b.with_limb(rig::ARM_TOOL, |b| {
                b.beam(
                    v3(-1.2, tool_y, arm_z),
                    COMMANDER_EMITTER,
                    v2(1.3, 1.3),
                    v2(0.8, 0.7),
                )
            });
        });
        return;
    }

    // Pelvis.
    b.paint(ACCENT);
    b.chamfered_box(v3(-0.1, 0.0, 8.3), v3(2.1, 2.2, 1.5), 0.5);
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.95, 0.0, 7.3),
        v2(0.45, 1.2),
        v2(0.65, 1.7),
        1.7,
        v2(0.18, 0.0),
    );
    b.frustum(
        v3(-1.15, 0.0, 7.5),
        v2(0.4, 1.3),
        v2(0.5, 1.8),
        1.4,
        v2(-0.12, 0.0),
    );

    b.with_part(part::TURRET, |b| {
        // Waist ring and a narrow abdomen under the chest.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 9.05), b.sides(8), 1.2, 1.2, 0.5);
        b.paint(METAL);
        b.chamfered_box(v3(-0.1, 0.0, 9.95), v3(1.9, 2.4, 0.9), 0.45);
        // Chest: a graphite wedge.
        b.paint(PLATING_DARK);
        b.extrude_y_chamfered(
            &[
                [-1.6, 10.3],
                [1.2, 10.3],
                [2.2, 11.3],
                [2.0, 12.5],
                [0.8, 13.4],
                [-1.7, 13.2],
                [-2.2, 11.7],
            ],
            2.0,
            0.55,
        );
        // White armour over it: breastplates either side of the cockpit, a belly plate.
        b.paint(PLATING);
        on_slope(b, [2.0, 12.5], [0.8, 13.4], 0.5, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.15, 0.0), v2(1.3, 1.05), 0.18, 0.06))
        });
        on_slope(b, [2.2, 11.3], [2.0, 12.5], 0.5, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.05, 0.0), v2(0.95, 1.2), 0.18, 0.06))
        });
        on_slope(b, [1.2, 10.3], [2.2, 11.3], 0.5, |b| {
            b.plate(Vec3::ZERO, v2(1.0, 2.0), 0.14, 0.05)
        });
        // Head, sunk between the shoulders.
        b.at(v3(0.4, 0.0, 0.0), |b| {
            b.paint(PLATING_DARK);
            b.loft_z(
                &turret_plan(1.7, 1.3),
                &[
                    Section::new(13.2, 0.85),
                    Section::new(13.8, 1.0),
                    Section::scaled(14.5, 0.5, 0.55).shifted(-0.2, 0.0),
                ],
            );
            b.paint(GLOW);
            b.block(v3(0.6, -0.33, 13.7), v3(0.85, 0.33, 13.88));
        });
        // Reactor pack.
        b.paint(ACCENT);
        b.block(v3(-3.0, -1.45, 10.4), v3(-1.8, 1.45, 13.0));
        b.paint(PLATING_DARK);
        b.extrude_y_chamfered(
            &[
                [-3.3, 10.7],
                [-2.95, 10.7],
                [-2.95, 13.5],
                [-1.9, 13.5],
                [-1.9, 13.1],
                [-3.3, 12.6],
            ],
            1.6,
            0.28,
        );

        // Shoulders: graphite pauldrons carrying the team colour, a white guard along the outer edge.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(0.0, 1.8, 12.2), v3(0.0, 2.6, 12.2), 0.7, 0.7, b.sides(8));
            b.paint(PLATING_DARK);
            b.at(v3(0.0, 2.95, 0.0), |b| {
                b.loft_z(
                    &chamfered_rect(v2(1.3, 0.9), 0.42),
                    &[
                        Section::new(11.95, 0.88),
                        Section::new(12.8, 1.0),
                        Section::scaled(13.35, 0.68, 0.58).shifted(0.0, -0.2),
                    ],
                );
            });
            team_panel(b, v3(0.0, 2.75, 13.35), v2(1.45, 0.85));
            b.paint(PLATING);
            b.extrude_y(
                &[[-1.2, 11.9], [1.2, 11.9], [1.1, 12.85], [-1.1, 12.85]],
                3.78,
                3.98,
            );
            // Long upper arms, hung from the shoulder to an elbow at the waist.
            b.paint(ACCENT);
            b.beam(
                v3(0.0, 3.0, 11.95),
                v3(-0.3, 3.0, 9.9),
                v2(0.8, 0.8),
                v2(0.7, 0.7),
            );
            b.paint(METAL);
            b.cylinder_between(
                v3(-0.3, 2.45, arm_z + 0.1),
                v3(-0.3, 3.55, arm_z + 0.1),
                0.5,
                0.5,
                b.sides(8),
            );
        });

        // Right forearm: the Meridian rail rifle, most of the arm's length.
        b.with_limb(rig::ARM_GUN, |b| {
            b.at(v3(0.0, gun_y, 0.0), |b| {
                b.paint(PLATING_DARK);
                b.extrude_y_chamfered(
                    &[
                        [-1.6, 8.95],
                        [1.6, 8.95],
                        [2.2, 9.35],
                        [2.2, 10.1],
                        [1.6, 10.5],
                        [-1.6, 10.5],
                    ],
                    0.8,
                    0.25,
                );
            });
            rail_gun(
                b,
                v3(1.8, gun_y, arm_z),
                COMMANDER_MUZZLE,
                v2(0.28, 0.6),
                0.28,
                Emitter::Blue,
            );
            if b.fine() {
                // Rifle furniture: energy cell with status lights, sight.
                b.paint(ACCENT);
                b.block(v3(-1.4, gun_y - 0.4, 10.5), v3(-0.1, gun_y + 0.4, 10.8));
                glow_strip(b, v3(-0.75, gun_y, 10.8), v2(1.0, 0.2), GLOW);
                b.paint(GLASS);
                b.block(v3(0.3, gun_y - 0.17, 10.5), v3(0.85, gun_y + 0.17, 10.75));
            }
        });

        b.with_limb(rig::ARM_TOOL, |b| {
            // Left forearm: the construction projector. What builds is marked in amber.
            let nozzle = COMMANDER_EMITTER - Vec3::X * 0.5;
            b.at(v3(0.0, tool_y, 0.0), |b| {
                b.paint(PLATING_DARK);
                b.extrude_y_chamfered(
                    &[
                        [-1.4, 9.0],
                        [1.4, 9.0],
                        [2.0, 9.35],
                        [2.0, 10.05],
                        [1.4, 10.4],
                        [-1.4, 10.4],
                    ],
                    0.75,
                    0.25,
                );
            });
            b.paint(METAL);
            b.cylinder_between(v3(1.8, tool_y, arm_z), nozzle, 0.45, 0.36, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(nozzle, COMMANDER_EMITTER, 0.3, 0.14, 6);
            b.cylinder_between(
                v3(2.3, tool_y, arm_z),
                v3(2.5, tool_y, arm_z),
                0.55,
                0.55,
                b.sides(8),
            );
            glow_strip(b, v3(-0.2, tool_y, 10.4), v2(2.0, 0.26), GLOW_AMBER);

            // Suite II: a focusing collar and long prongs on the projector, a capacitor
            // on the forearm, a fabricator tank on the pack, a beacon on the left shoulder.
            suite(b, tech, 2, 0.0, |b| {
                b.paint(METAL);
                b.cylinder_between(
                    v3(3.0, tool_y, arm_z),
                    v3(3.35, tool_y, arm_z),
                    0.68,
                    0.68,
                    b.sides(8),
                );
                for (dy, dz) in [(0.55, 0.32), (-0.55, 0.32), (0.0, -0.64)] {
                    b.paint(METAL);
                    b.beam(
                        v3(3.05, tool_y + dy, arm_z + dz),
                        COMMANDER_EMITTER + v3(-0.12, dy * 0.55, dz * 0.55),
                        v2(0.18, 0.18),
                        v2(0.11, 0.11),
                    );
                    b.paint(GLOW_AMBER);
                    b.beam(
                        COMMANDER_EMITTER + v3(-0.12, dy * 0.55, dz * 0.55),
                        COMMANDER_EMITTER + v3(0.0, dy * 0.5, dz * 0.5),
                        v2(0.12, 0.12),
                        v2(0.08, 0.08),
                    );
                }
            });
            suite(b, tech, 2, 0.2, |b| {
                b.paint(PLATING);
                b.block(v3(-1.3, tool_y + 0.75, 9.2), v3(0.8, tool_y + 1.15, 10.2));
                b.paint(GLOW_AMBER);
                b.block(v3(-1.0, tool_y + 1.15, 9.55), v3(0.5, tool_y + 1.2, 9.85));
            });
        });
        suite(b, tech, 2, 0.4, |b| {
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(-3.45, 0.85, 10.8),
                v3(-3.45, 0.85, 13.9),
                0.58,
                0.58,
                b.sides(8),
            );
            b.paint(GLOW_AMBER);
            for z in [11.4, 12.3, 13.2] {
                b.cylinder_between(
                    v3(-3.45, 0.85, z),
                    v3(-3.45, 0.85, z + 0.2),
                    0.64,
                    0.64,
                    b.sides(8),
                );
            }
            b.beam(
                v3(-2.8, 1.1, 13.6),
                v3(-0.5, 2.4, 13.3),
                v2(0.18, 0.1),
                v2(0.18, 0.1),
            );
        });
        suite(b, tech, 2, 0.65, |b| {
            b.paint(PLATING);
            b.block(v3(-0.75, 3.98, 12.0), v3(0.75, 4.3, 12.95));
            b.paint(METAL);
            b.cylinder_between(v3(-0.4, 2.6, 13.35), v3(-0.4, 2.6, 14.5), 0.1, 0.07, 5);
            b.paint(GLOW_AMBER);
            b.cylinder_between(v3(-0.4, 2.6, 14.5), v3(-0.4, 2.6, 14.78), 0.14, 0.1, 5);
            on_slope(b, [2.2, 11.3], [2.0, 12.5], 0.5, |b| {
                glow_strip(b, Vec3::ZERO, v2(0.6, 0.45), GLOW_AMBER)
            });
        });

        // Suite III: two more nozzles flanking the projector under a white shroud, a second
        // tank and a manifold across the pack, and fabricator masts behind the shoulders.
        b.with_limb(rig::ARM_TOOL, |b| {
            suite(b, tech, 3, 0.0, |b| {
                for dy in [-0.8, 0.8] {
                    b.paint(METAL);
                    b.cylinder_between(
                        v3(1.7, tool_y + dy, arm_z + 0.15),
                        v3(3.9, tool_y + dy, arm_z + 0.15),
                        0.28,
                        0.22,
                        b.sides(6),
                    );
                    b.paint(GLOW_AMBER);
                    b.cylinder_between(
                        v3(3.9, tool_y + dy, arm_z + 0.15),
                        v3(4.2, tool_y + dy, arm_z + 0.15),
                        0.2,
                        0.1,
                        6,
                    );
                }
                b.paint(PLATING);
                b.at(v3(0.0, tool_y, 0.0), |b| {
                    b.extrude_y_chamfered(
                        &[
                            [-0.9, 10.4],
                            [2.2, 10.4],
                            [2.8, 10.25],
                            [2.8, 10.6],
                            [2.0, 10.85],
                            [-0.9, 10.85],
                        ],
                        1.2,
                        0.25,
                    )
                });
                glow_strip(b, v3(0.6, tool_y, 10.85), v2(1.9, 0.3), GLOW_AMBER);
            });
        });
        suite(b, tech, 3, 0.3, |b| {
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(-3.45, -0.85, 10.8),
                v3(-3.45, -0.85, 13.9),
                0.58,
                0.58,
                b.sides(8),
            );
            b.block(v3(-3.85, -0.85, 13.9), v3(-3.05, 0.85, 14.25));
            b.paint(GLOW_AMBER);
            for z in [11.4, 12.3, 13.2] {
                b.cylinder_between(
                    v3(-3.45, -0.85, z),
                    v3(-3.45, -0.85, z + 0.2),
                    0.64,
                    0.64,
                    b.sides(8),
                );
            }
            b.block(v3(-3.9, -0.6, 13.98), v3(-3.85, 0.6, 14.18));
        });
        suite(b, tech, 3, 0.5, |b| {
            b.mirror_y(|b| {
                b.paint(PLATING);
                b.extrude_y(
                    &[[-1.9, 13.1], [-0.8, 13.1], [-1.2, 14.9], [-1.65, 14.9]],
                    1.55,
                    1.9,
                );
                b.paint(GLOW_AMBER);
                b.extrude_y(
                    &[[-1.75, 13.45], [-1.1, 13.45], [-1.32, 14.6], [-1.58, 14.6]],
                    1.9,
                    1.95,
                );
            });
        });

        if b.fine() {
            // Cockpit glass between the breastplates, reactor glow under the wedge.
            on_slope(b, [2.0, 12.5], [0.8, 13.4], 0.55, |b| {
                b.paint(GLASS);
                b.plate(Vec3::ZERO, v2(0.9, 0.95), 0.1, 0.04);
            });
            on_slope(b, [2.2, 11.3], [2.0, 12.5], 0.5, |b| {
                b.paint(ACCENT);
                b.plate(Vec3::ZERO, v2(0.75, 0.7), 0.08, 0.03);
            });
            on_slope(b, [1.2, 10.3], [2.2, 11.3], 0.5, |b| {
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.35, 0.0), v2(0.8, 0.35), GLOW))
            });
            // Prongs around the construction emitter.
            b.with_limb(rig::ARM_TOOL, |b| {
                let nozzle = COMMANDER_EMITTER - Vec3::X * 0.5;
                b.paint(METAL);
                for (dy, dz) in [(0.34, 0.22), (-0.34, 0.22), (0.0, -0.4)] {
                    b.beam(
                        nozzle + v3(-0.5, dy, dz),
                        COMMANDER_EMITTER + v3(-0.05, dy * 0.6, dz * 0.6),
                        v2(0.12, 0.12),
                        v2(0.08, 0.08),
                    );
                }
            });
            // Pack vents and an antenna.
            b.mirror_y(|b| vent(b, v3(-2.4, 0.75, 13.5), v2(0.75, 0.8), 3, GLOW));
            antenna(b, v3(-2.7, -1.3, 13.4), 1.6, 0.12);
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.block(v3(-3.34, 0.4, 11.0), v3(-3.3, 1.0, 12.1));
            });
        }
    });
}

// ---- Paladin: assault bot ----------------------------------------------------

pub fn assault_bot(b: &mut MeshBuilder, _tech: u8) {
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
    walker_legs(b, &stance, 8.0, 0.7);

    b.set_turret_pivot(v3(0.0, 0.0, 7.2));
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
                b.paint(PLATING).beam(
                    v3(-2.6, 3.6, 9.0),
                    v3(3.4, 3.6, 9.0),
                    v2(1.9, 1.7),
                    v2(1.2, 0.9),
                )
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
            b.paint(ACCENT);
            b.plate(Vec3::ZERO, v2(0.8, 3.0), 0.1, 0.04);
            glow_strip(b, v3(0.0, 0.0, 0.1), v2(0.3, 2.4), GLOW);
        });
        on_slope(b, [0.8, 10.6], [-2.4, 10.4], 0.35, |b| {
            team_panel(b, Vec3::ZERO, v2(1.5, 3.0))
        });

        // Arms: heavy arc projectors slung from the shoulders.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-0.4, 2.3, 9.2), v3(-0.4, 3.0, 9.2), 0.8, 0.8, b.sides(8));
            b.at(v3(0.0, 3.6, 0.0), |b| {
                b.paint(PLATING);
                b.extrude_y_chamfered(
                    &[
                        [-2.7, 8.1],
                        [0.5, 8.1],
                        [1.2, 8.5],
                        [1.2, 9.5],
                        [0.4, 9.95],
                        [-2.7, 9.95],
                    ],
                    0.98,
                    0.28,
                );
            });
            rail_gun(
                b,
                v3(0.9, 3.6, 9.0),
                v3(3.4, 3.6, 9.0),
                v2(0.28, 0.62),
                0.36,
                Emitter::Blue,
            );
            team_panel(b, v3(-1.9, 3.6, 9.95), v2(0.8, 1.2));
            if b.fine() {
                glow_strip(b, v3(-0.5, 3.6, 9.95), v2(1.5, 0.22), GLOW);
                // Capacitor coils behind each projector.
                for i in 0..3 {
                    let x = -2.5 + 0.55 * i as f32;
                    b.paint(GLOW);
                    b.block(v3(x, 4.58, 8.4), v3(x + 0.25, 4.62, 9.6));
                }
                b.paint(ACCENT);
                b.block(v3(-3.0, 3.0, 8.4), v3(-2.7, 4.2, 9.6));
            }
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
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.extrude_y(
                    &[[-2.9, 9.8], [-1.9, 10.4], [-2.3, 11.1], [-3.5, 10.4]],
                    1.0,
                    1.6,
                );
            });
            on_slope(b, [2.6, 9.6], [0.8, 10.6], 0.5, |b| {
                b.paint(GLASS);
                b.plate(Vec3::ZERO, v2(1.1, 2.2), 0.08, 0.04);
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.6, 0.0), v2(1.4, 0.16), GLOW));
            });
            on_slope(b, [1.6, 7.4], [3.0, 8.4], 0.5, |b| {
                b.mirror_y(|b| glow_strip(b, v3(0.0, 1.0, 0.0), v2(0.9, 0.6), GLOW))
            });
            antenna(b, v3(-2.6, 0.0, 10.4), 2.0, 0.1);
        }
    });

    if b.fine() {
        b.mirror_y(|b| {
            b.with_part(part::LOCOMOTION, |b| {
                b.paint(GLOW);
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

// ---- Lancer: light assault bot ---------------------------------------------

pub fn bot_light(b: &mut MeshBuilder, _tech: u8) {
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
