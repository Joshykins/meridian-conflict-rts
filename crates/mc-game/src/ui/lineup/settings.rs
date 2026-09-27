//! Match settings, out of the way: a slim bar over the chart names the map and
//! sums up the rules, and its Settings button opens a sheet with all of them
//! (the theatre, the rules, the sky, your callsign). Skirmish, the lobby and
//! survival all use it, so their set-up screens keep their room for the chart
//! and the commanders.

use super::super::maps::{Browser, MapCard};
use super::super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;

/// Height of the bar over the chart, and the gap under it.
pub const BAR_H: f32 = 64.0;
pub const BAR_GAP: f32 = 22.0;

/// The settings sheet: open or not, and how far it has faded in.
#[derive(Default)]
pub struct Sheet {
    open: bool,
    shown: f32,
}

/// What the bar was asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarAsk {
    ChangeMap,
    Settings,
}

impl Sheet {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    /// Draws the sheet over the screen when it is open: `title` over a body
    /// that `body` fills in the rect it is given, and a Done at the foot.
    /// `live` is false while something (the map browser, a race picker) lies
    /// over the sheet. The screen under it must be drawn with `ui.interactive`
    /// off while `is_open`.
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        title: &str,
        size: Vec2,
        live: bool,
        body: impl FnOnce(&mut Ui, Rect),
    ) {
        self.shown = if self.open {
            (self.shown + ui.dt * 6.0).min(1.0)
        } else {
            (self.shown - ui.dt * 8.0).max(0.0)
        };
        if self.shown <= 0.0 {
            return;
        }
        let k = 1.0 - (1.0 - self.shown) * (1.0 - self.shown);
        let (fade, shift, interactive) = (ui.fade, ui.shift, ui.interactive);
        ui.fade = fade * k;
        ui.shift = Vec2::new(0.0, 16.0 * (1.0 - k));
        ui.interactive = self.open && live;
        let (w, h) = (ui.size.x, ui.size.y);
        ui.fill(Rect::new(-ui.shift.x, -ui.shift.y, w, h), ink(0.6));
        let pw = size.x.min(w - 96.0);
        let ph = size.y.min(h - 96.0);
        let panel = Rect::new((w - pw) * 0.5, (h - ph) * 0.5, pw, ph);
        ui.panel(panel);
        let inner = panel.inset(36.0);
        ui.text(
            inner.x,
            inner.y + 14.0,
            type_scale::TITLE,
            rgb(0xFFFFFF, 1.0),
            title,
        );
        ui.fill(
            Rect::new(inner.x, inner.y + 46.0, 58.0, 2.0),
            rgb(palette::ACCENT, 1.0),
        );
        ui.gradient_h(
            Rect::new(inner.x + 66.0, inner.y + 46.0, inner.w - 66.0, 1.0),
            rgb(palette::LINE, 0.35),
            rgb(palette::LINE, 0.04),
        );
        body(
            ui,
            Rect::new(inner.x, inner.y + 76.0, inner.w, inner.h - 76.0 - 64.0),
        );
        let done = ui.button(
            id("sheet-done", 0),
            Rect::new(inner.right() - 220.0, inner.bottom() - 46.0, 220.0, 46.0),
            "Done",
            ButtonKind::Primary,
            true,
        );
        // After the body, so its controls take the pointer first: the panel
        // itself swallows clicks, and a click on the dim around it puts it away.
        let inside = ui.interact_with(id("sheet-panel", 0), panel, true, false);
        let outside = ui.interact_with(
            id("sheet-outside", 0),
            Rect::new(0.0, 0.0, w, h),
            true,
            false,
        );
        let typing = ui.mem.editing.is_some();
        let listing = ui.mem.popup.is_some();
        let away = outside.clicked && !inside.clicked && !listing;
        if self.open
            && ui.interactive
            && (done || away || (ui.input.key(Key::Escape) && !typing && !listing))
        {
            ui.audio.play(Sfx::Back);
            self.open = false;
        }
        ui.fade = fade;
        ui.shift = shift;
        ui.interactive = interactive;
    }
}

/// The sheet's two columns: the theatre on the left, the rules on the right.
pub fn sheet_columns(body: Rect) -> (Rect, Rect) {
    let gap = 48.0;
    let left_w = ((body.w - gap) * 0.5).min(420.0);
    (
        Rect::new(body.x, body.y, left_w, body.h),
        Rect::new(body.x + left_w + gap, body.y, body.w - left_w - gap, body.h),
    )
}

/// The sheet's usual size.
pub const SHEET: Vec2 = Vec2::new(1000.0, 540.0);

/// The bar over the chart in `r`: the map's thumbnail, name and facts, then
/// `chips` (a word on each setting) as far as they fit, then Change Map (for
/// whoever may plan) and Settings.
pub fn bar(
    ui: &mut Ui,
    r: Rect,
    browser: &mut Browser,
    maps: &[MapCard],
    selected: usize,
    chips: &[String],
    host: bool,
) -> Option<BarAsk> {
    ui.fill(r, ink(0.5));
    ui.gradient_h(r, rgb(palette::ACCENT, 0.10), rgb(palette::ACCENT, 0.0));
    ui.frame(r, rgb(palette::LINE, 0.14));
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(palette::ACCENT, 1.0));
    let mut ask = None;

    // The buttons from the right.
    let bh = 38.0;
    let by = r.mid_y() - bh * 0.5;
    let settings = Rect::new(r.right() - 12.0 - 120.0, by, 120.0, bh);
    if ui.button(
        id("bar-settings", 0),
        settings,
        "Settings",
        ButtonKind::Secondary,
        true,
    ) {
        ui.audio.play(Sfx::Select);
        ask = Some(BarAsk::Settings);
    }
    let mut right = settings.x - 10.0;
    if host {
        let change = Rect::new(right - 136.0, by, 136.0, bh);
        if ui.button(
            id("bar-change-map", 0),
            change,
            "Change Map",
            ButtonKind::Secondary,
            true,
        ) {
            ui.audio.play(Sfx::Select);
            ask = Some(BarAsk::ChangeMap);
        }
        right = change.x - 10.0;
    }

    // The map on the left; a click on it opens the browser too.
    let Some(m) = maps.get(selected) else {
        ui.text(
            r.x + 20.0,
            r.mid_y(),
            type_scale::BODY,
            rgb(palette::WARN, 1.0),
            "No maps in maps/",
        );
        return ask;
    };
    let side = r.h - 14.0;
    let thumb = Rect::new(r.x + 12.0, r.y + 7.0, side, side);
    browser.thumb(ui, selected, thumb, 1.0);
    let x = thumb.right() + 16.0;
    // The name and facts take what the chips leave them, but never less than this.
    let text_min = 160.0;
    let name_w = ui
        .text_width(type_scale::ITEM, &m.name)
        .max(ui.text_width(type_scale::MICRO, &facts(m)))
        .clamp(text_min, 360.0);
    let map_hit = Rect::new(r.x, r.y, (x + name_w - r.x).min(right - r.x), r.h);
    let res = ui.interact(id("bar-map", 0), map_hit, host);
    if res.clicked && host {
        ui.audio.play(Sfx::Select);
        ask = Some(BarAsk::ChangeMap);
    }
    let text_w = (right - x - 12.0).clamp(0.0, name_w);
    ui.text_fit_left(
        x,
        r.mid_y() - 10.0,
        text_w,
        type_scale::ITEM,
        rgb(palette::ACCENT, 0.9 + 0.1 * res.glow),
        &m.name,
    );
    ui.text_fit_left(
        x + 1.0,
        r.mid_y() + 13.0,
        text_w,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &facts(m),
    );

    // Chips, right to left from the buttons, as many as fit whole.
    let mut cx = right - 6.0;
    let floor = x + text_w + 20.0;
    for chip in chips.iter().rev() {
        let cw = ui.text_width(type_scale::CAPTION, chip) + 24.0;
        if cx - cw < floor {
            break;
        }
        let c = Rect::new(cx - cw, r.mid_y() - 14.0, cw, 28.0);
        ui.fill(c, ink(0.4));
        ui.frame(c, rgb(palette::LINE, 0.16));
        ui.text(
            c.x + 12.0,
            c.mid_y(),
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.85),
            chip,
        );
        cx = c.x - 8.0;
    }
    ask
}

/// The map's facts under its name in the bar.
fn facts(m: &MapCard) -> String {
    format!(
        "{:.0} km  \u{b7}  {} Players  \u{b7}  {}  \u{b7}  {}",
        m.km,
        m.starts,
        m.style.label(),
        m.biome.label()
    )
}

/// The bar's chip for the sky.
pub fn sky_chip(sky: &mc_data::weather::SkyChoice) -> String {
    match (sky.preset, sky.time) {
        (None, None) => "Map's Own Sky".to_owned(),
        (Some(p), None) => p.label().to_owned(),
        (None, Some(t)) => t.label().to_owned(),
        (Some(p), Some(t)) => format!("{}  \u{b7}  {}", p.label(), t.label()),
    }
}

/// The bar's chip for fog of war.
pub fn fog_chip(fog: bool) -> String {
    if fog { "Fog of War" } else { "No Fog" }.to_owned()
}

/// The bar's chip for a seed.
pub fn seed_chip(seed: u64) -> String {
    format!("Seed {:04X}-{:04X}", seed >> 16 & 0xFFFF, seed & 0xFFFF)
}

/// Where the bar goes over a chart in `centre`, and the chart's area under it
/// (`below` is the chart's caption). The bar spans the centre, but no more than
/// a little wider than the chart on a very wide screen.
pub fn over_chart(centre: Rect, below: f32) -> (Rect, Rect) {
    let chart = Rect::new(
        centre.x,
        centre.y + BAR_H + BAR_GAP,
        centre.w,
        centre.h - BAR_H - BAR_GAP,
    );
    let side = chart.w.min(chart.h - below);
    let w = (side + 400.0).min(centre.w);
    let bar = Rect::new(centre.x + (centre.w - w) * 0.5, centre.y, w, BAR_H);
    (bar, chart)
}
