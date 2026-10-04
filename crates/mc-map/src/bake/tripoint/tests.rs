use super::shape::depths;
use super::*;
use crate::bake::canyon::{BENCH_TOP, COCONINO_TOP, HERMIT_TOP, REDWALL_TOP, SUPAI_TOP};
use crate::landmark::{FROSTLINE_STRATA_LIFT as LIFT, TRIPOINT_WALLS};

fn map() -> &'static Terrain {
    &crate::bake::test_maps::TRIPOINT
}

#[test]
fn three_turns_come_back_and_the_thirds_lie_between_the_walls() {
    for p in [(0.0, 3_850.0), (1_234.5, -987.0), (-4_000.0, 10.0)] {
        let back = turn(turn(turn(p, 1), 1), 1);
        assert!(dist(back, p) < 1e-9, "{p:?} comes back as {back:?}");
        assert!(dist(turn(p, 2), turn(turn(p, 1), 1)) < 1e-9);
    }
    // The walls a third of a turn apart, the thirds where the region lookup
    // the renderer and the tests share puts them.
    for (k, [a, b]) in TRIPOINT_WALLS.iter().enumerate() {
        let heading = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        let want = 30.0 + 120.0 * k as f64;
        assert!(
            ((heading - want + 180.0).rem_euclid(360.0) - 180.0).abs() < 0.01,
            "wall {k} heads {heading}"
        );
        assert_eq!(*a, (MID, MID));
    }
    for i in 0..360 {
        let a = (i as f64 + 0.5).to_radians();
        let v = (2_500.0 * a.cos(), 2_500.0 * a.sin());
        assert_eq!(third(v), tripoint_region(on_map(v)), "{i} degrees");
        let d = depths(v);
        assert!(d[third(v)] >= 0.0 && (0..3).all(|k| k == third(v) || d[k] <= 0.0));
    }
}

/// Every base stands level in its own region at the north's height; the
/// installation's benches are at the plateau's level.
#[test]
fn the_bases_stand_level_each_in_its_own_climate() {
    let t = map();
    assert_eq!(t.starts.len(), 3);
    let h0 = t.height(t.starts[0].0, t.starts[0].1);
    for (k, &s) in t.starts.iter().enumerate() {
        assert_eq!(tripoint_region(s), k);
        let want = on_map(turn(design(t.starts[0]), k));
        assert!(dist(s, want) < 1e-6);
        let h = t.height(s.0, s.1);
        assert!(h > 40.0 && (h - h0).abs() < 1e-6, "{h} and {h0}");
        assert!(
            t.slope(s.0 + 150.0, s.1 - 90.0) < 0.02,
            "a pad is not level"
        );
    }
    assert_eq!(t.ore.len() % 3, 0);
    for f in t.ore.chunks(3) {
        for (k, g) in f.iter().enumerate() {
            let want = on_map(turn(design((f[0].x, f[0].y)), k));
            assert!(dist((g.x, g.y), want) < 1e-6);
        }
    }
    assert!(t.benches.iter().all(|b| b.level == HIGH));
}

/// What decides where a unit can go is the same in every third: the benches,
/// vales, ramps and plateau, turned.
#[test]
fn the_benches_are_the_same_turned() {
    let t = map();
    for i in 0..400 {
        let a = i as f64 * 0.731;
        let r = 300.0 + (i as f64 * 37.0) % 5_200.0;
        let q = (r * a.cos(), r * a.sin());
        let level = t.tp_level(q, 0.0);
        for k in 1..3 {
            let other = t.tp_level(turn(q, k), 0.0);
            assert!(
                (other.0 - level.0).abs() < 1e-6 && (other.1 - level.1).abs() < 1e-6,
                "{q:?}: {level:?} turned is {other:?}"
            );
        }
    }
}

/// The plateau stands over the uplands, and the uplands over the vales; the
/// ramps lead up.
#[test]
fn the_vales_lie_below_the_uplands_and_the_plateau_above() {
    let t = map();
    let (plateau, _, _, _) = t.tp_level((0.0, 400.0), 0.0);
    let (upland, _, _, _) = t.tp_level((0.0, 3_000.0), 0.0);
    let (vale, low, _, _) = t.tp_level(vale_point((3_000.0, 0.0)), 0.0);
    assert_eq!((plateau, upland, vale, low), (HIGH, UPLAND, LOW, 1.0));
    // Up the plateau's ramp: no step steeper than a unit can climb.
    let mut last = t.tp_level((0.0, 1_600.0), 0.0).0;
    for i in 1..=80 {
        let y = 1_600.0 - i as f64 * 10.0;
        let h = t.tp_level((0.0, y), 0.0).0;
        assert!(
            (h - last).abs() < 4.0,
            "the ramp steps {} m at {y}",
            h - last
        );
        last = h;
    }
    assert_eq!(last, HIGH);
    // The lake is open water, too shallow for a ship.
    let (along, left, _) = LAKE;
    let c = on_map(vale_point((along, left)));
    let h = t.natural(c.0, c.1);
    assert!((-6.0..-1.0).contains(&h), "the lake's floor is at {h}");
}

/// The wall's towers stand on its lines, every wall alike.
#[test]
fn the_towers_stand_on_the_walls() {
    let t = map();
    let towers: Vec<(f64, f64)> = t
        .precursor
        .iter()
        .filter(|s| s.kind == PropKind::PrecursorTower)
        .map(|s| (s.x, s.y))
        .collect();
    assert!(towers.len() >= 12, "{} towers", towers.len());
    for &p in &towers {
        let v = design(p);
        let off = depths(v)
            .iter()
            .map(|d| d.abs())
            .fold(f64::INFINITY, f64::min);
        assert!(off < 1.0, "the tower at {p:?} is {off:.0} m off its wall");
    }
    let axes = t
        .precursor
        .iter()
        .filter(|s| s.kind == PropKind::PrecursorAxis)
        .count();
    let bastions = t
        .precursor
        .iter()
        .filter(|s| s.kind == PropKind::PrecursorBastion)
        .count();
    assert_eq!((axes, bastions), (1, 3));
}

/// `TRIPOINT_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib tripoint_relief -- --ignored`:
/// a shaded picture of the land (sun from the north-west; Alaska green under
/// its snow and ice, the desert in its beds' colours, the jungle deep green,
/// water blue) with the starts, ore, towers and walls marked, to judge the
/// layout without the game.
#[test]
#[ignore]
fn tripoint_relief() {
    let spec =
        std::env::var("TRIPOINT_RELIEF").unwrap_or_else(|_| "0,0,12288,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = Terrain::new(&crate::bake::BakeParams::tripoint("t", 6, 13));
    let step = span / px as f64;
    let mut rgb = vec![0u8; px * px * 3];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let rows = px.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, out) in rgb.chunks_mut(rows * px * 3).enumerate() {
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
                        let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
                        let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
                        let l = (gx * gx + gy * gy + 1.0).sqrt();
                        let shade = ((gx * 0.5 - gy * 0.5 + 0.7) / l / 0.95).clamp(0.15, 1.0);
                        let slope = (gx * gx + gy * gy).sqrt();
                        let c = if h < 0.0 {
                            [60.0, 120.0, 150.0]
                        } else {
                            match tripoint_region((x, y)) {
                                ALASKA => {
                                    let (ice, snow) = t.tripoint_snow(x, y, h, gx, gy);
                                    let ground = if slope > 0.5 {
                                        [120.0, 118.0, 116.0]
                                    } else {
                                        [105.0, 130.0, 95.0]
                                    };
                                    let c = [0, 1, 2].map(|c| {
                                        ground[c] + ([240.0, 244.0, 250.0][c] - ground[c]) * snow
                                    });
                                    [0, 1, 2]
                                        .map(|k| c[k] + ([150.0, 205.0, 235.0][k] - c[k]) * ice)
                                }
                                // The beds by height, as the terrain shader lays them.
                                DESERT => match h + LIFT {
                                    a if a < 55.0 => [215.0, 205.0, 180.0],
                                    a if a < 70.0 => [190.0, 165.0, 120.0],
                                    a if a < BENCH_TOP => [170.0, 150.0, 105.0],
                                    a if a < REDWALL_TOP => [165.0, 95.0, 70.0],
                                    a if a < SUPAI_TOP => [175.0, 80.0, 55.0],
                                    a if a < HERMIT_TOP => [150.0, 60.0, 45.0],
                                    a if a < COCONINO_TOP => [215.0, 190.0, 150.0],
                                    _ => [185.0, 170.0, 145.0],
                                },
                                _ if h < 12.5 => [220.0, 205.0, 160.0],
                                _ if slope > 0.9 => [110.0, 115.0, 95.0],
                                _ => [60.0, 125.0, 60.0],
                            }
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
    for [a, b] in TRIPOINT_WALLS {
        let n = (dist(a, b) / step) as usize;
        for i in 0..n {
            let f = i as f64 / n as f64;
            dot(
                (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f),
                0,
                [60, 255, 255],
            );
        }
    }
    for f in &t.ore {
        dot((f.x, f.y), (f.radius / step) as i64, [255, 96, 40]);
    }
    for s in &t.precursor {
        let r = match s.kind {
            PropKind::PrecursorTower | PropKind::PrecursorAxis | PropKind::PrecursorBastion => {
                s.reach() * 0.7
            }
            _ => continue,
        };
        dot((s.x, s.y), (r / step).max(1.0) as i64, [255, 255, 255]);
    }
    for &s in &t.starts {
        dot(s, (90.0 / step).max(2.0) as i64, [230, 30, 30]);
    }
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    out.extend(rgb);
    std::fs::write(v[4], out).unwrap();
}
