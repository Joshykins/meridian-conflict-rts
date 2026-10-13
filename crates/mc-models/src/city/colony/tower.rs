//! The colony's operations tower over its low annex on the street.

use glam::Vec2;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// The annex on the street, under every tower.
const ANNEX: Rect = Rect {
    min: Vec2::new(-13.0, 7.0),
    max: Vec2::new(13.0, 17.0),
};

/// The tower's annex: a heavy single storey on a battered skirt, a bevelled cap, the
/// door in a deep portal under a hood.
fn annex(b: &mut MeshBuilder) {
    let annex = ANNEX;
    let acut = 1.0;
    if b.mid() {
        skirt(b, annex, acut, 1.0, 1.0, pat::CONCRETE);
    }
    cut_walls(
        b,
        annex,
        acut,
        1,
        if b.mid() { 1.0 } else { -2.0 },
        5.6,
        [pat::HAB, pat::HAB, pat::PREFAB, pat::PREFAB],
        pat::PREFAB,
    );
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(annex, acut, 1, 5.6),
            cut_ring(annex.grow(0.3), acut + 0.1, 1, 5.6),
            cut_ring(annex.grow(0.3), acut + 0.1, 1, 5.9),
            cut_ring(annex.grow(-0.6), acut, 1, 6.4),
        ],
        false,
        false,
    );
    deck(b, annex.grow(-0.6), 6.4, pat::ROOF_FLAT);
    if !b.mid() {
        return;
    }
    door(b, annex, Side::Front, 0.0, 2.2, 1.0 + 2.4, false, pat::TRIM);
    for (a0, a1, z0, z1) in [
        (-2.0, -1.1, 1.0, 4.0),
        (1.1, 2.0, 1.0, 4.0),
        (-2.0, 2.0, 3.4, 4.0),
    ] {
        let (min, max) = Side::Front.block(annex, a0, a1, 0.0, 0.9, z0, z1);
        boxed(b, min, max, pat::TRIM);
    }
    hood(b, annex, Side::Front, -2.4, 2.4, 4.0, 1.4, pat::TRIM);
    boxed(
        b,
        v3(-2.2, annex.max.y, 0.0),
        v3(2.2, annex.max.y + 2.0, 1.0),
        pat::CONCRETE,
    );
    if b.fine() {
        plant_with(b, annex.grow(-2.0), 6.4, 43, 3, false, false);
    }
}

/// Three chamfered blocks stepped back as they rise, each blank but for one row of
/// windows under a dark visor at its head, sloped shoulders between them; a service
/// spine up the back; a glazed cab wider than the block it sits on under a deep brim,
/// masts and a dish on its roof, a big dish on the lowest terrace; the heavy annex.
pub(super) fn tower(b: &mut MeshBuilder, _tech: u8) {
    let cut = 1.2;
    let base = 1.5;
    let blocks = [
        (Rect::new(-8.0, -12.0, 8.0, 4.0), base, 12.5),
        (Rect::new(-6.6, -10.6, 6.6, 2.6), 13.1, 23.6),
        (Rect::new(-5.2, -9.2, 5.2, 1.2), 24.2, 31.5),
    ];
    let cab = Rect::new(-6.4, -10.4, 6.4, 2.4);
    let (c0, c1) = (32.1, 35.5);
    let roof = c1 + 1.7;
    let spine = Rect::new(-2.2, -14.6, 2.2, -11.0);
    if !b.mid() {
        for (r, z0, z1) in blocks {
            solid(
                b,
                r,
                if z0 < 2.0 { -2.0 } else { z0 },
                z1 + 0.6,
                pat::PREFAB,
                pat::ROOF_FLAT,
            );
        }
        solid(b, cab.grow(0.7), c0, roof, pat::HAB_BAND, pat::ROOF_FLAT);
        solid(b, ANNEX, -2.0, 6.4, pat::PREFAB, pat::ROOF_FLAT);
        return;
    }
    if b.mid() {
        skirt(b, blocks[0].0, cut, 1.2, base, pat::CONCRETE);
    }
    for (k, &(r, z0, z1)) in blocks.iter().enumerate() {
        let z0 = if k == 0 && !b.mid() { -2.0 } else { z0 };
        cut_walls(b, r, cut, 1, z0, z1, all(pat::PREFAB), pat::PREFAB);
        // The shoulder up to the next block, or the flare out to the cab.
        let next = if k < 2 { blocks[k + 1].0 } else { cab };
        paint(b, pat::PREFAB);
        if k < 2 {
            b.loft(
                &[
                    cut_ring(r, cut, 1, z1),
                    cut_ring(r.grow(0.3), cut + 0.1, 1, z1),
                    cut_ring(r.grow(0.3), cut + 0.1, 1, z1 + 0.25),
                    cut_ring(next.grow(0.5), cut, 1, z1 + 0.6),
                ],
                false,
                false,
            );
            deck(b, next.grow(0.5), z1 + 0.6, pat::ROOF_FLAT);
        } else {
            b.loft(
                &[cut_ring(r, cut, 1, z1), cut_ring(next, cut, 1, c0)],
                false,
                false,
            );
        }
        if !b.mid() {
            continue;
        }
        // One row of windows at the block's head, a visor over it.
        for side in Side::ALL {
            let (a0, a1, _) = side.run(r);
            let (a0, a1) = (a0 + cut + 1.0, a1 - cut - 1.0);
            paint(b, pat::HAB);
            panel(b, r, side, a0, a1, z1 - 3.4, z1 - 0.6, 0.03);
            hood(b, r, side, a0 - 0.3, a1 + 0.3, z1 - 0.55, 0.9, pat::TRIM);
        }
    }
    // The cab under its brim.
    cut_walls(b, cab, cut + 0.4, 1, c0, c1, all(pat::HAB_BAND), pat::TRIM);
    paint(b, pat::PREFAB);
    b.loft(
        &[
            cut_ring(cab, cut + 0.4, 1, c1),
            cut_ring(cab.grow(1.4), cut + 0.9, 1, c1 + 0.4),
            cut_ring(cab.grow(1.4), cut + 0.9, 1, c1 + 1.1),
            cut_ring(cab.grow(-0.4), cut, 1, roof),
        ],
        false,
        false,
    );
    deck(b, cab.grow(-0.4), roof, pat::ROOF_FLAT);
    // The spine up the back.
    cut_walls(
        b,
        spine,
        0.6,
        1,
        if b.mid() { 0.0 } else { -2.0 },
        roof + 1.0,
        all(pat::PREFAB),
        pat::PREFAB,
    );
    cut_slab(
        b,
        spine.grow(0.2),
        0.7,
        1,
        roof + 1.0,
        roof + 1.5,
        pat::ACCENT,
    );
    comms_mast(b, v3(-3.0, -7.0, roof), 9.0);
    if !b.fine() {
        annex(b);
        return;
    }
    paint(b, pat::TRIM);
    panel(b, spine, Side::Back, -0.45, 0.45, 1.0, roof, 0.03);
    mast(b, v3(3.6, -1.0, roof), 5.0, 0.1);
    mast(b, v3(3.6, -8.0, roof), 3.5, 0.1);
    // Dishes: a small one on the cab, a big one on the lowest terrace.
    for (at, r) in [(v3(1.5, 0.5, roof), 1.1), (v3(6.0, -9.0, 13.1), 2.6)] {
        paint(b, pat::WHITE_STEEL);
        let sides = b.sides(14);
        b.prism(at, sides, r * 0.22, r * 0.18, r * 0.8);
        b.cylinder_between(
            at + v3(0.0, 0.0, r * 0.9),
            at + v3(r * 0.35, r * 0.25, r * 1.2),
            r,
            r * 0.15,
            sides,
        );
    }
    annex(b);
}
