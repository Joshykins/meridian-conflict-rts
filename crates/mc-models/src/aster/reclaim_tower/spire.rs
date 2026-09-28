//! `reclaim_tower~b`: the armoured spire. A solid faceted shaft tapers up from a
//! buttressed octagonal plinth to the slewing ring, and the glazed chute winds down
//! round it, so the stream spirals to the ground. Hoppers on the plinth take it, and
//! conveyors carry it out to bunkers on the lot.
//! - Tech 1: the spire, one spiral chute, the plinth hopper, a bunker to its side.
//! - Tech 2: taller; a second chute makes a double helix; a gallery collar at mid
//!   height; a bunker on the other side.
//! - Tech 3: tallest; flying struts from the plinth to the shaft, a crown ring under
//!   the head, a bunker behind and capacitor banks.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::head::{head, Style};
use super::{bunker, chute, conveyor, lot_slab, ring_top, tier};
use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::pattern;

/// The plinth: its radius on the ground, at its top, and its height.
const PLINTH_R: f32 = 15.0;
const PLINTH_TOP_R: f32 = 11.5;
const PLINTH: f32 = 5.5;
/// The shaft's radius at the plinth and under the head.
const SHAFT_FOOT: f32 = 6.6;
const SHAFT_TOP: f32 = 3.3;
/// How far out from the shaft the chute runs.
const CHUTE_OUT: f32 = 1.35;
/// Turns of the spiral per tier.
const TURNS: [f32; 3] = [1.5, 2.0, 2.5];

pub(in super::super) fn spire(b: &mut MeshBuilder, tech: u8) {
    let cap = ring_top(tech) - 1.6;
    if !b.coarse() {
        lot_slab(b);
    }
    let plan = ngon(b.sides(8), 1.0);
    b.paint(PLATING);
    b.loft_z(
        &turned(&plan),
        &[
            Section::new(0.0, PLINTH_R),
            Section::new(PLINTH * 0.45, PLINTH_R - 0.6),
            Section::new(PLINTH, PLINTH_TOP_R),
        ],
    );
    // The shaft, in plated courses between dark bands.
    b.paint(PLATING);
    let shaft = |z: f32| SHAFT_FOOT + (SHAFT_TOP - SHAFT_FOOT) * ((z - PLINTH) / (cap - PLINTH));
    b.loft_z(
        &turned(&plan),
        &[
            Section::new(PLINTH - 0.1, SHAFT_FOOT),
            Section::new(cap, SHAFT_TOP),
        ],
    );
    if b.coarse() {
        head(b, tech, Style::Pod);
        return;
    }
    let bands = ((cap - PLINTH) / 12.0).round().max(2.0) as usize;
    b.paint(ACCENT);
    for i in 1..=bands {
        let z = PLINTH + (cap - PLINTH) * i as f32 / (bands as f32 + 0.3);
        b.loft_z(
            &turned(&plan),
            &[
                Section::new(z - 0.5, shaft(z) + 0.18),
                Section::new(z + 0.5, shaft(z) + 0.18),
            ],
        );
    }
    b.paint(PLATING_DARK);
    b.loft_z(
        &turned(&plan),
        &[
            Section::new(cap - 0.1, SHAFT_TOP + 0.5),
            Section::new(cap + 0.1, SHAFT_TOP + 0.5),
        ],
    );
    buttresses(b);
    head(b, tech, Style::Pod);

    // The spiral chute, and a second one opposite from tech 2.
    let t = tier(tech);
    helix(b, cap, shaft, TURNS[t], FRAC_PI_2);
    kit(b, tech, 2, 0.35, |b| {
        helix(b, cap, shaft, TURNS[t], -FRAC_PI_2)
    });
    // Hoppers on the plinth take each chute; conveyors carry it out to bunkers.
    let hopper_at = |side: f32| v3(0.0, side * (PLINTH_TOP_R - 2.2), PLINTH);
    for (side, tier_needed) in [(1.0, 1), (-1.0, 2)] {
        kit(b, tech, tier_needed, 0.3, |b| {
            let at = hopper_at(side);
            b.paint(PLATING_DARK);
            b.chamfered_box(at + Vec3::Z * 1.3, v3(3.6, 3.2, 2.6), 0.5);
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.prism(at + Vec3::Z * 2.55, b.sides(8), 1.3, 0.9, 0.9);
            bunker(b, v3(-2.0, side * 18.5, 0.4), 7.0, 6.5);
            conveyor(b, at + v3(0.0, side * 1.6, 1.8), v3(-2.0, side * 15.4, 6.6));
        });
    }
    kit(b, tech, 2, 0.55, |b| collar(b, cap, shaft));
    kit(b, tech, 3, 0.4, |b| heavy_works(b, cap, shaft));
}

/// The octagon turned so its flats face the axes.
fn turned(plan: &[[f32; 2]]) -> Vec<[f32; 2]> {
    plan.iter().map(|p| super::rotate8(*p)).collect()
}

/// Four buttresses on the diagonals, from the plinth's foot up the shaft.
fn buttresses(b: &mut MeshBuilder) {
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            b.extrude_y_chamfered(
                &[
                    [SHAFT_FOOT - 0.8, PLINTH - 0.2],
                    [PLINTH_R + 3.5, 0.0],
                    [PLINTH_R + 3.5, 1.6],
                    [SHAFT_FOOT - 0.4, PLINTH + 13.0],
                ],
                1.3,
                0.35,
            );
            if b.fine() {
                b.paint(ACCENT);
                b.block(v3(PLINTH_R + 3.4, -1.0, 0.2), v3(PLINTH_R + 3.62, 1.0, 1.4));
            }
        });
    });
}

/// A spiral chute from under the head down to the plinth, ending at angle `end` (on the
/// side hopper), with a bracket to the shaft every few segments.
fn helix(b: &mut MeshBuilder, cap: f32, shaft: impl Fn(f32) -> f32, turns: f32, end: f32) {
    let per_turn = if b.fine() { 16.0 } else { 6.0 };
    let sides = if b.fine() { 8 } else { 4 };
    let n = (turns * per_turn).round() as usize;
    let (z_top, z_bot) = (cap - 1.8, PLINTH + 3.4);
    let at = |i: usize| {
        let u = i as f32 / n as f32;
        let z = z_top + (z_bot - z_top) * u;
        let a = end - turns * TAU * (1.0 - u);
        let r = shaft(z) + CHUTE_OUT;
        v3(r * a.cos(), r * a.sin(), z)
    };
    // From the head's feed out to where the spiral starts.
    chute(b, v3(0.0, 0.0, cap), at(0), 0.62);
    for i in 0..n {
        let (p, q) = (at(i), at(i + 1));
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.cylinder_between(p, q, 0.62, 0.62, sides);
        if b.fine() && i % 4 == 2 {
            let r = p.truncate().normalize_or_zero();
            b.paint(PLATING_DARK);
            b.beam(
                p - r.extend(0.0) * CHUTE_OUT - Vec3::Z * 0.2,
                p - Vec3::Z * 0.7,
                Vec2::new(0.5, 0.4),
                Vec2::new(0.5, 0.4),
            );
        }
    }
    // Down into the plinth hopper.
    let last = at(n);
    chute(
        b,
        last,
        v3(0.0, end.sin() * (PLINTH_TOP_R - 2.2), PLINTH + 2.9),
        0.62,
    );
}

/// Tech 2: a gallery collar at mid height, glazed all round.
fn collar(b: &mut MeshBuilder, cap: f32, shaft: impl Fn(f32) -> f32) {
    let z = PLINTH + (cap - PLINTH) * 0.52;
    let r = shaft(z) + CHUTE_OUT + 1.2;
    let plan = turned(&ngon(b.sides(8), 1.0));
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[
            Section::new(z - 1.6, shaft(z - 1.6) + 0.1),
            Section::new(z - 0.4, r),
            Section::new(z + 1.8, r),
            Section::new(z + 2.4, r - 0.8),
        ],
    );
    if b.mid() {
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::new(z + 0.4, r + 0.05),
                Section::new(z + 1.3, r + 0.05),
            ],
        );
    }
}

/// Tech 3: flying struts from the plinth to the shaft, a crown ring under the head, a
/// bunker behind, capacitor banks forward.
fn heavy_works(b: &mut MeshBuilder, cap: f32, shaft: impl Fn(f32) -> f32) {
    let z = PLINTH + (cap - PLINTH) * 0.4;
    b.radial(4, |b| {
        b.paint(PLATING_DARK);
        b.beam(
            v3(PLINTH_TOP_R - 0.8, 0.0, PLINTH),
            v3(shaft(z) + 0.2, 0.0, z),
            Vec2::new(1.6, 1.4),
            Vec2::new(1.0, 1.0),
        );
    });
    let plan = turned(&ngon(b.sides(8), 1.0));
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(cap - 5.0, shaft(cap - 5.0) + 0.1),
            Section::new(cap - 3.4, SHAFT_TOP + 2.8),
            Section::new(cap - 2.2, SHAFT_TOP + 2.8),
        ],
    );
    bunker(b, v3(-19.0, 0.0, 0.4), 7.5, 7.5);
    conveyor(
        b,
        v3(-PLINTH_TOP_R + 1.0, 0.0, PLINTH + 0.8),
        v3(-15.5, 0.0, 7.6),
    );
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(17.5, 9.0, 2.2), v3(5.0, 8.0, 4.4), 0.6);
        b.paint(METAL);
        for y in [6.6, 9.0, 11.4] {
            b.prism(v3(17.5, y, 4.4), b.sides(8), 1.0, 1.0, 1.4);
        }
    });
    if b.fine() {
        b.paint(GLOW_RED);
        b.radial(4, |b| {
            b.cuboid(v3(SHAFT_TOP + 2.7, 0.0, cap - 2.0), Vec3::splat(0.4));
        });
    }
}
