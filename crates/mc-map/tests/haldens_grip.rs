//! Holds "Halden's Grip" (4v4, `mc-bake --layout twin-bays`) to its promise:
//! the two sides play the same. On the baked terrain, by the simulation's
//! own rules:
//!
//! * starts come in pairs, the second the first turned half round the centre;
//! * every start can walk to every other, and the walkable ground is the same
//!   turned;
//! * each start's walk to every ore field is as long as its twin's to the
//!   twin field;
//! * each bay is one sea that ships can cross end to end, and the two bays
//!   do not meet (the land bridge runs corner to corner);
//! * both sides have the same timber.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test haldens_grip -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_map::{Heightfield, MapFile, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

struct Map {
    hf: Heightfield,
    file: MapFile,
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
            for (nx, ny) in [(cx + 1, cy), (cx.wrapping_sub(1), cy), (cx, cy + 1), (cx, cy.wrapping_sub(1))] {
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
fn haldens_grip_plays_the_same_from_both_sides() {
    let stem = "haldens_grip";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../maps/{stem}.mcmap"));
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    let map = Map { hf, file, w, h };
    let cell = CELL_SIZE_M as f64;
    let size = (w as f64 * cell, h as f64 * cell);
    let turn = |p: (f64, f64)| (size.0 - p.0, size.1 - p.1);
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> =
        map.file.start_positions().iter().map(|p| (p.x.to_f64(), p.y.to_f64())).collect();
    assert_eq!(starts.len(), 8);
    for i in (0..8).step_by(2) {
        let (a, b) = (starts[i], starts[i + 1]);
        if (b.0 - turn(a).0).abs() > 1.0 || (b.1 - turn(a).1).abs() > 1.0 {
            problems.push(format!("start {} is not start {i} turned", i + 1));
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
    let (mut reached, mut odd) = (0usize, 0usize);
    for cy in 0..h {
        for cx in 0..w {
            let a = walks[0][(cy * w + cx) as usize] >= 0;
            let b = walks[0][((h - 1 - cy) * w + (w - 1 - cx)) as usize] >= 0;
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
    // From start 1's twin (start 0) and so on: the walk to the one side's
    // ore is as long as the twin's walk to the other's.
    let mut island = 0;
    for region in map.file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        for i in (0..8).step_by(2) {
            let (a, b) = (at(&walks[i], c), at(&walks[i + 1], turn(c)));
            if a < 0 && b < 0 {
                island += (i == 0) as usize;
                continue;
            }
            if a < 0 || b < 0 {
                problems.push(format!("the ore field at {c:?} or its twin is out of reach"));
            } else if (a - b).abs() as f64 > 0.01 * a.max(b) as f64 + 4.0 {
                problems.push(format!("ore at {c:?}: {a} cells from start {i}, twin {b} from start {}", i + 1));
            }
        }
    }
    println!("{stem}: {} ore fields, {island} for ships and hovers only", map.file.ore_regions().len());
    if island != 2 {
        problems.push(format!("{island} ore fields cannot be walked to; the two island fields should be the only ones"));
    }

    // Two seas, one in each bay, each crossable end to end.
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
    println!("{stem}: seas {:?} km²", seas.iter().map(|&n| km2(n).round()).collect::<Vec<_>>());
    if seas.len() < 2 || seas[0] != seas[1] {
        problems.push("the two bays are not two seas of the same size".into());
    } else if seas[2..].iter().sum::<usize>() as f64 > 0.002 * seas[0] as f64 {
        problems.push("deep water cut off from both bays".into());
    }

    // Timber each side of the diagonal.
    let mut timber = [0.0f64; 2];
    for p in map.file.props().iter().filter(|p| p.kind.is_tree()) {
        let (x, y) = (p.pos.x.to_f64(), p.pos.y.to_f64());
        let s = p.scale_milli as f64 / 1000.0;
        timber[(x + y > size.0) as usize] += s * s * s;
    }
    let skew = (timber[0] - timber[1]).abs() / timber[0].max(timber[1]);
    println!("{stem}: timber each side {:.0} / {:.0} ({:.2}% apart)", timber[0], timber[1], 100.0 * skew);
    if skew > 0.03 {
        problems.push(format!("one side has {:.1}% more timber", 100.0 * skew));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
