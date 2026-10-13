//! A run of security fence (`PropKind::ColonyFence`): `FENCE_SEGMENT_M` along x
//! from its middle, 3 m high, laid end to end round a yard. Every piece that runs
//! along it runs its whole length, so runs meet without a seam, and its posts keep
//! one pitch across the joins.

use glam::{Vec2, Vec3};
use mc_map::city::FENCE_SEGMENT_M;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// Half the run.
const HALF: f32 = FENCE_SEGMENT_M as f32 * 0.5;
/// The fence's top over the ground.
const TOP: f32 = 3.0;

/// Weld-mesh panels between steel posts on a concrete kerb, Y-arms at the posts'
/// heads strung with barbed wire.
pub(super) fn fence(b: &mut MeshBuilder, _tech: u8) {
    let posts = pitch(3.0);
    let head = 2.55;
    kerb(b, 0.32, 0.3);
    if !b.mid() {
        rail(b, 0.0, head, 0.1);
        for &x in posts.iter().step_by(2) {
            strand(b, v3(x, 0.0, 0.3), v3(x, 0.0, TOP), Vec2::splat(0.16));
        }
        return;
    }
    for &x in &posts {
        post(b, x, 0.3, head + 0.05, 0.16);
        // The Y-arm, its two limbs leaning out to the strands.
        paint(b, pat::STEEL);
        for s in [-1.0f32, 1.0] {
            b.beam(
                v3(x, 0.0, head),
                v3(x, s * 0.45, TOP),
                Vec2::splat(0.08),
                Vec2::splat(0.07),
            );
        }
    }
    for z in [0.45, 1.5, head] {
        rail(b, 0.0, z, 0.07);
    }
    // The mesh: upright wires on a close pitch, laced through with level ones.
    let wire = 0.035;
    let step = if b.fine() { 0.2 } else { 0.8 };
    for x in cells(-HALF, 2.0 * HALF, step) {
        strand(
            b,
            v3(x, 0.0, 0.4),
            v3(x, 0.0, head),
            Vec2::new(wire, wire * 0.6),
        );
    }
    if b.fine() {
        for z in cells(0.45, head - 0.45, 0.4) {
            strand(
                b,
                v3(-HALF, 0.0, z),
                v3(HALF, 0.0, z),
                Vec2::splat(wire * 0.6),
            );
        }
        // Barbed strands along the arms.
        for s in [-1.0f32, 1.0] {
            for k in [0.45, 1.0] {
                let p = v3(0.0, s * 0.45 * k, head + (TOP - head) * k);
                strand(
                    b,
                    v3(-HALF, p.y, p.z),
                    v3(HALF, p.y, p.z),
                    Vec2::splat(0.04),
                );
            }
        }
    }
}

/// A palisade: heavy prefab piers banded in the colony's accent, a blank prefab
/// panel to chest height between them, flat steel pales over it and a coil of razor
/// wire along the top.
pub(super) fn palisade(b: &mut MeshBuilder, _tech: u8) {
    let piers = pitch(6.0);
    let panel_top = 1.3;
    let pales = 2.55;
    kerb(b, 0.4, 0.35);
    if !b.mid() {
        // The panel and the pales as one band, the piers through it.
        paint(b, pat::PREFAB);
        b.loft(
            &[
                Rect::new(-HALF, -0.14, HALF, 0.14).ring(0.35),
                Rect::new(-HALF, -0.14, HALF, 0.14).ring(pales),
            ],
            false,
            true,
        );
        for &x in &piers {
            strand(b, v3(x, 0.0, 0.35), v3(x, 0.0, TOP), Vec2::splat(0.6));
        }
        return;
    }
    // The panels, their seams at the piers.
    let r = Rect::new(-HALF, -0.14, HALF, 0.14);
    cut_slab(b, r, 0.0, 1, 0.35, panel_top - 0.25, pat::PREFAB);
    recess(
        b,
        r,
        0.0,
        1,
        panel_top - 0.25,
        panel_top - 0.1,
        0.05,
        pat::TRIM,
    );
    cut_slab(b, r, 0.0, 1, panel_top - 0.1, panel_top, pat::PREFAB);
    for &x in &piers {
        paint(b, pat::PREFAB);
        b.chamfered_box(v3(x, 0.0, 1.3), Vec3::new(0.7, 0.7, 2.6), 0.15);
        if b.fine() {
            paint(b, pat::ACCENT);
            b.chamfered_box(v3(x, 0.0, 2.35), Vec3::new(0.76, 0.76, 0.3), 0.16);
        }
        paint(b, pat::PREFAB);
        b.loft(
            &[
                Rect::centred(x, 0.0, 0.42, 0.42).ring(2.6),
                Rect::centred(x, 0.0, 0.42, 0.42).ring(2.75),
                Rect::centred(x, 0.0, 0.25, 0.25).ring(2.9),
            ],
            false,
            true,
        );
    }
    for z in [panel_top + 0.12, pales - 0.12] {
        rail(b, 0.0, z, 0.08);
    }
    let step = if b.fine() { 0.33 } else { 0.99 };
    paint(b, pat::STEEL);
    for x in cells(-HALF, 2.0 * HALF, step) {
        if piers.iter().any(|p| (x - p).abs() < 0.45) {
            continue;
        }
        let pale = |z: f32, w: f32| Rect::centred(x, 0.0, w, 0.0125).ring(z);
        if b.fine() {
            // Each pale pointed.
            b.loft(
                &[
                    pale(panel_top, 0.035),
                    pale(pales, 0.035),
                    pale(pales + 0.12, 0.005),
                ],
                false,
                false,
            );
        } else {
            b.loft(&[pale(panel_top, 0.035), pale(pales, 0.035)], false, false);
        }
    }
    coil(b, v3(0.0, 0.0, pales + 0.36), 0.3);
}

/// Where a run's posts stand, `step` apart, half a step in from each end so the
/// pitch holds across the join to the next run.
fn pitch(step: f32) -> Vec<f32> {
    cells(-HALF, 2.0 * HALF, step).collect()
}

/// The concrete kerb the run stands on, `half` either side, its top at `top`, taken
/// into the ground, its top edges bevelled at full detail.
fn kerb(b: &mut MeshBuilder, half: f32, top: f32) {
    let r = Rect::new(-HALF, -half, HALF, half);
    paint(b, pat::CONCRETE);
    if b.fine() {
        b.loft(
            &[
                r.ring(-0.8),
                r.ring(top - 0.08),
                Rect::new(-HALF, -half + 0.08, HALF, half - 0.08).ring(top),
            ],
            false,
            true,
        );
    } else {
        b.loft(&[r.ring(-0.8), r.ring(top)], false, true);
    }
}

/// A post at `x` from `z0` to `z1`, `w` square.
fn post(b: &mut MeshBuilder, x: f32, z0: f32, z1: f32, w: f32) {
    paint(b, pat::STEEL);
    b.loft(
        &[
            Rect::centred(x, 0.0, w * 0.5, w * 0.5).ring(z0),
            Rect::centred(x, 0.0, w * 0.5, w * 0.5).ring(z1),
        ],
        false,
        true,
    );
}

/// A rail the run's whole length at height `z`, `y` across, `w` thick.
fn rail(b: &mut MeshBuilder, y: f32, z: f32, w: f32) {
    strand(b, v3(-HALF, y, z), v3(HALF, y, z), Vec2::splat(w));
}

/// A bare bar from `a` to `c`, `size` across, its ends left open (they meet the
/// next run's, or are lost in what they meet).
fn strand(b: &mut MeshBuilder, a: Vec3, c: Vec3, size: Vec2) {
    let Some(side) = (c - a).cross(Vec3::Z).try_normalize() else {
        // Upright: its sides along x and y.
        paint(b, pat::STEEL);
        let r = Rect::centred(a.x, a.y, size.x * 0.5, size.y * 0.5);
        b.loft(&[r.ring(a.z), r.ring(c.z)], false, false);
        return;
    };
    let up = side.cross(c - a).normalize();
    let ring = |p: Vec3| -> Vec<Vec3> {
        let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
        vec![p - s - u, p + s - u, p + s + u, p - s + u]
    };
    paint(b, pat::STEEL);
    b.loft(&[ring(a), ring(c)], false, false);
}

/// A coil of razor wire the run's length round the line along x through `at`, `r`
/// round: a helix of thin wire, a turn every 0.7 m at full detail.
fn coil(b: &mut MeshBuilder, at: Vec3, r: f32) {
    let (pitch, per) = if b.fine() { (0.7, 5) } else { (1.5, 4) };
    let turns = (2.0 * HALF / pitch).round();
    let n = (turns as usize) * per;
    let wire = 0.03;
    let mut rings = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let a = t * turns * std::f32::consts::TAU;
        let c = at + v3(-HALF + 2.0 * HALF * t, a.cos() * r, a.sin() * r);
        // A small triangle round the wire, across the way it winds.
        let out = v3(0.0, a.cos(), a.sin());
        rings.push(vec![
            c + out * wire,
            c - out * wire * 0.5 + Vec3::X * wire * 0.87,
            c - out * wire * 0.5 - Vec3::X * wire * 0.87,
        ]);
    }
    paint(b, pat::STEEL);
    b.loft(&rings, false, false);
}
