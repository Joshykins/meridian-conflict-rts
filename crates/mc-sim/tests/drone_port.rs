//! The commander's salvage drone port: two free drones that salvage far round it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::focus::{Focus, Priority};
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

/// A side whose every bit of income goes into a factory going up, with an Osprey whose
/// drones are paid for under materials priority `mines`: the Osprey's row.
fn starved_osprey(w: &mut World, mines: Priority) -> usize {
    let add = |w: &mut World, key: &str, x: i32, y: i32| {
        let id = w.blueprints.id_of(key).unwrap();
        w.spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
            .unwrap()
    };
    // The commander's trickle of mass is all the side makes.
    add(w, "aster_commander", 250, 250);
    add(w, "aster_mass_storage", 300, 300);
    add(w, "aster_t1_power", 360, 300);
    let engineer = add(w, "aster_t1_engineer", 400, 400);
    w.state.players[0].mass = Fx::ZERO;
    w.state.players[0].energy = Fx::ZERO;
    w.state.players[0].focus = Focus {
        mines,
        power: Priority::Even,
    };
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
    add(w, "aster_t2_reclaim_carrier", 900, 900)
}

fn built(w: &World, drones: &[usize]) -> usize {
    drones
        .iter()
        .filter(|&&r| w.state.units.is_active(r))
        .count()
}

/// The Osprey is what a side short of mass builds to get some. Its drones cost a little,
/// so with materials put first they come out of it while every other bit of income goes
/// to building and nothing is left in store, and they salvage.
#[test]
fn an_osprey_on_a_starved_economy_fields_its_drones_with_materials_first() {
    let mut w = world();
    let osprey = starved_osprey(&mut w, Priority::First);
    let mut most_in_store = Fx::ZERO;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        most_in_store = most_in_store.max(w.state.players[0].mass);
    }
    assert!(
        most_in_store < Fx::from_int(5),
        "the economy was starved: {most_in_store:?} in store"
    );
    assert_eq!(
        built(&w, &drones_of(&w, osprey)),
        4,
        "a full flock with nothing in store"
    );
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

/// With materials put last, a starved side's drones wait for what the rest leaves: the
/// first one stands half-built on its pylon.
#[test]
fn materials_last_leaves_a_starved_ospreys_drones_waiting() {
    let mut w = world();
    let osprey = starved_osprey(&mut w, Priority::Last);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    let drones = drones_of(&w, osprey);
    assert_eq!(built(&w, &drones), 0, "none finished");
    assert_eq!(drones.len(), 1, "one going up on its pylon");
    let time = w
        .blueprints
        .unit(w.state.units.blueprint[drones[0]])
        .build_time;
    assert!(w.state.units.build_progress[drones[0]] < time / 2);
}

/// An Osprey's drones hang from pylons under its wings. Letting go they drop clear;
/// coming home they glide in and rise onto their pylons with no jump; docked, they
/// keep their place on the wing while it flies.
#[test]
fn osprey_drones_glide_on_and_off_their_pylons_and_ride_the_wing() {
    let mut w = world();
    w.state.players[0].free_build = true;
    // Room in store for what they bring home.
    let storage = w.blueprints.id_of("aster_mass_storage").unwrap();
    w.spawn_unit(storage, 0, FxVec2::from_ints(300, 300), Angle::ZERO, true)
        .unwrap();
    let key = "aster_t2_reclaim_carrier";
    let id = w.blueprints.id_of(key).unwrap();
    let osprey = w
        .spawn_unit(id, 0, FxVec2::from_ints(900, 900), Angle::ZERO, true)
        .unwrap();
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    let drones = drones_of(&w, osprey);
    assert_eq!(drones.len(), 4);
    let offsets = |w: &World| -> Vec<(Fx, Fx)> {
        drones_of(w, osprey)
            .iter()
            .map(|&r| {
                (
                    w.state.units.pos[r].distance(w.state.units.pos[osprey]),
                    w.state.units.z[osprey] - w.state.units.z[r],
                )
            })
            .collect()
    };
    let docked = offsets(&w);
    for &(across, below) in &docked {
        assert!(across > Fx::from_int(3), "out on the wing: {across:?}");
        // Its feet hang level with the Osprey's belly, under the wing.
        assert!(below > -Fx::ONE, "under the wing: {below:?}");
    }
    // Flown somewhere, the flock stays on the wing, not a tick behind.
    let osprey_id = w.state.units.id(osprey);
    w.tick(&[mc_sim::PlayerCommand {
        player: 0,
        command: mc_sim::Command::Move {
            units: vec![osprey_id],
            target: FxVec2::from_ints(1300, 900),
            queue: false,
        },
    }])
    .unwrap();
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        for (now, then) in offsets(&w).iter().zip(&docked) {
            assert!(
                (now.0 - then.0).abs() < Fx::ratio(1, 10)
                    && (now.1 - then.1).abs() < Fx::ratio(1, 10),
                "a docked drone slipped off its pylon: {now:?} (was {then:?})"
            );
        }
    }
    // A wreck turns up: they go out and come home, and no step on or off the pylon is a jump.
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let at = w.state.units.pos[osprey] + FxVec2::from_ints(120, 0);
    w.state
        .wrecks
        .spawn(tank, at, Fx::from_int(20), Angle::ZERO, Fx::from_int(40), 0)
        .unwrap();
    let mut went_out = false;
    let (mut reclaimed, mut busy) = (false, true);
    // Where each drone is from the Osprey, which may still be flying.
    let rel = |w: &World, r: usize| {
        (w.state.units.pos[r] - w.state.units.pos[osprey])
            .extend(w.state.units.z[r] - w.state.units.z[osprey])
    };
    let mut last: Vec<_> = drones.iter().map(|&r| rel(&w, r)).collect();
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        // A drone reclaiming keeps the Osprey off the idle Reclaimers card.
        use mc_sim::tables::flag::{RECLAIMING, WORKING};
        if drones
            .iter()
            .any(|&r| w.state.units.has_flag(r, RECLAIMING))
        {
            reclaimed = true;
            busy &= w.state.units.has_flag(osprey, WORKING);
        }
        for (k, &r) in drones.iter().enumerate() {
            let now = rel(&w, r);
            went_out |= now.xy().length() > Fx::from_int(30);
            if w.state.units.deploy[r] != 1 {
                let step = (now - last[k]).length();
                assert!(
                    step < Fx::from_int(5),
                    "a drone jumped {step:?} m on or off its pylon"
                );
            }
            last[k] = now;
        }
    }
    assert!(went_out, "the drones went out to the wreck");
    assert!(
        reclaimed && busy,
        "the Osprey is working while its drones reclaim"
    );
    assert!(
        !w.state
            .units
            .has_flag(osprey, mc_sim::tables::flag::WORKING),
        "the wreck gone, it is idle again"
    );
    for (now, then) in offsets(&w).iter().zip(&docked) {
        assert!(
            (now.0 - then.0).abs() < Fx::ratio(1, 10) && (now.1 - then.1).abs() < Fx::ratio(1, 10),
            "home on its pylon: {now:?} (was {then:?})"
        );
    }
}
