use super::*;
use crate::bake::BakeParams;

fn canyon() -> Terrain {
    Terrain::new(&BakeParams::canyon("t", 6, 11))
}

#[test]
fn the_dam_stands_on_its_dry_riverbed_against_the_lake() {
    let t = canyon();
    let (c, d) = (t.size_x / 2.0, GORGE_DAM);
    // The model is stood at the terrain's height at its origin.
    assert_eq!(t.height(c, TOE_V), d.floor_z);
    assert_eq!((TOE_V % 32.0, c % 32.0), (0.0, 0.0));
    // Level under the whole footprint, inside the concrete.
    for k in -10..=10 {
        for x in [0.0, 0.5 * d.base, d.base] {
            let y = k as f64 / 10.0 * (d.length / 2.0 - 20.0);
            assert_eq!(
                t.height(c + y, TOE_V + x),
                d.floor_z,
                "under the dam at {x},{y}"
            );
        }
    }
    // The lake against the heel; the riverbed below the toe dry, above the ring.
    assert!(
        t.height(c, TOE_V + d.base + 40.0) < 0.0,
        "no lake at the heel"
    );
    for back in [60.0, 300.0, 900.0, 1_800.0] {
        let h = t.height(c, TOE_V - back);
        assert!(h > RING_TOP, "riverbed {back} m below the toe at {h}");
    }
    // The ends run into rock standing near the crest's height.
    for side in [-1.0, 1.0] {
        let h = t.height(c + side * (d.length / 2.0 + d.key), TOE_V + d.base / 2.0);
        assert!(
            h > d.crest_z - 40.0,
            "the end at {side} stands in ground at {h}"
        );
    }
}

#[test]
fn trails_are_walkable_end_to_end() {
    let t = canyon();
    for (k, trail) in t.canyon.trails.iter().enumerate() {
        let (_, _, total) = along(trail.line[0], &trail.line);
        let mut last: Option<f64> = None;
        let n = (total / 8.0) as usize;
        for i in 0..=n {
            // The point `i * 8` m along the line.
            let want = i as f64 * 8.0;
            let (mut run, mut at) = (0.0, trail.line[0]);
            for w in trail.line.windows(2) {
                let len = seg_len(w[0], w[1]);
                if run + len >= want {
                    let f = ((want - run) / len).clamp(0.0, 1.0);
                    at = (
                        w[0].0 + (w[1].0 - w[0].0) * f,
                        w[0].1 + (w[1].1 - w[0].1) * f,
                    );
                    break;
                }
                run += len;
                at = w[1];
            }
            let h = t.height(at.0, at.1);
            if let Some(l) = last {
                assert!(
                    (h - l).abs() < 0.45 * 8.0,
                    "trail {k} steps {:.1} m at {want} m",
                    h - l
                );
            }
            last = Some(h);
        }
        let ends = (t.height(trail.line[0].0, trail.line[0].1), last.unwrap());
        assert!(
            ends.0 > RIM - 20.0 && ends.1 < BENCH_TOP + 10.0,
            "trail {k} runs {ends:?}"
        );
    }
}

/// `CANYON_RELIEF=x0,y0,span,px,out.ppm cargo test --release -p mc-map --lib canyon_relief -- --ignored`:
/// a hillshade of the canyon (sun from the north-west, water tinted, the beds
/// banded, ore white, starts yellow), to judge the landforms without the game.
/// `CANYON_BARE=1`: before erosion.
#[test]
#[ignore]
fn canyon_relief() {
    let spec =
        std::env::var("CANYON_RELIEF").unwrap_or_else(|_| "0,0,12288,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let mut t = canyon();
    if std::env::var("CANYON_BARE").is_ok() {
        t.erosion = Default::default();
    }
    let step = span / px as f64;
    let mut rows: Vec<Vec<u8>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..px)
            .map(|j| {
                let t = &t;
                s.spawn(move || {
                    let mut row = Vec::with_capacity(px * 3);
                    for i in 0..px {
                        let (x, y) = (
                            x0 + (i as f64 + 0.5) * step,
                            y0 + span - (j as f64 + 0.5) * step,
                        );
                        let e = step.max(4.0);
                        let h = t.height(x, y);
                        let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
                        let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
                        let (nx, ny, nz) = (-gx, -gy, 1.0);
                        let l = (nx * nx + ny * ny + nz * nz).sqrt();
                        let shade = ((nx * -0.5 + ny * 0.5 + nz * 0.7) / l / 0.95).clamp(0.15, 1.0);
                        let (r, g, b) = relief_colour(h);
                        row.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
                    }
                    row
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut mark = |p: (f64, f64), r: i64, rgb: [u8; 3]| {
        let (ci, cj) = (
            ((p.0 - x0) / step) as i64,
            ((y0 + span - p.1) / step) as i64,
        );
        for j in cj - r..=cj + r {
            for i in ci - r..=ci + r {
                if i >= 0 && j >= 0 && (i as usize) < px && (j as usize) < px {
                    rows[j as usize][i as usize * 3..i as usize * 3 + 3].copy_from_slice(&rgb);
                }
            }
        }
    };
    for f in &t.ore {
        mark((f.x, f.y), 2, [255, 255, 255]);
    }
    for &s in &t.starts {
        mark(s, 4, [255, 230, 0]);
    }
    for trail in &t.canyon.trails {
        for w in trail.line.windows(2) {
            for k in 0..8 {
                let f = k as f64 / 8.0;
                mark(
                    (
                        w[0].0 + (w[1].0 - w[0].0) * f,
                        w[0].1 + (w[1].1 - w[0].1) * f,
                    ),
                    0,
                    [0, 255, 255],
                );
            }
        }
    }
    for tm in &t.canyon.temples {
        mark(tm.a, 1, [255, 0, 255]);
        mark(tm.b, 1, [255, 0, 255]);
    }
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    for row in rows {
        out.extend(row);
    }
    std::fs::write(v[4], out).unwrap();
}

/// The hillshade's colour for a height: water by depth, then each bed.
fn relief_colour(h: f64) -> (f64, f64, f64) {
    if h < 0.0 {
        let d = (-h).min(60.0) / 60.0;
        (30.0 + 40.0 * (1.0 - d), 120.0 - 50.0 * d, 140.0 - 20.0 * d)
    } else if h < RING_TOP {
        (225.0, 215.0, 195.0)
    } else if h < BENCH_TOP {
        (165.0, 150.0, 120.0)
    } else if h < REDWALL_TOP {
        (190.0, 95.0, 70.0)
    } else if h < 212.0 {
        (150.0, 60.0, 40.0)
    } else if h < 240.0 {
        (200.0, 110.0, 70.0)
    } else if h < SUPAI_TOP {
        (140.0, 55.0, 45.0)
    } else if h < HERMIT_TOP {
        (110.0, 40.0, 35.0)
    } else if h < COCONINO_TOP {
        (225.0, 195.0, 150.0)
    } else if h < RIM + 2.0 {
        (240.0, 235.0, 225.0)
    } else {
        (170.0, 150.0, 115.0)
    }
}
