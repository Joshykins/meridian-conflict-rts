//! The Regency's tech 3 mobile Pinch-fusion guns (docs/STYLE.md "The Regency suite"): the
//! Sunspear's kind of gun taken off its keep and put on a body that moves.
//!
//! - `regency_skyspear` (`skyspear`): the mobile anti-spaceship gun. The Sunspear's
//!   rails gun drawn small and raised to the sky, against warships in the upper air.
//! - `regency_fusion_howitzer` (`howitzer`): heavy artillery. A split pinch bore that
//!   charges for a long beat and lobs one great fusion shot.
//!
//! Each gathers its charge in front of its bore between projectors, as the Sunspear does,
//! and its `muzzle` is the middle of that charge. Each carries a caged fusion core on its
//! turret's back (the Sunspear's, smaller) whose gimbal rings spin up through the charge
//! (`rig::CHARGE_GEAR_MASK`).
//!
//! Both ride the same tracked body under lapped armour skirts (`chassis`).

mod chassis;
mod howitzer;
mod skyspear;
#[cfg(test)]
mod tests;

use glam::{Affine3A, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::kit::{dark_plate, v3};
use super::machine::{armour, collar, hoop, Frame};
use super::turrets::sunspear;

pub(super) use howitzer::{HEIGHT as HOWITZER_HEIGHT, RADIUS as HOWITZER_RADIUS};
pub(super) use skyspear::{HEIGHT as SKYSPEAR_HEIGHT, RADIUS as SKYSPEAR_RADIUS};

/// The anti-spaceship gun: the Sunspear's rails gun raised to the sky on a tracked body.
pub(super) fn skyspear(b: &mut MeshBuilder, _tech: u8) {
    skyspear::draw(b);
}

/// The howitzer: a split pinch bore on a tracked body.
pub(super) fn howitzer(b: &mut MeshBuilder, _tech: u8) {
    howitzer::draw(b);
}

/// A gun's line: its trunnion, how far its bore is raised at rest, and the bore's length
/// out to the middle of its charge.
#[derive(Clone, Copy, Debug)]
struct Line {
    pivot: Vec3,
    pitch_deg: f32,
    len: f32,
}

impl Line {
    fn pitch(&self) -> f32 {
        self.pitch_deg.to_radians()
    }

    /// The middle of the charge: the unit file's `muzzle`.
    fn muzzle(&self) -> Vec3 {
        let a = self.pitch();
        self.pivot + Vec3::new(a.cos(), 0.0, a.sin()) * self.len
    }

    /// Records the turret's yaw axis (over the unit's middle at `deck`), the gun's
    /// trunnion and its recoil.
    fn rig(&self, b: &mut MeshBuilder, deck: f32, travel: f32) {
        b.set_turret_pivot(v3(0.0, 0.0, deck));
        b.set_arm_pivot(self.pivot);
        b.set_recoil(self.pivot, self.muzzle(), travel);
    }

    /// Draws `f` in the gun's frame, on the pitching arm: the origin at the trunnion, +x
    /// down the bore.
    fn gun(&self, b: &mut MeshBuilder, f: impl FnOnce(&mut MeshBuilder)) {
        b.with_limb(rig::ARM_GUN, |b| b.pitched(self.pivot, self.pitch(), f));
    }
}

/// A turret's hull in plan: `half` the outline (x, y >= 0) from the front round to the
/// back, mirrored across, from `z0` up to `z1`, its top edge drawn in.
fn turret_hull(b: &mut MeshBuilder, half: &[[f32; 2]], z0: f32, z1: f32) {
    let outline: Vec<[f32; 2]> = half
        .iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect();
    let ring = |z: f32, k: f32| -> Vec<Vec3> {
        outline.iter().map(|&[x, y]| v3(x * k, y * k, z)).collect()
    };
    dark_plate(b);
    let h = z1 - z0;
    b.with_facets(|b| {
        b.loft(
            &[ring(z0, 0.96), ring(z0 + h * 0.75, 1.0), ring(z1, 0.88)],
            true,
            true,
        )
    });
    // The bronze race it turns on.
    super::kit::metal(b);
    let segs = b.sides(20);
    let r = half.iter().map(|p| p[1]).fold(0.0, f32::max);
    hoop(b, v3(0.0, 0.0, z0 + 0.05), r * 0.8, 0.3, 0.3, segs);
}

/// The owner's colour as a flat patch on an upward face at height `z`.
fn team_patch(b: &mut MeshBuilder, x0: f32, x1: f32, half: f32, z: f32) {
    b.paint(TEAM);
    b.face(&[
        v3(x0, -half, z),
        v3(x1, -half, z),
        v3(x1, half, z),
        v3(x0, half, z),
    ]);
}

/// The yoke the gun's trunnions ride: a plated cheek either side, `y` out from the bore
/// and `thick` through, from `x[0]` to `x[1]` and `z[0]` up to `z[1]`, a plate down its
/// outside swept back into a spike, and a bronze pin through both at `pivot`.
fn yoke(b: &mut MeshBuilder, pivot: Vec3, y: f32, thick: f32, x: [f32; 2], z: [f32; 2]) {
    let h = z[1] - z[0];
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.loft(
                &[
                    vec![
                        v3(x[0], y, z[0]),
                        v3(x[1], y, z[0]),
                        v3(x[1] - h * 0.2, y, z[1]),
                        v3(x[0] + h * 0.1, y, z[1]),
                    ],
                    vec![
                        v3(x[0], y + thick, z[0]),
                        v3(x[1], y + thick, z[0]),
                        v3(x[1] - h * 0.2, y + thick, z[1]),
                        v3(x[0] + h * 0.1, y + thick, z[1]),
                    ],
                ],
                true,
                true,
            )
        });
        if b.mid() {
            armour(
                b,
                &Frame::new(v3(x[1], y + thick, z[0] + h * 0.55), -Vec3::X, Vec3::Y),
                &[
                    [0.0, -h * 0.35],
                    [0.0, h * 0.35],
                    [(x[1] - x[0]) * 0.8, h * 0.3],
                    [(x[1] - x[0]) + h * 0.4, h * 0.1],
                    [(x[1] - x[0]) * 0.8, -h * 0.35],
                ],
                thick * 0.5,
            );
        }
    });
    collar(b, pivot, Vec3::Y, thick * 0.9, (y + thick) * 2.0 + 0.2);
}

/// The Sunspear's caged fusion core drawn `s` times its size, its cradle's foot at `foot`;
/// records the hub its gimbal rings spin about, with the charge gear's travels at `gear`.
fn fusion_core(b: &mut MeshBuilder, foot: Vec3, s: f32, gear: f32) {
    let core = sunspear::CORE;
    b.set_charge_gear(foot + Vec3::Z * 4.4 * s, gear);
    b.with(
        Affine3A::from_translation(foot)
            * Affine3A::from_scale(Vec3::splat(s))
            * Affine3A::from_translation(-core),
        |b| sunspear::caged_core(b, core),
    );
}

/// Far off: the turret as a block with the owner's colour on it, the gun a bar that
/// still pitches and a prong `prong` either side of its charge.
fn coarse_gun(b: &mut MeshBuilder, line: &Line, deck: f32, top: f32, r: f32, prong: f32) {
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.prism(v3(0.0, 0.0, deck), 4, r, r * 0.8, top - deck);
        team_patch(b, -r * 0.5, r * 0.2, r * 0.3, top + 0.01);
        line.gun(b, |b| {
            let len = line.len;
            dark_plate(b);
            b.cylinder_between(Vec3::ZERO, v3(len * 0.7, 0.0, 0.0), 0.5, 0.4, 3);
            for y in [-prong, prong] {
                b.cylinder_between(v3(len * 0.6, y, 0.0), v3(len - 0.1, y, 0.0), 0.25, 0.2, 3);
            }
        });
    });
}
