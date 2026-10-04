//! The back refits, one at a time: Nanite Repair (`nano_repair`), the Nanite Repair
//! Field over it (`nano_field`), or the Material Formation Engine (`mfe`). They ride the
//! torso (`part::TURRET`), so nothing on them turns on its own. The two differ in outline:
//! the nanites stand tall and narrow on the spine, the engine lies low and wide across it.
//!
//! - Nanite Repair: a plated housing lapped down the spine between two keeled reservoirs,
//!   the nanites lit violet down their ridges and piped into the housing.
//! - The field adds a mast up out of the housing, taller than the pauldrons, with three
//!   violet vanes round its head that let the nanites out over everything round it.
//! - The engine: a low plated block across the shoulder blades, its back cut with red
//!   condenser slots, two short stacks raked back off its top corners.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::form::{ring, sleeve, KEEL, OCT};

/// The housing on the spine: a plated block lapped down the back.
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

/// Where the mast's vanes let the nanites out.
const MAST_HEAD: Vec3 = Vec3::new(-3.9, 0.0, 24.6);

pub(super) fn back(b: &mut MeshBuilder) {
    b.module("nano_repair", 0.0, |b| housing(b, 20.6));
    b.module("nano_repair", 0.35, reservoirs);
    b.module("nano_field", 0.4, mast);
    b.module("mfe", 0.0, engine);
}

/// A keeled reservoir either side of the housing, its ridge lit violet, piped in at the top.
fn reservoirs(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let out = Vec3::new(-1.0, 0.35, 0.0).normalize();
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(-2.85, 1.25, 16.7), out, 0.42, 0.4),
                ring(v3(-3.05, 1.35, 18.3), out, 0.56, 0.55),
                ring(v3(-3.0, 1.3, 20.0), out, 0.36, 0.34),
            ],
            &KEEL,
        );
        b.paint(GLOW_VIOLET);
        b.beam(
            v3(-3.05, 1.35, 17.3) + out * 0.56,
            v3(-3.05, 1.35, 19.3) + out * 0.5,
            Vec2::new(0.12, 0.12),
            Vec2::new(0.1, 0.1),
        );
        if b.fine() {
            metal(b);
            cable(
                b,
                &[
                    v3(-3.0, 1.25, 20.0),
                    v3(-3.1, 0.9, 20.5),
                    v3(-2.95, 0.55, 20.4),
                ],
                0.1,
            );
        }
    });
    b.paint(GLOW_VIOLET);
    b.beam(
        v3(-3.25, -0.4, 19.9),
        v3(-3.25, 0.4, 19.9),
        Vec2::splat(0.16),
        Vec2::splat(0.16),
    );
}

/// The field's mast up out of the housing, three violet vanes round its head.
fn mast(b: &mut MeshBuilder) {
    let foot = v3(-2.85, 0.0, 19.6);
    let along = (MAST_HEAD - foot).normalize();
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(foot, Vec3::X, 0.5, 0.42),
            ring(foot.lerp(MAST_HEAD, 0.55), Vec3::X, 0.36, 0.3),
            ring(MAST_HEAD - along * 0.2, Vec3::X, 0.24, 0.22),
        ],
        &OCT,
    );
    let vanes: &[f32] = if b.fine() {
        &[0.0, 120.0, 240.0]
    } else {
        &[60.0, 300.0]
    };
    for &deg in vanes {
        let a = deg.to_radians();
        // Round the mast's axis, from straight back.
        let side = along.cross(Vec3::Y).normalize();
        let dir = (side * a.cos() + Vec3::Y * a.sin()).normalize();
        let root = MAST_HEAD - along * 1.6;
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(root + dir * 0.2, dir, 0.08, 0.2),
                ring(root + along * 0.9 + dir * 0.55, dir, 0.1, 0.26),
                ring(MAST_HEAD + along * 0.5 + dir * 0.3, dir, 0.03, 0.03),
            ],
            &KEEL,
        );
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            root + along * 0.5 + dir * 0.3,
            root + along * 1.3 + dir * 0.3,
            0.09,
            0.06,
            6,
        );
    }
    b.paint(GLOW_VIOLET);
    b.cylinder_between(
        MAST_HEAD - along * 0.2,
        MAST_HEAD + along * 0.35,
        0.2,
        0.05,
        b.sides(8),
    );
}

/// The Material Formation Engine: a low plated block across the shoulder blades.
fn engine(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-3.05, -1.95, 18.2), -Vec3::X, 0.95, 0.55),
            ring(v3(-3.2, -1.6, 18.2), -Vec3::X, 1.25, 0.75),
            ring(v3(-3.2, 1.6, 18.2), -Vec3::X, 1.25, 0.75),
            ring(v3(-3.05, 1.95, 18.2), -Vec3::X, 0.95, 0.55),
        ],
        &OCT,
    );
    // Red condenser slots cut across its back.
    b.paint(GLOW_LASER);
    let slots: &[f32] = if b.fine() {
        &[17.65, 18.2, 18.75]
    } else {
        &[18.2]
    };
    for &z in slots {
        b.beam(
            v3(-3.94, -1.25, z),
            v3(-3.94, 1.25, z),
            Vec2::new(0.06, 0.14),
            Vec2::new(0.06, 0.14),
        );
    }
    // The stacks off its top corners, raked back, their mouths lit.
    b.mirror_y(|b| {
        let (foot, mouth) = (v3(-3.1, 1.1, 18.9), v3(-3.9, 1.25, 20.5));
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(foot, -Vec3::X, 0.42, 0.42),
                ring(foot.lerp(mouth, 0.5), -Vec3::X, 0.36, 0.36),
                ring(mouth, -Vec3::X, 0.32, 0.32),
            ],
            &OCT,
        );
        b.paint(GLOW_LASER);
        let along = (mouth - foot).normalize();
        b.cylinder_between(
            mouth - along * 0.1,
            mouth + along * 0.04,
            0.24,
            0.24,
            b.sides(8),
        );
    });
}

/// From far off: the repair housing a dark ridge, the field's mast a lit spike, the
/// engine a dark block.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("nano_repair", 0.0, |b| {
        dark_plate(b);
        b.block(v3(-3.3, -0.9, 16.6), v3(-2.5, 0.9, 20.2));
    });
    b.module("nano_field", 0.0, |b| {
        b.paint(GLOW_VIOLET);
        b.face(&[v3(-3.0, -0.4, 20.4), MAST_HEAD, v3(-3.0, 0.4, 20.4)]);
    });
    b.module("mfe", 0.0, |b| {
        dark_plate(b);
        b.block(v3(-3.9, -1.9, 17.6), v3(-2.6, 1.9, 18.8));
    });
}
