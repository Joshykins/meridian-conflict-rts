use super::*;
use crate::{tables::Controller, world::MapData, MatchConfig, PlayerSetup, World};
use mc_core::Angle;
use mc_map::{placed::Footprint, Heightfield, PropKind};
use std::{path::Path, sync::Arc};
fn terrain() -> Heightfield {
    Heightfield::from_samples(128, 128, vec![20; 129 * 129], Fx::ZERO, Fx::ONE, Fx::ZERO)
}
fn world(blocking: bool) -> World {
    let props = vec![Prop {
        kind: PropKind::CityBillboard,
        pos: FxVec2::from_ints(512, 512),
        heading: Angle::ZERO,
        scale_milli: 1000,
        wear_milli: 0,
    }];
    let component = Doodad {
        id: 1,
        name: "propaganda".into(),
        prop_index: 0,
        tint: [255; 3],
        blocking,
        health: 120,
        reclaim: 0,
        footprint: Some(Footprint {
            half: FxVec2::from_ints(20, 4),
            height: Fx::from_int(18),
        }),
    };
    let blueprints =
        mc_data::Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap();
    World::with_terrain(
        terrain(),
        MapData {
            name: "Doodads".into(),
            content_id: 1,
            props,
            doodads: [(0, component)].into_iter().collect(),
            ore: vec![],
            starts: vec![],
        },
        Arc::new(blueprints),
        Arc::new(mc_jobs::Pool::new(0)),
        &MatchConfig {
            seed: 7,
            spawn_commanders: false,
            cheats: true,
            fog: false,
            players: vec![PlayerSetup {
                name: "Author".into(),
                faction: "Aster".into(),
                ai: Default::default(),
                team: 0,
                controller: Controller::Human,
                start: 0,
            }],
        },
    )
    .unwrap()
}
#[test]
fn custom_prop_takes_damage_opens_its_footprint_and_restores_after_snapshot() {
    let mut w = world(true);
    let at = FxVec2::from_ints(512, 512);
    assert!(!w.nav.passable(mc_data::MoveLayer::Land, 0, at));
    assert_eq!(w.state.city.max, vec![120]);
    let from = FxVec2::from_ints(512, 400).extend(Fx::from_int(25));
    let to = FxVec2::from_ints(512, 600).extend(Fx::from_int(25));
    assert_eq!(
        w.city_shapes
            .first_hit(&w.state.city, from, to)
            .map(|(_, row)| row),
        Some(0)
    );
    w.damage_structure(0, Fx::from_int(50), false);
    assert_eq!(w.state.city.health, vec![70]);
    let checkpoint = w.snapshot();
    w.damage_structure(0, Fx::from_int(70), false);
    assert!(!w.is_prop_alive(0));
    assert!(w.city_shapes.first_hit(&w.state.city, from, to).is_none());
    assert!(w.nav.passable(mc_data::MoveLayer::Land, 0, at));
    assert!(w
        .events
        .iter()
        .any(|e| matches!(e, crate::SimEvent::StructureCollapsed { prop: 0, .. })));
    let mut resumed = world(true);
    resumed.restore(terrain(), &checkpoint).unwrap();
    assert_eq!(resumed.state.city.health, vec![70]);
    assert!(resumed.is_prop_alive(0));
    assert!(!resumed.nav.passable(mc_data::MoveLayer::Land, 0, at));
    resumed.damage_structure(0, Fx::from_int(70), false);
    assert!(!resumed.is_prop_alive(0));
    assert!(resumed.nav.passable(mc_data::MoveLayer::Land, 0, at));
}
#[test]
fn nonblocking_destructible_still_takes_damage() {
    let mut w = world(false);
    assert!(w
        .nav
        .passable(mc_data::MoveLayer::Land, 0, FxVec2::from_ints(512, 512)));
    w.damage_structure(0, Fx::from_int(120), false);
    assert!(!w.is_prop_alive(0));
}
#[test]
fn footprints_rotate_with_the_prop_and_reject_missing_hit_shapes() {
    let w = world(true);
    let mut p = w.map.props[0];
    let d = &w.map.doodads[&0];
    let horizontal = cells(&p, d, terrain().size_metres()).unwrap();
    p.heading = Angle::QUARTER_TURN;
    let vertical = cells(&p, d, terrain().size_metres()).unwrap();
    assert_ne!(horizontal, vertical);
    assert!(horizontal.len() < vertical.len());
    let mut components = w.map.doodads.clone();
    components.get_mut(&0).unwrap().footprint = None;
    assert!(validate(&[p], &components).is_err());
}

#[test]
fn authored_tree_is_not_removed_by_decorative_tree_clearing() {
    let mut w = world(true);
    w.map.props[0].kind = PropKind::TreeConifer;
    assert!(!w.is_scenery_tree(0));
    w.map.doodads.clear();
    assert!(w.is_scenery_tree(0));
}
