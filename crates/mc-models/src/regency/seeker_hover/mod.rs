//! The Regency mobile anti-air, the Vane (`regency_t2_mobile_aa`): a craft on gravity lift,
//! no legs, so it crosses water as it crosses land, and on its back a Gravitic Seeker
//! Battery.
//!
//! - The hull (`hull`): an armoured arrowhead, a swept sponson either side lapped in
//!   plates drawn back into points, a lift bell under each end of each sponson and one
//!   under the body, a sensor head with red optics at the nose, and a plated plinth over
//!   its middle that the battery turns on.
//! - The battery (`battery`): a narrow plated core with a red sensor eye, and either side
//!   of it on one trunnion a bank of three cradles, each holding a charge of plasma in
//!   gravity containment between two red-tipped projector claws: the seekers, each let go
//!   in turn from its cradle. The muzzles are the charges.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, dark seams, dark bronze on the
//! machinery, red optics and charges. No violet: it does not build.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells. The battery turns about the unit's middle and its banks pitch together about
//! their trunnion (`rig::ARM_GUN`). The numbers match `regency_t2_mobile_aa` in
//! `data/factions/regency/units/land_t2.ron`.

mod battery;
mod hull;

#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::kit::{dark_plate, seam, v3};

/// The banks' trunnion: the unit file's weapon `pivot`.
const PIVOT: Vec3 = Vec3::new(0.3, 0.0, 4.3);
/// The race the battery turns on, about the unit's middle.
const RACE: f32 = 3.4;
/// The charges in their cradles, relative to the trunnion: a bank either side, three
/// high. Plus `PIVOT`, the unit file's `muzzles`.
const CELL_X: f32 = 1.8;
const CELL_Y: f32 = 1.35;
const CELL_Z: [f32; 3] = [0.55, 0.0, -0.55];

/// The muzzles in the unit file's order: top left, top right, then down.
#[cfg(test)]
fn muzzles() -> Vec<Vec3> {
    CELL_Z
        .iter()
        .flat_map(|&z| [CELL_Y, -CELL_Y].map(|y| PIVOT + v3(CELL_X, y, z)))
        .collect()
}

pub(super) fn seeker_hover(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, RACE));
    b.set_arm_pivot(PIVOT);
    b.set_hover();
    b.set_dust_line(0.7);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull::hull(b);
    b.with_part(part::TURRET, |b| {
        battery::core(b);
        b.with_limb(rig::ARM_GUN, battery::banks);
    });
}

/// Far off: a wedge of a hull with the team colour on its back, the lift under it, and
/// each bank a block whose face reaches its three charges.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.frustum(
        v3(0.0, 0.0, 0.6),
        Vec2::new(8.4, 5.6),
        Vec2::new(5.0, 2.8),
        RACE - 0.6,
        Vec2::new(-0.3, 0.0),
    );
    b.paint(TEAM);
    b.face(&[
        v3(-0.4, 0.0, RACE + 0.01),
        v3(-1.6, 0.6, RACE + 0.01),
        v3(-1.6, -0.6, RACE + 0.01),
    ]);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[
            v3(3.0, 0.0, 0.45),
            v3(-3.2, -2.4, 0.45),
            v3(-3.2, 2.4, 0.45),
        ]);
    });
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            b.mirror_y(|b| {
                dark_plate(b);
                let c = PIVOT + v3(0.6, CELL_Y, 0.0);
                b.block(c - v3(1.2, 0.5, 0.85), c + v3(1.0, 0.5, 0.85));
                b.paint(GLOW_LASER);
                let at = |z: f32| PIVOT + v3(CELL_X + 0.1, CELL_Y, z);
                b.face(&[
                    at(CELL_Z[0]),
                    at(CELL_Z[2]),
                    at(CELL_Z[1]) + v3(0.15, 0.0, 0.0),
                ]);
            });
        });
    });
}
