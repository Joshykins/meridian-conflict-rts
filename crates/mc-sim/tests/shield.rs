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

/// A dome holds a commander's blast back while it has the charge for it, for every hull
/// under it; what it cannot hold goes through (a nuclear blast is stopped by nothing).
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
        // The commander's blast is nuclear now: its front takes a few ticks to run out.
        for _ in 0..40 {
            w.tick(&[]).unwrap();
        }
        for (id, health) in victims {
            let now = w.state.units.row(id).map(|row| w.state.units.health[row]);
            if charge >= 9000 {
                assert_eq!(now, Some(health), "a charged dome holds the blast back");
            } else {
                assert!(
                    now.is_none_or(|h| h < health),
                    "a drained dome lets the rest through"
                );
            }
        }
        if let Some(row) = w.state.units.row(shield) {
            assert!(
                w.state.units.shield_hp[row] < Fx::from_int(charge),
                "shield must absorb damage"
            );
        }
        let _ = shield_row;
    }
}

/// A grid with some income but not enough for the dome's upkeep drains its store,
/// and the stall's rounded-down shares leave a few raw units behind, not zero.
/// That residue must still count as empty, or the dome never drops.
#[test]
fn a_dome_drops_when_a_trickle_of_income_cannot_pay_its_upkeep() {
    let (mut w, generator) = with_shield();
    let row = w.state.units.row(generator).unwrap();
    w.tick(&[cmd(Command::DebugSpawn {
        owner: 0,
        blueprint: w.blueprints.id_of("aster_t1_power").unwrap(),
        pos: FxVec2::from_ints(700, 700),
        heading: Angle::ZERO,
        count: 1,
        flags: 0,
        build: 1000,
    })])
    .unwrap();
    let p = &mut w.state.players[0];
    p.free_build = false;
    p.bonus_storage[1] = mc_core::Fx::from_int(1000);
    p.energy = mc_core::Fx::from_int(100);
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    let p = &w.state.players[0];
    assert!(
        p.energy_income > mc_core::Fx::ZERO,
        "the grid still makes something"
    );
    assert!(p.upkeep_efficiency < mc_core::Fx::ONE, "upkeep is not paid");
    assert_eq!(
        w.state.units.shield_open[row], 0,
        "an unpaid dome should fold, whatever crumbs are left in the store"
    );
}

/// Upkeep is covered, but construction drains the store to nothing: that is an
/// energy stall too, and the dome drops with it.
#[test]
fn a_dome_drops_when_construction_stalls_the_grid() {
    let (mut w, generator) = with_shield();
    let row = w.state.units.row(generator).unwrap();
    // The power plant, and a tier 3 engineer: the side needs tech 3 for the tier 3 dome.
    let spawn = |key: &str, x: i32| {
        cmd(Command::DebugSpawn {
            owner: 0,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, 700),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        })
    };
    let spawns = [
        spawn("aster_t1_power", 700),
        spawn("aster_t3_engineer", 900),
    ];
    w.tick(&spawns).unwrap();
    let p = &mut w.state.players[0];
    p.free_build = false;
    p.income_permille[1] = 12_000;
    p.bonus_storage = [mc_core::Fx::from_int(100_000); 2];
    p.mass = mc_core::Fx::from_int(100_000);
    // Little in store: the upgrade (priced at the difference between the tiers) runs it dry.
    p.energy = mc_core::Fx::from_int(20);
    w.tick(&[cmd(Command::Upgrade {
        units: vec![generator],
    })])
    .unwrap();
    for _ in 0..60 {
        w.tick(&[]).unwrap();
    }
    let p = &w.state.players[0];
    // A stall slows upkeep with everything else (`economy.rs`): it is not paid first.
    assert!(
        p.upkeep_efficiency < mc_core::Fx::ONE,
        "upkeep shares the stall"
    );
    assert!(p.energy_demand > p.energy_income, "the refit asks for more");
    assert_eq!(
        w.state.units.shield_open[row], 0,
        "a stalled grid should drop the dome even when upkeep is paid"
    );
}

/// The Testudo's dome goes with it: driving beside a tank, the shells meant for
/// the tank break on the glass and the tank is untouched.
#[test]
fn a_mobile_shield_covers_the_tank_it_drives_with() {
    let mut w = world();
    let spawn = |w: &mut World, key: &str, owner: u8, x: i32, y: i32| {
        let bp = w.blueprints.id_of(key).unwrap();
        let row = w
            .spawn_unit(bp, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
            .unwrap();
        w.state.units.id(row)
    };
    w.state.players[0].free_build = true;
    let shield = spawn(&mut w, "aster_t2_mobile_shield", 0, 500, 500);
    let tank = spawn(&mut w, "aster_t2_tank", 0, 512, 500);
    for _ in 0..80 {
        w.tick(&[]).unwrap();
    }
    let shield_row = w.state.units.row(shield).unwrap();
    assert!(
        w.state.units.shield_hp[shield_row] > mc_core::Fx::ZERO,
        "the dome is not up"
    );
    let full = w.state.units.health[w.state.units.row(tank).unwrap()];
    let gun = spawn(&mut w, "aster_t2_tank", 1, 650, 800);
    w.tick(&[
        cmd(Command::Move {
            units: vec![shield, tank],
            target: FxVec2::from_ints(800, 500),
            queue: false,
        }),
        PlayerCommand {
            player: 1,
            command: Command::Attack {
                units: vec![gun],
                target: tank,
                queue: false,
            },
        },
    ])
    .unwrap();
    let start = w.state.units.pos[shield_row];
    let mut on_dome = 0;
    for _ in 0..150 {
        w.tick(&[]).unwrap();
        on_dome += w
            .events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    mc_sim::SimEvent::Impact {
                        on_shield: true,
                        ..
                    }
                )
            })
            .count();
    }
    let shield_row = w.state.units.row(shield).unwrap();
    assert!(
        (w.state.units.pos[shield_row] - start).length() > mc_core::Fx::from_int(60),
        "the Testudo did not drive"
    );
    assert!(on_dome >= 3, "only {on_dome} shells broke on the dome");
    assert_eq!(
        w.state.units.health[w.state.units.row(tank).unwrap()],
        full,
        "a shell got under the moving dome"
    );
}
