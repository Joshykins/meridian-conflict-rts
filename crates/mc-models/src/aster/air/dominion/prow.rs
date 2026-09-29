//! The Dominion's prow: the second stacked layer runs on past the hull as a narrow
//! faceted armoured ram to the ship's foremost point, and beneath it, a clear gap between
//! them, the narrow bore block carries the spinal AEB's mouth in its face.

use super::hull::{self, BOW, DECK};
use super::*;

/// The mouth in the bore block's face: its radius and its frame.
const MOUTH: f32 = 12.0;
const MOUTH_FRAME: f32 = 2.2;
/// Where the second layer's forward run starts (just aft of its own end, so the two meet
/// inside each other).
const LAYER_JOIN: f32 = 146.0;

pub(super) fn build(b: &mut MeshBuilder) {
    beak(b);
    bore_mouth(b, MOUTH, MOUTH_FRAME);
}

/// The strategy-zoom prow: the ram and the bore block as one box.
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.extrude_z(
        &[
            [BOW, 12.0],
            [BOW, -12.0],
            [hull::HULL_FORE, -20.0],
            [hull::HULL_FORE, 20.0],
        ],
        46.0,
        104.0,
    );
}

/// The bore block's port half from its row (x, half width, top, belly): a flat belly, one
/// chamfer under the flank, a flat flank, one broad chamfer onto a flat top.
fn block_half(row: [f32; 4]) -> Vec<[f32; 2]> {
    let [_, w, top, bot] = row;
    vec![
        [0.0, bot],
        [w - 6.0, bot],
        [w, bot + 6.0],
        [w, top - 8.0],
        [w - 10.0, top],
        [0.0, top],
    ]
}

/// The bore block along `rows` (x, half width, top, belly), open round the mouth at the
/// bow; armour on its cheeks and a raked brow plate over the face.
fn bore_block(b: &mut MeshBuilder, rows: &[[f32; 4]]) {
    let half = |x: f32| block_half(lerp_rows(rows, x));
    b.paint(PLATING_DARK).pattern(pattern::GENERIC);
    b.loft(
        &rows
            .iter()
            .map(|r| full_ring(r[0], &block_half(*r)))
            .collect::<Vec<_>>(),
        true,
        false,
    );
    bow_face(b, &full_ring(BOW, &half(BOW)), MOUTH + MOUTH_FRAME * 0.5);
    let a = rows[0][0];
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::GENERIC);
        plate_on(b, &half, [a + 12.0, BOW - 1.0], 2, 4, [0.3, 0.2], 1.6);
        plate_on(b, &half, [BOW - 18.0, BOW - 0.5], 3, 5, [0.2, 0.7], 1.8);
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on(b, &half, [BOW - 26.0, BOW - 20.0], 3, 4, [0.1, 0.6], 2.0);
    });
    // A chin plate raked back under the face.
    let [_, w, _, bot] = lerp_rows(rows, BOW);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            vec![
                v3(BOW - 16.0, -w + 4.0, bot - 1.4),
                v3(BOW - 16.0, w - 4.0, bot - 1.4),
                v3(BOW - 16.0, w - 4.0, bot + 0.2),
                v3(BOW - 16.0, -w + 4.0, bot + 0.2),
            ],
            vec![
                v3(BOW - 1.0, -w + 7.0, bot + 1.0),
                v3(BOW - 1.0, w - 7.0, bot + 1.0),
                v3(BOW - 1.0, w - 7.0, bot + 2.6),
                v3(BOW - 1.0, -w + 7.0, bot + 2.6),
            ],
        ],
        true,
        true,
    );
}

/// The second layer's half width where its forward run starts.
fn layer_w() -> f32 {
    super::stacked::l2_width(LAYER_JOIN)
}

/// The ram prow: the second layer runs on as a narrow faceted armoured beak to the ship's
/// foremost point, a gap between it and the narrow bore block beneath it.
fn beak(b: &mut MeshBuilder) {
    let ring = |x: f32, w: f32, bot: f32, top: f32| {
        full_ring(
            x,
            &[
                [0.0, bot],
                [w * 0.7, bot],
                [w, (bot + top) * 0.5],
                [w * 0.55, top],
                [0.0, top],
            ],
        )
    };
    b.paint(PLATING).pattern(pattern::GENERIC);
    b.loft(
        &[
            ring(LAYER_JOIN, layer_w(), DECK - 0.5, 112.0),
            ring(172.0, 30.0, 94.0, 110.0),
            ring(202.0, 22.0, 92.0, 106.0),
            ring(228.0, 12.0, 90.0, 100.0),
            ring(BOW + 1.0, 3.0, 89.0, 94.0),
        ],
        true,
        true,
    );
    // A dark keel strip down the beak's underside, and armour on its cheeks.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            vec![
                v3(176.0, -6.0, 92.8),
                v3(176.0, 6.0, 92.8),
                v3(176.0, 6.0, 94.2),
                v3(176.0, -6.0, 94.2),
            ],
            vec![
                v3(236.0, -2.0, 89.0),
                v3(236.0, 2.0, 89.0),
                v3(236.0, 2.0, 90.4),
                v3(236.0, -2.0, 90.4),
            ],
        ],
        true,
        true,
    );
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::GENERIC);
        b.loft(
            &[
                vec![
                    v3(180.0, 20.0, 100.0),
                    v3(180.0, 27.6, 101.0),
                    v3(180.0, 25.0, 106.0),
                    v3(180.0, 19.0, 106.0),
                ],
                vec![
                    v3(222.0, 11.0, 96.0),
                    v3(222.0, 15.2, 96.5),
                    v3(222.0, 13.6, 100.0),
                    v3(222.0, 10.0, 100.0),
                ],
            ],
            true,
            true,
        );
    });
    bore_block(
        b,
        &[
            [196.0, 24.0, 84.0, 44.0],
            [228.0, 22.0, 82.0, 44.0],
            [BOW, 19.0, 80.0, 48.0],
        ],
    );
}
