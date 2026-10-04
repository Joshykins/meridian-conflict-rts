//! The Auxiliary Engineering Suite (`aux_eng`): a second nanite arm over the left
//! shoulder. Its knuckle sits on a plated saddle on the upper back behind the pauldron
//! (`AUX_HINGE`); the arm is authored out, reaching up and forward over the pauldron, and
//! its head at the wrist (`AUX_WRIST`, the unit file's `hinge`) points down at the work, the
//! beam leaving its violet emitter (`AUX_EMITTER`, its `emitters`).
//!
//! Stowed (`MeshBuilder::with_fold`, entity.wgsl's folding gear), the arm swings back up
//! behind the shoulder and the head folds into line with it: one more keeled spike raked
//! back with the pauldron's own.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, v3};
use super::form::{ring, sleeve, KEEL, OCT};

pub(super) const AUX_HINGE: Vec3 = Vec3::new(-2.6, 2.6, 21.2);
/// Where the arm ends, inboard of the head, so the two fold flat.
const AUX_ELBOW: Vec3 = Vec3::new(1.3, 2.6, 24.4);
pub(super) const AUX_WRIST: Vec3 = Vec3::new(1.3, 3.15, 24.4);
pub(super) const AUX_EMITTER: Vec3 = Vec3::new(3.6, 3.15, 23.5);
/// Stowed, the arm stands raked back up behind the shoulder (2.25 rad against 0.69 out),
/// and the head folds up into line with it.
const AUX_STOWED: f32 = 1.56;
const AUX_HEAD_STOWED: f32 = 1.06;

pub(super) fn aux_eng(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.0, |b| {
        // The saddle on the upper back and the knuckle the arm turns on.
        dark_plate(b);
        b.with_facets(|b| {
            b.beam(
                v3(-2.0, 2.4, 20.5),
                v3(-2.9, 2.6, 21.0),
                Vec2::new(1.2, 1.1),
                Vec2::new(1.0, 0.9),
            )
        });
        metal(b);
        b.cylinder_between(
            AUX_HINGE - Vec3::Y * 0.55,
            AUX_HINGE + Vec3::Y * 0.55,
            0.42,
            0.42,
            b.sides(10),
        );
    });
    b.module("aux_eng", 0.3, |b| {
        b.with_fold(AUX_HINGE, AUX_STOWED, arm);
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, head);
    });
}

/// The arm: a keeled plate from the knuckle to the elbow, its ridge up, a lit feed line
/// down its inboard face and the wrist knuckle at its end.
fn arm(b: &mut MeshBuilder) {
    let along = (AUX_ELBOW - AUX_HINGE).normalize();
    let up = Vec3::new(-along.z, 0.0, along.x);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(AUX_HINGE, up, 0.4, 0.5),
            ring(AUX_HINGE.lerp(AUX_ELBOW, 0.35), up, 0.38, 0.62),
            ring(AUX_ELBOW, up, 0.3, 0.42),
        ],
        &KEEL,
    );
    metal(b);
    b.cylinder_between(
        AUX_ELBOW - Vec3::Y * 0.2,
        AUX_WRIST + Vec3::Y * 0.3,
        0.3,
        0.3,
        b.sides(8),
    );
    if b.fine() {
        b.paint(GLOW_VIOLET);
        b.beam(
            AUX_HINGE + along * 0.8 - Vec3::Y * 0.36,
            AUX_ELBOW - along * 0.6 - Vec3::Y * 0.3,
            Vec2::new(0.06, 0.12),
            Vec2::new(0.06, 0.1),
        );
        metal(b);
        cable(
            b,
            &[
                AUX_HINGE + up * 0.2 - Vec3::Y * 0.3,
                AUX_HINGE.lerp(AUX_ELBOW, 0.5) - up * 0.4 - Vec3::Y * 0.3,
                AUX_ELBOW - along * 0.4 - Vec3::Y * 0.25,
            ],
            0.09,
        );
    }
}

/// The head: a short faceted housing out of the wrist, two talons either side of the
/// violet emitter at its tip.
fn head(b: &mut MeshBuilder) {
    let d = (AUX_EMITTER - AUX_WRIST).normalize();
    let up = Vec3::new(-d.z, 0.0, d.x);
    let at = |t: f32| AUX_WRIST + d * t;
    let len = AUX_EMITTER.distance(AUX_WRIST);
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(at(-0.3), up, 0.34, 0.36),
            ring(at(0.6), up, 0.4, 0.42),
            ring(at(len - 0.7), up, 0.28, 0.3),
        ],
        &OCT,
    );
    for side in [1.0f32, -1.0] {
        let out = Vec3::Y * side;
        let talon = [
            ring(at(len - 1.0) + out * 0.3, out, 0.12, 0.14),
            ring(at(len - 0.3) + out * 0.34, out, 0.1, 0.12),
            ring(at(len - 0.05) + out * 0.14, out, 0.02, 0.02),
        ];
        sleeve(b, &talon, &KEEL);
    }
    b.paint(GLOW_VIOLET);
    b.cylinder_between(at(len - 0.75), AUX_EMITTER, 0.2, 0.06, b.sides(8));
}

/// From far off: the arm and its head as two bars, still folding.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("aux_eng", 0.3, |b| {
        dark_plate(b);
        b.with_fold(AUX_HINGE, AUX_STOWED, |b| {
            b.beam(AUX_HINGE, AUX_ELBOW, Vec2::splat(0.7), Vec2::splat(0.5))
        });
        b.with_fold_head(AUX_WRIST, AUX_HEAD_STOWED, |b| {
            b.beam(AUX_WRIST, AUX_EMITTER, Vec2::splat(0.6), Vec2::splat(0.3))
        });
    });
}
