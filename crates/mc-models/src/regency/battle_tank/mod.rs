//! The Regency battle tank, the Glaive (`regency_t2_tank`): a low armoured machine with a
//! turret carrying a Pinched-plasmeric Cannon, the Halberd's gun built for a tank. Its
//! projectors reach past the bore's mouth and gather the plasma into a ball between them;
//! the `muzzle` is the middle of that ball.
//!
//! - The hull (`hull`): a pointed armoured core on gravity lift with an armoured sponson
//!   either side where an ARC tank has its tracks, swept plates lapped back along each and
//!   drawn out into points, a lift bell under each end of each sponson and one under the
//!   core, bronze workings in the gaps.
//! - The turret (`turret`): a faceted block on a bronze race, swept plates down its
//!   flanks, the gun's trunnion in its face; the claw (three plated prongs round a short
//!   barrel, one over it and two below either side, red emitters at their tips turned in
//!   on the charge).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, dark seams, dark bronze on the
//! machinery, red optics and emitters. No violet: it does not build.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells. The turret turns about the unit's middle; the gun pitches about its trunnion
//! (`rig::ARM_GUN`) and its barrel recoils (the prongs hold). The numbers match
//! `regency_t2_tank` in `data/factions/regency/units/land_t2.ron`.

mod hull;
mod turret;

#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::kit::{dark_plate, seam, v3};

/// The gun's trunnion and the middle of its charge: the unit file's weapon `pivot` and
/// `muzzle`.
const PIVOT: Vec3 = Vec3::new(1.4, 0.0, 3.4);
const MUZZLE: Vec3 = Vec3::new(8.2, 0.0, 3.4);
/// The turret race: the turret turns about the unit's middle at this height.
const DECK: f32 = 2.4;
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 1.2;

pub(super) fn battle_tank(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(PIVOT);
    b.set_recoil(PIVOT, MUZZLE, 0.5);
    b.set_hover();
    b.set_dust_line(0.8);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull::hull(b);
    b.with_part(part::TURRET, |b| {
        turret::turret(b);
        b.with_limb(rig::ARM_GUN, turret::gun);
    });
}

/// Far off: a wedge of a hull with the team colour on its back, a block of a turret and
/// the gun as a bar over the claw's two lower prongs, which still pitch.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(0.0, 0.0, 0.5),
        Vec2::new(11.0, 6.4),
        Vec2::new(9.0, 4.6),
        DECK - 0.5,
        Vec2::new(-0.3, 0.0),
    );
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[
            v3(4.5, 0.0, 0.45),
            v3(-4.5, -2.8, 0.45),
            v3(-4.5, 2.8, 0.45),
        ]);
    });
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.block(v3(-3.2, -1.8, DECK), v3(2.4, 1.8, 4.2));
        b.paint(TEAM);
        b.face(&[
            v3(-1.2, 0.0, 4.21),
            v3(-2.8, 1.0, 4.21),
            v3(-2.8, -1.0, 4.21),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                PIVOT + Vec3::X * 0.8,
                v3(6.6, 0.0, PIVOT.z),
                Vec2::splat(1.0),
                Vec2::splat(0.7),
            );
            for y in [-0.82f32, 0.82] {
                b.cylinder_between(
                    v3(4.0, y, PIVOT.z - 0.47),
                    v3(MUZZLE.x + 0.4, y, PIVOT.z - 0.47),
                    0.35,
                    0.25,
                    3,
                );
            }
        });
    });
}
