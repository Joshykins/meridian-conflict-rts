//! Holds "Vermilion Gorge" (3v3, `mc-bake --layout canyon`) to its promise:
//! the two sides play the same, though the canyon's outlines are each side's
//! own. On the baked terrain, by the simulation's own rules:
//!
//! * starts come in pairs, the second the first mirrored across the middle;
//! * every start can walk to every other, over the dam's crest or the ford,
//!   and still can with either crossing cut;
//! * the level ground each side can reach is the same within 1.5 %, and the
//!   walkable ground mirrors but for the talus round each side's own temples;
//! * each start's walk to every ore field is as long as its twin's to the
//!   twin field, within 4 % (the long walks along a bench pass shores that
//!   are each side's own);
//! * the lake is one sea, and ships from each side's coves sail as far to
//!   every island's ore as the other side's;
//! * both sides have the same timber, within a few per cent.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test vermilion_gorge -- --nocapture`

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
fn vermilion_gorge_plays_the_same_from_both_sides() {
    let stem = "vermilion_gorge";
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
    let mirror = |p: (f64, f64)| (size.0 - p.0, p.1);
    let mirror_cell = |cx: u32, cy: u32| (cy * w + (w - 1 - cx)) as usize;
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 6);
    for i in (0..6).step_by(2) {
        let (a, b) = (starts[i], starts[i + 1]);
        if (b.0 - mirror(a).0).abs() > 1.0 || (b.1 - mirror(a).1).abs() > 1.0 {
            problems.push(format!("start {} is not start {i} mirrored", i + 1));
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
    // `VG_WALK=out.ppm`: where start 0 can walk (green), land it cannot reach
    // (yellow), too steep (red), shallow (cyan) and deep water (blue).
    if let Ok(out) = std::env::var("VG_WALK") {
        let mut img = format!("P6 {w} {h} 255\n").into_bytes();
        for cy in (0..h).rev() {
            for cx in 0..w {
                let low = map.cell_low(cx, cy);
                let (a, b) = (
                    walks[0][(cy * w + cx) as usize] >= 0,
                    walks[0][mirror_cell(cx, cy)] >= 0,
                );
                let rgb = if a != b {
                    [255, 0, 255]
                } else if a {
                    [60, 170, 60]
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
    // Either crossing alone joins the sides: cut the dam's crest, then the ford.
    let middle = size.0 / 2.0;
    for (name, lo, hi) in [
        ("the dam", 1_900.0, 2_700.0),
        ("the ford", 9_000.0, 11_500.0),
    ] {
        let cut = map.flood(map.cell_of(starts[0]), |x, y| {
            let (px, py) = ((x as f64 + 0.5) * cell, (y as f64 + 0.5) * cell);
            map.land_cell(x, y) && !((px - middle).abs() < 40.0 && py > lo && py < hi)
        });
        if at(&cut, starts[1]) < 0 {
            problems.push(format!("with {name} cut the sides cannot meet"));
        }
    }
    let (mut reached, mut odd) = (0usize, 0usize);
    for cy in 0..h {
        for cx in 0..w {
            let a = walks[0][(cy * w + cx) as usize] >= 0;
            let b = walks[0][mirror_cell(cx, cy)] >= 0;
            reached += a as usize;
            odd += (a != b) as usize;
        }
    }
    println!(
        "{stem}: walkable {:.1} km², {odd} cells ({:.2}%) without a mirrored twin",
        reached as f64 * cell * cell / 1e6,
        100.0 * odd as f64 / reached as f64
    );
    // The temples standing off the walls are each side's own: the talus
    // round them is walkable here and not there. It leads nowhere; what
    // must match is the level ground each side can reach and build on (and
    // evening that out moves each side's open shore a little, `canyon.rs`).
    if odd as f64 > 0.09 * reached as f64 {
        problems.push(format!("{odd} walkable cells have no mirrored twin"));
    }
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
    if level_skew > 0.015 {
        problems.push(format!(
            "one side has {:.1}% more level ground",
            100.0 * level_skew
        ));
    }
    let mut island = 0;
    for region in map.file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        for i in (0..6).step_by(2) {
            let (a, b) = (at(&walks[i], c), at(&walks[i + 1], mirror(c)));
            if a < 0 && b < 0 {
                island += (i == 0) as usize;
                continue;
            }
            if a < 0 || b < 0 {
                problems.push(format!(
                    "the ore field at {c:?} or its twin is out of reach"
                ));
            } else if (a - b).abs() as f64 > 0.04 * a.max(b) as f64 + 4.0 {
                problems.push(format!(
                    "ore at {c:?}: {a} cells from start {i}, twin {b} from start {}",
                    i + 1
                ));
            }
        }
    }
    println!(
        "{stem}: {} ore fields, {island} for ships and hovers only",
        map.file.ore_regions().len()
    );

    // One lake: the biggest sea holds nearly all the deep water.
    let mut seen = vec![false; (w * h) as usize];
    let mut seas = Vec::new();
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) && !seen[(cy * w + cx) as usize] {
                let sail = map.flood((cx, cy), |x, y| map.sea_cell(x, y));
                let n = sail.iter().filter(|&&d| d >= 0).count();
                for (i, &d) in sail.iter().enumerate() {
                    seen[i] |= d >= 0;
                }
                seas.push(n);
            }
        }
    }
    seas.sort_unstable_by(|a, b| b.cmp(a));
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    println!(
        "{stem}: seas {:?} km²",
        seas.iter()
            .map(|&n| (km2(n) * 10.0).round() / 10.0)
            .collect::<Vec<_>>()
    );
    if seas[1..].iter().sum::<usize>() as f64 > 0.01 * seas[0] as f64 {
        problems.push("deep water cut off from the lake".into());
    }
    // Each side's harbours (its three arm coves) sail to every island field
    // as far as the other side's do to the twin.
    let coves = [(3_894.0, 3_900.0), (3_744.0, 6_140.0), (3_994.0, 8_040.0)];
    let sail_from = |p: (f64, f64)| {
        // The nearest deep water to a cove's beach.
        let (cx, cy) = map.cell_of(p);
        let mut best = None;
        for r in 0..120i32 {
            for (dx, dy) in (-r..=r).flat_map(|dx| [(dx, -r), (dx, r), (-r, dx), (r, dx)]) {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x >= 0
                    && y >= 0
                    && (x as u32) < w
                    && (y as u32) < h
                    && map.sea_cell(x as u32, y as u32)
                {
                    best = Some((x as u32, y as u32));
                }
            }
            if best.is_some() {
                break;
            }
        }
        map.flood(best.expect("a cove on the lake"), |x, y| map.sea_cell(x, y))
    };
    for &cove in &coves {
        let (a, b) = (sail_from(cove), sail_from(mirror(cove)));
        for region in map.file.ore_regions() {
            let c = region.centre();
            let c = (c.x.to_f64(), c.y.to_f64());
            if at(&walks[0], c) >= 0 {
                continue;
            }
            // The deep water nearest the field and its twin.
            let near = |sail: &[i32], p: (f64, f64)| {
                let (cx, cy) = map.cell_of(p);
                let mut best = i32::MAX;
                for dy in -40i32..=40 {
                    for dx in -40i32..=40 {
                        let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                            let d = sail[(y as u32 * w + x as u32) as usize];
                            if d >= 0 {
                                best = best.min(d + dx.abs() + dy.abs());
                            }
                        }
                    }
                }
                best
            };
            let (da, db) = (near(&a, c), near(&b, mirror(c)));
            if da == i32::MAX || db == i32::MAX {
                problems.push(format!(
                    "island ore at {c:?} cannot be sailed to from the cove at {cove:?}"
                ));
            } else if (da - db).abs() as f64 > 0.12 * da.max(db) as f64 + 10.0 {
                problems.push(format!(
                    "island ore at {c:?}: {da} sailing from {cove:?}, twin {db}"
                ));
            }
        }
    }

    // Timber each side of the middle.
    let mut timber = [0.0f64; 2];
    for p in map.file.props().iter().filter(|p| p.kind.is_tree()) {
        let x = p.pos.x.to_f64();
        let s = p.scale_milli as f64 / 1000.0;
        timber[(x > middle) as usize] += s * s * s;
    }
    let skew = (timber[0] - timber[1]).abs() / timber[0].max(timber[1]);
    println!(
        "{stem}: timber each side {:.0} / {:.0} ({:.2}% apart)",
        timber[0],
        timber[1],
        100.0 * skew
    );
    if skew > 0.05 {
        problems.push(format!("one side has {:.1}% more timber", 100.0 * skew));
    }
    // The dam stands on its crest.
    let dam = map
        .file
        .props()
        .iter()
        .find(|p| p.kind == mc_map::PropKind::Dam)
        .expect("the dam");
    let crest = map.hf.height_at(dam.pos) - map.hf.water_level();
    if (crest.to_f64() - mc_map::landmark::DAM.crest_z).abs() > 0.01 {
        problems.push(format!("the dam's crest is at {crest:?}"));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
