//! A factory's batch: its products form up outside it and wait for the rest of the
//! queue, then leave together on the factory's orders (`batch.rs`).

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

fn batch_of(w: &World, fid: UnitId) -> Option<&mc_sim::batch::Batch> {
    w.state.batches.get(&fid)
}

/// Where `row` is headed: its front order's position, or where it stands.
fn goal(w: &World, row: usize) -> FxVec2 {
    w.state
        .orders
        .front(&w.state.units, row)
        .map_or(w.state.units.pos[row], |o| o.pos)
}

/// A repeating factory with a standing move to `post` and batch on, three tanks a lap.
fn batching(post: FxVec2) -> (World, UnitId) {
    let (mut w, fid) = with_factory();
    w.tick(&[
        cmd(Command::Move {
            units: vec![fid],
            target: post,
            queue: false,
        }),
        cmd(Command::SetRepeat {
            factories: vec![fid],
            repeat: true,
        }),
        cmd(Command::SetBatch {
            factories: vec![fid],
            batch: true,
        }),
        produce(&w, fid, "aster_t1_tank", 3),
    ])
    .unwrap();
    (w, fid)
}

#[test]
fn a_batch_forms_up_outside_and_leaves_together_once_the_lap_is_done() {
    let post = FxVec2::from_ints(1200, 900);
    let (mut w, fid) = batching(post);
    let factory = w.state.units.row(fid).unwrap();
    let first = out_tanks_after(&mut w, &[], 2);
    for &t in &first {
        let at = goal(&w, t);
        assert!(
            at.distance(post) > Fx::from_int(300),
            "a tank waits by the factory, not at the post"
        );
        assert!(
            at.distance(w.state.units.pos[factory]) < Fx::from_int(120),
            "its place is just outside the factory"
        );
    }
    assert_ne!(
        goal(&w, first[0]),
        goal(&w, first[1]),
        "each has its own place"
    );
    assert_eq!(batch_of(&w, fid).unwrap().held.len(), 2);

    let third = next_product(&mut w, "aster_t1_tank", &first);
    // The lap is done: all three are sent to the post, one group.
    let all = [first[0], first[1], third];
    for &t in &all {
        assert!(
            goal(&w, t).distance(post) < Fx::from_int(60),
            "every tank of the batch heads for the post"
        );
    }
    let formation = |t: usize| w.state.orders.front(&w.state.units, t).unwrap().formation;
    assert_ne!(formation(all[0]), 0, "they go as a formation");
    assert!(all.iter().all(|&t| formation(t) == formation(all[0])));
    let b = batch_of(&w, fid).unwrap();
    assert!(
        b.held.is_empty() && b.made == 0,
        "the next lap starts empty"
    );

    // The next lap forms up again.
    let fourth = next_product(&mut w, "aster_t1_tank", &all);
    assert!(goal(&w, fourth).distance(post) > Fx::from_int(300));
}

#[test]
fn a_batch_can_be_sent_early_and_turning_batch_off_sends_it_too() {
    let post = FxVec2::from_ints(1200, 900);
    let (mut w, fid) = batching(post);
    let first = next_product(&mut w, "aster_t1_tank", &[]);
    w.tick(&[cmd(Command::ReleaseBatch {
        factories: vec![fid],
    })])
    .unwrap();
    assert!(
        goal(&w, first).distance(post) < Fx::from_int(60),
        "sent now"
    );
    assert!(batch_of(&w, fid).is_some(), "batch stays on");

    let second = next_product(&mut w, "aster_t1_tank", &[first]);
    assert!(goal(&w, second).distance(post) > Fx::from_int(300));
    w.tick(&[cmd(Command::SetBatch {
        factories: vec![fid],
        batch: false,
    })])
    .unwrap();
    assert!(goal(&w, second).distance(post) < Fx::from_int(60));
    assert!(batch_of(&w, fid).is_none());
    // Off: each product leaves on its own again.
    let third = next_product(&mut w, "aster_t1_tank", &[first, second]);
    assert!(goal(&w, third).distance(post) < Fx::from_int(60));
}

#[test]
fn a_unit_ordered_away_leaves_the_batch() {
    let post = FxVec2::from_ints(1200, 900);
    let (mut w, fid) = batching(post);
    let first = next_product(&mut w, "aster_t1_tank", &[]);
    let elsewhere = FxVec2::from_ints(300, 300);
    let id = w.state.units.id(first);
    w.tick(&[cmd(Command::Move {
        units: vec![id],
        target: elsewhere,
        queue: false,
    })])
    .unwrap();
    assert!(batch_of(&w, fid).unwrap().held.is_empty());
    out_tanks_after(&mut w, &[first], 2);
    assert!(
        goal(&w, first).distance(elsewhere) < Fx::from_int(40),
        "the lap's release leaves it on its own order"
    );
}

/// Ticks until `n` more tanks than `skip` are out.
fn out_tanks_after(w: &mut World, skip: &[usize], n: usize) -> Vec<usize> {
    let mut rows = skip.to_vec();
    for _ in 0..n {
        let r = next_product(w, "aster_t1_tank", &rows);
        rows.push(r);
    }
    rows
}

#[test]
fn a_plain_queue_is_one_batch_and_leaves_for_the_rally_point_without_orders() {
    let (mut w, fid) = with_factory();
    let rally = FxVec2::from_ints(1000, 300);
    w.tick(&[
        cmd(Command::SetRally {
            factories: vec![fid],
            pos: rally,
        }),
        cmd(Command::SetBatch {
            factories: vec![fid],
            batch: true,
        }),
        produce(&w, fid, "aster_t1_tank", 2),
    ])
    .unwrap();
    let first = next_product(&mut w, "aster_t1_tank", &[]);
    assert!(goal(&w, first).distance(rally) > Fx::from_int(300));
    let second = next_product(&mut w, "aster_t1_tank", &[first]);
    for t in [first, second] {
        assert!(goal(&w, t).distance(rally) < Fx::from_int(60));
    }
}
