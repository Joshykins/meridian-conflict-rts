//! Shared pieces of the big walkers' limbs (the Behemoth's, the Breacher's): a bone's
//! frame, limbs lofted down it, joint drums, plates and rams laid on it, and rings about
//! a line along x for lofting gun bodies.

use std::f32::consts::TAU;

use glam::Vec3;

use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder};
use crate::material::*;
use crate::pattern;

const Y: Vec3 = Vec3::Y;

/// A far-off bar from `a` to `c`: three sides, open ends, `w0` and `w1` its size at each.
pub(super) fn tri_bar(b: &mut MeshBuilder, a: Vec3, c: Vec3, w0: f32, w1: f32) {
    let (_, across, face) = bone_frame(a, c);
    let ring = |p: Vec3, w: f32| {
        vec![
            p + face * w,
            p + across * w - face * (w * 0.6),
            p - across * w - face * (w * 0.6),
        ]
    };
    b.loft(&[ring(a, w0), ring(c, w1)], false, false);
}

/// A bone's frame from `a` to `c`: along it, across it (as near y as it can be), and its
/// face (square to both, the side toward +x).
pub(super) fn bone_frame(a: Vec3, c: Vec3) -> (Vec3, Vec3, Vec3) {
    let along = (c - a).normalize();
    let across = (Y - along * Y.dot(along)).normalize();
    let face = along.cross(across);
    (along, across, if face.x < 0.0 { -face } else { face })
}

/// A limb's cross-section at `p`: `w` either side across, `front` out along the face and
/// `back` behind it, corners cut `cut`.
pub(super) fn section(
    p: Vec3,
    across: Vec3,
    face: Vec3,
    w: f32,
    front: f32,
    back: f32,
    cut: f32,
) -> Vec<Vec3> {
    let cut = cut.max(0.01);
    [
        (w - cut, front),
        (w, front - cut),
        (w, -back + cut),
        (w - cut, -back),
        (-w + cut, -back),
        (-w, -back + cut),
        (-w, front - cut),
        (-w + cut, front),
    ]
    .iter()
    .map(|&(u, v)| p + across * u + face * v)
    .collect()
}

/// A limb lofted down the bone from `a` to `c` through sections `(t along it, w, front,
/// back, cut)`.
pub(super) fn limb(b: &mut MeshBuilder, a: Vec3, c: Vec3, sections: &[(f32, f32, f32, f32, f32)]) {
    let (_, across, face) = bone_frame(a, c);
    let rings: Vec<Vec<Vec3>> = sections
        .iter()
        .map(|&(t, w, f, k, cut)| section(a.lerp(c, t), across, face, w, f, k, cut))
        .collect();
    b.loft(&rings, true, true);
}

/// A joint drum across the leg at `at`: a dark drum, metal hubs either end, bolt heads round
/// the outer hub up close.
pub(super) fn joint(b: &mut MeshBuilder, at: Vec3, half: f32, r: f32) {
    let n = b.sides(16);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(at - Y * half, at + Y * half, r, r, n);
    b.paint(METAL);
    b.cylinder_between(at + Y * half, at + Y * (half + 0.9), r * 0.62, r * 0.55, n);
    b.cylinder_between(at - Y * (half + 0.7), at - Y * half, r * 0.5, r * 0.62, n);
    if b.fine() {
        for i in 0..10 {
            let a = i as f32 * TAU / 10.0;
            b.cuboid(
                at + v3(a.cos() * r * 0.8, half + 0.2, a.sin() * r * 0.8),
                v3(0.9, 0.6, 0.9),
            );
        }
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            at + Y * (half + 0.9),
            at + Y * (half + 1.5),
            r * 0.3,
            r * 0.3,
            10,
        );
    }
}

/// A plate laid on a limb from `t0` to `t1` of the bone `a`-`c`, on the side `out` of it
/// (`face`, `-face`, `across` in the bone's frame), `lift` off the bone's line and `shift`
/// across it, `(half width, thickness)` at each end.
pub(super) fn limb_plate(
    b: &mut MeshBuilder,
    (a, c): (Vec3, Vec3),
    (t0, t1): (f32, f32),
    out: Vec3,
    (lift0, lift1): (f32, f32),
    shift: Vec3,
    at_a: (f32, f32),
    at_c: (f32, f32),
) {
    armour(
        b,
        a.lerp(c, t0) + out * lift0 + shift,
        a.lerp(c, t1) + out * lift1 + shift,
        out,
        at_a,
        at_c,
    );
}

/// A sculpted armour plate from `a` to `c`, facing `out`: flat on the limb, bevelled to a
/// ridged face, `(half width, thickness)` at each end.
pub(super) fn armour(
    b: &mut MeshBuilder,
    a: Vec3,
    c: Vec3,
    out: Vec3,
    at_a: (f32, f32),
    at_c: (f32, f32),
) {
    let axis = (c - a).normalize();
    let side = out.cross(axis).normalize();
    let face = axis.cross(side).normalize() * out.dot(axis.cross(side)).signum();
    let ring = |p: Vec3, (w, t): (f32, f32)| -> Vec<Vec3> {
        [
            (-w, 0.0),
            (w, 0.0),
            (w, 0.45 * t),
            (0.5 * w, t),
            (-0.5 * w, t),
            (-w, 0.45 * t),
        ]
        .iter()
        .map(|&(s, u)| p + side * s + face * u)
        .collect()
    };
    b.loft(&[ring(a, at_a), ring(c, at_c)], true, true);
}

/// A hydraulic ram from `top` to `bottom`: a dark cylinder over the upper half, a bright
/// rod out of it.
pub(super) fn ram(b: &mut MeshBuilder, top: Vec3, bottom: Vec3, barrel: f32, rod: f32) {
    let sides = b.sides(8);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.cylinder_between(top, top.lerp(bottom, 0.56), barrel, barrel * 0.94, sides);
    b.paint(METAL);
    b.cylinder_between(top.lerp(bottom, 0.5), bottom, rod, rod, sides);
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.cylinder_between(
            top - (bottom - top).normalize() * 0.6,
            top + (bottom - top).normalize() * 0.9,
            barrel * 1.2,
            barrel * 1.2,
            8,
        );
    }
}

/// A ring of eight round a line along x at (`y`, `z`): an octagon `hy` by `hz` with its
/// corners cut `c`.
pub(super) fn x_ring(x: f32, y: f32, z: f32, hy: f32, hz: f32, c: f32) -> Vec<Vec3> {
    chamfered_rect(v2(hy, hz), c)
        .iter()
        .map(|p| v3(x, y + p[0], z + p[1]))
        .collect()
}

/// A round ring of `n` about a line along x at (`y`, `z`), `ry` across and `rz` high.
pub(super) fn o_ring(x: f32, y: f32, z: f32, ry: f32, rz: f32, n: usize) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let a = (i as f32 + 0.5) * TAU / n as f32;
            v3(x, y + a.cos() * ry, z + a.sin() * rz)
        })
        .collect()
}
