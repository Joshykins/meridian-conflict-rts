//! Torpedo launchers (docs/NAVY.md): the Breakwater float, which puts attack torpedoes
//! into hulls a little past a Barracuda's reach, and the Fathom on the seabed, which only
//! sonar finds and only torpedoes reach, and which also meets torpedoes coming in.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, UnitBlueprint};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::{snap_to_build_grid, MapData};
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// The sea: 45 m of water over a flat bed at zero; a shelf 15 m under the surface from
/// x 320 to 640 m, and land west of it.
const WATER: i32 = 45;
const BREAKWATER: &str = "aster_t1_torpedo_defense";
const FATHOM: &str = "aster_t3_torpedo_defense";

fn sea(fog: bool) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..80 {
            samples[y * 257 + x] = if x < 40 { 60 } else { 30 };
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
        name: "deep sea".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
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
        fog,
        spawn_commanders: false,
    };
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32, flags: u16) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let at = snap_to_build_grid(w.blueprints.unit(bp), FxVec2::from_ints(x, y));
    let row = w.spawn_unit(bp, owner, at, Angle::ZERO, true).unwrap();
    w.state.units.flags[row] |= flags;
    w.state.units.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state
        .units
        .row(id)
        .map_or(Fx::ZERO, |r| w.state.units.health[r])
}

fn attack(w: &mut World, player: u8, unit: UnitId, target: UnitId) {
    w.tick(&[PlayerCommand {
        player,
        command: Command::Attack {
            units: vec![unit],
            target,
            queue: false,
        },
    }])
    .unwrap();
}

/// Takes a structure's attack torpedoes off, leaving its interceptors.
fn only_interceptors(w: &mut World, key: &str) {
    let id = w.blueprints.id_of(key).unwrap();
    Arc::make_mut(&mut w.blueprints).units[id.index()]
        .weapons
        .retain(|weapon| weapon.intercepts);
}

fn dive(w: &mut World, player: u8, unit: UnitId) {
    w.tick(&[PlayerCommand {
        player,
        command: Command::SetDive {
            units: vec![unit],
            dive: true,
        },
    }])
    .unwrap();
}

/// Interceptions this tick.
fn met(w: &World) -> usize {
    w.events
        .iter()
        .filter(|e| matches!(e, SimEvent::TorpedoIntercepted { .. }))
        .count()
}

#[test]
fn a_seabed_installation_stands_on_the_bottom_where_the_water_is_deep() {
    let mut w = sea(false);
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of(FATHOM).unwrap())
        .clone();
    let deep = snap_to_build_grid(&bp, FxVec2::from_ints(1200, 1000));
    let shelf = snap_to_build_grid(&bp, FxVec2::from_ints(480, 1000));
    assert!(w.can_place(&bp, deep), "refused 45 m of water");
    assert!(!w.can_place(&bp, shelf), "stood in 15 m of water");
    let fathom = spawn(&mut w, FATHOM, 0, 1200, 1000, 0);
    let r = row(&w, fathom);
    assert_eq!(w.state.units.z[r], Fx::ZERO, "not on the bed");
    assert!(w.state.units.z[r] + bp.height < Fx::from_int(WATER));

    // A tier 1 float stands on the shelf; the installation does not.
    let float = w.blueprints.id_of("aster_t1_torpedo_defense").unwrap();
    let float = w.blueprints.unit(float).clone();
    assert!(w.can_place(
        &float,
        snap_to_build_grid(&float, FxVec2::from_ints(480, 1000))
    ));
}

#[test]
fn only_sonar_finds_a_seabed_installation_and_only_torpedoes_reach_it() {
    let mut w = sea(true);
    // Its batteries taken off, so the torpedoes get through to show that they can.
    let id = w.blueprints.id_of(FATHOM).unwrap();
    Arc::make_mut(&mut w.blueprints).units[id.index()]
        .weapons
        .clear();
    let fathom = spawn(&mut w, FATHOM, 1, 1200, 1000, 0);
    let full = health(&w, fathom);
    // A frigate close over it: a gun in reach and a radar, but no sonar.
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1000, 1000, 0);
    for _ in 0..120 {
        w.tick(&[]).unwrap();
    }
    assert!(!w.detects(0, row(&w, fathom)), "radar found it");
    assert_eq!(health(&w, fathom), full, "a gun reached it");
    attack(&mut w, 0, pike, fathom);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        health(&w, fathom),
        full,
        "a gun reached it on an attack order"
    );

    // A submarine hears it and puts torpedoes into it.
    let sub = spawn(&mut w, "aster_t1_submarine", 0, 900, 1000, 0);
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert!(w.detects(0, row(&w, fathom)), "sonar did not find it");
    attack(&mut w, 0, sub, fathom);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        if health(&w, fathom) < full {
            return;
        }
    }
    panic!("the torpedoes never reached it");
}

#[test]
fn a_seabed_installation_covers_a_whole_bay() {
    let mut w = sea(false);
    only_interceptors(&mut w, FATHOM);
    // A ship 450 m off the installation.
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1400, 1450, flag::PASSIVE);
    spawn(&mut w, FATHOM, 0, 1400, 1000, 0);
    let full = health(&w, pike);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1100, 1450, 0);
    attack(&mut w, 1, sub, pike);
    let mut seen = 0;
    for _ in 0..140 {
        w.tick(&[]).unwrap();
        seen += met(&w);
    }
    assert!(seen >= 3, "only {seen} torpedoes intercepted");
    assert_eq!(health(&w, pike), full, "a torpedo got through");
}

#[test]
fn a_breakwater_sinks_a_dived_barracuda_that_cannot_reach_it() {
    let mut w = sea(false);
    let float = spawn(&mut w, BREAKWATER, 0, 1300, 1000, 0);
    // 580 m off: inside the float's 600 m, outside the Barracuda's 550.
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 720, 1000, 0);
    dive(&mut w, 1, sub);
    let full = health(&w, float);
    for _ in 0..600 {
        w.tick(&[]).unwrap();
        if w.state.units.row(sub).is_none() {
            // (A kill's veterancy can lift its health past where it started.)
            assert!(health(&w, float) >= full, "the Barracuda reached the float");
            return;
        }
    }
    panic!(
        "the Barracuda lived through a minute of torpedoes ({:?} left)",
        health(&w, sub)
    );
}

#[test]
fn a_seabed_installation_puts_torpedoes_into_ships_over_it() {
    let mut w = sea(false);
    spawn(&mut w, FATHOM, 0, 1400, 1000, 0);
    let pike = spawn(&mut w, "aster_t1_frigate", 1, 1400, 1800, flag::PASSIVE);
    let full = health(&w, pike);
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        if health(&w, pike) < full {
            return;
        }
    }
    panic!("no torpedo reached a frigate 800 m off");
}

#[test]
fn torpedo_launchers_need_no_power() {
    let mut w = sea(false);
    // Shields drawing far more than the side makes: its grid goes dark.
    for x in [900, 1000, 1100] {
        spawn(&mut w, "aster_t3_shield", 0, x, 1800, 0);
    }
    spawn(&mut w, BREAKWATER, 0, 1300, 1000, 0);
    only_interceptors(&mut w, FATHOM);
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1300, 1300, flag::PASSIVE);
    spawn(&mut w, FATHOM, 0, 1500, 1300, 0);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.players[0].efficiency,
        Fx::ZERO,
        "the grid is not dark"
    );
    let full = health(&w, pike);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1300, 0);
    let sub_full = health(&w, sub);
    attack(&mut w, 1, sub, pike);
    let mut seen = 0;
    for _ in 0..140 {
        w.tick(&[]).unwrap();
        seen += met(&w);
    }
    assert!(
        seen >= 3,
        "only {seen} torpedoes intercepted on a dark grid"
    );
    assert!(health(&w, pike) > full - Fx::from_int(170));
    assert!(
        health(&w, sub) < sub_full,
        "the float held fire on a dark grid"
    );
}

/// Damage per second of what a unit carries against hulls (interceptors left out).
fn attack_dps(bp: &UnitBlueprint) -> f32 {
    bp.weapons
        .iter()
        .filter(|w| !w.intercepts && w.torpedo)
        .map(|w| w.damage.to_f32() * w.salvo.max(1) as f32 * 10.0 / w.reload_ticks.max(1) as f32)
        .sum()
}

fn torpedo_range(bp: &UnitBlueprint) -> f32 {
    bp.weapons
        .iter()
        .filter(|w| !w.intercepts && w.torpedo)
        .map(|w| w.range_max.to_f32())
        .fold(0.0, f32::max)
}

/// A launcher gives up moving, so at each tier it carries far more torpedo DPS and hull
/// per unit of mass than the submarine it meets, and it reaches past that submarine.
#[test]
fn launchers_are_far_more_mass_efficient_than_submarines() {
    let w = sea(false);
    let bp = |key: &str| w.blueprints.unit(w.blueprints.id_of(key).unwrap());
    for (launcher, sub) in [
        (BREAKWATER, "aster_t1_submarine"),
        ("aster_t2_torpedo_defense", "aster_t2_submarine"),
        (FATHOM, "aster_t3_submarine"),
    ] {
        let (l, s) = (bp(launcher), bp(sub));
        let per_mass = |b: &UnitBlueprint, v: f32| v / b.cost_mass.to_f32();
        let dps = per_mass(l, attack_dps(l)) / per_mass(s, attack_dps(s));
        let hp = per_mass(l, l.health.to_f32()) / per_mass(s, s.health.to_f32());
        assert!(
            dps >= 1.7,
            "{launcher}: only {dps:.2}x {sub}'s torpedo DPS per mass"
        );
        assert!(
            hp >= 1.7,
            "{launcher}: only {hp:.2}x {sub}'s health per mass"
        );
        assert!(
            torpedo_range(l) > torpedo_range(s),
            "{launcher} does not outrange {sub}"
        );
    }
}
