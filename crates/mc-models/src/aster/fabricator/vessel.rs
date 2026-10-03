//! The tech 2 Material Fabricator, on a 4×4 lot: a pressure vessel of two stages in the
//! middle, field coils round each, the upper stage inside a braced containment frame; the
//! capacitor banks on the grid side (-x) and both flanks, cabled in; radiators either side
//! of the hopper on the far side (+x).

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use glam::Vec3;

use super::super::parts::*;
use super::*;

/// The vessel's stages: (radius, foot, top of the straight body). The upper stands on the
/// lower's hatch.
const STAGES: [(f32, f32, f32); 2] = [(6.0, DECK, 9.5), (4.5, 12.3, 17.0)];

/// Where a stage's hatch lid is: its shoulder plus the collar.
fn hatch(stage: usize) -> f32 {
    let (r, _, z1) = STAGES[stage];
    z1 + r * 0.35 + 0.7
}

pub(super) fn vessel_plant(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    slab(b);
    deck_marks(b, 19.6);

    // The vessel's two stages and their coils.
    let (r, z0, z1) = STAGES[0];
    vessel(b, Vec3::ZERO, r, z0, z1);
    for z in [3.6, 7.0] {
        coil(b, v3(0.0, 0.0, z), r + 0.02, 0.9, 1.0);
    }
    let (r2, z0, z1) = STAGES[1];
    vessel(b, Vec3::ZERO, r2, z0, z1);
    for z in [13.8, 15.8] {
        coil(b, v3(0.0, 0.0, z), r2 + 0.02, 0.8, 0.9);
    }
    if b.fine() {
        relief(b, v3(0.0, 0.0, hatch(1)), 1.5);
    }

    // The containment frame: a brace from each corner of the deck to a collar round the
    // upper stage's top.
    b.radial(4, |b| {
        b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(15.0, 0.0, DECK + 0.5), v3(3.0, 3.0, 1.0), 0.5);
            brace(b, v3(15.0, 0.0, DECK + 0.8), v3(5.0, 0.0, 17.6), 1.1);
        });
    });
    b.paint(ACCENT);
    annulus_on(b, Vec3::ZERO, Vec3::Z, 8, 4.9, 5.9, 17.2, 18.2);

    // The banks, on the grid side and both flanks, each cabled in.
    bank(b, v3(-15.5, 0.0, DECK), 4, 6.5);
    for y in [-2.6, 2.6] {
        conduit(
            b,
            &[v3(-13.6, y, 3.0), v3(-9.0, y, 3.0), v3(-6.6, y * 0.6, 3.0)],
            0.9,
        );
    }
    for s in [-1.0f32, 1.0] {
        b.yawed(Vec3::ZERO, s * FRAC_PI_2, |b| {
            bank(b, v3(-15.5, 0.0, DECK), 3, 7.5);
            conduit(b, &[v3(-13.6, 0.0, 3.0), v3(-6.6, 0.0, 3.0)], 0.9);
        });
    }

    // The output, and the cooling either side of it.
    hopper(b, v3(6.2, 0.0, 5.4), v3(14.5, 0.0, DECK), 3.4);
    for y in [-7.0, 7.0] {
        radiator(b, v3(9.0, y, DECK), Vec3::X, 10.5, 2.6);
    }
}

/// Pressure relief on a hatch at `top`: two short stacks with dark caps, `h` tall.
fn relief(b: &mut MeshBuilder, top: Vec3, h: f32) {
    for y in [-0.9, 0.9] {
        b.paint(METAL);
        b.prism(top + v3(0.0, y, 0.0), 6, 0.35, 0.35, h);
        b.paint(ACCENT);
        b.prism(top + v3(0.0, y, h), 6, 0.5, 0.45, 0.3);
    }
}

/// The coarse level: the slab, the vessel as one stepped block, the bank and the hopper as
/// boxes.
fn coarse(b: &mut MeshBuilder) {
    slab(b);
    deck_marks(b, 19.6);
    let top = hatch(1);
    let r = STAGES[1].0 * 1.3;
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, DECK),
        v2(13.0, 13.0),
        v2(r, r),
        top - DECK,
        glam::Vec2::ZERO,
    );
    team_panel(b, v3(0.0, 0.0, top), v2(2.4, 2.4));
    bank(b, v3(-15.5, 0.0, DECK), 4, 6.5);
    hopper(b, v3(6.2, 0.0, 5.4), v3(14.5, 0.0, DECK), 3.4);
}
