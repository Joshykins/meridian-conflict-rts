//! A block on attack-move keeps the members that cannot hurt the enemy (anti-air,
//! shields) behind the members that are firing, instead of marching them on past.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

fn row_of(w: &World, key: &str, owner: u8) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.owner[r] == owner && w.state.units.blueprint[r] == id)
        .unwrap()
}

#[test]
fn anti_air_and_shields_stop_behind_the_firing_line() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 400, 500, flag::INVULNERABLE),
        spawn(&w, 0, "aster_t1_tank", 400, 524, flag::INVULNERABLE),
        spawn(&w, 0, "aster_t1_mobile_aa", 380, 500, flag::INVULNERABLE),
        spawn(
            &w,
            0,
            "aster_t2_mobile_shield",
            380,
            524,
            flag::INVULNERABLE,
        ),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            1200,
            512,
            flag::PASSIVE | flag::INVULNERABLE,
        ),
    ])
    .unwrap();
    let ours: Vec<_> = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == 0)
        .map(|r| w.state.units.id(r))
        .collect();
    let goal = FxVec2::from_ints(1500, 512);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::AttackMove {
            units: ours,
            target: goal,
            queue: false,
        },
    }])
    .unwrap();
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let x = |row: usize| w.state.units.pos[row].x;
    let tank = row_of(&w, "aster_t1_tank", 0);
    let aa = row_of(&w, "aster_t1_mobile_aa", 0);
    let shield = row_of(&w, "aster_t2_mobile_shield", 0);
    assert!(
        x(tank) < Fx::from_int(1200),
        "the tanks stop to fire, at {:?}",
        x(tank)
    );
    for (what, row) in [("anti-air", aa), ("shield", shield)] {
        assert!(
            x(row) <= x(tank) + Fx::from_int(4),
            "the {what} walked past the firing tanks: {:?} against {:?}",
            x(row),
            x(tank)
        );
        assert!(
            x(row) > Fx::from_int(600),
            "the {what} came up with the block: {:?}",
            x(row)
        );
    }
}
