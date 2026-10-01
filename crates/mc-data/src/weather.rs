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
//!
//! A map can also be split in two by a climate divide, a line from its south
//! edge to its north edge. West of it the map's own `climate` and `weather`
//! apply; east of it the divide's:
//!
//! ```ron
//! (
//!     climate: Desert,
//!     weather: Clear,
//!     // Metres the desert's rock beds are lowered on this map: ground at height
//!     // h is coloured as Vermilion Gorge's is at h + strata_lift. Default 0.
//!     strata_lift: 50,
//!     divide: (
//!         // South to north, map metres (x, y), y strictly ascending, 2 to 8
//!         // points; the first and last sit on (or beyond) the map's edges.
//!         line: [(6592, 0), (6592, 3400), (8192, 5000), (8192, 16384)],
//!         climate: Temperate,
//!         weather: Cloudy,
//!         tweaks: (rain: 0.7),
//!     ),
//! )
//! ```
//!
//! A preset picked in skirmish set-up plays over both sides.

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

    /// The weather east of the map's climate divide (`MapConfig::east_weather`).
    pub fn east_weather(&self, map: &MapConfig) -> Option<Weather> {
        map.east_weather(self.preset)
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
    /// How the map's ground and sea are coloured (`Climate`).
    pub climate: Climate,
    /// What the land is like, for the map browser's filter. Unset: from `climate`.
    pub biome: Option<Biome>,
    /// How the map is meant to be played, for the map browser's filter.
    /// Unset: a duel on two starts, teams on more.
    pub style: Option<MapStyle>,
    /// Metres the desert's rock beds are lowered on this map: ground at height
    /// `h` above the water is coloured as Vermilion Gorge's is at `h +
    /// strata_lift` (shaders/desert.wgsl).
    pub strata_lift: f32,
    /// A line splitting the map in two: `climate`, `weather` and `tweaks` above
    /// hold west of it, its own east of it.
    pub divide: Option<ClimateDivide>,
}

/// A map split in two climates along a line from its south edge to its north
/// edge (a Precursor climate wall): the map's own climate and weather west of
/// the line, these east of it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(try_from = "DivideFile")]
pub struct ClimateDivide {
    /// The line, south to north, in map metres (x, y): `y` strictly ascending,
    /// 2 to `MAX_POINTS` points. Past its ends it runs on due south and north.
    pub line: Vec<(f32, f32)>,
    pub climate: Climate,
    pub weather: WeatherPreset,
    pub tweaks: WeatherTweaks,
}

/// `ClimateDivide` as a map's file writes it, before the line is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DivideFile {
    line: Vec<(f32, f32)>,
    #[serde(default)]
    climate: Climate,
    #[serde(default)]
    weather: WeatherPreset,
    #[serde(default)]
    tweaks: WeatherTweaks,
}

impl TryFrom<DivideFile> for ClimateDivide {
    type Error = DivideError;

    fn try_from(file: DivideFile) -> Result<ClimateDivide, DivideError> {
        let divide = ClimateDivide {
            line: file.line,
            climate: file.climate,
            weather: file.weather,
            tweaks: file.tweaks,
        };
        divide.validate()?;
        Ok(divide)
    }
}

/// Why a climate divide's line cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DivideError {
    /// It has this many points: fewer than 2 or more than `ClimateDivide::MAX_POINTS`.
    Points(usize),
    /// The point at this index is not north of the one before it.
    NotAscending(usize),
    /// The point at this index is not a number.
    NotFinite(usize),
}

impl std::fmt::Display for DivideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DivideError::Points(n) => write!(
                f,
                "a divide's line has 2 to {} points, not {n}",
                ClimateDivide::MAX_POINTS
            ),
            DivideError::NotAscending(i) => write!(
                f,
                "a divide's line runs south to north: point {i} is not north of point {}",
                i - 1
            ),
            DivideError::NotFinite(i) => write!(f, "a divide's line: point {i} is not a number"),
        }
    }
}

impl std::error::Error for DivideError {}

impl ClimateDivide {
    /// The most points a line may have (the shaders hold this many:
    /// mc-models `gpu_consts::divide::POINTS`).
    pub const MAX_POINTS: usize = 8;

    /// Whether the line can be used: 2 to `MAX_POINTS` points, each north of the last.
    pub fn validate(&self) -> Result<(), DivideError> {
        let n = self.line.len();
        if !(2..=ClimateDivide::MAX_POINTS).contains(&n) {
            return Err(DivideError::Points(n));
        }
        for (i, p) in self.line.iter().enumerate() {
            if !(p.0.is_finite() && p.1.is_finite()) {
                return Err(DivideError::NotFinite(i));
            }
            if i > 0 && p.1 <= self.line[i - 1].1 {
                return Err(DivideError::NotAscending(i));
            }
        }
        Ok(())
    }

    /// The line's x at `y`: straight between its points, and its end points' x
    /// south of the first and north of the last.
    pub fn x_at(&self, y: f32) -> f32 {
        let (Some(first), Some(last)) = (self.line.first(), self.line.last()) else {
            return 0.0;
        };
        if y <= first.1 {
            return first.0;
        }
        for pair in self.line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if y <= b.1 {
                return a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1);
            }
        }
        last.0
    }

    /// Whether (`x`, `y`) lies east of the line (a point on it does).
    pub fn east_of(&self, x: f32, y: f32) -> bool {
        x >= self.x_at(y)
    }

    /// How far (`x`, `y`) is from the line in metres, positive east of it and
    /// negative west (shaders/common.wgsl `divide_east_of` is the same).
    pub fn east_distance(&self, x: f32, y: f32) -> f32 {
        let (Some(first), Some(last)) = (self.line.first(), self.line.last()) else {
            return 0.0;
        };
        let offset = x - self.x_at(y);
        // Past its ends the line runs on due south and north.
        let mut nearest = if y < first.1 || y > last.1 {
            offset.abs()
        } else {
            f32::MAX
        };
        for pair in self.line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (ex, ey) = (b.0 - a.0, b.1 - a.1);
            let (px, py) = (x - a.0, y - a.1);
            let t = ((px * ex + py * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
            nearest = nearest.min((px - ex * t).hypot(py - ey * t));
        }
        if offset >= 0.0 {
            nearest
        } else {
            -nearest
        }
    }

    /// The share of a `width` by `height` metre map that lies east of the line.
    pub fn east_share(&self, width: f32, height: f32) -> f32 {
        const ROWS: usize = 64;
        let east: f32 = (0..ROWS)
            .map(|i| {
                let y = (i as f32 + 0.5) / ROWS as f32 * height;
                (width - self.x_at(y)).clamp(0.0, width)
            })
            .sum();
        east / (ROWS as f32 * width.max(1.0))
    }
}

/// How a map's ground and sea are drawn: what the renderer and the map
/// previews take from its `MapConfig`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapLook {
    /// The map's climate: all of it, or west of `divide`.
    pub climate: Climate,
    /// `MapConfig::strata_lift`.
    pub strata_lift: f32,
    pub divide: Option<ClimateDivide>,
}

impl MapLook {
    /// One climate over the whole map, as `MERIDIAN_CLIMATE` forces it.
    pub fn single(climate: Climate) -> MapLook {
        MapLook {
            climate,
            ..MapLook::default()
        }
    }

    /// The climate at a map position.
    pub fn climate_at(&self, x: f32, y: f32) -> Climate {
        match &self.divide {
            Some(d) if d.east_of(x, y) => d.climate,
            _ => self.climate,
        }
    }
}

/// The land a map is set in, as the map browser files it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Biome {
    Temperate,
    Alpine,
    Coastal,
    Tropical,
    Arctic,
    Desert,
    Wasteland,
}

impl Biome {
    pub fn label(self) -> &'static str {
        match self {
            Biome::Temperate => "Temperate",
            Biome::Alpine => "Alpine",
            Biome::Coastal => "Coastal",
            Biome::Tropical => "Tropical",
            Biome::Arctic => "Arctic",
            Biome::Desert => "Desert",
            Biome::Wasteland => "Wasteland",
        }
    }
}

/// How a map is laid out for its players.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MapStyle {
    /// One against one.
    Duel,
    /// Two sides, each with a shared front.
    Teams,
    /// Every start on its own: free for all.
    FreeForAll,
}

impl MapStyle {
    pub fn label(self) -> &'static str {
        match self {
            MapStyle::Duel => "1v1",
            MapStyle::Teams => "Teams",
            MapStyle::FreeForAll => "Free for All",
        }
    }
}

/// The palette a map's land and water are drawn in: the ground scans' own
/// temperate greens and grey-green sea, a bright tropical one (white coral
/// sand, lush green, turquoise shallows over sapphire depths), or canyon-country
/// desert (red-rock strata, pale caliche and red sand dotted with dark shrubs,
/// jade-to-cobalt reservoir water with a bleached bathtub ring).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Climate {
    #[default]
    Temperate,
    Tropical,
    Desert,
}

impl Climate {
    /// A climate by name, any case (`MERIDIAN_CLIMATE`, `SKY_CLIMATE`).
    pub fn from_name(name: &str) -> Option<Climate> {
        match name.trim().to_ascii_lowercase().as_str() {
            "temperate" => Some(Climate::Temperate),
            "tropical" | "tropic" => Some(Climate::Tropical),
            "desert" | "arid" => Some(Climate::Desert),
            _ => None,
        }
    }
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

    /// The map's biome: its own, or the one its climate suggests.
    pub fn biome(&self) -> Biome {
        self.biome.unwrap_or(match self.climate {
            Climate::Tropical => Biome::Tropical,
            Climate::Desert => Biome::Desert,
            Climate::Temperate => Biome::Temperate,
        })
    }

    /// How the map is played, for a map with `starts` start positions.
    pub fn style(&self, starts: usize) -> MapStyle {
        self.style.unwrap_or(if starts <= 2 {
            MapStyle::Duel
        } else {
            MapStyle::Teams
        })
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
    /// (from skirmish set-up) when the player picked one. On a map with a
    /// climate divide this is the weather west of it (`east_weather`).
    pub fn weather(&self, choice: Option<WeatherPreset>) -> Weather {
        if let Some(preset) = choice {
            return Weather::from(preset);
        }
        self.tweaks.apply(Weather::from(self.weather))
    }

    /// The weather east of the map's climate divide: the divide's preset with
    /// its tweaks. None on a map without one, and when the player picked a
    /// preset (`choice`), which plays over both sides.
    pub fn east_weather(&self, choice: Option<WeatherPreset>) -> Option<Weather> {
        let divide = self.divide.as_ref()?;
        if choice.is_some() {
            return None;
        }
        Some(divide.tweaks.apply(Weather::from(divide.weather)))
    }

    /// How the map's ground and sea are drawn.
    pub fn look(&self) -> MapLook {
        MapLook {
            climate: self.climate,
            strata_lift: self.strata_lift,
            divide: self.divide.clone(),
        }
    }

    /// The climate at a map position: the divide's east of its line.
    pub fn climate_at(&self, x: f32, y: f32) -> Climate {
        self.look().climate_at(x, y)
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
        assert_eq!(
            c.weather(Some(WeatherPreset::Clear)),
            Weather::from(WeatherPreset::Clear)
        );
        let empty = MapConfig::parse("()").unwrap();
        assert_eq!(empty.weather(None), Weather::default());
        assert_eq!(empty.climate, Climate::Temperate);
    }

    #[test]
    fn map_config_reads_a_climate() {
        let c = MapConfig::parse("(weather: Fair, climate: Tropical)").unwrap();
        assert_eq!(c.climate, Climate::Tropical);
        assert_eq!(Climate::from_name("TROPICAL"), Some(Climate::Tropical));
        let c = MapConfig::parse("(climate: Desert)").unwrap();
        assert_eq!((c.climate, c.biome()), (Climate::Desert, Biome::Desert));
        assert_eq!(Climate::from_name("arid"), Some(Climate::Desert));
    }

    #[test]
    fn map_config_files_a_map_for_the_browser() {
        let c = MapConfig::parse("(biome: Alpine, style: FreeForAll)").unwrap();
        assert_eq!(
            (c.biome(), c.style(8)),
            (Biome::Alpine, MapStyle::FreeForAll)
        );
        // Unset: from the climate and the start count.
        let c = MapConfig::parse("(climate: Tropical)").unwrap();
        assert_eq!(
            (c.biome(), c.style(2), c.style(8)),
            (Biome::Tropical, MapStyle::Duel, MapStyle::Teams)
        );
        // Every map's own file still reads.
        let maps = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
        for e in std::fs::read_dir(maps).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "mcmap") {
                MapConfig::for_map(&e.path()).unwrap();
            }
        }
    }

    const DIVIDED: &str = "(
        climate: Desert,
        weather: Clear,
        tweaks: (rain: 0.0, wind: 8),
        strata_lift: 50,
        divide: (
            line: [(6592, 0), (6592, 3400), (8192, 5000), (8192, 11384), (9792, 12984), (9792, 16384)],
            climate: Temperate,
            weather: Cloudy,
            tweaks: (rain: 0.7),
        ),
    )";

    #[test]
    fn map_config_reads_a_climate_divide() {
        let c = MapConfig::parse(DIVIDED).unwrap();
        assert_eq!(c.strata_lift, 50.0);
        let d = c.divide.as_ref().unwrap();
        assert_eq!(d.line.len(), 6);
        assert_eq!(
            (d.climate, d.weather),
            (Climate::Temperate, WeatherPreset::Cloudy)
        );
        // West of the line the map's own climate and weather, east the divide's.
        assert_eq!(c.climate_at(1000.0, 1000.0), Climate::Desert);
        assert_eq!(c.climate_at(12000.0, 1000.0), Climate::Temperate);
        assert_eq!(
            c.weather(None).cover,
            Weather::from(WeatherPreset::Clear).cover
        );
        let east = c.east_weather(None).unwrap();
        assert_eq!(east.rain, 0.7);
        assert_eq!(east.cover, Weather::from(WeatherPreset::Cloudy).cover);
        // A preset picked in skirmish set-up plays over both sides.
        assert_eq!(c.east_weather(Some(WeatherPreset::Stormy)), None);
        assert_eq!(
            c.weather(Some(WeatherPreset::Stormy)),
            Weather::from(WeatherPreset::Stormy)
        );
        let picked = SkyChoice {
            preset: Some(WeatherPreset::Overcast),
            time: None,
        };
        assert_eq!(picked.east_weather(&c), None);
        assert_eq!(SkyChoice::default().east_weather(&c), Some(east));
        // What the renderer and the previews take.
        let look = c.look();
        assert_eq!((look.climate, look.strata_lift), (Climate::Desert, 50.0));
        assert_eq!(look.climate_at(12000.0, 1000.0), Climate::Temperate);
        assert_eq!(
            MapLook::single(Climate::Tropical).climate_at(12000.0, 1000.0),
            Climate::Tropical
        );
        // A map without one: the defaults, and no east side.
        let plain = MapConfig::parse("(climate: Desert)").unwrap();
        assert_eq!((plain.strata_lift, plain.divide.is_none()), (0.0, true));
        assert_eq!(plain.east_weather(None), None);
        assert_eq!(plain.climate_at(1.0e6, 0.0), Climate::Desert);
    }

    #[test]
    fn a_divide_follows_its_line() {
        let d = MapConfig::parse(DIVIDED).unwrap().divide.unwrap();
        // On a point, between points, and clamped past both ends.
        assert_eq!(d.x_at(0.0), 6592.0);
        assert_eq!(d.x_at(-500.0), 6592.0);
        assert_eq!(d.x_at(3400.0), 6592.0);
        assert_eq!(d.x_at(4200.0), 7392.0);
        assert_eq!(d.x_at(8000.0), 8192.0);
        assert_eq!(d.x_at(12184.0), 8992.0);
        assert_eq!(d.x_at(20000.0), 9792.0);
        assert!(!d.east_of(7391.0, 4200.0) && d.east_of(7393.0, 4200.0));
        assert!(d.east_of(7392.0, 4200.0), "a point on the line is east");
        // Distance is across the line, not along x: shorter on the diagonal.
        assert!((d.east_distance(8292.0, 8000.0) - 100.0).abs() < 1e-3);
        assert!((d.east_distance(8092.0, 8000.0) + 100.0).abs() < 1e-3);
        let across = d.east_distance(7492.0, 4200.0);
        assert!(
            (across - 100.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-2,
            "{across}"
        );
        // Past an end the line runs straight on.
        assert!((d.east_distance(6692.0, -300.0) - 100.0).abs() < 1e-3);
        assert!((d.east_distance(9692.0, 17000.0) + 100.0).abs() < 1e-3);
        // Its sign is `east_of`'s everywhere, and it is never further than the
        // line is along x.
        for i in 0..40 {
            for j in 0..42 {
                let (x, y) = (i as f32 * 420.0, j as f32 * 420.0 - 400.0);
                let across = d.east_distance(x, y);
                assert_eq!(across >= 0.0, d.east_of(x, y), "{x}, {y}");
                assert!(across.abs() <= (x - d.x_at(y)).abs() + 1e-2, "{x}, {y}");
            }
        }
        // East of this line: half of a 16384 m square.
        let share = d.east_share(16384.0, 16384.0);
        assert!((share - 0.5).abs() < 0.02, "{share}");
    }

    #[test]
    fn a_bad_divide_line_is_refused() {
        let with_line = |line: &str| format!("(divide: (line: {line}, climate: Temperate))");
        // South to north: y strictly ascending.
        for line in [
            "[(100, 0), (100, 0)]",
            "[(100, 500), (100, 0)]",
            "[(0, 0), (10, 200), (20, 100), (30, 300)]",
        ] {
            let err = MapConfig::parse(&with_line(line)).unwrap_err().to_string();
            assert!(err.contains("south to north"), "{err}");
        }
        // 2 to 8 points.
        let nine: Vec<String> = (0..9).map(|i| format!("(100, {})", i * 100)).collect();
        for line in ["[]", "[(100, 0)]", &format!("[{}]", nine.join(", "))] {
            let err = MapConfig::parse(&with_line(line)).unwrap_err().to_string();
            assert!(err.contains("2 to 8 points"), "{err}");
        }
        let eight = format!("[{}]", nine[..8].join(", "));
        assert!(MapConfig::parse(&with_line(&eight)).is_ok());
        // A line is asked for, and nothing else is read into a divide.
        assert!(MapConfig::parse("(divide: (climate: Temperate))").is_err());
        assert!(MapConfig::parse("(divide: (line: [(0, 0), (0, 9)], snow: true))").is_err());
        // Checked the same way when built in code.
        let mut d = MapConfig::parse(&with_line("[(100, 0), (100, 900)]"))
            .unwrap()
            .divide
            .unwrap();
        assert_eq!(d.validate(), Ok(()));
        d.line[1].1 = -5.0;
        assert_eq!(d.validate(), Err(DivideError::NotAscending(1)));
        d.line.truncate(1);
        assert_eq!(d.validate(), Err(DivideError::Points(1)));
        d.line = vec![(0.0, 0.0), (f32::NAN, 5.0)];
        assert_eq!(d.validate(), Err(DivideError::NotFinite(1)));
    }

    #[test]
    fn a_sky_choice_saved_with_old_tweaks_still_loads() {
        let c: SkyChoice =
            ron::from_str("(preset: Some(Stormy), tweaks: (rain: Some(0.0)), time: None)").unwrap();
        assert_eq!(c.preset, Some(WeatherPreset::Stormy));
        assert_eq!(
            c.weather(&MapConfig::default()),
            Weather::from(WeatherPreset::Stormy)
        );
    }
}
