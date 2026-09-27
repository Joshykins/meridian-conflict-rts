//! The Culverin, tech 4 strategic artillery: it reaches across a 16 km map, shells the
//! nearest enemy its side knows of (building, land unit or ship), fires only on what its
//! side can see or has on radar, and scatters its shells round the mark.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::SimEvent;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const CULVERIN: &str = "aster_t4_artillery";
/// The gun's lot and the enemy base, 12 km apart across a 16 km map.
const GUN: (i32, i32) = (2000, 8192);
const BASE: (i32, i32) = (14000, 8192);

fn world(fog: bool) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    // 2048 cells of 8 m: 16 km a side.
    let terrain = Heightfield::flat(2048, 2048, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![
            FxVec2::from_ints(GUN.0, GUN.1),
            FxVec2::from_ints(BASE.0, BASE.1),
        ],
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
        seed: 9,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn spawn(w: &mut World, key: &str, owner: u8, (x, y): (i32, i32)) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn target_of(w: &World, gun: UnitId) -> UnitId {
    let row = w.state.units.row(gun).unwrap();
    w.state.units.weapon_target[row][0]
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}

#[test]
fn it_takes_the_nearest_enemy_building_or_unit() {
    let mut w = world(false);
    let gun = spawn(&mut w, CULVERIN, 0, GUN);
    spawn(&mut w, "aster_t3_land_factory", 1, BASE);
    let power = spawn(&mut w, "aster_t1_power", 1, (7000, 8192));
    run(&mut w, 30);
    assert_eq!(
        target_of(&w, gun),
        power,
        "the nearer building over the dearer one"
    );

    let mut w = world(false);
    let gun = spawn(&mut w, CULVERIN, 0, GUN);
    spawn(&mut w, "aster_t1_power", 1, (7000, 8192));
    let tank = spawn(&mut w, "aster_t1_tank", 1, (5000, 8192));
    run(&mut w, 30);
    assert_eq!(target_of(&w, gun), tank, "a tank nearer than any building");
}

#[test]
fn it_fires_only_on_what_its_side_can_see_or_has_on_radar() {
    let mut w = world(true);
    let gun = spawn(&mut w, CULVERIN, 0, GUN);
    let factory = spawn(&mut w, "aster_t3_land_factory", 1, BASE);
    run(&mut w, 60);
    assert_eq!(
        w.state.units.row(target_of(&w, gun)),
        None,
        "a base nobody has found is not a target"
    );
    // A tech 3 radar reaches 8 km: from half-way it has the base on its screen. It
    // draws power, and is dark while the grid cannot pay.
    spawn(&mut w, "aster_t3_power", 0, (1800, 8600));
    spawn(&mut w, "aster_t3_radar", 0, (8000, 8192));
    run(&mut w, 60);
    assert_eq!(target_of(&w, gun), factory);
}

#[test]
fn its_shells_scatter_round_the_mark() {
    let mut w = world(false);
    let gun = spawn(&mut w, CULVERIN, 0, GUN);
    // Tough enough to outlast the test, so every shell is aimed at the same mark.
    let target = spawn(&mut w, "aster_t4_nuke_silo", 1, BASE);
    let blueprint = w.state.units.blueprint[w.state.units.row(gun).unwrap()];
    let mark = FxVec2::from_ints(BASE.0, BASE.1);
    let mut misses = Vec::new();
    for _ in 0..4000 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::Impact {
                pos, blueprint: b, ..
            } = e
            {
                if *b == blueprint {
                    misses.push(pos.xy().distance(mark).to_f64());
                }
            }
        }
        if misses.len() >= 16 {
            break;
        }
    }
    assert!(
        w.state.units.row(target).is_some(),
        "the silo was to outlast the test"
    );
    assert!(misses.len() >= 16, "only {} shells landed", misses.len());
    misses.sort_by(f64::total_cmp);
    let mean = misses.iter().sum::<f64>() / misses.len() as f64;
    let worst = misses[misses.len() - 1];
    // One degree at 12 km: up to about 210 m wide and 210 m short or long.
    assert!(
        worst < 320.0,
        "a shell landed {worst:.0} m off at 12 km: {misses:?}"
    );
    assert!(
        mean > 60.0,
        "the gun is too true, {mean:.0} m off on average: {misses:?}"
    );
    assert!(
        misses[0] < 100.0,
        "none of its shells came near the mark: {misses:?}"
    );
}
