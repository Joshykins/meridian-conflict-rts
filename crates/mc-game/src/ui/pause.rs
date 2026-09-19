//! The in-match menu. Lockstep matches do not pause, so this is a panel over a
//! battle that carries on; it also reports the result when the match is decided.

use super::{ink, id, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseAction {
    Resume,
    Leave,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heading {
    Menu,
    Victory,
    Defeat,
}

pub struct PauseOutcome {
    pub action: Option<PauseAction>,
    pub settings_changed: bool,
}

pub fn draw(ui: &mut Ui, heading: Heading, settings: &mut Settings, enter: f32) -> PauseOutcome {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut out = PauseOutcome { action: None, settings_changed: false };
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.55 * enter));
    ui.fade = enter;
    ui.shift.y = 18.0 * (1.0 - enter);

    let panel = Rect::new((w - 460.0) * 0.5, (h - 470.0) * 0.5, 460.0, 470.0);
    ui.panel(panel);
    let (x, cw) = (panel.x + 30.0, panel.w - 60.0);
    let (title, note, ink) = match heading {
        Heading::Menu => ("COMMAND MENU", "THE BATTLE CONTINUES WHILE THIS IS OPEN", palette::TEXT),
        Heading::Victory => ("VICTORY", "EVERY ENEMY COMMANDER IS DESTROYED", palette::ACCENT),
        Heading::Defeat => ("DEFEAT", "YOUR COMMANDER HAS BEEN DESTROYED", palette::BAD),
    };
    ui.text_centred(panel.x + panel.w * 0.5 + 7.0, panel.y + 56.0, type_scale::TITLE, rgb(ink, 1.0), title);
    ui.text_centred(panel.x + panel.w * 0.5, panel.y + 92.0, type_scale::MICRO, rgb(palette::DIM, 1.0), note);
    ui.gradient_h(Rect::new(x, panel.y + 116.0, cw * 0.5, 1.0), rgb(palette::LINE, 0.0), rgb(palette::LINE, 0.4));
    ui.gradient_h(Rect::new(x + cw * 0.5, panel.y + 116.0, cw * 0.5, 1.0), rgb(palette::LINE, 0.4), rgb(palette::LINE, 0.0));

    let resume = if heading == Heading::Menu { "RESUME" } else { "KEEP WATCHING" };
    let mut y = panel.y + 146.0;
    if ui.button(id("pause-resume", 0), Rect::new(x, y, cw, 54.0), resume, ButtonKind::Primary, true) || (ui.input.key(Key::Escape) && ui.interactive) {
        ui.audio.play(Sfx::Back);
        out.action = Some(PauseAction::Resume);
    }
    y += 78.0;
    // Narrower than the settings screen's rows, so the slider is laid out by hand there; here just volume.
    out.settings_changed |= volume(ui, Rect::new(x, y, cw, 50.0), &mut settings.master_volume);
    y += 78.0;
    if ui.button(id("pause-leave", 0), Rect::new(x, y, cw, 50.0), "LEAVE MATCH", ButtonKind::Secondary, true) {
        ui.audio.play(Sfx::Select);
        out.action = Some(PauseAction::Leave);
    }
    y += 62.0;
    if ui.button(id("pause-quit", 0), Rect::new(x, y, cw, 50.0), "EXIT TO DESKTOP", ButtonKind::Secondary, true) {
        ui.audio.play(Sfx::Back);
        out.action = Some(PauseAction::Quit);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    out
}

/// A compact volume slider: label above, track below.
fn volume(ui: &mut Ui, r: Rect, value: &mut f32) -> bool {
    let res = ui.interact(id("pause-volume", 0), r, true);
    let track = Rect::new(r.x, r.y + 34.0, r.w, 4.0);
    let mut changed = false;
    if res.held {
        let v = (((ui.cursor.x - ui.shift.x - track.x) / track.w).clamp(0.0, 1.0) * 20.0).round() / 20.0;
        if (v - *value).abs() > 1e-4 {
            *value = v;
            changed = true;
            ui.audio.play(Sfx::Tick);
        }
    }
    ui.text(r.x, r.y + 10.0, type_scale::CAPTION, rgb(palette::DIM, 0.8 + 0.2 * res.glow), "MASTER VOLUME");
    ui.text_right(r.right(), r.y + 10.0, type_scale::VALUE, rgb(palette::TEXT, 1.0), &format!("{:.0}", *value * 100.0));
    ui.fill(track, rgb(palette::LINE, 0.16));
    ui.fill(Rect::new(track.x, track.y, track.w * *value, track.h), rgb(palette::ACCENT, 0.9));
    let grow = 1.5 * res.glow;
    ui.fill(Rect::new(track.x + track.w * *value - 4.0 - grow * 0.5, track.y - 7.0 - grow * 0.5, 8.0 + grow, 18.0 + grow), rgb(0xFFFFFF, 1.0));
    changed
}
