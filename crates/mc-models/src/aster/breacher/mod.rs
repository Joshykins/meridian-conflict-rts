//! Breacher (`aster_t4_breacher`, mesh "breacher"): the tech 4 assault walker, authored in
//! metres at radius 24 and 54 m tall; the unit file draws it 1.25 times that (radius 30,
//! 67.5 m), and its muzzles and pivots are these numbers grown to match. A hunched upper
//! body thrown forward over two short, heavy, reverse-kneed legs set wide; a
//! gatling-breach cannon for each forearm (the Behemoth's Tempest cut down: six barrels
//! turning out of a shroud), carried low and ahead; two sixteen-cell thermobaric rocket
//! launchers in a hump on its back, raked steeply over its head; a low pointed head with
//! one thin sensor line under a heavy brow; a hull field thrown from a projector behind
//! the hump.
//!
//! Rig:
//! - `part::LOCOMOTION` legs posed by the shader's IK (`LEG`, `STRIDE`, `STANCE`, `LIFT`,
//!   `CROUCH`, `FOOT`), reverse-kneed (`set_hock`).
//! - `part::HULL` the pelvis.
//! - `part::TURRET` the body over the waist (`WAIST`) and the launchers (weapon 0, the
//!   torso's own), turning about the z axis.
//! - The arms are gun houses (weapons 1 and 2, right and left) turning about `PIVOT` on
//!   the torso's axis: they go round with the torso, each swinging off it on its own
//!   (`sway`) and pitching on its own. The right gun's barrels turn about its bore
//!   (`with_spin`, `Model::spins`); the left gun's about the mirror of that line
//!   (`entity.wgsl`), so the guns stay mirrored in y.
//!
//! Style: layered faceted plates over a dark gunmetal frame, thin blue seams of light
//! between some of them, grey metal for joints and rams only.

mod body;
pub(crate) mod gun;
mod head;
mod legs;
pub(crate) mod pods;

use glam::Vec3;

use super::limbs::*;
use super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;
use legs::Leg;

/// The left leg's joints at rest.
const LEG: Leg = Leg {
    hip: Vec3::new(-3.0, 9.5, 22.0),
    knee: Vec3::new(5.5, 11.0, 15.5),
    hock: Vec3::new(-6.0, 12.0, 8.0),
    ankle: Vec3::new(-0.5, 12.5, 3.6),
};
/// Ground to a full cycle (a power of two), share of it a foot is down, how high a foot
/// lifts, how far the hips settle in full stride.
const STRIDE: f32 = 32.0;
const STANCE: f32 = 0.62;
const LIFT: f32 = 3.6;
const CROUCH: f32 = 1.6;
/// How much of the leg's swing the tarsus leans with (`MeshBuilder::set_hock`).
const HOCK_FOLLOW: f32 = 0.55;
/// The sole: metres behind the ankle, ahead of it, across.
pub(crate) const FOOT: (f32, f32, f32) = (-7.0, 9.0, 9.0);
/// The body turns on the waist at this height.
const WAIST: f32 = 25.0;
/// The arms' and the launchers' pivot (every weapon's `pivot`), on the torso's axis.
pub(crate) const PIVOT: Vec3 = Vec3::new(0.0, 0.0, 40.0);
/// The left gun's origin on its bore line; its muzzle is `gun::REACH` ahead.
pub(crate) const GUN: Vec3 = Vec3::new(13.0, 17.5, 24.0);
/// The left launcher's face and its rake (radians nose-up).
pub(crate) const POD: (Vec3, f32) = (Vec3::new(-7.0, 5.2, 50.0), 0.95);
/// The shield projector's tip.
const SHIELD: Vec3 = Vec3::new(-17.0, 0.0, 47.0);

/// The left gun's muzzle.
#[cfg(test)]
pub(crate) fn muzzle() -> Vec3 {
    GUN + Vec3::X * gun::REACH
}

pub(crate) fn breacher(b: &mut MeshBuilder, _tech: u8) {
    let l = LEG;
    b.set_legs(l.hip, l.knee, l.ankle, STRIDE, STANCE, LIFT);
    b.set_hock(l.hock, HOCK_FOLLOW);
    b.set_walk_crouch(CROUCH);
    b.set_walk_sway(0.04, 0.03, 0.6);
    b.set_foot(FOOT.0, FOOT.1, FOOT.2);
    b.set_dust_line(3.0);
    b.set_turret_pivot(v3(0.0, 0.0, WAIST));
    b.set_shield_emitter(SHIELD);
    if b.coarse() {
        coarse(b);
        return;
    }
    // The arms first, at every level, so their houses take the same slots (0 and 1) in
    // each, and the right gun's spin axis is the model's.
    for side in [-1.0f32, 1.0] {
        let origin = GUN * v3(1.0, side, 1.0);
        b.with_house(gun::weapon(side), PIVOT, gun::KICK, |b| {
            b.with_recoil(|b| {
                body::arm(b, side);
                gun::gun(b, side, origin, true);
            })
        });
    }
    b.mirror_y(|b| legs::leg(b, l));
    body::body(b);
    b.with_part(part::TURRET, |b| {
        head::head(b);
        b.mirror_y(|b| pods::launcher_at(b, POD.0, POD.1, true));
        projector(b, v3(-15.0, 0.0, 44.0), SHIELD);
    });
}

/// From far off: the legs as bars, a bar of a body, the launchers' faces and the arms as
/// bars out to their muzzles.
fn coarse(b: &mut MeshBuilder) {
    b.mirror_y(|b| legs::coarse_leg(b, LEG));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        tri_bar(b, v3(-1.0, 0.0, 25.0), v3(3.0, 0.0, 48.0), 11.0, 11.0);
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            let (f, h) = (POD.0, pods::HALF);
            b.face(&[
                f + v3(0.0, -h, -h),
                f + v3(0.0, h, -h),
                f + v3(0.0, h, h),
                f + v3(0.0, -h, h),
            ]);
        });
    });
    for side in [-1.0f32, 1.0] {
        b.with_house(gun::weapon(side), PIVOT, 0.0, |b| {
            b.with_recoil(|b| {
                b.paint(PLATING);
                let origin = GUN * v3(1.0, side, 1.0);
                tri_bar(
                    b,
                    origin - Vec3::X * 9.0,
                    origin + Vec3::X * gun::REACH,
                    4.0,
                    0.5,
                );
            })
        });
    }
}

/// The shield projector: a short mast up from `base`, a collar and a lit ring at `tip`.
fn projector(b: &mut MeshBuilder, base: Vec3, tip: Vec3) {
    b.paint(ACCENT).pattern(crate::pattern::PLAIN);
    b.cylinder_between(base, tip - Vec3::Z * 0.8, 1.0, 0.8, b.sides(8));
    b.paint(PLATING);
    b.cylinder_between(
        tip - Vec3::Z * 0.8,
        tip + Vec3::Z * 0.2,
        1.8,
        1.6,
        b.sides(12),
    );
    b.paint(GLOW_SHIELD);
    b.cylinder_between(
        tip + Vec3::Z * 0.2,
        tip + Vec3::Z * 0.5,
        1.2,
        1.2,
        b.sides(12),
    );
}
