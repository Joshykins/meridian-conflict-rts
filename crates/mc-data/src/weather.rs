//! The weather a map is played in: how much cloud, how many storms, rain,
//! how tall and how big the clouds grow. Cosmetic and client-side only; the
//! simulation never sees it.
//!
//! A map names its weather in `maps/<stem>.ron` next to the baked map:
//!
//! ```ron
//! (
//!     weather: Stormy,
//!     // Optional: change any value of the preset.
//!     tweaks: (rain: 0.9, scale: 1.6),
//!     // Optional: when in the day (Dawn, Morning, Noon, Afternoon, Dusk,
//!     // Night), or an exact `hour: 19.2`.
//!     time: Dusk,
//! )
//! ```
//!
//! Skirmish set-up can override the map's choice with another preset.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A named kind of weather. `Weather::from(preset)` gives its values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherPreset {
    /// A few small fair-weather clouds, no storms.
    Clear,
    /// Scattered cumulus and the odd storm.
    #[default]
    Fair,
    /// Big, tall cloud fields with some storms and showers.
    Cloudy,
    /// Towering storm cells, lightning and heavy rain.
    Stormy,
    /// A grey deck over everything, steady rain, few breaks.
    Overcast,
}

impl WeatherPreset {
    pub const ALL: [WeatherPreset; 5] = [
        WeatherPreset::Clear,
        WeatherPreset::Fair,
        WeatherPreset::Cloudy,
        WeatherPreset::Stormy,
        WeatherPreset::Overcast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WeatherPreset::Clear => "Clear",
            WeatherPreset::Fair => "Fair",
            WeatherPreset::Cloudy => "Cloudy",
            WeatherPreset::Stormy => "Stormy",
            WeatherPreset::Overcast => "Overcast",
        }
    }
}

/// The values the sky runs on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Weather {
    /// How much of the sky the air mass fills: 0.6 clear, 1.2 fair, 1.9 overcast.
    pub cover: f32,
    /// Storms at once, against a map-size default: 0 none, 1 usual, 3 many.
    pub storms: f32,
    /// How readily heavy cloud rains, 0 to 1.
    pub rain: f32,
    /// How tall clouds grow and how much their tops vary, 0 flat to 1 towering.
    pub towering: f32,
    /// Size of cloud masses against the usual: 2 makes some twice as wide.
    pub scale: f32,
    /// Prevailing wind, metres a second.
    pub wind: f32,
    /// Lightning, against the usual rate: 0 none.
    pub lightning: f32,
}

impl Default for Weather {
    fn default() -> Weather {
        Weather::from(WeatherPreset::Fair)
    }
}

impl From<WeatherPreset> for Weather {
    fn from(preset: WeatherPreset) -> Weather {
        match preset {
            WeatherPreset::Clear => Weather {
                cover: 0.75,
                storms: 0.0,
                rain: 0.0,
                towering: 0.2,
                scale: 0.8,
                wind: 8.0,
                lightning: 0.0,
            },
            WeatherPreset::Fair => Weather {
                cover: 1.2,
                storms: 1.0,
                rain: 0.5,
                towering: 0.5,
                scale: 1.0,
                wind: 12.0,
                lightning: 1.0,
            },
            WeatherPreset::Cloudy => Weather {
                cover: 1.32,
                storms: 1.2,
                rain: 0.6,
                towering: 0.8,
                scale: 1.5,
                wind: 14.0,
                lightning: 0.7,
            },
            WeatherPreset::Stormy => Weather {
                cover: 1.5,
                storms: 2.8,
                rain: 1.0,
                towering: 1.0,
                scale: 1.5,
                wind: 18.0,
                lightning: 2.0,
            },
            WeatherPreset::Overcast => Weather {
                cover: 1.9,
                storms: 0.6,
                rain: 0.8,
                towering: 0.3,
                scale: 2.2,
                wind: 10.0,
                lightning: 0.4,
            },
        }
    }
}

/// When in the day a match is played: where the sun (or the moon) stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDay {
    Dawn,
    Morning,
    Noon,
    #[default]
    Afternoon,
    Dusk,
    Night,
}

impl TimeOfDay {
    pub const ALL: [TimeOfDay; 6] = [
        TimeOfDay::Dawn,
        TimeOfDay::Morning,
        TimeOfDay::Noon,
        TimeOfDay::Afternoon,
        TimeOfDay::Dusk,
        TimeOfDay::Night,
    ];

    /// The hour on a 24-hour clock (the sky follows a simple equinox day, sun
    /// up at 6 and down at 18).
    pub fn hour(self) -> f32 {
        match self {
            TimeOfDay::Dawn => 6.6,
            TimeOfDay::Morning => 9.0,
            TimeOfDay::Noon => 12.5,
            TimeOfDay::Afternoon => 15.3,
            TimeOfDay::Dusk => 17.6,
            TimeOfDay::Night => 23.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TimeOfDay::Dawn => "Dawn",
            TimeOfDay::Morning => "Morning",
            TimeOfDay::Noon => "Noon",
            TimeOfDay::Afternoon => "Afternoon",
            TimeOfDay::Dusk => "Dusk",
            TimeOfDay::Night => "Night",
        }
    }
}

/// Changes to a preset's values; anything left out keeps the preset's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherTweaks {
    pub cover: Option<f32>,
    pub storms: Option<f32>,
    pub rain: Option<f32>,
    pub towering: Option<f32>,
    pub scale: Option<f32>,
    pub wind: Option<f32>,
    pub lightning: Option<f32>,
}

impl WeatherTweaks {
    /// `w` with these changes made.
    pub fn apply(&self, mut w: Weather) -> Weather {
        let set = |v: &mut f32, o: Option<f32>| {
            if let Some(o) = o {
                *v = o;
            }
        };
        set(&mut w.cover, self.cover);
        set(&mut w.storms, self.storms);
        set(&mut w.rain, self.rain);
        set(&mut w.towering, self.towering);
        set(&mut w.scale, self.scale);
        set(&mut w.wind, self.wind);
        set(&mut w.lightning, self.lightning);
        w
    }
}

/// The weather a player picked for a match (skirmish set-up, the test range):
/// a preset or the map's own, and the time of day. Either left at none plays
/// what the map asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkyChoice {
    pub preset: Option<WeatherPreset>,
    pub time: Option<TimeOfDay>,
}

impl SkyChoice {
    pub fn weather(&self, map: &MapConfig) -> Weather {
        map.weather(self.preset)
    }

    pub fn hour(&self, map: &MapConfig) -> f32 {
        map.hour(self.time)
    }
}

/// A map's own settings file, `maps/<stem>.ron`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MapConfig {
    pub weather: WeatherPreset,
    pub tweaks: WeatherTweaks,
    /// When in the day the map is played, unless skirmish set-up picks another.
    pub time: TimeOfDay,
    /// An exact hour instead of `time` (24-hour clock), for a map that wants one.
    pub hour: Option<f32>,
    /// Present on maps made for survival (`crate::survival`).
    pub survival: Option<crate::survival::SurvivalLayout>,
}

impl MapConfig {
    /// The config beside `map_path` (`foo.mcmap` -> `foo.ron`). A missing file
    /// is the defaults; one that does not parse is an error the caller reports.
    pub fn for_map(map_path: &Path) -> Result<MapConfig, String> {
        let path = map_path.with_extension("ron");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(MapConfig::default());
        };
        MapConfig::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Reads a config; tweaks are written as plain numbers (`rain: 0.3`).
    pub fn parse(text: &str) -> Result<MapConfig, ron::error::SpannedError> {
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(text)
    }

    /// The hour to play at: `choice` (from skirmish set-up), or the map's own.
    pub fn hour(&self, choice: Option<TimeOfDay>) -> f32 {
        match choice {
            Some(t) => t.hour(),
            None => self.hour.unwrap_or_else(|| self.time.hour()),
        }
    }

    /// The weather to play in: the map's preset with its tweaks, or `choice`
    /// (from skirmish set-up) when the player picked one.
    pub fn weather(&self, choice: Option<WeatherPreset>) -> Weather {
        if let Some(preset) = choice {
            return Weather::from(preset);
        }
        self.tweaks.apply(Weather::from(self.weather))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_config_reads_a_preset_and_tweaks() {
        let c = MapConfig::parse("(weather: Stormy, tweaks: (rain: 0.3))").unwrap();
        let w = c.weather(None);
        assert_eq!(w.rain, 0.3);
        assert_eq!(w.storms, Weather::from(WeatherPreset::Stormy).storms);
        assert_eq!(c.weather(Some(WeatherPreset::Clear)), Weather::from(WeatherPreset::Clear));
        let empty = MapConfig::parse("()").unwrap();
        assert_eq!(empty.weather(None), Weather::default());
    }

    #[test]
    fn a_sky_choice_saved_with_old_tweaks_still_loads() {
        let c: SkyChoice = ron::from_str("(preset: Some(Stormy), tweaks: (rain: Some(0.0)), time: None)").unwrap();
        assert_eq!(c.preset, Some(WeatherPreset::Stormy));
        assert_eq!(c.weather(&MapConfig::default()), Weather::from(WeatherPreset::Stormy));
    }
}
