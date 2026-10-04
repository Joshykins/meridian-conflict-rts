//! The Basilisk (tech 2 artillery installation, a 2 x 2 lot): a Pinched-plasmeric
//! Howitzer. It gathers its charge in front of the bore, as the Halberd does, and lobs it.
//! The `muzzle` is the middle of that charge.
//!
//! The gun is the Kiln's kind of bore at tech 2: split down its length into two heavy
//! plated halves that part through the charge, Pinched red let into their inner faces,
//! a course of swept plates along each half and graphite bands across them. It is laid
//! up off an open cradle, slab cheeks swept back into spikes, on the Halberd's braced core.
//!
//! Design round (`~` keys): how the bore is split.
//! - `regency_basilisk`: two halves side by side, plates along their tops.
//! - `regency_basilisk~jacket`: the same halves, their back half sheathed in a plated
//!   jacket with plates feathered down its flanks.
//! - `regency_basilisk~upright`: the halves one above the other, plates along their flanks.
//!
//! Its pivot and muzzle are the unit file's (`data/factions/regency/units/structures.ron`).

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::*;
use super::halberd::braced_core;
use super::*;

/// The trunnion over the core's middle, the charge 13.4 m out down a bore laid 22 degrees
/// up.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 7.6), Vec3::new(12.4, 0.0, 12.6));
/// How far round the charge the halves' tips stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.2;
/// The braced core's top, where the turret's race sits.
const CORE: f32 = 5.2;
/// Half the gap between the halves.
const GAP: f32 = 0.3;

#[derive(Clone, Copy, PartialEq)]
enum Bore {
    Side,
    Jacket,
    Upright,
}

pub(crate) fn basilisk(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Bore::Side);
}

pub(crate) fn basilisk_jacket(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Bore::Jacket);
}

pub(crate) fn basilisk_upright(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Bore::Upright);
}

fn draw(b: &mut MeshBuilder, bore: Bore) {
    LINE.rig(b, 1.2);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 9.4,
                base_h: CORE,
                x0: -5.6,
                x1: 3.0,
                half: 3.0,
                top: LINE.pivot.z + 1.4,
                gun: Vec2::splat(2.2),
                tip: Vec2::splat(1.4),
                mouth: Some((LINE.len() - 3.0, 1.4)),
            },
        );
        return;
    }
    braced_core(b);
    b.with_part(part::TURRET, |b| {
        plan_hull(
            b,
            &[[3.0, 2.8], [-2.0, 3.4], [-6.2, 2.6], [-6.6, 0.0]],
            &[(CORE + 0.4, 1.0), (CORE + 1.4, 1.0), (CORE + 1.8, 0.86)],
        );
        cheeks(b, [-2.6, 2.4], 2.0, [CORE + 1.6, LINE.pivot.z + 1.4], 0.55);
        // A plated block on the turret's back, swept up at its front: what feeds the
        // charge.
        plan_hull(
            b,
            &[[-3.4, 1.5], [-5.8, 1.5], [-6.2, 0.0]],
            &[(CORE + 1.7, 1.0), (CORE + 3.0, 0.8)],
        );
        team_patch(b, -5.2, -3.8, 0.8, CORE + 3.01);
        if b.fine() {
            b.mirror_y(|b| slit(b, v3(-4.7, 1.5, CORE + 2.2), Vec3::Y, Vec3::X, 1.8, 0.2));
        }
        b.with_limb(rig::ARM_GUN, |b| gun_frame(b, &LINE, |b| gun(b, bore)));
    });
}

/// The gun in its own frame: the trunnion pin, a short plated breech, and the split bore,
/// which recoils.
fn gun(b: &mut MeshBuilder, bore: Bore) {
    collar(b, Vec3::ZERO, Vec3::Y, 0.8, 4.4);
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(-2.6, 0.0, 0.0),
            v3(2.0, 0.0, 0.0),
            Vec2::new(2.8, 2.2),
            Vec2::new(2.6, 2.0),
        )
    });
    b.with_recoil(|b| {
        let turn = if bore == Bore::Upright {
            std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };
        b.with(Affine3A::from_rotation_x(turn), split);
        if bore == Bore::Jacket {
            jacket(b);
        }
    });
}

/// The bore split down its length into two plated halves either side of the gap, a red
/// strip let into each inner face a pace at a time from the breech, a course of swept
/// plates along each half's top, graphite bands across both, and a graphite rod down the
/// gap that stops short of the charge.
fn split(b: &mut MeshBuilder) {
    let len = LINE.len();
    let fine = b.fine();
    // A half's section, out from its flat inner face (`y` from 0 out, `z` up).
    let shape: &[[f32; 2]] = &[
        [0.0, 1.0],
        [0.55, 0.9],
        [1.0, 0.4],
        [1.0, -0.4],
        [0.55, -0.9],
        [0.0, -1.0],
    ];
    let stations = [
        (1.6, 1.15, 1.05),
        (3.6, 1.15, 1.05),
        (len - 2.6, 0.9, 0.8),
        (len - 1.0, 0.72, 0.62),
    ];
    metal(b);
    b.cylinder_between(
        v3(1.8, 0.0, 0.0),
        v3(len - 3.4, 0.0, 0.0),
        0.24,
        0.2,
        b.sides(8),
    );
    for x in [3.0f32, 6.4, 9.6] {
        b.block(v3(x, -GAP - 1.25, -1.12), v3(x + 0.55, GAP + 1.25, -0.98));
    }
    b.mirror_y(|b| {
        let rings: Vec<Vec<Vec3>> = stations
            .iter()
            .map(|&(x, w, h)| {
                shape
                    .iter()
                    .map(|&[y, z]| v3(x, GAP + y * w, z * h))
                    .collect()
            })
            .collect();
        dark_plate(b);
        b.with_facets(|b| b.loft(&rings, true, true));
        // The inner face lit a pace at a time from the breech: Pinched red, not fusion.
        let strips: Vec<u32> = if fine { (0..6).collect() } else { vec![1, 4] };
        b.paint(GLOW_LASER);
        for i in strips {
            let x = 2.4 + 1.5 * i as f32;
            b.block(v3(x, GAP - 0.04, -0.22), v3(x + 1.0, GAP, 0.22));
        }
        if fine {
            metal(b);
            for x in [3.0f32, 6.4, 9.6] {
                b.block(v3(x, GAP + 0.5, -1.02), v3(x + 0.55, GAP + 1.2, 1.02));
            }
            dark_plate(b);
            Course {
                count: 3,
                step: 3.4,
                len: 4.0,
                half: 0.5,
                tip: -0.4,
                thick: 0.18,
                tail: 0.6,
            }
            .lay(
                b,
                &Frame::new(
                    v3(len - 1.6, GAP + 0.55, 0.86),
                    v3(-1.0, 0.0, 0.03),
                    v3(0.0, 0.3, 1.0),
                ),
            );
        }
    });
}

/// A plated jacket over the bore's back half, keeled, with a course of swept plates
/// feathered down each flank.
fn jacket(b: &mut MeshBuilder) {
    dark_plate(b);
    hull_x(
        b,
        &[
            [1.0, 3.6, 2.7, 0.0],
            [5.4, 3.4, 2.5, 0.0],
            [6.4, 3.0, 2.2, 0.0],
        ],
        &KEELED,
    );
    if b.fine() {
        b.mirror_y(|b| {
            dark_plate(b);
            Course {
                count: 2,
                step: 2.4,
                len: 3.0,
                half: 0.55,
                tip: 0.4,
                thick: 0.16,
                tail: 0.8,
            }
            .lay(
                b,
                &Frame::new(v3(5.6, 1.72, 0.1), v3(-1.0, 0.03, 0.0), Vec3::Y),
            );
            slit(b, v3(3.4, 1.71, -0.6), Vec3::Y, Vec3::X, 3.0, 0.2);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basilisk_holds_its_charge() {
        for key in [
            "regency_basilisk",
            "regency_basilisk~jacket",
            "regency_basilisk~upright",
        ] {
            super::super::super::check_charge(
                key,
                10.5,
                12.0,
                Some(2),
                &[LINE.muzzle.to_array()],
                HOLD,
            );
        }
    }
}
