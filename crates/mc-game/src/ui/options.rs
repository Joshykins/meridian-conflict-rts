//! Settings: sound, display, interface. Changes apply as they are made; the
//! caller saves them and pushes them to the window, renderer and mixer.

use super::{ink, id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;
use glam::Vec2;

#[derive(Default)]
pub struct OptionsOutcome {
    pub back: bool,
    /// Something changed this frame.
    pub changed: bool,
    /// The change needs the window or the renderer touched (full screen, vsync).
    pub display_changed: bool,
}

pub fn draw(ui: &mut Ui, settings: &mut Settings, enter: f32) -> OptionsOutcome {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut out = OptionsOutcome::default();
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);

    let left = 64.0;
    ui.reticle(Vec2::new(left + 15.0, 84.0), 13.0, rgb(palette::TEXT, 0.9));
    let end = ui.text(left + 50.0, 84.0, type_scale::TITLE, rgb(0xFFFFFF, 1.0), "SETTINGS");
    ui.text(end + 18.0, 90.0, type_scale::CAPTION, rgb(palette::DIM, 1.0), "SOUND, DISPLAY AND INTERFACE");
    ui.fill(Rect::new(left, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(Rect::new(left + 66.0, 124.0, w - 2.0 * left - 66.0, 1.0), rgb(palette::LINE, 0.35), rgb(palette::LINE, 0.04));

    let panel = Rect::new(left, 164.0, 780.0, 552.0);
    ui.panel(panel);
    let (x, cw) = (panel.x + 28.0, panel.w - 56.0);
    let mut y = panel.y + 34.0;
    let row = |y: &mut f32| {
        let r = Rect::new(x, *y, cw, 50.0);
        *y += 54.0;
        r
    };

    ui.section(x, y, cw, "SOUND");
    y += 22.0;
    for (key, label, value) in [("vol-master", "MASTER VOLUME", &mut settings.master_volume), ("vol-ui", "INTERFACE", &mut settings.interface_volume), ("vol-amb", "AMBIENCE", &mut settings.ambience_volume)] {
        out.changed |= ui.slider(id(key, 0), row(&mut y), label, value);
    }

    y += 22.0;
    ui.section(x, y, cw, "DISPLAY");
    y += 22.0;
    out.display_changed |= ui.toggle(id("fullscreen", 0), row(&mut y), "FULL SCREEN", "BORDERLESS", &mut settings.fullscreen);
    out.display_changed |= ui.toggle(id("vsync", 0), row(&mut y), "VERTICAL SYNC", "", &mut settings.vsync);

    y += 22.0;
    ui.section(x, y, cw, "INTERFACE");
    y += 22.0;
    let r = row(&mut y);
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.10));
    ui.text(r.x + 16.0, r.mid_y(), type_scale::BODY, rgb(palette::TEXT, 0.82), "INTERFACE SCALE");
    let step = ui.stepper(id("ui-scale", 0), Rect::new(r.right() - 180.0, r.y + 9.0, 164.0, 32.0), &format!("{:.0}%", settings.ui_scale * 100.0), rgb(palette::TEXT, 1.0), true);
    if step != 0 {
        settings.ui_scale = ((settings.ui_scale * 20.0).round() + step as f32).clamp(15.0, 30.0) / 20.0;
        out.changed = true;
    }
    out.changed |= ui.toggle(id("profiler", 0), row(&mut y), "PERFORMANCE OVERLAY", "F1 IN A MATCH", &mut settings.show_profiler);
    out.changed |= out.display_changed;

    let back = ui.button(id("options-back", 0), Rect::new(left, h - 64.0 - 52.0, 200.0, 52.0), "BACK", ButtonKind::Secondary, true);
    if back || (ui.input.key(Key::Escape) && ui.interactive) {
        ui.audio.play(Sfx::Back);
        out.back = true;
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    out
}
