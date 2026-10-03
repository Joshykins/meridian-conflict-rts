//! The Regency's tech 3 fusion guns. The Spire (`regency_t3_mobile_aa`) shoots spaceships
//! and nothing else, its gun raised to the sky. The Kiln (`regency_t3_artillery`) charges,
//! then lobs one fusion shot high onto a building over a kilometre off.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const SPIRE: &str = "regency_t3_mobile_aa";
const KILN: &str = "regency_t3_artillery";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "regency".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(4000, 4000)],
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
        seed: 7,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Spawns `key`; the other side's units hold their fire, so only the Regency guns shoot.
fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(
            id,
            owner,
            FxVec2::from_ints(x, y),
            Angle::from_degrees(heading),
            true,
        )
        .unwrap();
    if owner != 0 {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn shots(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp))
        .count()
}

#[test]
fn the_spire_shoots_down_at_a_warship_and_leaves_the_ground_alone() {
    let mut w = world();
    let _spire = add(&mut w, SPIRE, 0, 1000, 1000, 0);
    // A tank and a gunship close by, which it cannot touch, and a frigate 1.6 km off.
    let tank = add(&mut w, "aster_t1_tank", 1, 1200, 1000, 180);
    let gunship = add(&mut w, "aster_t1_rotor_gunship", 1, 1000, 1250, 180);
    let frigate = add(&mut w, "aster_t4_frigate", 1, 2600, 1000, 180);
    let (tank_hp, gunship_hp, frigate_hp) =
        (health(&w, tank), health(&w, gunship), health(&w, frigate));
    let mut fired = 0;
    for _ in 0..(40 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        fired += shots(&w, SPIRE);
    }
    assert!(fired >= 2, "the Spire fired {fired} times");
    assert!(
        health(&w, frigate) <= frigate_hp - 7000.0,
        "the frigate took {} of its {frigate_hp}",
        frigate_hp - health(&w, frigate)
    );
    assert_eq!(health(&w, tank), tank_hp, "it fired on a tank");
    assert_eq!(health(&w, gunship), gunship_hp, "it fired on an aircraft");
}

#[test]
fn the_kiln_lobs_its_charge_onto_a_far_building() {
    let mut w = world();
    let _kiln = add(&mut w, KILN, 0, 1000, 1000, 0);
    let target = add(&mut w, "aster_t1_power", 1, 2300, 1000, 0);
    let full = health(&w, target);
    let mut fired = 0;
    let mut hit_at = None;
    for t in 0..(60 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        fired += shots(&w, KILN);
        if hit_at.is_none() && health(&w, target) < full {
            hit_at = Some(t);
        }
    }
    assert!(fired >= 1, "the Kiln never fired");
    // It charges for 2.8 s, then the shot flies a long, high lob.
    let hit = hit_at.expect("the shot never landed on the building");
    assert!(
        hit > 4 * TICKS_PER_SECOND,
        "landed at tick {hit}: no charge and lob"
    );
}
