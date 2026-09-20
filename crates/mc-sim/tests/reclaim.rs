//! Reclaim: live units, idle builders, the reclaimer tower.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{
    Command, MatchConfig, PlayerCommand, PlayerSetup, RenderFrame, SimEvent, UnitId, World,
};
use std::path::Path;
use std::sync::Arc;

const ENGINEER: &str = "aster_t1_engineer";
const TANK: &str = "aster_t1_tank";
const TOWER: &str = "aster_t2_reclaimer";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "reclaim".into(),
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
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    // Somewhere to put the mass: stores are counted afresh every tick.
    let storage = spawn(&w, 0, "aster_mass_storage", 200, 0);
    w.tick(&[storage]).unwrap();
    w
}

fn cmd(command: Command) -> PlayerCommand {
    PlayerCommand { player: 0, command }
}

fn spawn(w: &World, owner: u8, key: &str, x: i32, flags: u16) -> PlayerCommand {
    cmd(Command::DebugSpawn {
        owner,
        blueprint: w.blueprints.id_of(key).unwrap(),
        pos: FxVec2::from_ints(x, 512),
        heading: Angle::ZERO,
        count: 1,
        flags,
        build: 1000,
    })
}

fn ids(w: &World, owner: u8, key: &str) -> Vec<UnitId> {
    let (u, bp) = (&w.state.units, w.blueprints.id_of(key).unwrap());
    u.slots
        .iter()
        .filter(|&r| u.owner[r] == owner && u.blueprint[r] == bp)
        .map(|r| u.id(r))
        .collect()
}

/// Ticks until `done`, collecting events. Panics when it never happens.
fn run_until(w: &mut World, most: u32, mut done: impl FnMut(&World) -> bool) -> Vec<SimEvent> {
    let mut events = Vec::new();
    for _ in 0..most {
        w.tick(&[]).unwrap();
        events.extend(w.events.iter().cloned());
        if done(w) {
            return events;
        }
    }
    panic!("not within {most} ticks");
}

#[test]
fn an_enemy_reclaimed_to_nothing_goes_quietly_and_pays() {
    let mut w = world();
    let setup = [
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 1, TANK, 540, flag::PASSIVE),
    ];
    w.tick(&setup).unwrap();
    let (engineer, tank) = (ids(&w, 0, ENGINEER), ids(&w, 1, TANK)[0]);
    w.tick(&[cmd(Command::ReclaimUnit {
        units: engineer,
        target: tank,
        queue: false,
    })])
    .unwrap();

    let mut frame = RenderFrame::default();
    w.tick(&[]).unwrap();
    w.write_render_frame(Some(0), &mut frame);
    assert_eq!(frame.beams.len(), 1, "a beam is drawn while it works");
    let row = w.state.units.row(tank).unwrap();
    assert!(
        w.state.units.health[row] < w.bp(row).health,
        "the tank is losing health"
    );

    let events = run_until(&mut w, 2000, |w| w.state.units.row(tank).is_none());
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::Reclaimed { wreck: false, .. })));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SimEvent::UnitDied { .. })),
        "nothing blew up"
    );
    assert_eq!(
        w.state.wrecks.slots.iter().count(),
        0,
        "and nothing was left lying about"
    );
    assert!(w.state.stains.is_empty());
    let cost = w.blueprints.unit_by_key(TANK).unwrap().cost_mass;
    let got = w.state.players[0].reclaimed_mass;
    assert!(
        got > cost * Fx::ratio(35, 100) && got <= cost * Fx::ratio(2, 5),
        "two fifths of its mass came back: {got:?} of {cost:?}"
    );
    assert_eq!(w.state.players[0].units_killed, 1);
    assert_eq!(w.state.players[1].units_lost, 1);
}

#[test]
fn own_structures_can_be_reclaimed_and_allies_cannot() {
    let mut w = world();
    let setup = [
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, "aster_t1_power", 548, 0),
    ];
    w.tick(&setup).unwrap();
    let power = ids(&w, 0, "aster_t1_power")[0];
    w.tick(&[cmd(Command::ReclaimUnit {
        units: ids(&w, 0, ENGINEER),
        target: power,
        queue: false,
    })])
    .unwrap();
    run_until(&mut w, 2000, |w| w.state.units.row(power).is_none());
    assert!(w.state.players[0].reclaimed_mass > Fx::ZERO);
    // The ground it stood on is free again, and the pour went with it.
    let bp = w.blueprints.unit_by_key("aster_t1_power").unwrap().clone();
    let lot = mc_sim::world::snap_to_build_grid(&bp, FxVec2::from_ints(548, 512));
    assert!(w.can_place(&bp, lot));
    assert!(w.state.pads.index_at(lot).is_none());

    // Same team, another player: not theirs to take apart.
    w.state.players[1].team = 0;
    w.tick(&[spawn(&w, 1, TANK, 540, flag::PASSIVE)]).unwrap();
    let tank = ids(&w, 1, TANK)[0];
    w.tick(&[cmd(Command::ReclaimUnit {
        units: ids(&w, 0, ENGINEER),
        target: tank,
        queue: false,
    })])
    .unwrap();
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    let row = w.state.units.row(tank).unwrap();
    assert_eq!(w.state.units.health[row], w.bp(row).health);
}

#[test]
fn an_idle_engineer_clears_the_wrecks_in_reach_while_there_is_room() {
    let mut w = world();
    let setup = [
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 1, TANK, 540, flag::PASSIVE),
        spawn(&w, 1, TANK, 900, flag::PASSIVE),
    ];
    w.tick(&setup).unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 1, TANK),
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 2);

    // Full stores: the wreck keeps its mass for later.
    w.state.players[0].mass = w.state.players[0].mass_capacity;
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.players[0].reclaimed_mass, Fx::ZERO);

    w.state.players[0].mass = Fx::ZERO;
    let events = run_until(&mut w, 2000, |w| w.state.wrecks.slots.iter().count() == 1);
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::Reclaimed { wreck: true, .. })));
    let engineer = w.state.units.row(ids(&w, 0, ENGINEER)[0]).unwrap();
    assert_eq!(
        w.state.units.pos[engineer],
        FxVec2::from_ints(500, 512),
        "it never left its post"
    );
    assert_eq!(
        w.state.units.order_head[engineer],
        mc_sim::tables::NO_ORDER,
        "and is still idle to whoever asks"
    );
    // The far wreck is out of reach and stays.
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.wrecks.slots.iter().count(), 1);
}

#[test]
fn a_reclaimer_tower_clears_wrecks_by_itself_but_only_takes_live_units_on_an_order() {
    let mut w = world();
    let setup = [
        spawn(&w, 0, TOWER, 500, 0),
        spawn(&w, 1, TANK, 600, flag::PASSIVE),
        spawn(&w, 1, TANK, 640, flag::PASSIVE),
        spawn(&w, 1, TANK, 1200, flag::PASSIVE),
        spawn(&w, 0, "aster_wall", 560, 0),
    ];
    w.tick(&setup).unwrap();
    let tanks = ids(&w, 1, TANK);

    // Enemies within reach are left alone until it is told otherwise.
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    for &tank in &tanks {
        let row = w.state.units.row(tank).unwrap();
        assert_eq!(
            w.state.units.health[row],
            w.bp(row).health,
            "nothing was drained unasked"
        );
    }
    assert_eq!(w.state.players[0].reclaimed_mass, Fx::ZERO);

    // A wreck within reach it clears by itself.
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![tanks[1]],
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 1);
    run_until(&mut w, 3000, |w| w.state.wrecks.slots.iter().count() == 0);

    // On an order it takes an enemy, and a wall of its own; told to reach too far, it lets the order go.
    let (tower, wall) = (ids(&w, 0, TOWER), ids(&w, 0, "aster_wall")[0]);
    w.tick(&[cmd(Command::ReclaimUnit {
        units: tower.clone(),
        target: tanks[0],
        queue: false,
    })])
    .unwrap();
    let events = run_until(&mut w, 3000, |w| w.state.units.row(tanks[0]).is_none());
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::Reclaimed { wreck: false, .. })));
    w.tick(&[cmd(Command::ReclaimUnit {
        units: tower.clone(),
        target: wall,
        queue: false,
    })])
    .unwrap();
    run_until(&mut w, 1000, |w| w.state.units.row(wall).is_none());
    w.tick(&[cmd(Command::ReclaimUnit {
        units: tower.clone(),
        target: tanks[2],
        queue: false,
    })])
    .unwrap();
    run_until(&mut w, 10, |w| {
        w.state.units.order_head[w.state.units.row(tower[0]).unwrap()] == mc_sim::tables::NO_ORDER
    });
    assert!(w.state.units.row(tanks[2]).is_some());
}

#[test]
fn a_reclaimer_tower_aims_and_charges_before_the_beam() {
    let mut w = world();
    // Facing east, wreck already in front: it still has to charge.
    w.tick(&[
        spawn(&w, 0, TOWER, 500, 0),
        spawn(&w, 1, TANK, 580, flag::PASSIVE),
    ])
    .unwrap();
    let tank = ids(&w, 1, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: tank,
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 1);

    let mut frame = RenderFrame::default();
    for _ in 0..15 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert!(frame.beams.is_empty(), "the beam waits on the charge");
    }
    assert_eq!(w.state.players[0].reclaimed_mass, Fx::ZERO);
    run_until(&mut w, 400, |w| w.state.wrecks.slots.iter().count() == 0);

    // A wreck behind it: the turret has to turn before it can charge again.
    w.tick(&[spawn(&w, 1, TANK, 420, flag::PASSIVE)]).unwrap();
    let rear = ids(&w, 1, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: rear,
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    let tower = w.state.units.row(ids(&w, 0, TOWER)[0]).unwrap();
    let start_yaw = w.state.units.weapon_yaw[tower][0];
    let mut turned = false;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert!(frame.beams.is_empty(), "it is still turning or charging");
        if w.state.units.weapon_yaw[tower][0] != start_yaw {
            turned = true;
        }
    }
    assert!(turned, "the turret turned toward the wreck behind it");
}

#[test]
fn an_armed_builder_leaves_wrecks_alone_while_an_enemy_is_about() {
    let mut w = world();
    let acu = w
        .blueprints
        .unit(w.blueprints.factions[0].commander)
        .key
        .clone();
    // A wreck beside the commander, and an enemy it cannot hurt standing within its guns' reach.
    let setup = [
        spawn(&w, 0, &acu, 500, 0),
        spawn(&w, 1, TANK, 530, flag::PASSIVE),
        spawn(&w, 1, TANK, 700, flag::PASSIVE | flag::INVULNERABLE),
    ];
    w.tick(&setup).unwrap();
    let tanks = ids(&w, 1, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: vec![tanks[0]],
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 1);
    let mut frame = RenderFrame::default();
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert!(frame.beams.is_empty(), "its torso stays on the enemy");
    }
    assert_eq!(w.state.players[0].reclaimed_mass, Fx::ZERO);

    // The enemy gone, it gets on with the wreck.
    w.tick(&[cmd(Command::DebugRemove {
        units: vec![tanks[1]],
    })])
    .unwrap();
    run_until(&mut w, 3000, |w| w.state.wrecks.slots.iter().count() == 0);
}

#[test]
fn a_reclaimed_commander_still_goes_up() {
    let mut w = world();
    let acu = w.blueprints.factions[0].commander;
    let key = w.blueprints.unit(acu).key.clone();
    w.tick(&[
        spawn(&w, 0, TOWER, 500, 0),
        spawn(&w, 1, &key, 560, flag::PASSIVE),
    ])
    .unwrap();
    // It is built like nothing else, and comes apart as slowly: start with the last of it.
    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 1, &key),
        permille: 999,
    })])
    .unwrap();
    w.tick(&[cmd(Command::ReclaimUnit {
        units: ids(&w, 0, TOWER),
        target: ids(&w, 1, &key)[0],
        queue: false,
    })])
    .unwrap();
    let events = run_until(&mut w, 20_000, |w| ids(w, 1, &key).is_empty());
    assert!(events
        .iter()
        .any(|e| matches!(e, SimEvent::UnitDied { .. })));
}
