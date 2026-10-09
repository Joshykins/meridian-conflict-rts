//! The Auxiliary Engineering Suite (`aux_eng`): a nanite tendril over the left shoulder.
//! From a plated saddle on the upper back behind the pauldron (`AUX_HINGE`) it runs as one
//! unbroken curve (`SPINE`) up, over the shoulder and down at the work: a run of separate
//! keeled vertebrae, each held clear of the next with nothing between them, tapering into
//! the head (from `AUX_WRIST`, the unit file's `hinge`), where three keeled petals cup the
//! nanite crystal the beam leaves from (`AUX_EMITTER`, its `emitters`). The light is the
//! plates' own (`kit::nanite_plate`), not a lit piece.
//!
//! Stowed (`MeshBuilder::with_fold`, entity.wgsl's folding gear), the tendril rocks back
//! to stand raised over the shoulder and the head curls down under its crest.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::kit::{dark_plate, metal, nanite_plate, v3};
use super::form::{ring, sleeve, Ring, KEEL, OCT};

pub(super) const AUX_HINGE: Vec3 = Vec3::new(-2.6, 2.9, 21.4);
pub(super) const AUX_EMITTER: Vec3 = Vec3::new(3.5, 3.5, 24.0);
/// The tendril's line: a cubic from the knuckle up and over the shoulder, ending at the
/// emitter pointing down and forward at the work.
const SPINE: [Vec3; 4] = [
    AUX_HINGE,
    Vec3::new(-2.9, 3.2, 26.2),
    Vec3::new(1.2, 3.45, 27.8),
    AUX_EMITTER,
];
/// Where on `SPINE` the vertebrae end and the head begins: `AUX_WRIST`.
const HEAD_AT: f32 = 0.76;
pub(super) const AUX_WRIST: Vec3 = Vec3::new(1.6187, 3.4315, 25.8333);
const VERTEBRAE: usize = 7;
/// The air between two vertebrae, as a share of each one's run.
const GAP: f32 = 0.12;
/// Stowed, the tendril rocks back to stand raised over the shoulder, and the head curls
/// down and forward under its crest, the curve still unbroken.
const AUX_STOWED: f32 = 0.8;
const AUX_HEAD_STOWED: f32 = -1.2;

pub(super) fn aux_eng(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.0, saddle);
    b.module("aux_eng", 0.3, |b| {
        b.with_fold(AUX_HINGE, AUX_STOWED, vertebrae);
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

/// How thick the tendril is at `t`, tapering from the knuckle into the head.
fn girth(t: f32) -> f32 {
    1.0 - 0.5 * t
}

/// A section across `SPINE` at `t`, `w` across and `h` outward at full girth.
fn section(t: f32, w: f32, h: f32) -> Ring {
    ring(spine(t), outward(t), w * girth(t), h * girth(t))
}

/// The saddle on the upper back and the knuckle the tendril turns on.
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

/// The vertebrae: keeled plates swollen just ahead of their middle, each lofted along the
/// curve with a little air before the next, a short spur raked back off each one's ridge.
fn vertebrae(b: &mut MeshBuilder) {
    let step = HEAD_AT / VERTEBRAE as f32;
    for i in 0..VERTEBRAE {
        let t0 = i as f32 * step + step * GAP * 0.5;
        let t1 = (i + 1) as f32 * step - step * GAP * 0.5;
        let at = |f: f32| t0 + (t1 - t0) * f;
        nanite_plate(b);
        sleeve(
            b,
            &[
                section(at(0.0), 0.34, 0.4),
                section(at(0.35), 0.5, 0.64),
                section(at(0.7), 0.44, 0.52),
                section(at(1.0), 0.3, 0.34),
            ],
            &KEEL,
        );
        if b.fine() {
            let t = at(0.4);
            let g = girth(t);
            let root = spine(t) + outward(t) * 0.55 * g;
            let tip = root + (outward(t) * 0.5 + along(t) * 0.45) * g;
            dark_plate(b);
            sleeve(
                b,
                &[
                    ring(root, along(t), 0.1 * g, 0.12 * g),
                    ring(tip, along(t), 0.02, 0.02),
                ],
                &KEEL,
            );
        }
    }
}

/// The head: the end of the curve as a housing that carries on from the vertebrae, three
/// petals cupped forward round the crystal at its tip.
fn head(b: &mut MeshBuilder) {
    let start = HEAD_AT + HEAD_AT / VERTEBRAE as f32 * GAP * 0.5;
    let t = |f: f32| start + (1.0 - start) * f;
    dark_plate(b);
    sleeve(
        b,
        &[
            section(t(0.0), 0.32, 0.34),
            section(t(0.2), 0.42, 0.44),
            section(t(0.45), 0.36, 0.38),
        ],
        &OCT,
    );
    let o = spine(t(0.4));
    let d = along(t(0.7));
    let up = outward(t(0.7));
    for k in 0..3 {
        let a = std::f32::consts::TAU * (k as f32 / 3.0);
        let r = up * a.cos() + Vec3::Y * a.sin();
        sleeve(
            b,
            &[
                ring(o + r * 0.24, r, 0.2, 0.13),
                ring(o + r * 0.5 + d * 0.55, r, 0.18, 0.13),
                ring(o + r * 0.34 + d * 1.25, r, 0.02, 0.02),
            ],
            &KEEL,
        );
    }
    nanite_plate(b);
    let from = spine(t(0.42));
    sleeve(
        b,
        &[
            ring(from, up, 0.02, 0.02),
            ring(from.lerp(AUX_EMITTER, 0.4), up, 0.3, 0.3),
            ring(AUX_EMITTER, up, 0.02, 0.02),
        ],
        &OCT,
    );
}

/// From far off: the tendril as three bars along its curve and the head as a fourth,
/// still folding.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.3, |b| {
        dark_plate(b);
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            for i in 0..3 {
                let (t0, t1) = (HEAD_AT * i as f32 / 3.0, HEAD_AT * (i + 1) as f32 / 3.0);
                let w = 0.8 * girth(t0);
                b.beam(spine(t0), spine(t1), Vec2::splat(w), Vec2::splat(w * 0.8));
            }
        });
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            b.beam(AUX_WRIST, AUX_EMITTER, Vec2::splat(0.6), Vec2::splat(0.3))
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
