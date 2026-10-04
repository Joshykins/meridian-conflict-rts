//! A unit shut in among structures walks out through them. Replay 20261004-172909
//! mark 2: a Regency engineer built a column of four fabricators between two T3
//! power generators while standing on the fabricators' lots, and the last hull
//! shut it into an 8 m strip of apron between two of them, where it sat for the
//! rest of the match.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// The replay's layout, moved by whole 24 m steps so it sits on the path and
/// build grids the same way: (key, x, y).
const LAYOUT: [(&str, i32, i32); 6] = [
    ("regency_t3_power", 996, 1008),
    ("regency_t3_fabricator", 1056, 972),
    ("regency_t3_fabricator", 1056, 996),
    ("regency_t3_fabricator", 1056, 1020),
    ("regency_t3_fabricator", 1056, 1044),
    ("regency_t3_power", 1116, 1008),
];
/// Where the engineer was left, between the fabricators at y 972 and 996.
const TRAPPED: (i32, i32) = (1042, 982);
const ENGINEER: &str = "regency_t3_engineer";
const NORTH: Angle = Angle(0x4000);

fn flat() -> World {
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![40u16; 257 * 257],
        Fx::ZERO,
        Fx::ONE,
        Fx::from_int(20),
    );
    let map = MapData {
        name: "flat".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(150, 300), FxVec2::from_ints(150, 1700)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Regency".into(),
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

fn standable(w: &World, pos: FxVec2) -> bool {
    let bp = w.blueprints.id_of(ENGINEER).unwrap();
    let m = w.blueprints.unit(bp).motion.unwrap();
    assert_eq!(m.layer, MoveLayer::Hover);
    w.nav.passable(m.layer, m.size_class, pos)
}

#[test]
fn an_engineer_shut_in_by_the_structures_round_it_walks_out() {
    let mut w = flat();
    let engineer = w.blueprints.id_of(ENGINEER).unwrap();
    let start = FxVec2::from_ints(TRAPPED.0, TRAPPED.1);
    let row = w.spawn_unit(engineer, 0, start, NORTH, true).unwrap();
    let id = w.state.units.id(row);
    assert!(standable(&w, start), "open ground before anything is built");
    for (key, x, y) in LAYOUT {
        let bp = w.blueprints.id_of(key).unwrap();
        w.spawn_unit(bp, 0, FxVec2::from_ints(x, y), NORTH, true)
            .unwrap();
    }
    assert!(w.nav.pocket_cells() > 0, "the layout leaves pockets");
    assert!(
        !standable(&w, start),
        "the engineer's patch is a pocket, not ground to stand on"
    );
    let mut pos = start;
    for _ in 0..300 {
        w.tick(&[]).unwrap();
        pos = w.state.units.pos[w.state.units.row(id).unwrap()];
        if standable(&w, pos) {
            break;
        }
    }
    assert!(standable(&w, pos), "it never got out: {pos:?}");
    assert!(
        pos.distance(start) < Fx::from_int(60),
        "it took the short way out: {start:?} -> {pos:?}"
    );
}

#[test]
fn a_pocket_opens_again_when_the_structures_round_it_go() {
    let mut w = flat();
    let start = FxVec2::from_ints(TRAPPED.0, TRAPPED.1);
    let mut built = Vec::new();
    for (key, x, y) in LAYOUT {
        let bp = w.blueprints.id_of(key).unwrap();
        let row = w
            .spawn_unit(bp, 0, FxVec2::from_ints(x, y), NORTH, true)
            .unwrap();
        built.push(w.state.units.id(row));
    }
    assert!(!standable(&w, start));
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::DebugRemove { units: built },
    }])
    .unwrap();
    assert_eq!(
        w.nav.pocket_cells(),
        0,
        "no pockets once the structures are gone"
    );
    assert!(standable(&w, start));
}
