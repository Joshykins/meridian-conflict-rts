//! The Dominion's prow, run on from the forward block's face at `hull::HULL_FORE` and no
//! taller than the hull: three tiers stepping back as they rise, the lowest the ram and
//! the foremost, each face raked and lipped in dark armour. The keel blade rakes up under
//! it, so its foot is cut back and it overhangs.

use super::hull::{DECK, FORE_W, HULL_FORE};
use super::*;

/// The prow's foremost point, the ship's.
pub(super) const BOW: f32 = 324.0;

/// A tier's port half from its row: square shoulders, small chamfers.
fn boxed(row: [f32; 4]) -> Vec<[f32; 2]> {
    let [_, w, top, bot] = row;
    vec![
        [0.0, bot],
        [w - 4.0, bot],
        [w, bot + 4.0],
        [w, top - 3.0],
        [w - 3.0, top],
        [0.0, top],
    ]
}

pub(super) fn build(b: &mut MeshBuilder) {
    terrace(b);
    if b.mid() {
        // Working lamps either side of the top tier's face, and the team's mark on it.
        let rows = &TIERS[2];
        let [x, w, top, _] = lerp_rows(rows, rows[2][0] - 6.0);
        b.paint(GLOW_ORANGE);
        b.mirror_y(|b| b.cuboid(v3(x, w - 1.0, top - 3.0), v3(2.4, 0.6, 1.0)));
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.block(v3(x - 24.0, -4.0, top - 0.2), v3(x - 18.0, 4.0, top + 0.6));
    }
}

/// The strategy-zoom prow: one tapered block.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.extrude_z(
        &[
            [BOW, 16.0],
            [BOW, -16.0],
            [HULL_FORE, -FORE_W],
            [HULL_FORE, FORE_W],
        ],
        56.0,
        DECK,
    );
}

/// The terrace's tiers, lowest first: x, half width, top, belly. Each runs further
/// forward than the one over it.
const TIERS: [[[f32; 4]; 3]; 3] = [
    [
        [HULL_FORE, FORE_W, 80.0, 62.0],
        [262.0, 58.0, 80.0, 64.0],
        [BOW, 24.0, 77.0, 70.0],
    ],
    [
        [HULL_FORE, FORE_W - 4.0, 90.0, 79.0],
        [246.0, 56.0, 90.0, 79.0],
        [294.0, 34.0, 88.0, 79.0],
    ],
    [
        [HULL_FORE, FORE_W - 9.0, DECK, 89.0],
        [226.0, 54.0, DECK, 89.0],
        [262.0, 40.0, 94.0, 89.0],
    ],
];

/// Three tiers stepping back as they rise, a dark lip along each tier's face.
fn terrace(b: &mut MeshBuilder) {
    for (k, rows) in TIERS.iter().enumerate() {
        let end = rows[2][0];
        b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
        b.loft(
            &rows
                .iter()
                .map(|r| full_ring(r[0], &boxed(*r)))
                .collect::<Vec<_>>(),
            true,
            true,
        );
        let at = |x: f32| boxed(lerp_rows(rows, x));
        b.mirror_y(|b| {
            b.paint(if k == 1 { PLATING } else { PLATING_DARK })
                .pattern(pattern::WARSHIP);
            plate_on(b, &at, [HULL_FORE + 4.0, end - 6.0], 2, 4, [0.2, 0.5], 1.4);
            b.paint(ACCENT).pattern(pattern::PLAIN);
            plate_on(b, &at, [HULL_FORE + 2.0, end - 2.0], 0, 2, [0.6, 0.6], 0.5);
        });
        let [x, w, top, _] = rows[2];
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(x - 2.0, -w + 3.0, top - 1.2),
            v3(x + 0.6, w - 3.0, top + 0.4),
        );
    }
}
