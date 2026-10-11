//! The engineering works round the foot, each arriving with its tier:
//! - parts magazines on the step behind and beside the foot, drawers in their faces
//!   (tech 1);
//! - canister racks on the step's corners, cabled to the plinth (rear at tech 2,
//!   front at tech 3);
//! - spare projector heads docked on the mast's back quarters (tech 2).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::body::mast_face;
use super::{BASE, DECK, FOOT, STEP};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// The racks' and magazines' distance out from the axis.
const CORNER: f32 = 11.8;
const MAGAZINE: f32 = 12.4;

pub(super) fn works(b: &mut MeshBuilder, tech: u8) {
    for a in [0.0, FRAC_PI_2, -FRAC_PI_2] {
        b.yawed(Vec3::ZERO, a, magazine);
    }
    kit(b, tech, 2, 0.4, |b| {
        b.mirror_y(|b| rack(b, -CORNER));
    });
    kit(b, tech, 3, 0.5, |b| {
        b.mirror_y(|b| rack(b, CORNER));
    });
    kit(b, tech, 2, 0.7, |b| {
        b.mirror_y(|b| b.yawed(Vec3::ZERO, PI - 0.36, spare));
    });
}

/// A parts magazine on the step (built facing +x, on the -x side of the foot turned
/// out): an armoured case, three drawer fronts in its face, a conduit along its brow, a
/// lid on top.
fn magazine(b: &mut MeshBuilder) {
    let x = -MAGAZINE;
    b.paint(PLATING);
    b.chamfered_box(v3(x, 0.0, STEP + 1.4), v3(2.4, 5.0, 2.8), 0.5);
    if b.fine() {
        b.paint(PLATING_DARK);
        for y in [-1.55, 0.0, 1.55] {
            b.block(
                v3(x - 1.26, y - 0.6, STEP + 0.4),
                v3(x - 1.1, y + 0.6, STEP + 2.0),
            );
        }
    }
    if b.mid() {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(
            v3(x - 1.28, -2.0, STEP + 2.2),
            v3(x - 1.15, 2.0, STEP + 2.5),
        );
    }
    b.paint(METAL);
    b.plate(
        v3(x + 0.1, 0.0, STEP + 2.8),
        Vec2::new(1.6, 3.8),
        0.12,
        0.05,
    );
    if b.fine() {
        // Its feed into the plinth.
        b.paint(PLATING_DARK);
        b.beam(
            v3(x + 1.2, 0.0, STEP + 0.5),
            v3(-FOOT - 0.4, 0.0, BASE + 0.4),
            Vec2::new(0.8, 0.5),
            Vec2::new(0.8, 0.5),
        );
    }
}

/// A canister rack on the step's corner at (`x`, `CORNER`) (+y side), facing out: a
/// dark base, a frame behind, three canisters with lit bands and domed caps, a cable
/// into the plinth.
fn rack(b: &mut MeshBuilder, x: f32) {
    let out = if x > 0.0 { FRAC_PI_4 } else { 3.0 * FRAC_PI_4 };
    b.at(v3(x, CORNER, STEP), |b| {
        b.yawed(Vec3::ZERO, out, |b| {
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(0.0, 0.0, 0.25), v3(2.2, 5.0, 0.5), 0.4);
            b.paint(PLATING);
            b.block(v3(-1.15, -2.4, 0.5), v3(-0.8, 2.4, 3.4));
            b.block(v3(-1.15, -2.4, 3.4), v3(0.4, 2.4, 3.7));
            for y in [-1.6, 0.0, 1.6] {
                b.paint(METAL);
                b.prism(v3(0.1, y, 0.5), b.sides(6), 0.68, 0.68, 2.3);
                if b.fine() {
                    b.paint(PLATING);
                    b.prism(v3(0.1, y, 2.8), 6, 0.68, 0.36, 0.45);
                }
                if b.fine() {
                    b.paint(ACCENT).pattern(pattern::CONDUIT);
                    b.prism(v3(0.1, y, 1.5), 6, 0.71, 0.71, 0.4);
                }
            }
        });
    });
    if b.fine() {
        b.paint(METAL);
        let dir = Vec2::new(x, CORNER).normalize();
        let from = Vec2::new(x, CORNER) - dir * 1.6;
        let to = Vec2::new(x, CORNER) - dir * 4.0;
        b.cylinder_between(
            from.extend(STEP + 0.3),
            to.extend(STEP + 0.3),
            0.22,
            0.22,
            6,
        );
    }
}

/// A spare projector head docked on the mast's back quarter (built on +x), standing
/// nose up in a cradle off the mast: its collar, the head, its unlit lens and prongs.
fn spare(b: &mut MeshBuilder) {
    let (z0, z1) = (DECK - 3.4, DECK - 1.0);
    let r = 0.62;
    let x = mast_face(z0) + 1.2;
    let at = |u: f32| v3(x, 0.0, z0 + (z1 - z0) * u);
    let sides = b.sides(8);
    // The cradle: a bracket off the mast and a clamp round the collar.
    b.paint(PLATING_DARK);
    b.block(
        v3(mast_face(z0) - 0.2, -0.45, z0 + 0.1),
        v3(x - 0.3, 0.45, z0 + 0.9),
    );
    b.paint(METAL);
    b.cylinder_between(at(0.05), at(0.3), r * 1.05, r * 1.05, sides);
    // The head.
    b.paint(PLATING_DARK);
    b.cylinder_between(at(0.0), at(0.32), r * 0.8, r * 0.86, sides);
    b.paint(PLATING);
    b.prism(at(0.3), sides, r * 1.15, r * 1.1, (z1 - z0) * 0.26);
    if b.fine() {
        b.paint(PLATING_DARK);
        b.cylinder_between(at(0.56), at(0.62), r * 0.95, r * 0.7, sides);
        b.paint(ACCENT);
        b.cylinder_between(at(0.62), at(0.64), r * 0.5, r * 0.46, sides);
        b.paint(METAL);
        for k in 0..3 {
            let a = FRAC_PI_2 + k as f32 * TAU / 3.0;
            let off = Vec3::new(a.cos(), a.sin(), 0.0);
            b.beam(
                at(0.56) + off * r * 0.9,
                at(1.0) + off * r * 0.3,
                Vec2::splat(0.16),
                Vec2::splat(0.1),
            );
        }
    }
}
