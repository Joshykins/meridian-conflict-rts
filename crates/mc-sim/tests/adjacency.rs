//! Adjacency (`mc_sim::adjacency`): reactors against a building save it energy,
//! fabricators against a factory save it materials, and a fabricator's blast takes a
//! power plant of its tier that touches it (and a reactor's its fabricators).

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

/// What a ring of `key`s would save of `r`.
fn ring(w: &World, key: &str, r: Resource) -> f32 {
    let a = w
        .blueprints
        .unit(w.blueprints.id_of(key).unwrap())
        .adjacency
        .unwrap();
    match r {
        Resource::Mass => a.mass,
        Resource::Energy => a.energy,
    }
    .to_f32()
}

#[test]
fn a_full_ring_saves_more_the_more_a_provider_makes() {
    let w = world();
    for (key, r, want) in [
        ("aster_t1_power", Resource::Energy, 0.2006),
        ("aster_t2_power", Resource::Energy, 0.388),
        ("aster_t3_power", Resource::Energy, 0.6),
        ("regency_t1_power", Resource::Energy, 0.2006),
        ("aster_t2_fabricator", Resource::Mass, 0.296),
        ("aster_t3_fabricator", Resource::Mass, 0.4),
    ] {
        let got = ring(&w, key, r);
        assert!((got - want).abs() < 2e-3, "{key}: {got}");
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
    // One side of four: a quarter of a ring's 60%.
    assert!((saving(&w, fab, Resource::Energy) - 0.15).abs() < 1e-3);
    let next_door = w.flows[row(&w, fab)].wanted[1].to_f32();
    assert!(
        (next_door / alone - 0.85).abs() < 1e-3,
        "upkeep {alone} -> {next_door}"
    );
    assert!(w.adjacency.links.iter().any(|l| l.consumer == fab));
}

#[test]
fn savings_grow_with_every_side_until_the_building_is_ringed() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t3_fabricator", 792, 792, 1000);
    // Four Reactor IIIs pinwheeled round its lot (780..804 both ways), each lot covering
    // one whole side and clear of the others'.
    let sides = [(852, 756), (732, 828), (828, 852), (756, 732)];
    for (n, (x, y)) in sides.into_iter().enumerate() {
        spawn(&mut w, 0, "aster_t3_power", x, y, 1000);
        w.tick(&[]).unwrap();
        let want = 0.15 * (n + 1) as f32;
        let got = saving(&w, fab, Resource::Energy);
        assert!((got - want).abs() < 1e-3, "{} sides: {got}", n + 1);
    }
}

#[test]
fn a_ring_of_small_and_big_plants_adds_up_side_by_side() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t2_fabricator", 792, 792, 1000);
    // A Reactor III along the east side and a Reactor I (24 m, a whole side of a 2x2)
    // against each of the other three.
    spawn(&mut w, 0, "aster_t3_power", 852, 792, 1000);
    spawn(&mut w, 0, "aster_t1_power", 768, 792, 1000);
    spawn(&mut w, 0, "aster_t1_power", 792, 816, 1000);
    spawn(&mut w, 0, "aster_t1_power", 792, 768, 1000);
    w.tick(&[]).unwrap();
    let want = (ring(&w, "aster_t3_power", Resource::Energy)
        + 3.0 * ring(&w, "aster_t1_power", Resource::Energy))
        / 4.0;
    let got = saving(&w, fab, Resource::Energy);
    assert!((got - want).abs() < 1e-3, "{got} against {want}");
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
    // The factory's lot is 96 m square: 384 m round. The fabricators each cover 24 m
    // of its east side, the reactor 36 m (804..840).
    let share = |key, r, m: f32| ring(&w, key, r) * m / 384.0;
    let mass = share("aster_t3_fabricator", Resource::Mass, 24.0)
        + share("aster_t2_fabricator", Resource::Mass, 24.0);
    assert!((saving(&w, factory, Resource::Mass) - mass).abs() < 1e-3);
    let energy = share("aster_t2_power", Resource::Energy, 36.0);
    assert!((saving(&w, factory, Resource::Energy) - energy).abs() < 1e-3);
}

/// Where a lot of `cells` square stands against the fabricator at (792, 792) (lot
/// 780..804) on its east side, slid as far north as it goes and still shares an edge:
/// the furthest a touching neighbour's centre can be.
fn far_east(cells: i32) -> (i32, i32) {
    let half = cells * 6;
    (804 + half, 792 + half)
}

#[test]
fn a_fabricators_blast_takes_a_power_plant_of_its_tier_it_touches() {
    for (fab, plant, cells) in [
        ("aster_t2_fabricator", "aster_t2_power", 4),
        ("aster_t3_fabricator", "aster_t3_power", 8),
        ("regency_t2_fabricator", "regency_t2_power", 4),
        ("regency_t3_fabricator", "regency_t3_power", 8),
    ] {
        let mut w = world();
        let f = spawn(&mut w, 0, fab, 792, 792, 1000);
        let (x, y) = far_east(cells);
        let p = spawn(&mut w, 0, plant, x, y, 1000);
        destroy(&mut w, f);
        assert!(w.state.units.row(p).is_none(), "{fab}'s blast left {plant}");
    }
}

#[test]
fn a_reactors_blast_takes_the_fabricators_it_touches() {
    for (plant, fab, cells) in [
        ("aster_t2_power", "aster_t2_fabricator", 4),
        ("aster_t3_power", "aster_t3_fabricator", 8),
    ] {
        let mut w = world();
        let f = spawn(&mut w, 0, fab, 792, 792, 1000);
        let (x, y) = far_east(cells);
        let p = spawn(&mut w, 0, plant, x, y, 1000);
        destroy(&mut w, p);
        assert!(w.state.units.row(f).is_none(), "{plant}'s blast left {fab}");
    }
    // A Power Generator burns out as light, with no blast: its Condenser stands.
    let mut w = world();
    let f = spawn(&mut w, 0, "regency_t2_fabricator", 792, 792, 1000);
    let p = spawn(&mut w, 0, "regency_t2_power", 828, 792, 1000);
    destroy(&mut w, p);
    assert!(w.state.units.row(f).is_some());
}

#[test]
fn a_fabricators_blast_leaves_a_factory_of_its_tier_standing() {
    for (fab, factory) in [
        ("aster_t2_fabricator", "aster_t2_air_factory"),
        ("aster_t3_fabricator", "aster_t3_air_factory"),
        ("regency_t2_fabricator", "regency_t2_air_factory"),
        ("regency_t3_fabricator", "regency_t3_air_factory"),
    ] {
        let mut w = world();
        let f = spawn(&mut w, 0, fab, 792, 792, 1000);
        // Flush against the fabricator's west side, centre to centre: the worst of it.
        let k = spawn(&mut w, 0, factory, 732, 792, 1000);
        destroy(&mut w, f);
        assert!(
            w.state.units.row(k).is_some(),
            "{fab}'s blast took {factory}"
        );
    }
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

#[test]
fn the_mirror_shows_how_hard_a_fabricator_works() {
    let mut w = world();
    let fab = spawn(&mut w, 0, "aster_t2_fabricator", 792, 792, 1000);
    let work = |w: &World| {
        let mut frame = mc_sim::mirror::RenderFrame::default();
        w.write_render_frame(Some(0), &mut frame);
        frame
            .units
            .iter()
            .find(|u| u.unit_id == fab.0)
            .expect("drawn")
            .deploy
    };
    w.tick(&[]).unwrap();
    assert_eq!(work(&w), 0.0, "no power, no work");
    spawn(&mut w, 0, "aster_t3_power", 552, 552, 1000);
    w.tick(&[]).unwrap();
    assert!((work(&w) - 1.0).abs() < 1e-3, "full power, full work");
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::SetPaused {
            units: vec![fab],
            paused: true,
        },
    }])
    .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(work(&w), 0.0, "paused");
}
