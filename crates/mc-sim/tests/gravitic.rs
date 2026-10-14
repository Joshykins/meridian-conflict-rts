//! The Fallen, the Regency's gravitic engine (`gravitic.rs`): aimed anywhere, it gathers,
//! tears a rift through the ground to the point and holds a gravity well there that
//! crushes and slows the enemy (their aircraft with a quarter of it), until it is aimed elsewhere
//! or stopped. A shield dome over the well takes the crush for what is under it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::gravitic::WellPhase;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const ENGINE: &str = "regency_t5_gravitic_engine";
const TANK: &str = "aster_t1_tank";
const INTERCEPTOR: &str = "aster_t1_interceptor";
const ASF: &str = "aster_t3_air_superiority";
const DOME: &str = "aster_t3_shield";
const GROUND: i32 = 40;
/// A 4 km map.
const CELLS: u32 = 512;
/// Where the engine stands, and the well's point 2.4 km off.
const AT: (i32, i32) = (500, 500);
const MARK: (i32, i32) = (2900, 500);

fn world() -> World {
    world_with(false)
}

fn world_with(fog: bool) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let stride = CELLS as usize + 1;
    let samples = vec![GROUND as u16; stride * stride];
    let terrain = Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
    let map = MapData {
        doodads: Default::default(),
        name: "gravitic".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(3800, 3800)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 7,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // The engine's side can pay for it, and the other's domes stay up.
    for p in &mut w.state.players {
        p.free_build = true;
    }
    w
}

/// A finished unit that never shoots back.
fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.flags[row] |= flag::PASSIVE;
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state.units.health[row(w, id)]
}

fn run(w: &mut World, commands: &[PlayerCommand]) {
    w.tick(commands).unwrap();
}

fn aim(w: &mut World, engine: UnitId, (x, y): (i32, i32)) {
    let c = PlayerCommand {
        player: 0,
        command: Command::Fracture {
            units: vec![engine],
            pos: FxVec2::from_ints(x, y),
        },
    };
    run(w, &[c]);
}

/// Ticks until an event `pick` finds, up to `limit`.
fn until<T>(w: &mut World, limit: u32, pick: impl Fn(&SimEvent) -> Option<T>) -> Option<T> {
    for _ in 0..limit {
        run(w, &[]);
        if let Some(v) = w.events.iter().find_map(&pick) {
            return Some(v);
        }
    }
    None
}

fn struck(w: &mut World) -> bool {
    until(w, 300, |e| {
        matches!(e, SimEvent::RiftStruck { .. }).then_some(())
    })
    .is_some()
}

/// It gathers for its 15 s, the rift runs 2.4 km at 900 m/s, and it strikes at the point.
#[test]
fn gathers_then_the_rift_runs_to_the_point_and_strikes() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let start = w.tick_count();
    aim(&mut w, engine, MARK);
    let launched = until(&mut w, 200, |e| match e {
        SimEvent::RiftLaunched { ticks, .. } => Some(*ticks),
        _ => None,
    })
    .expect("the rift left");
    assert_eq!(w.tick_count() - start, 150, "it gathered for 15 s");
    assert_eq!(launched, 27, "2.4 km at 90 m a tick");
    let at = until(&mut w, 40, |e| match e {
        SimEvent::RiftStruck { pos, .. } => Some(pos.xy()),
        _ => None,
    })
    .expect("it struck");
    assert_eq!(at, FxVec2::from_ints(MARK.0, MARK.1));
    assert!(matches!(w.state.wells[0].phase, WellPhase::Holding { .. }));
}

/// The well crushes the enemy's units in it, as hard at its edge as at its heart, a tick at
/// a time with no blow as it strikes; friends and whatever is outside are untouched.
#[test]
fn the_well_crushes_the_enemy_and_spares_friends() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let heart = add(&mut w, TANK, 1, MARK.0, MARK.1);
    let rim = add(&mut w, TANK, 1, MARK.0 + 260, MARK.1);
    let outside = add(&mut w, TANK, 1, MARK.0, MARK.1 + 500);
    let friend = add(&mut w, TANK, 0, MARK.0 - 60, MARK.1);
    // Tough enough to live through it, to measure it.
    for u in [heart, rim, outside, friend] {
        let r = row(&w, u);
        w.state.units.health[r] = Fx::from_int(100_000);
    }
    let full = health(&w, heart);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    // One tick's crush as it strikes: 800 a second, no blow on top.
    let tick = Fx::from_int(80);
    assert_eq!(health(&w, heart), full - tick, "no blow as it strikes");
    assert_eq!(
        health(&w, rim),
        full - tick,
        "the rim takes what the heart does"
    );
    for _ in 0..10 {
        run(&mut w, &[]);
    }
    assert_eq!(
        health(&w, heart),
        full - tick * 11,
        "the well goes on crushing"
    );
    assert_eq!(health(&w, rim), health(&w, heart));
    assert_eq!(health(&w, outside), full);
    assert_eq!(health(&w, friend), full);
}

/// An enemy driving through the well crawls: it keeps a third of its speed.
#[test]
fn the_well_slows_what_is_in_it() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let tank = add(&mut w, TANK, 1, MARK.0 - 100, MARK.1 + 100);
    let free = add(&mut w, TANK, 1, MARK.0 - 100, MARK.1 + 1000);
    // Enough health to live through it: this measures the pace, not the crush.
    for t in [tank, free] {
        let r = row(&w, t);
        w.state.units.flags[r] |= flag::INVULNERABLE;
    }
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    let go = |to: (i32, i32), u| PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![u],
            target: FxVec2::from_ints(to.0, to.1),
            queue: false,
        },
    };
    run(
        &mut w,
        &[
            go((MARK.0 + 200, MARK.1 + 100), tank),
            go((MARK.0 + 200, MARK.1 + 1000), free),
        ],
    );
    let (a0, b0) = (
        w.state.units.pos[row(&w, tank)],
        w.state.units.pos[row(&w, free)],
    );
    for _ in 0..40 {
        run(&mut w, &[]);
    }
    let slowed = w.state.units.pos[row(&w, tank)].distance(a0);
    let freely = w.state.units.pos[row(&w, free)].distance(b0);
    assert!(
        slowed * 2 < freely,
        "in the well it went {slowed} m, outside {freely} m"
    );
}

/// An aircraft in the well takes a quarter of the crush: 200 a second, not 800.
#[test]
fn aircraft_take_a_quarter_of_the_crush() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let plane = add(&mut w, INTERCEPTOR, 1, MARK.0, MARK.1);
    let tank = add(&mut w, TANK, 1, MARK.0 + 60, MARK.1);
    for u in [plane, tank] {
        let r = row(&w, u);
        w.state.units.health[r] = Fx::from_int(100_000);
    }
    let full = health(&w, plane);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    for _ in 0..9 {
        run(&mut w, &[]);
    }
    assert_eq!(
        health(&w, tank),
        full - Fx::from_int(800),
        "a second's crush"
    );
    assert_eq!(
        health(&w, plane),
        full - Fx::from_int(200),
        "a quarter of it in the air"
    );
}

/// An air superiority fighter flown straight across the well is slowed and hurt, but
/// gets out alive.
#[test]
fn a_fighter_crossing_the_well_gets_out() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    let plane = add(&mut w, ASF, 1, MARK.0 - 900, MARK.1);
    let full = health(&w, plane);
    let fly = PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![plane],
            target: FxVec2::from_ints(MARK.0 + 1500, MARK.1),
            queue: false,
        },
    };
    run(&mut w, &[fly]);
    let out = Fx::from_int(MARK.0 + 400);
    for _ in 0..400 {
        run(&mut w, &[]);
        let Some(r) = w.state.units.row(plane) else {
            panic!("the fighter died in the well");
        };
        if w.state.units.pos[r].x > out {
            break;
        }
    }
    let r = row(&w, plane);
    assert!(w.state.units.pos[r].x > out, "it got across");
    let left = health(&w, plane);
    assert!(left < full, "the well hurt it");
    assert!(
        left * 2 > full,
        "it got out with over half its health: {left} of {full}"
    );
}

/// A shield dome over the well takes the crush; what stands under it is spared while the
/// dome holds.
#[test]
fn a_dome_takes_the_crush_for_what_is_under_it() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let dome = add(&mut w, DOME, 1, MARK.0 + 150, MARK.1);
    let under = add(&mut w, TANK, 1, MARK.0 + 150, MARK.1 + 40);
    // Let the dome rise.
    for _ in 0..40 {
        run(&mut w, &[]);
    }
    let full = health(&w, under);
    let field = w.state.units.shield_hp[row(&w, dome)];
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    for _ in 0..5 {
        run(&mut w, &[]);
    }
    assert_eq!(health(&w, under), full, "spared under the dome");
    assert!(
        w.state.units.shield_hp[row(&w, dome)] < field,
        "the dome took it"
    );
}

/// Aimed elsewhere, it lets its well go and gathers anew; a stop lets it go too.
#[test]
fn aimed_again_or_stopped_it_lets_the_well_go() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    aim(&mut w, engine, (MARK.0, MARK.1 + 1500));
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, SimEvent::WellReleased { radius, .. } if *radius > Fx::ZERO)));
    assert!(matches!(
        w.state.wells[0].phase,
        WellPhase::Gathering { .. }
    ));
    let stop = PlayerCommand {
        player: 0,
        command: Command::Stop {
            units: vec![engine],
        },
    };
    run(&mut w, &[stop]);
    assert!(w.state.wells.is_empty());
}

/// It draws its upkeep only while it is at work.
#[test]
fn it_draws_energy_only_at_work() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    run(&mut w, &[]);
    let idle = w.state.players[0].energy_demand;
    aim(&mut w, engine, MARK);
    run(&mut w, &[]);
    let busy = w.state.players[0].energy_demand;
    assert!(busy > idle, "idle {idle}, at work {busy}");
}

/// Under fog, an enemy who sees only the near side of the well (not its middle, nor the engine
/// far off) still has it drawn: its purple light shows over the units it crushes.
#[test]
fn an_enemy_seeing_part_of_the_well_sees_it() {
    let mut w = world_with(true);
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    // Outside the well: it sees the near side of it, not its middle.
    add(&mut w, TANK, 1, MARK.0 + 500, MARK.1);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    let mut frame = RenderFrame::default();
    w.write_render_frame(Some(1), &mut frame);
    assert!(
        frame.units.iter().all(|u| u.unit_id != engine.0),
        "the engine is out of the enemy's sight"
    );
    assert_eq!(frame.wells.len(), 1, "the well is in the enemy's frame");
}
