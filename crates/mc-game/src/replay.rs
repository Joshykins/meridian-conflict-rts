//! Playing a recorded match back (`--replay FILE`): headless up to `--ticks`
//! for a screenshot, `--bench` or `--perf` report of a marked moment, or in a
//! window to watch.

use crate::setup;
use mc_net::{Replay, TickBundle};
use mc_sim::{Command, MatchConfig, PlayerCommand, World};
use std::path::{Path, PathBuf};

/// The build a recording says made it: `MERIDIAN_BUILD` at compile time (a
/// release pipeline sets it), else the package version marked `-dev`.
pub const BUILD: &str = match option_env!("MERIDIAN_BUILD") {
    Some(b) => b,
    None => concat!(env!("CARGO_PKG_VERSION"), "-dev"),
};

pub struct Playback {
    replay: Replay,
    path: PathBuf,
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
            path: path.to_owned(),
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
    seek: Option<u32>,
) -> crate::game::GameStart {
    let mut colors = setup::TEAM_COLORS;
    if playback.survival.is_some() {
        colors[1] = crate::survival::ENGINE_COLOR;
    }
    let roster = playback.config.players.clone();
    // Marks made while watching go on this match's id.
    let record = playback
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|id| crate::issues::MatchRecord {
            id: id.to_owned(),
            replay: playback.path.clone(),
        });
    let session =
        mc_net::ReplaySession::new(playback.into_replay(), mc_net::Pacing::RealTime).keep_open();
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
        record,
        seek,
        net: None,
    }
}

/// Ticks between the snapshots a watched replay keeps to rewind to: 30 s of play.
const KEYFRAME_EVERY: u32 = 300;
/// Keyframes past this many bytes are thinned (every other one dropped), so a
/// long match cannot eat the memory; rewinding then replays a little more.
const KEYFRAME_BYTES: usize = 1 << 30;

/// The sim thread's side of a replay being watched: snapshots to rewind to,
/// and where a seek is headed.
#[derive(Default)]
pub struct Scrubber {
    /// Sorted by tick.
    keyframes: Vec<(u32, Vec<u8>)>,
    bytes: usize,
    every: u32,
    /// The tick a seek is running up to.
    pub target: Option<u32>,
}

impl Scrubber {
    pub fn new() -> Scrubber {
        Scrubber {
            every: KEYFRAME_EVERY,
            ..Default::default()
        }
    }

    /// Call with the world standing between ticks (after one, or before the first).
    pub fn keep(&mut self, world: &mut World) {
        let tick = world.tick_count();
        if !tick.is_multiple_of(self.every) || self.keyframes.iter().any(|k| k.0 == tick) {
            return;
        }
        let blob = world.snapshot();
        self.bytes += blob.len();
        let at = self.keyframes.partition_point(|k| k.0 < tick);
        self.keyframes.insert(at, (tick, blob));
        // A deliberate cap: memory, not correctness; see KEYFRAME_BYTES.
        if self.bytes > KEYFRAME_BYTES {
            self.every *= 2;
            let every = self.every;
            self.keyframes.retain(|k| k.0 % every == 0);
            self.bytes = self.keyframes.iter().map(|k| k.1.len()).sum();
            log::info!(
                "replay keyframes thinned to every {} ticks ({} MB)",
                every,
                self.bytes >> 20
            );
        }
    }

    /// Starts a seek to `to`: back to the nearest keyframe at or before it
    /// when the world has to go back (or a keyframe gets it closer), then
    /// the session runs the rest at full speed.
    pub fn seek(
        &mut self,
        world: &mut World,
        map: &mc_map::MapFile,
        session: &mut dyn mc_net::Session,
        to: u32,
    ) -> Result<(), String> {
        let to = session.length().map_or(to, |n| to.min(n));
        let now = world.tick_count();
        let key = self.keyframes.iter().rev().find(|k| k.0 <= to);
        let from = match key {
            Some((tick, blob)) if to < now || *tick > now => {
                let base = mc_map::Heightfield::load(map).map_err(|e| e.to_string())?;
                world.restore(base, blob).map_err(|e| e.to_string())?;
                *tick
            }
            _ if to < now => return Err("no keyframe to go back to".into()),
            _ => now,
        };
        session.seek(from, to);
        self.target = (to > from).then_some(to);
        Ok(())
    }

    /// Still running up to a seek's target: frames need not be drawn.
    pub fn rushing(&mut self, world: &World) -> bool {
        match self.target {
            Some(t) if world.tick_count() < t => true,
            _ => {
                self.target = None;
                false
            }
        }
    }
}

/// `--at`: a match time (`8:43`, `1:02:05`) or a mark of this replay's match (`mark:2`).
pub fn tick_at(replay: &Path, at: &str) -> Result<u32, String> {
    if let Some(n) = at.strip_prefix("mark:") {
        let n: u32 = n.parse().map_err(|_| "--at mark:N takes a number")?;
        let id = replay
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("the replay has no name")?;
        return crate::issues::marks_of(id)
            .into_iter()
            .find(|m| m.number == n)
            .map(|m| m.tick)
            .ok_or_else(|| format!("match {id} has no mark {n} in replays/issues.log"));
    }
    let parts: Vec<u32> = at
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()
        .ok_or("--at takes M:SS, H:MM:SS or mark:N")?;
    let secs = parts.iter().fold(0, |t, p| t * 60 + p);
    Ok(secs * mc_core::TICKS_PER_SECOND)
}

/// What the Replays screen lists about one recording.
pub struct Summary {
    pub path: PathBuf,
    pub id: String,
    /// The map's name, when a map in maps/ has its content id.
    pub map: Option<String>,
    pub length: u32,
    /// False when the recording stopped without its end marker: still running, or crashed.
    pub complete: bool,
    /// Names, with the human seats first-marked.
    pub players: Vec<(String, bool)>,
    pub survival: bool,
    pub marks: Vec<crate::issues::Mark>,
    /// Why it cannot be played, when it cannot.
    pub problem: Option<String>,
    /// The build that recorded it, when it said (older recordings did not).
    pub build: Option<String>,
}

impl Summary {
    pub fn playable(&self) -> bool {
        self.problem.is_none()
    }
}

/// Every recording in replays/, newest first. Reads every file, so off the UI thread.
pub fn summaries() -> Vec<Summary> {
    let maps: Vec<(u64, String)> = setup::list_maps()
        .iter()
        .filter_map(|p| mc_map::MapFile::open(p).ok())
        .map(|m| (m.content_id(), m.name().to_owned()))
        .collect();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(crate::issues::DIR)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == mc_net::REPLAY_EXTENSION))
        .collect();
    // The names are start times: newest first.
    paths.sort_unstable_by(|a, b| b.cmp(a));
    paths.into_iter().map(|p| summary(p, &maps)).collect()
}

fn summary(path: PathBuf, maps: &[(u64, String)]) -> Summary {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    let mut out = Summary {
        marks: crate::issues::marks_of(&id),
        id,
        path,
        map: None,
        length: 0,
        complete: false,
        players: Vec::new(),
        survival: false,
        problem: None,
        build: None,
    };
    let replay = match Replay::load(&out.path) {
        Ok(r) => r,
        Err(mc_net::NetError::Version { theirs }) => {
            out.problem = Some(format!(
                "Recorded by another build (replay format {theirs}, this build reads {})",
                mc_net::REPLAY_FORMAT_VERSION
            ));
            return out;
        }
        Err(e) => {
            out.problem = Some(format!("Unreadable: {e}"));
            return out;
        }
    };
    out.length = replay.bundles.len() as u32;
    out.build = replay.build.clone();
    out.complete = replay.complete;
    out.map = maps
        .iter()
        .find(|m| m.0 == replay.start.content.map_id)
        .map(|m| m.1.clone());
    if out.map.is_none() {
        out.problem = Some("Its map is not in maps/ (rebaked since?)".into());
    }
    match setup::config_from_start(&replay.start) {
        Ok(config) => {
            out.players = config
                .players
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        p.controller == mc_sim::tables::Controller::Human,
                    )
                })
                .collect();
        }
        Err(e) => out.problem = Some(e),
    }
    out.survival = crate::survival::from_start(&replay.start).is_ok_and(|s| s.is_some());
    out
}
