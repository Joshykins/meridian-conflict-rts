//! The Auxiliary Engineering Suite (`aux_eng`): a floating nanite lens over the left
//! shoulder. From a plated saddle on the upper back behind the pauldron (`AUX_HINGE`) a
//! keeled mast rises along one unbroken curve (`SPINE`), leaning over the shoulder; at its
//! end (`AUX_WRIST`, the unit file's `hinge`) it forks, the two tines carrying the curve on
//! either side, and between them a nanite crystal hangs free on the same line, its tip the
//! emitter the beam leaves from (`AUX_EMITTER`, its `emitters`). A broken ring of shards
//! floats round the crystal. The light is the plates' own (`kit::nanite_plate`), not a lit
//! piece.
//!
//! Stowed (`MeshBuilder::with_fold`, entity.wgsl's folding gear), the mast rocks back to
//! stand upright behind the pauldron like a staff, the lens held up on top.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::kit::{dark_plate, metal, nanite_plate, v3};
use super::form::{ring, sleeve, Ring, KEEL, OCT};

pub(super) const AUX_HINGE: Vec3 = Vec3::new(-2.6, 2.9, 21.4);
pub(super) const AUX_EMITTER: Vec3 = Vec3::new(2.0, 3.2, 25.0);
/// The suite's line: a cubic from the knuckle up and over the shoulder, ending at the
/// emitter pointing down and forward at the work.
const SPINE: [Vec3; 4] = [
    AUX_HINGE,
    Vec3::new(-2.8, 3.1, 25.2),
    Vec3::new(-0.6, 3.2, 26.8),
    AUX_EMITTER,
];
/// Where on `SPINE` the mast ends and the fork begins: `AUX_WRIST`.
const HEAD_AT: f32 = 0.62;
pub(super) const AUX_WRIST: Vec3 = Vec3::new(-0.681, 3.1567, 25.645);
/// Where the crystal starts, clear of the mast's end.
const CRYSTAL_AT: f32 = 0.7;
/// Stowed, the mast stands upright behind the pauldron, the lens held up on top.
const AUX_STOWED: f32 = 0.9;
const AUX_HEAD_STOWED: f32 = 0.6;

pub(super) fn aux_eng(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.0, saddle);
    b.module("aux_eng", 0.3, |b| {
        b.with_fold(AUX_HINGE, AUX_STOWED, mast);
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, head);
    });
}

/// A point on `SPINE` at `t`.
fn spine(t: f32) -> Vec3 {
    let s = 1.0 - t;
    SPINE[0] * (s * s * s)
        + SPINE[1] * (3.0 * s * s * t)
        + SPINE[2] * (3.0 * s * t * t)
        + SPINE[3] * (t * t * t)
}

/// The way `SPINE` runs at `t`.
fn along(t: f32) -> Vec3 {
    let s = 1.0 - t;
    ((SPINE[1] - SPINE[0]) * (3.0 * s * s)
        + (SPINE[2] - SPINE[1]) * (6.0 * s * t)
        + (SPINE[3] - SPINE[2]) * (3.0 * t * t))
        .normalize()
}

/// Square to `SPINE` at `t`, away from the curve's centre: the outside of the arc.
fn outward(t: f32) -> Vec3 {
    let d = along(t);
    Vec3::new(-d.z, 0.0, d.x)
}

/// The saddle on the upper back and the knuckle the mast turns on.
fn saddle(b: &mut MeshBuilder) {
    dark_plate(b);
    let base = v3(-2.4, 2.6, 20.3);
    let d = (AUX_HINGE - base).normalize();
    let out = Vec3::new(-d.z, 0.0, d.x);
    sleeve(
        b,
        &[
            ring(base - d * 0.3, out, 0.9, 0.8),
            ring(base.lerp(AUX_HINGE, 0.5), out, 0.78, 0.7),
            ring(AUX_HINGE, out, 0.55, 0.5),
        ],
        &OCT,
    );
    metal(b);
    b.cylinder_between(
        AUX_HINGE - Vec3::Y * 0.6,
        AUX_HINGE + Vec3::Y * 0.6,
        0.4,
        0.4,
        b.sides(10),
    );
}

/// The mast: one keeled plate lofted along the curve, ridge to the outside, tapering from
/// the knuckle to the fork.
fn mast(b: &mut MeshBuilder) {
    let rings: Vec<Ring> = (0..=6)
        .map(|i| {
            let f = i as f32 / 6.0;
            let t = HEAD_AT * f;
            ring(spine(t), outward(t), 0.44 - 0.14 * f, 0.66 - 0.24 * f)
        })
        .collect();
    nanite_plate(b);
    sleeve(b, &rings, &KEEL);
}

/// The head: the fork's two tines carrying the curve on either side, the crystal free
/// between them on the line itself, and the broken ring of shards round it.
fn head(b: &mut MeshBuilder) {
    let t = |f: f32| HEAD_AT + (1.0 - HEAD_AT) * f;
    // The fork's root: a short block across the mast's end, out to both tines.
    dark_plate(b);
    let root = ring(spine(HEAD_AT), outward(HEAD_AT), 0.3, 0.42);
    let root_end = ring(spine(t(0.08)), outward(t(0.08)), 0.5, 0.36);
    sleeve(b, &[root, root_end], &OCT);
    for side in [1.0f32, -1.0] {
        let y = Vec3::Y * side;
        let tine = |f: f32, spread: f32, w: f32, h: f32| ring(spine(t(f)) + y * spread, y, w, h);
        sleeve(
            b,
            &[
                tine(0.0, 0.3, 0.28, 0.22),
                tine(0.3, 0.95, 0.26, 0.22),
                tine(0.7, 0.9, 0.18, 0.15),
                tine(1.05, 0.5, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
    // The crystal, its tip the emitter, its waist the widest point of the head.
    let a = spine(CRYSTAL_AT);
    let up = outward(0.85);
    let d = (AUX_EMITTER - a).normalize();
    let waist = a.lerp(AUX_EMITTER, 0.45);
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(a, up, 0.02, 0.02),
            ring(waist, up, 0.72, 0.72),
            ring(AUX_EMITTER, up, 0.02, 0.02),
        ],
        &OCT,
    );
    dark_plate(b);
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 / 3.0) + 0.5;
        let r = up * a.cos() + Vec3::Y * a.sin();
        let tangent = d.cross(r).normalize();
        let mid = waist + r * 1.35;
        sleeve(
            b,
            &[
                ring(mid - tangent * 0.8 - d * 0.15, r, 0.04, 0.04),
                ring(mid, r, 0.34, 0.26),
                ring(mid + tangent * 0.8 + d * 0.15, r, 0.04, 0.04),
            ],
            &KEEL,
        );
    }
}

/// From far off: the mast as two bars along its curve and the crystal as a third, still
/// folding.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.3, |b| {
        dark_plate(b);
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            let mid = HEAD_AT * 0.5;
            b.beam(spine(0.0), spine(mid), Vec2::splat(0.8), Vec2::splat(0.65));
            b.beam(spine(mid), AUX_WRIST, Vec2::splat(0.65), Vec2::splat(0.5));
        });
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            b.beam(
                spine(CRYSTAL_AT),
                AUX_EMITTER,
                Vec2::splat(1.0),
                Vec2::splat(0.3),
            )
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrist_and_emitter_are_on_the_line() {
        assert!(
            spine(HEAD_AT).distance(AUX_WRIST) < 1e-3,
            "{}",
            spine(HEAD_AT)
        );
        assert!(spine(1.0).distance(AUX_EMITTER) < 1e-5);
    }
}
