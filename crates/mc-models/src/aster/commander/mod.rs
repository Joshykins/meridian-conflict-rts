//! The ARC Commander (`commander`).
//!
//! Black-and-white armour: white plates (`PLATING`) sit over a black underlayer
//! (`ACCENT`), so the black is the gap between the plates, not a graphite hull. Grey
//! (`METAL`) is joints and pistons only. Every plate is seated on a face of the piece it
//! is fixed to, and what lights is drawn on the surface (`pattern`), not modelled. The
//! right arm carries the guns, the left is the construction projector, and everything
//! that builds is marked in amber. The bare commander has nothing on its back.
//!
//! Legs and feet are `LOCOMOTION`, the pelvis is `HULL`, and everything above the waist
//! ring (torso, arms, head) is the `TURRET`, twisting about the model's z axis so
//! arm-mounted muzzles track the sim's muzzle offsets.
//!
//! Everything a refit adds is tagged with its module's key from the unit file
//! (`b.module`): the model carries every module's pieces and the shader shows the ones
//! the unit has fitted. The engineering suites stay on the left arm and leg; the back
//! takes the formation engine or the shield pack; the right shoulder a flak cannon or a
//! howitzer; the left shoulder a second projector that folds out to build.
//!
//! The points below are the unit file's (`data/factions/aster/units/command.ron`) over
//! 1.6: the model is authored at 6.5 m, the unit is 10.4.

mod arms;
mod body;
mod flak;
mod head;
mod legs;
mod refits;

use glam::{Vec2, Vec3};

pub(crate) use arms::RAIL;
use flak::{MUZZLE as AA_MUZZLE, TRUNNION};

use crate::aster::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

/// Where the machine gun ends and where the build beam leaves.
const COMMANDER_MUZZLE: Vec3 = Vec3::new(5.5, -3.0, 9.7);
const COMMANDER_EMITTER: Vec3 = Vec3::new(4.8, 3.0, 9.7);
/// Muzzles of the right arm's main guns: they take the forearm's axis, and the machine gun
/// moves to a pod slung along the outside of the forearm (`SLUNG_AXIS` to `SLUNG_MUZZLE`).
const COMMANDER_CANNON: Vec3 = Vec3::new(6.8, -3.0, 9.7);
const COMMANDER_RAIL: Vec3 = Vec3::new(7.8, -3.0, 9.7);
/// The rail cannon's rails (`rail_gun`): where they leave the capacitor bank, each bar's
/// width and height, and the slot between them (`rail_gun` opens it to 1.5 bar widths).
const RAIL_BREECH: Vec3 = Vec3::new(1.4, -3.0, 9.7);
const RAIL_BARS: Vec2 = Vec2::new(0.24, 0.5);
const RAIL_SLOT: f32 = 0.22;
/// The right elbow the guns pitch about: the unit file's `pivot`, over 1.6.
const GUN_ELBOW: Vec3 = Vec3::new(-0.3, -3.0, 9.8);
const SLUNG_AXIS: Vec3 = Vec3::new(0.9, -4.15, 9.35);
const SLUNG_MUZZLE: Vec3 = Vec3::new(3.9, -4.15, 9.35);
/// Where the howitzer's tube ends, ahead of the trunnion it shares with the flak cannon
/// (`flak::TRUNNION`): the model has one mount.
const HOWITZER_MUZZLE: Vec3 = Vec3::new(TRUNNION.x + 4.9, TRUNNION.y, TRUNNION.z);
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

/// How far out each arm hangs, and the height of the forearms' axis.
const SHOULDER_Y: f32 = 3.0;
const ARM_Z: f32 = COMMANDER_MUZZLE.z;

/// Half the chest's width across, and how far forward of the lean frame its breast stands.
const CHEST_HALF: f32 = 2.0;
const PROW: f32 = 0.15;
/// The limbs' girth, times the lean frame's.
const GIRTH: f32 = 1.15;
/// The foot's length behind the ankle and ahead of it, and its width.
const FOOT: (f32, f32, f32) = (1.4, 2.7, 2.3);

pub(super) fn commander(b: &mut MeshBuilder, _tech: u8) {
    legs::legs(b);
    let (gun_y, tool_y) = (COMMANDER_MUZZLE.y, COMMANDER_EMITTER.y);
    b.set_turret_pivot(v3(0.0, 0.0, 9.2));
    // The elbows: both forearms pitch about them to point at what they shoot or build.
    b.set_arm_pivot(v3(-0.3, tool_y, ARM_Z + 0.1));
    if b.coarse() {
        b.with_part(part::TURRET, coarse);
        return;
    }
    body::pelvis(b);
    b.with_part(part::TURRET, |b| {
        body::torso(b);
        head::head(b, v3(0.3, 0.0, 13.2));
        body::shoulders(b);
        arms::gun_arm(b, gun_y, ARM_Z);
        arms::tool_arm(b, tool_y, ARM_Z);
        refits::back(b);
        refits::shoulders(b);
    });
}

/// The far level: the torso and pauldrons as blocks, each arm a bar, each module's
/// silhouette.
fn coarse(b: &mut MeshBuilder) {
    let (gun_y, tool_y, arm_z) = (COMMANDER_MUZZLE.y, COMMANDER_EMITTER.y, ARM_Z);
    // The torso flares out to the pauldrons' width at the top: from this far that is the
    // shoulders.
    b.paint(ACCENT);
    b.frustum_open(
        v3(-0.1, 0.0, 9.0),
        v2(2.8, 3.6),
        v2(2.6 + PROW, 2.0 * (SHOULDER_Y + 0.95)),
        4.25,
        v2(PROW * 0.5, 0.0),
    );
    b.mirror_y(|b| team_panel(b, v3(-0.1, SHOULDER_Y, 13.25), v2(1.2, 0.8)));
    b.paint(METAL);
    b.with_limb(rig::ARM_GUN, |b| {
        b.beam(
            v3(-1.4, gun_y, arm_z),
            COMMANDER_MUZZLE,
            v2(1.4, 1.4),
            v2(0.8, 0.65),
        );
        b.module("cannon", 0.3, |b| {
            b.beam(SLUNG_AXIS, SLUNG_MUZZLE, v2(0.5, 0.5), v2(0.4, 0.4));
            b.until("railgun", |b| {
                b.paint(PLATING);
                b.beam(
                    v3(1.8, gun_y, arm_z),
                    COMMANDER_CANNON,
                    v2(0.9, 0.9),
                    v2(0.7, 0.7),
                );
            })
        });
        b.module("railgun", 0.3, |b| {
            b.paint(METAL);
            b.beam(
                v3(1.6, gun_y, arm_z),
                COMMANDER_RAIL,
                v2(0.85, 0.85),
                v2(0.75, 0.75),
            );
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
    b.module("shield_2", 0.3, |b| {
        b.paint(ACCENT);
        b.block(v3(-3.1, -1.4, 15.8), v3(-2.7, 1.4, 16.2));
    });
    b.module("aa", 0.3, |b| {
        b.with_mount(TRUNNION, 0.0, |b| {
            b.paint(PLATING);
            b.block(
                TRUNNION + v3(-0.6, -0.75, -0.55),
                TRUNNION + v3(0.8, 0.75, 0.5),
            );
            b.paint(METAL);
            b.beam(
                TRUNNION + Vec3::X * 0.8,
                v3(AA_MUZZLE, TRUNNION.y, TRUNNION.z),
                v2(0.8, 0.3),
                v2(0.8, 0.3),
            );
        })
    });
    b.module("artillery", 0.3, |b| {
        b.with_mount(TRUNNION, 0.0, |b| {
            b.paint(PLATING);
            b.beam(
                TRUNNION - Vec3::X * 1.2,
                HOWITZER_MUZZLE,
                v2(1.2, 1.2),
                v2(0.55, 0.55),
            );
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
}
