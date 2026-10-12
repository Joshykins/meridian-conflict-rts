//! Holds "Erebus Strait" (2v2 across a polar sea, `mc-bake --layout
//! antarctic`) to its promise: both teams play the same map. On the baked
//! terrain, by the simulation's own rules:
//!
//! * the starts come in pairs, each the one before it turned half round, so
//!   the teams (slot % 2) hold one coast each;
//! * allies can walk to each other two ways, along the shore strip and over
//!   the valley's foot, or behind the mountains over the ice sheet; the
//!   teams cannot walk to each other at all;
//! * each start's walk to every ore field on its coast is as long as the
//!   turned start's to the turned field;
//! * the islands' fields are dry and ships reach them; the sea is one;
//! * nothing grows.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test antarctic -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_map::{Heightfield, MapFile, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

struct Map {
    hf: Heightfield,
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
    /// The sim's rule: dry, slope at most 1/2.
    fn land_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) > 0.0 && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
    }
    /// The sim's rule for ships: 6 m deep or more.
    fn sea_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) <= -6.0
    }
    fn cell_of(&self, p: (f64, f64)) -> (u32, u32) {
        self.hf.cell_at(fx(p))
    }
    fn centre(&self, cx: u32, cy: u32) -> (f64, f64) {
        let cell = CELL_SIZE_M as f64;
        ((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell)
    }
    fn flood(&self, from: (u32, u32), ok: impl Fn(u32, u32) -> bool) -> Vec<i32> {
        let (w, h) = (self.w, self.h);
        let mut d = vec![-1i32; (w * h) as usize];
        if !ok(from.0, from.1) {
            return d;
        }
        d[(from.1 * w + from.0) as usize] = 0;
        let mut queue = VecDeque::from([from]);
        while let Some((cx, cy)) = queue.pop_front() {
            let here = d[(cy * w + cx) as usize];
            for (nx, ny) in [
                (cx + 1, cy),
                (cx.wrapping_sub(1), cy),
                (cx, cy + 1),
                (cx, cy.wrapping_sub(1)),
            ] {
                if nx >= w || ny >= h || d[(ny * w + nx) as usize] >= 0 || !ok(nx, ny) {
                    continue;
                }
                d[(ny * w + nx) as usize] = here + 1;
                queue.push_back((nx, ny));
            }
        }
        d
    }
    fn walk(&self, from: (f64, f64)) -> Vec<i32> {
        self.flood(self.cell_of(from), |x, y| self.land_cell(x, y))
    }
}

#[test]
fn erebus_strait_plays_the_same_for_both_teams() {
    let stem = std::env::var("ANTARCTIC_MAP").unwrap_or_else(|_| "erebus_strait".into());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../maps/{stem}.mcmap"));
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    assert_eq!(w, h);
    let map = Map { hf, w, h };
    let cell = CELL_SIZE_M as f64;
    let size = w as f64 * cell;
    let mid = size / 2.0;
    let turn = |p: (f64, f64)| (size - p.0, size - p.1);
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 4);
    for i in [1, 3] {
        let want = turn(starts[i - 1]);
        if (starts[i].0 - want.0).abs() > 1.0 || (starts[i].1 - want.1).abs() > 1.0 {
            problems.push(format!("start {i} is not start {} turned", i - 1));
        }
    }
    if starts[0].0 > mid || starts[2].0 > mid {
        problems.push("the first team's starts are not both on the west coast".into());
    }

    let walks: Vec<Vec<i32>> = starts.iter().map(|&s| map.walk(s)).collect();
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    if at(&walks[0], starts[2]) < 0 || at(&walks[1], starts[3]) < 0 {
        problems.push("allies cannot walk to each other".into());
    }
    if at(&walks[0], starts[1]) >= 0 || at(&walks[0], starts[3]) >= 0 {
        problems.push("the teams can walk to each other".into());
    }
    // Two ways between allies: with the ice sheet's top closed (60 m up),
    // the shore across the valley's foot; with all the low ground east of the
    // starts closed, over the sheet behind the mountains.
    for (name, open) in [
        (
            "along the shore",
            Box::new(|x: u32, y: u32| map.cell_low(x, y) < 60.0) as Box<dyn Fn(u32, u32) -> bool>,
        ),
        (
            "over the ice sheet",
            Box::new(|x: u32, y: u32| map.centre(x, y).0 < 2_300.0 || map.cell_low(x, y) > 60.0),
        ),
    ] {
        let walk = map.flood(map.cell_of(starts[0]), |x, y| {
            map.land_cell(x, y) && open(x, y)
        });
        if at(&walk, starts[2]) < 0 {
            problems.push(format!("allies cannot meet {name}"));
        }
    }

    // Each side's ice sheet edges and far coasts are its own shape, so the
    // walkable ground need not match cell for cell: its area must, and round
    // the starts (2.5 km) it must match cell for cell.
    let (mut west, mut east, mut home, mut odd) = (0usize, 0usize, 0usize, 0usize);
    for cy in 0..h {
        for cx in 0..w {
            let a = walks[0][(cy * w + cx) as usize] >= 0;
            let b = walks[1][((h - 1 - cy) * w + (w - 1 - cx)) as usize] >= 0;
            west += a as usize;
            east += b as usize;
            let p = map.centre(cx, cy);
            let (d0, d2) = (
                (p.0 - starts[0].0).hypot(p.1 - starts[0].1),
                (p.0 - starts[2].0).hypot(p.1 - starts[2].1),
            );
            if d0.min(d2) < 1_600.0 {
                home += a as usize;
                odd += (a != b) as usize;
            }
        }
    }
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    let apart = (west as f64 - east as f64).abs() / west.max(east) as f64;
    println!(
        "{stem}: walkable {:.2} vs {:.2} km² ({:.2}% apart); round the starts {odd} of {home} cells without a turned twin",
        km2(west),
        km2(east),
        100.0 * apart
    );
    if apart > 0.02 {
        problems.push(format!(
            "one coast has {:.1}% more walkable ground",
            100.0 * apart
        ));
    }
    if odd as f64 > 0.005 * home as f64 {
        problems.push(format!(
            "{odd} walkable cells round the starts have no turned twin"
        ));
    }

    let sea = {
        let (cx, cy) = map.cell_of((mid, mid));
        map.flood((cx, cy), |x, y| map.sea_cell(x, y))
    };
    let mut fields = 0;
    for region in file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        let (cx, cy) = map.cell_of(c);
        if !map.land_cell(cx, cy) {
            problems.push(format!("the ore field at {c:?} is not dry, level ground"));
            continue;
        }
        if c.0 > mid {
            continue;
        }
        fields += 1;
        // A coast's field: walked from the nearer start of the team, and the
        // twin from that start's twin.
        let (a, b) = [(0, 1), (2, 3)]
            .iter()
            .map(|&(i, j)| (at(&walks[i], c), at(&walks[j], turn(c))))
            .filter(|&(a, _)| a >= 0)
            .min()
            .unwrap_or((-1, -1));
        if a < 0 {
            // An island's: walking from it reaches a shore ships can come to.
            let walk = map.walk(c);
            let landing = (0..h).any(|y| {
                (0..w).any(|x| {
                    walk[(y * w + x) as usize] >= 0
                        && [(4, 0), (-4, 0), (0, 4), (0, -4)].iter().any(|&(dx, dy)| {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            nx >= 0
                                && ny >= 0
                                && (nx as u32) < w
                                && (ny as u32) < h
                                && sea[(ny as u32 * w + nx as u32) as usize] >= 0
                        })
                })
            });
            if !landing {
                problems.push(format!("ore at {c:?} is out of reach by land and by sea"));
            }
        } else if b < 0 || (a - b).abs() as f64 > 0.03 * a.max(b) as f64 + 6.0 {
            problems.push(format!("ore at {c:?}: {a} cells away, its twin {b}"));
        }
    }
    println!(
        "{stem}: {} ore fields, {fields} west of the middle",
        file.ore_regions().len()
    );

    // One sea; bergs and the calderas' rims may close off a scrap of water,
    // nothing bigger.
    let mut seen = vec![false; (w * h) as usize];
    let mut seas = Vec::new();
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) && !seen[(cy * w + cx) as usize] {
                let sail = map.flood((cx, cy), |x, y| map.sea_cell(x, y));
                let mut n = 0usize;
                for (i, &d) in sail.iter().enumerate() {
                    if d >= 0 {
                        seen[i] = true;
                        n += 1;
                    }
                }
                seas.push(n);
            }
        }
    }
    seas.sort_unstable_by(|a, b| b.cmp(a));
    println!(
        "{stem}: seas {:?} km²",
        seas.iter().take(6).map(|&n| km2(n)).collect::<Vec<_>>()
    );
    if seas.len() > 1 && km2(seas[1]) > 0.1 {
        problems.push(format!("a second sea of {:.2} km²", km2(seas[1])));
    }
    if file.props().iter().any(|p| p.kind.is_tree()) {
        problems.push("trees grow on Antarctica".into());
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
