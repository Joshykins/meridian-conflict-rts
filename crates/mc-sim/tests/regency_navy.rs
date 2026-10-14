//! The Regency's navy (docs/NAVY.md "The Regency's navy"): only the submarines dive, the
//! surface hulls stay up and fight, and the cruiser's heavy seeker takes structures and
//! nothing else.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// 20 m of water over a flat bed, a strip of land along the west edge (x < 320 m).
const WATER: i32 = 20;

fn sea() -> World {
    sea_of(WATER)
}

/// The sea with `water` metres over the bed.
fn sea_of(water: i32) -> World {
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            samples[y * 257 + x] = if x < 40 { 40 } else { 0 };
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(water));
    let map = MapData {
        doodads: Default::default(),
        name: "sea".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
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
        seed: 7,
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

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn under(w: &World, id: UnitId) -> bool {
    under_of(w, id, WATER)
}

fn under_of(w: &World, id: UnitId, water: i32) -> bool {
    let r = row(w, id);
    w.state.units.z[r] + w.bp(r).height < Fx::from_int(water)
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state.units.health[row(w, id)]
}

fn full(w: &World, id: UnitId) -> Fx {
    w.bp(row(w, id)).health
}

#[test]
fn only_the_submarines_dive() {
    let w = sea();
    let subs = ["regency_t1_submarine", "regency_t3_submarine"];
    for bp in &w.blueprints.units {
        let naval = bp
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval);
        if !naval || !bp.key.starts_with("regency_") {
            continue;
        }
        assert_eq!(
            bp.dive.is_some(),
            subs.contains(&bp.key.as_str()),
            "{}: only the submarines dive",
            bp.key
        );
    }
}

#[test]
fn the_attack_boat_stays_up_and_fires() {
    let mut w = sea();
    let dirk = spawn(&mut w, "regency_t1_attack_boat", 0, 1000, 1000);
    run(&mut w, 60);
    assert!(!under(&w, dirk), "it rides the surface");
    let skiff = spawn(&mut w, "aster_t1_attack_boat", 1, 1250, 1000);
    run(&mut w, 80);
    assert!(
        w.state.units.row(skiff).is_none() || health(&w, skiff) < full(&w, skiff),
        "it fired on the boat in reach"
    );
    assert!(!under(&w, dirk), "and never went down");
}

#[test]
fn the_cruisers_heavy_seeker_takes_structures_only() {
    let w = sea();
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of("regency_t2_cruiser").unwrap());
    let seeker = bp
        .weapons
        .iter()
        .find(|w| w.name == "Heavy Gravitic Seeker")
        .expect("a heavy seeker");
    assert_eq!(seeker.target_mask, mc_data::cat::STRUCTURE);
    assert!(seeker.missile && seeker.guided && seeker.salvo <= 1);
    assert!(
        seeker.range_max
            > bp.weapons
                .iter()
                .filter(|o| !std::ptr::eq(*o, seeker))
                .map(|o| o.range_max)
                .max()
                .unwrap(),
        "its long arm outreaches everything else aboard"
    );
}

#[test]
fn the_assault_submarine_is_the_fastest_regency_hull() {
    let w = sea();
    let speed = |key: &str| {
        w.blueprints
            .unit(w.blueprints.id_of(key).unwrap())
            .motion
            .unwrap()
            .speed
    };
    let rapier = speed("regency_t3_submarine");
    for key in [
        "regency_t1_submarine",
        "regency_t1_frigate",
        "regency_t2_destroyer",
        "regency_t2_cruiser",
        "regency_t3_battleship",
        "regency_t3_carrier",
    ] {
        assert!(rapier > speed(key), "{key} is as fast as the Rapier");
    }
}

#[test]
fn surfaced_guns_hold_while_the_rapier_is_under() {
    let mut w = sea_of(40);
    let rapier = spawn(&mut w, "regency_t3_submarine", 0, 1000, 1000);
    run(&mut w, 200);
    assert!(under_of(&w, rapier, 40));
    // A boat on the surface, in reach of everything aboard: only the torpedoes may take it.
    spawn(&mut w, "aster_t1_frigate", 1, 1500, 1000);
    run(&mut w, 1);
    let r = row(&w, rapier);
    let bp = w.bp(r);
    assert!(bp.weapons.iter().any(|w| w.surfaced));
    for (weapon, mark) in bp.weapons.iter().zip(w.state.units.weapon_target[r]) {
        if weapon.surfaced {
            assert_eq!(
                mark,
                mc_sim::tables::Handle::NONE,
                "{} has a mark dived",
                weapon.name
            );
        }
    }
}

#[test]
fn no_warship_is_slower_than_32() {
    // Both navies: the big hulls used to crawl at 24-30 m/s (2026-10-08).
    let w = sea();
    for bp in &w.blueprints.units {
        let Some(m) = bp.motion.filter(|m| m.layer == mc_data::MoveLayer::Naval) else {
            continue;
        };
        assert!(
            m.speed >= Fx::from_int(32),
            "{} sails at {:?}",
            bp.key,
            m.speed
        );
    }
}
