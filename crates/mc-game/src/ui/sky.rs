//! The weather rows shared by skirmish set-up and the test range: a preset
//! and the time of day, each a list whose "Map Default" leaves it to the map.
//! On a map with regions every region has a weather row of its own.

use super::{id, palette, rgb, type_scale, Rect, Ui};
use mc_data::weather::{MapConfig, SkyChoice, TimeOfDay, WeatherPreset};

/// How the rows are drawn: set-up's full rows or the range panel's compact ones.
#[derive(Clone, Copy)]
pub struct Look {
    pub row_h: f32,
    /// Row pitch, row height plus the gap under it.
    pub pitch: f32,
    pub value_w: f32,
    pub compact: bool,
}

/// How many rows `rows` draws for `map`: its weather (one for each region of a map
/// with regions), then the time of day.
pub fn row_count(map: &MapConfig) -> usize {
    map.regions().len() + 1
}

/// What a weather row is called: "Weather", or "Weather: Alaska" for a region.
fn weather_label(map: &MapConfig, region: usize) -> String {
    match map.regions().get(region) {
        Some(r) if map.has_regions() => format!("Weather: {}", r.name),
        _ => "Weather".to_owned(),
    }
}

/// Draws the rows for `map` from `y` down; returns true when anything changed.
/// Each value's arrows step it; clicking the value opens its list (`Ui::popups`
/// draws it, so the screen must call that last).
#[expect(
    clippy::too_many_arguments,
    reason = "where the rows go, how they look, and the map and choice they show"
)]
pub fn rows(
    ui: &mut Ui,
    tag: usize,
    x: f32,
    y: f32,
    w: f32,
    look: Look,
    map: &MapConfig,
    sky: &mut SkyChoice,
) -> bool {
    let before = *sky;
    let presets: Vec<Option<WeatherPreset>> = std::iter::once(None)
        .chain(WeatherPreset::ALL.map(Some))
        .collect();
    let times: Vec<Option<TimeOfDay>> = std::iter::once(None)
        .chain(TimeOfDay::ALL.map(Some))
        .collect();
    let weathers = row_count(map) - 1;
    for k in 0..=weathers {
        let r = Rect::new(x, y + k as f32 * look.pitch, w, look.row_h);
        let label = if k < weathers {
            weather_label(map, k)
        } else {
            "Time of Day".to_owned()
        };
        if look.compact {
            ui.text_fit_left(
                r.x,
                r.mid_y(),
                r.w - look.value_w - 6.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &label,
            );
        } else {
            ui.hline(
                r.x,
                r.bottom() + (look.pitch - look.row_h) * 0.5,
                r.w,
                rgb(palette::LINE, 0.10),
            );
            ui.text_fit_left(
                r.x + 16.0,
                r.mid_y(),
                r.w - look.value_w - 28.0,
                type_scale::BODY,
                rgb(palette::TEXT, 0.82),
                &label,
            );
        }
        let h = look.row_h.min(32.0);
        let at = Rect::new(
            r.right() - look.value_w,
            r.mid_y() - h * 0.5,
            look.value_w,
            h,
        );
        // What the row shows now: a region's pick, the whole map's, or the hour.
        let picked = if k == weathers {
            None
        } else if map.has_regions() {
            sky.region(map, k)
        } else {
            sky.preset
        };
        let (options, selected): (Vec<&str>, usize) = if k < weathers {
            (
                presets
                    .iter()
                    .map(|p| p.map_or("Map Default", |p| p.label()))
                    .collect(),
                presets.iter().position(|p| *p == picked).unwrap_or(0),
            )
        } else {
            (
                times
                    .iter()
                    .map(|t| t.map_or("Map Default", |t| t.label()))
                    .collect(),
                times.iter().position(|t| *t == sky.time).unwrap_or(0),
            )
        };
        let Some(i) = ui.dropdown(id("sky-row", tag * 16 + k), at, &options, selected, true) else {
            continue;
        };
        if k == weathers {
            sky.time = times[i];
        } else if map.has_regions() {
            sky.set_region(map, k, presets[i]);
        } else {
            sky.preset = presets[i];
        }
    }
    *sky != before
}

/// The sky in a few words, for a card's chip and the set-up's log: what was picked
/// for `map`, or that the map's own is left alone.
pub fn summary(sky: &SkyChoice, map: &MapConfig) -> String {
    let weather = if map.has_regions() {
        let picks: Vec<Option<WeatherPreset>> = (0..map.regions().len())
            .map(|i| sky.region(map, i))
            .collect();
        match picks.as_slice() {
            picks if picks.iter().all(Option::is_none) => None,
            // One weather picked for every region reads as on any map.
            [Some(first), rest @ ..] if rest.iter().all(|p| *p == Some(*first)) => {
                Some(first.label().to_owned())
            }
            // The regions differ: each one's pick, in the regions' order ("Own"
            // where the map's own plays).
            picks => Some(
                picks
                    .iter()
                    .map(|p| p.map_or("Own", |p| p.label()))
                    .collect::<Vec<_>>()
                    .join(" / "),
            ),
        }
    } else {
        sky.preset.map(|p| p.label().to_owned())
    };
    match (weather, sky.time) {
        (None, None) => "Map's Own Sky".to_owned(),
        (Some(w), None) => w,
        (None, Some(t)) => t.label().to_owned(),
        (Some(w), Some(t)) => format!("{w}  \u{b7}  {}", t.label()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Input, Memory};
    use glam::Vec2;
    use mc_render::Overlay;

    const CANVAS: Vec2 = Vec2::new(1920.0, 1080.0);
    /// Set-up's rows, 600 wide from (100, 100).
    const LOOK: Look = Look {
        row_h: 42.0,
        pitch: 46.0,
        value_w: 220.0,
        compact: false,
    };

    fn regions() -> MapConfig {
        MapConfig::parse(
            "(regions: [(name: \"Desert\", climate: Desert), (name: \"Alaska\")],
              walls: [(line: [(0, 500), (900, 500)], left: 0, right: 1)])",
        )
        .unwrap()
    }

    /// The rows for `map` drawn alone, for one frame of `input`.
    fn frame(map: &MapConfig, sky: &mut SkyChoice, memory: &mut Memory, input: &Input) -> bool {
        let audio = crate::audio::Audio::silent();
        let mut overlay = Overlay::default();
        memory.begin_frame();
        let mut ui = Ui::new(
            &mut overlay,
            input,
            memory,
            &audio,
            CANVAS,
            1.0,
            1.0,
            1.0 / 30.0,
        );
        let changed = rows(&mut ui, 0, 100.0, 100.0, 600.0, LOOK, map, sky);
        ui.popups();
        memory.end_frame(input);
        changed
    }

    /// A click at `at`, then a frame at rest; true if any frame changed the choice.
    fn click(map: &MapConfig, sky: &mut SkyChoice, memory: &mut Memory, at: Vec2) -> bool {
        let input = |down: bool, released: bool| Input {
            cursor: at,
            down,
            pressed: down,
            released,
            ..Default::default()
        };
        let mut changed = false;
        for input in [
            input(false, false),
            input(true, false),
            input(false, true),
            input(false, false),
        ] {
            changed |= frame(map, sky, memory, &input);
        }
        changed
    }

    /// Opens row `row`'s list by the middle of its value and picks the list's row `pick`.
    fn pick(map: &MapConfig, sky: &mut SkyChoice, row: usize, pick: usize) -> bool {
        let mut memory = Memory::default();
        let value = Vec2::new(100.0 + 600.0 - 110.0, 100.0 + row as f32 * 46.0 + 21.0);
        assert!(
            !click(map, sky, &mut memory, value),
            "opening a list picks nothing"
        );
        let at = memory
            .popup
            .as_ref()
            .expect("the row's list is open")
            .row_centre(pick, CANVAS);
        click(map, sky, &mut memory, at)
    }

    #[test]
    fn each_region_has_a_weather_row_of_its_own() {
        let map = regions();
        let mut sky = SkyChoice::default();
        // The second row is Alaska's: Map Default, Clear, Fair, Cloudy, Stormy...
        assert!(pick(&map, &mut sky, 1, 4));
        assert_eq!(
            (sky.region(&map, 0), sky.region(&map, 1)),
            (None, Some(WeatherPreset::Stormy))
        );
        assert!(pick(&map, &mut sky, 0, 1));
        assert_eq!(sky.region(&map, 0), Some(WeatherPreset::Clear));
        // ...and neither is the preset a map without regions plays.
        assert_eq!(sky.preset, None);
        // The time of day comes after the regions.
        assert!(pick(&map, &mut sky, 2, 1));
        assert_eq!(sky.time, Some(TimeOfDay::Dawn));
        // Map Default hands a region back to the map.
        assert!(pick(&map, &mut sky, 1, 0));
        assert_eq!(sky.region(&map, 1), None);
        // On a map without regions the first row is the whole map's weather and the
        // second the time of day; the regions' picks are left as they were.
        let plain = MapConfig::default();
        assert!(pick(&plain, &mut sky, 0, 5));
        assert!(pick(&plain, &mut sky, 1, 3));
        assert_eq!(
            (sky.preset, sky.time),
            (Some(WeatherPreset::Overcast), Some(TimeOfDay::Noon))
        );
        assert_eq!(sky.region(&map, 0), Some(WeatherPreset::Clear));
    }

    #[test]
    fn the_summary_says_when_regions_differ() {
        let plain = MapConfig::default();
        let regions = regions();
        assert_eq!((row_count(&plain), row_count(&regions)), (2, 3));
        assert_eq!(weather_label(&plain, 0), "Weather");
        assert_eq!(weather_label(&regions, 1), "Weather: Alaska");
        let mut sky = SkyChoice::default();
        assert_eq!(summary(&sky, &regions), "Map's Own Sky");
        sky.set_region(&regions, 1, Some(WeatherPreset::Stormy));
        assert_eq!(summary(&sky, &regions), "Own / Stormy");
        // A map without regions hears nothing of them.
        assert_eq!(summary(&sky, &plain), "Map's Own Sky");
        sky.set_region(&regions, 0, Some(WeatherPreset::Clear));
        sky.time = Some(TimeOfDay::Dusk);
        assert_eq!(summary(&sky, &regions), "Clear / Stormy  \u{b7}  Dusk");
        sky.set_region(&regions, 0, Some(WeatherPreset::Stormy));
        assert_eq!(summary(&sky, &regions), "Stormy  \u{b7}  Dusk");
        // The whole-map preset is a plain map's, not a region's.
        sky.preset = Some(WeatherPreset::Overcast);
        assert_eq!(summary(&sky, &plain), "Overcast  \u{b7}  Dusk");
        assert_eq!(summary(&sky, &regions), "Stormy  \u{b7}  Dusk");
    }
}
