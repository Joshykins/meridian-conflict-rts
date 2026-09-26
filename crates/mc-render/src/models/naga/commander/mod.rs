//! The Naga commander, the Sovereign: a top-heavy reverse-kneed walker. Very broad
//! shoulders under stacked pauldrons, a narrow tall head with swept horn plates and one
//! red optic, a long plasmeric cannon for a right forearm and a clawed left hand that
//! builds, digitigrade legs on talon feet. Layered dark plates sweep back over bronze
//! machinery (ribs, rams, joint drums), and every spike is a plate's trailing edge
//! (docs/STYLE.md "The Naga look"). It is asymmetric only in its arms.
//!
//! Rig (`naga_commander` in `data/factions/naga/units/command.ron` has these numbers):
//! - `part::LOCOMOTION` legs, reverse-kneed: hip, knee, hock and ankle (`HIP`, `KNEE`,
//!   `HOCK`, `ANKLE`), posed by `entity.wgsl` `hock_leg`; the thigh, shin, tarsus and foot
//!   ride `rig::THIGH`, `SHIN`, `TARSUS` and `FOOT`. Long high steps (`STRIDE`, `LIFT`).
//! - `part::HULL` pelvis.
//! - `part::TURRET` everything above the waist ring (`WAIST`), turning to face what it shoots
//!   or builds. The head idles about `NECK`. The forearms pitch about the elbows (`ELBOW` and
//!   its mirror): the cannon on the right (`rig::ARM_GUN`, muzzle `MUZZLE`, its barrel kicking
//!   back), the claw on the left (`rig::ARM_TOOL`), the build beam leaving the violet emitter
//!   in its palm (`EMITTER`). The claw's fingers open round it while it builds.
//!
//! Refits (`b.module`): the engineering suites go on the claw arm as more nanite kit.
//! Suite II adds twin emitters rocking on the forearm and bronze feed canisters along it;
//! Suite III a fabrication lance that runs out of the palm while it builds (`LANCE_TIP`,
//! the unit file's `arm_emitter` when run out), a ring of emitters round the wrist and a
//! feed drum on the shoulder.

mod arms;
mod legs;
#[cfg(test)]
mod tests;
mod torso;

use glam::Vec3;

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::kit::{dark_plate, v3};

// ---- rig ----------------------------------------------------------------------------

/// Left leg joints at rest (the right is the mirror): the thigh runs forward and down to
/// the knee, the long shin back and down to the hock, the tarsus forward and down to the
/// ankle over the talons.
const HIP: Vec3 = Vec3::new(-0.4, 2.5, 11.2);
const KNEE: Vec3 = Vec3::new(2.3, 2.9, 8.2);
const HOCK: Vec3 = Vec3::new(-1.7, 3.2, 3.8);
const ANKLE: Vec3 = Vec3::new(0.1, 3.4, 1.3);
/// How much of the leg's swing the tarsus leans with (`MeshBuilder::set_hock`).
const HOCK_FOLLOW: f32 = 0.5;
/// Ground to a full cycle, share of it a foot is down (under half: a moment in the air
/// between steps), how high a foot lifts, how far the hips settle in full stride.
const STRIDE: f32 = 16.0;
const STANCE: f32 = 0.48;
const LIFT: f32 = 1.9;
const CROUCH: f32 = 1.0;
/// The sole: metres behind the ankle (the spur), ahead of it (the talons' tips), across.
const FOOT: (f32, f32, f32) = (-1.9, 3.3, 2.4);
/// The ring the torso turns on.
const WAIST: f32 = 12.4;
/// The left elbow (the claw's); the cannon's is its mirror. Both forearms' axes run
/// forward from it at its height.
const ELBOW: Vec3 = Vec3::new(-0.5, 4.8, 13.9);
/// The cannon's muzzle: the unit file's weapon `muzzle`.
const MUZZLE: Vec3 = Vec3::new(8.9, -4.8, 13.9);
/// Where the build beam leaves the claw's palm emitter: the unit file's `builder.arm.emitter`.
const EMITTER: Vec3 = Vec3::new(4.7, 4.8, 13.9);
/// Suite III's lance tip at rest; it runs out `LANCE_RUN` while it builds
/// (`rig::WORK_EXTEND`), to the unit file's `arm_emitter`.
const LANCE_TIP: f32 = 6.4;
#[cfg(test)]
const LANCE_RUN: f32 = 1.28;
/// Where the head turns while the commander stands idle.
const NECK: Vec3 = Vec3::new(1.1, 0.0, 18.8);

pub(super) fn commander(b: &mut MeshBuilder, _tech: u8) {
    b.set_legs(HIP, KNEE, ANKLE, STRIDE, STANCE, LIFT);
    b.set_hock(HOCK, HOCK_FOLLOW);
    b.set_walk_crouch(CROUCH);
    b.set_foot(FOOT.0, FOOT.1, FOOT.2);
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_arm_pivot(ELBOW);
    b.set_dust_line(5.0);
    if b.coarse() {
        coarse(b);
        return;
    }
    b.mirror_y(legs::leg);
    torso::pelvis(b);
    b.with_part(part::TURRET, |b| {
        torso::torso(b);
        arms::cannon(b);
        arms::claw(b);
    });
}

/// From far off: each leg bone a bar still posed by the rig, the torso a wedge widening
/// to the shoulders with the team colour on top, the forearms two bars that still pitch.
fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.mirror_y(|b| {
            dark_plate(b);
            b.with_limb(rig::THIGH, |b| {
                bar(b, HIP + Vec3::Z * 0.4, KNEE, 0.7, false)
            });
            b.with_limb(rig::SHIN, |b| bar(b, KNEE, HOCK, 0.6, true));
            b.with_limb(rig::TARSUS, |b| {
                bar(b, HOCK, v3(ANKLE.x + 1.2, ANKLE.y, 0.05), 0.5, false)
            });
        });
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum_open(
            v3(-0.2, 0.0, WAIST),
            glam::Vec2::new(3.6, 3.4),
            glam::Vec2::new(3.6, 11.4),
            20.2 - WAIST,
            glam::Vec2::new(-0.3, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.8, -1.4, 20.25),
            v3(0.8, 1.4, 20.25),
            v3(-1.6, 1.4, 20.25),
            v3(-1.6, -1.4, 20.25),
        ]);
        dark_plate(b);
        b.with_limb(rig::ARM_GUN, |b| {
            bar(b, v3(-1.0, MUZZLE.y, MUZZLE.z), MUZZLE, 0.8, false)
        });
        b.with_limb(rig::ARM_TOOL, |b| {
            bar(
                b,
                v3(-1.0, EMITTER.y, EMITTER.z),
                v3(6.6, EMITTER.y, EMITTER.z),
                0.7,
                false,
            );
            // The suites as a violet fleck each, so a refit still shows from far off.
            b.paint(GLOW_VIOLET);
            let (y, z) = (EMITTER.y, EMITTER.z + 0.9);
            b.module("eng_2", 0.3, |b| {
                b.face(&[v3(0.2, y - 0.4, z), v3(2.2, y, z), v3(0.2, y + 0.4, z)])
            });
            b.module("eng_3", 0.3, |b| {
                b.face(&[v3(2.4, y - 0.3, z), v3(4.4, y, z), v3(2.4, y + 0.3, z)])
            });
        });
    });
}

/// A far-off bar from `a` to `c`, `w` either side of its ridge, narrowing to a fifth at
/// `c`: three sides when `closed`, else a roof of two faces turned up and out.
fn bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32, closed: bool) {
    let along = (c - a).normalize();
    let across = (Vec3::Y - along * Vec3::Y.dot(along)).normalize();
    let up = along.cross(across);
    let up = if up.z < 0.0 { -up } else { up };
    let (wa, wc) = (w, w * 0.2);
    if closed {
        let ring = |p: Vec3, w: f32| vec![p + across * w, p + up * w, p - across * w];
        b.loft(&[ring(a, wa), ring(c, wc)], false, false);
        return;
    }
    for side in [across, -across] {
        let mut quad = [a + side * wa, c + side * wc, c + up * wc, a + up * wa];
        let normal = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
        if normal.dot(side + up) < 0.0 {
            quad.reverse();
        }
        b.face(&quad);
    }
}
