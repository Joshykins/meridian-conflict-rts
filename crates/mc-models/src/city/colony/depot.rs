//! A colony depot: its stores hall.

use glam::Vec3;

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// A heavy hall on a battered skirt, buttress pylons along its long walls, a
/// recessed band and clerestory glazing under a thick bevelled cap, two great doors
/// in deep portals with hoods.
pub(super) fn depot(b: &mut MeshBuilder, _tech: u8) {
    let core = Rect::new(-26.0, -15.0, 26.0, 15.0);
    let cut = 1.5;
    let base = 1.0;
    let band = 7.0;
    let roof = 10.4;
    if !b.mid() {
        solid(
            b,
            core.grow(0.3),
            -2.0,
            roof + 0.9,
            pat::PREFAB,
            pat::ROOF_FLAT,
        );
        return;
    }
    let blank = all(pat::PREFAB);
    if b.mid() {
        skirt(b, core, cut, 1.0, base, pat::CONCRETE);
        cut_walls(b, core, cut, 1, base, band, blank, pat::PREFAB);
        recess(b, core, cut, 1, band, band + 0.6, 0.4, pat::TRIM);
        cut_walls(
            b,
            core,
            cut,
            1,
            band + 0.6,
            roof,
            [pat::HAB_BAND, pat::HAB_BAND, pat::PREFAB, pat::PREFAB],
            pat::PREFAB,
        );
    } else {
        cut_walls(b, core, cut, 1, -2.0, roof, blank, pat::PREFAB);
    }
    paint(b, pat::PREFAB);
    let cap = core.grow(0.35);
    b.loft(
        &[
            cut_ring(core, cut, 1, roof),
            cut_ring(cap, cut + 0.1, 1, roof),
            cut_ring(cap, cut + 0.1, 1, roof + 0.4),
            cut_ring(core.grow(-0.8), cut, 1, roof + 0.9),
        ],
        false,
        false,
    );
    deck(b, core.grow(-0.8), roof + 0.9, pat::ROOF_FLAT);
    let doors = [-10.5f32, 10.5];
    // Buttresses along the long walls, clear of the doors.
    for side in [Side::Front, Side::Back] {
        let (a0, a1, _) = side.run(core);
        for c in cells(a0 + cut, a1 - a0 - 2.0 * cut, 8.6) {
            if side == Side::Front && doors.iter().any(|d| (c - d).abs() < 6.2) {
                continue;
            }
            let p = side.at(core, c, 0.0, 0.0);
            paint(b, pat::PREFAB);
            b.chamfered_box(
                v3(p.x, p.y, roof * 0.5 - 0.6),
                Vec3::new(1.4, 2.0, roof + 2.8),
                0.35,
            );
            if b.fine() {
                paint(b, pat::ACCENT);
                b.chamfered_box(v3(p.x, p.y, roof - 0.5), Vec3::new(1.5, 2.1, 0.8), 0.37);
            }
        }
    }
    if !b.mid() {
        return;
    }
    for x in doors {
        let r = core;
        for (a0, a1, z0, z1) in [
            (x - 5.5, x - 4.5, 0.0, 7.6),
            (x + 4.5, x + 5.5, 0.0, 7.6),
            (x - 5.5, x + 5.5, 6.6, 7.6),
        ] {
            let (min, max) = Side::Front.block(r, a0, a1, 0.0, 1.6, z0, z1);
            boxed(b, min, max, pat::TRIM);
        }
        paint(b, pat::STEEL);
        panel(b, r, Side::Front, x - 4.5, x + 4.5, 0.0, 6.6, 1.25);
        paint(b, pat::SHADOW);
        panel(b, r, Side::Front, x - 0.05, x + 0.05, 0.0, 6.6, 1.27);
        hood(b, r, Side::Front, x - 5.8, x + 5.8, 7.6, 2.0, pat::PREFAB);
    }
    for y in cells(core.min.y + 4.0, core.size().y - 8.0, 14.0) {
        door(b, core, Side::West, y, 1.4, base + 2.3, true, pat::TRIM);
    }
    if b.fine() {
        for x in cells(core.min.x + 4.0, core.size().x - 8.0, 9.0) {
            for y in [-6.0, 6.0] {
                paint(b, pat::STEEL);
                b.prism(v3(x, y, roof + 0.9), 8, 0.7, 0.6, 1.1);
            }
        }
        plant_with(
            b,
            Rect::new(-6.0, -8.0, 6.0, 8.0),
            roof + 0.9,
            29,
            3,
            false,
            false,
        );
    }
}
