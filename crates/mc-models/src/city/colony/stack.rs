//! Stacked habs: a colony's flats.

use super::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// Four storeys of rounded modules terraced back from the street, a thick slab at
/// each floor carrying the terrace above it, strip windows to the street; a round stair
/// core at the west end.
pub(super) fn stack(b: &mut MeshBuilder, _tech: u8) {
    let floors = [
        Rect::new(-20.5, -8.0, 20.5, 7.0),
        Rect::new(-20.5, -8.0, 20.5, 5.0),
        Rect::new(-17.0, -8.0, 18.0, 3.0),
        Rect::new(-12.0, -8.0, 13.0, 1.0),
    ];
    let base = 0.5;
    let wall = 2.9;
    let slab = 0.4;
    let core = Rect::centred(-19.6, -5.4, 2.4, 2.4);
    let top = base + 4.0 * (wall + slab);
    if !b.mid() {
        for (k, r) in floors.iter().enumerate() {
            let z = base + k as f32 * (wall + slab);
            solid(
                b,
                *r,
                if k == 0 { -2.0 } else { z },
                z + wall + slab,
                pat::PREFAB,
                pat::ROOF_FLAT,
            );
        }
        return;
    }
    plinth(b, floors[0], base, pat::CONCRETE);
    for (k, r) in floors.iter().enumerate() {
        let z = base + k as f32 * (wall + slab);
        let z0 = if k == 0 && !b.mid() { -2.0 } else { z };
        let ends = if k == 3 { pat::ACCENT } else { pat::PREFAB };
        cut_walls(
            b,
            *r,
            2.4,
            3,
            z0,
            z + wall,
            [pat::HAB_BAND, pat::HAB, ends, ends],
            pat::PREFAB,
        );
        cut_slab(
            b,
            r.grow(0.6),
            3.0,
            3,
            z + wall,
            z + wall + slab,
            pat::PREFAB,
        );
        // The terrace's rail along the slab's street edge, under the floor set back
        // above it.
        if k < 3 {
            let y = r.max.y + 0.45;
            rail(
                b,
                v3(r.min.x + 2.4, y, z + wall + slab),
                v3(r.max.x - 2.4, y, z + wall + slab),
            );
        }
    }
    // The stair core, round, a little taller than the roof.
    cut_walls(
        b,
        core,
        2.39,
        5,
        if b.mid() { 0.0 } else { -2.0 },
        top + 1.2,
        all(pat::PREFAB),
        pat::PREFAB,
    );
    cut_slab(
        b,
        core.grow(0.3),
        2.69,
        5,
        top + 1.2,
        top + 1.6,
        pat::ACCENT,
    );
    if !b.mid() {
        return;
    }
    for x in [-8.0, 8.0] {
        door(
            b,
            floors[0],
            Side::Front,
            x,
            1.8,
            base + 2.3,
            true,
            pat::TRIM,
        );
    }
    let roof = floors[3].grow(-0.6);
    solar(
        b,
        Rect::new(
            roof.min.x + 1.0,
            roof.min.y,
            roof.centre().x - 1.0,
            roof.max.y,
        ),
        top,
    );
    if b.fine() {
        plant_with(
            b,
            Rect::new(roof.centre().x + 1.0, roof.min.y, roof.max.x, roof.max.y),
            top,
            11,
            2,
            false,
            false,
        );
        tank(b, v3(6.0, -6.4, top + 1.0), 5.0, 0.7, pat::WHITE_STEEL);
    }
}
