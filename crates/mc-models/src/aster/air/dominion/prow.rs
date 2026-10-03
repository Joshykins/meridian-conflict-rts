//! Dominion bow design round: sharp reference, broad Citadel, split bow and chisel.
//! Each option shares the forward hull root, weapon clearance and spacecraft rig.

use super::hull::{FORE_W, HULL_FORE};
use super::*;

/// The prow's foremost point, the ship's.
pub(super) const BOW: f32 = 324.0;

/// x, half width, crown, belly. The root matches the forward hull's section.
const PROW: [[f32; 4]; 4] = [
    [HULL_FORE, FORE_W, 96.0, 62.0],
    [228.0, 53.0, 94.0, 64.0],
    [278.0, 28.0, 87.0, 68.0],
    [BOW, 3.5, 79.0, 73.0],
];

/// The hull's eight-corner half section, scaled into a progressively finer wedge.
fn half(x: f32) -> Vec<[f32; 2]> {
    let [_, w, top, bot] = lerp_rows(&PROW, x);
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

fn sharp(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    let xs: Vec<_> = PROW.iter().map(|r| r[0]).collect();
    hull_loft(b, &xs, &half);

    // Thick cheeks follow the taper, with dark joints between armour courses.
    // These masses remain at medium LOD, where the bow's facets still need to read.
    b.mirror_y(|b| {
        for (x0, x1) in [(200.0, 225.0), (231.0, 275.0), (281.0, 319.0)] {
            b.paint(PLATING).pattern(pattern::WARSHIP);
            plate_on(b, &half, [x0, x1], 3, 6, [0.2, 0.9], 2.0);
            b.paint(ACCENT).pattern(pattern::PLAIN);
            plate_on(b, &half, [x0, x1], 2, 3, [0.15, 0.85], 0.8);
        }
    });

    // A solid centre ridge joins the cheeks into a spear rather than another terrace.
    let ridge = |x: f32, w: f32, rise: f32| {
        let [_, _, top, _] = lerp_rows(&PROW, x);
        full_ring(
            x,
            &[
                [0.0, top - 0.4],
                [w, top - 0.4],
                [w * 0.7, top + rise * 0.6],
                [w * 0.25, top + rise],
                [0.0, top + rise],
            ],
        )
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(
        &[
            ridge(HULL_FORE, 8.0, 4.0),
            ridge(228.0, 6.5, 4.0),
            ridge(278.0, 4.0, 3.0),
            ridge(BOW - 2.0, 1.2, 0.5),
        ],
        true,
        true,
    );

    if b.mid() {
        b.mirror_y(|b| {
            let x = 236.0;
            let h = half(x);
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.cuboid(v3(x, h[5][0], h[5][1]), v3(4.0, 1.4, 2.0));
            b.paint(GLOW_ORANGE);
            b.cuboid(v3(x, h[5][0] + 0.8, h[5][1]), v3(2.4, 0.5, 0.8));
            b.paint(TEAM).pattern(pattern::PLAIN);
            plate_on(b, &half, [211.0, 217.0], 3, 5, [0.25, 0.75], 2.15);
        });
    }
}

/// Keep the narrow nose and the raked top and belly at strategy zoom, at the same
/// twelve-triangle cost as the previous coarse prow.
fn coarse_sharp(b: &mut MeshBuilder) {
    let ring = |row: [f32; 4]| {
        let [x, w, top, bot] = row;
        vec![v3(x, -w, bot), v3(x, w, bot), v3(x, w, top), v3(x, -w, top)]
    };
    b.loft(&[ring(PROW[0]), ring(PROW[3])], true, true);
}

/// Bow candidates for this design round; the rest of the Dominion is shared.
#[derive(Clone, Copy)]
pub(super) enum Bow {
    Sharp,
    Citadel,
    Fork,
    Chisel,
}

const CITADEL: [[f32; 4]; 4] = [
    [HULL_FORE, FORE_W, 96.0, 62.0],
    [274.0, 68.0, 100.0, 62.0],
    [302.0, 60.0, 94.0, 65.0],
    [BOW, 48.0, 86.0, 70.0],
];
const CHISEL: [[f32; 4]; 4] = [
    [HULL_FORE, FORE_W, 96.0, 62.0],
    [248.0, 64.0, 95.0, 62.0],
    [294.0, 45.0, 92.0, 65.0],
    [BOW, 26.0, 85.0, 69.0],
];
const FORK_ROOT: [[f32; 4]; 3] = [
    [HULL_FORE, FORE_W, 96.0, 62.0],
    [244.0, 60.0, 95.0, 64.0],
    [272.0, 52.0, 90.0, 67.0],
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

fn citadel(b: &mut MeshBuilder) {
    broad(b, &CITADEL, &[HULL_FORE, 248.0, 274.0, 302.0, BOW]);
    // A broad front belt and recessed centre panel make this a heavy blunt bow.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(BOW - 1.0, -42.0, 78.0), v3(BOW + 0.7, 42.0, 81.0));
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.block(v3(BOW - 2.0, -39.0, 83.0), v3(BOW + 0.4, 39.0, 86.4));
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.block(v3(BOW + 0.4, -9.0, 72.0), v3(BOW + 0.8, 9.0, 84.0));
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(GLOW_ORANGE);
            b.cuboid(v3(BOW + 0.9, 34.0, 80.0), v3(0.4, 4.0, 0.8));
        });
    }
}

fn chisel(b: &mut MeshBuilder) {
    broad(b, &CHISEL, &[HULL_FORE, 248.0, 294.0, BOW]);
    // A wide flush deck panel replaces the spear's ridge. Its nose stays flat.
    let deck = |x: f32, w: f32| {
        let [_, _, top, _] = lerp_rows(&CHISEL, x);
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

fn fork(b: &mut MeshBuilder) {
    broad(b, &FORK_ROOT, &[HULL_FORE, 244.0, 272.0]);
    let arm = |x: f32, inner: f32, outer: f32, top: f32, bot: f32| {
        vec![
            v3(x, inner, bot),
            v3(x, outer - 4.0, bot),
            v3(x, outer, bot + 4.0),
            v3(x, outer, top - 4.0),
            v3(x, outer - 4.0, top),
            v3(x, inner, top),
        ]
    };
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::WARSHIP);
        b.loft(
            &[
                arm(254.0, 17.0, 56.0, 94.0, 65.0),
                arm(300.0, 24.0, 51.0, 91.0, 67.0),
                arm(BOW, 24.0, 45.0, 84.0, 70.0),
            ],
            true,
            true,
        );
        b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
        b.block(v3(272.0, 24.0, 72.0), v3(313.0, 26.0, 86.0));
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(BOW - 1.0, 27.0, 75.5), v3(BOW + 0.6, 40.0, 78.5));
        if b.mid() {
            b.paint(GLOW_ORANGE);
            b.cuboid(v3(BOW + 0.8, 34.0, 80.0), v3(0.4, 4.0, 0.8));
        }
    });
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(272.0, -23.0, 73.0), v3(273.0, 23.0, 86.0));
    if b.mid() {
        // Recessed service slots at the back of the notch, behind its armoured jaws.
        b.paint(METAL).pattern(pattern::PLAIN);
        for y in [-15.0, -5.0, 5.0, 15.0] {
            b.block(v3(273.0, y - 1.0, 75.0), v3(273.4, y + 1.0, 83.0));
        }
    }
}

pub(super) fn build(b: &mut MeshBuilder, bow: Bow) {
    match bow {
        Bow::Sharp => sharp(b),
        Bow::Citadel => citadel(b),
        Bow::Fork => fork(b),
        Bow::Chisel => chisel(b),
    }
}

pub(super) fn coarse(b: &mut MeshBuilder, bow: Bow) {
    let ring = |row: [f32; 4]| {
        let [x, w, top, bot] = row;
        vec![v3(x, -w, bot), v3(x, w, bot), v3(x, w, top), v3(x, -w, top)]
    };
    match bow {
        Bow::Sharp => coarse_sharp(b),
        Bow::Citadel => b.loft(&[ring(CITADEL[0]), ring(CITADEL[3])], true, true),
        Bow::Chisel => b.loft(&[ring(CHISEL[0]), ring(CHISEL[3])], true, true),
        Bow::Fork => b.mirror_y(|b| {
            b.loft(
                &[
                    vec![
                        v3(HULL_FORE, 0.0, 62.0),
                        v3(HULL_FORE, FORE_W, 72.0),
                        v3(HULL_FORE, 0.0, 96.0),
                    ],
                    vec![
                        v3(BOW, 24.0, 70.0),
                        v3(BOW, 45.0, 76.0),
                        v3(BOW, 24.0, 84.0),
                    ],
                ],
                true,
                true,
            );
        }),
    }
}
