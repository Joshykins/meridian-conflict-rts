//! Thousand Spires laid out (not baked): what the layout promises stands
//! where it should. (The walks are held even on the baked map, by
//! `tests/thousand_spires.rs`.)

use super::*;
use crate::bake::test_maps::SPIRES;

#[test]
fn the_bases_stand_level() {
    let t = &*SPIRES;
    assert_eq!(t.starts.len(), 12);
    for (i, &(x, y)) in t.starts.iter().enumerate() {
        let h = t.height(x, y);
        assert!(h > 40.0, "base {i} at {h} m, down by the sea");
        for r in [150.0, 300.0, 420.0] {
            for k in 0..8 {
                let a = k as f64 * PI / 4.0;
                let s = t.slope(x + r * a.cos(), y + r * a.sin());
                assert!(s < 0.25, "base {i}: slope {s} {r} m out");
            }
        }
    }
}

#[test]
fn the_middle_and_the_sea_are_where_they_should_be() {
    let t = &*SPIRES;
    for k in 0..3 {
        let at = |p: (f64, f64)| {
            let m = on_map(turn(p, k));
            t.height(m.0, m.1)
        };
        // The lake's middle is the island, dry; round it the lake, too
        // shallow for ships.
        assert!(at((0.0, 0.0)) > 4.0, "island");
        // (Pillars stand in it: most of a ring round the island is water.)
        let wet = (0..24)
            .map(|i| {
                let a = i as f64 * PI / 36.0 + PI / 3.0;
                at((950.0 * a.cos(), 950.0 * a.sin()))
            })
            .filter(|h| (-LAKE_DEEP - 0.1..0.0).contains(h))
            .count();
        assert!(wet >= 16, "only {wet} of 24 in the lake in third {k}");
        // The sea behind every team.
        assert!(at((0.0, 12_100.0)) < -20.0, "sea behind team {k}");
        // The massif stands tall either side of the gate.
        let massif = [2_900.0, 3_500.0, 4_100.0]
            .iter()
            .flat_map(|&r| [at((-900.0, r)), at((900.0, r))])
            .fold(0.0, f64::max);
        assert!(massif > 300.0, "massif in third {k}: {massif}");
    }
}

/// `SPIRES_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib spires_relief -- --ignored`:
/// a hillshade of the land (sun from the north-west, water tinted, ground a
/// unit cannot climb grey, the bases red), to judge the landforms without
/// the game.
#[test]
#[ignore]
fn spires_relief() {
    let spec =
        std::env::var("SPIRES_RELIEF").unwrap_or_else(|_| "0,0,24576,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = &*SPIRES;
    let step = span / px as f64;
    let row = |j: usize| {
        let mut rgb = Vec::new();
        for i in 0..px {
            let (x, y) = (
                x0 + (i as f64 + 0.5) * step,
                y0 + span - (j as f64 + 0.5) * step,
            );
            let e = step.max(4.0);
            let h = t.height(x, y);
            let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
            let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
            let l = (gx * gx + gy * gy + 1.0).sqrt();
            let shade = ((gx * 0.5 - gy * 0.5 + 0.7) / l / 0.95).clamp(0.0, 1.0);
            let slope = (gx * gx + gy * gy).sqrt();
            let base = t
                .starts
                .iter()
                .any(|s| (s.0 - x).hypot(s.1 - y) < 2.5 * step.max(40.0));
            let (r, g, b) = if base {
                (255.0, 40.0, 40.0)
            } else if h < 0.0 {
                (40.0, 70.0 + h.max(-45.0), 120.0 + h.max(-45.0))
            } else if slope > 0.5 {
                (150.0, 140.0, 130.0)
            } else {
                let k = (h / 160.0).clamp(0.0, 1.0);
                (110.0 + 90.0 * k, 150.0 + 30.0 * k, 90.0 + 60.0 * k)
            };
            let shade = if base { 1.0 } else { shade };
            rgb.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
        }
        rgb
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut rows: Vec<Vec<u8>> = vec![Vec::new(); px];
    std::thread::scope(|s| {
        for (k, chunk) in rows.chunks_mut(px.div_ceil(threads)).enumerate() {
            let row = &row;
            s.spawn(move || {
                for (r, out) in chunk.iter_mut().enumerate() {
                    *out = row(k * px.div_ceil(threads) + r);
                }
            });
        }
    });
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    for rgb in rows {
        out.extend(rgb);
    }
    std::fs::write(v[4], out).unwrap();
}
