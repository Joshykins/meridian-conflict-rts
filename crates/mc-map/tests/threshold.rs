//! Holds the survival map "The Threshold" (`maps/threshold.mcmap`, baked with
//! `mc-bake --layout threshold`) to its sidecar `maps/threshold.ron`: the print
//! bays, cradles and guns stand on level open ground (or deep water), the fronts
//! run where their forces can go, everything the facility prints can reach the
//! defenders by the one road, and only by the pass and the Rampart's ramp. Maps
//! are not checked in; without the file the test says so and passes.
//!
//! `cargo test --release -p mc-map --test threshold -- --nocapture`

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
    /// Path cells a precursor artifact makes solid, as the sim blocks them.
    solid: Vec<bool>,
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
    fn solid_cell(&self, cx: u32, cy: u32) -> bool {
        let (w, _) = self.hf.size_cells();
        self.solid
            .get((cy * w + cx) as usize)
            .copied()
            .unwrap_or(false)
    }
    fn solid_at(&self, p: (f64, f64)) -> bool {
        let (cx, cy) = self.hf.cell_at(fx(p));
        self.solid_cell(cx, cy)
    }
    /// A land unit can stand in the cell (the sim's rule: dry, slope <= 1/2, nothing built on it).
    fn land_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) > 0.0
            && self.hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
            && !self.solid_cell(cx, cy)
    }
    /// A ship can sail the cell (the sim's rule: 6 m or deeper, nothing standing in it).
    fn sea_cell(&self, cx: u32, cy: u32) -> bool {
        self.cell_low(cx, cy) <= -6.0 && !self.solid_cell(cx, cy)
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
        .filter(|p| {
            !p.kind.is_precursor() && dist((p.pos.x.to_f64(), p.pos.y.to_f64()), at) < radius
        })
        .count();
    // Conduits lie flush and are walked over; anything solid is in the way.
    let mut solid = 0;
    for ring in 0..=8 {
        let r = radius * ring as f64 / 8.0;
        for k in 0..48 {
            let a = k as f64 / 48.0 * std::f64::consts::TAU;
            solid += map.solid_at((at.0 + r * a.cos(), at.1 + r * a.sin())) as usize;
        }
    }
    if solid > 0 {
        problems.push(format!(
            "{what} at {at:?} has a precursor artifact within {radius} m"
        ));
    }
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
    let (w, h) = hf.size_cells();
    let mut solid = vec![false; (w * h) as usize];
    let mut artifacts = 0;
    for p in file.props().iter().filter(|p| p.kind.is_precursor()) {
        artifacts += 1;
        for (y, a, b) in p.solid_runs((w, h)) {
            for x in a..=b {
                solid[(y * w + x) as usize] = true;
            }
        }
    }
    println!(
        "{artifacts} precursor artifacts, {} solid cells",
        solid.iter().filter(|s| **s).count()
    );
    assert!(artifacts > 150, "only {artifacts} precursor artifacts");
    let map = Map {
        hf,
        file,
        size,
        solid,
    };
    let config = MapConfig::for_map(map_path).unwrap();
    let layout: SurvivalLayout = config
        .survival
        .clone()
        .expect("no survival block in the sidecar");
    assert_eq!(layout.problem(), None);
    let mut problems = Vec::new();

    // Starts: three defenders in the west, then the facility's, at its heart.
    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 4);
    assert_eq!(layout.engine_start as usize, starts.len() - 1);
    let heart = (layout.engine.0 as f64, layout.engine.1 as f64);
    assert!(
        dist(heart, starts[3]) < 1.0,
        "heart {heart:?} is not start 3 {:?}",
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
            "spawn {:<12} start {} at {:?}, {:.1} m up",
            s.name,
            s.start,
            at,
            map.z(at)
        );
        if at.0 > size / 2.0 {
            problems.push(format!("spawn {} is in the facility's half", s.name));
        }
    }

    // Print bays: a land bay's stand is open, level ground; a slip is deep water.
    let (mut land_bays, mut slips) = (Vec::new(), Vec::new());
    for b in &layout.bays {
        let at = (b.at.0 as f64, b.at.1 as f64);
        match b.domain {
            Domain::Naval => {
                assert_deep(&map, "slip", at, 40.0, 12.0, &mut problems);
                slips.push(at);
            }
            _ => {
                assert_level(&map, "print bay", at, 30.0, &mut problems);
                land_bays.push(at);
            }
        }
    }
    // Two halls and the Great Forge (four bays each) and four aeries; three slips.
    assert_eq!((land_bays.len(), slips.len()), (16, 3));
    let harbor = layout
        .harbor
        .map(|h| (h.0 as f64, h.1 as f64))
        .expect("no harbor");
    assert_deep(&map, "sea gate", harbor, 60.0, 15.0, &mut problems);

    // Guns: each on open, level ground.
    for g in &layout.guards {
        assert_level(
            &map,
            &format!("guard {}", g.key),
            (g.at.0 as f64, g.at.1 as f64),
            20.0,
            &mut problems,
        );
    }

    // Cradles: a row of three Shapers, 72 m apart across their facing.
    let (mut land_sites, mut sea_sites) = (0, 0);
    for site in &layout.node_sites {
        let a = site.facing.unwrap_or(180.0).to_radians() as f64;
        for k in [-1.0, 0.0, 1.0] {
            let at = (
                site.at.0 as f64 - a.sin() * 72.0 * k,
                site.at.1 as f64 + a.cos() * 72.0 * k,
            );
            match site.domain {
                Domain::Land => assert_level(
                    &map,
                    &format!("cradle {} slot {k}", site.name),
                    at,
                    32.0,
                    &mut problems,
                ),
                _ => assert_deep(
                    &map,
                    &format!("cradle {} slot {k}", site.name),
                    at,
                    32.0,
                    12.0,
                    &mut problems,
                ),
            }
        }
        match site.domain {
            Domain::Land => land_sites += 1,
            _ => sea_sites += 1,
        }
    }
    assert!(land_sites >= 8 && sea_sites >= 2);

    // Fronts, sampled every 4 m along the path and across a 48 m (land) or 80 m (naval) lane.
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
                            if map.solid_at(p) {
                                problems.push(format!(
                                    "{} runs into a precursor artifact at {p:?}",
                                    front.name
                                ));
                            } else if z <= 0.5 || s > 0.5 {
                                problems.push(format!(
                                    "{} is not drivable at {p:?} ({z:.1} m up, slope {s:.2})",
                                    front.name
                                ));
                            }
                        }
                        Domain::Naval => {
                            let depth = -map.z(p);
                            shallowest = shallowest.min(depth);
                            if map.solid_at(p) {
                                problems.push(format!(
                                    "{} runs into a precursor artifact at {p:?}",
                                    front.name
                                ));
                            } else if depth < 10.0 {
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
        let last = *path.last().unwrap();
        let short = spawns
            .iter()
            .map(|&s| dist(s, last))
            .fold(f64::INFINITY, f64::min);
        if front.domain != Domain::Air && !(700.0..=3000.0).contains(&short) {
            problems.push(format!(
                "{} ends {short:.0} m from the nearest spawn",
                front.name
            ));
        }
        println!(
            "front {:<20} {:?}: ends {short:.0} m short of a spawn{}",
            front.name,
            front.domain,
            match front.domain {
                Domain::Land => format!(", lowest {lowest:.1} m up, steepest {worst_slope:.2}"),
                Domain::Naval => format!(", shallowest {shallowest:.1} m"),
                Domain::Air => String::new(),
            }
        );
        problems.dedup_by(|a, b| a.split(" at ").next() == b.split(" at ").next());
    }

    // Land: from the print bays everything is reachable: every spawn, every
    // land front, every land cradle.
    let land = map.flood(land_bays[0], |cx, cy| map.land_cell(cx, cy), |_, _| false);
    for &b in &land_bays {
        if !map.reached(&land, b) {
            problems.push(format!("print bay {b:?} is cut off"));
        }
    }
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        if !map.reached(&land, at) {
            problems.push(format!("no land route from the Forges to {}", s.name));
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
        if !map.reached(&land, (site.at.0 as f64 + 40.0, site.at.1 as f64)) {
            problems.push(format!("cradle {} is cut off by land", site.name));
        }
    }
    // One road: shut the pass and the defenders cannot be reached by land.
    let shut = map.flood(
        land_bays[0],
        |cx, cy| map.land_cell(cx, cy),
        |x, _| (7_700.0..7_900.0).contains(&x),
    );
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        if map.reached(&shut, at) {
            problems.push(format!(
                "{} can be reached by land without the pass",
                s.name
            ));
        }
    }
    // One road up the Rampart: shut the ramp and the plateau is cut off.
    let ramp_shut = map.flood(
        land_bays[0],
        |cx, cy| map.land_cell(cx, cy),
        |x, y| (9_800.0..10_000.0).contains(&x) && (8_104.0..8_504.0).contains(&y),
    );
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        if map.reached(&ramp_shut, at) {
            problems.push(format!(
                "{} can be reached from the plateau without the ramp",
                s.name
            ));
        }
    }

    let sea = map.flood(slips[0], |cx, cy| map.sea_cell(cx, cy), |_, _| false);
    for &s in &slips {
        if !map.reached(&sea, s) {
            problems.push(format!("slip {s:?} cannot be sailed out of"));
        }
    }
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
            problems.push(format!("cradle {} cannot be sailed to", site.name));
        }
    }

    // Ore: scarce, none in the facility's half.
    let ore = map.file.ore_regions();
    for r in ore {
        let n = r.points.len() as f64;
        let c = (
            r.points.iter().map(|p| p.x.to_f64()).sum::<f64>() / n,
            r.points.iter().map(|p| p.y.to_f64()).sum::<f64>() / n,
        );
        if c.0 > 9_500.0 {
            problems.push(format!("ore at {c:?} is in the facility"));
        }
    }
    println!(
        "{} ore fields, {land_sites} land and {sea_sites} naval cradles",
        ore.len()
    );

    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn threshold_matches_its_sidecar() {
    let path = std::env::var("MC_CHECK_THRESHOLD")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maps/threshold.mcmap")
        });
    if !path.exists() {
        eprintln!(
            "{} is not baked; skipping (see mc-bake.rs for the bake command)",
            path.display()
        );
        return;
    }
    check(&path);
}
