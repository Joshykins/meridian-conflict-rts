//! The back refits, one at a time: Nanite Repair (`nano_repair`), the Nanite Repair
//! Field over it (`nano_field`), or the Material Formation Engine (`mfe`). They ride the
//! torso (`part::TURRET`), so nothing on them turns on its own.
//!
//! - Nanite Repair: a dorsal stack of keeled plates in courses down the spine over a
//!   plated core, topped by a keeled crest; the nanites show in a glazed slot let into
//!   each plate (`kit::nanite_plate`: the light is the surface's, not a lit piece).
//! - The field is its next tier, built onto the same stack: the pieces it changes are
//!   drawn `until("nano_field")` and go up again rebuilt. Every course fans a wing plate
//!   out of either side, slotted like the courses, and the crest splits into two
//!   prongs, the nanites let out between them.
//! - The engine: a low plated block across the shoulder blades, its back cut with red
//!   condenser slots, two short stacks raked back off its top corners.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, nanite_plate, v3};
use super::form::{blade, ring, sleeve, KEEL, OCT};

pub(super) fn back(b: &mut MeshBuilder) {
    b.module("nano_repair", 0.0, core);
    b.until("nano_field", |b| {
        b.module("nano_repair", 0.3, |b| courses(b, false))
    });
    b.module("nano_field", 0.2, |b| courses(b, true));
    b.module("nano_field", 0.6, prongs);
    b.module("mfe", 0.0, engine);
}

/// A keeled emitter prong from `root` to a point at `tip`, keeled toward `out`, the
/// nanites showing in its faces.
fn prong(b: &mut MeshBuilder, root: Vec3, tip: Vec3, out: Vec3, w: f32) {
    nanite_plate(b);
    sleeve(
        b,
        &[
            ring(root, out, w, w * 0.8),
            ring(root.lerp(tip, 0.55), out, w * 0.7, w * 0.6),
            ring(tip, out, 0.03, 0.03),
        ],
        &KEEL,
    );
}

/// Plates in the stack, top to bottom.
const COURSES: usize = 4;

/// Course `k` from the top: where its upper edge is.
fn course(k: usize) -> Vec3 {
    v3(-3.05 - 0.04 * k as f32, 0.0, 21.2 - 1.35 * k as f32)
}

/// The plated core the courses lie on, a feed line either side of it.
fn core(b: &mut MeshBuilder) {
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-2.6, 0.0, 15.2), -Vec3::X, 0.6, 0.3),
            ring(v3(-2.85, 0.0, 18.0), -Vec3::X, 0.7, 0.5),
            ring(v3(-2.95, 0.0, 21.3), -Vec3::X, 0.5, 0.4),
        ],
        &OCT,
    );
    b.mirror_y(|b| {
        metal(b);
        cable(
            b,
            &[
                v3(-2.6, 0.75, 15.6),
                v3(-2.95, 0.8, 18.0),
                v3(-3.0, 0.6, 20.8),
            ],
            0.13,
        );
    });
}

/// The courses; `field`: with their wings, and the crest left for the prongs.
fn courses(b: &mut MeshBuilder, field: bool) {
    let n = if b.fine() { COURSES } else { 2 };
    for k in 0..n {
        let o = course(k);
        let half = 0.95 - 0.08 * k as f32;
        nanite_plate(b);
        blade(
            b,
            o,
            Vec3::new(-0.25, 0.0, -1.0),
            -Vec3::X,
            1.85,
            half,
            0.0,
            0.42,
        );
        if field {
            b.mirror_y(|b| {
                let root = o + Vec3::new(-0.15, half * 0.7, -0.2);
                let u = Vec3::new(-0.55, 0.8, -0.35).normalize();
                let n = Vec3::new(-0.6, -0.2, 0.75).normalize();
                let len = 2.4 - 0.3 * k as f32;
                nanite_plate(b);
                blade(b, root, u, n, len, 0.46, 0.2, 0.3);
            });
        }
    }
    if !field {
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(-3.0, 0.0, 21.0), -Vec3::X, 0.45, 0.35),
                ring(v3(-3.35, 0.0, 22.2), -Vec3::X, 0.32, 0.26),
                ring(v3(-3.85, 0.0, 23.3), -Vec3::X, 0.03, 0.03),
            ],
            &KEEL,
        );
    }
}

/// The field's split crest.
fn prongs(b: &mut MeshBuilder) {
    for s in [-1.0f32, 1.0] {
        let y = Vec3::Y * s;
        prong(
            b,
            v3(-3.0, 0.3 * s, 21.0),
            v3(-4.1, 0.75 * s, 24.4),
            y,
            0.34,
        );
    }
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

/// From far off: the nanite stack a dark ridge; the field fans a wing out of either side
/// and raises the split crest. The engine a dark block.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.module("nano_repair", 0.0, |b| {
        dark_plate(b);
        b.block(v3(-3.5, -0.9, 15.4), v3(-2.6, 0.9, 21.5));
    });
    b.module("nano_field", 0.0, |b| {
        nanite_plate(b);
        b.mirror_y(|b| {
            b.face(&[
                v3(-3.1, 0.7, 21.0),
                v3(-4.4, 2.3, 19.4),
                v3(-3.3, 0.8, 17.0),
            ]);
            b.face(&[
                v3(-3.0, 0.1, 21.0),
                v3(-4.1, 0.75, 24.4),
                v3(-3.0, 0.5, 21.0),
            ]);
        });
    });
    b.module("mfe", 0.0, |b| {
        dark_plate(b);
        b.block(v3(-3.9, -1.9, 17.6), v3(-2.6, 1.9, 18.8));
    });
}
