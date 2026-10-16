//! The Exarch's AA Beam Projector (`aa_beam`): a held beam on a mount of its own that
//! brings down aircraft the bare Exarch cannot touch.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{Controller, FireState};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(
            &Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data"),
        )
        .unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        doodads: Default::default(),
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1500, 1500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Regency".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Five seconds of an Exarch (with `keys` fitted) at a hostile bomber 300 m off: the
/// share of the bomber's health lost, out of one.
fn bomber_loss(keys: &[&str]) -> Fx {
    let mut w = world();
    let mut at = w.blueprints.id_of("regency_commander").unwrap();
    for key in keys {
        let set = w.blueprints.refit_set(at).unwrap();
        let kit = set
            .slots
            .iter()
            .flat_map(|s| &s.modules)
            .find(|m| m.key == *key)
            .unwrap()
            .kit;
        at = w.blueprints.refit_result(at, kit).unwrap();
    }
    w.spawn_unit(at, 0, FxVec2::from_ints(900, 900), Angle::ZERO, true)
        .unwrap();
    let bomber = w.blueprints.id_of("regency_t1_bomber").unwrap();
    let bomber = w
        .spawn_unit(bomber, 1, FxVec2::from_ints(1200, 900), Angle::ZERO, true)
        .unwrap();
    w.state.units.fire_state[bomber] = FireState::HoldFire;
    let full = w.state.units.health[bomber];
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    (full - w.state.units.health[bomber]) / full
}

#[test]
fn the_projector_brings_down_what_the_bare_exarch_cannot_touch() {
    assert_eq!(
        bomber_loss(&[]),
        Fx::ZERO,
        "the bare Exarch has no anti-air"
    );
    // 220 a second against a tech 1 bomber's 250: down well inside five seconds.
    let lost = bomber_loss(&["aa_beam"]);
    assert_eq!(
        lost,
        Fx::ONE,
        "the beam took only {lost:?} of it in five seconds"
    );
}
