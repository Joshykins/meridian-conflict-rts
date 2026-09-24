use super::*;
use crate::world::MapData;
use crate::{MatchConfig, PlayerSetup};
use mc_data::{Blueprints, MoveLayer};
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
fn production_counters_air_then_returns_to_ground() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let aa = w.blueprints.id_of("aster_t1_mobile_aa").unwrap();
    let choose = |w: &World| {
        w.choose_combat_unit(0, &[tank, aa], &Default::default(), Stance::Defend, 0)
            .unwrap()
    };
    assert_eq!(choose(&w), tank);
    contact(&mut w, "aster_t1_bomber", 12);
    assert_eq!(
        choose(&w),
        aa,
        "observed bomber mass should prompt AA production"
    );
    w.state.tick += w.state.ai[0].config.memory_ticks() + 1;
    w.remember_enemies(0);
    assert_eq!(
        choose(&w),
        tank,
        "expired air intel should stop dominating production"
    );
}

#[test]
fn adaptation_off_and_domain_weights_are_respected() {
    let mut w = world();
    contact(&mut w, "aster_t1_bomber", 12);
    let choices = [
        w.blueprints.id_of("aster_t1_tank").unwrap(),
        w.blueprints.id_of("aster_t1_mobile_aa").unwrap(),
    ];
    w.state.ai[0].config.adaptation = 0;
    let known = w.choose_combat_unit(0, &choices, &Default::default(), Stance::Expand, 3);
    w.state.ai[0].contacts.clear();
    assert_eq!(
        known,
        w.choose_combat_unit(0, &choices, &Default::default(), Stance::Expand, 3)
    );
    w.state.ai[0].config.domain_weights = [0, 100, 100];
    assert_eq!(
        w.choose_combat_unit(0, &choices, &Default::default(), Stance::Expand, 3),
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
fn wounded_busy_units_retreat_and_are_not_reassigned_to_attack() {
    let mut w = world();
    w.state.fog_enabled = false;
    let own = spawn(&mut w, "aster_t1_tank", 0, 850, 850);
    spawn(&mut w, "aster_t1_tank", 1, 1000, 850);
    w.state.units.health[own] = Fx::from_int(40);
    let id = w.state.units.id(own);
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::AttackMove {
            units: vec![id],
            target: FxVec2::from_ints(1600, 1600),
            queue: false,
        },
    })
    .unwrap();
    w.remember_enemies(0);
    let mut c = w.survey_own(0);
    assert!(!c.army_idle.contains(&own));
    let intel = w.survey_intel(0, &c);
    let mut out = vec![];
    w.react_tactically(0, &mut c, &intel, &mut out);
    assert!(out
        .iter()
        .any(|o| matches!(o,Command::Move { units,.. } if units.contains(&id))));
    assert!(w.state.ai[0].recovering.iter().any(|r| r.id == id));
    assert!(!w.survey_own(0).army_idle.contains(&own));
}

#[test]
fn new_naval_blueprint_is_selected_and_ships_only_receive_water_destinations() {
    let mut w = world();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let aa = w.blueprints.id_of("aster_t1_mobile_aa").unwrap();
    let mut data = (*w.blueprints).clone();
    let ship = &mut data.units[tank.index()];
    ship.key = "future_ship_not_known_to_ai".into();
    ship.categories = cat::NAVAL | cat::MOBILE | cat::DIRECT_FIRE;
    ship.motion.as_mut().unwrap().layer = MoveLayer::Naval;
    ship.weapons[0].target_mask = cat::NAVAL | cat::LAND;
    let mut samples = vec![40u16; 257 * 257];
    for y in 0..257 {
        for x in 110..257 {
            samples[y * 257 + x] = 0;
        }
    }
    w.terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20));
    w.nav = crate::nav::Nav::new(&w.terrain, w.pool.clone()).unwrap();
    w.blueprints = Arc::new(data);
    let row = w
        .spawn_unit(tank, 0, FxVec2::from_ints(1200, 800), Angle::ZERO, true)
        .unwrap();
    w.state.ai[0].contacts.push(Contact {
        id: UnitId::new(99, 0),
        blueprint: tank,
        pos: FxVec2::from_ints(1500, 800),
        seen: 0,
    });
    assert_eq!(
        w.choose_combat_unit(0, &[tank, aa], &Default::default(), Stance::Push, 0),
        Some(tank)
    );
    let census = w.survey_own(0);
    assert!(census.naval_idle.contains(&row));
    let mut out = vec![];
    w.direct_fleet(0, &census, &mut out);
    assert!(!out.is_empty());
    for c in w.route_ai_commands(out) {
        if let Command::AttackMove { target, .. } = c {
            assert!(w.nav.passable(MoveLayer::Naval, 0, target));
        }
    }
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
    w.state.ai[0].config.adaptation = 12;
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
fn raiders_prefer_an_undefended_expansion_and_support_is_bounded() {
    let mut w = world();
    let near = FxVec2::from_ints(1000, 300);
    let far = FxVec2::from_ints(1000, 1000);
    let defense = w
        .blueprints
        .units
        .iter()
        .find(|bp| bp.has(cat::DEFENSE | cat::DIRECT_FIRE))
        .unwrap()
        .id;
    w.state.ai[0].contacts.push(Contact {
        id: UnitId::new(10, 0),
        blueprint: defense,
        pos: near,
        seen: 0,
    });
    let start = w.state.players[0].start;
    assert!(w.ai_objective_cost(0, near, start) > w.ai_objective_cost(0, far, start));
    let support = w.blueprints.id_of("aster_t2_support").unwrap();
    let tank = w.blueprints.id_of("aster_t1_tank").unwrap();
    let mut counts = std::collections::BTreeMap::new();
    assert_eq!(
        w.choose_combat_unit(0, &[support], &counts, Stance::Expand, 0),
        None
    );
    counts.insert(tank, 10);
    let scout = w.blueprints.id_of("aster_t1_air_scout").unwrap();
    assert_eq!(
        w.choose_combat_unit(0, &[scout], &counts, Stance::Expand, 0),
        None,
        "scouts use their own quota, not support production"
    );

    assert_eq!(
        w.choose_combat_unit(0, &[support], &counts, Stance::Expand, 0),
        Some(support)
    );
    counts.insert(support, 1);
    assert_eq!(
        w.choose_combat_unit(0, &[support], &counts, Stance::Expand, 0),
        None
    );
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
    let water = w.terrain.water_level();
    let size = w.terrain.size_metres();
    w.ore = crate::mines::OreGrid::new(&w.map.ore, size, |p| w.terrain.height_at(p) > water);
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
        w.state.ai.iter().any(|ai| ai.waves > 0 || ai.raids > 0),
        "AI must launch an offensive wave or economic raid"
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
    assert!(w.state.ai[0].waves > 0);
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
    w.direct_factories(
        0,
        &census,
        Stance::Expand,
        Personality::Aggressive,
        &mut out,
    );
    let engineer = w.blueprints.id_of("aster_t2_engineer").unwrap();
    assert!(
        out.iter()
            .any(|c| matches!(c,Command::Produce {blueprint,..} if *blueprint==engineer)),
        "tech access must not be blocked by the existing low-tier engineer count"
    );
}

#[test]
fn bare_mines_only_go_where_they_keep_their_ground() {
    let mut w = world_of(1024);
    // A ring of mines a reach apart round the start: the gaps between them are
    // worth little, the ground beyond them a lot.
    for (x, y) in [
        (1300, 300),
        (300, 1300),
        (1300, 1300),
        (2300, 300),
        (300, 2300),
    ] {
        spawn(&mut w, "aster_core_mine", 0, x, y);
    }
    w.tick(&[]).unwrap();
    let mine = w
        .blueprints
        .unit(w.blueprints.id_of("aster_core_mine").unwrap())
        .clone();
    let intel = Intel::default();
    let start = FxVec2::from_ints(1800, 800);
    let least = Fx::ratio(55, 100);
    let spot = w
        .free_deposit(start, &[], Fx::from_int(3000), &intel, Some(least))
        .expect("open ground beyond the ring");
    assert!(
        w.mine_share_at(&mine, spot).efficiency(&mine.mine.unwrap()) >= least,
        "a new bare mine keeps most of its reach"
    );
    assert!(
        w.free_deposit(start, &[], Fx::from_int(3000), &intel, Some(Fx::ONE))
            .is_none(),
        "nowhere near the ring is a mine's reach all its own"
    );
}

#[test]
fn mine_upgrades_go_to_the_mine_that_pays_back_soonest() {
    let mut w = world_of(1024);
    // A crowded cluster, and one mine with its reach to itself.
    let crowded: Vec<usize> = [(600, 600), (1100, 600), (600, 1100), (1100, 1100)]
        .into_iter()
        .map(|(x, y)| spawn(&mut w, "aster_core_mine", 0, x, y))
        .collect();
    let alone = spawn(&mut w, "aster_core_mine", 0, 4000, 4000);
    spawn(&mut w, "aster_t2_engineer", 0, 400, 400);
    w.tick(&[]).unwrap();
    w.state.players[0].mass = Fx::from_int(800);
    w.state.players[0].mass_income = Fx::from_int(20);
    let mut census = w.survey_own(0);
    assert_eq!(w.mine_to_upgrade(0, &census, false), Some(alone));

    // With the lone mine taken, a crowded one pays back too slowly, unless
    // materials pile up with nothing better to spend them on.
    census.extractors.retain(|&r| r != alone);
    w.state.ai[0].config.difficulty = Difficulty::Hard;
    assert_eq!(w.mine_to_upgrade(0, &census, false), None);
    assert!(crowded.contains(&w.mine_to_upgrade(0, &census, true).unwrap()));
    w.state.ai[0].config.difficulty = Difficulty::Easy;
    assert_eq!(w.mine_to_upgrade(0, &census, true), None);
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
fn a_planned_mine_claims_its_deposit_even_when_its_site_stands_off_it() {
    let w = world();
    let start = FxVec2::from_ints(1000, 1000);
    let reach = w
        .blueprints
        .units
        .iter()
        .find(|b| b.tech == 1 && b.mine.is_some())
        .unwrap()
        .mine
        .unwrap()
        .reach;
    let open = w
        .free_deposit(
            start,
            &[],
            Fx::from_int(2000),
            &Intel::default(),
            Some(Fx::ZERO),
        )
        .unwrap();
    // A builder already walking to a site 200 m off that spot.
    let claim = Claim {
        pos: open + FxVec2::from_ints(200, 0),
        foot: 7,
        mine: true,
        factory: false,
        cover: Fx::ZERO,
    };
    let next = w.free_deposit(
        start,
        &[claim],
        Fx::from_int(2000),
        &Intel::default(),
        Some(Fx::ZERO),
    );
    assert!(
        next.is_none_or(|p| p.distance(claim.pos) >= reach),
        "{next:?}"
    );
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

/// Attack orders in `out`, as (units, target).
fn attacks(out: &[Command]) -> Vec<(usize, FxVec2)> {
    out.iter()
        .filter_map(|c| match c {
            Command::AttackMove { units, target, .. } => Some((units.len(), *target)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_lone_bomber_waits_for_its_wing_and_the_wing_strikes_together() {
    let mut w = world();
    let (start, staging) = (FxVec2::from_ints(300, 300), FxVec2::from_ints(500, 500));
    let intel = Intel {
        enemy_extractors: vec![FxVec2::from_ints(1600, 1500)],
        ..Intel::default()
    };
    spawn(&mut w, "aster_t1_bomber", 0, 500, 500);
    let mut out = vec![];
    w.direct_air(0, &w.survey_own(0), &intel, start, staging, &mut out);
    assert!(attacks(&out).is_empty(), "one bomber alone is not a strike");
    for i in 1..4 {
        spawn(&mut w, "aster_t1_bomber", 0, 500 + i * 20, 500);
    }
    w.direct_air(0, &w.survey_own(0), &intel, start, staging, &mut out);
    assert_eq!(attacks(&out), vec![(4, FxVec2::from_ints(1600, 1500))]);
}

#[test]
fn torpedo_bombers_are_not_sent_at_mines() {
    let mut w = world();
    let intel = Intel {
        enemy_extractors: vec![FxVec2::from_ints(1600, 1500)],
        ..Intel::default()
    };
    for i in 0..4 {
        spawn(&mut w, "aster_t2_torpedo_bomber", 0, 500 + i * 20, 500);
    }
    let mut out = vec![];
    let (start, staging) = (FxVec2::from_ints(300, 300), FxVec2::from_ints(500, 500));
    w.direct_air(0, &w.survey_own(0), &intel, start, staging, &mut out);
    assert!(
        attacks(&out).is_empty(),
        "no ship seen, nothing a torpedo can hit"
    );
}

#[test]
fn units_out_in_the_field_wait_for_the_rest_of_their_wave() {
    let mut w = world();
    let rows: Vec<usize> = (0..6)
        .map(|i| spawn(&mut w, "aster_t1_tank", 0, 1300 + i * 20, 1300))
        .collect();
    // Half of the wave is still busy: those done wait for it.
    let busy: Vec<UnitId> = rows[..3].iter().map(|&r| w.state.units.id(r)).collect();
    w.apply_command(&PlayerCommand {
        player: 0,
        command: Command::Move {
            units: busy,
            target: FxVec2::from_ints(1300, 1000),
            queue: false,
        },
    })
    .unwrap();
    let army = |w: &mut World| {
        let census = w.survey_own(0);
        let intel = Intel {
            enemy_start: Some(FxVec2::from_ints(1700, 1700)),
            ..Intel::default()
        };
        let mut out = vec![];
        w.direct_army(
            0,
            &census,
            &intel,
            Stance::Push,
            Personality::Aggressive,
            FxVec2::from_ints(300, 300),
            Angle::ZERO,
            None,
            &mut out,
        );
        attacks(&out)
    };
    assert!(army(&mut w).is_empty(), "three of six idle: they wait");
    for &r in &rows {
        w.state.units.order_head[r] = NO_ORDER;
    }
    assert_eq!(army(&mut w).len(), 1, "all done: they go on as one group");
    assert_eq!(army(&mut w)[0].0, 6);
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
    let start = FxVec2::from_ints(1800, 800);
    let first = w
        .free_deposit(
            start,
            &[],
            Fx::from_int(3000),
            &Intel::default(),
            Some(Fx::ZERO),
        )
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
        .free_deposit(start, &[], Fx::from_int(3000), &intel, Some(Fx::ZERO))
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
            power: 0,
            radar: 0,
            pd: 0,
            artillery: 0,
            shields: 0,
            storage: 0,
            guards: vec![],
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

/// A flat map with a lake east of player 0's start.
fn lake_world() -> World {
    let mut w = world();
    let mut samples = vec![40u16; 257 * 257];
    for y in 0..257 {
        for x in 0..257 {
            // 8 m cells: x 480..1100 m, y 0..700 m is under water.
            if (60..137).contains(&x) && y < 88 {
                samples[y * 257 + x] = 0;
            }
        }
    }
    w.terrain = Heightfield::from_samples(256, 256, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20));
    w.nav = crate::nav::Nav::new(&w.terrain, w.pool.clone()).unwrap();
    w
}

#[test]
fn a_raider_the_army_cannot_reach_does_not_hold_it_at_home() {
    let mut w = lake_world();
    w.state.fog_enabled = false;
    w.state.players[0].controller = Controller::Ai;
    let tanks: Vec<usize> = (0..16)
        .map(|i| {
            spawn(
                &mut w,
                "aster_t1_tank",
                0,
                200 + (i % 4) * 16,
                260 + (i / 4) * 16,
            )
        })
        .collect();
    // A hover raider out on the lake, well within the base's raid radius
    // but out of the tanks' reach from the shore.
    let hover = w.blueprints.id_of("aster_t2_hover").unwrap();
    let raider = w
        .spawn_unit(hover, 1, FxVec2::from_ints(700, 300), Angle::ZERO, true)
        .unwrap();
    let hold = PlayerCommand {
        player: 1,
        command: Command::SetFireState {
            units: vec![w.state.units.id(raider)],
            state: FireState::HoldFire,
        },
    };
    w.apply_command(&hold).unwrap();
    for _ in 0..1800 {
        // It sits there all game, out of reach and never worn down.
        w.state.units.health[raider] = w.bp(raider).health;
        w.tick(&[]).unwrap();
    }
    let s = &w.state;
    let far = tanks
        .iter()
        .filter(|&&r| {
            s.units.slots.is_alive(r)
                && s.units.pos[r].distance(s.players[0].start) > Fx::from_int(700)
        })
        .count();
    let at = |r: usize| s.units.pos[r];
    assert!(
        far >= 12,
        "{far} of 16 left home; {:?}",
        tanks.iter().map(|&r| at(r)).collect::<Vec<_>>()
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
