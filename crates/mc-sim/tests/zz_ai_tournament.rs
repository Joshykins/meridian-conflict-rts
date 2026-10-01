//! One tournament match between two AI sides on a real map (`docs/AI_COMMANDER.md`,
//! "Tournament"): side A's brain and doctrine against side B's, the same difficulty.
//! `scripts/ai-tournament.sh` plays many of these in parallel and adds them up.
//!
//! `TOURNEY=map:players:minutes:seed:A:B[:difficulty]`, where A and B are
//! `brain/doctrine` (brain `commander` or `classic`; doctrine `adaptive`,
//! `aggressive`, `economic` or `defensive`). With 2 players slot 0 is side A; with
//! more, the starts west of the middle are side A. `TOURNEY_EVERY=N` prints each
//! side's state every N minutes, `TOURNEY_ROSTER=1` each player's units at the end,
//! `TOURNEY_DEATHS=1` every unit that dies, where and when.
//! The `RESULT key=value ...` line sums the match up.
//!
//! `TOURNEY=serac_divide:2:40:7:commander/adaptive:classic/adaptive cargo test --profile gate -p mc-sim --test sim -- zz_ai_tournament:: --ignored --nocapture`
use mc_core::Fx;
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Brain, Difficulty, Doctrine, MatchConfig, PlayerSetup, SimEvent, World};
use std::path::Path;
use std::sync::Arc;

fn side(spec: &str) -> (Brain, Doctrine) {
    let (b, d) = spec.split_once('/').unwrap_or((spec, "adaptive"));
    let brain = match b {
        "classic" => Brain::Classic,
        _ => Brain::Commander,
    };
    let doctrine = match d {
        "aggressive" => Doctrine::Aggressive,
        "economic" => Doctrine::Economic,
        "defensive" => Doctrine::Defensive,
        _ => Doctrine::Adaptive,
    };
    (brain, doctrine)
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
        .unwrap_or_else(|_| "serac_divide:2:30:7:commander/adaptive:classic/adaptive".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(2);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(30);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let a = side(parts.get(4).copied().unwrap_or("commander/adaptive"));
    let b = side(parts.get(5).copied().unwrap_or("classic/adaptive"));
    let difficulty = match parts.get(6).copied() {
        Some("easy") => Difficulty::Easy,
        Some("normal") => Difficulty::Normal,
        _ => Difficulty::Hard,
    };
    let deaths = std::env::var("TOURNEY_DEATHS").is_ok();
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
                let (brain, doctrine) = if team_of(i) == 0 { a } else { b };
                PlayerSetup {
                    name: format!("AI {i}"),
                    faction: "Aster".into(),
                    ai: AiConfig {
                        brain,
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
                        println!(
                            "  died {:>4}s P{owner} {} {} m from home",
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
            if w.state.winner.is_some() {
                break;
            }
        }
        if every > 0 && minute % every == 0 {
            println!("{minute:>3}m");
            for (p, pl) in w.state.players.iter().enumerate() {
                println!(
                    "  P{p} side {} mass {:>5}/s worth {:>7} kills {} lost {} | {}",
                    ["A", "B"][pl.team as usize],
                    pl.mass_income.floor_int(),
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
        parts.get(4).unwrap_or(&"commander"),
        parts.get(5).unwrap_or(&"classic"),
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
        }
    }
    println!("PLANS A {}", plans(0));
    println!("PLANS B {}", plans(1));
}
