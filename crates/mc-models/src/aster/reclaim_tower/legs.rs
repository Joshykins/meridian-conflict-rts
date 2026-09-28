//! `reclaim_tower~c`: the four-legged installation. Plated legs rake in from the lot's
//! corners to a crown under the slewing ring, like a headframe, and each working leg
//! carries a glazed chute down its back into a bunker at its foot. A storage silo stands
//! between the legs, fed from the bunkers.
//! - Tech 1: the four legs and crown; chutes down the two back legs into their bunkers;
//!   the silo.
//! - Tech 2: taller; chutes and bunkers on the front legs too; a ring girder and walkway
//!   tying the legs at mid height.
//! - Tech 3: tallest; a second ring girder high up, the silo raised with a gantry head,
//!   the lower legs clad, capacitor banks between the front feet.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::head::{head, Style};
use super::{bunker, chute, conveyor, lot_slab, ring_top};
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;

/// Where a leg meets the ground (each of x, y), and where it meets the crown.
const FOOT: f32 = 16.8;
const CROWN: f32 = 3.6;
/// Bunker size at a leg's foot.
const BUNKER: f32 = 7.4;
const BUNKER_H: f32 = 7.0;

pub(in super::super) fn legs(b: &mut MeshBuilder, tech: u8) {
    let cap = ring_top(tech) - 1.6;
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.0, 0.0, 0.0),
            Vec2::splat(FOOT * 2.0),
            Vec2::splat(CROWN * 2.0),
            cap,
            Vec2::ZERO,
        );
        head(b, tech, Style::Jaws);
        return;
    }
    lot_slab(b);
    let leg = |sx: f32, sy: f32, z: f32| {
        let f = z / cap;
        v3(
            sx * (FOOT + (CROWN - FOOT) * f),
            sy * (FOOT + (CROWN - FOOT) * f),
            z,
        )
    };
    // Legs: box girders, wide at the foot, their footings, the crown they meet in.
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.paint(PLATING);
        b.beam(
            leg(sx, sy, 0.0),
            leg(sx, sy, cap - 0.8),
            Vec2::new(2.8, 2.8),
            Vec2::new(1.6, 1.6),
        );
        b.paint(PLATING_DARK);
        b.chamfered_box(leg(sx, sy, 0.0) + Vec3::Z * 0.9, v3(4.2, 4.2, 1.8), 0.5);
    }
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.chamfered_box(
            v3(0.0, 0.0, cap - 1.2),
            v3(CROWN * 2.0 + 3.2, CROWN * 2.0 + 3.2, 2.4),
            1.4,
        );
    });
    b.paint(ACCENT);
    b.chamfered_box(
        v3(0.0, 0.0, cap - 2.7),
        v3(CROWN * 2.0 + 1.6, CROWN * 2.0 + 1.6, 0.8),
        1.0,
    );
    head(b, tech, Style::Jaws);

    // Chutes down the back legs into their bunkers; the front pair from tech 2.
    for (sx, sy, tier_needed) in [
        (-1.0, 1.0, 1),
        (-1.0, -1.0, 1),
        (1.0, 1.0, 2),
        (1.0, -1.0, 2),
    ] {
        kit(b, tech, tier_needed, 0.3, |b| {
            let foot = leg(sx, sy, 0.0);
            bunker(
                b,
                foot.with_z(0.0) + v3(-sx * 1.0, -sy * 1.0, 0.0),
                BUNKER,
                BUNKER_H,
            );
            // The chute rides the leg's inner face, off it on brackets.
            let inset = |p: Vec3| p - v3(sx, sy, 0.0) * 1.55 + Vec3::Z * 0.6;
            let top = inset(leg(sx, sy, cap - 3.2));
            let low = inset(leg(sx, sy, BUNKER_H + 2.4));
            chute(b, v3(0.0, 0.0, cap - 1.4), top, 0.7);
            chute(b, top, low, 0.7);
            let throat = foot.with_z(BUNKER_H + 1.2) - v3(sx, sy, 0.0) * 1.0;
            chute(b, low, throat, 0.7);
            if b.fine() {
                b.paint(PLATING_DARK);
                for f in [0.3, 0.55, 0.8] {
                    let z = BUNKER_H + 2.4 + (cap - 5.6 - BUNKER_H) * f;
                    b.beam(
                        leg(sx, sy, z),
                        inset(leg(sx, sy, z)),
                        Vec2::splat(0.45),
                        Vec2::splat(0.45),
                    );
                }
            }
        });
    }
    silo(b, tech);
    kit(b, tech, 2, 0.5, |b| ring_girder(b, cap * 0.46, &leg));
    kit(b, tech, 3, 0.35, |b| {
        ring_girder(b, cap * 0.74, &leg);
        heavy_works(b, cap, &leg);
    });
}

/// The storage silo between the legs, a conveyor from each rear bunker up into it.
fn silo(b: &mut MeshBuilder, tech: u8) {
    let r = 5.2;
    let h = if tech >= 3 { 20.0 } else { 15.0 };
    let plan = ngon(b.sides(12), r);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(0.3, 1.0),
            Section::new(h, 1.0),
            Section::new(h + 2.2, 0.55),
        ],
    );
    if b.mid() {
        b.paint(ACCENT);
        for z in [3.2, h * 0.5 + 1.5, h - 1.0] {
            b.prism(v3(0.0, 0.0, z), b.sides(12), r + 0.2, r + 0.2, 0.55);
        }
    }
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, h + 2.1), b.sides(8), 1.4, 1.2, 0.8);
    team_panel(b, v3(r * 0.7, 0.0, h + 1.1), Vec2::new(1.4, 2.0));
    for sy in [1.0, -1.0] {
        conveyor(
            b,
            v3(-FOOT + 3.2, sy * (FOOT - 3.2), BUNKER_H - 1.0),
            v3(-r * 0.6, sy * r * 0.6, h - 1.5),
        );
    }
}

/// A ring girder tying the four legs at height `z`, a walkway on it.
fn ring_girder(b: &mut MeshBuilder, z: f32, leg: &impl Fn(f32, f32, f32) -> Vec3) {
    let h = leg(1.0, 1.0, z).x;
    b.paint(PLATING_DARK);
    b.radial(4, |b| {
        b.beam(
            v3(h, -h, z),
            v3(h, h, z),
            Vec2::new(1.2, 1.4),
            Vec2::new(1.2, 1.4),
        );
    });
    if b.fine() {
        b.paint(PLATING);
        b.radial(4, |b| {
            b.beam(
                v3(h + 1.0, -h - 1.0, z + 0.7),
                v3(h + 1.0, h + 1.0, z + 0.7),
                Vec2::new(1.4, 0.2),
                Vec2::new(1.4, 0.2),
            );
        });
    }
    if b.fine() {
        b.paint(METAL);
        b.radial(4, |b| {
            b.beam(
                v3(h + 1.6, -h - 1.6, z + 1.8),
                v3(h + 1.6, h + 1.6, z + 1.8),
                Vec2::splat(0.12),
                Vec2::splat(0.12),
            );
        });
    }
}

/// Tech 3: cladding on the lower legs, capacitor banks between the front feet, lamps.
fn heavy_works(b: &mut MeshBuilder, cap: f32, leg: &impl Fn(f32, f32, f32) -> Vec3) {
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.paint(PLATING_DARK);
        b.beam(
            leg(sx, sy, 2.0),
            leg(sx, sy, cap * 0.3),
            Vec2::new(3.4, 3.4),
            Vec2::new(2.9, 2.9),
        );
    }
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(FOOT + 0.5, 0.0, 2.3), v3(5.0, 14.0, 4.6), 0.7);
    b.paint(METAL);
    for y in [-4.2, 0.0, 4.2] {
        b.prism(v3(FOOT + 0.5, y, 4.6), b.sides(8), 1.2, 1.2, 1.5);
    }
    if b.fine() {
        b.paint(GLOW_RED);
        b.radial(4, |b| {
            b.cuboid(v3(CROWN + 1.9, CROWN + 1.9, cap + 0.3), Vec3::splat(0.4));
        });
    }
}
