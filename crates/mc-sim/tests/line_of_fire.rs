//! Line of fire: a gun that shoots flat does not fire into a hill between it and its
//! target, turns to a target it can see, and an attack order walks the unit on until
//! it has a shot.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, FireState};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const CELLS: u32 = 256;
const CELL: f64 = 8.0;
/// A round hill half-way between the shooter (x 500) and a target at x 660.
const HILL: (f64, f64) = (580.0, 512.0);

/// Flat ground at 20 m with a 26 m hill on it, wide enough to drive over.
fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let step = 1.0 / 64.0;
    let n = CELLS as usize + 1;
    let mut samples = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f64 * CELL - HILL.0, y as f64 * CELL - HILL.1);
            let r2 = (dx * dx + dy * dy) / (55.0 * 55.0);
            let z = 20.0 + 26.0 * (-r2).exp();
            samples.push((z / step).round() as u16);
        }
    }
    let terrain =
        Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ratio(1, 64), Fx::ZERO);
    let map = MapData {
        name: "hill".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

fn rows_of(w: &World, owner: u8) -> Vec<usize> {
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == owner)
        .collect()
}

fn fired(w: &World) -> bool {
    w.events
        .iter()
        .any(|e| matches!(e, SimEvent::ShotFired { owner: 0, .. }))
}

const DUMMY: u16 = flag::PASSIVE | flag::INVULNERABLE;

#[test]
fn a_gun_holds_fire_while_a_hill_hides_its_target() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 512, 0),
        spawn(&w, 1, "aster_t1_tank", 660, 512, DUMMY),
    ])
    .unwrap();
    let shooter = rows_of(&w, 0)[0];
    let target = rows_of(&w, 1)[0];
    w.tick(&[cmd(Command::SetFireState {
        units: vec![w.state.units.id(shooter)],
        state: FireState::HoldPosition,
    })])
    .unwrap();
    let start = w.state.units.pos[shooter];
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        assert!(!fired(&w), "it fired into the hill");
    }
    assert_eq!(
        w.state.units.weapon_target[shooter][0],
        w.state.units.id(target),
        "it should stay laid on the hidden target"
    );
    assert_ne!(w.state.units.shot_blocked[shooter] & 1, 0);
    assert!(
        start.distance(w.state.units.pos[shooter]) < Fx::ONE,
        "holding position, it should not move"
    );
}

#[test]
fn a_gun_turns_to_a_target_it_can_see() {
    let mut w = world();
    // The hidden one is nearer (160 m); the other is in the open, 175 m off to the north.
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 512, 0),
        spawn(&w, 1, "aster_t1_tank", 660, 512, DUMMY),
    ])
    .unwrap();
    let shooter = rows_of(&w, 0)[0];
    w.tick(&[
        spawn(&w, 1, "aster_t1_tank", 500, 687, DUMMY),
        cmd(Command::SetFireState {
            units: vec![w.state.units.id(shooter)],
            state: FireState::HoldPosition,
        }),
    ])
    .unwrap();
    let open = rows_of(&w, 1)
        .into_iter()
        .find(|&r| w.state.units.pos[r].y > Fx::from_int(600))
        .unwrap();
    let mut shot = false;
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        shot |= fired(&w);
    }
    assert_eq!(
        w.state.units.weapon_target[shooter][0],
        w.state.units.id(open),
        "it should have turned to the target in the open"
    );
    assert!(shot, "it should be firing at the one it can see");
}

#[test]
fn an_attack_order_walks_on_until_it_has_a_shot() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_tank", 500, 512, 0),
        spawn(&w, 1, "aster_t1_tank", 660, 512, DUMMY),
    ])
    .unwrap();
    let shooter = rows_of(&w, 0)[0];
    let target = rows_of(&w, 1)[0];
    let start = w.state.units.pos[shooter];
    w.tick(&[cmd(Command::Attack {
        units: vec![w.state.units.id(shooter)],
        target: w.state.units.id(target),
        queue: false,
    })])
    .unwrap();
    let mut first_shot = None;
    for i in 0..400 {
        w.tick(&[]).unwrap();
        if fired(&w) {
            first_shot = Some(i);
            break;
        }
    }
    let moved = start.distance(w.state.units.pos[shooter]);
    assert!(
        first_shot.is_some(),
        "it never got a shot (moved {moved:?})"
    );
    assert!(
        moved > Fx::from_int(5),
        "it fired without moving: into the hill"
    );
    assert!(
        w.state.units.pos[shooter].distance(w.state.units.pos[target]) > Fx::from_int(40),
        "it should stop as soon as it can see, not drive up to the target"
    );
}

#[test]
fn artillery_lobs_over_the_hill() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, "aster_t1_artillery", 500, 512, 0),
        spawn(&w, 1, "aster_t1_tank", 660, 512, DUMMY),
    ])
    .unwrap();
    let shooter = rows_of(&w, 0)[0];
    w.tick(&[cmd(Command::SetFireState {
        units: vec![w.state.units.id(shooter)],
        state: FireState::HoldPosition,
    })])
    .unwrap();
    let mut shot = false;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        shot |= fired(&w);
        assert_eq!(
            w.state.units.shot_blocked[shooter], 0,
            "a lobbed shell needs no line"
        );
    }
    assert!(shot, "artillery should shell a target behind a hill");
}
