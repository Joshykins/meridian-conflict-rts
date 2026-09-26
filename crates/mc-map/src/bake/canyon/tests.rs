use super::*;
use crate::bake::BakeParams;

fn canyon() -> Terrain {
    Terrain::new(&BakeParams::canyon("t", 6, 11))
}

#[test]
fn what_plays_is_the_same_under_the_mirror() {
    let t = canyon();
    let n = 181;
    let mut kept = 0;
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (
                (i as f64 + 0.37) * t.size_x / n as f64,
                (j as f64 + 0.61) * t.size_y / n as f64,
            );
            if t.canyon_free(x, y) > 0.0 {
                continue;
            }
            kept += 1;
            let (mx, my) = t.mirrored((x, y));
            let (a, b) = (t.height(x, y), t.height(mx, my));
            assert!((a - b).abs() < 1e-9, "{a} vs {b} at {x},{y}");
            let (fa, fb) = (
                t.canyon_forest(x, y, a, 0.1).0,
                t.canyon_forest(mx, my, b, 0.1).0,
            );
            assert!((fa - fb).abs() < 1e-9, "woods {fa} vs {fb} at {x},{y}");
        }
    }
    // The bases, ore, trails, dam, coves and ford are a fair share of the map.
    assert!(kept > n * n / 10, "only {kept} kept points");
}

#[test]
fn the_dam_stands_on_its_crest() {
    let t = canyon();
    let c = t.size_x / 2.0;
    // The model is stood at the terrain's height at its origin.
    assert_eq!(t.height(c, DAM_V), DAM.crest_z);
    assert_eq!(DAM_V % 32.0, 0.0);
    assert_eq!(c % 32.0, 0.0);
    // Water both sides of the crest, the crest walkable end to end.
    assert!(t.height(c, DAM_V + 60.0) < 0.0, "no lake above the dam");
    assert!(
        t.height(c, DAM_V - 60.0) < 0.0,
        "no tailwater below the dam"
    );
    for k in -20..=20 {
        let a = k as f64 / 20.0 * DAM.half_angle;
        let (mx, my) = DAM.crest_at(a);
        let (x, y) = (c - my, DAM_V + mx);
        assert!((t.height(x, y) - DAM.crest_z).abs() < 1e-9, "crest at {a}");
    }
}

/// `CANYON_RELIEF=x0,y0,span,px,out.ppm cargo test --release -p mc-map --lib canyon_relief -- --ignored`:
/// a hillshade of the canyon (sun from the north-west, water tinted, the beds
/// banded), to judge the landforms without the game.
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
    // `CANYON_BARE=1`: the shape before erosion.
    if std::env::var("CANYON_BARE").is_ok() {
        t.erosion = Default::default();
    }
    let step = span / px as f64;
    let rows: Vec<Vec<u8>> = std::thread::scope(|s| {
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
                        let (r, g, b) = if h < 0.0 {
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
                        };
                        row.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
                    }
                    row
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut rows = rows;
    // Markers: ore white, starts yellow, temples magenta.
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
