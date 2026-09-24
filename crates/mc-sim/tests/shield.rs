//! A shield generator is refitted in place, like the commander: the next kit is
//! built onto the spire, and the unit stays the same.

use mc_core::{Angle, FxVec2};
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

fn with_shield() -> (World, UnitId) {
    let mut w = world();
    w.tick(&[
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of("aster_t2_shield").unwrap(),
            pos: FxVec2::from_ints(512, 512),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        }),
        cmd(Command::DebugFreeBuild {
            player: 0,
            on: true,
        }),
    ])
    .unwrap();
    let id = w.state.units.id(w.state.units.slots.iter().next().unwrap());
    (w, id)
}

#[test]
fn a_shield_is_refitted_in_place_and_stays_the_same_unit() {
    let (mut w, generator) = with_shield();
    let (t2, t3) = (
        w.blueprints.id_of("aster_t2_shield").unwrap(),
        w.blueprints.id_of("aster_t3_shield").unwrap(),
    );
    let row = w.state.units.row(generator).unwrap();

    w.tick(&[cmd(Command::Upgrade {
        units: vec![generator],
    })])
    .unwrap();
    assert_eq!(
        w.state.orders.front(&w.state.units, row).map(|o| o.kind),
        Some(OrderKind::Upgrade),
        "the refit is an order in its queue"
    );

    let mut shown = 0.0f32;
    let mut frame = mc_sim::RenderFrame::default();
    let done = (0..8000).any(|_| {
        w.tick(&[]).unwrap();
        w.write_render_frame(None, &mut frame);
        assert_eq!(
            frame.units.len(),
            1,
            "the refit is never drawn as a second unit"
        );
        let u = &frame.units[0];
        assert_eq!(
            u.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8,
            0,
            "the generator is not rebuilt from the weld"
        );
        shown = shown.max(u.upgrade);
        w.state.units.blueprint[row] == t3
    });
    assert!(done, "the refit never finished");
    assert!(
        shown > 0.9,
        "the mirror reported the refit's progress ({shown})"
    );
    assert_eq!(
        w.state.units.row(generator),
        Some(row),
        "same unit, same id"
    );
    assert_eq!(w.state.units.blueprint[row], t3, "it is now the next tier");
    assert_eq!(
        w.state.units.slots.iter().count(),
        1,
        "nothing is left behind"
    );
    w.write_render_frame(None, &mut frame);
    assert_eq!(frame.units[0].upgrade, 0.0);
    assert_eq!(w.blueprints.unit(t2).visual.mesh, "shield");
}

#[test]
fn a_dry_grid_marks_the_generator_unpowered() {
    use mc_sim::mirror::{STATE_CHARGING, STATE_UNPOWERED};

    let (mut w, _) = with_shield();
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    assert_eq!(
        frame.units[0].owner_flags & STATE_UNPOWERED,
        0,
        "a paid generator is live"
    );
    assert_eq!(frame.units[0].owner_flags & STATE_CHARGING, 0);

    w.state.players[0].free_build = false;
    w.tick(&[]).unwrap();
    w.write_render_frame(None, &mut frame);
    assert_ne!(
        frame.units[0].owner_flags & STATE_UNPOWERED,
        0,
        "the crystal should go dark while the grid is dry"
    );
    assert_eq!(
        frame.units[0].owner_flags & STATE_CHARGING,
        0,
        "a stall is not a fill"
    );
}

#[test]
fn a_shattered_dome_marks_the_generator_charging() {
    use mc_sim::mirror::{STATE_CHARGING, STATE_UNPOWERED};

    let (mut w, generator) = with_shield();
    let row = w.state.units.row(generator).unwrap();
    w.state.units.shield_recharge[row] = 1;
    w.state.units.shield_open[row] = 0;
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(None, &mut frame);
    assert_ne!(
        frame.units[0].owner_flags & STATE_CHARGING,
        0,
        "the crystal should pulse while the dome fills"
    );
    assert_eq!(frame.units[0].owner_flags & STATE_UNPOWERED, 0);
    assert!(
        (frame.shields[0].projector - 16.0).abs() < 0.01,
        "the launch beam is born in the crystal: {}",
        frame.shields[0].projector
    );

    w.state.players[0].free_build = false;
    w.tick(&[]).unwrap();
    w.write_render_frame(None, &mut frame);
    assert_ne!(
        frame.units[0].owner_flags & STATE_UNPOWERED,
        0,
        "a stall pauses the fill and kills the emitters"
    );
    assert_eq!(
        frame.units[0].owner_flags & STATE_CHARGING,
        0,
        "the pulse waits for the bus"
    );
}

/// Every hull under a field is protected, including when this blast drains it.
#[test]
fn commander_blast_cannot_leak_through_a_shield_to_later_victims() {
    use mc_core::Fx;
    for charge in [9000, 100] {
        let (mut w, shield) = with_shield();
        let shield_row = w.state.units.row(shield).unwrap();
        let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
        let commander = w.blueprints.id_of("aster_commander").unwrap();
        let spawn = |owner, blueprint, x, y| {
            cmd(Command::DebugSpawn {
                owner,
                blueprint,
                pos: FxVec2::from_ints(x, y),
                heading: Angle::ZERO,
                count: 1,
                flags: flag::PASSIVE,
                build: 1000,
            })
        };
        w.tick(&[
            spawn(0, tank, 552, 490),
            spawn(0, tank, 552, 530),
            spawn(1, commander, 626, 512),
        ])
        .unwrap();
        w.state.units.shield_open[shield_row] = 255;
        w.state.units.prev_shield_open[shield_row] = 255;
        w.state.units.shield_hp[shield_row] = Fx::from_int(charge);
        w.state.units.shield_recharge[shield_row] = 0;
        let victims: Vec<_> = w
            .state
            .units
            .slots
            .iter()
            .filter(|&r| w.state.units.owner[r] == 0)
            .map(|r| (w.state.units.id(r), w.state.units.health[r]))
            .collect();
        assert!(victims.len() >= 3);
        let enemy = w
            .state
            .units
            .slots
            .iter()
            .find(|&r| w.state.units.owner[r] == 1)
            .unwrap();
        let enemy = w.state.units.id(enemy);
        w.tick(&[cmd(Command::DebugDamage {
            units: vec![enemy],
            permille: 1000,
        })])
        .unwrap();
        for (id, health) in victims {
            let row = w.state.units.row(id).expect("shielded unit must survive");
            assert_eq!(
                w.state.units.health[row], health,
                "blast penetrated shield at charge {charge}"
            );
        }
        assert!(
            w.state.units.shield_hp[shield_row] < Fx::from_int(charge),
            "shield must absorb damage"
        );
    }
}
