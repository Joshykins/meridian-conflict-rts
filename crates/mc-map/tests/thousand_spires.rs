//! Holds "Thousand Spires" (three teams of four, `mc-bake --layout spires`)
//! to its promise. The three thirds are not the same to look at: the plan's
//! lines lie their own way in each, and the hills, islands, massif and
//! pillars are each third's own. They are to play the same. On the baked
//! terrain, by the simulation's own rules:
//!
//! * there are twelve starts, the north team's four and the same turned a
//!   third of a turn and two thirds, and every start can walk to every other;
//! * every ore field has its two turned twins, and every start can walk to
//!   every field;
//! * each base's walk to its own team's ore is as long as its twins' in the
//!   other teams within 8%, to the island in the middle within 12%, to the
//!   nearest enemy base within 17%;
//! * the sea is one, round the map and up the three estuaries, and the lake
//!   in the middle is too shallow for ships;
//! * the woods are deciduous: most trees broadleaf.
//!
//! The map is not checked in; without the file the test says so and passes.
//!
//! `cargo test --profile gate -p mc-map --test thousand_spires -- --nocapture`;
//! with `SPIRES_WALK=out.ppm` it also writes where units can walk, one pixel
//! a cell, north up: grey where they can (lighter the nearer to start 0, or
//! to start `SPIRES_WALK_FROM`, banded every kilometre), red where they
//! could but cannot get to, black where they cannot, blue where ships can
//! sail.

use mc_core::{Fx, FxVec2};
use mc_map::{Heightfield, MapFile, PropKind, CELL_SIZE_M};
use std::collections::VecDeque;
use std::path::PathBuf;

fn fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0) as i64), Fx((p.1 * 65536.0) as i64))
}

struct Map {
    hf: Heightfield,
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
    /// The walk from a point to every cell, in hundredths of a cell: a shortest path
    /// over sixteen headings (a straight line's length within 3% whichever
    /// way it runs), never cutting a corner of ground it cannot cross.
    fn walk(&self, from: (f64, f64), land: &[bool]) -> Vec<i32> {
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
        let ok = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && land[(y * w + x) as usize];
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

/// The widest spread among three amounts, as a share of the largest.
fn spread(v: [f64; 3]) -> f64 {
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(0.0, f64::max);
    (hi - lo) / hi.max(1e-9)
}

#[test]
fn thousand_spires_plays_the_same_for_every_team() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maps/thousand_spires.mcmap");
    let Ok(file) = MapFile::open(&path) else {
        eprintln!("{} not baked; skipping", path.display());
        return;
    };
    let hf = Heightfield::load(&file).expect("heightfield");
    let (w, h) = hf.size_cells();
    let cell = CELL_SIZE_M as f64;
    let map = Map {
        hf,
        w,
        h,
        mid: (w as f64 * cell / 2.0, h as f64 * cell / 2.0),
    };
    let mut problems = Vec::new();

    let starts: Vec<(f64, f64)> = file
        .start_positions()
        .iter()
        .map(|p| (p.x.to_f64(), p.y.to_f64()))
        .collect();
    assert_eq!(starts.len(), 12);
    for (i, &s) in starts.iter().enumerate() {
        if near(s, map.turn(starts[i % 4], i / 4)) > 1.0 {
            problems.push(format!("start {i} is not start {} turned", i % 4));
        }
    }

    let land: Vec<bool> = (0..w * h).map(|i| map.land_cell(i % w, i / w)).collect();
    let walks: Vec<Vec<i32>> = std::thread::scope(|s| {
        let handles: Vec<_> = starts
            .iter()
            .map(|&p| {
                let (map, land) = (&map, &land);
                s.spawn(move || map.walk(p, land))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let at = |walk: &[i32], p: (f64, f64)| {
        let (cx, cy) = map.cell_of(p);
        walk[(cy * w + cx) as usize]
    };
    if let Ok(out) = std::env::var("SPIRES_WALK") {
        // (`SPIRES_WALK_FROM=i` shades start i's walk instead of start 0's.)
        let from = std::env::var("SPIRES_WALK_FROM")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
            .min(11);
        write_walk(&map, &walks[from], &out);
    }
    for (i, &s) in starts.iter().enumerate() {
        if at(&walks[0], s) < 0 {
            problems.push(format!("start {i} cannot be walked to from start 0"));
        }
    }

    // Every field has its twins and can be walked to.
    let fields: Vec<(f64, f64)> = file
        .ore_regions()
        .iter()
        .map(|r| {
            let c = r.centre();
            (c.x.to_f64(), c.y.to_f64())
        })
        .collect();
    for &c in &fields {
        if !(1..3).all(|k| fields.iter().any(|&f| near(f, map.turn(c, k)) < 2.0)) {
            problems.push(format!("the ore field at {c:?} has no turned twins"));
        }
        if at(&walks[0], c) < 0 {
            problems.push(format!("the ore field at {c:?} is out of reach"));
        }
    }

    // Each base against its twins in the other teams.
    // (Walks are in hundredths of a cell.)
    let metres = |d: i32| d as f64 / 100.0 * cell;
    let mut worst = 0.0f64;
    for slot in 0..4 {
        let twins = [slot, 4 + slot, 8 + slot];
        let island = twins.map(|i| metres(at(&walks[i], map.mid)));
        let enemy = twins.map(|i| {
            (0..12)
                .filter(|&j| j / 4 != i / 4)
                .map(|j| at(&walks[i], starts[j]))
                .filter(|&d| d >= 0)
                .map(metres)
                .fold(f64::INFINITY, f64::min)
        });
        // The team's own ore: the fields in its third, by the north team's.
        let own = twins.map(|i| {
            let k = i / 4;
            fields
                .iter()
                .filter(|&&c| {
                    let (dx, dy) = (c.0 - map.mid.0, c.1 - map.mid.1);
                    let a = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                    (30.0..150.0).contains(&a)
                })
                .map(|&c| metres(at(&walks[i], map.turn(c, k))))
                .sum::<f64>()
        });
        for (what, v, most) in [
            // (The ground is natural, not turned: a base's way to the
            // middle or to an enemy wanders more than its way to its ore.)
            ("to the island", island, 0.12),
            ("to the nearest enemy", enemy, 0.17),
            ("to its team's ore", own, 0.08),
        ] {
            println!(
                "slot {slot} {what}: {:.0} / {:.0} / {:.0} m ({:.1}% apart)",
                v[0],
                v[1],
                v[2],
                100.0 * spread(v)
            );
            worst = worst.max(spread(v));
            if spread(v) > most {
                problems.push(format!("slot {slot}'s walks {what} differ: {v:?}"));
            }
        }
    }
    println!("the teams' walks differ by {:.1}% at most", 100.0 * worst);

    // One sea round the map and up the estuaries; the lake not sailable.
    let corner = (2u32, 2u32);
    let sea = map.flood(corner, |x, y| map.sea_cell(x, y));
    let mut stray = 0usize;
    for cy in 0..h {
        for cx in 0..w {
            if map.sea_cell(cx, cy) && sea[(cy * w + cx) as usize] < 0 {
                stray += 1;
            }
        }
    }
    let lake = map.cell_of(map.mid);
    let (lx, ly) = (lake.0, lake.1 + (1_000.0 / cell) as u32);
    if map.sea_cell(lx, ly) {
        problems.push("the lake is deep enough for ships".into());
    }
    let km2 = |n: usize| n as f64 * cell * cell / 1e6;
    println!(
        "{:.2} km² of water a ship could sail cut off from the sea",
        km2(stray)
    );
    if km2(stray) > 0.5 {
        problems.push(format!("{:.2} km² of sailable water cut off", km2(stray)));
    }

    // Deciduous woods: most trees broadleaf (the autumn map turns them).
    let trees: Vec<PropKind> = file
        .props()
        .iter()
        .map(|p| p.kind)
        .filter(|k| k.is_tree())
        .collect();
    let broadleaf = trees
        .iter()
        .filter(|&&k| k == PropKind::TreeBroadleaf)
        .count();
    let pines = trees.iter().filter(|&&k| k == PropKind::TreePine).count();
    println!(
        "{} trees: {broadleaf} broadleaf, {pines} pine, {} other",
        trees.len(),
        trees.len() - broadleaf - pines
    );
    if (broadleaf as f64) < 0.6 * trees.len() as f64 {
        problems.push(format!(
            "only {broadleaf} of {} trees broadleaf",
            trees.len()
        ));
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Where units can walk, one pixel a cell, north up.
fn write_walk(map: &Map, walk: &[i32], out: &str) {
    let mut ppm = format!("P6 {} {} 255\n", map.w, map.h).into_bytes();
    for cy in (0..map.h).rev() {
        for cx in 0..map.w {
            let i = (cy * map.w + cx) as usize;
            let rgb = if map.sea_cell(cx, cy) {
                [40, 70, 140]
            } else if walk[i] >= 0 {
                // Lighter the nearer, banded every kilometre.
                let km = walk[i] as f64 / 100.0 * CELL_SIZE_M as f64 / 1_000.0;
                let v =
                    200.0 - 120.0 * (km / 25.0).min(1.0) - 25.0 * (km.fract() < 0.08) as u8 as f64;
                [v as u8, v as u8, v as u8]
            } else if map.land_cell(cx, cy) {
                [150, 60, 60]
            } else {
                [0, 0, 0]
            };
            ppm.extend(rgb);
        }
    }
    std::fs::write(out, ppm).unwrap();
}
