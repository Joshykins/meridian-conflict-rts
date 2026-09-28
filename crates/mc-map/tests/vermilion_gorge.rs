//! Holds "Vermilion Gorge" (3v3, `mc-bake --layout canyon`) to its promise:
//! the two sides play alike, though each is its own shape (the east's design
//! is the west's displaced along the canyon, and all the noise is each
//! side's own). On the baked terrain, by the simulation's own rules:
//!
//! * starts come in pairs, the east's near the west's mirror image;
//! * every start can walk to every other; with the dry valley below the dam
//!   cut, or the ford at the north, the sides still meet; nothing walks
//!   through the dam;
//! * each side can reach and build on the same level ground, within 2 %;
//! * each start's walk to every ore field is within 6 % of its twin's walk
//!   to the twin field, and each isle's ore lies as far offshore from its
//!   side's walkable shore as its twin, within a quarter;
//! * the lake is one sea;
//! * both sides have the same timber, within 6 %.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test vermilion_gorge -- --nocapture`
//! (`VG_WALK=out.ppm` also writes where start 0 can walk).

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
    /// Cells a prop's solid plan covers (the dam).
    solid: Vec<bool>,
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
        self.cell_low(cx, cy) > 0.0
            && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
            && !self.solid[(cy * self.w + cx) as usize]
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
}

#[test]
fn vermilion_gorge_plays_alike_from_both_sides() {
    let stem = "vermilion_gorge";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../maps/{stem}.mcmap"));
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    let mut solid = vec![false; (w * h) as usize];
    for p in file.props() {
        for (y, a, b) in p.solid_runs((w, h)) {
            for x in a..=b {
                solid[(y * w + x) as usize] = true;
            }
        }
    }
    let map = Map {
        hf,
        file,
        w,
        h,
        solid,
    };
    let cell = CELL_SIZE_M as f64;
    let size = (w as f64 * cell, h as f64 * cell);
    let middle = size.0 / 2.0;
    let mirror = |p: (f64, f64)| (size.0 - p.0, p.1);
    let dist = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 6);
    for i in (0..6).step_by(2) {
        if dist(starts[i + 1], mirror(starts[i])) > 400.0 {
            problems.push(format!(
                "start {} is far from start {i}'s mirror image",
                i + 1
            ));
        }
    }

    let walks: Vec<Vec<i32>> = starts.iter().map(|&s| map.walk(s)).collect();
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    // A field's walk: to its nearest walkable cell within 40 m of its middle.
    let to_field = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        let mut best = -1;
        for dy in -5i32..=5 {
            for dx in -5i32..=5 {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    let d = walk[(y as u32 * w + x as u32) as usize];
                    if d >= 0 && (best < 0 || d < best) {
                        best = d;
                    }
                }
            }
        }
        best
    };
    for (i, &s) in starts.iter().enumerate() {
        if at(&walks[0], s) < 0 {
            problems.push(format!("start {i} cannot be walked to from start 0"));
        }
    }
    if let Ok(out) = std::env::var("VG_WALK") {
        let mut img = format!("P6 {w} {h} 255\n").into_bytes();
        for cy in (0..h).rev() {
            for cx in 0..w {
                let low = map.cell_low(cx, cy);
                let rgb = if walks[0][(cy * w + cx) as usize] >= 0 {
                    [60, 170, 60]
                } else if map.solid[(cy * w + cx) as usize] {
                    [255, 0, 255]
                } else if map.land_cell(cx, cy) {
                    [230, 200, 40]
                } else if low > 0.0 {
                    [150, 50, 40]
                } else if low > -6.0 {
                    [80, 200, 220]
                } else {
                    [20, 50, 140]
                };
                img.extend(rgb);
            }
        }
        std::fs::write(out, img).unwrap();
    }
    // Either crossing alone joins the sides: cut the valley below the dam,
    // then the ford.
    for (name, lo, hi) in [
        ("the dry valley", -100.0, 2_300.0),
        ("the ford", 9_000.0, 11_500.0),
    ] {
        let cut = map.flood(map.cell_of(starts[0]), |x, y| {
            let (px, py) = ((x as f64 + 0.5) * cell, (y as f64 + 0.5) * cell);
            map.land_cell(x, y) && !((px - middle).abs() < 300.0 && py > lo && py < hi)
        });
        if at(&cut, starts[1]) < 0 {
            problems.push(format!("with {name} cut the sides cannot meet"));
        }
    }

    // Level ground each side can reach.
    let mut level = [0usize; 2];
    for cy in 0..h {
        for cx in 0..w {
            if walks[0][(cy * w + cx) as usize] >= 0 && map.hf.cell_slope(cx, cy) < Fx::ratio(1, 10)
            {
                level[(cx >= w / 2) as usize] += 1;
            }
        }
    }
    let level_skew = (level[0] as f64 - level[1] as f64).abs() / level[0].max(level[1]) as f64;
    println!(
        "{stem}: level ground each side {:.1} / {:.1} km² ({:.2}% apart)",
        level[0] as f64 * cell * cell / 1e6,
        level[1] as f64 * cell * cell / 1e6,
        100.0 * level_skew
    );
    if level_skew > 0.02 {
        problems.push(format!(
            "one side has {:.1}% more level ground",
            100.0 * level_skew
        ));
    }

    // Ore: each west field's twin is the east field nearest its mirror image.
    let fields: Vec<(f64, f64)> = map
        .file
        .ore_regions()
        .iter()
        .map(|r| {
            let c = r.centre();
            (c.x.to_f64(), c.y.to_f64())
        })
        .collect();
    let twin = |c: (f64, f64)| {
        *fields
            .iter()
            .filter(|f| f.0 > middle + 50.0)
            .min_by(|a, b| dist(**a, mirror(c)).total_cmp(&dist(**b, mirror(c))))
            .unwrap()
    };
    let mut island = Vec::new();
    for &c in fields.iter().filter(|f| f.0 < middle - 50.0) {
        let t = twin(c);
        if dist(t, mirror(c)) > 600.0 {
            problems.push(format!(
                "the ore at {c:?} has no twin near its mirror image"
            ));
        }
        for i in (0..6).step_by(2) {
            let (a, b) = (to_field(&walks[i], c), to_field(&walks[i + 1], t));
            if a < 0 && b < 0 {
                if i == 0 {
                    island.push((c, t));
                }
                continue;
            }
            if a < 0 || b < 0 {
                problems.push(format!(
                    "the ore at {c:?} ({a}) or its twin at {t:?} ({b}) is out of reach"
                ));
            } else if (a - b).abs() as f64 > 0.06 * a.max(b) as f64 + 8.0 {
                problems.push(format!(
                    "ore at {c:?}: {a} cells from start {i}, twin {t:?} {b} from start {}",
                    i + 1
                ));
            }
        }
    }
    println!(
        "{stem}: {} ore fields, {} pairs for ships and hovers only",
        fields.len(),
        island.len()
    );

    // One lake.
    let mut seen = vec![false; (w * h) as usize];
    let mut seas = Vec::new();
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) && !seen[(cy * w + cx) as usize] {
                let sail = map.flood((cx, cy), |x, y| map.sea_cell(x, y));
                for (i, &d) in sail.iter().enumerate() {
                    seen[i] |= d >= 0;
                }
                seas.push(sail.iter().filter(|&&d| d >= 0).count());
            }
        }
    }
    seas.sort_unstable_by(|a, b| b.cmp(a));
    println!(
        "{stem}: seas {:?} km²",
        seas.iter()
            .map(|&n| (n as f64 * cell * cell / 1e5).round() / 10.0)
            .collect::<Vec<_>>()
    );
    if seas[1..].iter().sum::<usize>() as f64 > 0.01 * seas[0] as f64 {
        problems.push("deep water cut off from the lake".into());
    }
    // The lake: the biggest sea.
    let lake = {
        let mut best: Vec<i32> = Vec::new();
        let mut count = 0;
        let mut seen = vec![false; (w * h) as usize];
        for cy in 0..h {
            for cx in 0..w {
                if map.sea_cell(cx, cy) && !seen[(cy * w + cx) as usize] {
                    let sail = map.flood((cx, cy), |x, y| map.sea_cell(x, y));
                    let n = sail.iter().filter(|&&d| d >= 0).count();
                    for (i, &d) in sail.iter().enumerate() {
                        seen[i] |= d >= 0;
                    }
                    if n > count {
                        (count, best) = (n, sail);
                    }
                }
            }
        }
        best
    };
    // Each isle's ore lies about as far offshore from its own side's
    // walkable shore as its twin does from the other's: sail from the field
    // until the water meets ground that side's bases can walk to.
    let offshore = |field: (f64, f64), walk: &[i32], east: bool| {
        // Water within 25 m of ground that side's bases walk to: a landing
        // (a beach or a cove; the gorge's walls stand wider than that).
        let mut landing = vec![false; (w * h) as usize];
        for cy in 0..h {
            for cx in 0..w {
                let at = (cy * w + cx) as usize;
                if walk[at] < 0 || ((cx as f64 + 0.5) * cell > middle) != east {
                    continue;
                }
                for dy in -3i32..=3 {
                    for dx in -3i32..=3 {
                        let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                            landing[(y as u32 * w + x as u32) as usize] = true;
                        }
                    }
                }
            }
        }
        let (cx, cy) = map.cell_of(field);
        let mut dist = vec![-1i32; (w * h) as usize];
        let mut queue = VecDeque::new();
        for dy in -60i32..=60 {
            for dx in -60i32..=60 {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    let at = (y as u32 * w + x as u32) as usize;
                    if dx == 0 && dy == 0 {
                        dist[at] = 0;
                        queue.push_back((x as u32, y as u32));
                    }
                }
            }
        }
        while let Some((x, y)) = queue.pop_front() {
            let here = dist[(y * w + x) as usize];
            for (nx, ny) in [
                (x + 1, y),
                (x.wrapping_sub(1), y),
                (x, y + 1),
                (x, y.wrapping_sub(1)),
            ] {
                if nx >= w || ny >= h {
                    continue;
                }
                let at = (ny * w + nx) as usize;
                // Over the isle itself to its shore, then over the lake.
                let on_isle = (nx as i32 - cx as i32).abs() + (ny as i32 - cy as i32).abs() <= 40;
                if (lake[at] < 0 && !on_isle) || dist[at] >= 0 {
                    continue;
                }
                if landing[at] {
                    return here + 1;
                }
                {
                    dist[at] = here + 1;
                    queue.push_back((nx, ny));
                }
            }
        }
        i32::MAX
    };
    for &(c, t) in &island {
        let (da, db) = (offshore(c, &walks[0], false), offshore(t, &walks[1], true));
        println!("{stem}: isle ore {c:?} {da} cells offshore, twin {db}");
        if da == i32::MAX || db == i32::MAX {
            problems.push(format!("the isle ore at {c:?} or its twin has no shore"));
        } else if (da - db).abs() as f64 > 0.25 * da.max(db) as f64 + 10.0 {
            problems.push(format!("isle ore at {c:?}: {da} cells offshore, twin {db}"));
        }
    }

    // Timber each side of the middle.
    let mut timber = [0.0f64; 2];
    for p in map.file.props().iter().filter(|p| p.kind.is_tree()) {
        let s = p.scale_milli as f64 / 1000.0;
        timber[(p.pos.x.to_f64() > middle) as usize] += s * s * s;
    }
    let skew = (timber[0] - timber[1]).abs() / timber[0].max(timber[1]);
    println!(
        "{stem}: timber each side {:.0} / {:.0} ({:.2}% apart)",
        timber[0],
        timber[1],
        100.0 * skew
    );
    if skew > 0.06 {
        problems.push(format!("one side has {:.1}% more timber", 100.0 * skew));
    }
    // The dam stands on its riverbed.
    let dam = map
        .file
        .props()
        .iter()
        .find(|p| p.kind == mc_map::PropKind::Dam)
        .expect("the dam");
    let floor = map.hf.height_at(dam.pos) - map.hf.water_level();
    if (floor.to_f64() - mc_map::landmark::GORGE_DAM.floor_z).abs() > 0.01 {
        problems.push(format!("the dam's toe is at {floor:?}"));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
