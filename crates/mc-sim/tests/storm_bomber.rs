//! The Nimbus (`aster_t4_storm_bomber`): while foes are in reach its Storm Seeder lets a
//! cloud go from the tail every reload, aimed at nothing, so a run lays a trail of them;
//! each grows where it hangs and strikes down onto what is under it (`Weapon::cloud`,
//! `cloud_seed.rs`). Its twin AA mounts reach out at aircraft. `cargo test -p mc-sim --test sim -- storm_bomber::`

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const NIMBUS: &str = "aster_t4_storm_bomber";
const TANK: &str = "aster_t2_tank";
const SCOUT: &str = "aster_t1_air_scout";

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
        faction: "Aster".into(),
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

fn spawn(w: &World, owner: u8, key: &str, x: i32, y: i32, flags: u16) -> PlayerCommand {
    PlayerCommand {
        player: 0,
        command: Command::DebugSpawn {
            owner,
            blueprint: w.blueprints.id_of(key).unwrap(),
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags,
            build: 1000,
        },
    }
}

fn health(w: &World, key: &str) -> f64 {
    let bp = w.blueprints.id_of(key).unwrap();
    w.state
        .units
        .slots
        .iter()
        .filter(|&r| w.state.units.blueprint[r] == bp)
        .map(|r| w.state.units.health[r].to_f64())
        .sum()
}

#[test]
fn a_run_lays_a_trail_of_clouds_that_strike_down_onto_what_it_crossed() {
    let mut w = world();
    let nimbus = w.blueprints.id_of(NIMBUS).unwrap();
    let seeder = &w.blueprints.unit(nimbus).weapons[0];
    let cloud = seeder.cloud.expect("the Storm Seeder seeds clouds");
    let reload = seeder.reload_ticks as u32;
    // A column of tanks along its line of flight (it flies east, along x).
    let mut spawns = vec![spawn(&w, 1, NIMBUS, 1000, 1000, flag::INVULNERABLE)];
    for k in 0..6 {
        spawns.push(spawn(&w, 0, TANK, 1150 + 60 * k, 1000, flag::PASSIVE));
    }
    w.tick(&spawns).unwrap();
    let full = health(&w, TANK);
    let (mut seeded, mut bolts) = (Vec::new(), Vec::new());
    for tick in 1..=200u32 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match *e {
                SimEvent::ShotFired {
                    pos,
                    blueprint,
                    weapon: 0,
                    ..
                } if blueprint == nimbus => seeded.push((tick, pos)),
                SimEvent::CloudBolt {
                    from,
                    to,
                    blueprint,
                    ..
                } if blueprint == nimbus => bolts.push((tick, from, to)),
                _ => {}
            }
        }
    }
    println!("seeded {seeded:?}\nbolts {bolts:?}");
    let &(first, at) = seeded
        .first()
        .expect("it seeds as soon as the tanks are in reach");
    // Once it is up to speed (a parked or slow aircraft holds fire), nothing lined up.
    assert!(first <= 15, "no run lined up first: seeded on tick {first}");
    assert!(
        at.z > Fx::from_int(200),
        "let go at the wing's height, {at:?}"
    );
    // From the dispenser at the tail, on its line.
    assert!(
        at.x < Fx::from_int(1000) && (at.y - Fx::from_int(1000)).abs() < Fx::from_int(4),
        "let go from the tail, {at:?}"
    );
    // A seed every reload while the tanks are in reach, each let go further along: a trail.
    assert!(seeded.len() >= 5, "a trail of seeds: {}", seeded.len());
    for pair in seeded.windows(2).take(4) {
        assert_eq!(
            pair[1].0 - pair[0].0,
            reload,
            "it seeds on its reload, not on a mark"
        );
        assert!(
            pair[1].1.x > pair[0].1.x + Fx::from_int(20),
            "each further along"
        );
    }
    // Each cloud strikes from about where it was let go: carried on a little, never out to
    // a mark of its own.
    for &(tick, released) in seeded.iter().take(4) {
        let own: Vec<_> = bolts
            .iter()
            .filter(|b| {
                b.0 >= tick + cloud.grow_ticks as u32 && b.0 < tick + cloud.life_ticks() as u32
            })
            .filter(|b| b.1.xy().distance(released.xy()) < cloud.radius)
            .collect();
        assert!(!own.is_empty(), "the cloud let go on tick {tick} struck");
        for &&(_, from, to) in &own {
            assert!(from.z > Fx::from_int(200));
            assert!(to.z < Fx::from_int(40), "down to the ground: {to:?}");
            assert!(
                to.xy().distance(from.xy()) <= cloud.radius / 3 + Fx::from_int(10),
                "under the cloud: {to:?} from {from:?}"
            );
        }
    }
    let dealt = full - health(&w, TANK);
    println!("dealt {dealt:.0} of {full:.0}");
    assert!(
        dealt > full / 4.0,
        "the bolts burst on the tanks under the trail"
    );
}

#[test]
fn its_twin_aa_mounts_reach_out_at_aircraft() {
    let mut w = world();
    let nimbus = w.blueprints.id_of(NIMBUS).unwrap();
    let bp = w.blueprints.unit(nimbus);
    let (aa, reach) = bp
        .weapons
        .iter()
        .filter(|w| w.target_mask & mc_data::cat::AIR != 0)
        .fold((0, Fx::ZERO), |(n, r), w| (n + 1, r.max(w.range_max)));
    assert_eq!(aa, 4, "four mounts");
    for gun in bp
        .weapons
        .iter()
        .filter(|w| w.target_mask & mc_data::cat::AIR != 0)
    {
        // Two barrels fired together, no spinning cluster.
        assert_eq!((gun.salvo, gun.salvo_batch, gun.muzzles.len()), (2, 2, 2));
        assert_eq!(gun.spin_ticks, 0);
    }
    assert!(reach >= Fx::from_int(600), "decent reach: {reach:?}");
    // A scout hanging 500 m off the wing.
    let spawns = [
        spawn(&w, 1, NIMBUS, 1000, 1000, flag::INVULNERABLE),
        spawn(&w, 0, SCOUT, 1500, 1000, flag::PASSIVE),
    ];
    w.tick(&spawns).unwrap();
    let mut shots = 0;
    for _ in 0..60 {
        w.tick(&[]).unwrap();
        shots += w
            .events
            .iter()
            .filter(|e| {
                matches!(e, SimEvent::ShotFired { blueprint, weapon, .. }
                    if *blueprint == nimbus && *weapon > 0)
            })
            .count();
    }
    assert!(shots > 0, "the guns open on the scout");
}

#[test]
fn its_lasers_burn_missiles_and_a_field_covers_its_hull() {
    let mut w = world();
    let nimbus = w.blueprints.id_of(NIMBUS).unwrap();
    assert!(
        w.blueprints
            .unit(nimbus)
            .shield
            .is_some_and(|s| s.is_hull()),
        "a hull field"
    );
    w.tick(&[spawn(&w, 1, NIMBUS, 1000, 1000, flag::INVULNERABLE)])
        .unwrap();
    w.state.players[1].energy = Fx::from_int(100_000);
    let row = w
        .state
        .units
        .slots
        .iter()
        .find(|&r| w.state.units.blueprint[r] == nimbus)
        .unwrap();
    // A rocket from the Wasp's pod (slot 1) where the bomber flies.
    let rocket = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
    let at = w.state.units.pos[row].extend(w.state.units.z[row]);
    w.state
        .projectiles
        .spawn(
            at,
            mc_core::FxVec3::ZERO,
            0,
            mc_sim::Handle::NONE,
            rocket,
            1,
            30,
        )
        .unwrap();
    w.tick(&[]).unwrap();
    assert!(
        w.events
            .iter()
            .any(|e| matches!(e, SimEvent::MissileLased { killed: true, .. })),
        "the rocket was burned"
    );
}
