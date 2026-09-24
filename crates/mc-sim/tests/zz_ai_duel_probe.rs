//! AI against AI on a real map, printing each side's economy over time.
//! The check for AI difficulty and economy changes; run with
//! `DUEL=dev16:easy:hard:30 cargo test --release -p mc-sim --test zz_ai_duel_probe -- --ignored --nocapture`
//! (map, player 0's difficulty, player 1's, minutes; optional `:seed` and `:swap` to trade starts).
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

fn difficulty(s: &str) -> Difficulty {
    match s {
        "easy" => Difficulty::Easy,
        "hard" => Difficulty::Hard,
        _ => Difficulty::Normal,
    }
}

fn report(w: &World, minute: u32) {
    let s = &w.state;
    for p in 0..s.players.len() {
        let pl = &s.players[p];
        let (mut mines, mut eff, mut mine_tiers) = (0, 0.0f32, [0; 5]);
        let (mut factories, mut army, mut army_mass, mut engineers) = (0, 0, 0.0f32, 0);
        let (mut power, mut builders, mut busy) = ([0; 4], [0; 4], 0);
        let (mut pd, mut fac_tiers) = (0, [0; 4]);
        for row in s
            .units
            .slots
            .iter()
            .filter(|&r| s.units.owner[r] as usize == p)
        {
            let bp = w.bp(row);
            if !s.units.is_active(row) {
                continue;
            }
            if let Some(m) = bp.mine {
                mines += 1;
                mine_tiers[bp.tech.min(4) as usize] += 1;
                if let Some(st) = s.mines.by_unit.get(&s.units.id(row)) {
                    eff += st.land.efficiency(&m).to_f32();
                }
            } else if bp.has(cat::POWER) && bp.is_structure() {
                power[bp.tech.min(3) as usize] += 1;
            } else if bp.has(cat::FACTORY) && bp.is_structure() {
                factories += 1;
                fac_tiers[bp.tech.min(3) as usize] += 1;
            } else if bp.has(cat::DEFENSE) && bp.is_structure() {
                pd += 1;
            } else if bp.has(cat::ENGINEER) || bp.has(cat::COMMANDER) {
                engineers += 1;
                builders[bp.tech.min(3) as usize] += 1;
                busy += (s.units.order_head[row] != mc_sim::tables::NO_ORDER) as usize;
            } else if bp.is_mobile() && !bp.weapons.is_empty() && !bp.has(cat::COMMANDER) {
                army += 1;
                army_mass += bp.cost_mass.to_f32();
            }
        }
        if std::env::var("DUEL_WHY").is_ok() {
            let count = |c: u32| {
                s.units
                    .slots
                    .iter()
                    .filter(|&r| {
                        s.units.owner[r] as usize == p
                            && s.units.is_active(r)
                            && w.bp(r).has(c)
                            && w.bp(r).is_structure()
                    })
                    .count()
            };
            let upgrading = s
                .units
                .slots
                .iter()
                .filter(|&r| {
                    s.units.owner[r] as usize == p && w.bp(r).mine.is_some() && {
                        let h = s.units.order_head[r];
                        h != mc_sim::tables::NO_ORDER
                            && matches!(
                                s.orders.order[h as usize].kind,
                                mc_sim::tables::OrderKind::Upgrade
                            )
                    }
                })
                .count();
            println!(
                "      why P{p}: flow eff {:.2} mass {:.0}/{:.0} energy {:.0}/{:.0} in {:.0} out {:.0} | pd {} radar {} power {} | mines upgrading {upgrading}",
                pl.efficiency.to_f32(), pl.mass.to_f32(), pl.mass_capacity.to_f32(), pl.energy.to_f32(), pl.energy_capacity.to_f32(),
                pl.energy_income.to_f32(), pl.energy_demand.to_f32(),
                count(cat::DEFENSE | cat::DIRECT_FIRE), count(cat::INTEL), count(cat::POWER),
            );
        }
        if std::env::var("DUEL_JOBS").is_ok() {
            for row in s
                .units
                .slots
                .iter()
                .filter(|&r| s.units.owner[r] as usize == p)
            {
                let bp = w.bp(row);
                if !(bp.has(cat::ENGINEER) || bp.has(cat::COMMANDER)) || !s.units.is_active(row) {
                    continue;
                }
                let head = s.units.order_head[row];
                let what = if head == mc_sim::tables::NO_ORDER {
                    "idle".to_string()
                } else {
                    let o = &s.orders.order[head as usize];
                    let target = s
                        .units
                        .row(o.target)
                        .map(|t| w.bp(t).key.clone())
                        .unwrap_or_default();
                    let dist = (o.pos - s.units.pos[row]).length().to_f32();
                    format!(
                        "{:?} {} {target} ({dist:.0} m away)",
                        o.kind,
                        w.blueprints.unit(o.blueprint).key
                    )
                };
                println!("      {} T{}: {what}", bp.key, bp.tech);
            }
        }
        println!(
            "  {minute:>2}m P{p} {:?}: stored {:.0}/{:.0} demand {:.0}/s defenses {pd} factories T1-3 {:?} | mass {:>6.1}/s energy {:>7.1}/s eff {:.2} tech {} | mines {mines} (T1-4 {:?}) avg eff {:.2} | power T1-3 {:?} builders T1-3 {:?} busy {busy} | factories {factories} engineers {engineers} army {army} ({:.0} mass) | killed {} lost {}",
            s.ai[p].config.difficulty,
            pl.mass.to_f32(),
            pl.mass_capacity.to_f32(),
            pl.mass_demand.to_f32(),
            &fac_tiers[1..],
            pl.mass_income.to_f32(),
            pl.energy_income.to_f32(),
            pl.efficiency.to_f32(),
            w.side_tech(p as u8),
            &mine_tiers[1..],
            if mines > 0 { eff / mines as f32 } else { 0.0 },
            &power[1..],
            &builders[1..],
            army_mass,
            pl.units_killed,
            pl.units_lost,
        );
    }
}

#[test]
#[ignore]
fn duel() {
    let spec = std::env::var("DUEL").unwrap_or_else(|_| "dev16:normal:normal:20".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let map_name = parts[0];
    let (a, b) = (difficulty(parts[1]), difficulty(parts[2]));
    let minutes: u32 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(20);
    let seed: u64 = parts.get(4).and_then(|m| m.parse().ok()).unwrap_or(7);
    let swap = parts.get(5) == Some(&"swap");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{map_name}.mcmap"))).unwrap();
    let player = |i: u8, d| PlayerSetup {
        name: format!("AI {i}"),
        faction: "Aster".into(),
        ai: AiConfig {
            difficulty: d,
            ..AiConfig::default()
        },
        team: i,
        controller: Controller::Ai,
        start: if swap { 1 - i } else { i },
    };
    let config = MatchConfig {
        seed,
        players: vec![player(0, a), player(1, b)],
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    println!("duel {spec}");
    for minute in 1..=minutes {
        for _ in 0..600 {
            w.tick(&[]).unwrap();
            if w.state.winner.is_some() {
                break;
            }
        }
        if minute % 3 == 0
            || w.state.winner.is_some()
            || minute == minutes
            || std::env::var("DUEL_EVERY").is_ok()
        {
            report(&w, minute);
        }
        if let Some(winner) = w.state.winner {
            println!("  winner P{winner} at {minute}m");
            return;
        }
    }
    println!("  no winner after {minutes}m");
}
