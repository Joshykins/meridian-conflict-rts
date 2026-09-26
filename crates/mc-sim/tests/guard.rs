//! The guard order: units hold a spot, go after what comes into the area, and walk back.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind, UnitId};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// 256 cells of 8 m of flat dry ground at 20 m.
fn field() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::from_samples(
        256,
        256,
        vec![60; 257 * 257],
        Fx::from_int(-40),
        Fx::ONE,
        Fx::from_int(5),
    );
    let map = MapData {
        name: "field".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
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
        seed: 5,
        players: vec![player("you", 0), player("other", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, owner: u8, key: &str, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

fn id(w: &World, row: usize) -> UnitId {
    w.state.units.id(row)
}

fn give(w: &mut World, player: u8, command: Command) {
    w.tick(&[PlayerCommand { player, command }]).unwrap();
}

fn run(w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        w.tick(&[]).unwrap();
    }
}
/// Runs until `done`, at most `limit` ticks; the ticks it took.
fn until(w: &mut World, limit: usize, done: impl Fn(&World) -> bool) -> Option<usize> {
    for t in 0..limit {
        if done(w) {
            return Some(t);
        }
        w.tick(&[]).unwrap();
    }
    done(w).then_some(limit)
}


#[test]
fn guards_hold_their_spot_chase_what_comes_in_and_walk_back() {
    let mut w = field();
    let tanks: Vec<usize> = (0..2)
        .map(|i| add(&mut w, 0, "aster_t1_tank", 600 + 20 * i, 600))
        .collect();
    let ids: Vec<UnitId> = tanks.iter().map(|&t| id(&w, t)).collect();
    let spot = FxVec2::from_ints(900, 600);
    let c = Command::Guard {
        units: ids,
        pos: spot,
        radius: Fx::from_int(300),
        queue: false,
    };
    give(&mut w, 0, c);
    run(&mut w, 400);
    for &t in &tanks {
        assert!(
            w.state.units.pos[t].distance(spot) < Fx::from_int(40),
            "holding the spot"
        );
    }
    let before: Vec<FxVec2> = tanks.iter().map(|&t| w.state.units.pos[t]).collect();
    // Outside the area and out of gun range: left alone.
    let far = add(&mut w, 1, "aster_t1_scout", 1500, 600);
    w.state.units.flags[far] |= flag::PASSIVE;
    run(&mut w, 100);
    for (&t, b) in tanks.iter().zip(&before) {
        assert!(
            w.state.units.pos[t].distance(*b) < Fx::from_int(4),
            "no chase outside the area"
        );
    }
    // Inside the area, beyond gun range: chased.
    w.state.units.pos[far] = FxVec2::from_ints(1150, 600);
    w.state.units.prev_pos[far] = w.state.units.pos[far];
    let far_id = id(&w, far);
    run(&mut w, 8);
    assert!(tanks.iter().any(|&t| {
        w.state
            .orders
            .front(&w.state.units, t)
            .is_some_and(|o| o.kind == OrderKind::Attack && o.target == far_id)
    }));
    assert!(
        until(&mut w, 1200, |w| w.state.units.row(far_id).is_none()).is_some(),
        "intruder killed"
    );
    run(&mut w, 600);
    for &t in &tanks {
        let front = w.state.orders.front(&w.state.units, t).copied().unwrap();
        assert_eq!(front.kind, OrderKind::Guard, "still guarding");
        assert!(
            w.state.units.pos[t].distance(spot) < Fx::from_int(40),
            "walked back"
        );
    }
}

