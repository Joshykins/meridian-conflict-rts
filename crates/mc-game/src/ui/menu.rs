//! The main menu: title block with its instrument marks, the menu itself, and
//! the background-scene controls in the corner.

use super::backdrop::{Director, SCENES, SCENE_SECONDS};
use super::{id, ink, palette, rgb, type_scale, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Skirmish,
    Range,
    Options,
    Quit,
}

struct Entry {
    label: &'static str,
    blurb: &'static str,
    action: Option<MenuAction>,
}

const ENTRIES: [Entry; 6] = [
    Entry {
        label: "SKIRMISH",
        blurb: "Command your battlefield",
        action: Some(MenuAction::Skirmish),
    },
    Entry {
        label: "TEST RANGE",
        blurb: "One unit on a pad: attack it, build it, break it, reset",
        action: Some(MenuAction::Range),
    },
    Entry {
        label: "MULTIPLAYER",
        blurb: "Relay lobbies are command-line only in this build",
        action: None,
    },
    Entry {
        label: "REPLAYS",
        blurb: "Matches are recorded; playback is not built yet",
        action: None,
    },
    Entry {
        label: "SETTINGS",
        blurb: "Sound, display and interface",
        action: Some(MenuAction::Options),
    },
    Entry {
        label: "EXIT",
        blurb: "Return to the desktop",
        action: Some(MenuAction::Quit),
    },
];

#[derive(Default)]
pub struct MenuState {
    selected: usize,
}

/// What the corner readouts say about the live backdrop.
pub struct Telemetry<'a> {
    pub map_name: &'a str,
    pub tick: u32,
    pub units: usize,
    pub camera: Vec2,
    pub altitude: f32,
    /// The backdrop map's preview is in image slot `PREVIEW_SLOT`.
    pub preview: bool,
}

/// Image slot holding the backdrop map's preview.
pub const PREVIEW_SLOT: usize = 1;

const LEFT: f32 = 64.0;

/// `enter` runs 0..1 as the screen arrives (and back down as it leaves).
pub fn draw(
    ui: &mut Ui,
    state: &mut MenuState,
    director: &mut Director,
    telemetry: &Telemetry,
    enter: f32,
) -> Option<MenuAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    // Scrims: the left column needs contrast; the edges get a vignette.
    // The whole backdrop is graded down a little too: it is a backdrop.
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.22 * enter));
    ui.scrim(Rect::new(0.0, 0.0, 1040.0, h), 0.90 * enter, 0.0, true);
    ui.scrim(Rect::new(0.0, 0.0, w, 240.0), 0.60 * enter, 0.0, false);
    ui.scrim(
        Rect::new(0.0, h - 300.0, w, 300.0),
        0.0,
        0.72 * enter,
        false,
    );

    title_block(ui, enter);
    let action = entries(ui, state, enter);
    readouts(ui, telemetry, enter);
    scenes_panel(ui, director, telemetry, enter);

    // Footer.
    ui.fade = enter;
    ui.hline(LEFT, h - 70.0, 28.0, rgb(palette::LINE, 0.5));
    ui.text(
        LEFT,
        h - 46.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        concat!(
            "MERIDIAN CONFLICT   //   PRE-ALPHA V",
            env!("CARGO_PKG_VERSION")
        ),
    );
    ui.fade = 1.0;
    action
}

fn title_block(ui: &mut Ui, enter: f32) {
    let t = ui.time;
    let logo = Vec2::new(LEFT + 24.0, 84.0);
    ui.fade = enter;

    // Instrument marks around the logo: broken rings turning at their own speeds,
    // a graduated sector, and a sweep line. The window's corner crops them.
    let line = |a: f32| rgb(palette::LINE, a);
    for (radius, from, sweep, speed, alpha) in [
        (118.0, -0.2, 1.5, 0.05, 0.30),
        (118.0, 2.2, 0.5, 0.05, 0.30),
        (176.0, 0.9, 2.1, -0.032, 0.20),
        (252.0, -0.1, 0.9, 0.021, 0.15),
        (252.0, 1.3, 0.35, 0.021, 0.15),
        (340.0, 0.35, 1.0, -0.012, 0.10),
    ] {
        let a0 = from + t * speed;
        ui.arc(logo, radius, a0, a0 + sweep, 1.0, line(alpha));
    }
    let base = 0.12 + (t * 0.04).sin() * 0.06;
    for i in 0..=30 {
        let a = base + i as f32 * (1.45 / 30.0);
        let (inner, alpha) = if i % 5 == 0 {
            (141.0, 0.34)
        } else {
            (146.0, 0.18)
        };
        let d = Vec2::new(a.cos(), a.sin());
        ui.stroke(logo + d * inner, logo + d * 152.0, 1.0, line(alpha));
    }
    let sweep = (t * 0.22).rem_euclid(TAU);
    if sweep < 1.7 {
        let d = Vec2::new(sweep.cos(), sweep.sin());
        ui.stroke(
            logo + d * 40.0,
            logo + d * 250.0,
            1.0,
            rgb(palette::ACCENT, 0.16),
        );
    }
    ui.text(
        logo.x + 190.0,
        logo.y - 34.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 0.9),
        &format!("BRG {:05.1}", sweep.to_degrees()),
    );
    // Registration crosses, the kind a targeting display leaves lying around.
    for p in [
        Vec2::new(430.0, 84.0),
        Vec2::new(430.0, 300.0),
        Vec2::new(LEFT + 24.0, 300.0),
    ] {
        ui.hline(p.x - 5.0, p.y, 11.0, line(0.28));
        ui.vline(p.x, p.y - 5.0, 11.0, line(0.28));
    }

    ui.reticle(logo, 17.0, rgb(palette::TEXT, 0.95));
    let pulse = 0.5 + 0.5 * (t * 1.3).sin();
    ui.arc(
        logo,
        25.0 + 3.0 * pulse,
        0.0,
        TAU,
        1.0,
        rgb(palette::ACCENT, 0.30 * (1.0 - pulse)),
    );

    // Letters of the title arrive one after another.
    ui.shift.x = -18.0 * (1.0 - enter);
    ui.text(
        LEFT,
        158.0,
        type_scale::OVERLINE,
        rgb(palette::TEXT, 0.9),
        "MERIDIAN",
    );
    ui.text(
        LEFT - 2.0,
        206.0,
        type_scale::DISPLAY,
        rgb(0xFFFFFF, 1.0),
        "CONFLICT",
    );
    ui.shift.x = 0.0;
    ui.fill(
        Rect::new(LEFT, 250.0, 58.0 * enter, 2.0),
        rgb(palette::ACCENT, 1.0),
    );
    ui.gradient_h(
        Rect::new(LEFT + 66.0, 250.0, 300.0 * enter, 1.0),
        line(0.35),
        line(0.0),
    );
    ui.fade = 1.0;
}

fn entries(ui: &mut Ui, state: &mut MenuState, enter: f32) -> Option<MenuAction> {
    let top = 322.0;
    ui.fade = enter;
    ui.gradient_h(
        Rect::new(LEFT, top, 330.0, 1.0),
        rgb(palette::LINE, 0.4),
        rgb(palette::LINE, 0.0),
    );
    ui.text(
        LEFT,
        top + 24.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "MAIN MENU",
    );

    // Keyboard: up and down move the highlight, enter activates it.
    let before = state.selected;
    if ui.input.key(Key::Down) {
        state.selected = (state.selected + 1) % ENTRIES.len();
    }
    if ui.input.key(Key::Up) {
        state.selected = (state.selected + ENTRIES.len() - 1) % ENTRIES.len();
    }
    if state.selected != before {
        ui.audio.play(Sfx::Hover);
    }

    let mut chosen = None;
    for (i, entry) in ENTRIES.iter().enumerate() {
        // Entries arrive one after another, sliding in from the left.
        let k = ((enter * 1.7 - i as f32 * 0.14) / 0.9).clamp(0.0, 1.0);
        let k = 1.0 - (1.0 - k) * (1.0 - k);
        ui.fade = k;
        ui.shift.x = -28.0 * (1.0 - k);

        let slot = Rect::new(LEFT, top + 58.0 + i as f32 * 68.0, 388.0, 64.0);
        let available = entry.action.is_some();
        let res = ui.interact(id("menu", i), slot, true);
        if res.hovered && state.selected != i {
            state.selected = i;
        }
        let key_enter = state.selected == i && ui.input.key(Key::Enter) && ui.interactive;
        if res.clicked || key_enter {
            match entry.action {
                Some(action) => {
                    chosen = Some(action);
                    ui.audio.play(if action == MenuAction::Quit {
                        Sfx::Back
                    } else {
                        Sfx::Select
                    });
                }
                None => ui.audio.play(Sfx::Deny),
            }
        }

        let g = ui.ease(
            id("menu-glow", i),
            if state.selected == i { 1.0 } else { 0.0 },
            13.0,
        );
        let press = ui.ease(id("menu-press", i), if res.held { 1.0 } else { 0.0 }, 30.0);
        let accent = if available {
            palette::ACCENT
        } else {
            palette::DIM
        };
        if g > 0.004 {
            ui.gradient_h(
                slot,
                rgb(accent, (0.17 + 0.10 * press) * g),
                rgb(accent, 0.015 * g),
            );
            ui.frame(slot, rgb(accent, 0.30 * g));
            let bar = slot.h * (0.35 + 0.65 * g);
            ui.fill(
                Rect::new(slot.x, slot.mid_y() - bar * 0.5, 4.0, bar),
                rgb(accent, g),
            );
        }
        let tone = if available {
            rgb(palette::TEXT, 0.86)
        } else {
            rgb(palette::DIM, 0.62)
        };
        let lit = rgb(accent, 1.0);
        let color = [0, 1, 2, 3].map(|c| tone[c] + (lit[c] - tone[c]) * g);
        let x = slot.x + 22.0 + 8.0 * g + 2.0 * press;
        let end = ui.text(
            x,
            slot.mid_y() - 10.0 * g,
            type_scale::ITEM,
            color,
            entry.label,
        );
        if !available {
            let tag = Rect::new(end + 6.0, slot.mid_y() - 10.0 * g - 9.0, 54.0, 18.0);
            ui.frame(tag, rgb(palette::DIM, 0.45));
            ui.text_centred(
                tag.x + tag.w * 0.5 + 1.5,
                tag.mid_y(),
                type_scale::MICRO,
                rgb(palette::DIM, 0.85),
                "SOON",
            );
        }
        if g > 0.02 {
            ui.text(
                x + 1.0,
                slot.mid_y() + 14.0,
                style_blurb(),
                rgb(palette::DIM, g),
                entry.blurb,
            );
        }
    }
    ui.fade = 1.0;
    ui.shift.x = 0.0;
    chosen
}

fn style_blurb() -> super::Style {
    super::style(mc_render::Face::Medium, 13.5, 2.2)
}

/// Top-right: what the backdrop is, in the manner of a camera feed's burn-in.
fn readouts(ui: &mut Ui, telemetry: &Telemetry, enter: f32) {
    let right = ui.size.x - LEFT;
    ui.fade = enter;
    let blink = (ui.time * 1.1).fract() < 0.6;
    ui.text_right(
        right,
        72.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, 0.85),
        &telemetry.map_name.to_uppercase(),
    );
    let w = ui.text_width(type_scale::CAPTION, &telemetry.map_name.to_uppercase());
    ui.text_right(
        right - w - 16.0,
        72.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "SECTOR",
    );
    ui.text_right(
        right,
        94.0,
        type_scale::MICRO,
        rgb(palette::DIM, 0.9),
        &format!(
            "CAM {:05.0} : {:05.0}    ALT {:04.0} M",
            telemetry.camera.x, telemetry.camera.y, telemetry.altitude
        ),
    );
    let live = format!(
        "LIVE SIMULATION    TICK {:05}    {:03} UNITS",
        telemetry.tick, telemetry.units
    );
    let lw = ui.text_width(type_scale::MICRO, &live);
    ui.text_right(
        right,
        114.0,
        type_scale::MICRO,
        rgb(palette::DIM, 0.9),
        &live,
    );
    ui.fill(
        Rect::new(right - lw - 16.0, 110.0, 7.0, 7.0),
        rgb(palette::ACCENT, if blink { 1.0 } else { 0.25 }),
    );
    ui.brackets(
        Rect::new(right - 360.0, 52.0, 372.0, 78.0),
        8.0,
        rgb(palette::LINE, 0.25),
    );
    ui.fade = 1.0;
}

fn scenes_panel(ui: &mut Ui, director: &mut Director, telemetry: &Telemetry, enter: f32) {
    let panel = Rect::new(
        ui.size.x - LEFT - 500.0,
        ui.size.y - 64.0 - 244.0,
        500.0,
        244.0,
    );
    ui.fade = enter;
    ui.shift.y = 26.0 * (1.0 - enter);
    ui.panel(panel);
    let (x, y, right) = (panel.x + 18.0, panel.y, panel.right() - 18.0);

    ui.text(
        x,
        y + 25.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, 0.85),
        "BACKGROUND SCENES",
    );
    // Auto-advance switch.
    let switch = Rect::new(right - 138.0, y + 10.0, 138.0, 30.0);
    let res = ui.interact(id("auto-advance", 0), switch, true);
    if res.clicked {
        director.auto_advance = !director.auto_advance;
        ui.audio.play(if director.auto_advance {
            Sfx::ToggleOn
        } else {
            Sfx::ToggleOff
        });
    }
    let on = ui.ease(
        id("auto-advance-on", 0),
        if director.auto_advance { 1.0 } else { 0.0 },
        16.0,
    );
    let track = Rect::new(right - 28.0, y + 18.0, 28.0, 14.0);
    ui.fill(track, rgb(palette::ACCENT_DEEP, 0.2 + 0.5 * on));
    ui.frame(track, rgb(palette::ACCENT, 0.3 + 0.5 * on + 0.2 * res.glow));
    ui.fill(
        Rect::new(track.x + 3.0 + 14.0 * on, track.y + 3.0, 8.0, 8.0),
        rgb(0xFFFFFF, 0.6 + 0.4 * on),
    );
    ui.text_right(
        track.x - 10.0,
        y + 25.0,
        type_scale::MICRO,
        rgb(
            if director.auto_advance {
                palette::ACCENT
            } else {
                palette::FAINT
            },
            1.0,
        ),
        if director.auto_advance { "ON" } else { "OFF" },
    );
    ui.text_right(
        track.x - 38.0,
        y + 25.0,
        type_scale::MICRO,
        rgb(palette::DIM, 0.7 + 0.3 * res.glow),
        "AUTO-ADVANCE",
    );
    ui.hline(x, y + 46.0, panel.w - 36.0, rgb(palette::LINE, 0.14));

    // Now playing.
    let scene = &SCENES[director.scene];
    let end = ui.text(
        x,
        y + 72.0,
        type_scale::VALUE,
        rgb(palette::ACCENT, 1.0),
        &format!("{:02}", director.scene + 1),
    );
    let end = ui.text(
        end + 4.0,
        y + 72.0,
        type_scale::VALUE,
        rgb(palette::FAINT, 1.0),
        &format!("/ {:02}", SCENES.len()),
    );
    ui.text(
        end + 16.0,
        y + 72.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, 1.0),
        scene.name,
    );
    let bar = Rect::new(x, y + 96.0, 186.0, 3.0);
    ui.fill(bar, rgb(palette::LINE, 0.16));
    ui.fill(
        Rect::new(bar.x, bar.y, bar.w * director.progress(), bar.h),
        rgb(palette::ACCENT, 1.0),
    );
    let clock = |s: f32| format!("{:02}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32);
    ui.text(
        bar.right() + 12.0,
        bar.y + 1.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "{} / {}",
            clock(director.t.min(SCENE_SECONDS)),
            clock(SCENE_SECONDS)
        ),
    );

    // Transport: previous, pause, next.
    for (k, icon) in ["prev", "pause", "next"].into_iter().enumerate() {
        let b = Rect::new(
            right - 3.0 * 40.0 - 2.0 * 8.0 + k as f32 * 48.0,
            y + 58.0,
            40.0,
            40.0,
        );
        let res = ui.interact(id("transport", k), b, true);
        ui.fill(b, ink(0.6));
        ui.fill(b, rgb(palette::ACCENT, 0.16 * res.glow));
        ui.frame(b, rgb(palette::LINE, 0.22 + 0.55 * res.glow));
        let (c, tone) = (
            Vec2::new(
                b.x + b.w * 0.5,
                b.mid_y() + if res.held { 1.0 } else { 0.0 },
            ),
            rgb(palette::TEXT, 0.75 + 0.25 * res.glow),
        );
        match (icon, director.paused) {
            ("pause", false) => {
                ui.fill(Rect::new(c.x - 5.0, c.y - 6.0, 3.0, 12.0), tone);
                ui.fill(Rect::new(c.x + 2.0, c.y - 6.0, 3.0, 12.0), tone);
            }
            ("pause", true) => ui.triangle(
                c + Vec2::new(-4.0, -7.0),
                c + Vec2::new(7.0, 0.0),
                c + Vec2::new(-4.0, 7.0),
                tone,
            ),
            (_, _) => {
                let d = if icon == "prev" { -1.0 } else { 1.0 };
                ui.stroke(
                    c + Vec2::new(-d * 3.0, -6.0),
                    c + Vec2::new(d * 3.0, 0.0),
                    1.8,
                    tone,
                );
                ui.stroke(
                    c + Vec2::new(d * 3.0, 0.0),
                    c + Vec2::new(-d * 3.0, 6.0),
                    1.8,
                    tone,
                );
            }
        }
        if res.clicked {
            ui.audio.play(Sfx::Select);
            match icon {
                "prev" => director.previous(),
                "next" => director.next(),
                _ => director.paused = !director.paused,
            }
        }
    }
    let next = &SCENES[(director.scene + 1) % SCENES.len()];
    ui.text_right(
        right,
        y + 114.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &format!("UP NEXT  \u{b7}  {}", next.name),
    );

    // Scene cards: the map, with a marker where that scene's camera is looking.
    let gap = 10.0;
    let card_w = (panel.w - 36.0 - 2.0 * gap) / 3.0;
    for (i, info) in SCENES.iter().enumerate() {
        let card = Rect::new(x + i as f32 * (card_w + gap), y + 134.0, card_w, 92.0);
        let active = director.scene == i;
        let res = ui.interact(id("scene-card", i), card, true);
        if res.clicked && !active {
            ui.audio.play(Sfx::Select);
            director.go(i);
        }
        let lit = ui.ease(
            id("scene-card-lit", i),
            if active { 1.0 } else { 0.0 },
            10.0,
        );
        let bright = 0.45 + 0.25 * res.glow + 0.30 * lit;
        if telemetry.preview {
            // A band across the middle of the square preview, as wide as the card.
            let side = super::preview::SIZE as f32;
            let band = side * card.h / card.w;
            ui.image(
                PREVIEW_SLOT,
                [0.0, (side - band) * 0.5, side, band],
                card,
                [bright, bright, bright, 1.0],
            );
        } else {
            ui.fill(card, ink(0.8));
        }
        if telemetry.preview {
            // Where this scene's camera looks: the theatre shot frames the map, the others a spot on it.
            let band = card.h / card.w;
            let m = director.marker(i);
            let p = Vec2::new(
                card.x + m.x * card.w,
                card.y + ((m.y - 0.5) / band + 0.5) * card.h,
            );
            if i == 1 {
                ui.brackets(
                    Rect::new(
                        card.x + card.w * 0.2,
                        card.y + 8.0,
                        card.w * 0.6,
                        card.h - 34.0,
                    ),
                    6.0,
                    rgb(palette::ACCENT, 0.5 + 0.5 * lit),
                );
            } else {
                ui.arc(
                    p,
                    5.0 + 1.5 * lit * (ui.time * 2.0).sin(),
                    0.0,
                    TAU,
                    1.2,
                    rgb(palette::ACCENT, 0.6 + 0.4 * lit),
                );
                ui.disc(p, 1.6, rgb(0xFFFFFF, 0.9));
            }
        }
        ui.scrim(
            Rect::new(card.x, card.mid_y(), card.w, card.h * 0.5),
            0.0,
            0.85,
            false,
        );
        ui.frame(
            card,
            rgb(
                if active {
                    palette::ACCENT
                } else {
                    palette::LINE
                },
                0.25 + 0.35 * res.glow + 0.4 * lit,
            ),
        );
        if active {
            ui.fill(
                Rect::new(
                    card.x,
                    card.bottom() - 2.0,
                    card.w * director.progress(),
                    2.0,
                ),
                rgb(palette::ACCENT, 1.0),
            );
        }
        ui.text(
            card.x + 9.0,
            card.bottom() - 14.0,
            type_scale::MICRO,
            rgb(palette::ACCENT, 0.6 + 0.4 * lit),
            &format!("{:02}", i + 1),
        );
        ui.text(
            card.x + 32.0,
            card.bottom() - 14.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.7 + 0.3 * lit.max(res.glow)),
            info.name,
        );
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
}
