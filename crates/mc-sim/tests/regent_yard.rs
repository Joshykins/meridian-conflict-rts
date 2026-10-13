//! The Regent's yard (`regency_t5_sky_yard`, `sky_yard.rs`): the hull is raised in the
//! sky over the yard at its cruise height, out where it can be shot at, never in a bay;
//! builders helping it come to the yard; finished, it gets under way; if the yard falls
//! while it is half built, it comes down too, its wreck worth what was built; and
//! nothing but the yard builds it.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const YARD: &str = "regency_t5_sky_yard";
const REGENT: &str = "regency_t5_super_destroyer";
const ENGINEER: &str = "regency_t3_engineer";
const AT: (i32, i32) = (4096, 4096);
const GROUND: i32 = 20;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    // 1024 cells of 8 m: 8 km a side, room for the Regent to get under way.
    let terrain = Heightfield::flat(1024, 1024, Fx::from_int(GROUND));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(AT.0, AT.1), FxVec2::from_ints(7600, 7600)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Regency".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 9,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    w.state.players[0].free_build = true;
    w
}

fn spawn(w: &mut World, key: &str, x: i32, y: i32, heading: Angle) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, 0, FxVec2::from_ints(x, y), heading, true)
        .unwrap();
    w.state.units.id(row)
}

fn command(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn produce(w: &World, yard: UnitId) -> PlayerCommand {
    command(Command::Produce {
        factories: vec![yard],
        blueprint: w.blueprints.id_of(REGENT).unwrap(),
        count: 1,
    })
}

fn run(w: &mut World, ticks: u32, commands: &[PlayerCommand]) {
    for i in 0..ticks {
        let c: &[PlayerCommand] = if i == 0 { commands } else { &[] };
        w.tick(c).unwrap();
    }
}

/// The Regent of player 0's, if there is one.
fn regent(w: &World) -> Option<usize> {
    let bp = w.blueprints.id_of(REGENT).unwrap();
    let u = &w.state.units;
    u.slots.iter().find(|&r| u.blueprint[r] == bp)
}

/// A yard with a Regent going up over it, `heading` its facing.
fn raising(heading: Angle) -> (World, UnitId) {
    let mut w = world();
    let yard = spawn(&mut w, YARD, AT.0, AT.1, heading);
    let start = produce(&w, yard);
    run(&mut w, 3, &[start]);
    (w, yard)
}

#[test]
fn the_hull_goes_up_aloft_over_the_yard() {
    let heading = Angle::from_degrees(30);
    let (mut w, yard) = raising(heading);
    let y = w.state.units.row(yard).unwrap();
    let r = regent(&w).expect("the yard started the Regent");
    let u = &w.state.units;
    assert!(u.has_flag(r, flag::UNDER_CONSTRUCTION));
    assert!(
        !u.has_flag(r, flag::IN_FACTORY),
        "out in the open, not in a bay"
    );
    assert_eq!(u.build_target[y], u.id(r));
    assert_eq!(u.pos[r], u.pos[y], "over the yard");
    assert_eq!(u.heading[r], heading, "under the yard's heading");
    let altitude = w.blueprints.unit(u.blueprint[r]).motion.unwrap().altitude;
    assert_eq!(
        u.z[r],
        Fx::from_int(GROUND) + altitude,
        "at its cruise height"
    );
    // It is built by the yard alone, and gains as it goes; it stays where it is.
    let before = u.build_progress[r];
    run(&mut w, 20, &[]);
    let u = &w.state.units;
    assert!(u.build_progress[r] > before);
    assert_eq!(u.pos[r], u.pos[y]);
    assert!(u.has_flag(r, flag::UNDER_CONSTRUCTION));
}

#[test]
fn helpers_come_to_the_yard_not_under_the_hull() {
    let (mut w, yard) = raising(Angle::ZERO);
    let r = regent(&w).unwrap();
    let hull = w.state.units.id(r);
    // One assists the yard; one is told to help with the hull itself.
    let a = spawn(&mut w, ENGINEER, AT.0 - 1200, AT.1, Angle::ZERO);
    let b = spawn(&mut w, ENGINEER, AT.0 + 1200, AT.1 + 300, Angle::ZERO);
    let orders = [
        command(Command::Assist {
            units: vec![a],
            target: yard,
            queue: false,
        }),
        command(Command::Assist {
            units: vec![b],
            target: hull,
            queue: false,
        }),
    ];
    let before = w.state.units.build_progress[r];
    run(&mut w, 600, &orders);
    let u = &w.state.units;
    let y = u.row(yard).unwrap();
    let yard_bp = w.blueprints.unit(u.blueprint[y]);
    for e in [a, b] {
        let e = u.row(e).unwrap();
        let reach = w
            .blueprints
            .unit(u.blueprint[e])
            .builder
            .as_ref()
            .unwrap()
            .range;
        let off = u.pos[e].distance(u.pos[y]);
        assert!(
            off <= reach + yard_bp.radius + Fx::from_int(40),
            "an engineer stands {off} m from the yard"
        );
        assert_eq!(u.build_target[e], hull, "it works on the hull");
    }
    // With the yard's own power and theirs it gains faster than the yard alone.
    let alone = yard_bp.builder.as_ref().unwrap().power * 60;
    assert!(u.build_progress[r] - before > alone);
}

#[test]
fn finished_it_gets_under_way() {
    let (mut w, yard) = raising(Angle::ZERO);
    let r = regent(&w).unwrap();
    let hull = w.state.units.id(r);
    let finish = command(Command::DebugSetBuild {
        units: vec![hull],
        permille: 999,
    });
    run(&mut w, 30, &[finish]);
    let u = &w.state.units;
    let r = u.row(hull).expect("the Regent stands");
    assert!(!u.has_flag(r, flag::UNDER_CONSTRUCTION));
    assert_eq!(u.health[r], w.unit_max_health(r));
    let y = u.row(yard).unwrap();
    assert_ne!(u.build_target[y], hull, "the yard let it go");
    let order = w.state.orders.front(u, r).map(|o| o.kind);
    assert_eq!(order, Some(OrderKind::Move), "it sets off");
    let from = u.pos[r];
    run(&mut w, 200, &[]);
    let u = &w.state.units;
    let r = u.row(hull).unwrap();
    assert!(
        u.pos[r].distance(from) > Fx::from_int(100),
        "it got under way"
    );
}

#[test]
fn the_yards_fall_brings_the_half_built_hull_down() {
    let (mut w, yard) = raising(Angle::ZERO);
    let r = regent(&w).unwrap();
    let hull = w.state.units.id(r);
    let half = command(Command::DebugSetBuild {
        units: vec![hull],
        permille: 500,
    });
    run(&mut w, 1, &[half]);
    let built = {
        let r = w.state.units.row(hull).unwrap();
        let bp = w.blueprints.unit(w.state.units.blueprint[r]);
        w.state.units.build_progress[r] / bp.build_time
    };
    let y = w.state.units.row(yard).unwrap();
    w.state.units.health[y] = Fx::ZERO;
    run(&mut w, 3, &[]);
    assert!(w.state.units.row(yard).is_none());
    assert!(
        w.state.units.row(hull).is_none(),
        "the hull went with its yard"
    );
    assert_eq!(w.state.aircraft_crashes.len(), 1, "it falls");
    let crash = &w.state.aircraft_crashes[0];
    let bp = w.blueprints.unit(crash.blueprint).clone();
    assert_eq!(bp.key, REGENT);
    // Half, and the yard's last tick of work on it.
    assert!(crash.built >= built && crash.built < built + Fx::ratio(1, 100));
    let built = crash.built;
    let whole = bp.cost_materials * bp.wreck_fraction;
    assert_eq!(crash.materials, whole * built, "worth what was built");
    // Down on the ground it is a wreck of that worth.
    for _ in 0..2000 {
        if w.state.aircraft_crashes.is_empty() {
            break;
        }
        w.tick(&[]).unwrap();
    }
    assert!(w.state.aircraft_crashes.is_empty(), "it came down");
    let wrecks = &w.state.wrecks;
    let wreck = wrecks
        .slots
        .iter()
        .find(|&k| wrecks.blueprint[k] == bp.id)
        .expect("a wreck where it struck");
    assert_eq!(wrecks.materials[wreck], whole * built);
}

#[test]
fn called_off_it_is_gone_not_fallen() {
    let (mut w, yard) = raising(Angle::ZERO);
    let stop = command(Command::Stop { units: vec![yard] });
    run(&mut w, 2, &[stop]);
    assert!(regent(&w).is_none());
    assert!(w.state.aircraft_crashes.is_empty());
}

#[test]
fn nothing_else_builds_it() {
    let mut w = world();
    let engineer = spawn(&mut w, ENGINEER, AT.0, AT.1, Angle::ZERO);
    let regent_bp = w.blueprints.id_of(REGENT).unwrap();
    let e = w.state.units.row(engineer).unwrap();
    let builds = &w
        .blueprints
        .unit(w.state.units.blueprint[e])
        .builder
        .as_ref()
        .unwrap()
        .builds;
    assert!(!builds.contains(&regent_bp));
    assert!(builds.contains(&w.blueprints.id_of(YARD).unwrap()));
    let order = command(Command::Build {
        units: vec![engineer],
        blueprint: regent_bp,
        pos: FxVec2::from_ints(AT.0 + 400, AT.1),
        heading: Angle::ZERO,
        queue: false,
    });
    run(&mut w, 50, &[order]);
    assert!(regent(&w).is_none(), "an engineer cannot raise it");
    // A Regency factory cannot make it either.
    let anvil = w.blueprints.unit_by_key("regency_t3_land_factory").unwrap();
    assert!(!anvil.builder.as_ref().unwrap().builds.contains(&regent_bp));
}
