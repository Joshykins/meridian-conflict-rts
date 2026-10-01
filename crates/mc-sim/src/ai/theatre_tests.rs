use super::*;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, MatchConfig, PlayerSetup};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

/// Two islands 960 m square in opposite corners of a 4 km sea, a start on
/// each; with `bridge`, a causeway joins them along the diagonal.
fn islands(bridge: bool) -> World {
    let data = Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 5,
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
                ai: AiConfig::default(),
            })
            .collect(),
    };
    // 8 m cells: land 40 m up, the sea 20 m deep.
    let mut samples = vec![0u16; 513 * 513];
    for y in 0..513 {
        for x in 0..513 {
            let island = (x < 120 && y < 120) || (x > 392 && y > 392);
            let causeway = bridge && (x as i32 - y as i32).abs() < 12;
            if island || causeway {
                samples[y * 513 + x] = 40;
            }
        }
    }
    let mut w = World::with_terrain(
        Heightfield::from_samples(512, 512, samples, Fx::ZERO, Fx::ONE, Fx::from_int(20)),
        MapData {
            name: "islands".into(),
            content_id: 5,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(300, 300), FxVec2::from_ints(3800, 3800)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap();
    let water = w.terrain.water_level();
    let size = w.terrain.size_metres();
    w.ore = crate::mines::OreGrid::new(&w.map.ore, size, |p| w.terrain.height_at(p) > water);
    w
}

fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    w.spawn_unit(
        w.blueprints.id_of(key).unwrap(),
        0,
        FxVec2::from_ints(x, y),
        Angle::ZERO,
        true,
    )
    .unwrap()
}

#[test]
fn the_sea_between_two_islands_cuts_the_land_route() {
    let mut w = islands(false);
    assert!(w.land_route_to_enemy(0), "unknown until the first think");
    w.find_land_route_once(0);
    assert!(!w.land_route_to_enemy(0));

    let mut w = islands(true);
    w.find_land_route_once(0);
    assert!(w.land_route_to_enemy(0), "a causeway joins them");
}

#[test]
fn with_no_land_route_a_factory_makes_only_a_home_guard_of_land_units() {
    let mut w = islands(false);
    w.find_land_route_once(0);
    let factory = spawn(&mut w, "aster_t2_land_factory", 300, 500);
    for i in 0..theatre::HOME_GUARD as i32 {
        spawn(&mut w, "aster_t1_tank", 200 + i * 20, 200);
    }
    // Enough engineers that the factory makes a combat unit.
    for i in 0..8 {
        spawn(&mut w, "aster_t2_engineer", 200 + i * 20, 700);
    }
    w.rebuild_index();
    let census = w.survey_own(0);
    assert!(census.factories_idle.contains(&factory));
    let mut out = Vec::new();
    for _ in 0..6 {
        w.direct_factories(
            0,
            &census,
            &[],
            Stance::Expand,
            Personality::Expander,
            &mut out,
        );
    }
    let made: Vec<&UnitBlueprint> = out
        .iter()
        .filter_map(|c| match c {
            Command::Produce { blueprint, .. } => Some(w.blueprints.unit(*blueprint)),
            _ => None,
        })
        .collect();
    assert!(!made.is_empty());
    for bp in made {
        assert!(
            !(theatre::land_bound(bp) && !bp.weapons.is_empty()),
            "{} cannot reach the enemy",
            bp.key
        );
    }
}

#[test]
fn a_shipyard_goes_on_the_water_nearest_home() {
    let w = islands(false);
    let yard = w
        .blueprints
        .unit(w.blueprints.id_of("aster_t1_naval_factory").unwrap());
    let start = w.state.players[0].start;
    let at = w
        .shipyard_anchor(yard, start)
        .expect("the sea is 700 m off");
    assert!(w.can_place(yard, at));
    assert!(at.distance(start) < Fx::from_int(1100), "{at:?}");
}

#[test]
fn a_sea_mine_is_not_kept_off_by_the_islands_own_land_mines() {
    let mut w = islands(false);
    for (x, y) in [(300, 700), (700, 300)] {
        spawn(&mut w, "aster_core_mine", x, y);
    }
    w.tick(&[]).unwrap();
    let start = w.state.players[0].start;
    let spot = w
        .free_deposit(
            start,
            &[],
            Fx::from_int(1000),
            &Intel::default(),
            Some(Fx::ratio(1, 2)),
        )
        .expect("the sea round the island");
    assert!(w.ore.at_sea(spot), "{spot:?}");
    // The next one keeps a sea reach off it.
    let sea = w
        .blueprints
        .unit(w.blueprints.id_of("aster_core_mine").unwrap())
        .mine
        .unwrap()
        .sea_reach;
    spawn(
        &mut w,
        "aster_core_mine",
        spot.x.floor_int(),
        spot.y.floor_int(),
    );
    w.tick(&[]).unwrap();
    let next = w.free_deposit(
        start,
        &[],
        Fx::from_int(3000),
        &Intel::default(),
        Some(Fx::ratio(1, 2)),
    );
    assert!(
        next.is_none_or(|p| !w.ore.at_sea(p) || p.distance(spot) >= sea),
        "{next:?}"
    );
}

#[test]
fn a_heavy_warship_strikes_alone_but_a_corvette_waits_for_another() {
    let mut w = islands(false);
    let start = w.state.players[0].start;
    let intel = Intel {
        enemy_start: Some(w.state.players[1].start),
        ..Intel::default()
    };
    let staging = offset_toward(start, w.state.players[1].start, Fx::from_int(400));
    let corvette = spawn(
        &mut w,
        "aster_t2_corvette",
        staging.x.floor_int(),
        staging.y.floor_int(),
    );
    w.rebuild_index();
    let mut out = Vec::new();
    w.direct_capital(0, &w.survey_own(0), &intel, start, &mut out);
    assert!(out.is_empty(), "{out:?}");

    let frigate = spawn(
        &mut w,
        "aster_t3_frigate",
        staging.x.floor_int(),
        staging.y.floor_int() + 200,
    );
    w.rebuild_index();
    w.direct_capital(0, &w.survey_own(0), &intel, start, &mut out);
    let sent: Vec<UnitId> = out
        .iter()
        .flat_map(|c| match c {
            Command::AttackMove { units, .. } => units.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(sent.contains(&w.state.units.id(frigate)), "{out:?}");
    assert!(
        sent.contains(&w.state.units.id(corvette)),
        "they go together"
    );
}

#[test]
fn an_unarmed_radar_ship_is_no_project_and_a_side_wants_only_one() {
    let mut w = islands(false);
    let vigil = w.blueprints.id_of("aster_t1_sensor_ship").unwrap();
    assert_eq!(projects::project_kind(w.blueprints.unit(vigil)), None);
    let mason = spawn(&mut w, "aster_t1_engineer", 300, 400);
    w.state.players[0].mass_income = Fx::from_int(30);
    let start = w.state.players[0].start;
    let job = w
        .spotter_job(mason, 2, start, Angle::ZERO)
        .expect("one radar ship");
    assert_eq!(job.blueprint, vigil);
    spawn(&mut w, "aster_t1_sensor_ship", 600, 600);
    assert!(w.spotter_job(mason, 2, start, Angle::ZERO).is_none());
}
