//! The in-match menu: resume, settings, leave, quit. A single-player match
//! holds its clock while this is open; a lockstep match cannot, so there it is
//! a panel over a battle that carries on. It also reports the result when the
//! match is decided.

use super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseAction {
    Resume,
    Settings,
    Leave,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heading {
    Menu,
    Victory,
    Defeat,
    Complete,
}

pub struct PauseOutcome {
    pub action: Option<PauseAction>,
    pub settings_changed: bool,
}

pub fn draw(
    ui: &mut Ui,
    heading: Heading,
    holds_clock: bool,
    settings: &mut Settings,
    enter: f32,
) -> PauseOutcome {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut out = PauseOutcome {
        action: None,
        settings_changed: false,
    };
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.55 * enter));
    ui.fade = enter;
    ui.shift.y = 18.0 * (1.0 - enter);

    let panel = Rect::new((w - 460.0) * 0.5, (h - 532.0) * 0.5, 460.0, 532.0);
    ui.panel(panel);
    let (x, cw) = (panel.x + 30.0, panel.w - 60.0);
    let (title, note, ink) = match heading {
        Heading::Menu if holds_clock => {
            ("Command Menu", "The Battlefield Is Holding", palette::TEXT)
        }
        Heading::Menu => (
            "Command Menu",
            "The battle continues while this is open",
            palette::TEXT,
        ),
        Heading::Victory => (
            "Victory",
            "Every enemy commander is destroyed",
            palette::ACCENT,
        ),
        Heading::Defeat => ("Defeat", "Your commander has been destroyed", palette::BAD),
        Heading::Complete => (
            "Engagement Complete",
            "The last commander standing holds the field",
            palette::ACCENT,
        ),
    };
    ui.text_centred(
        panel.x + panel.w * 0.5 + 7.0,
        panel.y + 56.0,
        type_scale::TITLE,
        rgb(ink, 1.0),
        title,
    );
    ui.text_centred(
        panel.x + panel.w * 0.5,
        panel.y + 92.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        note,
    );
    ui.gradient_h(
        Rect::new(x, panel.y + 116.0, cw * 0.5, 1.0),
        rgb(palette::LINE, 0.0),
        rgb(palette::LINE, 0.4),
    );
    ui.gradient_h(
        Rect::new(x + cw * 0.5, panel.y + 116.0, cw * 0.5, 1.0),
        rgb(palette::LINE, 0.4),
        rgb(palette::LINE, 0.0),
    );

    let resume = if heading == Heading::Menu {
        "Resume"
    } else {
        "Keep Watching"
    };
    let mut y = panel.y + 146.0;
    if ui.button(
        id("pause-resume", 0),
        Rect::new(x, y, cw, 54.0),
        resume,
        ButtonKind::Primary,
        true,
    ) || (ui.input.key(Key::Escape) && ui.interactive)
    {
        ui.audio.play(Sfx::Back);
        out.action = Some(PauseAction::Resume);
    }
    y += 66.0;
    if ui.button(
        id("pause-settings", 0),
        Rect::new(x, y, cw, 50.0),
        "Settings",
        ButtonKind::Secondary,
        true,
    ) {
        ui.audio.play(Sfx::Select);
        out.action = Some(PauseAction::Settings);
    }
    y += 74.0;
    // Narrower than the settings screen's rows, so the slider is laid out by hand there; here just volume.
    out.settings_changed |= volume(ui, Rect::new(x, y, cw, 50.0), &mut settings.master_volume);
    y += 78.0;
    if ui.button(
        id("pause-leave", 0),
        Rect::new(x, y, cw, 50.0),
        "Leave Match",
        ButtonKind::Secondary,
        true,
    ) {
        ui.audio.play(Sfx::Select);
        out.action = Some(PauseAction::Leave);
    }
    y += 62.0;
    if ui.button(
        id("pause-quit", 0),
        Rect::new(x, y, cw, 50.0),
        "Exit to Desktop",
        ButtonKind::Secondary,
        true,
    ) {
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
        let v = (((ui.cursor.x - ui.shift.x - track.x) / track.w).clamp(0.0, 1.0) * 20.0).round()
            / 20.0;
        if (v - *value).abs() > 1e-4 {
            *value = v;
            changed = true;
            ui.audio.play(Sfx::Tick);
        }
    }
    ui.text(
        r.x,
        r.y + 10.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 0.8 + 0.2 * res.glow),
        "Master Volume",
    );
    ui.text_right(
        r.right(),
        r.y + 10.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!("{:.0}", *value * 100.0),
    );
    ui.fill(track, rgb(palette::LINE, 0.16));
    ui.fill(
        Rect::new(track.x, track.y, track.w * *value, track.h),
        rgb(palette::ACCENT, 0.9),
    );
    let grow = 1.5 * res.glow;
    ui.fill(
        Rect::new(
            track.x + track.w * *value - 4.0 - grow * 0.5,
            track.y - 7.0 - grow * 0.5,
            8.0 + grow,
            18.0 + grow,
        ),
        rgb(0xFFFFFF, 1.0),
    );
    changed
}
