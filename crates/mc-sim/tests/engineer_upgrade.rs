//! An engineer builds its own next tier onto itself, once the side has reached that tier.

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1280, 1280, Fx::from_int(20));
    let map = MapData {
        name: "engineer upgrade".into(),
        content_id: 1,
        ore: Vec::new(),
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
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Stores full enough to pay for the upgrade outright: no commander, no income.
    // The stock is set the tick after the storage, once it counts in what they can hold.
    w.tick(&[cmd(Command::DebugStorage {
        player: 0,
        mass: 10_000,
        energy: 100_000,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugStock {
        player: 0,
        mass: Some(1000),
        energy: Some(1000),
    })])
    .unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

/// A finished unit of `key` for player 0 at (x, y); returns its id after one tick.
fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of(key).unwrap();
    let pos = FxVec2::from_ints(x, y);
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: bp,
        pos,
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == 0)
        .min_by_key(|&r| u.pos[r].distance_sq(pos))
        .expect("spawned");
    u.id(row)
}

#[test]
fn an_engineer_upgrades_itself_once_the_side_has_the_tier() {
    let mut w = world();
    let mason = spawn(&mut w, "aster_t1_engineer", 5000, 5000);
    let t2 = w.blueprints.id_of("aster_t2_engineer").unwrap();
    // On tech 1 it cannot: the upgrade is not a way to reach tech 2.
    w.tick(&[cmd(Command::Upgrade { units: vec![mason] })])
        .unwrap();
    let row = w.state.units.row(mason).unwrap();
    assert!(
        w.state.orders.front(&w.state.units, row).is_none(),
        "upgraded on tech 1"
    );
    // A tech 2 factory brings the side there.
    spawn(&mut w, "aster_t2_land_factory", 6000, 6000);
    assert_eq!(w.side_tech(0), 2);
    w.tick(&[cmd(Command::Upgrade { units: vec![mason] })])
        .unwrap();
    let before = w.state.units.pos[w.state.units.row(mason).unwrap()];
    // It fits its next tier at that tier's build power: 900 / 30 = 30 s, with some to spare.
    let mut paid = (0.0, 0.0);
    for _ in 0..40 * TICKS_PER_SECOND {
        w.tick(&[]).unwrap();
        let pl = &w.state.players[0];
        paid.0 += pl.mass_spent.to_f64() / TICKS_PER_SECOND as f64;
        paid.1 += pl.energy_spent.to_f64() / TICKS_PER_SECOND as f64;
        if w.state
            .units
            .row(mason)
            .is_some_and(|r| w.state.units.blueprint[r] == t2)
        {
            break;
        }
    }
    let row = w
        .state
        .units
        .row(mason)
        .expect("the same unit after the upgrade");
    assert_eq!(w.state.units.blueprint[row], t2);
    assert_eq!(w.state.units.pos[row], before, "upgraded where it stood");
    // It paid only what tech 2 costs over tech 1: 200 - 52 materials, 1200 - 260 energy.
    assert!((paid.0 - 148.0).abs() < 2.0, "paid {} materials", paid.0);
    assert!((paid.1 - 940.0).abs() < 10.0, "paid {} energy", paid.1);
    assert!(w.state.orders.front(&w.state.units, row).is_none());
    // Nothing is left behind: no second engineer, finished or not.
    let u = &w.state.units;
    let engineers = u
        .slots
        .iter()
        .filter(|&r| u.owner[r] == 0 && w.bp(r).has(mc_data::cat::ENGINEER))
        .count();
    assert_eq!(engineers, 1);
}

#[test]
fn a_structure_upgrade_costs_only_the_difference_too() {
    let w = world();
    let t2 = w
        .blueprints
        .unit(w.blueprints.id_of("aster_core_mine_t2").unwrap());
    assert_eq!(
        w.blueprints.upgrade_cost(t2),
        (Fx::from_int(560 - 45), Fx::from_int(3360 - 270))
    );
}
