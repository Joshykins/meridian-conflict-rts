//! Greebles in clusters over the Dominion's decks and stacked layers, big enough to read
//! from the strategy camera: conduit runs down the second layer's walls, sensor blisters
//! and antenna masts on the third, docking clamps and hatch clusters along the hull's
//! deck, vent banks on the second layer's back.

use super::hull::{deck_edge, DECK};
use super::stacked::{l2_width, l3_width, L2_TOP, L3_TOP};
use super::*;

pub(super) fn build(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        // Conduit runs down the second layer's walls, broken at the vertebrae's feet.
        b.paint(METAL).pattern(pattern::PLAIN);
        for &(a, f, z) in &[
            (-196.0, -110.0, 101.0),
            (-176.0, -128.0, 104.5),
            (-30.0, 74.0, 101.0),
            (10.0, 76.0, 104.5),
            (110.0, 140.0, 101.0),
        ] {
            let at = |x: f32| v3(x, l2_width(x) - 1.2 + (z - 101.0) * -0.25, z);
            b.cylinder_between(at(a), at(f), 0.9, 0.9, if fine { 6 } else { 4 });
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                let mut x = a + 6.0;
                while x < f - 4.0 {
                    b.cuboid(at(x), v3(1.4, 2.2, 2.2));
                    x += 12.0;
                }
                b.paint(METAL).pattern(pattern::PLAIN);
            }
        }
        // Docking clamps and hatch clusters along the hull's deck, outboard of the layer.
        for &x in &[-200.0, -160.0, -128.0, -24.0, 14.0, 48.0, 104.0, 132.0] {
            let (w2, e) = (l2_width(x), deck_edge(x));
            let y = (w2 + e) * 0.5;
            b.paint(PLATING_DARK).pattern(pattern::PLAIN);
            b.chamfered_box(v3(x, e - 2.5, DECK + 1.2), v3(5.0, 3.0, 2.4), 0.8);
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.plate(v3(x + 6.0, y, DECK), v2(7.0, 6.0), 0.4, 0.15);
            if fine {
                b.paint(METAL).pattern(pattern::PLAIN);
                b.plate(v3(x + 6.0, y, DECK + 0.4), v2(4.4, 3.6), 0.25, 0.08);
                b.plate(v3(x - 6.0, y + 2.0, DECK), v2(3.6, 3.6), 0.3, 0.1);
            }
        }
        // Sensor blisters on the third layer's shoulders.
        b.paint(PLATING).pattern(pattern::PLAIN);
        for &x in &[-100.0, -20.0, 22.0] {
            let w = l3_width(x) - 7.0;
            b.prism(v3(x, w, L3_TOP), 6, 2.6, 1.4, 2.2);
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.prism(v3(x, w, L3_TOP + 2.2), 6, 1.4, 1.0, 0.6);
                b.paint(PLATING).pattern(pattern::PLAIN);
            }
        }
        // Antenna masts over the third layer's after end.
        b.paint(METAL).pattern(pattern::PLAIN);
        for &(x, h) in &[(-112.0, 10.0), (-96.0, 7.0)] {
            let y = l3_width(x) - 5.0;
            b.beam(
                v3(x, y, L3_TOP - 1.0),
                v3(x, y, L3_TOP + h),
                v2(0.7, 0.7),
                v2(0.35, 0.35),
            );
            if fine {
                b.beam(
                    v3(x, y - 2.5, L3_TOP + h * 0.7),
                    v3(x, y + 2.5, L3_TOP + h * 0.7),
                    v2(0.3, 0.3),
                    v2(0.3, 0.3),
                );
            }
        }
        // Vent banks on the second layer's back, clear of the cells and the ribs.
        if fine {
            for &x in &[-150.0, 64.0, 118.0] {
                let y = (l2_width(x) - 6.0 + l3_width(x).max(10.0)) * 0.5;
                vent(b, v3(x, y, L2_TOP), v2(10.0, 4.0), 5, METAL);
            }
        }
    });
}
