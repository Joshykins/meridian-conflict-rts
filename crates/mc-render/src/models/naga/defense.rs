//! The Naga's defences, each on a one-cell lot (12 m square): the Barb (point defence),
//! the Thornspitter (flak) and the Thornwall (wall section). The two guns are stalks grown
//! out of the ground, the head the turret; they are told apart at a glance by their heads
//! and what their thorns do.
//!
//! - Barb: a plated mound of six arched plates, roots crawling out between them, a stalk
//!   barbed downward like a rose stem and a hooded head that levels a long segmented
//!   stinger, reverse barbs down it and a red-lit tip (the muzzle). A brawler: low, heavy,
//!   pointed at the ground it holds.
//! - Thornspitter: three arched buttresses holding a stalk thorned upward, and a plated
//!   gland of a head under a crown of long quills splayed at the sky, their tips burning
//!   red, twin flared throats bristling with needles. From above it is a starburst.
//! - Thornwall: a bank of soft hide on a footing that fills the lot to its edges, so
//!   sections set side by side on the grid join along x and y (as ARC's wall does), black
//!   plates overlapping like scales on its four faces and thorns raked out of the gaps.
//!
//! The numbers match `data/factions/naga/units/structures.ron`: a turret's pivot is the
//! weapon's `pivot` and its gun's tip the `muzzle`.

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use crate::models::builder::{MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, rig};

use super::kit::*;

const BARB_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 6.0);
const BARB_MUZZLE: Vec3 = Vec3::new(5.2, 0.0, 6.8);
const SPIT_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 6.2);
const SPIT_MUZZLE: Vec3 = Vec3::new(4.4, 0.0, 7.4);

/// A horizontal ring of `n` points of radius `r` about the z axis at height `z`.
fn disc(n: usize, r: f32, z: f32) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let a = (i as f32 + 0.5) * TAU / n as f32;
            v3(a.cos() * r, a.sin() * r, z)
        })
        .collect()
}

/// A root crawling from a stalk's foot out onto the lot at `angle`: a tendon of soft hide
/// arched from radius `r0`, height `z0` down into the ground at `r1`, half as wide as `w`
/// at its root, plated over, a bare claw where it bites in.
pub(super) fn root(b: &mut MeshBuilder, angle: f32, r0: f32, z0: f32, r1: f32, w: f32) {
    let at = |t: f32| {
        let wt = w * (1.0 - 0.7 * t);
        let z = (z0 * (1.0 - t).powf(1.6)).max(wt * 0.85);
        (v3(r0 + (r1 - r0) * t, 0.0, z), wt, wt * 0.8)
    };
    b.yawed(Vec3::ZERO, angle, |b| {
        under_hide(b);
        let points: Vec<_> = [0.0, 0.35, 0.7, 1.0].into_iter().map(at).collect();
        segment(b, &points, Vec3::Z);
        if !b.mid() {
            return;
        }
        hide(b);
        let plate = |t0: f32, t1: f32| {
            let [(a, wa, ha), (c, wc, hc)] = [at(t0), at(t1)];
            [(a + Vec3::Z * (ha * 0.5), wa * 1.15, wa * 0.5), (c + Vec3::Z * (hc * 0.5), wc * 1.15, wc * 0.5)]
        };
        shell(b, &plate(0.08, 0.42), Vec3::Z);
        if b.fine() {
            shell(b, &plate(0.52, 0.82), Vec3::Z);
            metal(b);
            let (tip, wt, _) = at(1.0);
            spike(b, tip - Vec3::X * (wt * 0.6), v3(tip.x + wt * 1.6, 0.0, 0.02), wt * 0.45);
        }
    });
}

/// A knot of soft hide at a stalk's foot, `r` wide and `h` high, with `plates` arched hide
/// plates laid down its slope (the first at `turn`), red seams showing between them.
pub(super) fn knot(b: &mut MeshBuilder, r: f32, h: f32, plates: usize, turn: f32) {
    let n = b.sides(12);
    under_hide(b);
    b.loft(&[disc(n, r, 0.0), disc(n, r * 0.9, h * 0.38), disc(n, r * 0.6, h * 0.78), disc(n, r * 0.34, h)], true, true);
    hide(b);
    let hint = v3(h, 0.0, r).normalize();
    for k in 0..plates {
        b.yawed(Vec3::ZERO, turn + TAU * k as f32 / plates as f32, |b| {
            shell(b, &[(v3(r * 0.98, 0.0, h * 0.12), r * 0.5, r * 0.12), (v3(r * 0.5, 0.0, h * 0.82), r * 0.3, r * 0.09)], hint);
        });
    }
}

/// A segmented stalk from `z0` to `z1` in `count` segments, radius `r0` at the foot to `r1`
/// at the top: each a core of soft hide sleeved in `splints` hide plates standing apart
/// (turned half a gap from the segment below), a bare vertebra drum at every joint.
/// Returns each segment's bottom, top and radius, for the thorns a model hangs on it.
pub(super) fn stalk(b: &mut MeshBuilder, z0: f32, z1: f32, r0: f32, r1: f32, count: usize, splints: usize) -> Vec<(f32, f32, f32)> {
    let step = (z1 - z0) / count as f32;
    let sides = b.sides(8);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let (lo, hi) = (z0 + step * i as f32, z0 + step * (i + 1) as f32);
        let r = r0 + (r1 - r0) * (i as f32 + 0.5) / count as f32;
        under_hide(b);
        b.prism(v3(0.0, 0.0, lo), sides, r * 0.8, r * 0.7, step);
        metal(b);
        b.prism(v3(0.0, 0.0, lo - 0.12), sides, r * 1.02, r * 0.96, 0.34);
        hide(b);
        let turn = i as f32 * PI / splints as f32;
        for k in 0..splints {
            let a = turn + TAU * k as f32 / splints as f32;
            let dir = v3(a.cos(), a.sin(), 0.0);
            shell(b, &[(dir * (r * 0.6) + Vec3::Z * (lo + 0.32), r * 0.6, r * 0.34), (dir * (r * 0.52) + Vec3::Z * (hi - 0.22), r * 0.52, r * 0.3)], dir);
        }
        out.push((lo, hi, r));
    }
    metal(b);
    b.prism(v3(0.0, 0.0, z1 - 0.12), sides, r1 * 1.02, r1 * 0.9, 0.34);
    out
}

// ---- Barb: point defence ------------------------------------------------------------

pub fn barb(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(BARB_PIVOT);
    b.set_arm_pivot(BARB_PIVOT);
    if b.coarse() {
        barb_coarse(b);
        return;
    }
    barb_mound(b);
    let segments = stalk(b, 2.3, 5.3, 0.9, 0.66, 3, 3);
    if b.fine() {
        // Barbs down the stalk, hooked at the ground like a rose stem's.
        hide(b);
        for (i, &(lo, hi, r)) in segments.iter().enumerate() {
            let z = lo + (hi - lo) * 0.62;
            for k in 0..3 {
                let a = (i as f32 + 0.5) * PI / 3.0 + TAU * k as f32 / 3.0;
                let dir = v3(a.cos(), a.sin(), 0.0);
                spike(b, dir * (r * 0.72) + Vec3::Z * z, dir * (r + 0.85) + Vec3::Z * (z - 0.75), 0.2);
            }
        }
    }
    b.with_part(part::TURRET, |b| {
        barb_head(b);
        b.with_limb(rig::ARM_GUN, barb_gun);
    });
}

/// Far off: the mound, the stalk, the head and the stinger.
fn barb_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(Vec3::ZERO, Vec2::splat(9.0), Vec2::splat(2.6), 2.5, Vec2::ZERO);
    b.frustum_open(v3(0.0, 0.0, 2.5), Vec2::splat(1.5), Vec2::splat(1.1), 3.1, Vec2::ZERO);
    b.with_part(part::TURRET, |b| {
        hide(b);
        b.beam(v3(-2.9, 0.0, 6.4), v3(1.5, 0.0, 6.5), Vec2::new(2.2, 1.7), Vec2::new(1.6, 1.3));
        b.with_limb(rig::ARM_GUN, |b| {
            hide(b);
            b.beam(v3(0.6, 0.0, 6.1), BARB_MUZZLE, Vec2::new(1.1, 0.9), Vec2::new(0.12, 0.12));
        });
        b.paint(TEAM);
        b.decal(v3(-1.3, 0.0, 7.3), Vec2::new(1.6, 1.2));
    });
}

/// The rooted mound: soft hide under six arched plates with the red seams between them,
/// roots crawling out through the gaps.
fn barb_mound(b: &mut MeshBuilder) {
    let n = b.sides(12);
    under_hide(b);
    b.loft(&[disc(n, 4.3, 0.0), disc(n, 4.0, 0.9), disc(n, 3.1, 1.9), disc(n, 1.7, 2.6), disc(n, 1.0, 2.75)], true, true);
    b.radial(6, |b| {
        hide(b);
        let hint = v3(0.75, 0.0, 0.66);
        shell(b, &[(v3(4.35, 0.0, 0.42), 1.55, 0.36), (v3(3.35, 0.0, 1.55), 1.4, 0.42), (v3(2.0, 0.0, 2.55), 0.9, 0.32)], hint);
        if b.fine() {
            // A lit seam along the plate's spine and a rib standing off it.
            b.paint(GLOW_LASER);
            b.beam(v3(3.85, 0.0, 1.35), v3(2.75, 0.0, 2.3), Vec2::new(0.1, 0.1), Vec2::new(0.1, 0.1));
            hide(b);
            blade(b, v3(3.0, 0.0, 2.25), v3(4.2, 0.0, 2.6), 0.28, Vec3::Y);
        }
    });
    for k in 0..6 {
        root(b, (k as f32 + 0.5) * TAU / 6.0, 3.3, 1.05, 5.45, 0.62);
    }
    if b.fine() {
        // Tendons from the stalk's foot out over the plates.
        metal(b);
        b.radial(3, |b| cable(b, &[v3(0.8, 0.45, 2.6), v3(1.9, 0.9, 2.45), v3(2.9, 1.3, 1.85)], 0.12));
    }
}

/// The head: a bearing drum, a trunnion drum across the pivot, a hood of plates with a
/// crest raked back, cheek guards flanking the stinger's root, red eyes under the hood.
fn barb_head(b: &mut MeshBuilder) {
    metal(b);
    b.prism(v3(0.0, 0.0, 5.25), b.sides(10), 1.2, 1.05, 0.38);
    knuckle(b, BARB_PIVOT, Vec3::Y, 0.55, 2.3);
    under_hide(b);
    segment(b, &[(v3(0.9, 0.0, 6.3), 0.7, 0.6), (v3(-0.8, 0.0, 6.5), 1.05, 0.8), (v3(-2.4, 0.0, 6.4), 0.75, 0.6), (v3(-3.1, 0.0, 6.2), 0.3, 0.3)], Vec3::Z);
    hide(b);
    shell(b, &[(v3(1.7, 0.0, 6.95), 0.95, 0.32), (v3(0.2, 0.0, 7.15), 1.4, 0.6), (v3(-1.6, 0.0, 7.2), 1.25, 0.55), (v3(-2.9, 0.0, 6.9), 0.6, 0.3)], Vec3::Z);
    b.mirror_y(|b| {
        hide(b);
        shell(b, &[(v3(1.3, 1.3, 6.1), 0.55, 0.28), (v3(-0.4, 1.45, 6.35), 0.85, 0.38), (v3(-2.0, 1.2, 6.45), 0.55, 0.28)], v3(0.0, 1.0, 0.25));
        b.paint(GLOW_LASER);
        b.beam(v3(1.6, 0.72, 6.62), v3(1.35, 1.12, 6.56), Vec2::new(0.24, 0.16), Vec2::new(0.2, 0.12));
        if b.fine() {
            b.beam(v3(1.25, 1.3, 6.9), v3(1.05, 1.5, 6.82), Vec2::new(0.16, 0.12), Vec2::new(0.12, 0.1));
        }
    });
    if b.fine() {
        hide(b);
        spike(b, v3(-0.3, 0.0, 7.6), v3(-2.2, 0.0, 8.55), 0.34);
        spike(b, v3(-1.7, 0.0, 7.55), v3(-3.3, 0.0, 8.05), 0.28);
        b.mirror_y(|b| blade(b, v3(-2.2, 1.0, 6.9), v3(-3.6, 1.6, 7.4), 0.28, v3(0.0, 1.0, 0.3)));
        // The stinger's ram: the cylinder under the head, its rod on the gun.
        under_hide(b);
        b.cylinder_between(v3(-1.6, 0.0, 5.75), v3(0.1, 0.0, 5.55), 0.22, 0.22, 6);
        metal(b);
        b.radial(2, |b| cable(b, &[v3(-2.6, 0.5, 6.0), v3(-1.2, 0.95, 5.55), v3(0.0, 0.9, 5.45)], 0.1));
    }
    // The owner's colour: a chevron on the hood.
    b.paint(TEAM);
    b.mirror_y(|b| b.beam(v3(-0.55, 0.22, 7.74), v3(-1.45, 0.85, 7.62), Vec2::new(0.36, 0.07), Vec2::new(0.36, 0.07)));
}

/// A point on the stinger's line, `x` metres out from the pivot toward the muzzle.
fn sting(x: f32) -> Vec3 {
    BARB_PIVOT + v3(x, 0.0, x * (BARB_MUZZLE.z - BARB_PIVOT.z) / BARB_MUZZLE.x)
}

/// The stinger: three plated segments on bare metal necks, reverse barbs down it, three
/// hooked barbs round a red tip (three bolts a shot).
fn barb_gun(b: &mut MeshBuilder) {
    let dir = (BARB_MUZZLE - BARB_PIVOT).normalize();
    let (side, up) = frame(dir, Vec3::Z);
    let parts: [(f32, f32, f32, f32); 3] = [(0.35, 1.9, 0.62, 0.55), (2.15, 3.45, 0.5, 0.42), (3.7, 4.7, 0.4, 0.26)];
    for (i, &(x0, x1, w0, w1)) in parts.iter().enumerate() {
        under_hide(b);
        segment(b, &[(sting(x0), w0, w0 * 0.88), (sting(x1), w1, w1 * 0.88)], Vec3::Z);
        hide(b);
        shell(b, &[(sting(x0 + 0.1) + up * (w0 * 0.3), w0 * 1.08, w0 * 0.6), (sting(x1 - 0.1) + up * (w1 * 0.3), w1 * 1.08, w1 * 0.6)], Vec3::Z);
        if i + 1 < parts.len() {
            metal(b);
            b.cylinder_between(sting(x1 - 0.1), sting(parts[i + 1].0 + 0.1), w1 * 0.66, w1 * 0.66, b.sides(8));
        }
        if b.fine() {
            hide(b);
            b.mirror_y(|b| blade(b, sting(x1 - 0.2) + side * (w1 * 0.7), sting(x1 - 1.0) + side * (w1 + 0.55) + up * 0.15, 0.2, up));
            b.paint(GLOW_LASER);
            b.mirror_y(|b| b.beam(sting(x0 + 0.25) + side * (w0 * 0.9), sting(x1 - 0.25) + side * (w1 * 0.9), Vec2::new(0.07, 0.08), Vec2::new(0.07, 0.08)));
        }
    }
    metal(b);
    b.cylinder_between(sting(4.6), sting(5.0), 0.27, 0.13, b.sides(8));
    b.paint(GLOW_LASER);
    b.cylinder_between(sting(4.95), BARB_MUZZLE, 0.15, 0.03, b.sides(6));
    if b.mid() {
        // The three hooked barbs round the tip, raked back like an arrowhead's.
        hide(b);
        for k in 0..3 {
            let a = PI * 0.5 + TAU * k as f32 / 3.0;
            let r = side * a.cos() + up * a.sin();
            spike(b, sting(4.75) + r * 0.2, sting(4.1) + r * 0.62, 0.12);
        }
    }
}

// ---- Thornspitter: anti-air flak ----------------------------------------------------

pub fn spitter(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(SPIT_PIVOT);
    b.set_arm_pivot(SPIT_PIVOT);
    if b.coarse() {
        spitter_coarse(b);
        return;
    }
    spitter_base(b);
    let segments = stalk(b, 2.2, 5.3, 0.78, 0.6, 3, 3);
    if b.mid() {
        // Thorns up the stalk, raked at the sky.
        hide(b);
        for (i, &(lo, hi, r)) in segments.iter().enumerate() {
            let z = lo + (hi - lo) * 0.45;
            let count = if b.fine() { 3 } else { 1 };
            for k in 0..count {
                let a = i as f32 * 1.1 + TAU * k as f32 / 3.0;
                let dir = v3(a.cos(), a.sin(), 0.0);
                spike(b, dir * (r * 0.7) + Vec3::Z * z, dir * (r + 0.6) + Vec3::Z * (z + 1.05), 0.2);
            }
        }
    }
    b.with_part(part::TURRET, |b| {
        spitter_head(b);
        quills(b);
        b.with_limb(rig::ARM_GUN, spitter_throats);
    });
}

/// Far off: a tapered stalk, the head, three quills of the crown and the throats in one.
fn spitter_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(Vec3::ZERO, Vec2::splat(8.6), Vec2::splat(1.2), 5.4, Vec2::ZERO);
    b.with_part(part::TURRET, |b| {
        hide(b);
        b.beam(v3(-2.8, 0.0, 6.6), v3(1.0, 0.0, 6.5), Vec2::new(2.6, 2.2), Vec2::new(2.2, 1.8));
        for k in 0..3 {
            let a = PI / 3.0 + TAU * k as f32 / 3.0;
            let dir = v3(a.cos(), a.sin(), 0.0);
            spike(b, v3(-0.8, 0.0, 7.5) + dir * 0.5, v3(-0.8, 0.0, 9.5) + dir * 2.0, 0.45);
        }
        b.with_limb(rig::ARM_GUN, |b| {
            hide(b);
            b.beam(v3(0.5, 0.0, 6.35), SPIT_MUZZLE, Vec2::new(1.8, 0.8), Vec2::new(0.5, 0.4));
        });
        b.paint(TEAM);
        b.decal(v3(-2.0, 0.0, 7.72), Vec2::new(1.4, 1.4));
    });
}

/// Three arched buttresses planted round a knot of hide, lesser roots between them.
fn spitter_base(b: &mut MeshBuilder) {
    knot(b, 2.5, 2.4, 3, PI / 3.0);
    b.radial(3, |b| {
        under_hide(b);
        let spine = [
            (v3(0.5, 0.0, 4.3), 0.48, 0.42),
            (v3(1.9, 0.0, 3.7), 0.55, 0.5),
            (v3(3.3, 0.0, 2.35), 0.55, 0.48),
            (v3(4.4, 0.0, 0.95), 0.45, 0.4),
            (v3(5.0, 0.0, 0.34), 0.28, 0.27),
        ];
        segment(b, &spine, Vec3::Z);
        hide(b);
        shell(b, &[(v3(0.9, 0.0, 4.6), 0.6, 0.34), (v3(2.2, 0.0, 3.98), 0.66, 0.4)], Vec3::Z);
        shell(b, &[(v3(2.8, 0.0, 3.3), 0.64, 0.38), (v3(4.0, 0.0, 1.85), 0.56, 0.34)], Vec3::Z);
        if b.fine() {
            knuckle(b, v3(2.5, 0.0, 3.35), Vec3::Y, 0.42, 1.35);
            ram(b, v3(0.55, 0.0, 1.5), v3(2.9, 0.0, 2.35), 0.14);
            b.paint(GLOW_LASER);
            b.beam(v3(3.2, 0.0, 2.95), v3(4.1, 0.0, 1.6), Vec2::new(0.1, 0.1), Vec2::new(0.1, 0.1));
            metal(b);
            b.mirror_y(|b| spike(b, v3(4.7, 0.25, 0.55), v3(5.6, 0.75, 0.02), 0.16));
        }
    });
    for k in 0..3 {
        root(b, PI / 3.0 + TAU * k as f32 / 3.0, 1.8, 1.25, 4.6, 0.45);
    }
}

/// The head: a bearing drum, a gland of soft hide wrapped in three plates with red gaps
/// between them, a trunnion across the pivot, and a chevron of the owner's colour.
fn spitter_head(b: &mut MeshBuilder) {
    metal(b);
    b.prism(v3(0.0, 0.0, 5.25), b.sides(10), 1.05, 0.95, 0.4);
    knuckle(b, SPIT_PIVOT, Vec3::Y, 0.5, 2.5);
    under_hide(b);
    segment(b, &[(v3(1.0, 0.0, 6.4), 0.9, 0.8), (v3(-0.5, 0.0, 6.7), 1.45, 1.2), (v3(-2.1, 0.0, 6.6), 1.2, 1.0), (v3(-2.9, 0.0, 6.4), 0.45, 0.45)], Vec3::Z);
    hide(b);
    shell(b, &[(v3(0.7, 0.0, 7.35), 0.75, 0.3), (v3(-0.6, 0.0, 7.7), 1.0, 0.36), (v3(-2.2, 0.0, 7.4), 0.8, 0.3)], Vec3::Z);
    b.mirror_y(|b| {
        hide(b);
        shell(b, &[(v3(0.6, 1.1, 6.45), 0.6, 0.3), (v3(-0.7, 1.45, 6.6), 0.95, 0.36), (v3(-2.2, 1.1, 6.5), 0.62, 0.26)], v3(0.0, 1.0, 0.15));
        if b.fine() {
            // A red gill down the gap between the plates.
            b.paint(GLOW_LASER);
            b.beam(v3(0.2, 0.9, 7.3), v3(-1.8, 0.95, 7.3), Vec2::new(0.08, 0.1), Vec2::new(0.08, 0.1));
            metal(b);
            cable(b, &[v3(-2.6, 0.55, 6.1), v3(-1.1, 0.9, 5.55), v3(0.1, 0.85, 5.5)], 0.1);
        }
    });
    b.paint(TEAM);
    b.mirror_y(|b| b.beam(v3(-1.6, 0.18, 7.96), v3(-2.3, 0.6, 7.8), Vec2::new(0.32, 0.07), Vec2::new(0.32, 0.07)));
}

/// The crown: long quills splayed at the sky out of the gland's top, their tips burning.
fn quills(b: &mut MeshBuilder) {
    let centre = v3(-0.7, 0.0, 7.7);
    let count = if b.fine() { 11 } else { 7 };
    for k in 0..count {
        let a = TAU * k as f32 / count as f32;
        let dir = v3(a.cos(), a.sin(), 0.0);
        let long = if k % 2 == 0 { 1.0 } else { 0.8 };
        let root = centre + dir * 0.75;
        let tip = centre + dir * (2.3 * long) + Vec3::Z * (2.0 * long);
        hide(b);
        spike(b, root, tip, 0.2);
        if b.fine() {
            b.paint(GLOW_LASER);
            let from = root.lerp(tip, 0.8);
            b.cylinder_between(from, tip + (tip - root) * 0.06, 0.07, 0.02, 4);
        }
    }
    if b.fine() {
        // A short inner ring, upright.
        hide(b);
        for k in 0..5 {
            let a = TAU * (k as f32 + 0.5) / 5.0;
            let dir = v3(a.cos(), a.sin(), 0.0);
            spike(b, centre + dir * 0.35, centre + dir * 0.8 + Vec3::Z * 1.3, 0.14);
        }
    }
}

/// The twin throats: soft-hide gullets under a plated cowl on bare metal bands, flared
/// mouths lit red inside, needles bristling out of them.
fn spitter_throats(b: &mut MeshBuilder) {
    let sides = b.sides(10);
    b.mirror_y(|b| {
        let (y0, y1) = (0.55, 0.45);
        let at = |x: f32| v3(x, y0 + (y1 - y0) * (x - 0.5) / 3.85, SPIT_PIVOT.z + 0.1 + (SPIT_MUZZLE.z - SPIT_PIVOT.z - 0.13) * (x - 0.5) / 3.85);
        under_hide(b);
        b.cylinder_between(at(0.5), at(3.75), 0.4, 0.38, sides);
        metal(b);
        for x in [1.4, 2.5] {
            b.cylinder_between(at(x), at(x + 0.28), 0.47, 0.47, sides);
        }
        hide(b);
        b.cylinder_between(at(3.55), at(4.35), 0.44, 0.6, sides);
        b.paint(GLOW_LASER);
        b.cylinder_between(at(3.9), at(4.3), 0.3, 0.44, sides);
        if b.fine() {
            metal(b);
            for k in 0..3 {
                let a = TAU * k as f32 / 3.0 + 0.4;
                let r = v3(0.0, a.cos(), a.sin()) * 0.22;
                b.cylinder_between(at(3.9) + r, at(4.72) + r * 1.7, 0.05, 0.015, 4);
            }
        }
    });
    hide(b);
    // The cowl over both throats.
    shell(b, &[(v3(0.7, 0.0, 6.75), 1.0, 0.32), (v3(3.2, 0.0, 7.45), 0.9, 0.3)], Vec3::Z);
    if b.fine() {
        spike(b, v3(2.6, 0.0, 7.6), v3(1.2, 0.0, 8.3), 0.22);
    }
}

// ---- Thornwall: wall section --------------------------------------------------------

/// The half width of a section's footing: the lot's edge, less a hair, so neighbours meet.
const WALL_HALF: f32 = 5.95;
/// The footing's height: a ledge there, where the plated bank steps in from the edge.
const WALL_FOOT: f32 = 0.6;

pub fn thornwall(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        hide(b);
        b.frustum_open(Vec3::ZERO, Vec2::splat(WALL_HALF * 2.0), Vec2::splat(4.6), 4.3, Vec2::ZERO);
        b.paint(TEAM);
        b.decal(v3(0.0, 0.0, 4.32), Vec2::splat(2.4));
        return;
    }
    // The bank: a footing standing straight up at the lot's edge on all four sides, so
    // sections side by side join, then soft hide sloping to a flat crown.
    let c = 0.7;
    let plan = [
        [WALL_HALF, -WALL_HALF + c],
        [WALL_HALF, WALL_HALF - c],
        [WALL_HALF - c, WALL_HALF],
        [-WALL_HALF + c, WALL_HALF],
        [-WALL_HALF, WALL_HALF - c],
        [-WALL_HALF, -WALL_HALF + c],
        [-WALL_HALF + c, -WALL_HALF],
        [WALL_HALF - c, -WALL_HALF],
    ];
    under_hide(b);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.0), Section::new(WALL_FOOT, 1.0), Section::new(WALL_FOOT, 0.93), Section::new(3.4, 0.62), Section::new(4.25, 0.42)],
    );
    b.radial(4, |b| {
        wall_face(b);
        wall_crown(b);
    });
    // The crown: the owner's colour, big thorns raked out over the corners.
    b.paint(TEAM);
    if b.fine() {
        b.cuboid(v3(0.0, 0.0, 4.28), v3(1.7, 1.7, 0.08));
    } else {
        b.decal(v3(0.0, 0.0, 4.27), Vec2::splat(1.7));
    }
    hide(b);
    b.radial(4, |b| {
        b.yawed(Vec3::ZERO, PI * 0.25, |b| {
            blade(b, v3(2.1, 0.0, 4.1), v3(4.1, 0.0, 5.7), 0.45, v3(0.0, 1.0, 0.0));
            if b.fine() {
                blade(b, v3(3.9, 0.0, 2.6), v3(5.75, 0.0, 3.7), 0.36, v3(0.0, 1.0, 0.0));
            }
        });
    });
}

/// The crown's edge on one face (the +x one): a plate over the upper slope and a pair of
/// thorns raked up and out of the gap behind it.
fn wall_crown(b: &mut MeshBuilder) {
    let (lo, hi) = ((WALL_HALF * 0.62, 3.4), (WALL_HALF * 0.42, 4.25));
    let normal = v3(hi.1 - lo.1, 0.0, lo.0 - hi.0).normalize();
    let p = |t: f32, y: f32| v3(lo.0 + (hi.0 - lo.0) * t, y, lo.1 + (hi.1 - lo.1) * t) + normal * 0.06;
    slab(b, [p(0.05, -2.9), p(0.05, 2.9), p(0.85, 2.0), p(0.85, -2.0)], normal * 0.2);
    if b.fine() {
        for y in [-1.3f32, 1.3] {
            let root = p(0.9, y) + normal * 0.2;
            blade(b, root, root + v3(1.5, y * 0.25, 1.35), 0.32, Vec3::Y);
        }
    }
}

/// One face of a Thornwall (the +x one): overlapping plates in two rows, thorns raked out
/// of the gaps between them, a tendon along the footing.
fn wall_face(b: &mut MeshBuilder) {
    // The slope, from the ledge on the footing up toward the crown.
    let slope_x = |z: f32| WALL_HALF * (0.93 - (z - WALL_FOOT) * 0.31 / (3.4 - WALL_FOOT));
    let normal = v3(3.4 - WALL_FOOT, 0.0, WALL_HALF * 0.31).normalize();
    let plate = |b: &mut MeshBuilder, z0: f32, z1: f32, ya: f32, yb: f32, stand: f32| {
        let p = |z: f32, y: f32| v3(slope_x(z), y, z) + normal * stand;
        slab(b, [p(z0, ya), p(z0, yb), p(z1, yb * 0.92), p(z1, ya * 0.92)], normal * 0.22);
    };
    hide(b);
    for s in [-1.0f32, 1.0] {
        // Lower row: two wide plates each side of the middle, standing off at the foot.
        plate(b, 0.75, 2.35, s * 0.3, s * 4.6, 0.12);
        // Upper row: overlapping the lower, a gap down the middle.
        plate(b, 2.1, 3.45, s * 0.55, s * 3.3, 0.28);
    }
    if !b.fine() {
        return;
    }
    // Thorns out of the gaps, raked up and out.
    for y in [-3.9f32, 0.0, 3.9] {
        let z = if y == 0.0 { 2.9 } else { 2.2 };
        let root = v3(slope_x(z), y, z);
        blade(b, root, root + v3(1.4, y * 0.06, 1.5), 0.34, Vec3::Y);
    }
    // A tendon along the footing's ledge, pinned by bare knuckles.
    metal(b);
    b.cylinder_between(v3(WALL_HALF - 0.3, -4.7, WALL_FOOT + 0.1), v3(WALL_HALF - 0.3, 4.7, WALL_FOOT + 0.1), 0.13, 0.13, 5);
    for y in [-2.4f32, 2.4] {
        b.cuboid(v3(WALL_HALF - 0.3, y, WALL_FOOT + 0.1), v3(0.4, 0.4, 0.42));
    }
    // A red seam where the upper plates part.
    b.paint(GLOW_LASER);
    b.beam(v3(slope_x(2.3) + 0.2, 0.0, 2.3), v3(slope_x(3.4) + 0.2, 0.0, 3.4), Vec2::new(0.12, 0.08), Vec2::new(0.12, 0.08));
    hide(b);
    for y in [-2.0f32, 2.0] {
        let root = v3(slope_x(1.6) + 0.3, y, 1.6);
        spike(b, root, root + v3(0.55, 0.0, -0.45), 0.14);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn barb_fits() {
        super::super::check("naga_barb", 5.5, 8.0, Some(1), &[[5.2, 0.0, 6.8]]);
    }

    #[test]
    fn spitter_fits() {
        super::super::check("naga_spitter", 5.5, 8.5, Some(1), &[[4.4, 0.0, 7.4]]);
    }

    #[test]
    fn thornwall_fits() {
        super::super::check("naga_thornwall", 6.0, 5.0, Some(1), &[]);
    }
}
