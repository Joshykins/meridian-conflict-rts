//! A family hab.

use glam::Vec3;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// One heavy block on a battered concrete skirt, its two storeys parted by a dark
/// recessed band, hooded windows, a thick bevelled cap and pylons at its front corners.
pub(super) fn hab(b: &mut MeshBuilder, _tech: u8) {
    let core = Rect::new(-8.4, -5.4, 8.4, 5.4);
    let cut = 0.8;
    let base = 1.2;
    let band = 3.9;
    let roof = 6.9;
    if !b.mid() {
        solid(
            b,
            core.grow(0.3),
            -2.0,
            roof + 0.8,
            pat::PREFAB,
            pat::ROOF_FLAT,
        );
        return;
    }
    let looks = [pat::HAB, pat::HAB, pat::PREFAB, pat::PREFAB];
    if b.mid() {
        skirt(b, core, cut, 1.0, base, pat::CONCRETE);
        cut_walls(b, core, cut, 1, base, band, looks, pat::PREFAB);
        recess(b, core, cut, 1, band, band + 0.5, 0.35, pat::TRIM);
        cut_walls(b, core, cut, 1, band + 0.5, roof, looks, pat::PREFAB);
    } else {
        cut_walls(b, core, cut, 1, -2.0, roof, looks, pat::PREFAB);
    }
    // The cap: proud of the walls, then bevelled in to the roof.
    paint(b, pat::PREFAB);
    let cap = core.grow(0.3);
    b.loft(
        &[
            cut_ring(core, cut, 1, roof),
            cut_ring(cap, cut + 0.1, 1, roof),
            cut_ring(cap, cut + 0.1, 1, roof + 0.35),
            cut_ring(core.grow(-0.6), cut, 1, roof + 0.9),
        ],
        false,
        false,
    );
    deck(b, core.grow(-0.6), roof + 0.9, pat::ROOF_FLAT);
    // Pylons at the front corners.
    for x in [core.min.x, core.max.x] {
        paint(b, pat::PREFAB);
        b.chamfered_box(v3(x, core.max.y - 0.4, 3.0), Vec3::new(1.6, 1.6, 10.0), 0.4);
        if b.fine() {
            paint(b, pat::ACCENT);
            b.chamfered_box(v3(x, core.max.y - 0.4, 6.6), Vec3::new(1.7, 1.7, 0.7), 0.42);
        }
    }
    if !b.mid() {
        return;
    }
    // Hoods over every window, front and back.
    for side in [Side::Front, Side::Back] {
        let (a0, a1, _) = side.run(core);
        let (a0, a1) = (a0 + cut, a1 - cut);
        let (_, cw) = grid(a1 - a0, pat::HAB_BAY);
        for c in cells(a0, a1 - a0, pat::HAB_BAY) {
            let half = cw * pat::HAB_WIDE * 0.5 + 0.2;
            for (z0, z1) in [(base, band), (band + 0.5, roof)] {
                let head = z0 + (z1 - z0) * pat::HAB_HEAD + 0.18;
                hood(b, core, side, c - half, c + half, head, 0.55, pat::TRIM);
            }
        }
    }
    // The door in a deep portal, steps up to it.
    let at = 0.0;
    door(b, core, Side::Front, at, 1.4, base + 2.3, false, pat::TRIM);
    for (a0, a1, z0, z1) in [
        (at - 1.25, at - 0.7, base, base + 2.75),
        (at + 0.7, at + 1.25, base, base + 2.75),
        (at - 1.25, at + 1.25, base + 2.3, base + 2.75),
    ] {
        let (min, max) = Side::Front.block(core, a0, a1, 0.0, 0.7, z0, z1);
        boxed(b, min, max, pat::TRIM);
    }
    boxed(
        b,
        v3(at - 1.4, core.max.y, 0.0),
        v3(at + 1.4, core.max.y + 1.6, base),
        pat::CONCRETE,
    );
    if b.fine() {
        plant_with(b, core.grow(-1.6), roof + 0.9, 7, 2, false, false);
    }
}
