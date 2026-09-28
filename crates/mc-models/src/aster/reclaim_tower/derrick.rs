//! `reclaim_tower~a`: the lattice derrick. Four legs of box girder taper from a plated
//! plinth to the slewing ring, tied by girts and crossed braces, so the glazed drop tube
//! down the middle is seen through the steel. At the foot the tube feeds a plant house,
//! and conveyors carry its output out to bunkers on the lot.
//! - Tech 1: the derrick, the plant house, two bunkers behind.
//! - Tech 2: taller; a separator drum and a walkway at mid height, two more bunkers.
//! - Tech 3: tallest; the lower third clad in plate, a second, outside tube down the
//!   back, a press house behind the plant, capacitor banks on the flanks.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::super::structures::kit;
use super::head::{head, Style};
use super::{bunker, chute, conveyor, lot_slab, ring_top};
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::pattern;

/// Half the plinth's width, and the legs' half spread at their feet.
const FOOT: f32 = 8.6;
/// The legs' half spread under the cap.
const TOP: f32 = 2.9;
/// Plinth height.
const PLINTH: f32 = 3.2;

pub(in super::super) fn derrick(b: &mut MeshBuilder, tech: u8) {
    let cap = ring_top(tech) - 1.6;
    if !b.coarse() {
        lot_slab(b);
    }
    // Plinth.
    b.paint(PLATING);
    b.frustum(
        v3(0.0, 0.0, 0.0),
        Vec2::splat(FOOT * 2.0 + 3.0),
        Vec2::splat(FOOT * 2.0 + 0.6),
        PLINTH,
        Vec2::ZERO,
    );
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.frustum_open(
            v3(0.0, 0.0, PLINTH),
            Vec2::splat(FOOT * 1.7),
            Vec2::splat(TOP * 2.0),
            cap - PLINTH,
            Vec2::ZERO,
        );
        head(b, tech, Style::Boom);
        return;
    }
    lattice(b, cap, tech);
    // The cap deck the head turns on.
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, cap - 1.2),
        Vec2::splat(TOP * 2.0 + 0.6),
        Vec2::splat(TOP * 2.0 + 1.6),
        1.2,
        Vec2::ZERO,
    );
    if b.fine() {
        b.paint(METAL);
        rail_square(b, cap, TOP + 0.8, 1.0);
    }
    // Drop tube down the middle, from the head's feed to the plant.
    let plant_top = PLINTH + 6.0;
    chute(b, v3(0.0, 0.0, cap - 1.2), v3(0.0, 0.0, plant_top), 0.95);
    plant(b, plant_top);
    head(b, tech, Style::Boom);

    // Two bunkers behind at tech 1; two in front join them at tech 2.
    let deck = PLINTH + 3.5;
    for (x, y, tier_needed) in [
        (-16.0, 15.0, 1),
        (-16.0, -15.0, 1),
        (16.0, 15.0, 2),
        (16.0, -15.0, 2),
    ] {
        kit(b, tech, tier_needed, 0.3, |b| {
            bunker(b, v3(x, y, 0.4), 6.5, 6.0);
            conveyor(
                b,
                v3(x.signum() * 5.6, y.signum() * 3.0, deck),
                v3(x, y - y.signum() * 2.0, 6.2),
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
            v3(10.8, 10.8, top - PLINTH),
            1.6,
        );
    });
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, top - 0.02), b.sides(8), 2.2, 1.5, 0.9);
    if b.fine() {
        // Glazed ports down the sides show the stream falling into the plant.
        b.radial(4, |b| {
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(v3(5.36, -1.1, PLINTH + 1.0), v3(5.5, 1.1, top - 1.0));
        });
    }
    if b.fine() {
        b.paint(PLATING_DARK);
        b.plate(v3(0.0, 0.0, top), Vec2::new(8.4, 8.4), 0.14, 0.05);
        vent(b, v3(-2.8, 2.8, top + 0.14), Vec2::new(2.2, 1.4), 3, ACCENT);
    }
}

/// Tech 2: the separator drum the tube runs through at mid height, and a walkway round
/// the derrick at that level.
fn mid_works(b: &mut MeshBuilder, cap: f32) {
    let z = PLINTH + (cap - PLINTH) * 0.5;
    let half = FOOT + (TOP - FOOT) * ((z - PLINTH) / (cap - 1.0 - PLINTH));
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, z - 2.4), b.sides(10), 2.4, 2.4, 4.2);
    b.paint(PLATING_DARK);
    b.prism(v3(0.0, 0.0, z - 3.6), b.sides(10), 2.4, 1.0, 1.2);
    if b.mid() {
        b.paint(ACCENT).pattern(pattern::MASS_FLOW);
        b.prism(v3(0.0, 0.0, z - 1.0), b.sides(10), 2.46, 2.46, 1.1);
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
    chute(b, v3(lean(0.3), 0.0, z1 + 0.6), v3(-18.0, 0.0, 9.0), 0.75);
    // Press house behind the plant.
    b.paint(PLATING);
    b.with_bevel(0.2, |b| {
        b.chamfered_box(v3(-18.5, 0.0, 4.6), v3(7.0, 9.0, 8.8), 1.2);
    });
    b.paint(PLATING_DARK);
    b.plate(v3(-18.5, 0.0, 9.0), Vec2::new(5.8, 7.6), 0.2, 0.06);
    team_panel(b, v3(-18.5, 0.0, 9.2), Vec2::new(3.0, 5.0));
    // Capacitor banks on the flanks.
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(2.0, 18.0, 2.2), v3(9.0, 4.2, 4.4), 0.6);
        if b.fine() {
            b.paint(METAL);
            for x in [-0.8, 2.0, 4.8] {
                b.prism(v3(x, 18.0, 4.4), 8, 1.1, 1.1, 1.6);
            }
        }
    });
    if b.fine() {
        // Obstruction lamps at the cap corners: an 84 m mast needs them.
        b.paint(GLOW_RED);
        b.radial(4, |b| {
            b.cuboid(v3(TOP + 0.6, TOP + 0.6, cap + 0.2), Vec3::splat(0.4))
        });
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
