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

fn spawn_for(w: &World, owner: u8, key: &str, x: i32, y: i32) -> PlayerCommand {
    let mut c = spawn_at(w, key, x, y);
    if let Command::DebugSpawn { owner: o, .. } = &mut c.command {
        *o = owner;
    }
    c
}

fn refused_for_reach(w: &World) -> bool {
    w.events.iter().any(|e| {
        matches!(
            e,
            mc_sim::SimEvent::CommandRefused {
                player: 0,
                reason: mc_sim::Refusal::MineReach,
            }
        )
    })
}

#[test]
fn core_mines_keep_out_of_each_others_reach() {
    let mut w = world();
    let mine = w
        .blueprints
        .unit(w.blueprints.id_of("aster_core_mine").unwrap())
        .clone();
    let reach = mine.mine.unwrap().reach.floor_int();
    w.tick(&[spawn_at(&w, "aster_core_mine", 522, 522)])
        .unwrap();
    assert_eq!(count(&w, "aster_core_mine"), 1);
    let site = |x: i32, y: i32| mc_sim::snap_to_build_grid(&mine, FxVec2::from_ints(x, y));
    let lot = mine.footprint.0 as i32 * mc_map::BUILD_CELL_M;
    assert!(!w.can_place(&mine, site(522 + lot + 6, 522)), "beside it");
    assert!(
        !w.can_place(&mine, site(522 + reach - 20, 522)),
        "just inside its reach"
    );
    assert!(
        w.can_place(&mine, site(522 + reach + 20, 522)),
        "just outside"
    );
    // Anyone's mine keeps the ground: an enemy's as much as the side's own.
    w.tick(&[spawn_for(&w, 1, "aster_core_mine", 522, 1580)])
        .unwrap();
    assert!(
        !w.can_place(&mine, site(1000, 1580)),
        "inside the enemy's reach"
    );
    // Other structures may stand in a mine's reach as ever.
    let pgen = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_power").unwrap())
        .clone();
    assert!(w.can_place(&pgen, FxVec2::from_ints(612, 612)));
}

#[test]
fn a_mine_ordered_into_its_own_sides_reach_is_refused_and_the_player_told() {
    let mut w = world();
    w.tick(&[
        spawn_at(&w, "aster_core_mine", 522, 522),
        spawn_at(&w, "aster_t1_engineer", 700, 700),
    ])
    .unwrap();
    let mine = w.blueprints.id_of("aster_core_mine").unwrap();
    let build = |w: &World, x: i32, y: i32| {
        cmd(Command::Build {
            units: engineer_ids(w),
            blueprint: mine,
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            queue: false,
        })
    };
    w.tick(&[build(&w, 900, 522)]).unwrap();
    assert!(refused_for_reach(&w), "the player is told why");
    let row = w.state.units.row(engineer_ids(&w)[0]).unwrap();
    assert!(
        w.state.orders.front(&w.state.units, row).is_none(),
        "no order was given"
    );
}

#[test]
fn an_enemy_mine_turns_a_builder_away_only_when_it_gets_there() {
    let mut w = world();
    w.tick(&[
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
        spawn_for(&w, 1, "aster_core_mine", 522, 522),
        spawn_at(&w, "aster_t1_engineer", 880, 540),
    ])
    .unwrap();
    let mine = w.blueprints.id_of("aster_core_mine").unwrap();
    w.tick(&[cmd(Command::Build {
        units: engineer_ids(&w),
        blueprint: mine,
        pos: FxVec2::from_ints(900, 522),
        heading: Angle::ZERO,
        queue: false,
    })])
    .unwrap();
    // The order says nothing of a mine the side may not have seen.
    assert!(!refused_for_reach(&w));
    let mut told = false;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
        told |= refused_for_reach(&w);
        if told {
            break;
        }
    }
    assert!(told, "the builder arrived and the player was told why");
    assert_eq!(count(&w, "aster_core_mine"), 1, "no second mine begun");
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
fn packed_storage_leaves_a_lane() {
    for key in [
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
