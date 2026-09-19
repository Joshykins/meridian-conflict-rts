//! Player preferences, kept between runs in the user's config directory.
//! A missing, unreadable or outdated file is never an error: unknown fields
//! are ignored and missing ones take their defaults.

use crate::audio::Volumes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub player_name: String,
    pub master_volume: f32,
    pub interface_volume: f32,
    pub ambience_volume: f32,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Multiplies the interface scale that follows the window height.
    pub ui_scale: f32,
    pub show_profiler: bool,
    /// The front end cycles through its background scenes by itself.
    pub backdrop_auto_advance: bool,
    /// File stem of the map last chosen for a skirmish.
    pub skirmish_map: String,
    pub skirmish_fog: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            player_name: "Commander".into(),
            master_volume: 0.8,
            interface_volume: 0.8,
            ambience_volume: 0.6,
            fullscreen: false,
            vsync: true,
            ui_scale: 1.0,
            show_profiler: false,
            backdrop_auto_advance: true,
            skirmish_map: String::new(),
            skirmish_fog: true,
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    Some(base?.join("meridian-conflict").join("settings.ron"))
}

impl Settings {
    pub fn load() -> Settings {
        let Some(path) = path() else { return Settings::default() };
        let Ok(text) = std::fs::read_to_string(&path) else { return Settings::default() };
        match ron::from_str::<Settings>(&text) {
            Ok(s) => s.sanitised(),
            Err(e) => {
                log::warn!("{}: {e}; using default settings", path.display());
                Settings::default()
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        let write = || -> Result<(), String> {
            std::fs::create_dir_all(path.parent().expect("settings path has a parent")).map_err(|e| e.to_string())?;
            let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| e.to_string())
        };
        if let Err(e) = write() {
            log::warn!("settings were not saved to {}: {e}", path.display());
        }
    }

    /// A hand-edited file cannot put the game in a state the UI could not.
    fn sanitised(mut self) -> Settings {
        for v in [&mut self.master_volume, &mut self.interface_volume, &mut self.ambience_volume] {
            *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.8 };
        }
        self.ui_scale = if self.ui_scale.is_finite() { self.ui_scale.clamp(0.75, 1.5) } else { 1.0 };
        self.player_name = clean_name(&self.player_name);
        self
    }

    pub fn volumes(&self) -> Volumes {
        Volumes { master: self.master_volume, interface: self.interface_volume, ambience: self.ambience_volume }
    }
}

/// Names are ASCII (the fonts and the wire format both cope with more, the
/// bitmap HUD font does not) and at most 16 characters.
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(16).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() { "Commander".into() } else { trimmed.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_old_files() {
        let s = Settings { player_name: "Josh".into(), master_volume: 0.3, fullscreen: true, ..Settings::default() };
        let text = ron::ser::to_string_pretty(&s, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(ron::from_str::<Settings>(&text).unwrap(), s);
        // A file from a version with fewer fields, and values out of range.
        let old: Settings = ron::from_str("(player_name: \"  \", master_volume: 7.0)").unwrap();
        let old = old.sanitised();
        assert_eq!(old.player_name, "Commander");
        assert_eq!(old.master_volume, 1.0);
        assert_eq!(old.vsync, Settings::default().vsync);
    }
}
