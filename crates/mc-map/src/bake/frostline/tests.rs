use super::shape::{mesa, mesa_run, DEEP};
use super::*;
use crate::bake::canyon::{BENCH_TOP, COCONINO_TOP, HERMIT_TOP, REDWALL_TOP, RIM, SUPAI_TOP};
use crate::landmark::FROSTLINE_STRATA_LIFT as LIFT;

fn map() -> &'static Terrain {
    &crate::bake::test_maps::FROSTLINE
}

#[test]
fn the_wall_is_its_own_half_turn() {
    for (a, b) in WALL.iter().zip(WALL.iter().rev()) {
        assert_eq!((a.0 + b.0, a.1 + b.1), (SIZE, SIZE));
    }
    assert!(WALL.windows(2).all(|w| w[0].1 < w[1].1));
    assert_eq!(wall_x(SIZE / 2.0), SIZE / 2.0);
}

#[test]
fn the_mesas_are_cut_in_the_canyons_beds() {
    // The big cliff from the bench to the Redwall's top, the rim at its height.
    assert_eq!(mesa(0.0), BENCH_TOP - LIFT);
    assert_eq!(mesa(37.5), REDWALL_TOP - LIFT);
    assert!((mesa(400.0) - (RIM - LIFT)).abs() < 20.0);
    let mut last = mesa(0.0);
    for i in 1..500 {
        let h = mesa(i as f64);
        assert!(h >= last, "the beds overhang at {i}");
        last = h;
    }
    assert!((mesa(mesa_run(100.0)) - 100.0).abs() < 1e-9);
}

/// Every base stands on its own side of the wall on level, dry ground at
/// its twin's height, and the two coves' heads are beaches.
#[test]
fn the_bases_stand_level_and_the_coves_come_down_to_the_water() {
    let t = map();
    for pair in t.starts.chunks(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(east_of(a.0, a.1) < 0.0 && east_of(b.0, b.1) > 0.0);
        let (ha, hb) = (t.height(a.0, a.1), t.height(b.0, b.1));
        assert!(ha > 10.0 && (ha - hb).abs() < 1e-6, "{ha} and {hb}");
        for p in [a, b] {
            assert!(
                t.slope(p.0 + 150.0, p.1 - 90.0) < 0.02,
                "a pad is not level"
            );
        }
    }
    for head in [on_map(COVE_HEAD), t.turned(on_map(COVE_HEAD))] {
        // A little way up the canyon from the head: its floor, just over the water.
        let h = t.height(head.0, head.1);
        assert!((-4.0..8.0).contains(&h), "a cove's head stands at {h}");
    }
}

/// The design's frame and the map's are one turn apart about the middle.
#[test]
fn the_design_is_laid_on_the_map_turned() {
    let mid = (SIZE / 2.0, SIZE / 2.0);
    assert_eq!(on_map(mid), mid);
    for p in [(1_500.0, 8_192.0), (3_784.0, 1_434.0), (-2_000.0, 13_000.0)] {
        let m = on_map(p);
        let back = design(m);
        assert!(dist(back, p) < 1e-6, "{p:?} comes back as {back:?}");
        assert!((dist(m, mid) - dist(p, mid)).abs() < 1e-6);
    }
    // Clockwise: the design's west lies north of west on the map.
    let rear = on_map(STARTS[0]);
    assert!(rear.0 < mid.0 && rear.1 > mid.1, "{rear:?}");
    // Every base and every ore field lies well inside the map.
    for &(x, y) in STARTS {
        let (mx, my) = on_map((x, y));
        let inside = mx.min(my).min(SIZE - mx).min(SIZE - my);
        assert!(inside > 1_500.0, "the base at {x}, {y} is {inside:.0} m in");
    }
    for &(x, y, _) in ORE.iter().chain(ISLE_ORE) {
        let (mx, my) = on_map((x, y));
        let inside = mx.min(my).min(SIZE - mx).min(SIZE - my);
        assert!(inside > 600.0, "the ore at {x}, {y} is {inside:.0} m in");
    }
}

#[test]
fn the_wall_stands_in_the_sea_and_blocks_little() {
    let t = map();
    let towers: Vec<_> = t
        .precursor
        .iter()
        .filter(|s| s.kind == PropKind::PrecursorTower)
        .collect();
    assert!(towers.len() >= 8, "{} towers", towers.len());
    // No ground is made for the wall: the towers stand on the sea floor.
    assert!(t.benches.is_empty());
    let mid = (SIZE / 2.0, SIZE / 2.0);
    for s in &towers {
        // On the line, in open sea, off the land bridge.
        assert!(east_of(s.x, s.y).abs() < 1.0, "a tower off the wall");
        let h = t.natural(s.x, s.y);
        assert!(h < -TOWER_DEPTH, "a tower stands in {:.0} m of water", -h);
        assert!(dist((s.x, s.y), mid) > 1_400.0, "a tower on the bridge");
        // Its twin is there too.
        let q = t.turned((s.x, s.y));
        assert!(towers.iter().any(|o| dist((o.x, o.y), q) < 1.0));
    }
    // Between any two towers there is room for an army or a fleet.
    for a in &towers {
        let nearest = towers
            .iter()
            .filter(|b| dist((a.x, a.y), (b.x, b.y)) > 1.0)
            .map(|b| dist((a.x, a.y), (b.x, b.y)))
            .fold(f64::INFINITY, f64::min);
        assert!(nearest > 790.0, "towers {nearest:.0} m apart");
    }
    // Over land the wall is the shader's line of light alone: no conduit
    // pieces, which stepped in and out of the slopes they crossed.
    assert!(!t
        .precursor
        .iter()
        .any(|s| s.kind == PropKind::PrecursorConduit));
}

/// `FROSTLINE_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib frostline_relief -- --ignored`:
/// a shaded picture of the land (sun from the north-west; the desert in its
/// beds' colours, the snow and ice east of the wall, the sea by depth) with
/// the starts, ore, towers and wall marked, to judge the layout without the
/// game, and the heights beside it as little-endian f32 rows, north first
/// (`out.ppm.f32`).
#[test]
#[ignore]
fn frostline_relief() {
    let spec =
        std::env::var("FROSTLINE_RELIEF").unwrap_or_else(|_| "0,0,16384,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = Terrain::new(&crate::bake::BakeParams::frostline("t", 8, 9));
    let step = span / px as f64;
    let mut rgb = vec![0u8; px * px * 3];
    let mut raw = vec![0u8; px * px * 4];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows = px.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, (out, heights)) in rgb
            .chunks_mut(rows * px * 3)
            .zip(raw.chunks_mut(rows * px * 4))
            .enumerate()
        {
            let t = &t;
            s.spawn(move || {
                for r in 0..out.len() / (px * 3) {
                    let j = k * rows + r;
                    for i in 0..px {
                        let (x, y) = (
                            x0 + (i as f64 + 0.5) * step,
                            y0 + span - (j as f64 + 0.5) * step,
                        );
                        let e = step.max(8.0);
                        let h = t.height(x, y);
                        heights[(r * px + i) * 4..][..4].copy_from_slice(&(h as f32).to_le_bytes());
                        let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
                        let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
                        let l = (gx * gx + gy * gy + 1.0).sqrt();
                        let shade = ((gx * 0.5 - gy * 0.5 + 0.7) / l / 0.95).clamp(0.15, 1.0);
                        let slope = (gx * gx + gy * gy).sqrt();
                        let (ice, snow) = t.frostline_snow(x, y, h, gx, gy);
                        let c = if h < 0.0 {
                            let d = (-h / DEEP).clamp(0.0, 1.0);
                            let shallow = if east_of(x, y) < 0.0 {
                                [70.0, 150.0, 140.0]
                            } else {
                                [70.0, 110.0, 130.0]
                            };
                            [0, 1, 2].map(|c| (shallow[c] * (1.0 - 0.7 * d)) / shade)
                        } else if east_of(x, y) < 0.0 {
                            // The beds by height, as the terrain shader lays them.
                            match h + LIFT {
                                a if a < 55.0 => [215.0, 205.0, 180.0],
                                a if a < 70.0 => [190.0, 165.0, 120.0],
                                a if a < BENCH_TOP => [170.0, 150.0, 105.0],
                                a if a < REDWALL_TOP => [165.0, 95.0, 70.0],
                                a if a < SUPAI_TOP => [175.0, 80.0, 55.0],
                                a if a < HERMIT_TOP => [150.0, 60.0, 45.0],
                                a if a < COCONINO_TOP => [215.0, 190.0, 150.0],
                                _ => [185.0, 170.0, 145.0],
                            }
                        } else {
                            let ground = if slope > 0.5 {
                                [120.0, 118.0, 116.0]
                            } else {
                                [105.0, 130.0, 95.0]
                            };
                            let c = [0, 1, 2]
                                .map(|c| ground[c] + ([240.0, 244.0, 250.0][c] - ground[c]) * snow);
                            [0, 1, 2].map(|k| c[k] + ([150.0, 205.0, 235.0][k] - c[k]) * ice)
                        };
                        for (k, v) in c.iter().enumerate() {
                            out[(r * px + i) * 3 + k] = (v * shade).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            });
        }
    });
    let mut dot = |p: (f64, f64), radius: i64, colour: [u8; 3]| {
        let (cx, cy) = (
            ((p.0 - x0) / step) as i64,
            ((y0 + span - p.1) / step) as i64,
        );
        for y in (cy - radius).max(0)..=(cy + radius).min(px as i64 - 1) {
            for x in (cx - radius).max(0)..=(cx + radius).min(px as i64 - 1) {
                rgb[(y as usize * px + x as usize) * 3..][..3].copy_from_slice(&colour);
            }
        }
    };
    let mut y = 0.0;
    while y < SIZE {
        dot((wall_x(y), y), 0, [60, 255, 255]);
        y += step;
    }
    for f in &t.ore {
        dot((f.x, f.y), (f.radius / step) as i64, [255, 96, 40]);
    }
    for s in &t.precursor {
        if s.kind == PropKind::PrecursorTower {
            dot((s.x, s.y), (60.0 / step).max(1.0) as i64, [255, 255, 255]);
        }
    }
    for &s in &t.starts {
        dot(s, (90.0 / step).max(2.0) as i64, [230, 30, 30]);
    }
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    out.extend(rgb);
    std::fs::write(v[4], out).unwrap();
    std::fs::write(format!("{}.f32", v[4]), raw).unwrap();
}
