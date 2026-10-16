//! The tech 2 artillery installations (Onager, Basilisk): counter-siege and coast defence.
//! A tech 3 siege gun or a tech 2 destroyer drives up from out of reach, stops inside its
//! own reach of the gun and fires; the gun, which outreaches both, must win the duel. And
//! with a tank nearer, it still turns onto the siege gun. The `zz_t2_artillery_probe`
//! prints each duel, against one attacker and against two (about the gun's price: it
//! should beat two Trebuchets and lose to two Kilns or destroyers), with the time, the
//! health each side kept and the shots each fired:
//! `cargo test --profile gate -p mc-sim --test sim -- t2_artillery:: --ignored --nocapture`

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const ONAGER: &str = "aster_t2_artillery";
const BASILISK: &str = "regency_t2_artillery";

/// Height of the water in `sea` worlds; the bed is at zero.
const WATER: i32 = 20;

/// A 4 km map: flat ground, or (`sea`) a strip of shore along the west edge, 320 m deep,
/// and 20 m of water past it.
fn world(sea: bool) -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let cells = 512;
    let samples = (0..(cells + 1) * (cells + 1))
        .map(|i| if !sea || i % (cells + 1) < 40 { 40 } else { 0 })
        .collect();
    let terrain = Heightfield::from_samples(
        cells as u32,
        cells as u32,
        samples,
        Fx::ZERO,
        Fx::ONE,
        Fx::from_int(WATER),
    );
    let map = MapData {
        doodads: Default::default(),
        name: "artillery".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(3800, 3800)],
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
        players: vec![player("defender", 0), player("attacker", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // These measure the gun, not the grid: its fed reload (`fed_reload.rs`) is paid. The
    // attacker's side stays as it was, its shields unpowered.
    w.state.players[0].free_build = true;
    w
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(
            id,
            owner,
            FxVec2::from_ints(x, y),
            Angle::from_degrees(heading),
            true,
        )
        .unwrap();
    w.state.units.id(row)
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state
        .units
        .row(id)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

/// How a duel ended: seconds it took, the share of its health each side kept (per mille)
/// and the shots each side fired.
struct Duel {
    seconds: i32,
    gun: i32,
    attacker: i32,
    gun_shots: usize,
    attacker_shots: usize,
}

fn shots(w: &World, key: &str) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp))
        .count()
}

/// `gun` stands at the west; `count` of `attacker` start 2100 m east, out of its reach,
/// 80 m apart, and are ordered to `stand` metres from it, where they fire at will. Runs
/// until the gun or all of them die, or four minutes.
fn duel(gun: &str, attacker: &str, stand: i32, count: i32) -> Duel {
    let sea = attacker.contains("destroyer");
    let mut w = world(sea);
    let (x, y) = (150, 2000);
    let gun_id = add(&mut w, gun, 0, x, y, 0);
    let foes: Vec<UnitId> = (0..count)
        .map(|i| add(&mut w, attacker, 1, x + 2100, y + i * 80, 180))
        .collect();
    let orders: Vec<PlayerCommand> = foes
        .iter()
        .enumerate()
        .map(|(i, &foe)| PlayerCommand {
            player: 1,
            command: Command::Move {
                units: vec![foe],
                target: FxVec2::from_ints(x + stand, y + i as i32 * 80),
                queue: false,
            },
        })
        .collect();
    let foes_health = |w: &World| {
        foes.iter()
            .map(|&f| health(w, f))
            .fold(Fx::ZERO, |a, b| a + b)
    };
    let (gun_full, foe_full) = (health(&w, gun_id), foes_health(&w));
    w.tick(&orders).unwrap();
    let (mut ticks, mut gun_shots, mut attacker_shots) = (1, 0, 0);
    while ticks < 240 * TICKS_PER_SECOND as i32
        && health(&w, gun_id) > Fx::ZERO
        && foes_health(&w) > Fx::ZERO
    {
        w.tick(&[]).unwrap();
        gun_shots += shots(&w, gun);
        attacker_shots += shots(&w, attacker);
        ticks += 1;
    }
    let share = |hp: Fx, of: Fx| (hp * 1000 / of).round_int();
    Duel {
        seconds: ticks / TICKS_PER_SECOND as i32,
        gun: share(health(&w, gun_id), gun_full),
        attacker: share(foes_health(&w), foe_full),
        gun_shots,
        attacker_shots,
    }
}

/// Each gun against each faction's siege gun and destroyer, and where they stop: just
/// inside their own reach.
const DUELS: [(&str, &str, i32); 8] = [
    (ONAGER, "aster_t3_artillery", 1750),
    (ONAGER, "regency_t3_artillery", 1550),
    (ONAGER, "aster_t2_destroyer", 1450),
    (ONAGER, "regency_t2_destroyer", 1250),
    (BASILISK, "aster_t3_artillery", 1750),
    (BASILISK, "regency_t3_artillery", 1550),
    (BASILISK, "aster_t2_destroyer", 1450),
    (BASILISK, "regency_t2_destroyer", 1250),
];

#[test]
fn the_t2_artillery_wins_its_duels_with_siege_guns_and_destroyers() {
    for (gun, attacker, stand) in DUELS {
        let d = duel(gun, attacker, stand, 1);
        assert!(
            d.attacker == 0 && d.gun > 0,
            "{gun} vs {attacker}: after {} s the gun has {}‰, the attacker {}‰",
            d.seconds,
            d.gun,
            d.attacker
        );
    }
}

/// With a tank on the road nearer than the siege gun, it shells the siege gun.
#[test]
fn the_t2_artillery_turns_onto_a_siege_gun_from_a_nearer_tank() {
    for gun in [ONAGER, BASILISK] {
        let mut w = world(false);
        add(&mut w, gun, 0, 150, 2000, 0);
        let tank = add(&mut w, "aster_t2_tank", 1, 150 + 1000, 2000, 180);
        let siege = add(&mut w, "aster_t3_artillery", 1, 150 + 1750, 2300, 180);
        let row = w.state.units.row(tank).unwrap();
        w.state.units.fire_state[row] = FireState::HoldFire;
        let siege_full = health(&w, siege);
        let tank_full = health(&w, tank);
        for _ in 0..(20 * TICKS_PER_SECOND) {
            w.tick(&[]).unwrap();
        }
        assert!(
            health(&w, siege) < siege_full,
            "{gun} never hit the siege gun"
        );
        assert_eq!(health(&w, tank), tank_full, "{gun} shelled the tank first");
    }
}

/// Prints every duel: `cargo test --profile gate -p mc-sim --test sim --
/// t2_artillery:: --ignored --nocapture`.
#[test]
#[ignore]
fn zz_t2_artillery_probe() {
    for (count, (gun, attacker, stand)) in [1, 2].into_iter().flat_map(|n| DUELS.map(|d| (n, d))) {
        let d = duel(gun, attacker, stand, count);
        println!(
            "{gun:>22} vs {count}x {attacker:<22} {:>4} s  gun {:>4}‰ {:>3} shots  attacker {:>4}‰ {:>3} shots",
            d.seconds, d.gun, d.gun_shots, d.attacker, d.attacker_shots
        );
    }
}
