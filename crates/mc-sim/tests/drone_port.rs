//! The commander's salvage drone port: two free drones that salvage far round it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// A commander with the drone port on, at (x, y).
fn ported_commander(w: &mut World, x: i32, y: i32) -> usize {
    let acu = w.blueprints.id_of("aster_commander").unwrap();
    let set = w.blueprints.refit_set(acu).unwrap();
    let kit = set
        .slots
        .iter()
        .flat_map(|s| &s.modules)
        .find(|m| m.key == "drone_port")
        .unwrap()
        .kit;
    let fitted = w.blueprints.refit_result(acu, kit).unwrap();
    w.spawn_unit(fitted, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn drones_of(w: &World, row: usize) -> Vec<usize> {
    let parent = w.state.units.id(row);
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.drone_parent[r] == parent)
        .collect()
}

#[test]
fn the_port_makes_two_free_drones_that_salvage_far_off_and_come_home() {
    let mut w = world();
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::ZERO;
    w.state.players[0].mass_capacity = Fx::from_int(20000);
    let acu = ported_commander(&mut w, 900, 900);
    let reach = w.blueprints.unit(w.state.units.blueprint[acu]).drone_radius;
    assert!(reach >= Fx::from_int(600), "a very large radius: {reach:?}");
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    // Far past what the commander's own arm reaches, well inside the drones'.
    let wreck = w
        .state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(1450, 900),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(300),
            0,
        )
        .unwrap();
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
    }
    let drones = drones_of(&w, acu);
    assert_eq!(
        drones.len(),
        2,
        "one drone for each pad, built with nothing in the bank"
    );
    assert!(
        !w.state.wrecks.slots.is_alive(wreck) || w.state.wrecks.mass[wreck] < Fx::from_int(300),
        "the drones salvage the far wreck"
    );
    assert!(w.state.players[0].reclaimed_mass > Fx::ZERO);
    // The commander itself never walked there: its drones did the work.
    assert!(w.state.units.pos[acu].distance(FxVec2::from_ints(900, 900)) < Fx::from_int(5));
    for _ in 0..1800 {
        w.tick(&[]).unwrap();
    }
    assert!(
        !w.state.wrecks.slots.is_alive(wreck),
        "the wreck is salvaged away"
    );
    // Nothing left to do: they are back on the pads on its back.
    for r in drones_of(&w, acu) {
        assert_eq!(w.state.units.deploy[r], 0, "a drone is home");
        assert!(w.state.units.pos[r].distance(w.state.units.pos[acu]) < Fx::from_int(8));
        assert!(w.state.units.z[r] > w.state.units.z[acu] + Fx::from_int(15));
    }
}

#[test]
fn a_lost_drone_is_rebuilt_and_the_drones_go_with_the_commander() {
    let mut w = world();
    let acu = ported_commander(&mut w, 900, 900);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    let drones = drones_of(&w, acu);
    assert_eq!(drones.len(), 2);
    w.state.units.health[drones[0]] = Fx::ZERO;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(drones_of(&w, acu).len(), 2, "the lost drone is replaced");
    w.state.units.health[acu] = Fx::ZERO;
    w.tick(&[]).unwrap();
    w.tick(&[]).unwrap();
    assert!(drones_of(&w, acu)
        .iter()
        .all(|&r| !w.state.units.slots.is_alive(r)));
}

/// The Osprey is what a side short of mass builds to get some: its drones must come out
/// of it while every bit of income goes to building and nothing is left in store. (They
/// used to be paid for out of what was left after the economy's own spending, so they
/// waited for a surplus a new side never has.)
#[test]
fn an_osprey_on_a_starved_economy_still_fields_its_drones_and_salvages() {
    let mut w = world();
    let add = |w: &mut World, key: &str, x: i32, y: i32| {
        let id = w.blueprints.id_of(key).unwrap();
        w.spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
            .unwrap()
    };
    add(&mut w, "aster_mass_storage", 300, 300);
    add(&mut w, "aster_t1_power", 360, 300);
    let engineer = add(&mut w, "aster_t1_engineer", 400, 400);
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::ZERO;
    // Everything that comes in goes straight into a factory going up.
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let id = w.state.units.id(engineer);
    w.tick(&[mc_sim::PlayerCommand {
        player: 0,
        command: mc_sim::Command::Build {
            units: vec![id],
            blueprint: factory,
            pos: FxVec2::from_ints(460, 460),
            heading: Angle::ZERO,
            queue: false,
        },
    }])
    .unwrap();
    let osprey = add(&mut w, "aster_t1_reclaim_carrier", 900, 900);
    let mut most_in_store = Fx::ZERO;
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        most_in_store = most_in_store.max(w.state.players[0].mass);
    }
    assert!(
        most_in_store < Fx::from_int(5),
        "the economy was starved: {most_in_store:?} in store"
    );
    assert_eq!(
        drones_of(&w, osprey).len(),
        4,
        "a full flock with nothing in store"
    );
    // Then a wreck turns up in reach.
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let wreck = w
        .state
        .wrecks
        .spawn(
            tank,
            FxVec2::from_ints(1250, 900),
            Fx::from_int(20),
            Angle::ZERO,
            Fx::from_int(200),
            0,
        )
        .unwrap();
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    assert!(
        !w.state.wrecks.slots.is_alive(wreck) || w.state.wrecks.mass[wreck] < Fx::from_int(200),
        "the flock salvaged the wreck"
    );
}
