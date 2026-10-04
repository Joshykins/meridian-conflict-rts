//! The cannon forearm rebuilt (the `gun` slot): the Pinched-plasmeric Cannon (`pinched`),
//! then the Pinch-fusion Cannon (`fusion`) over it. Each takes the Repeater's sleeves off
//! (`arms::cannon` draws them `until` pinched), keeps the armoured housing and rides the
//! recoil with the rest of the gun; each is longer than the last, its muzzle the unit
//! file's (`PINCHED_TIP`, `FUSION_TIP` along the forearm).
//!
//! - Pinched: heavier sleeves and four keeled vanes laid along the barrel, closing on the
//!   bore toward the muzzle: the gravity lens that squeezes the plasma into a stream, the
//!   stream lit red down the gaps between them.
//! - Pinch-fusion: the Regency's fusion bore (as the Fusion Howitzer's): a plated chamber out
//!   of the housing with the white-hot prism let into its vents, then the barrel split down
//!   its length into two keeled halves, the fusion light running between them to the mouth.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam};
use super::arms::BREECH;
use super::form::{blade, ring, sleeve, KEEL, OCT};

/// The muzzles, along the forearm.
pub(super) const PINCHED_TIP: f32 = 10.6;
pub(super) const FUSION_TIP: f32 = 11.6;

/// Both tiers; `c(x)` is the bore's axis `x` along the forearm.
pub(super) fn cannons(b: &mut MeshBuilder, c: &dyn Fn(f32) -> Vec3) {
    b.module("pinched", 0.3, |b| b.until("fusion", |b| pinched(b, c)));
    b.module("fusion", 0.3, |b| fusion(b, c));
}

fn pinched(b: &mut MeshBuilder, c: &dyn Fn(f32) -> Vec3) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(c(BREECH - 0.5), Vec3::Z, 0.9, 0.9),
            ring(c(BREECH + 0.4), Vec3::Z, 0.96, 0.96),
            ring(c(7.0), Vec3::Z, 0.74, 0.74),
        ],
        &OCT,
    );
    metal(b);
    b.cylinder_between(c(6.9), c(7.3), 0.58, 0.58, b.sides(8));
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(c(7.2), Vec3::Z, 0.64, 0.64),
            ring(c(7.6), Vec3::Z, 0.66, 0.66),
            ring(c(9.7), Vec3::Z, 0.48, 0.48),
        ],
        &OCT,
    );
    // The four vanes, keeled outward on the diagonals, closing on the bore.
    for deg in [45.0f32, 135.0, 225.0, 315.0] {
        let a = deg.to_radians();
        let dir = Vec3::new(0.0, -a.sin(), a.cos());
        let vane = [
            ring(c(4.5) + dir * 1.0, dir, 0.16, 0.3),
            ring(c(7.6) + dir * 1.02, dir, 0.2, 0.44),
            ring(c(9.9) + dir * 0.62, dir, 0.13, 0.26),
            ring(c(PINCHED_TIP) + dir * 0.3, dir, 0.03, 0.03),
        ];
        if b.fine() {
            sleeve(b, &vane, &KEEL);
        } else {
            sleeve(b, &[vane[0], vane[1], vane[3]], &KEEL);
        }
    }
    // The stream lit down the gaps between the vanes, on the sides and on top.
    if b.fine() {
        b.paint(GLOW_LASER);
        // (across, up) sizes: thin through the plate, longer along it.
        for (out, a, e) in [
            (Vec3::Y, Vec2::new(0.05, 0.1), Vec2::new(0.05, 0.08)),
            (-Vec3::Y, Vec2::new(0.05, 0.1), Vec2::new(0.05, 0.08)),
            (Vec3::Z, Vec2::new(0.1, 0.05), Vec2::new(0.08, 0.05)),
        ] {
            b.beam(c(4.8) + out * 0.9, c(6.7) + out * 0.73, a, e);
        }
    }
    // The bore, lit at the mouth inside the vanes' tips.
    seam(b);
    b.cylinder_between(c(9.6), c(10.2), 0.34, 0.3, b.sides(8));
    b.paint(GLOW_LASER);
    b.cylinder_between(c(10.1), c(PINCHED_TIP), 0.22, 0.14, b.sides(8));
}

fn fusion(b: &mut MeshBuilder, c: &dyn Fn(f32) -> Vec3) {
    // The chamber out of the housing: a heavy plated block, deepest at its middle.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(c(BREECH - 0.5), Vec3::Z, 0.98, 0.98),
            ring(c(BREECH + 0.4), Vec3::Z, 1.08, 1.12),
            ring(c(5.6), Vec3::Z, 1.12, 1.16),
            ring(c(6.6), Vec3::Z, 0.86, 0.86),
        ],
        &OCT,
    );
    // Its cheeks: a blade laid back along each side over the vents' ends.
    for side in [1.0f32, -1.0] {
        blade(
            b,
            c(6.3) + Vec3::Y * (side * 1.0),
            Vec3::new(-1.0, 0.0, 0.1),
            Vec3::Y * side,
            3.0,
            0.62,
            0.5,
            0.26,
        );
    }
    // The vents down each side, the fusion light in them.
    b.paint(GLOW_PRISM);
    let vents: &[f32] = if b.fine() {
        &[4.3, 4.9, 5.5]
    } else {
        &[4.6, 5.4]
    };
    for side in [1.0f32, -1.0] {
        for &x in vents {
            b.beam(
                c(x) + Vec3::Y * (side * 1.1) - Vec3::Z * 0.45,
                c(x) + Vec3::Y * (side * 1.1) + Vec3::Z * 0.45,
                Vec2::new(0.06, 0.14),
                Vec2::new(0.06, 0.14),
            );
        }
    }
    // The bore split down its length: two keeled halves above and below.
    dark_plate(b);
    for up in [1.0f32, -1.0] {
        let out = Vec3::Z * up;
        let half = [
            ring(c(6.4) + out * 0.56, out, 0.7, 0.44),
            ring(c(9.4) + out * 0.5, out, 0.6, 0.38),
            ring(c(10.9) + out * 0.42, out, 0.44, 0.3),
            ring(c(FUSION_TIP) + out * 0.3, out, 0.16, 0.1),
        ];
        if b.fine() {
            sleeve(b, &half, &KEEL);
        } else {
            sleeve(b, &[half[0], half[2], half[3]], &KEEL);
        }
    }
    // Bronze ties across the split, holding the halves.
    metal(b);
    for x in [7.4f32, 9.0] {
        b.beam(
            c(x) - Vec3::Z * 0.8,
            c(x) + Vec3::Z * 0.8,
            Vec2::new(1.42, 0.22),
            Vec2::new(1.42, 0.22),
        );
    }
    // The fusion light between the halves, white-hot from the chamber to the mouth.
    b.paint(GLOW_PRISM);
    b.beam(
        c(6.4),
        c(FUSION_TIP - 0.2),
        Vec2::new(0.9, 0.2),
        Vec2::new(0.3, 0.12),
    );
    b.cylinder_between(c(FUSION_TIP - 0.6), c(FUSION_TIP), 0.16, 0.08, b.sides(8));
}
