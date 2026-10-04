//! Playing a recorded match back (`--replay FILE`): headless up to `--ticks`
//! for a screenshot, `--bench` or `--perf` report of a marked moment, or in a
//! window to watch.

use crate::chronicle::Chronicle;
use crate::setup;
use mc_net::{Replay, TickBundle};
use mc_sim::{Command, MatchConfig, PlayerCommand, World};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub struct Playback {
    replay: Replay,
    path: PathBuf,
    pub config: MatchConfig,
    pub survival: Option<mc_sim::SurvivalConfig>,
    /// The weather and time of day the match was played under.
    pub sky: mc_data::weather::SkyChoice,
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
        let options = crate::match_options::MatchOptions::from_start(&replay.start)?;
        Ok(Playback {
            config: options.config,
            survival: options.survival,
            sky: options.sky,
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

/// The battle report's record of a recorded match: the one kept beside it, or else
/// worked out by playing it through, headless and as fast as it goes, and then kept.
/// `played` counts the ticks played so far, out of `Playback::ticks`; setting `stop`
/// gives up part way.
pub fn chronicle_of(
    path: &Path,
    map: &mc_map::MapFile,
    blueprints: &Arc<mc_data::Blueprints>,
    pool: &Arc<mc_jobs::Pool>,
    played: &AtomicU32,
    stop: &AtomicBool,
) -> Result<Chronicle, String> {
    let kept = crate::chronicle::file_for(path);
    if let Some(chronicle) = Chronicle::load(&kept, blueprints) {
        return Ok(chronicle);
    }
    let mut playback = Playback::open(path)?;
    let mut world = World::new(map, blueprints.clone(), pool.clone(), &playback.config)
        .map_err(|e| e.to_string())?;
    if let Some(survival) = playback.survival.clone() {
        world.begin_survival(survival).map_err(|e| e.to_string())?;
    }
    let mut chronicle = Chronicle::new(glam::Vec2::from(map.info().size_metres().to_f32()));
    for t in 0..playback.ticks() {
        if stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        playback.step(&mut world, t)?;
        chronicle.record(&world);
        played.store(t + 1, Ordering::Relaxed);
        if chronicle.ended.is_some() {
            break;
        }
    }
    if let Err(e) = chronicle.save(&kept, blueprints.content_hash()) {
        log::warn!("the report of {} was not kept: {e}", path.display());
    }
    Ok(chronicle)
}

/// The test range's weather a recording showed in front of `tick`, if it kept any.
pub fn range_sky_at(path: &Path, tick: u32) -> Option<crate::range::RangeSky> {
    let replay = Replay::load(path).ok()?;
    replay
        .notes
        .iter()
        .take_while(|(t, _)| *t <= tick)
        .filter_map(|(_, note)| match crate::recorder::Note::decode(note)? {
            crate::recorder::Note::RangeSky(sky) => Some(sky),
        })
        .last()
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

/// The sides' colours a recorded match is shown in.
pub fn colors(survival: bool) -> setup::Palette {
    let mut colors = setup::TEAM_COLORS;
    if survival {
        colors[1] = crate::survival::ENGINE_COLOR;
    }
    colors
}

/// Watching the replay in a window, as an observer, at the pace it was played.
pub fn game_start(
    playback: Playback,
    map: std::sync::Arc<mc_map::MapFile>,
    seek: Option<u32>,
) -> crate::game::GameStart {
    let colors = colors(playback.survival.is_some());
    let roster = playback.config.players.clone();
    let sky = playback.sky;
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
        sky,
        session: Box::new(session),
        prefetched: Vec::new(),
        local: 0,
        start_index: 0,
        roster,
        observing: true,
        scene: None,
        range: None,
        record,
        recorder: None,
        seek,
        net: None,
        keep: Vec::new(),
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

/// What Match History lists about one recording.
pub struct Summary {
    pub path: PathBuf,
    pub id: String,
    /// The map's name and file, when a map in maps/ has its content id.
    pub map: Option<String>,
    pub map_path: Option<PathBuf>,
    pub length: u32,
    /// False when the recording stopped without its end marker: still running, or crashed.
    pub complete: bool,
    /// Names, with the human seats first-marked.
    pub players: Vec<(String, bool)>,
    pub survival: bool,
    pub marks: Vec<crate::issues::Mark>,
    /// Why it cannot be played here, when it cannot.
    pub problem: Option<String>,
    /// Who recorded it, as far as the file says.
    pub origin: Option<mc_net::Origin>,
    /// Whether this build plays it as it was played.
    pub fit: Fit,
    /// Its battle report's record is kept beside it, so the report opens at once.
    pub report_kept: bool,
}

impl Summary {
    pub fn playable(&self) -> bool {
        self.problem.is_none()
    }

    /// The build that plays it faithfully, when that is not this one.
    pub fn other_build(&self) -> Option<&str> {
        match self.fit {
            Fit::Here => None,
            Fit::Differs | Fit::Elsewhere => self
                .origin
                .as_ref()
                .map(|o| o.build.as_str())
                // The same build with other unit data: edited data/, nothing to fetch.
                .filter(|b| !b.is_empty() && *b != crate::BUILD),
        }
    }
}

/// Whether this build plays a recording as it was played.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    /// Same simulation, unit data and map: it plays out as it did.
    Here,
    /// This build can open it, but its simulation or unit data differ (or the
    /// recording does not say): it may play out differently.
    Differs,
    /// Only the build that recorded it can open it: another replay format, or
    /// a map that is no longer in maps/.
    Elsewhere,
}

/// How a recording made by `origin`, in replay `format`, fits this build,
/// whose unit data hashes to `blueprints`; `map_here` when maps/ has its map.
pub fn fit(format: u32, origin: &mc_net::Origin, blueprints: u64, map_here: bool) -> Fit {
    if format != mc_net::REPLAY_FORMAT_VERSION || !map_here {
        Fit::Elsewhere
    } else if origin.sim == crate::build_info::sim() && origin.content.blueprint_hash == blueprints
    {
        Fit::Here
    } else {
        Fit::Differs
    }
}

/// Every recording in replays/, newest first, as it fits this build, whose
/// unit data hashes to `blueprints`. Reads every file, so off the UI thread.
pub fn summaries(blueprints: u64) -> Vec<Summary> {
    let maps: Vec<(u64, String, PathBuf)> = setup::list_maps()
        .into_iter()
        .filter_map(|p| {
            let m = mc_map::MapFile::open(&p).ok()?;
            Some((m.content_id(), m.name().to_owned(), p))
        })
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
    paths
        .into_iter()
        .map(|p| summary(p, &maps, blueprints))
        .collect()
}

fn summary(path: PathBuf, maps: &[(u64, String, PathBuf)], blueprints: u64) -> Summary {
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
        map_path: None,
        length: 0,
        complete: false,
        players: Vec::new(),
        survival: false,
        problem: None,
        origin: None,
        fit: Fit::Elsewhere,
        report_kept: false,
    };
    out.report_kept = crate::chronicle::file_for(&out.path).exists();
    let peeked = match mc_net::Origin::peek_file(&out.path) {
        Ok(p) => p,
        Err(e) => {
            out.problem = Some(format!("Unreadable: {e}"));
            return out;
        }
    };
    if let Some(m) = maps.iter().find(|m| m.0 == peeked.origin.content.map_id) {
        out.map = Some(m.1.clone());
        out.map_path = Some(m.2.clone());
    }
    out.fit = fit(peeked.format, &peeked.origin, blueprints, out.map.is_some());
    out.origin = Some(peeked.origin);
    if out.fit == Fit::Elsewhere {
        let by = match out.origin.as_ref().map(|o| o.build.as_str()) {
            Some(b) if !b.is_empty() => b.to_owned(),
            _ => "another build".to_owned(),
        };
        out.problem = Some(
            if out.map.is_none() && peeked.format == mc_net::REPLAY_FORMAT_VERSION {
                format!("Plays in {by}: its map is not in this build's maps/")
            } else {
                format!("Plays in {by}")
            },
        );
        return out;
    }
    let replay = match Replay::load(&out.path) {
        Ok(r) => r,
        Err(e) => {
            out.problem = Some(format!("Unreadable: {e}"));
            return out;
        }
    };
    out.length = replay.bundles.len() as u32;
    out.complete = replay.complete;
    match crate::match_options::MatchOptions::from_start(&replay.start) {
        Ok(options) => {
            out.survival = options.survival.is_some();
            out.players = options
                .config
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
    out
}
