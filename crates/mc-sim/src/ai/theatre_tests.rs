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
        w.direct_factories(0, &census, &[], &mut out);
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
fn an_unarmed_radar_ship_is_no_project() {
    let w = islands(false);
    let vigil = w.blueprints.id_of("aster_t2_sensor_ship").unwrap();
    assert_eq!(projects::project_kind(w.blueprints.unit(vigil)), None);
}
