//! AI-only match on a real map, printing how much of each side's army sits
//! idle in one place and how tightly its mines are packed. The check for
//! "the AI bunches its units and never moves them" and mine clusters; run with
//! `STALL=meridian_basin:4:30 cargo test --release -p mc-sim --test zz_ai_stall_probe -- --ignored --nocapture`
//! (map, players, minutes; optional `:seed`).
use mc_core::FxVec2;
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_sim::tables::{Controller, NO_ORDER};
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[test]
#[ignore]
fn stall() {
    let spec = std::env::var("STALL").unwrap_or_else(|_| "meridian_basin:4:30".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(4);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(30);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let human = std::env::var("STALL_HUMAN").is_ok();
    let difficulty = match std::env::var("STALL_DIFF").as_deref() {
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
                // STALL_HUMAN=1: slot 0 is a player who never gives an order,
                // the AI's army then grows at home instead of trading at the front.
                controller: if i == 0 && human {
                    Controller::Human
                } else {
                    Controller::Ai
                },
                start: i,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    println!("stall {spec}");
    // Where each idle army unit stood two minutes ago.
    let mut seen: HashMap<u64, (FxVec2, u32)> = HashMap::new();
    // STALL_FORT=N (with STALL_HUMAN): the passive player's base holds behind
    // N turrets, topped up every minute, so the AI's waves keep dying against
    // it the way they do against a player who defends.
    let fort: usize = std::env::var("STALL_FORT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let gun = w.blueprints.id_of("aster_t2_point_defense").unwrap();
    for minute in 1..=minutes {
        if human && fort > 0 {
            let s = &w.state;
            let have = s
                .units
                .slots
                .iter()
                .filter(|&r| s.units.owner[r] == 0 && s.units.blueprint[r] == gun)
                .count();
            let home = s.players[0].start;
            let toward = s.players[1].start;
            let mut k = 0;
            for _ in have..fort {
                // Rings of turrets facing the enemy, the first a few hundred metres out.
                for _ in 0..40 {
                    k += 1;
                    let ring = 260 + (k / 12) * 60;
                    let a = (toward - home).angle()
                        + mc_core::Angle(((k % 12) as i32 * 65536 / 24 - 16384 + 2730) as u16);
                    let at = home + FxVec2::from_angle(a) * mc_core::Fx::from_int(ring as i32);
                    if w.can_place(w.blueprints.unit(gun), at) {
                        w.spawn_unit(gun, 0, at, mc_core::Angle::ZERO, true)
                            .unwrap();
                        break;
                    }
                }
            }
        }
        for _ in 0..600 {
            w.tick(&[]).unwrap();
        }
        let s = &w.state;
        for p in 0..s.players.len() {
            let start = s.players[p].start;
            let (mut army, mut idle, mut stuck, mut far_sum) = (0, 0, 0, 0.0f32);
            let mut mines = Vec::new();
            let mut why: std::collections::BTreeMap<String, u32> = Default::default();
            let mut at_sum = FxVec2::ZERO;
            for row in s
                .units
                .slots
                .iter()
                .filter(|&r| s.units.owner[r] as usize == p)
            {
                let bp = w.bp(row);
                let pos = s.units.pos[row];
                if bp.mine.is_some() {
                    if s.units.is_active(row) {
                        mines.push(pos);
                    }
                    continue;
                }
                // STALL_ALL=1: every ground unit, engineers and support too.
                let all = std::env::var("STALL_ALL").is_ok();
                if !s.units.is_active(row)
                    || !bp.is_mobile()
                    || (!all && (bp.weapons.is_empty() || bp.has(cat::COMMANDER | cat::ENGINEER)))
                    || bp.has(cat::AIR)
                {
                    continue;
                }
                army += 1;
                far_sum += pos.distance(start).to_f32();
                let id = s.units.id(row).0 as u64;
                idle += (s.units.order_head[row] == NO_ORDER) as u32;
                // Parked: within 60 m of one spot near home for 3 minutes, orders or not.
                let e = seen.entry(id).or_insert((pos, minute));
                if e.0.distance(pos).to_f32() > 60.0 {
                    *e = (pos, minute);
                } else if minute - e.1 >= 3 && pos.distance(start).to_f32() < 1500.0 {
                    stuck += 1;
                    if std::env::var("STALL_WHY").is_ok() {
                        let head = s.units.order_head[row];
                        let what = if head == NO_ORDER {
                            "idle".to_string()
                        } else {
                            let o = &s.orders.order[head as usize];
                            format!(
                                "{:?} to {:.0} m off",
                                o.kind,
                                (o.pos - pos).length().to_f32()
                            )
                        };
                        *why.entry(format!("{} {what}", bp.key)).or_insert(0) += 1;
                        at_sum = at_sum + pos;
                    }
                }
            }
            let mut close = 0;
            let mut nearest = f32::MAX;
            for (i, a) in mines.iter().enumerate() {
                for b in &mines[i + 1..] {
                    let d = a.distance(*b).to_f32();
                    nearest = nearest.min(d);
                    close += (d < 400.0) as u32;
                }
            }
            println!(
                "  {minute:>2}m P{p}: waves {} army {army} idle {idle} parked>=3m {stuck} mean dist from start {:.0} | mines {} pairs<400m {close} nearest {:.0}",
                s.ai[p].waves,
                if army > 0 { far_sum / army as f32 } else { 0.0 },
                mines.len(),
                nearest,
            );
            if std::env::var("STALL_POND").is_ok() {
                // Ships parked idle: how much water can they reach?
                for row in s
                    .units
                    .slots
                    .iter()
                    .filter(|&r| s.units.owner[r] as usize == p)
                {
                    let bp = w.bp(row);
                    let Some(m) = bp.motion.filter(|m| m.layer == mc_data::MoveLayer::Naval) else {
                        continue;
                    };
                    if s.units.order_head[row] != NO_ORDER {
                        continue;
                    }
                    let from = s.units.pos[row];
                    let mut seen = std::collections::HashSet::new();
                    let mut q = std::collections::VecDeque::new();
                    let c = |v: FxVec2| ((v.x.to_f32() / 8.0) as i32, (v.y.to_f32() / 8.0) as i32);
                    q.push_back(c(from));
                    seen.insert(c(from));
                    while let Some((x, y)) = q.pop_front() {
                        if seen.len() > 50000 {
                            break;
                        }
                        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            let n = (x + dx, y + dy);
                            let pos = FxVec2::from_ints(n.0 * 8 + 4, n.1 * 8 + 4);
                            if !seen.contains(&n) && w.nav.passable(m.layer, m.size_class, pos) {
                                seen.insert(n);
                                q.push_back(n);
                            }
                        }
                    }
                    println!(
                        "      pond P{p} {} at {:.0},{:.0} size {} reaches {} cells, stands {}",
                        bp.key,
                        from.x.to_f32(),
                        from.y.to_f32(),
                        m.size_class,
                        seen.len(),
                        w.nav.passable(m.layer, m.size_class, from)
                    );
                    break;
                }
            }
            if std::env::var("STALL_WHY").is_ok() && s.players[p].controller == Controller::Ai {
                println!("      ai P{p}: {}", s.ai[p].summary());
            }
            if !why.is_empty() {
                let n: u32 = why.values().sum();
                let at = FxVec2::new(at_sum.x / n as i32, at_sum.y / n as i32);
                println!("      parked P{p} around {:.0},{:.0} ({:.0} m from start {:.0},{:.0}): {why:?}",
                    at.x.to_f32(), at.y.to_f32(), at.distance(start).to_f32(), start.x.to_f32(), start.y.to_f32());
            }
        }
        if let Some(winner) = w.state.winner {
            println!("  winner P{winner} at {minute}m");
            return;
        }
    }
}
