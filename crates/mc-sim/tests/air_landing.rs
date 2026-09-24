//! Idle aircraft never hover for good: stopped over water or on a taken pad,
//! they fly to the nearest ground they fit on and set down.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// 256 cells of 8 m: dry ground (20 m) west of x = 1024 m, sea (-40 m) east of it.
fn coast() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let samples = (0..257 * 257)
        .map(|i| if i % 257 < 128 { 60 } else { 0 })
        .collect();
    let terrain = Heightfield::from_samples(
        256,
        256,
        samples,
        Fx::from_int(-40),
        Fx::ONE,
        Fx::from_int(5),
    );
    let map = MapData {
        name: "coast".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(512, 1500)],
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
        players: vec![player("you", 0), player("other", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn landed(w: &World, row: usize) -> bool {
    w.state.units.z[row] <= Fx::from_int(21) && w.state.units.speed[row] <= Fx::ONE
}

#[test]
fn aircraft_sent_out_to_sea_come_back_and_land_on_the_nearest_shore() {
    let mut w = coast();
    let keys = [
        "aster_t1_rotor_gunship",
        "aster_t1_bomber",
        "aster_t2_gunship",
    ];
    let rows: Vec<usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| add(&mut w, k, 800, 900 + 150 * i as i32))
        .collect();
    for (i, &row) in rows.iter().enumerate() {
        let command = Command::Move {
            units: vec![w.state.units.id(row)],
            target: FxVec2::from_ints(1250, 900 + 150 * i as i32),
            queue: false,
        };
        w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
    }
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
    }
    for (&row, key) in rows.iter().zip(keys) {
        let pos = w.state.units.pos[row];
        assert!(
            landed(&w, row),
            "{key} still up at {pos:?}, z {:?}",
            w.state.units.z[row]
        );
        assert!(pos.x < Fx::from_int(1024), "{key} set down at sea: {pos:?}");
        assert!(
            pos.x > Fx::from_int(900),
            "{key} flew past the nearest shore: {pos:?}"
        );
    }
}

#[test]
fn a_second_aircraft_on_a_taken_pad_lands_beside_it() {
    let mut w = coast();
    let a = add(&mut w, "aster_t1_bomber", 600, 600);
    let b = add(&mut w, "aster_t1_bomber", 600, 600);
    for _ in 0..900 {
        w.tick(&[]).unwrap();
    }
    assert!(landed(&w, a) && landed(&w, b), "both should be down");
    let gap = w.state.units.pos[a].distance(w.state.units.pos[b]);
    let r = w.blueprints.unit(w.state.units.blueprint[a]).radius;
    assert!(gap >= r * 2, "parked on top of each other: {gap:?}");
    assert!(gap <= Fx::from_int(120), "went too far: {gap:?}");
}

#[test]
fn the_reclaim_carrier_still_stays_up_over_the_sea() {
    let mut w = coast();
    let c = add(&mut w, "aster_t2_reclaim_carrier", 1400, 900);
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.units.z[c] > Fx::from_int(20), "carrier landed");
    assert!(
        w.state.units.pos[c].x > Fx::from_int(1300),
        "carrier went looking for land"
    );
}
