//! Structure lots snap to 12 m. Pathing blocks only the hull, so the rest of
//! the lot is apron units walk on; neighbouring lots still sit edge to edge,
//! with a lane between them.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "place".into(),
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

fn spawn_at(w: &World, key: &str, x: i32, y: i32) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, y),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })
}

fn count(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .count()
}

fn id_at(w: &World, key: &str, x: i32, y: i32) -> mc_sim::UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == bp && w.state.units.pos[r] == at)
        .expect("structure at site");
    w.state.units.id(row)
}

#[test]
fn a_unit_can_squeeze_along_a_power_generator() {
    let mut w = world();
    w.tick(&[spawn_at(&w, "aster_t1_power", 528, 528)]).unwrap();
    // 2x2 lot is 516..540. Only the hull blocks, so the apron is walkable.
    let land = mc_data::MoveLayer::Land;
    assert!(w.nav.passable(land, 0, FxVec2::from_ints(514, 528)));
    assert!(w.nav.passable(land, 0, FxVec2::from_ints(518, 528)));
    assert!(!w.nav.passable(land, 0, FxVec2::from_ints(528, 528)));
}

#[test]
fn two_power_generators_sit_edge_to_edge() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 552, 528),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 2);
}

#[test]
fn a_second_power_generator_cannot_overlap_the_first() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 540, 528),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
}

#[test]
fn a_wall_can_touch_a_power_generator() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_wall", 546, 534),
    ])
    .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
    assert_eq!(count(&w, "aster_wall"), 1);
}

#[test]
fn removing_one_neighbour_frees_its_lot() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_t1_power", 528, 528),
        spawn_at(&w, "aster_t1_power", 552, 528),
    ])
    .unwrap();
    let gone = id_at(&w, "aster_t1_power", 528, 528);
    w.tick(&[cmd(Command::DebugRemove { units: vec![gone] })])
        .unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 1);
    w.tick(&[spawn_at(&w, "aster_t1_power", 528, 528)]).unwrap();
    assert_eq!(count(&w, "aster_t1_power"), 2);
    w.tick(&[spawn_at(&w, "aster_t1_power", 552, 528)]).unwrap();
    assert_eq!(
        count(&w, "aster_t1_power"),
        2,
        "the remaining generator still occupies its lot"
    );
}

#[test]
fn core_mines_may_stand_close_but_not_overlap() {
    let mut w = world();
    let mine = w
        .blueprints
        .unit(w.blueprints.id_of("aster_core_mine").unwrap())
        .clone();
    let lot = mine.footprint.0 as i32 * mc_map::BUILD_CELL_M;
    w.tick(&[spawn_at(&w, "aster_core_mine", 522, 522)])
        .unwrap();
    assert_eq!(count(&w, "aster_core_mine"), 1);
    let on_top = mc_sim::snap_to_build_grid(&mine, FxVec2::from_ints(522 + lot / 2, 522));
    let beside = mc_sim::snap_to_build_grid(&mine, FxVec2::from_ints(522 + lot + 6, 522));
    assert!(!w.can_place(&mine, on_top));
    assert!(w.can_place(&mine, beside));
}

fn engineer_ids(w: &World) -> Vec<mc_sim::UnitId> {
    let bp = w.blueprints.id_of("aster_t1_engineer").unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .map(|r| w.state.units.id(r))
        .collect()
}

fn printing_on(w: &World, site: usize) -> usize {
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| {
            w.state.units.flags[r] & flag::BUILDING != 0
                && w.state.units.row(w.state.units.build_target[r]) == Some(site)
        })
        .count()
}

/// Several engineers given one structure all print on it, including the ones
/// that arrive the same tick the first of them starts it.
#[test]
fn a_group_of_engineers_all_join_the_same_site() {
    let mut w = world();
    w.tick(&[
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of("aster_t1_engineer").unwrap(),
            pos: FxVec2::from_ints(540, 516),
            heading: Angle::ZERO,
            count: 3,
            flags: 0,
            build: 1000,
        }),
    ])
    .unwrap();
    let masons = engineer_ids(&w);
    assert_eq!(masons.len(), 3);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[cmd(Command::Build {
        units: masons,
        blueprint: power,
        pos: FxVec2::from_ints(600, 516),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();

    let mut printers = 0;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        printers = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == power)
            .map_or(0, |s| printing_on(&w, s));
        if printers == 3 {
            break;
        }
    }
    assert_eq!(count(&w, "aster_t1_power"), 1, "one reactor, not three");
    assert_eq!(printers, 3, "every engineer is printing on the reactor");
}

/// Engineers standing on the lot they were told to build still start it:
/// they are helpers, not obstacles.
#[test]
fn engineers_on_the_lot_still_start_the_build() {
    let mut w = world();
    w.tick(&[
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of("aster_t1_engineer").unwrap(),
            pos: FxVec2::from_ints(600, 516),
            heading: Angle::ZERO,
            count: 3,
            flags: 0,
            build: 1000,
        }),
    ])
    .unwrap();
    let masons = engineer_ids(&w);
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    w.tick(&[cmd(Command::Build {
        units: masons,
        blueprint: power,
        pos: FxVec2::from_ints(600, 516),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();

    let mut printers = 0;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        printers = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.blueprint[r] == power)
            .map_or(0, |s| printing_on(&w, s));
        if printers == 3 {
            break;
        }
    }
    assert_eq!(count(&w, "aster_t1_power"), 1);
    assert_eq!(printers, 3, "the group standing on the lot all print");
}

/// A row of `key` packed lot to lot from `x0`: every seam between two lots
/// has a lane of 8 m path cells a small unit walks the whole depth of.
fn packed_row_has_lanes(key: &str, x0: i32, y: i32) {
    let mut w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(key).unwrap()).clone();
    let lot = bp.footprint.0 as i32 * mc_map::BUILD_CELL_M;
    let spawns: Vec<_> = (0..3)
        .map(|i| spawn_at(&w, key, x0 + lot / 2 + i * lot, y))
        .collect();
    w.tick(&spawns).unwrap();
    assert_eq!(count(&w, key), 3, "{key} packs edge to edge");
    let land = mc_data::MoveLayer::Land;
    let cell = mc_map::CELL_SIZE_M;
    for i in 0..3 {
        let centre = FxVec2::from_ints(x0 + lot / 2 + i * lot, y);
        assert!(!w.nav.passable(land, 0, centre), "{key}: the hull blocks");
    }
    for seam in [x0 + lot, x0 + 2 * lot] {
        let depth = (y - lot / 2 + 1..y + lot / 2).step_by(4);
        let lane = (seam / cell - 1..=seam / cell).any(|cx| {
            depth.clone().all(|py| {
                w.nav
                    .passable(land, 0, FxVec2::from_ints(cx * cell + cell / 2, py))
            })
        });
        assert!(lane, "{key}: no lane at the seam x={seam} (row from {x0})");
    }
}

#[test]
fn packed_generators_leave_a_lane() {
    // Lots start on both 8 m alignments the 12 m grid lands on.
    packed_row_has_lanes("aster_t1_power", 528, 600);
    packed_row_has_lanes("aster_t1_power", 540, 600);
    packed_row_has_lanes("aster_t2_power", 528, 600);
    packed_row_has_lanes("aster_t2_power", 540, 600);
}

#[test]
fn packed_mines_and_storage_leave_a_lane() {
    for key in [
        "aster_core_mine",
        "aster_mass_storage",
        "aster_energy_storage",
        "aster_t2_shield",
    ] {
        packed_row_has_lanes(key, 528, 606);
        packed_row_has_lanes(key, 540, 606);
    }
}

#[test]
fn the_apron_cannot_be_built_on() {
    let mut w = world();
    w.tick(&[spawn_at(&w, "aster_t1_power", 528, 528)]).unwrap();
    let pgen = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_power").unwrap())
        .clone();
    let wall = w
        .blueprints
        .unit(w.blueprints.id_of("aster_wall").unwrap())
        .clone();
    // The 1x1 cell in the lot's corner is walkable apron, not a free lot.
    let corner = mc_sim::snap_to_build_grid(&wall, FxVec2::from_ints(520, 520));
    assert!(w
        .nav
        .passable(mc_data::MoveLayer::Land, 0, FxVec2::from_ints(518, 518)));
    assert!(!w.can_place(&wall, corner));
    assert!(!w.can_place(&pgen, FxVec2::from_ints(540, 528)));
}
