//! The Regency's tech 3 aircraft (`data/factions/regency/units/air_t3.ron`): the Augur
//! cruises above the weather, where a gun reaches it only along the line of sight.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const AUGUR: &str = "regency_t3_spy_plane";

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

fn tick(w: &mut World) {
    w.tick(&[]).unwrap();
}

/// Lets the Augur climb to its cruise height, circling where it is: its height over the
/// ground.
fn climb(w: &mut World, augur: usize) -> Fx {
    for _ in 0..600 {
        tick(w);
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
        tick(&mut w);
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
        tick(&mut w);
        if !w.state.units.slots.is_alive(augur) || w.state.units.health[augur] < full {
            hit = true;
            break;
        }
    }
    assert!(hit, "a SAM site reaches up to it");
}
