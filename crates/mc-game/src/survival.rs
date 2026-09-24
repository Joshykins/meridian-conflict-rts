//! Setting up a survival match: the map's layout (`maps/<stem>.ron`,
//! `mc_data::survival`) and the player's rules become the sim's
//! `SurvivalConfig`, which travels in the match options after the
//! `MatchConfig` so every machine, and every replay, plays the same rounds.

use crate::setup;
use mc_core::{Fx, FxVec2};
use mc_data::survival::{Domain, SurvivalLayout};
use mc_map::MapFile;
use mc_sim::survival::{Front, NodeSite};
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, MatchConfig, PlayerSetup, SurvivalConfig, SurvivalRules};

/// The engine side's colour: replication violet.
pub const ENGINE_COLOR: [f32; 3] = [0.62, 0.36, 1.0];

/// The map's survival layout, if it has a usable one.
pub fn layout(map: &MapFile) -> Option<SurvivalLayout> {
    let layout = setup::map_config(map).survival?;
    if let Some(problem) = layout.problem() {
        log::warn!("{}: {problem}", map.name());
        return None;
    }
    let starts = map.start_positions().len();
    if layout.engine_start as usize >= starts || layout.spawns.iter().any(|s| s.start as usize >= starts) {
        log::warn!("{}: survival names a start position the map does not have", map.name());
        return None;
    }
    Some(layout)
}

fn fx(p: (f32, f32)) -> FxVec2 {
    FxVec2::new(Fx::from_f32(p.0), Fx::from_f32(p.1))
}

/// Everything a survival match needs: the players and the engine's rules.
pub struct Setup<'a> {
    pub layout: &'a SurvivalLayout,
    pub rules: SurvivalRules,
    /// Index into `layout.spawns`.
    pub spawn: usize,
    pub name: String,
    pub seed: u64,
    pub fog: bool,
    /// The defenders' seat is an AI (tools and probes): nobody plays.
    pub observe: bool,
    pub ai: AiConfig,
}

pub fn match_for(s: &Setup) -> (MatchConfig, SurvivalConfig) {
    let spawn = &s.layout.spawns[s.spawn.min(s.layout.spawns.len() - 1)];
    let players = vec![
        PlayerSetup {
            name: s.name.clone(),
            faction: "Aster".into(),
            ai: s.ai,
            team: 0,
            controller: if s.observe { Controller::Ai } else { Controller::Human },
            start: spawn.start,
        },
        PlayerSetup {
            name: "Replication Engine".into(),
            faction: "Aster".into(),
            ai: AiConfig::default(),
            team: 1,
            controller: Controller::Ai,
            start: s.layout.engine_start,
        },
    ];
    let config = MatchConfig {
        seed: s.seed,
        players,
        cheats: false,
        fog: s.fog,
        spawn_commanders: true,
    };
    let survival = SurvivalConfig {
        engine_player: 1,
        engine: fx(s.layout.engine),
        harbor: s.layout.harbor.map(fx),
        fronts: s
            .layout
            .fronts
            .iter()
            .map(|f| Front { domain: f.domain, path: f.path.iter().copied().map(fx).collect() })
            .collect(),
        node_sites: s
            .layout
            .node_sites
            .iter()
            .map(|n| NodeSite { at: fx(n.at), domain: n.domain })
            .collect(),
        rules: s.rules,
    };
    (config, survival)
}

/// The match options for a start message: the config, then the survival half.
pub fn encode_options(config: &MatchConfig, survival: Option<&SurvivalConfig>) -> Result<Vec<u8>, String> {
    let mut out = bincode::serialize(config).map_err(|e| e.to_string())?;
    if let Some(s) = survival {
        out.extend(bincode::serialize(s).map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// The survival half of a start message's options, if the match is survival.
pub fn from_start(start: &mc_net::MatchStart) -> Result<Option<SurvivalConfig>, String> {
    let mut cursor = std::io::Cursor::new(&start.options[..]);
    let _: MatchConfig = bincode::deserialize_from(&mut cursor)
        .map_err(|e| format!("the host sent unreadable match options: {e}"))?;
    if cursor.position() as usize >= start.options.len() {
        return Ok(None);
    }
    bincode::deserialize_from(&mut cursor)
        .map(Some)
        .map_err(|e| format!("the host sent unreadable survival options: {e}"))
}

/// The survival match `--scene survival` plays: the map's first spawn, default
/// rules (overridable with `MERIDIAN_SURVIVAL=rounds:grace:interval:intensity:fronts:tier:nodes`).
pub fn scene_match(opts: &setup::Options, map: &MapFile) -> Result<(MatchConfig, SurvivalConfig), String> {
    let layout = layout(map).ok_or_else(|| format!("{} has no survival layout", map.name()))?;
    let mut rules = SurvivalRules::default();
    if let Ok(v) = std::env::var("MERIDIAN_SURVIVAL") {
        let n: Vec<u16> = v.split(':').filter_map(|x| x.parse().ok()).collect();
        let get = |i: usize, d: u16| n.get(i).copied().unwrap_or(d);
        rules = SurvivalRules {
            rounds: get(0, rules.rounds),
            grace_secs: get(1, rules.grace_secs),
            interval_secs: get(2, rules.interval_secs),
            intensity: get(3, rules.intensity),
            fronts: get(4, rules.fronts as u16) as u8,
            tier_cap: get(5, rules.tier_cap as u16) as u8,
            nodes: get(6, rules.nodes as u16) as u8,
        };
    }
    let spawn = std::env::var("MERIDIAN_SURVIVAL_SPAWN").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    Ok(match_for(&Setup {
        layout: &layout,
        rules,
        spawn,
        name: if opts.observe { "ARC AI".into() } else { "Commander".into() },
        seed: opts.seed,
        fog: opts.fog,
        observe: opts.observe,
        ai: opts.ai,
    }))
}

/// Number of fronts of each domain in a layout, `Domain::ALL` order.
pub fn front_counts(layout: &SurvivalLayout) -> [usize; 3] {
    Domain::ALL.map(|d| layout.fronts_of(d).count())
}

/// Headless survival runs print what happens: rounds, nodes, and a line a minute.
pub fn log_tick(world: &mc_sim::World, tick: u32) {
    use mc_sim::SimEvent;
    let name = |b: mc_data::BlueprintId| world.blueprints.unit(b).name.clone();
    let min = tick as f32 / 600.0;
    for e in &world.events {
        match e {
            SimEvent::RoundPrinting { round } => println!("[{min:5.1} min] round {round} printing"),
            SimEvent::RoundLaunched { round, units } => println!("[{min:5.1} min] round {round} launched, {units} units"),
            SimEvent::NodeRaising { site, product, .. } => println!("[{min:5.1} min] node rising at site {site} ({})", name(*product)),
            SimEvent::NodeOnline { site, product, .. } => println!("[{min:5.1} min] node online at site {site} ({})", name(*product)),
            SimEvent::NodeDestroyed { site, wreck, .. } => println!("[{min:5.1} min] node at site {site} destroyed, wreck {wreck}"),
            SimEvent::SurvivalWon { rounds } => println!("[{min:5.1} min] defenders won after {rounds} rounds"),
            SimEvent::PlayerDefeated { player } => println!("[{min:5.1} min] player {player} defeated"),
            _ => {}
        }
    }
    if tick % 600 == 0 {
        if let Some(s) = world.survival_status() {
            let p = &world.state.players[0];
            println!(
                "[{min:5.1} min] {:?} round {} | hostile {} | nodes {} | defender units {} mass {:.0}/{:.0} +{:.0}/s reclaim +{:.0}/s | tick {:.2} ms",
                s.phase,
                s.round,
                s.hostile,
                s.nodes.len(),
                world.state.units.slots.iter().filter(|&r| world.state.units.owner[r] == 0).count(),
                p.mass.to_f32(),
                p.mass_capacity.to_f32(),
                p.mass_income.to_f32(),
                p.reclaim_income.to_f32(),
                world.timings.total_ns as f64 / 1e6
            );
            if std::env::var("MERIDIAN_SURVIVAL_WHY").is_ok_and(|v| !v.is_empty()) {
                let u = &world.state.units;
                let target = world.state.players[0].start;
                let mut rows: std::collections::BTreeMap<(String, u32, bool), u32> = Default::default();
                for r in u.slots.iter().filter(|&r| u.owner[r] == 1) {
                    let bp = world.blueprints.unit(u.blueprint[r]);
                    if !bp.is_mobile() {
                        continue;
                    }
                    let km = (u.pos[r].distance(target).to_f32() / 1000.0) as u32;
                    if bp.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Naval) && km >= 15 {
                        let o = world.state.orders.front(u, r).map(|o| (o.kind, o.pos.to_f32(), o.offset.to_f32()));
                        println!("      ship {} at {:?} moving {} order {:?}", bp.key, u.pos[r].to_f32(), u.has_flag(r, mc_sim::tables::flag::MOVING), o);
                    }
                    let idle = u.order_head[r] == mc_sim::tables::NO_ORDER;
                    *rows.entry((bp.key.clone(), km, idle)).or_default() += 1;
                }
                for ((k, km, idle), n) in rows {
                    println!("      {n:>3} x {k} {km} km from target{}", if idle { " IDLE" } else { "" });
                }
            }
        }
    }
}
