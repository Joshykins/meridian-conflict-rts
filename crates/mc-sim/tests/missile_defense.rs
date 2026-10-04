//! The Corona, tech 2 tactical missile defence: its lasers burn Javelin rockets and
//! Ballista shells out of the air before they reach what it guards.

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

/// Rounds burnt out of the air and rounds that got through over `ticks`, `launchers`
/// of `key` firing from `x`, `coronas` guarding an invulnerable Redoubt at x = 500.
fn barrage(key: &str, x: i32, launchers: usize, coronas: usize, ticks: u32) -> (usize, usize) {
    barrage_on(CORONA, key, x, launchers, coronas, ticks)
}

/// `barrage` against Coronas of blueprint `corona`.
fn barrage_on(
    corona: &str,
    key: &str,
    x: i32,
    launchers: usize,
    coronas: usize,
    ticks: u32,
) -> (usize, usize) {
    let mut w = world();
    let launcher = w.blueprints.id_of(key).unwrap();
    let mut spawns = vec![spawn(
        &w,
        0,
        "aster_t2_point_defense",
        500,
        512,
        flag::INVULNERABLE | flag::PASSIVE,
    )];
    for i in 0..launchers as i32 {
        let (x, y) = (x + 20 * (i / 3), 472 + 40 * (i % 3));
        spawns.push(spawn(&w, 1, key, x, y, flag::INVULNERABLE));
    }
    for i in 0..coronas {
        let y = 512 + if i % 2 == 0 { 24 } else { -24 };
        let x = 540 - 24 * (i as i32 / 2);
        spawns.push(spawn(&w, 0, corona, x, y, flag::INVULNERABLE));
    }
    w.tick(&spawns).unwrap();
    let (mut fired, mut killed) = (0, 0);
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, .. } if *blueprint == launcher => fired += 1,
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
    // About 11 rockets, one every 3 s.
    let ticks = 340;
    let none = barrage(JAVELIN, 1060, 1, 0, ticks);
    let one = barrage(JAVELIN, 1060, 1, 1, ticks);
    let two = barrage(JAVELIN, 1060, 1, 2, ticks);
    println!("javelin rockets (killed, through): 0 Coronas {none:?}, 1 {one:?}, 2 {two:?}");
    assert_eq!(none.0, 0);
    assert!(none.1 >= 8, "the Javelin landed only {} rockets", none.1);
    // One Corona stops most of the stream; a pair stops nearly all of it.
    assert!(
        one.0 >= one.1 * 2,
        "one Corona let {} of {} through",
        one.1,
        one.0 + one.1
    );
    assert!(two.1 <= one.1, "two Coronas let more through than one");
}

#[test]
fn corona_burns_ballista_shells_but_a_battery_saturates_it() {
    let ticks = 340;
    let none = barrage(BALLISTA, 900, 2, 0, ticks);
    let two = barrage(BALLISTA, 900, 2, 1, ticks);
    let eight = barrage(BALLISTA, 900, 8, 1, ticks);
    println!(
        "ballista shells (killed, through): 2 vs none {none:?}, 2 vs 1 {two:?}, 8 vs 1 {eight:?}"
    );
    assert_eq!(none.0, 0);
    assert!(none.1 >= 10, "two Ballistas landed only {} shells", none.1);
    assert_eq!(two.1, 0, "one Corona let a pair's shells through");
    // A shell takes 1.5 s of burn: a battery of eight outruns one Corona's two lasers.
    assert!(
        eight.1 >= 10,
        "one Corona stopped all but {} of eight Ballistas' shells",
        eight.1
    );
}

const CORONA: &str = "aster_t2_missile_defense";
const CORONA_T3: &str = "aster_t3_missile_defense";
const JAVELIN: &str = "aster_t2_missile";
const BALLISTA: &str = "aster_t1_artillery";

#[test]
fn the_tier_three_corona_holds_where_two_lasers_saturate() {
    let w = world();
    let t2 = w.blueprints.unit(w.blueprints.id_of(CORONA).unwrap());
    let t3 = w.blueprints.id_of(CORONA_T3).unwrap();
    assert_eq!(t2.upgrades_to, Some(t3));
    let t3 = w.blueprints.unit(t3);
    assert_eq!(t3.anti_missile_lasers, 4);
    assert!(t3.anti_missile > t2.anti_missile);
    let two = barrage_on(CORONA, BALLISTA, 900, 8, 1, 340);
    let four = barrage_on(CORONA_T3, BALLISTA, 900, 8, 1, 340);
    println!("eight ballistas (killed, through): vs T2 {two:?}, vs T3 {four:?}");
    assert!(
        four.1 * 2 <= two.1,
        "a T3 Corona let {} through, a T2 {}",
        four.1,
        two.1
    );
}

#[test]
#[ignore]
/// Rounds killed and let through as launchers and Coronas are added: Javelins at 560 m,
/// Ballistas at 400 m. Run with `cargo test --profile gate -p mc-sim --test sim --
/// missile_defense::zz_probe_saturation --ignored --nocapture`.
fn zz_probe_saturation() {
    for (key, x, most) in [(JAVELIN, 1060, 6), (BALLISTA, 900, 9)] {
        for j in 1..=most {
            let row: Vec<_> = (0..=3).map(|c| barrage(key, x, j, c, 340)).collect();
            println!("{j} x {key}: (killed, through) by 0..3 Coronas {row:?}");
        }
    }
}
