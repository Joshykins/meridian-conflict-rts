//! Patrol in formation: a group walks its loop as one block, and a post laid while
//! the patrol runs joins the loop where it was laid.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, OrderKind};
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
        name: "patrol".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(256, 256), FxVec2::from_ints(1800, 1800)],
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
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn order(command: Command) -> Vec<PlayerCommand> {
    vec![PlayerCommand { player: 0, command }]
}

fn add(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    w.state.units.id(row)
}

fn pos(w: &World, id: UnitId) -> FxVec2 {
    w.state.units.pos[w.state.units.row(id).expect("alive")]
}

fn posts(w: &World, id: UnitId) -> Vec<FxVec2> {
    let row = w.state.units.row(id).unwrap();
    w.state
        .orders
        .iter(&w.state.units, row)
        .filter(|o| o.kind == OrderKind::Patrol)
        .map(|o| o.pos)
        .collect()
}

fn centre(w: &World, ids: &[UnitId]) -> FxVec2 {
    let sum = ids.iter().fold(FxVec2::ZERO, |s, &id| s + pos(w, id));
    FxVec2::new(sum.x / ids.len() as i32, sum.y / ids.len() as i32)
}

fn spread(w: &World, ids: &[UnitId]) -> Fx {
    let c = centre(w, ids);
    ids.iter().map(|&id| pos(w, id).distance(c)).max().unwrap()
}

#[test]
fn a_group_patrols_in_formation_and_shares_its_legs() {
    let mut w = world();
    let tanks: Vec<UnitId> = (0..5)
        .map(|i| add(&mut w, "aster_t1_tank", 400 + i * 14, 400))
        .collect();
    let far = FxVec2::from_ints(640, 400);
    w.tick(&order(Command::Patrol {
        units: tanks.clone(),
        points: vec![far],
        queue: false,
    }))
    .unwrap();
    // One formation per leg, the same for every member.
    let legs = |w: &World, id: UnitId| {
        let row = w.state.units.row(id).unwrap();
        w.state
            .orders
            .iter(&w.state.units, row)
            .map(|o| o.formation)
            .collect::<Vec<_>>()
    };
    let first = legs(&w, tanks[0]);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|&f| f != 0));
    for &t in &tanks {
        assert_eq!(legs(&w, t), first, "members share their legs");
    }
    let start = centre(&w, &tanks);
    let (mut out, mut back, mut widest) = (false, false, Fx::ZERO);
    for tick in 0..1600 {
        w.tick(&[]).unwrap();
        let c = centre(&w, &tanks);
        out |= c.distance(far) < Fx::from_int(12);
        back |= out && c.distance(start) < Fx::from_int(12);
        if tick > 150 {
            widest = widest.max(spread(&w, &tanks));
        }
        // Every member is on the same leg, give or take the tick they all leave a post.
        let fronts: Vec<FxVec2> = tanks.iter().map(|&t| posts(&w, t)[0]).collect();
        if tick % 10 == 0 {
            assert!(
                fronts.windows(2).all(|p| p[0] == p[1]),
                "tick {tick}: legs split {fronts:?}"
            );
        }
    }
    assert!(out && back, "went {out}, came back {back}");
    assert!(widest < Fx::from_int(45), "the block came apart: {widest}");
}

#[test]
fn a_post_laid_later_joins_the_loop_after_the_last_one() {
    let mut w = world();
    let tanks: Vec<UnitId> = (0..3)
        .map(|i| add(&mut w, "aster_t1_tank", 400 + i * 14, 400))
        .collect();
    let a = FxVec2::from_ints(560, 400);
    w.tick(&order(Command::Patrol {
        units: tanks.clone(),
        points: vec![a],
        queue: false,
    }))
    .unwrap();
    let home = posts(&w, tanks[0])[1];
    let b = FxVec2::from_ints(560, 520);
    let c = FxVec2::from_ints(440, 560);
    w.tick(&order(Command::PatrolInsert {
        units: tanks.clone(),
        after: a,
        point: b,
    }))
    .unwrap();
    w.tick(&order(Command::PatrolInsert {
        units: tanks.clone(),
        after: b,
        point: c,
    }))
    .unwrap();
    for &t in &tanks {
        assert_eq!(posts(&w, t), vec![a, b, c, home]);
    }
    let mut seen = Vec::new();
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let front = posts(&w, tanks[0])[0];
        if seen.last() != Some(&front) {
            seen.push(front);
        }
    }
    assert!(seen.len() >= 6, "the loop stalled: {seen:?}");
    let order_of = |p: FxVec2| [a, b, c, home].iter().position(|q| *q == p).unwrap();
    for pair in seen.windows(2) {
        assert_eq!((order_of(pair[0]) + 1) % 4, order_of(pair[1]), "{seen:?}");
    }
}

#[test]
fn a_queued_patrol_starts_where_the_move_ends() {
    let mut w = world();
    let tank = add(&mut w, "aster_t1_tank", 400, 400);
    let there = FxVec2::from_ints(520, 400);
    w.tick(&order(Command::Move {
        units: vec![tank],
        target: there,
        queue: false,
    }))
    .unwrap();
    let a = FxVec2::from_ints(520, 520);
    w.tick(&order(Command::Patrol {
        units: vec![tank],
        points: vec![a],
        queue: true,
    }))
    .unwrap();
    assert_eq!(posts(&w, tank), vec![a, there]);
}

#[test]
fn a_flight_patrols_together() {
    let mut w = world();
    let planes: Vec<UnitId> = (0..5)
        .map(|i| add(&mut w, "aster_t1_interceptor", 400 + i * 20, 400))
        .collect();
    let a = FxVec2::from_ints(900, 400);
    let b = FxVec2::from_ints(900, 900);
    w.tick(&order(Command::Patrol {
        units: planes.clone(),
        points: vec![a, b],
        queue: false,
    }))
    .unwrap();
    let mut legs = 0;
    let mut last = FxVec2::ZERO;
    for _ in 0..2500 {
        w.tick(&[]).unwrap();
        let front = posts(&w, planes[0])[0];
        if front != last {
            legs += 1;
            last = front;
        }
    }
    assert!(legs >= 4, "the flight flew only {legs} legs");
}
