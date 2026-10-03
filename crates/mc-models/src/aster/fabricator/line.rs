//! Design C (`fabricator~line`): a plant laid out the way the work flows, grid to product,
//! along +x: the capacitor banks at the -x end, an accelerator line of coils on trestles
//! driving the charge into the vessel, the hopper at the +x end.
//! - Tech 1: one bank, one line of three coils, the vessel, the hopper.
//! - Tech 2: two more lines in from banks on the flanks, an upper stage, radiators down
//!   both long sides.
//! - Tech 3: a containment portal over the vessel holding a third stage and its crown,
//!   more coils on the main line, more banks.

use std::f32::consts::FRAC_PI_2;

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::*;

/// The vessel's middle on x, and its stages: (radius, foot, top of the straight body).
const AT: f32 = 1.5;
const STAGES: [(f32, f32, f32); 3] = [(5.2, DECK, 9.5), (4.0, 12.0, 17.2), (2.9, 19.3, 23.6)];
/// The lines' height and duct size.
const LINE_Z: f32 = 4.4;
const DUCT: f32 = 1.4;
/// Where the main line starts (the bank's face) and the portal's height.
const LINE_FROM: f32 = -16.3;
const PORTAL: f32 = 26.6;

fn hatch(stage: usize) -> f32 {
    let (r, _, z1) = STAGES[stage];
    z1 + r * 0.35 + 0.7
}

fn centre() -> Vec3 {
    v3(AT, 0.0, 0.0)
}

pub(in crate::aster) fn line_plant(b: &mut MeshBuilder, tech: u8) {
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

    // ---- Tech 1: a bank, the main line, the vessel, the hopper.
    bank(b, v3(-18.0, 0.0, DECK), 4, 6.5);
    let (r, z0, z1) = STAGES[0];
    let face = AT - r * 0.92;
    line(
        b,
        v3(LINE_FROM, 0.0, LINE_Z),
        v3(face + 0.2, 0.0, LINE_Z),
        &[0.15, 0.45, 0.75],
    );
    vessel(b, centre(), r, z0, z1);
    coil(b, v3(AT, 0.0, 7.4), r + 0.02, 0.8, 1.0);
    hopper(b, v3(AT + r * 0.9, 0.0, 5.6), v3(15.5, 0.0, DECK), 3.4);

    // ---- Tech 2: flank banks and their lines, the upper stage, radiators.
    fitted(b, tech, 2, |b| {
        for y in [-11.0f32, 11.0] {
            bank_x(b, v3(-17.0, y, DECK), 3, 7.0);
            let from = v3(-12.6, y * 0.92, LINE_Z);
            let to = v3(AT - 4.2, y.signum() * 3.6, LINE_Z);
            line(b, from, to, &[0.25, 0.65]);
        }
    });
    fitted(b, tech, 2, |b| {
        let (r, z0, z1) = STAGES[1];
        vessel(b, centre(), r, z0, z1);
        coil(b, v3(AT, 0.0, 14.0), r + 0.02, 0.8, 0.9);
        coil(b, v3(AT, 0.0, 15.9), r + 0.02, 0.8, 0.9);
    });
    fitted(b, tech, 2, |b| {
        for y in [-13.5, 13.5] {
            radiator(b, v3(4.0, y, DECK), Vec3::X, 12.0, 2.6);
        }
    });

    // ---- Tech 3: the portal, a third stage under its crown, more coils and banks.
    fitted(b, tech, 3, portal);
    fitted(b, tech, 3, |b| {
        let (r, z0, z1) = STAGES[2];
        vessel(b, centre(), r, z0, z1);
        coil(b, v3(AT, 0.0, 21.0), r + 0.02, 0.7, 0.9);
        b.paint(PLATING_DARK);
        annulus_on(b, centre(), Vec3::Z, 8, 3.1, 5.6, 24.4, 25.8);
        b.paint(GLOW).pattern(pattern::CHARGE);
        annulus_on(b, centre(), Vec3::Z, 8, 5.6, 5.68, 24.8, 25.4);
    });
    fitted(b, tech, 3, |b| {
        for y in [-15.8f32, 15.8] {
            bank_x(b, v3(-17.0, y, DECK), 3, 8.5);
        }
        for x in [-12.3, -8.4] {
            coil_on(b, v3(x, 0.0, LINE_Z), Vec3::X, DUCT * 0.6, 0.8, 1.2);
        }
    });
    fitted(b, tech, 3, |b| {
        b.mirror_y(|b| {
            conduit(
                b,
                &[
                    v3(-15.0, 14.5, 9.6),
                    v3(-6.5, 9.0, 24.0),
                    v3(AT - 4.0, 2.4, 24.6),
                ],
                0.7,
            );
        });
    });
}

/// A bank whose row runs along x, centred at `at`.
fn bank_x(b: &mut MeshBuilder, at: Vec3, cans: usize, h: f32) {
    b.yawed(at, FRAC_PI_2, |b| bank(b, Vec3::ZERO, cans, h));
}

/// An accelerator line from `from` to `to`: a dark duct carrying the charge, coils round it
/// at the fractions `coils` along it, and trestles under it to the deck.
fn line(b: &mut MeshBuilder, from: Vec3, to: Vec3, coils: &[f32]) {
    let d = to - from;
    let axis = d.normalize();
    b.paint(PLATING_DARK).pattern(pattern::FLUX);
    b.beam(from, to, Vec2::splat(DUCT), Vec2::splat(DUCT));
    for &t in coils {
        coil_on(b, from + d * t, axis, DUCT * 0.6, 0.8, 1.2);
    }
    // A trestle under each coil.
    if b.mid() {
        let across = axis.cross(Vec3::Z).normalize() * 1.1;
        for &t in coils {
            let top = from + d * t - Vec3::Z * (DUCT * 0.6 + 0.8);
            let foot = v3(top.x, top.y, DECK);
            b.paint(ACCENT);
            for s in [-1.0, 1.0] {
                b.beam(foot + across * s, top, Vec2::splat(0.45), Vec2::splat(0.4));
            }
        }
    }
}

/// The containment portal over the vessel: four plated columns on the lot's diagonals
/// round it, two heavy girders across them along y and the crown's ties.
fn portal(b: &mut MeshBuilder) {
    for (x, y) in [(-6.5f32, -9.5f32), (-6.5, 9.5), (9.5, -9.5), (9.5, 9.5)] {
        b.paint(PLATING_DARK);
        b.chamfered_box(v3(x, y, DECK + 0.6), v3(3.2, 3.2, 1.2), 0.5);
        b.paint(PLATING);
        b.frustum(
            v3(x, y, DECK + 1.2),
            v2(2.4, 2.4),
            v2(2.0, 2.0),
            PORTAL - DECK - 2.6,
            Vec2::ZERO,
        );
        b.paint(GLOW).pattern(pattern::NONE);
        b.cuboid(v3(x, y, PORTAL - 4.0), v3(2.08, 2.08, 0.45));
    }
    for x in [-6.5, 9.5] {
        b.paint(PLATING);
        b.chamfered_box(v3(x, 0.0, PORTAL - 0.7), v3(2.6, 22.0, 1.4), 0.4);
        team_panel(b, v3(x, 0.0, PORTAL), v2(1.2, 6.0));
    }
    // Ties from the girders in to the crown.
    for x in [-6.5f32, 9.5] {
        brace(
            b,
            v3(x, 0.0, PORTAL - 1.2),
            v3(AT + (x - AT).signum() * 5.0, 0.0, 25.2),
            0.9,
        );
    }
}

/// The coarse level: slab, the bank, the main line as a bar, the vessel as one block to
/// the tier's height, the hopper; the portal's girders at tech 3.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    slab(b);
    bank(b, v3(-18.0, 0.0, DECK), 4, 6.5);
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(-9.0, 0.0, LINE_Z), v3(13.0, DUCT, DUCT));
    let top = hatch(tech as usize - 1);
    let r = STAGES[tech as usize - 1].0;
    b.paint(PLATING);
    b.frustum_open(
        v3(AT, 0.0, DECK),
        v2(11.0, 11.0),
        v2(r * 1.3, r * 1.3),
        top - DECK,
        Vec2::ZERO,
    );
    team_panel(b, v3(AT, 0.0, top), v2(2.0, 2.0));
    if tech >= 3 {
        b.cuboid_open(v3(AT + 1.5, 0.0, PORTAL - 0.7), v3(16.0, 3.0, 1.4));
    } else {
        hopper(b, v3(AT + 5.0, 0.0, 5.6), v3(15.5, 0.0, DECK), 3.4);
    }
}
