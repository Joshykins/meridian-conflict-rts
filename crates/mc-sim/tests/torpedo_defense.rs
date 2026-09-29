//! Torpedo defence structures (docs/NAVY.md): the Breakwater float, which meets torpedoes
//! running at anything near it, and the Fathom on the seabed, which only sonar finds and
//! only torpedoes reach.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
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
fn a_breakwater_bursts_torpedoes_running_at_a_ship_beside_it() {
    let mut w = sea(false);
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1300, 1000, flag::PASSIVE);
    spawn(&mut w, "aster_t1_torpedo_defense", 0, 1300, 1100, 0);
    let full = health(&w, pike);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1000, 0);
    attack(&mut w, 1, sub, pike);
    let mut seen = 0;
    for _ in 0..140 {
        w.tick(&[]).unwrap();
        seen += met(&w);
    }
    assert!(seen >= 3, "only {seen} torpedoes intercepted");
    assert!(
        health(&w, pike) > full - Fx::from_int(170),
        "the Pike took a salvo"
    );
}

#[test]
fn a_seabed_installation_covers_a_whole_bay() {
    let mut w = sea(false);
    // A ship 450 m off the installation, past a tier 1 float's reach.
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
fn torpedo_defence_needs_no_power() {
    let mut w = sea(false);
    // Shields drawing far more than the side makes: its grid goes dark.
    for x in [900, 1000, 1100] {
        spawn(&mut w, "aster_t3_shield", 0, x, 1800, 0);
    }
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1300, 1000, flag::PASSIVE);
    spawn(&mut w, "aster_t1_torpedo_defense", 0, 1300, 1100, 0);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.players[0].efficiency,
        Fx::ZERO,
        "the grid is not dark"
    );
    let full = health(&w, pike);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1000, 0);
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
}

#[test]
fn a_breakwater_turret_turns_toward_the_torpedoes_and_fires_before_it_is_round() {
    let mut w = sea(false);
    let pike = spawn(&mut w, "aster_t1_frigate", 0, 1300, 1000, flag::PASSIVE);
    // Its turret rests facing east; the torpedoes come in from the west.
    let float = spawn(&mut w, "aster_t1_torpedo_defense", 0, 1300, 1100, 0);
    let sub = spawn(&mut w, "aster_t1_submarine", 1, 1000, 1000, 0);
    attack(&mut w, 1, sub, pike);
    let bp = w.blueprints.id_of("aster_t1_torpedo_defense").unwrap();
    let mut first_shot_yaw = None;
    let mut widest = 0u16;
    for _ in 0..140 {
        w.tick(&[]).unwrap();
        let r = row(&w, float);
        let yaw = w.state.units.weapon_yaw[r][0]
            .delta_to(mc_core::Angle::ZERO)
            .unsigned_abs();
        widest = widest.max(yaw);
        let fired = w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp));
        if fired && first_shot_yaw.is_none() {
            first_shot_yaw = Some(yaw);
        }
    }
    let quarter = mc_core::Angle::from_degrees(90)
        .delta_to(mc_core::Angle::ZERO)
        .unsigned_abs();
    let first = first_shot_yaw.expect("the float never fired");
    assert!(
        first < quarter * 3 / 2,
        "it waited to come round before firing"
    );
    assert!(
        widest > quarter * 3 / 2,
        "the turret never turned toward the torpedoes"
    );
}
