//! The weather rows shared by skirmish set-up and the test range: a preset
//! and the time of day, each a list whose "Map Default" leaves it to the map.

use super::{id, palette, rgb, type_scale, Rect, Ui};
use mc_data::weather::{SkyChoice, TimeOfDay, WeatherPreset};

/// How the rows are drawn: set-up's full rows or the range panel's compact ones.
#[derive(Clone, Copy)]
pub struct Look {
    pub row_h: f32,
    /// Row pitch, row height plus the gap under it.
    pub pitch: f32,
    pub value_w: f32,
    pub compact: bool,
}

/// How many rows `rows` draws.
pub const ROWS: usize = 2;

/// Draws the rows from `y` down; returns true when anything changed. Each
/// value's arrows step it; clicking the value opens its list (`Ui::popups`
/// draws it, so the screen must call that last).
pub fn rows(ui: &mut Ui, tag: usize, x: f32, y: f32, w: f32, look: Look, sky: &mut SkyChoice) -> bool {
    let before = *sky;
    let presets: Vec<Option<WeatherPreset>> = std::iter::once(None).chain(WeatherPreset::ALL.map(Some)).collect();
    let times: Vec<Option<TimeOfDay>> = std::iter::once(None).chain(TimeOfDay::ALL.map(Some)).collect();
    for k in 0..ROWS {
        let r = Rect::new(x, y + k as f32 * look.pitch, w, look.row_h);
        let label = ["Weather", "Time of Day"][k];
        if look.compact {
            ui.text(r.x, r.mid_y(), type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        } else {
            ui.hline(r.x, r.bottom() + (look.pitch - look.row_h) * 0.5, r.w, rgb(palette::LINE, 0.10));
            ui.text(r.x + 16.0, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82), label);
        }
        let h = look.row_h.min(32.0);
        let at = Rect::new(r.right() - look.value_w, r.mid_y() - h * 0.5, look.value_w, h);
        let (options, selected): (Vec<&str>, usize) = match k {
            0 => (
                presets.iter().map(|p| p.map_or("Map Default", |p| p.label())).collect(),
                presets.iter().position(|p| *p == sky.preset).unwrap_or(0),
            ),
            _ => (
                times.iter().map(|t| t.map_or("Map Default", |t| t.label())).collect(),
                times.iter().position(|t| *t == sky.time).unwrap_or(0),
            ),
        };
        let Some(i) = ui.dropdown(id("sky-row", tag * 8 + k), at, &options, selected, true) else {
            continue;
        };
        match k {
            0 => sky.preset = presets[i],
            _ => sky.time = times[i],
        }
    }
    *sky != before
}
