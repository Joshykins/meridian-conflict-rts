//! Surge shells (`Weapon::surge`, `surge.rs`): the Regency Saker's shell strikes hard
//! where it lands, then a wake rolls out all round from the crater and burns lightly and
//! evenly over the wide disc it has rolled over for a while, never a friend or an aircraft.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};

const TPS: u32 = TICKS_PER_SECOND;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::surge::SurgeWake;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
const SAKER: &str = "regency_t4_surge_artillery";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let side = CELLS as usize + 1;
    let terrain = Heightfield::from_samples(
        CELLS,
        CELLS,
        vec![20; side * side],
        Fx::ZERO,
        Fx::ONE,
        Fx::ZERO,
    );
    let map = MapData {
        doodads: Default::default(),
        name: "surge".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state
        .units
        .row(id)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

fn full(w: &World, key: &str) -> Fx {
    w.blueprints.unit(w.blueprints.id_of(key).unwrap()).health
}

/// `id` of `player` stands where it is and holds its fire.
fn hold(w: &mut World, player: u8, id: UnitId) {
    w.tick(&[
        PlayerCommand {
            player,
            command: Command::Stop { units: vec![id] },
        },
        PlayerCommand {
            player,
            command: Command::SetFireState {
                units: vec![id],
                state: FireState::HoldFire,
            },
        },
    ])
    .unwrap();
}

/// A Saker's wake let go at `(x, y)` by `source`, as its shell would.
fn let_go(w: &mut World, source: UnitId, x: i32, y: i32) {
    let blueprint = w.blueprints.id_of(SAKER).unwrap();
    w.state.surges.push(SurgeWake {
        pos: FxVec2::from_ints(x, y),
        z: Fx::from_int(20),
        owner: 0,
        source,
        blueprint,
        weapon: 0,
        age: 0,
    });
}

#[test]
fn the_shell_strikes_hard_and_its_wake_burns_round_it() {
    let mut w = world();
    let saker = add(&mut w, SAKER, 0, 200, 1000);
    // 700 m off, where the Surge Mortar's spread keeps the shell within its splash.
    let target = add(&mut w, "aster_t3_assault_bot", 1, 900, 1000);
    let wide = add(&mut w, "aster_t3_assault_bot", 1, 900, 1140);
    let friend = add(&mut w, "regency_t1_tank", 0, 800, 1100);
    for (player, id) in [(1, target), (1, wide), (0, friend)] {
        hold(&mut w, player, id);
    }
    let before = health(&w, wide);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![saker],
            target,
            queue: false,
        },
    }])
    .unwrap();
    let saker_bp = w.blueprints.id_of(SAKER).unwrap();
    let mut landed = false;
    for _ in 0..TPS * 60 {
        w.tick(&[]).unwrap();
        if w.events.iter().any(|e| {
            matches!(e, SimEvent::Impact { blueprint, on_shield: false, .. } if *blueprint == saker_bp)
        }) {
            landed = true;
            break;
        }
    }
    assert!(landed, "the Saker never landed a shell");
    assert_eq!(w.state.surges.len(), 1, "no wake rolled out of the crater");
    let struck = full(&w, "aster_t3_assault_bot") - health(&w, target);
    assert!(
        struck > Fx::from_int(1000),
        "the shell's strike dealt only {struck:?}"
    );
    assert_eq!(
        health(&w, wide),
        before,
        "a hull 140 m out took the shell's own blast"
    );

    // The wake reaches 140 m out in a little over two seconds and burns there.
    for _ in 0..TPS * 6 {
        w.tick(&[]).unwrap();
    }
    assert!(health(&w, wide) < before, "the wake never burnt 140 m out");
    assert_eq!(
        health(&w, friend),
        full(&w, "regency_t1_tank"),
        "a friend in the wake was burnt"
    );
}

#[test]
fn the_burn_is_light_even_and_spares_aircraft_then_it_is_gone() {
    let mut w = world();
    let source = add(&mut w, SAKER, 0, 200, 200);
    let near = add(&mut w, "aster_t3_assault_bot", 1, 1000, 1030);
    let edge = add(&mut w, "aster_t3_assault_bot", 1, 1000, 1170);
    let outside = add(&mut w, "aster_t3_assault_bot", 1, 1000, 1260);
    let plane = add(&mut w, "aster_t1_air_scout", 1, 1100, 1000);
    for (player, id) in [(1, near), (1, edge), (1, outside), (1, plane)] {
        hold(&mut w, player, id);
    }
    let plane_before = health(&w, plane);
    let_go(&mut w, source, 1000, 1000);
    let surge = w
        .blueprints
        .unit(w.blueprints.id_of(SAKER).unwrap())
        .weapons[0]
        .surge
        .expect("the Saker's mortar is a surge shell");
    let life = surge.life_ticks() as u32;
    for _ in 0..life {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.surges.is_empty(), "the wake outlived its linger");

    let full_hp = full(&w, "aster_t3_assault_bot");
    let took = |id| full_hp - health(&w, id);
    // Light: the whole burn is a few hundred at most, a small share of a tech 3 hull.
    let most = surge.damage * Fx::from_int(life as i32) / Fx::from_int(TPS as i32);
    assert!(took(near) > Fx::ZERO && took(near) <= most);
    assert!(took(near) * 5 < full_hp, "the burn took {:?}", took(near));
    // Even: the near hull was burnt from the start, the far one only once the wake got
    // there; both took the same each tick they stood in it.
    assert!(
        took(edge) > Fx::ZERO,
        "a hull at the wake's edge took nothing"
    );
    assert!(took(edge) < took(near));
    assert_eq!(took(outside), Fx::ZERO, "a hull past the wake was burnt");
    assert_eq!(
        health(&w, plane),
        plane_before,
        "the wake burnt an aircraft"
    );
}
