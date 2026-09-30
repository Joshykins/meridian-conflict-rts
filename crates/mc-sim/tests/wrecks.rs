//! Wrecks: how they lie once they come down, and blasts wearing them away.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
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
        name: "wrecks".into(),
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
        seed: 4,
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

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 0,
            blueprint,
            pos: at,
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    }])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

/// A tank wreck at (x, y): its row in the wreck table.
fn wreck(w: &mut World, x: i32, y: i32) -> usize {
    let at = FxVec2::from_ints(x, y);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugWrecks {
            blueprint: w.blueprints.id_of("aster_t2_tank").unwrap(),
            pos: at,
            count: 1,
        },
    }])
    .unwrap();
    let wr = &w.state.wrecks;
    wr.slots
        .iter()
        .min_by_key(|&r| wr.pos[r].distance_sq(at))
        .unwrap()
}

fn destroy(w: &mut World, id: UnitId) {
    let row = w.state.units.row(id).unwrap();
    w.state.units.health[row] = Fx::ZERO;
    w.tick(&[]).unwrap();
}

#[test]
fn a_blast_wears_down_the_wrecks_it_reaches_and_blows_the_nearest_apart() {
    let mut w = world();
    let reactor = spawn(&mut w, "aster_t3_power", 800, 800);
    let near = wreck(&mut w, 800, 850);
    let edge = wreck(&mut w, 800, 800 + 120);
    let far = wreck(&mut w, 800, 1100);
    let handle = |w: &World, r: usize| w.state.wrecks.slots.handle(r);
    let (near_id, edge_id, far_id) = (handle(&w, near), handle(&w, edge), handle(&w, far));
    let full = w.state.wrecks.mass[edge];
    // Past the moment a fresh wreck is spared.
    for _ in 0..TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
    }
    destroy(&mut w, reactor);
    w.tick(&[]).unwrap();
    let wr = &w.state.wrecks;
    assert!(
        wr.slots.resolve(near_id).is_none(),
        "the wreck beside it is gone"
    );
    let edge = wr
        .slots
        .resolve(edge_id)
        .expect("the wreck at the edge is still there");
    assert!(wr.mass[edge] < full, "the wreck at the edge lost mass");
    let far = wr
        .slots
        .resolve(far_id)
        .expect("the far wreck is untouched");
    assert_eq!(wr.mass[far], full);
}

/// The wreck a unit leaves takes no blast damage for about a second, so the splash of
/// the shell that killed it, or of the next one in the salvo, does not blow it away.
#[test]
fn a_fresh_wreck_is_spared_blasts_for_a_second() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t2_tank", 800, 850);
    destroy(&mut w, tank);
    let wr = &w.state.wrecks;
    let row = wr.slots.iter().next().expect("the tank left a wreck");
    let id = wr.slots.handle(row);
    let full = wr.mass[row];
    // A reactor going up beside it at once: close enough to blow an old wreck apart.
    let reactor = spawn(&mut w, "aster_t3_power", 800, 800);
    destroy(&mut w, reactor);
    let wr = &w.state.wrecks;
    let row = wr
        .slots
        .resolve(id)
        .expect("the fresh wreck was blown away");
    assert_eq!(wr.mass[row], full, "the fresh wreck lost mass");

    // A second on, the next blast wears it as any other.
    for _ in 0..TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
    }
    let reactor = spawn(&mut w, "aster_t3_power", 800, 800);
    destroy(&mut w, reactor);
    let wr = &w.state.wrecks;
    assert!(
        wr.slots.resolve(id).is_none_or(|r| wr.mass[r] < full),
        "a blast a second later left it whole"
    );
}

#[test]
fn a_crashed_aircraft_lies_nose_down_in_the_ground() {
    let mut w = world();
    let plane = spawn(&mut w, "aster_t1_bomber", 700, 700);
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    destroy(&mut w, plane);
    for _ in 0..400 {
        if w.state.aircraft_crashes.is_empty() {
            break;
        }
        w.tick(&[]).unwrap();
    }
    let wr = &w.state.wrecks;
    let row = wr.slots.iter().next().expect("it left a wreck");
    assert_eq!(wr.landing[row], 1, "it came down out of the sky");
    assert!(wr.pitch[row] < 0, "nose down: {}", wr.pitch[row]);
    assert!(
        wr.pitch[row] >= -(Angle::from_degrees(30).0 as i16),
        "not stood on its nose: {}",
        wr.pitch[row]
    );
}
