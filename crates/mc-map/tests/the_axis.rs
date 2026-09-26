//! Holds the tropical map "The Axis" (`maps/the_axis.mcmap`, baked with
//! `mc-bake --layout archipelago`) to its design: every player on an island of
//! their own, starting on level ground; every home island's ore can be walked
//! to from its start; every start has a harbour on water that sails to every
//! other start's; and twin starts (a half turn apart) get the same ground to
//! build on and the same distances by sea, though their coasts differ. The
//! Precursor artifacts' solid cells count as obstacles, as the sim blocks them.
//! Without the baked file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test the_axis -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_map::{Heightfield, MapFile, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

struct Map {
    hf: Heightfield,
    solid: Vec<bool>,
    w: u32,
    h: u32,
}

impl Map {
    fn cell_low(&self, cx: u32, cy: u32) -> f64 {
        let w = self.hf.water_level();
        [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|&(dx, dy)| (self.hf.sample_height(cx + dx, cy + dy) - w).to_f64())
            .fold(f64::INFINITY, f64::min)
    }
    fn land(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) > 0.0
            && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
            && !self.solid[(cy * self.w + cx) as usize]
    }
    fn sea(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) <= -6.0 && !self.solid[(cy * self.w + cx) as usize]
    }
    fn cell(&self, p: (f64, f64)) -> (u32, u32) {
        self.hf.cell_at(fx(p))
    }
    /// Steps (cells) from `from` to every cell `open` accepts; `u32::MAX` where unreached.
    fn walk(&self, from: (f64, f64), open: impl Fn(u32, u32) -> bool) -> Vec<u32> {
        self.walk_from(&[self.cell(from)], open)
    }
    /// The same from the nearest of several cells.
    fn walk_from(&self, from: &[(u32, u32)], open: impl Fn(u32, u32) -> bool) -> Vec<u32> {
        let mut steps = vec![u32::MAX; (self.w * self.h) as usize];
        let mut queue: VecDeque<((u32, u32), u32)> = from.iter().map(|&c| (c, 0u32)).collect();
        while let Some(((cx, cy), n)) = queue.pop_front() {
            if cx >= self.w || cy >= self.h {
                continue;
            }
            let i = (cy * self.w + cx) as usize;
            if steps[i] != u32::MAX || !open(cx, cy) {
                continue;
            }
            steps[i] = n;
            for next in [
                (cx + 1, cy),
                (cx.wrapping_sub(1), cy),
                (cx, cy + 1),
                (cx, cy.wrapping_sub(1)),
            ] {
                queue.push_back((next, n + 1));
            }
        }
        steps
    }
    fn at(&self, steps: &[u32], p: (f64, f64)) -> u32 {
        let (cx, cy) = self.cell(p);
        steps[(cy * self.w + cx) as usize]
    }
}

/// Cells of navigable water within `reach` metres of `p`.
fn harbour(map: &Map, p: (f64, f64), reach: f64) -> Vec<(u32, u32)> {
    let (cx, cy) = map.cell(p);
    let r = (reach / CELL_SIZE_M as f64) as i32;
    let mut out = Vec::new();
    for dy in (-r..=r).step_by(4) {
        for dx in (-r..=r).step_by(4) {
            let (x, y) = (cx as i32 + dx, cy as i32 + dy);
            if dx * dx + dy * dy <= r * r
                && x >= 0
                && y >= 0
                && (x as u32) < map.w
                && (y as u32) < map.h
                && map.sea(x as u32, y as u32)
            {
                out.push((x as u32, y as u32));
            }
        }
    }
    out
}

fn check(path: &std::path::Path) {
    let file = MapFile::open(path).unwrap();
    let hf = Heightfield::load(&file).unwrap();
    let (w, h) = hf.size_cells();
    let size = hf.size_metres().y.to_f64();
    let mut solid = vec![false; (w * h) as usize];
    for p in file.props().iter().filter(|p| p.kind.is_precursor()) {
        for (y, a, b) in p.solid_runs((w, h)) {
            for x in a..=b {
                solid[(y * w + x) as usize] = true;
            }
        }
    }
    let map = Map { hf, solid, w, h };
    let starts: Vec<(f64, f64)> = file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 8);
    let mut problems = Vec::new();
    let km = |n: u32| n as f64 * CELL_SIZE_M as f64 / 1000.0;

    // Starts come in pairs a half turn apart, the west side's first.
    for pair in starts.chunks(2) {
        if (pair[0].0 + pair[1].0 - size).abs() > 0.5
            || (pair[0].1 + pair[1].1 - size).abs() > 0.5
            || pair[0].0 > size / 2.0
        {
            problems.push(format!(
                "starts {:?} and {:?} are not a west/east pair",
                pair[0], pair[1]
            ));
        }
    }

    // Level, open ground round every start.
    for (i, &s) in starts.iter().enumerate() {
        let (cx, cy) = map.cell(s);
        let r = (120.0 / CELL_SIZE_M as f64) as i32;
        let mut bad = 0;
        for dy in -r..=r {
            for dx in -r..=r {
                let (x, y) = ((cx as i32 + dx) as u32, (cy as i32 + dy) as u32);
                if !map.land(x, y) || map.hf.cell_slope(x, y) > Fx::ratio(1, 20) {
                    bad += 1;
                }
            }
        }
        if bad > 0 {
            problems.push(format!(
                "start {i} at {s:?}: {bad} cells within 120 m are not level open ground"
            ));
        }
    }

    // Every player on an island of their own; the island's ore walkable from the start.
    let walks: Vec<Vec<u32>> = starts
        .iter()
        .map(|&s| map.walk(s, |x, y| map.land(x, y)))
        .collect();
    let ore: Vec<(f64, f64)> = file
        .ore_regions()
        .iter()
        .map(|r| {
            let c = r
                .points
                .iter()
                .fold((0.0, 0.0), |a, p| (a.0 + p.x.to_f64(), a.1 + p.y.to_f64()));
            (c.0 / r.points.len() as f64, c.1 / r.points.len() as f64)
        })
        .collect();
    let mut own = vec![0; 8];
    let mut ground = vec![0usize; 8];
    for (i, steps) in walks.iter().enumerate() {
        for (j, &t) in starts.iter().enumerate() {
            if j != i && map.at(steps, t) != u32::MAX {
                problems.push(format!(
                    "start {i} can walk to start {j}: they share an island"
                ));
            }
        }
        for &c in &ore {
            let d = ((c.0 - starts[i].0).powi(2) + (c.1 - starts[i].1).powi(2)).sqrt();
            if d < 1_000.0 * size / 20_480.0 {
                own[i] += 1;
                if map.at(steps, c) == u32::MAX {
                    problems.push(format!("start {i} cannot walk to its ore field at {c:?}"));
                }
            }
        }
        // Buildable ground on the island: level enough for a structure.
        ground[i] = (0..w * h)
            .filter(|&c| {
                steps[c as usize] != u32::MAX && map.hf.cell_slope(c % w, c / w) < Fx::ratio(1, 6)
            })
            .count();
    }
    println!("ore by each start: {own:?}");
    println!("buildable cells on each home island: {ground:?}");
    for i in (0..8).step_by(2) {
        if own[i] != own[i + 1] || own[i] < 5 {
            problems.push(format!(
                "starts {i} and {} have {} and {} fields of their own",
                i + 1,
                own[i],
                own[i + 1]
            ));
        }
        let (a, b) = (ground[i] as f64, ground[i + 1] as f64);
        if (a - b).abs() > 0.1 * a.max(b) {
            problems.push(format!(
                "starts {i} and {}: {a} vs {b} buildable cells",
                i + 1
            ));
        }
    }

    // Harbours: deep water close to every start, all on one sea.
    // The open sea: off the middle of the north edge.
    let sea = map.walk((size * 0.5, size - 60.0), |x, y| map.sea(x, y));
    let mut sails = Vec::new();
    for (i, &s) in starts.iter().enumerate() {
        let near = harbour(&map, s, 1_400.0);
        let open: Vec<_> = near
            .iter()
            .filter(|&&(x, y)| sea[(y * w + x) as usize] != u32::MAX)
            .collect();
        if open.is_empty() {
            problems.push(format!(
                "start {i} has no harbour on the open sea within 1.4 km"
            ));
            sails.push(None);
            continue;
        }
        let open: Vec<(u32, u32)> = open.into_iter().copied().collect();
        sails.push(Some(map.walk_from(&open, |x, y| map.sea(x, y))));
    }
    // Sailing distance from each start's harbour to each other's: twins see the same.
    let by_sea = |i: usize, j: usize| -> Option<f64> {
        let steps = sails[i].as_ref()?;
        harbour(&map, starts[j], 1_400.0)
            .iter()
            .map(|&(x, y)| steps[(y * w + x) as usize])
            .min()
            .filter(|&n| n != u32::MAX)
            .map(km)
    };
    for i in 0..8 {
        for j in 0..8 {
            if i == j {
                continue;
            }
            match (by_sea(i, j), by_sea(i ^ 1, j ^ 1)) {
                (Some(a), Some(b)) => {
                    if (a - b).abs() > 0.12 * a.max(b) + 0.3 {
                        problems.push(format!(
                            "start {i} to {j} by sea is {a:.1} km, its twins' {b:.1} km"
                        ));
                    }
                }
                _ => problems.push(format!("ships cannot sail from start {i} to start {j}")),
            }
        }
    }
    if let (Some(a), Some(b)) = (by_sea(0, 1), by_sea(2, 3)) {
        println!("by sea: start 0 to its twin {a:.1} km, start 2 to its twin {b:.1} km");
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_axis_holds_to_its_design() {
    let path = std::env::var("MC_CHECK_AXIS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maps/the_axis.mcmap")
        });
    if !path.exists() {
        eprintln!(
            "{} is not baked; skipping (see mc-bake's header for the command)",
            path.display()
        );
        return;
    }
    check(&path);
}
