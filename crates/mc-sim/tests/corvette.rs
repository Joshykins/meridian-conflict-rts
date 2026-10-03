//! The Valiant rail corvette (`aster_t3_corvette`): its long rail cannon hangs under the
//! belly and lays onto the ground, even close under the ship; its rotary cannon on the
//! back cuts down aircraft round it.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const CORVETTE: &str = "aster_t3_corvette";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

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
    let unit = w.state.units.id(row);
    w.state.units.fire_state[row] = FireState::HoldFire;
    unit
}

fn health(w: &World, id: UnitId) -> f32 {
    w.state
        .units
        .row(id)
        .map_or(0.0, |r| w.state.units.health[r].to_f32())
}

fn seconds(s: u32) -> usize {
    (s * TICKS_PER_SECOND) as usize
}

/// A corvette of player 0, climbed to its cruise height and free to fire.
fn corvette(w: &mut World) -> UnitId {
    let ship = add(w, CORVETTE, 0, 3000, 3000, 0);
    let row = w.state.units.row(ship).unwrap();
    w.state.units.fire_state[row] = Default::default();
    for _ in 0..seconds(20) {
        w.tick(&[]).unwrap();
    }
    ship
}

#[test]
fn the_rail_cannon_lays_onto_ground_far_off_and_close_under_the_ship() {
    for (x, y) in [(3000 + 1350, 3000), (3000 - 150, 3000 + 150)] {
        let mut w = world();
        let ship = corvette(&mut w);
        let heading = w.state.units.heading[w.state.units.row(ship).unwrap()];
        let tank = add(&mut w, "aster_t4_assault_tank", 1, x, y, 0);
        let full = health(&w, tank);
        for _ in 0..seconds(15) {
            w.tick(&[]).unwrap();
        }
        assert!(
            health(&w, tank) <= full - 600.0,
            "the rail never hit the tank at ({x}, {y}): {} of {full}",
            health(&w, tank)
        );
        // A turret, not the hull: the ship never turned to aim.
        let row = w.state.units.row(ship).unwrap();
        assert_eq!(w.state.units.heading[row], heading);
    }
}

#[test]
fn the_rotary_cannon_shoots_down_aircraft_round_the_ship() {
    let mut w = world();
    let _ship = corvette(&mut w);
    let gunship = add(&mut w, "aster_t2_gunship", 1, 3300, 3100, 0);
    let full = health(&w, gunship);
    for _ in 0..seconds(10) {
        w.tick(&[]).unwrap();
    }
    assert!(
        health(&w, gunship) < full * 0.5,
        "the rotary cannon left the gunship alone: {} of {full}",
        health(&w, gunship)
    );
}
