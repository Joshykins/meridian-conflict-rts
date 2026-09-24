//! Holds the alpine maps, "Serac Divide" (1v1, `mc-bake --layout alpine`)
//! and "Serac Sound" (4v4, `--layout alpine-teams`), to their promise: they
//! look different from one side to the other but play the same from both.
//! On the baked terrain, by the simulation's own rules:
//!
//! * the ground a unit can walk to from each start is the mirror (across the
//!   middle line) of what it can walk to from its twin on the other side;
//! * nobody can walk onto a glacier or up onto a massif;
//! * each start's walk to every ore field is as long as its twin's to its twin;
//! * all the sea is one: a ship can sail anywhere deep from anywhere deep;
//! * the woods, which are not mirrored, hold about as much timber each side.
//!
//! Maps are not checked in; without the files the test says so and passes.
//!
//! `cargo test --release -p mc-map --test serac -- --nocapture`

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
    /// Distance in cells (4-neighbour) from `from` through cells `ok` takes, -1 unreached.
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
    /// Glacier ice under a cell, 0 to 1.
    fn ice(&self, cx: u32, cy: u32) -> f64 {
        let Some(snow) = self.file.snow() else { return 0.0 };
        let (sw, _) = self.file.info().snow_dims();
        snow[(((cy / 2) * sw + cx / 2) * 2) as usize] as f64 / 255.0
    }
}

fn load(stem: &str) -> Option<Map> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../maps/{stem}.mcmap"));
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return None;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    Some(Map { hf, file, w, h })
}

fn check(stem: &str) -> Vec<String> {
    let Some(map) = load(stem) else { return Vec::new() };
    let cell = CELL_SIZE_M as f64;
    let (w, h) = (map.w, map.h);
    let size = (w as f64 * cell, h as f64 * cell);
    let mirror = |p: (f64, f64)| (p.0, size.1 - p.1);
    let mut problems = Vec::new();
    let starts: Vec<(f64, f64)> = map.file.start_positions().iter().map(|p| (p.x.to_f64(), p.y.to_f64())).collect();
    // Starts come in pairs: south, then its mirror in the north.
    for i in (0..starts.len()).step_by(2) {
        let (a, b) = (starts[i], starts[i + 1]);
        if (b.0 - mirror(a).0).abs() > 1.0 || (b.1 - mirror(a).1).abs() > 1.0 {
            problems.push(format!("{stem}: start {} is not start {i} mirrored", i + 1));
        }
    }
    let half = 1;

    // Walkable ground from the first start, against its mirror.
    let walk0 = map.walk(starts[0]);
    let walk1 = map.walk(starts[half]);
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    if at(&walk0, starts[half]) <= 0 {
        problems.push(format!("{stem}: no way on foot from one side to the other"));
    }
    for (i, &s) in starts.iter().enumerate() {
        if at(&walk0, s) < 0 {
            problems.push(format!("{stem}: start {i} cannot be walked to from start 0"));
        }
    }
    let (mut reached, mut odd, mut high, mut on_ice) = (0usize, 0usize, 0usize, 0usize);
    let mut highest: f64 = 0.0;
    for cy in 0..h {
        for cx in 0..w {
            let a = walk0[(cy * w + cx) as usize] >= 0;
            let b = walk0[((h - 1 - cy) * w + cx) as usize] >= 0;
            if a {
                reached += 1;
                let z = map.cell_low(cx, cy);
                highest = highest.max(z);
                high += (z > 160.0) as usize;
                on_ice += (map.ice(cx, cy) > 0.3) as usize;
            }
            odd += (a != b) as usize;
        }
    }
    let odd_share = odd as f64 / reached as f64;
    println!(
        "{stem}: walkable {:.1} km², {odd} cells ({:.2}%) without a mirrored twin; highest walkable ground {highest:.0} m",
        reached as f64 * cell * cell / 1e6,
        100.0 * odd_share
    );
    if odd_share > 0.005 {
        problems.push(format!("{stem}: {:.2}% of the walkable ground has no mirrored twin", 100.0 * odd_share));
    }
    if high > 0 {
        problems.push(format!("{stem}: {high} walkable cells stand higher than 160 m: a way up a massif"));
    }
    if on_ice > 0 {
        problems.push(format!("{stem}: {on_ice} walkable cells are glacier ice"));
    }

    // Every ore field, from start 0 and its twin from start 0's mirror.
    for region in map.file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        let (a, b) = (at(&walk0, c), at(&walk1, mirror(c)));
        if a < 0 && b < 0 {
            continue; // an island field: for ships and hovers
        }
        if a < 0 || b < 0 {
            problems.push(format!("{stem}: the ore field at {c:?} or its twin is out of reach"));
        } else if (a - b).abs() as f64 > 0.02 * a.max(b) as f64 + 4.0 {
            problems.push(format!("{stem}: the ore field at {c:?} is {a} cells' walk away, its twin {b}"));
        }
    }

    // One sea.
    let mut first = None;
    let mut deep = 0usize;
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) {
                deep += 1;
                first.get_or_insert((cx, cy));
            }
        }
    }
    if let Some(from) = first {
        let sail = map.flood(from, |x, y| map.sea_cell(x, y));
        let sailed = sail.iter().filter(|&&d| d >= 0).count();
        let cut_off = deep - sailed;
        println!("{stem}: {:.1} km² of sea, {cut_off} cells cut off", deep as f64 * cell * cell / 1e6);
        if cut_off as f64 > 0.002 * deep as f64 {
            problems.push(format!("{stem}: {cut_off} cells of deep water cannot be sailed to"));
        }
    }

    // Timber per side.
    let mut timber = [0.0f64; 2];
    for p in map.file.props().iter().filter(|p| p.kind.is_tree()) {
        let (cx, cy) = map.cell_of((p.pos.x.to_f64(), p.pos.y.to_f64()));
        if walk0[(cy * w + cx) as usize] >= 0 {
            let s = p.scale_milli as f64 / 1000.0;
            timber[(cy > h / 2) as usize] += s * s * s;
        }
    }
    let skew = (timber[0] - timber[1]).abs() / timber[0].max(timber[1]);
    println!("{stem}: timber each side {:.0} / {:.0} ({:.1}% apart)", timber[0], timber[1], 100.0 * skew);
    if skew > 0.12 {
        problems.push(format!("{stem}: one side has {:.0}% more timber", 100.0 * skew));
    }
    problems
}

#[test]
fn the_alpine_maps_play_the_same_from_both_sides() {
    let mut problems = check("serac_divide");
    problems.extend(check("serac_sound"));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
