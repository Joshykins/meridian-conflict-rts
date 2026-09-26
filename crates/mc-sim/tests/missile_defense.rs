//! The Corona, tech 2 tactical missile defence: its lasers burn Javelin rockets out
//! of the air before they reach what it guards.

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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // The range fights with the economy off; the lasers still ask for power.
    w.state.players[0].free_build = true;
    w
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            // Everything faces west, toward the base the Javelin shoots at.
            heading: Angle::from_degrees(180),
            count: 1,
            flags,
            build: 1000,
        },
    }
}

fn spawn_at(spawns: &mut Vec<PlayerCommand>, w: &World, x: i32, y: i32) {
    spawns.push(spawn(w, 1, "aster_t2_missile", x, y, flag::INVULNERABLE));
}

/// Javelin rockets burnt out of the air and rockets that got through over `ticks`,
/// `javelins` firing, `coronas` guarding an invulnerable Bastion 560 m from the launcher.
fn barrage(javelins: usize, coronas: usize, ticks: u32) -> (usize, usize) {
    let mut w = world();
    let javelin = w.blueprints.id_of("aster_t2_missile").unwrap();
    let mut spawns = vec![
        spawn(&w, 0, "aster_t2_point_defense", 500, 512, flag::INVULNERABLE | flag::PASSIVE),
    ];
    for i in 0..javelins as i32 {
        spawn_at(&mut spawns, &w, 1060 + 20 * (i / 3), 472 + 40 * (i % 3));
    }
    for i in 0..coronas {
        let y = 512 + if i % 2 == 0 { 24 } else { -24 };
        let x = 540 - 24 * (i as i32 / 2);
        spawns.push(spawn(&w, 0, "aster_t2_missile_defense", x, y, flag::INVULNERABLE));
    }
    w.tick(&spawns).unwrap();
    let (mut fired, mut killed) = (0, 0);
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, .. } if *blueprint == javelin => fired += 1,
                SimEvent::MissileLased { killed: true, .. } => killed += 1,
                _ => {}
            }
        }
    }
    let flying = w.state.projectiles.len();
    (killed, fired - killed - flying)
}

#[test]
fn corona_burns_javelin_rockets_out_of_the_air() {
    // Four salvos (reload 8 s).
    let ticks = 340;
    let none = barrage(1, 0, ticks);
    let one = barrage(1, 1, ticks);
    let two = barrage(1, 2, ticks);
    println!("javelin rockets (killed, through): 0 Coronas {none:?}, 1 {one:?}, 2 {two:?}");
    assert_eq!(none.0, 0);
    assert!(none.1 >= 18, "the Javelin landed only {} rockets", none.1);
    // One Corona stops most of a salvo; a pair stops nearly all of it.
    assert!(one.0 >= one.1 * 2, "one Corona let {} of {} through", one.1, one.0 + one.1);
    assert!(two.1 <= one.1, "two Coronas let more through than one");
}

#[test]
#[ignore]
/// Rockets killed and let through as launchers and Coronas are added.
fn zz_probe_saturation() {
    for j in 1..=6 {
        let row: Vec<_> = (0..=3).map(|c| barrage(j, c, 340)).collect();
        println!("{j} javelins: (killed, through) by 0..3 Coronas {row:?}");
    }
}
