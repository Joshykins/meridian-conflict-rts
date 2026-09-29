//! The Dominion's flanks, laid on the hull's wall (chine to shoulder): armoured sponsons
//! of different sizes and shapes stood out of the wall in a broken rhythm, a heavy
//! armoured section over the engines, and the lower chine varied under the overhang.

use super::hull::{self, width};
use super::*;

pub(super) fn build(b: &mut MeshBuilder) {
    let half = hull::hull_half;
    b.mirror_y(|b| {
        lower_chine(b, &half);
        engine_armour(b, &half);
        sponsons(b, &half);
        b.paint(TEAM).pattern(pattern::PLAIN);
        plate_on(b, &half, [178.0, 186.0], 4, 6, [0.2, 0.5], 2.4);
    });
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
        (-172.0, -150.0, 0.9),
        (-40.0, -2.0, 1.4),
        (2.0, 58.0, 2.2),
        (116.0, 146.0, 1.0),
        (150.0, 196.0, 1.8),
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
    // (from, to, how far out, foot, head, rake of the ends)
    let list: &[(f32, f32, f32, f32, f32, f32)] = &[
        (-214.0, -162.0, 14.0, 70.0, 90.0, 10.0),
        (-134.0, -108.0, 8.0, 73.0, 87.0, 6.0),
        (-36.0, -12.0, 5.5, 76.0, 84.0, 3.0),
        (6.0, 58.0, 7.0, 72.0, 82.0, 12.0),
        (124.0, 152.0, 9.0, 72.0, 88.0, 7.0),
        (166.0, 190.0, 4.5, 76.0, 86.0, 4.0),
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
    plate_on(b, half, [-100.0, -46.0], 4, 6, [0.2, 0.8], 1.4);
    plate_on(b, half, [66.0, 116.0], 4, 6, [0.2, 0.8], 1.4);
}
