//! Settings: sound, display, interface. Changes apply as they are made; the
//! caller saves them and pushes them to the window, renderer and mixer.

mod graphics;
#[cfg(test)]
mod tests;

use super::{id, ink, palette, rgb, type_scale, ButtonKind, Id, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;
use glam::Vec2;

#[derive(Default)]
pub struct OptionsOutcome {
    pub back: bool,
    /// Something changed this frame.
    pub changed: bool,
    /// The change needs the window or the renderer touched (full screen, vsync, render quality).
    pub display_changed: bool,
}

pub fn draw(ui: &mut Ui, settings: &mut Settings, enter: f32) -> OptionsOutcome {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut out = OptionsOutcome::default();
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);

    let left = 64.0;
    ui.emblem(Vec2::new(left + 15.0, 84.0), 13.0, rgb(palette::TEXT, 0.9));
    let end = ui.text(
        left + 50.0,
        84.0,
        type_scale::TITLE,
        rgb(0xFFFFFF, 1.0),
        "Settings",
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Sound, Display and Interface",
    );
    ui.fill(Rect::new(left, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(
        Rect::new(left + 66.0, 124.0, w - 2.0 * left - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );

    // The panel ends above Back at every interface scale; rows that do not fit scroll.
    let back_top = h - 64.0 - 52.0;
    // The open tab and each tab's scroll live in the UI memory, so they survive leaving.
    let tab_key = id("options-open-tab", 0);
    let tab = remembered(ui, tab_key) as usize % TABS.len();
    let scroll_key = id("options-scroll", tab);
    let wanted = remembered(ui, id("options-rows", tab)).max(5.0);
    let room = back_top - 20.0 - 164.0;
    let panel_h = room
        .min(ROWS_TOP + wanted * ROW + 26.0)
        .max(ROWS_TOP + ROW + 26.0);
    let panel = Rect::new(left, 164.0, 780.0, panel_h);
    ui.panel(panel);
    let (x, cw) = (panel.x + 28.0, panel.w - 56.0);

    let picked = tabs(ui, Rect::new(x, panel.y + 18.0, cw, TAB_H), tab);
    let next = if ui.interactive && ui.input.key(Key::Tab) {
        Some((tab + 1) % TABS.len())
    } else {
        picked
    };
    if let Some(next) = next.filter(|&t| t != tab) {
        ui.audio.play(Sfx::Select);
        ui.snap(tab_key, next as f32);
        ui.snap(id("options-scroll", next), 0.0);
    }

    let list = Rect::new(
        x,
        panel.y + ROWS_TOP,
        cw,
        panel.bottom() - 26.0 - panel.y - ROWS_TOP,
    );
    let fits = ((list.h + 2.0) / ROW).floor().max(1.0) as usize;
    let mut first = remembered(ui, scroll_key) as usize;
    if ui.interactive && panel.contains(ui.cursor - ui.shift) && ui.input.scroll != 0.0 {
        first = if ui.input.scroll > 0.0 {
            first.saturating_sub(1)
        } else {
            first + 1
        };
    }
    let mut rows = Rows {
        x,
        w: cw - SCROLLBAR_W,
        top: list.y,
        first,
        fits,
        next: 0,
    };
    match tab {
        0 => {
            for (key, label, value) in [
                ("vol-master", "Master Volume", &mut settings.master_volume),
                ("vol-ui", "Interface", &mut settings.interface_volume),
                ("vol-fx", "Battle", &mut settings.effects_volume),
                ("vol-weather", "Weather", &mut settings.weather_volume),
                ("vol-music", "Music", &mut settings.music_volume),
            ] {
                if let Some(r) = rows.row() {
                    out.changed |= ui.slider(id(key, 0), r, label, value);
                }
            }
        }
        1 => out.display_changed |= graphics::draw(ui, settings, &mut rows),
        _ => out.changed |= interface(ui, settings, &mut rows),
    }
    out.changed |= out.display_changed;

    let total = rows.next;
    let first = first.min(total.saturating_sub(fits));
    ui.snap(id("options-rows", tab), total as f32);
    let first = scrollbar(ui, list, first, fits, total);
    ui.snap(scroll_key, first as f32);

    let back = ui.button(
        id("options-back", 0),
        Rect::new(left, back_top, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    if back || (ui.input.key(Key::Escape) && ui.interactive) {
        ui.audio.play(Sfx::Back);
        out.back = true;
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    out
}

const TABS: [&str; 3] = ["Sound", "Display", "Interface"];
const TAB_H: f32 = 40.0;
/// From the panel's top to the first row: the tab strip and a gap.
const ROWS_TOP: f32 = 18.0 + TAB_H + 22.0;
const ROW: f32 = 48.0;
const SCROLLBAR_W: f32 = 18.0;

fn remembered(ui: &Ui, key: Id) -> f32 {
    ui.mem.anims.get(&key).copied().unwrap_or(0.0)
}

/// The open tab's rows, a window of `fits` whole rows from `first`. Each control
/// asks for its row in turn; one scrolled out of the window gets none and is not drawn.
pub(super) struct Rows {
    x: f32,
    w: f32,
    top: f32,
    first: usize,
    fits: usize,
    /// Rows asked for so far; after the tab is drawn, how many it has.
    next: usize,
}

impl Rows {
    fn row(&mut self) -> Option<Rect> {
        let i = self.next;
        self.next += 1;
        (i >= self.first && i < self.first + self.fits).then(|| {
            Rect::new(
                self.x,
                self.top + (i - self.first) as f32 * ROW,
                self.w,
                ROW - 2.0,
            )
        })
    }
}

/// The tab strip. Returns the tab clicked this frame.
fn tabs(ui: &mut Ui, r: Rect, open: usize) -> Option<usize> {
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.18));
    let mut clicked = None;
    let mut tx = r.x;
    for (i, name) in TABS.iter().enumerate() {
        let w = ui.text_width(type_scale::BUTTON, name) + 44.0;
        let t = Rect::new(tx, r.y, w, r.h);
        let res = ui.interact(id("options-tab", i), t, true);
        let on = ui.ease(
            id("options-tab-on", i),
            if i == open { 1.0 } else { 0.0 },
            16.0,
        );
        ui.gradient_v(
            t,
            rgb(palette::ACCENT, 0.0),
            rgb(palette::ACCENT, 0.05 * res.glow + 0.10 * on),
        );
        ui.text_centred(
            t.x + t.w * 0.5,
            t.mid_y(),
            type_scale::BUTTON,
            rgb(
                if i == open { 0xFFFFFF } else { palette::DIM },
                0.85 + 0.15 * res.glow,
            ),
            name,
        );
        let bar = (t.w - 8.0) * (0.3 * res.glow).max(on);
        ui.fill(
            Rect::new(t.x + (t.w - bar) * 0.5, t.bottom() - 2.0, bar, 2.0),
            rgb(palette::ACCENT, 1.0),
        );
        if res.clicked {
            clicked = Some(i);
        }
        tx = t.right() + 4.0;
    }
    clicked
}

/// A thin track at the list's right edge when the rows do not all fit; dragging or
/// clicking it scrolls. Returns the first row to show.
fn scrollbar(ui: &mut Ui, list: Rect, first: usize, fits: usize, total: usize) -> usize {
    if total <= fits {
        return 0;
    }
    let track = Rect::new(
        list.right() - 6.0,
        list.y + 4.0,
        3.0,
        fits as f32 * ROW - 10.0,
    );
    let hit = Rect::new(
        list.right() - SCROLLBAR_W,
        list.y,
        SCROLLBAR_W,
        track.h + 8.0,
    );
    let res = ui.interact_with(id("options-scrollbar", 0), hit, true, false);
    let mut first = first;
    if res.held {
        let k = ((ui.cursor.y - ui.shift.y - track.y) / track.h).clamp(0.0, 1.0);
        first =
            ((k * total as f32 - fits as f32 * 0.5).round().max(0.0) as usize).min(total - fits);
    }
    ui.fill(track, rgb(palette::LINE, 0.12 + 0.1 * res.glow));
    let thumb_h = track.h * fits as f32 / total as f32;
    let thumb_y = track.y + (track.h - thumb_h) * first as f32 / (total - fits) as f32;
    ui.fill(
        Rect::new(track.x - 1.0, thumb_y, track.w + 2.0, thumb_h),
        rgb(palette::TEXT, 0.45 + 0.4 * res.glow),
    );
    first
}

fn interface(ui: &mut Ui, settings: &mut Settings, rows: &mut Rows) -> bool {
    let mut changed = false;
    if let Some(r) = rows.row() {
        ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
        ui.text(
            r.x + 16.0,
            r.mid_y(),
            type_scale::BODY,
            rgb(palette::TEXT, 0.82),
            "Interface Scale",
        );
        let step = ui.stepper(
            id("ui-scale", 0),
            Rect::new(r.right() - 180.0, r.y + 7.0, 164.0, 32.0),
            &format!("{:.0}%", settings.ui_scale * 100.0),
            rgb(palette::TEXT, 1.0),
            true,
        );
        if step != 0 {
            settings.ui_scale =
                ((settings.ui_scale * 20.0).round() + step as f32).clamp(15.0, 30.0) / 20.0;
            changed = true;
        }
    }
    if let Some(r) = rows.row() {
        changed |= ui.toggle(
            id("profiler", 0),
            r,
            "Performance Overlay",
            "F1 in a Match",
            &mut settings.show_profiler,
        );
    }
    changed
}
