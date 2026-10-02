//! The back refits: the Personal Shield (`shield`), a small caged star between the
//! shoulder blades that throws the prism veil round the Exarch. The veil unfolds from it
//! (`MeshBuilder::set_shield_emitter`, entity.wgsl `hull_emitter`), so the emitter is the
//! star's heart. It rides the torso (`part::TURRET`), so nothing on it turns on its own.
//!
//! Three plated vertebrae step up the back, each lapped over the one below with a prism
//! bar lit across it, and the star sits at the top over the highest on a bronze post.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};

/// The housing on the spine the vertebrae stand on: a plated block lapped down the back.
fn housing(b: &mut MeshBuilder, top: f32) {
    dark_plate(b);
    b.beam(
        v3(-2.55, 0.0, 16.4),
        v3(-2.75, 0.0, top),
        Vec2::new(2.0, 1.1),
        Vec2::new(1.6, 1.0),
    );
    seam(b);
    b.beam(
        v3(-3.05, 0.0, 16.8),
        v3(-3.2, 0.0, top - 0.3),
        Vec2::new(1.3, 0.25),
        Vec2::new(1.1, 0.25),
    );
}

/// The star's white-hot heart at `at`, the field's emitter.
fn heart(b: &mut MeshBuilder, at: Vec3, r: f32) {
    b.set_shield_emitter(at);
    b.paint(GLOW_PRISM);
    let sides = b.sides(10);
    b.spheroid(at, Vec3::splat(r), sides, if b.fine() { 6 } else { 4 });
}

pub(super) fn shield(b: &mut MeshBuilder) {
    let top = v3(-3.5, 0.0, 21.6);
    b.module("shield", 0.0, |b| {
        housing(b, 17.4);
        heart(b, top, 0.5);
    });
    b.module("shield", 0.35, |b| {
        for k in 0..3 {
            let z = 17.6 + 1.25 * k as f32;
            let x = -2.95 - 0.15 * k as f32;
            // A vertebra: a plated wedge lapped down over the one below.
            dark_plate(b);
            b.beam(
                v3(x, 0.0, z),
                v3(x - 0.9, 0.0, z + 0.9),
                Vec2::new(1.8 - 0.2 * k as f32, 0.5),
                Vec2::new(1.2 - 0.15 * k as f32, 0.35),
            );
            b.paint(GLOW_PRISM);
            b.beam(
                v3(x - 0.45, -0.6 + 0.07 * k as f32, z + 0.65),
                v3(x - 0.45, 0.6 - 0.07 * k as f32, z + 0.65),
                Vec2::splat(0.14),
                Vec2::splat(0.14),
            );
        }
        metal(b);
        b.beam(
            v3(-3.2, 0.0, 20.2),
            top,
            Vec2::splat(0.28),
            Vec2::splat(0.2),
        );
    });
}
