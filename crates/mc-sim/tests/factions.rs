//! Seating commanders of different factions, one of them on a stand-in roster.

use mc_core::{Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

#[test]
fn a_regency_commander_grows_its_own_base() {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "factions".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Ai,
        start: team,
    };
    let config = MatchConfig {
        seed: 3,
        players: vec![player("Aster", 0), player("Regency", 1)],
        cheats: false,
        fog: false,
        spawn_commanders: true,
    };
    let mut w = World::with_terrain(
        terrain,
        map,
        blueprints.clone(),
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap();
    let regency = blueprints.faction_by_key("regency").unwrap();
    assert_eq!(w.state.players[1].faction, regency.id.0);
    for p in 0..2 {
        let commander = w.state.players[p].commander;
        assert!(
            w.state.units.row(commander).is_some(),
            "player {p} has a commander"
        );
    }
    // The Regency field their own commander.
    let row = w.state.units.row(w.state.players[1].commander).unwrap();
    let bp = blueprints.unit(w.state.units.blueprint[row]);
    assert_eq!(bp.key, "regency_commander");
    // Both AIs get going: the Regency's commander builds the Regency's own structures with the
    // nanite emitter in its claw, never the stand-in's.
    for _ in 0..300 {
        w.tick(&[]).unwrap();
    }
    let regency_units: Vec<_> = (0..w.state.units.slots.rows())
        .filter(|&r| w.state.units.slots.is_alive(r) && w.state.units.owner[r] == 1)
        .map(|r| blueprints.unit(w.state.units.blueprint[r]))
        .collect();
    assert!(regency_units.len() > 1, "the Regency AI built something");
    assert!(
        regency_units
            .iter()
            .filter(|u| u.is_structure())
            .all(|u| u.faction == regency.id),
        "only Regency structures: {:?}",
        regency_units.iter().map(|u| &u.key).collect::<Vec<_>>()
    );
}
