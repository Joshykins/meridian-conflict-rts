//! The AI and the space elevator: a rich side raises one, and its crates come down
//! full of the side's combat units.
use super::*;

use crate::command::PlayerCommand;
use crate::world::MapData;
use crate::{MatchConfig, PlayerSetup};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

const ELEVATOR: &str = "aster_t5_space_elevator";

/// A flat 8 km map: an AI in slot 0 against an idle player.
fn world() -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 11,
        cheats: true,
        fog: false,
        spawn_commanders: true,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("P{i}"),
                faction: "Aster".into(),
                team: i,
                controller: if i == 0 {
                    Controller::Ai
                } else {
                    Controller::Human
                },
                start: i,
                ai: AiConfig::default(),
            })
            .collect(),
    };
    World::with_terrain(
        Heightfield::flat(1024, 1024, Fx::from_int(20)),
        MapData {
            name: "elevator AI test".into(),
            content_id: 11,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(1200, 1200), FxVec2::from_ints(6800, 6800)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap()
}

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    w.spawn_unit(
        w.blueprints.id_of(key).unwrap(),
        0,
        FxVec2::from_ints(x, y),
        Angle::ZERO,
        true,
    )
    .unwrap()
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[] as &[PlayerCommand]).unwrap();
    }
}

#[test]
fn an_ai_elevator_comes_down_with_a_full_payload_of_combat_units() {
    let mut w = world();
    let row = spawn(&mut w, ELEVATOR, 2000, 1200);
    let id = w.state.units.id(row);
    let e = w.bp(row).elevator.unwrap();
    run(&mut w, 100);
    let row = w.state.units.row(id).unwrap();
    let queued: Vec<BlueprintId> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .filter(|o| o.kind == OrderKind::Produce)
        .map(|o| o.blueprint)
        .collect();
    let mass: Fx = queued.iter().map(|b| w.blueprints.unit(*b).cost_mass).sum();
    assert!(
        mass <= e.payload && mass * 10 >= e.payload * 9,
        "queued {mass} of {} in {} units",
        e.payload,
        queued.len()
    );
    for b in &queued {
        let bp = w.blueprints.unit(*b);
        assert!(
            !bp.weapons.is_empty() && !bp.has(cat::ENGINEER),
            "{} is no combat unit",
            bp.key
        );
    }
    // Delivered at the first touchdown, and the queue filled again for the next fall.
    let before = w.state.players[0].units_built;
    run(&mut w, e.tether_ticks + e.descent_ticks + 50);
    assert!(w.state.players[0].units_built > before + 5);
    let row = w.state.units.row(id).unwrap();
    assert!(w.state.orders.iter(&w.state.units, row).next().is_some());
}

#[test]
fn a_side_whose_income_carries_one_raises_a_space_elevator_and_finishes_it() {
    let mut w = world();
    // A rich side: its tech 3 engineer can raise one, and its income pays for one in
    // well under `ELEVATOR_MINUTES`.
    w.state.players[0].income_permille = [65_000, 65_000];
    spawn(&mut w, "aster_t3_engineer", 1300, 1200);
    for i in 0..3 {
        spawn(&mut w, "aster_t2_fabricator", 1000 + i * 40, 1000);
        spawn(&mut w, "aster_t3_power", 1000 + i * 60, 900);
    }
    let elevator = w.blueprints.id_of(ELEVATOR).unwrap();
    let first = |w: &World| {
        let units = &w.state.units;
        units
            .slots
            .iter()
            .find(|&r| units.owner[r] == 0 && units.blueprint[r] == elevator)
            .map(|r| units.id(r))
    };
    let mut wanted = false;
    let mut raised = None;
    for _ in 0..600 {
        run(&mut w, 10);
        wanted |= w.state.ai[0].commander.wants.contains(&elevator);
        raised = first(&w);
        if raised.is_some() {
            break;
        }
    }
    let Some(id) = raised else {
        panic!("never raised one (wanted {wanted})");
    };
    // The side's builders gather on it: ten minutes at the most, not the hour its
    // first builder alone would take.
    let started = w.state.tick;
    while w.state.tick < started + 6000 {
        run(&mut w, 100);
        let row = w.state.units.row(id).expect("standing");
        if w.state.units.is_active(row) {
            return;
        }
    }
    let row = w.state.units.row(id).unwrap();
    panic!(
        "not finished in ten minutes: {} of {} health",
        w.state.units.health[row],
        w.bp(row).health
    );
}

#[test]
fn a_side_on_an_ordinary_income_wants_no_space_elevator() {
    let mut w = world();
    spawn(&mut w, "aster_t3_engineer", 1300, 1200);
    run(&mut w, 600);
    let elevator = w.blueprints.id_of(ELEVATOR).unwrap();
    assert!(!w.state.ai[0].commander.wants.contains(&elevator));
}
