//! Volatile units: a reactor that is destroyed blows up and hurts every unit in
//! reach, its owner's too; one that was never finished does not.

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
        name: "volatile".into(),
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

/// One `key` of `owner`'s at (x, y), `build` permille done; its id.
fn spawn(w: &mut World, owner: u8, key: &str, x: i32, y: i32, build: u16) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint,
            pos: at,
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build,
        },
    }])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

fn health(w: &World, id: UnitId) -> Option<f32> {
    w.state
        .units
        .row(id)
        .map(|r| w.state.units.health[r].to_f32())
}

fn destroy(w: &mut World, id: UnitId) {
    let row = w.state.units.row(id).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    w.tick(&[]).unwrap();
}

#[test]
fn a_reactor_going_up_hurts_friend_and_foe_in_reach() {
    let mut w = world();
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t3_power").unwrap())
        .clone();
    let blast = bp.death_blast.expect("the tech 3 reactor is volatile");
    assert!(bp.volatile());
    let reactor = spawn(&mut w, 0, "aster_t3_power", 800, 800, 1000);
    // Near: well inside the full-damage half. Edge: in reach, a quarter of the damage. Far: out of it.
    let near_friend = spawn(&mut w, 0, "aster_t2_tank", 800, 860, 1000);
    let near_foe = spawn(&mut w, 1, "aster_t2_tank", 860, 800, 1000);
    let edge = spawn(&mut w, 0, "aster_t3_artillery", 800 - 115, 800, 1000);
    let far = spawn(&mut w, 1, "aster_t1_tank", 800, 1000, 1000);
    let edge_before = health(&w, edge).unwrap();
    destroy(&mut w, reactor);
    w.tick(&[]).unwrap();
    assert!(health(&w, reactor).is_none());
    assert!(
        health(&w, near_friend).is_none(),
        "its owner's tank next to it is gone"
    );
    assert!(
        health(&w, near_foe).is_none(),
        "an enemy tank next to it is gone"
    );
    let lost = edge_before - health(&w, edge).expect("the gun at the edge survives");
    let full = blast.damage.to_f32();
    assert!(
        lost > full * 0.2 && lost < full * 0.6,
        "edge took {lost} of {full}"
    );
    assert_eq!(
        health(&w, far),
        Some(
            w.blueprints
                .unit(w.blueprints.id_of("aster_t1_tank").unwrap())
                .health
                .to_f32()
        )
    );
}

#[test]
fn an_unfinished_reactor_does_not_go_up() {
    let mut w = world();
    let reactor = spawn(&mut w, 0, "aster_t2_power", 800, 800, 500);
    let friend = spawn(&mut w, 0, "aster_t1_tank", 800, 840, 1000);
    let before = health(&w, friend).unwrap();
    destroy(&mut w, reactor);
    w.tick(&[]).unwrap();
    assert_eq!(health(&w, friend), Some(before));
}

#[test]
fn a_row_of_reactors_goes_up_one_after_another() {
    let mut w = world();
    let ids: Vec<_> = (0..4)
        .map(|i| spawn(&mut w, 0, "aster_t1_power", 600 + 24 * i, 600, 1000))
        .collect();
    // Worn down, so one neighbour's blast is enough to set each off.
    for &id in &ids[1..] {
        let row = w.state.units.row(id).unwrap();
        w.state.units.health[row] = Fx::from_int(100);
    }
    destroy(&mut w, ids[0]);
    for _ in 0..6 {
        w.tick(&[]).unwrap();
    }
    assert!(
        ids.iter().all(|&id| health(&w, id).is_none()),
        "the chain ran down the row"
    );
}
