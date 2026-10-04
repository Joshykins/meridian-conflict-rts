//! The Breacher's gatling-breach cannon: a round armoured body with a breech hump on its
//! roof, an ammunition drum behind it (where the design has room for one), and six
//! barrels turning out of a shroud at its face. Authored along +x from its own origin on
//! the bore line; each design hangs it where it wants it (`gun`).

use std::f32::consts::TAU;

use glam::{Affine3A, Quat, Vec3};

use super::super::limbs::*;
use super::super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

const X: Vec3 = Vec3::X;
const Y: Vec3 = Vec3::Y;

/// How far the gun kicks back along its bore when it fires.
pub(super) const KICK: f32 = 0.5;
/// The gun is authored this long, breech face to muzzle, and drawn at `SCALE`.
const LENGTH: f32 = 30.0;
pub(super) const SCALE: f32 = 0.75;
/// The muzzle, along the bore from the gun's origin, as drawn.
pub(super) const REACH: f32 = LENGTH * SCALE;
/// The barrel cluster: its drum plate's face, the barrels' distance off the axis.
const CLUSTER: f32 = 11.6;
const BARREL_R: f32 = 2.6;

/// The weapon a gun is: the right (`side` -1) is weapon 1, the left weapon 2.
pub(super) fn weapon(side: f32) -> usize {
    if side < 0.0 {
        1
    } else {
        2
    }
}

/// One gun with its bore through `origin`, `side` -1 for the right, +1 for the left: the
/// body, the breech hump, the drum magazine behind if `drum`, and the turning cluster.
/// Called inside the design's gun house, under `with_recoil`.
pub(super) fn gun(b: &mut MeshBuilder, side: f32, origin: Vec3, drum: bool) {
    let frame =
        Affine3A::from_scale_rotation_translation(Vec3::splat(SCALE), Quat::IDENTITY, origin);
    b.with(frame, |b| {
        let (y, z) = (0.0, 0.0);
        let fine = b.fine();
        // The body: a faceted housing along the bore, in two plated sections with a dark
        // gap between them and a thin seam of light along its outboard flank.
        b.paint(PLATING);
        b.loft(
            &[
                x_ring(-11.0, y, z, 3.4, 3.2, 1.2),
                x_ring(-9.0, y, z, 4.8, 4.6, 1.8),
                x_ring(-2.2, y, z, 5.0, 4.8, 1.9),
            ],
            true,
            true,
        );
        b.loft(
            &[
                x_ring(-1.0, y, z, 5.3, 5.1, 2.0),
                x_ring(6.0, y, z, 5.3, 5.1, 2.0),
                x_ring(9.0, y, z, 4.8, 4.6, 1.8),
                x_ring(10.8, y, z, 4.3, 4.1, 1.6),
            ],
            true,
            true,
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.loft(
            &[
                x_ring(-2.4, y, z, 4.6, 4.4, 1.7),
                x_ring(-0.8, y, z, 4.6, 4.4, 1.7),
            ],
            true,
            true,
        );
        b.loft(
            &[
                x_ring(10.4, y, z, 4.6, 4.4, 1.7),
                x_ring(11.6, y, z, 4.4, 4.2, 1.6),
            ],
            true,
            true,
        );
        if fine {
            b.paint(GLOW).pattern(pattern::PLAIN);
            b.beam(
                v3(0.0, y + side * 5.32, z + 1.6),
                v3(8.0, y + side * 5.32, z + 1.6),
                v2(0.12, 0.25),
                v2(0.12, 0.25),
            );
        }
        breech(b, y, z);
        if drum {
            magazine(b, side, y, z);
        }
        // The spin motor under the body's front.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            v3(0.0, y, z - 5.3),
            v3(9.0, y, z - 4.7),
            1.6,
            1.4,
            b.sides(10),
        );
        // The ejection port in the outboard flank.
        let port = v3(1.0, y + side * 5.3, z - 0.6);
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(port + v3(-2.2, -0.35, -1.3), port + v3(2.2, 0.35, 1.3));
        b.paint(TREAD);
        b.block(port + v3(-1.5, -0.5, -0.6), port + v3(1.5, 0.5, 0.6));
        if fine {
            body_detail(b, side, y, z, drum);
        }
        b.with_spin(v3(0.0, y, z), |b| cluster(b, y, z));
    });
}

/// The breech hump on the body's roof: a long armoured block over the action, the loading
/// gate on its flank, vents along its crown.
fn breech(b: &mut MeshBuilder, y: f32, z: f32) {
    b.paint(PLATING);
    b.frustum(
        v3(-1.5, y, z + 4.2),
        v2(13.0, 5.6),
        v2(10.0, 4.0),
        2.4,
        v2(-0.6, 0.0),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(-6.0, y - 1.4, z + 6.5), v3(2.0, y + 1.4, z + 6.9));
    b.paint(METAL);
    for k in 0..5 {
        let x = -5.4 + 1.6 * k as f32;
        b.block(v3(x, y - 1.2, z + 6.8), v3(x + 0.6, y + 1.2, z + 7.1));
    }
}

/// The ammunition drum behind the body, its axis across the arm, and the feed chute from
/// it into the body's flank.
fn magazine(b: &mut MeshBuilder, side: f32, y: f32, z: f32) {
    let drum = v3(-15.0, y, z + 0.4);
    let n = b.sides(18);
    b.paint(PLATING);
    b.cylinder_between(drum - Y * 3.6, drum + Y * 3.6, 4.8, 4.8, n);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    let fine = b.fine();
    for s in [-1.0f32, 1.0].into_iter().filter(|_| fine) {
        b.cylinder_between(drum + Y * (s * 3.6), drum + Y * (s * 4.1), 4.0, 3.6, n);
    }
    // The chute: from the drum's underside forward into the body.
    b.paint(PLATING);
    b.beam(
        drum + v3(3.0, 0.0, -3.6),
        v3(-8.0, y, z - 4.4),
        v2(3.2, 1.8),
        v2(3.0, 1.6),
    );
    // A brace from the drum's hub up to the body's back.
    b.paint(METAL);
    b.cylinder_between(
        drum + Y * (side * 4.1),
        v3(-10.0, y + side * 3.2, z + 1.6),
        0.6,
        0.6,
        6,
    );
}

/// Up close: ribs round the drum, rails along the body, a hatch, cable runs.
fn body_detail(b: &mut MeshBuilder, side: f32, y: f32, z: f32, magazine: bool) {
    let drum = v3(-15.0, y, z + 0.4);
    b.paint(METAL);
    for k in (0..8).filter(|_| magazine) {
        let a = k as f32 * TAU / 8.0;
        let off = v3(a.cos(), 0.0, a.sin()) * 4.85;
        b.beam(
            drum + off - Y * 3.2,
            drum + off + Y * 3.2,
            v2(0.6, 0.4),
            v2(0.6, 0.4),
        );
    }
    b.paint(PLATING).pattern(pattern::PLAIN);
    for k in 0..8 {
        let a = (k as f32 + 0.5) * TAU / 8.0;
        let off = v3(0.0, a.cos() * 5.35, a.sin() * 5.15);
        if off.z > 3.0 {
            continue;
        }
        b.beam(
            v3(-8.0, y, z) + off,
            v3(-5.2, y, z) + off,
            v2(0.6, 0.4),
            v2(0.6, 0.4),
        );
    }
    b.paint(METAL);
    for s in [-1.0f32, 1.0] {
        b.cylinder_between(
            v3(6.0, y + s * 1.4, z - 5.5),
            v3(-9.0, y + s * 1.4, z - 4.6),
            0.35,
            0.35,
            6,
        );
    }
    b.paint(TEAM).pattern(pattern::PLAIN);
    b.block(
        v3(-8.6, y + side * 5.0 - 0.3, z + 1.0),
        v3(-3.4, y + side * 5.0 + 0.3, z + 2.2),
    );
}

/// What turns, about the bore line at (`y`, `z`): the drum plate, the six barrels, the
/// collars clamping them, and the hub out to the muzzle.
fn cluster(b: &mut MeshBuilder, y: f32, z: f32) {
    let axis = v3(0.0, y, z);
    let fine = b.fine();
    let n = b.sides(12);
    let end = LENGTH;
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.cylinder_between(
        axis + X * (CLUSTER - 0.2),
        axis + X * (CLUSTER + 1.4),
        4.3,
        4.1,
        n,
    );
    b.paint(METAL);
    b.cylinder_between(
        axis + X * (CLUSTER + 1.4),
        axis + X * (end - 1.2),
        0.9,
        0.8,
        8,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for (x0, x1) in [(18.0, 19.2), (25.0, 26.0)].into_iter().filter(|_| fine) {
        b.cylinder_between(axis + X * x0, axis + X * x1, 3.7, 3.7, n);
    }
    b.cylinder_between(axis + X * (end - 1.2), axis + X * end, 1.2, 1.1, 8);
    if fine {
        b.paint(METAL);
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0;
            let off = v3(0.0, a.cos(), a.sin()) * 3.4;
            b.cylinder_between(
                axis + off + X * (CLUSTER + 1.3),
                axis + off + X * (CLUSTER + 1.8),
                0.45,
                0.45,
                6,
            );
        }
    }
    for k in 0..6 {
        let a = (k as f32 + 0.5) * TAU / 6.0;
        b.with(
            Affine3A::from_translation(axis) * Affine3A::from_rotation_x(a),
            |b| barrel(b, BARREL_R, end),
        );
    }
}

/// One barrel `r` off the cluster's axis (local +z), running along x out to `end`: a jacket
/// out of the drum plate, the tube, a slotted brake at the muzzle.
fn barrel(b: &mut MeshBuilder, r: f32, end: f32) {
    let fine = b.fine();
    let n = b.sides(10);
    let at = |x: f32| v3(x, 0.0, r);
    b.paint(PLATING);
    if fine {
        b.cylinder_between(at(CLUSTER + 1.4), at(16.5), 1.15, 1.05, n);
    }
    b.paint(METAL);
    let breech = if fine { 16.5 } else { CLUSTER + 1.4 };
    b.cylinder_between(at(breech), at(end - 2.0), 0.8, 0.72, n);
    b.paint(PLATING);
    b.chamfered_box(at(end - 1.0), v3(2.0, 1.8, 1.8), 0.5);
    if fine {
        b.paint(TREAD);
        for x in [end - 1.5, end - 0.7] {
            b.block(
                at(x) + v3(-0.25, -0.95, -0.45),
                at(x) + v3(0.25, 0.95, 0.45),
            );
        }
    }
    b.paint(TREAD);
    b.cylinder_between(at(end - 0.02), at(end + 0.03), 0.5, 0.5, n);
}
