//! The Dominion's chisel bow: a broad flat nose with clipped corners, swept
//! cheek armour and a wide, low upper deck plate. The detailed and coarse hulls
//! share the same flat-ended outline.

use super::hull::{FORE_W, HULL_FORE};
use super::*;

/// The prow's foremost point, the ship's.
pub(super) const BOW: f32 = 324.0;

/// x, half width, crown, belly. The root matches the forward hull's section.
const PROW: [[f32; 4]; 4] = [
    [HULL_FORE, FORE_W, 96.0, 62.0],
    [248.0, 64.0, 95.0, 62.0],
    [294.0, 45.0, 92.0, 65.0],
    [BOW, 26.0, 85.0, 69.0],
];
fn broad_half<const N: usize>(rows: &[[f32; 4]; N], x: f32) -> Vec<[f32; 2]> {
    let [_, w, top, bot] = lerp_rows(rows, x);
    let h = top - bot;
    vec![
        [0.0, bot],
        [w * 20.0 / 68.0, bot],
        [w * 56.0 / 68.0, bot + h * 6.0 / 34.0],
        [w, bot + h * 10.0 / 34.0],
        [w * 67.0 / 68.0, bot + h * 26.0 / 34.0],
        [w * 62.0 / 68.0, top - h * 2.0 / 34.0],
        [w * 58.0 / 68.0, top],
        [0.0, top],
    ]
}

fn broad<const N: usize>(b: &mut MeshBuilder, rows: &[[f32; 4]; N], splits: &[f32]) {
    let at = |x| broad_half(rows, x);
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    let xs: Vec<_> = rows.iter().map(|r| r[0]).collect();
    hull_loft(b, &xs, &at);
    b.mirror_y(|b| {
        for ends in splits.windows(2) {
            let x = [ends[0] + 2.0, ends[1] - 2.0];
            b.paint(PLATING).pattern(pattern::WARSHIP);
            plate_on(b, &at, x, 3, 6, [0.12, 0.85], 2.0);
            b.paint(ACCENT).pattern(pattern::PLAIN);
            plate_on(b, &at, x, 2, 3, [0.15, 0.85], 0.8);
        }
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on(b, &at, [214.0, 222.0], 3, 5, [0.2, 0.8], 2.15);
    });
}

pub(super) fn build(b: &mut MeshBuilder) {
    broad(b, &PROW, &[HULL_FORE, 248.0, 294.0, BOW]);
    // A wide flush deck panel follows the clipped bow, ending behind its flat nose.
    let deck = |x: f32, w: f32| {
        let [_, _, top, _] = lerp_rows(&PROW, x);
        full_ring(
            x,
            &[
                [0.0, top - 0.3],
                [w, top - 0.3],
                [w - 2.0, top + 1.6],
                [0.0, top + 1.6],
            ],
        )
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(
        &[deck(226.0, 28.0), deck(284.0, 22.0), deck(314.0, 14.0)],
        true,
        true,
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(BOW - 0.7, -19.0, 77.0), v3(BOW + 0.5, 19.0, 80.0));
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(GLOW_ORANGE);
            b.cuboid(v3(BOW + 0.6, 15.0, 78.5), v3(0.4, 2.4, 0.8));
        });
    }
}

/// The same flat nose and sloping cheeks at strategy zoom.
pub(super) fn coarse(b: &mut MeshBuilder) {
    let ring = |row: [f32; 4]| {
        let [x, w, top, bot] = row;
        vec![v3(x, -w, bot), v3(x, w, bot), v3(x, w, top), v3(x, -w, top)]
    };
    b.loft(&[ring(PROW[0]), ring(PROW[3])], true, true);
}
