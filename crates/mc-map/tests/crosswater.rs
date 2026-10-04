//! Holds "Crosswater" (4-player free for all, `mc-bake --layout crosswater`)
//! to its promise: every player plays the same map. On the baked terrain, by
//! the simulation's own rules:
//!
//! * the four starts are the first turned a quarter at a time;
//! * every start can walk to every other, round the lake as well as over the
//!   fords, and the walkable ground is the same turned;
//! * each start's walk to every ore field is as long as the turned start's to
//!   the turned field, and every field (the island's too) can be walked to;
//! * the sea is one: the bays and the sea round the map are joined, so ships
//!   can sail round it; the lake is too shallow for them;
//! * every player has the same timber.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test crosswater -- --nocapture`

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
    fn flood(&self, from: (u32, u32), ok: impl Fn(u32, u32) -> bool) -> Vec<i32> {
        let (w, h) = (self.w, self.h);
        let mut d = vec![-1i32; (w * h) as usize];
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
    /// The cell a quarter turn counter-clockwise from `(cx, cy)`.
    fn turned(&self, (cx, cy): (u32, u32)) -> (u32, u32) {
        (self.w - 1 - cy, cx)
    }
}

#[test]
fn crosswater_plays_the_same_for_every_player() {
    let stem = "crosswater";
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
    let mid = w as f64 * cell / 2.0;
    let turn = |p: (f64, f64)| (2.0 * mid - p.1, p.0);
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 4);
    for i in 1..4 {
        let want = turn(starts[i - 1]);
        if (starts[i].0 - want.0).abs() > 1.0 || (starts[i].1 - want.1).abs() > 1.0 {
            problems.push(format!("start {i} is not start {} turned", i - 1));
        }
    }

    let walks: Vec<Vec<i32>> = starts.iter().map(|&s| map.walk(s)).collect();
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    for (i, &s) in starts.iter().enumerate() {
        if at(&walks[0], s) < 0 {
            problems.push(format!("start {i} cannot be walked to from start 0"));
        }
    }
    // Round the lake: with the lake, the island and the fords' landings
    // closed, the neighbours still meet by land (through the pass, or by the
    // road along the shore).
    let shore = map.flood(map.cell_of(starts[0]), |x, y| {
        let (px, py) = ((x as f64 + 0.5) * cell - mid, (y as f64 + 0.5) * cell - mid);
        map.land_cell(x, y) && px.hypot(py) > 2_300.0
    });
    if at(&shore, starts[1]) < 0 || at(&shore, starts[3]) < 0 {
        problems.push("the neighbours meet only over the island".into());
    }

    let (mut reached, mut odd) = (0usize, 0usize);
    for cy in 0..h {
        for cx in 0..w {
            let a = walks[0][(cy * w + cx) as usize] >= 0;
            let (tx, ty) = map.turned((cx, cy));
            let b = walks[0][(ty * w + tx) as usize] >= 0;
            reached += a as usize;
            odd += (a != b) as usize;
        }
    }
    println!(
        "{stem}: walkable {:.1} km², {odd} cells without a turned twin",
        reached as f64 * cell * cell / 1e6
    );
    if odd as f64 > 0.002 * reached as f64 {
        problems.push(format!("{odd} walkable cells have no turned twin"));
    }

    for region in file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        for i in 0..4 {
            let (a, b) = (at(&walks[i], c), at(&walks[(i + 1) % 4], turn(c)));
            if a < 0 || b < 0 {
                problems.push(format!(
                    "the ore field at {c:?} or its twin is out of reach"
                ));
            } else if (a - b).abs() as f64 > 0.01 * a.max(b) as f64 + 4.0 {
                problems.push(format!(
                    "ore at {c:?}: {a} cells from start {i}, twin {b} from start {}",
                    (i + 1) % 4
                ));
            }
        }
    }
    println!("{stem}: {} ore fields", file.ore_regions().len());

    // One sea, round the map and into every bay; sea stacks may close off a
    // scrap of water, nothing bigger. The lake is no sea.
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
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    println!(
        "{stem}: seas {:?} km²",
        seas.iter().take(6).map(|&n| km2(n)).collect::<Vec<_>>()
    );
    if seas.len() > 1 && km2(seas[1]) > 0.1 {
        problems.push(format!("a second sea of {:.2} km²", km2(seas[1])));
    }
    let (lx, ly) = map.cell_of((mid - 900.0, mid));
    if map.sea_cell(lx, ly) {
        problems.push("the lake is deep enough for ships".into());
    }

    // Timber in each quarter.
    let mut timber = [0.0f64; 4];
    for p in file.props().iter().filter(|p| p.kind.is_tree()) {
        let (x, y) = (p.pos.x.to_f64() - mid, p.pos.y.to_f64() - mid);
        let s = p.scale_milli as f64 / 1000.0;
        let quarter = match (x < 0.0, y <= 0.0) {
            (true, true) => 0,
            (false, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        };
        timber[quarter] += s * s * s;
    }
    let (lo, hi) = (
        timber.iter().cloned().fold(f64::INFINITY, f64::min),
        timber.iter().cloned().fold(0.0, f64::max),
    );
    let skew = (hi - lo) / hi;
    println!(
        "{stem}: timber per quarter {:?} ({:.2}% apart)",
        timber.map(|t| t.round()),
        100.0 * skew
    );
    if skew > 0.03 {
        problems.push(format!("one quarter has {:.1}% more timber", 100.0 * skew));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
