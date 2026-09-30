//! How well fighters fight. Two parts:
//! - A flight of five fighters on one big slow air target (a Valiant, passive and
//!   too tough to die): per fighter, the share of ticks the target sat inside its gun
//!   arc and reach, the share it was off the nose and the nose was not coming round,
//!   and the longest spell between two shots.
//! - Equal fighter swarms flown into each other (attack-move at the other side's
//!   start), both ways round and at three spacings: survivors per side, seconds to
//!   the end, and how far from the merge the fight wandered.
//!
//! `cargo test --profile gate -p mc-sim --test sim -- zz_dogfight_probe:: --ignored --nocapture`
//! Knobs: DOGFIGHT_KEY=<fighter key> for the first part (default the Peregrine),
//! DOGFIGHT_SWARM=<key> and DOGFIGHT_N=<per side> for the second (default 25 Raptors).

use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::Heightfield;
use mc_sim::tables::{flag, Controller};
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn world(seed: u64) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(768, 768, Fx::from_int(20));
    let map = MapData {
        name: "range".into(),
        content_id: 1,
        ore: Vec::new(),
        starts: vec![FxVec2::from_ints(1000, 3072), FxVec2::from_ints(5000, 3072)],
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
        seed,
        players: vec![player("a", 0), player("b", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap()
}

fn add(w: &mut World, key: &str, owner: u8, x: i32, y: i32, heading: Angle) -> usize {
    let id = w.blueprints.id_of(key).unwrap();
    let row = w
        .spawn_unit(id, owner, FxVec2::from_ints(x, y), heading, true)
        .unwrap();
    if let Some(m) = w.blueprints.unit(id).motion {
        w.state.units.speed[row] = m.speed;
        w.state.units.z[row] = w.terrain.height_at(w.state.units.pos[row]) + m.altitude;
    }
    row
}

#[test]
#[ignore]
fn zz_dogfight_probe() {
    let key = std::env::var("DOGFIGHT_KEY").unwrap_or("aster_t2_interceptor".into());
    strike(&key);
    let swarm = std::env::var("DOGFIGHT_SWARM").unwrap_or("aster_t3_air_superiority".into());
    let n: i32 = std::env::var("DOGFIGHT_N").map_or(25, |v| v.parse().unwrap());
    let mut totals = [0usize; 2];
    for (seed, gap) in [(1u64, 2400), (2, 3000), (3, 3600)] {
        for (flip, late) in [(false, false), (true, false), (false, true)] {
            let (a, b, secs, wander) = swarms(&swarm, n, seed, gap, flip, late);
            totals[0] += a;
            totals[1] += b;
            println!(
                "swarm {swarm} {n}v{n} seed {seed} gap {gap} flip {flip} b-first {late}: survivors a {a} b {b}, over in {secs:.1} s, wandered {wander:.0} m"
            );
        }
    }
    println!("swarm totals a {} b {}", totals[0], totals[1]);
}

fn strike(key: &str) {
    let mut w = world(7);
    let t = add(&mut w, "aster_t2_corvette", 1, 3000, 3072, Angle::ZERO);
    w.state.units.flags[t] |= flag::PASSIVE | flag::INVULNERABLE;
    let fighters: Vec<usize> = (0..5)
        .map(|k| add(&mut w, key, 0, 2300, 2972 + 50 * k, Angle::ZERO))
        .collect();
    let ids = fighters.iter().map(|&r| w.state.units.id(r)).collect();
    w.tick(&[PlayerCommand {
        player: 0,
        command: Command::Attack {
            units: ids,
            target: w.state.units.id(t),
            queue: false,
        },
    }])
    .unwrap();
    let bp = w
        .blueprints
        .unit(w.state.units.blueprint[fighters[0]])
        .clone();
    let weapon = &bp.weapons[0];
    let ticks = 60 * TICKS_PER_SECOND as usize;
    let mut trained = vec![0usize; 5];
    let mut adrift = vec![0usize; 5];
    let mut last_shot = vec![0usize; 5];
    let mut worst_gap = vec![0usize; 5];
    let mut shots = vec![0usize; 5];
    let mut first_shot = vec![usize::MAX; 5];
    let mut prev_off = vec![0u16; 5];
    for tick in 0..ticks {
        w.tick(&[]).unwrap();
        let tp = w.state.units.pos[t];
        for (i, &f) in fighters.iter().enumerate() {
            let p = w.state.units.pos[f];
            let off = w.state.units.heading[f]
                .delta_to((tp - p).angle())
                .unsigned_abs();
            let reach = weapon.range_max + w.blueprints.unit(w.state.units.blueprint[t]).radius;
            if off <= weapon.half_arc && p.distance(tp) <= reach {
                trained[i] += 1;
            } else if off > weapon.half_arc && off >= prev_off[i] {
                adrift[i] += 1;
            }
            prev_off[i] = off;
            if i == 0 && std::env::var("DOGFIGHT_TRACE").is_ok() && tick < 300 {
                println!(
                    "t {tick} hd {} bank {} d {:.0} off {:.0} spd {:.0} z {:.0} tz {:.0} turn {} brk {} flags {:x}",
                    w.state.units.heading[f].0 as i32 * 360 / 65536,
                    w.state.units.bank[f],
                    p.distance(tp).to_f32(),
                    off as f32 * 360.0 / 65536.0,
                    w.state.units.speed[f].to_f32(),
                    w.state.units.z[f].to_f32(),
                    w.state.units.z[t].to_f32(),
                    w.state.units.air_turn_ticks[f],
                    w.state.units.air_break_ticks[f],
                    w.state.units.flags[f],
                );
            }
        }
        for e in &w.events {
            if let SimEvent::ShotFired { owner: 0, pos, .. } = e {
                let near = fighters
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, &f)| w.state.units.pos[f].distance(pos.xy()))
                    .unwrap()
                    .0;
                shots[near] += 1;
                if first_shot[near] == usize::MAX {
                    first_shot[near] = tick;
                } else {
                    worst_gap[near] = worst_gap[near].max(tick - last_shot[near]);
                }
                last_shot[near] = tick;
            }
        }
    }
    let per = |n: usize| n as f32 / TICKS_PER_SECOND as f32;
    for i in 0..5 {
        println!(
            "strike {key} #{i}: trained {:>3.0}%, off and not turning in {:>3.0}%, shots {:>3}, first at {:.1} s, longest gap {:.1} s",
            trained[i] as f32 * 100.0 / ticks as f32,
            adrift[i] as f32 * 100.0 / ticks as f32,
            shots[i],
            per(first_shot[i]),
            per(worst_gap[i]),
        );
    }
}

/// Returns survivors of side a and side b, seconds to the end, and the farthest any
/// fighter strayed from the midpoint between the two starts after the first shot.
/// `flip` swaps which player owns which side, `late` spawns the right side first
/// (lower rows).
fn swarms(
    key: &str,
    n: i32,
    seed: u64,
    gap: i32,
    flip: bool,
    late: bool,
) -> (usize, usize, f32, f32) {
    let mut w = world(seed);
    let cx = 3072;
    let cy = 3072;
    let mut sides = [Vec::new(), Vec::new()];
    for side in if late { [1u8, 0] } else { [0, 1] } {
        let sign = if side == 0 { -1 } else { 1 };
        let heading = if side == 0 {
            Angle::ZERO
        } else {
            Angle(0x8000)
        };
        for k in 0..n {
            let col = k % 5;
            let rank = k / 5;
            let x = cx + sign * (gap / 2 + rank * 30);
            let y = cy + (col - 2) * 40 + if side == 0 { 0 } else { 7 };
            let owner = if flip { 1 - side } else { side };
            sides[owner as usize].push(add(&mut w, key, owner, x, y, heading));
        }
    }
    let mut cmds = Vec::new();
    for owner in 0..2u8 {
        let target = w.state.units.pos[sides[1 - owner as usize][0]];
        cmds.push(PlayerCommand {
            player: owner,
            command: Command::AttackMove {
                units: sides[owner as usize]
                    .iter()
                    .map(|&r| w.state.units.id(r))
                    .collect(),
                target,
                queue: false,
            },
        });
    }
    w.tick(&cmds).unwrap();
    let ids: [Vec<_>; 2] = [
        sides[0].iter().map(|&r| w.state.units.id(r)).collect(),
        sides[1].iter().map(|&r| w.state.units.id(r)).collect(),
    ];
    let mid = FxVec2::from_ints(cx, cy);
    let mut wander = Fx::ZERO;
    let alive = |w: &World, s: usize| {
        ids[s]
            .iter()
            .filter(|&&id| w.state.units.row(id).is_some())
            .count()
    };
    let mut end = 0;
    let mut merged = false;
    for tick in 0..(180 * TICKS_PER_SECOND as usize) {
        w.tick(&[]).unwrap();
        merged |= w
            .events
            .iter()
            .any(|e| matches!(e, SimEvent::ShotFired { .. }));
        for s in 0..2 {
            if !merged {
                break;
            }
            for &id in &ids[s] {
                if let Some(r) = w.state.units.row(id) {
                    wander = wander.max(w.state.units.pos[r].distance(mid));
                }
            }
        }
        end = tick;
        if alive(&w, 0) == 0 || alive(&w, 1) == 0 {
            break;
        }
    }
    (
        alive(&w, 0),
        alive(&w, 1),
        end as f32 / TICKS_PER_SECOND as f32,
        wander.to_f32(),
    )
}
