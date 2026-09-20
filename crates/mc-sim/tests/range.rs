//! The debug commands the test range is made of.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

fn world(cheats: bool) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        deposits: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16, build: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build,
    })
}

fn ids(w: &World, owner: u8) -> Vec<UnitId> {
    let u = &w.state.units;
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner)
        .map(|r| u.id(r))
        .collect()
}

#[test]
fn a_dummy_is_shot_at_but_neither_shoots_nor_dies() {
    let mut w = world(true);
    let setup = [
        spawn(&w, 0, "aster_t1_tank", 500, 0, 1000),
        spawn(
            &w,
            1,
            "aster_t1_tank",
            600,
            flag::PASSIVE | flag::INVULNERABLE,
            1000,
        ),
    ];
    w.tick(&setup).unwrap();
    let mut shots = 0;
    for _ in 0..200 {
        w.tick(&[]).unwrap();
        assert!(
            w.state.projectiles.owner.iter().all(|&o| o == 0),
            "the dummy fired"
        );
        shots = shots.max(w.state.projectiles.len());
    }
    assert!(shots > 0, "the tank never fired at the dummy");
    let u = &w.state.units;
    let dummy = u.row(ids(&w, 1)[0]).expect("the dummy is still there");
    assert_eq!(u.health[dummy], w.bp(dummy).health);

    // Hold fire can be lifted, and then it fights back.
    let lift = cmd(Command::DebugSetFlags {
        units: ids(&w, 1),
        set: 0,
        clear: flag::PASSIVE,
    });
    w.tick(&[lift]).unwrap();
    let fired_back = (0..100).any(|_| {
        w.tick(&[]).unwrap();
        w.state.projectiles.owner.contains(&1)
    });
    assert!(fired_back);
}

#[test]
fn damage_heal_and_kill() {
    let mut w = world(true);
    let setup = [spawn(&w, 0, "aster_t1_tank", 500, flag::INVULNERABLE, 1000)];
    w.tick(&setup).unwrap();
    let tank = ids(&w, 0);
    let full = w.bp(w.state.units.row(tank[0]).unwrap()).health;
    let health = |w: &World| w.state.units.health[w.state.units.row(tank[0]).unwrap()];
    w.tick(&[cmd(Command::DebugDamage {
        units: tank.clone(),
        permille: 250,
    })])
    .unwrap();
    assert_eq!(health(&w), full - full / 4);
    w.tick(&[cmd(Command::DebugDamage {
        units: tank.clone(),
        permille: -1000,
    })])
    .unwrap();
    assert_eq!(health(&w), full);
    // Kill works on an invulnerable unit too, and leaves a wreck like any death.
    w.tick(&[cmd(Command::DebugDamage {
        units: tank.clone(),
        permille: 1000,
    })])
    .unwrap();
    assert!(w.state.units.row(tank[0]).is_none());
    assert_eq!(w.state.wrecks.slots.live(), 1);
}

#[test]
fn a_build_state_holds_until_someone_finishes_it() {
    let mut w = world(true);
    let setup = [
        spawn(&w, 0, "aster_t1_tank", 500, 0, 400),
        spawn(&w, 0, "aster_t1_engineer", 470, 0, 1000),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ];
    w.tick(&setup).unwrap();
    let tank_bp = w.blueprints.id_of("aster_t1_tank").unwrap();
    let u = &w.state.units;
    let site = u.slots.iter().find(|&r| u.blueprint[r] == tank_bp).unwrap();
    let (site_id, time) = (u.id(site), w.bp(site).build_time);
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    let u = &w.state.units;
    assert!(u.has_flag(site, flag::UNDER_CONSTRUCTION) && !u.is_active(site));
    assert_eq!(u.build_progress[site], time * 400 / 1000);

    // An engineer finishes it, and with free build it costs nothing although the bank is empty.
    let engineer: Vec<UnitId> = ids(&w, 0).into_iter().filter(|id| *id != site_id).collect();
    w.tick(&[cmd(Command::Assist {
        units: engineer,
        target: site_id,
        queue: false,
    })])
    .unwrap();
    let done = (0..600).any(|_| {
        w.tick(&[]).unwrap();
        w.state.units.is_active(site)
    });
    assert!(done, "the engineer never finished the tank");
    assert_eq!(w.state.players[0].efficiency, Fx::ONE);

    // And back to a bare frame.
    w.tick(&[cmd(Command::DebugSetBuild {
        units: vec![site_id],
        permille: 0,
    })])
    .unwrap();
    assert!(!w.state.units.is_active(site));
}

#[test]
fn the_other_side_can_be_steered_and_the_range_cleared() {
    let mut w = world(true);
    let setup = [
        spawn(&w, 0, "aster_t1_tank", 500, flag::PASSIVE, 1000),
        spawn(&w, 1, "aster_t1_tank", 900, flag::PASSIVE, 1000),
    ];
    w.tick(&setup).unwrap();
    let hostile = ids(&w, 1);
    let order = cmd(Command::Move {
        units: hostile.clone(),
        target: FxVec2::from_ints(1200, 512),
        queue: false,
    });
    // Not ours: ignored.
    w.tick(&[order.clone()]).unwrap();
    let row = w.state.units.row(hostile[0]).unwrap();
    assert_eq!(w.state.units.order_head[row], mc_sim::tables::NO_ORDER);
    w.tick(&[cmd(Command::DebugControl { player: 1 }), order])
        .unwrap();
    assert_ne!(w.state.units.order_head[row], mc_sim::tables::NO_ORDER);

    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 0),
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[cmd(Command::DebugClear)]).unwrap();
    let s = &w.state;
    assert_eq!(
        (
            s.units.slots.live(),
            s.wrecks.slots.live(),
            s.projectiles.len(),
            s.stains.len(),
            s.pads.len()
        ),
        (0, 0, 0, 0, 0)
    );
    // The range is usable again straight away.
    let again = [spawn(&w, 1, "aster_t1_tank", 500, 0, 1000)];
    w.tick(&again).unwrap();
    assert_eq!(w.state.units.slots.live(), 1);
}

#[test]
fn a_dead_structure_keeps_its_foundation() {
    let mut w = world(true);
    w.tick(&[spawn(&w, 0, "aster_t1_power", 500, 0, 1000)])
        .unwrap();
    assert_eq!(w.state.pads.len(), 1);
    let lot = w.state.pads.pos[0];

    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 0),
        permille: 1000,
    })])
    .unwrap();
    assert!(w.state.units.slots.live() == 0);
    assert_eq!(w.state.wrecks.slots.live(), 1);
    assert!(!w.state.stains.is_empty(), "death leaves a scorch stain");
    assert_eq!(w.state.pads.len(), 1);
    assert_eq!(w.state.pads.pos[0], lot);

    // The wreck can go; the poured lot stays.
    let wreck = w.state.wrecks.slots.iter().next().unwrap();
    w.state.wrecks.slots.free(wreck);
    assert_eq!(w.state.pads.len(), 1);
    assert_eq!(w.state.pads.pos[0], lot);
}

#[test]
fn without_cheats_none_of_it_happens() {
    let mut w = world(false);
    let setup = [
        spawn(&w, 0, "aster_t1_tank", 500, 0, 1000),
        cmd(Command::DebugControl { player: 1 }),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ];
    w.tick(&setup).unwrap();
    assert_eq!(w.state.units.slots.live(), 0);
    assert_eq!(w.state.players[0].acts_as, 0);
    assert!(!w.state.players[0].free_build);
}
