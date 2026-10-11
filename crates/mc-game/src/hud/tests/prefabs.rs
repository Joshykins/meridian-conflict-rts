use super::*;

#[test]
fn an_engineering_station_places_prefabs_and_a_factory_does_not() {
    let rig = Rig::new("aster_t1_eng_station");
    let builds = crate::hud::prefabs::selection_builds(&rig.view, &rig.blueprints);
    let power = rig.blueprints.id_of("aster_t1_power").unwrap();
    assert!(builds.contains(&power), "a station raises what it builds");
    let rig = Rig::new("aster_t1_land_factory");
    assert!(crate::hud::prefabs::selection_builds(&rig.view, &rig.blueprints).is_empty());
}
