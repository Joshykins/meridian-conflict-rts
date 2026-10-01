//! How an all-AI team match uses each domain: per side and every few minutes,
//! its mines on land and at sea, its factories by domain, and its army by
//! domain (land, hover, naval, air, space) with how much of each is at home,
//! out past it, or parked (in one place for 3+ minutes). Starts are split into
//! two teams by the map's middle (west against east). The check for "the AI
//! is ineffective on island maps"; run with
//! `DOMAIN=the_axis:8:40 cargo test --profile gate -p mc-sim --test sim -- zz_ai_domain_probe:: --ignored --nocapture`
//! (map, players, minutes; optional `:seed` and `:difficulty`). `DOMAIN_EVERY=N`
//! prints every N minutes (default 5); `DOMAIN_ROSTER=1` lists each side's
//! finished units by key at the end.
use mc_core::FxVec2;
use mc_data::{cat, Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

const DOMAINS: [&str; 5] = ["land", "hover", "naval", "air", "space"];

fn domain(bp: &mc_data::UnitBlueprint) -> usize {
    if bp.has(cat::SPACE) {
        return 4;
    }
    match bp.motion.map(|m| m.layer) {
        Some(MoveLayer::Hover) => 1,
        Some(MoveLayer::Naval) => 2,
        Some(MoveLayer::Air) => 3,
        _ => 0,
    }
}

#[test]
#[ignore]
fn domains() {
    let spec = std::env::var("DOMAIN").unwrap_or_else(|_| "the_axis:8:40".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(8);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(40);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let difficulty = match parts.get(4).copied() {
        Some("easy") => Difficulty::Easy,
        Some("normal") => Difficulty::Normal,
        _ => Difficulty::Hard,
    };
    let every: u32 = std::env::var("DOMAIN_EVERY")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(5);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let starts = map.start_positions().to_vec();
    // The middle of the starts in use: west of it is one team, east the other.
    let mid = starts[..players as usize]
        .iter()
        .fold(mc_core::Fx::ZERO, |a, s| a + s.x)
        / players as i32;
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
                team: (starts[i as usize].x > mid) as u8,
                controller: Controller::Ai,
                start: i,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    println!("domain {spec}");
    let mut seen: HashMap<u32, (FxVec2, u32)> = HashMap::new();
    for minute in 1..=minutes {
        for _ in 0..600 {
            w.tick(&[]).unwrap();
            if w.state.winner.is_some() {
                break;
            }
        }
        let s = &w.state;
        // Army units standing in one place (within 60 m) for three minutes or more.
        let mut parked = vec![[0u32; 5]; players as usize];
        // DOMAIN_WHY=1: what the parked units are doing, per side and domain.
        let mut why: BTreeMap<(usize, &str, String), u32> = BTreeMap::new();
        for r in s.units.slots.iter() {
            let bp = w.bp(r);
            if !s.units.is_active(r)
                || !bp.is_mobile()
                || bp.weapons.is_empty()
                || bp.categories & (cat::COMMANDER | cat::ENGINEER) != 0
            {
                continue;
            }
            let id = s.units.id(r).0;
            let pos = s.units.pos[r];
            let e = seen.entry(id).or_insert((pos, minute));
            if e.0.distance(pos).to_f32() > 60.0 {
                *e = (pos, minute);
            } else if minute - e.1 >= 3 {
                let p = s.units.owner[r] as usize;
                parked[p][domain(bp)] += 1;
                let head = s.units.order_head[r];
                let what = if head == mc_sim::tables::NO_ORDER {
                    "idle".to_string()
                } else {
                    let o = &s.orders.order[head as usize];
                    format!(
                        "{:?} {:.0} m off",
                        o.kind,
                        (o.pos - pos).length().to_f32() / 100.0 * 100.0
                    )
                };
                let home = (pos.distance(s.players[p].start).to_f32() / 500.0) as i32 * 500;
                let what = format!(
                    "{what} {} hp {:.0}%",
                    bp.key,
                    (s.units.health[r] / bp.health).to_f32() * 100.0
                );
                *why.entry((
                    p,
                    DOMAINS[domain(bp)],
                    format!("{what}, {home} m from start"),
                ))
                .or_insert(0) += 1;
            }
        }
        let done = w.state.winner.is_some();
        if minute % every != 0 && !done && minute != minutes {
            continue;
        }
        println!("{minute:>2}m");
        if std::env::var("DOMAIN_WHY").is_ok() {
            for ((p, d, what), n) in &why {
                println!("    why P{p} {d}: {n} {what}");
            }
        }
        for p in 0..players as usize {
            let pl = &s.players[p];
            let start = pl.start;
            let (mut land_mines, mut sea_mines, mut mine_sites) = (0, 0, 0);
            let mut factories = [0; 5];
            let mut army = [[0u32; 3]; 5];
            let mut army_mass = [0f32; 5];
            for r in s
                .units
                .slots
                .iter()
                .filter(|&r| s.units.owner[r] as usize == p)
            {
                let bp = w.bp(r);
                let live = s.units.is_active(r);
                let pos = s.units.pos[r];
                if bp.mine.is_some() {
                    if !live {
                        mine_sites += 1;
                    } else if w.ore.at_sea(pos) {
                        sea_mines += 1;
                    } else {
                        land_mines += 1;
                    }
                } else if bp.has(cat::FACTORY) && bp.is_structure() && live {
                    let d = bp
                        .builder
                        .as_ref()
                        .and_then(|b| {
                            b.builds
                                .iter()
                                .map(|&id| w.blueprints.unit(id))
                                .find(|u| u.is_mobile() && !u.has(cat::ENGINEER))
                        })
                        .map_or(0, domain);
                    factories[d] += 1;
                } else if live
                    && bp.is_mobile()
                    && !bp.weapons.is_empty()
                    && bp.categories & (cat::COMMANDER | cat::ENGINEER) == 0
                {
                    let d = domain(bp);
                    let far = pos.distance(start).to_f32();
                    let enemy_half = (pos.x > mid) != (start.x > mid);
                    let band = if enemy_half {
                        2
                    } else if far > 1500.0 {
                        1
                    } else {
                        0
                    };
                    army[d][band] += 1;
                    army_mass[d] += bp.cost_mass.to_f32();
                }
            }
            let armies: Vec<String> = (0..5)
                .filter(|&d| army[d].iter().sum::<u32>() > 0)
                .map(|d| {
                    format!(
                        "{} {}/{}/{} ({:.0}k, parked {})",
                        DOMAINS[d],
                        army[d][0],
                        army[d][1],
                        army[d][2],
                        army_mass[d] / 1000.0,
                        parked[p][d]
                    )
                })
                .collect();
            println!(
                "  P{p} t{} mines land {land_mines} sea {sea_mines} (+{mine_sites}) mass {:>5.1}/s tech {} | factories L/H/N/A {:?} | army home/out/enemy half: {} | kills {} lost {}{}",
                (starts[p].x > mid) as u8,
                pl.mass_income.to_f32(),
                w.side_tech(p as u8),
                &factories[..4],
                armies.join(", "),
                pl.units_killed,
                pl.units_lost,
                if minute == every {
                    format!(" | {}", s.ai[p].summary())
                } else {
                    String::new()
                },
            );
        }
        if done {
            break;
        }
    }
    if std::env::var("DOMAIN_ROSTER").is_ok() {
        for p in 0..players as usize {
            let mut roster: BTreeMap<String, u32> = BTreeMap::new();
            let s = &w.state;
            for r in s.units.slots.iter() {
                if s.units.owner[r] as usize == p && s.units.is_active(r) {
                    let bp = w.bp(r);
                    *roster.entry(bp.key.clone()).or_insert(0) += 1;
                }
            }
            println!("  roster P{p}: {roster:?}");
        }
    }
    let alive: Vec<usize> = (0..players as usize)
        .filter(|&p| !w.state.players[p].defeated)
        .collect();
    println!(
        "  RESULT {spec}: winner {} | alive {alive:?}",
        w.state
            .winner
            .map_or("none".into(), |p| format!("P{p} at tick {}", w.state.tick)),
    );
}
