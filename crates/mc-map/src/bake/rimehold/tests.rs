//! Rimehold laid out (not baked): the rooms stand level and the ways climb
//! gently, as the plan promises. (The baked map is held to its sidecar by
//! `tests/rimehold.rs`.)

use super::*;
use crate::bake::test_maps::RIMEHOLD;

#[test]
fn rooms_are_level_and_ways_gentle() {
    let t = &*RIMEHOLD;
    let mut problems = Vec::new();
    for s in SITES {
        let rings = if s.ground == Ground::Land { 4 } else { 1 };
        for ring in 0..=rings {
            let r = if s.ground == Ground::Land {
                s.room() * ring as f64 / 4.0
            } else {
                60.0 * ring as f64
            };
            for k in 0..24 {
                let a = k as f64 / 24.0 * std::f64::consts::TAU;
                let p = (s.at.0 + r * a.cos(), s.at.1 + r * a.sin());
                let h = t.height(p.0, p.1);
                if (h - s.level).abs() > 0.5 {
                    problems.push(format!(
                        "{}: {h:.1} m {r:.0} m out, not {}",
                        s.name, s.level
                    ));
                    break;
                }
            }
        }
    }
    for (s, way) in SITES.iter().zip(&t.rh.plan.ways) {
        if s.ground == Ground::Naval {
            continue;
        }
        let pts = way.points(16.0);
        let (worst, at) = pts
            .iter()
            .map(|&p| (t.slope(p.0, p.1), p))
            .fold((0.0, (0.0, 0.0)), |a, b| if b.0 > a.0 { b } else { a });
        if worst > 0.3 {
            problems.push(format!(
                "{}'s way is {worst:.2} steep at {at:?}, {:.0} m up",
                s.name,
                t.height(at.0, at.1)
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Prints the sidecar's survival block: `cargo test --profile gate -p mc-map --lib rimehold_block -- --nocapture`.
#[test]
fn rimehold_block() {
    println!("{}", RIMEHOLD.rimehold_block());
}

/// `RIMEHOLD_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib rimehold_relief -- --ignored`:
/// a hillshade (sun from the north-west), sea ice and bergs pale blue, water blue,
/// ground a unit cannot climb grey, the starts red, the sites' rooms yellow
/// (auxiliary ones orange), to judge the land without the game.
#[test]
#[ignore]
fn rimehold_relief() {
    let spec =
        std::env::var("RIMEHOLD_RELIEF").unwrap_or_else(|_| "0,0,20480,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = &*RIMEHOLD;
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
            let mark = |p: (f64, f64), r: f64| dist(p, (x, y)) < r;
            let start = t.starts.iter().any(|&s| mark(s, 2.5 * step.max(40.0)));
            let room = SITES.iter().find(|s| {
                let d = dist(s.at, (x, y));
                d < s.room() && d > s.room() - 2.0 * step.max(12.0)
            });
            let (r, g, b) = if start {
                (255.0, 40.0, 40.0)
            } else if let Some(s) = room {
                if s.kind == Kind::Printer {
                    (255.0, 230.0, 40.0)
                } else {
                    (255.0, 130.0, 20.0)
                }
            } else if h < 0.0 {
                (40.0, 70.0 + h.max(-45.0), 120.0 + h.max(-45.0))
            } else if slope > 0.5 {
                (150.0, 140.0, 130.0)
            } else if t.rh_ice((x, y), h) > 0.5 {
                (200.0, 225.0, 240.0)
            } else {
                // By height: pale tundra low, grey-brown up the plateau, grey rock, white heights.
                let ramp = [
                    (0.0, (150.0, 160.0, 150.0)),
                    (60.0, (175.0, 175.0, 165.0)),
                    (200.0, (160.0, 150.0, 140.0)),
                    (500.0, (140.0, 140.0, 145.0)),
                    (1_000.0, (240.0, 240.0, 245.0)),
                ];
                let k = ramp
                    .iter()
                    .rposition(|r| h >= r.0)
                    .unwrap_or(0)
                    .min(ramp.len() - 2);
                let (a, b) = (ramp[k], ramp[k + 1]);
                let t = ((h - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
                (
                    a.1 .0 + (b.1 .0 - a.1 .0) * t,
                    a.1 .1 + (b.1 .1 - a.1 .1) * t,
                    a.1 .2 + (b.1 .2 - a.1 .2) * t,
                )
            };
            let shade = if start || room.is_some() { 1.0 } else { shade };
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
