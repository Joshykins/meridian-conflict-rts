//! The Strider, the Regency assault tripod: its two cannons take turns, every charge that
//! is seen is fired, and its seekers leave from the cells on its head.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const STRIDER: &str = "regency_t4_strider";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "strider".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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
        seed: 3,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 512),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

/// What the cannons did: every (tick, weapon) they began a charge on and fired on, and
/// the last tick logged.
struct Log {
    charged: Vec<(u32, u8)>,
    fired: Vec<(u32, u8)>,
    end: u32,
}

/// What the Strider's cannons did over
/// `ticks` after a target `gap` metres off appears (after it sat idle a while).
fn cannon_log(gap: i32, ticks: u32) -> Log {
    let mut w = world();
    let strider = w.blueprints.id_of(STRIDER).unwrap();
    w.tick(&[spawn(&w, 0, STRIDER, 500, 0)]).unwrap();
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    w.tick(&[spawn(
        &w,
        1,
        "aster_t3_assault_bot",
        500 + gap,
        flag::PASSIVE | flag::INVULNERABLE,
    )])
    .unwrap();
    let (mut charged, mut fired) = (Vec::new(), Vec::new());
    for k in 0..=ticks {
        if k > 0 {
            w.tick(&[]).unwrap();
        }
        let tick = w.tick_count();
        for e in &w.events {
            match e {
                SimEvent::WeaponCharging {
                    blueprint, weapon, ..
                } if *blueprint == strider && *weapon < 2 => charged.push((tick, *weapon)),
                SimEvent::ShotFired {
                    blueprint, weapon, ..
                } if *blueprint == strider && *weapon < 2 => fired.push((tick, *weapon)),
                _ => {}
            }
        }
    }
    Log {
        charged,
        fired,
        end: w.tick_count(),
    }
}

/// Out of idle both cannons are seen charging, and each charge ends in that cannon's own
/// shot a charge later: the second does not sit a charged ball out while the first fires.
/// The two never fire on one tick, and both keep firing.
#[test]
fn every_charge_is_fired() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(STRIDER).unwrap());
    let charge = bp.weapons[0].charge_ticks as u32;
    assert!(charge > 0, "the cannons charge");
    let Log {
        charged,
        fired,
        end,
    } = cannon_log(250, 400);
    for gun in 0..2u8 {
        let starts: Vec<u32> = charged.iter().filter(|c| c.1 == gun).map(|c| c.0).collect();
        let shots: Vec<u32> = fired.iter().filter(|f| f.1 == gun).map(|f| f.0).collect();
        assert!(shots.len() >= 3, "cannon {gun} fired {shots:?}");
        for shot in &shots {
            assert!(
                starts.contains(&(shot - charge)),
                "cannon {gun} fired at {shot} without a charge {charge} ticks before: \
                 charges {charged:?}, shots {fired:?}"
            );
        }
        for start in &starts {
            // The last charge may still be under way when the log ends.
            if start + charge <= end {
                assert!(
                    shots.contains(&(start + charge)),
                    "cannon {gun}'s charge at {start} never fired: charges {charged:?}, \
                     shots {fired:?}"
                );
            }
        }
    }
    for pair in fired.windows(2) {
        assert_ne!(pair[0].0, pair[1].0, "both fired on one tick: {fired:?}");
    }
}

/// The seekers leave from the cells on the back of the head wherever the head has turned:
/// the launcher rides it (a `mount` vertical launcher, `combat.rs`).
#[test]
fn seekers_leave_from_the_heads_cells() {
    let mut w = world();
    let id = w.blueprints.id_of(STRIDER).unwrap();
    let cells = w.blueprints.unit(id).weapons[2].muzzles.clone();
    assert_eq!(cells.len(), 8);
    w.tick(&[spawn(&w, 0, STRIDER, 500, 0)]).unwrap();
    let row = w
        .state
        .units
        .blueprint
        .iter()
        .position(|b| *b == id)
        .unwrap();
    // A mark off the left flank: the head turns a quarter round onto it.
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of("aster_t3_assault_bot").unwrap(),
            pos: FxVec2::from_ints(500, 1100),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE | flag::INVULNERABLE,
            build: 1000,
        },
    }])
    .unwrap();
    let mut turned = 0;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        let torso = u.heading[row] + u.weapon_yaw[row][0];
        for e in &w.events {
            let SimEvent::ShotFired {
                pos,
                blueprint,
                weapon: 2,
                ..
            } = e
            else {
                continue;
            };
            assert_eq!(*blueprint, id);
            let near = cells
                .iter()
                .map(|c| (u.pos[row] + c.xy().rotate(torso)).distance(pos.xy()))
                .min()
                .unwrap();
            assert!(
                near < Fx::from_int(2),
                "a seeker left {near:?} m from the nearest cell"
            );
            if u.heading[row].delta_to(torso).unsigned_abs() > Angle::from_degrees(45).0 {
                turned += 1;
            }
        }
    }
    assert!(turned >= 8, "{turned} seekers left with the head turned");
}

/// No anti-air: nothing on it shoots at aircraft.
#[test]
fn it_has_no_anti_air() {
    let w = world();
    let bp = w.blueprints.unit(w.blueprints.id_of(STRIDER).unwrap());
    assert!(bp.weapons.iter().all(|w| w.target_mask & cat::AIR == 0));
}
