//! Player preferences, kept between runs in the user's config directory.
//! A match's own choices (sky, fog, survival rules, landing zone) are not
//! kept: every new skirmish, survival or range visit starts on the defaults.
//! A missing, unreadable or outdated file is never an error: unknown fields
//! are ignored and missing ones take their defaults.

use crate::audio::Volumes;
use mc_core::Channel;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod quality;
pub use quality::Quality;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub player_name: String,
    pub master_volume: f32,
    pub interface_volume: f32,
    /// Weapons, impacts and explosions.
    pub effects_volume: f32,
    /// Rain and thunder.
    pub weather_volume: f32,
    /// The score (`audio/music.rs`).
    pub music_volume: f32,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Pick `quality` for the graphics card at every start (`Quality::detect`). On
    /// for a new install; a file from before it existed keeps the preset it chose.
    #[serde(default)]
    pub auto_quality: bool,
    /// Base scenery/cloud preset; resolution and AA may be customised below.
    pub quality: Quality,
    /// The 3D scene's resolution against the window's, one of `RENDER_SCALES`:
    /// over 1 supersamples (smoother edges, costlier), under 1 is cheaper.
    pub render_scale: f32,
    /// Edge smoothing on the finished picture.
    pub antialiasing: Antialiasing,
    /// Multiplies the interface scale that follows the window height.
    pub ui_scale: f32,
    pub show_profiler: bool,
    /// The front end cycles through its background scenes by itself.
    pub backdrop_auto_advance: bool,
    /// File stem of the map last chosen for a skirmish.
    pub skirmish_map: String,
    /// File stem of the map last chosen for the test range; empty is the default map.
    pub range_map: String,
    /// File stem of the map last chosen for survival.
    pub survival_map: String,
    /// The multiplayer server last connected to, as typed, for this build's channel:
    /// read from and written to `servers`, and the channel's own server
    /// ([`default_server`]) until another is typed.
    #[serde(skip)]
    pub server: String,
    /// The server typed in each channel's builds, by channel name, when it is not
    /// the channel's own: a playtest and a release build on one computer share this
    /// file, and must not follow each other to their servers.
    pub(crate) servers: BTreeMap<String, String>,
    /// `server` as files from before `servers` kept it: taken over by the first
    /// build that reads the file.
    #[serde(rename = "server", skip_serializing)]
    pub(crate) older_server: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            player_name: mc_net::DEFAULT_PLAYER_NAME.into(),
            master_volume: 0.8,
            interface_volume: 0.8,
            effects_volume: 0.8,
            weather_volume: 0.6,
            music_volume: 0.6,
            fullscreen: false,
            vsync: true,
            auto_quality: true,
            quality: Quality::default(),
            render_scale: Quality::default().render_scale(),
            antialiasing: Quality::default().antialiasing(),
            ui_scale: 1.0,
            show_profiler: false,
            backdrop_auto_advance: true,
            skirmish_map: String::new(),
            range_map: String::new(),
            survival_map: String::new(),
            server: default_server(crate::build_info::channel()).to_owned(),
            servers: BTreeMap::new(),
            older_server: String::new(),
        }
    }
}

/// Edge smoothing, in the order the settings step through it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Antialiasing {
    Off,
    /// FXAA was an option once; a saved one loads as SMAA.
    #[default]
    #[serde(alias = "Fxaa")]
    Smaa,
}

impl Antialiasing {
    pub const ALL: [Antialiasing; 2] = [Antialiasing::Off, Antialiasing::Smaa];

    pub fn label(self) -> &'static str {
        match self {
            Antialiasing::Off => "Off",
            Antialiasing::Smaa => "SMAA",
        }
    }

    pub fn to_renderer(self) -> mc_render::Antialiasing {
        match self {
            Antialiasing::Off => mc_render::Antialiasing::Off,
            Antialiasing::Smaa => mc_render::Antialiasing::Smaa,
        }
    }
}

/// The render scales the settings offer.
pub const RENDER_SCALES: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// Where the game keeps its files: `%APPDATA%\meridian-conflict` on Windows,
/// `$XDG_CONFIG_HOME/meridian-conflict` (or `~/.config/...`) elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    Some(base?.join("meridian-conflict"))
}

fn path() -> Option<PathBuf> {
    Some(config_dir()?.join("settings.ron"))
}

impl Settings {
    pub fn load() -> Settings {
        let Some(path) = path() else {
            return Settings::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Settings::default();
        };
        match ron::from_str::<Settings>(&text) {
            Ok(s) => s.sanitised().with_server_of(crate::build_info::channel()),
            Err(e) => {
                log::warn!("{}: {e}; using default settings", path.display());
                Settings::default()
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        let write = || -> Result<(), String> {
            std::fs::create_dir_all(path.parent().expect("settings path has a parent"))
                .map_err(|e| e.to_string())?;
            let kept = self.keeping_server_of(crate::build_info::channel());
            let text = ron::ser::to_string_pretty(&kept, ron::ser::PrettyConfig::default())
                .map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())
        };
        if let Err(e) = write() {
            log::warn!("settings were not saved to {}: {e}", path.display());
        }
    }

    /// A hand-edited file cannot put the game in a state the UI could not.
    fn sanitised(mut self) -> Settings {
        for v in [
            &mut self.master_volume,
            &mut self.interface_volume,
            &mut self.effects_volume,
            &mut self.weather_volume,
            &mut self.music_volume,
        ] {
            *v = if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.8
            };
        }
        self.ui_scale = if self.ui_scale.is_finite() {
            self.ui_scale.clamp(0.75, 1.5)
        } else {
            1.0
        };
        self.render_scale = if self.render_scale.is_finite() {
            RENDER_SCALES
                .into_iter()
                .min_by(|a, b| {
                    (a - self.render_scale)
                        .abs()
                        .total_cmp(&(b - self.render_scale).abs())
                })
                .unwrap_or(1.0)
        } else {
            1.0
        };
        self.player_name = clean_name(&self.player_name);
        self
    }

    /// `server` as a build of `channel` reads it: what was typed in that channel's
    /// builds, else the channel's own server.
    fn with_server_of(mut self, channel: Channel) -> Settings {
        let older = std::mem::take(&mut self.older_server);
        if !older.is_empty() && !self.servers.contains_key(channel.name()) {
            self.servers.insert(channel.name().to_owned(), older);
        }
        self.server = match self.servers.get(channel.name()) {
            Some(typed) => typed.clone(),
            None => default_server(channel).to_owned(),
        };
        self
    }

    /// The settings to write from a build of `channel`: `server` kept under that
    /// channel, unless it is the channel's own (so a build that moves the server
    /// moves everyone who never typed one).
    fn keeping_server_of(&self, channel: Channel) -> Settings {
        let mut kept = self.clone();
        if self.server == default_server(channel) {
            kept.servers.remove(channel.name());
        } else {
            kept.servers
                .insert(channel.name().to_owned(), self.server.clone());
        }
        kept
    }

    pub fn volumes(&self) -> Volumes {
        Volumes {
            master: self.master_volume,
            interface: self.interface_volume,
            effects: self.effects_volume,
            weather: self.weather_volume,
        }
    }
}

/// The server a build of `channel` plays on until the player types another, set
/// when the game is built: `MERIDIAN_SERVER_PLAYTEST` for a playtest build,
/// `MERIDIAN_SERVER_RELEASE` for a release one (`host:port`, or a host for port
/// 7777). Each channel has its own server, so playtesters and players of the
/// release never meet in one list of games (docs/RELEASES.md). A dev build has
/// none: its multiplayer screen opens on the local network.
pub fn default_server(channel: Channel) -> &'static str {
    let set = match channel {
        Channel::Dev => None,
        Channel::Playtest => option_env!("MERIDIAN_SERVER_PLAYTEST"),
        Channel::Release => option_env!("MERIDIAN_SERVER_RELEASE"),
    };
    set.map_or("", str::trim)
}

/// Names are ASCII (the fonts and the wire format both cope with more, the
/// bitmap HUD font does not) and at most 16 characters.
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(16)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        mc_net::DEFAULT_PLAYER_NAME.into()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_old_files() {
        let s = Settings {
            player_name: "Josh".into(),
            master_volume: 0.3,
            fullscreen: true,
            ..Settings::default()
        };
        let text = ron::ser::to_string_pretty(&s, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(ron::from_str::<Settings>(&text).unwrap(), s);
        // A file from a version with fewer fields, and values out of range.
        let old: Settings = ron::from_str("(player_name: \"  \", master_volume: 7.0)").unwrap();
        let old = old.sanitised();
        assert_eq!(old.player_name, "Commander");
        assert_eq!(old.master_volume, 1.0);
        assert_eq!(old.vsync, Settings::default().vsync);
        assert!(!old.auto_quality, "an older file keeps the preset it had");
        assert!(Settings::default().auto_quality);
        let odd: Settings = ron::from_str("(render_scale: 1.4)").unwrap();
        assert_eq!(odd.sanitised().render_scale, 1.5);
        let fxaa: Settings = ron::from_str("(antialiasing: Fxaa)").unwrap();
        assert_eq!(fxaa.antialiasing, Antialiasing::Smaa);
    }

    /// A server typed in a playtest build stays with playtest builds: a release
    /// build reading the same file plays on its own server.
    #[test]
    fn each_channel_remembers_its_own_server() {
        let read = |text: &str, channel| {
            ron::from_str::<Settings>(text)
                .unwrap()
                .with_server_of(channel)
        };
        let write =
            |s: &Settings, channel| ron::ser::to_string(&s.keeping_server_of(channel)).unwrap();
        let mut playtest = read("()", Channel::Playtest);
        assert_eq!(playtest.server, default_server(Channel::Playtest));
        playtest.server = "trial.example:7777".into();
        let text = write(&playtest, Channel::Playtest);
        assert_eq!(read(&text, Channel::Playtest).server, "trial.example:7777");
        let release = read(&text, Channel::Release);
        assert_eq!(release.server, default_server(Channel::Release));
        // Saving from the release build keeps the playtest build's server.
        let text = write(&release, Channel::Release);
        assert_eq!(read(&text, Channel::Playtest).server, "trial.example:7777");
        assert!(
            !release
                .keeping_server_of(Channel::Release)
                .servers
                .contains_key("release"),
            "the channel's own server is not pinned"
        );
        // A file from before: its one server goes to the first build to read it.
        let old = read("(server: \"home.example\")", Channel::Dev);
        assert_eq!(old.server, "home.example");
        let text = write(&old, Channel::Dev);
        assert_eq!(read(&text, Channel::Dev).server, "home.example");
        assert_eq!(
            read(&text, Channel::Release).server,
            default_server(Channel::Release)
        );
    }
}
