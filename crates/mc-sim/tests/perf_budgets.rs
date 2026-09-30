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

/// A flat 16 km map with `players` starts on a 3 km ring round the centre.
fn ring_world(players: u8, fog: bool) -> World {
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
    World::with_terrain(
        terrain,
        map,
        blueprints,
        Arc::new(Pool::with_default_threads()),
        &config,
    )
    .unwrap()
}

fn ring_point(p: u8, players: u8, radius: f32) -> FxVec2 {
    let a = p as f32 / players as f32 * std::f32::consts::TAU;
    FxVec2::from_ints(
        8192 + (a.cos() * radius) as i32,
        8192 + (a.sin() * radius) as i32,
    )
}

/// `total` units shared out between `players` armies, each four blocks of tanks,
/// bots, heavy tanks and interceptors `gap` metres apart, `radius` metres from
/// the centre. With `fight` every army attack-moves on the centre.
fn armies(players: u8, total: u32, radius: f32, gap: i32, fight: bool) -> World {
    let mut w = ring_world(players, true);
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
        orders.push(attack_move(p, ids, (8192, 8192)));
    }
    if fight {
        w.tick(&orders).unwrap();
    }
    w
}

/// Eight armies of 500 meeting in the middle: the counter budgets catch a
/// query that stops scaling with the crowd around it.
#[test]
fn eight_armies_clash() {
    let mut w = armies(8, 4000, 900.0, 6, true);
    let report = w
        .perf_ticks("eight_armies_clash", 200, |_, _| Vec::new())
        .unwrap();
    mc_sim::perf::save(&report);
    mc_sim::perf::budget(&report, &[("sim.tick", 40.0)]);
}

/// How the tick grows with unit and player count, for the 30 000 unit goal.
/// Not a budget: it prints a table per case.
///
/// `MERIDIAN_SCALE="8:4000 8:8000:idle 32:30000" cargo test --profile gate -p mc-sim
/// --test perf_budgets -- --ignored zz_scale_probe --nocapture`
///
/// Each case is `players:units`, fighting in the middle; `:idle` spreads the
/// armies 20 m apart on the start ring and gives no orders.
#[test]
#[ignore]
fn zz_scale_probe() {
    let cases = std::env::var("MERIDIAN_SCALE").unwrap_or_else(|_| "8:2000 8:8000".into());
    for case in cases.split_whitespace() {
        let parts: Vec<&str> = case.split(':').collect();
        let players: u8 = parts[0].parse().unwrap();
        let total: u32 = parts[1].parse().unwrap();
        let idle = parts.get(2) == Some(&"idle");
        let mut w = if idle {
            armies(players, total, 3000.0, 20, false)
        } else {
            armies(players, total, 900.0, 6, true)
        };
        let report = w
            .perf_ticks(&format!("scale {case}"), 300, |_, _| Vec::new())
            .unwrap();
        println!("{}", report.text());
    }
}
