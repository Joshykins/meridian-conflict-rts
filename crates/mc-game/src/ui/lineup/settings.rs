//! Match settings, out of the way: a card at the top of the left column names
//! the map and sums up the rules, and its Settings button opens a sheet with
//! all of them (the theatre, the rules, the sky, your callsign). The chat runs
//! under the card. Skirmish, survival and the lobby all use it, so their set-up
//! screens keep the middle for the chart.

use super::super::maps::{Browser, MapCard};
use super::super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;

/// The settings sheet: open or not, and how far it has faded in.
#[derive(Default)]
pub struct Sheet {
    open: bool,
    shown: f32,
}

/// What the match card was asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardAsk {
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
        if self.draw_with(ui, title, size, live, ("Done", true), body) {
            ui.audio.play(Sfx::Back);
            self.open = false;
        }
    }

    /// [`Self::draw`] with `action` (a label, and whether it may be clicked) in
    /// place of Done: true the frame it is clicked, and the sheet stays open.
    /// Escape and a click outside still put it away.
    pub fn draw_with(
        &mut self,
        ui: &mut Ui,
        title: &str,
        size: Vec2,
        live: bool,
        action: (&str, bool),
        body: impl FnOnce(&mut Ui, Rect),
    ) -> bool {
        self.shown = if self.open {
            (self.shown + ui.dt * 6.0).min(1.0)
        } else {
            (self.shown - ui.dt * 8.0).max(0.0)
        };
        if self.shown <= 0.0 {
            return false;
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
            action.0,
            ButtonKind::Primary,
            action.1,
        ) && action.1;
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
        let acted = self.open && ui.interactive && done;
        if self.open
            && ui.interactive
            && !done
            && (away || (ui.input.key(Key::Escape) && !typing && !listing))
        {
            ui.audio.play(Sfx::Back);
            self.open = false;
        }
        ui.fade = fade;
        ui.shift = shift;
        ui.interactive = interactive;
        acted
    }

    pub fn close(&mut self) {
        self.open = false;
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

/// The match card in the width of `r`, from its top: the map's thumbnail,
/// name and facts, then `chips` (a word on each setting) in as many lines as
/// they need, then Change Map (for whoever may plan) and Settings. Returns
/// what was asked and the card's height.
pub fn card(
    ui: &mut Ui,
    r: Rect,
    browser: &mut Browser,
    maps: &[MapCard],
    selected: usize,
    chips: &[String],
    host: bool,
) -> (Option<CardAsk>, f32) {
    let mut ask = None;
    let side = (r.w * 0.3).clamp(72.0, 104.0);
    let head = Rect::new(r.x, r.y, r.w, side + 20.0);
    // The map: a click on it opens the browser too.
    let res = ui.interact(id("card-map", 0), head, host && !maps.is_empty());
    ui.fill(head, ink(0.5));
    ui.gradient_h(
        head,
        rgb(palette::ACCENT, 0.12 + 0.08 * res.glow),
        rgb(palette::ACCENT, 0.0),
    );
    ui.frame(head, rgb(palette::LINE, 0.14 + 0.2 * res.glow));
    ui.fill(
        Rect::new(head.x, head.y, 3.0, head.h),
        rgb(palette::ACCENT, 1.0),
    );
    if res.clicked && host {
        ui.audio.play(Sfx::Select);
        ask = Some(CardAsk::ChangeMap);
    }
    match maps.get(selected) {
        Some(m) => {
            let thumb = Rect::new(head.x + 12.0, head.y + 10.0, side, side);
            browser.thumb(ui, selected, thumb, 0.92 + 0.08 * res.glow);
            let x = thumb.right() + 14.0;
            let w = head.right() - x - 10.0;
            ui.text_fit_left(
                x,
                head.y + 26.0,
                w,
                type_scale::ITEM,
                rgb(palette::ACCENT, 0.9 + 0.1 * res.glow),
                &m.name,
            );
            let lines = [
                format!(
                    "{:.0} km  \u{b7}  {} Players  \u{b7}  {}",
                    m.km,
                    m.starts,
                    m.size_class().label()
                ),
                format!("{}  \u{b7}  {}", m.style.label(), m.biome.label()),
                format!("{} Ore Fields", m.ores),
            ];
            let pitch = ((side - 30.0) / 3.0).min(19.0);
            for (k, line) in lines.iter().enumerate() {
                ui.text_fit_left(
                    x + 1.0,
                    head.y + 52.0 + k as f32 * pitch,
                    w,
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    line,
                );
            }
        }
        None => {
            ui.text(
                head.x + 20.0,
                head.mid_y(),
                type_scale::BODY,
                rgb(palette::WARN, 1.0),
                "No maps in maps/",
            );
        }
    }

    // The chips, in lines across the card's width.
    let (chip_h, gap) = (26.0, 6.0);
    let mut y = head.bottom() + 10.0;
    let mut x = r.x;
    for chip in chips {
        let cw = (ui.text_width(type_scale::MICRO, chip) + 20.0).min(r.w);
        if x > r.x && x + cw > r.right() {
            x = r.x;
            y += chip_h + gap;
        }
        let c = Rect::new(x, y, cw, chip_h);
        ui.fill(c, ink(0.4));
        ui.frame(c, rgb(palette::LINE, 0.16));
        ui.text_fit_left(
            c.x + 10.0,
            c.mid_y(),
            cw - 20.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.85),
            chip,
        );
        x = c.right() + gap;
    }
    if !chips.is_empty() {
        y += chip_h + 10.0;
    }

    // The buttons, side by side across the card.
    let bh = 38.0;
    let settings = if host {
        let half = (r.w - 10.0) * 0.5;
        if ui.button(
            id("card-change-map", 0),
            Rect::new(r.x, y, half, bh),
            "Change Map",
            ButtonKind::Secondary,
            !maps.is_empty(),
        ) {
            ui.audio.play(Sfx::Select);
            ask = Some(CardAsk::ChangeMap);
        }
        Rect::new(r.x + half + 10.0, y, half, bh)
    } else {
        Rect::new(r.x, y, r.w, bh)
    };
    if ui.button(
        id("card-settings", 0),
        settings,
        if host { "Settings" } else { "Match Settings" },
        ButtonKind::Secondary,
        true,
    ) {
        ui.audio.play(Sfx::Select);
        ask = Some(CardAsk::Settings);
    }
    (ask, y + bh - r.y)
}

/// The card's chip for the sky.
pub fn sky_chip(sky: &mc_data::weather::SkyChoice) -> String {
    match (sky.preset, sky.time) {
        (None, None) => "Map's Own Sky".to_owned(),
        (Some(p), None) => p.label().to_owned(),
        (None, Some(t)) => t.label().to_owned(),
        (Some(p), Some(t)) => format!("{}  \u{b7}  {}", p.label(), t.label()),
    }
}

/// The card's chip for fog of war.
pub fn fog_chip(fog: bool) -> String {
    if fog { "Fog of War" } else { "No Fog" }.to_owned()
}

/// The card's chip for a seed.
pub fn seed_chip(seed: u64) -> String {
    format!("Seed {:04X}-{:04X}", seed >> 16 & 0xFFFF, seed & 0xFFFF)
}
