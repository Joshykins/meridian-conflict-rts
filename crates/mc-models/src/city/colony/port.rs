//! A spaceport's terminal and its control cab.

use glam::Vec2;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// The terminal's plan and the cab's stalk's, at scale 1.
const TERMINAL: Rect = Rect {
    min: Vec2::new(-24.0, -14.0),
    max: Vec2::new(12.0, 14.0),
};

/// A heavy terminal block: battered skirt, a glazed concourse floor to the street
/// and the apron, a dark band, a punched upper floor and a bevelled cap at `top`.
fn terminal(b: &mut MeshBuilder, top: f32) {
    let r = TERMINAL.grow(-0.6);
    let cut = 1.2;
    let base = 1.0;
    let band = 5.0;
    if b.mid() {
        skirt(b, r, cut, 0.8, base, pat::CONCRETE);
    }
    let z0 = if b.mid() { base } else { -2.0 };
    cut_walls(
        b,
        r,
        cut,
        1,
        z0,
        band,
        [pat::HAB_BAND, pat::HAB_BAND, pat::PREFAB, pat::PREFAB],
        pat::PREFAB,
    );
    recess(b, r, cut, 1, band, band + 0.5, 0.35, pat::TRIM);
    cut_walls(
        b,
        r,
        cut,
        1,
        band + 0.5,
        top - 0.8,
        [pat::HAB, pat::HAB, pat::PREFAB, pat::PREFAB],
        pat::PREFAB,
    );
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(r, cut, 1, top - 0.8),
            cut_ring(r.grow(0.3), cut + 0.1, 1, top - 0.8),
            cut_ring(r.grow(0.3), cut + 0.1, 1, top - 0.5),
            cut_ring(r.grow(-0.6), cut, 1, top),
        ],
        false,
        false,
    );
    deck(b, r.grow(-0.6), top, pat::ROOF_FLAT);
    if !b.mid() {
        return;
    }
    // The entrance under a hood, steps up to it.
    let at = -10.0;
    door(b, r, Side::Front, at, 3.2, base + 2.5, false, pat::TRIM);
    hood(
        b,
        r,
        Side::Front,
        at - 2.6,
        at + 2.6,
        base + 2.7,
        1.6,
        pat::TRIM,
    );
    boxed(
        b,
        v3(at - 2.4, r.max.y, 0.0),
        v3(at + 2.4, r.max.y + 1.8, base),
        pat::CONCRETE,
    );
    if b.fine() {
        plant_with(b, r.grow(-3.0), top, 51, 4, false, false);
    }
}

/// A radome and a mast on a roof at `z`, round `at`.
fn sensors(b: &mut MeshBuilder, at: Vec2, z: f32) {
    comms_mast(b, v3(at.x - 1.8, at.y, z), 7.0);
    if b.fine() {
        let sides = b.sides(16);
        paint(b, pat::WHITE_STEEL);
        b.prism(v3(at.x + 1.6, at.y, z), sides, 1.3, 1.3, 0.8);
        b.prism(v3(at.x + 1.6, at.y, z + 0.8), sides, 1.3, 0.3, 1.0);
    }
}

/// The heavy terminal; a chamfered stalk at its east end in dark-banded runs, one
/// line of windows up its face, the cab canted out over it, a radome and a mast.
pub(super) fn port(b: &mut MeshBuilder, _tech: u8) {
    let stalk = Rect::new(14.0, -5.0, 22.0, 5.0);
    if !b.mid() {
        solid(b, TERMINAL, -2.0, 9.0, pat::HAB, pat::ROOF_FLAT);
        solid(
            b,
            Rect::new(12.0, -7.5, 24.0, 7.5),
            -2.0,
            24.0,
            pat::PREFAB,
            pat::ROOF_FLAT,
        );
        return;
    }
    terminal(b, 9.0);
    skirt(b, stalk, 1.0, 0.8, 1.0, pat::CONCRETE);
    let runs = [(1.0, 7.0), (7.5, 13.5), (14.0, 20.0)];
    for (k, &(z0, z1)) in runs.iter().enumerate() {
        cut_walls(b, stalk, 1.0, 1, z0, z1, all(pat::PREFAB), pat::PREFAB);
        if k < 2 {
            recess(b, stalk, 1.0, 1, z1, z1 + 0.5, 0.3, pat::TRIM);
        }
        paint(b, pat::HAB);
        panel(b, stalk, Side::Front, 16.8, 19.2, z0 + 0.5, z1 - 0.5, 0.03);
    }
    let lo = Rect::new(13.0, -6.5, 23.0, 6.5);
    paint(b, pat::PREFAB);
    b.loft(
        &[cut_ring(stalk, 1.0, 1, 20.0), cut_ring(lo, 1.4, 1, 20.8)],
        false,
        false,
    );
    let roof = canted_cab(b, lo, 20.8, Rect::new(12.2, -7.4, 23.8, 7.4), 24.2, 1.4);
    sensors(b, Vec2::new(18.0, -2.0), roof);
}
