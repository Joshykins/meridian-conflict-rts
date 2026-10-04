//! The Basilisk (tech 2 artillery installation, a 2 x 2 lot): a Pinched-plasmeric
//! Howitzer. Like the Halberd it gathers its charge in front of the bore and pinches it
//! out, but lobs it: the gun stands laid up off the deck, a long tube on trunnions, and
//! the `muzzle` is the middle of the charge.
//!
//! A low octagonal keep, braced square by four plated buttresses, carries an open cradle
//! turret: a plated cheek either side of the trunnion, a capacitor block on the turret's
//! back under the owner's colour, and the gun.
//!
//! Design round (`~` keys): how the charge is held ahead of the mouth.
//! - `regency_basilisk`: a ring held out on three struts.
//! - `regency_basilisk~blades`: two deep blades above and below the bore.
//! - `regency_basilisk~cross`: four short projectors round the bore.
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
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.4;
/// Where the bore ends, down the gun's own x.
const MOUTH: f32 = 10.8;
/// The keep's top, where the turret's race sits.
const KEEP: f32 = 4.6;

#[derive(Clone, Copy)]
enum Hold {
    Ring,
    Blades,
    Cross,
}

pub(crate) fn basilisk(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Hold::Ring);
}

pub(crate) fn basilisk_blades(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Hold::Blades);
}

pub(crate) fn basilisk_cross(b: &mut MeshBuilder, _tech: u8) {
    draw(b, Hold::Cross);
}

fn draw(b: &mut MeshBuilder, hold: Hold) {
    LINE.rig(b, 1.4);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 9.6,
                base_h: KEEP,
                x0: -6.0,
                x1: 2.8,
                half: 2.8,
                top: LINE.pivot.z + 1.6,
                gun: Vec2::splat(2.0),
                tip: Vec2::splat(1.3),
                mouth: Some((MOUTH, 1.4)),
            },
        );
        return;
    }
    keep(b);
    b.with_part(part::TURRET, |b| {
        plan_hull(
            b,
            &[[2.8, 2.6], [-1.6, 3.2], [-6.4, 2.6], [-6.8, 0.0]],
            &[(KEEP + 0.4, 1.0), (KEEP + 1.6, 1.0), (KEEP + 2.0, 0.86)],
        );
        cheeks(b, [-2.4, 2.4], 1.9, [KEEP + 1.8, LINE.pivot.z + 1.5], 0.55);
        capacitor(b);
        b.with_limb(rig::ARM_GUN, |b| gun_frame(b, &LINE, |b| gun(b, hold)));
    });
}

/// The keep: an octagonal drum, a buttress out of each side with a plate down its back
/// and a red line let into its flank, plated struts on the corners between them, and the
/// race the turret turns on.
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
            if b.fine() {
                slit(
                    b,
                    v3(5.6, 1.45, 1.4),
                    Vec3::Y,
                    v3(1.0, 0.0, -0.5),
                    1.6,
                    0.24,
                );
                slit(
                    b,
                    v3(5.6, -1.45, 1.4),
                    -Vec3::Y,
                    v3(1.0, 0.0, -0.5),
                    1.6,
                    0.24,
                );
            }
        });
        if b.fine() {
            b.yawed(Vec3::ZERO, a + std::f32::consts::FRAC_PI_4, |b| {
                // The drum's slanted face between two buttresses: a plate swept down it
                // into a spike, a red line under it.
                let (out, down) = (v3(0.972, 0.0, 0.235), v3(0.235, 0.0, -0.972));
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(4.66, 0.0, 4.4), down, out),
                    &swept(3.4, 1.5, 0.0, 0.6),
                    0.25,
                );
                slit(b, v3(5.64, 0.0, 0.55) + out * 0.05, out, Vec3::Y, 2.6, 0.3);
            });
        }
    }
    b.paint(TEAM);
    hoop(b, Vec3::Z * (KEEP + 0.02), 4.4, 0.8, 0.05, b.sides(24));
    metal(b);
    hoop(b, Vec3::Z * (KEEP + 0.2), 3.4, 1.4, 0.4, b.sides(24));
}

/// The capacitor block on the turret's back that feeds the charge: a plated box with
/// fins down its flanks, a red line round its tail, the owner's colour on its top.
fn capacitor(b: &mut MeshBuilder) {
    dark_plate(b);
    b.chamfered_box(v3(-4.6, 0.0, KEEP + 2.9), v3(3.6, 3.4, 2.0), 0.5);
    team_patch(b, -6.0, -3.4, 0.9, KEEP + 3.91);
    if b.fine() {
        let fin = Fin {
            len: 0.7,
            w0: 1.4,
            w1: 1.0,
            thick: 0.18,
        };
        b.mirror_y(|b| {
            dark_plate(b);
            for x in [-5.8f32, -5.0, -4.2, -3.4] {
                fin.at(b, v3(x, 1.7, KEEP + 2.9), Vec3::Y, Vec3::X);
            }
        });
        slit(b, v3(-6.41, 0.0, KEEP + 2.9), -Vec3::X, Vec3::Y, 2.4, 0.3);
    }
}

/// The gun in its own frame: the trunnion pin through the cheeks, a short breech behind
/// it, the recoiling tube with its bands and the red line down its back, and what holds
/// the charge past its mouth.
fn gun(b: &mut MeshBuilder, hold: Hold) {
    let fine = b.fine();
    let len = LINE.len();
    collar(b, Vec3::ZERO, Vec3::Y, 0.8, 4.9);
    dark_plate(b);
    hull_x(
        b,
        &[
            [-3.4, 2.4, 2.2, -0.2],
            [-0.6, 3.0, 2.8, 0.0],
            [2.6, 3.0, 2.6, 0.0],
        ],
        &KEELED,
    );
    if fine {
        let fin = Fin {
            len: 0.5,
            w0: 1.6,
            w1: 1.2,
            thick: 0.16,
        };
        b.mirror_y(|b| {
            dark_plate(b);
            for x in [-2.6f32, -1.8, -1.0] {
                fin.at(b, v3(x, 1.25, 0.3), Vec3::Y, Vec3::X);
            }
        });
    }
    b.with_recoil(|b| {
        dark_plate(b);
        hull_x(
            b,
            &[
                [2.4, 1.9, 1.9, 0.0],
                [7.0, 1.6, 1.6, 0.0],
                [MOUTH, 1.4, 1.4, 0.0],
            ],
            &CHAMFERED,
        );
        if fine {
            for x in [3.6f32, 5.2, 8.6] {
                collar(b, v3(x, 0.0, 0.0), Vec3::X, 1.05, 0.4);
            }
            slit(b, v3(6.0, 0.0, 0.8), Vec3::Z, Vec3::X, 4.0, 0.16);
        }
        metal(b);
        b.cylinder_between(
            v3(MOUTH - 0.2, 0.0, 0.0),
            v3(MOUTH + 0.3, 0.0, 0.0),
            0.6,
            0.55,
            b.sides(10),
        );
        mouth_rim(b, v3(MOUTH + 0.3, 0.0, 0.0), 0.55, b.sides(10));
    });
    match hold {
        Hold::Ring => ring(b, len),
        Hold::Blades => blades(b, len),
        Hold::Cross => cross(b, len),
    }
}

/// A plated ring stood out ahead of the mouth on three struts off a collar on the tube,
/// red on its inner face.
fn ring(b: &mut MeshBuilder, len: f32) {
    let x = len - 0.9;
    let segs = b.sides(16);
    collar(b, v3(MOUTH - 1.6, 0.0, 0.0), Vec3::X, 1.0, 0.8);
    dark_plate(b);
    hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, 1.75, 0.5, 0.7, segs);
    b.paint(GLOW_LASER);
    hoop_on(b, v3(x, 0.0, 0.0), Vec3::X, 1.47, 0.08, 0.4, segs);
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 + 0.5) / 3.0;
        let d = v3(0.0, a.cos(), a.sin());
        strut(
            b,
            v3(MOUTH - 1.6, 0.0, 0.0) + d * 0.9,
            v3(x - 0.2, 0.0, 0.0) + d * 1.6,
            0.16,
        );
    }
}

/// Two deep flat blades above and below the bore, reaching past its mouth either side of
/// the charge, plated outside and lit red inside.
fn blades(b: &mut MeshBuilder, len: f32) {
    for (s, tip) in [(1.0f32, len - 0.4), (-1.0, len - 0.7)] {
        dark_plate(b);
        bar_through(
            b,
            &[
                (v3(MOUTH - 3.4, 0.0, s * 1.05), Vec2::new(0.9, 0.7)),
                (v3(MOUTH - 0.4, 0.0, s * 1.6), Vec2::new(0.8, 1.5)),
                (v3(tip, 0.0, s * 1.6), Vec2::new(0.55, 1.0)),
            ],
            Vec3::Y,
        );
        emitter(b, v3(tip - 0.4, 0.0, s * 1.05), v3(len, 0.0, 0.0), 0.34);
        if b.fine() {
            slit(
                b,
                v3(tip - 2.0, 0.0, s * 1.08),
                -Vec3::Z * s,
                Vec3::X,
                2.0,
                0.28,
            );
        }
    }
}

/// Four short projectors round the bore on a collar at its mouth, each a plated rod with
/// a red lens turned on the charge.
fn cross(b: &mut MeshBuilder, len: f32) {
    collar(b, v3(MOUTH - 1.2, 0.0, 0.0), Vec3::X, 1.0, 1.0);
    for k in 0..4 {
        let a = std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32;
        let d = v3(0.0, a.cos(), a.sin());
        let tip = len - 0.6;
        strut(
            b,
            v3(MOUTH - 1.6, 0.0, 0.0) + d * 1.0,
            v3(tip - 0.4, 0.0, 0.0) + d * 1.5,
            0.24,
        );
        emitter(b, v3(tip, 0.0, 0.0) + d * 1.4, v3(len, 0.0, 0.0), 0.3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basilisk_holds_its_charge() {
        for key in [
            "regency_basilisk",
            "regency_basilisk~blades",
            "regency_basilisk~cross",
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
