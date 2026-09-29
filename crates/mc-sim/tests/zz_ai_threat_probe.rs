//! How much of a threat the AI is: an AI against a player who never gives an
//! order (a commander standing at its start), printing per minute the AI's
//! army, economy, spending and how close its army is to the enemy start, then
//! the minute it wins. A real opponent finishes an idle commander early; run with
//! `THREAT=meridian_basin:hard:20 cargo test --release -p mc-sim --test sim -- zz_ai_threat_probe:: --ignored --nocapture`
//! (map, difficulty, minutes; optional `:seed` and `:swap` to trade starts).
//! `THREAT_ROSTER=1` lists the AI's finished units by key at the end;
//! `THREAT_AI=1` makes slot 0 an AI too (a duel with the same readout).
//! `THREAT_ECON=1` adds each side's economy: mines and plants by tier (sites
//! in brackets), energy stored, and the seconds of the minute out of energy.
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

fn difficulty(s: &str) -> Difficulty {
    match s {
        "easy" => Difficulty::Easy,
        "hard" => Difficulty::Hard,
        _ => Difficulty::Normal,
    }
}

/// One side's army and spending at this minute.
fn line(w: &World, p: usize, enemy_start: mc_core::FxVec2) -> String {
    let s = &w.state;
    let pl = &s.players[p];
    let (mut army, mut army_mass, mut near, mut factories, mut eng) = (0, 0.0f32, 0, 0, 0);
    let mut max_unit_tech = 0;
    for row in s
        .units
        .slots
        .iter()
        .filter(|&r| s.units.owner[r] as usize == p && s.units.is_active(r))
    {
        let bp = w.bp(row);
        if bp.has(cat::FACTORY) && bp.is_structure() {
            factories += 1;
        } else if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
            eng += 1;
        } else if bp.is_mobile() && !bp.weapons.is_empty() && !bp.has(cat::COMMANDER) {
            army += 1;
            army_mass += bp.cost_mass.to_f32();
            max_unit_tech = max_unit_tech.max(bp.tech);
            near += (s.units.pos[row].distance(enemy_start).to_f32() < 1200.0) as u32;
        }
    }
    format!(
        "army {army:>3} ({army_mass:>6.0} m, best T{max_unit_tech}) near enemy {near:>3} | mass {:>5.1}/s stored {:>5.0}/{:<5.0} spent {:>5.1}/s energy {:>5.0}/{:<5.0} eff {:.2} | factories {factories} eng {eng} tech {} | kills {} lost {} | {}",
        pl.mass_income.to_f32(),
        pl.mass.to_f32(),
        pl.mass_capacity.to_f32(),
        pl.mass_demand.to_f32(),
        pl.energy_income.to_f32(),
        pl.energy_demand.to_f32(),
        pl.efficiency.to_f32(),
        w.side_tech(p as u8),
        pl.units_killed,
        pl.units_lost,
        s.ai[p].summary(),
    )
}

/// One side's economy: mines and power plants by tier (sites going up in
/// brackets), energy in store, and how much of the last minute it ran out of energy.
fn econ(w: &World, p: usize, stalled: u32, mine_power: f32) -> String {
    let s = &w.state;
    let pl = &s.players[p];
    let (mut mines, mut mine_sites, mut plants, mut plant_sites) = ([0; 5], 0, [0; 5], 0);
    for row in s
        .units
        .slots
        .iter()
        .filter(|&r| s.units.owner[r] as usize == p)
    {
        let bp = w.bp(row);
        let tier = (bp.tech as usize).min(4);
        let live = s.units.is_active(row);
        if bp.has(cat::EXTRACTOR) && bp.mine.is_some() {
            if live {
                mines[tier] += 1;
            } else {
                mine_sites += 1;
            }
        } else if bp.has(cat::POWER) && bp.is_structure() {
            if live {
                plants[tier] += 1;
            } else {
                plant_sites += 1;
            }
        }
    }
    format!(
        "mines T1-4 {:?} (+{mine_sites}) plants T1-3 {:?} (+{plant_sites}) | energy {:>6.0}/{:<6.0} energy stall {:>2}s mine power {:.2} reclaim {:.1}/s",
        &mines[1..],
        &plants[1..4],
        pl.energy.to_f32(),
        pl.energy_capacity.to_f32(),
        stalled / 10,
        mine_power,
        pl.reclaim_income.to_f32(),
    )
}

/// Where one side's army is and what it is doing: by distance from its own
/// start, idle or by order kind.
fn where_army(w: &World, p: usize) -> String {
    let s = &w.state;
    let start = s.players[p].start;
    let mut bins: BTreeMap<String, u32> = BTreeMap::new();
    for row in s
        .units
        .slots
        .iter()
        .filter(|&r| s.units.owner[r] as usize == p && s.units.is_active(r))
    {
        let bp = w.bp(row);
        if !bp.is_mobile()
            || bp.weapons.is_empty()
            || bp.categories & (cat::COMMANDER | cat::ENGINEER) != 0
        {
            continue;
        }
        let d = s.units.pos[row].distance(start).to_f32();
        let band = match d as i32 {
            0..=699 => "home",
            700..=1999 => "near",
            _ => "far",
        };
        let head = s.units.order_head[row];
        let what = if head == mc_sim::tables::NO_ORDER {
            "idle".to_string()
        } else {
            format!("{:?}", s.orders.order[head as usize].kind)
        };
        let dom = if bp.has(cat::AIR) { "air" } else { "gnd" };
        *bins.entry(format!("{dom} {band} {what}")).or_insert(0) += 1;
    }
    // Where the ground army's orders point: each target (rounded to 50 m)
    // with how many units head there and how far off they are on average.
    let mut targets: BTreeMap<(i32, i32), (u32, f32)> = BTreeMap::new();
    for row in s
        .units
        .slots
        .iter()
        .filter(|&r| s.units.owner[r] as usize == p && s.units.is_active(r))
    {
        let bp = w.bp(row);
        if !bp.is_mobile()
            || bp.weapons.is_empty()
            || bp.categories & (cat::COMMANDER | cat::ENGINEER | cat::AIR) != 0
        {
            continue;
        }
        let head = s.units.order_head[row];
        if head == mc_sim::tables::NO_ORDER {
            continue;
        }
        let o = &s.orders.order[head as usize];
        let key = (
            (o.pos.x.to_f32() / 50.0) as i32 * 50,
            (o.pos.y.to_f32() / 50.0) as i32 * 50,
        );
        let e = targets.entry(key).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += (o.pos - s.units.pos[row]).length().to_f32();
    }
    let mut t: Vec<_> = targets.into_iter().collect();
    t.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    let t: Vec<String> = t
        .iter()
        .take(4)
        .map(|((x, y), (n, d))| {
            let at = mc_core::FxVec2::from_ints(*x, *y);
            format!(
                "{n}u -> {x},{y} ({:.0} m from home, {:.0} m from enemy, units {:.0} m off)",
                at.distance(start).to_f32(),
                s.players
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != p)
                    .map(|(_, q)| at.distance(q.start).to_f32())
                    .fold(f32::MAX, f32::min),
                d / *n as f32
            )
        })
        .collect();
    format!("{bins:?}\n        targets: {t:?}")
}

#[test]
#[ignore]
fn threat() {
    let spec = std::env::var("THREAT").unwrap_or_else(|_| "meridian_basin:hard:20".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let diff = difficulty(parts.get(1).copied().unwrap_or("hard"));
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(20);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let swap = parts.get(4) == Some(&"swap");
    let duel = std::env::var("THREAT_AI").is_ok();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let player = |i: u8| PlayerSetup {
        name: format!("P{i}"),
        faction: "Aster".into(),
        ai: AiConfig {
            difficulty: diff,
            ..AiConfig::default()
        },
        team: i,
        controller: if i == 0 && !duel {
            Controller::Human
        } else {
            Controller::Ai
        },
        start: if swap { 1 - i } else { i },
    };
    let config = MatchConfig {
        seed,
        players: vec![player(0), player(1)],
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    println!("threat {spec}{}", if duel { " (AI duel)" } else { "" });
    let mut first_near: Option<u32> = None;
    let opening: u32 = std::env::var("THREAT_OPENING")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let mut known = std::collections::HashSet::new();
    let show_econ = std::env::var("THREAT_ECON").is_ok();
    for minute in 1..=minutes {
        let mut stalled = [0u32; 2];
        let mut mine_power = [0.0f32; 2];
        for _ in 0..600 {
            w.tick(&[]).unwrap();
            for p in 0..2 {
                let pl = &w.state.players[p];
                // Out of energy with more asked than comes in (`shields_unpowered`).
                stalled[p] +=
                    (pl.energy.to_f32() < 1.0 && pl.energy_demand > pl.energy_income) as u32;
                mine_power[p] += pl.mine_power.to_f32() / 600.0;
            }
            if w.state.winner.is_some() {
                break;
            }
            // THREAT_OPENING=N: every structure P1 starts in the first N minutes,
            // with the economy at that moment.
            if opening >= minute {
                let s = &w.state;
                for r in s.units.slots.iter() {
                    let id = s.units.id(r).0;
                    if s.units.owner[r] == 1 && w.bp(r).is_structure() && known.insert(id) {
                        let pl = &s.players[1];
                        println!(
                            "    {:>4}s start {:<28} mass {:>4.0} ({:>4.1}/s) energy {:>5.0} ({:>4.0}/{:<4.0}) eff {:.2}",
                            s.tick / 10,
                            w.bp(r).key,
                            pl.mass.to_f32(),
                            pl.mass_income.to_f32(),
                            pl.energy.to_f32(),
                            pl.energy_income.to_f32(),
                            pl.energy_demand.to_f32(),
                            pl.efficiency.to_f32()
                        );
                    }
                }
            }
        }
        let s = &w.state;
        let (s0, s1) = (s.players[0].start, s.players[1].start);
        let near = s.units.slots.iter().any(|r| {
            s.units.owner[r] == 1
                && s.units.is_active(r)
                && !w.bp(r).weapons.is_empty()
                && w.bp(r).is_mobile()
                && s.units.pos[r].distance(s0).to_f32() < 1200.0
        });
        if near && first_near.is_none() {
            first_near = Some(minute);
        }
        println!(
            "  {minute:>2}m P1 {} | nukes ordered {}",
            line(&w, 1, s0),
            s.strategic.orders
        );
        if show_econ {
            println!("      econ P1: {}", econ(&w, 1, stalled[1], mine_power[1]));
        }
        if std::env::var("THREAT_WHERE").is_ok() {
            println!("      where P1: {}", where_army(&w, 1));
        }
        if duel {
            println!("      P0 {}", line(&w, 0, s1));
            if show_econ {
                println!("      econ P0: {}", econ(&w, 0, stalled[0], mine_power[0]));
            }
            if std::env::var("THREAT_WHERE").is_ok() {
                println!("      where P0: {}", where_army(&w, 0));
            }
        }
        if w.state.winner.is_some() {
            break;
        }
    }
    if std::env::var("THREAT_ROSTER").is_ok() {
        for p in 0..w.state.players.len() {
            let mut roster: BTreeMap<String, u32> = BTreeMap::new();
            let s = &w.state;
            for r in s.units.slots.iter() {
                if s.units.owner[r] as usize == p && s.units.is_active(r) {
                    let bp = w.bp(r);
                    *roster
                        .entry(format!("T{} {}", bp.tech, bp.key))
                        .or_insert(0) += 1;
                }
            }
            println!("  roster P{p}: {roster:?}");
        }
    }
    println!(
        "  RESULT {spec}: first army within 1.2 km of the enemy start {} | winner {}",
        first_near.map_or("never".into(), |m| format!("{m}m")),
        w.state
            .winner
            .map_or("none".into(), |p| format!("P{p} at tick {}", w.state.tick)),
    );
}
