//! "Halcyon" as baked (`maps/halcyon.mcmap`, `Layout::Siege`): the wall shuts
//! the city off but for its gates and breaches, every base can walk to every
//! other, the two sides' fields are about the same walk from their bases, and
//! every base has the same level room.
//!
//! `HALCYON_PLAN=out.ppm[,x0,y0,span,px] cargo test --profile gate -p mc-map
//! --test halcyon -- --ignored plan_image` draws the plan: ground classes and
//! roads from the streets layer, solid footprints, ore, starts.

use mc_core::{Fx, FxVec2};
use mc_map::{Ground, Heightfield, MapFile, PropKind, Road, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::Path;

fn open() -> Option<MapFile> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps/halcyon.mcmap");
    if !path.exists() {
        eprintln!("halcyon.mcmap not baked; skipping");
        return None;
    }
    Some(MapFile::open(&path).expect("halcyon.mcmap opens"))
}

struct Map {
    hf: Heightfield,
    file: MapFile,
    w: u32,
    h: u32,
    solid: Vec<bool>,
}

fn load() -> Option<Map> {
    let file = open()?;
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
    Some(Map {
        hf,
        file,
        w,
        h,
        solid,
    })
}

impl Map {
    /// The sim's rule: dry, slope at most 1/2, nothing solid.
    fn land_cell(&self, cx: u32, cy: u32) -> bool {
        let water = self.hf.water_level();
        let dry = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .all(|&(dx, dy)| self.hf.sample_height(cx + dx, cy + dy) > water);
        dry && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
            && !self.solid[(cy * self.w + cx) as usize]
    }

    fn cell_of(&self, p: (f64, f64)) -> (u32, u32) {
        self.hf.cell_at(FxVec2::new(
            Fx((p.0 * 65536.0) as i64),
            Fx((p.1 * 65536.0) as i64),
        ))
    }

    /// Walk distances in cells from `from`; `diagonal` also steps corner to
    /// corner past two blocked cells (the most a leak could use).
    fn flood(&self, from: (f64, f64), diagonal: bool, ok: impl Fn(u32, u32) -> bool) -> Vec<i32> {
        let (w, h) = (self.w, self.h);
        let from = self.cell_of(from);
        let mut d = vec![-1i32; (w * h) as usize];
        d[(from.1 * w + from.0) as usize] = 0;
        let mut queue = VecDeque::from([from]);
        let steps: &[(i32, i32)] = if diagonal {
            &[
                (1, 0),
                (-1, 0),
                (0, 1),
                (0, -1),
                (1, 1),
                (1, -1),
                (-1, 1),
                (-1, -1),
            ]
        } else {
            &[(1, 0), (-1, 0), (0, 1), (0, -1)]
        };
        while let Some((cx, cy)) = queue.pop_front() {
            let here = d[(cy * w + cx) as usize];
            for &(dx, dy) in steps {
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if d[(ny * w + nx) as usize] >= 0 || !ok(nx, ny) {
                    continue;
                }
                d[(ny * w + nx) as usize] = here + 1;
                queue.push_back((nx, ny));
            }
        }
        d
    }

    fn at(&self, walk: &[i32], p: (f64, f64)) -> i32 {
        let (cx, cy) = self.cell_of(p);
        walk[(cy * self.w + cx) as usize]
    }

    fn starts(&self) -> Vec<(f64, f64)> {
        self.file
            .start_positions()
            .iter()
            .map(|p| (p.x.to_f64(), p.y.to_f64()))
            .collect()
    }
}

fn pos(p: &mc_map::Prop) -> (f64, f64) {
    (p.pos.x.to_f64(), p.pos.y.to_f64())
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The wall's line at `x`: the nearest wall piece's y.
fn wall_y(file: &MapFile, x: f64) -> f64 {
    file.props()
        .iter()
        .filter(|p| {
            matches!(
                p.kind,
                PropKind::CityWall | PropKind::CityWallTower | PropKind::CityGate
            )
        })
        .map(pos)
        .min_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
        .expect("a wall")
        .1
}

#[test]
fn the_wall_shuts_the_city_but_for_its_gates_and_breaches() {
    let Some(map) = load() else { return };
    let starts = map.starts();
    assert_eq!(starts.len(), 6);
    let mut problems = Vec::new();
    for (k, &s) in starts.iter().enumerate() {
        let city = s.1 > wall_y(&map.file, s.0);
        if city != (k % 2 == 0) {
            problems.push(format!("start {k} is on the wrong side of the wall"));
        }
    }
    // Every base walks to every other.
    let walks: Vec<Vec<i32>> = starts
        .iter()
        .map(|&s| map.flood(s, false, |x, y| map.land_cell(x, y)))
        .collect();
    for (i, walk) in walks.iter().enumerate() {
        for (j, &s) in starts.iter().enumerate() {
            if map.at(walk, s) < 0 {
                problems.push(format!("start {i} cannot walk to start {j}"));
            }
        }
    }
    // With the gates and the breaches stopped, the city is shut even to a
    // walker that slips corner to corner.
    let gates: Vec<(f64, f64)> = map
        .file
        .props()
        .iter()
        .filter(|p| p.kind == PropKind::CityGate)
        .map(pos)
        .collect();
    assert_eq!(gates.len(), 3, "three gates");
    let breaches: Vec<(f64, f64)> = map
        .file
        .props()
        .iter()
        .filter(|p| p.kind == PropKind::CityRubble)
        .map(pos)
        .filter(|&p| (p.1 - wall_y(&map.file, p.0)).abs() < 60.0)
        .collect();
    let cell = CELL_SIZE_M as f64;
    let stopped = |x: u32, y: u32| {
        let p = ((x as f64 + 0.5) * cell, (y as f64 + 0.5) * cell);
        gates.iter().any(|&g| dist(p, g) < 160.0) || breaches.iter().any(|&b| dist(p, b) < 160.0)
    };
    let shut = map.flood(starts[0], true, |x, y| {
        map.land_cell(x, y) && !stopped(x, y)
    });
    for (k, &s) in starts.iter().enumerate() {
        let reached = map.at(&shut, s) >= 0;
        if reached != (k % 2 == 0) {
            problems.push(format!(
                "with the gates and breaches shut, the city's start 0 {} start {k}",
                if reached {
                    "still reaches"
                } else {
                    "cannot reach"
                }
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Each side's fields are about the same walk from its bases as the other
/// side's; each base's own three are as near as every other base's.
#[test]
fn both_sides_walk_as_far_to_their_ore() {
    let Some(map) = load() else { return };
    let starts = map.starts();
    let walks: Vec<Vec<i32>> = starts
        .iter()
        .map(|&s| map.flood(s, false, |x, y| map.land_cell(x, y)))
        .collect();
    let fields: Vec<(f64, f64)> = map
        .file
        .ore_regions()
        .iter()
        .map(|r| {
            let n = r.points.len() as f64;
            let sum = r
                .points
                .iter()
                .fold((0.0, 0.0), |a, p| (a.0 + p.x.to_f64(), a.1 + p.y.to_f64()));
            (sum.0 / n, sum.1 / n)
        })
        .collect();
    let cell = CELL_SIZE_M as f64;
    // A field's walk from a start: to the nearest walkable cell near its middle.
    let walk_to = |walk: &[i32], f: (f64, f64)| {
        let (cx, cy) = map.cell_of(f);
        let mut best = i32::MAX;
        for dy in -6..=6_i32 {
            for dx in -6..=6_i32 {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                let d = walk[(y as u32 * map.w + x as u32) as usize];
                if d >= 0 {
                    best = best.min(d);
                }
            }
        }
        best as f64 * cell
    };
    let mut problems = Vec::new();
    // Own fields: the three nearest each base.
    let mut own = Vec::new();
    for walk in &walks {
        let mut d: Vec<f64> = fields.iter().map(|&f| walk_to(walk, f)).collect();
        d.sort_by(f64::total_cmp);
        own.push(d[..3].iter().sum::<f64>());
    }
    println!("own fields (sum of three nearest): {own:.0?}");
    let (lo, hi) = own
        .iter()
        .fold((f64::MAX, 0.0_f64), |a, &d| (a.0.min(d), a.1.max(d)));
    if hi > lo * 1.15 {
        problems.push(format!("own fields' walks differ: {own:?}"));
    }
    // Contested fields: each side's (on its side of the wall) summed over
    // its three bases' nearest walks.
    let side_of = |p: (f64, f64)| p.1 > wall_y(&map.file, p.0);
    let mut sides = [0.0, 0.0];
    let mut counts = [0, 0];
    for &f in &fields {
        let nearest_base = (0..6)
            .map(|k| walk_to(&walks[k], f))
            .fold(f64::MAX, f64::min);
        if nearest_base < 700.0 {
            continue;
        }
        let side = if side_of(f) { 0 } else { 1 };
        let team: f64 = (0..6)
            .filter(|k| k % 2 == side)
            .map(|k| walk_to(&walks[k], f))
            .fold(f64::MAX, f64::min);
        sides[side] += team;
        counts[side] += 1;
    }
    println!(
        "contested walks: city {:.0} m over {}, outskirts {:.0} m over {}",
        sides[0], counts[0], sides[1], counts[1]
    );
    if counts[0] != counts[1] {
        problems.push(format!(
            "contested fields: city {} outskirts {}",
            counts[0], counts[1]
        ));
    }
    if (sides[0] - sides[1]).abs() > 0.12 * sides[0].max(sides[1]) {
        problems.push(format!("contested walks differ: {sides:?}"));
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Every road and street can be driven down its whole length: no crater or
/// levelled lot leaves a cell on one too steep.
#[test]
fn every_road_is_walkable() {
    let Some(map) = load() else { return };
    let streets = map.file.streets().expect("a streets layer");
    let (sw, _) = map.file.info().streets_dims();
    let mut transit = vec![false; (map.w * map.h) as usize];
    for p in map
        .file
        .props()
        .iter()
        .filter(|p| matches!(p.kind, PropKind::CityTransit | PropKind::CityTransitStation))
    {
        for (y, a, b) in p.solid_runs((map.w, map.h)) {
            for x in a..=b {
                transit[(y * map.w + x) as usize] = true;
            }
        }
    }
    let mut steep = Vec::new();
    let mut roads = 0;
    for y in 0..map.h {
        for x in 0..map.w {
            // The cell's middle, by its four corner samples.
            let corner = |dx: u32, dy: u32| {
                let at = (((y + dy) * sw + x + dx) * 4) as usize;
                (streets[at] as f64 - 128.0).abs() / 4.0 < streets[at + 1] as f64 / 4.0 - 1.0
                    && streets[at + 2] & 0x7F != Road::Rail as u8
                    && streets[at + 2] & 0x7F != Road::None as u8
            };
            if !(corner(0, 0) && corner(1, 0) && corner(0, 1) && corner(1, 1)) {
                continue;
            }
            roads += 1;
            // The maglev's pylons stand in the avenues' medians.
            if !map.land_cell(x, y) && !transit[(y * map.w + x) as usize] {
                steep.push((x * 8, y * 8));
            }
        }
    }
    println!("{roads} road cells, {} not walkable", steep.len());
    assert!(
        steep.is_empty(),
        "not walkable: {:?}",
        &steep[..steep.len().min(30)]
    );
}

/// The wreckage of the assaults: the same salvage each side of the wall,
/// all of it on ground a unit can walk to.
#[test]
fn both_sides_start_with_the_same_salvage() {
    let Some(map) = load() else { return };
    let weight = |key: &str| {
        if key.contains("_t3_") {
            16
        } else if key.contains("_t2_") {
            4
        } else {
            1
        }
    };
    let mut sides = [0, 0];
    let mut problems = Vec::new();
    let walk = map.flood(map.starts()[0], false, |x, y| map.land_cell(x, y));
    for w in map.file.wrecks() {
        let p = (w.pos.x.to_f64(), w.pos.y.to_f64());
        let city = p.1 > wall_y(&map.file, p.0);
        sides[!city as usize] += weight(&w.blueprint);
        if map.at(&walk, p) < 0 {
            problems.push(format!(
                "the {} wreck at {p:?} cannot be walked to",
                w.blueprint
            ));
        }
    }
    println!("salvage weight: city {}, outskirts {}", sides[0], sides[1]);
    if sides[0] != sides[1] || sides[0] == 0 {
        problems.push(format!("salvage differs: {sides:?}"));
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
#[ignore = "writes an inspection image"]
fn plan_image() {
    let Some(file) = open() else { return };
    let spec = std::env::var("HALCYON_PLAN").unwrap_or_else(|_| "halcyon-plan.ppm".into());
    let mut parts = spec.split(',');
    let out = parts.next().unwrap().to_owned();
    let nums: Vec<f64> = parts.map(|v| v.parse().unwrap()).collect();
    let size = file.info().size_metres().x.to_f64();
    let (x0, y0, span, px) = match nums[..] {
        [x0, y0, span, px] => (x0, y0, span, px as usize),
        _ => (0.0, 0.0, size, 2048),
    };
    let m = span / px as f64;
    let (sw, sh) = file.info().streets_dims();
    let streets = file.streets().expect("a streets layer");
    let cell = mc_map::CELL_SIZE_M as f64;
    let mut rgb = vec![0u8; px * px * 3];
    for j in 0..px {
        for i in 0..px {
            let (x, y) = (x0 + (i as f64 + 0.5) * m, y0 + span - (j as f64 + 0.5) * m);
            let (si, sj) = ((x / cell).round() as i64, (y / cell).round() as i64);
            let colour = if si < 0 || sj < 0 || si >= sw as i64 || sj >= sh as i64 {
                [0, 0, 0]
            } else {
                let at = (sj as usize * sw as usize + si as usize) * 4;
                let s = &streets[at..at + 4];
                let (d, half) = ((s[0] as f64 - 128.0).abs() / 4.0, s[1] as f64 / 4.0);
                let on_road = d < half;
                let road = s[2] & 0x7F;
                let ground = s[3] & 0x0F;
                if on_road {
                    match road {
                        r if r == Road::Avenue as u8 => [70, 70, 78],
                        r if r == Road::Highway as u8 => [90, 84, 70],
                        r if r == Road::Lane as u8 => [150, 120, 80],
                        r if r == Road::Rail as u8 => [110, 60, 40],
                        _ => [55, 55, 60],
                    }
                } else {
                    match ground {
                        g if g == Ground::Paving as u8 => [170, 168, 160],
                        g if g == Ground::Lawn as u8 => [110, 160, 90],
                        g if g == Ground::Yard as u8 => [140, 135, 120],
                        g if g == Ground::Ballast as u8 => [120, 100, 90],
                        g if g == Ground::Field as u8 => [190, 175, 100],
                        g if g == Ground::Rubble as u8 => [150, 120, 110],
                        g if g == Ground::Earth as u8 => [130, 105, 80],
                        g if g == Ground::Apron as u8 => [185, 185, 190],
                        _ => [80, 120, 60],
                    }
                }
            };
            rgb[(j * px + i) * 3..][..3].copy_from_slice(&colour);
        }
    }
    let (w, h) = ((size / cell) as u32, (size / cell) as u32);
    let mut dot = |x: f64, y: f64, c: [u8; 3]| {
        let (i, j) = ((x - x0) / m, (y0 + span - y) / m);
        if i >= 0.0 && j >= 0.0 && (i as usize) < px && (j as usize) < px {
            rgb[(j as usize * px + i as usize) * 3..][..3].copy_from_slice(&c);
        }
    };
    for p in file.props() {
        if p.kind.is_tree() {
            dot(p.pos.x.to_f64(), p.pos.y.to_f64(), [20, 70, 30]);
            continue;
        }
        let wear = p.wear_milli as f64 / 1000.0;
        let tone = [
            (40.0 + 150.0 * wear) as u8,
            (40.0 + 20.0 * wear) as u8,
            (60.0 - 30.0 * wear) as u8,
        ];
        let colour = if p.kind == mc_map::PropKind::CityWall
            || p.kind == mc_map::PropKind::CityWallTower
            || p.kind == mc_map::PropKind::CityGate
        {
            [230, 230, 240]
        } else {
            tone
        };
        for (y, a, b) in p.solid_runs((w, h)) {
            for x in a..=b {
                let (cx, cy) = (x as f64 * cell, y as f64 * cell);
                let n = (cell / m).ceil().max(1.0) as usize;
                for v in 0..n {
                    for u in 0..n {
                        dot(cx + u as f64 * m, cy + v as f64 * m, colour);
                    }
                }
            }
        }
    }
    for region in file.ore_regions() {
        for c in &region.points {
            dot(c.x.to_f64(), c.y.to_f64(), [255, 120, 0]);
        }
    }
    for (k, s) in file.start_positions().iter().enumerate() {
        let colour = if k % 2 == 0 {
            [0, 120, 255]
        } else {
            [255, 30, 30]
        };
        for v in -12..=12 {
            for u in -12..=12 {
                dot(
                    s.x.to_f64() + u as f64 * m,
                    s.y.to_f64() + v as f64 * m,
                    colour,
                );
            }
        }
    }
    let mut bytes = format!("P6\n{px} {px}\n255\n").into_bytes();
    bytes.extend(rgb);
    std::fs::write(&out, bytes).unwrap();
}
