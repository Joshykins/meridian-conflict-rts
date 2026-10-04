use super::state::{OpKind, PlanKind, Stake};
use crate::command::PlayerCommand;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

/// A flat 8 km map: a Commander AI in slot 0 against an idle player.
fn world() -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 9,
        cheats: true,
        fog: false,
        spawn_commanders: true,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("P{i}"),
                faction: "Aster".into(),
                team: i,
                controller: if i == 0 {
                    Controller::Ai
                } else {
                    Controller::Human
                },
                start: i,
                ai: AiConfig {
                    difficulty: Difficulty::Hard,
                    ..AiConfig::default()
                },
            })
            .collect(),
    };
    World::with_terrain(
        Heightfield::flat(1024, 1024, Fx::from_int(20)),
        MapData {
            name: "commander test".into(),
            content_id: 9,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(1200, 1200), FxVec2::from_ints(6800, 6800)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap()
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.tick(&[] as &[PlayerCommand]).unwrap();
    }
}

#[test]
fn a_commander_opens_with_plans_and_puts_its_army_in_an_operation() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    for i in 0..10 {
        w.spawn_unit(
            tank,
            0,
            FxVec2::from_ints(1400 + i * 20, 1400),
            Angle::ZERO,
            true,
        )
        .unwrap();
    }
    run(&mut w, 200);
    let c = &w.state.ai[0].commander;
    assert_eq!(c.plan(PlanKind::Boom), Stake::Invest, "{}", c.summary());
    // The army, and the guard kept home against raids, between them.
    assert!(
        c.ops.iter().any(|o| o.kind == OpKind::Army),
        "{}",
        c.summary()
    );
    let held: usize = c
        .ops
        .iter()
        .filter(|o| matches!(o.kind, OpKind::Army | OpKind::Guard))
        .map(|o| o.units.len())
        .sum();
    assert_eq!(held, 10, "{}", c.summary());
}

#[test]
fn an_army_that_has_gathered_goes_for_the_enemy_and_trades_are_kept() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t2_tank").unwrap();
    for i in 0..24 {
        w.spawn_unit(
            tank,
            0,
            FxVec2::from_ints(1300 + (i % 8) * 20, 1500 + (i / 8) * 20),
            Angle::ZERO,
            true,
        )
        .unwrap();
    }
    // Something of theirs to find: a mine halfway, seen.
    let mine = w.blueprints.id_of("aster_core_mine").unwrap();
    w.spawn_unit(mine, 1, FxVec2::from_ints(4000, 4000), Angle::ZERO, true)
        .unwrap();
    // Its scouts look first; the wave goes once it knows where to.
    run(&mut w, 9000);
    // Kept by kind of operation: the army that made the kill may have finished
    // since, and with the enemy's commander dead no new one need open.
    let c = &w.state.ai[0].commander;
    assert!(
        c.trades[OpKind::Army as usize].killed > Fx::ZERO,
        "the army never killed anything: {}\n{}",
        c.summary(),
        c.op_lines().join("\n")
    );
}

#[test]
fn bombers_seen_bring_anti_air_into_the_army() {
    let mut w = world();
    let f = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let row = w
        .spawn_unit(f, 0, FxVec2::from_ints(1400, 1300), Angle::ZERO, true)
        .unwrap();
    let menu = w.bp(row).builder.as_ref().unwrap().builds.clone();
    let bomber = w.blueprints.id_of("aster_t1_bomber").unwrap();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let seen = w.state.tick;
    for i in 0..12 {
        w.state.ai[0].contacts.push(crate::ai::adaptive::Contact {
            id: crate::tables::UnitId::new(500 + i, 0),
            blueprint: if i < 8 { bomber } else { tank },
            pos: FxVec2::from_ints(5000, 5000),
            seen,
        });
    }
    w.state.ai[0].commander.sticky.air_seen = seen.max(1);
    let picks: Vec<String> = (0..6)
        .filter_map(|n| w.solve_production(0, &menu, &Default::default(), n))
        .map(|id| w.blueprints.unit(id).key.clone())
        .collect();
    assert!(picks.iter().any(|k| k.contains("mobile_aa")), "{picks:?}");
}

#[test]
fn an_observer_can_read_a_commanders_mind() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    for i in 0..6 {
        w.spawn_unit(
            tank,
            0,
            FxVec2::from_ints(1400 + i * 20, 1400),
            Angle::ZERO,
            true,
        )
        .unwrap();
    }
    run(&mut w, 200);
    let mind = w.ai_mind(0).expect("a Commander has a mind to show");
    assert_eq!(mind.plans.len(), 14, "every plan, held or not");
    assert!(mind.plans[0].level > 0, "held plans come first");
    let land: u32 = mind
        .ops
        .iter()
        .filter(|o| o.kind == "army" || o.kind == "guard")
        .map(|o| o.units)
        .sum();
    assert_eq!(land, 6, "{mind:?}");
    assert!(mind.ops.iter().all(|o| o.units == 0 || o.at.is_some()));
    assert!(w.ai_mind(1).is_none(), "a human side has none");
}

#[test]
fn the_economy_sets_expanders_apart_and_claims_mines() {
    let mut w = world();
    run(&mut w, 3600);
    let mines = w
        .state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.owner[r] == 0 && w.bp(r).mine.is_some())
        .count();
    let eco = &w.state.ai[0].commander.eco;
    assert!(
        mines >= 4,
        "only {mines} mines: {}",
        w.state.ai[0].commander.summary()
    );
    assert!(
        w.state.players[0].mine_power >= Fx::ratio(9, 10),
        "the mines go unpaid: {}",
        w.state.ai[0].commander.summary()
    );
    assert!(eco.engineers >= 2, "{}", w.state.ai[0].commander.summary());
}

#[test]
fn a_filling_store_is_read_as_floating_and_asks_for_sinks() {
    let mut w = world();
    run(&mut w, 1200);
    let before = w.state.ai[0].commander.eco.clone();
    let cap = w.state.players[0].mass_capacity;
    for _ in 0..8 {
        w.state.players[0].mass = cap;
        run(&mut w, 30);
    }
    let eco = &w.state.ai[0].commander.eco;
    assert!(eco.floating, "{}", w.state.ai[0].commander.summary());
    assert!(
        eco.upgrades > before.upgrades && eco.payback > before.payback,
        "more mine upgrades, and longer ones, while it floats: {}",
        w.state.ai[0].commander.summary()
    );
}
