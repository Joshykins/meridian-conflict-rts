//! Ground blocks crossing broken terrain: they keep their ranks round a
//! mountain's flank and only file through a pass that is truly narrow.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
const CELL: i32 = 8;
const PLAIN: i32 = 20;

/// Steep cones (slope 1, twice what a hull can climb) on a flat plain.
fn terrain(cones: &[(i32, i32, i32)], wall: Option<(i32, i32, i32)>) -> Heightfield {
    let n = CELLS as i32 + 1;
    let mut samples = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let (px, py) = (x * CELL, y * CELL);
            let mut h = PLAIN;
            for &(cx, cy, r) in cones {
                let d = (((px - cx).pow(2) + (py - cy).pow(2)) as f64).sqrt() as i32;
                h = h.max(PLAIN + (r - d).max(0));
            }
            // A ridge across the map at x, with a gap between y0 and y1.
            if let Some((wx, y0, y1)) = wall {
                if (px - wx).abs() <= 40 && (py < y0 || py > y1) {
                    h = h.max(PLAIN + 80 - (px - wx).abs());
                }
            }
            samples.push(h as u16);
        }
    }
    Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::from_int(-10))
}

/// Rolling ground with steep knolls scattered through it, from a fixed hash.
fn rough() -> Heightfield {
    let mut cones = Vec::new();
    let mut h: u32 = 0x9e37_79b9;
    for _ in 0..40 {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        let x = 550 + (h % 1000) as i32;
        let y = 780 + ((h >> 10) % 440) as i32;
        let r = 12 + ((h >> 20) % 30) as i32;
        cones.push((x, y, r));
    }
    terrain(&cones, None)
}

fn world(terrain: Heightfield) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(200, 200), FxVec2::from_ints(1900, 1900)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

#[derive(Debug)]
#[allow(dead_code)]
struct March {
    /// Ticks the group spent filing through (phase 3) while under way.
    filing: u32,
    travel: u32,
    /// Mean over the march of the worst member's distance from its rank.
    mean_distortion: f64,
    /// Share of member-ticks under way with a neighbour nearer than 4/5 of
    /// the formation's own spacing: ranks squeezed together.
    bunched: f64,
    /// Share of member-ticks squeezed while the whole formation, at its
    /// anchor, stood on open ground: bunching with room to spare.
    bunched_with_room: f64,
    /// Mean distance a member drove over what one tank alone drives there.
    detour: f64,
    /// Deepest overlap between two hulls seen during the march, metres.
    overlap: f64,
    arrived: Option<u32>,
}

/// A square block of `count` tanks centred on `at`.
fn spawn_block(w: &mut World, at: FxVec2, count: i32) -> Vec<mc_sim::UnitId> {
    let bp = w.blueprints.id_of("aster_t1_tank").unwrap();
    let cols = (count as f64).sqrt().ceil() as i32;
    let before: Vec<_> = w.state.units.slots.iter().collect();
    let spawns: Vec<_> = (0..count)
        .map(|i| {
            cmd(Command::DebugSpawn {
                owner: 0,
                blueprint: bp,
                pos: at
                    + FxVec2::from_ints(
                        (i % cols) * 22 - (cols - 1) * 11,
                        (i / cols) * 22 - (cols - 1) * 11,
                    ),
                heading: Angle::ZERO,
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            })
        })
        .collect();
    w.tick(&spawns).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|r| !before.contains(r))
        .map(|r| w.state.units.id(r))
        .collect()
}

/// How far one tank alone drives from `from` to `to`.
fn solo_path(mut w: World, from: FxVec2, to: FxVec2) -> f64 {
    let id = spawn_block(&mut w, from, 1)[0];
    w.tick(&[cmd(Command::Move {
        units: vec![id],
        target: to,
        queue: false,
    })])
    .unwrap();
    let mut path = 0.0;
    for _ in 0..4000 {
        let row = w.state.units.row(id).unwrap();
        if w.state.orders.front(&w.state.units, row).is_none() {
            break;
        }
        let before = w.state.units.pos[row];
        w.tick(&[]).unwrap();
        path += w.state.units.pos[row].distance(before).to_f64();
    }
    path
}

/// Moves the block to `to` and measures the march. `window` limits the rank
/// measurements to where the block's centre is between two x values.
fn run(
    mut w: World,
    ids: &[mc_sim::UnitId],
    to: FxVec2,
    window: Option<(i32, i32)>,
    trace: Option<String>,
) -> (March, World) {
    let mut lines = String::new();
    w.tick(&[cmd(Command::Move {
        units: ids.to_vec(),
        target: to,
        queue: false,
    })])
    .unwrap();
    let rows: Vec<_> = ids
        .iter()
        .map(|id| w.state.units.row(*id).unwrap())
        .collect();
    let identity = w
        .state
        .orders
        .front(&w.state.units, rows[0])
        .unwrap()
        .formation;
    let offsets: Vec<_> = rows
        .iter()
        .map(|&r| w.state.orders.front(&w.state.units, r).unwrap().offset)
        .collect();
    let mut spacing = f64::MAX;
    for (i, a) in offsets.iter().enumerate() {
        for b in &offsets[i + 1..] {
            spacing = spacing.min(a.distance(*b).to_f64());
        }
    }
    let radius = w
        .blueprints
        .unit(w.state.units.blueprint[rows[0]])
        .radius
        .to_f64();
    let (mut filing, mut travel, mut sum, mut arrived, mut overlap) = (0, 0, 0.0, None, 0.0f64);
    let (mut bunched, mut counted, mut path) = (0u32, 0u32, 0.0);
    let mut with_room = 0u32;
    for t in 0..4000 {
        let before: Vec<_> = rows.iter().map(|&r| w.state.units.pos[r]).collect();
        w.tick(&[]).unwrap();
        for (k, &r) in rows.iter().enumerate() {
            path += w.state.units.pos[r].distance(before[k]).to_f64();
        }
        if trace.is_some() && t % 8 == 0 {
            let phase = w.state.formations.get(&identity).map_or(9, |g| g.phase);
            for &r in &rows {
                let p = w.state.units.pos[r];
                let slot = match (
                    w.state.formations.get(&identity),
                    w.state.orders.front(&w.state.units, r),
                ) {
                    (Some(g), Some(o)) => g.anchor + o.offset.rotate(g.heading - o.heading),
                    _ => p,
                };
                lines += &format!(
                    "{t},{r},{},{},{phase},{},{}\n",
                    p.x.to_f64(),
                    p.y.to_f64(),
                    slot.x.to_f64(),
                    slot.y.to_f64()
                );
            }
        }
        if rows
            .iter()
            .all(|&r| w.state.orders.front(&w.state.units, r).is_none())
        {
            arrived = Some(t);
            break;
        }
        // Room: every slot of the formation, where it stands, is ground a tank drives, with a
        // hull's breadth free round it.
        let room = w.state.formations.get(&identity).is_some_and(|g| {
            g.phase != 3
                && rows.iter().all(|&r| {
                    let Some(o) = w.state.orders.front(&w.state.units, r) else {
                        return true;
                    };
                    let slot = g.anchor + o.offset.rotate(g.heading - o.heading);
                    let m = w
                        .blueprints
                        .unit(w.state.units.blueprint[r])
                        .motion
                        .unwrap();
                    let clear = Fx::from_int(radius as i32 + 4);
                    [
                        FxVec2::ZERO,
                        FxVec2::new(clear, Fx::ZERO),
                        FxVec2::new(-clear, Fx::ZERO),
                        FxVec2::new(Fx::ZERO, clear),
                        FxVec2::new(Fx::ZERO, -clear),
                    ]
                    .iter()
                    .all(|&d| w.nav.passable(m.layer, m.size_class, slot + d))
                })
        });
        for (i, &a) in rows.iter().enumerate() {
            let mut nearest = f64::MAX;
            for (j, &b) in rows.iter().enumerate() {
                if i != j {
                    let d = w.state.units.pos[a].distance(w.state.units.pos[b]).to_f64();
                    nearest = nearest.min(d);
                    overlap = overlap.max(radius * 2.0 - d);
                }
            }
            // Once the block has formed up from its spawn, until it settles.
            if t > 60 && w.state.orders.front(&w.state.units, a).is_some() {
                counted += 1;
                let squeezed = nearest < spacing * 0.8;
                bunched += squeezed as u32;
                with_room += (squeezed && room) as u32;
            }
        }
        let Some(g) = w.state.formations.get(&identity) else {
            continue;
        };
        let heading = g.heading;
        let phase = g.phase;
        let samples: Vec<_> = rows
            .iter()
            .filter_map(|&r| {
                let o = w.state.orders.front(&w.state.units, r)?;
                Some(w.state.units.pos[r] - o.offset.rotate(heading - o.heading))
            })
            .collect();
        let n = samples.len() as i32;
        let sum_p = samples.iter().copied().fold(FxVec2::ZERO, |a, b| a + b);
        let center = FxVec2::new(sum_p.x / n, sum_p.y / n);
        if let Some((x0, x1)) = window {
            if center.x < Fx::from_int(x0) || center.x > Fx::from_int(x1) {
                continue;
            }
        }
        travel += 1;
        filing += (phase == 3) as u32;
        let worst = samples.iter().map(|p| p.distance(center)).max().unwrap();
        sum += worst.to_f64();
    }
    if let Some(file) = trace {
        std::fs::write(file, lines).unwrap();
    }
    let m = March {
        filing,
        travel,
        mean_distortion: sum / travel.max(1) as f64,
        bunched: bunched as f64 / counted.max(1) as f64,
        bunched_with_room: with_room as f64 / counted.max(1) as f64,
        detour: path / rows.len() as f64,
        overlap,
        arrived,
    };
    (m, w)
}

/// A block of `count` tanks round (400, 1000) marched east to (1700, 1000).
fn march(name: &str, terrain: impl Fn() -> Heightfield, count: i32) -> March {
    let (from, to) = (FxVec2::from_ints(400, 1000), FxVec2::from_ints(1700, 1000));
    let mut w = world(terrain());
    let ids = spawn_block(&mut w, from, count);
    let trace = std::env::var("TRACE_DIR")
        .ok()
        .map(|d| format!("{d}/{}.csv", name.replace(' ', "_")));
    let (mut m, _) = run(w, &ids, to, Some((550, 1550)), trace);
    m.detour /= solo_path(world(terrain()), from, to);
    m
}

/// Every march must finish, and no two hulls may sink far into each other:
/// a crowd pressed against a slope can close up by a metre before the
/// contact passes spread it.
fn checked(name: &str, terrain: impl Fn() -> Heightfield, count: i32) -> March {
    let m = march(name, terrain, count);
    assert!(
        m.arrived.is_some(),
        "{name}: the block never arrived: {m:?}"
    );
    assert!(m.overlap < 1.0, "{name}: hulls overlapped: {m:?}");
    m
}

#[test]
fn a_block_skirts_a_mountain_flank_in_its_ranks() {
    for (count, y) in [(16, 1110), (36, 1150)] {
        let m = checked("graze", || terrain(&[(1000, y, 110)], None), count);
        assert_eq!(m.filing, 0, "{count} tanks broke ranks on a flank: {m:?}");
        assert!(m.mean_distortion < 1.0, "{count} tanks: {m:?}");
        assert!(
            m.bunched < 0.01,
            "{count} tanks squeezed along the flank: {m:?}"
        );
    }
}

#[test]
fn a_block_goes_round_a_mountain_ahead_in_one_piece() {
    for count in [16, 36] {
        let m = checked("astride", || terrain(&[(1000, 1000, 90)], None), count);
        assert_eq!(m.filing, 0, "{count} tanks split round the mountain: {m:?}");
        assert!(m.mean_distortion < 12.0, "{count} tanks: {m:?}");
        assert!(
            m.bunched_with_room < 0.01,
            "{count} tanks squeezed with room to spare: {m:?}"
        );
        assert!(
            m.detour < 1.05,
            "{count} tanks went far out of their way: {m:?}"
        );
    }
}

#[test]
fn scattered_knolls_do_not_break_the_block() {
    let knolls = [
        (750, 1060, 40),
        (900, 930, 35),
        (1100, 1050, 45),
        (1300, 950, 40),
        (1250, 1120, 30),
    ];
    for count in [16, 36] {
        let m = checked("hills", || terrain(&knolls, None), count);
        assert_eq!(m.filing, 0, "{count} tanks: {m:?}");
        assert!(m.mean_distortion < 10.0, "{count} tanks: {m:?}");
        assert!(
            m.bunched_with_room < 0.01,
            "{count} tanks squeezed with room to spare: {m:?}"
        );
        assert!(
            m.detour < 1.08,
            "{count} tanks went far out of their way: {m:?}"
        );
    }
    let m = checked("rough", rough, 16);
    assert!(
        m.filing * 10 < m.travel,
        "filed through most of the rough: {m:?}"
    );
    assert!(
        m.mean_distortion < 25.0,
        "strung out across the rough: {m:?}"
    );
    assert!(
        m.bunched_with_room < 0.01,
        "squeezed with room to spare: {m:?}"
    );
}

#[test]
fn a_pass_the_block_fits_keeps_its_ranks_and_a_slot_is_filed_through() {
    let m = checked("pass", || terrain(&[], Some((1000, 975, 1025))), 16);
    assert_eq!(m.filing, 0, "{m:?}");
    let m = checked("slot", || terrain(&[], Some((1000, 990, 1012))), 16);
    assert!(m.filing > 0, "a one-hull gap must be filed through: {m:?}");
}

/// `TRACE_DIR=... cargo test --test formation_terrain -- --ignored --nocapture`
/// writes every member's track per case, for plotting.
#[test]
#[ignore]
fn probe() {
    let knolls = [
        (750, 1060, 40),
        (900, 930, 35),
        (1100, 1050, 45),
        (1300, 950, 40),
        (1250, 1120, 30),
    ];
    let cases: Vec<(&str, Box<dyn Fn() -> Heightfield>)> = vec![
        ("flat", Box::new(|| terrain(&[], None))),
        ("graze", Box::new(|| terrain(&[(1000, 1110, 110)], None))),
        ("astride", Box::new(|| terrain(&[(1000, 1000, 90)], None))),
        ("hills", Box::new(move || terrain(&knolls, None))),
        (
            "wide pass",
            Box::new(|| terrain(&[], Some((1000, 900, 1100)))),
        ),
        (
            "narrow pass",
            Box::new(|| terrain(&[], Some((1000, 975, 1025)))),
        ),
        ("slot", Box::new(|| terrain(&[], Some((1000, 990, 1012))))),
        (
            "big graze",
            Box::new(|| terrain(&[(1000, 1150, 110)], None)),
        ),
        (
            "big astride",
            Box::new(|| terrain(&[(1000, 1000, 90)], None)),
        ),
        ("big hills", Box::new(move || terrain(&knolls, None))),
        ("rough", Box::new(rough)),
    ];
    for (name, t) in cases {
        let count = if name.starts_with("big") { 36 } else { 16 };
        let m = march(name, t, count);
        println!(
            "{name:12} filing {:4}/{:4} distortion {:6.2} bunched {:5.1}% (room {:5.1}%) detour {:5.1}% overlap {:.2} arrived {:?}",
            m.filing, m.travel, m.mean_distortion, m.bunched * 100.0, m.bunched_with_room * 100.0, (m.detour - 1.0) * 100.0, m.overlap, m.arrived
        );
    }
}

/// Marches across dev16's real relief, on routes whose straight line crosses
/// ground a tank cannot climb. `ROUTES` (8) and `BLOCK` (25 tanks) size the
/// run; `TRACE_DIR` gets each route's tracks, slots and the unclimbable cells
/// round it, for plotting.
#[test]
#[ignore]
fn real_map_probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = mc_map::MapFile::open(root.join("maps/dev16.mcmap")).unwrap();
    let relief = || Heightfield::load(&file).unwrap();
    let probe_world = world(relief());
    let tank = probe_world
        .blueprints
        .unit(probe_world.blueprints.id_of("aster_t1_tank").unwrap());
    let m = tank.motion.unwrap();
    let open = |p: FxVec2| probe_world.nav.passable(m.layer, m.size_class, p);
    let size = probe_world.terrain.size_metres().x.floor_int();
    let mut routes = Vec::new();
    let mut h: u32 = 0x1234_5678;
    let block: i32 = std::env::var("BLOCK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let count: usize = std::env::var("ROUTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    while routes.len() < count {
        h ^= h << 13;
        h ^= h >> 17;
        h ^= h << 5;
        let from = FxVec2::from_ints(
            1500 + (h % (size as u32 - 3000)) as i32,
            1500 + ((h >> 8) % (size as u32 - 3000)) as i32,
        );
        let to = from + FxVec2::from_angle(Angle((h >> 16) as u16)) * Fx::from_int(1400);
        // A clearing round the start for the block, open ground at the end.
        let reach = ((block as f64).sqrt().ceil() as i32 * 11 + 10) / 20;
        let clearing = (-reach..=reach)
            .all(|i| (-reach..=reach).all(|j| open(from + FxVec2::from_ints(i * 20, j * 20))));
        if !clearing || !open(to) {
            continue;
        }
        let steep = (0..=140)
            .filter(|&k| !open(from.lerp(to, Fx::ratio(k, 140))))
            .count();
        if (6..40).contains(&steep) {
            routes.push((from, to));
        }
    }
    let trace = std::env::var("TRACE_DIR").ok();
    let mut summary = (0.0, 0u32, 0u32, 0.0f64, 0.0, 0usize, 0.0);
    for (i, &(from, to)) in routes.iter().enumerate() {
        let mut w = world(relief());
        let ids = spawn_block(&mut w, from, block);
        let (mut m, _) = run(
            w,
            &ids,
            to,
            None,
            trace.as_ref().map(|d| format!("{d}/real{i}.csv")),
        );
        m.detour /= solo_path(world(relief()), from, to);
        summary.0 += m.bunched;
        summary.6 += m.bunched_with_room;
        summary.1 += m.filing;
        summary.2 += m.travel;
        summary.3 = summary.3.max(m.mean_distortion);
        summary.4 += m.detour - 1.0;
        summary.5 += m.arrived.is_some() as usize;
        println!(
            "route {i}: filing {:4}/{:4} distortion {:6.2} bunched {:5.1}% (room {:5.1}%) detour {:5.1}% overlap {:.2} arrived {:?}",
            m.filing, m.travel, m.mean_distortion, m.bunched * 100.0, m.bunched_with_room * 100.0, (m.detour - 1.0) * 100.0, m.overlap, m.arrived
        );
        if let Some(dir) = &trace {
            let lo = FxVec2::new(from.x.min(to.x), from.y.min(to.y)) - FxVec2::from_ints(900, 900);
            let hi = FxVec2::new(from.x.max(to.x), from.y.max(to.y)) + FxVec2::from_ints(900, 900);
            let mut cells = String::new();
            let mut y = lo.y.floor_int() / CELL * CELL;
            while y < hi.y.floor_int() {
                let mut x = lo.x.floor_int() / CELL * CELL;
                while x < hi.x.floor_int() {
                    if !open(FxVec2::from_ints(x + CELL / 2, y + CELL / 2)) {
                        cells += &format!("{x},{y}\n");
                    }
                    x += CELL;
                }
                y += CELL;
            }
            std::fs::write(format!("{dir}/real{i}_steep.csv"), cells).unwrap();
        }
    }
    let n = routes.len() as f64;
    println!(
        "SUMMARY bunched {:.1}% with room {:.1}% filing {:.1}% worst distortion {:.1} detour {:.1}% arrived {}/{}",
        summary.0 / n * 100.0, summary.6 / n * 100.0, summary.1 as f64 / summary.2 as f64 * 100.0, summary.3, summary.4 / n * 100.0, summary.5, routes.len()
    );
}

/// Movement phase cost with eight blocks of 25 tanks crossing dev16 at once.
#[test]
#[ignore]
fn real_map_movement_cost() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = mc_map::MapFile::open(root.join("maps/dev16.mcmap")).unwrap();
    let mut w = world(Heightfield::load(&file).unwrap());
    let bp = w.blueprints.id_of("aster_t1_tank").unwrap();
    let routes = [
        ((4972, 8571), (3675, 9099)),
        ((9473, 14758), (10202, 15952)),
        ((9541, 2786), (9715, 1396)),
        ((5268, 9304), (6663, 9192)),
        ((1585, 7774), (1945, 9126)),
        ((5568, 3502), (5541, 4901)),
        ((9184, 7228), (10557, 6955)),
        ((10928, 13404), (9754, 12640)),
    ];
    let mut orders = Vec::new();
    for &((fx, fy), (tx, ty)) in &routes {
        let before: Vec<_> = w.state.units.slots.iter().collect();
        let spawns: Vec<_> = (0..25)
            .map(|k| {
                cmd(Command::DebugSpawn {
                    owner: 0,
                    blueprint: bp,
                    pos: FxVec2::from_ints(fx + (k % 5) * 18 - 36, fy + (k / 5) * 18 - 36),
                    heading: Angle::ZERO,
                    count: 1,
                    flags: flag::PASSIVE,
                    build: 1000,
                })
            })
            .collect();
        w.tick(&spawns).unwrap();
        let ids: Vec<_> = w
            .state
            .units
            .slots
            .iter()
            .filter(|r| !before.contains(r))
            .map(|r| w.state.units.id(r))
            .collect();
        orders.push(cmd(Command::Move {
            units: ids,
            target: FxVec2::from_ints(tx, ty),
            queue: false,
        }));
    }
    w.tick(&orders).unwrap();
    let mut total = 0u64;
    let mut worst = 0u64;
    // CPU time of this single-threaded process, less swayed by a busy machine than the clock.
    let cpu = || {
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let fields: Vec<&str> = stat
            .rsplit(')')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        fields
            .get(11)
            .and_then(|u| u.parse::<u64>().ok())
            .unwrap_or(0)
            + fields
                .get(12)
                .and_then(|u| u.parse::<u64>().ok())
                .unwrap_or(0)
    };
    let cpu_before = cpu();
    for _ in 0..700 {
        w.tick(&[]).unwrap();
        let ns = w
            .timings
            .phases
            .iter()
            .find(|p| p.0 == "movement")
            .map_or(0, |p| p.1);
        total += ns;
        worst = worst.max(ns);
    }
    println!(
        "movement: mean {:.3} ms, worst {:.3} ms; ticks used {} ms of CPU",
        total as f64 / 700e6,
        worst as f64 / 1e6,
        (cpu() - cpu_before) * 10
    );
}
