//! Aster walkers: the commander, the Paladin assault bot and the Lancer light bot.
//!
//! Legs and feet are `LOCOMOTION`, the pelvis is `HULL`, and everything above
//! the waist ring (torso, arms, head) is the `TURRET`, twisting about the
//! model's z axis so arm-mounted muzzles track the sim's muzzle offsets.

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

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

// ---- ARC Commander -----------------------------------------------------------
//
// Black-and-white armour: white plates (`PLATING`) sit over a black underlayer
// (`ACCENT`), so the black is the gap between the plates, not a graphite hull.
// Grey (`METAL`) is joints and pistons only. The right arm carries the guns, the
// left is the construction projector, and everything that builds is marked in
// amber. The bare commander has nothing on its back.
//
// Everything a refit adds is tagged with its module's key from the unit file
// (`b.module`): the model carries every module's pieces and the shader shows
// the ones the unit has fitted. The engineering suites stay on the left arm and
// leg; the back takes the formation engine or the shield pack; the right
// shoulder a shatter cannon or a howitzer; the left shoulder a second
// projector that folds out to build.

/// Where the machine gun ends and where the build beam leaves, as the unit file has them
/// (the unit file's numbers are these times 1.6: the model is authored at 6.5 m, the unit is 10.4).
const COMMANDER_MUZZLE: Vec3 = Vec3::new(5.5, -3.0, 9.7);
const COMMANDER_EMITTER: Vec3 = Vec3::new(4.8, 3.0, 9.7);
/// Muzzles of the right arm's main guns: they take the forearm's axis, and the machine gun
/// moves to a pod slung along the outside of the forearm (`SLUNG_AXIS` to `SLUNG_MUZZLE`).
const COMMANDER_CANNON: Vec3 = Vec3::new(6.8, -3.0, 9.7);
const COMMANDER_RAIL: Vec3 = Vec3::new(7.8, -3.0, 9.7);
const SLUNG_AXIS: Vec3 = Vec3::new(0.9, -4.15, 9.35);
const SLUNG_MUZZLE: Vec3 = Vec3::new(3.9, -4.15, 9.35);
/// The right shoulder's turrets: (muzzle, trunnion), authored level and facing forward.
const SHOULDER_AA: (Vec3, Vec3) = (Vec3::new(2.7, -3.1, 14.55), Vec3::new(0.0, -3.1, 14.55));
/// Both stand on the same trunnion: the model has one mount.
const SHOULDER_HOWITZER: (Vec3, Vec3) = (Vec3::new(4.9, -3.1, 14.55), Vec3::new(0.0, -3.1, 14.55));
/// The auxiliary suite: a robot arm on the back behind the left shoulder. `AUX_HINGE` is its
/// base knuckle, `AUX_WRIST` where the head bends (the unit file's `hinge`), `AUX_EMITTER`
/// the head's tip (its `emitters`). All authored out: the arm up and forward over the
/// shoulder, the head pointing a little down past level, as the sim aims it.
const AUX_HINGE: Vec3 = Vec3::new(-2.35, 2.6, 12.7);
const AUX_WRIST: Vec3 = Vec3::new(1.6, 3.5, 15.4);
const AUX_EMITTER: Vec3 = Vec3::new(4.099, 3.5, 14.893);
/// Where the arm ends, beside the head (which rides outboard of it, so they fold flat).
const AUX_ELBOW: Vec3 = Vec3::new(1.6, 3.0, 15.4);
/// Stowed, the arm hangs down the back from its knuckle, raked aft (-1.9 rad against 0.60
/// out), with the head folded up alongside it: nothing stands above the shoulder. It comes
/// out the long way round, back and up and over the shoulder (0.60 + 3.783 = 2π - 1.9).
const AUX_STOWED: f32 = 3.783;
const AUX_HEAD_STOWED: f32 = -1.88;
/// Suite III's lance: where it ends at rest, and how far it runs out while it builds
/// (the shader's `WORK_EXTEND`, 1.28 m on the 1.6 model). The unit file's `arm_emitter`
/// is its tip run out.
const LANCE_TIP: f32 = 6.7;

fn commander_leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, ankle: Vec3) {
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
            // Greave, with a black knee cap standing proud between it and the cuisse.
            b.paint(PLATING);
            b.beam(
                knee + v3(0.34, 0.0, -0.7),
                ankle + v3(0.5, 0.0, 0.55),
                v2(1.5, 1.1),
                v2(1.18, 0.82),
            );
            b.paint(ACCENT);
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
                b.module("eng_3", 0.72, |b| {
                    // Suite III: feed conduits down the greaves.
                    b.paint(GLOW_AMBER);
                    b.beam(
                        knee + v3(0.9, 0.0, -1.1),
                        ankle + v3(0.92, 0.0, 0.9),
                        v2(0.28, 0.07),
                        v2(0.22, 0.07),
                    );
                });
            }
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
                b.paint(PLATING);
                on_slope(b, [x1, 0.35 * h], [x1 - 1.1, h], 0.5, |b| {
                    b.plate(v3(0.0, ankle.y, 0.0), v2(1.0, 1.6), 0.12, 0.05)
                });
                b.plate(v3(ankle.x + 0.2, ankle.y, h), v2(1.4, 1.55), 0.1, 0.04);
            }
        });
    });
}

pub fn commander(b: &mut MeshBuilder, _tech: u8) {
    // Lean and long in the limb, stood up rather than crouched at rest. It walks:
    // a foot is down for over half the cycle, so one is always planted and the
    // body never leaves the ground. Its long stride needs more reach than the
    // straight legs have, so it settles onto bent knees as it gets going.
    let (hip, knee, ankle) = (
        v3(-0.1, 1.75, 8.7),
        v3(0.9, 1.85, 4.75),
        v3(-0.3, 1.85, 1.3),
    );
    b.set_legs(hip, knee, ankle, 16.0, 0.55, 1.3);
    b.set_walk_crouch(1.3);
    // Sole as `commander_leg` draws it: 1.2 m behind the ankle, 2.4 m ahead, 1.9 m across.
    b.set_foot(-1.2, 2.4, 1.9);
    b.mirror_y(|b| commander_leg(b, hip, knee, ankle));

    let (gun_y, tool_y, arm_z) = (COMMANDER_MUZZLE.y, COMMANDER_EMITTER.y, COMMANDER_MUZZLE.z);
    b.set_turret_pivot(v3(0.0, 0.0, 9.2));
    // The elbows: both forearms pitch about them to point at what they shoot or build.
    b.set_arm_pivot(v3(-0.3, tool_y, arm_z + 0.1));
    if b.coarse() {
        b.with_part(part::TURRET, |b| {
            b.paint(ACCENT);
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
                b.beam(v3(-1.4, gun_y, arm_z), COMMANDER_MUZZLE, v2(1.4, 1.4), v2(0.8, 0.65));
                b.module("cannon", 0.3, |b| {
                    b.beam(SLUNG_AXIS, SLUNG_MUZZLE, v2(0.5, 0.5), v2(0.4, 0.4));
                    b.until("railgun", |b| {
                        b.paint(PLATING);
                        b.beam(v3(1.8, gun_y, arm_z), COMMANDER_CANNON, v2(0.9, 0.9), v2(0.7, 0.7));
                    })
                });
                b.module("railgun", 0.3, |b| {
                    b.paint(PLATING);
                    b.beam(v3(1.6, gun_y, arm_z), COMMANDER_RAIL, v2(0.9, 1.1), v2(0.5, 0.8));
                });
            });
            b.paint(ACCENT);
            b.with_limb(rig::ARM_TOOL, |b| {
                b.beam(
                    v3(-1.2, tool_y, arm_z),
                    COMMANDER_EMITTER,
                    v2(1.3, 1.3),
                    v2(0.8, 0.7),
                );
                b.module("eng_2", 0.3, |b| {
                    b.paint(GLOW_AMBER);
                    b.block(v3(-1.0, tool_y + 0.65, 9.3), v3(0.8, tool_y + 0.8, 10.1));
                });
                b.module("eng_3", 0.3, |b| {
                    b.paint(PLATING);
                    b.block(v3(-0.9, tool_y - 0.32, 10.3), v3(3.2, tool_y + 0.32, 10.6));
                    b.paint(METAL);
                    b.beam(
                        v3(3.0, tool_y, arm_z),
                        v3(LANCE_TIP, tool_y, arm_z),
                        v2(0.45, 0.45),
                        v2(0.25, 0.25),
                    );
                });
            });
            b.module("mfe", 0.3, |b| {
                b.paint(ACCENT);
                b.block(v3(-3.6, -1.7, 10.2), v3(-1.8, 1.7, 13.6));
            });
            b.module("shield", 0.3, |b| {
                b.paint(PLATING);
                b.block(v3(-3.0, -1.0, 10.6), v3(-1.8, 1.0, 13.2));
                b.paint(ACCENT);
                b.mirror_y(|b| b.block(v3(-3.0, 1.0, 12.4), v3(-2.2, 1.4, 16.0)));
            });
            b.module("aa", 0.3, |b| {
                b.with_mount(SHOULDER_AA.1, 0.0, |b| {
                    b.paint(ACCENT);
                    b.block(v3(-0.9, -3.8, 13.7), v3(0.9, -2.4, 15.2));
                    b.paint(GLOW);
                    b.beam(SHOULDER_AA.1, SHOULDER_AA.0, v2(1.1, 0.9), v2(1.1, 0.9));
                })
            });
            b.module("artillery", 0.3, |b| {
                b.with_mount(SHOULDER_HOWITZER.1, 0.0, |b| {
                    b.paint(PLATING);
                    b.beam(SHOULDER_HOWITZER.1 - Vec3::X * 1.2, SHOULDER_HOWITZER.0, v2(1.2, 1.2), v2(0.55, 0.55));
                })
            });
            b.module("aux_eng", 0.3, |b| {
                b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
                    b.paint(ACCENT);
                    b.beam(AUX_HINGE, AUX_ELBOW, v2(0.6, 0.6), v2(0.5, 0.5));
                });
                b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
                    b.paint(ACCENT);
                    b.beam(AUX_WRIST, AUX_EMITTER, v2(0.6, 0.6), v2(0.35, 0.35));
                });
            });
        });
        return;
    }

    // Pelvis: black skirts with a white plate on the front. Sits on the hip.
    b.paint(ACCENT);
    b.chamfered_box(v3(-0.1, 0.0, 8.7), v3(2.1, 2.2, 1.5), 0.5);
    b.frustum(
        v3(0.95, 0.0, 7.7),
        v2(0.45, 1.2),
        v2(0.65, 1.7),
        1.7,
        v2(0.18, 0.0),
    );
    b.frustum(
        v3(-1.15, 0.0, 7.9),
        v2(0.4, 1.3),
        v2(0.5, 1.8),
        1.4,
        v2(-0.12, 0.0),
    );
    b.paint(PLATING);
    b.plate(v3(1.05, 0.0, 9.35), v2(0.7, 1.35), 0.1, 0.04);

    b.with_part(part::TURRET, |b| {
        // Waist ring and a narrow abdomen under the chest.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 9.05), b.sides(8), 1.2, 1.2, 0.5);
        b.paint(METAL);
        b.chamfered_box(v3(-0.1, 0.0, 9.95), v3(1.9, 2.4, 0.9), 0.45);
        // Chest: a black wedge, white plates on it with the black showing between.
        b.paint(ACCENT);
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
        b.paint(PLATING);
        on_slope(b, [2.0, 12.5], [0.8, 13.4], 0.5, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.15, 0.0), v2(1.3, 1.05), 0.18, 0.06))
        });
        on_slope(b, [2.2, 11.3], [2.0, 12.5], 0.5, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.05, 0.0), v2(0.95, 1.2), 0.18, 0.06))
        });
        on_slope(b, [1.2, 10.3], [2.2, 11.3], 0.5, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 0.7, 0.0), v2(0.9, 0.85), 0.14, 0.05))
        });
        on_slope(b, [0.8, 13.4], [-1.7, 13.2], 0.45, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.05, 0.0), v2(1.2, 0.8), 0.12, 0.05))
        });
        // Back of the chest: a spine and two white shoulder-blade plates where a
        // pack would bolt on. Bare, it is only the black wedge and these.
        b.paint(ACCENT);
        b.block(v3(-2.45, -0.5, 10.6), v3(-2.05, 0.5, 13.0));
        b.paint(PLATING);
        on_slope(b, [-1.7, 13.2], [-2.2, 11.7], 0.55, |b| {
            b.mirror_y(|b| b.plate(v3(0.0, 1.1, 0.0), v2(1.05, 0.95), 0.12, 0.05))
        });
        // Head, sunk between the shoulders: black helmet, white crown and cheeks.
        // It looks about while the commander stands idle.
        b.with_head(v3(0.3, 0.0, 13.2), |b| {
            b.at(v3(0.4, 0.0, 0.0), |b| {
                b.paint(ACCENT);
                b.loft_z(
                    &turret_plan(1.7, 1.3),
                    &[
                        Section::new(13.2, 0.85),
                        Section::new(13.8, 1.0),
                        Section::scaled(14.5, 0.5, 0.55).shifted(-0.2, 0.0),
                    ],
                );
                b.paint(PLATING);
                b.plate(v3(-0.15, 0.0, 14.48), v2(0.55, 0.72), 0.07, 0.03);
                b.mirror_y(|b| b.plate(v3(0.15, 0.52, 13.92), v2(0.7, 0.28), 0.07, 0.03));
                b.paint(GLOW);
                b.block(v3(0.6, -0.33, 13.7), v3(0.85, 0.33, 13.88));
            });
            if b.fine() {
                // A short aerial off the back of the helmet.
                antenna(b, v3(-0.35, -0.45, 14.3), 1.2, 0.18);
            }
        });

        // Shoulders: black pauldrons carrying the team colour, white plates on
        // the cap and a guard along the outer edge, black in the gaps.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(0.0, 1.8, 12.2), v3(0.0, 2.6, 12.2), 0.7, 0.7, b.sides(8));
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
            team_panel(b, v3(0.15, 2.75, 13.35), v2(0.7, 0.7));
            b.paint(PLATING);
            b.plate(v3(-0.7, 3.1, 13.28), v2(0.7, 0.55), 0.08, 0.03);
            b.plate(v3(0.8, 3.1, 13.28), v2(0.55, 0.55), 0.08, 0.03);
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

        commander_gun_arm(b, gun_y, arm_z);
        commander_tool_arm(b, tool_y, arm_z);
        commander_back(b);
        commander_shoulders(b);

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
        }
    });
}

/// Right forearm. Bare, the Vulcan rotary gun sits on the forearm's axis. A main gun takes
/// that place when it goes on (the breech cannon, later rebuilt as the rail cannon) and the
/// Vulcan moves to a pod slung along the outside of the forearm. The main gun kicks back
/// when it fires; the Vulcan's barrels spin up before it fires.
fn commander_gun_arm(b: &mut MeshBuilder, gun_y: f32, arm_z: f32) {
    b.with_limb(rig::ARM_GUN, |b| {
        b.at(v3(0.0, gun_y, 0.0), |b| {
            b.paint(ACCENT);
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
            b.paint(PLATING);
            b.plate(v3(-0.7, 0.0, 10.5), v2(1.3, 1.05), 0.1, 0.04);
            b.plate(v3(1.15, 0.0, 10.5), v2(1.4, 1.05), 0.1, 0.04);
        });
        if b.fine() {
            b.paint(ACCENT);
            b.block(v3(-1.4, gun_y - 0.4, 10.6), v3(-0.1, gun_y + 0.4, 10.8));
            glow_strip(b, v3(-0.75, gun_y, 10.8), v2(1.0, 0.2), GLOW_ORANGE);
        }

        // The Vulcan in the primary place: three barrels in a spinning clamp, a drum on the side.
        b.until("cannon", |b| {
            let axis = v3(2.2, gun_y, arm_z);
            b.paint(METAL);
            b.cylinder_between(axis, axis + Vec3::X * 0.7, 0.55, 0.5, b.sides(8));
            b.with_spin(axis, |b| vulcan(b, axis + Vec3::X * 0.5, COMMANDER_MUZZLE, 0.26, 0.09));
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.2, gun_y - 0.8, arm_z - 0.1),
                v3(0.2, gun_y - 1.25, arm_z - 0.1),
                0.62,
                0.62,
                b.sides(10),
            );
            if b.fine() {
                b.paint(PLATING);
                b.cylinder_between(
                    v3(0.2, gun_y - 1.25, arm_z - 0.1),
                    v3(0.2, gun_y - 1.3, arm_z - 0.1),
                    0.5,
                    0.5,
                    10,
                );
                b.paint(METAL);
                b.beam(
                    v3(0.6, gun_y - 0.95, arm_z - 0.1),
                    v3(2.1, gun_y - 0.35, arm_z - 0.05),
                    v2(0.16, 0.24),
                    v2(0.14, 0.2),
                );
            }
        });
        // Moved aside: the same gun, slung in a pod along the outside of the forearm.
        b.module("cannon", 0.1, |b| {
            b.paint(ACCENT);
            b.chamfered_box(
                v3(0.1, SLUNG_AXIS.y + 0.1, SLUNG_AXIS.z),
                v3(1.9, 0.75, 0.8),
                0.2,
            );
            b.paint(PLATING);
            b.block(
                v3(-0.6, SLUNG_AXIS.y - 0.28, SLUNG_AXIS.z + 0.35),
                v3(0.8, SLUNG_AXIS.y + 0.4, SLUNG_AXIS.z + 0.45),
            );
            b.paint(METAL);
            b.beam(
                v3(0.0, SLUNG_AXIS.y + 0.45, SLUNG_AXIS.z),
                v3(0.0, gun_y - 0.78, SLUNG_AXIS.z),
                v2(0.4, 0.3),
                v2(0.4, 0.3),
            );
            b.with_spin(SLUNG_AXIS, |b| vulcan(b, SLUNG_AXIS + Vec3::X * 0.2, SLUNG_MUZZLE, 0.2, 0.07));
        });

        // The breech cannon: a white breech housing on the forearm's nose, the tube in it.
        b.module("cannon", 0.3, |b| {
            b.until("railgun", |b| {
                let breech = v3(1.2, gun_y, arm_z);
                b.set_recoil(breech, COMMANDER_CANNON, 0.55);
                b.paint(PLATING);
                b.at(v3(0.0, gun_y, 0.0), |b| {
                    b.extrude_y_chamfered(
                        &[[1.0, 9.05], [2.5, 9.05], [2.9, 9.4], [2.9, 10.0], [2.5, 10.35], [1.0, 10.35]],
                        0.62,
                        0.16,
                    )
                });
                b.with_recoil(|b| cannon(b, breech, COMMANDER_CANNON, 0.28, Emitter::Unlit));
                if b.fine() {
                    b.paint(METAL);
                    b.mirror_y(|b| {
                        b.at(v3(0.0, gun_y, 0.0), |b| {
                            b.cylinder_between(v3(1.4, 0.5, 10.25), v3(3.1, 0.3, 10.05), 0.1, 0.1, 6)
                        })
                    });
                }
            });
        });

        // The rail cannon: a capacitor bank on the forearm feeding two long rails above and
        // below the bore, a lit core between them, coils along them, a split muzzle.
        b.module("railgun", 0.3, |b| {
            let breech = v3(1.4, gun_y, arm_z);
            b.set_recoil(breech, COMMANDER_RAIL, 0.35);
            // Capacitor bank: black, white lid, a row of blue cells.
            b.paint(ACCENT);
            b.at(v3(0.0, gun_y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[[-1.5, 10.5], [1.3, 10.5], [1.7, 10.8], [1.7, 11.25], [-1.3, 11.25], [-1.5, 11.0]],
                    0.72,
                    0.18,
                )
            });
            b.paint(PLATING);
            b.plate(v3(-0.1, gun_y, 11.25), v2(2.4, 1.1), 0.08, 0.03);
            if b.fine() {
                b.paint(GLOW);
                for i in 0..4 {
                    let x = -1.1 + i as f32 * 0.62;
                    b.mirror_y(|b| {
                        b.at(v3(0.0, gun_y, 0.0), |b| b.block(v3(x, 0.72, 10.65), v3(x + 0.38, 0.75, 11.05)))
                    });
                }
            }
            b.with_recoil(|b| {
                let len = COMMANDER_RAIL.x - breech.x;
                b.at(v3(breech.x, gun_y, arm_z), |b| {
                    // Rails, above and below the bore.
                    for dz in [0.36, -0.36] {
                        b.paint(METAL);
                        b.beam(v3(0.0, 0.0, dz), v3(len, 0.0, dz * 0.8), v2(0.3, 0.26), v2(0.24, 0.2));
                    }
                    // The charged core between them.
                    b.paint(GLOW);
                    b.beam(v3(0.3, 0.0, 0.0), v3(len - 0.25, 0.0, 0.0), v2(0.1, 0.2), v2(0.1, 0.16));
                    // Coils round the rails.
                    b.paint(ACCENT);
                    let coils = if b.fine() { 5 } else { 2 };
                    for i in 0..coils {
                        let x = 1.0 + i as f32 * (len - 2.2) / (coils - 1).max(1) as f32;
                        b.beam(v3(x, 0.0, 0.0), v3(x + 0.26, 0.0, 0.0), v2(0.62, 1.12), v2(0.62, 1.12));
                    }
                    // White breech shroud and the split muzzle.
                    b.paint(PLATING);
                    b.extrude_y_chamfered(
                        &[[-0.2, -0.62], [1.6, -0.62], [2.0, -0.4], [2.0, 0.4], [1.6, 0.62], [-0.2, 0.62]],
                        0.42,
                        0.12,
                    );
                    b.paint(ACCENT);
                    for dz in [0.44, -0.44] {
                        b.beam(v3(len - 0.5, 0.0, dz), v3(len + 0.15, 0.0, dz * 1.15), v2(0.42, 0.18), v2(0.36, 0.14));
                    }
                });
            });
        });
    });
}

/// Three barrels round an axis in a clamp: the Vulcan, wherever it is carried.
fn vulcan(b: &mut MeshBuilder, from: Vec3, to: Vec3, ring: f32, barrel: f32) {
    if b.fine() {
        for i in 0..3 {
            let a = i as f32 * std::f32::consts::TAU / 3.0 + 0.5;
            let off = v3(0.0, a.cos() * ring, a.sin() * ring);
            machine_gun(b, from + off, to + off * 0.8, barrel);
        }
        b.paint(ACCENT);
        let mid = from.lerp(to, 0.55);
        b.cylinder_between(mid, mid + Vec3::X * 0.22, ring * 1.8, ring * 1.8, 8);
    } else {
        // From further off the three barrels read as one.
        b.paint(METAL);
        b.cylinder_between(from, to, ring * 1.2, ring, 6);
        b.paint(GLOW_ORANGE);
        b.cylinder_between(to - Vec3::X * 0.05, to + Vec3::X * 0.02, ring * 0.55, ring * 0.45, 6);
    }
}

/// Left forearm: the construction projector, and the engineering suites. Each suite is
/// built into the arm (and Suite III down the leg), never onto the back. What builds is
/// amber; the suites' conduit housings run with light while the commander builds.
fn commander_tool_arm(b: &mut MeshBuilder, tool_y: f32, arm_z: f32) {
    b.with_limb(rig::ARM_TOOL, |b| {
        let nozzle = COMMANDER_EMITTER - Vec3::X * 0.5;
        b.at(v3(0.0, tool_y, 0.0), |b| {
            b.paint(ACCENT);
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
            b.paint(PLATING);
            b.plate(v3(-0.6, 0.0, 10.4), v2(1.2, 0.95), 0.1, 0.04);
            b.plate(v3(1.0, 0.0, 10.4), v2(1.3, 0.95), 0.1, 0.04);
        });
        // Suite III's lance barrel takes the projector's place.
        b.until("eng_3", |b| {
            b.paint(METAL);
            b.cylinder_between(v3(1.8, tool_y, arm_z), nozzle, 0.45, 0.36, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(nozzle, COMMANDER_EMITTER, 0.3, 0.14, 6);
        });
        b.paint(GLOW_AMBER);
        b.cylinder_between(
            v3(2.3, tool_y, arm_z),
            v3(2.5, tool_y, arm_z),
            0.55,
            0.55,
            b.sides(8),
        );
        glow_strip(b, v3(-0.2, tool_y, 10.4), v2(2.0, 0.26), GLOW_AMBER);
        if b.fine() {
            // Prongs around the construction emitter.
            b.until("eng_3", |b| {
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
        }

        // Suite II. Conduit housings along both flanks of the forearm, white-capped, running
        // with light as it builds; a fabricator head of a focusing collar and three long
        // prongs round the projector; a wrist drum under it.
        b.module("eng_2", 0.0, |b| {
            b.mirror_y(|b| {
                b.at(v3(0.0, tool_y, 0.0), |b| {
                    b.paint(ACCENT).pattern(pattern::CONDUIT);
                    b.at(v3(0.0, 0.92, 0.0), |b| {
                        b.extrude_y_chamfered(
                            &[[-1.2, 9.1], [1.3, 9.1], [1.7, 9.45], [1.7, 10.0], [1.3, 10.3], [-1.2, 10.3]],
                            0.2,
                            0.08,
                        )
                    });
                    b.paint(PLATING);
                    b.block(v3(-1.25, 0.78, 10.3), v3(1.35, 1.14, 10.42));
                    b.block(v3(1.3, 0.78, 9.15), v3(1.75, 1.14, 10.3));
                });
            });
        });
        // The fabricator head twists back and forth while it builds. Suite III takes its place.
        b.module("eng_2", 0.3, |b| b.until("eng_3", |b| b.with_work(rig::WORK_TWIST, |b| {
            b.paint(PLATING);
            b.cylinder_between(
                v3(2.95, tool_y, arm_z),
                v3(3.4, tool_y, arm_z),
                0.72,
                0.66,
                b.sides(10),
            );
            b.paint(GLOW_AMBER);
            b.cylinder_between(
                v3(3.4, tool_y, arm_z),
                v3(3.46, tool_y, arm_z),
                0.6,
                0.6,
                b.sides(10),
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
        })));
        b.module("eng_2", 0.55, |b| {
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.6, tool_y - 0.6, 8.75),
                v3(0.6, tool_y + 0.6, 8.75),
                0.44,
                0.44,
                b.sides(10),
            );
            b.paint(PLATING);
            b.cylinder_between(
                v3(0.6, tool_y - 0.66, 8.75),
                v3(0.6, tool_y - 0.6, 8.75),
                0.36,
                0.36,
                b.sides(10),
            );
            b.paint(GLOW_AMBER);
            for dy in [-0.3, 0.3] {
                b.cylinder_between(
                    v3(0.6, tool_y + dy - 0.05, 8.75),
                    v3(0.6, tool_y + dy + 0.05, 8.75),
                    0.48,
                    0.48,
                    b.sides(10),
                );
            }
        });

        // Suite III. Long and lean: a narrow white spine down the top of the forearm, and a
        // lance past the projector, carried in a slim barrel between two fine rails. While it
        // builds the lance runs out, the focusing rings on the barrel twist back and forth,
        // and the prongs round the lance's tip open and close.
        b.module("eng_3", 0.0, |b| {
            b.paint(PLATING);
            b.at(v3(0.0, tool_y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[[-1.2, 10.4], [2.6, 10.4], [3.4, 10.2], [3.3, 10.5], [2.4, 10.72], [-1.2, 10.72]],
                    0.4,
                    0.12,
                )
            });
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(v3(-0.9, tool_y - 0.12, 10.72), v3(2.3, tool_y + 0.12, 10.78));
        });
        b.module("eng_3", 0.3, |b| {
            let (y, z) = (tool_y, arm_z);
            // The barrel: from the wrist to past the old projector, tapering.
            b.paint(ACCENT);
            b.cylinder_between(v3(2.2, y, z), v3(5.3, y, z), 0.36, 0.28, b.sides(10));
            b.paint(PLATING);
            b.beam(v3(2.3, y, z + 0.34), v3(4.9, y, z + 0.27), v2(0.34, 0.08), v2(0.24, 0.07));
            // Rails either side, lit at the ends.
            for dy in [-0.44, 0.44] {
                b.paint(METAL);
                b.cylinder_between(v3(2.0, y + dy, z - 0.05), v3(5.6, y + dy * 0.8, z - 0.05), 0.09, 0.07, 6);
                b.paint(GLOW_AMBER);
                b.cylinder_between(v3(5.6, y + dy * 0.8, z - 0.05), v3(5.85, y + dy * 0.78, z - 0.05), 0.07, 0.03, 6);
            }
            // Focusing rings, each with three lit studs, twisting on the barrel.
            b.with_work(rig::WORK_TWIST, |b| {
                for (i, x) in [3.2f32, 3.85, 4.5].into_iter().enumerate() {
                    let r = 0.5 - i as f32 * 0.05;
                    b.paint(PLATING);
                    b.cylinder_between(v3(x, y, z), v3(x + 0.16, y, z), r, r, b.sides(12));
                    b.paint(GLOW_AMBER);
                    for k in 0..3 {
                        let a = (k as f32 + i as f32 * 0.5) * std::f32::consts::TAU / 3.0;
                        let c = v3(x + 0.08, y + a.cos() * r, z + a.sin() * r);
                        b.block(c - v3(0.07, 0.07, 0.07), c + v3(0.07, 0.07, 0.07));
                    }
                }
            });
            // The lance: a slender rod out of the barrel, amber at the point.
            b.with_work(rig::WORK_EXTEND, |b| {
                b.paint(METAL);
                b.cylinder_between(v3(4.4, y, z), v3(LANCE_TIP - 0.45, y, z), 0.19, 0.15, b.sides(8));
                b.paint(ACCENT);
                b.cylinder_between(v3(5.45, y, z), v3(5.6, y, z), 0.24, 0.24, b.sides(8));
                b.paint(GLOW_AMBER);
                b.cylinder_between(v3(LANCE_TIP - 0.45, y, z), v3(LANCE_TIP, y, z), 0.15, 0.04, 6);
            });
            // Prongs round the barrel's mouth.
            b.with_work(rig::WORK_BREATHE, |b| {
                for k in 0..3 {
                    let a = (k as f32 + 0.25) * std::f32::consts::TAU / 3.0;
                    let (cy, cz) = (a.cos(), a.sin());
                    b.paint(METAL);
                    b.beam(
                        v3(4.95, y + cy * 0.3, z + cz * 0.3),
                        v3(5.95, y + cy * 0.2, z + cz * 0.2),
                        v2(0.1, 0.1),
                        v2(0.06, 0.06),
                    );
                    b.paint(GLOW_AMBER);
                    b.beam(
                        v3(5.95, y + cy * 0.2, z + cz * 0.2),
                        v3(6.1, y + cy * 0.18, z + cz * 0.18),
                        v2(0.06, 0.06),
                        v2(0.04, 0.04),
                    );
                }
            });
        });
    });
    // Suite III: an armoured sleeve and a lit feed line down the upper arm.
    b.module("eng_3", 0.55, |b| {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.beam(
            v3(0.05, tool_y + 0.5, 11.7),
            v3(-0.25, tool_y + 0.48, 10.3),
            v2(0.95, 0.26),
            v2(0.85, 0.24),
        );
        b.paint(PLATING);
        b.beam(
            v3(0.05, tool_y + 0.66, 11.75),
            v3(-0.1, tool_y + 0.64, 11.0),
            v2(1.0, 0.08),
            v2(0.95, 0.08),
        );
    });
}

/// The back: bare, or one of the two packs. The formation engine is a wide,
/// low block of furnace and drums; the shield pack a narrow core under two
/// tall projector fins that stand clear of the shoulders.
fn commander_back(b: &mut MeshBuilder) {
    b.module("mfe", 0.0, |b| {
        // Shell: black, two white lids with a furnace slot between them.
        b.paint(ACCENT);
        b.block(v3(-3.2, -1.55, 10.4), v3(-2.0, 1.55, 13.3));
        b.extrude_y_chamfered(
            &[[-3.75, 10.2], [-3.1, 10.2], [-3.1, 13.6], [-2.1, 13.6], [-2.1, 13.9], [-3.75, 13.2]],
            1.75,
            0.3,
        );
        b.paint(PLATING);
        b.mirror_y(|b| b.plate(v3(-2.9, 0.95, 13.62), v2(1.2, 1.0), 0.1, 0.04));
        b.paint(GLOW_ORANGE).pattern(pattern::FURNACE);
        b.block(v3(-3.3, -0.32, 13.55), v3(-2.3, 0.32, 13.72));
    });
    b.module("mfe", 0.35, |b| {
        // Formation drums, one each side, banded in the furnace's light.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.cylinder_between(v3(-3.3, 2.0, 10.4), v3(-3.3, 2.0, 13.2), 0.62, 0.62, b.sides(10));
            b.paint(PLATING);
            b.cylinder_between(v3(-3.3, 2.0, 13.2), v3(-3.3, 2.0, 13.45), 0.5, 0.42, b.sides(10));
            b.paint(GLOW_ORANGE);
            for z in [10.9, 11.7, 12.5] {
                b.cylinder_between(v3(-3.3, 2.0, z), v3(-3.3, 2.0, z + 0.16), 0.66, 0.66, b.sides(10));
            }
        });
    });
    b.module("mfe", 0.7, |b| {
        if b.fine() {
            b.mirror_y(|b| vent(b, v3(-2.6, 0.95, 13.72), v2(0.6, 0.7), 3, GLOW_ORANGE));
            b.paint(METAL);
            b.mirror_y(|b| {
                b.cylinder_between(v3(-2.3, 1.3, 13.0), v3(-2.3, 2.55, 12.6), 0.12, 0.12, 6)
            });
        }
    });

    b.module("shield", 0.0, |b| {
        // Generator core: a narrow white housing with a glass lens, set close to the spine.
        b.paint(ACCENT);
        b.block(v3(-2.85, -0.85, 10.6), v3(-2.05, 0.85, 13.1));
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[[-3.2, 10.9], [-2.8, 10.9], [-2.8, 13.0], [-3.2, 12.7]],
            0.95,
            0.2,
        );
        b.paint(GLASS);
        b.cylinder_between(v3(-3.2, 0.0, 11.9), v3(-3.35, 0.0, 11.9), 0.55, 0.5, b.sides(10));
        b.paint(GLOW);
        b.cylinder_between(v3(-3.34, 0.0, 11.9), v3(-3.4, 0.0, 11.9), 0.32, 0.3, b.sides(10));
    });
    b.module("shield", 0.35, |b| {
        // Projector fins: tall, raked back, lit along the leading edge.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(
                &[[-2.9, 12.2], [-2.3, 12.4], [-2.7, 16.1], [-3.2, 16.3], [-3.3, 12.4]],
                1.05,
                1.3,
            );
            b.paint(ACCENT);
            b.extrude_y(
                &[[-3.3, 11.2], [-2.4, 11.2], [-2.3, 12.4], [-3.3, 12.4]],
                0.95,
                1.4,
            );
            b.paint(GLOW);
            b.extrude_y(
                &[[-2.32, 12.5], [-2.2, 12.5], [-2.62, 16.0], [-2.72, 16.0]],
                1.1,
                1.25,
            );
        });
    });
    b.module("shield", 0.7, |b| {
        if b.fine() {
            b.paint(GLOW);
            b.mirror_y(|b| b.cuboid(v3(-2.95, 1.18, 16.35), Vec3::splat(0.22)));
            b.paint(METAL);
            b.cylinder_between(v3(-2.6, -1.1, 14.0), v3(-2.6, 1.1, 14.0), 0.1, 0.1, 6);
        }
    });
}

/// The shoulders' refits. On the right, a turret of its own that aims at what it shoots,
/// whatever the torso is doing: the AA shatter cannon, a squat four-tube flak drum lit blue,
/// or the howitzer, one long tube over the shoulder. On the left, a second projector that
/// lies back behind the shoulder and folds out over it to build.
fn commander_shoulders(b: &mut MeshBuilder) {
    // Both right-shoulder turrets stand on the same pylon.
    let pylon = |b: &mut MeshBuilder| {
        b.paint(ACCENT);
        b.cylinder_between(v3(-0.2, -3.1, 13.25), v3(-0.2, -3.1, 13.75), 0.72, 0.62, b.sides(10));
    };
    let (aa_muzzle, aa_trunnion) = SHOULDER_AA;
    b.module("aa", 0.0, |b| {
        pylon(b);
        b.with_mount(aa_trunnion, 0.35, |b| {
            // The drum: black, with a white cowl and a blue ring round its waist.
            b.paint(ACCENT);
            b.cylinder_between(
                aa_trunnion - Vec3::Y * 0.75,
                aa_trunnion + Vec3::Y * 0.75,
                0.78,
                0.78,
                b.sides(12),
            );
            b.paint(PLATING);
            b.at(v3(0.0, aa_trunnion.y, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[[-0.9, 14.9], [0.5, 14.9], [0.95, 14.6], [0.4, 15.45], [-0.8, 15.45]],
                    0.8,
                    0.14,
                )
            });
            // A blue band round the drum's waist.
            b.paint(GLOW);
            b.cylinder_between(
                aa_trunnion - Vec3::Y * 0.07,
                aa_trunnion + Vec3::Y * 0.07,
                0.81,
                0.81,
                b.sides(12),
            );
        });
    });
    b.module("aa", 0.4, |b| {
        b.with_mount(aa_trunnion, 0.35, |b| {
            // Four short tubes, two by two, blue at the mouth.
            b.with_recoil(|b| {
                for (dy, dz) in [(-0.24, 0.22), (0.24, 0.22), (-0.24, -0.22), (0.24, -0.22)] {
                    let off = v3(0.0, dy, dz);
                    cannon(b, aa_trunnion + off + Vec3::X * 0.5, aa_muzzle + off, 0.1, Emitter::Blue);
                }
            });
            if b.fine() {
                // Fire-control plate on the outer side.
                b.paint(ACCENT);
                b.block(
                    aa_trunnion + v3(-0.6, -0.95, 0.1),
                    aa_trunnion + v3(0.4, -0.8, 0.8),
                );
                b.paint(GLOW);
                b.block(
                    aa_trunnion + v3(-0.4, -0.99, 0.35),
                    aa_trunnion + v3(0.2, -0.95, 0.55),
                );
            }
        });
    });

    let (how_muzzle, how_trunnion) = SHOULDER_HOWITZER;
    b.module("artillery", 0.0, |b| {
        pylon(b);
        b.with_mount(how_trunnion, 0.7, |b| {
            // Cradle: two white cheeks the tube swings between, a black counterweight behind.
            // Mirrored about the trunnion, not the body's centreline, or one cheek would sit
            // on the other shoulder and swing with the gun.
            b.paint(PLATING);
            b.at(v3(0.0, how_trunnion.y, 0.0), |b| {
                b.mirror_y(|b| {
                    b.extrude_y(
                        &[[-1.3, 13.8], [0.5, 13.8], [0.3, 14.95], [-1.0, 15.1]],
                        0.36,
                        0.52,
                    )
                })
            });
            b.paint(ACCENT);
            b.chamfered_box(how_trunnion + v3(-1.6, 0.0, 0.0), v3(1.1, 0.8, 0.9), 0.2);
            b.paint(METAL);
            b.cylinder_between(
                how_trunnion - Vec3::Y * 0.56,
                how_trunnion + Vec3::Y * 0.56,
                0.24,
                0.24,
                b.sides(8),
            );
        });
    });
    b.module("artillery", 0.4, |b| {
        b.with_mount(how_trunnion, 0.7, |b| {
            b.with_recoil(|b| howitzer(b, how_trunnion - Vec3::X * 1.1, how_muzzle, 0.3));
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(
                    how_trunnion + v3(0.1, 0.0, -0.32),
                    how_trunnion + v3(1.8, 0.0, -0.32),
                    0.1,
                    0.1,
                    6,
                );
                b.paint(GLOW_ORANGE);
                b.block(how_trunnion + v3(-2.1, -0.3, 0.1), how_trunnion + v3(-2.14, 0.3, 0.3));
            }
        });
    });

    b.module("aux_eng", 0.0, |b| {
        // The mount on the back behind the left pauldron: a black saddle and the base knuckle.
        b.paint(ACCENT);
        b.chamfered_box(v3(-2.35, 2.55, 12.45), v3(0.9, 1.0, 1.3), 0.18);
        b.paint(PLATING);
        b.block(v3(-2.85, 2.1, 12.2), v3(-2.78, 3.0, 13.0));
        b.paint(METAL);
        b.cylinder_between(
            AUX_HINGE - Vec3::Y * 0.5,
            AUX_HINGE + Vec3::Y * 0.5,
            0.36,
            0.36,
            b.sides(10),
        );
    });
    b.module("aux_eng", 0.3, |b| {
        // The arm, authored out: black, a white guard down its back, a piston under it and
        // a lit feed line; knuckles at both ends.
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            let along = (AUX_ELBOW - AUX_HINGE).normalize();
            let over = Vec3::new(-along.z, 0.0, along.x);
            b.paint(ACCENT);
            b.beam(AUX_HINGE, AUX_ELBOW, v2(0.5, 0.55), v2(0.4, 0.45));
            b.paint(PLATING);
            b.beam(
                AUX_HINGE + along * 0.6 + over * 0.3,
                AUX_ELBOW - along * 0.5 + over * 0.26,
                v2(0.56, 0.1),
                v2(0.46, 0.09),
            );
            b.paint(METAL);
            b.cylinder_between(
                AUX_ELBOW - Vec3::Y * 0.2,
                AUX_ELBOW + Vec3::Y * 0.75,
                0.3,
                0.3,
                b.sides(8),
            );
            if b.fine() {
                b.cylinder_between(
                    AUX_HINGE + along * 0.5 - over * 0.36,
                    AUX_HINGE + along * 3.2 - over * 0.3,
                    0.1,
                    0.1,
                    6,
                );
                b.paint(ACCENT).pattern(pattern::CONDUIT);
                b.beam(
                    AUX_HINGE + along * 0.9 + Vec3::Y * 0.22,
                    AUX_ELBOW - along * 0.7 + Vec3::Y * 0.22,
                    v2(0.08, 0.3),
                    v2(0.08, 0.26),
                );
            }
        });
        // The head, outboard of the arm on the wrist knuckle: a short housing, a white cowl
        // and an amber projector with three prongs, pointing at the work.
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            let d = (AUX_EMITTER - AUX_WRIST).normalize();
            let up = Vec3::new(-d.z, 0.0, d.x);
            let at = |t: f32| AUX_WRIST + d * t;
            b.paint(ACCENT);
            b.beam(at(-0.3), at(1.5), v2(0.5, 0.55), v2(0.42, 0.42));
            b.paint(PLATING);
            b.beam(at(0.0) + up * 0.3, at(1.35) + up * 0.24, v2(0.5, 0.08), v2(0.4, 0.07));
            b.paint(METAL);
            b.cylinder_between(at(1.5), at(2.2), 0.24, 0.2, b.sides(8));
            b.paint(GLOW_AMBER);
            b.cylinder_between(at(2.2), at(2.55), 0.16, 0.05, 6);
            if b.fine() {
                let side = Vec3::Y;
                b.paint(METAL);
                for (s, u) in [(0.28, 0.16), (-0.28, 0.16), (0.0, -0.3)] {
                    b.beam(
                        at(1.7) + side * s + up * u,
                        at(2.45) + side * s * 0.55 + up * u * 0.55,
                        v2(0.08, 0.08),
                        v2(0.05, 0.05),
                    );
                }
                glow_strip(b, at(0.7) + up * 0.28, v2(0.5, 0.16), GLOW_AMBER);
            }
        });
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
    // Twice the hull, twice the ground: 16 m to a cycle, feet lifted clear of a
    // shuffle. The shader plants each foot, so the stride stays a power of two.
    walker_legs(b, &stance, 16.0, 1.4);
    // It walks the seabed: a sealed torpedo tube strapped to each shin, riding the shin bone.
    b.mirror_y(shin_tube);

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

/// Where each shin tube's mouth is at rest (left leg; the right is its mirror): the unit
/// file's torpedo muzzles, (2.2, ±4.6, 3.0) on the 24 m Paladin, over two.
const PALADIN_TUBE_MOUTH: Vec3 = Vec3::new(1.1, 2.3, 1.5);

/// A short armoured pod strapped to the front of the lower shin, its mouth facing
/// forward: dark bore in a white cap, a thin blue ring behind the cap (tech 3).
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
            b.block(v3(-0.75, m.y - 0.22, m.z - 0.34), v3(-0.25, m.y + 0.22, m.z + 0.12));
            // White cap round the mouth, the bore dark inside it.
            b.paint(PLATING);
            b.cylinder_between(at(m.x - 0.18), at(m.x - 0.01), 0.33, 0.31, sides);
            b.paint(ACCENT);
            b.cylinder_between(at(m.x - 0.05), at(m.x), 0.17, 0.17, sides);
            if b.fine() {
                b.paint(GLOW);
                b.cylinder_between(at(m.x - 0.26), at(m.x - 0.2), 0.3, 0.3, sides);
            }
        });
    });
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
