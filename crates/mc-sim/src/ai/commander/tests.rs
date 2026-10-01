use super::state::{OpKind, PlanKind, Stake};
use crate::command::PlayerCommand;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, Brain, Difficulty, MatchConfig, PlayerSetup, World};
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
                    brain: Brain::Commander,
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
    let army = c
        .ops
        .iter()
        .find(|o| o.kind == OpKind::Army)
        .expect("an army operation");
    assert_eq!(army.units.len(), 10, "{}", c.summary());
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
    run(&mut w, 3000);
    let c = &w.state.ai[0].commander;
    let army = c
        .ops
        .iter()
        .find(|o| o.kind == OpKind::Army)
        .expect("an army operation");
    assert!(
        army.ledger.killed > Fx::ZERO,
        "the army never killed anything: {}",
        c.summary()
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
