//! AI-only match on a real map, measuring whether the AI attacks in groups or
//! in a stream. Two numbers per side and domain (land, air, sea):
//! - sends: the size of every AI order that sends units towards the enemy
//!   (target past the middle of the map), as a count of orders of 1-2, 3-5
//!   and 6+ units;
//! - alone: of the unit-samples on the enemy's half, the share with fewer than
//!   three friends of the same domain within 250 m (sampled every 5 s).
//!
//! `STREAM=meridian_basin:2:30 cargo test --release -p mc-sim --test zz_ai_stream_probe -- --ignored --nocapture`
//! (map, players, minutes; optional `:seed`; `STREAM_DIFF=easy|normal|hard`).
use mc_core::FxVec2;
use mc_data::{cat, Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_sim::command::Command;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

const DOMAINS: [&str; 3] = ["land", "air", "sea"];

#[derive(Default, Clone, Copy)]
struct Tally {
    sends: [u32; 3],
    units_sent: u32,
    samples: u32,
    alone: u32,
}

fn domain(w: &World, row: usize) -> usize {
    match w.bp(row).motion.map(|m| m.layer) {
        Some(MoveLayer::Air) => 1,
        Some(MoveLayer::Naval) => 2,
        _ => 0,
    }
}

#[test]
#[ignore]
fn stream() {
    let spec = std::env::var("STREAM").unwrap_or_else(|_| "meridian_basin:2:30".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(2);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(30);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let difficulty = match std::env::var("STREAM_DIFF").as_deref() {
        Ok("easy") => Difficulty::Easy,
        Ok("hard") => Difficulty::Hard,
        _ => Difficulty::Normal,
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let config = MatchConfig {
        seed,
        players: (0..players)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                ai: AiConfig {
                    difficulty,
                    ..AiConfig::default()
                },
                team: i,
                controller: Controller::Ai,
                start: i,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    let n = players as usize;
    let mut tally = vec![[Tally::default(); 3]; n];
    // Nearest enemy start per player: "towards the enemy" is past the middle.
    let starts: Vec<FxVec2> = w.state.players.iter().map(|p| p.start).collect();
    let enemy_side = |p: usize, at: FxVec2| {
        let mine = at.distance(starts[p]);
        (0..n)
            .filter(|&q| q != p)
            .any(|q| at.distance(starts[q]) < mine)
    };
    println!("stream {spec} {difficulty:?}");
    for minute in 1..=minutes {
        for t in 0..600 {
            w.tick(&[]).unwrap();
            for pc in &w.state.ai_pending {
                let (units, target) = match &pc.command {
                    Command::Move { units, target, .. }
                    | Command::AttackMove { units, target, .. } => (units, *target),
                    _ => continue,
                };
                let p = pc.player as usize;
                if !enemy_side(p, target) {
                    continue;
                }
                let mut by = [0u32; 3];
                for id in units {
                    if let Some(row) = w.state.units.row(*id) {
                        if !w.bp(row).weapons.is_empty()
                            && w.bp(row).is_mobile()
                            && w.bp(row).builder.is_none()
                            && !w.bp(row).has(cat::SCOUT)
                        {
                            by[domain(&w, row)] += 1;
                        }
                    }
                }
                for d in 0..3 {
                    if by[d] > 0 {
                        let b = match by[d] {
                            1..=2 => 0,
                            3..=5 => 1,
                            _ => 2,
                        };
                        tally[p][d].sends[b] += 1;
                        tally[p][d].units_sent += by[d];
                    }
                }
            }
            if t % 50 == 0 {
                let s = &w.state;
                let army: Vec<(usize, usize, FxVec2)> = s
                    .units
                    .slots
                    .iter()
                    .filter(|&r| {
                        let bp = w.bp(r);
                        s.units.is_active(r)
                            && bp.is_mobile()
                            && !bp.weapons.is_empty()
                            && bp.builder.is_none()
                            && !bp.has(cat::SCOUT)
                    })
                    .map(|r| (s.units.owner[r] as usize, domain(&w, r), s.units.pos[r]))
                    .collect();
                for &(p, d, pos) in &army {
                    if p >= n || !enemy_side(p, pos) {
                        continue;
                    }
                    let friends = army
                        .iter()
                        .filter(|&&(q, e, at)| {
                            q == p && e == d && at != pos && at.distance(pos).to_f32() < 250.0
                        })
                        .count();
                    tally[p][d].samples += 1;
                    tally[p][d].alone += (friends < 3) as u32;
                }
            }
        }
        if minute % 5 == 0 || minute == minutes || w.state.winner.is_some() {
            for p in 0..n {
                let mut alive = [0; 3];
                for r in w.state.units.slots.iter() {
                    let bp = w.bp(r);
                    if w.state.units.owner[r] as usize == p
                        && bp.is_mobile()
                        && !bp.weapons.is_empty()
                        && bp.builder.is_none()
                    {
                        alive[domain(&w, r)] += 1;
                    }
                }
                let mut line = format!(
                    "  {minute:>2}m P{p} waves {:>2} raids {:>3} alive {alive:?} |",
                    w.state.ai[p].waves, w.state.ai[p].raids
                );
                for d in 0..3 {
                    let t = tally[p][d];
                    let alone = if t.samples > 0 {
                        100.0 * t.alone as f32 / t.samples as f32
                    } else {
                        0.0
                    };
                    line += &format!(
                        " {}: sends 1-2/3-5/6+ {}/{}/{} ({} units) alone {alone:.0}% of {} |",
                        DOMAINS[d], t.sends[0], t.sends[1], t.sends[2], t.units_sent, t.samples
                    );
                }
                println!("{line}");
            }
        }
        if let Some(winner) = w.state.winner {
            println!("  winner P{winner} at {minute}m");
            return;
        }
    }
}
