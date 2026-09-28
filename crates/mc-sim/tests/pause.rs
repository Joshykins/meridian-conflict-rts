//! Paused work: the queue is kept, nothing is spent, and resuming carries on
//! from where it stopped.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })])
    .unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Spawns one finished `key` at (`x`, 512) and returns its id.
fn spawn(w: &mut World, key: &str, x: i32) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let before: Vec<UnitId> = w
        .state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .collect();
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint,
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })])
    .unwrap();
    let units = &w.state.units;
    units
        .slots
        .iter()
        .find(|&r| units.blueprint[r] == blueprint && !before.contains(&units.id(r)))
        .map(|r| units.id(r))
        .expect("spawned")
}

fn pause(w: &mut World, units: Vec<UnitId>, paused: bool) {
    w.tick(&[cmd(Command::SetPaused { units, paused })])
        .unwrap();
}

fn target_progress(w: &World, builder: UnitId) -> Option<mc_core::Fx> {
    let units = &w.state.units;
    let t = units.row(units.build_target[units.row(builder)?])?;
    Some(units.build_progress[t])
}

#[test]
fn a_paused_engineer_leaves_its_site_half_built_and_finishes_it_when_resumed() {
    let mut w = world();
    let mason = spawn(&mut w, "aster_t1_engineer", 500);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    let site = FxVec2::from_ints(540, 512);
    w.tick(&[cmd(Command::Build {
        units: vec![mason],
        blueprint: power,
        pos: site,
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    let started = (0..400).any(|_| {
        w.tick(&[]).unwrap();
        target_progress(&w, mason).is_some_and(|p| p > mc_core::Fx::ZERO)
    });
    assert!(started, "the engineer began the power plant");

    pause(&mut w, vec![mason], true);
    let row = w.state.units.row(mason).unwrap();
    let held = target_progress(&w, mason).expect("the site is still its target");
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        assert!(
            !w.state.units.has_flag(row, flag::BUILDING),
            "a paused engineer spends nothing"
        );
    }
    assert_eq!(
        target_progress(&w, mason),
        Some(held),
        "the site stays where it got to"
    );
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Build),
        "the build order is kept"
    );

    pause(&mut w, vec![mason], false);
    let finished = (0..2000).any(|_| {
        w.tick(&[]).unwrap();
        w.state.orders.front(&w.state.units, row).is_none()
    });
    assert!(finished, "resumed, it finished the plant");
    let units = &w.state.units;
    assert!(
        units
            .slots
            .iter()
            .any(|r| units.blueprint[r] == power && units.is_active(r)),
        "the plant stands complete"
    );
}

#[test]
fn a_paused_engineer_walks_its_queue_but_starts_nothing() {
    let mut w = world();
    let mason = spawn(&mut w, "aster_t1_engineer", 500);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    pause(&mut w, vec![mason], true);
    w.tick(&[cmd(Command::Build {
        units: vec![mason],
        blueprint: power,
        pos: FxVec2::from_ints(620, 512),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let units = &w.state.units;
    let row = units.row(mason).unwrap();
    assert!(
        !units.slots.iter().any(|r| units.blueprint[r] == power),
        "no site was begun"
    );
    assert!(
        units.pos[row].distance(FxVec2::from_ints(620, 512)) < mc_core::Fx::from_int(60),
        "it went within reach of the lot (from 120 m) and waits there"
    );
    assert_eq!(
        w.state.orders.front(units, row).map(|o| o.kind),
        Some(OrderKind::Build)
    );
}

#[test]
fn pausing_a_factory_stops_its_product_and_the_engineers_feeding_it() {
    let mut w = world();
    let factory = spawn(&mut w, "aster_t1_land_factory", 600);
    let mason = spawn(&mut w, "aster_t1_engineer", 520);
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    w.tick(&[
        cmd(Command::Produce {
            factories: vec![factory],
            blueprint: tank,
            count: 1,
        }),
        cmd(Command::Assist {
            units: vec![mason],
            target: factory,
            queue: false,
        }),
    ])
    .unwrap();
    let under_way = (0..300).any(|_| {
        w.tick(&[]).unwrap();
        target_progress(&w, factory).is_some_and(|p| p > mc_core::Fx::ZERO)
    });
    assert!(under_way, "the factory began the tank");

    pause(&mut w, vec![factory], true);
    let held = target_progress(&w, factory).unwrap();
    let helper = w.state.units.row(mason).unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
        assert!(
            !w.state.units.has_flag(helper, flag::BUILDING),
            "the helper stops with the factory"
        );
    }
    assert_eq!(
        target_progress(&w, factory),
        Some(held),
        "the tank waits half made"
    );

    pause(&mut w, vec![factory], false);
    let row = w.state.units.row(factory).unwrap();
    let done = (0..2000).any(|_| {
        w.tick(&[]).unwrap();
        w.state.orders.front(&w.state.units, row).is_none()
    });
    assert!(done, "resumed, the tank rolled out");
}

#[test]
fn only_units_with_work_can_be_paused() {
    let mut w = world();
    let tank = spawn(&mut w, "aster_t1_tank", 500);
    let mason = spawn(&mut w, "aster_t1_engineer", 540);
    pause(&mut w, vec![tank, mason], true);
    let units = &w.state.units;
    assert!(
        !units.paused[units.row(tank).unwrap()],
        "a tank has nothing to pause"
    );
    assert!(units.paused[units.row(mason).unwrap()]);
    // Another player cannot pause them back.
    w.tick(&[PlayerCommand {
        player: 1,
        command: Command::SetPaused {
            units: vec![mason],
            paused: false,
        },
    }])
    .unwrap();
    assert!(w.state.units.paused[w.state.units.row(mason).unwrap()]);
}

#[test]
fn a_paused_shield_or_radar_powers_down_and_stops_its_upkeep() {
    let mut w = world();
    w.tick(&[cmd(Command::DebugFreeBuild {
        player: 0,
        on: false,
    })])
    .unwrap();
    // Stores that outlast the test, so the grid never stalls.
    w.state.players[0].bonus_storage[1] = mc_core::Fx::from_int(50_000);
    w.state.players[0].energy = mc_core::Fx::from_int(50_000);
    let radar = spawn(&mut w, "aster_t1_radar", 500);
    let dome = spawn(&mut w, "aster_t2_shield", 800);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    let (r, d) = (
        w.state.units.row(radar).unwrap(),
        w.state.units.row(dome).unwrap(),
    );
    let bp = w.bp(r);
    // A spot the tower paints with radar but cannot see.
    let far =
        FxVec2::from_ints(500, 512) + FxVec2::new((bp.vision + bp.radar) / 2, mc_core::Fx::ZERO);
    let open = w.state.units.shield_open[d];
    assert!(w.fog.is_detected(far, 1), "the tower paints radar");
    assert!(open > 0, "the dome is up");
    assert!(w.flows[r].wanted[1] > mc_core::Fx::ZERO);
    assert!(w.flows[d].wanted[1] > mc_core::Fx::ZERO);

    pause(&mut w, vec![radar, dome], true);
    assert!(w.state.units.paused[r] && w.state.units.paused[d]);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert!(!w.fog.is_detected(far, 1), "a paused tower paints nothing");
    assert_eq!(w.state.units.shield_open[d], 0, "a paused dome is down");
    assert_eq!(w.flows[r].wanted[1], mc_core::Fx::ZERO, "no upkeep");
    assert_eq!(w.flows[d].wanted[1], mc_core::Fx::ZERO, "no upkeep");
    assert_eq!(w.flows[d].used[1], mc_core::Fx::ZERO, "no upkeep");

    pause(&mut w, vec![radar, dome], false);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert!(w.fog.is_detected(far, 1), "radar is back");
    assert!(w.state.units.shield_open[d] > 0, "the dome is back up");
    assert!(w.flows[d].wanted[1] > mc_core::Fx::ZERO);
}
