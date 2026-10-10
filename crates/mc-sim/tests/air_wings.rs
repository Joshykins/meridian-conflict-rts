//! Air wings (`air_wings.rs`): a carrier builds a wing aboard as one job, launches its
//! aircraft off the deck, and they form up and fly guard round it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, Handle, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn sea() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
    let terrain = Heightfield::flat(512, 512, Fx::from_int(-40));
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.state.players[0].free_build = true;
    w
}

fn order(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

fn carrier(w: &mut World) -> usize {
    let bp = w.blueprints.id_of("aster_t3_carrier").unwrap();
    w.spawn_unit(bp, 0, FxVec2::from_ints(2000, 2000), Angle::ZERO, true)
        .unwrap()
}

#[test]
fn a_wing_is_built_launched_off_the_deck_and_forms_up_round_its_carrier() {
    let mut w = sea();
    let ship = carrier(&mut w);
    let id = w.state.units.id(ship);
    let fighter = w.blueprints.id_of("aster_t2_interceptor").unwrap();
    order(
        &mut w,
        Command::QueueWing {
            units: vec![id],
            wing: 0,
            count: 1,
        },
    );
    let mut aboard_together = false;
    let mut out = Vec::new();
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        let wing: Vec<usize> = u
            .slots
            .iter()
            .filter(|&r| u.blueprint[r] == fighter)
            .collect();
        aboard_together |= wing.len() == 8 && wing.iter().all(|&r| u.hangar[r] == id);
        if wing.len() == 8 && wing.iter().all(|&r| u.hangar[r] == Handle::NONE) {
            out = wing;
            break;
        }
    }
    assert!(aboard_together, "the whole wing was built aboard at once");
    assert_eq!(out.len(), 8, "all eight went off the deck");
    // Then they fly guard round the carrier, in one formation.
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    let u = &w.state.units;
    let orders: Vec<_> = out
        .iter()
        .map(|&r| *w.state.orders.front(u, r).expect("an order"))
        .collect();
    assert!(orders
        .iter()
        .all(|o| o.kind == OrderKind::Guard && o.target == id));
    assert!(orders[0].formation != 0 && orders.iter().all(|o| o.formation == orders[0].formation));
    assert!(w.state.wings.bays[&id].queue.is_empty());
}

#[test]
fn a_wing_queue_is_capped_and_cancelled_from_the_back() {
    let mut w = sea();
    let ship = carrier(&mut w);
    let id = w.state.units.id(ship);
    order(
        &mut w,
        Command::QueueWing {
            units: vec![id],
            wing: 1,
            count: 20,
        },
    );
    assert_eq!(
        w.state.wings.bays[&id].queue.len(),
        mc_sim::air_wings::MAX_WING_QUEUE
    );
    assert!(w.events.iter().any(|e| matches!(
        e,
        mc_sim::SimEvent::CommandRefused {
            reason: mc_sim::Refusal::WingQueueFull,
            ..
        }
    )));
    order(
        &mut w,
        Command::CancelWing {
            units: vec![id],
            wing: 1,
        },
    );
    assert_eq!(
        w.state.wings.bays[&id].queue.len(),
        mc_sim::air_wings::MAX_WING_QUEUE - 1
    );
}

#[test]
fn what_a_carrier_was_building_goes_down_with_it() {
    let mut w = sea();
    let ship = carrier(&mut w);
    let id = w.state.units.id(ship);
    order(
        &mut w,
        Command::QueueWing {
            units: vec![id],
            wing: 2,
            count: 2,
        },
    );
    w.state.units.health[ship] = Fx::ZERO;
    for _ in 0..5 {
        w.tick(&[]).unwrap();
    }
    assert!(!w.state.wings.bays.contains_key(&id));
}
