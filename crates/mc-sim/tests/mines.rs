//! Core mines: they stand only on the map's mine points, one to an ore field, and
//! dig a fixed rate a second that each tier raises.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Heightfield, OreRegion};
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, Refusal, SimEvent, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// A square ore field `2 * half` metres across.
fn square(x: i32, y: i32, half: i32) -> OreRegion {
    OreRegion {
        points: [(-1, -1), (1, -1), (1, 1), (-1, 1)]
            .into_iter()
            .map(|(dx, dy)| FxVec2::from_ints(x + dx * half, y + dy * half))
            .collect(),
    }
}

fn world(ore: Vec<OreRegion>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "mines".into(),
        content_id: 1,
        ore,
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(9500, 9500)],
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
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let terrain = Heightfield::flat(1280, 1280, Fx::from_int(20));
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    for p in &mut w.state.players {
        p.mass_capacity = Fx::from_int(100_000);
        p.energy_capacity = Fx::from_int(1_000_000);
    }
    w
}

fn cmd(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

/// A finished mine for `owner` at (x, y); returns its id after one tick.
fn mine(w: &mut World, owner: u8, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of("aster_core_mine").unwrap();
    w.tick(&[cmd(
        0,
        Command::DebugSpawn {
            owner,
            blueprint: bp,
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    )])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(FxVec2::from_ints(x, y)))
        .expect("mine spawned");
    u.id(row)
}

fn spec(w: &World, key: &str) -> mc_data::Mine {
    let bp = w.blueprints.id_of(key).unwrap();
    w.blueprints.unit(bp).mine.unwrap()
}

fn made(w: &World, id: UnitId) -> Fx {
    let row = w.state.units.row(id).unwrap();
    w.bp(row).mine.unwrap().rate
}

/// A finished unit of `key` for `owner` at (x, y), by its id.
fn unit(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    w.tick(&[cmd(
        0,
        Command::DebugSpawn {
            owner,
            blueprint: bp,
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    )])
    .unwrap();
    let u = &w.state.units;
    u.id(u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(FxVec2::from_ints(x, y)))
        .unwrap())
}

fn refused(w: &World, reason: Refusal) -> bool {
    w.events
        .iter()
        .any(|e| matches!(e, SimEvent::CommandRefused { reason: r, .. } if *r == reason))
}

#[test]
fn every_ore_field_holds_one_mine_point_on_the_lot_grid() {
    let fields = [(2000, 2000), (3000, 2400), (6000, 7100)];
    let w = world(fields.iter().map(|&(x, y)| square(x, y, 60)).collect());
    assert_eq!(w.mine_points.len(), fields.len());
    for (p, &(x, y)) in w.mine_points.iter().zip(&fields) {
        assert!(
            p.distance(FxVec2::from_ints(x, y)) < Fx::from_int(12),
            "{p:?}"
        );
        // A 3x3 lot centres on a 12 m cell.
        assert_eq!(p.x.floor_int() % 12, 6);
        assert_eq!(p.y.floor_int() % 12, 6);
    }
}

#[test]
fn each_tier_digs_its_rate_and_pays_back_slower_than_the_last() {
    let mut w = world(Vec::new());
    let keys = [
        "aster_core_mine",
        "aster_core_mine_t2",
        "aster_core_mine_t3",
        "aster_core_mine_t4",
    ];
    let rates: Vec<Fx> = keys.iter().map(|k| spec(&w, k).rate).collect();
    assert_eq!(rates, [1, 4, 12, 24].map(Fx::from_int));
    // A new mine pays back in under a minute; each upgrade slower than the one before.
    let cost = |w: &World, k: &str| w.blueprints.unit(w.blueprints.id_of(k).unwrap()).cost_mass;
    assert!(cost(&w, keys[0]) / rates[0] <= Fx::from_int(60));
    let mut last = Fx::ZERO;
    for i in 1..keys.len() {
        let payback = (cost(&w, keys[i]) - cost(&w, keys[i - 1])) / (rates[i] - rates[i - 1]);
        assert!(payback > last, "{}: {payback:?}", keys[i]);
        last = payback;
    }
    let id = mine(&mut w, 0, 5002, 5002);
    w.state.players[0].energy = Fx::from_int(1000);
    w.tick(&[]).unwrap();
    // Paid a tick at a time, so to within rounding.
    assert!((w.state.players[0].mass_income - made(&w, id)).abs() < Fx::ratio(1, 100));
}

#[test]
fn a_mine_goes_only_on_a_mine_point() {
    let mut w = world(vec![square(3000, 3000, 60)]);
    let point = w.mine_points[0];
    let engineer = unit(&mut w, "aster_t1_engineer", 0, 2900, 2900);
    let mine = w.blueprints.id_of("aster_core_mine").unwrap();
    let build = |at: FxVec2| {
        cmd(
            0,
            Command::Build {
                units: vec![engineer],
                blueprint: mine,
                pos: at,
                heading: Angle::ZERO,
                queue: false,
            },
        )
    };
    // Far from any point: refused, saying why.
    w.tick(&[build(FxVec2::from_ints(3400, 3000))]).unwrap();
    assert!(refused(&w, Refusal::NotOnMinePoint));
    let row = w.state.units.row(engineer).unwrap();
    assert!(w.state.orders.front(&w.state.units, row).is_none());
    // Dropped near one: it goes onto the point itself.
    w.tick(&[build(point + FxVec2::from_ints(35, -20))])
        .unwrap();
    let row = w.state.units.row(engineer).unwrap();
    let order = w
        .state
        .orders
        .front(&w.state.units, row)
        .expect("a build order");
    assert_eq!(order.pos, point);
    assert!(w.can_place(w.blueprints.unit(mine), point));
    assert!(!w.can_place(w.blueprints.unit(mine), point + FxVec2::from_ints(12, 0)));
}

#[test]
fn a_point_with_anyones_mine_on_it_is_taken() {
    let mut w = world(vec![square(3000, 3000, 60)]);
    let point = w.mine_points[0];
    mine(&mut w, 1, point.x.floor_int(), point.y.floor_int());
    let engineer = unit(&mut w, "aster_t1_engineer", 0, 2900, 2900);
    let blueprint = w.blueprints.id_of("aster_core_mine").unwrap();
    w.tick(&[cmd(
        0,
        Command::Build {
            units: vec![engineer],
            blueprint,
            pos: point,
            heading: Angle::ZERO,
            queue: false,
        },
    )])
    .unwrap();
    assert!(refused(&w, Refusal::MinePointTaken));
    assert!(!w.can_place(w.blueprints.unit(blueprint), point));
}

#[test]
fn an_upgraded_mine_digs_more_once_the_side_has_the_tech() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    let t2 = w.blueprints.id_of("aster_core_mine_t2").unwrap();
    // Not before the side has tech 2: nothing is queued. (Free building would skip this.)
    w.tick(&[cmd(0, Command::Upgrade { units: vec![id] })])
        .unwrap();
    let row = w.state.units.row(id).unwrap();
    assert!(
        w.state.orders.front(&w.state.units, row).is_none(),
        "upgraded without tech 2"
    );
    w.tick(&[cmd(
        0,
        Command::DebugFreeBuild {
            player: 0,
            on: true,
        },
    )])
    .unwrap();
    let before = w.state.mines.by_unit[&id].clone();
    unit(&mut w, "aster_t2_engineer", 0, 5400, 5002);
    assert_eq!(w.side_tech(0), 2);
    w.tick(&[cmd(0, Command::Upgrade { units: vec![id] })])
        .unwrap();
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        if w.state
            .units
            .row(id)
            .is_some_and(|r| w.state.units.blueprint[r] == t2)
        {
            break;
        }
    }
    let row = w
        .state
        .units
        .row(id)
        .expect("the same unit after the refit");
    assert_eq!(w.state.units.blueprint[row], t2);
    w.tick(&[]).unwrap();
    assert_eq!(made(&w, id), Fx::from_int(4));
    assert!(
        w.state.mines.by_unit[&id].age > before.age,
        "digging carries on through the upgrade"
    );
}

#[test]
fn a_mine_stops_paying_when_it_dies() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    w.tick(&[]).unwrap();
    assert!(w.state.players[0].mass_income > Fx::ZERO);
    w.tick(&[cmd(0, Command::DebugRemove { units: vec![id] })])
        .unwrap();
    w.tick(&[]).unwrap();
    assert!(w.state.mines.by_unit.is_empty());
    assert_eq!(w.state.players[0].mass_income, Fx::ZERO);
}

#[test]
fn a_mine_short_of_energy_digs_slower_down_to_a_quarter() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    let row = w.state.units.row(id).unwrap();
    let upkeep = w.bp(row).economy.energy_upkeep;
    assert!(upkeep > Fx::ZERO, "mines draw energy");
    let per_tick = upkeep / mc_core::TICKS_PER_SECOND as i32;
    let tick_with = |w: &mut World, energy: Fx| {
        w.state.players[0].energy = energy;
        w.tick(&[]).unwrap();
        let p = &w.state.players[0];
        (
            p.mass_income.to_f64(),
            p.mine_lost.to_f64(),
            p.mine_power.to_f64(),
        )
    };
    let full = made(&w, id).to_f64();

    let (income, lost, power) = tick_with(&mut w, Fx::from_int(1000));
    assert!((income - full).abs() < 0.01, "powered: {income} of {full}");
    assert_eq!((lost, power), (0.0, 1.0));

    // Half this tick's upkeep in store: half powered, 1/4 + 3/4 * 1/2 of the output.
    let (income, lost, power) = tick_with(&mut w, per_tick / 2);
    assert!((power - 0.5).abs() < 0.01, "power {power}");
    assert!(
        (income - full * 0.625).abs() < 0.01,
        "half: {income} of {full}"
    );
    assert!((income + lost - full).abs() < 0.01, "lost {lost}");

    // None at all: a quarter, and the stall says what it costs.
    let (income, lost, power) = tick_with(&mut w, Fx::ZERO);
    assert_eq!(power, 0.0);
    assert!(
        (income - full * 0.25).abs() < 0.01,
        "unpowered: {income} of {full}"
    );
    assert!((lost - full * 0.75).abs() < 0.01, "lost {lost}");
    assert!(w.state.players[0].upkeep_efficiency < Fx::ONE);
}

#[test]
fn a_mason_iii_builds_a_deep_core_outright() {
    let mut w = world(vec![square(3000, 3000, 200)]);
    let key = |w: &World, k: &str| w.blueprints.id_of(k).unwrap();
    let (engineer, deep) = (key(&w, "aster_t3_engineer"), key(&w, "aster_core_mine_t4"));
    w.tick(&[
        cmd(
            0,
            Command::DebugSpawn {
                owner: 0,
                blueprint: engineer,
                pos: FxVec2::from_ints(2900, 2900),
                heading: Angle::ZERO,
                count: 1,
                flags: 0,
                build: 1000,
            },
        ),
        cmd(
            0,
            Command::DebugFreeBuild {
                player: 0,
                on: true,
            },
        ),
    ])
    .unwrap();
    let u = &w.state.units;
    let mason = u.id(u
        .slots
        .iter()
        .find(|&r| u.blueprint[r] == engineer)
        .unwrap());
    w.tick(&[cmd(
        0,
        Command::Build {
            units: vec![mason],
            blueprint: deep,
            pos: FxVec2::from_ints(3000, 3000),
            heading: Angle::ZERO,
            queue: false,
        },
    )])
    .unwrap();
    let built = (0..60_000).any(|_| {
        w.tick(&[]).unwrap();
        let u = &w.state.units;
        u.slots.iter().any(|r| {
            u.blueprint[r] == deep && !u.has_flag(r, mc_sim::tables::flag::UNDER_CONSTRUCTION)
        })
    });
    assert!(built, "the deep core went up where it was placed");
    // Standing mines are counted in at the start of a tick.
    w.tick(&[]).unwrap();
    let u = &w.state.units;
    let row = u.slots.iter().find(|&r| u.blueprint[r] == deep).unwrap();
    assert!(
        w.state.mines.by_unit.contains_key(&u.id(row)),
        "and digs as a mine"
    );
}
