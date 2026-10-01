use super::super::landing::Landing;
use super::*;
use crate::tables::{Controller, WarpPhase};
use crate::world::MapData;
use crate::{AiConfig, Difficulty, MatchConfig, PlayerSetup};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

/// A flat map 8 km a side, starts 6 km apart, a full energy store for both.
fn world() -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 11,
        cheats: true,
        fog: false,
        spawn_commanders: false,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                team: i,
                controller: Controller::Human,
                start: i,
                ai: AiConfig {
                    difficulty: Difficulty::Hard,
                    ..AiConfig::default()
                },
            })
            .collect(),
    };
    let mut w = World::with_terrain(
        Heightfield::flat(1024, 1024, Fx::from_int(20)),
        MapData {
            name: "strategy test".into(),
            content_id: 11,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(1000, 1000), FxVec2::from_ints(7000, 7000)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap();
    for p in &mut w.state.players {
        p.bonus_storage[1] = Fx::from_int(200_000);
        p.energy = Fx::from_int(100_000);
    }
    w
}

fn spawn(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    w.spawn_unit(
        w.blueprints.id_of(key).unwrap(),
        owner,
        FxVec2::from_ints(x, y),
        Angle::ZERO,
        true,
    )
    .unwrap()
}

/// `player` remembers an enemy `key` at `(x, y)`, seen now.
fn remember(w: &mut World, player: u8, key: &str, x: i32, y: i32) {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let n = w.state.ai[player as usize].contacts.len();
    let seen = w.state.tick;
    w.state.ai[player as usize].contacts.push(Contact {
        id: UnitId::new(900 + n, 0),
        blueprint,
        pos: FxVec2::from_ints(x, y),
        seen,
    });
}

fn review(w: &mut World, player: u8) -> Strategy {
    w.find_land_route_once(player);
    // Past the first review's time.
    w.state.tick = w.state.tick.max(2400);
    let census = w.survey_own(player);
    let intel = w.survey_intel(player, &census);
    w.state.ai[player as usize].strategy.next_review = 0;
    w.review_strategy(player, &census, &intel);
    w.state.ai[player as usize].strategy
}

fn hold(w: &mut World, player: u8, g: Gambit) {
    w.state.ai[player as usize].strategy.active |= 1 << g as u8;
    w.state.ai[player as usize].strategy.next_review = u32::MAX;
}

#[test]
fn a_side_holds_as_many_plans_as_its_skill_allows_commits_then_moves_on() {
    let mut w = world();
    spawn(&mut w, "aster_t3_engineer", 0, 1000, 1000);
    remember(&mut w, 0, "aster_t1_land_factory", 7000, 7000);
    // A fortified front: a landing round it and a siege both have a case.
    for i in 0..6 {
        remember(&mut w, 0, "aster_t1_point_defense", 6000 + i * 60, 6200);
    }
    let first = review(&mut w, 0);
    assert_eq!(
        first.active.count_ones(),
        3,
        "Hard holds three: {}",
        first.names()
    );
    // Nothing changed: a held plan's bonus keeps the same three.
    let again = review(&mut w, 0);
    assert_eq!(
        again.active,
        first.active,
        "{} -> {}",
        first.names(),
        again.names()
    );
    // Held long enough, a plan gives way to one not yet tried.
    let mut later = again;
    for _ in 0..6 {
        later = review(&mut w, 0);
    }
    assert_ne!(later.active, first.active, "{} all game", first.names());
    w.state.ai[0].config.difficulty = Difficulty::Easy;
    assert_eq!(review(&mut w, 0).active.count_ones(), 1);
}

#[test]
fn a_slot_stays_empty_rather_than_hold_a_plan_with_nothing_to_do() {
    let mut w = world();
    // A commander and a land factory, nothing of the enemy seen but its
    // start: no engineers to hunt, no fortified front, no silo in the menu.
    spawn(&mut w, "aster_commander", 0, 1000, 1000);
    spawn(&mut w, "aster_t1_land_factory", 0, 1100, 1000);
    remember(&mut w, 0, "aster_commander", 7000, 7000);
    let s = review(&mut w, 0);
    assert_eq!(s.names(), "scouting");
}

#[test]
fn a_nuke_race_is_run_only_while_no_interceptor_has_been_seen() {
    let mut w = world();
    spawn(&mut w, "aster_t3_engineer", 0, 1000, 1000);
    // The enemy base scouted, no interceptor in it.
    remember(&mut w, 0, "aster_t1_land_factory", 7000, 7000);
    assert!(review(&mut w, 0).holds(Gambit::NukeRace));
    remember(&mut w, 0, "aster_t3_nuke_defense", 6900, 7000);
    assert!(!review(&mut w, 0).holds(Gambit::NukeRace), "answered");
}

#[test]
fn submarines_go_against_an_enemy_at_sea_with_no_sonar() {
    let mut w = world();
    spawn(&mut w, "aster_t2_naval_factory", 0, 1000, 1400);
    remember(&mut w, 0, "aster_t1_frigate", 6000, 7000);
    let census = w.survey_own(0);
    let intel = w.survey_intel(0, &census);
    let menu = w.side_menu(0);
    let persona = Personality::Expander;
    let score = |w: &World| w.gambit_score(0, Gambit::Submarines, &census, &intel, &menu, persona);
    assert!(score(&w) > 0, "a fleet with no sonar: subs");
    for i in 0..3 {
        remember(&mut w, 0, "aster_t1_sonar", 6000 + i * 100, 6800);
    }
    assert_eq!(score(&w), 0, "sonar all over their sea");
}

#[test]
fn a_warship_strike_jumps_short_of_its_target_and_attacks_in() {
    let mut w = world();
    let frigate = spawn(&mut w, "aster_t3_frigate", 0, 1280, 1280);
    remember(&mut w, 0, "aster_t1_land_factory", 6500, 6500);
    let census = w.survey_own(0);
    let intel = w.survey_intel(0, &census);
    let mut out = vec![];
    w.direct_capital(0, &census, &intel, FxVec2::from_ints(1000, 1000), &mut out);
    let warp = out.iter().find_map(|c| match c {
        Command::Warp { units, pos, .. } => Some((units.clone(), *pos)),
        _ => None,
    });
    let (units, mark) = warp.expect("a heavy frigate alone strikes by warp");
    assert_eq!(units, vec![w.state.units.id(frigate)]);
    let target = FxVec2::from_ints(6500, 6500);
    let short = target.distance(mark);
    assert!(
        short > Fx::from_int(200) && short < Fx::from_int(300),
        "{short:?} short"
    );
    assert!(out.iter().any(|c| matches!(
        c,
        Command::AttackMove { target: t, queue: true, .. } if *t == target
    )));
}

#[test]
fn a_jump_comes_out_clear_of_a_remembered_dampener() {
    let mut w = world();
    remember(&mut w, 0, "aster_t2_warp_damper", 6400, 6400);
    let radius = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t2_warp_damper").unwrap())
        .warp_damper
        .unwrap()
        .radius;
    let mark = w.safe_mark(
        0,
        FxVec2::from_ints(6500, 6500),
        FxVec2::from_ints(1000, 1000),
    );
    assert!(mark.distance(FxVec2::from_ints(6400, 6400)) > radius);
}

#[test]
fn a_hurt_warship_out_at_the_front_jumps_home() {
    let mut w = world();
    let frigate = spawn(&mut w, "aster_t3_frigate", 0, 6000, 6000);
    let full = w.bp(frigate).health;
    w.state.units.health[frigate] = full * Fx::ratio(1, 3);
    let census = w.survey_own(0);
    let intel = w.survey_intel(0, &census);
    let mut out = vec![];
    w.direct_capital(0, &census, &intel, FxVec2::from_ints(1000, 1000), &mut out);
    assert!(
        out.iter().any(|c| matches!(c, Command::Warp { pos, .. }
            if pos.distance(FxVec2::from_ints(1000, 1000)) < Fx::from_int(500))),
        "{out:?}"
    );
}

#[test]
fn a_bomber_fleet_waits_behind_the_base_for_a_big_wing_and_hits_unguarded_mines() {
    let mut w = world();
    hold(&mut w, 0, Gambit::AirFleet);
    let (start, staging) = (FxVec2::from_ints(1000, 1000), FxVec2::from_ints(1240, 1240));
    // A guarded mine near, an open one further.
    remember(&mut w, 0, "aster_core_mine", 5000, 5000);
    for i in 0..4 {
        remember(&mut w, 0, "aster_t2_aa", 5000 + i * 40, 5050);
    }
    remember(&mut w, 0, "aster_core_mine", 6000, 3000);
    let hangar = offset_toward(start, start + (start - staging), Fx::from_int(350));
    let mut bombers = vec![];
    for i in 0..4 {
        let at = hangar + FxVec2::from_ints(i * 20, 0);
        bombers.push(spawn(
            &mut w,
            "aster_t1_bomber",
            0,
            at.x.floor_int(),
            at.y.floor_int(),
        ));
    }
    let intel = Intel::default();
    let mut out = vec![];
    w.direct_air(0, &w.survey_own(0), &intel, start, staging, &mut out);
    assert!(
        !out.iter().any(|c| matches!(c, Command::AttackMove { .. })),
        "four is a wing, not a fleet"
    );
    for i in 4..8 {
        let at = hangar + FxVec2::from_ints(i * 20, 0);
        spawn(
            &mut w,
            "aster_t1_bomber",
            0,
            at.x.floor_int(),
            at.y.floor_int(),
        );
    }
    let mut out = vec![];
    w.direct_air(0, &w.survey_own(0), &intel, start, staging, &mut out);
    let targets: Vec<FxVec2> = out
        .iter()
        .filter_map(|c| match c {
            Command::AttackMove { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(
        targets,
        vec![FxVec2::from_ints(6000, 3000)],
        "the unguarded mine"
    );
}

#[test]
fn a_hunt_sends_the_fastest_few_after_an_engineer_on_the_outskirts() {
    let mut w = world();
    hold(&mut w, 0, Gambit::Hunt);
    remember(&mut w, 0, "aster_t1_engineer", 4000, 6800);
    let mut at_stage: Vec<usize> = (0..8)
        .map(|i| spawn(&mut w, "aster_t1_tank", 0, 1200 + i * 15, 1200))
        .collect();
    let mut out = vec![];
    w.direct_hunt(0, &mut at_stage, FxVec2::from_ints(1200, 1200), &mut out);
    let sent = out.iter().find_map(|c| match c {
        Command::AttackMove { units, target, .. } => Some((units.len(), *target)),
        _ => None,
    });
    assert_eq!(sent, Some((6, FxVec2::from_ints(4000, 6800))));
    assert_eq!(at_stage.len(), 2, "the rest stay for the wave");
    let mut out = vec![];
    w.direct_hunt(0, &mut at_stage, FxVec2::from_ints(1200, 1200), &mut out);
    assert!(out.is_empty(), "one squad a minute");
}

/// Runs player 0's landing every think for `ticks`, applying what it orders:
/// whether the ship jumped, and where its cargo was sent to attack.
fn run_landing(w: &mut World, ticks: u32, staging: FxVec2, ship: UnitId) -> (bool, Vec<FxVec2>) {
    let start = w.state.players[0].start;
    let (mut jumped, mut attacks) = (false, vec![]);
    for t in 0..ticks {
        let mut out = vec![];
        if t % 20 == 0 {
            let census = w.survey_own(0);
            let intel = w.survey_intel(0, &census);
            let mut idle = census.army_idle.clone();
            w.direct_landing(0, &census, &intel, start, staging, 4, &mut idle, &mut out);
        }
        for c in &out {
            if let Command::AttackMove { target, .. } = c {
                attacks.push(*target);
            }
        }
        let cmds: Vec<PlayerCommand> = out
            .into_iter()
            .map(|command| PlayerCommand { player: 0, command })
            .collect();
        w.tick(&cmds).unwrap();
        if let Some(r) = w.state.units.row(ship) {
            jumped |= w.state.units.warp[r].phase == WarpPhase::Transit;
        }
    }
    (jumped, attacks)
}

#[test]
fn a_courier_carries_a_squad_across_by_warp_and_lets_it_out_by_its_target() {
    let mut w = world();
    hold(&mut w, 0, Gambit::Landing);
    let staging = FxVec2::from_ints(1240, 1240);
    let courier = spawn(&mut w, "aster_t1_lift_ship", 0, 1150, 1150);
    let ship = w.state.units.id(courier);
    let rows: Vec<usize> = (0..6)
        .map(|i| spawn(&mut w, "aster_t1_tank", 0, 1240 + i * 18, 1260))
        .collect();
    let tanks: Vec<UnitId> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    let target = FxVec2::from_ints(6200, 5200);
    remember(&mut w, 0, "aster_core_mine", 6200, 5200);
    let (jumped, attacks) = run_landing(&mut w, 2000, staging, ship);
    assert!(jumped, "the courier never jumped");
    let ashore = tanks
        .iter()
        .filter_map(|&id| w.state.units.row(id))
        .filter(|&r| {
            w.state.units.hangar[r] == Handle::NONE
                && w.state.units.pos[r].distance(target) < Fx::from_int(900)
        })
        .count();
    assert!(ashore >= 5, "{ashore} of 6 put ashore by the target");
    assert_eq!(attacks, vec![target], "those put ashore go for the target");
    assert_eq!(
        w.state.ai[0].landing, None::<Landing>,
        "the landing is over"
    );
}

#[test]
fn a_sensor_ship_jumps_out_to_look_at_the_enemy_and_home_when_shot_at() {
    let mut w = world();
    let vigil = spawn(&mut w, "aster_t1_sensor_ship", 0, 1100, 1100);
    let start = FxVec2::from_ints(1000, 1000);
    let mut out = vec![];
    w.direct_sensor_ships(0, &w.survey_own(0), start, &mut out);
    let mark = out.iter().find_map(|c| match c {
        Command::Warp { pos, .. } => Some(*pos),
        _ => None,
    });
    let mark = mark.expect("an idle Vigil jumps out to look");
    let enemy = FxVec2::from_ints(7000, 7000);
    assert!(mark.distance(enemy) < Fx::from_int(1000), "{mark:?}");
    // Out there, with anti-air beside it: home.
    w.state.units.pos[vigil] = mark;
    remember(&mut w, 0, "aster_t2_aa", 6500, 6500);
    let mut out = vec![];
    w.direct_sensor_ships(0, &w.survey_own(0), start, &mut out);
    assert!(
        out.iter()
            .any(|c| matches!(c, Command::Warp { pos, .. } if *pos == start)),
        "{out:?}"
    );
}
