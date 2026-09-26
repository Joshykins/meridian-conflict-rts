//! The Javelin's anti-building role: it picks a building over a nearer tank, and
//! `javelin_probe` prints how much of a volley lands on each target kind.
//! `cargo test -p mc-sim --test javelin_role -- --nocapture`

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
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
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::from_degrees(90),
            count: 1,
            flags,
            build: 1000,
        },
    }
}

/// Damage dealt per missile fired, and missiles fired, over 60 s.
fn probe(target: &str, dist: i32, moving: bool) -> (f64, usize, Option<u32>) {
    let mut w = world();
    let jav = w.blueprints.id_of("aster_t2_missile").unwrap();
    let tgt = w.blueprints.id_of(target).unwrap();
    w.tick(&[
        spawn(&w, 1, "aster_t2_missile", 600 + dist, 1000, flag::INVULNERABLE),
        spawn(&w, 0, target, 600, 1000, flag::PASSIVE),
    ])
    .unwrap();
    let row = (0..w.state.units.owner.len())
        .find(|&r| w.state.units.blueprint[r] == tgt)
        .unwrap();
    let id = w.state.units.id(row);
    let full = w.state.units.health[row].to_f64();
    let mut fired = 0;
    let mut dealt = 0.0;
    let mut last = full;
    let mut killed = None;
    for t in 0..600 {
        let mut cmds = Vec::new();
        if moving && t % 60 == 0 {
            let y = if (t / 60) % 2 == 0 { 1400 } else { 600 };
            cmds.push(PlayerCommand {
                player: 0,
                command: Command::Move { units: vec![id], target: FxVec2::from_ints(600, y), queue: false },
            });
        }
        w.tick(&cmds).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired { blueprint, .. } = e {
                if *blueprint == jav {
                    fired += 1;
                }
            }
        }
        match w.state.units.row(id) {
            Some(r) => {
                let h = w.state.units.health[r].to_f64();
                dealt += (last - h).max(0.0);
                last = h;
            }
            None => {
                dealt += last;
                killed = Some(t);
                break;
            }
        }
    }
    (dealt / fired.max(1) as f64, fired, killed)
}

#[test]
fn javelin_probe() {
    for (target, moving) in [
        ("aster_t2_land_factory", false),
        ("aster_t2_tank", false),
        ("aster_t2_tank", true),
        ("aster_t1_tank", true),
    ] {
        for dist in [250, 400, 600] {
            let (per, fired, killed) = probe(target, dist, moving);
            let kill = killed.map_or("alive".to_string(), |t| format!("dead {:.1}s", t as f64 / 10.0));
            println!(
                "{target:24} moving={moving:5} {dist}m: {per:6.1} dmg/missile of 174 ({:4.0}%), {fired} fired, {kill}",
                per / 174.0 * 100.0
            );
        }
    }
}

#[test]
fn javelin_picks_the_building_over_a_nearer_tank() {
    let mut w = world();
    let factory = w.blueprints.id_of("aster_t2_land_factory").unwrap();
    let tank = w.blueprints.id_of("aster_t2_tank").unwrap();
    w.tick(&[
        spawn(&w, 1, "aster_t2_missile", 1100, 1000, flag::INVULNERABLE),
        spawn(&w, 0, "aster_t2_tank", 850, 1000, flag::PASSIVE | flag::INVULNERABLE),
        spawn(&w, 0, "aster_t2_land_factory", 600, 1000, flag::PASSIVE),
    ])
    .unwrap();
    let find = |w: &World, bp| (0..w.state.units.owner.len()).find(|&r| w.state.units.blueprint[r] == bp).unwrap();
    let (f, t) = (find(&w, factory), find(&w, tank));
    let (f0, t0) = (w.state.units.health[f], w.state.units.health[t]);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.units.health[f] < f0, "the Javelin never hit the factory");
    assert_eq!(w.state.units.health[t], t0, "the Javelin shot the tank");
}
