//! The Dominion's flanks, laid on the hull's walls (chine to shoulder): heavy armoured
//! cheeks stood well out of the aft and forward blocks, smaller sponsons between them in
//! a broken rhythm, a heavy armoured section over the engines, the waist's lit hangar
//! recess, the lower chine varied under the overhang, and the ship's name and the ARC's
//! painted on the walls.

use super::hull::{self, width, CHINE_Z, WAIST, WALL_TOP};
use super::lettering;
use super::*;

/// The names painted on the walls: the ARC's on the aft block, the ship's on the forward
/// block (x of the middle of each, letter height).
const ARC_NAME: (f32, f32) = (-62.0, 11.0);
const SHIP_NAME: (f32, f32) = (114.0, 11.0);

pub(super) fn build(b: &mut MeshBuilder) {
    let half = hull::hull_half;
    b.mirror_y(|b| {
        lower_chine(b, &half);
        engine_armour(b, &half);
        sponsons(b, &half);
        hangar(b);
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on(b, &half, [178.0, 186.0], 4, 6, [0.2, 0.5], 2.4);
    });
    names(b);
}

/// The names, painted in the hull's light plating on the walls.
fn names(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    b.paint(PLATING).pattern(pattern::PLAIN);
    for (text, (x, height)) in [("ARC", ARC_NAME), ("DOMINION", SHIP_NAME)] {
        let foot = (CHINE_Z + WALL_TOP - height) * 0.5;
        lettering::paint_flanks(b, text, x, hull::wall(x), foot, height);
    }
}

/// The waist's hangar recess (port; mirrored): a dark bay let into the drawn-in wall,
/// framed by armoured ribs from chine to shoulder, a row of lit ports along its head.
fn hangar(b: &mut MeshBuilder) {
    let [a, f] = WAIST;
    let w = width(a + 1.0);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(a + 3.0, w - 0.8, CHINE_Z + 2.0),
        v3(f - 3.0, w + 0.1, WALL_TOP - 3.0),
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    let ribs = 5;
    for k in 0..=ribs {
        let x = a + 2.0 + (f - a - 4.0) * k as f32 / ribs as f32;
        b.loft(
            &[x - 1.6, x + 1.6].map(|x| {
                vec![
                    v3(x, w - 1.0, CHINE_Z - 1.0),
                    v3(x, w + 2.4, CHINE_Z),
                    v3(x, w + 1.4, WALL_TOP + 1.0),
                    v3(x, w - 2.0, WALL_TOP + 2.0),
                ]
            }),
            true,
            true,
        );
    }
    if b.mid() {
        b.paint(GLOW_LAMP);
        let mut x = a + 6.0;
        while x < f - 5.0 {
            b.cuboid(v3(x, w + 0.15, WALL_TOP - 5.0), v3(3.0, 0.3, 1.4));
            x += 5.5;
        }
    }
}

type Half<'a> = &'a dyn Fn(f32) -> Vec<[f32; 2]>;

/// The heavy armoured section over the engines (port; mirrored): two thick courses of
/// armour down the wall astern, the upper standing proud of the lower, bolted.
fn engine_armour(b: &mut MeshBuilder, half: Half) {
    b.paint(PLATING).pattern(pattern::GENERIC);
    plate_on(b, half, [STERN + 2.0, -150.0], 3, 5, [0.0, 0.5], 3.4);
    b.paint(PLATING_DARK).pattern(pattern::GENERIC);
    plate_on(b, half, [STERN + 6.0, -156.0], 4, 6, [0.0, 0.9], 2.2);
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        let mut x = -218.0;
        while x < -154.0 {
            let w = width(x) + 3.4;
            for z in [76.0, 84.0] {
                b.cuboid(v3(x, w, z), v3(1.2, 0.8, 1.2));
            }
            x += 8.0;
        }
    }
}

/// The lower chine under the overhang (port; mirrored): slabs of different lengths and
/// depths down the underside, a dark strake where they stop.
fn lower_chine(b: &mut MeshBuilder, half: Half) {
    let runs: &[(f32, f32, f32)] = &[
        (-214.0, -176.0, 1.8),
        (-172.0, -100.0, 0.9),
        (-44.0, -32.0, 2.2),
        (-26.0, 32.0, 1.4),
        (86.0, 142.0, 1.0),
    ];
    for (k, &(x0, x1, t)) in runs.iter().enumerate() {
        if !b.fine() && k % 2 == 1 {
            continue;
        }
        b.paint(if k % 2 == 0 { PLATING } else { PLATING_DARK })
            .pattern(pattern::GENERIC);
        plate_on(b, half, [x0, x1], 1, 3, [0.3, 0.7], t);
    }
}

/// Sponsons (port; mirrored): armoured sponsons of different sizes and shapes stood out
/// of the wall in a broken rhythm, some carrying vents, blisters or sensor heads.
fn sponsons(b: &mut MeshBuilder, half: Half) {
    let fine = b.fine();
    // (from, to, how far out, foot, head, rake of the ends): the two heavy cheeks on the
    // aft block, one on the forward block, smaller ones between; none over the names.
    let list: &[(f32, f32, f32, f32, f32, f32)] = &[
        (-214.0, -162.0, 14.0, 70.0, 90.0, 10.0),
        (-152.0, -104.0, 11.0, 70.0, 93.0, 14.0),
        (-40.0, -31.0, 4.0, 76.0, 86.0, 3.0),
        (38.0, 52.0, 5.0, 74.0, 86.0, 4.0),
        (152.0, 194.0, 12.0, 70.0, 93.0, 14.0),
    ];
    for (k, &(x0, x1, out, foot, head, rake)) in list.iter().enumerate() {
        let ring = |x: f32, reach: f32| {
            let w = width(x) - 0.5;
            vec![
                v3(x, w, foot),
                v3(x, w + reach - 2.0, foot),
                v3(x, w + reach, foot + 2.5),
                v3(x, w + reach, head - 2.0),
                v3(x, w + reach - 2.5, head),
                v3(x, w, head + 0.5),
            ]
        };
        b.paint(if k % 2 == 0 { PLATING } else { PLATING_DARK })
            .pattern(pattern::GENERIC);
        b.loft(
            &[
                ring(x0, out * 0.4),
                ring(x0 + rake, out),
                ring(x1 - rake, out),
                ring(x1, out * 0.4),
            ],
            true,
            true,
        );
        if fine {
            let xm = (x0 + x1) * 0.5;
            let w = width(xm) - 0.5 + out;
            match k % 3 {
                0 => {
                    b.paint(ACCENT).pattern(pattern::PLAIN);
                    b.block(
                        v3(xm - (x1 - x0) * 0.3, w - 0.2, head - 7.0),
                        v3(xm + (x1 - x0) * 0.3, w + 0.4, head - 4.0),
                    );
                }
                1 => {
                    b.paint(PLATING).pattern(pattern::PLAIN);
                    b.prism(v3(xm, w - out * 0.5, head), 6, 2.4, 1.2, 2.4);
                }
                _ => {
                    vent(
                        b,
                        v3(xm, w - out * 0.5, head),
                        v2((x1 - x0) * 0.5, out * 0.6),
                        4,
                        METAL,
                    );
                }
            }
        }
    }
    // Plain belts between them, up under the shoulder.
    b.paint(PLATING).pattern(pattern::GENERIC);
    plate_on(b, half, [-100.0, -32.0], 4, 6, [0.2, 0.8], 1.4);
    plate_on(b, half, [40.0, 150.0], 4, 6, [0.2, 0.8], 1.4);
}
