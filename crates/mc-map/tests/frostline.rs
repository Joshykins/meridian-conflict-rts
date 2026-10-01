//! Holds "Frostline" (4v4, `mc-bake --layout frostline`) to its promises. The
//! two sides are not the same to look at, each is its own country; they are
//! to play the same. On the baked terrain, by the simulation's own rules:
//!
//! * starts come in pairs, the second the first turned half round the centre,
//!   and each side's four stand on its own side of the wall;
//! * every start can walk to every other, and the two sides have the same
//!   ground to walk and build on, within a few percent;
//! * each start's walk to every ore field is as long as its twin's to the
//!   twin field, within a few percent, and only the islands' fields cannot
//!   be walked to;
//! * there are two oceans of about one size that do not meet, a ship crosses
//!   the wall in each, and each side has as much sea on its side of the wall;
//! * the four bases keep their roles, on both sides alike: the beach base has
//!   a strand close by, the cliff base's only way down to the water is its
//!   cove well away along the coast, the front has to go out onto the
//!   bridge, the rear is furthest of all from the middle and from the sea;
//! * the wall's towers stand on its line, a pair to every turn, and it parts
//!   the climates: snow only east of it, juniper, pinyon and cottonwood only
//!   west, spruce, pine and birch only east;
//! * both sides have the same timber and the same wreckage.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test frostline -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_map::landmark::FROSTLINE_WALL;
use mc_map::{Heightfield, MapFile, PropKind, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

/// The head of the west's cove, where its canyon comes out (`bake/frostline.rs`).
const COVE_HEAD: (f64, f64) = (3_784.0, 1_434.0);

/// Metres east of the wall.
fn east_of((x, y): (f64, f64)) -> f64 {
    let i = FROSTLINE_WALL
        .windows(2)
        .position(|w| y <= w[1].1)
        .unwrap_or(FROSTLINE_WALL.len() - 2);
    let (a, b) = (FROSTLINE_WALL[i], FROSTLINE_WALL[i + 1]);
    x - (a.0 + (b.0 - a.0) * ((y - a.1) / (b.1 - a.1)).clamp(0.0, 1.0))
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
    /// A strand: walkable ground at the water's edge, a ship's draught of
    /// water within reach of a slipway.
    fn strand(&self, cx: u32, cy: u32) -> bool {
        const REACH: u32 = 16;
        self.land_cell(cx, cy)
            && self.cell_low(cx, cy) < 4.0
            && [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| {
                    let (x, y) = (cx as i64 + dx * REACH as i64, cy as i64 + dy * REACH as i64);
                    x >= 0
                        && y >= 0
                        && (x as u32) < self.w
                        && (y as u32) < self.h
                        && self.sea_cell(x as u32, y as u32)
                })
    }
}

#[test]
fn frostline_plays_the_same_from_both_sides() {
    let stem = "frostline";
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
    let turn_if = |yes: bool, p: (f64, f64)| if yes { turn(p) } else { p };
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 8);
    for i in (0..8).step_by(2) {
        let (a, b) = (starts[i], starts[i + 1]);
        if (b.0 - turn(a).0).abs() > 1.0 || (b.1 - turn(a).1).abs() > 1.0 {
            problems.push(format!("start {} is not start {i} turned", i + 1));
        }
        if east_of(a) > 0.0 || east_of(b) < 0.0 {
            problems.push(format!(
                "starts {i} and {} are on the wrong sides of the wall",
                i + 1
            ));
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
    // Ground a unit can reach, each side of the wall.
    let mut ground = [0usize; 2];
    for cy in 0..h {
        for cx in 0..w {
            if walks[0][(cy * w + cx) as usize] >= 0 {
                let p = ((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell);
                ground[(east_of(p) > 0.0) as usize] += 1;
            }
        }
    }
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    let apart = |a: f64, b: f64| (a - b).abs() / a.max(b).max(1e-9);
    println!(
        "{stem}: ground to walk and build on: west {:.1} km², east {:.1} km² ({:.1}% apart)",
        km2(ground[0]),
        km2(ground[1]),
        100.0 * apart(ground[0] as f64, ground[1] as f64)
    );
    if apart(ground[0] as f64, ground[1] as f64) > 0.04 {
        problems.push("one side has over 4% more ground than the other".into());
    }

    // The walk to the one side's ore is as long as the twin's to the other's.
    let (mut island, mut cut_off) = (0, Vec::new());
    for region in map.file.ore_regions() {
        let c = region.centre();
        let c = (c.x.to_f64(), c.y.to_f64());
        for i in (0..8).step_by(2) {
            let (a, b) = (at(&walks[i], c), at(&walks[i + 1], turn(c)));
            if a < 0 && b < 0 {
                if i == 0 {
                    island += 1;
                    cut_off.push(format!("({:.0}, {:.0})", c.0, c.1));
                }
                continue;
            }
            if a < 0 || b < 0 {
                problems.push(format!(
                    "the ore field at {c:?} or its twin is out of reach"
                ));
            } else if (a - b).abs() as f64 > 0.15 * a.max(b) as f64 + 10.0 {
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
    if island != 4 {
        problems.push(format!(
            "{island} ore fields cannot be walked to ({}); the four island fields should be the only ones",
            cut_off.join(", ")
        ));
    }

    // Two oceans of one size that do not meet; a ship crosses the wall in each.
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
        seas.iter().map(|&n| km2(n).round()).collect::<Vec<_>>()
    );
    // (A rock pool under a cliff may be a ship's draught deep; it is no sea.)
    if seas.len() < 2
        || apart(seas[0] as f64, seas[1] as f64) > 0.06
        || seas[2..].iter().any(|&n| km2(n) > 0.02)
    {
        problems.push(format!(
            "the two oceans are not the map's only two seas, of about one size: {:?} km²",
            seas.iter().map(|&n| km2(n)).collect::<Vec<_>>()
        ));
    }
    let north = map.flood(map.cell_of((7_000.0, 12_000.0)), |x, y| map.sea_cell(x, y));
    if at(&north, (10_600.0, 12_000.0)) < 0 {
        problems.push("no ship can cross the wall in the north ocean".into());
    }
    if at(&north, turn((7_000.0, 12_000.0))) >= 0 {
        problems.push("the two oceans meet".into());
    }
    let mut sea = [0usize; 2];
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) {
                let p = ((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell);
                sea[(east_of(p) > 0.0) as usize] += 1;
            }
        }
    }
    println!(
        "{stem}: sea west of the wall {:.1} km², east {:.1} km²",
        km2(sea[0]),
        km2(sea[1])
    );
    if apart(sea[0] as f64, sea[1] as f64) > 0.1 {
        problems.push("one side has over 10% more sea than the other".into());
    }

    // The bases' roles, by the walk from each to the nearest strand.
    let strands: Vec<(u32, u32)> = (0..h)
        .flat_map(|cy| (0..w).map(move |cx| (cx, cy)))
        .filter(|&(cx, cy)| map.strand(cx, cy))
        .collect();
    let to_sea: Vec<(f64, (f64, f64))> = (0..8)
        .map(|i| {
            strands
                .iter()
                .filter(|&&(cx, cy)| walks[i][(cy * w + cx) as usize] >= 0)
                .map(|&(cx, cy)| {
                    (
                        walks[i][(cy * w + cx) as usize] as f64 * cell,
                        (cx as f64 * cell, cy as f64 * cell),
                    )
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap_or((f64::INFINITY, (0.0, 0.0)))
        })
        .collect();
    for (role, i) in [("rear", 0), ("front", 2), ("beach", 4), ("cliff", 6)] {
        println!(
            "{stem}: the {role} bases' walk to the nearest strand: west {:.0} m, east {:.0} m",
            to_sea[i].0,
            to_sea[i + 1].0
        );
    }
    for side in 0..2 {
        let name = ["west", "east"][side];
        let (rear, front, beach, cliff) = (
            to_sea[side].0,
            to_sea[2 + side].0,
            to_sea[4 + side].0,
            to_sea[6 + side].0,
        );
        if beach > 1_500.0 {
            problems.push(format!(
                "the {name}'s beach base is {beach:.0} m from its strand"
            ));
        }
        if cliff < 2_000.0 || cliff < 1.5 * beach {
            problems.push(format!(
                "the {name}'s cliff base is only {cliff:.0} m from a way down to the water"
            ));
        }
        let cove = turn_if(side == 1, COVE_HEAD);
        let landing = to_sea[6 + side].1;
        if (landing.0 - cove.0).hypot(landing.1 - cove.1) > 700.0 {
            problems.push(format!(
                "the {name}'s cliff base comes down to the water at {landing:?}, not at its cove"
            ));
        }
        if front < 1_200.0 {
            problems.push(format!(
                "the {name}'s front base has a strand {front:.0} m off"
            ));
        }
        if rear < front.max(cliff) {
            problems.push(format!(
                "the {name}'s rear base is only {rear:.0} m from the sea"
            ));
        }
    }
    for (role, i) in [("front", 2), ("beach", 4), ("cliff", 6)] {
        if apart(to_sea[i].0, to_sea[i + 1].0) > 0.15 {
            problems.push(format!(
                "the {role} bases are not as far from the water as each other"
            ));
        }
    }
    let mid = (size.0 / 2.0, size.1 / 2.0);
    let from_mid = |p: (f64, f64)| (p.0 - mid.0).hypot(p.1 - mid.1);
    if (2..8).any(|i| from_mid(starts[i]) >= from_mid(starts[0])) {
        problems.push("the rear base is not the furthest from the middle".into());
    }

    // The wall: its towers on the line, a pair to every turn.
    let towers: Vec<(f64, f64)> = map
        .file
        .props()
        .iter()
        .filter(|p| p.kind == PropKind::PrecursorTower)
        .map(|p| (p.pos.x.to_f64(), p.pos.y.to_f64()))
        .collect();
    if towers.len() < 13 || towers.iter().any(|&t| east_of(t).abs() > 1.0) {
        problems.push(format!("{} towers, or one off the wall", towers.len()));
    }
    for &t in &towers {
        let q = turn(t);
        if !towers.iter().any(|&o| (o.0 - q.0).hypot(o.1 - q.1) < 1.0) {
            problems.push(format!("the tower at {t:?} has no turned twin"));
        }
    }

    // The climates: snow east of the wall only, each side its own trees.
    match map.file.snow() {
        None => problems.push("no snow layer".into()),
        Some(snow) => {
            let (sw, sh) = map.file.info().snow_dims();
            let pitch = size.0 / (sw - 1) as f64;
            let (mut west, mut east, mut east_n) = (0usize, 0usize, 0usize);
            for j in 0..sh {
                for i in 0..sw {
                    let e = east_of((i as f64 * pitch, j as f64 * pitch));
                    let lying = snow[((j * sw + i) * 2 + 1) as usize] > 40;
                    let dry = map.cell_low(
                        (i * mc_map::format::SNOW_STRIDE).min(w - 1),
                        (j * mc_map::format::SNOW_STRIDE).min(h - 1),
                    ) > 2.0;
                    if e < -64.0 {
                        west += lying as usize;
                    } else if e > 64.0 && dry {
                        east += lying as usize;
                        east_n += 1;
                    }
                }
            }
            println!(
                "{stem}: snow lies on {:.0}% of the east's land, {west} samples west of the wall",
                100.0 * east as f64 / east_n as f64
            );
            if west > 0 {
                problems.push(format!("snow lies on {west} samples in the desert"));
            }
            if (east as f64) < 0.2 * east_n as f64 {
                problems.push("hardly any snow east of the wall".into());
            }
        }
    }
    let mut timber = [0.0f64; 2];
    let mut strays = 0;
    for p in map.file.props().iter().filter(|p| p.kind.is_tree()) {
        let east = east_of((p.pos.x.to_f64(), p.pos.y.to_f64())) > 0.0;
        let s = p.scale_milli as f64 / 1000.0;
        timber[east as usize] += s * s * s;
        strays += match p.kind {
            PropKind::TreeJuniper | PropKind::TreePinyon | PropKind::TreeCottonwood => {
                east as usize
            }
            PropKind::TreeConifer | PropKind::TreePine | PropKind::TreeBroadleaf => !east as usize,
            _ => 0,
        };
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
    if strays > 0 {
        problems.push(format!("{strays} trees grow in the wrong climate"));
    }

    // Wreckage: every wreck has its twin.
    let wrecks: Vec<(f64, f64)> = map
        .file
        .wrecks()
        .iter()
        .map(|w| (w.pos.x.to_f64(), w.pos.y.to_f64()))
        .collect();
    let lone = wrecks
        .iter()
        .filter(|&&p| {
            let q = turn(p);
            !wrecks.iter().any(|&o| (o.0 - q.0).hypot(o.1 - q.1) < 2.0)
        })
        .count();
    println!("{stem}: {} wrecks, {lone} without a twin", wrecks.len());
    if wrecks.is_empty() || lone > 0 {
        problems.push(format!("{lone} of {} wrecks have no twin", wrecks.len()));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
