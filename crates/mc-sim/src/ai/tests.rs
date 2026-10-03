use super::*;

use crate::world::MapData;
use crate::{Difficulty, MatchConfig, PlayerSetup};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

fn world() -> World {
    world_of(256)
}

/// A flat map `cells` 8 m cells a side.
fn world_of(cells: u32) -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 47,
        cheats: true,
        fog: true,
        spawn_commanders: false,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                team: i,
                controller: Controller::Human,
                start: i,
                ai: AiConfig::default(),
            })
            .collect(),
    };
    World::with_terrain(
        Heightfield::flat(cells, cells, Fx::from_int(20)),
        MapData {
            name: "AI test".into(),
            content_id: 47,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(1700, 1700)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap()
}

/// Mine points every `pitch` metres across the map, on the 3x3 lot grid.
fn point_grid(w: &mut World, pitch: i32) {
    let size = w.terrain.size_metres().x.floor_int();
    w.mine_points = (1..size / pitch)
        .flat_map(|j| (1..size / pitch).map(move |i| (i, j)))
        .map(|(i, j)| FxVec2::from_ints(i * pitch / 12 * 12 + 6, j * pitch / 12 * 12 + 6))
        .collect();
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

fn contact(w: &mut World, key: &str, count: u32) {
    let blueprint = w.blueprints.id_of(key).unwrap();
    w.state.ai[0].contacts = (0..count)
        .map(|i| Contact {
            id: UnitId::new(i as usize, 0),
            blueprint,
            pos: FxVec2::from_ints(1000, 1000),
            seen: w.state.tick,
        })
        .collect();
}

#[test]
fn a_force_weighted_to_zero_is_never_produced() {
    let mut w = world();
    let choices = [
        w.blueprints.id_of("aster_t1_tank").unwrap(),
        w.blueprints.id_of("aster_t1_mobile_aa").unwrap(),
    ];
    assert!(w
        .solve_production(0, &choices, &Default::default(), 3)
        .is_some());
    w.state.ai[0].config.domain_weights = [0, 100, 100];
    assert_eq!(
        w.solve_production(0, &choices, &Default::default(), 3),
        None
    );
}

#[test]
fn memory_never_sees_hidden_or_unidentified_radar_units() {
    let mut w = world();
    let enemy = spawn(&mut w, "aster_t1_tank", 1, 1200, 1200);
    w.update_fog();
    w.remember_enemies(0);
    assert!(w.state.ai[0].contacts.is_empty());
    w.fog.reveal(
        FxVec2::from_ints(1200, 1200),
        Fx::ZERO,
        Fx::from_int(500),
        1,
    );
    w.remember_enemies(0);
    assert!(
        w.state.ai[0].contacts.is_empty(),
        "unidentified radar contact must not reveal blueprint"
    );
    w.fog.reveal(
        FxVec2::from_ints(1200, 1200),
        Fx::from_int(100),
        Fx::ZERO,
        1,
    );
    w.fog
        .identify(enemy, w.state.units.id(enemy).generation(), 1);
    w.remember_enemies(0);
    assert_eq!(w.state.ai[0].contacts.len(), 1);
    let remembered = w.state.ai[0].contacts[0].pos;
    w.fog.begin();
    w.state.units.pos[enemy] = FxVec2::from_ints(1600, 1600);
    w.state.tick += 60;
    w.remember_enemies(0);
    assert_eq!(
        w.state.ai[0].contacts[0].pos, remembered,
        "hidden movement must not update memory"
    );
    w.fog.reveal(remembered, Fx::from_int(100), Fx::ZERO, 1);
    w.remember_enemies(0);
    assert!(
        w.state.ai[0].contacts.is_empty(),
        "scouting an empty old position clears the sighting"
    );
}

#[test]
fn doctrine_is_independent_of_player_slot_and_difficulty_has_no_income_bonus() {
    let mut w = world();
    let c = Census::default();
    let i = Intel::default();
    for slot in 0..2 {
        w.state.ai[slot].config.doctrine = Doctrine::Aggressive;
        assert!(matches!(
            w.ai_personality(slot as u8, &c, &i),
            Personality::Aggressive
        ));
        w.state.ai[slot].config.doctrine = Doctrine::Defensive;
        assert!(matches!(
            w.ai_personality(slot as u8, &c, &i),
            Personality::Turtle
        ));
    }
    let mut easy = w.state.ai[0].config;
    easy.difficulty = Difficulty::Easy;
    let mut hard = easy;
    hard.difficulty = Difficulty::Hard;
    assert!(hard.think_period() < easy.think_period());
    assert!(hard.memory_ticks() > easy.memory_ticks());
    spawn(&mut w, "aster_commander", 0, 300, 300);
    spawn(&mut w, "aster_commander", 1, 1700, 1700);
    w.state.ai[0].config = easy;
    w.state.ai[1].config = hard;
    w.tick(&[]).unwrap();
    assert_eq!(
        w.state.players[0].mass_income,
        w.state.players[1].mass_income
    );
    assert_eq!(
        w.state.players[0].energy_income,
        w.state.players[1].energy_income
    );
}

#[test]
fn air_army_and_busy_scouts_count_and_config_memory_survive_snapshot() {
    let mut w = world();
    spawn(&mut w, "aster_t1_bomber", 0, 500, 500);
    let scout = spawn(&mut w, "aster_t1_scout", 0, 600, 600);
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::Move {
            units: vec![w.state.units.id(scout)],
            target: FxVec2::from_ints(1200, 1200),
            queue: false,
        },
    })
    .unwrap();
    let c = w.survey_own(0);
    assert!(c.army >= 1);
    assert_eq!(c.scouts, 1);
    w.state.ai[0].config.doctrine = Doctrine::Economic;
    contact(&mut w, "aster_t1_bomber", 3);
    let hash = w.hash();
    w.state.ai[0].config.domain_weights = [100, 50, 100];
    assert_ne!(
        hash,
        w.hash(),
        "configuration must affect deterministic state hash"
    );
    let bytes = w.snapshot();
    let mut restored = world();
    restored
        .restore(Heightfield::flat(256, 256, Fx::from_int(20)), &bytes)
        .unwrap();
    assert_eq!(w.hash(), restored.hash());
    assert_eq!(w.state.ai[0].config, restored.state.ai[0].config);
    for _ in 0..60 {
        assert_eq!(w.tick(&[]).unwrap(), restored.tick(&[]).unwrap());
    }
}

#[test]
fn static_enemy_fortifications_do_not_pin_the_army_in_raid_defense() {
    let mut w = world();
    w.state.fog_enabled = false;
    let defense = w
        .blueprints
        .units
        .iter()
        .find(|bp| bp.has(cat::DEFENSE | cat::DIRECT_FIRE))
        .unwrap()
        .id;
    w.spawn_unit(defense, 1, FxVec2::from_ints(650, 300), Angle::ZERO, true)
        .unwrap();
    w.rebuild_index();
    let c = w.survey_own(0);
    assert!(w.survey_intel(0, &c).threats.is_empty());
}

#[test]
fn configured_economy_duel_launches_attacks_and_sustains_combat() {
    let mut w = world();
    w.state.players[0].controller = Controller::Ai;
    w.state.players[1].controller = Controller::Ai;
    w.state.ai[0].config.doctrine = Doctrine::Aggressive;
    w.state.ai[1].config.doctrine = Doctrine::Economic;
    w.state.ai[0].config.difficulty = Difficulty::Hard;
    for p in 0..2u8 {
        let start = w.state.players[p as usize].start;
        let commander = w.blueprints.factions[0].commander;
        let row = w
            .spawn_unit(commander, p, start, Angle::ZERO, true)
            .unwrap();
        w.state.players[p as usize].commander = w.state.units.id(row);
        let sign = if p == 0 { 1 } else { -1 };
        for (x, y) in [(300, 0), (0, 300), (-260, -160), (330, 350)] {
            let c = start + FxVec2::from_ints(x * sign, y * sign);
            w.map.ore.push(mc_map::OreRegion {
                points: [(-40, -40), (40, -40), (40, 40), (-40, 40)]
                    .into_iter()
                    .map(|(dx, dy)| c + FxVec2::from_ints(dx, dy))
                    .collect(),
            });
        }
    }
    w.mine_points = w.find_mine_points();
    for _ in 0..12000 {
        w.tick(&[]).unwrap();
        if w.state.winner.is_some() {
            break;
        }
    }
    eprintln!(
        "duel tick={} winner={:?} A={} B={} kills={}/{}",
        w.state.tick,
        w.state.winner,
        w.state.ai[0].summary(),
        w.state.ai[1].summary(),
        w.state.players[0].units_killed,
        w.state.players[1].units_killed
    );
    assert!(
        w.state.players.iter().any(|p| p.units_killed > 3),
        "opponents must actually fight"
    );
    assert!(
        w.state.players.iter().all(|p| p.units_built > 20),
        "both economies should function"
    );
}

#[test]
fn ai_converts_a_decisive_army_advantage_into_a_win() {
    let mut w = world();
    w.state.players[0].controller = Controller::Ai;
    w.state.ai[0].config.doctrine = Doctrine::Aggressive;
    w.state.ai[0].config.difficulty = Difficulty::Hard;
    for p in 0..2u8 {
        let start = w.state.players[p as usize].start;
        let row = w
            .spawn_unit(
                w.blueprints.factions[0].commander,
                p,
                start,
                Angle::ZERO,
                true,
            )
            .unwrap();
        w.state.players[p as usize].commander = w.state.units.id(row);
    }
    for n in 0..10 {
        spawn(&mut w, "aster_t3_assault_bot", 0, 500 + n * 18, 500);
    }
    for _ in 0..6000 {
        w.tick(&[]).unwrap();
        if w.state.winner.is_some() {
            break;
        }
    }
    assert_eq!(
        w.state.winner,
        Some(0),
        "a superior AI army must find and kill the known enemy commander"
    );
}

#[test]
fn builders_are_reassigned_after_finishing_assistance_and_upgrades_wait_for_power() {
    let mut w = world();
    let builder = spawn(&mut w, "aster_t1_engineer", 0, 320, 300);
    let power = spawn(&mut w, "aster_t1_power", 0, 380, 300);
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::Assist {
            units: vec![w.state.units.id(builder)],
            target: w.state.units.id(power),
            queue: false,
        },
    })
    .unwrap();
    assert!(
        w.survey_own(0).builders_idle.contains(&builder),
        "completed assist must not trap an engineer forever"
    );
    let factory = spawn(&mut w, "aster_t1_land_factory", 0, 500, 500);
    spawn(&mut w, "aster_t1_point_defense", 0, 650, 500);
    spawn(&mut w, "aster_t1_radar", 0, 650, 650);
    spawn(&mut w, "aster_t1_power", 0, 750, 650);
    w.state.players[0].mass = Fx::from_int(650);
    w.state.players[0].mass_income = Fx::from_int(20);
    w.state.players[0].energy_income = Fx::from_int(40);
    w.state.players[0].energy_demand = Fx::from_int(400);
    let mut out = vec![];
    w.direct_upgrades(0, &w.survey_own(0), &mut out);
    assert!(
        out.is_empty(),
        "an energy-starved economy must not queue more upgrades"
    );
    assert!(w.survey_own(0).factories_idle.contains(&factory));
}

#[test]
fn upgraded_factory_trains_a_tech_builder_even_with_many_old_engineers() {
    let mut w = world();
    spawn(&mut w, "aster_t2_land_factory", 0, 400, 400);
    for i in 0..10 {
        spawn(&mut w, "aster_t1_engineer", 0, 300 + i * 10, 600);
    }
    let census = w.survey_own(0);
    let mut out = vec![];
    w.direct_factories(0, &census, &[], &mut out);
    let engineer = w.blueprints.id_of("aster_t2_engineer").unwrap();
    assert!(
        out.iter()
            .any(|c| matches!(c,Command::Produce {blueprint,..} if *blueprint==engineer)),
        "tech access must not be blocked by the existing low-tier engineer count"
    );
}

#[test]
fn the_next_mine_goes_on_the_nearest_free_point() {
    let mut w = world_of(1024);
    point_grid(&mut w, 600);
    let start = FxVec2::from_ints(1800, 800);
    let intel = Intel::default();
    let nearest = |w: &World| w.free_deposit(start, &[], Fx::from_int(3000), &intel);
    let first = nearest(&w).expect("a free point in range");
    assert!(w.mine_points.contains(&first));
    // Taken by anyone's mine, it is passed over for the next nearest.
    spawn(
        &mut w,
        "aster_core_mine",
        1,
        first.x.floor_int(),
        first.y.floor_int(),
    );
    let second = nearest(&w).expect("another point");
    assert_ne!(second, first);
    assert!(second.distance(start) >= first.distance(start));
    assert!(
        w.free_deposit(start, &[], Fx::from_int(10), &intel)
            .is_none(),
        "nothing out of range"
    );
}

#[test]
fn mine_upgrades_go_to_the_mine_that_pays_back_soonest() {
    let mut w = world_of(1024);
    // A tech 1 mine climbs to tech 2 for a quicker payback than a tech 2 to tech 3.
    let low = spawn(&mut w, "aster_core_mine", 0, 606, 606);
    let high = spawn(&mut w, "aster_core_mine_t2", 0, 1206, 606);
    spawn(&mut w, "aster_t3_engineer", 0, 400, 400);
    w.tick(&[]).unwrap();
    w.state.players[0].mass = Fx::from_int(800);
    w.state.players[0].mass_income = Fx::from_int(20);
    let eco = &mut w.state.ai[0].commander.eco;
    eco.upgrades = 2;
    eco.payback = 600;
    let mut census = w.survey_own(0);
    assert_eq!(w.mine_to_upgrade(0, &census).map(|(r, _)| r), Some(low));

    // With the tech 1 mine taken, the tech 2 one pays back too slowly for that
    // horizon, and in time for a longer one.
    census.extractors.retain(|&r| r != low);
    assert_eq!(w.mine_to_upgrade(0, &census).map(|(r, _)| r), None);
    w.state.ai[0].commander.eco.payback = 1200;
    assert_eq!(w.mine_to_upgrade(0, &census).map(|(r, _)| r), Some(high));
}

#[test]
fn aircraft_and_ships_by_a_mine_do_not_pin_the_land_army() {
    let mut w = world();
    w.state.fog_enabled = false;
    let gunship = w
        .blueprints
        .units
        .iter()
        .find(|bp| bp.has(cat::AIR) && bp.is_mobile() && !bp.weapons.is_empty())
        .unwrap()
        .id;
    let boat = w.blueprints.id_of("aster_t1_attack_boat").unwrap();
    w.spawn_unit(gunship, 1, FxVec2::from_ints(400, 300), Angle::ZERO, true)
        .unwrap();
    w.spawn_unit(boat, 1, FxVec2::from_ints(300, 400), Angle::ZERO, true)
        .unwrap();
    w.rebuild_index();
    let c = w.survey_own(0);
    assert!(w.survey_intel(0, &c).threats.is_empty());
    spawn(&mut w, "aster_t1_tank", 1, 350, 350);
    w.rebuild_index();
    assert_eq!(
        w.survey_intel(0, &c).threats.len(),
        1,
        "a tank is still a threat"
    );
}

#[test]
fn a_planned_mine_claims_its_point() {
    let mut w = world();
    point_grid(&mut w, 600);
    let start = FxVec2::from_ints(1000, 1000);
    let open = w
        .free_deposit(start, &[], Fx::from_int(2000), &Intel::default())
        .unwrap();
    // A builder already walking to that point.
    let claim = Claim {
        pos: open,
        foot: 3,
        factory: false,
        cover: Fx::ZERO,
    };
    let next = w.free_deposit(start, &[claim], Fx::from_int(2000), &Intel::default());
    assert!(next.is_none_or(|p| p != open), "{next:?}");
}

/// A 400 m square shelf, 100 m up a sheer cliff, under player 0's start.
fn shelf_world() -> World {
    let mut w = world();
    let mut samples = vec![0u16; 257 * 257];
    for y in 0..50 {
        for x in 0..50 {
            samples[y * 257 + x] = 100;
        }
    }
    w.terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(-50));
    w.nav = crate::nav::Nav::new(&w.terrain, w.pool.clone()).unwrap();
    w
}

#[test]
fn the_army_stages_on_its_own_shelf_not_below_the_cliff() {
    let w = shelf_world();
    let start = FxVec2::from_ints(200, 200);
    let below = FxVec2::from_ints(470, 470);
    let ground = w.home_ground(start, 1).unwrap();
    assert!(!ground.reaches(below));
    let staging = w.reachable_staging(start, below, &ground);
    assert!(ground.reaches(staging), "{staging:?}");
    assert!(staging.distance(start) > Fx::from_int(60), "{staging:?}");
}

#[test]
fn base_buildings_stay_on_ground_walkable_from_home() {
    let w = shelf_world();
    let start = FxVec2::from_ints(200, 200);
    let ground = w.home_ground(start, 1).unwrap();
    let factory = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_land_factory").unwrap())
        .clone();
    let site = w
        .find_site(
            &factory,
            FxVec2::from_ints(470, 470),
            &[],
            Angle::ZERO,
            Fx::ZERO,
            true,
            Some(&ground),
        )
        .unwrap();
    assert!(ground.reaches(site), "{site:?}");
}

#[test]
fn buildings_keep_a_factorys_exit_and_a_lane_clear() {
    let mut w = world();
    let factory = w.blueprints.id_of("aster_t1_land_factory").unwrap();
    let at = FxVec2::from_ints(1000, 1000);
    let row = w
        .spawn_unit(factory, 0, at, AI_BUILD_HEADING, true)
        .unwrap();
    w.state.units.heading[row] = AI_BUILD_HEADING;
    w.rebuild_index();
    let fbp = w.bp(row).clone();
    let half = fbp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2;
    let storage = w
        .blueprints
        .units
        .iter()
        .find(|b| b.has(cat::STORAGE) && b.is_structure())
        .unwrap()
        .clone();
    // Right in front of the exit: refused, and the search lands elsewhere.
    let front = at - FxVec2::from_ints(0, half + 20);
    assert!(!w.keeps_lanes(&storage, front, &[]));
    let site = w
        .find_site(&storage, front, &[], Angle::ZERO, Fx::ZERO, false, None)
        .unwrap();
    assert!(w.keeps_lanes(&storage, site, &[]));
    let sh = storage.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2;
    let d = site - at;
    let (gx, gy) = (
        d.x.abs().round_int() - half - sh,
        d.y.abs().round_int() - half - sh,
    );
    assert!(gx.max(gy) >= 24, "a lane stays open: {site:?}");
    // Nothing on the exit's apron (4 cells deep) or within a lane of it.
    let beside = d.x.abs().round_int() - half - sh;
    let below = (at.y - site.y).round_int() - half - 48 - sh;
    assert!(
        d.y > Fx::ZERO || beside >= 24 || below >= 24,
        "the exit stays open: {site:?}"
    );
}

#[test]
fn builders_keep_off_ground_where_their_buildings_were_just_shot_down() {
    let mut w = world();
    w.state.players[0].controller = Controller::Ai;
    let spot = FxVec2::from_ints(900, 900);
    let kill = |w: &mut World| {
        let pd = spawn(w, "aster_t1_point_defense", 0, 900, 900);
        w.state.units.health[pd] = Fx::ZERO;
        w.tick(&[]).unwrap();
    };
    kill(&mut w);
    let hot = |w: &World, p: FxVec2| w.ai_danger(0).hot(p);
    assert!(
        hot(&w, spot),
        "a builder does not walk straight back to rebuild it"
    );
    assert!(hot(&w, spot + FxVec2::from_ints(150, 0)), "nor next to it");
    assert!(
        !hot(&w, spot + FxVec2::from_ints(400, 0)),
        "ground further off is fine"
    );
    for _ in 0..460 {
        w.tick(&[]).unwrap();
    }
    assert!(!hot(&w, spot), "one loss keeps them off for 45 s");
    // Lost again, and again: they stay off longer each time.
    kill(&mut w);
    kill(&mut w);
    for _ in 0..460 {
        w.tick(&[]).unwrap();
    }
    assert!(hot(&w, spot), "a spot lost three times stays hot past 45 s");
    for _ in 0..1000 {
        w.tick(&[]).unwrap();
    }
    assert!(!hot(&w, spot), "and is tried again once it cools");
}

#[test]
fn builders_do_not_start_or_help_build_under_an_enemys_guns() {
    let mut w = world_of(1024);
    point_grid(&mut w, 300);
    let start = FxVec2::from_ints(1800, 800);
    let first = w
        .free_deposit(start, &[], Fx::from_int(3000), &Intel::default())
        .unwrap();
    // An enemy turret the AI has seen, standing on that spot.
    let gun = spawn(
        &mut w,
        "aster_t1_point_defense",
        1,
        first.x.floor_int(),
        first.y.floor_int(),
    );
    let reach = w
        .bp(gun)
        .weapons
        .iter()
        .map(|wp| wp.range_max)
        .max()
        .unwrap();
    w.state.ai[0].contacts = vec![Contact {
        id: w.state.units.id(gun),
        blueprint: w.state.units.blueprint[gun],
        pos: w.state.units.pos[gun],
        seen: w.state.tick,
    }];
    let intel = Intel {
        danger: w.ai_danger(0),
        ..Intel::default()
    };
    let spot = w
        .free_deposit(start, &[], Fx::from_int(3000), &intel)
        .expect("somewhere out of its reach");
    assert!(spot.distance(first) > reach, "no new mine under its guns");

    // A site of its own the turret covers is not worth another engineer.
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    let site = w
        .spawn_unit(
            power,
            0,
            first + FxVec2::from_ints(40, 0),
            Angle::ZERO,
            false,
        )
        .unwrap();
    // Short of power: a power site is the first thing an idle builder helps with.
    w.state.players[0].energy_demand = Fx::from_int(100);
    w.state.players[0].energy_spent = Fx::from_int(100);
    let builder = spawn(&mut w, "aster_t1_engineer", 0, 1700, 800);
    let mut census = w.survey_own(0);
    census.sites = vec![site];
    census.builders_idle = vec![builder];
    let mut out = vec![];
    w.direct_builders(
        0,
        &census,
        &intel,
        Stance::Expand,
        Personality::Expander,
        start,
        Angle::ZERO,
        None,
        &mut vec![],
        &mut Planned {
            factories: 1,
            anti_air: 0,
            engineer_factories: 1,
            air_factories: 0,
            naval_factories: 0,
            power: 0,
            radars: vec![],
            pd: 0,
            artillery: 0,
            shields: 0,
            storage: 0,
            towers: vec![],
            salvage: vec![],
            projects: 0,
            guards: vec![],
            failed_mines: vec![],
        },
        &mut out,
    );
    let target = w.state.units.id(site);
    assert!(
        !out.iter()
            .any(|c| matches!(c, Command::Assist { target: t, .. } if *t == target)),
        "{out:?}"
    );
}

/// A flat map with a lake a few hundred metres west of (1000, 1000).
fn lake_behind_world() -> World {
    let mut w = world();
    let mut samples = vec![40u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            // 8 m cells: x 520..800 m, y 560..1440 m is under water.
            if (65..100).contains(&x) && (70..180).contains(&y) {
                samples[y * 257 + x] = 0;
            }
        }
    }
    w.terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20));
    w.nav = crate::nav::Nav::new(&w.terrain, w.pool.clone()).unwrap();
    w
}

#[test]
fn power_packs_into_a_block_on_open_ground_not_along_a_shore() {
    let mut w = lake_behind_world();
    let start = FxVec2::from_ints(1000, 1000);
    // The enemy is east, so straight back from the start is the lake.
    let facing = Angle::ZERO;
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_power").unwrap())
        .clone();
    let home = w.home_ground(start, 1);
    let mut plants = Vec::new();
    for _ in 0..20 {
        let site = w.farm_site(&bp, start, facing, &[], home.as_ref()).unwrap();
        spawn(
            &mut w,
            "aster_t1_power",
            0,
            site.x.round_int(),
            site.y.round_int(),
        );
        w.rebuild_index();
        plants.push(site);
    }
    // Every plant stands flush against another one: one block, no strays.
    for p in &plants {
        assert!(
            plants
                .iter()
                .any(|q| q != p && q.distance(*p) <= Fx::from_int(25)),
            "{p:?} stands alone in {plants:?}"
        );
    }
    let (x0, x1) = plants.iter().fold((i32::MAX, i32::MIN), |(a, b), p| {
        (a.min(p.x.round_int()), b.max(p.x.round_int()))
    });
    let (y0, y1) = plants.iter().fold((i32::MAX, i32::MIN), |(a, b), p| {
        (a.min(p.y.round_int()), b.max(p.y.round_int()))
    });
    let (wide, high) = (x1 - x0 + 24, y1 - y0 + 24);
    assert!(
        wide.max(high) <= 2 * wide.min(high),
        "a strip, not a block: {wide} x {high} m"
    );
    // Clear of the water: a lot at least one plant's width from the shore.
    for p in &plants {
        assert!(
            p.x.round_int() > 800 + 24
                || p.x.round_int() < 520 - 24
                || p.y.round_int() > 1440 + 24
                || p.y.round_int() < 560 - 24,
            "{p:?} on the shore"
        );
    }
}

#[test]
fn a_shield_goes_only_where_it_covers_something_worth_it() {
    let mut w = world();
    let start = FxVec2::from_ints(1000, 1000);
    let bp = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t2_shield").unwrap())
        .clone();
    // Two tech 1 factories and some radar are not worth a shield's cost.
    spawn(&mut w, "aster_t1_land_factory", 0, 1000, 1000);
    spawn(&mut w, "aster_t1_air_factory", 0, 1120, 1000);
    spawn(&mut w, "aster_t1_radar", 0, 1060, 1100);
    w.rebuild_index();
    assert!(w.shield_spot(0, &bp, start, &[]).is_none());
    // A tech 2 factory is: the shield stands where it covers it.
    let t2 = spawn(&mut w, "aster_t2_land_factory", 0, 1000, 1180);
    w.rebuild_index();
    let spot = w
        .shield_spot(0, &bp, start, &[])
        .expect("worth a shield now");
    let site = w
        .shield_site(0, &bp, spot, start, &[], None)
        .expect("room for it");
    let radius = bp.shield.as_ref().unwrap().radius;
    assert!(site.distance(w.state.units.pos[t2]) < radius, "{site:?}");
    // Once it stands, nothing more is worth another.
    spawn(
        &mut w,
        "aster_t2_shield",
        0,
        site.x.round_int(),
        site.y.round_int(),
    );
    w.rebuild_index();
    assert!(w.shield_spot(0, &bp, start, &[]).is_none());
}

#[test]
fn units_in_a_gathered_crowd_count_as_arrived_though_their_order_runs_on() {
    let mut w = world();
    let mut rows = Vec::new();
    for i in 0..30 {
        rows.push(spawn(
            &mut w,
            "aster_t1_tank",
            0,
            940 + (i % 6) * 25,
            950 + (i / 6) * 25,
        ));
    }
    let straggler = spawn(&mut w, "aster_t1_tank", 0, 1000, 1500);
    rows.push(straggler);
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::AttackMove {
            units: rows.iter().map(|&r| w.state.units.id(r)).collect(),
            target: FxVec2::from_ints(1000, 1000),
            queue: false,
        },
    })
    .unwrap();
    let census = w.survey_own(0);
    assert!(
        census.army_idle.len() >= 28,
        "the crowd at its point counts as gathered: {} of 30",
        census.army_idle.len()
    );
    assert!(
        !census.army_idle.contains(&straggler),
        "one still on the road does not"
    );
}

#[test]
fn strategic_projects_are_told_apart_by_their_data() {
    let w = world();
    let kind =
        |key: &str| projects::project_kind(w.blueprints.unit(w.blueprints.id_of(key).unwrap()));
    use projects::Project::*;
    assert_eq!(kind("aster_t4_nuke_silo"), Some(Nuke));
    assert_eq!(kind("aster_t3_nuke_defense"), Some(Interceptor));
    assert_eq!(kind("aster_t4_anti_ship"), Some(SkyGun));
    assert_eq!(kind("aster_t4_artillery"), Some(MapGun));
    assert_eq!(kind("aster_t4_assault_tank"), Some(Mobile));
    assert_eq!(kind("aster_t5_titan"), Some(Mobile));
    assert_eq!(kind("aster_t4_frigate"), Some(Mobile));
    // A transport, a mine and a plain tank are not projects.
    assert_eq!(kind("aster_t3_lift_ship"), None);
    assert_eq!(kind("aster_core_mine_t4"), None);
    assert_eq!(kind("aster_t1_tank"), None);
}

#[test]
fn a_ready_warhead_goes_at_the_enemy_commander_unless_interceptors_guard_it() {
    let mut w = world();
    let silo = spawn(&mut w, "aster_t4_nuke_silo", 0, 300, 300);
    let id = w.state.units.id(silo);
    w.state.strategic.launchers.entry(id).or_default().stock = 1;
    let commander = w.blueprints.id_of("aster_commander").unwrap();
    let at = FxVec2::from_ints(1700, 1700);
    w.state.ai[0].contacts = vec![Contact {
        id: UnitId::new(900, 0),
        blueprint: commander,
        pos: at,
        seen: w.state.tick,
    }];
    let mut out = vec![];
    w.direct_nukes(0, &mut out);
    assert!(
        matches!(out.as_slice(), [Command::LaunchNuke { pos, .. }] if *pos == at),
        "{out:?}"
    );
    // One warhead into an interceptor's cover is wasted: held until a salvo can get through.
    w.state.ai[0].contacts.push(Contact {
        id: UnitId::new(901, 0),
        blueprint: w.blueprints.id_of("aster_t3_nuke_defense").unwrap(),
        pos: FxVec2::from_ints(1650, 1650),
        seen: w.state.tick,
    });
    out.clear();
    w.direct_nukes(0, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn a_factory_goes_up_a_tier_at_the_income_mark_without_waiting_for_a_surplus() {
    let mut w = world();
    let factory = spawn(&mut w, "aster_t1_land_factory", 0, 500, 500);
    spawn(&mut w, "aster_t1_power", 0, 700, 500);
    spawn(&mut w, "aster_t1_power", 0, 700, 560);
    let skill = w.state.ai[0].config.skill();
    let pl = &mut w.state.players[0];
    // Everything it makes is spent: no pile of materials.
    pl.mass = Fx::from_int(50);
    pl.mass_income = Fx::from_int(skill.tech_income);
    pl.energy_income = Fx::from_int(2000);
    pl.energy_demand = Fx::ZERO;
    pl.energy_capacity = Fx::from_int(5000);
    pl.energy = pl.energy_capacity;
    pl.efficiency = Fx::ONE;
    let mut out = vec![];
    w.direct_upgrades(0, &w.survey_own(0), &mut out);
    let id = w.state.units.id(factory);
    assert!(
        matches!(out.as_slice(), [Command::Upgrade { units }] if units == &vec![id]),
        "{out:?}"
    );
}

#[test]
fn one_watchtower_covers_the_base_and_its_near_mines() {
    let mut w = world_of(512);
    spawn(&mut w, "aster_t1_radar", 0, 380, 420);
    let census = w.survey_own(0);
    let planned = w.plan_counts(0, &census);
    // It sees 2 km: a mine a kilometre out is covered, one two out is not.
    assert!(radar_covers(&planned.radars, FxVec2::from_ints(1100, 1000)));
    assert!(!radar_covers(
        &planned.radars,
        FxVec2::from_ints(1900, 1900)
    ));
}

#[test]
fn a_watchtower_or_scavenger_a_builder_walks_to_counts_as_planned() {
    let mut w = world_of(512);
    let engineer = spawn(&mut w, "aster_t3_engineer", 0, 300, 300);
    for (key, x) in [("aster_t1_radar", 1500), ("aster_t2_reclaimer", 1600)] {
        w.apply_command(&PlayerCommand {
            player: 0,
            command: Command::Build {
                units: vec![w.state.units.id(engineer)],
                blueprint: w.blueprints.id_of(key).unwrap(),
                pos: FxVec2::from_ints(x, 1500),
                heading: Angle::ZERO,
                queue: true,
            },
        })
        .unwrap();
    }
    let census = w.survey_own(0);
    assert!(census.radar.is_empty() && census.towers.is_empty());
    // Not yet sites, only orders: the next idle builder must still see them,
    // or each one orders another.
    let planned = w.plan_counts(0, &census);
    assert_eq!(planned.radars.len(), 1);
    assert_eq!(planned.towers.len(), 1);
}

#[test]
fn a_lesser_builder_helps_raise_a_big_plant_and_starts_no_small_one() {
    let mut w = world_of(512);
    spawn(&mut w, "aster_t2_engineer", 0, 420, 300);
    let mason = spawn(&mut w, "aster_t1_engineer", 0, 320, 320);
    let plant = w
        .spawn_unit(
            w.blueprints.id_of("aster_t2_power").unwrap(),
            0,
            FxVec2::from_ints(520, 420),
            Angle::ZERO,
            false,
        )
        .unwrap();
    let pl = &mut w.state.players[0];
    pl.energy_income = Fx::from_int(100);
    pl.energy_demand = Fx::from_int(300);
    pl.energy_spent = Fx::from_int(300);
    pl.energy_capacity = Fx::from_int(5000);
    pl.energy = Fx::from_int(3000);
    let start = FxVec2::from_ints(300, 300);
    let intel = Intel::default();
    let run = |w: &World, sites: Vec<usize>| {
        let mut census = w.survey_own(0);
        census.sites = sites;
        census.builders_idle = vec![mason];
        let mut planned = w.plan_counts(0, &census);
        let mut out = vec![];
        w.direct_builders(
            0,
            &census,
            &intel,
            Stance::Expand,
            Personality::Expander,
            start,
            Angle::ZERO,
            None,
            &mut vec![],
            &mut planned,
            &mut out,
        );
        out
    };
    let out = run(&w, vec![plant]);
    assert!(
        matches!(out.as_slice(), [Command::Assist { target, .. }] if *target == w.state.units.id(plant)),
        "{out:?}"
    );
    // No plant going up: still no small one of its own while the side has energy in store.
    let small = w.blueprints.id_of("aster_t1_power").unwrap();
    let out = run(&w, vec![]);
    assert!(
        !out.iter()
            .any(|c| matches!(c, Command::Build { blueprint, .. } if *blueprint == small)),
        "{out:?}"
    );
    // A mass stall asks for more energy than comes in, but pays out only a share of it
    // and the store fills: that is no call for power.
    let pl = &mut w.state.players[0];
    pl.energy_income = Fx::from_int(2000);
    pl.energy_demand = Fx::from_int(3000);
    pl.energy_spent = Fx::from_int(500);
    let out = run(&w, vec![plant]);
    assert!(
        !out.iter().any(
            |c| matches!(c, Command::Assist { target, .. } if *target == w.state.units.id(plant))
        ),
        "{out:?}"
    );
}

#[test]
fn an_engineer_goes_up_a_tier_once_the_side_has_it() {
    let mut w = world_of(512);
    spawn(&mut w, "aster_t2_land_factory", 0, 500, 500);
    spawn(&mut w, "aster_t1_power", 0, 700, 500);
    spawn(&mut w, "aster_t1_power", 0, 700, 560);
    let masons: Vec<usize> = (0..3)
        .map(|i| spawn(&mut w, "aster_t1_engineer", 0, 300 + i * 20, 300))
        .collect();
    let pl = &mut w.state.players[0];
    pl.mass_income = Fx::from_int(5);
    pl.energy_income = Fx::from_int(1000);
    pl.energy_demand = Fx::ZERO;
    pl.energy_capacity = Fx::from_int(5000);
    pl.energy = pl.energy_capacity;
    pl.upkeep_efficiency = Fx::ONE;
    let mut out = vec![];
    w.direct_upgrades(0, &w.survey_own(0), &mut out);
    assert!(
        out.iter().any(|c| matches!(c, Command::Upgrade { units }
            if masons.iter().any(|&m| units == &vec![w.state.units.id(m)]))),
        "{out:?}"
    );
    // One in three at most at a time: with one under way, none more.
    for c in &out {
        w.apply_command(&PlayerCommand {
            player: 0,
            command: c.clone(),
        })
        .unwrap();
    }
    assert!(w.engineer_to_upgrade(0).is_none());
}

#[test]
fn a_builder_joins_the_same_building_going_up_rather_than_start_a_second() {
    let mut w = world_of(512);
    let a = spawn(&mut w, "aster_t1_engineer", 0, 320, 320);
    let b = spawn(&mut w, "aster_t1_engineer", 0, 340, 320);
    let sparrow = w.blueprints.id_of("aster_t1_aa").unwrap();
    let spot = FxVec2::from_ints(400, 360);
    let census = w.survey_own(0);
    let intel = Intel::default();
    // One ordered this think: the second builder choosing a Sparrow goes to the same lot.
    let ordered = [(sparrow, spot, AI_BUILD_HEADING)];
    let join = w.join_same_build(b, sparrow, &census, &intel, &ordered);
    assert!(
        matches!(join, Some(Command::Build { blueprint, pos, .. }) if blueprint == sparrow && pos == spot),
        "{join:?}"
    );
    // Another building is not joined.
    let power = w.blueprints.id_of("aster_t1_power").unwrap();
    assert!(w
        .join_same_build(b, power, &census, &intel, &ordered)
        .is_none());
    // A plan the other builder is walking to is joined too.
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::Build {
            units: vec![w.state.units.id(a)],
            blueprint: sparrow,
            pos: spot,
            heading: AI_BUILD_HEADING,
            queue: false,
        },
    })
    .unwrap();
    let planned: Vec<_> = w.planned_sites(0).map(|(_, o)| o.pos).collect();
    let join = w.join_same_build(b, sparrow, &census, &intel, &[]);
    assert!(
        matches!(join, Some(Command::Build { pos, .. }) if planned.contains(&pos)),
        "{join:?}"
    );
    // A Sparrow already begun is helped.
    let site = w.spawn_unit(sparrow, 0, spot, Angle::ZERO, false).unwrap();
    let mut census = w.survey_own(0);
    census.sites = vec![site];
    let join = w.join_same_build(b, sparrow, &census, &intel, &[]);
    let target = w.state.units.id(site);
    assert!(
        matches!(join, Some(Command::Assist { target: t, .. }) if t == target),
        "{join:?}"
    );
}
