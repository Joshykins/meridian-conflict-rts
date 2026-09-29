//! A ship put down where it cannot float (replay 20260929-190223 marks 6-7: a
//! range spawn of three Leviathans left one on a headland, and it drove on along
//! its heading, 900 m over the land, into a landlocked pond) makes for the
//! nearest water its hull fits, and never crosses land on the way.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

const WATER: i32 = 20;
/// Sample columns (8 m apart) from here east are open sea.
const SEA_FROM: usize = 145;
/// A landlocked pond, deep enough for any hull, west of the headland.
const POND_X: (usize, usize) = (10, 60);
const POND_Y: (usize, usize) = (90, 170);
/// The headland the ships are put down on: 17 samples from the sea, 68 from the pond.
const SPAWN: (i32, i32) = (128 * 8, 130 * 8);
/// Facing west, at the pond.
const WEST: Angle = Angle(0x8000);
/// Where the range spawn is aimed: its block of three lands 60 m and 210 m from the sea.
const RANGE_SPAWN: (i32, i32) = (1100, 130 * 8);

/// Land at 40 m, the sea and the pond at 0 under 20 m of water, and `shallow`
/// samples round the spawn at 17 m: a puddle no ship floats in.
fn headland(shallow: bool) -> World {
    let mut samples = vec![40u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            let pond = (POND_X.0..POND_X.1).contains(&x) && (POND_Y.0..POND_Y.1).contains(&y);
            let puddle = shallow && (120..137).contains(&x) && (122..139).contains(&y);
            samples[y * 257 + x] = if x >= SEA_FROM || pond {
                0
            } else if puddle {
                17
            } else {
                40
            };
        }
    }
    let terrain =
        Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(WATER));
    let map = MapData {
        name: "headland".into(),
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
        seed: 7,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn size_class(w: &World, key: &str) -> u8 {
    let bp = w.blueprints.id_of(key).unwrap();
    w.blueprints.unit(bp).motion.unwrap().size_class
}

fn afloat(w: &World, key: &str, pos: FxVec2) -> bool {
    w.nav.passable(MoveLayer::Naval, size_class(w, key), pos)
}

fn in_pond(pos: FxVec2) -> bool {
    pos.x < Fx::from_int(POND_X.1 as i32 * 8 + 8)
}

/// Runs until every ship is afloat (or `ticks` run out), checking each tick that
/// no ship moved away from the sea or ever reached the pond.
fn sail_out(w: &mut World, key: &str, ships: &[UnitId], ticks: u32) {
    let mut last: Vec<FxVec2> = ships
        .iter()
        .map(|&s| w.state.units.pos[w.state.units.row(s).unwrap()])
        .collect();
    for t in 0..ticks {
        w.tick(&[]).unwrap();
        let mut all_afloat = true;
        for (i, &s) in ships.iter().enumerate() {
            let pos = w.state.units.pos[w.state.units.row(s).unwrap()];
            assert!(
                !in_pond(pos),
                "tick {t}: ship {i} went for the pond: {pos:?}"
            );
            if !afloat(w, key, last[i]) {
                assert!(
                    pos.x >= last[i].x,
                    "tick {t}: ship {i} moved further over the land: {:?} -> {pos:?}",
                    last[i]
                );
            }
            all_afloat &= afloat(w, key, pos);
            last[i] = pos;
        }
        if all_afloat {
            return;
        }
    }
    panic!("the ships never reached the sea: {last:?}");
}

fn stranded_ship_takes_the_nearest_water(key: &str, shallow: bool) {
    let mut w = headland(shallow);
    let bp = w.blueprints.id_of(key).unwrap();
    let spawn = FxVec2::from_ints(SPAWN.0, SPAWN.1);
    let row = w.spawn_unit(bp, 0, spawn, WEST, true).unwrap();
    w.state.units.flags[row] |= flag::PASSIVE;
    assert!(!afloat(&w, key, spawn), "it starts where it cannot float");
    let ship = w.state.units.id(row);
    sail_out(&mut w, key, &[ship], 1500);
    let pos = w.state.units.pos[w.state.units.row(ship).unwrap()];
    assert!(
        pos.x > spawn.x && (pos.y - spawn.y).abs() < Fx::from_int(24),
        "it went straight east to the sea: {pos:?}"
    );
    // Afloat, it coasts to a stop and stays afloat, clear of the land.
    for _ in 0..100 {
        w.tick(&[]).unwrap();
    }
    let after = w.state.units.pos[w.state.units.row(ship).unwrap()];
    assert!(
        afloat(&w, key, after) && after.x >= pos.x && after.distance(pos) < Fx::from_int(60),
        "{pos:?} -> {after:?}"
    );
}

#[test]
fn a_battleship_on_a_headland_takes_the_sea_beside_it_not_the_pond_ahead() {
    stranded_ship_takes_the_nearest_water("aster_t3_battleship", false);
}

#[test]
fn a_battleship_in_a_puddle_takes_the_sea_beside_it_not_the_pond_ahead() {
    stranded_ship_takes_the_nearest_water("aster_t3_battleship", true);
}

#[test]
fn a_small_boat_on_a_headland_takes_the_nearest_water_too() {
    stranded_ship_takes_the_nearest_water("aster_t1_frigate", false);
}

/// The reported case: the test range puts a block of three down on the headland
/// (a square block, `count` 3 fills three of its four places, west and north of `pos`).
#[test]
fn a_range_spawn_off_the_water_puts_ships_on_the_nearest_water() {
    let key = "aster_t3_battleship";
    let mut w = headland(false);
    let before = w.state.units.slots.live();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(RANGE_SPAWN.0, RANGE_SPAWN.1),
            heading: WEST,
            count: 3,
            flags: flag::PASSIVE,
            build: 1000,
        },
    }])
    .unwrap();
    let units = &w.state.units;
    let ships: Vec<UnitId> = (0..units.slots.rows())
        .filter(|&r| units.slots.is_alive(r))
        .map(|r| units.id(r))
        .collect();
    assert_eq!(ships.len(), before + 3);
    for &s in &ships {
        let pos = w.state.units.pos[w.state.units.row(s).unwrap()];
        assert!(
            afloat(&w, key, pos) && pos.x >= Fx::from_int(SEA_FROM as i32 * 8 - 16),
            "put down on the nearest water, the sea: {pos:?}"
        );
    }
    sail_out(&mut w, key, &ships, 600);
}
