//! The Regency's tech 2 aircraft (`data/factions/regency/units/air_t2.ron`): the Voulge
//! fights from a circle out of reach of short-range anti-air and falls to fighters.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const VOULGE: &str = "regency_t2_strike_drone";
/// ARC's tech 2 mobile flak: the short-range anti-air the Voulge stands off from.
const SQUALL: &str = "aster_t2_mobile_aa";
const PEREGRINE: &str = "aster_t2_interceptor";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "regency air t2".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(400, 400), FxVec2::from_ints(3600, 3600)],
        props: Vec::new(),
    };
    let player = |faction: &str, team, start| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("Regency", 0, 0), player("Aster", 1, 1)],
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

fn ids(w: &World, owner: u8, key: &str) -> Vec<UnitId> {
    let (u, bp) = (&w.state.units, w.blueprints.id_of(key).unwrap());
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner && u.blueprint[r] == bp && u.health[r] > Fx::ZERO)
        .map(|r| u.id(r))
        .collect()
}

fn row(w: &World, id: UnitId) -> Option<usize> {
    w.state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.id(r) == id)
}

#[test]
fn the_voulge_circles_its_mark_out_of_reach_of_flak_and_kills_it() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, VOULGE, 600, 2000, 0),
        spawn(&w, 1, SQUALL, 2600, 2000, 0),
    ])
    .unwrap();
    let voulge = ids(&w, 0, VOULGE)[0];
    let squall = ids(&w, 1, SQUALL)[0];
    let whole = w.state.units.health[row(&w, voulge).unwrap()];
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![voulge],
            target: squall,
            queue: false,
        },
    }])
    .unwrap();
    let flak = w
        .blueprints
        .unit(w.blueprints.id_of(SQUALL).unwrap())
        .max_weapon_range();
    let reach = w
        .blueprints
        .unit(w.blueprints.id_of(VOULGE).unwrap())
        .max_weapon_range();
    let mut engaged = false;
    let mut nearest = Fx::from_int(100_000);
    let mut killed = false;
    for _ in 0..150 * TICKS_PER_SECOND as usize {
        w.tick(&[]).unwrap();
        let Some(v) = row(&w, voulge) else {
            panic!("the Voulge was shot down");
        };
        let Some(s) = row(&w, squall).filter(|&s| w.state.units.health[s] > Fx::ZERO) else {
            killed = true;
            break;
        };
        let gap = w.state.units.pos[v].distance(w.state.units.pos[s]);
        engaged |= gap <= reach;
        if engaged {
            nearest = nearest.min(gap);
        }
    }
    assert!(engaged, "the Voulge never came into reach");
    assert!(killed, "the Voulge never killed the Squall");
    assert!(
        nearest > flak + Fx::from_int(100),
        "the Voulge came within {nearest:?} m of a flak gun that reaches {flak:?}"
    );
    let v = row(&w, voulge).unwrap();
    // Its kill's veterancy may have raised it; nothing took any off.
    assert!(
        w.state.units.health[v] >= whole,
        "the flak never touched it"
    );
}

#[test]
fn a_fighter_runs_the_voulge_down() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, VOULGE, 1500, 2000, 0),
        spawn(&w, 1, PEREGRINE, 2300, 2000, 0),
    ])
    .unwrap();
    let voulge = ids(&w, 0, VOULGE)[0];
    for _ in 0..60 * TICKS_PER_SECOND as usize {
        w.tick(&[]).unwrap();
        if row(&w, voulge).is_none_or(|r| w.state.units.health[r] <= Fx::ZERO) {
            return;
        }
    }
    panic!("a Peregrine could not bring down a Voulge in a minute");
}
