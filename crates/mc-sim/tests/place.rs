//! Structure lots snap to 12 m and pathing rounds out to 8 m. Neighbouring
//! 2x2s share those overhang cells for pathing, but they still sit edge to edge.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "place".into(),
        content_id: 1,
        deposits: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn_at(w: &World, key: &str, x: i32, y: i32) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })
}

fn count(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .count()
}

fn id_at(w: &World, key: &str, x: i32, y: i32) -> mc_sim::UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bp && w.state.units.pos[r] == at)
        .expect("structure at site");
    w.state.units.id(row)
}

#[test]
fn two_power_generators_sit_edge_to_edge() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 552, 528),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 2);
}

#[test]
fn a_second_power_generator_cannot_overlap_the_first() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 540, 528),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
}

#[test]
fn a_wall_can_touch_a_power_generator() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_wall", 546, 534),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
    assert_eq!(count(&w, "aster_wall"), 1);
}

#[test]
fn removing_one_neighbour_frees_its_lot() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 552, 528),
    ])
    .unwrap();
    let gone = id_at(&w, "aster_t1_power", 528, 528);
    w.tick(&[cmd(Command::DebugRemove { units: vec![gone] })])
        .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
    w.tick(&[spawn_at(&w, "aster_t1_power", 528, 528)]).unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 2);
    w.tick(&[spawn_at(&w, "aster_t1_power", 552, 528)]).unwrap();
    assert_eq!(
        count(&w, "aster_t1_power"),
        2,
        "the remaining generator still occupies its lot"
    );
}
