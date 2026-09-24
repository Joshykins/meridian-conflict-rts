//! The flow economy's stall rule: upkeep and the building of power and mines are
//! paid first, so a side short of energy can always build its way out.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "economy".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1800, 1800)],
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
        seed: 9,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(128, 128, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// One `key` of player 0's at (x, y), `build` permille done; its id.
fn spawn(w: &mut World, key: &str, x: i32, y: i32, build: u16) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint,
        pos: at,
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build,
    })])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == 0)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

#[test]
fn power_is_built_at_full_rate_while_the_rest_stalls() {
    let mut w = world();
    // 20 energy a second from one reactor, no energy in store, mass to spare.
    spawn(&mut w, "aster_t1_power", 300, 300, 1000);
    spawn(&mut w, "aster_mass_storage", 330, 300, 1000);
    let reactor = spawn(&mut w, "aster_t1_power", 500, 500, 100);
    let factory = spawn(&mut w, "aster_t1_land_factory", 600, 500, 100);
    let a = spawn(&mut w, "aster_t1_engineer", 520, 470, 1000);
    let b = spawn(&mut w, "aster_t1_engineer", 580, 470, 1000);
    w.tick(&[
        cmd(Command::Assist {
            units: vec![a],
            target: reactor,
            queue: false,
        }),
        cmd(Command::Assist {
            units: vec![b],
            target: factory,
            queue: false,
        }),
    ])
    .unwrap();
    for _ in 0..20 {
        w.state.players[0].energy = Fx::ZERO;
        w.state.players[0].mass = Fx::from_int(1000);
        w.tick(&[]).unwrap();
    }
    let progress =
        |w: &World, id: UnitId| w.state.units.build_progress[w.state.units.row(id).unwrap()];
    let (r0, f0) = (progress(&w, reactor), progress(&w, factory));
    for _ in 0..50 {
        w.state.players[0].energy = Fx::ZERO;
        w.state.players[0].mass = Fx::from_int(1000);
        w.tick(&[]).unwrap();
    }
    // The reactor wants 18 energy a second from the 20 there is: all of it. The factory
    // wants 35 and shares the 2 left over.
    let reactor_rate = (progress(&w, reactor) - r0).to_f64() / 5.0;
    let factory_rate = (progress(&w, factory) - f0).to_f64() / 5.0;
    assert!(
        (reactor_rate - 5.0).abs() < 0.1,
        "reactor built at {reactor_rate} a second"
    );
    assert!(
        factory_rate > 0.0 && factory_rate < 0.6,
        "factory built at {factory_rate} a second"
    );
    assert!(w.state.players[0].efficiency < Fx::ONE);
}
