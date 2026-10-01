use super::super::profile::{role, Domain, Profile, Profiles};
use super::*;
use crate::command::{Command, PlayerCommand};
use crate::tables::Controller;
use crate::world::MapData;
use crate::{AiConfig, MatchConfig, PlayerSetup, World};
use mc_core::{Angle, FxVec2};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::Heightfield;
use std::{path::Path, sync::Arc};

fn roster() -> Arc<Blueprints> {
    Arc::new(Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap())
}

#[test]
fn the_estimate_knows_who_can_hit_whom() {
    let b = roster();
    let p = |k: &str| Profile::of(b.unit(b.id_of(k).unwrap()));
    let tank = p("aster_t1_tank");
    let aa = p("aster_t1_mobile_aa");
    let bomber = p("aster_t1_bomber");
    let fighter = p("aster_t1_interceptor");
    assert!(fight(&tank, &aa) > Fx::ONE, "AA cannot shoot tanks");
    assert!(
        fight(&bomber, &tank) > Fx::ONE,
        "tanks cannot shoot bombers"
    );
    assert!(fight(&fighter, &bomber) > Fx::ONE);
    assert!(fight(&p("aster_t2_tank"), &tank) > Fx::ONE);
    // A sub under water: only torpedoes reach it.
    let sub = p("aster_t1_submarine");
    assert!(fight(&sub, &p("aster_t1_frigate")) > Fx::ONE);
    assert!(edge(&tank, &tank).abs() < Fx::ratio(1, 100));
}

/// A flat arena `side` 8 m cells across: land at 20 m, or sea 40 m deep.
fn arena(b: &Arc<Blueprints>, sea: bool) -> World {
    let config = MatchConfig {
        seed: 3,
        cheats: true,
        fog: false,
        spawn_commanders: false,
        players: (0..2)
            .map(|i| PlayerSetup {
                name: format!("P{i}"),
                faction: "Aster".into(),
                team: i,
                controller: Controller::Human,
                start: i,
                ai: AiConfig::default(),
            })
            .collect(),
    };
    let side = 512u32;
    let terrain = if sea {
        let n = (side as usize + 1) * (side as usize + 1);
        Heightfield::from_samples(side, side, vec![0; n], Fx::ZERO, Fx::ONE, Fx::from_int(40))
    } else {
        Heightfield::flat(side, side, Fx::from_int(20))
    };
    let mut w = World::with_terrain(
        terrain,
        MapData {
            name: "arena".into(),
            content_id: 3,
            ore: Vec::new(),
            starts: vec![FxVec2::from_ints(500, 2048), FxVec2::from_ints(3500, 2048)],
            props: vec![],
        },
        b.clone(),
        Arc::new(Pool::new(0)),
        &config,
    )
    .unwrap();
    for p in &mut w.state.players {
        p.bonus_storage[1] = Fx::from_int(500_000);
        p.energy = Fx::from_int(500_000);
    }
    w
}

/// Most units a side fields in one staged fight.
const MOST: i32 = 48;

/// Stages equal mass of `a` (player 0) against `b` (player 1) and plays it out:
/// the mass each has left. `None` when the pair cannot be staged.
fn stage(blueprints: &Arc<Blueprints>, a: &Profile, b: &Profile) -> Option<(Fx, Fx)> {
    let sea = |p: &Profile| matches!(p.domain, Some(Domain::Naval | Domain::Sub));
    let land = |p: &Profile| matches!(p.domain, Some(Domain::Land));
    // A ship and a tank never meet.
    if (sea(a) && land(b)) || (land(a) && sea(b)) {
        return None;
    }
    let stake = STAKE.max(a.cost).max(b.cost);
    let count = |p: &Profile| (stake / p.cost).round_int().max(1);
    let (na, nb) = (count(a), count(b));
    if na > MOST || nb > MOST {
        return None;
    }
    let mut w = arena(blueprints, sea(a) || sea(b));
    let place = |w: &mut World, id, owner: u8, n: i32, x: i32| {
        for i in 0..n {
            let (col, row) = (i % 8, i / 8);
            let at = FxVec2::from_ints(
                x + row * 40 * if owner == 0 { -1 } else { 1 },
                1900 + col * 40,
            );
            let facing = if owner == 0 {
                Angle::ZERO
            } else {
                Angle::HALF_TURN
            };
            w.spawn_unit(id, owner, at, facing, true).unwrap();
        }
    };
    place(&mut w, a.id, 0, na, 1500);
    place(&mut w, b.id, 1, nb, 2500);
    let ids = |w: &World, owner: u8| -> Vec<crate::tables::UnitId> {
        w.state
            .units
            .slots
            .iter()
            .filter(|&r| w.state.units.owner[r] == owner)
            .map(|r| w.state.units.id(r))
            .collect()
    };
    let mass = |w: &World, owner: u8| -> Fx {
        w.state
            .units
            .slots
            .iter()
            .filter(|&r| w.state.units.owner[r] == owner)
            .map(|r| w.bp(r).cost_mass * w.state.units.health[r] / w.bp(r).health)
            .sum()
    };
    for t in 0..2400 {
        let cmds = if t % 100 == 0 {
            vec![
                PlayerCommand {
                    player: 0,
                    command: Command::AttackMove {
                        units: ids(&w, 0),
                        target: FxVec2::from_ints(2600, 2048),
                        queue: false,
                    },
                },
                PlayerCommand {
                    player: 1,
                    command: Command::AttackMove {
                        units: ids(&w, 1),
                        target: FxVec2::from_ints(1400, 2048),
                        queue: false,
                    },
                },
            ]
        } else {
            vec![]
        };
        w.tick(&cmds).unwrap();
        if t % 50 == 0 && (mass(&w, 0) == Fx::ZERO || mass(&w, 1) == Fx::ZERO) {
            break;
        }
    }
    let start_a = a.mass * na;
    let start_b = b.mass * nb;
    Some((
        mass(&w, 0) / start_a.max(Fx::ONE),
        mass(&w, 1) / start_b.max(Fx::ONE),
    ))
}

/// Staged fights between every pair of combat units, against the estimate: how often
/// the estimate picks the winner, and each pair it gets wrong. A calibration probe:
/// `MATCHUP=SHARD/SHARDS` (default 0/1), `MATCHUP_ONLY=key` for one unit's pairs.
/// `cargo test --profile gate -p mc-sim --lib ai::commander::matchup::tests::zz_matchup_probe -- --ignored --nocapture`
#[test]
#[ignore]
fn zz_matchup_probe() {
    let b = roster();
    let profiles = Profiles::build(&b);
    let (shard, shards) = std::env::var("MATCHUP")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('/')?;
            Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
        })
        .unwrap_or((0, 1));
    let only = std::env::var("MATCHUP_ONLY").ok();
    let combat: Vec<&Profile> = b
        .units
        .iter()
        .filter(|u| {
            !u.has(cat::COMMANDER | cat::ENGINEER)
                && u.key.starts_with("aster_")
                && !u.key.contains('#')
                && !u.key.contains('+')
        })
        .map(|u| profiles.get(u.id))
        .filter(|p| {
            p.mobile()
                && p.armed()
                && (p.roles & role::PROJECT == 0 || p.domain == Some(Domain::Space))
        })
        .collect();
    let (mut right, mut wrong, mut n) = (0, 0, 0usize);
    for (i, a) in combat.iter().enumerate() {
        for bp in &combat[i + 1..] {
            n += 1;
            if n % shards != shard {
                continue;
            }
            let key = |p: &Profile| b.unit(p.id).key.clone();
            if only.as_ref().is_some_and(|k| key(a) != *k && key(bp) != *k) {
                continue;
            }
            let guess = edge(a, bp);
            let Some((la, lb)) = stage(&b, a, bp) else {
                continue;
            };
            let real = la - lb;
            let sure = guess.abs() > Fx::ratio(1, 5);
            let agree = (guess > Fx::ZERO) == (real > Fx::ZERO);
            if !sure || real.abs() < Fx::ratio(1, 10) {
                // Too close to call either way.
            } else if agree {
                right += 1;
            } else {
                wrong += 1;
            }
            println!(
                "{} {:<26} vs {:<26} guess {} real {} (left {}/{})",
                if sure && real.abs() >= Fx::ratio(1, 10) && !agree {
                    "MISS"
                } else {
                    "    "
                },
                key(a),
                key(bp),
                hundredths(guess),
                hundredths(real),
                hundredths(la),
                hundredths(lb)
            );
        }
    }
    println!("RESULT matchups right {right} wrong {wrong}");
}

/// `x` to two decimals, for the probe's report (no floats in mc-sim).
fn hundredths(x: Fx) -> String {
    let h = (x * 100).round_int();
    let sign = if h < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", h.abs() / 100, h.abs() % 100)
}
