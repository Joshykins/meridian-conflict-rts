//! Adjacency (`mc_sim::adjacency`): reactors against a building save it energy,
//! fabricators against a factory save it materials, and a fabricator and a power
//! plant of its tier that touch go down together.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::adjacency::Resource;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

// Lots are 12 m cells: a 2x2 fabricator at (792, 792) stands on 780..804, so an 8x8
// reactor (96 m) against its east side stands at x 852, a 4x4 (48 m) at x 828.

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "adjacency".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(1800, 1800)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 9,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(
        Heightfield::flat(128, 128, Fx::from_int(20)),
        map,
        blueprints,
        Arc::new(Pool::new(1)),
        &config,
    )
    .unwrap()
}

/// One `key` of `owner`'s at (x, y), `build` permille done; its id.
fn spawn(w: &mut World, owner: u8, key: &str, x: i32, y: i32, build: u16) -> UnitId {
    let blueprint = w.blueprints.id_of(key).unwrap();
    let at = FxVec2::from_ints(x, y);
    w.tick(&[PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint,
            pos: at,
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build,
        },
    }])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == blueprint && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(at))
        .unwrap();
    u.id(row)
}

fn row(w: &World, id: UnitId) -> usize {
    w.state.units.row(id).expect("alive")
}

fn saving(w: &World, id: UnitId, r: Resource) -> f32 {
    w.adjacency.saving(row(w, id), r).to_f32()
}

fn destroy(w: &mut World, id: UnitId) {
    let r = row(w, id);
    w.state.units.health[r] = Fx::ZERO;
    for _ in 0..3 {
        w.tick(&[]).unwrap();
    }
}

#[test]
fn a_reactor_against_a_fabricator_cuts_its_upkeep() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    w.tick(&[]).unwrap();
    let alone = w.flows[row(&w, fab)].wanted[1].to_f32();
    spawn(&mut w, 0, "aster_t3_power", 852, 792, 1000);
    w.tick(&[]).unwrap();
    assert!((saving(&w, fab, Resource::Energy) - 0.2).abs() < 1e-3);
    let next_door = w.flows[row(&w, fab)].wanted[1].to_f32();
    assert!(
        (next_door / alone - 0.8).abs() < 1e-3,
        "upkeep {alone} -> {next_door}"
    );
    assert!(w.adjacency.links.iter().any(|l| l.consumer == fab));
}

#[test]
fn savings_add_up_to_a_cap() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    // East and west, set south so their lots clear the north one's.
    spawn(&mut w, 0, "aster_t3_power", 852, 756, 1000);
    spawn(&mut w, 0, "aster_t3_power", 732, 756, 1000);
    spawn(&mut w, 0, "aster_t3_power", 792, 852, 1000);
    w.tick(&[]).unwrap();
    assert!((saving(&w, fab, Resource::Energy) - 0.5).abs() < 1e-3);
}

#[test]
fn a_corner_an_enemy_or_a_gap_is_no_neighbour() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    // Corner to corner, south-east.
    spawn(&mut w, 0, "aster_t2_power", 828, 828, 1000);
    // Someone else's, against the west side.
    spawn(&mut w, 1, "aster_t2_power", 756, 792, 1000);
    // One cell off the north side.
    spawn(&mut w, 0, "aster_t2_power", 792, 840, 1000);
    w.tick(&[]).unwrap();
    assert_eq!(saving(&w, fab, Resource::Energy), 0.0);
}

#[test]
fn fabricators_against_a_factory_save_its_materials_and_reactors_its_energy() {
    let mut w = world();
    let factory = spawn(&mut w, 0, "aster_t3_air_factory", 732, 792, 1000);
    spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    spawn(&mut w, 0, "aster_t2_fabricator", 792, 768, 1000);
    spawn(&mut w, 0, "aster_t2_power", 804, 828, 1000);
    w.tick(&[]).unwrap();
    assert!((saving(&w, factory, Resource::Mass) - 0.15).abs() < 1e-3);
    assert!((saving(&w, factory, Resource::Energy) - 0.10).abs() < 1e-3);
}

#[test]
fn a_fabricator_and_a_reactor_of_its_tier_go_down_together() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    let reactor = spawn(&mut w, 0, "aster_t3_power", 852, 792, 1000);
    // The fabricator's blast alone would not finish a Reactor III.
    let r = row(&w, reactor);
    w.state.units.health[r] = w.blueprints.unit(w.state.units.blueprint[r]).health;
    destroy(&mut w, fab);
    assert!(
        w.state.units.row(reactor).is_none(),
        "the reactor went with it"
    );
}

#[test]
fn a_reactor_takes_its_fabricator_but_not_one_of_another_tier() {
    let mut w = world();
    let same = spawn(&mut w, 0, "regency_t2_fabricator", 792, 792, 1000);
    let other = spawn(&mut w, 0, "regency_t3_fabricator", 864, 792, 1000);
    // A Power Generator II between them: 804..852, touching both.
    let reactor = spawn(&mut w, 0, "regency_t2_power", 828, 792, 1000);
    destroy(&mut w, reactor);
    assert!(
        w.state.units.row(same).is_none(),
        "its tier's fabricator went"
    );
    assert!(
        w.state.units.row(other).is_some(),
        "a tech 3 fabricator is not bound to a tech 2 plant"
    );
}

#[test]
fn a_paused_fabricator_makes_nothing_and_draws_nothing() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t2_fabricator", 792, 792, 1000);
    // Power for it, well away.
    spawn(&mut w, 0, "aster_t3_power", 552, 552, 1000);
    w.tick(&[]).unwrap();
    assert!(w.flows[row(&w, fab)].made[0] > Fx::ZERO);
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::SetPaused {
            units: vec![fab],
            paused: true,
        },
    }])
    .unwrap();
    w.tick(&[]).unwrap();
    let flow = w.flows[row(&w, fab)];
    assert_eq!(flow.made[0], Fx::ZERO);
    assert_eq!(flow.wanted[1], Fx::ZERO);
}

#[test]
fn the_tech_2_fabricator_upgrades_in_place_to_tech_3() {
    let w = world();
    let b = &w.blueprints;
    let t2 = b.unit(b.id_of("aster_t2_fabricator").unwrap());
    let t3 = b.unit(b.id_of("aster_t3_fabricator").unwrap());
    assert_eq!(t2.upgrades_to, Some(t3.id));
    assert_eq!((t2.footprint, t3.footprint), ((2, 2), (2, 2)));
    assert!(t2.volatile() && t3.volatile());
    assert!(b.id_of("aster_t1_fabricator").is_none());
    assert!(b.id_of("regency_t1_fabricator").is_none());
}
