//! Playing a recorded match back (`--replay FILE`): headless up to `--ticks`
//! for a screenshot, `--bench` or `--perf` report of a marked moment, or in a
//! window to watch.

use crate::setup;
use mc_net::{Replay, TickBundle};
use mc_sim::{Command, MatchConfig, PlayerCommand, World};
use std::path::{Path, PathBuf};

pub struct Playback {
    replay: Replay,
    pub config: MatchConfig,
    pub survival: Option<mc_sim::SurvivalConfig>,
    /// `replay.held` carried out so far.
    held: usize,
}

impl Playback {
    pub fn open(path: &Path) -> Result<Playback, String> {
        let replay = Replay::load(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !replay.complete {
            log::info!(
                "{} has no end marker (the match was still running, or crashed); it plays up to tick {}",
                path.display(),
                replay.bundles.len()
            );
        }
        Ok(Playback {
            config: setup::config_from_start(&replay.start)?,
            survival: crate::survival::from_start(&replay.start)?,
            replay,
            held: 0,
        })
    }

    pub fn start(&self) -> &mc_net::MatchStart {
        &self.replay.start
    }

    pub fn ticks(&self) -> u32 {
        self.replay.bundles.len() as u32
    }

    pub fn into_replay(self) -> Replay {
        self.replay
    }

    /// Runs tick `t`: first the orders given while the clock was held in
    /// front of it, then the tick with its own. Returns the state hash.
    pub fn step(&mut self, world: &mut World, t: u32) -> Result<u64, String> {
        while let Some(h) = self.replay.held.get(self.held).filter(|h| h.tick <= t) {
            world.apply_held(&commands(h)).map_err(|e| e.to_string())?;
            self.held += 1;
        }
        let bundle = self
            .replay
            .bundles
            .get(t as usize)
            .ok_or_else(|| format!("the replay ends at tick {}", self.ticks()))?;
        let hash = world.tick(&commands(bundle)).map_err(|e| e.to_string())?;
        if let Some(&want) = self.replay.hashes.get(&t) {
            if want != hash {
                log::warn!(
                    "replay diverged at tick {t}: recorded {want:016x}, played {hash:016x} (the game or its data changed since)"
                );
            }
        }
        Ok(hash)
    }
}

/// Commands as the sim thread decodes them; malformed ones are skipped there too.
fn commands(bundle: &TickBundle) -> Vec<PlayerCommand> {
    bundle
        .commands()
        .filter_map(|(player, bytes)| {
            Command::decode(bytes).map(|command| PlayerCommand {
                player: player.0,
                command,
            })
        })
        .collect()
}

/// The map the replay was recorded on, found by its content id.
pub fn find_map(start: &mc_net::MatchStart) -> Result<PathBuf, String> {
    setup::list_maps()
        .into_iter()
        .find(|p| {
            mc_map::MapFile::open(p).is_ok_and(|m| m.content_id() == start.content.map_id)
        })
        .ok_or_else(|| {
            format!(
                "no map in maps/ has the replay's content id {:016x} (rebaked since?); pass --map to force one",
                start.content.map_id
            )
        })
}

/// Watching the replay in a window, as an observer, at the pace it was played.
pub fn game_start(
    playback: Playback,
    map: std::sync::Arc<mc_map::MapFile>,
) -> crate::game::GameStart {
    let mut colors = setup::TEAM_COLORS;
    if playback.survival.is_some() {
        colors[1] = crate::survival::ENGINE_COLOR;
    }
    let roster = playback.config.players.clone();
    let session = mc_net::ReplaySession::new(playback.into_replay(), mc_net::Pacing::RealTime);
    crate::game::GameStart {
        map,
        colors,
        session: Box::new(session),
        prefetched: Vec::new(),
        local: 0,
        start_index: 0,
        roster,
        observing: true,
        scene: None,
        range: None,
        record: None,
    }
}
