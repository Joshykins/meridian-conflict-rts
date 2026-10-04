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
//! A map can also be parted into regions (`crate::regions`), each with a climate and a
//! weather of its own, by climate walls. Such a map lists its regions in place of a
//! `climate`, `weather` and `tweaks` of its own:
//!
//! ```ron
//! (
//!     // Metres the desert's rock beds are lowered on this map: ground at height
//!     // h is coloured as Vermilion Gorge's is at h + strata_lift. Default 0.
//!     strata_lift: 50,
//!     // 2 to 8 regions. The first is region 0: its wind blows over the whole map,
//!     // and its climate files the map in the browser unless `biome` says.
//!     regions: [
//!         (name: "Desert", climate: Desert, weather: Clear, tweaks: (rain: 0.0, wind: 9)),
//!         (name: "Alaska", climate: Temperate, weather: Cloudy, tweaks: (rain: 0.7)),
//!     ],
//!     // Climate walls: each a line of two or more points in map metres (x, y; y
//!     // north) with the index of the region on its left and on its right hand,
//!     // walking from its first point to its last (for a line that runs south to
//!     // north the left hand is west). A wall ends on or beyond the map's edge, or
//!     // on another wall; where three regions meet, the walls end on one point.
//!     // At most 32 segments between them all.
//!     walls: [
//!         (line: [(6592, 0), (6592, 3400), (8192, 5000), (8192, 16384)], left: 0, right: 1),
//!     ],
//! )
//! ```
//!
//! Skirmish set-up picks a weather for each region by its name, or leaves it the
//! region's own (`SkyChoice`).

use crate::regions::{Region, RegionError, Wall, Walls, MAX_REGIONS};
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
    /// Banks of low cloud lying in the hollows and over the water, 0 none to 1
    /// thick; most at dawn, dusk and night, burning off toward midday.
    pub mist: f32,
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
                mist: 0.6,
            },
            WeatherPreset::Fair => Weather {
                cover: 1.2,
                storms: 1.0,
                rain: 0.5,
                towering: 0.5,
                scale: 1.0,
                wind: 12.0,
                lightning: 1.0,
                mist: 0.7,
            },
            WeatherPreset::Cloudy => Weather {
                cover: 1.32,
                storms: 1.2,
                rain: 0.6,
                towering: 0.8,
                scale: 1.5,
                wind: 14.0,
                lightning: 0.7,
                mist: 0.5,
            },
            WeatherPreset::Stormy => Weather {
                cover: 1.5,
                storms: 2.8,
                rain: 1.0,
                towering: 1.0,
                scale: 1.5,
                wind: 18.0,
                lightning: 2.0,
                mist: 0.15,
            },
            WeatherPreset::Overcast => Weather {
                cover: 1.9,
                storms: 0.6,
                rain: 0.8,
                towering: 0.3,
                scale: 2.2,
                wind: 10.0,
                lightning: 0.4,
                mist: 0.4,
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
            // Golden hour: the sun some 14 degrees up. Later (17.6) it lit the
            // land from 6 degrees and the scene went a murky green.
            TimeOfDay::Dusk => 17.0,
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
    pub mist: Option<f32>,
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
        set(&mut w.mist, self.mist);
        w
    }
}

/// The weather a player picked for a match (skirmish set-up, the test range):
/// a preset or the map's own, and the time of day. Either left at none plays
/// what the map asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkyChoice {
    /// The weather on a map without regions.
    pub preset: Option<WeatherPreset>,
    pub time: Option<TimeOfDay>,
    /// The weather of each region, on a map with regions.
    regions: RegionPicks,
}

/// A weather picked for each region of one map (`SkyChoice::set_region`). The picks
/// go by the regions' names: on a map whose regions are called anything else they
/// are not used, so a pick made for one map's "Alaska" never lands on another
/// map's second region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct RegionPicks {
    /// The regions they were picked for (`MapConfig::regions_key`).
    key: u64,
    picks: [Option<WeatherPreset>; MAX_REGIONS],
}

impl SkyChoice {
    /// The weather of each of `map`'s regions (`MapConfig::weathers`).
    pub fn weathers(&self, map: &MapConfig) -> Vec<Weather> {
        map.weathers(self)
    }

    pub fn hour(&self, map: &MapConfig) -> f32 {
        map.hour(self.time)
    }

    /// The weather picked for region `region` of `map`; none plays the region's own.
    pub fn region(&self, map: &MapConfig, region: usize) -> Option<WeatherPreset> {
        if self.regions.key != map.regions_key() {
            return None;
        }
        self.regions.picks.get(region).copied().flatten()
    }

    /// Picks the weather of region `region` of `map` (none: the region's own).
    /// Picks made for another map's regions are dropped.
    pub fn set_region(&mut self, map: &MapConfig, region: usize, pick: Option<WeatherPreset>) {
        let key = map.regions_key();
        if self.regions.key != key {
            self.regions = RegionPicks {
                key,
                picks: [None; MAX_REGIONS],
            };
        }
        if let Some(slot) = self.regions.picks.get_mut(region) {
            *slot = pick;
        }
    }
}

/// A map's own settings file, `maps/<stem>.ron`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(try_from = "MapConfigFile")]
pub struct MapConfig {
    /// The map's regions: one, the map's own climate and weather, on a map without
    /// regions; two or more on a map that lists them.
    regions: Vec<Region>,
    /// The climate walls between the regions; none with one region.
    walls: Walls,
    /// When in the day the map is played, unless skirmish set-up picks another.
    pub time: TimeOfDay,
    /// An exact hour instead of `time` (24-hour clock), for a map that wants one.
    pub hour: Option<f32>,
    /// Present on maps made for survival (`crate::survival`).
    pub survival: Option<crate::survival::SurvivalLayout>,
    /// What the land is like, for the map browser's filter. Unset: from the climate.
    pub biome: Option<Biome>,
    /// How the map is meant to be played, for the map browser's filter.
    /// Unset: a duel on two starts, teams on more.
    pub style: Option<MapStyle>,
    /// Metres the desert's rock beds are lowered on this map: ground at height
    /// `h` above the water is coloured as Vermilion Gorge's is at `h +
    /// strata_lift` (shaders/desert.wgsl).
    pub strata_lift: f32,
    /// Only playtest and dev builds list the map: a release build has no
    /// `playtest: true` map in any list (mc-game `setup::list_maps`).
    pub playtest: bool,
}

impl Default for MapConfig {
    fn default() -> MapConfig {
        MapConfig {
            regions: vec![Region::default()],
            walls: Walls::default(),
            time: TimeOfDay::default(),
            hour: None,
            survival: None,
            biome: None,
            style: None,
            strata_lift: 0.0,
            playtest: false,
        }
    }
}

/// `MapConfig` as a map's file writes it, before its regions and walls are checked.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct MapConfigFile {
    /// The map's own weather, tweaks to it and climate: on a map without regions.
    weather: Option<WeatherPreset>,
    tweaks: Option<WeatherTweaks>,
    climate: Option<Climate>,
    /// The map's regions, each with its own, and the walls between them.
    regions: Option<Vec<Region>>,
    walls: Vec<Wall>,
    time: TimeOfDay,
    hour: Option<f32>,
    survival: Option<crate::survival::SurvivalLayout>,
    biome: Option<Biome>,
    style: Option<MapStyle>,
    strata_lift: f32,
    playtest: bool,
}

impl TryFrom<MapConfigFile> for MapConfig {
    type Error = RegionError;

    fn try_from(file: MapConfigFile) -> Result<MapConfig, RegionError> {
        let regions = match file.regions {
            Some(regions) => {
                if !(2..=MAX_REGIONS).contains(&regions.len()) {
                    return Err(RegionError::Regions(regions.len()));
                }
                let own = [
                    ("climate", file.climate.is_some()),
                    ("weather", file.weather.is_some()),
                    ("tweaks", file.tweaks.is_some()),
                ];
                if let Some((field, _)) = own.into_iter().find(|(_, given)| *given) {
                    return Err(RegionError::OwnAndRegions(field));
                }
                regions
            }
            None => {
                if !file.walls.is_empty() {
                    return Err(RegionError::WallsWithoutRegions);
                }
                vec![Region {
                    name: String::new(),
                    climate: file.climate.unwrap_or_default(),
                    weather: file.weather.unwrap_or_default(),
                    tweaks: file.tweaks.unwrap_or_default(),
                }]
            }
        };
        Ok(MapConfig {
            walls: Walls::new(file.walls, regions.len())?,
            regions,
            time: file.time,
            hour: file.hour,
            survival: file.survival,
            biome: file.biome,
            style: file.style,
            strata_lift: file.strata_lift,
            playtest: file.playtest,
        })
    }
}

/// How a map's ground and sea are drawn: what the renderer and the map
/// previews take from its `MapConfig`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapLook {
    /// Each region's climate: one, the map's own, on a map without regions.
    climates: Vec<Climate>,
    /// `MapConfig::strata_lift`.
    pub strata_lift: f32,
    /// The climate walls between the regions.
    walls: Walls,
}

impl Default for MapLook {
    fn default() -> MapLook {
        MapLook::single(Climate::default())
    }
}

impl MapLook {
    /// One climate over the whole map, as `MERIDIAN_CLIMATE` forces it.
    pub fn single(climate: Climate) -> MapLook {
        MapLook {
            climates: vec![climate],
            strata_lift: 0.0,
            walls: Walls::default(),
        }
    }

    /// Each region's climate, region 0's (the map's own) first.
    pub fn climates(&self) -> &[Climate] {
        &self.climates
    }

    /// The climate walls between the regions: none on a map without regions.
    pub fn walls(&self) -> &Walls {
        &self.walls
    }

    /// The climate at a map position: its region's.
    pub fn climate_at(&self, x: f32, y: f32) -> Climate {
        self.climates[self.walls.region_at(x, y)]
    }

    /// Whether what falls from the clouds at a map position is snow, not rain: in a
    /// temperate region of a map with regions, if the map's file carries a snow layer
    /// (`snow_layer`). The alpine part of such a map is cold enough for it; any
    /// other map keeps its rain. (shaders/bindings.wgsl `snowfall_at`.)
    pub fn snows_at(&self, x: f32, y: f32, snow_layer: bool) -> bool {
        snow_layer && self.climates.len() > 1 && self.climate_at(x, y) == Climate::Temperate
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
    /// A city and its outskirts.
    Urban,
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
            Biome::Urban => "Urban",
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

    /// The map's regions: one, without a name, on a map without regions.
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// Whether the map is parted into regions, each with a weather to pick.
    pub fn has_regions(&self) -> bool {
        self.regions.len() > 1
    }

    /// The climate walls between the regions.
    pub fn walls(&self) -> &Walls {
        &self.walls
    }

    /// The map's climate: region 0's, on a map with regions.
    pub fn climate(&self) -> Climate {
        self.regions[0].climate
    }

    /// What the map's regions are called, as one number: what a `SkyChoice` keeps
    /// its picks by. 0 on a map without regions.
    fn regions_key(&self) -> u64 {
        if !self.has_regions() {
            return 0;
        }
        // FNV-1a over the names, each closed by a byte no name holds. (Not the
        // standard library's hasher: the number is kept in the settings file.)
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self
            .regions
            .iter()
            .flat_map(|r| r.name.bytes().chain(std::iter::once(0xff)))
        {
            hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash.max(1)
    }

    /// The map's biome: its own, or the one its climate suggests.
    pub fn biome(&self) -> Biome {
        self.biome.unwrap_or(match self.climate() {
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

    /// The weather to play each region in, region 0 first: the region's preset with
    /// its tweaks, or the preset `choice` (from skirmish set-up) picked for it. On a
    /// map without regions that is one weather, the map's own or `choice.preset`.
    /// The wind is the first weather's over the whole map.
    pub fn weathers(&self, choice: &SkyChoice) -> Vec<Weather> {
        if !self.has_regions() {
            return vec![choice
                .preset
                .map_or_else(|| self.regions[0].weather(), Weather::from)];
        }
        self.regions
            .iter()
            .enumerate()
            .map(|(i, region)| {
                choice
                    .region(self, i)
                    .map_or_else(|| region.weather(), Weather::from)
            })
            .collect()
    }

    /// How the map's ground and sea are drawn.
    pub fn look(&self) -> MapLook {
        MapLook {
            climates: self.regions.iter().map(|r| r.climate).collect(),
            strata_lift: self.strata_lift,
            walls: self.walls.clone(),
        }
    }

    /// The climate at a map position: its region's.
    pub fn climate_at(&self, x: f32, y: f32) -> Climate {
        self.regions[self.walls.region_at(x, y)].climate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one weather of a map without regions, as set-up left alone plays it.
    fn own_weather(c: &MapConfig) -> Weather {
        let weathers = c.weathers(&SkyChoice::default());
        assert_eq!(weathers.len(), 1);
        weathers[0]
    }

    #[test]
    fn map_config_reads_a_preset_and_tweaks() {
        let c = MapConfig::parse("(weather: Stormy, tweaks: (rain: 0.3))").unwrap();
        let w = own_weather(&c);
        assert_eq!(w.rain, 0.3);
        assert_eq!(w.storms, Weather::from(WeatherPreset::Stormy).storms);
        let clear = SkyChoice {
            preset: Some(WeatherPreset::Clear),
            ..SkyChoice::default()
        };
        assert_eq!(c.weathers(&clear), [Weather::from(WeatherPreset::Clear)]);
        let empty = MapConfig::parse("()").unwrap();
        assert_eq!(empty, MapConfig::default());
        assert_eq!(own_weather(&empty), Weather::default());
        assert_eq!(empty.climate(), Climate::Temperate);
        assert!(!empty.has_regions() && empty.walls().is_empty());
    }

    #[test]
    fn map_config_reads_a_climate() {
        let c = MapConfig::parse("(weather: Fair, climate: Tropical)").unwrap();
        assert_eq!(c.climate(), Climate::Tropical);
        assert_eq!(Climate::from_name("TROPICAL"), Some(Climate::Tropical));
        let c = MapConfig::parse("(climate: Desert)").unwrap();
        assert_eq!((c.climate(), c.biome()), (Climate::Desert, Biome::Desert));
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

    const REGIONS: &str = "(
        strata_lift: 50,
        regions: [
            (name: \"Desert\", climate: Desert, weather: Clear, tweaks: (rain: 0.0, wind: 8)),
            (name: \"Alaska\", climate: Temperate, weather: Cloudy, tweaks: (rain: 0.7)),
        ],
        walls: [
            (line: [(6592, 0), (6592, 3400), (8192, 5000), (8192, 11384), (9792, 12984), (9792, 16384)], left: 0, right: 1),
        ],
    )";

    #[test]
    fn map_config_reads_regions_and_walls() {
        let c = MapConfig::parse(REGIONS).unwrap();
        assert_eq!(c.strata_lift, 50.0);
        assert!(c.has_regions());
        let names: Vec<&str> = c.regions().iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Desert", "Alaska"]);
        assert_eq!(c.walls().walls()[0].line.len(), 6);
        assert_eq!((c.walls().regions(), c.walls().segments().len()), (2, 5));
        // Each region its own climate and weather; region 0's files the map.
        assert_eq!((c.climate(), c.biome()), (Climate::Desert, Biome::Desert));
        assert_eq!(c.climate_at(1000.0, 1000.0), Climate::Desert);
        assert_eq!(c.climate_at(12000.0, 1000.0), Climate::Temperate);
        let weathers = c.weathers(&SkyChoice::default());
        assert_eq!(weathers.len(), 2);
        assert_eq!(
            (weathers[0].cover, weathers[0].wind),
            (Weather::from(WeatherPreset::Clear).cover, 8.0)
        );
        assert_eq!(
            (weathers[1].cover, weathers[1].rain),
            (Weather::from(WeatherPreset::Cloudy).cover, 0.7)
        );
        // What the renderer and the previews take.
        let look = c.look();
        assert_eq!(look.climates(), [Climate::Desert, Climate::Temperate]);
        assert_eq!((look.strata_lift, look.walls()), (50.0, c.walls()));
        assert_eq!(look.climate_at(12000.0, 1000.0), Climate::Temperate);
        assert_eq!(
            MapLook::single(Climate::Tropical).climate_at(12000.0, 1000.0),
            Climate::Tropical
        );
        // Snow falls in the temperate region, if the map's file has a snow layer.
        assert!(look.snows_at(12000.0, 1000.0, true));
        assert!(!look.snows_at(12000.0, 1000.0, false));
        assert!(!look.snows_at(1000.0, 1000.0, true));
        assert!(!MapLook::default().snows_at(12000.0, 1000.0, true));
        // A map without regions: one climate everywhere, and no snow from the sky.
        let plain = MapConfig::parse("(climate: Desert)").unwrap();
        assert_eq!((plain.strata_lift, plain.regions().len()), (0.0, 1));
        assert_eq!(plain.climate_at(1.0e6, 0.0), Climate::Desert);
        assert_eq!(plain.look(), MapLook::single(Climate::Desert));
    }

    #[test]
    fn each_region_plays_the_weather_picked_for_it() {
        let c = MapConfig::parse(REGIONS).unwrap();
        let own = c.weathers(&SkyChoice::default());
        let mut sky = SkyChoice::default();
        assert_eq!((sky.region(&c, 0), sky.region(&c, 1)), (None, None));
        sky.set_region(&c, 1, Some(WeatherPreset::Stormy));
        assert_eq!(sky.region(&c, 1), Some(WeatherPreset::Stormy));
        assert_eq!(
            sky.weathers(&c),
            [own[0], Weather::from(WeatherPreset::Stormy)]
        );
        sky.set_region(&c, 0, Some(WeatherPreset::Overcast));
        sky.set_region(&c, 1, None);
        assert_eq!(
            sky.weathers(&c),
            [Weather::from(WeatherPreset::Overcast), own[1]]
        );
        // The preset for a map without regions is not a region's pick, and the
        // regions' picks are nothing to a map without regions.
        sky.preset = Some(WeatherPreset::Fair);
        assert_eq!(sky.weathers(&c)[1], own[1]);
        let plain = MapConfig::parse("(weather: Stormy)").unwrap();
        assert_eq!(sky.weathers(&plain), [Weather::from(WeatherPreset::Fair)]);
        assert_eq!(sky.region(&plain, 0), None);
        // Picks go by the regions' names: another map's regions do not take them,
        // and a pick made there starts that map's picks afresh.
        let other = MapConfig::parse(
            "(regions: [(name: \"Reef\", climate: Tropical), (name: \"Alaska\")],
              walls: [(line: [(0, 500), (900, 500)], left: 0, right: 1)])",
        )
        .unwrap();
        assert_eq!(sky.region(&other, 0), None);
        assert_eq!(other.weathers(&sky), other.weathers(&SkyChoice::default()));
        sky.set_region(&other, 1, Some(WeatherPreset::Clear));
        assert_eq!(
            (sky.region(&other, 0), sky.region(&other, 1)),
            (None, Some(WeatherPreset::Clear))
        );
        assert_eq!(sky.region(&c, 0), None);
        // It survives the settings file.
        let saved = ron::to_string(&sky).unwrap();
        assert_eq!(ron::from_str::<SkyChoice>(&saved).unwrap(), sky);
        // A region past the last a map may have is no pick.
        sky.set_region(&other, MAX_REGIONS, Some(WeatherPreset::Stormy));
        assert_eq!(sky.region(&other, MAX_REGIONS), None);
    }

    #[test]
    fn bad_regions_are_refused() {
        let error = |text: &str| MapConfig::parse(text).unwrap_err().to_string();
        let wall = "(line: [(100, 0), (100, 900)], left: 0, right: 1)";
        let region = |i: usize| format!("(name: \"R{i}\")");
        let regions = |n: usize| (0..n).map(region).collect::<Vec<_>>().join(", ");
        // 2 to 8 regions.
        for n in [0, 1, 9] {
            let err = error(&format!("(regions: [{}])", regions(n)));
            assert!(err.contains("2 to 8 regions"), "{err}");
        }
        assert!(MapConfig::parse(&format!("(regions: [{}], walls: [{wall}])", regions(8))).is_ok());
        // With regions the map has no climate, weather or tweaks of its own.
        for own in ["climate: Desert", "weather: Clear", "tweaks: (rain: 0.2)"] {
            let err = error(&format!(
                "({own}, regions: [{}], walls: [{wall}])",
                regions(2)
            ));
            assert!(err.contains("each region says its own"), "{err}");
        }
        // Walls part regions.
        let err = error(&format!("(climate: Desert, walls: [{wall}])"));
        assert!(err.contains("no regions"), "{err}");
        // A wall's line and its two hands (`regions::tests` has the rest).
        let with_wall = |wall: &str| format!("(regions: [{}], walls: [{wall}])", regions(2));
        for (wall, why) in [
            ("(line: [(100, 0)], left: 0, right: 1)", "2 points or more"),
            (
                "(line: [(1, 0), (1, 0)], left: 0, right: 1)",
                "is where point 0 is",
            ),
            ("(line: [(1, 0), (1, 9)], left: 1, right: 1)", "same region"),
            ("(line: [(1, 0), (1, 9)], left: 0, right: 2)", "no region 2"),
        ] {
            let err = error(&with_wall(wall));
            assert!(err.contains(why), "{err}");
        }
        // Both hands are asked for, and nothing else is read into a wall or a region.
        assert!(MapConfig::parse(&with_wall("(line: [(1, 0), (1, 9)], left: 0)")).is_err());
        assert!(MapConfig::parse(&with_wall(
            "(line: [(1, 0), (1, 9)], left: 0, right: 1, snow: true)"
        ))
        .is_err());
        assert!(MapConfig::parse("(regions: [(name: \"A\", snow: true), (name: \"B\")])").is_err());
    }

    #[test]
    fn a_sky_choice_saved_with_old_tweaks_still_loads() {
        let c: SkyChoice =
            ron::from_str("(preset: Some(Stormy), tweaks: (rain: Some(0.0)), time: None)").unwrap();
        assert_eq!(c.preset, Some(WeatherPreset::Stormy));
        assert_eq!(
            c.weathers(&MapConfig::default()),
            [Weather::from(WeatherPreset::Stormy)]
        );
    }
}
