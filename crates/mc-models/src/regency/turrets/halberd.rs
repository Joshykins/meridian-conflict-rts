//! The Halberd (tech 2 point defence, a 2 x 2 lot): a Pinched-plasmeric Cannon. Its
//! projectors reach past the bore's mouth and gather the plasma into a ball between them,
//! then it goes out as a burst of three. The `muzzle` is the middle of that ball.
//!
//! The fork: two thick flat prongs either side of a square barrel, reaching past its
//! mouth, red emitters on their inner faces. An open cradle of a turret, a low deck and a
//! tall plated cheek either side of the gun, on a core braced by four diagonal buttresses
//! with plated struts between them.
//!
//! Its pivot and muzzle are the unit file's (`data/factions/regency/units/structures.ron`).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, v3};
use super::super::machine::*;
use super::*;

pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 8.4), Vec3::new(11.6, 0.0, 8.4));
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 2.2;
const THICK: f32 = 0.3;

fn coarse_pinch(b: &mut MeshBuilder) {
    coarse(
        b,
        &LINE,
        &Coarse {
            base_r: 9.4,
            base_h: 5.2,
            x0: -5.6,
            x1: 3.4,
            half: 3.0,
            top: LINE.pivot.z + 2.0,
            gun: Vec2::splat(2.0),
            tip: Vec2::splat(1.2),
            mouth: Some((9.4, 1.3)),
        },
    );
}

/// Red slots round a core's faces, `r` out, at height `z`, `n` of them from +x.
fn slots(b: &mut MeshBuilder, n: usize, r: f32, z: f32, len: f32) {
    if !b.fine() {
        return;
    }
    for k in 0..n {
        let a = std::f32::consts::TAU * k as f32 / n as f32;
        let d = v3(a.cos(), a.sin(), 0.0);
        slit(b, d * r + Vec3::Z * z, d, v3(-d.y, d.x, 0.0), len, 0.2);
    }
}

/// The owner's colour round the top of a base at `z`, and the race the turret rides.
fn race(b: &mut MeshBuilder, z: f32, r: f32) {
    let segs = if b.fine() { 24 } else { 8 };
    b.paint(TEAM);
    hoop(b, Vec3::Z * (z + 0.02), r + 0.6, 0.9, 0.05, segs);
    metal(b);
    hoop(b, Vec3::Z * (z + 0.2), r - 0.3, 1.4, 0.4, segs);
}

pub(crate) fn halberd(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 0.8);
    if b.coarse() {
        coarse_pinch(b);
        return;
    }
    braced_core(b);
    b.with_part(part::TURRET, |b| {
        plan_hull(
            b,
            &[[3.0, 3.0], [-2.0, 3.4], [-6.0, 2.6], [-5.0, 0.0]],
            &[(5.6, 1.0), (6.8, 1.0), (7.3, 0.85)],
        );
        cheeks(b, [-2.6, 2.2], 2.1, [6.6, 10.0], 0.5);
        team_patch(b, -4.0, -3.0, 0.7, 7.32);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| fork(b, LINE.len()))
        });
    });
}

/// A square core with its corners cut, a buttress out from each corner with a plate
/// down its back, a plated strut on each face between them.
fn braced_core(b: &mut MeshBuilder) {
    dark_plate(b);
    b.chamfered_box(v3(0.0, 0.0, 2.6), v3(9.0, 9.0, 5.2), 2.2);
    for k in 0..4 {
        b.yawed(Vec3::ZERO, (45.0 + 90.0 * k as f32).to_radians(), |b| {
            buttress(b, 3.5, 3.4, 4.8, 11.0, 2.2, 1.0);
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(4.0, 0.0, 4.9), v3(1.0, 0.0, -0.52), v3(0.52, 0.0, 1.0)),
                &swept(6.6, 1.0, 0.0, 0.55),
                THICK,
            );
        });
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        if b.fine() {
            strut(b, d * 4.9 + Vec3::Z * 0.2, d * 4.9 + Vec3::Z * 4.6, 0.4);
        }
    }
    slots(b, 4, 4.52, 2.4, 1.6);
    race(b, 5.2, 3.6);
}

/// The fork in its own frame: a square barrel with bronze bands and a red channel down
/// its back, two flat prongs either side reaching past its mouth, plated outside and lit
/// red inside, tied under the barrel in bronze. The barrel recoils; the prongs hold.
fn fork(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.9, 5.2);
    dark_plate(b);
    hull_x(b, &[[-2.6, 3.0, 2.4, 0.2], [2.4, 3.2, 2.6, 0.0]], &KEELED);
    b.mirror_y(|b| {
        dark_plate(b);
        bar_through(
            b,
            &[
                (v3(1.6, 1.2, 0.0), Vec2::new(0.9, 2.0)),
                (v3(7.5, 1.8, 0.0), Vec2::new(0.8, 1.7)),
                (v3(len + 0.4, 1.7, 0.0), Vec2::new(0.6, 1.1)),
            ],
            Vec3::Y,
        );
        armour(
            b,
            &Frame::new(v3(len, 2.03, 0.0), v3(-1.0, 0.03, 0.0), Vec3::Y),
            &swept(7.0, 0.8, 0.0, 0.6),
            0.2,
        );
        emitter(b, v3(len - 0.5, 1.42, 0.0), v3(len, 0.0, 0.0), 0.38);
        if fine {
            slit(b, v3(len - 2.4, 1.43, 0.0), -Vec3::Y, Vec3::X, 2.4, 0.3);
            strut(b, v3(4.0, 0.6, -0.5), v3(6.2, 1.25, -0.4), 0.18);
        }
    });
    metal(b);
    for x in [5.0f32, 8.0] {
        bar_through(
            b,
            &[
                (v3(x, -1.4, -0.6), Vec2::new(0.4, 0.4)),
                (v3(x, 1.4, -0.6), Vec2::new(0.4, 0.4)),
            ],
            Vec3::X,
        );
    }
    b.with_recoil(|b| {
        dark_plate(b);
        hull_x(b, &[[2.0, 2.0, 1.8, 0.0], [9.0, 1.7, 1.5, 0.0]], &CHAMFERED);
        if fine {
            for x in [3.4f32, 4.9, 6.4] {
                collar(b, v3(x, 0.0, 0.0), Vec3::X, 1.15, 0.35);
            }
            slit(b, v3(5.4, 0.0, 0.85), Vec3::Z, Vec3::X, 3.0, 0.14);
        }
        metal(b);
        b.cylinder_between(v3(8.8, 0.0, 0.0), v3(9.4, 0.0, 0.0), 0.55, 0.5, b.sides(10));
        b.paint(GLOW_LASER);
        hoop_on(
            b,
            v3(9.35, 0.0, 0.0),
            Vec3::X,
            0.45,
            0.15,
            0.12,
            b.sides(10),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halberd_holds_its_charge() {
        super::super::super::check_charge(
            "regency_pinch_cannon",
            10.5,
            11.0,
            Some(2),
            &[LINE.muzzle.to_array()],
            HOLD,
        );
    }
}
