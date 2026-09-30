//! Area reclaim: sent to a point, a reclaimer clears the wrecks it passes; given a
//! circle, it clears every wreck inside it and leaves those outside.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const ENGINEER: &str = "aster_t1_engineer";
const TANK: &str = "aster_t1_tank";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "reclaim_area".into(),
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
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Somewhere to put the mass: plenty of it.
    w.tick(&[cmd(Command::DebugStorage {
        player: 0,
        mass: 100_000,
        energy: 0,
    })])
    .unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn engineers(w: &mut World, at: &[(i32, i32)]) -> Vec<UnitId> {
    let blueprint = w.blueprints.id_of(ENGINEER).unwrap();
    let spawns: Vec<PlayerCommand> = at
        .iter()
        .map(|&(x, y)| {
            cmd(Command::DebugSpawn {
                owner: 0,
                blueprint,
                pos: FxVec2::from_ints(x, y),
                heading: Angle::ZERO,
                count: 1,
                flags: 0,
                build: 1000,
            })
        })
        .collect();
    w.tick(&spawns).unwrap();
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == 0 && u.blueprint[r] == blueprint)
        .map(|r| u.id(r))
        .collect()
}

/// `count` tank wrecks in a loose block about `(x, y)`.
fn wrecks(w: &mut World, x: i32, y: i32, count: u16) {
    let blueprint = w.blueprints.id_of(TANK).unwrap();
    w.tick(&[cmd(Command::DebugWrecks {
        blueprint,
        pos: FxVec2::from_ints(x, y),
        count,
    })])
    .unwrap();
}

/// Where the live wrecks lie.
fn wrecks_left(w: &World) -> Vec<FxVec2> {
    let wr = &w.state.wrecks;
    wr.slots.iter().map(|r| wr.pos[r]).collect()
}

/// Ticks until none of `units` has an order left. Panics when that never happens.
fn run_until_idle(w: &mut World, units: &[UnitId], most: u32) {
    for _ in 0..most {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        if units.iter().all(|&id| {
            u.row(id)
                .is_none_or(|r| w.state.orders.front(u, r).is_none())
        }) {
            return;
        }
    }
    panic!("the reclaim order was still going after {most} ticks");
}

#[test]
fn a_point_order_reclaims_the_wrecks_along_the_way_and_stops_there() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 512)]);
    // One just off the way, one well away from it.
    wrecks(&mut w, 600, 530, 1);
    wrecks(&mut w, 600, 850, 1);
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: FxVec2::from_ints(900, 512),
        radius: Fx::ZERO,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 4000);

    let left = wrecks_left(&w);
    assert_eq!(left.len(), 1, "only the far wreck is left: {left:?}");
    assert!(left[0].y > Fx::from_int(800));
    let row = w.state.units.row(units[0]).unwrap();
    let end = w.state.units.pos[row];
    assert!(
        end.distance(FxVec2::from_ints(900, 512)) < Fx::from_int(20),
        "it went on to the point: {end:?}"
    );
}

#[test]
fn a_circle_is_cleared_and_what_lies_outside_it_is_left() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 500), (300, 540), (300, 580)]);
    let centre = FxVec2::from_ints(900, 700);
    let radius = Fx::from_int(120);
    wrecks(&mut w, 900, 700, 9);
    wrecks(&mut w, 900, 1000, 1);
    let inside = wrecks_left(&w)
        .iter()
        .filter(|p| p.distance(centre) <= radius)
        .count();
    assert_eq!(inside, 9, "the block lies inside the circle");
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: centre,
        radius,
        queue: false,
    })])
    .unwrap();
    run_until_idle(&mut w, &units, 6000);

    let left = wrecks_left(&w);
    assert_eq!(left.len(), 1, "every wreck in the circle is gone: {left:?}");
    assert!(left[0].distance(centre) > radius);
}

#[test]
fn a_queued_area_reclaim_waits_its_turn() {
    let mut w = world();
    let units = engineers(&mut w, &[(300, 512)]);
    wrecks(&mut w, 500, 900, 1);
    w.tick(&[cmd(Command::Move {
        units: units.clone(),
        target: FxVec2::from_ints(700, 512),
        queue: false,
    })])
    .unwrap();
    w.tick(&[cmd(Command::ReclaimArea {
        units: units.clone(),
        pos: FxVec2::from_ints(500, 900),
        radius: Fx::from_int(60),
        queue: true,
    })])
    .unwrap();
    let row = w.state.units.row(units[0]).unwrap();
    let kinds: Vec<_> = w
        .state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            mc_sim::tables::OrderKind::Move,
            mc_sim::tables::OrderKind::ReclaimArea
        ]
    );
    run_until_idle(&mut w, &units, 4000);
    assert!(wrecks_left(&w).is_empty());
}
