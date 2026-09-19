//! Match set-up shared by the windowed game, the headless tools and the test scenes.

use mc_core::{Angle, FxVec2};
use mc_data::Blueprints;
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup};
use std::path::PathBuf;

/// Player colours by slot, linear RGB.
pub const TEAM_COLORS: [[f32; 3]; 8] = [
    [0.05, 0.35, 1.0],
    [1.0, 0.08, 0.06],
    [0.1, 0.85, 0.2],
    [1.0, 0.75, 0.05],
    [0.65, 0.15, 1.0],
    [0.05, 0.85, 0.85],
    [1.0, 0.4, 0.05],
    [0.9, 0.9, 0.9],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    /// Commanders and AI opponents: the actual game.
    Skirmish,
    /// Two pre-built armies meet in the middle of the map.
    Battle,
    /// Every player fields a large army: the full-load performance scene.
    Stress,
    /// One of every unit in a row, for looking at models.
    Showcase,
}

impl Scene {
    pub fn parse(s: &str) -> Option<Scene> {
        Some(match s {
            "skirmish" => Scene::Skirmish,
            "battle" => Scene::Battle,
            "stress" => Scene::Stress,
            "showcase" => Scene::Showcase,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct Options {
    pub map: PathBuf,
    pub scene: Scene,
    pub players: usize,
    pub seed: u64,
    /// Units per player in the stress scene.
    pub army: u16,
    pub fog: bool,
}

pub fn find_map(name: Option<&str>) -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = match name {
        Some(n) => vec![PathBuf::from(n), PathBuf::from(format!("maps/{n}.mcmap"))],
        None => vec![PathBuf::from("maps/dev16.mcmap"), PathBuf::from("maps/meridian_basin.mcmap")],
    };
    let roots: Vec<PathBuf> = std::env::current_dir().ok().into_iter().flat_map(|d| d.ancestors().map(|a| a.to_path_buf()).collect::<Vec<_>>()).collect();
    for c in &candidates {
        if c.is_absolute() && c.exists() {
            return Ok(c.clone());
        }
        for root in &roots {
            if root.join(c).exists() {
                return Ok(root.join(c));
            }
        }
    }
    Err(format!(
        "no map found (looked for {candidates:?}). Bake one with: cargo run --release -p mc-map --bin mc-bake -- --size-km 16 --seed 7 -o maps/dev16.mcmap"
    ))
}

pub fn match_config(opts: &Options, map: &MapFile) -> MatchConfig {
    let count = opts.players.clamp(1, map.start_positions().len().clamp(1, 8));
    let players = (0..count)
        .map(|i| PlayerSetup {
            name: if i == 0 { "Commander".into() } else { format!("ARC AI {i}") },
            faction: "Aster".into(),
            team: i as u8,
            controller: if i == 0 || opts.scene != Scene::Skirmish { Controller::Human } else { Controller::Ai },
            start: i as u8,
        })
        .collect();
    MatchConfig { seed: opts.seed, players, cheats: opts.scene != Scene::Skirmish, fog: opts.fog && opts.scene == Scene::Skirmish, spawn_commanders: opts.scene == Scene::Skirmish }
}

/// Commands for the first tick of a test scene.
pub fn opening_commands(opts: &Options, map: &MapFile, blueprints: &Blueprints, config: &MatchConfig) -> Vec<PlayerCommand> {
    let size = map.info().size_metres();
    let centre = size * mc_core::Fx::HALF;
    let id = |key: &str| blueprints.id_of(key).expect("blueprint exists");
    let spawn = |owner: u8, key: &str, pos: FxVec2, heading: Angle, count: u16| PlayerCommand {
        player: owner,
        command: Command::DebugSpawn { owner, blueprint: id(key), pos, heading, count },
    };
    let mut out = Vec::new();
    match opts.scene {
        Scene::Skirmish => {}
        Scene::Battle => {
            let gap = FxVec2::from_ints(420, 0);
            for (owner, side, heading) in [(0u8, centre - gap, Angle::ZERO), (1u8, centre + gap, Angle::HALF_TURN)] {
                if owner as usize >= config.players.len() {
                    continue;
                }
                let back = if owner == 0 { FxVec2::from_ints(-140, 0) } else { FxVec2::from_ints(140, 0) };
                out.push(spawn(owner, "aster_t1_tank", side, heading, 48));
                out.push(spawn(owner, "aster_t2_tank", side + FxVec2::from_ints(0, 220), heading, 16));
                out.push(spawn(owner, "aster_t1_bot", side - FxVec2::from_ints(0, 200), heading, 30));
                out.push(spawn(owner, "aster_t1_artillery", side + back, heading, 12));
                out.push(spawn(owner, "aster_t3_assault_bot", side + back + FxVec2::from_ints(0, 160), heading, 6));
                out.push(spawn(owner, "aster_commander", side + back + back, heading, 1));
            }
        }
        Scene::Stress => {
            let n = config.players.len() as i32;
            for p in 0..n {
                let angle = Angle(((p as i64 * 65536) / n as i64) as u16);
                let ring = mc_core::Fx::from_int((size.x.floor_int() / 5).min(2500));
                let pos = centre + FxVec2::from_angle(angle) * ring;
                let facing = angle + Angle::HALF_TURN;
                let per = opts.army;
                out.push(spawn(p as u8, "aster_t1_tank", pos, facing, per / 2));
                out.push(spawn(p as u8, "aster_t1_bot", pos + FxVec2::from_angle(angle + Angle::QUARTER_TURN) * mc_core::Fx::from_int(500), facing, per / 4));
                out.push(spawn(p as u8, "aster_t2_tank", pos - FxVec2::from_angle(angle + Angle::QUARTER_TURN) * mc_core::Fx::from_int(500), facing, per / 8));
                out.push(spawn(p as u8, "aster_t1_artillery", pos + FxVec2::from_angle(angle) * mc_core::Fx::from_int(300), facing, per / 8));
            }
        }
        Scene::Showcase => {
            // Laid out on the flat ground of the first start position: mobile units in
            // front, structures in a row behind them.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let (mut mobile_x, mut structure_x) = (-280, -560);
            for bp in &blueprints.units {
                if bp.is_structure() {
                    out.push(spawn(0, &bp.key, base + FxVec2::from_ints(structure_x, 176), Angle::from_degrees(270), 1));
                    structure_x += bp.footprint.0 as i32 * 16 + 48;
                } else {
                    out.push(spawn(0, &bp.key, base + FxVec2::from_ints(mobile_x, 0), Angle::from_degrees(250), 1));
                    mobile_x += 40;
                }
            }
        }
    }
    out
}

/// Follow-up orders for a scene once its units exist (second tick).
pub fn scene_orders(opts: &Options, map: &MapFile, world_units: &[(u8, mc_sim::UnitId, bool)]) -> Vec<PlayerCommand> {
    if !matches!(opts.scene, Scene::Battle | Scene::Stress) {
        return Vec::new();
    }
    let centre = map.info().size_metres() * mc_core::Fx::HALF;
    let mut by_owner: std::collections::BTreeMap<u8, Vec<mc_sim::UnitId>> = Default::default();
    for (owner, id, mobile) in world_units {
        if *mobile {
            by_owner.entry(*owner).or_default().push(*id);
        }
    }
    let mut out = Vec::new();
    for (owner, ids) in by_owner {
        for chunk in ids.chunks(mc_sim::command::MAX_COMMAND_UNITS) {
            out.push(PlayerCommand { player: owner, command: Command::AttackMove { units: chunk.to_vec(), target: centre, queue: false } });
        }
    }
    out
}

/// The match every machine builds from a session's start message. The host's
/// template (in `options`) lists every slot; slots a person joined become
/// human-controlled and take that person's name, the rest stay as templated.
pub fn config_from_start(start: &mc_net::MatchStart) -> Result<MatchConfig, String> {
    let mut config: MatchConfig = bincode::deserialize(&start.options).map_err(|e| format!("the host sent unreadable match options: {e}"))?;
    config.seed = start.seed;
    for p in &start.players {
        let slot = config.players.get_mut(p.slot.index()).ok_or(format!("{} joined slot {} but the match has {} slots", p.name, p.slot.0, start.players.len()))?;
        slot.controller = Controller::Human;
        slot.name = p.name.clone();
    }
    Ok(config)
}
