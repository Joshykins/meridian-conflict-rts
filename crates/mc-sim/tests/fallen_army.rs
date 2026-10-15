//! What becomes of a defeated side's army (`succession.rs`): destroyed with its
//! commander, or handed to a living ally.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{
    Command, FallenArmy, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World,
};
use std::path::Path;
use std::sync::Arc;

fn terrain() -> Heightfield {
    Heightfield::flat(256, 256, Fx::from_int(20))
}

/// Two allies (seats 0 and 1, a person and an AI) against seat 2, each with a
/// commander at their start, far enough apart that a commander's reactor going
/// up reaches nobody else's units.
fn world(rule: FallenArmy, threads: usize) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        doodads: Default::default(),
        name: "fallen".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![
            FxVec2::from_ints(300, 300),
            FxVec2::from_ints(1700, 300),
            FxVec2::from_ints(1000, 1800),
        ],
        props: Vec::new(),
    };
    let player = |name: &str, team, controller, start| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller,
        start,
    };
    let config = MatchConfig {
        seed: 9,
        players: vec![
            player("you", 0, Controller::Human, 0),
            player("ally", 0, Controller::Human, 1),
            player("hostile", 1, Controller::Human, 2),
        ],
        cheats: true,
        fog: false,
        spawn_commanders: true,
    };
    let mut w = World::with_terrain(
        terrain(),
        map,
        blueprints,
        Arc::new(Pool::new(threads)),
        &config,
    )
    .unwrap();
    w.set_fallen_army(rule);
    w
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    }
}

/// The ally's army away from every commander: a tank, an engineer and a generator.
/// Returns their ids.
fn ally_army(w: &mut World) -> Vec<UnitId> {
    let before: Vec<UnitId> = w
        .state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .collect();
    let setup = [
        spawn(w, 1, "aster_t1_tank", 1000, 1000),
        spawn(w, 1, "aster_t1_engineer", 1040, 1000),
        spawn(w, 1, "aster_t1_power", 1000, 1060),
    ];
    w.tick(&setup).unwrap();
    let units = &w.state.units;
    units
        .slots
        .iter()
        .map(|r| units.id(r))
        .filter(|id| !before.contains(id))
        .collect()
}

fn kill_commander(w: &mut World, player: u8) -> Vec<SimEvent> {
    let acu = w.state.players[player as usize].commander;
    let kill = PlayerCommand {
        player: 0,
        command: Command::DebugDamage {
            units: vec![acu],
            permille: 1000,
        },
    };
    let mut events = Vec::new();
    w.tick(&[kill]).unwrap();
    events.append(&mut w.events);
    w.tick(&[]).unwrap();
    events.append(&mut w.events);
    events
}

fn defeat_of(events: &[SimEvent], player: u8) -> Option<Option<u8>> {
    events.iter().find_map(|e| match e {
        SimEvent::PlayerDefeated { player: p, heir } if *p == player => Some(*heir),
        _ => None,
    })
}

fn owners(w: &World, ids: &[UnitId]) -> Vec<Option<u8>> {
    let units = &w.state.units;
    ids.iter()
        .map(|&id| units.row(id).map(|r| units.owner[r]))
        .collect()
}

#[test]
fn by_default_a_fallen_commanders_army_is_destroyed() {
    let mut w = world(FallenArmy::Destroyed, 1);
    let army = ally_army(&mut w);
    let events = kill_commander(&mut w, 1);
    assert_eq!(defeat_of(&events, 1), Some(None));
    assert_eq!(owners(&w, &army), vec![None; army.len()]);
}

#[test]
fn an_ally_takes_over_a_fallen_commanders_army_and_store() {
    let mut w = world(FallenArmy::ToAlly, 1);
    let army = ally_army(&mut w);
    w.state.players[0].materials = Fx::from_int(100);
    w.state.players[1].materials = Fx::from_int(300);
    let events = kill_commander(&mut w, 1);
    assert_eq!(defeat_of(&events, 1), Some(Some(0)), "seat 0 is the heir");
    assert_eq!(owners(&w, &army), vec![Some(0); army.len()]);
    assert!(w.state.players[1].defeated);
    assert!(!w.state.players[0].defeated);
    assert_eq!(w.state.winner, None, "the team fights on");
    assert_eq!(w.state.players[1].materials, Fx::ZERO);
    assert!(
        w.state.players[0].materials > Fx::from_int(100),
        "the store came with the army"
    );
    // The heir orders them.
    let tank = army[0];
    let go = PlayerCommand {
        player: 0,
        command: Command::Stop { units: vec![tank] },
    };
    w.tick(&[go]).unwrap();
    assert!(w.state.units.row(tank).is_some());
}

#[test]
fn with_no_ally_standing_the_army_is_destroyed() {
    let mut w = world(FallenArmy::ToAlly, 1);
    let army = ally_army(&mut w);
    kill_commander(&mut w, 0);
    let events = kill_commander(&mut w, 1);
    assert_eq!(defeat_of(&events, 1), Some(None));
    assert_eq!(owners(&w, &army), vec![None; army.len()]);
    assert_eq!(w.state.winner, Some(1));
}

#[test]
fn a_resigned_sides_commander_fights_on_for_the_heir_without_deciding_its_defeat() {
    let mut w = world(FallenArmy::ToAlly, 1);
    let acu = w.state.players[1].commander;
    let resign = PlayerCommand {
        player: 1,
        command: Command::Resign,
    };
    w.tick(&[resign]).unwrap();
    w.tick(&[]).unwrap();
    assert!(w.state.players[1].defeated);
    assert_eq!(owners(&w, &[acu]), vec![Some(0)]);
    // Losing it costs the heir a unit, not the match.
    let events = kill_commander(&mut w, 1);
    assert_eq!(owners(&w, &[acu]), vec![None]);
    assert_eq!(defeat_of(&events, 0), None);
    assert!(!w.state.players[0].defeated);
}

#[test]
fn a_handover_is_the_same_at_any_worker_count_and_after_a_restore() {
    let run = |threads| {
        let mut w = world(FallenArmy::ToAlly, threads);
        ally_army(&mut w);
        kill_commander(&mut w, 1);
        for _ in 0..20 {
            w.tick(&[]).unwrap();
        }
        w
    };
    let mut one = run(0);
    for threads in [1, 3, 8] {
        assert_eq!(run(threads).hash(), one.hash(), "{threads} workers");
    }
    let blob = one.snapshot();
    let mut restored = world(FallenArmy::Destroyed, 1);
    restored.restore(terrain(), &blob).unwrap();
    assert_eq!(restored.state.fallen_army, FallenArmy::ToAlly);
    for _ in 0..20 {
        assert_eq!(one.tick(&[]).unwrap(), restored.tick(&[]).unwrap());
    }
}
