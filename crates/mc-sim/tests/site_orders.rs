//! Orders given to a unit still being raised on its own lot (a warship, a titan) are
//! kept in its queue and carried out once it is finished (`owned_or_raising`).

use mc_core::{Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind, UnitId, WarpPhase};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const CORVETTE: &str = "aster_t3_corvette";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7500, 7500)],
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
    for p in &mut w.state.players {
        p.bonus_storage[1] = Fx::from_int(2_000_000);
        p.energy = Fx::from_int(1_000_000);
    }
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// A corvette's site, half built on its lot.
fn corvette_site(w: &mut World) -> UnitId {
    let bp = w.blueprints.id_of(CORVETTE).unwrap();
    let heading = w.blueprints.unit(bp).build_heading();
    let row = w
        .spawn_unit(bp, 0, FxVec2::from_ints(1500, 1500), heading, false)
        .unwrap();
    assert!(w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION));
    w.state.units.id(row)
}

fn front(w: &World, id: UnitId) -> Option<OrderKind> {
    let row = w.state.units.row(id).expect("alive");
    w.state.orders.front(&w.state.units, row).map(|o| o.kind)
}

fn finish(w: &mut World, id: UnitId) {
    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![id],
        permille: 1000,
    })])
    .unwrap();
    let row = w.state.units.row(id).expect("alive");
    assert!(!w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION));
}

#[test]
fn a_warp_given_to_a_warship_on_its_lot_is_made_once_it_is_finished() {
    let mut w = world();
    let ship = corvette_site(&mut w);
    w.tick(&[cmd(Command::Warp {
        units: vec![ship],
        pos: FxVec2::from_ints(6000, 1500),
        queue: false,
    })])
    .unwrap();
    assert_eq!(
        front(&w, ship),
        Some(OrderKind::Warp),
        "the warp was dropped"
    );
    for _ in 0..TICKS_PER_SECOND * 5 {
        w.tick(&[]).unwrap();
    }
    let row = w.state.units.row(ship).unwrap();
    assert_eq!(
        w.state.units.warp[row].phase,
        WarpPhase::Idle,
        "a half-built ship charged its drive"
    );
    finish(&mut w, ship);
    let jumped = (0..TICKS_PER_SECOND * 60).any(|_| {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(ship).unwrap();
        w.state.units.warp[row].phase == WarpPhase::Transit
    });
    assert!(jumped, "the finished ship never jumped");
}

#[test]
fn moves_queued_on_a_warship_on_its_lot_are_walked_once_it_is_finished() {
    let mut w = world();
    let ship = corvette_site(&mut w);
    let start = w.state.units.pos[w.state.units.row(ship).unwrap()];
    let goal = FxVec2::from_ints(2500, 1500);
    w.tick(&[
        cmd(Command::Move {
            units: vec![ship],
            target: goal,
            queue: false,
        }),
        cmd(Command::Move {
            units: vec![ship],
            target: FxVec2::from_ints(2500, 2500),
            queue: true,
        }),
    ])
    .unwrap();
    let row = w.state.units.row(ship).unwrap();
    assert_eq!(w.state.orders.iter(&w.state.units, row).count(), 2);
    assert_eq!(w.state.units.pos[row], start, "a half-built ship moved");
    finish(&mut w, ship);
    let arrived = (0..TICKS_PER_SECOND * 90).any(|_| {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(ship).unwrap();
        w.state.units.pos[row].distance(goal) < Fx::from_int(100)
    });
    assert!(arrived, "the finished ship never went where it was sent");

    // Stop reaches a half-built ship too.
    let other = corvette_site(&mut w);
    w.tick(&[cmd(Command::Move {
        units: vec![other],
        target: goal,
        queue: false,
    })])
    .unwrap();
    w.tick(&[cmd(Command::Stop { units: vec![other] })])
        .unwrap();
    assert_eq!(front(&w, other), None, "Stop left the site's orders");
}
