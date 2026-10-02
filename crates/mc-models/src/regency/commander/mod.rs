//! The Regency commander, the Exarch: a war-machine tyrant, tall and long-limbed, every
//! mass a faceted solid. Heavy thighs taper to armoured knees, the shins
//! flare into great greaves over clawed feet; a narrow waist of banded bronze carries a
//! deep chest under a keeled prow with courses of plates laid back from it like the
//! Anvil's wings; high pauldrons sweep back into
//! blades above a long narrow head sunk between them. A telescoping fusion cannon is its
//! right forearm, a taloned hand that builds its left. Plates lap back over bronze
//! machinery the way the Anvil's do (docs/STYLE.md "The Regency look").
//!
//! Rig (`regency_commander` in `data/factions/regency/units/command.ron` has these numbers):
//! - `part::LOCOMOTION` legs, knees forward: the thigh, shin and boot ride `rig::THIGH`,
//!   `SHIN` and `FOOT` about `HIP`, `KNEE` and `ANKLE`.
//! - `part::HULL` the pelvis; `part::TURRET` everything above the waist ring (`WAIST`),
//!   turning to face what it shoots or builds. The head idles about `NECK`.
//! - The forearms pitch about the elbows (`ELBOW` and its mirror): the cannon on the right
//!   (`rig::ARM_GUN`, muzzle `MUZZLE`, its sleeves kicking back), the hand on the left
//!   (`rig::ARM_TOOL`), the build beam leaving the violet emitter in its palm (`EMITTER`).
//!   Its talons open round it while it builds.
//! - Refits (`b.module`): the engineering suites go on the hand's forearm. Suite II adds
//!   twin emitters rocking on it and bronze feed canisters; Suite III a lance that runs
//!   out of the palm while it builds (`LANCE_TIP`, the unit file's `arm_emitter` when run
//!   out), a ring of emitters round the wrist and a feed drum over the elbow. The back
//!   carries the Personal Shield (`back`).

mod arms;
mod back;
mod body;
mod chest;
pub(super) mod form;
mod head;
mod legs;
#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::kit::{dark_plate, v3};

// ---- rig ----------------------------------------------------------------------------

/// The left leg's joints at rest: hip, knee (forward) and ankle.
const HIP: Vec3 = Vec3::new(0.0, 2.3, 11.6);
const KNEE: Vec3 = Vec3::new(1.0, 2.55, 7.0);
const ANKLE: Vec3 = Vec3::new(-0.2, 2.75, 1.7);
const STRIDE: f32 = 16.0;
const STANCE: f32 = 0.48;
const LIFT: f32 = 2.0;
const CROUCH: f32 = 0.8;
/// The sole: behind the ankle (the heel), ahead of it (the toe cap), across.
const FOOT: (f32, f32, f32) = (-1.8, 3.3, 2.7);
/// The ring the torso turns on.
const WAIST: f32 = 12.8;
/// The left shoulder joint the upper arm hangs from.
const SHOULDER: Vec3 = Vec3::new(-0.5, 4.3, 19.4);
/// The left elbow (the hand's); the cannon's is its mirror.
const ELBOW: Vec3 = Vec3::new(-0.5, 5.2, 14.6);
/// The cannon's muzzle, the hand's emitter, Suite III's lance tip at rest.
const MUZZLE: Vec3 = Vec3::new(9.6, -5.2, 14.6);
const EMITTER: Vec3 = Vec3::new(5.2, 5.2, 14.6);
const LANCE_TIP: f32 = 6.9;
/// How far the lance runs out while it builds (`rig::WORK_EXTEND`).
#[cfg(test)]
const LANCE_RUN: f32 = 1.28;
/// Where the head turns while the commander stands idle.
const NECK: Vec3 = Vec3::new(1.0, 0.0, 20.1);

pub(super) fn commander(b: &mut MeshBuilder, _tech: u8) {
    b.set_legs(HIP, KNEE, ANKLE, STRIDE, STANCE, LIFT);
    b.set_walk_crouch(CROUCH);
    b.set_foot(FOOT.0, FOOT.1, FOOT.2);
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_arm_pivot(ELBOW);
    b.set_dust_line(1.5);
    if b.coarse() {
        coarse(b);
        return;
    }
    b.mirror_y(legs::leg);
    body::pelvis(b);
    b.with_part(part::TURRET, |b| {
        body::torso(b);
        head::head(b);
        arms::cannon(b);
        arms::claw(b);
        back::shield(b);
    });
}

/// From far off: each leg bone a bar still posed by the rig, the foot a wedge, the torso
/// a wedge widening to the shoulders with the team colour on top, the forearms two bars
/// that still pitch.
fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.mirror_y(|b| {
            dark_plate(b);
            b.with_limb(rig::THIGH, |b| bar(b, HIP, KNEE, 1.2, false));
            b.with_limb(rig::SHIN, |b| bar(b, KNEE, ANKLE, 1.3, true));
            b.with_limb(rig::FOOT, |b| {
                let w = FOOT.2 * 0.5;
                b.face(&[
                    v3(ANKLE.x + FOOT.1, ANKLE.y, 0.02),
                    v3(ANKLE.x + FOOT.0, ANKLE.y + w, 1.0),
                    v3(ANKLE.x + FOOT.0, ANKLE.y - w, 1.0),
                ]);
            });
        });
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.frustum_open(
            v3(-0.2, 0.0, WAIST - 1.2),
            Vec2::new(3.0, 3.4),
            Vec2::new(4.8, 11.0),
            20.6 - WAIST + 1.2,
            Vec2::new(-0.2, 0.0),
        );
        b.paint(TEAM);
        b.face(&[
            v3(1.0, -1.5, 20.65),
            v3(1.0, 1.5, 20.65),
            v3(-1.6, 1.5, 20.65),
            v3(-1.6, -1.5, 20.65),
        ]);
        dark_plate(b);
        b.face(&[v3(1.6, -0.5, 20.6), v3(-0.6, 0.0, 23.0), v3(1.6, 0.5, 20.6)]);
        b.with_limb(rig::ARM_GUN, |b| {
            bar(b, v3(-1.0, MUZZLE.y, MUZZLE.z), MUZZLE, 0.6, false)
        });
        b.with_limb(rig::ARM_TOOL, |b| {
            bar(
                b,
                v3(-1.0, EMITTER.y, EMITTER.z),
                v3(7.6, EMITTER.y, EMITTER.z),
                0.9,
                false,
            );
            b.paint(GLOW_VIOLET);
            let (y, z) = (EMITTER.y, EMITTER.z + 0.9);
            b.module("eng_2", 0.3, |b| {
                b.face(&[v3(0.2, y - 0.4, z), v3(2.2, y, z), v3(0.2, y + 0.4, z)])
            });
            b.module("eng_3", 0.3, |b| {
                b.face(&[v3(2.4, y - 0.3, z), v3(4.4, y, z), v3(2.4, y + 0.3, z)])
            });
        });
        // The Personal Shield's star on the back, a lit point far off.
        b.module("shield", 0.0, |b| {
            b.paint(GLOW_PRISM);
            b.face(&[
                v3(-3.3, -0.5, 20.8),
                v3(-3.6, 0.0, 22.2),
                v3(-3.3, 0.5, 20.8),
            ]);
        });
    });
}

/// A far-off bar from `a` to `c`, `w` either side of its ridge, narrowing to 0.4 of that
/// at `c`: three sides when `closed`, else a roof of two faces turned up and out.
fn bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, w: f32, closed: bool) {
    let along = (c - a).normalize();
    let across = (Vec3::Y - along * Vec3::Y.dot(along)).normalize();
    let up = along.cross(across);
    let up = if up.z < 0.0 { -up } else { up };
    let (wa, wc) = (w, w * 0.4);
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
