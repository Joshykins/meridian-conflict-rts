//! Stress scenes held to a cost budget, so a change that makes a system blow up
//! with unit count fails here instead of in a match.
//!
//! `cargo test --release -p mc-sim --test perf_budgets -- --nocapture --test-threads=1`
//! prints each scene's table with `MERIDIAN_PERF_PRINT=1`; `MERIDIAN_PERF_DIR=dir`
//! also saves them as JSON for `mc-perf diff`. Time budgets assume a release
//! build: debug builds should run with `MERIDIAN_PERF_NO_BUDGET=1`. Counter
//! budgets (queries, rays, steps) hold in any build. `zz_scale_probe` (ignored)
//! measures how the tick grows toward 30 000 units.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world(size_cells: u32) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(size_cells, size_cells, Fx::from_int(20));
    let map = MapData {
        name: "perf".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
        seed: 7,
        players: vec![player("blue", 0), player("red", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        terrain,
        map,
        blueprints,
        Arc::new(Pool::with_default_threads()),
        &config,
    )
    .unwrap()
}

/// A block of `count` units of `key`, `gap` metres apart, facing `heading`.
fn block(
    w: &mut World,
    key: &str,
    owner: u8,
    count: u32,
    at: (i32, i32),
    gap: i32,
    heading: i32,
) -> Vec<UnitId> {
    let id = w.blueprints.id_of(key).unwrap();
    let side = (count as f32).sqrt().ceil() as u32;
    (0..count)
        .map(|i| {
            let (x, y) = (
                at.0 + (i % side) as i32 * gap,
                at.1 + (i / side) as i32 * gap,
            );
            let row = w
                .spawn_unit(
                    id,
                    owner,
                    FxVec2::from_ints(x, y),
                    Angle::from_degrees(heading),
                    true,
                )
                .unwrap();
            w.state.units.id(row)
        })
        .collect()
}

fn attack_move(player: u8, units: Vec<UnitId>, to: (i32, i32)) -> PlayerCommand {
    PlayerCommand {
        player,
        command: Command::AttackMove {
            units,
            target: FxVec2::from_ints(to.0, to.1),
            queue: false,
        },
    }
}

/// The user's lag report of 2026-09-25: a hundred Paladins against one Behemoth.
#[test]
fn paladins_vs_titan() {
    let mut w = world(1024);
    let paladins = block(&mut w, "aster_t3_assault_bot", 0, 100, (1500, 1700), 14, 0);
    let titan = block(&mut w, "aster_t5_titan", 1, 1, (2300, 1770), 0, 180);
    let report = w
        .perf_ticks("paladins_vs_titan", 300, |t, _| match t {
            0 => vec![
                attack_move(0, paladins.clone(), (2400, 1770)),
                attack_move(1, titan.clone(), (1500, 1770)),
            ],
            _ => Vec::new(),
        })
        .unwrap();
    mc_sim::perf::save(&report);
    mc_sim::perf::budget(&report, &[("sim.tick", 10.0)]);
}

/// The same fight without the giant, as the yardstick for what it adds.
#[test]
fn paladins_vs_paladins() {
    let mut w = world(1024);
    let blue = block(&mut w, "aster_t3_assault_bot", 0, 100, (1500, 1700), 14, 0);
    let red = block(
        &mut w,
        "aster_t3_assault_bot",
        1,
        100,
        (2300, 1700),
        14,
        180,
    );
    let report = w
        .perf_ticks("paladins_vs_paladins", 300, |t, _| match t {
            0 => vec![
                attack_move(0, blue.clone(), (2400, 1770)),
                attack_move(1, red.clone(), (1500, 1770)),
            ],
            _ => Vec::new(),
        })
        .unwrap();
    mc_sim::perf::save(&report);
    mc_sim::perf::budget(&report, &[("sim.tick", 10.0)]);
}

/// A flat 16 km map with `players` starts on a 3 km ring round the centre,
/// on a pool of `threads` workers (all the machine's with `None`).
fn ring_world(players: u8, fog: bool, threads: Option<usize>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(2048, 2048, Fx::from_int(20));
    let starts = (0..players)
        .map(|p| ring_point(p, players, 3000.0))
        .collect();
    let map = MapData {
        name: "ring".into(),
        content_id: 1,
        ore: Vec::new(),
        starts,
        props: Vec::new(),
    };
    let config = MatchConfig {
        seed: 7,
        players: (0..players)
            .map(|p| PlayerSetup {
                name: format!("p{p}"),
                faction: "Aster".into(),
                ai: Default::default(),
                team: p,
                controller: Controller::Human,
                start: p,
            })
            .collect(),
        cheats: true,
        fog,
        spawn_commanders: false,
    };
    let pool = threads.map_or_else(Pool::with_default_threads, Pool::new);
    World::with_terrain(terrain, map, blueprints, Arc::new(pool), &config).unwrap()
}

fn ring_point(p: u8, players: u8, radius: f32) -> FxVec2 {
    let a = p as f32 / players as f32 * std::f32::consts::TAU;
    FxVec2::from_ints(
        8192 + (a.cos() * radius) as i32,
        8192 + (a.sin() * radius) as i32,
    )
}

/// Where the armies of [`armies`] go.
#[derive(Clone, Copy, PartialEq)]
enum Plan {
    /// Nowhere: they stand.
    Idle,
    /// Every army on the centre.
    Pile,
    /// Each army on the one beside it on the ring: one battle per pair of sides.
    Pairs,
}

/// `total` units shared out between `players` armies, each four blocks of tanks,
/// bots, heavy tanks and interceptors `gap` metres apart, `radius` metres from
/// the centre, sent as `plan` says.
fn armies(
    players: u8,
    total: u32,
    radius: f32,
    gap: i32,
    plan: Plan,
    threads: Option<usize>,
) -> World {
    let mut w = ring_world(players, true, threads);
    let per = total / players as u32;
    let mix = [
        ("aster_t1_tank", 4),
        ("aster_t1_bot", 3),
        ("aster_t2_tank", 2),
        ("aster_t1_interceptor", 1),
    ];
    let mut orders = Vec::new();
    for p in 0..players {
        let at = ring_point(p, players, radius);
        let (x, y) = (at.x.floor_int(), at.y.floor_int());
        let block_side = ((per / 3) as f32).sqrt() as i32 * gap + 50;
        let mut ids = Vec::new();
        for (i, (key, share)) in mix.iter().enumerate() {
            let corner = (
                x + (i as i32 % 2) * block_side - block_side,
                y + (i as i32 / 2) * block_side - block_side,
            );
            ids.extend(block(&mut w, key, p, per * share / 10, corner, gap, 0));
        }
        let to = match plan {
            Plan::Pairs => {
                let (a, b) = (
                    ring_point(p, players, radius),
                    ring_point(p ^ 1, players, radius),
                );
                ((a.x + b.x).floor_int() / 2, (a.y + b.y).floor_int() / 2)
            }
            _ => (8192, 8192),
        };
        orders.push(attack_move(p, ids, to));
    }
    if plan != Plan::Idle {
        w.tick(&orders).unwrap();
    }
    w
}

/// Eight armies of 500 meeting in the middle: the counter budgets catch a
/// query that stops scaling with the crowd around it.
#[test]
fn eight_armies_clash() {
    let mut w = armies(8, 4000, 900.0, 6, Plan::Pile, None);
    let report = w
        .perf_ticks("eight_armies_clash", 200, |_, _| Vec::new())
        .unwrap();
    mc_sim::perf::save(&report);
    // Counts, not milliseconds: they hold on a machine busy with other builds.
    // The time budget only catches a collapse.
    mc_sim::perf::budget(
        &report,
        &[
            ("spatial.tested", 750_000.0),
            ("targeting>spatial.tested", 60_000.0),
            ("orders>spatial.tested", 25_000.0),
            ("sim.tick", 250.0),
        ],
    );
}

/// How the tick grows with unit and player count, for the 30 000 unit goal.
/// Not a budget: it prints a table per case.
///
/// `MERIDIAN_SCALE="8:4000 32:30000:pairs 32:30000:idle" cargo test --profile gate
/// -p mc-sim --test perf_budgets -- --ignored zz_scale_probe --nocapture`
///
/// Each case is `players:units`, every army fighting in the middle (the worst
/// crowd there is); `:pairs` sets each army on its neighbour on a 3 km ring,
/// one battle per two sides as a big match plays; `:idle` spreads the armies
/// 20 m apart on that ring and gives no orders.
///
/// With `MERIDIAN_SCALE_SERIAL=1` the tick runs on this thread alone and the
/// table ends with its CPU time per tick, which a machine busy with other
/// work barely moves: the number to compare two versions by.
#[test]
#[ignore]
fn zz_scale_probe() {
    let cases = std::env::var("MERIDIAN_SCALE").unwrap_or_else(|_| "8:2000 8:8000".into());
    for case in cases.split_whitespace() {
        let parts: Vec<&str> = case.split(':').collect();
        let players: u8 = parts[0].parse().unwrap();
        let total: u32 = parts[1].parse().unwrap();
        let serial = std::env::var("MERIDIAN_SCALE_SERIAL").is_ok_and(|v| v == "1");
        let threads = serial.then_some(0);
        let mut w = match parts.get(2) {
            Some(&"idle") => armies(players, total, 3000.0, 20, Plan::Idle, threads),
            Some(&"pairs") => armies(players, total, 3000.0, 6, Plan::Pairs, threads),
            _ => armies(players, total, 900.0, 6, Plan::Pile, threads),
        };
        let cpu_before = thread_cpu_ns();
        let report = w
            .perf_ticks(&format!("scale {case}"), 300, |_, _| Vec::new())
            .unwrap();
        println!("{}", report.text());
        if serial {
            let per_tick = (thread_cpu_ns() - cpu_before) as f64 / 300.0 / 1e6;
            println!("   cpu ms per tick (this thread): {per_tick:.2}");
        }
    }
}

/// One order to a whole army of 6000: the user's report of 2026-09-30 that a
/// selection of 3000 took no orders at all (the command was over a 1024-unit
/// cap and dropped). The order must go through and not stall the tick.
#[test]
fn one_order_for_an_army() {
    let mut w = ring_world(2, false, None);
    let mix = [
        ("aster_t1_tank", 3000),
        ("aster_t1_bot", 2000),
        ("aster_t2_tank", 1000),
    ];
    let mut ids = Vec::new();
    for (i, (key, n)) in mix.iter().enumerate() {
        ids.extend(block(
            &mut w,
            key,
            0,
            *n,
            (4000 + i as i32 * 700, 7000),
            8,
            0,
        ));
    }
    let order = PlayerCommand {
        player: 0,
        command: Command::FormationMove {
            units: ids.clone(),
            target: FxVec2::from_ints(9000, 9000),
            queue: false,
            attack_move: true,
            facing: None,
            together: true,
            spacing: 1,
        },
    };
    let encoded = order.command.encode();
    assert_eq!(Command::decode(&encoded).as_ref(), Some(&order.command));
    let report = w
        .perf_ticks("one_order_for_an_army", 100, |t, _| {
            if t == 0 {
                vec![order.clone()]
            } else {
                Vec::new()
            }
        })
        .unwrap();
    let moving = ids
        .iter()
        .filter(|&&id| {
            w.state
                .units
                .row(id)
                .is_some_and(|r| w.state.units.pos[r].y > Fx::from_int(7300))
        })
        .count();
    assert!(
        moving > ids.len() / 2,
        "only {moving} of {} moved off",
        ids.len()
    );
    mc_sim::perf::save(&report);
    mc_sim::perf::budget(
        &report,
        &[("spatial.tested", 3_300_000.0), ("sim.tick", 250.0)],
    );
}

/// CPU time this thread has run, nanoseconds (Linux; 0 elsewhere).
fn thread_cpu_ns() -> u64 {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}
