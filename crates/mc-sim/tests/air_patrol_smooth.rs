//! Aircraft on patrol fly their loop without stalling at posts or weaving
//! through corners: speed stays up, and the turn builds and eases once per corner.

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
        name: "air patrol".into(),
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

fn add(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

struct Flight {
    /// Slowest airspeed once under way, as a share of cruise.
    slowest: f32,
    /// Largest change of yaw rate from one tick to the next, degrees per second per tick.
    jerk: f32,
    /// Hard turns against the one just flown, per aircraft per leg: weaving.
    weaves: f32,
    legs: u32,
}

/// Flies `count` of `key` round a square patrol and measures how they fly it.
fn fly(key: &str, count: i32) -> Flight {
    let mut w = world();
    let planes: Vec<UnitId> = (0..count)
        .map(|i| add(&mut w, key, 500 + i * 24, 500))
        .collect();
    let points = vec![
        FxVec2::from_ints(1300, 500),
        FxVec2::from_ints(1300, 1300),
        FxVec2::from_ints(500, 1300),
        FxVec2::from_ints(500, 500),
    ];
    let cmd = Command::Patrol {
        units: planes.clone(),
        points,
        queue: false,
    };
    w.tick(&[PlayerCommand {
        player: 0,
        command: cmd,
    }])
    .unwrap();
    let cruise = w
        .bp(w.state.units.row(planes[0]).unwrap())
        .motion
        .unwrap()
        .speed
        .to_f32();
    let rows: Vec<usize> = planes
        .iter()
        .map(|&id| w.state.units.row(id).unwrap())
        .collect();
    let mut out = Flight {
        slowest: f32::MAX,
        jerk: 0.0,
        weaves: 0.0,
        legs: 0,
    };
    let mut heading: Vec<Angle> = rows.iter().map(|&r| w.state.units.heading[r]).collect();
    let mut rate = vec![0.0f32; rows.len()];
    let mut side = vec![0i32; rows.len()];
    let mut weaves = 0;
    let mut front = None;
    for t in 0..2400 {
        w.tick(&[]).unwrap();
        let now = w.state.orders.front(&w.state.units, rows[0]).map(|o| o.pos);
        if t > 300 && now != front {
            out.legs += 1;
        }
        front = now;
        for (i, &row) in rows.iter().enumerate() {
            let h = w.state.units.heading[row];
            // Degrees per second.
            let r = heading[i].delta_to(h) as f32 * 3600.0 / 65536.0;
            heading[i] = h;
            if t > 300 {
                out.slowest = out.slowest.min(w.state.units.speed[row].to_f32() / cruise);
                out.jerk = out.jerk.max((r - rate[i]).abs());
                // The small settle as a wing rolls out onto its station is not a weave.
                let s = if r > 15.0 {
                    1
                } else if r < -15.0 {
                    -1
                } else {
                    0
                };
                if s != 0 {
                    weaves += (side[i] != 0 && s != side[i]) as i32;
                    side[i] = s;
                }
            }
            rate[i] = r;
        }
    }
    out.weaves = weaves as f32 / rows.len() as f32 / out.legs.max(1) as f32;
    out
}

#[test]
fn a_lone_aircraft_holds_its_speed_round_the_loop() {
    for key in [
        "aster_t1_interceptor",
        "aster_t1_bomber",
        "aster_t3_strategic_bomber",
    ] {
        let f = fly(key, 1);
        assert!(f.legs >= 8, "{key} flew {} legs", f.legs);
        assert!(
            f.slowest >= 0.95,
            "{key} slowed to {:.2} of cruise",
            f.slowest
        );
        assert!(
            f.jerk <= 15.0,
            "{key} snapped its turn by {:.1} deg/s in a tick",
            f.jerk
        );
        assert_eq!(f.weaves, 0.0, "{key} weaved");
    }
}

#[test]
fn a_flight_sweeps_its_corners_without_stalling_or_weaving() {
    for (key, n) in [
        ("aster_t1_interceptor", 5),
        ("aster_t1_bomber", 3),
        ("aster_t2_interceptor", 4),
    ] {
        let f = fly(key, n);
        assert!(f.legs >= 8, "{key} x{n} flew {} legs", f.legs);
        assert!(
            f.slowest >= 0.45,
            "{key} x{n} slowed to {:.2} of cruise",
            f.slowest
        );
        assert!(
            f.jerk <= 15.0,
            "{key} x{n} snapped its turn by {:.1} deg/s in a tick",
            f.jerk
        );
        assert!(
            f.weaves <= 0.1,
            "{key} x{n} weaved {:.2} times a leg",
            f.weaves
        );
    }
}
