//! One tournament match between two AI sides on a real map (`docs/AI_COMMANDER.md`,
//! "Tournament"): side A's doctrine against side B's, the same difficulty.
//! `scripts/ai-tournament.sh` plays many of these in parallel and adds them up.
//!
//! `TOURNEY=map:players:minutes:seed:A:B[:difficulty]`, where A and B are doctrines
//! (`adaptive`, `aggressive`, `economic` or `defensive`). With 2 players slot 0 is side A; with
//! more, the starts west of the middle are side A. `TOURNEY_EVERY=N` prints each
//! side's state every N minutes, `TOURNEY_ROSTER=1` each player's units at the end,
//! `TOURNEY_ARMY=key*n,key*n` gives every side the same army at its start,
//! `TOURNEY_SNAP=tick` each side's armed mobile units at that tick,
//! `TOURNEY_DEATHS=1` every unit that dies, where and when, `TOURNEY_OPS=N` slot N's
//! operations whenever one changes (every 10 s).
//! The `RESULT key=value ...` line sums the match up.
//!
//! `TOURNEY=serac_divide:2:40:7:adaptive:aggressive cargo test --profile gate -p mc-sim --test sim -- zz_ai_tournament:: --ignored --nocapture`
use mc_core::Fx;
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, Doctrine, MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn side(spec: &str) -> Doctrine {
    match spec {
        "aggressive" => Doctrine::Aggressive,
        "economic" => Doctrine::Economic,
        "defensive" => Doctrine::Defensive,
        _ => Doctrine::Adaptive,
    }
}

/// Mass of everything a side has standing, finished or not.
fn worth(w: &World, team: u8) -> Fx {
    let s = &w.state;
    s.units
        .slots
        .iter()
        .filter(|&r| s.players[s.units.owner[r] as usize].team == team)
        .map(|r| {
            let bp = w.bp(r);
            if bp.has(cat::COMMANDER) {
                Fx::ZERO
            } else {
                bp.cost_mass
            }
        })
        .sum()
}

#[test]
#[ignore]
fn match_up() {
    let spec = std::env::var("TOURNEY")
        .unwrap_or_else(|_| "serac_divide:2:30:7:adaptive:aggressive".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(2);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(30);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let a = side(parts.get(4).copied().unwrap_or("adaptive"));
    let b = side(parts.get(5).copied().unwrap_or("aggressive"));
    let difficulty = match parts.get(6).copied() {
        Some("easy") => Difficulty::Easy,
        Some("normal") => Difficulty::Normal,
        _ => Difficulty::Hard,
    };
    let deaths = std::env::var("TOURNEY_DEATHS").is_ok();
    let snap: Option<u32> = std::env::var("TOURNEY_SNAP")
        .ok()
        .and_then(|t| t.parse().ok());
    let ops_trace: Option<usize> = std::env::var("TOURNEY_OPS")
        .ok()
        .and_then(|p| p.parse().ok());
    let mut last_ops: Vec<String> = Vec::new();
    let every: u32 = std::env::var("TOURNEY_EVERY")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let starts = map.start_positions().to_vec();
    let mid = starts[..players as usize]
        .iter()
        .fold(Fx::ZERO, |s, p| s + p.x)
        / players as i32;
    let team_of = |i: usize| -> u8 {
        if players == 2 {
            i as u8
        } else {
            (starts[i].x > mid) as u8
        }
    };
    let config = MatchConfig {
        seed,
        players: (0..players as usize)
            .map(|i| {
                let doctrine = if team_of(i) == 0 { a } else { b };
                PlayerSetup {
                    name: format!("AI {i}"),
                    faction: "Aster".into(),
                    ai: AiConfig {
                        doctrine,
                        difficulty,
                        ..AiConfig::default()
                    },
                    team: team_of(i),
                    controller: Controller::Ai,
                    start: i as u8,
                }
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    // A head start: the same army for every side, beside its start, to judge how
    // each doctrine fights apart from how it builds.
    if let Ok(army) = std::env::var("TOURNEY_ARMY") {
        for part in army.split(',') {
            let (key, n) = part.split_once('*').unwrap_or((part, "10"));
            let id = w.blueprints.id_of(key).expect("TOURNEY_ARMY: no such unit");
            let n: i32 = n.parse().unwrap_or(10);
            for p in 0..w.state.players.len() {
                let start = w.state.players[p].start;
                for i in 0..n {
                    let at =
                        start + mc_core::FxVec2::from_ints(150 + (i % 8) * 18, -60 + (i / 8) * 18);
                    w.spawn_unit(id, p as u8, at, mc_core::Angle::ZERO, true)
                        .unwrap();
                }
            }
        }
    }
    let mut jumps = [0u32; 2];
    let mut nukes = [0u32; 2];
    let mut first_blood: Option<u32> = None;
    let mut minute = 0;
    while minute < minutes && w.state.winner.is_none() {
        minute += 1;
        for _ in 0..600 {
            w.tick(&[]).unwrap();
            for e in &w.events {
                let owner = |u: mc_sim::tables::UnitId| {
                    w.state
                        .units
                        .row(u)
                        .map(|r| w.state.players[w.state.units.owner[r] as usize].team)
                };
                match e {
                    SimEvent::WarpSpooling { unit, .. } => {
                        if let Some(t) = owner(*unit) {
                            jumps[t as usize] += 1;
                        }
                    }
                    SimEvent::UnitDied {
                        blueprint,
                        owner,
                        pos,
                        ..
                    } if deaths => {
                        let start = w.state.players[*owner as usize].start;
                        // The nearest enemy unit: likely what killed it.
                        let near = w
                            .state
                            .units
                            .slots
                            .iter()
                            .filter(|&r| {
                                w.state.players[w.state.units.owner[r] as usize].team
                                    != w.state.players[*owner as usize].team
                                    && !w.bp(r).weapons.is_empty()
                            })
                            .min_by_key(|&r| w.state.units.pos[r].distance_sq(pos.xy()))
                            .map(|r| {
                                format!(
                                    "{} at {} m",
                                    w.bp(r).key,
                                    w.state.units.pos[r].distance(pos.xy()).floor_int()
                                )
                            })
                            .unwrap_or_default();
                        println!(
                            "  died {:>4}s P{owner} {} {} m from home, nearest enemy {near}",
                            w.state.tick / 10,
                            w.blueprints.unit(*blueprint).key,
                            (pos.xy().distance(start)).floor_int()
                        );
                    }
                    SimEvent::NuclearLaunch { owner, .. } => {
                        nukes[w.state.players[*owner as usize].team as usize] += 1;
                    }
                    _ => {}
                }
            }
            if first_blood.is_none() && w.state.players.iter().any(|p| p.units_lost > 0) {
                first_blood = Some(w.state.tick / 60);
            }
            if let Some(p) = ops_trace.filter(|_| w.state.tick.is_multiple_of(100)) {
                let lines = w.state.ai[p].op_report();
                for l in &lines {
                    if !last_ops.contains(l) {
                        println!("  {:>4}s P{p} {l}", w.state.tick / 10);
                    }
                }
                last_ops = lines;
            }
            if snap == Some(w.state.tick) {
                for p in 0..w.state.players.len() {
                    let mut mass = 0;
                    let mut n = 0;
                    for r in w.state.units.slots.iter() {
                        let bp = w.bp(r);
                        if w.state.units.owner[r] as usize == p
                            && bp.is_mobile()
                            && !bp.weapons.is_empty()
                            && !bp.has(cat::COMMANDER | cat::ENGINEER)
                        {
                            mass += bp.cost_mass.floor_int();
                            n += 1;
                        }
                    }
                    println!(
                        "SNAP tick {} P{p} armed {n} units {mass} mass",
                        w.state.tick
                    );
                }
            }
            if w.state.winner.is_some() {
                break;
            }
        }
        if every > 0 && minute % every == 0 {
            println!("{minute:>3}m");
            for (p, pl) in w.state.players.iter().enumerate() {
                println!(
                    "  P{p} side {} mass {:>5}/s energy {:>6}/s eff {:>3}% build {:>3}% store {:>3}%/{:>3}% mines {} worth {:>7} kills {} lost {} | {}",
                    ["A", "B"][pl.team as usize],
                    pl.mass_income.floor_int(),
                    pl.energy_income.floor_int(),
                    (pl.upkeep_efficiency * 100).floor_int(),
                    (pl.build_speed * 100).floor_int(),
                    (pl.mass * 100 / pl.mass_capacity.max(mc_core::Fx::ONE)).floor_int(),
                    (pl.energy * 100 / pl.energy_capacity.max(mc_core::Fx::ONE)).floor_int(),
                    w.state.units.slots.iter().filter(|&r| w.state.units.owner[r] as usize == p && w.bp(r).mine.is_some() && w.state.units.is_active(r)).count(),
                    worth(&w, pl.team).floor_int(),
                    pl.units_killed,
                    pl.units_lost,
                    w.state.ai[p].summary()
                );
            }
        }
    }
    let (wa, wb) = (worth(&w, 0), worth(&w, 1));
    // No winner by the end: the side worth half again as much is ahead.
    let verdict = match w.state.winner {
        Some(0) => "A",
        Some(_) => "B",
        None if wa * 2 >= wb * 3 => "a",
        None if wb * 2 >= wa * 3 => "b",
        None => "-",
    };
    let sum = |team: u8, f: &dyn Fn(&mc_sim::tables::Player) -> u32| -> u32 {
        w.state
            .players
            .iter()
            .filter(|p| p.team == team)
            .map(f)
            .sum()
    };
    let plans = |team: u8| -> String {
        w.state
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.team == team)
            .map(|(i, _)| w.state.ai[i].summary())
            .collect::<Vec<_>>()
            .join(" || ")
    };
    println!(
        "RESULT map={} players={} seed={} A={} B={} difficulty={:?} verdict={} minute={} worthA={} worthB={} killsA={} killsB={} jumpsA={} jumpsB={} nukesA={} nukesB={} first_loss={}",
        parts[0],
        players,
        seed,
        parts.get(4).unwrap_or(&"adaptive"),
        parts.get(5).unwrap_or(&"aggressive"),
        difficulty,
        verdict,
        minute,
        wa.floor_int(),
        wb.floor_int(),
        sum(0, &|p| p.units_killed),
        sum(1, &|p| p.units_killed),
        jumps[0],
        jumps[1],
        nukes[0],
        nukes[1],
        first_blood.map_or(-1, |t| t as i64 / 10),
    );
    if std::env::var("TOURNEY_ROSTER").is_ok() {
        for p in 0..w.state.players.len() {
            let mut roster = std::collections::BTreeMap::new();
            for r in w.state.units.slots.iter() {
                if w.state.units.owner[r] as usize == p {
                    *roster.entry(w.bp(r).key.clone()).or_insert(0) += 1;
                }
            }
            println!("ROSTER P{p}: {roster:?}");
            // Where its mass stands: army, defence, economy, factories, the rest.
            let mut by: std::collections::BTreeMap<&str, i64> = std::collections::BTreeMap::new();
            for r in w.state.units.slots.iter() {
                if w.state.units.owner[r] as usize != p {
                    continue;
                }
                let bp = w.bp(r);
                let kind = if bp.has(cat::COMMANDER) {
                    "commander"
                } else if bp.is_mobile() && bp.has(cat::ENGINEER) {
                    "engineers"
                } else if bp.is_mobile() && !bp.weapons.is_empty() {
                    "army"
                } else if bp.is_mobile() {
                    "support"
                } else if bp.has(cat::FACTORY) {
                    "factories"
                } else if bp.mine.is_some() || bp.has(cat::POWER) || bp.has(cat::STORAGE) {
                    "economy"
                } else if !bp.weapons.is_empty() || bp.shield.is_some() {
                    "defence"
                } else {
                    "other"
                };
                *by.entry(kind).or_insert(0) += bp.cost_mass.floor_int() as i64;
            }
            println!("MASS P{p}: {by:?}");
        }
    }
    println!("PLANS A {}", plans(0));
    println!("PLANS B {}", plans(1));
}
