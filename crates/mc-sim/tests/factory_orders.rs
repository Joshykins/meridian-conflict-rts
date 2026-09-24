//! A factory takes orders for the units it will make: each one it rolls out is given
//! them as its own. A factory still being built takes production and orders too.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, OrderKind};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, mc_core::Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
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

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, key: &str, x: i32) -> PlayerCommand {
    spawn_at(w, key, FxVec2::from_ints(x, 512), Angle::ZERO)
}

fn spawn_at(w: &World, key: &str, pos: FxVec2, heading: Angle) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos,
        heading,
        count: 1,
        flags: 0,
        build: 1000,
    })
}

fn with_factory() -> (World, UnitId) {
    let mut w = world();
    w.tick(&[
        spawn(&w, "aster_t1_land_factory", 600),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let id = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory)
        .map(|r| w.state.units.id(r))
        .expect("the factory spawned");
    (w, id)
}

fn kinds(w: &World, row: usize) -> Vec<OrderKind> {
    w.state
        .orders
        .iter(&w.state.units, row)
        .map(|o| o.kind)
        .collect()
}

/// Ticks until a `key` unit has left the factory, and returns its row.
fn next_product(w: &mut World, key: &str, skip: &[usize]) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    for _ in 0..1500 {
        w.tick(&[]).unwrap();
        let found = w.state.units.slots.iter().find(|&r| {
            w.state.units.blueprint[r] == bp
                && !skip.contains(&r)
                && w.state.units.is_active(r)
                && !w.state.units.has_flag(r, flag::IN_FACTORY)
        });
        if let Some(r) = found {
            return r;
        }
    }
    panic!("no {key} came out");
}

fn produce(w: &World, fid: UnitId, key: &str, count: u8) -> PlayerCommand {
    cmd(Command::Produce {
        factories: vec![fid],
        blueprint: w.blueprints.id_of(key).unwrap(),
        count,
    })
}

#[test]
fn products_take_the_factorys_orders_in_order() {
    let (mut w, fid) = with_factory();
    let post_a = FxVec2::from_ints(900, 700);
    let post_b = FxVec2::from_ints(1000, 400);
    let first = FxVec2::from_ints(800, 800);
    w.tick(&[
        cmd(Command::Move {
            units: vec![fid],
            target: first,
            queue: false,
        }),
        cmd(Command::Patrol {
            units: vec![fid],
            points: vec![post_a, post_b],
            queue: true,
        }),
        produce(&w, fid, "aster_t1_tank", 2),
    ])
    .unwrap();
    let factory = w.state.units.row(fid).unwrap();
    assert_eq!(
        w.state.units.standing[factory].len(),
        2,
        "the factory keeps both"
    );
    assert_eq!(
        kinds(&w, factory),
        vec![OrderKind::Produce, OrderKind::Produce],
        "the factory itself is only given production"
    );

    let a = next_product(&mut w, "aster_t1_tank", &[]);
    assert_eq!(
        kinds(&w, a),
        vec![OrderKind::Move, OrderKind::Patrol, OrderKind::Patrol],
        "the first tank goes to the point, then patrols"
    );
    let at: Vec<FxVec2> = w
        .state
        .orders
        .iter(&w.state.units, a)
        .map(|o| o.pos)
        .collect();
    assert_eq!(at[1..], [post_a, post_b]);
    let b = next_product(&mut w, "aster_t1_tank", &[a]);
    assert_eq!(kinds(&w, b)[0], OrderKind::Move, "so does the second");

    // A plain order replaces them; Stop forgets them.
    w.tick(&[cmd(Command::AttackMove {
        units: vec![fid],
        target: post_b,
        queue: false,
    })])
    .unwrap();
    assert_eq!(w.state.units.standing[factory].len(), 1);
    w.tick(&[cmd(Command::Stop { units: vec![fid] })]).unwrap();
    assert!(w.state.units.standing[factory].is_empty());
}

#[test]
fn a_product_that_cannot_carry_the_order_out_goes_to_the_rally_point() {
    let (mut w, fid) = with_factory();
    // Only a builder can assist: a tank leaves by the default way out.
    w.tick(&[
        cmd(Command::Assist {
            units: vec![fid],
            target: fid,
            queue: false,
        }),
        produce(&w, fid, "aster_t1_tank", 1),
    ])
    .unwrap();
    let tank = next_product(&mut w, "aster_t1_tank", &[]);
    assert_eq!(kinds(&w, tank), vec![OrderKind::Move]);
    // An engineer guards the factory it came from.
    w.tick(&[produce(&w, fid, "aster_t1_engineer", 1)]).unwrap();
    let eng = next_product(&mut w, "aster_t1_engineer", &[]);
    let first = *w.state.orders.front(&w.state.units, eng).unwrap();
    assert_eq!((first.kind, first.target), (OrderKind::Assist, fid));
}

#[test]
fn a_factory_under_construction_takes_production_and_orders() {
    let mut w = world();
    w.tick(&[
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of("aster_t1_land_factory").unwrap(),
            pos: FxVec2::from_ints(600, 512),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 200,
        }),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == factory)
        .unwrap();
    let fid = w.state.units.id(row);
    assert!(w.state.units.has_flag(row, flag::UNDER_CONSTRUCTION));
    let out = FxVec2::from_ints(900, 900);
    w.tick(&[
        produce(&w, fid, "aster_t1_tank", 1),
        cmd(Command::Move {
            units: vec![fid],
            target: out,
            queue: false,
        }),
    ])
    .unwrap();
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(kinds(&w, row), vec![OrderKind::Produce], "the queue waits");
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    assert!(
        !w.state
            .units
            .slots
            .iter()
            .any(|r| w.state.units.blueprint[r] == tank),
        "nothing is made while the factory is going up"
    );

    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![fid],
        permille: 1000,
    })])
    .unwrap();
    let t = next_product(&mut w, "aster_t1_tank", &[]);
    let o = *w.state.orders.front(&w.state.units, t).unwrap();
    assert_eq!(o.kind, OrderKind::Move);
    assert!(
        o.pos.distance(out) < Fx::from_int(20),
        "it went where it was told: {:?}",
        o.pos
    );
}

#[test]
fn standing_posts_can_be_moved_added_and_dropped() {
    let (mut w, fid) = with_factory();
    let (a, b) = (FxVec2::from_ints(900, 700), FxVec2::from_ints(1000, 400));
    w.tick(&[cmd(Command::Patrol {
        units: vec![fid],
        points: vec![a, b],
        queue: false,
    })])
    .unwrap();
    let row = w.state.units.row(fid).unwrap();
    let posts = |w: &World| match &w.state.units.standing[row][..] {
        [Command::Patrol { points, .. }] => points.clone(),
        other => panic!("{other:?}"),
    };
    let c = FxVec2::from_ints(950, 600);
    w.tick(&[cmd(Command::PatrolInsert {
        units: vec![fid],
        after: a,
        point: c,
    })])
    .unwrap();
    assert_eq!(posts(&w), vec![a, c, b]);
    let d = FxVec2::from_ints(700, 700);
    w.tick(&[cmd(Command::RelocateOrder {
        units: vec![fid],
        kind: OrderKind::Patrol,
        from: c,
        to: d,
    })])
    .unwrap();
    assert_eq!(posts(&w), vec![a, d, b]);
    w.tick(&[cmd(Command::CancelOrder {
        units: vec![fid],
        kind: OrderKind::Patrol,
        pos: a,
    })])
    .unwrap();
    assert_eq!(posts(&w), vec![d, b]);
}

#[test]
fn a_factory_copies_another_factorys_orders_in_place_of_its_own() {
    let (mut w, a) = with_factory();
    w.tick(&[spawn(&w, "aster_t1_land_factory", 800)]).unwrap();
    let bp = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let b = w
        .state
        .units
        .slots
        .iter()
        .map(|r| w.state.units.id(r))
        .find(|&id| {
            id != a
                && w.state
                    .units
                    .row(id)
                    .is_some_and(|r| w.state.units.blueprint[r] == bp)
        })
        .expect("the second factory spawned");
    let (p, q, r) = (
        FxVec2::from_ints(900, 700),
        FxVec2::from_ints(1000, 400),
        FxVec2::from_ints(700, 900),
    );
    w.tick(&[
        cmd(Command::Patrol {
            units: vec![a],
            points: vec![r],
            queue: false,
        }),
        cmd(Command::Patrol {
            units: vec![b],
            points: vec![p, q],
            queue: false,
        }),
        cmd(Command::SetRally {
            factories: vec![b],
            pos: q,
        }),
        cmd(Command::Patrol {
            units: vec![b],
            points: vec![p, q],
            queue: false,
        }),
    ])
    .unwrap();
    // Twice: copying again still leaves the one patrol.
    for _ in 0..2 {
        w.tick(&[cmd(Command::CopyFactoryOrders {
            factories: vec![a],
            from: b,
        })])
        .unwrap();
    }
    let (ra, rb) = (w.state.units.row(a).unwrap(), w.state.units.row(b).unwrap());
    assert_eq!(w.state.units.standing[ra], w.state.units.standing[rb]);
    assert_eq!(
        w.state.units.standing[ra].len(),
        1,
        "one patrol, a's own is gone"
    );
    assert_eq!(w.state.units.rally[ra], w.state.units.rally[rb]);

    w.tick(&[produce(&w, a, "aster_t1_tank", 1)]).unwrap();
    let tank = next_product(&mut w, "aster_t1_tank", &[]);
    let at: Vec<FxVec2> = w
        .state
        .orders
        .iter(&w.state.units, tank)
        .map(|o| o.pos)
        .collect();
    assert_eq!(kinds(&w, tank), vec![OrderKind::Patrol, OrderKind::Patrol]);
    assert_eq!(at, [p, q]);
}
