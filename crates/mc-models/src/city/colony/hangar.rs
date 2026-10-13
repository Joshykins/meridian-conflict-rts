//! A hangar for small craft, its doors the width of its front.

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// Door leaves across the front of `r` from `a0` to `a1`, `h` high, `inset` back from
/// its wall, one leaf run back to show the dark inside.
fn doors(b: &mut MeshBuilder, r: Rect, a0: f32, a1: f32, h: f32, inset: f32) {
    let leaves = 4;
    let w = (a1 - a0) / leaves as f32;
    paint(b, pat::SHADOW);
    panel(b, r, Side::Front, a0, a1, 0.0, h, -inset - 0.4);
    for k in 0..leaves {
        if k == 1 {
            continue;
        }
        let x0 = a0 + k as f32 * w;
        let (min, max) =
            Side::Front.block(r, x0 + 0.08, x0 + w - 0.08, -inset - 0.3, -inset, 0.0, h);
        boxed(b, min, max, pat::STEEL);
    }
    if b.fine() {
        // The open leaf, run back behind its neighbour.
        let x0 = a0 + 2.0 * w;
        let (min, max) = Side::Front.block(
            r,
            x0 + 0.1,
            x0 + w - 0.1,
            -inset - 0.75,
            -inset - 0.45,
            0.0,
            h,
        );
        boxed(b, min, max, pat::STEEL);
    }
}

/// A tall heavy box on a battered skirt, a clerestory band under a bevelled cap,
/// the doors deep in a portal with a massive lintel and a hood over it.
pub(super) fn hangar(b: &mut MeshBuilder, _tech: u8) {
    let r = Rect::new(-31.0, -23.0, 31.0, 23.0);
    let cut = 1.5;
    let roof = 17.0;
    if !b.mid() {
        solid(b, r, -2.0, roof + 1.2, pat::PREFAB, pat::ROOF_FLAT);
        return;
    }
    skirt(b, r, cut, 1.0, 1.0, pat::CONCRETE);
    cut_walls(b, r, cut, 1, 1.0, 12.0, all(pat::PREFAB), pat::PREFAB);
    recess(b, r, cut, 1, 12.0, 12.6, 0.4, pat::TRIM);
    cut_walls(
        b,
        r,
        cut,
        1,
        12.6,
        roof,
        [pat::PREFAB, pat::HAB_BAND, pat::HAB_BAND, pat::HAB_BAND],
        pat::PREFAB,
    );
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(r, cut, 1, roof),
            cut_ring(r.grow(0.35), cut + 0.1, 1, roof),
            cut_ring(r.grow(0.35), cut + 0.1, 1, roof + 0.4),
            cut_ring(r.grow(-0.8), cut, 1, roof + 1.2),
        ],
        false,
        false,
    );
    deck(b, r.grow(-0.8), roof + 1.2, pat::ROOF_FLAT);
    // The portal: jambs and a lintel proud of the front, the doors back in it.
    for (a0, a1, z0, z1) in [
        (-28.0, -25.5, 0.0, 14.6),
        (25.5, 28.0, 0.0, 14.6),
        (-28.0, 28.0, 11.8, 14.6),
    ] {
        let (min, max) = Side::Front.block(r, a0, a1, 0.0, 1.6, z0, z1);
        boxed(b, min, max, pat::PREFAB);
    }
    doors(b, r, -25.5, 25.5, 11.8, -1.0);
    hood(b, r, Side::Front, -28.4, 28.4, 14.6, 2.4, pat::TRIM);
    if b.fine() {
        paint(b, pat::ACCENT);
        let (min, max) = Side::Front.block(r, -28.05, 28.05, 1.6, 1.65, 12.6, 13.3);
        boxed(b, min, max, pat::ACCENT);
        plant_with(b, r.grow(-4.0), roof + 1.2, 61, 5, false, false);
        door(b, r, Side::East, -10.0, 1.4, 1.0 + 2.4, true, pat::TRIM);
    }
}
