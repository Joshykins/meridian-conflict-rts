//! Self-destruct: at once, or after a five-second countdown that the same order calls
//! off, shown to its own side only.

use mc_core::{Angle, FxVec2};
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
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Spawns one finished `key` at (`x`, 512) and returns its id.
fn spawn(w: &mut World, key: &str, x: i32) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let before: Vec<UnitId> = w
        .state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .collect();
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint,
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })])
    .unwrap();
    let units = &w.state.units;
    units
        .slots
        .iter()
        .find(|&r| units.blueprint[r] == blueprint && !before.contains(&units.id(r)))
        .map(|r| units.id(r))
        .expect("spawned")
}

fn destruct(w: &mut World, player: u8, units: Vec<UnitId>, timed: bool) {
    w.tick(&[PlayerCommand {
        player,
        command: Command::SelfDestruct { units, timed },
    }])
    .unwrap();
}

fn alive(w: &World, id: UnitId) -> bool {
    w.state.units.row(id).is_some()
}

/// Ticks in five seconds.
const FIVE_SECONDS: u32 = 5 * mc_core::TICKS_PER_SECOND;

#[test]
fn at_once_blows_up_this_tick() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t1_tank", 500);
    destruct(&mut w, 0, vec![tank], false);
    assert!(!alive(&w, tank));
}

#[test]
fn timed_blows_up_after_five_seconds() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t1_tank", 500);
    destruct(&mut w, 0, vec![tank], true);
    for _ in 1..FIVE_SECONDS {
        w.tick(&[]).unwrap();
        assert!(alive(&w, tank), "gone before the five seconds were up");
    }
    w.tick(&[]).unwrap();
    assert!(!alive(&w, tank), "still there five seconds on");
}

#[test]
fn ordering_it_again_calls_every_countdown_off() {
    let mut w = world();
    let a = spawn(&mut w, "aster_t1_tank", 500);
    destruct(&mut w, 0, vec![a], true);
    let b = spawn(&mut w, "aster_t1_tank", 540);
    // One of the two is counting down: the order stops it rather than arming both.
    destruct(&mut w, 0, vec![a, b], true);
    for _ in 0..FIVE_SECONDS * 2 {
        w.tick(&[]).unwrap();
    }
    assert!(alive(&w, a) && alive(&w, b));
    let row = w.state.units.row(a).unwrap();
    assert_eq!(w.state.units.destruct[row], 0);
}

#[test]
fn an_enemy_cannot_arm_it() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t1_tank", 500);
    destruct(&mut w, 1, vec![tank], true);
    destruct(&mut w, 1, vec![tank], false);
    let row = w.state.units.row(tank).unwrap();
    assert_eq!(w.state.units.destruct[row], 0);
}

#[test]
fn the_countdown_is_shown_to_its_own_side_only() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t1_tank", 500);
    destruct(&mut w, 0, vec![tank], true);
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    let view = frame.destructs.first().expect("listed for its own side");
    assert_eq!(view.unit_id, tank.0);
    assert_eq!(view.ticks_left, view.length);
    w.write_render_frame(Some(1), &mut frame);
    assert!(frame.destructs.is_empty(), "shown to the enemy");
}
