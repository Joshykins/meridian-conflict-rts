//! Moves across open ground at any bearing drive a straight line. The flow
//! field only knows eight directions; on open ground an axis step and a
//! diagonal one often cost the same, and a unit that took whichever one the
//! tie-break picked cell by cell drove along an axis, then wiggled along the
//! seam where the pick flipped.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "straight".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(200, 200), FxVec2::from_ints(1800, 1800)],
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
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Goal points 1500 m out at bearings between an axis and the diagonal,
/// given as whole-metre offsets (sin/cos of 8, 15, 22, 30 and 38 degrees).
const OFFSETS: [(i32, i32); 5] = [
    (1485, 209),
    (1449, 388),
    (1391, 562),
    (1299, 750),
    (1182, 923),
];

/// Worst heading error off the start-to-goal line over the trip, in
/// 1/65536 turns, ignoring the last 200 m where arrival takes over.
fn worst_heading(w: &mut World, ids: &[UnitId], goal: FxVec2, command: Command) -> i32 {
    let rows: Vec<usize> = ids
        .iter()
        .map(|&id| w.state.units.row(id).unwrap())
        .collect();
    let aim = (goal - w.state.units.pos[rows[0]]).angle();
    w.tick(&[PlayerCommand { player: 0, command }]).unwrap();
    let mut worst = 0;
    for tick in 0..1500 {
        w.tick(&[]).unwrap();
        if rows
            .iter()
            .any(|&r| (w.state.units.pos[r] - goal).length() < Fx::from_int(200))
        {
            return worst;
        }
        // The first second is spent turning and closing up.
        if tick >= 30 {
            for &r in &rows {
                worst = worst.max((w.state.units.heading[r].delta_to(aim) as i32).abs());
            }
        }
    }
    panic!("never got within 200 m of {goal:?}");
}

fn spawn(w: &mut World, points: &[FxVec2], heading: Angle) -> Vec<UnitId> {
    let bp = w.blueprints.id_of("aster_t1_tank").unwrap();
    let commands: Vec<_> = points
        .iter()
        .map(|&pos| PlayerCommand {
            player: 0,
            command: Command::DebugSpawn {
                owner: 0,
                blueprint: bp,
                pos,
                heading,
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            },
        })
        .collect();
    w.tick(&commands).unwrap();
    w.state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .collect()
}

#[test]
fn off_diagonal_moves_hold_their_heading() {
    for (dx, dy) in OFFSETS {
        let mut w = world();
        let start = FxVec2::from_ints(150, 150);
        let goal = start + FxVec2::from_ints(dx, dy);
        let ids = spawn(&mut w, &[start], (goal - start).angle());
        let move_to = Command::Move {
            units: ids.clone(),
            target: goal,
            queue: false,
        };
        let worst = worst_heading(&mut w, &ids, goal, move_to);
        // 2 degrees; the wiggle swung up to 30.
        assert!(
            worst < 364,
            "towards {dx},{dy}: heading off by {worst}/65536 turn"
        );
    }
}
