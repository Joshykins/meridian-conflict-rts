//! The Fallen, the Regency's gravitic engine (`gravitic.rs`): aimed anywhere, it gathers,
//! tears a rift through the ground to the point and holds a gravity well there that
//! crushes and slows the enemy and tears their aircraft down, until it is aimed elsewhere
//! or stopped. A shield dome over the well takes the crush for what is under it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::gravitic::WellPhase;
use mc_sim::tables::{flag, Controller, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const ENGINE: &str = "regency_t5_gravitic_engine";
const TANK: &str = "aster_t1_tank";
const INTERCEPTOR: &str = "aster_t1_interceptor";
const DOME: &str = "aster_t3_shield";
const GROUND: i32 = 40;
/// A 4 km map.
const CELLS: u32 = 512;
/// Where the engine stands, and the well's point 2.4 km off.
const AT: (i32, i32) = (500, 500);
const MARK: (i32, i32) = (2900, 500);

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let stride = CELLS as usize + 1;
    let samples = vec![GROUND as u16; stride * stride];
    let terrain = Heightfield::from_samples(CELLS, CELLS, samples, Fx::ZERO, Fx::ONE, Fx::ZERO);
    let map = MapData {
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
        fog: false,
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

/// The strike and the well crush the enemy's units in it, less at the edge; friends and
/// whatever is outside are untouched.
#[test]
fn the_well_crushes_the_enemy_and_spares_friends() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    let heart = add(&mut w, TANK, 1, MARK.0, MARK.1);
    let rim = add(&mut w, TANK, 1, MARK.0 + 260, MARK.1);
    let outside = add(&mut w, TANK, 1, MARK.0, MARK.1 + 500);
    let friend = add(&mut w, TANK, 0, MARK.0 - 60, MARK.1);
    // Tough enough to live through the strike, to measure it.
    for u in [heart, rim, outside, friend] {
        let r = row(&w, u);
        w.state.units.health[r] = Fx::from_int(100_000);
    }
    let full = health(&w, heart);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    let (h, r) = (health(&w, heart), health(&w, rim));
    assert!(h < full && r < full, "struck: heart {h}, rim {r}");
    assert!(r > h, "the rim takes less than the heart");
    for _ in 0..10 {
        run(&mut w, &[]);
    }
    assert!(health(&w, rim) < r, "the well goes on crushing");
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

/// An aircraft flying into the well is torn out of the sky and falls as a crash.
#[test]
fn aircraft_are_torn_down() {
    let mut w = world();
    let engine = add(&mut w, ENGINE, 0, AT.0, AT.1);
    aim(&mut w, engine, MARK);
    assert!(struck(&mut w));
    let plane = add(&mut w, INTERCEPTOR, 1, MARK.0 - 900, MARK.1);
    let fly = PlayerCommand {
        player: 1,
        command: Command::Move {
            units: vec![plane],
            target: FxVec2::from_ints(MARK.0 + 900, MARK.1),
            queue: false,
        },
    };
    run(&mut w, &[fly]);
    let downed = until(&mut w, 300, |e| match e {
        SimEvent::AircraftDowned { unit, .. } => Some(*unit),
        _ => None,
    });
    assert_eq!(downed, Some(plane));
    run(&mut w, &[]);
    assert!(w.state.units.row(plane).is_none(), "it died");
    assert!(
        w.state
            .aircraft_crashes
            .iter()
            .any(|c| c.unit_id == plane.0 && c.velocity.z < -Fx::from_int(4)),
        "it falls, thrown at the ground"
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
