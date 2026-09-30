//! Aircraft given a string of queued moves fly through each waypoint onto the
//! next leg, holding their speed, and stop only at the last one.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "air waypoints".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(256, 256), FxVec2::from_ints(1800, 1800)],
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
        seed: 11,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

struct Route {
    /// Slowest airspeed on the legs before the last, once up to speed, as a share of cruise.
    slowest: f32,
    /// Whether every aircraft came to rest at the end of the route.
    parked: bool,
}

/// Sends `count` of `key` along a zig-zag of queued moves (one right-click
/// and three shift-right-clicks) and measures how they fly it.
fn fly(key: &str, count: i32) -> Route {
    let mut w = world();
    let planes: Vec<UnitId> = (0..count)
        .map(|i| {
            let id = w.blueprints.id_of(key).unwrap();
            let row = w
                .spawn_unit(
                    id,
                    0,
                    FxVec2::from_ints(400 + i * 24, 400),
                    Angle::ZERO,
                    true,
                )
                .unwrap();
            w.state.units.id(row)
        })
        .collect();
    let points = [(1100, 500), (1400, 1100), (800, 1400), (1400, 1700)];
    let cmds: Vec<PlayerCommand> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| PlayerCommand {
            player: 0,
            command: Command::Move {
                units: planes.clone(),
                target: FxVec2::from_ints(x, y),
                queue: i > 0,
            },
        })
        .collect();
    w.tick(&cmds).unwrap();
    let rows: Vec<usize> = planes
        .iter()
        .map(|&id| w.state.units.row(id).unwrap())
        .collect();
    let cruise = w.bp(rows[0]).motion.unwrap().speed.to_f32();
    let mut up_to_speed = vec![false; rows.len()];
    let mut slowest = f32::MAX;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        for (i, &row) in rows.iter().enumerate() {
            let speed = w.state.units.speed[row].to_f32() / cruise;
            up_to_speed[i] |= speed >= 0.95;
            let legs = w.state.orders.iter(&w.state.units, row).count();
            if up_to_speed[i] && legs >= 2 {
                slowest = slowest.min(speed);
            }
        }
    }
    let parked = rows.iter().all(|&row| {
        w.state.orders.front(&w.state.units, row).is_none() && w.state.units.speed[row] <= Fx::ONE
    });
    Route { slowest, parked }
}

#[test]
fn aircraft_fly_through_queued_waypoints() {
    // A hovering airframe bleeds speed as its way swings round a sharp corner
    // (90 and 127 degrees here), then picks it up again: it never stops.
    for (key, n, floor) in [
        ("aster_t1_interceptor", 1, 0.9),
        ("aster_t1_bomber", 1, 0.9),
        ("aster_t1_rotor_gunship", 1, 0.5),
        ("aster_t1_interceptor", 5, 0.45),
        ("aster_t1_bomber", 3, 0.45),
        ("aster_t2_gunship", 4, 0.35),
    ] {
        let r = fly(key, n);
        assert!(
            r.slowest >= floor,
            "{key} x{n} slowed to {:.2} of cruise at a waypoint",
            r.slowest
        );
        assert!(r.parked, "{key} x{n} did not stop at the last waypoint");
    }
}
