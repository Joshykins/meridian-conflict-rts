//! The Atoll's SAM battery (`launch_cells.rs` on a ship): its twelve cell hatches open
//! before a salvo and shut while the cells reload, it reaches aircraft far past any
//! gun afloat, and a salvo is spread over the aircraft in range.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

/// Open sea 30 m deep, 8 km across.
fn sea() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(7000, 7000)],
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let terrain = Heightfield::from_samples(
        1024,
        1024,
        vec![0; 1025 * 1025],
        Fx::from_int(-30),
        Fx::ONE,
        Fx::ZERO,
    );
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(id, owner, FxVec2::from_ints(x, y), Angle::ZERO, true)
        .unwrap()
}

/// Aircraft that neither fight back nor die, at `spots`.
fn targets(w: &mut World, spots: &[(i32, i32)]) -> Vec<usize> {
    spots
        .iter()
        .map(|&(x, y)| {
            let t = add(w, "aster_t2_interceptor", 1, x, y);
            w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
            t
        })
        .collect()
}

#[test]
fn atoll_opens_its_hatches_and_spreads_a_salvo_far_out() {
    let mut w = sea();
    let atoll = add(&mut w, "aster_t3_carrier", 0, 3000, 3000);
    let bp = w.state.units.blueprint[atoll];
    let weapon = w.blueprints.unit(bp).weapons[0].clone();
    assert_eq!(weapon.muzzles.len(), 12);
    // Past any gun's reach and a Manta's, inside the battery's.
    let far: Vec<(i32, i32)> = (0..6)
        .map(|k| (3000 + 2200 + (k % 2) * 300, 2600 + k * 160))
        .collect();
    let rows = targets(&mut w, &far);
    let ids: Vec<_> = rows.iter().map(|&r| w.state.units.id(r)).collect();
    let (mut fired, mut shut, mut opened_again) = (0, false, false);
    let mut chased = BTreeSet::new();
    for _ in 0..(weapon.reload_ticks as usize + 80) {
        w.tick(&[]).unwrap();
        for e in &w.events {
            if matches!(e, SimEvent::ShotFired { blueprint, .. } if *blueprint == bp) {
                assert_eq!(
                    w.state.units.deploy[atoll], weapon.hatch_ticks,
                    "no missile leaves before the hatches are open"
                );
                fired += 1;
            }
        }
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            if p.blueprint[i] == bp && ids.contains(&p.target[i]) {
                chased.insert(p.target[i]);
            }
        }
        if fired == 12 && w.state.units.deploy[atoll] == 0 {
            shut = true;
        }
        if shut && w.state.units.deploy[atoll] == weapon.hatch_ticks {
            opened_again = true;
        }
    }
    assert!(fired >= 12, "a full salvo at 2.2 km ({fired})");
    assert_eq!(
        chased.len(),
        6,
        "a salvo spread over every aircraft in range"
    );
    assert!(
        shut && opened_again,
        "the hatches shut to reload and open again"
    );
}
