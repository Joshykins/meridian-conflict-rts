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
        units: engineer.clone(),
        target: tank,
        queue: false,
    })])
    .unwrap();

    let mut frame = RenderFrame::default();
    let mut saw_beam = false;
    for _ in 0..40 {
        w.tick(&[]).unwrap();
        w.write_render_frame(Some(0), &mut frame);
        if frame.beams.len() == 1 {
            saw_beam = true;
            break;
        }
    }
    assert!(saw_beam, "a beam is drawn once the arm is on the target");
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
        got > cost * Fx::ratio(15, 100) && got <= cost * Fx::ratio(1, 5),
        "a fifth of its mass came back: {got:?} of {cost:?}"
    );
    let by: Fx = engineer
        .iter()
        .map(|&e| w.state.units.reclaimed[w.state.units.row(e).unwrap()])
        .fold(Fx::ZERO, |a, b| a + b);
    assert_eq!(by, got, "the engineers count what they brought in");
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
fn an_engineer_sent_to_reclaim_a_far_wreck_takes_what_it_passes_on_the_way() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 300, 0),
        spawn(&w, 1, TANK, 600, flag::PASSIVE),
        spawn(&w, 1, TANK, 1000, flag::PASSIVE),
    ])
    .unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 1, TANK),
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    let wrecks = &w.state.wrecks;
    let at = |x: i32| {
        wrecks
            .slots
            .iter()
            .find(|&r| wrecks.pos[r].x == Fx::from_int(x))
            .unwrap()
    };
    let (passed, far) = (at(600), at(1000));
    let (full, far_id) = (wrecks.mass[passed], wrecks.slots.handle(far));
    w.tick(&[cmd(Command::ReclaimWreck {
        units: ids(&w, 0, ENGINEER),
        wreck: far_id,
        queue: false,
    })])
    .unwrap();
    let engineer = w.state.units.row(ids(&w, 0, ENGINEER)[0]).unwrap();
    run_until(&mut w, 2000, |w| {
        w.state.units.pos[engineer].x > Fx::from_int(700)
    });
    assert!(
        !w.state.wrecks.slots.is_alive(passed) || w.state.wrecks.mass[passed] < full,
        "it took from the wreck it passed"
    );
    run_until(&mut w, 3000, |w| !w.state.wrecks.slots.is_alive(far));
    assert!(
        !w.state.wrecks.slots.is_alive(far),
        "and reclaimed the one it was sent to"
    );
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
        spawn(&w, 1, TANK, 1750, flag::PASSIVE),
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
fn a_scavenger_tower_bites_at_once_and_sweeps_a_dimmer_beam_while_it_searches() {
    use mc_sim::reclaim::{BEAM_RECLAIM, BEAM_SWEEP};
    let kinds = |frame: &RenderFrame| frame.beams.iter().map(|b| b.kind).collect::<Vec<_>>();
    let mut w = world();
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
    // Facing east, the wreck in front and below: the beam bites as soon as the head has
    // pitched down onto it, with no charge after, and sweeps down onto it until then.
    let mut frame = RenderFrame::default();
    let on = (0..30).position(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        kinds(&frame) == [BEAM_RECLAIM]
    });
    assert!(on.is_some_and(|t| t < 25), "bit after {on:?} ticks");
    assert!(w.state.players[0].reclaimed_mass > Fx::ZERO);
    run_until(&mut w, 400, |w| w.state.wrecks.slots.iter().count() == 0);

    // Nothing in reach: the head turns slowly round, a sweep on the ground.
    let tower = w.state.units.row(ids(&w, 0, TOWER)[0]).unwrap();
    w.tick(&[]).unwrap();
    let searching = w.state.units.weapon_yaw[tower][0];
    for _ in 0..10 {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert_eq!(
            kinds(&frame),
            vec![BEAM_SWEEP],
            "it sweeps while it searches"
        );
    }
    assert_ne!(
        w.state.units.weapon_yaw[tower][0], searching,
        "the sweep turns"
    );

    // A wreck behind it: the sweep swings round onto it, then bites.
    w.tick(&[spawn(&w, 1, TANK, 420, flag::PASSIVE)]).unwrap();
    let rear = ids(&w, 1, TANK);
    w.tick(&[cmd(Command::DebugDamage {
        units: rear,
        permille: 1000,
    })])
    .unwrap();
    let mut swept = 0;
    let bit = (0..80).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        match kinds(&frame)[..] {
            [BEAM_SWEEP] => {
                swept += 1;
                false
            }
            [BEAM_RECLAIM] => true,
            ref other => panic!("{other:?}"),
        }
    });
    assert!(bit && swept > 2, "swept for {swept} ticks, then bit: {bit}");
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

#[test]
fn a_reclaimer_tower_sweeps_its_wrecks_in_turn_order_not_nearest_first() {
    let mut w = world();
    let tank = |x: i32, y: i32| {
        cmd(Command::DebugSpawn {
            owner: 1,
            blueprint: w.blueprints.id_of(TANK).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: flag::PASSIVE,
            build: 1000,
        })
    };
    // The turret faces east. Ahead of it a wreck, a little north of that another,
    // and nearest of all one behind it.
    let (ahead, beside, behind) = ((600, 512), (650, 540), (440, 512));
    let setup = [
        spawn(&w, 0, TOWER, 500, 0),
        tank(ahead.0, ahead.1),
        tank(beside.0, beside.1),
        tank(behind.0, behind.1),
    ];
    w.tick(&setup).unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 1, TANK),
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.wrecks.slots.iter().count(), 3);
    let there = |w: &World, (x, y): (i32, i32)| {
        let at = FxVec2::from_ints(x, y);
        w.state
            .wrecks
            .slots
            .iter()
            .any(|r| w.state.wrecks.pos[r].distance(at) < Fx::from_int(8))
    };
    let mut order = Vec::new();
    for _ in 0..6000 {
        w.tick(&[]).unwrap();
        for (name, at) in [("ahead", ahead), ("beside", beside), ("behind", behind)] {
            if !there(&w, at) && !order.contains(&name) {
                order.push(name);
            }
        }
        if order.len() == 3 {
            break;
        }
    }
    assert_eq!(order, ["ahead", "beside", "behind"]);
}

#[test]
fn an_engineer_assisting_one_that_reclaims_helps_take_the_wreck_apart() {
    let mut w = world();
    w.tick(&[
        spawn(&w, 0, ENGINEER, 500, 0),
        spawn(&w, 0, ENGINEER, 480, 0),
        spawn(&w, 1, TANK, 700, flag::PASSIVE),
    ])
    .unwrap();
    w.tick(&[cmd(Command::DebugDamage {
        units: ids(&w, 1, TANK),
        permille: 1000,
    })])
    .unwrap();
    w.tick(&[]).unwrap();
    let wreck = w.state.wrecks.slots.iter().next().unwrap();
    let handle = w.state.wrecks.slots.handle(wreck);
    let masons = ids(&w, 0, ENGINEER);
    let (lead, helper) = (masons[0], masons[1]);
    w.tick(&[
        cmd(Command::ReclaimWreck {
            units: vec![lead],
            wreck: handle,
            queue: false,
        }),
        cmd(Command::Assist {
            units: vec![helper],
            target: lead,
            queue: false,
        }),
    ])
    .unwrap();
    run_until(&mut w, 3000, |w| !w.state.wrecks.slots.is_alive(wreck));
    let row = w.state.units.row(helper).unwrap();
    assert!(
        w.state.units.reclaimed[row] > Fx::ZERO,
        "the helper took part of the wreck"
    );
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(mc_sim::tables::OrderKind::Assist),
        "and went back to assisting"
    );
}
