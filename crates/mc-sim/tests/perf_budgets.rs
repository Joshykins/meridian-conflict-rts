//! Stress scenes held to a cost budget, so a change that makes a system blow up
//! with unit count fails here instead of in a match.
//!
//! `cargo test --release -p mc-sim --test perf_budgets -- --nocapture --test-threads=1`
//! prints each scene's table with `MERIDIAN_PERF_PRINT=1`; `MERIDIAN_PERF_DIR=dir`
//! also saves them as JSON for `mc-perf diff`. Time budgets assume a release
//! build: debug builds should run with `MERIDIAN_PERF_NO_BUDGET=1`. Counter
//! budgets (queries, rays, steps) hold in any build.

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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::with_default_threads()), &config)
        .unwrap()
}

/// A block of `count` units of `key`, `gap` metres apart, facing `heading`.
fn block(w: &mut World, key: &str, owner: u8, count: u32, at: (i32, i32), gap: i32, heading: i32) -> Vec<UnitId> {
    let id = w.blueprints.id_of(key).unwrap();
    let side = (count as f32).sqrt().ceil() as u32;
    (0..count)
        .map(|i| {
            let (x, y) = (at.0 + (i % side) as i32 * gap, at.1 + (i / side) as i32 * gap);
            let row = w
                .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::from_degrees(heading), true)
                .unwrap();
            w.state.units.id(row)
        })
        .collect()
}

fn attack_move(player: u8, units: Vec<UnitId>, to: (i32, i32)) -> PlayerCommand {
    PlayerCommand {
        player,
        command: Command::AttackMove { units, target: FxVec2::from_ints(to.0, to.1), queue: false },
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
    let red = block(&mut w, "aster_t3_assault_bot", 1, 100, (2300, 1700), 14, 180);
    let report = w
        .perf_ticks("paladins_vs_paladins", 300, |t, _| match t {
            0 => vec![attack_move(0, blue.clone(), (2400, 1770)), attack_move(1, red.clone(), (1500, 1770))],
            _ => Vec::new(),
        })
        .unwrap();
    mc_sim::perf::save(&report);
    mc_sim::perf::budget(&report, &[("sim.tick", 10.0)]);
}
