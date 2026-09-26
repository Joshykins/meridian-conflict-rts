//! The score: which song plays where (`data/music/score.ron`).
//!
//! Songs live next to it as `data/music/<name>.ron`. Game moments that the
//! music marks (a nuke landing, a wave arriving, the match won) are cued by
//! name: a song that has a section of that name plays it, as a stinger over
//! the music or, for `victory`/`defeat`, as its ending. A song without one
//! simply carries on.

use crate::song::Song;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Score {
    /// The front end and set-up screens.
    #[serde(default)]
    pub menu: String,
    /// A skirmish, by the player's faction key; `default` for any other.
    #[serde(default)]
    pub battle: BTreeMap<String, String>,
    #[serde(default)]
    pub survival: String,
    /// Seconds to cross-fade from one song to the next.
    #[serde(default = "fade")]
    pub fade: f32,
    /// Game events and the moment each plays (`data/music/moments/<name>.ron`), by the
    /// cue names in `cue`. The song dips under a moment and comes back after it.
    #[serde(default)]
    pub moments: BTreeMap<String, String>,
    /// Events whose moment ends the music instead of handing back to the song.
    #[serde(default)]
    pub endings: Vec<String>,
    /// How far the song dips under a moment, dB.
    #[serde(default = "duck_db")]
    pub duck_db: f32,
}

fn duck_db() -> f32 {
    -16.0
}

fn fade() -> f32 {
    3.0
}

/// Names of the cues the game sends. A song answers the ones it has sections for.
pub mod cue {
    /// A strategic warhead landed anywhere on the map.
    pub const NUKE: &str = "nuke";
    /// The player's commander died (in modes where the match continues).
    pub const COMMANDER_LOST: &str = "commander_lost";
    /// An enemy commander fell.
    pub const ENEMY_COMMANDER: &str = "enemy_commander";
    /// A survival wave arrived.
    pub const WAVE: &str = "wave";
    /// A T4/T5 unit of the player's was finished.
    pub const TITAN: &str = "titan";
    /// Endings.
    pub const VICTORY: &str = "victory";
    pub const DEFEAT: &str = "defeat";
}

impl Score {
    pub fn load(dir: &Path) -> Result<Score, String> {
        let path = dir.join("score.ron");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl Score {
    /// The moment for a game event, and whether it ends the music.
    pub fn moment_for(&self, event: &str) -> Option<(&str, bool)> {
        self.moments
            .get(event)
            .filter(|m| !m.is_empty())
            .map(|m| (m.as_str(), self.endings.iter().any(|e| e == event)))
    }
}

/// Every moment file in `dir/moments`, sorted by name.
pub fn moment_paths(dir: &Path) -> Vec<PathBuf> {
    song_paths(&dir.join("moments"))
}

/// Loads `dir/moments/<name>.ron`.
pub fn load_moment(dir: &Path, name: &str) -> Result<Song, String> {
    Song::load(&dir.join("moments").join(format!("{name}.ron")))
}

/// Every song file in `dir` (not the score), sorted by name.
pub fn song_paths(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().is_some_and(|x| x == "ron")
                        && p.file_stem().is_some_and(|s| s != "score")
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Loads `dir/<name>.ron`.
pub fn load_song(dir: &Path, name: &str) -> Result<Song, String> {
    Song::load(&dir.join(format!("{name}.ron")))
}
