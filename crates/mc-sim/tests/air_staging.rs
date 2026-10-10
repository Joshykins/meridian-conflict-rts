//! Air staging (`staging.rs`): aircraft burn fuel while up and fly slower dry; free ones
//! that run low or are hurt go down on a staging base (the Roost, a carrier), fill and
//! mend aboard, and leave again to what they were doing.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, Handle, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world(terrain: Heightfield) -> World {
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn land() -> World {
    world(Heightfield::flat(512, 512, Fx::from_int(20)))
}

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(bp, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn range_of(w: &World, row: usize) -> Fx {
    w.blueprints
        .unit(w.state.units.blueprint[row])
        .fuel
        .expect("a tank")
        .range
}

fn step(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn order(w: &mut World, command: Command) {
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
}

fn front(w: &World, row: usize) -> Option<OrderKind> {
    w.state.orders.front(&w.state.units, row).map(|o| o.kind)
}

#[test]
fn an_aircraft_burns_fuel_in_the_air_and_flies_slower_dry() {
    let mut w = land();
    let fighter = spawn(&mut w, "aster_t1_interceptor", 1000, 1000);
    let full = range_of(&w, fighter);
    assert_eq!(w.state.units.fuel[fighter], full, "built full");
    let id = w.state.units.id(fighter);
    let go = |to: i32| Command::Move {
        units: vec![id],
        target: FxVec2::from_ints(to, 1000),
        queue: false,
    };
    order(&mut w, go(3000));
    step(&mut w, 200);
    let used = full - w.state.units.fuel[fighter];
    assert!(used > Fx::from_int(500), "burned {used:?} flying");

    // Dry, it flies at half speed at most.
    let speed = w
        .blueprints
        .unit(w.state.units.blueprint[fighter])
        .motion
        .unwrap()
        .speed;
    w.state.units.fuel[fighter] = Fx::ZERO;
    order(&mut w, go(500));
    step(&mut w, 300);
    assert!(
        w.state.units.speed[fighter] <= speed / 2 + Fx::ONE,
        "dry at {:?} of {speed:?}",
        w.state.units.speed[fighter]
    );
}

#[test]
fn a_low_aircraft_in_reach_goes_down_on_the_roost_fills_mends_and_leaves() {
    let mut w = land();
    let roost = spawn(&mut w, "aster_t1_airbase", 1200, 1200);
    let fighter = spawn(&mut w, "aster_t1_interceptor", 1600, 1200);
    let full = range_of(&w, fighter);
    let max = w.unit_max_health(fighter);
    w.state.units.fuel[fighter] = full / 5;
    w.state.units.health[fighter] = max / 2;
    step(&mut w, 20);
    assert_eq!(front(&w, fighter), Some(OrderKind::Stage), "sent down");
    let roost_id = w.state.units.id(roost);
    let mut aboard_at = None;
    for t in 0..1200 {
        w.tick(&[]).unwrap();
        if w.state.units.hangar[fighter] == roost_id {
            aboard_at = Some(t);
            break;
        }
    }
    assert!(aboard_at.is_some(), "it went aboard");
    assert!(w.state.units.deploy[roost] > 0, "the hatch opened for it");
    // Out again, full and mended, with nothing left of the Stage.
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
        if w.state.units.hangar[fighter] == Handle::NONE {
            break;
        }
    }
    assert_eq!(w.state.units.hangar[fighter], Handle::NONE, "it left");
    assert_eq!(w.state.units.fuel[fighter], full);
    assert_eq!(w.state.units.health[fighter], max);
    assert_ne!(front(&w, fighter), Some(OrderKind::Stage));
}

#[test]
fn a_guarding_aircraft_goes_back_to_its_guard_after_staging() {
    let mut w = land();
    spawn(&mut w, "aster_t1_airbase", 1200, 1200);
    let fighter = spawn(&mut w, "aster_t1_interceptor", 1500, 1500);
    let id = w.state.units.id(fighter);
    order(
        &mut w,
        Command::Guard {
            units: vec![id],
            pos: FxVec2::from_ints(1500, 1500),
            target: Handle::NONE,
            radius: Fx::from_int(300),
            queue: false,
        },
    );
    step(&mut w, 10);
    let full = range_of(&w, fighter);
    w.state.units.fuel[fighter] = full / 4;
    step(&mut w, 20);
    assert_eq!(front(&w, fighter), Some(OrderKind::Stage));
    for _ in 0..2400 {
        w.tick(&[]).unwrap();
        if w.state.units.fuel[fighter] == full && w.state.units.hangar[fighter] == Handle::NONE {
            break;
        }
    }
    assert_eq!(w.state.units.fuel[fighter], full, "filled");
    assert_eq!(front(&w, fighter), Some(OrderKind::Guard), "back on guard");
}

#[test]
fn a_player_order_is_not_cut_short_but_a_free_aircraft_at_bingo_flies_home() {
    let mut w = land();
    let roost = spawn(&mut w, "aster_t1_airbase", 1000, 1000);
    // Far out of the Roost's reach, nearly dry.
    let busy = spawn(&mut w, "aster_t1_interceptor", 4000, 4000);
    let free = spawn(&mut w, "aster_t1_interceptor", 4000, 3800);
    for r in [busy, free] {
        w.state.units.fuel[r] = Fx::from_int(5000);
    }
    let id = w.state.units.id(busy);
    order(
        &mut w,
        Command::Move {
            units: vec![id],
            target: FxVec2::from_ints(4500, 4500),
            queue: false,
        },
    );
    step(&mut w, 20);
    assert_eq!(
        front(&w, busy),
        Some(OrderKind::Move),
        "the player's move stands"
    );
    let o = w.state.orders.front(&w.state.units, free).copied();
    assert_eq!(
        o.map(|o| o.kind),
        Some(OrderKind::Stage),
        "the free one turns home"
    );
    assert_eq!(o.unwrap().target, w.state.units.id(roost));
}

#[test]
fn what_is_aboard_a_base_dies_with_it() {
    let mut w = land();
    let roost = spawn(&mut w, "aster_t1_airbase", 1200, 1200);
    let fighter = spawn(&mut w, "aster_t1_interceptor", 1300, 1200);
    w.state.units.fuel[fighter] = Fx::from_int(100);
    let roost_id = w.state.units.id(roost);
    for _ in 0..1200 {
        w.tick(&[]).unwrap();
        if w.state.units.hangar[fighter] == roost_id {
            break;
        }
    }
    assert_eq!(w.state.units.hangar[fighter], roost_id);
    let fighter_id = w.state.units.id(fighter);
    w.state.units.health[roost] = Fx::ZERO;
    step(&mut w, 5);
    assert!(
        w.state.units.row(fighter_id).is_none(),
        "lost with the base"
    );
}

#[test]
fn a_carrier_takes_aircraft_in_over_the_stern_and_launches_them_off_the_bow() {
    let mut w = world(Heightfield::flat(512, 512, Fx::from_int(-40)));
    let carrier = spawn(&mut w, "aster_t3_carrier", 2000, 2000);
    let fighter = spawn(&mut w, "aster_t1_interceptor", 2600, 2000);
    let staging = w
        .blueprints
        .unit(w.state.units.blueprint[carrier])
        .staging
        .unwrap();
    w.state.units.fuel[fighter] = range_of(&w, fighter) / 5;
    let carrier_id = w.state.units.id(carrier);
    // It comes round astern (x < the carrier's) before it goes down.
    let mut astern = false;
    for _ in 0..2400 {
        w.tick(&[]).unwrap();
        let x = w.state.units.pos[fighter].x - w.state.units.pos[carrier].x;
        astern |= x < Fx::from_int(-300);
        if w.state.units.hangar[fighter] == carrier_id {
            break;
        }
    }
    assert!(astern, "came in from astern");
    assert_eq!(w.state.units.hangar[fighter], carrier_id, "went aboard");
    for _ in 0..2400 {
        w.tick(&[]).unwrap();
        if w.state.units.hangar[fighter] == Handle::NONE {
            break;
        }
    }
    assert_eq!(w.state.units.hangar[fighter], Handle::NONE, "launched");
    let off = w.state.units.pos[fighter] - w.state.units.pos[carrier];
    assert!(
        (off.x - staging.launch[0].0.x).abs() < Fx::from_int(40),
        "off the bow: {off:?}"
    );
}
