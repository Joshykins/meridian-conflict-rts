//! Engineering stations: structures that build, raise and mend round them within their
//! build range, as an engineer does, without moving. They refuse work beyond their reach,
//! take up what turns up in it by themselves when idle, and upgrade in place.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::Refusal;
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
        name: "stations".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(2500, 2500)],
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Spawns `units` (key, x, y, build progress of 1000) for player 0 with free building on.
fn spawn(w: &mut World, units: &[(&str, i32, i32, u16)]) -> Vec<UnitId> {
    let before: Vec<UnitId> = w
        .state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .collect();
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
        .map(|r| w.state.units.id(r))
        .filter(|id| !before.contains(id))
        .collect()
}

fn built(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    let units = &w.state.units;
    units
        .slots
        .iter()
        .filter(|&r| units.blueprint[r] == bp && !units.has_flag(r, flag::UNDER_CONSTRUCTION))
        .count()
}

fn refused_out_of_reach(w: &World) -> bool {
    w.events.iter().any(|e| {
        matches!(
            e,
            SimEvent::CommandRefused {
                reason: Refusal::OutOfReach,
                ..
            }
        )
    })
}

fn build(w: &World, units: Vec<UnitId>, key: &str, x: i32, y: i32) -> PlayerCommand {
    cmd(Command::Build {
        units,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        queue: false,
    })
}

/// A station builds a structure well beyond an engineer's reach without moving, and
/// refuses one beyond its own, saying why.
#[test]
fn a_station_builds_within_its_reach_and_refuses_beyond_it() {
    let mut w = world();
    let station = spawn(&mut w, &[("aster_t1_eng_station", 600, 600, 1000)])[0];
    let row = w.state.units.row(station).unwrap();
    let (at, range) = (
        w.state.units.pos[row],
        w.blueprints
            .unit(w.state.units.blueprint[row])
            .builder
            .as_ref()
            .unwrap()
            .range,
    );
    assert!(range.floor_int() >= 250, "a station reaches far: {range:?}");

    let far = 600 + range.floor_int() + 200;
    w.tick(&[build(&w, vec![station], "aster_t1_power", far, 600)])
        .unwrap();
    assert!(
        refused_out_of_reach(&w),
        "a site beyond its reach is refused"
    );
    assert!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind)
            != Some(mc_sim::tables::OrderKind::Build),
        "and no order is taken"
    );

    let near = 600 + range.floor_int() - 40;
    w.tick(&[build(&w, vec![station], "aster_t1_power", near, 600)])
        .unwrap();
    assert!(!refused_out_of_reach(&w));
    for _ in 0..2000 {
        w.tick(&[]).unwrap();
        if built(&w, "aster_t1_power") == 1 {
            break;
        }
    }
    assert_eq!(built(&w, "aster_t1_power"), 1, "the station raised it");
    assert_eq!(w.state.units.pos[row], at, "without moving");
}

/// Left alone, a station raises a friendly site in its reach and mends a hurt unit
/// there by itself; a site beyond its reach it leaves alone.
#[test]
fn an_idle_station_raises_and_mends_in_its_reach() {
    let mut w = world();
    let ids = spawn(
        &mut w,
        &[
            ("aster_t1_eng_station", 600, 600, 1000),
            ("aster_t1_power", 760, 600, 50),
            ("aster_t1_power", 1400, 600, 50),
            ("aster_t1_tank", 600, 780, 1000),
        ],
    );
    let (near, far, tank) = (ids[1], ids[2], ids[3]);
    let tank_row = w.state.units.row(tank).unwrap();
    let full = w.state.units.health[tank_row];
    w.state.units.health[tank_row] = full / 3;
    let mut raised = false;
    let mut mended = false;
    for _ in 0..4000 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        raised |= units
            .row(near)
            .is_some_and(|r| !units.has_flag(r, flag::UNDER_CONSTRUCTION));
        mended |= units.health[tank_row] >= full;
        if raised && mended {
            break;
        }
    }
    assert!(raised, "the station raised the site in its reach by itself");
    assert!(mended, "the station mended the tank in its reach by itself");
    let units = &w.state.units;
    let far_row = units.row(far).expect("the far site still stands");
    assert!(
        units.has_flag(far_row, flag::UNDER_CONSTRUCTION),
        "the site beyond its reach was left alone"
    );
}

/// Told to help a unit beyond its reach, a station refuses; within it, it helps.
#[test]
fn a_station_assists_only_within_its_reach() {
    let mut w = world();
    let ids = spawn(
        &mut w,
        &[
            ("aster_t1_eng_station", 600, 600, 1000),
            ("aster_t1_land_factory", 1500, 600, 1000),
        ],
    );
    w.tick(&[cmd(Command::Assist {
        units: vec![ids[0]],
        target: ids[1],
        queue: false,
    })])
    .unwrap();
    assert!(refused_out_of_reach(&w), "the factory is out of reach");
}

/// A station upgrades in place: the same unit, at the next tier, with a longer reach
/// and more build power.
#[test]
fn a_station_upgrades_in_place() {
    let mut w = world();
    let ids = spawn(
        &mut w,
        &[
            ("aster_t1_eng_station", 600, 600, 1000),
            // Opens tech 2 for the side.
            ("aster_t2_engineer", 900, 900, 1000),
        ],
    );
    let station = ids[0];
    w.tick(&[cmd(Command::Upgrade {
        units: vec![station],
    })])
    .unwrap();
    let t2 = w.blueprints.id_of("aster_t2_eng_station").unwrap();
    let mut done = false;
    for _ in 0..6000 {
        w.tick(&[]).unwrap();
        let units = &w.state.units;
        if let Some(r) = units.row(station) {
            if units.blueprint[r] == t2 && !units.has_flag(r, flag::UPGRADE) {
                let upgrading = w
                    .state
                    .orders
                    .front(units, r)
                    .is_some_and(|o| o.kind == mc_sim::tables::OrderKind::Upgrade);
                if !upgrading {
                    done = true;
                    break;
                }
            }
        }
    }
    assert!(
        done,
        "the station is now its tech 2 self, under the same id"
    );
}

/// A reactor or fabricator against a station saves it nothing: the station builds out in
/// the field at full price, so it is no consumer and draws no links. A factory is one.
#[test]
fn a_station_takes_no_adjacency_saving() {
    use mc_sim::adjacency::{offers, uses, Resource};
    let bps = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let bp = |key: &str| bps.unit(bps.id_of(key).unwrap());
    for key in ["aster_t2_eng_station", "regency_t3_eng_station"] {
        assert!(!uses(bp(key), Resource::Materials), "{key}");
        assert!(!uses(bp(key), Resource::Energy), "{key}");
        assert!(
            offers(bp("aster_t3_power"), bp(key))
                .iter()
                .all(Option::is_none),
            "{key}"
        );
    }
    assert!(uses(bp("aster_t2_land_factory"), Resource::Materials));
}
