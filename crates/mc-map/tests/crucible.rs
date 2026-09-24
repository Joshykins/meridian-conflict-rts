//! Holds the survival map "The Crucible" (`maps/crucible.mcmap`, baked with
//! `mc-bake --layout survival`) to its sidecar `maps/crucible.ron`: every
//! front runs where its forces can go, every spawn, the engine and every node
//! site stands on level ground (or open water), and the corridors are
//! separate. Maps are not checked in; without the file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test crucible -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SurvivalLayout};
use mc_data::weather::MapConfig;
use mc_map::{Heightfield, MapFile, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

struct Map {
    hf: Heightfield,
    file: MapFile,
    size: f64,
}

impl Map {
    fn z(&self, p: (f64, f64)) -> f64 {
        self.hf.height_at(fx(p)).to_f64() - self.hf.water_level().to_f64()
    }
    fn slope(&self, p: (f64, f64)) -> f64 {
        self.hf.slope_at(fx(p)).to_f64()
    }
    fn cell_slope(&self, p: (f64, f64)) -> f64 {
        let (cx, cy) = self.hf.cell_at(fx(p));
        self.hf.cell_slope(cx, cy).to_f64()
    }
    fn in_map(&self, p: (f64, f64)) -> bool {
        p.0 > 0.0 && p.1 > 0.0 && p.0 < self.size && p.1 < self.size
    }
    /// Lowest corner of the cell, relative to the water.
    fn cell_low(&self, cx: u32, cy: u32) -> f64 {
        let w = self.hf.water_level();
        [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|&(dx, dy)| (self.hf.sample_height(cx + dx, cy + dy) - w).to_f64())
            .fold(f64::INFINITY, f64::min)
    }
    /// A land unit can stand in the cell (the sim's rule: dry, slope <= 1/2).
    fn land_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) > 0.0 && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
    }
    /// A ship can sail the cell (the sim's rule: 6 m or deeper).
    fn sea_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) <= -6.0
    }

    /// Cells reachable from `from` through cells `open` accepts and `blocked` does not.
    fn flood(
        &self,
        from: (f64, f64),
        open: impl Fn(u32, u32) -> bool,
        blocked: impl Fn(f64, f64) -> bool,
    ) -> Vec<bool> {
        let (w, h) = self.hf.size_cells();
        let cell = CELL_SIZE_M as f64;
        let mut seen = vec![false; (w * h) as usize];
        let mut queue = VecDeque::from([self.hf.cell_at(fx(from))]);
        while let Some((cx, cy)) = queue.pop_front() {
            if cx >= w || cy >= h || seen[(cy * w + cx) as usize] {
                continue;
            }
            if !open(cx, cy) || blocked((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell) {
                continue;
            }
            seen[(cy * w + cx) as usize] = true;
            queue.extend([
                (cx + 1, cy),
                (cx.wrapping_sub(1), cy),
                (cx, cy + 1),
                (cx, cy.wrapping_sub(1)),
            ]);
        }
        seen
    }
    fn reached(&self, seen: &[bool], p: (f64, f64)) -> bool {
        let (w, _) = self.hf.size_cells();
        let (cx, cy) = self.hf.cell_at(fx(p));
        seen[(cy * w + cx) as usize]
    }
}

/// Level, dry ground within `radius` of `at`, with nothing standing on it.
fn assert_level(map: &Map, what: &str, at: (f64, f64), radius: f64, problems: &mut Vec<String>) {
    let z = map.z(at);
    if z < 5.0 {
        problems.push(format!("{what} at {at:?} is {z:.1} m above the water"));
        return;
    }
    let mut worst: (f64, f64) = (0.0, 0.0);
    for ring in 1..=4 {
        let r = radius * ring as f64 / 4.0;
        for k in 0..24 {
            let a = k as f64 / 24.0 * std::f64::consts::TAU;
            let p = (at.0 + r * a.cos(), at.1 + r * a.sin());
            worst.0 = worst.0.max((map.z(p) - z).abs());
            worst.1 = worst.1.max(map.slope(p));
        }
    }
    if worst.0 > 1.0 || worst.1 > 0.05 {
        problems.push(format!(
            "{what} at {at:?} is not level within {radius} m (height off by {:.2} m, slope {:.3})",
            worst.0, worst.1
        ));
    }
    let clutter = map
        .file
        .props()
        .iter()
        .filter(|p| dist((p.pos.x.to_f64(), p.pos.y.to_f64()), at) < radius)
        .count();
    if clutter > 0 {
        problems.push(format!(
            "{what} at {at:?} has {clutter} props within {radius} m"
        ));
    }
}

/// Open water at least `depth` deep within `radius` of `at`.
fn assert_deep(
    map: &Map,
    what: &str,
    at: (f64, f64),
    radius: f64,
    depth: f64,
    problems: &mut Vec<String>,
) {
    let mut shallowest = f64::INFINITY;
    for ring in 0..=4 {
        let r = radius * ring as f64 / 4.0;
        for k in 0..24 {
            let a = k as f64 / 24.0 * std::f64::consts::TAU;
            shallowest = shallowest.min(-map.z((at.0 + r * a.cos(), at.1 + r * a.sin())));
        }
    }
    if shallowest < depth {
        problems.push(format!(
            "{what} at {at:?}: water only {shallowest:.1} m deep within {radius} m (want {depth})"
        ));
    }
}

fn check(map_path: &std::path::Path) {
    let file = MapFile::open(map_path).unwrap();
    let hf = Heightfield::load(&file).unwrap();
    let size = hf.size_metres().x.to_f64();
    let map = Map { hf, file, size };
    let config = MapConfig::for_map(map_path).unwrap();
    let layout: SurvivalLayout = config
        .survival
        .clone()
        .expect("no survival block in the sidecar");
    assert_eq!(layout.problem(), None);
    let mut problems = Vec::new();

    // Starts: three defenders, then the engine, exactly where the sidecar says.
    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 4);
    assert_eq!(layout.engine_start as usize, starts.len() - 1);
    let engine = (layout.engine.0 as f64, layout.engine.1 as f64);
    assert!(
        dist(engine, starts[3]) < 1.0,
        "engine {engine:?} is not start 3 {:?}",
        starts[3]
    );
    assert_eq!(layout.spawns.len(), 3);
    let spawns: Vec<(f64, f64)> = layout
        .spawns
        .iter()
        .map(|s| starts[s.start as usize])
        .collect();
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        assert_level(&map, &format!("spawn {}", s.name), at, 150.0, &mut problems);
        println!(
            "spawn {:<10} start {} at {:?}, {:.1} m up",
            s.name,
            s.start,
            at,
            map.z(at)
        );
    }
    for (i, &a) in spawns.iter().enumerate() {
        for &b in &spawns[i + 1..] {
            let d = dist(a, b);
            if !(2000.0..=4200.0).contains(&d) {
                problems.push(format!("spawns {a:?} and {b:?} are {d:.0} m apart"));
            }
        }
    }
    // The engine: a 240 m foundry with a turret ring 300-500 m out, on a level plateau.
    assert_level(&map, "engine", engine, 460.0, &mut problems);
    println!("engine at {engine:?}, {:.1} m up", map.z(engine));

    let harbor = layout
        .harbor
        .map(|h| (h.0 as f64, h.1 as f64))
        .expect("no harbor");
    assert_deep(&map, "harbor", harbor, 160.0, 15.0, &mut problems);
    let d = dist(harbor, engine);
    if d > 1250.0 {
        problems.push(format!("the harbor is {d:.0} m from the engine"));
    }

    // Node sites.
    let (mut land_sites, mut sea_sites) = (0, 0);
    for site in &layout.node_sites {
        let at = (site.at.0 as f64, site.at.1 as f64);
        match site.domain {
            Domain::Land => {
                land_sites += 1;
                assert_level(
                    &map,
                    &format!("node site {}", site.name),
                    at,
                    100.0,
                    &mut problems,
                );
            }
            Domain::Naval => {
                sea_sites += 1;
                assert_deep(
                    &map,
                    &format!("node site {}", site.name),
                    at,
                    120.0,
                    15.0,
                    &mut problems,
                );
            }
            Domain::Air => problems.push(format!("node site {} is an air site", site.name)),
        }
    }
    assert!((8..=9).contains(&land_sites) && (2..=3).contains(&sea_sites));

    // Fronts, sampled every 4 m along the path and across a 48 m (land) or
    // 80 m (naval) wide lane.
    for front in &layout.fronts {
        let path: Vec<(f64, f64)> = front
            .path
            .iter()
            .map(|&(x, y)| (x as f64, y as f64))
            .collect();
        let (mut worst_slope, mut lowest, mut shallowest) = (0.0f64, f64::INFINITY, f64::INFINITY);
        for w in path.windows(2) {
            let len = dist(w[0], w[1]);
            let (ux, uy) = ((w[1].0 - w[0].0) / len, (w[1].1 - w[0].1) / len);
            let steps = (len / 4.0).ceil() as usize;
            for i in 0..=steps {
                let t = len * i as f64 / steps as f64;
                let c = (w[0].0 + ux * t, w[0].1 + uy * t);
                if !map.in_map(c) {
                    problems.push(format!("{} leaves the map at {c:?}", front.name));
                    break;
                }
                let half: f64 = match front.domain {
                    Domain::Land => 24.0,
                    Domain::Naval => 40.0,
                    Domain::Air => 0.0,
                };
                for k in -2..=2 {
                    let o = half * k as f64 / 2.0;
                    let p = (c.0 - uy * o, c.1 + ux * o);
                    match front.domain {
                        Domain::Land => {
                            let (z, s) = (map.z(p), map.cell_slope(p));
                            lowest = lowest.min(z);
                            worst_slope = worst_slope.max(s);
                            if z <= 0.5 || s > 0.5 {
                                problems.push(format!(
                                    "{} is not drivable at {p:?} ({z:.1} m up, slope {s:.2})",
                                    front.name
                                ));
                            }
                        }
                        Domain::Naval => {
                            let depth = -map.z(p);
                            shallowest = shallowest.min(depth);
                            if depth < 10.0 {
                                problems.push(format!(
                                    "{} is only {depth:.1} m deep at {p:?}",
                                    front.name
                                ));
                            }
                        }
                        Domain::Air => {}
                    }
                }
            }
        }
        let first = path[0];
        let last = *path.last().unwrap();
        let short = spawns
            .iter()
            .map(|&s| dist(s, last))
            .fold(f64::INFINITY, f64::min);
        if !(1400.0..=2700.0).contains(&short) {
            problems.push(format!(
                "{} ends {short:.0} m from the nearest spawn",
                front.name
            ));
        }
        if dist(first, engine) > 2000.0 && front.domain != Domain::Naval {
            problems.push(format!(
                "{} starts {:.0} m from the engine",
                front.name,
                dist(first, engine)
            ));
        }
        println!(
            "front {:<18} {:?}: ends {short:.0} m short of a spawn{}",
            front.name,
            front.domain,
            match front.domain {
                Domain::Land => format!(", lowest {lowest:.1} m up, steepest {worst_slope:.2}"),
                Domain::Naval => format!(", shallowest {shallowest:.1} m"),
                Domain::Air => String::new(),
            }
        );
        // Dedupe the problem list (one bad patch reports many samples).
        problems.dedup_by(|a, b| a.split(" at ").next() == b.split(" at ").next());
    }

    // Land: the engine reaches every spawn, every land front and every land
    // node site; ships leave the harbor for every naval front and naval site.
    let land = map.flood(engine, |cx, cy| map.land_cell(cx, cy), |_, _| false);
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        if !map.reached(&land, at) {
            problems.push(format!("no land route from the engine to {}", s.name));
        }
    }
    for f in layout.fronts_of(Domain::Land) {
        for &(x, y) in &f.path {
            if !map.reached(&land, (x as f64, y as f64)) {
                problems.push(format!("{} at ({x}, {y}) is cut off by land", f.name));
            }
        }
    }
    for site in layout
        .node_sites
        .iter()
        .filter(|s| s.domain == Domain::Land)
    {
        if !map.reached(&land, (site.at.0 as f64, site.at.1 as f64)) {
            problems.push(format!("node site {} is cut off by land", site.name));
        }
    }
    let sea = map.flood(harbor, |cx, cy| map.sea_cell(cx, cy), |_, _| false);
    for f in layout.fronts_of(Domain::Naval) {
        for &(x, y) in &f.path {
            if !map.reached(&sea, (x as f64, y as f64)) {
                problems.push(format!("{} at ({x}, {y}) cannot be sailed to", f.name));
            }
        }
    }
    for site in layout
        .node_sites
        .iter()
        .filter(|s| s.domain == Domain::Naval)
    {
        if !map.reached(&sea, (site.at.0 as f64, site.at.1 as f64)) {
            problems.push(format!("node site {} cannot be sailed to", site.name));
        }
    }

    // The land fronts are separate: with the plateau and the defenders'
    // pocket walled off, no corridor's middle reaches another's.
    let pocket = {
        let n = spawns.len() as f64;
        (
            spawns.iter().map(|s| s.0).sum::<f64>() / n,
            spawns.iter().map(|s| s.1).sum::<f64>() / n,
        )
    };
    let walled = |x: f64, y: f64| dist((x, y), engine) < 2200.0 || dist((x, y), pocket) < 3600.0;
    let middles: Vec<(&str, (f64, f64))> = layout
        .fronts_of(Domain::Land)
        .map(|f| {
            let p = f.path[f.path.len() / 2];
            (f.name.as_str(), (p.0 as f64, p.1 as f64))
        })
        .collect();
    for (i, &(name, at)) in middles.iter().enumerate() {
        let seen = map.flood(at, |cx, cy| map.land_cell(cx, cy), walled);
        for &(other, there) in &middles[i + 1..] {
            if map.reached(&seen, there) {
                problems.push(format!(
                    "{name} and {other} are joined outside the plateau and the pocket"
                ));
            }
        }
    }

    // Ore: scarce, none near the engine.
    let ore = map.file.ore_regions();
    for r in ore {
        let n = r.points.len() as f64;
        let c = (
            r.points.iter().map(|p| p.x.to_f64()).sum::<f64>() / n,
            r.points.iter().map(|p| p.y.to_f64()).sum::<f64>() / n,
        );
        if dist(c, engine) < 3500.0 {
            problems.push(format!("ore at {c:?} is near the engine"));
        }
    }
    println!(
        "{} ore fields, {} land and {} naval node sites",
        ore.len(),
        land_sites,
        sea_sites
    );

    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn crucible_matches_its_sidecar() {
    let path = std::env::var("MC_CHECK_CRUCIBLE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maps/crucible.mcmap")
        });
    if !path.exists() {
        eprintln!(
            "{} is not baked; skipping (see README for the bake command)",
            path.display()
        );
        return;
    }
    check(&path);
}
