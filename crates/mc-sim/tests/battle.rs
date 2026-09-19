//! End-to-end simulation tests on a flat synthetic map: a battle, a build
//! chain, snapshots, and above all determinism across thread counts.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const MAP_CELLS: u32 = 512; // 4 km

fn blueprints() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

fn flat_world(threads: usize, commanders: bool, ai: bool) -> World {
    let terrain = Heightfield::flat(MAP_CELLS, MAP_CELLS, Fx::from_int(20));
    let map = MapData {
        name: "flat".into(),
        content_id: 1,
        deposits: vec![
            FxVec2::from_ints(640, 512), FxVec2::from_ints(512, 640), FxVec2::from_ints(704, 704), FxVec2::from_ints(400, 400),
            FxVec2::from_ints(3456, 3584), FxVec2::from_ints(3584, 3456), FxVec2::from_ints(3392, 3392), FxVec2::from_ints(3696, 3696),
        ],
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(3584, 3584)],
        props: Vec::new(),
    };
    let controller = if ai { Controller::Ai } else { Controller::Human };
    let config = MatchConfig {
        seed: 42,
        players: vec![
            PlayerSetup { name: "one".into(), faction: "Aster".into(), team: 0, controller, start: 0 },
            PlayerSetup { name: "two".into(), faction: "Aster".into(), team: 1, controller, start: 1 },
        ],
        cheats: true,
        fog: true,
        spawn_commanders: commanders,
    };
    World::with_terrain(terrain, map, blueprints(), Arc::new(Pool::new(threads)), &config).unwrap()
}

fn cmd(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

/// Two armies spawn, march at each other and fight. Returns the hash of every tick.
fn run_battle(threads: usize, ticks: u32) -> (Vec<u64>, World) {
    let mut w = flat_world(threads, false, false);
    let bp = w.blueprints.clone();
    let tank = bp.id_of("aster_t1_tank").unwrap();
    let arty = bp.id_of("aster_t1_artillery").unwrap();
    let bot = bp.id_of("aster_t1_bot").unwrap();
    let mut hashes = Vec::new();
    let setup = vec![
        cmd(0, Command::DebugSpawn { owner: 0, blueprint: tank, pos: FxVec2::from_ints(1500, 2000), heading: Angle::ZERO, count: 60 }),
        cmd(0, Command::DebugSpawn { owner: 0, blueprint: arty, pos: FxVec2::from_ints(1350, 2000), heading: Angle::ZERO, count: 16 }),
        cmd(1, Command::DebugSpawn { owner: 1, blueprint: tank, pos: FxVec2::from_ints(2500, 2050), heading: Angle::HALF_TURN, count: 50 }),
        cmd(1, Command::DebugSpawn { owner: 1, blueprint: bot, pos: FxVec2::from_ints(2650, 2050), heading: Angle::HALF_TURN, count: 40 }),
    ];
    hashes.push(w.tick(&setup).unwrap());
    let ids = |w: &World, owner: u8| -> Vec<_> {
        let u = &w.state.units;
        u.slots.iter().filter(|&r| u.owner[r] == owner).map(|r| u.id(r)).collect()
    };
    let orders = vec![
        cmd(0, Command::AttackMove { units: ids(&w, 0), target: FxVec2::from_ints(2600, 2050), queue: false }),
        cmd(1, Command::AttackMove { units: ids(&w, 1), target: FxVec2::from_ints(1400, 2000), queue: false }),
    ];
    hashes.push(w.tick(&orders).unwrap());
    for _ in 2..ticks {
        hashes.push(w.tick(&[]).unwrap());
    }
    (hashes, w)
}

#[test]
fn battle_is_fought_and_leaves_wreckage() {
    let (_, w) = run_battle(4, 900);
    let s = &w.state;
    let alive = |owner: u8| s.units.slots.iter().filter(|&r| s.units.owner[r] == owner).count();
    let (a, b) = (alive(0), alive(1));
    assert!(a + b < 166, "nobody died: {a} + {b}");
    assert!(s.wrecks.slots.live() > 10, "wrecks: {}", s.wrecks.slots.live());
    assert!(!s.stains.is_empty(), "no scorch marks");
    assert!(s.players[0].units_killed + s.players[1].units_killed > 10);
    // Somebody should have won the field.
    assert!(a == 0 || b == 0 || a + b < 100, "battle stalled at {a} vs {b}");
}

#[test]
fn identical_hashes_at_any_thread_count() {
    let (reference, _) = run_battle(0, 400);
    for threads in [1, 3, 8] {
        let (hashes, _) = run_battle(threads, 400);
        let first_diff = reference.iter().zip(&hashes).position(|(a, b)| a != b);
        assert_eq!(first_diff, None, "desync with {threads} threads at tick {first_diff:?}");
    }
}

#[test]
fn snapshot_restores_to_the_same_future() {
    let (_, mut a) = run_battle(2, 150);
    let blob = a.snapshot();
    let mut b = flat_world(2, false, false);
    b.restore(Heightfield::flat(MAP_CELLS, MAP_CELLS, Fx::from_int(20)), &blob).unwrap();
    assert_eq!(a.hash(), b.hash());
    for t in 0..200 {
        assert_eq!(a.tick(&[]).unwrap(), b.tick(&[]).unwrap(), "diverged {t} ticks after the snapshot");
    }
}

#[test]
fn commander_builds_a_base_and_a_factory_builds_tanks() {
    let mut w = flat_world(2, true, false);
    let bp = w.blueprints.clone();
    let acu = w.state.players[0].commander;
    let power = bp.id_of("aster_t1_power").unwrap();
    let extractor = bp.id_of("aster_t1_extractor").unwrap();
    let factory = bp.id_of("aster_t1_land_factory").unwrap();
    let tank = bp.id_of("aster_t1_tank").unwrap();
    let build = |blueprint, x, y, queue| cmd(0, Command::Build { units: vec![acu], blueprint, pos: FxVec2::from_ints(x, y), heading: Angle::ZERO, queue });
    w.tick(&[build(power, 600, 440, false), build(power, 650, 440, true), build(extractor, 640, 512, true), build(factory, 420, 640, true)]).unwrap();

    let find = |w: &World, blueprint| {
        let u = &w.state.units;
        u.slots.iter().find(|&r| u.blueprint[r] == blueprint && u.owner[r] == 0 && u.is_active(r))
    };
    let mut factory_row = None;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        factory_row = find(&w, factory);
        if factory_row.is_some() {
            break;
        }
    }
    let factory_row = factory_row.expect("factory finished within five minutes");
    assert!(find(&w, power).is_some() && find(&w, extractor).is_some());
    assert!(w.state.players[0].mass_income > Fx::from_int(2), "extractor income: {:?}", w.state.players[0].mass_income);
    // The ground under the factory is level now.
    let p = w.state.units.pos[factory_row];
    let z = w.terrain.height_at(p);
    assert_eq!(w.terrain.height_at(p + FxVec2::from_ints(40, -40)), z);

    let fid = w.state.units.id(factory_row);
    w.tick(&[cmd(0, Command::Produce { factories: vec![fid], blueprint: tank, count: 2 }), cmd(0, Command::SetRally { factories: vec![fid], pos: FxVec2::from_ints(700, 900) })]).unwrap();
    let mut tanks = 0;
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        tanks = u.slots.iter().filter(|&r| u.blueprint[r] == tank && u.is_active(r) && u.flags[r] & flag::IN_FACTORY == 0).count();
        if tanks == 2 {
            break;
        }
    }
    assert_eq!(tanks, 2, "factory output");
    // They drive to the rally point.
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let u = &w.state.units;
    let near_rally = u.slots.iter().filter(|&r| u.blueprint[r] == tank && u.pos[r].distance(FxVec2::from_ints(700, 900)) < Fx::from_int(60)).count();
    assert_eq!(near_rally, 2, "tanks at the rally point");
}

#[test]
fn ai_players_fight_a_whole_match_deterministically() {
    let run = |threads| {
        let mut w = flat_world(threads, true, true);
        let mut hashes = Vec::new();
        for _ in 0..6000 {
            hashes.push(w.tick(&[]).unwrap());
        }
        (hashes, w)
    };
    let (a, w) = run(0);
    let (b, _) = run(6);
    assert_eq!(a.iter().zip(&b).position(|(x, y)| x != y), None);
    let s = &w.state;
    let count = |owner: u8, cats: u32| s.units.slots.iter().filter(|&r| s.units.owner[r] == owner && w.bp(r).has(cats)).count();
    for p in 0..2u8 {
        assert!(count(p, mc_data::cat::FACTORY) >= 1, "player {p} built no factory");
        assert!(count(p, mc_data::cat::EXTRACTOR) >= 2, "player {p} built no extractors");
        assert!(s.players[p as usize].units_built > 15, "player {p} built {} units", s.players[p as usize].units_built);
    }
}
