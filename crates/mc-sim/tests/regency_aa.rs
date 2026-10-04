//! The Regency's anti-air structures. The Gorget (`regency_t2_aa`) fires pinched bolts that
//! burst on a timed fuse where they were laid (`Weapon::airburst`); the Belfry
//! (`regency_t3_aa`) empties its eight cells in one salvo, each seeker leaving straight up
//! the moment it is fired.

use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller, FireState, UnitId};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

const GORGET: &str = "regency_t2_aa";
const BELFRY: &str = "regency_t3_aa";

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
    let map = MapData {
        name: "regency".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(4000, 4000)],
        props: Vec::new(),
    };
    let player = |faction: &str, team| PlayerSetup {
        name: faction.into(),
        faction: faction.into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 7,
        players: vec![player("Regency", 0), player("Aster", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

/// Spawns `key`; the other side's units hold their fire.
fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> UnitId {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap();
    if owner != 0 {
        w.state.units.fire_state[row] = FireState::HoldFire;
    }
    w.state.units.id(row)
}

/// A Gorget bolt laid beside a gunship, wider than its proximity fuse, still bursts on its
/// timed fuse where it was laid, and its splash takes the gunship.
#[test]
fn a_gorget_bolt_bursts_on_its_fuse_beside_a_gunship() {
    let mut w = world();
    let gun_bp = w.blueprints.id_of(GORGET).unwrap();
    let bolt = w.blueprints.unit(gun_bp).weapons[0].clone();
    assert!(bolt.airburst && !bolt.flak);
    let target_bp = w.blueprints.id_of("aster_t1_rotor_gunship").unwrap();
    let id = add(&mut w, "aster_t1_rotor_gunship", 1, 900, 900);
    let near = w.state.units.row(id).unwrap();
    w.state.units.flags[near] |= flag::PASSIVE;
    w.state.units.health[near] = Fx::from_int(2000);
    let radius = w.blueprints.unit(target_bp).radius;
    let z = w.state.units.z[near] + w.blueprints.unit(target_bp).height / 2;
    let wide = bolt.splash + radius - Fx::from_int(4);
    assert!(wide > bolt.proximity + radius + Fx::ONE);
    let mark = FxVec2::new(Fx::from_int(900), Fx::from_int(900) - wide).extend(z);
    let from = mark - FxVec3::new(Fx::from_int(200), Fx::ZERO, Fx::ZERO);
    w.state
        .projectiles
        .spawn(
            from,
            FxVec3::new(Fx::from_int(30), Fx::ZERO, Fx::ZERO),
            0,
            mc_sim::Handle::NONE,
            gun_bp,
            0,
            60,
        )
        .unwrap();
    let shot = w.state.projectiles.len() - 1;
    w.state.projectiles.mark[shot] = mark;
    let mut burst = None;
    for _ in 0..12 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::Impact { pos, on_unit, .. } = e {
                assert!(!on_unit, "it burst on the fuse, not on a hull");
                burst = Some(*pos);
            }
        }
        if burst.is_some() {
            break;
        }
    }
    let at = burst.expect("the bolt never burst");
    assert!(
        at.distance(mark) < Fx::ONE,
        "burst {at:?}, laid on {mark:?}"
    );
    assert_eq!(w.state.units.health[near], Fx::from_int(2000) - bolt.damage);
}

/// With aircraft in reach, a Belfry empties all eight cells in one salvo, every seeker
/// leaving straight up at once, and spreads them over the aircraft.
#[test]
fn a_belfry_salvo_leaves_its_cells_at_once() {
    let mut w = world();
    let silo_bp = w.blueprints.id_of(BELFRY).unwrap();
    add(&mut w, BELFRY, 0, 1000, 1000);
    for y in [700, 900, 1100, 1300] {
        add(&mut w, "aster_t1_rotor_gunship", 1, 2600, y);
    }
    let mut launches = Vec::new();
    for _ in 0..(6 * TICKS_PER_SECOND) {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if let SimEvent::ShotFired { vel, blueprint, .. } = e {
                if *blueprint == silo_bp {
                    launches.push(*vel);
                }
            }
        }
    }
    assert_eq!(launches.len(), 8, "one full salvo");
    for v in &launches {
        assert!(
            v.z > Fx::ZERO && v.x.abs() < Fx::ONE / 100 && v.y.abs() < Fx::ONE / 100,
            "a seeker left at {v:?}, not straight up"
        );
    }
}
