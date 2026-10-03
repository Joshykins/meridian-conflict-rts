//! Design B (`fabricator~ring`): a square accelerator ring round the lot, its coils driving
//! the charge round and four injectors firing it into a squat vessel in the middle; the
//! banks outside the ring on the grid side (-x), the hopper inside it.
//! - Tech 1: the ring with a coil on each side, the injectors, the vessel, one bank.
//! - Tech 2: coils all round the ring, an upper stage, a gantry over the ring's corners
//!   holding the stage's collar, banks on the flanks, radiators.
//! - Tech 3: a third stage under a crown on the gantry raised higher, a second bank row and
//!   more cooling.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// Half the ring's span (to its duct's middle), the duct's size and the height of its
/// middle.
const RING: f32 = 12.0;
const DUCT: f32 = 2.4;
const DUCT_Z: f32 = 3.4;
/// The vessel's stages: (radius, foot, top of the straight body).
const STAGES: [(f32, f32, f32); 3] = [(4.8, DECK, 9.5), (3.8, 12.4, 17.0), (2.8, 19.1, 23.4)];
/// The gantry posts' tops at tech 2 and 3.
const GANTRY: [f32; 2] = [18.6, 26.0];

fn hatch(stage: usize) -> f32 {
    let (r, _, z1) = STAGES[stage];
    z1 + r * 0.35 + 0.7
}

pub(in crate::aster) fn ring_plant(b: &mut MeshBuilder, tech: u8) {
    standalone(b, tech, plant);
}

/// The plant drawn with every tier's machinery up to `tech`, at the 4x4 lot's scale.
fn plant(b: &mut MeshBuilder, tech: u8) {
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    slab(b);
    deck_marks(b, 19.8);

    // ---- Tech 1: the ring, a coil on each side, the injectors, the vessel, a bank.
    b.radial(4, |b| {
        duct(b);
        coil_on(b, v3(RING, -0.7, DUCT_Z), Vec3::Y, DUCT * 0.55, 0.9, 1.4);
        conduit(
            b,
            &[
                v3(RING - DUCT * 0.5, 0.0, DUCT_Z),
                v3(STAGES[0].0 + 0.2, 0.0, DUCT_Z),
            ],
            1.0,
        );
    });
    let (r, z0, z1) = STAGES[0];
    vessel(b, Vec3::ZERO, r, z0, z1);
    coil(b, v3(0.0, 0.0, 6.4), r + 0.02, 0.8, 1.0);
    bank(b, v3(-17.0, 0.0, DECK), 4, 6.0);
    conduit(
        b,
        &[v3(-15.2, 2.0, 2.6), v3(-RING - DUCT * 0.5, 2.0, 2.6)],
        0.8,
    );
    hopper(b, v3(3.0, 3.0, 5.2), v3(7.6, 7.6, DECK), 2.8);

    // ---- Tech 2: coils all round, the upper stage on a gantry, flank banks, cooling.
    fitted(b, tech, 2, |b| {
        b.radial(4, |b| {
            for y in [-6.5, 6.5] {
                coil_on(b, v3(RING, y - 0.7, DUCT_Z), Vec3::Y, DUCT * 0.55, 0.9, 1.4);
            }
        });
    });
    fitted(b, tech, 2, |b| {
        let (r, z0, z1) = STAGES[1];
        vessel(b, Vec3::ZERO, r, z0, z1);
        coil(b, v3(0.0, 0.0, 14.6), r + 0.02, 0.8, 0.9);
        b.paint(ACCENT);
        annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 4.0, 5.2, 17.0, 18.0);
    });
    fitted(b, tech, 2, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                let corner = RING * std::f32::consts::SQRT_2;
                post(b, corner, GANTRY[0]);
                brace(
                    b,
                    v3(corner - 0.8, 0.0, GANTRY[0] - 0.6),
                    v3(5.0, 0.0, 17.5),
                    1.0,
                );
            });
        });
    });
    fitted(b, tech, 2, |b| {
        for s in [-1.0f32, 1.0] {
            b.yawed(Vec3::ZERO, s * FRAC_PI_2, |b| {
                bank(b, v3(-17.0, 0.0, DECK), 3, 7.0);
                conduit(
                    b,
                    &[v3(-15.2, 0.0, 2.6), v3(-RING - DUCT * 0.5, 0.0, 2.6)],
                    0.8,
                );
            });
        }
    });
    fitted(b, tech, 2, |b| {
        for y in [-4.5, 4.5] {
            radiator(b, v3(15.0, y, DECK), Vec3::X, 5.8, 2.6);
        }
    });

    // ---- Tech 3: the gantry raised, a third stage under a crown, more banks and cooling.
    fitted(b, tech, 3, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.radial(4, |b| {
                let corner = RING * std::f32::consts::SQRT_2;
                b.paint(PLATING);
                b.frustum(
                    v3(corner, 0.0, GANTRY[0]),
                    v2(2.2, 2.2),
                    v2(1.8, 1.8),
                    GANTRY[1] - GANTRY[0],
                    glam::Vec2::ZERO,
                );
                b.paint(GLOW).pattern(pattern::NONE);
                b.cuboid(v3(corner, 0.0, GANTRY[1] - 2.0), v3(2.0, 2.0, 0.45));
                brace(
                    b,
                    v3(corner - 0.6, 0.0, GANTRY[1] - 3.0),
                    v3(5.8, 0.0, 24.9),
                    0.9,
                );
                conduit(
                    b,
                    &[v3(corner, 0.0, GANTRY[1] + 0.2), v3(5.4, 0.0, 25.8)],
                    0.6,
                );
            });
        });
    });
    fitted(b, tech, 3, |b| {
        let (r, z0, z1) = STAGES[2];
        vessel(b, Vec3::ZERO, r, z0, z1);
        coil(b, v3(0.0, 0.0, 21.0), r + 0.02, 0.7, 0.9);
        b.paint(PLATING_DARK);
        annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 3.0, 6.0, 24.6, 26.4);
        b.paint(GLOW).pattern(pattern::CHARGE);
        annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 6.0, 6.08, 25.0, 26.0);
    });
    fitted(b, tech, 3, |b| {
        bank(b, v3(-20.2, 0.0, DECK), 5, 8.5);
    });
    fitted(b, tech, 3, |b| {
        for (x, y) in [(-17.5, 8.4), (-17.5, -16.4), (17.5, 8.4), (17.5, -16.4)] {
            radiator(b, v3(x, y, DECK), Vec3::Y, 8.0, 2.4);
        }
    });
}

/// One side of the ring along +x's edge: the duct from corner to corner on pedestals, a
/// dark corner housing where it turns.
fn duct(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::FLUX);
    b.beam(
        v3(RING, -RING + DUCT * 0.7, DUCT_Z),
        v3(RING, RING - DUCT * 0.7, DUCT_Z),
        v2(DUCT, DUCT),
        v2(DUCT, DUCT),
    );
    b.paint(PLATING);
    b.chamfered_box(v3(RING, RING, DUCT_Z), v3(3.6, 3.6, DUCT + 1.6), 0.8);
    if b.fine() {
        team_panel(b, v3(RING, RING, DUCT_Z + DUCT * 0.5 + 0.8), v2(1.6, 1.6));
        b.paint(ACCENT);
        for y in [-8.0, -3.2, 3.2, 8.0] {
            b.cuboid(
                v3(RING, y, (DECK + DUCT_Z - DUCT * 0.5) * 0.5),
                v3(DUCT * 0.8, 1.0, DUCT_Z - DUCT * 0.5 - DECK + 0.05),
            );
        }
    }
}

/// A gantry post on +x at `out`, from a foot on the ring's corner housing to `top`.
fn post(b: &mut MeshBuilder, out: f32, top: f32) {
    let foot = DUCT_Z + DUCT * 0.5 + 0.8;
    b.paint(PLATING);
    b.frustum(
        v3(out, 0.0, foot),
        v2(2.6, 2.6),
        v2(2.2, 2.2),
        top - foot,
        glam::Vec2::ZERO,
    );
    b.paint(ACCENT);
    b.chamfered_box(v3(out, 0.0, top - 0.5), v3(2.8, 2.8, 1.0), 0.4);
}

/// The coarse level: slab, the ring as four low blocks, the vessel as one block to the
/// tier's height, the bank.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    slab(b);
    b.paint(PLATING_DARK);
    for (c, s) in [
        (v3(RING, 0.0, DUCT_Z), v3(DUCT, RING * 2.0, DUCT)),
        (v3(-RING, 0.0, DUCT_Z), v3(DUCT, RING * 2.0, DUCT)),
    ] {
        b.cuboid_open(c, s);
    }
    let top = hatch(tech as usize - 1);
    let r = STAGES[tech as usize - 1].0;
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, DECK),
        v2(10.0, 10.0),
        v2(r * 1.3, r * 1.3),
        top - DECK,
        glam::Vec2::ZERO,
    );
    team_panel(b, v3(0.0, 0.0, top), v2(2.0, 2.0));
    bank(b, v3(-17.0, 0.0, DECK), 4, 6.0);
}
