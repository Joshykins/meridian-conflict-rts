//! A group of builders given a row of structures builds them one after another,
//! all together, and helpers on a unit being raised let it go once it is done.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "build line".into(),
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Spawns `units` (key, x, y) for player 0 with free building on; their ids.
fn spawn(w: &mut World, units: &[(&str, i32, i32, u16)]) -> Vec<UnitId> {
    let mut setup = vec![cmd(Command::DebugFreeBuild {
        player: 0,
        on: true,
    })];
    for &(key, x, y, build) in units {
        setup.push(cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build,
        }));
    }
    w.tick(&setup).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == 0)
        .map(|r| w.state.units.id(r))
        .collect()
}

/// Lays four `key` lots edge to edge from x = 600 for `builders` and runs until
/// they are idle: every site is built, none is refused, and the group never has
/// two going at once.
fn build_a_row(key: &str, builders: &[(&str, i32, i32, u16)]) {
    let mut w = world();
    let group = spawn(&mut w, builders);
    let bp = w.blueprints.id_of(key).unwrap();
    let lot = w.blueprints.unit(bp).footprint.0 as i32 * mc_map::BUILD_CELL_M;
    let row: Vec<_> = (0..4)
        .map(|i| {
            cmd(Command::Build {
                units: group.clone(),
                blueprint: bp,
                pos: FxVec2::from_ints(600 + i * lot, 516),
                heading: Angle::ZERO,
                queue: i > 0,
            })
        })
        .collect();
    w.tick(&row).unwrap();
    let mut idle = false;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        assert!(
            !w.events
                .iter()
                .any(|e| matches!(e, SimEvent::BuildRejected { .. })),
            "{key}: a builder was told it cannot build on its own row"
        );
        let units = &w.state.units;
        let mut sites: Vec<usize> = group
            .iter()
            .filter_map(|&u| units.row(u))
            .filter_map(|r| units.row(units.build_target[r]))
            .filter(|&s| units.has_flag(s, flag::UNDER_CONSTRUCTION))
            .collect();
        sites.sort_unstable();
        sites.dedup();
        assert!(sites.len() <= 1, "{key}: the group split over two sites");
        idle = group
            .iter()
            .filter_map(|&u| units.row(u))
            .all(|r| w.state.orders.front(units, r).is_none());
        if idle {
            break;
        }
    }
    assert!(idle, "{key}: the row never finished");
    let built = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| {
            w.state.units.blueprint[r] == bp && !w.state.units.has_flag(r, flag::UNDER_CONSTRUCTION)
        })
        .count();
    assert_eq!(built, 4, "{key}: every lot in the row is built");
}

/// Builders walking in from all sides: the first to arrive starts the lot,
/// which is built over the goal the others were walking to. They come on
/// round it rather than give it up for the next lot.
#[test]
fn builders_from_all_sides_build_the_row_together() {
    build_a_row(
        "aster_mass_storage",
        &[
            ("aster_t1_engineer", 500, 400, 1000),
            ("aster_t1_engineer", 700, 620, 1000),
            ("aster_t2_engineer", 800, 516, 1000),
            ("aster_t1_engineer", 600, 560, 1000),
        ],
    );
}

/// A mixed group standing on the row: the quick ones finish lots ahead of the
/// slow ones, who pass them by instead of being refused at each.
#[test]
fn a_mixed_group_on_the_row_builds_it_in_order() {
    build_a_row(
        "aster_t1_land_factory",
        &[
            ("aster_t1_engineer", 610, 516, 1000),
            ("aster_t2_engineer", 640, 516, 1000),
            ("aster_t3_engineer", 670, 516, 1000),
            ("aster_commander", 700, 516, 1000),
        ],
    );
}

/// Engineers told to help raise an experimental stop when it is finished,
/// rather than trail after it for the rest of the match.
#[test]
fn helping_raise_a_unit_ends_when_it_is_done() {
    let mut w = world();
    let ids = spawn(
        &mut w,
        &[
            ("aster_t4_assault_tank", 600, 516, 995),
            ("aster_t1_engineer", 560, 470, 1000),
            ("aster_t1_engineer", 640, 470, 1000),
        ],
    );
    let (tank, helpers) = ids.split_first().unwrap();
    w.tick(&[cmd(Command::Assist {
        units: helpers.to_vec(),
        target: *tank,
        queue: false,
    })])
    .unwrap();
    let mut done_at = None;
    for t in 0..2000 {
        w.tick(&[]).unwrap();
        let row = w.state.units.row(*tank).expect("the tank stands");
        if done_at.is_none() && !w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION) {
            done_at = Some(t);
        }
        if done_at.is_some_and(|d| t > d + 2) {
            break;
        }
    }
    assert!(done_at.is_some(), "the helpers finished the tank");
    for &h in helpers {
        let row = w.state.units.row(h).unwrap();
        assert!(
            w.state.orders.front(&w.state.units, row).is_none(),
            "a helper is still on the finished tank"
        );
    }
}

/// Mixed tiers told to build what only the top tier can: that one starts it, the
/// others walk over, wait by the lot and then help raise it. Without one that can
/// start it the lower tiers take no order, and they give up when it is called off.
#[test]
fn lower_tiers_help_what_only_a_higher_tier_can_start() {
    let mut w = world();
    let ids = spawn(
        &mut w,
        &[
            ("aster_t2_engineer", 820, 516, 1000),
            ("aster_t1_engineer", 580, 470, 1000),
            ("aster_t1_engineer", 620, 470, 1000),
        ],
    );
    let helpers = &ids[1..];
    let power = w.blueprints.id_of("aster_t2_power").unwrap();
    let build = |units: Vec<UnitId>| {
        cmd(Command::Build {
            units,
            blueprint: power,
            pos: FxVec2::from_ints(600, 516),
            heading: Angle::ZERO,
            queue: false,
        })
    };
    let has_order = |w: &World, u: UnitId| {
        let row = w.state.units.row(u).unwrap();
        w.state.orders.front(&w.state.units, row).is_some()
    };

    w.tick(&[build(helpers.to_vec())]).unwrap();
    assert!(
        helpers.iter().all(|&h| !has_order(&w, h)),
        "T1 engineers alone take an order for a T2 structure"
    );

    w.tick(&[build(ids.clone())]).unwrap();
    let mut helped = false;
    let mut done = false;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        let site = units.slots.iter().find(|&r| units.blueprint[r] == power);
        if let Some(site) = site {
            done = !units.has_flag(site, flag::UNDER_CONSTRUCTION);
            helped |= helpers.iter().any(|&h| {
                units
                    .row(h)
                    .is_some_and(|r| units.build_target[r] == units.id(site))
            });
        }
        if done {
            break;
        }
    }
    assert!(helped, "the T1 engineers helped raise it");
    assert!(done, "the T2 power plant was built");

    // Called off before it is started: nobody left to start it, so the helpers go idle.
    let mut w2 = world();
    let ids = spawn(
        &mut w2,
        &[
            ("aster_t2_engineer", 1200, 516, 1000),
            ("aster_t1_engineer", 580, 470, 1000),
        ],
    );
    w2.tick(&[build(ids.clone())]).unwrap();
    for _ in 0..200 {
        w2.tick(&[]).unwrap();
    }
    assert!(
        has_order(&w2, ids[1]),
        "the T1 engineer waits for the T2 one"
    );
    w2.tick(&[cmd(Command::Stop {
        units: vec![ids[0]],
    })])
    .unwrap();
    for _ in 0..5 {
        w2.tick(&[]).unwrap();
    }
    assert!(!has_order(&w2, ids[1]), "the T1 engineer gave up");
}
