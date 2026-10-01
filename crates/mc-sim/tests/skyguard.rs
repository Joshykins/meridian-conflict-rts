//! The Skyguard's salvo (`launch_cells.rs`): the cell hatches open before it, each
//! missile is boosted straight up out of its cell, coasts while it turns over onto its
//! mark and only then lights its motor, and the four missiles go after different
//! targets while there are any.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::mirror::{RenderFrame, PROJECTILE_COLD, PROJECTILE_MISSILE};
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{MatchConfig, PlayerSetup, SimEvent, World};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

fn world() -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let map = MapData {
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
        seed: 3,
        players: vec![player("you", 0), player("hostile", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let terrain = Heightfield::flat(256, 256, Fx::from_int(20));
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
            let t = add(w, "aster_t2_reclaim_carrier", 1, x, y);
            w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
            t
        })
        .collect()
}

#[test]
fn skyguard_boosts_up_turns_over_then_lights() {
    let mut w = world();
    let sam = add(&mut w, "aster_t3_sam", 0, 600, 900);
    let target = targets(&mut w, &[(1200, 900)])[0];
    let bp = w.state.units.blueprint[sam];
    let weapon = w.blueprints.unit(bp).weapons[0].clone();
    let (boost, cold, hatch) = (
        weapon.boost_ticks,
        weapon.cold_launch_ticks,
        weapon.hatch_ticks,
    );
    assert!(boost > 0 && cold > boost && hatch > 0);
    let cruise = weapon.projectile_speed / 10;
    let mut ignitions = 0;
    let mut launches = 0;
    let (mut saw_boost, mut saw_coast, mut saw_turn) = (false, false, false);
    // Climb a tick, by age.
    let mut vz = BTreeMap::new();
    let mut frame = RenderFrame::default();
    for _ in 0..80 {
        w.tick(&[]).unwrap();
        for e in &w.events {
            match e {
                SimEvent::ShotFired { blueprint, .. } if *blueprint == bp => {
                    assert_eq!(
                        w.state.units.deploy[sam], hatch,
                        "no missile leaves before the hatches are open"
                    );
                    launches += 1;
                }
                SimEvent::MissileIgnited {
                    pos,
                    vel,
                    blueprint,
                    ..
                } if *blueprint == bp => {
                    ignitions += 1;
                    assert!(pos.z > w.state.units.z[sam] + Fx::from_int(50), "{pos:?}");
                    let aim = w.state.units.pos[target].extend(
                        w.state.units.z[target]
                            + w.blueprints.unit(w.state.units.blueprint[target]).height / 2,
                    );
                    assert!(
                        vel.normalize().dot((aim - *pos).normalize()) > Fx::ratio(95, 100),
                        "it lights already turned onto the intercept"
                    );
                }
                _ => {}
            }
        }
        let p = &w.state.projectiles;
        for i in 0..p.len() {
            if p.blueprint[i] != bp {
                continue;
            }
            // One target: every missile of the salvo goes after it.
            assert_eq!(p.target[i], w.state.units.id(target));
            let age = p.age[i];
            if age < cold {
                assert_eq!(
                    p.vel[i].xy().length(),
                    Fx::ZERO,
                    "straight up until the motor lights"
                );
                assert!(p.vel[i].z > Fx::ZERO, "still climbing at {age}");
            }
            if age <= boost {
                assert!(p.aim[i].z > Fx::ratio(99, 100) || p.aim[i] == mc_core::FxVec3::ZERO);
            }
            // Every missile flies the same climb.
            vz.entry(age).or_insert(p.vel[i].z);
            if age > cold + 5 {
                assert!(p.vel[i].length() > cruise / 2);
            }
        }
        w.write_render_frame(None, &mut frame);
        for p in &frame.projectiles {
            if p.color & PROJECTILE_MISSILE == 0 {
                continue;
            }
            if p.color & PROJECTILE_COLD != 0 {
                saw_coast = true;
                saw_turn |= p._pad[1] < 0.0 && p.aim[0] > 0.5;
            } else if p.pos[2] - p.prev_pos[2] > 0.0 && (p.pos[0] - p.prev_pos[0]).abs() < 0.01 {
                // The booster's flame and smoke: drawn powered on the way up.
                saw_boost = true;
            }
        }
    }
    assert_eq!(launches, 4, "a salvo of four");
    assert_eq!(ignitions, 4);
    assert!(saw_boost && saw_coast && saw_turn);
    // The booster speeds it up, the coast slows it.
    assert!(vz[&1] < vz[&boost], "{vz:?}");
    assert!(vz[&boost] > vz[&(cold - 1)], "{vz:?}");
}

#[test]
fn skyguard_salvo_spreads_over_targets() {
    for (spots, want) in [
        (
            &[(1200, 900), (1200, 1150), (1050, 650), (1350, 1300)][..],
            4,
        ),
        (&[(1200, 900), (1200, 1150)][..], 2),
    ] {
        let mut w = world();
        let sam = add(&mut w, "aster_t3_sam", 0, 600, 900);
        let ids: BTreeSet<_> = targets(&mut w, spots)
            .into_iter()
            .map(|t| w.state.units.id(t))
            .collect();
        let bp = w.state.units.blueprint[sam];
        let mut chased = BTreeMap::new();
        for _ in 0..30 {
            w.tick(&[]).unwrap();
            let p = &w.state.projectiles;
            for i in 0..p.len() {
                if p.blueprint[i] == bp && p.age[i] == 1 {
                    assert!(ids.contains(&p.target[i]));
                    *chased.entry(p.target[i]).or_insert(0) += 1;
                }
            }
        }
        assert_eq!(chased.values().sum::<i32>(), 4, "{chased:?}");
        assert_eq!(chased.len(), want, "{chased:?}");
        assert!(
            chased.values().all(|&n| n == 4 / want as i32),
            "shared out evenly: {chased:?}"
        );
    }
}

#[test]
fn skyguard_hatches_shut_while_the_cells_reload() {
    let mut w = world();
    let sam = add(&mut w, "aster_t3_sam", 0, 600, 900);
    targets(&mut w, &[(1200, 900)]);
    let weapon = w.blueprints.unit(w.state.units.blueprint[sam]).weapons[0].clone();
    let mut opened_again = false;
    let mut shut = false;
    let mut fired = 0;
    for _ in 0..(weapon.reload_ticks as usize + 60) {
        w.tick(&[]).unwrap();
        fired += w
            .events
            .iter()
            .filter(|e| matches!(e, SimEvent::ShotFired { .. }))
            .count();
        if fired == 4 && w.state.units.deploy[sam] == 0 {
            shut = true;
        }
        if shut && w.state.units.deploy[sam] == weapon.hatch_ticks {
            opened_again = true;
        }
    }
    assert!(shut, "the hatches shut after the salvo");
    assert!(
        opened_again && fired == 8,
        "and open again for the next ({fired})"
    );
}
