use super::{own_share, turn, MID, SIZE};

#[test]
fn the_shares_of_a_point_and_its_image_sum_to_one() {
    for &(x, y) in &[
        (100.0, 200.0),
        (6_000.0, 6_300.0),
        (MID, 3_000.0),
        (11_000.0, 9_000.0),
    ] {
        let sum = own_share((x, y)) + own_share(turn((x, y)));
        assert!((sum - 1.0).abs() < 1e-9, "{sum} at {x},{y}");
    }
    assert_eq!(turn(turn((1.0, 2.0))), (1.0, 2.0));
    assert_eq!(turn((MID, MID)), (MID, MID));
    assert_eq!(turn((0.0, 0.0)), (SIZE, SIZE));
}

#[test]
fn erebus_strait_is_the_same_under_a_half_turn_where_it_is_tame() {
    let t = &*crate::bake::test_maps::ANTARCTIC;
    let n = 121;
    let mut tame = 0;
    for j in 0..n {
        for i in 0..n {
            let p = (
                (i as f64 + 0.37) * t.size_x / n as f64,
                (j as f64 + 0.61) * t.size_y / n as f64,
            );
            let q = turn(p);
            if t.an_tame(p.0, p.1) < 1.0 {
                continue;
            }
            tame += 1;
            let (a, b) = (t.height(p.0, p.1), t.height(q.0, q.1));
            assert!((a - b).abs() < 0.02, "{a} vs {b} at {p:?}");
            // Where the ice lies (its blue patches and the snow on it are
            // each side's own).
            let (ia, ib) = (t.an_ice_cover(p.0, p.1).0, t.an_ice_cover(q.0, q.1).0);
            assert!((ia - ib).abs() < 1e-6, "ice {ia} vs {ib} at {p:?}");
        }
    }
    // The starts' country, the shore strips, the ore: a good share of the map.
    assert!(tame > n * n / 5, "only {tame} tame samples");
}

#[test]
fn noise_has_no_crease_on_the_middle_line() {
    let t = &*crate::bake::test_maps::ANTARCTIC;
    // Across the line x = middle, over the islands and the bergs' sea: the
    // ground steps from one metre to the next across the line as it does a
    // metre beside it.
    for k in 0..60 {
        let y = 200.0 + k as f64 * 200.0;
        let at = |x: f64| t.natural(x, y);
        let across = at(MID + 0.5) - at(MID - 0.5);
        let beside = at(MID + 1.5) - at(MID + 0.5);
        assert!(
            (across - beside).abs() < 0.5,
            "crease {across} vs {beside} at {MID},{y}"
        );
    }
}

#[test]
fn nothing_grows() {
    let t = &*crate::bake::test_maps::ANTARCTIC;
    for k in 0..400 {
        let (x, y) = (
            (k % 20) as f64 * 600.0 + 150.0,
            (k / 20) as f64 * 600.0 + 150.0,
        );
        assert_eq!(t.forest_density(x, y, 30.0, 0.05).0, 0.0);
    }
}

/// `ANTARCTIC_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib antarctic_relief -- --ignored`:
/// a hillshade of the land (sun from the north-west, water tinted, ice pale
/// blue, ground a unit cannot climb grey), to judge the landforms without the
/// game, and the heights beside it as little-endian f32 rows, north first
/// (`out.ppm.f32`).
#[test]
#[ignore]
fn antarctic_relief() {
    let spec =
        std::env::var("ANTARCTIC_RELIEF").unwrap_or_else(|_| "0,0,12288,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = &*crate::bake::test_maps::ANTARCTIC;
    let step = span / px as f64;
    let row = |j: usize| {
        let (mut rgb, mut raw) = (Vec::new(), Vec::new());
        for i in 0..px {
            let (x, y) = (
                x0 + (i as f64 + 0.5) * step,
                y0 + span - (j as f64 + 0.5) * step,
            );
            let e = step.max(4.0);
            let h = t.height(x, y);
            raw.extend((h as f32).to_le_bytes());
            let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
            let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
            let l = (gx * gx + gy * gy + 1.0).sqrt();
            let shade = ((gx * 0.5 - gy * 0.5 + 0.7) / l / 0.95).clamp(0.0, 1.0);
            let slope = (gx * gx + gy * gy).sqrt();
            let (ice, snow) = t.antarctic_snow(x, y, h, gx, gy);
            let (r, g, b) = if h < 0.0 {
                (30.0, 50.0 + h.max(-45.0) * 0.5, 80.0 + h.max(-45.0))
            } else if slope > 0.5 {
                (120.0, 115.0, 110.0)
            } else if t.ore.iter().any(|f| f.covers(x, y, 0.0)) {
                (220.0, 120.0, 40.0)
            } else {
                let base = (90.0, 85.0, 80.0);
                let white = if ice > 0.5 {
                    (200.0, 225.0, 250.0)
                } else {
                    (245.0, 245.0, 245.0)
                };
                let k = snow.max(ice);
                (
                    base.0 + (white.0 - base.0) * k,
                    base.1 + (white.1 - base.1) * k,
                    base.2 + (white.2 - base.2) * k,
                )
            };
            rgb.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
        }
        (rgb, raw)
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut rows: Vec<(Vec<u8>, Vec<u8>)> = vec![Default::default(); px];
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
    let mut raw = Vec::new();
    for (rgb, r) in rows {
        out.extend(rgb);
        raw.extend(r);
    }
    std::fs::write(v[4], out).unwrap();
    std::fs::write(format!("{}.f32", v[4]), raw).unwrap();
}
