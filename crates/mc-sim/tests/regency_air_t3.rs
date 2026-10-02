//! The Regency's tech 3 aircraft (`data/factions/regency/units/air_t3.ron`): the Augur
//! cruises above the weather, where a gun reaches it only along the line of sight, and the
//! Scythe follows the army it guards, taking apart the wrecks that fall round it, the
//! biggest included.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const AUGUR: &str = "regency_t3_spy_plane";
const SCYTHE: &str = "regency_t3_scavenger";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(512, 512, Fx::from_int(20));
    let map = MapData {
        name: "regency air".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3500, 3500)],
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
        seed: 5,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// Mass stores are counted afresh each tick: keep room for what is reclaimed.
fn tick(w: &mut World, commands: &[PlayerCommand]) {
    w.tick(commands).unwrap();
    w.state.players[0].mass_capacity = Fx::from_int(1_000_000);
}

/// A wreck of `key` at (`x`, `y`) holding `mass`.
fn wreck(w: &mut World, key: &str, x: i32, y: i32, mass: i32) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    let radius = w.blueprints.unit(bp).radius;
    w.state
        .wrecks
        .spawn(
            bp,
            FxVec2::from_ints(x, y),
            radius,
            Angle::ZERO,
            Fx::from_int(mass),
            0,
        )
        .unwrap()
}

/// Lets the Augur climb to its cruise height, circling where it is: its height over the
/// ground.
fn climb(w: &mut World, augur: usize) -> Fx {
    for _ in 0..600 {
        tick(w, &[]);
    }
    w.state.units.z[augur] - Fx::from_int(20)
}

#[test]
fn an_augur_cruises_over_the_cloud_deck_out_of_reach_of_fighters_and_flak() {
    let mut w = world();
    let augur = add(&mut w, AUGUR, 0, 2000, 2000);
    let up = climb(&mut w, augur);
    // The fair-weather deck tops out about 530 m over the ground (mc-render `sky.rs`).
    assert!(up > Fx::from_int(800), "cruising {up} m up");
    let full = w.state.units.health[augur];
    // A fighter and a flak gun right under it: both reach it across the map, neither up.
    add(&mut w, "aster_t3_air_superiority", 1, 2000, 2050);
    add(&mut w, "regency_t1_mobile_aa", 1, 2030, 2000);
    for _ in 0..300 {
        tick(&mut w, &[]);
    }
    assert_eq!(w.state.units.health[augur], full, "nothing reached it");
}

#[test]
fn a_long_range_launcher_still_reaches_an_augur() {
    let mut w = world();
    let augur = add(&mut w, AUGUR, 0, 2000, 2000);
    climb(&mut w, augur);
    let full = w.state.units.health[augur];
    add(&mut w, "aster_t3_sam", 1, 2300, 2000);
    let mut hit = false;
    for _ in 0..600 {
        tick(&mut w, &[]);
        if !w.state.units.slots.is_alive(augur) || w.state.units.health[augur] < full {
            hit = true;
            break;
        }
    }
    assert!(hit, "a SAM site reaches up to it");
}

#[test]
fn a_scythe_on_guard_follows_its_tank_and_clears_the_wrecks_round_it() {
    let mut w = world();
    let scythe = add(&mut w, SCYTHE, 0, 1000, 1000);
    let tank = add(&mut w, "regency_t1_tank", 0, 1000, 1080);
    // Off the tank's way, out of the heads' reach of it but inside the guard's ring.
    let wrecks = [
        wreck(&mut w, "aster_t1_tank", 1400, 1500, 300),
        wreck(&mut w, "aster_t1_tank", 1900, 650, 300),
    ];
    let (scythe_id, tank_id) = (w.state.units.id(scythe), w.state.units.id(tank));
    let at = w.state.units.pos[tank];
    tick(
        &mut w,
        &[cmd(Command::Guard {
            units: vec![scythe_id],
            pos: at,
            target: tank_id,
            radius: Fx::from_int(500),
            queue: false,
        })],
    );
    tick(
        &mut w,
        &[cmd(Command::Move {
            units: vec![tank_id],
            target: FxVec2::from_ints(2000, 1080),
            queue: false,
        })],
    );
    for _ in 0..1500 {
        tick(&mut w, &[]);
    }
    for wreck in wrecks {
        assert!(
            !w.state.wrecks.slots.is_alive(wreck),
            "it took apart the wreck the army passed"
        );
    }
    let apart = w.state.units.pos[scythe].distance(w.state.units.pos[tank]);
    assert!(
        apart < Fx::from_int(600),
        "it kept with the tank: {apart} m off"
    );
}

#[test]
fn a_scythe_takes_an_experimentals_wreck_apart_in_seconds() {
    let mut w = world();
    let scythe = add(&mut w, SCYTHE, 0, 1000, 1000);
    let power = w.bp(scythe).reclaimer.unwrap().power;
    let big = wreck(&mut w, "aster_t4_assault_tank", 1150, 1000, 2400);
    // Ten seconds at its pull, and a few to get there and charge.
    let seconds = (Fx::from_int(2400) / power).ceil_int() + 8;
    for _ in 0..seconds * mc_core::TICKS_PER_SECOND as i32 {
        tick(&mut w, &[]);
        if !w.state.wrecks.slots.is_alive(big) {
            break;
        }
    }
    assert!(
        !w.state.wrecks.slots.is_alive(big),
        "the titan's wreck is gone within {seconds} s"
    );
}
