//! The commander's legs: reverse-kneed, each bone a bronze frame under dark plates that
//! overlap and sweep back, rams working in the gaps, on a foot of three talons and a spur.
//! Every piece rides its bone (`rig::THIGH`, `SHIN`, `TARSUS`, `FOOT`) so the shader's
//! `hock_leg` poses it.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::plating::{joint, piston, side_plate};
use super::{ANKLE, HIP, HOCK, KNEE};

/// The left leg (the right is drawn by mirroring it).
pub(super) fn leg(b: &mut MeshBuilder) {
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, thigh);
        b.with_limb(rig::SHIN, shin);
        b.with_limb(rig::TARSUS, tarsus);
        b.with_limb(rig::FOOT, foot);
    });
}

/// A bone in the leg's (x, z) plane from `a` to `c`: its unit direction, and the unit
/// normal on the side `toward` points to.
fn bone(a: Vec3, c: Vec3, toward: Vec2) -> (Vec2, Vec2) {
    let u = Vec2::new(c.x - a.x, c.z - a.z).normalize();
    let n = Vec2::new(-u.y, u.x);
    (u, if n.dot(toward) < 0.0 { -n } else { n })
}

fn xz(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}

/// A profile for `side_plate` from points in the (x, z) plane.
fn profile(points: &[Vec2]) -> Vec<[f32; 2]> {
    points.iter().map(|p| [p.x, p.y]).collect()
}

/// The thigh: a hip drum, a bronze beam to the knee under a cuisse whose top sweeps back
/// past the hip into a spike, a front plate, and a ram down its back.
fn thigh(b: &mut MeshBuilder) {
    let (a, c) = (xz(HIP), xz(KNEE));
    let (u, n) = bone(HIP, KNEE, Vec2::new(1.0, 1.0));
    joint(b, HIP, Vec3::Y * 0.8, 0.8);
    metal(b);
    b.beam(HIP, KNEE, Vec2::new(0.9, 0.9), Vec2::new(0.7, 0.7));
    dark_plate(b);
    // The cuisse, over the outside.
    side_plate(
        b,
        &profile(&[
            a + n * 0.9 + u * 0.2,
            c + n * 0.75 - u * 0.3,
            c + n * 0.1 + u * 0.15,
            c - n * 0.55 - u * 0.25,
            a - n * 0.7 + u * 0.9,
            a - n * 1.25 - u * 0.95,
            a + n * 0.15 - u * 0.35,
        ]),
        3.05,
        3.35,
    );
    // The front plate, the gap between it and the cuisse showing the beam.
    side_plate(
        b,
        &profile(&[
            a + n * 0.45 + u * 0.35,
            c + n * 0.35 - u * 0.45,
            c + n * 0.72 - u * 0.4,
            a + n * 0.85 + u * 0.15,
        ]),
        2.05,
        2.95,
    );
    // The ram that swings the thigh, down its back.
    let at = |p: Vec2, y: f32| v3(p.x, y, p.y);
    piston(
        b,
        at(a - n * 0.75 + u * 0.4, 2.6),
        at(c - n * 0.5 - u * 0.2, 2.8),
        0.18,
    );
}

/// The shin, the long bone that shows most: twin bronze struts from knee to hock, a ram
/// along their front, a knee cap, and two greaves over the back overlapping down it.
fn shin(b: &mut MeshBuilder) {
    let (k, h) = (xz(KNEE), xz(HOCK));
    let (u, n) = bone(KNEE, HOCK, Vec2::new(-1.0, 1.0));
    joint(b, KNEE, Vec3::Y * 0.65, 0.62);
    metal(b);
    let sides = b.sides(6);
    for dy in [-0.36f32, 0.36] {
        b.cylinder_between(KNEE + Vec3::Y * dy, HOCK + Vec3::Y * dy, 0.2, 0.18, sides);
    }
    let at = |p: Vec2, y: f32| v3(p.x, y, p.y);
    piston(
        b,
        at(k - n * 0.5 + u * 0.7, 3.0),
        at(h - n * 0.45 - u * 0.7, 3.2),
        0.22,
    );
    dark_plate(b);
    // The knee cap, its top edge standing up over the thigh's front plate.
    side_plate(
        b,
        &profile(&[
            k + Vec2::new(0.55, 0.85),
            k + Vec2::new(0.95, 0.05),
            k + Vec2::new(0.6, -0.9),
            k + Vec2::new(0.15, -0.55),
            k + Vec2::new(0.05, 0.35),
            k + Vec2::new(-0.15, 1.45),
        ]),
        2.3,
        3.5,
    );
    // The lower greave, then the upper one lapped over its top.
    side_plate(
        b,
        &profile(&[
            k + n * 0.3 + u * 0.6,
            k + n * 0.9 + u * 1.3,
            h + n * 0.95 - u * 0.9,
            h + n * 1.0 + u * 0.1,
            h + n * 0.35 - u * 0.45,
        ]),
        2.55,
        3.55,
    );
    side_plate(
        b,
        &profile(&[
            k + n * 0.7 + u * 0.75,
            k + n * 1.2 + u * 1.2,
            k + n * 1.25 + u * 3.3,
            k + n * 1.45 + u * 3.9,
            k + n * 0.85 + u * 3.3,
        ]),
        2.65,
        3.45,
    );
    if b.fine() {
        // A red line in the seam between the greaves.
        b.paint(GLOW_LASER);
        let (p, q) = (k + n * 1.02 + u * 1.5, k + n * 1.05 + u * 3.1);
        b.beam(
            at(p, 3.46),
            at(q, 3.46),
            Vec2::new(0.05, 0.05),
            Vec2::new(0.05, 0.05),
        );
        // Cables down the struts.
        metal(b);
        b.cylinder_between(
            at(k - n * 0.15 + u * 0.4, 3.45),
            at(h - n * 0.1 - u * 0.4, 3.6),
            0.08,
            0.08,
            5,
        );
    }
}

/// The tarsus, hock to ankle: the hock drum, a bronze beam under a front plate, and the
/// hock's spur, a plate swept back behind the joint.
fn tarsus(b: &mut MeshBuilder) {
    let (h, a) = (xz(HOCK), xz(ANKLE));
    let (u, n) = bone(HOCK, ANKLE, Vec2::new(1.0, 1.0));
    joint(b, HOCK, Vec3::Y * 0.55, 0.55);
    metal(b);
    b.beam(HOCK, ANKLE, Vec2::new(0.75, 0.75), Vec2::new(0.6, 0.6));
    dark_plate(b);
    side_plate(
        b,
        &profile(&[
            h + n * 0.3 + u * 0.35,
            h + n * 0.78 + u * 0.55,
            a + n * 0.62 - u * 0.5,
            a + n * 0.25 - u * 0.3,
        ]),
        2.85,
        3.75,
    );
    side_plate(
        b,
        &profile(&[
            h + Vec2::new(0.25, 0.55),
            h + Vec2::new(-0.45, 0.5),
            h + Vec2::new(-1.65, 0.25),
            h + Vec2::new(-0.55, -0.45),
            h + Vec2::new(0.45, -0.65),
        ]),
        2.95,
        3.45,
    );
    if b.fine() {
        let at = |p: Vec2, y: f32| v3(p.x, y, p.y);
        piston(
            b,
            at(h - n * 0.55 + u * 0.45, 3.3),
            at(a - n * 0.4 - u * 0.5, 3.3),
            0.14,
        );
    }
}

/// The foot: an ankle drum over a bronze hub, three talons splayed forward to the ground
/// and a spur behind, each a dark plate over a bronze knuckle.
fn foot(b: &mut MeshBuilder) {
    let y = ANKLE.y;
    joint(b, ANKLE, Vec3::Y * 0.5, 0.45);
    metal(b);
    b.chamfered_box(v3(0.3, y, 0.8), v3(1.5, 1.3, 0.9), 0.3);
    seam(b);
    b.block(v3(-0.9, y - 0.6, 0.0), v3(1.3, y + 0.6, 0.35));
    for (dy, reach) in [(-0.8f32, 2.85f32), (0.0, 3.3), (0.8, 2.85)] {
        let root = v3(0.7, y + dy * 0.35, 0.95);
        let knuckle = v3(0.7 + reach * 0.55, y + dy * 0.9, 0.62);
        let tip = v3(reach, y + dy * 1.1, 0.035);
        dark_plate(b);
        if !b.fine() {
            b.beam(root, tip, Vec2::new(0.55, 0.5), Vec2::new(0.06, 0.06));
            continue;
        }
        b.beam(root, knuckle, Vec2::new(0.55, 0.5), Vec2::new(0.42, 0.4));
        b.beam(knuckle, tip, Vec2::new(0.4, 0.38), Vec2::new(0.06, 0.06));
        metal(b);
        b.cylinder_between(
            knuckle - Vec3::Y * 0.26,
            knuckle + Vec3::Y * 0.26,
            0.22,
            0.22,
            6,
        );
    }
    dark_plate(b);
    b.beam(
        v3(-0.2, y, 0.95),
        v3(-1.9, y, 0.05),
        Vec2::new(0.5, 0.45),
        Vec2::new(0.08, 0.08),
    );
}
