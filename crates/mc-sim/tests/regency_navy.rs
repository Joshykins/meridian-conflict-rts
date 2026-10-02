//! The Regency's navy (docs/NAVY.md "The Regency's navy"): the attack boat lies dived in
//! ambush and comes up to fire, the heavy destroyer fights dived with torpedoes only, and
//! the cruiser's heavy seeker takes structures and nothing else.

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
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            samples[y * 257 + x] = if x < 40 { 40 } else { 0 };
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
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
    let r = row(w, id);
    w.state.units.z[r] + w.bp(r).height < Fx::from_int(WATER)
}

fn health(w: &World, id: UnitId) -> Fx {
    w.state.units.health[row(w, id)]
}

fn full(w: &World, id: UnitId) -> Fx {
    w.bp(row(w, id)).health
}

#[test]
fn the_attack_boat_lies_dived_until_it_has_a_mark() {
    let mut w = sea();
    let dirk = spawn(&mut w, "regency_t1_attack_boat", 0, 1000, 1000);
    run(&mut w, 60);
    assert!(
        under(&w, dirk),
        "with nothing to fight it goes down and stays"
    );
    assert!(w.state.units.dive_goal[row(&w, dirk)], "ordered down");
}

#[test]
fn the_attack_boat_surfaces_to_fire_and_dives_again() {
    let mut w = sea();
    let dirk = spawn(&mut w, "regency_t1_attack_boat", 0, 1000, 1000);
    run(&mut w, 40);
    assert!(under(&w, dirk));
    // An enemy boat sails into reach of the repeater.
    let skiff = spawn(&mut w, "aster_t1_attack_boat", 1, 1250, 1000);
    let mut surfaced = false;
    for _ in 0..80 {
        run(&mut w, 1);
        if w.state.units.row(skiff).is_none() {
            break;
        }
        surfaced |= !under(&w, dirk);
        if under(&w, dirk) {
            assert_eq!(
                health(&w, skiff),
                full(&w, skiff),
                "nothing hits it while the boat is under"
            );
        }
        if health(&w, skiff) < full(&w, skiff) {
            break;
        }
    }
    assert!(surfaced, "it came up for the mark");
    assert!(
        w.state.units.row(skiff).is_none() || health(&w, skiff) < full(&w, skiff),
        "and fired on it"
    );
    // The mark is gone: it goes back down.
    if let Some(r) = w.state.units.row(skiff) {
        w.state.units.health[r] = Fx::ZERO;
    }
    run(&mut w, 60);
    assert!(under(&w, dirk), "with the mark gone it dives again");
}

#[test]
fn the_heavy_destroyer_fights_dived_with_torpedoes_only() {
    let mut w = sea();
    let claymore = spawn(&mut w, "regency_t2_destroyer", 0, 1000, 1000);
    run(&mut w, 120);
    assert!(under(&w, claymore), "it dives by default");
    // An enemy boat in reach of the torpedoes: dived, only they can strike it.
    let target = spawn(&mut w, "aster_t1_attack_boat", 1, 1500, 1000);
    run(&mut w, 200);
    let hit = w
        .state
        .units
        .row(target)
        .is_none_or(|r| w.state.units.health[r] < w.bp(r).health);
    assert!(hit, "its torpedoes reach a hull from under the water");
    assert!(
        under(&w, claymore),
        "it stays dived: it was ordered down, not in ambush"
    );
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
