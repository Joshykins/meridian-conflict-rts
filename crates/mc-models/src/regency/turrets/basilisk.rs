//! The Basilisk (tech 2 artillery installation, a 2 x 2 lot): a Pinched-plasmeric
//! Howitzer. Like the Halberd it gathers its charge in front of the bore and pinches it
//! out, but lobs it. It is drawn as a heavy howitzer: a long thick barrel laid up off a
//! turret on a low keep, ending in a plated muzzle brake, and the charge forms at the
//! brake's mouth between its baffles. The `muzzle` is the middle of that charge.
//!
//! Design round (`~` keys): the turret.
//! - `regency_basilisk`: an armoured casemate, the barrel out of a mantlet.
//! - `regency_basilisk~cradle`: an open cradle between two tall sloped cheeks.
//! - `regency_basilisk~sleeve`: the casemate, a plated jacket over the barrel's back half.
//!
//! Its pivot and muzzle are the unit file's (`data/factions/regency/units/structures.ron`).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::*;
use super::*;

/// The trunnion over the keep's middle, the charge 13.4 m out down a bore laid 22 degrees
/// up.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 7.6), Vec3::new(12.4, 0.0, 12.6));
/// How far round the charge the brake's baffles stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.4;
/// The brake: how long it is, and its half width and height.
const BRAKE: f32 = 2.8;
const BRAKE_W: f32 = 1.3;
const BRAKE_H: f32 = 0.95;
/// The keep's top, where the turret's race sits.
const KEEP: f32 = 4.6;

#[derive(Clone, Copy, PartialEq)]
enum Turret {
    Casemate,
    Cradle,
    Sleeve,
}

pub(crate) fn basilisk(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Turret::Casemate);
}

pub(crate) fn basilisk_cradle(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Turret::Cradle);
}

pub(crate) fn basilisk_sleeve(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Turret::Sleeve);
}

fn draw(b: &mut MeshBuilder, turret: Turret) {
    LINE.rig(b, 1.4);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 9.6,
                base_h: KEEP,
                x0: -6.0,
                x1: 3.0,
                half: 3.0,
                top: LINE.pivot.z + 1.8,
                gun: Vec2::splat(2.2),
                tip: Vec2::splat(1.6),
                mouth: Some((LINE.len() - BRAKE, BRAKE_W)),
            },
        );
        return;
    }
    keep(b);
    b.with_part(part::TURRET, |b| {
        match turret {
            Turret::Casemate | Turret::Sleeve => casemate(b),
            Turret::Cradle => cradle(b),
        }
        b.with_limb(rig::ARM_GUN, |b| gun_frame(b, &LINE, |b| gun(b, turret)));
    });
}

/// The keep: an octagonal drum braced by a buttress out of each side, a plate swept
/// down each slanted face between them, and the race the turret turns on.
fn keep(b: &mut MeshBuilder) {
    step(b, 8, 6.2, 5.0, 0.0, KEEP);
    for k in 0..4 {
        let a = (90.0 * k as f32).to_radians();
        b.yawed(Vec3::ZERO, a, |b| {
            buttress(b, 4.2, 3.6, 4.2, 7.6, 2.4, 0.9);
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(4.6, 0.0, 4.2), v3(1.0, 0.0, -0.9), v3(0.9, 0.0, 1.0)),
                &swept(4.6, 1.1, 0.0, 0.55),
                0.3,
            );
        });
        if b.fine() {
            b.yawed(Vec3::ZERO, a + std::f32::consts::FRAC_PI_4, |b| {
                let (out, down) = (v3(0.972, 0.0, 0.235), v3(0.235, 0.0, -0.972));
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(4.66, 0.0, 4.4), down, out),
                    &swept(3.4, 1.5, 0.0, 0.6),
                    0.25,
                );
                if k % 2 == 1 {
                    slit(b, v3(5.64, 0.0, 0.55) + out * 0.05, out, Vec3::Y, 2.2, 0.24);
                }
            });
        }
    }
    b.paint(TEAM);
    hoop(b, Vec3::Z * (KEEP + 0.02), 4.4, 0.8, 0.05, b.sides(24));
    metal(b);
    hoop(b, Vec3::Z * (KEEP + 0.2), 3.4, 1.4, 0.4, b.sides(24));
}

/// An armoured house round the trunnion: sloped faces drawn in toward a flat roof, the
/// owner's colour on the roof's back, a red line down each flank.
fn casemate(b: &mut MeshBuilder) {
    let z = LINE.pivot.z;
    plan_hull(
        b,
        &[
            [3.0, 1.9],
            [1.2, 3.1],
            [-4.4, 3.1],
            [-6.2, 2.0],
            [-6.6, 0.0],
        ],
        &[(KEEP + 0.4, 1.0), (z + 0.2, 1.0), (z + 2.0, 0.8)],
    );
    team_patch(b, -4.8, -2.6, 1.2, z + 2.01);
    if b.fine() {
        // Plates hung on the flanks, swept back into spikes past the house's tail.
        b.mirror_y(|b| {
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(0.6, 3.1, z - 0.6), -Vec3::X, Vec3::Y),
                &swept(6.8, 1.1, 0.3, 0.7),
                0.25,
            );
            slit(b, v3(-2.4, 3.37, z - 1.6), Vec3::Y, Vec3::X, 3.0, 0.24);
        });
    }
}

/// An open cradle: a low deck, a tall cheek either side of the gun with a plate swept
/// down its outside, and a wedge of armour on the deck's back under the owner's colour.
fn cradle(b: &mut MeshBuilder) {
    let z = LINE.pivot.z;
    plan_hull(
        b,
        &[[3.0, 2.6], [-1.6, 3.2], [-6.4, 2.6], [-6.8, 0.0]],
        &[(KEEP + 0.4, 1.0), (KEEP + 1.6, 1.0), (KEEP + 2.0, 0.86)],
    );
    cheeks(b, [-2.8, 2.8], 1.7, [KEEP + 1.8, z + 2.0], 0.8);
    plan_hull(
        b,
        &[[-3.6, 1.6], [-6.2, 1.6], [-6.4, 0.0]],
        &[(KEEP + 1.9, 1.0), (KEEP + 3.4, 0.82)],
    );
    team_patch(b, -5.6, -4.0, 0.9, KEEP + 3.41);
}

/// The gun in its own frame: the trunnion pin, a heavy breech, the recoiling barrel and
/// its muzzle brake.
fn gun(b: &mut MeshBuilder, turret: Turret) {
    let fine = b.fine();
    let len = LINE.len();
    let neck = len - BRAKE;
    collar(b, Vec3::ZERO, Vec3::Y, 0.8, 4.4);
    dark_plate(b);
    hull_x(
        b,
        &[
            [-3.6, 2.4, 2.4, 0.0],
            [-0.4, 2.8, 2.8, 0.0],
            [2.2, 2.8, 2.6, 0.0],
        ],
        &CHAMFERED,
    );
    if turret != Turret::Cradle {
        // The mantlet the barrel leaves the casemate through.
        hull_x(b, &[[2.0, 3.4, 3.0, 0.0], [3.4, 3.0, 2.6, 0.0]], &CHAMFERED);
    }
    b.with_recoil(|b| {
        dark_plate(b);
        hull_x(
            b,
            &[
                [2.2, 2.0, 2.0, 0.0],
                [neck - 2.0, 1.6, 1.6, 0.0],
                [neck, 1.5, 1.5, 0.0],
            ],
            &CHAMFERED,
        );
        if turret == Turret::Sleeve {
            hull_x(
                b,
                &[
                    [3.0, 2.3, 2.2, 0.08],
                    [7.6, 2.1, 2.0, 0.06],
                    [8.2, 1.7, 1.6, 0.0],
                ],
                &CHAMFERED,
            );
            if fine {
                slit(b, v3(5.2, 0.0, 1.17), Vec3::Z, Vec3::X, 3.6, 0.16);
            }
        } else if fine {
            for x in [4.4f32, 7.2] {
                collar(b, v3(x, 0.0, 0.0), Vec3::X, 0.95, 0.45);
            }
        }
        brake(b, neck, len);
    });
}

/// A muzzle brake from `x0` to just short of `len`: a plated box, open at the front, its
/// flanks cut through by two windows each, so the baffles that hold the charge stand
/// either side of its mouth.
fn brake(b: &mut MeshBuilder, x0: f32, len: f32) {
    let x1 = len - 0.1;
    let (w, h, t) = (BRAKE_W, BRAKE_H, 0.26);
    dark_plate(b);
    // Roof and floor.
    b.block(v3(x0, -w, h - t), v3(x1, w, h));
    b.block(v3(x0, -w, -h), v3(x1, w, -h + t));
    // Three posts down each flank: back, middle and the front baffle.
    let posts = [(x0, x0 + 0.6), (x0 + 1.15, x0 + 1.55), (x1 - 0.45, x1)];
    b.mirror_y(|b| {
        dark_plate(b);
        for &(a, c) in &posts {
            b.block(v3(a, w - t, -h + t), v3(c, w, h - t));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basilisk_holds_its_charge() {
        for key in [
            "regency_basilisk",
            "regency_basilisk~cradle",
            "regency_basilisk~sleeve",
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
