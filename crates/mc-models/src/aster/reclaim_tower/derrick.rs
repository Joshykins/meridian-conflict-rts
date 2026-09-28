//! The lattice derrick the reclaim head stands on. Four legs of box girder taper from a
//! plated plinth to a plated head house, tied by girts and crossed braces, so the glazed
//! drop tube down the middle is seen through the steel. At the foot the tube feeds a
//! plant house, and conveyors carry its output out to bunkers on the lot's corners.
//! - Tech 1: the derrick, the plant house, two bunkers behind.
//! - Tech 2: taller; a separator drum and a walkway at mid height, two more bunkers.
//! - Tech 3: tallest; the lower third clad in plate, a second, outside tube down the
//!   back into a press house, a capacitor bank in front, obstruction lamps.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::turret::{turret, Turret};
use super::{bunker, chute, conveyor, lot_slab, ring_top};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// Half the plinth's width, and the legs' half spread at their feet.
const FOOT: f32 = 7.0;
/// The legs' half spread under the cap.
const TOP: f32 = 3.0;
/// Plinth height.
const PLINTH: f32 = 2.8;

pub(super) fn derrick(b: &mut MeshBuilder, tech: u8, kind: Turret) {
    let cap = ring_top(tech) - HOUSE;
    if b.coarse() {
        // The lot and the plinth as one low slab, the derrick as a tapered shell.
        b.paint(PLATING);
        b.cuboid(v3(0.0, 0.0, PLINTH * 0.5), v3(34.8, 34.8, PLINTH));
        b.paint(PLATING_DARK);
        b.frustum_open(
            v3(0.0, 0.0, PLINTH),
            Vec2::splat(FOOT * 1.7),
            Vec2::splat(TOP * 2.0),
            cap - PLINTH,
            Vec2::ZERO,
        );
        turret(b, tech, kind);
        return;
    }
    lot_slab(b);
    // Plinth.
    b.paint(PLATING);
    b.frustum(
        v3(0.0, 0.0, 0.0),
        Vec2::splat(FOOT * 2.0 + 3.0),
        Vec2::splat(FOOT * 2.0 + 0.6),
        PLINTH,
        Vec2::ZERO,
    );
    lattice(b, cap, tech);
    head_house(b, cap);
    // Drop tube down the middle, from the head's feed to the plant.
    let plant_top = PLINTH + 5.0;
    chute(b, v3(0.0, 0.0, cap - 1.0), v3(0.0, 0.0, plant_top), 0.85);
    plant(b, plant_top);
    turret(b, tech, kind);

    // Two bunkers behind at tech 1; two in front join them at tech 2.
    let deck = PLINTH + 3.0;
    for (x, y, tier_needed) in [
        (-12.6, 12.6, 1),
        (-12.6, -12.6, 1),
        (12.6, 12.6, 2),
        (12.6, -12.6, 2),
    ] {
        kit(b, tech, tier_needed, 0.3, |b| {
            bunker(b, v3(x, y, 0.4), 5.2, 5.0);
            conveyor(
                b,
                v3(x.signum() * 4.7, y.signum() * 2.6, deck),
                v3(x - x.signum() * 1.0, y - y.signum() * 1.0, 5.2),
            );
        });
    }
    kit(b, tech, 2, 0.55, |b| mid_works(b, cap));
    kit(b, tech, 3, 0.4, |b| heavy_works(b, cap));
}

/// Four legs from the plinth to the cap, girts at every level, braces crossed between.
fn lattice(b: &mut MeshBuilder, cap: f32, tech: u8) {
    let z0 = PLINTH;
    let z1 = cap - 1.0;
    let at = |f: f32| FOOT + (TOP - FOOT) * f;
    let levels = ((z1 - z0) / 7.5).round().max(3.0) as usize;
    b.paint(PLATING_DARK);
    for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        b.beam(
            v3(sx * FOOT, sy * FOOT, z0),
            v3(sx * TOP, sy * TOP, z1 + 0.4),
            Vec2::splat(1.25),
            Vec2::splat(0.8),
        );
    }
    let level = |i: usize| {
        (
            z0 + (z1 - z0) * i as f32 / levels as f32,
            i as f32 / levels as f32,
        )
    };
    for i in 1..levels {
        let (z, f) = level(i);
        let h = at(f);
        b.paint(PLATING_DARK);
        b.radial(4, |b| {
            b.beam(
                v3(h, -h, z),
                v3(h, h, z),
                Vec2::new(0.5, 0.6),
                Vec2::new(0.5, 0.6),
            );
        });
    }
    // Braces: crossed at full detail, one diagonal a face in the middle level.
    for i in 0..levels {
        let (za, fa) = level(i);
        let (zb, fb) = level(i + 1);
        let (ha, hb) = (at(fa), at(fb));
        if b.fine() {
            b.paint(METAL);
            b.radial(4, |b| {
                b.beam(
                    v3(ha, -ha, za),
                    v3(hb, hb, zb),
                    Vec2::splat(0.3),
                    Vec2::splat(0.3),
                );
                b.beam(
                    v3(ha, ha, za),
                    v3(hb, -hb, zb),
                    Vec2::splat(0.3),
                    Vec2::splat(0.3),
                );
            });
        } else if b.mid() && i % 3 == 1 {
            b.paint(METAL);
            b.radial(4, |b| {
                b.beam(
                    v3(ha, -ha, za),
                    v3(hb, hb, zb),
                    Vec2::splat(0.35),
                    Vec2::splat(0.35),
                );
            });
        }
    }
    if b.fine() && tech >= 1 {
        // A ladder cage up the back face.
        b.paint(METAL);
        b.beam(
            v3(-FOOT - 0.3, 0.0, z0),
            v3(-TOP - 0.5, 0.0, z1),
            Vec2::new(0.9, 0.2),
            Vec2::new(0.9, 0.2),
        );
    }
}

/// The plant house the drop tube feeds, between the legs.
fn plant(b: &mut MeshBuilder, top: f32) {
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.chamfered_box(
            v3(0.0, 0.0, (PLINTH + top) * 0.5),
            v3(9.0, 9.0, top - PLINTH),
            1.4,
        );
    });
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, top - 0.02), b.sides(8), 1.9, 1.3, 0.8);
    if b.fine() {
        // Glazed ports down the sides show the stream falling into the plant.
        b.radial(4, |b| {
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(v3(4.46, -1.0, PLINTH + 0.9), v3(4.6, 1.0, top - 0.9));
        });
    }
    if b.fine() {
        b.paint(PLATING_DARK);
        b.plate(v3(0.0, 0.0, top), Vec2::new(7.0, 7.0), 0.14, 0.05);
        vent(b, v3(-2.3, 2.3, top + 0.14), Vec2::new(1.9, 1.2), 3, ACCENT);
    }
}

/// Tech 2: the separator drum the tube runs through at mid height, and a walkway round
/// the derrick at that level.
fn mid_works(b: &mut MeshBuilder, cap: f32) {
    let z = PLINTH + (cap - PLINTH) * 0.5;
    let half = FOOT + (TOP - FOOT) * ((z - PLINTH) / (cap - 1.0 - PLINTH));
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, z - 2.4), b.sides(10), 2.0, 2.0, 4.0);
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, z - 3.6), b.sides(10), 2.0, 0.9, 1.2);
    if b.mid() {
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.prism(v3(0.0, 0.0, z - 1.0), b.sides(10), 2.06, 2.06, 1.0);
    }
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, z - 3.0),
        Vec2::splat(half * 2.0 + 2.6),
        Vec2::splat(half * 2.0 + 2.6),
        0.35,
        Vec2::ZERO,
    );
    if b.fine() {
        b.paint(METAL);
        rail_square(b, z - 2.65, half + 1.2, 1.0);
    }
}

/// Tech 3: cladding over the lower third, an outside tube down the back into a press
/// house, capacitor banks on the flanks.
fn heavy_works(b: &mut MeshBuilder, cap: f32) {
    let z1 = PLINTH + (cap - PLINTH) * 0.3;
    let top = FOOT + (TOP - FOOT) * 0.3 + 0.5;
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, PLINTH),
        Vec2::splat(FOOT * 2.0 + 1.2),
        Vec2::splat(top * 2.0),
        z1 - PLINTH,
        Vec2::ZERO,
    );
    b.paint(ACCENT);
    b.frustum(
        v3(0.0, 0.0, z1),
        Vec2::splat(top * 2.0 + 0.4),
        Vec2::splat(top * 2.0 - 0.4),
        0.6,
        Vec2::ZERO,
    );
    // The second tube: from the cap down the outside of the back face.
    let lean = |f: f32| -(FOOT + (TOP - FOOT) * f) - 1.6;
    chute(
        b,
        v3(lean(1.0), 0.0, cap - 1.4),
        v3(lean(0.3), 0.0, z1 + 0.6),
        0.75,
    );
    chute(b, v3(lean(0.3), 0.0, z1 + 0.6), v3(-13.6, 0.0, 7.6), 0.7);
    // Press house behind the plant, between the rear bunkers.
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.chamfered_box(v3(-13.8, 0.0, 3.8), v3(5.0, 7.6, 7.2), 1.0);
    });
    b.paint(PLATING_DARK);
    b.plate(v3(-13.8, 0.0, 7.4), Vec2::new(4.0, 6.2), 0.2, 0.06);
    team_panel(b, v3(-13.8, 0.0, 7.6), Vec2::new(2.4, 4.0));
    // A capacitor bank in front, between the front bunkers.
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(13.6, 0.0, 2.2), v3(4.2, 8.4, 4.4), 0.6);
    if b.fine() {
        b.paint(METAL);
        for y in [-2.6, 0.0, 2.6] {
            b.prism(v3(13.6, y, 4.4), 8, 1.0, 1.0, 1.5);
        }
    }
    if b.fine() {
        // Obstruction lamps on the head house's corners: a 68 m mast needs them.
        b.paint(GLOW_RED);
        b.radial(4, |b| {
            b.cuboid(
                v3(TOP + 0.9, TOP + 0.9, cap + HOUSE + 0.25),
                Vec3::splat(0.4),
            )
        });
    }
}

/// How tall the plated head house under the slewing ring is.
const HOUSE: f32 = 3.2;

/// The plated head house on the derrick's cap, wider than the legs: the machinery deck
/// the head turns on, the drop tube's feed hopper inside it, a rail round its roof.
fn head_house(b: &mut MeshBuilder, cap: f32) {
    let half = TOP + 1.5;
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.chamfered_box(
            v3(0.0, 0.0, cap + HOUSE * 0.5),
            v3(half * 2.0, half * 2.0, HOUSE),
            0.9,
        );
    });
    // A dark skirt under it where the legs go in, and a band round its top.
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, cap - 1.4),
        Vec2::splat(TOP * 2.0 + 0.6),
        Vec2::splat(half * 2.0 - 0.4),
        1.4,
        Vec2::ZERO,
    );
    if b.mid() {
        b.paint(ACCENT);
        b.chamfered_box(
            v3(0.0, 0.0, cap + HOUSE - 0.35),
            v3(half * 2.0 + 0.2, half * 2.0 + 0.2, 0.5),
            1.0,
        );
    }
    if b.fine() {
        // Access doors and louvres on the faces.
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.block(
                v3(half - 0.02, -0.8, cap + 0.3),
                v3(half + 0.1, 0.8, cap + 2.3),
            );
        });
        b.paint(METAL);
        rail_square(b, cap + HOUSE, half - 0.2, 1.0);
    }
}

/// A square guard rail at height `z` round a deck of half width `half`.
fn rail_square(b: &mut MeshBuilder, z: f32, half: f32, height: f32) {
    b.radial(4, |b| {
        b.beam(
            v3(half, -half, z + height),
            v3(half, half, z + height),
            Vec2::splat(0.12),
            Vec2::splat(0.12),
        );
        for f in [-0.66, 0.0, 0.66] {
            b.beam(
                v3(half, half * f, z),
                v3(half, half * f, z + height),
                Vec2::splat(0.1),
                Vec2::splat(0.1),
            );
        }
    });
}
