//! The engine's works between and round the shards: plated gantries spanning from blade
//! to blade with conduits under them, coil drums between the boots, conduits running in
//! to the well, and the tall spike masts round the site with their stays.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::kit::{cable, dark_plate, metal, v3};
use super::super::machine::{armour, collar, strut, swept, Frame};
use super::super::shard::Shard;
use super::footing;

/// A gantry from shard `a` to shard `c`: a plated girder edge to edge at each of `levels`
/// (heights), a conduit under each, a diagonal from each level up to the next, and a
/// column at the middle carrying them, collared where they cross it.
pub(super) fn span(b: &mut MeshBuilder, a: &Shard, c: &Shard, levels: &[f32]) {
    let ends: Vec<(Vec3, Vec3)> = levels
        .iter()
        .map(|&z| {
            let p = a.edge_toward(z, c.root.lerp(c.tip, 0.2));
            let q = c.edge_toward(z, a.root.lerp(a.tip, 0.2));
            (p, q)
        })
        .collect();
    let g = |z: f32| 2.2 + z * 0.012;
    for (k, &(p, q)) in ends.iter().enumerate() {
        let z = levels[k];
        dark_plate(b);
        b.beam(
            p,
            q,
            Vec2::new(g(z), g(z) * 1.4),
            Vec2::new(g(z), g(z) * 1.4),
        );
        if b.fine() {
            metal(b);
            let low = Vec3::Z * (g(z) * 1.3);
            b.cylinder_between(p - low, q - low, g(z) * 0.32, g(z) * 0.32, 6);
        }
        if b.fine() {
            if let Some(&(p1, q1)) = ends.get(k + 1) {
                let (s, e) = if k % 2 == 0 { (p, q1) } else { (q, p1) };
                strut(b, s.lerp(e, 0.04), e.lerp(s, 0.04), g(z) * 0.35);
            }
        }
    }
    if b.coarse() {
        return;
    }
    let (Some(&(p0, q0)), Some(&(pn, qn))) = (ends.first(), ends.last()) else {
        return;
    };
    let foot = p0.lerp(q0, 0.5).with_z(1.0);
    let top = pn.lerp(qn, 0.5);
    dark_plate(b);
    b.beam(
        foot,
        top + Vec3::Z * 3.0,
        Vec2::splat(5.0),
        Vec2::splat(3.2),
    );
    for &(p, q) in &ends {
        let m = p.lerp(q, 0.5);
        collar(b, m, Vec3::Z, 3.4, 2.6);
    }
}

/// A coil drum lying between two boots: graphite, plated over its top, collared at its
/// ends.
pub(super) fn coil(b: &mut MeshBuilder, at: Vec3, axis: Vec3, r: f32, len: f32) {
    if b.coarse() {
        return;
    }
    let axis = axis.normalize();
    let half = axis * (len * 0.5);
    metal(b);
    let sides = b.sides(12);
    b.cylinder_between(at - half, at + half, r, r, sides);
    collar(b, at - half * 0.92, axis, r * 1.12, len * 0.08);
    collar(b, at + half * 0.92, axis, r * 1.12, len * 0.08);
    // Its saddles, and plates over its top swept along it.
    dark_plate(b);
    for s in [-0.6, 0.6] {
        let c = at + half * s;
        b.beam(
            c.with_z(0.0) + Vec3::Z * 0.1,
            c.with_z(at.z),
            Vec2::new(r * 2.2, len * 0.1),
            Vec2::new(r * 1.6, len * 0.1),
        );
    }
    if b.fine() {
        for s in [-1.0, 1.0] {
            let f = Frame::new(
                at - half * 0.85 * s + Vec3::Z * (r * 0.92),
                axis * s + Vec3::Z * 0.04,
                Vec3::Z,
            );
            armour(b, &f, &swept(len * 0.62, r * 0.7, 0.0, 0.6), 0.6);
        }
    }
}

/// A conduit run of `n` graphite pipes from `a` to `c` on the ground, on plated trestles.
pub(super) fn conduit(b: &mut MeshBuilder, a: Vec3, c: Vec3, n: usize, r: f32) {
    if b.coarse() {
        return;
    }
    let along = (c - a).with_z(0.0).normalize();
    let side = along.cross(Vec3::Z);
    let h = a.z.max(c.z) + r * 2.5;
    if b.fine() {
        metal(b);
        for k in 0..n {
            let o = side * ((k as f32 - (n - 1) as f32 * 0.5) * r * 2.6);
            b.cylinder_between(a.with_z(h) + o, c.with_z(h) + o, r, r, 6);
        }
    }
    dark_plate(b);
    let len = a.with_z(0.0).distance(c.with_z(0.0));
    let trestles = (len / 14.0).ceil().max(2.0) as usize;
    for k in 0..trestles {
        let p = a.lerp(c, (k as f32 + 0.5) / trestles as f32).with_z(0.0);
        b.beam(
            p,
            p + Vec3::Z * (h - r),
            Vec2::new(r * 2.6 * n as f32 + 1.0, 1.6),
            Vec2::new(r * 2.6 * n as f32 + 1.0, 1.2),
        );
    }
    if !b.fine() {
        b.beam(
            a.with_z(h),
            c.with_z(h),
            Vec2::new(r * 2.6 * n as f32, r * 2.0),
            Vec2::new(r * 2.6 * n as f32, r * 2.0),
        );
    }
}

/// A tall spike mast standing at `foot`, its point at `top`: three plated legs drawn in
/// toward the top, braced at every stage, a plated housing a third of the way up, and a
/// point of two swept plates back to back. Gives where its stays tie on: its top and its
/// housing.
pub(super) fn mast(
    b: &mut MeshBuilder,
    foot: Vec3,
    top: Vec3,
    spread: f32,
    seed: u32,
) -> [Vec3; 2] {
    let height = top.z - foot.z;
    let crown = top - Vec3::Z * (height * 0.14);
    let lean = (crown - foot).normalize();
    let leg = |i: usize, s: f32| -> Vec3 {
        let a = TAU * i as f32 / 3.0 + seed as f32 * 0.7;
        let d = v3(a.cos(), a.sin(), 0.0);
        foot.lerp(crown, s) + d * (spread * (1.0 - 0.8 * s))
    };
    let housing = foot.lerp(crown, 0.36);
    if b.coarse() {
        dark_plate(b);
        b.loft(
            &[vec![leg(0, 0.0), leg(1, 0.0), leg(2, 0.0)], vec![top; 3]],
            false,
            false,
        );
        return [top, housing];
    }
    let stages = if b.fine() { 7 } else { 3 };
    let r = spread * 0.12;
    for i in 0..3 {
        strut(b, leg(i, 0.0), leg(i, 1.0), r);
    }
    for s in 1..stages {
        let k = s as f32 / stages as f32;
        let w = r * (0.75 - 0.3 * k);
        for i in 0..3 {
            strut(b, leg(i, k), leg((i + 1) % 3, k), w);
            if b.fine() {
                let k0 = (s - 1) as f32 / stages as f32;
                strut(b, leg(i, k0), leg((i + 1) % 3, k), w * 0.7);
            }
        }
    }
    // The housing a third of the way up, and a graphite drum over it.
    dark_plate(b);
    let hw = spread * 0.9;
    b.with_facets(|b| {
        b.prism(
            housing - lean * (height * 0.04),
            6,
            hw,
            hw * 0.8,
            height * 0.08,
        );
    });
    collar(
        b,
        housing + lean * (height * 0.05),
        lean,
        hw * 0.55,
        height * 0.03,
    );
    // The point: two plates back to back, swept up past the legs.
    let up = (top - crown).normalize();
    let across = up.cross(Vec3::X).try_normalize().unwrap_or(Vec3::Y);
    let n = across.cross(up);
    for s in [-1.0, 1.0] {
        dark_plate(b);
        let f = Frame::new(
            leg(0, 0.9).lerp(leg(1, 0.9), 0.5).lerp(leg(2, 0.9), 0.33) + n * (s * 0.2),
            up,
            n * s,
        );
        armour(
            b,
            &f,
            &swept(top.distance(f.o), spread * 0.5, 0.0, 0.3),
            spread * 0.08,
        );
    }
    [top, housing]
}

/// A stay from `a` to `c`, hanging `sag` metres at its middle. Close up only.
pub(super) fn stay(b: &mut MeshBuilder, a: Vec3, c: Vec3, sag: f32, r: f32) {
    if !b.fine() {
        return;
    }
    let points: Vec<Vec3> = (0..=6)
        .map(|k| {
            let s = k as f32 / 6.0;
            a.lerp(c, s) - Vec3::Z * (sag * 4.0 * s * (1.0 - s))
        })
        .collect();
    metal(b);
    cable(b, &points, r);
}

/// Where a stay is made fast in the ground: a plated block with a graphite eye.
pub(super) fn anchor(b: &mut MeshBuilder, at: Vec3, toward: Vec3) {
    if b.coarse() {
        return;
    }
    let d = (toward - at).with_z(0.0).normalize_or(Vec3::X);
    dark_plate(b);
    footing(b, at - d * 3.0, at + d * 3.0, 2.5, (3.4, 2.6));
    if b.fine() {
        collar(b, at + d * 2.2 + Vec3::Z * 3.0, d.cross(Vec3::Z), 1.0, 2.0);
    }
}
