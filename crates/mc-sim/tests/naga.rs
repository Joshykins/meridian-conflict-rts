//! The Naga's own rules: the Sovereign's tail reaches only across its nose (`aim_arc`), so
//! it turns its whole body onto a mark behind it instead of swinging the tail round.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "naga".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 3,
        players: vec![player("Naga", 0), player("Aster", 1)],
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
    w.state.units.id(row)
}

#[test]
fn the_sovereign_turns_its_body_onto_a_mark_behind_it() {
    let mut w = world();
    // Facing east, with an ARC tank 150 m to its west: dead astern.
    let sovereign = add(&mut w, "naga_commander", 0, 512, 512, 0);
    let tank = add(&mut w, "aster_t1_tank", 1, 362, 512, 0);
    let hp = |w: &World| {
        w.state
            .units
            .row(tank)
            .map_or(0.0, |r| w.state.units.health[r].to_f32())
    };
    let reach = Angle::from_degrees(60).0 as i32 + Angle::from_degrees(1).0 as i32;
    for _ in 0..(8 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(sovereign).unwrap();
        let yaw = Angle::ZERO.delta_to(w.state.units.weapon_yaw[row][0]) as i32;
        assert!(
            yaw.abs() <= reach,
            "the tail swung {} degrees off the nose",
            yaw * 360 / 65536
        );
    }
    // It squared up to the tank (due west) and the stinger killed it.
    let row = w.state.units.row(sovereign).unwrap();
    let off = w.state.units.heading[row]
        .delta_to(Angle::from_degrees(180))
        .unsigned_abs();
    assert!(
        off <= Angle::from_degrees(3).0,
        "still {} degrees off the tank",
        off as u32 * 360 / 65536
    );
    assert_eq!(hp(&w), 0.0, "the stinger killed it");
}
