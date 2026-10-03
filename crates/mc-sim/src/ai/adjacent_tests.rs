use super::*;
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, MatchConfig, PlayerSetup};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

fn world() -> World {
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
    World::with_terrain(
        Heightfield::flat(256, 256, Fx::from_int(20)),
        MapData {
            name: "adjacency test".into(),
            content_id: 5,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(1000, 1000), FxVec2::from_ints(1800, 1800)],
            props: vec![],
        },
        Arc::new(data),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap()
}

/// A finished building of `key` for player 0, facing the way the AI builds.
fn spawn(w: &mut World, key: &str, x: i32, y: i32) -> usize {
    let row = w
        .spawn_unit(
            w.blueprints.id_of(key).unwrap(),
            0,
            FxVec2::from_ints(x, y),
            AI_BUILD_HEADING,
            true,
        )
        .unwrap();
    w.state.units.heading[row] = AI_BUILD_HEADING;
    w.rebuild_index();
    row
}

fn bp(w: &World, key: &str) -> UnitBlueprint {
    w.blueprints.unit(w.blueprints.id_of(key).unwrap()).clone()
}

fn touches(w: &World, row: usize, bp: &UnitBlueprint, site: FxVec2) -> bool {
    let units = &w.state.units;
    crate::adjacency::shared_edge(
        crate::adjacency::lot(w.bp(row), units.pos[row]),
        crate::adjacency::lot(bp, site),
    )
    .is_some()
}

#[test]
fn plants_ring_the_factory_they_save_and_keep_its_exit_clear() {
    let mut w = world();
    let start = FxVec2::from_ints(1000, 1000);
    let factory = spawn(&mut w, "aster_t2_land_factory", 1000, 1000);
    let plant = bp(&w, "aster_t2_power");
    let mut sites = Vec::new();
    // Four Reactor IIs go round a 96 m factory, one on each side but the one
    // it opens on, and two up top; that side and a lane round it stay clear.
    for i in 0..4 {
        let site = w
            .adjacent_site(&plant, 0, start, &[], None)
            .unwrap_or_else(|| panic!("no lot against the factory for plant {i}: {sites:?}"));
        assert!(
            touches(&w, factory, &plant, site),
            "{site:?} is off the factory"
        );
        assert!(w.keeps_lanes(&plant, site, &[]), "{site:?}");
        // AI_BUILD_HEADING faces -y: nothing below the factory.
        assert!(site.y > Fx::from_int(1000 - 48), "{site:?} by the exit");
        spawn(
            &mut w,
            "aster_t2_power",
            site.x.round_int(),
            site.y.round_int(),
        );
        sites.push(site);
    }
    w.refresh_adjacency();
    let saved = w
        .adjacency
        .saving(factory, crate::adjacency::Resource::Energy);
    assert!(saved > Fx::ratio(1, 6), "the factory is saved {saved:?}");
}

#[test]
fn a_plant_with_nothing_to_save_goes_to_a_farm() {
    let mut w = world();
    let start = FxVec2::from_ints(1000, 1000);
    spawn(&mut w, "aster_t1_power", 1000, 1000);
    let plant = bp(&w, "aster_t1_power");
    assert_eq!(w.adjacent_site(&plant, 0, start, &[], None), None);
}

#[test]
fn a_fabricator_goes_where_it_saves_most_and_never_against_a_plant_it_dies_with() {
    let mut w = world();
    let start = FxVec2::from_ints(1000, 1000);
    let reactor = spawn(&mut w, "aster_t3_power", 1000, 1000);
    let factory = spawn(&mut w, "aster_t3_land_factory", 1300, 1000);
    // A tech 2 fabricator saves most against the big plant, which cuts its upkeep.
    let fab2 = bp(&w, "aster_t2_fabricator");
    let site = w.adjacent_site(&fab2, 0, start, &[], None).unwrap();
    assert!(touches(&w, reactor, &fab2, site), "{site:?}");
    // A tech 3 one would go up with that plant: it goes against the factory instead.
    let fab3 = bp(&w, "aster_t3_fabricator");
    let site = w.adjacent_site(&fab3, 0, start, &[], None).unwrap();
    assert!(
        !touches(&w, reactor, &fab3, site),
        "{site:?} bound to the plant"
    );
    assert!(touches(&w, factory, &fab3, site), "{site:?}");
}
