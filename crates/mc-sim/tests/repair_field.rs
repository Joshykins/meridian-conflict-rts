//! Nanite repair fields: friends within reach heal a flat amount a second; units out of
//! reach and the enemy's do not. The Censer's field mends the Censer too, the Exarch's
//! does not mend the Exarch.

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
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
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

/// The Exarch with `keys` fitted, one after another, at (x, y).
fn exarch(w: &mut World, keys: &[&str], x: i32, y: i32) -> usize {
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
    w.spawn_unit(at, 0, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

#[test]
fn the_field_mends_friends_in_reach_and_no_one_else() {
    let mut w = world();
    let acu = exarch(&mut w, &["nano_repair", "nano_field"], 900, 900);
    let field = w
        .blueprints
        .unit(w.state.units.blueprint[acu])
        .repair_field
        .expect("the field is on");
    let reach = field.radius.floor_int();
    let tank = w.blueprints.id_of("regency_t1_tank").unwrap();
    let mut put = |owner: u8, x: i32| {
        w.spawn_unit(tank, owner, FxVec2::from_ints(x, 900), Angle::ZERO, true)
            .unwrap()
    };
    let (near, far, foe) = (
        put(0, 900 + reach - 10),
        put(0, 900 + reach + 60),
        put(1, 860),
    );
    for row in [near, far, foe] {
        w.state.units.health[row] = Fx::from_int(50);
    }
    // Nobody shoots: only the field changes anyone's health.
    for row in [acu, near, far, foe] {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    let healed = w.state.units.health[near] - Fx::from_int(50);
    // Five seconds of the field: its flat heal a second, give or take a tick's worth.
    let expect = field.heal * 5;
    assert!(expect + Fx::from_int(50) < w.unit_max_health(near));
    assert!(
        healed >= expect * 9 / 10 && healed <= expect * 11 / 10,
        "healed {healed:?} in five seconds, expected about {expect:?}"
    );
    assert_eq!(w.state.units.health[far], Fx::from_int(50));
    assert_eq!(w.state.units.health[foe], Fx::from_int(50));
}

#[test]
fn the_censer_mends_itself_and_the_exarch_field_does_not() {
    let mut w = world();
    let censer = w.blueprints.id_of("regency_t3_field_mender").unwrap();
    let censer = w
        .spawn_unit(censer, 0, FxVec2::from_ints(900, 900), Angle::ZERO, true)
        .unwrap();
    let field = w
        .blueprints
        .unit(w.state.units.blueprint[censer])
        .repair_field
        .unwrap();
    assert!(field.mends_self);
    let acu = exarch(&mut w, &["nano_repair", "nano_field"], 1500, 1500);
    assert!(
        !w.blueprints
            .unit(w.state.units.blueprint[acu])
            .repair_field
            .unwrap()
            .mends_self
    );
    w.state.units.health[censer] = Fx::from_int(500);
    for _ in 0..50 {
        w.tick(&[]).unwrap();
    }
    let healed = w.state.units.health[censer] - Fx::from_int(500);
    let expect = field.heal * 5;
    assert!(
        healed >= expect * 9 / 10,
        "the Censer healed {healed:?} in five seconds, expected at least about {expect:?}"
    );
}

#[test]
fn nanite_repair_alone_mends_only_the_exarch() {
    let mut w = world();
    let acu = exarch(&mut w, &["nano_repair"], 900, 900);
    let bp = w.blueprints.unit(w.state.units.blueprint[acu]);
    assert!(bp.repair_field.is_none());
    let base = w
        .blueprints
        .unit(w.blueprints.id_of("regency_commander").unwrap());
    assert!(bp.regen > base.regen);
}

#[test]
fn the_mirror_shows_the_nanites_at_work() {
    let mut w = world();
    let acu = exarch(&mut w, &["nano_repair", "nano_field"], 900, 900);
    let tank = w.blueprints.id_of("regency_t1_tank").unwrap();
    let mut put = |owner: u8, x: i32| {
        w.spawn_unit(tank, owner, FxVec2::from_ints(x, 900), Angle::ZERO, true)
            .unwrap()
    };
    let (hurt, whole, foe) = (put(0, 940), put(0, 960), put(1, 860));
    for row in [acu, hurt, foe] {
        w.state.units.health[row] = Fx::from_int(50);
    }
    for row in [acu, hurt, whole, foe] {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    w.tick(&[]).unwrap();
    let mut frame = mc_sim::RenderFrame::default();
    w.write_render_frame(Some(0), &mut frame);
    let id = |row: usize| w.state.units.id(row).0;
    let mended = |row: usize| frame.mends.iter().find(|m| m.unit_id == id(row));
    // The Exarch mends itself with its own nanites; the hurt friend is fed from it.
    assert!(mended(acu).is_some_and(|m| m.from.is_none()));
    assert!(mended(hurt).is_some_and(|m| m.from.is_some()));
    assert!(mended(whole).is_none() && mended(foe).is_none());
    assert_eq!(frame.repair_fields.len(), 1);
    assert_eq!(frame.repair_fields[0].unit_id, id(acu));
}

#[test]
fn nanite_repair_mends_faster_under_fire_than_bare_regen() {
    // An enemy tank shooting at each Exarch, half dead, its own fire held.
    let under_fire = |keys: &[&str]| {
        let mut w = world();
        let acu = exarch(&mut w, keys, 900, 900);
        let tank = w.blueprints.id_of("regency_t1_tank").unwrap();
        w.spawn_unit(tank, 1, FxVec2::from_ints(960, 900), Angle::ZERO, true)
            .unwrap();
        w.state.units.fire_state[acu] = FireState::HoldFire;
        let start = Fx::from_int(5000);
        w.state.units.health[acu] = start;
        let mut hit = false;
        for _ in 0..100 {
            w.tick(&[]).unwrap();
            hit |= w.state.units.has_flag(acu, mc_sim::tables::flag::HURT);
        }
        assert!(hit, "the tank never hit it");
        w.state.units.health[acu] - start
    };
    let bare = under_fire(&[]);
    let nanite = under_fire(&["nano_repair"]);
    // Regen runs under fire either way; the nanites' is the stronger.
    assert!(nanite > bare, "nanites {nanite:?}, bare {bare:?}");
}
