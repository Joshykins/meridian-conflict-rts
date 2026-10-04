//! Holds "Tripoint" (three-way free-for-all, `mc-bake --layout tripoint`) to
//! its promises. The three thirds are not the same to look at, each is its own
//! climate; they are to play the same. On the baked terrain, by the
//! simulation's own rules:
//!
//! * the starts are one base turned a third of a turn and two thirds, each in
//!   its own region (Alaska, the desert, the jungle);
//! * every start can walk to the others, and the three have the same ground
//!   to walk and build on, within a few percent;
//! * each start's walk to every ore field is as long as the others' to the
//!   same field turned, within a few percent, and every field can be walked to;
//! * there is no water a ship can sail, and the three lakes, one on each
//!   wall, are of about one size;
//! * the walls' towers stand on their lines, each with its two turned twins,
//!   and the installation (the Axis and three bastions) stands in the middle;
//! * the walls part the climates: snow only in Alaska, spruce, pine and birch
//!   only there, juniper, pinyon and cottonwood only in the desert, jungle
//!   and palms only in the jungle;
//! * all three have the same wreckage (their woods are their own: trees
//!   carry no mass and slow no one);
//! * the map's sidecar gives the renderer the walls' own lines, the climates
//!   between them and the lift the mesas were cut to.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test tripoint -- --nocapture`; with
//! `TRIPOINT_WALK=out.ppm` it also writes where units can walk, one pixel a
//! cell, north up: grey where they can, black where they cannot, each start's
//! own ground tinted, and the cells some start reaches sooner than its twins
//! reach theirs in red.

use mc_core::{Fx, FxVec2};
use mc_map::landmark::{
    tripoint_region, FROSTLINE_STRATA_LIFT, TRIPOINT_ALASKA, TRIPOINT_DESERT, TRIPOINT_JUNGLE,
    TRIPOINT_WALLS,
};
use mc_map::{Heightfield, MapFile, PropKind, CELL_SIZE_M};
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
    mid: (f64, f64),
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
    fn centre(&self, cx: u32, cy: u32) -> (f64, f64) {
        let cell = CELL_SIZE_M as f64;
        ((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell)
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
    /// The walk from a point to every cell, in centimetres: a shortest path
    /// over sixteen headings (a straight line's length within 3% whichever
    /// way it runs: a third of a turn must not change it), never cutting a
    /// corner of ground it cannot cross.
    fn walk(&self, from: (f64, f64)) -> Vec<i32> {
        const STEPS: [(i32, i32, i32); 16] = [
            (1, 0, 100),
            (-1, 0, 100),
            (0, 1, 100),
            (0, -1, 100),
            (1, 1, 141),
            (1, -1, 141),
            (-1, 1, 141),
            (-1, -1, 141),
            (2, 1, 224),
            (2, -1, 224),
            (-2, 1, 224),
            (-2, -1, 224),
            (1, 2, 224),
            (1, -2, 224),
            (-1, 2, 224),
            (-1, -2, 224),
        ];
        let (w, h) = (self.w as i32, self.h as i32);
        let ok = |x: i32, y: i32| {
            x >= 0 && y >= 0 && x < w && y < h && self.land_cell(x as u32, y as u32)
        };
        let mut d = vec![-1i32; (w * h) as usize];
        let (fx, fy) = self.cell_of(from);
        let mut heap = std::collections::BinaryHeap::new();
        heap.push(std::cmp::Reverse((0i32, fx as i32, fy as i32)));
        while let Some(std::cmp::Reverse((here, cx, cy))) = heap.pop() {
            let at = (cy * w + cx) as usize;
            if d[at] >= 0 {
                continue;
            }
            d[at] = here;
            for (dx, dy, cost) in STEPS {
                let (nx, ny) = (cx + dx, cy + dy);
                // The cells the step passes over, as well as where it lands.
                let (sx, sy) = (dx.signum(), dy.signum());
                let clear = ok(nx, ny)
                    && ok(cx + sx, cy)
                    && ok(cx, cy + sy)
                    && ok(cx + sx, cy + sy)
                    && (dx.abs() < 2 || ok(cx + 2 * sx, cy + sy))
                    && (dy.abs() < 2 || ok(cx + sx, cy + 2 * sy));
                if clear && d[(ny * w + nx) as usize] < 0 {
                    heap.push(std::cmp::Reverse((here + cost, nx, ny)));
                }
            }
        }
        d
    }
    /// A point turned `k` thirds of a turn counter-clockwise about the middle.
    fn turn(&self, p: (f64, f64), k: usize) -> (f64, f64) {
        let a = k as f64 * std::f64::consts::TAU / 3.0;
        let (s, c) = a.sin_cos();
        let (dx, dy) = (p.0 - self.mid.0, p.1 - self.mid.1);
        (self.mid.0 + dx * c - dy * s, self.mid.1 + dx * s + dy * c)
    }
}

fn near(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// How far apart two amounts are, as a share of the larger.
fn apart(a: f64, b: f64) -> f64 {
    (a - b).abs() / a.max(b).max(1e-9)
}

/// The widest spread among three amounts, as a share of the largest.
fn spread(v: [f64; 3]) -> f64 {
    let (lo, hi) = (
        v.iter().copied().fold(f64::INFINITY, f64::min),
        v.iter().copied().fold(0.0, f64::max),
    );
    apart(lo, hi)
}

#[test]
fn tripoint_plays_the_same_from_every_third() {
    let stem = "tripoint";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../maps/{stem}.mcmap"));
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    let cell = CELL_SIZE_M as f64;
    let size = (w as f64 * cell, h as f64 * cell);
    let map = Map {
        hf,
        file,
        w,
        h,
        mid: (size.0 / 2.0, size.1 / 2.0),
    };
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = map
        .file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 3);
    for (k, &s) in starts.iter().enumerate() {
        if near(s, map.turn(starts[0], k)) > 1.0 {
            problems.push(format!("start {k} is not start 0 turned {k} thirds"));
        }
        if tripoint_region(s) != k {
            problems.push(format!("start {k} is not in region {k}"));
        }
    }

    let walks: Vec<Vec<i32>> = starts.iter().map(|&s| map.walk(s)).collect();
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    if let Ok(out) = std::env::var("TRIPOINT_WALK") {
        write_walk(&map, &walks, &out);
    }
    for (k, &s) in starts.iter().enumerate() {
        if at(&walks[0], s) < 0 {
            problems.push(format!("start {k} cannot be walked to from start 0"));
        }
    }
    // Ground a unit can reach, in each region.
    let mut ground = [0usize; 3];
    for cy in 0..h {
        for cx in 0..w {
            if walks[0][(cy * w + cx) as usize] >= 0 {
                ground[tripoint_region(map.centre(cx, cy))] += 1;
            }
        }
    }
    println!(
        "{stem}: ground to walk and build on: Alaska {:.1} km², desert {:.1} km², jungle {:.1} km² ({:.1}% apart)",
        km2(ground[0]),
        km2(ground[1]),
        km2(ground[2]),
        100.0 * spread(ground.map(|g| g as f64))
    );
    if spread(ground.map(|g| g as f64)) > 0.04 {
        problems.push("one region has over 4% more ground than another".into());
    }

    // Each start's walk to a field is as long as the others' to it turned.
    let fields: Vec<(f64, f64)> = map
        .file
        .ore_regions()
        .iter()
        .map(|r| {
            let c = r.centre();
            (c.x.to_f64(), c.y.to_f64())
        })
        .collect();
    let (mut worst, mut worst_at) = (0.0f64, (0.0, 0.0));
    for &c in &fields {
        let d: Vec<i32> = (0..3).map(|k| at(&walks[k], map.turn(c, k))).collect();
        if d.iter().any(|&d| d < 0) {
            problems.push(format!("the ore field at {c:?} or a twin is out of reach"));
            continue;
        }
        if !(1..3).all(|k| fields.iter().any(|&f| near(f, map.turn(c, k)) < 2.0)) {
            problems.push(format!("the ore field at {c:?} has no turned twins"));
        }
        let (lo, hi) = (*d.iter().min().unwrap(), *d.iter().max().unwrap());
        let off = (hi - lo) as f64 / (hi as f64 + 1_000.0);
        if off > worst {
            (worst, worst_at) = (off, c);
        }
        if (hi - lo) as f64 > 0.1 * hi as f64 + 1_000.0 {
            problems.push(format!(
                "ore at {c:?}: {d:?} cm from the three starts to it and its twins"
            ));
        }
    }
    println!(
        "{stem}: {} ore fields; the walks to a field and its twins differ by {:.1}% at most (at {worst_at:?})",
        fields.len(),
        100.0 * worst
    );

    // No sea; three lakes of about one size, one on each wall.
    let mut seen = vec![false; (w * h) as usize];
    let mut lakes = Vec::new();
    let mut sailable = 0usize;
    for cy in 0..h {
        for cx in 0..w {
            sailable += map.sea_cell(cx, cy) as usize;
            if map.cell_low(cx, cy) < -0.5 && !seen[(cy * w + cx) as usize] {
                let wet = map.flood((cx, cy), |x, y| map.cell_low(x, y) < -0.5);
                let mut n = 0usize;
                let mut sum = (0.0, 0.0);
                for (i, &d) in wet.iter().enumerate() {
                    if d >= 0 {
                        seen[i] = true;
                        n += 1;
                        let p = map.centre(i as u32 % w, i as u32 / w);
                        sum = (sum.0 + p.0, sum.1 + p.1);
                    }
                }
                if km2(n) > 0.05 {
                    lakes.push((km2(n), (sum.0 / n as f64, sum.1 / n as f64)));
                }
            }
        }
    }
    println!(
        "{stem}: lakes {:?} km², {:.3} km² a ship could sail",
        lakes
            .iter()
            .map(|l| (l.0 * 100.0).round() / 100.0)
            .collect::<Vec<_>>(),
        km2(sailable)
    );
    if km2(sailable) > 0.01 {
        problems.push(format!(
            "{:.3} km² of water a ship could sail",
            km2(sailable)
        ));
    }
    if lakes.len() != 3 || spread([0, 1, 2].map(|i| lakes.get(i).map_or(0.0, |l| l.0))) > 0.15 {
        problems.push(format!("not three lakes of about one size: {lakes:?}"));
    }

    // The installation in the middle, the towers on the walls.
    let props = map.file.props();
    let of = |kind: PropKind| -> Vec<(f64, f64)> {
        props
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| (p.pos.x.to_f64(), p.pos.y.to_f64()))
            .collect()
    };
    let (axis, bastions, towers) = (
        of(PropKind::PrecursorAxis),
        of(PropKind::PrecursorBastion),
        of(PropKind::PrecursorTower),
    );
    if axis.len() != 1 || near(axis[0], map.mid) > 1.0 || bastions.len() != 3 {
        problems.push(format!(
            "the installation: {} axes, {} bastions",
            axis.len(),
            bastions.len()
        ));
    }
    let off_wall = |p: (f64, f64)| {
        TRIPOINT_WALLS
            .iter()
            .map(|[a, b]| {
                let (ux, uy) = (b.0 - a.0, b.1 - a.1);
                let len = ux.hypot(uy);
                ((p.0 - a.0) * uy - (p.1 - a.1) * ux).abs() / len
            })
            .fold(f64::INFINITY, f64::min)
    };
    if towers.len() < 12 {
        problems.push(format!("only {} towers", towers.len()));
    }
    for &t in towers.iter().chain(&bastions) {
        if off_wall(t) > 2.0 {
            problems.push(format!("the tower or bastion at {t:?} is off the walls"));
        }
    }
    for &t in &towers {
        // (Out by the map's corners one wall runs on further than the others.)
        let on_map = |p: (f64, f64)| p.0 > 0.0 && p.1 > 0.0 && p.0 < size.0 && p.1 < size.1;
        for k in 1..3 {
            let q = map.turn(t, k);
            if on_map(q) && !towers.iter().any(|&o| near(o, q) < 2.0) {
                problems.push(format!("the tower at {t:?} has no twin turned {k} thirds"));
            }
        }
    }

    // The climates: snow only in Alaska, each region its own trees.
    let mid_dist = |p: (f64, f64)| near(p, map.mid);
    let wall_dist = |p: (f64, f64)| off_wall(p).min(mid_dist(p));
    match map.file.snow() {
        None => problems.push("no snow layer".into()),
        Some(snow) => {
            let (sw, sh) = map.file.info().snow_dims();
            let pitch = size.0 / (sw - 1) as f64;
            let (mut stray, mut alaska, mut alaska_n) = (0usize, 0usize, 0usize);
            for j in 0..sh {
                for i in 0..sw {
                    let p = (i as f64 * pitch, j as f64 * pitch);
                    let lying = snow[((j * sw + i) * 2 + 1) as usize] > 40;
                    let dry = map.cell_low(
                        (i * mc_map::format::SNOW_STRIDE).min(w - 1),
                        (j * mc_map::format::SNOW_STRIDE).min(h - 1),
                    ) > 2.0;
                    if wall_dist(p) < 64.0 {
                        continue;
                    }
                    if tripoint_region(p) == TRIPOINT_ALASKA {
                        alaska += (lying && dry) as usize;
                        alaska_n += dry as usize;
                    } else {
                        stray += lying as usize;
                    }
                }
            }
            println!(
                "{stem}: snow lies on {:.0}% of Alaska's land, {stray} samples outside it",
                100.0 * alaska as f64 / alaska_n as f64
            );
            if stray > 0 {
                problems.push(format!("snow lies on {stray} samples outside Alaska"));
            }
            if (alaska as f64) < 0.2 * alaska_n as f64 {
                problems.push("hardly any snow in Alaska".into());
            }
        }
    }
    let mut trees = [0usize; 3];
    let mut strays = 0;
    for p in props.iter().filter(|p| p.kind.is_tree()) {
        let region = tripoint_region((p.pos.x.to_f64(), p.pos.y.to_f64()));
        trees[region] += 1;
        let belongs = match p.kind {
            PropKind::TreeConifer | PropKind::TreePine => region == TRIPOINT_ALASKA,
            PropKind::TreeBroadleaf => region != TRIPOINT_DESERT,
            PropKind::TreeJuniper | PropKind::TreePinyon | PropKind::TreeCottonwood => {
                region == TRIPOINT_DESERT
            }
            PropKind::TreeJungle | PropKind::TreePalm => region == TRIPOINT_JUNGLE,
            _ => true,
        };
        strays += !belongs as usize;
    }
    println!(
        "{stem}: trees, Alaska {}, desert {}, jungle {}",
        trees[0], trees[1], trees[2]
    );
    if strays > 0 {
        problems.push(format!("{strays} trees grow in the wrong climate"));
    }

    // Wreckage: every wreck has its two turned twins.
    let wrecks: Vec<(f64, f64)> = map
        .file
        .wrecks()
        .iter()
        .map(|w| (w.pos.x.to_f64(), w.pos.y.to_f64()))
        .collect();
    let lone = wrecks
        .iter()
        .filter(|&&p| {
            !(1..3).all(|k| {
                let q = map.turn(p, k);
                wrecks.iter().any(|&o| near(o, q) < 2.0)
            })
        })
        .count();
    println!(
        "{stem}: {} wrecks, {lone} without their twins",
        wrecks.len()
    );
    if wrecks.is_empty() || lone > 0 {
        problems.push(format!("{lone} of {} wrecks lack a twin", wrecks.len()));
    }

    // The sidecar: what the renderer draws the three climates by.
    use mc_data::weather::{Climate, MapConfig};
    let config = MapConfig::for_map(&path).expect("the map's sidecar");
    let walls = config.walls().walls();
    let lines: Vec<Vec<(f64, f64)>> = walls
        .iter()
        .map(|w| w.line.iter().map(|p| (p.0 as f64, p.1 as f64)).collect())
        .collect();
    let want: Vec<Vec<(f64, f64)>> = TRIPOINT_WALLS.iter().map(|l| l.to_vec()).collect();
    if lines != want {
        problems.push(format!("the sidecar's walls {lines:?} are not the bake's"));
    }
    let sides: Vec<(usize, usize)> = walls.iter().map(|w| (w.left, w.right)).collect();
    if sides
        != [
            (TRIPOINT_ALASKA, TRIPOINT_JUNGLE),
            (TRIPOINT_DESERT, TRIPOINT_ALASKA),
            (TRIPOINT_JUNGLE, TRIPOINT_DESERT),
        ]
    {
        problems.push(format!("the sidecar's walls part {sides:?}"));
    }
    let climates: Vec<Climate> = config.regions().iter().map(|r| r.climate).collect();
    if climates != [Climate::Temperate, Climate::Desert, Climate::Tropical] {
        problems.push(format!("the sidecar's climates are {climates:?}"));
    }
    // The renderer's region lookup agrees with the bake's, off the walls.
    for i in 0..720 {
        let a = (i as f64 * 0.5 + 0.25).to_radians();
        let p = (map.mid.0 + 3_000.0 * a.cos(), map.mid.1 + 3_000.0 * a.sin());
        if config.walls().region_at(p.0 as f32, p.1 as f32) != tripoint_region(p) {
            problems.push(format!(
                "the renderer and the bake disagree on the region at {p:?}"
            ));
            break;
        }
    }
    if config.strata_lift as f64 != FROSTLINE_STRATA_LIFT {
        problems.push(format!(
            "the sidecar's strata_lift is {}, the bake's {FROSTLINE_STRATA_LIFT}",
            config.strata_lift
        ));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The walk picture (`TRIPOINT_WALK`).
fn write_walk(map: &Map, walks: &[Vec<i32>], out: &str) {
    let (w, h) = (map.w, map.h);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for cy in (0..h).rev() {
        for cx in 0..w {
            let i = (cy * w + cx) as usize;
            let mut c = if walks[0][i] < 0 {
                [0u8, 0, 0]
            } else {
                [[150, 165, 150], [175, 150, 125], [120, 160, 120]]
                    [tripoint_region(map.centre(cx, cy))]
            };
            // This cell from its own third's start, against its twins from theirs.
            let p = map.centre(cx, cy);
            let k = tripoint_region(p);
            let own = walks[k][i];
            let twins: Vec<i32> = (1..3)
                .map(|j| {
                    let q = map.turn(p, j);
                    let (qx, qy) = map.cell_of(q);
                    if qx < w && qy < h && q.0 > 0.0 && q.1 > 0.0 {
                        walks[(k + j) % 3][(qy * w + qx) as usize]
                    } else {
                        -1
                    }
                })
                .collect();
            if own >= 0 && twins.iter().all(|&t| t >= 0) {
                let worst = twins.iter().map(|&t| (t - own).abs()).max().unwrap_or(0);
                if worst as f64 > 0.1 * own as f64 + 2_000.0 {
                    c = [230, 40, 40];
                }
            }
            rgb.extend(c);
        }
    }
    let mut file = format!("P6 {w} {h} 255\n").into_bytes();
    file.extend(rgb);
    std::fs::write(out, file).expect("the walk picture");
}
