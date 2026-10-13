//! Holds the survival map "Rimehold" (`maps/rimehold.mcmap`, baked with
//! `mc-bake --layout rimehold`) to its sidecar `maps/rimehold.ron`: the
//! starts, print bays, slips, guns and every Shaper site's room stand on
//! level, open ground (a naval site's slips and the sea gates' in deep
//! water), every site's path runs where its forces can go; the starts stand
//! close together on the hold, which only its four necks join to the rest of
//! the land (each opening the way from its own sites), the auxiliary sites
//! are reachable only by their narrow ways (or by sea and air), every start
//! has ships of its sea basin at its shore; each start has its ore. Maps are
//! not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test rimehold -- --nocapture`

use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SiteKind, SurvivalLayout};
use mc_data::weather::{Climate, MapConfig};
use mc_map::{Heightfield, MapFile, CELL_SIZE_M};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn f(p: (f32, f32)) -> (f64, f64) {
    (p.0 as f64, p.1 as f64)
}

/// A point on a narrow way or a neck, and how far round it to shut.
type Throat = ((f64, f64), f64);

/// Where each auxiliary site's narrow way runs through its ring (or, for an
/// island, nowhere: it is reached by sea and air): a point on the way, and
/// how much round it to shut.
const THROATS: [(&str, Option<Throat>); 4] = [
    ("Rime Cirque", Some(((4_516.0, 18_565.0), 160.0))),
    ("Frostvault", Some(((3_771.0, 2_122.0), 150.0))),
    ("Echo Vault", Some(((17_090.0, 16_558.0), 160.0))),
    ("Far Skerry", None),
];

/// The necks across the hold's sea: a point at each one's middle, how much
/// round it shuts the neck, and the sites whose land ways cross it.
const NECKS: [(&str, Throat, &[&str]); 4] = [
    (
        "north neck",
        ((7_650.0, 13_700.0), 520.0),
        &["Hoarfrost", "Whitecap"],
    ),
    (
        "south neck",
        ((8_450.0, 6_600.0), 560.0),
        &["Scarfell", "Blackwater"],
    ),
    ("west neck", ((4_500.0, 10_200.0), 480.0), &["Wester Isle"]),
    (
        "the Causeway",
        ((11_600.0, 10_190.0), 760.0),
        &["The Forecourt", "North Court", "South Court"],
    ),
];

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
    /// Whether any cell within `radius` of `p` was reached.
    fn reached_near(&self, seen: &[bool], p: (f64, f64), radius: f64) -> bool {
        let step = CELL_SIZE_M as f64;
        let n = (radius / step) as i64;
        (-n..=n).any(|j| {
            (-n..=n).any(|i| {
                let q = (p.0 + i as f64 * step, p.1 + j as f64 * step);
                dist(p, q) <= radius && self.in_map(q) && self.reached(seen, q)
            })
        })
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
            let a = k as f64 / 24.0 * TAU;
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
    let mut solid = 0;
    for ring in 0..=8 {
        let r = radius * ring as f64 / 8.0;
        for k in 0..48 {
            let a = k as f64 / 48.0 * TAU;
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

/// Open water at least `depth` deep at every point of `points`.
fn assert_deep(
    map: &Map,
    what: &str,
    points: &[(f64, f64)],
    depth: f64,
    problems: &mut Vec<String>,
) {
    let (shallowest, at) =
        points
            .iter()
            .map(|&p| (-map.z(p), p))
            .fold(
                (f64::INFINITY, (0.0, 0.0)),
                |a, b| if b.0 < a.0 { b } else { a },
            );
    if shallowest < depth || points.iter().any(|&p| map.solid_at(p)) {
        problems.push(format!(
            "{what}: water only {shallowest:.1} m deep at {at:?} (want {depth}), or blocked"
        ));
    }
}

/// A path is drivable (land) or sailable (naval) along a lane either side of it.
fn check_path(
    map: &Map,
    name: &str,
    domain: Domain,
    path: &[(f64, f64)],
    problems: &mut Vec<String>,
) {
    let half: f64 = match domain {
        Domain::Land => 24.0,
        Domain::Naval => 40.0,
        Domain::Air => return,
    };
    let before = problems.len();
    for w in path.windows(2) {
        let len = dist(w[0], w[1]);
        let (ux, uy) = ((w[1].0 - w[0].0) / len, (w[1].1 - w[0].1) / len);
        let steps = (len / 4.0).ceil() as usize;
        for i in 0..=steps {
            let t = len * i as f64 / steps as f64;
            let c = (w[0].0 + ux * t, w[0].1 + uy * t);
            if !map.in_map(c) {
                problems.push(format!("{name} leaves the map at {c:?}"));
                return;
            }
            for k in -2..=2 {
                let o = half * k as f64 / 2.0;
                let p = (c.0 - uy * o, c.1 + ux * o);
                if map.solid_at(p) {
                    problems.push(format!("{name} runs into a precursor artifact at {p:?}"));
                }
                match domain {
                    Domain::Land => {
                        let (z, s) = (map.z(p), map.cell_slope(p));
                        if z <= 0.5 || s > 0.5 {
                            problems.push(format!(
                                "{name} is not drivable at {p:?} ({z:.1} m up, slope {s:.2})"
                            ));
                        }
                    }
                    _ => {
                        let depth = -map.z(p);
                        if depth < 10.0 {
                            problems.push(format!("{name} is only {depth:.1} m deep at {p:?}"));
                        }
                    }
                }
            }
            if problems.len() > before + 3 {
                return;
            }
        }
    }
}

fn load(path: &std::path::Path) -> (Map, SurvivalLayout, MapConfig) {
    let file = MapFile::open(path).unwrap();
    let hf = Heightfield::load(&file).unwrap();
    let size = hf.size_metres().x.to_f64();
    let (w, h) = hf.size_cells();
    let mut solid = vec![false; (w * h) as usize];
    for p in file.props().iter().filter(|p| p.kind.is_precursor()) {
        for (y, a, b) in p.solid_runs((w, h)) {
            for x in a..=b {
                solid[(y * w + x) as usize] = true;
            }
        }
    }
    let map = Map {
        hf,
        file,
        size,
        solid,
    };
    let config = MapConfig::for_map(path).unwrap();
    let layout = config
        .survival
        .clone()
        .expect("no survival block in the sidecar");
    (map, layout, config)
}

fn check(path: &std::path::Path) {
    let (map, layout, config) = load(path);
    assert_eq!(layout.problem(), None);
    assert!(
        (map.size - 20_480.0).abs() < 1.0,
        "the map is {} m",
        map.size
    );
    let climates: Vec<Climate> = config.regions().iter().map(|r| r.climate).collect();
    assert_eq!(climates, [Climate::Polar], "all of it arctic");
    let mut problems = Vec::new();

    // Starts: the four defenders' close together on the hold, then the Heart's.
    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 5);
    assert_eq!(layout.engine_start, 4);
    let heart = f(layout.engine);
    assert!(
        dist(heart, starts[4]) < 1.0,
        "heart {heart:?} is not start 4"
    );
    assert_eq!(layout.spawns.len(), 4);
    let spawns: Vec<(f64, f64)> = layout
        .spawns
        .iter()
        .map(|s| starts[s.start as usize])
        .collect();
    let middle = (
        spawns.iter().map(|s| s.0).sum::<f64>() / 4.0,
        spawns.iter().map(|s| s.1).sum::<f64>() / 4.0,
    );
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        assert_level(&map, &format!("spawn {}", s.name), at, 150.0, &mut problems);
        println!(
            "spawn {:<14} at {at:?}, {:.1} m up, {:.0} m from the middle",
            s.name,
            map.z(at),
            dist(at, middle)
        );
        if dist(at, middle) > 2_000.0 {
            problems.push(format!("spawn {} is far from the others", s.name));
        }
    }
    for (i, &a) in spawns.iter().enumerate() {
        for &b in &spawns[i + 1..] {
            if dist(a, b) < 2_000.0 {
                problems.push(format!("spawns at {a:?} and {b:?} crowd each other"));
            }
        }
    }
    let zs: Vec<f64> = spawns.iter().map(|&s| map.z(s)).collect();
    let (lo, hi) = zs
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |a, &z| (a.0.min(z), a.1.max(z)));
    if hi - lo > 15.0 {
        problems.push(format!("the starts stand {lo:.0} to {hi:.0} m up"));
    }
    // The base is near enough that its first round arrives early.
    let road = dist(heart, middle);
    println!("the Heart is {road:.0} m from the hold's middle");
    if road > 10_500.0 {
        problems.push(format!("the Heart is {road:.0} m from the hold"));
    }

    // Print bays level and open; the slips in deep water.
    for b in &layout.bays {
        match b.domain {
            Domain::Naval => assert_deep(&map, "slip", &[f(b.at)], 12.0, &mut problems),
            _ => assert_level(&map, "print bay", f(b.at), 30.0, &mut problems),
        }
    }
    assert_eq!(layout.bays.len(), 22);
    for g in &layout.guards {
        assert_level(
            &map,
            &format!("guard {}", g.key),
            f(g.at),
            20.0,
            &mut problems,
        );
    }

    // The sites: each room level and clear; a naval site's pad level and its
    // slips' arc in deep water.
    let (mut printers, mut aux) = (0, 0);
    for s in &layout.node_sites {
        let at = f(s.at);
        let room = s.room_or_default() as f64;
        match (s.kind, s.domain) {
            (SiteKind::Printer, Domain::Naval) => {
                assert_level(&map, &format!("{}'s pad", s.name), at, 80.0, &mut problems);
                let first = f(s.path[0]);
                let a = (first.1 - at.1).atan2(first.0 - at.0);
                let arc: Vec<(f64, f64)> = (0..=6)
                    .flat_map(|i| {
                        let b = a + (i as f64 - 3.0) * 0.2;
                        [150.0, 275.0, 400.0].map(|r| (at.0 + r * b.cos(), at.1 + r * b.sin()))
                    })
                    .collect();
                assert_deep(
                    &map,
                    &format!("{}'s slips", s.name),
                    &arc,
                    12.0,
                    &mut problems,
                );
            }
            _ => assert_level(&map, &format!("{}'s room", s.name), at, room, &mut problems),
        }
        match s.kind {
            SiteKind::Printer => {
                printers += 1;
                assert!(!s.path.is_empty(), "{} has no path", s.name);
                let mut path = vec![at];
                path.extend(s.path.iter().map(|&p| f(p)));
                // (The first leg starts in the room, where the Shaper stands.)
                check_path(&map, &s.name, s.domain, &path[1..], &mut problems);
                if dist(at, f(s.path[0])) > room + 60.0 {
                    problems.push(format!("{}'s path starts away from its room", s.name));
                }
            }
            _ => aux += 1,
        }
    }
    println!("{printers} printing sites, {aux} auxiliary");
    assert!(printers >= 10 && aux == 4);
    assert_eq!(
        layout
            .node_sites
            .iter()
            .filter(|s| s.domain == Domain::Naval)
            .count(),
        4
    );

    // The base's fronts.
    for front in &layout.fronts {
        let path: Vec<(f64, f64)> = front.path.iter().map(|&p| f(p)).collect();
        check_path(&map, &front.name, front.domain, &path, &mut problems);
        let last = *path.last().unwrap();
        let short = spawns
            .iter()
            .map(|&s| dist(s, last))
            .fold(f64::INFINITY, f64::min);
        if front.domain == Domain::Land && !(700.0..=3_000.0).contains(&short) {
            problems.push(format!(
                "{} ends {short:.0} m from the nearest spawn",
                front.name
            ));
        }
    }

    check_reach(&map, &layout, &spawns, &mut problems);
    check_seas(&map, &layout, &spawns, &mut problems);

    // Ore: three fields at every start, plenty more about the hold.
    let ore: Vec<(f64, f64)> = map
        .file
        .ore_regions()
        .iter()
        .map(|r| {
            let n = r.points.len() as f64;
            (
                r.points.iter().map(|p| p.x.to_f64()).sum::<f64>() / n,
                r.points.iter().map(|p| p.y.to_f64()).sum::<f64>() / n,
            )
        })
        .collect();
    for (s, &at) in layout.spawns.iter().zip(&spawns) {
        let home = ore.iter().filter(|&&o| dist(o, at) < 800.0).count();
        let near = ore.iter().filter(|&&o| dist(o, at) < 3_000.0).count();
        println!("{:<14} {home} fields at home, {near} within 3 km", s.name);
        if home < 3 || near < 6 {
            problems.push(format!(
                "{} has {home} home fields, {near} within 3 km",
                s.name
            ));
        }
    }
    if ore.len() < 30 {
        problems.push(format!("only {} ore fields", ore.len()));
    }

    // Wreckage along the waves' ways, none in a room.
    let wrecks: Vec<(f64, f64)> = map
        .file
        .wrecks()
        .iter()
        .map(|w| (w.pos.x.to_f64(), w.pos.y.to_f64()))
        .collect();
    println!("{} wrecks", wrecks.len());
    if wrecks.len() < 120 {
        problems.push(format!("only {} wrecks", wrecks.len()));
    }
    for s in &layout.node_sites {
        if wrecks
            .iter()
            .any(|&w| dist(w, f(s.at)) < s.room_or_default() as f64)
        {
            problems.push(format!("a wreck lies in {}'s room", s.name));
        }
    }

    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

/// By land from the defenders: every land printing site and the base's bays;
/// the auxiliary sites only by their narrow ways (shut one and its site is
/// cut off), the atoll not at all. The hold joins the rest of the land only
/// by its necks: shut all four and nothing outside is reached; open one and
/// its own sites are.
fn check_reach(
    map: &Map,
    layout: &SurvivalLayout,
    spawns: &[(f64, f64)],
    problems: &mut Vec<String>,
) {
    let land = map.flood(spawns[0], |cx, cy| map.land_cell(cx, cy), |_, _| false);
    for (i, &s) in spawns.iter().enumerate() {
        if !map.reached(&land, s) {
            problems.push(format!("spawn {i} is cut off from spawn 0 by land"));
        }
    }
    for b in layout.bays.iter().filter(|b| b.domain != Domain::Naval) {
        if !map.reached(&land, f(b.at)) {
            problems.push(format!(
                "print bay {:?} is cut off from the defenders",
                b.at
            ));
        }
    }
    for s in &layout.node_sites {
        let at = f(s.at);
        match (s.kind, s.domain) {
            (SiteKind::Printer, Domain::Land) => {
                if !map.reached(&land, at) {
                    problems.push(format!("{} is cut off from the defenders by land", s.name));
                }
            }
            (SiteKind::Printer, _) => {}
            _ => {
                let throat = THROATS
                    .iter()
                    .find(|t| t.0 == s.name)
                    .unwrap_or_else(|| panic!("no throat for {}", s.name))
                    .1;
                match throat {
                    Some((p, r)) => {
                        if !map.reached(&land, at) {
                            problems.push(format!("{} cannot be reached by land", s.name));
                        }
                        let shut = map.flood(
                            spawns[0],
                            |cx, cy| map.land_cell(cx, cy),
                            |x, y| dist((x, y), p) < r,
                        );
                        if map.reached(&shut, at) {
                            problems.push(format!(
                                "{} can be reached by land without its narrow way",
                                s.name
                            ));
                        }
                    }
                    None => {
                        if map.reached(&land, at) {
                            problems.push(format!("{} can be walked to", s.name));
                        }
                    }
                }
            }
        }
    }
    // The ring: with every neck shut the hold stands alone.
    let shut_all = |open: Option<usize>| {
        move |x: f64, y: f64| {
            NECKS
                .iter()
                .enumerate()
                .any(|(k, n)| Some(k) != open && dist((x, y), n.1 .0) < n.1 .1)
        }
    };
    let alone = map.flood(spawns[0], |cx, cy| map.land_cell(cx, cy), shut_all(None));
    for s in &layout.node_sites {
        if map.reached(&alone, f(s.at)) {
            problems.push(format!(
                "{} is reached from the hold with every neck shut",
                s.name
            ));
        }
    }
    if map.reached(&alone, f(layout.engine)) {
        problems.push("the base is reached from the hold with every neck shut".into());
    }
    for (k, (neck, _, sites)) in NECKS.iter().enumerate() {
        let one = map.flood(spawns[0], |cx, cy| map.land_cell(cx, cy), shut_all(Some(k)));
        for name in *sites {
            let s = layout
                .node_sites
                .iter()
                .find(|s| s.name == *name)
                .unwrap_or_else(|| panic!("no site {name}"));
            if !map.reached(&one, f(s.at)) {
                problems.push(format!("{name} is not reached over {neck} alone"));
            }
        }
    }
}

/// The sea ring: three basins (the west, the north lagoon, the south
/// lagoon), each with its own slips, and every start has water of a basin
/// that ships are printed in within 2 km. The atoll can be sailed to.
fn check_seas(
    map: &Map,
    layout: &SurvivalLayout,
    spawns: &[(f64, f64)],
    problems: &mut Vec<String>,
) {
    let mut sources: Vec<(String, (f64, f64))> = layout
        .node_sites
        .iter()
        .filter(|s| s.domain == Domain::Naval)
        .map(|s| (s.name.clone(), f(*s.path.last().unwrap())))
        .collect();
    sources.extend(
        layout
            .bays
            .iter()
            .filter(|b| b.domain == Domain::Naval)
            .map(|b| ("a sea gate".to_string(), f(b.at))),
    );
    let mut covered = vec![false; spawns.len()];
    let mut basins: Vec<Vec<bool>> = Vec::new();
    for (name, at) in &sources {
        let sea = map.flood(*at, |cx, cy| map.sea_cell(cx, cy), |_, _| false);
        if !map.reached(&sea, *at) {
            problems.push(format!(
                "{name}'s ships start out of sailable water at {at:?}"
            ));
            continue;
        }
        for (i, &s) in spawns.iter().enumerate() {
            covered[i] |= map.reached_near(&sea, s, 2_000.0);
        }
        if !basins.iter().any(|b| map.reached(b, *at)) {
            basins.push(sea);
        }
    }
    println!("{} sea basins with slips", basins.len());
    if basins.len() != 3 {
        problems.push(format!("{} sea basins have slips, not 3", basins.len()));
    }
    for (i, c) in covered.iter().enumerate() {
        if !c {
            problems.push(format!("no ships' water within 2 km of spawn {i}"));
        }
    }
    let atoll = layout
        .node_sites
        .iter()
        .find(|s| s.name == "Far Skerry")
        .unwrap();
    if !basins
        .iter()
        .any(|b| map.reached_near(b, f(atoll.at), 1_600.0))
    {
        problems.push("the atoll cannot be sailed to".into());
    }
}

#[test]
fn rimehold_matches_its_sidecar() {
    let path = std::env::var("MC_CHECK_RIMEHOLD")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maps/rimehold.mcmap")
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
