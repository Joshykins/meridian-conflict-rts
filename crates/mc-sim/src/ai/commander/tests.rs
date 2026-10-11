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
    world_with(false)
}

/// `world`, with or without the fog of war.
fn world_with(fog: bool) -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 9,
        cheats: true,
        fog,
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
fn a_wave_that_takes_its_target_goes_on_rather_than_home() {
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
    // Two undefended mines a kilometre apart: once the first falls there is no armed
    // group near to take on, and the wave used to walk home.
    let mine = w.blueprints.id_of("aster_core_mine").unwrap();
    let mines = [
        w.spawn_unit(mine, 1, FxVec2::from_ints(4000, 4000), Angle::ZERO, true)
            .unwrap(),
        w.spawn_unit(mine, 1, FxVec2::from_ints(4900, 4600), Angle::ZERO, true)
            .unwrap(),
    ]
    .map(|r| w.state.units.id(r));
    for _ in 0..90 {
        run(&mut w, 100);
        let c = &w.state.ai[0].commander;
        assert!(
            !c.ops
                .iter()
                .any(|o| o.kind == OpKind::Army && o.phase == super::state::Phase::Withdrawing),
            "a winning wave fell back at tick {}:\n{}",
            w.state.tick,
            c.op_lines().join("\n")
        );
        if mines.iter().all(|&m| w.state.units.row(m).is_none()) {
            return;
        }
    }
    panic!(
        "the wave never took both mines:\n{}",
        w.state.ai[0].commander.op_lines().join("\n")
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
    let cap = w.state.players[0].materials_capacity;
    for _ in 0..8 {
        w.state.players[0].materials = cap;
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

/// A Behemoth of the AI's at home, and a factory of the enemy's halfway with `guards`
/// tech 3 point defence turrets round it (1500 m: the Behemoth outranges them).
fn titan_world(guards: i32, fog: bool) -> (World, crate::tables::UnitId, FxVec2) {
    let mut w = world_with(fog);
    let titan = w.blueprints.id_of("aster_t5_titan").unwrap();
    let row = w
        .spawn_unit(titan, 0, FxVec2::from_ints(1400, 1400), Angle::ZERO, true)
        .unwrap();
    let id = w.state.units.id(row);
    let at = FxVec2::from_ints(4600, 4600);
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    w.spawn_unit(factory, 1, at, Angle::ZERO, true).unwrap();
    let pd = w.blueprints.id_of("aster_t3_point_defense").unwrap();
    for i in 0..guards {
        let off = FxVec2::from_ints([160, -160, 0, 0][i as usize], [0, 0, 160, -160][i as usize]);
        w.spawn_unit(pd, 1, at + off, Angle::ZERO, true).unwrap();
    }
    if guards > 0 {
        let shield = w.blueprints.id_of("aster_t3_shield").unwrap();
        for off in [FxVec2::from_ints(110, 110), FxVec2::from_ints(-110, -110)] {
            w.spawn_unit(shield, 1, at + off, Angle::ZERO, true)
                .unwrap();
        }
    }
    (w, id, at)
}

#[test]
fn a_titan_goes_out_alone_at_once() {
    let (mut w, id, _) = titan_world(0, false);
    run(&mut w, 300);
    let c = &w.state.ai[0].commander;
    let op = c
        .ops
        .iter()
        .find(|o| o.has(id))
        .unwrap_or_else(|| panic!("the titan is in no operation: {}", c.summary()));
    assert_eq!(op.kind, OpKind::Titan, "{}", c.summary());
    let row = w.state.units.row(id).unwrap();
    assert!(
        w.state.units.pos[row].distance(FxVec2::from_ints(1400, 1400)) > Fx::from_int(200),
        "the titan stood at home: {}",
        c.summary()
    );
}

#[test]
fn a_titan_razes_a_guarded_base_from_beyond_its_guns() {
    // In the fog it finds the base only on its way, and takes it from where it sees it.
    let (mut w, id, at) = titan_world(4, true);
    let theirs = |w: &World| {
        w.state
            .units
            .slots
            .iter()
            .filter(|&r| w.state.units.owner[r] == 1)
            .count()
    };
    let mut nearest = Fx::from_int(1 << 20);
    for _ in 0..30 {
        run(&mut w, 50);
        let row = w.state.units.row(id).expect("the titan died");
        // Once the base is gone it walks on over it, to their commander.
        if theirs(&w) > 1 {
            nearest = nearest.min(w.state.units.pos[row].distance(at));
        }
    }
    let c = &w.state.ai[0].commander;
    // Only their commander is left, far off at their start.
    assert_eq!(theirs(&w), 1, "the base stands: {}", c.summary());
    // The tech 3 point defence reaches 1500 m.
    assert!(
        nearest > Fx::from_int(1500),
        "it walked in to {nearest} m of the guns: {}",
        c.summary()
    );
}

#[test]
fn gravitic_engines_put_their_wells_on_the_commander_then_the_army() {
    let mut w = world();
    let engine = w.blueprints.id_of("regency_t5_gravitic_engine").unwrap();
    let engines: Vec<_> = [1000, 1800]
        .into_iter()
        .map(|x| {
            let row = w
                .spawn_unit(engine, 0, FxVec2::from_ints(x, 1600), Angle::ZERO, true)
                .unwrap();
            w.state.units.id(row)
        })
        .collect();
    // Their army, standing off to one side; and one lone tank elsewhere.
    let tank = w.blueprints.id_of("aster_t2_tank").unwrap();
    let army = FxVec2::from_ints(5200, 2400);
    for i in 0..16 {
        w.spawn_unit(
            tank,
            1,
            army + FxVec2::from_ints((i % 4) * 30, (i / 4) * 30),
            Angle::ZERO,
            true,
        )
        .unwrap();
    }
    w.spawn_unit(tank, 1, FxVec2::from_ints(2400, 5600), Angle::ZERO, true)
        .unwrap();
    run(&mut w, 300);
    let mut marks: Vec<FxVec2> = engines
        .iter()
        .map(|&id| {
            w.state
                .wells
                .iter()
                .find(|g| g.source == id)
                .unwrap_or_else(|| {
                    panic!(
                        "an engine was never aimed: {}",
                        w.state.ai[0].commander.summary()
                    )
                })
                .mark
        })
        .collect();
    marks.sort_by_key(|m| (m.x, m.y));
    // Their commander (it ends the game) under one, their army under the other.
    let commander = FxVec2::from_ints(6800, 6800);
    assert!(
        marks[0].distance(army) < Fx::from_int(300)
            && marks[1].distance(commander) < Fx::from_int(300),
        "the wells went to {marks:?}, not the army at {army:?} and the commander at {commander:?}"
    );
}
