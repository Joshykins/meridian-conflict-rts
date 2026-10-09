//! The Regency's hovering combat craft (`data/factions/regency/units/air_gunships.ron`): the
//! Quiver lets its Wicks go at what it fights and each bursts on the mark (`strike_drones.rs`),
//! and the Reaper strafes its mark with two beams, leaving the ground burning
//! (`Weapon::strafe`).

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
        props: Vec::new(),
    };
    let player = |name: &str, faction: &str, team| PlayerSetup {
        name: name.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 11,
        players: vec![player("you", "Regency", 0), player("hostile", "Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let bp = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, bp, Arc::new(Pool::new(1)), &config).unwrap()
}

/// A side with plenty in the bank (and a commander, whose stores hold it), so its drones
/// are built as fast as they can be.
fn rich(w: &mut World, player: usize) {
    spawn(w, "regency_commander", player as u8, 150, 150);
    let p = &mut w.state.players[player];
    p.mass_capacity = Fx::from_int(20000);
    p.energy_capacity = Fx::from_int(200000);
    p.mass = Fx::from_int(20000);
    p.energy = Fx::from_int(200000);
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

/// The carrier's drones: (alive, finished and home on their sockets).
fn wicks(w: &World, carrier: UnitId) -> (usize, usize) {
    let units = &w.state.units;
    let flock: Vec<usize> = units
        .slots
        .iter()
        .filter(|&r| units.drone_parent[r] == carrier)
        .collect();
    let home = flock
        .iter()
        .filter(|&&r| units.is_active(r) && units.deploy[r] == 0)
        .count();
    (flock.len(), home)
}

fn attack(w: &mut World, unit: UnitId, target: UnitId) {
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: vec![unit],
            target,
            queue: false,
        },
    }])
    .unwrap();
}

/// A hostile tank parked and holding fire, so only what is tested shoots.
fn target(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let id = spawn(w, key, 1, x, y);
    let r = row(w, id);
    w.state.units.flags[r] |= mc_sim::tables::flag::PASSIVE;
    id
}

#[test]
fn a_quiver_racks_six_wicks_and_bursts_them_on_its_mark() {
    let mut w = world();
    rich(&mut w, 0);
    let quiver = spawn(&mut w, "regency_t2_drone_carrier", 0, 600, 600);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(wicks(&w, quiver), (6, 6), "the bays fill");
    let wick_bp = w.blueprints.id_of("regency_wick").unwrap();
    let wrecks_before = w.state.wrecks.slots.live();
    let tank = target(&mut w, "aster_t2_tank", 1000, 600);
    let full = w.state.units.health[row(&w, tank)];
    attack(&mut w, quiver, tank);
    let mut launched = 0;
    let mut bursts = 0;
    let mut low = Fx::from_int(1000);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        let (_, home) = wicks(&w, quiver);
        launched = launched.max(6 - home);
        let units = &w.state.units;
        for r in units.slots.iter() {
            if units.blueprint[r] == wick_bp && units.deploy[r] == 1 {
                low = low.min(units.z[r]);
            }
        }
        bursts += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::Impact { on_unit: true, .. }))
            .count();
        if w.state.units.row(tank).is_none() {
            break;
        }
    }
    assert!(launched >= 3, "it let its Wicks go ({launched})");
    assert!(bursts > 0, "a Wick burst on the tank");
    assert!(
        low < Fx::from_int(60),
        "the Wicks dived from cruise ({low})"
    );
    let left = w
        .state
        .units
        .row(tank)
        .map_or(Fx::ZERO, |t| w.state.units.health[t]);
    assert!(left < full, "the tank was hurt: {left} of {full}");
    // Spent Wicks leave nothing behind, and are no loss to their side.
    assert_eq!(
        w.state.wrecks.slots.live(),
        wrecks_before + usize::from(left == Fx::ZERO)
    );
    assert_eq!(w.state.players[0].units_lost, 0);
    // The nanites build the spent ones again.
    for _ in 0..400 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(wicks(&w, quiver).0, 6, "the bays are filled again");
}

#[test]
fn a_wick_whose_mark_dies_first_comes_home_to_its_bay() {
    let mut w = world();
    rich(&mut w, 0);
    let quiver = spawn(&mut w, "regency_t2_drone_carrier", 0, 600, 600);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    let tank = target(&mut w, "aster_t1_tank", 1050, 600);
    attack(&mut w, quiver, tank);
    // Let the salvo go, then take its mark away before any Wick gets there.
    for _ in 0..12 {
        w.tick(&[]).unwrap();
    }
    let out = 6 - wicks(&w, quiver).1;
    assert!(out > 0, "Wicks were let go");
    let t = row(&w, tank);
    w.state.units.health[t] = Fx::ZERO;
    for _ in 0..400 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        wicks(&w, quiver),
        (6, 6),
        "every Wick is home again, none spent"
    );
}

#[test]
fn a_quiver_with_empty_bays_waits_for_its_wicks() {
    let mut w = world();
    // Nothing in the bank: no Wick is ever finished, so nothing is let go.
    let p = &mut w.state.players[0];
    p.mass = Fx::ZERO;
    p.energy = Fx::ZERO;
    p.mass_capacity = Fx::ZERO;
    p.energy_capacity = Fx::ZERO;
    let quiver = spawn(&mut w, "regency_t2_drone_carrier", 0, 600, 600);
    let tank = target(&mut w, "aster_t1_tank", 900, 600);
    let full = w.state.units.health[row(&w, tank)];
    attack(&mut w, quiver, tank);
    for _ in 0..200 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(wicks(&w, quiver).1, 0, "no Wick was finished");
    assert_eq!(w.state.units.health[row(&w, tank)], full);
    assert!(w.state.projectiles.is_empty());
}

#[test]
fn the_reaper_strafes_its_mark_with_two_burning_lines() {
    let mut w = world();
    rich(&mut w, 0);
    let reaper = spawn(&mut w, "regency_t3_assault_aircraft", 0, 400, 600);
    let tank = target(&mut w, "aster_t4_assault_tank", 1300, 600);
    let full = w
        .blueprints
        .unit(w.state.units.blueprint[row(&w, tank)])
        .health;
    attack(&mut w, reaper, tank);
    // The first run's strikes, until the beams go out.
    let mut first = Vec::new();
    let mut closest = Fx::from_int(10_000);
    let mut plasma_fires = false;
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        let struck: Vec<_> = w
            .events
            .iter()
            .filter_map(|e| match e {
                SimEvent::Impact { pos, .. } => Some(pos.xy()),
                _ => None,
            })
            .collect();
        if struck.is_empty() && !first.is_empty() {
            break;
        }
        first.extend(struck);
        if let Some(t) = w.state.units.row(tank) {
            let r = row(&w, reaper);
            closest = closest.min(w.state.units.pos[r].distance(w.state.units.pos[t]));
        }
        plasma_fires |= w.state.fires.plasma.iter().any(|&p| p);
    }
    assert!(
        first.len() >= 10,
        "{} strikes on the first run",
        first.len()
    );
    // The run is flown west to east along y = 600: the strikes lie along it, a line
    // through the mark, not a point on it.
    let (lo, hi) = first.iter().fold((i32::MAX, i32::MIN), |(lo, hi), p| {
        (lo.min(p.x.floor_int()), hi.max(p.x.floor_int()))
    });
    assert!(hi - lo > 100, "the strikes spread over {lo}..{hi}");
    assert!(
        lo < 1300 && hi > 1300,
        "the line ran through the mark: {lo}..{hi}"
    );
    // Two lines, one either side of the run's, 16 m apart.
    let port = first.iter().filter(|p| p.y > Fx::from_int(605)).count();
    let starboard = first.iter().filter(|p| p.y < Fx::from_int(595)).count();
    assert!(
        port > 3 && starboard > 3,
        "port {port}, starboard {starboard}"
    );
    assert!(
        plasma_fires,
        "the seared ground was left burning with plasma"
    );
    // It flew on in, not hanging off at its reach.
    assert!(
        closest < Fx::from_int(150),
        "came no nearer than {closest} m"
    );
    let left = w
        .state
        .units
        .row(tank)
        .map_or(Fx::ZERO, |t| w.state.units.health[t]);
    assert!(left < full, "the beams hurt the tank");
}
