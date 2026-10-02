//! Amphibious emplacements and tracked AA share recognisable weapon assemblies.
use super::parts::*;
mod flak_battery;
mod floats;
mod gnat;
mod skyguard;
mod squall;
use crate::builder::MeshBuilder;
use crate::{material::*, part, rig};

/// The Sparrow's turret: a collar, a receiver block and the rotary gun on the trunnion.
fn mount(b: &mut MeshBuilder, z: f32) {
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, z - 0.9), b.sides(10), 2.2, 1.8, 1.1);
        b.paint(PLATING);
        b.cuboid(v3(-0.5, 0.0, z + 0.35), v3(3.0, 2.5, 1.3));
        b.with_limb(rig::ARM_GUN, |b| rotary(b, z, 5.5));
        team_panel(b, v3(-0.5, 0.0, z + 1.02), v2(1.4, 1.3));
    });
}
/// The Sparrow's gun (and the Fulgur's): a receiver on the trunnion, six barrels in clamps that spin
/// about the bore line, a drum feeding it from the left and the ejection port on
/// the right, where the casings come out.
pub(super) fn rotary(b: &mut MeshBuilder, z: f32, end: f32) {
    let axis = v3(0.0, 0.0, z);
    b.paint(ACCENT);
    b.cylinder_between(v3(0.3, 0.0, z), v3(1.45, 0.0, z), 0.56, 0.5, b.sides(10));
    if b.fine() {
        b.paint(PLATING_DARK);
        b.cuboid(v3(0.95, -0.5, z + 0.1), v3(0.55, 0.12, 0.3));
        // The feed: a drum on the turret's left side and the chute to the receiver.
        b.paint(PLATING);
        b.cylinder_between(
            v3(-0.4, 1.3, z - 0.2),
            v3(-0.4, 2.05, z - 0.2),
            0.7,
            0.7,
            b.sides(10),
        );
        b.paint(METAL);
        b.beam(
            v3(-0.1, 1.35, z),
            v3(0.8, 0.4, z + 0.05),
            v2(0.3, 0.26),
            v2(0.26, 0.22),
        );
    }
    b.with_spin(axis, |b| {
        b.paint(METAL);
        if b.fine() {
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                let off = v3(0.0, a.cos() * 0.3, a.sin() * 0.3);
                b.cylinder_between(
                    v3(1.45, 0.0, z) + off,
                    v3(end, 0.0, z) + off * 0.85,
                    0.1,
                    0.085,
                    6,
                );
            }
            b.paint(PLATING_DARK);
            for (x, r) in [(1.5, 0.47), (3.2, 0.43), (end - 0.35, 0.4)] {
                b.cylinder_between(v3(x, 0.0, z), v3(x + 0.22, 0.0, z), r, r, b.sides(8));
            }
        } else {
            // From further off the barrels read as one fat tube.
            b.cylinder_between(v3(1.45, 0.0, z), v3(end, 0.0, z), 0.42, 0.36, 6);
        }
    });
}
fn platform(b: &mut MeshBuilder, radius: f32) {
    b.paint(PLATING_DARK);
    b.plate(v3(0.0, 0.0, 0.1), v2(radius * 1.7, radius * 1.7), 1.0, 1.2);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.chamfered_box(
            v3(0.0, radius * 0.62, 1.1),
            v3(radius * 1.5, radius * 0.35, 2.0),
            0.5,
        );
        if !b.coarse() {
            b.paint(METAL);
            b.cuboid(v3(0.0, radius * 0.62, 2.15), v3(radius, 0.5, 0.18));
        }
    });
}
/// Sparrow: the tech 1 AA gun, on a pontoon at sea (`floats`).
pub(super) fn gun(b: &mut MeshBuilder, _: u8) {
    floats::pontoon(b, SPARROW_PAD);
    if !b.fine() {
        reduced_aa(b, 6.0, 6.0);
        return;
    }
    platform(b, 5.5);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 1.0), b.sides(8), 1.5, 1.2, 4.3);
    mount(b, 6.0);
}
/// Barrage: the tech 2 flak battery, on a pontoon at sea.
pub(super) fn flak_battery(b: &mut MeshBuilder, _: u8) {
    flak_battery::build(b);
    floats::pontoon(b, BARRAGE_PAD);
}
/// Skyguard: the tech 3 SAM site (`skyguard.rs`), on a pontoon at sea.
pub(super) fn sam(b: &mut MeshBuilder, _: u8) {
    skyguard::build(b);
    floats::pontoon(b, skyguard::PAD);
}
/// Half the Sparrow's pad and the Barrage's (`platform`).
const SPARROW_PAD: f32 = 5.5 * 0.85;
const BARRAGE_PAD: f32 = 9.5 * 0.85;
/// Tracked AA by tier: the Gnat, then the Squall.
pub(super) fn mobile(b: &mut MeshBuilder, tech: u8) {
    if tech == 1 {
        gnat::build(b);
    } else {
        squall::build(b);
    }
}

fn reduced_aa(b: &mut MeshBuilder, r: f32, z: f32) {
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, 0.8), v3(r * 1.6, r * 1.5, 1.6));
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, z * 0.5), v3(r * 0.6, r * 0.6, z));
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.with_part(part::TURRET, |b| {
        b.paint(METAL);
        b.cuboid_open(v3(r * 0.6, 0.0, z), v3(r * 1.1, r * 0.35, 0.4));
    });
    b.paint(TEAM);
    b.face(&[
        v3(-r * 0.2, -r * 0.2, z + 0.02),
        v3(r * 0.2, -r * 0.2, z + 0.02),
        v3(r * 0.2, r * 0.2, z + 0.02),
        v3(-r * 0.2, r * 0.2, z + 0.02),
    ]);
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, r * 0.6, 1.5), v3(r * 1.5, r * 0.25, 1.0));
        });
    }
}
