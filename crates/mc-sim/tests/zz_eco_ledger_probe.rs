//! The economy ledger: where a side's materials go over a whole AI duel, and whether
//! it has the build power to spend them. Every `LEDGER_EVERY` minutes (default 3) each
//! side prints its income (mines, reclaim, the rest), what it spent, its store and the
//! materials lost to a full store, its build power (factories, engineers, commander:
//! owned and busy), its energy by source (reactors by tier, the rest) and the value it
//! has standing by kind. The last line of each side is the whole game in one row.
//!
//! `LEDGER=serac_divide:40[:seed[:swap]] cargo test --profile gate -p mc-sim --test sim
//! -- zz_eco_ledger_probe:: --ignored --nocapture`
//!
//! What it measures: "the store sits full" (share of minutes past 10 with the store over
//! 80%) and "build power runs out" (busy build power over owned) are the two signs that
//! income has outgrown the build power to spend it.
use mc_data::cat;
use mc_jobs::Pool;
use mc_sim::tables::{flag, Controller};
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

/// Build power by kind: owned and busy this tick.
#[derive(Default, Clone, Copy)]
struct Power {
    factory: [f32; 2],
    engineer: [f32; 2],
    commander: [f32; 2],
}

impl Power {
    fn owned(&self) -> f32 {
        self.factory[0] + self.engineer[0] + self.commander[0]
    }
    fn busy(&self) -> f32 {
        self.factory[1] + self.engineer[1] + self.commander[1]
    }
}

fn power(w: &World, p: usize) -> Power {
    let s = &w.state;
    let mut out = Power::default();
    for row in s.units.slots.iter() {
        if s.units.owner[row] as usize != p || !s.units.is_active(row) {
            continue;
        }
        let bp = w.bp(row);
        let Some(b) = bp.builder.as_ref() else {
            continue;
        };
        let bp_power = b.power.to_f32();
        let busy = s.units.flags[row] & flag::BUILDING != 0;
        let slot = if bp.has(cat::COMMANDER) {
            &mut out.commander
        } else if bp.has(cat::FACTORY) {
            &mut out.factory
        } else {
            &mut out.engineer
        };
        slot[0] += bp_power;
        slot[1] += if busy { bp_power } else { 0.0 };
    }
    out
}

/// Energy a second by source: reactors T1, T2, T3, then everything else.
fn energy(w: &World, p: usize) -> [f32; 4] {
    let s = &w.state;
    let mut e = [0.0; 4];
    for row in s.units.slots.iter() {
        if s.units.owner[row] as usize != p || !s.units.is_active(row) {
            continue;
        }
        let bp = w.bp(row);
        let made = bp.economy.energy_income.to_f32();
        if made <= 0.0 {
            continue;
        }
        let i = if bp.has(cat::POWER) && bp.is_structure() {
            (bp.tech as usize).clamp(1, 3) - 1
        } else {
            3
        };
        e[i] += made;
    }
    e
}

/// Mass standing (finished units' price) by kind: economy, factories, army, defences, T4+.
fn standing(w: &World, p: usize) -> [f32; 5] {
    let s = &w.state;
    let mut v = [0.0; 5];
    for row in s.units.slots.iter() {
        if s.units.owner[row] as usize != p || !s.units.is_active(row) {
            continue;
        }
        let bp = w.bp(row);
        if bp.has(cat::COMMANDER) {
            continue;
        }
        let i = if bp.tech >= 4 {
            4
        } else if bp.has(cat::ECONOMY) || bp.has(cat::ENGINEER) {
            0
        } else if bp.has(cat::FACTORY) {
            1
        } else if bp.is_mobile() {
            2
        } else {
            3
        };
        v[i] += bp.cost_mass.to_f32();
    }
    v
}

fn mines(w: &World, p: usize) -> [u32; 4] {
    let s = &w.state;
    let mut m = [0; 4];
    for row in s.units.slots.iter() {
        let bp = w.bp(row);
        if s.units.owner[row] as usize == p && s.units.is_active(row) && bp.mine.is_some() {
            m[(bp.tech as usize).clamp(1, 4) - 1] += 1;
        }
    }
    m
}

/// A side's running totals over the game.
#[derive(Default)]
struct Totals {
    mined: f32,
    reclaimed: f32,
    spent: f32,
    wasted: f32,
    /// Seconds past minute 10, and of those with the store over 80% full.
    late: u32,
    late_full: u32,
    /// Busy and owned build power summed over seconds past minute 10.
    busy: f32,
    owned: f32,
    stall: u32,
}

#[test]
#[ignore]
fn ledger() {
    let spec = std::env::var("LEDGER").unwrap_or_else(|_| "serac_divide:40".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let minutes: u32 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(40);
    let seed: u64 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(7);
    let swap = parts.get(3) == Some(&"swap");
    let every: u32 = std::env::var("LEDGER_EVERY")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(3);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let player = |i: u8| PlayerSetup {
        name: format!("P{i}"),
        faction: "Aster".into(),
        ai: AiConfig {
            difficulty: Difficulty::Hard,
            ..AiConfig::default()
        },
        team: i,
        controller: Controller::Ai,
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
    println!("ledger {spec}");
    let mut totals = [Totals::default(), Totals::default()];
    let mut reclaimed_before = [0.0f32; 2];
    'game: for minute in 1..=minutes {
        for tick in 0..600 {
            w.tick(&[]).unwrap();
            if tick % 10 != 9 {
                continue;
            }
            // Once a second.
            for (p, t) in totals.iter_mut().enumerate() {
                let pl = &w.state.players[p];
                let income = pl.mass_income.to_f32();
                let spent = pl.mass_spent.to_f32();
                let reclaimed = pl.reclaimed_mass.to_f32();
                t.mined += income;
                t.reclaimed += reclaimed - reclaimed_before[p];
                reclaimed_before[p] = reclaimed;
                t.spent += spent;
                let full = pl.mass.to_f32() >= pl.mass_capacity.to_f32() - 1.0;
                if full {
                    t.wasted += (income - spent).max(0.0);
                }
                t.stall += (pl.energy.to_f32() < 1.0 && pl.energy_demand > pl.energy_income) as u32;
                if minute > 10 {
                    t.late += 1;
                    t.late_full += (pl.mass.to_f32() > 0.8 * pl.mass_capacity.to_f32()) as u32;
                    let bp = power(&w, p);
                    t.busy += bp.busy();
                    t.owned += bp.owned();
                }
            }
            if let Some(winner) = w.state.winner {
                println!("  winner P{winner} at {minute}m");
                break 'game;
            }
        }
        if minute % every != 0 {
            continue;
        }
        for (p, t) in totals.iter().enumerate() {
            let pl = &w.state.players[p];
            let bp = power(&w, p);
            let e = energy(&w, p);
            let v = standing(&w, p);
            let m = mines(&w, p);
            println!(
                "  {minute:>2}m P{p} mass {:>5.1}/s +reclaim {:>4.1} spent {:>5.1} store {:>5.0}/{:<5.0} wasted {:>6.0} | bp fac {:>4.0}/{:<4.0} eng {:>4.0}/{:<4.0} cmd {:>3.0}/{:<3.0} | e/s T1 {:>5.0} T2 {:>5.0} T3 {:>5.0} other {:>4.0} | mines {:?} tech {} | standing eco {:>6.0} fac {:>6.0} army {:>6.0} def {:>6.0} T4+ {:>6.0}",
                pl.mass_income.to_f32(),
                pl.reclaim_income.to_f32(),
                pl.mass_spent.to_f32(),
                pl.mass.to_f32(),
                pl.mass_capacity.to_f32(),
                t.wasted,
                bp.factory[1],
                bp.factory[0],
                bp.engineer[1],
                bp.engineer[0],
                bp.commander[1],
                bp.commander[0],
                e[0],
                e[1],
                e[2],
                e[3],
                m,
                w.side_tech(p as u8),
                v[0],
                v[1],
                v[2],
                v[3],
                v[4],
            );
        }
    }
    for (p, t) in totals.iter().enumerate() {
        println!(
            "  RESULT {spec} P{p}: mined {:.0} reclaimed {:.0} spent {:.0} wasted {:.0} ({:.0}%) | after 10m: store >80% {:.0}% of the time, build power busy {:.0}% | energy stall {} s",
            t.mined,
            t.reclaimed,
            t.spent,
            t.wasted,
            100.0 * t.wasted / (t.mined + t.reclaimed).max(1.0),
            100.0 * t.late_full as f32 / t.late.max(1) as f32,
            100.0 * t.busy / t.owned.max(1.0),
            t.stall,
        );
    }
}
