//! The Skyhook space elevator: once finished its cable comes down and is clamped, and
//! then every period three cargo crates fall down it with its queue, free, as much as
//! fits in their payload and their seats, and pay the rest of the payload into the store.
//! Each crate's units drive out of its doors between the towers and leave as one group.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::SimEvent;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const ELEVATOR: &str = "aster_t5_space_elevator";
const AT: (i32, i32) = (2048, 2048);

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    // 512 cells of 8 m: 4 km a side.
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(AT.0, AT.1), FxVec2::from_ints(3600, 3600)],
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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Room in the store for a whole payload and more.
    w.state.players[0].bonus_storage[0] = Fx::from_int(200_000);
    w
}

fn elevator(w: &mut World) -> UnitId {
    let bp = w.blueprints.id_of(ELEVATOR).unwrap();
    let row = w
        .spawn_unit(bp, 0, FxVec2::from_ints(AT.0, AT.1), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn spec(w: &World) -> mc_data::elevator::Elevator {
    w.blueprints
        .unit(w.blueprints.id_of(ELEVATOR).unwrap())
        .elevator
        .unwrap()
}

fn produce(w: &World, id: UnitId, key: &str, count: u8) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::Produce {
            factories: vec![id],
            blueprint: w.blueprints.id_of(key).unwrap(),
            count,
        },
    }
}

/// Ticks until the first crates land, counted from the tick after it was spawned.
fn first_dock(w: &World) -> u32 {
    let e = spec(w);
    // The clock starts the tick after it is finished (at 0), and counts from there.
    1 + e.tether_ticks + e.descent_ticks
}

/// Runs `ticks` ticks, gathering the events.
fn run(w: &mut World, ticks: u32, commands: &[PlayerCommand]) -> Vec<SimEvent> {
    let mut seen = Vec::new();
    for i in 0..ticks {
        let c: &[PlayerCommand] = if i == 0 { commands } else { &[] };
        w.tick(c).unwrap();
        seen.extend(w.events.iter().cloned());
    }
    seen
}

fn count_of(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp && w.state.units.owner[r] == 0)
        .count()
}

#[test]
fn the_cable_comes_down_then_crates_fall_on_the_period() {
    let mut w = world();
    let id = elevator(&mut w);
    let e = spec(&w);
    let n = first_dock(&w) + e.period_ticks;
    let events = run(&mut w, n, &[]);
    let tick_of = |want: fn(&SimEvent) -> bool| -> Vec<usize> {
        events
            .iter()
            .enumerate()
            .filter(|(_, ev)| want(ev))
            .map(|(i, _)| i)
            .collect()
    };
    let tethering = tick_of(|ev| matches!(ev, SimEvent::ElevatorTethering { .. }));
    let tethered = tick_of(|ev| matches!(ev, SimEvent::ElevatorTethered { .. }));
    let inbound = tick_of(|ev| matches!(ev, SimEvent::ElevatorInbound { .. }));
    let docked: Vec<_> = events
        .iter()
        .filter_map(|ev| match ev {
            SimEvent::ElevatorDocked {
                unit, materials, ..
            } => Some((*unit, *materials)),
            _ => None,
        })
        .collect();
    assert_eq!(tethering.len(), 1, "the cable comes down once");
    assert_eq!(tethered.len(), 1, "and is clamped once");
    assert_eq!(inbound.len(), 2, "crates each period");
    assert_eq!(docked.len(), 2, "each fall lands");
    assert!(docked.iter().all(|&(u, _)| u == id));
    // With nothing queued the whole payload comes down as materials.
    assert!(docked.iter().all(|&(_, m)| m == e.payload));
}

#[test]
fn empty_crates_pay_their_payload_into_the_store() {
    let mut w = world();
    elevator(&mut w);
    let e = spec(&w);
    let n = first_dock(&w) - 1;
    run(&mut w, n, &[]);
    let before = w.state.players[0].materials;
    run(&mut w, 1, &[]);
    let gained = w.state.players[0].materials - before;
    assert!(
        gained >= e.payload - Fx::from_int(1),
        "the store gains the payload: {gained:?}"
    );
}

#[test]
fn queued_units_come_down_free_up_to_the_payload_and_seats_in_order() {
    let mut w = world();
    let id = elevator(&mut w);
    let e = spec(&w);
    // Assault bots first, then tanks behind them. As many bots ride as the payload and the
    // seats take, and the next does not fit: nothing behind it comes down on this fall.
    let bot = w.blueprints.id_of("aster_t3_assault_bot").unwrap();
    let (cost, radius) = {
        let bp = w.blueprints.unit(bot);
        (bp.cost_materials, bp.radius)
    };
    let (fits, materials) =
        mc_sim::mirror::crate_load(std::iter::repeat_n((radius, cost), 200), e.payload);
    assert!(fits > 1 && fits < 200, "the payload stops the bots: {fits}");
    let cmds = [
        produce(&w, id, "aster_t3_assault_bot", fits as u8 + 1),
        produce(&w, id, "aster_t1_tank", 4),
    ];
    let n = first_dock(&w);
    let events = run(&mut w, n, &cmds);
    assert_eq!(count_of(&w, "aster_t3_assault_bot"), fits as usize);
    assert_eq!(
        count_of(&w, "aster_t1_tank"),
        0,
        "the queue keeps its order"
    );
    let paid = events
        .iter()
        .find_map(|ev| match ev {
            SimEvent::ElevatorDocked {
                units, materials, ..
            } => Some((*units, *materials)),
            _ => None,
        })
        .unwrap();
    assert_eq!(paid.0, fits);
    assert_eq!(paid.1, e.payload - materials);
    // The next fall takes the last bot and the tanks.
    run(&mut w, e.period_ticks, &[]);
    assert_eq!(count_of(&w, "aster_t3_assault_bot"), fits as usize + 1);
    assert_eq!(count_of(&w, "aster_t1_tank"), 4);
}

#[test]
fn nothing_is_built_while_waiting_and_nothing_is_paid_for() {
    let mut w = world();
    let id = elevator(&mut w);
    w.state.players[0].materials = Fx::from_int(0);
    let order = produce(&w, id, "aster_t3_assault_bot", 3);
    run(&mut w, 40, &[order]);
    assert_eq!(count_of(&w, "aster_t3_assault_bot"), 0, "no hull on a pad");
    let row = w.state.units.row(id).unwrap();
    assert!(!w.state.units.has_flag(row, flag::BUILDING));
    assert_eq!(w.state.players[0].materials, Fx::ZERO, "nothing spent");
}

#[test]
fn its_units_drive_out_of_the_crates_between_the_towers_and_away() {
    let mut w = world();
    let id = elevator(&mut w);
    let cmds = [
        produce(&w, id, "aster_t1_tank", 30),
        produce(&w, id, "aster_t1_interceptor", 4),
    ];
    let n = first_dock(&w);
    run(&mut w, n, &cmds);
    run(&mut w, 600, &[]);
    let centre = FxVec2::from_ints(AT.0, AT.1);
    let half = Fx::from_int(26 * mc_map::BUILD_CELL_M / 2);
    for key in ["aster_t1_tank", "aster_t1_interceptor"] {
        let bp = w.blueprints.id_of(key).unwrap();
        let rows: Vec<usize> = w
            .state
            .units
            .slots
            .iter()
            .filter(|&r| w.state.units.blueprint[r] == bp)
            .collect();
        assert!(!rows.is_empty(), "{key} came down");
        for r in rows {
            assert!(w.state.units.is_active(r), "{key} is out of the bay");
            let d = w.state.units.pos[r].distance(centre);
            assert!(d > half, "{key} left the lot: {d:?} from the middle");
        }
    }
}

#[test]
fn a_repeated_queue_comes_down_on_every_fall() {
    let mut w = world();
    let id = elevator(&mut w);
    let e = spec(&w);
    let repeat = PlayerCommand {
        player: 0,
        command: Command::SetRepeat {
            factories: vec![id],
            repeat: true,
        },
    };
    let order = produce(&w, id, "aster_t3_assault_bot", 2);
    w.tick(&[repeat, order]).unwrap();
    let n = first_dock(&w) - 1 + 2 * e.period_ticks;
    run(&mut w, n, &[]);
    assert_eq!(
        count_of(&w, "aster_t3_assault_bot"),
        6,
        "two a fall, three falls"
    );
}

#[test]
fn it_brings_down_no_ships_and_no_titans() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(ELEVATOR).unwrap());
    let builds = &bp.builder.as_ref().unwrap().builds;
    for key in ["aster_t1_frigate", "aster_t5_titan", "aster_t1_engineer"] {
        let made = w.blueprints.id_of(key).unwrap();
        assert_eq!(builds.contains(&made), key == "aster_t1_engineer", "{key}");
    }
}

#[test]
fn an_unfinished_elevator_lowers_nothing() {
    let mut w = world();
    let bp = w.blueprints.id_of(ELEVATOR).unwrap();
    let row = w
        .spawn_unit(bp, 0, FxVec2::from_ints(AT.0, AT.1), Angle::ZERO, false)
        .unwrap();
    assert!(w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION));
    let events = run(&mut w, 400, &[]);
    assert!(!events.iter().any(|ev| matches!(
        ev,
        SimEvent::ElevatorTethering { .. } | SimEvent::ElevatorDocked { .. }
    )));
}

#[test]
fn a_paused_elevator_holds_its_queue_and_brings_down_materials() {
    let mut w = world();
    let id = elevator(&mut w);
    let e = spec(&w);
    let pause = PlayerCommand {
        player: 0,
        command: Command::SetPaused {
            units: vec![id],
            paused: true,
        },
    };
    let order = produce(&w, id, "aster_t1_tank", 5);
    w.tick(&[pause, order]).unwrap();
    let n = first_dock(&w) - 1;
    let events = run(&mut w, n, &[]);
    assert_eq!(count_of(&w, "aster_t1_tank"), 0, "the queue waits");
    assert!(events.iter().any(
        |ev| matches!(ev, SimEvent::ElevatorDocked { units: 0, materials, .. } if *materials == e.payload)
    ));
}

#[test]
fn nothing_is_set_down_by_the_cable() {
    let mut w = world();
    let id = elevator(&mut w);
    let order = produce(&w, id, "aster_t1_scout", 60);
    w.tick(&[order]).unwrap();
    let n = first_dock(&w) - 1;
    run(&mut w, n, &[]);
    let centre = FxVec2::from_ints(AT.0, AT.1);
    let bp = w.blueprints.id_of("aster_t1_scout").unwrap();
    let near = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .map(|r| w.state.units.pos[r].distance(centre))
        .fold(Fx::from_int(9999), |a, b| a.min(b));
    assert!(near >= Fx::from_int(30), "nearest {near:?}");
}

/// Counts the units of `key` that are still in their crates (not yet rolled out).
fn in_crates(w: &World) -> usize {
    w.state.rollouts.len()
}

#[test]
fn units_wait_for_the_doors_then_leave_a_crate_together() {
    let mut w = world();
    let id = elevator(&mut w);
    let order = produce(&w, id, "aster_t1_tank", 5);
    w.tick(&[order]).unwrap();
    let n = first_dock(&w) - 2;
    run(&mut w, n, &[]);
    assert_eq!(count_of(&w, "aster_t1_tank"), 0);
    run(&mut w, 1, &[]);
    assert_eq!(
        count_of(&w, "aster_t1_tank"),
        5,
        "set down as the crates land"
    );
    let tanks = in_crates(&w);
    assert_eq!(tanks, 5, "all of them are in the crates");
    // The doors take a while to open: nothing has moved yet.
    let at: Vec<_> = w
        .state
        .rollouts
        .keys()
        .map(|&u| {
            let r = w.state.units.row(u).unwrap();
            w.state.units.pos[r]
        })
        .collect();
    run(&mut w, 20, &[]);
    for (&u, p) in w.state.rollouts.keys().zip(&at) {
        let r = w.state.units.row(u).unwrap();
        assert_eq!(w.state.units.pos[r], *p, "held while the doors open");
    }
    // They are one group: a squad stands for them until the last is out, and then goes.
    assert_eq!(w.state.elevators.squads.len(), 1);
    let mut ticks = 0;
    while !w.state.elevators.squads.is_empty() && ticks < 400 {
        run(&mut w, 1, &[]);
        ticks += 1;
    }
    assert_eq!(in_crates(&w), 0, "all are out");
    assert!(
        w.state.elevators.squads.is_empty(),
        "the group has been sent off"
    );
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let moving = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == tank)
        .filter(|&r| w.state.orders.front(&w.state.units, r).is_some())
        .count();
    assert_eq!(moving, 5, "and each has its orders");
}

#[test]
fn a_batch_overrides_the_crates_groups() {
    let mut w = world();
    let id = elevator(&mut w);
    let batch = PlayerCommand {
        player: 0,
        command: Command::SetBatch {
            factories: vec![id],
            batch: true,
        },
    };
    let order = produce(&w, id, "aster_t1_tank", 4);
    w.tick(&[batch, order]).unwrap();
    let n = first_dock(&w) - 1;
    run(&mut w, n + 1, &[]);
    assert!(
        w.state.elevators.squads.is_empty(),
        "no crate group is made"
    );
    run(&mut w, 400, &[]);
    assert_eq!(in_crates(&w), 0);
    assert!(
        !w.state.batches.is_empty(),
        "the tanks muster in the batch instead"
    );
}

#[test]
fn units_still_inside_when_the_crates_sink_are_put_out() {
    let mut w = world();
    let id = elevator(&mut w);
    let e = spec(&w);
    let order = produce(&w, id, "aster_t3_assault_bot", 12);
    w.tick(&[order]).unwrap();
    let sink = first_dock(&w) - 1 + e.dock_ticks;
    run(&mut w, sink + 2, &[]);
    assert_eq!(in_crates(&w), 0, "nothing is left in a sinking crate");
}
