//! The main menu, laid out like the match HUD: glass panels at the screen's
//! edges, the name where the economy sits, the menu as a panel of HUD tiles,
//! readouts about the live backdrop, and the background-scene controls.

use super::backdrop::{Director, SCENES, SCENE_SECONDS};
use super::{id, ink, palette, rgb, type_scale, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Skirmish,
    Survival,
    Range,
    Options,
    Quit,
}

struct Entry {
    label: &'static str,
    blurb: &'static str,
    action: Option<MenuAction>,
}

const ENTRIES: [Entry; 7] = [
    Entry {
        label: "Skirmish",
        blurb: "Command your battlefield",
        action: Some(MenuAction::Skirmish),
    },
    Entry {
        label: "Survival",
        blurb: "Hold out against the Replication Engine",
        action: Some(MenuAction::Survival),
    },
    Entry {
        label: "Test Range",
        blurb: "One unit on a pad: attack it, build it, break it, reset",
        action: Some(MenuAction::Range),
    },
    Entry {
        label: "Multiplayer",
        blurb: "Relay lobbies are command-line only in this build",
        action: None,
    },
    Entry {
        label: "Replays",
        blurb: "Matches are recorded; playback is not built yet",
        action: None,
    },
    Entry {
        label: "Settings",
        blurb: "Sound, display and interface",
        action: Some(MenuAction::Options),
    },
    Entry {
        label: "Exit",
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

/// Gap between the screen's edge and its panels, as in the match HUD.
const MARGIN: f32 = 20.0;
const COLUMN_W: f32 = 404.0;
const BRAND_H: f32 = 96.0;
const TILE_H: f32 = 60.0;
const TILE_GAP: f32 = 8.0;

/// `enter` runs 0..1 as the screen arrives (and back down as it leaves).
pub fn draw(
    ui: &mut Ui,
    state: &mut MenuState,
    director: &mut Director,
    telemetry: &Telemetry,
    enter: f32,
) -> Option<MenuAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    // The backdrop is graded down a little (it is a backdrop); everything else
    // stands on its own glass, the way the match HUD does.
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.12 * enter));
    ui.scrim(Rect::new(0.0, 0.0, 700.0, h), 0.35 * enter, 0.0, true);

    brand(ui, enter);
    let action = entries(ui, state, enter);
    readouts(ui, telemetry, enter);
    scenes_panel(ui, director, telemetry, enter);
    ui.fade = 1.0;
    ui.shift = Vec2::ZERO;
    action
}

/// How far along a panel is in arriving: eased, and `delay` later than the screen.
fn arrive(enter: f32, delay: f32) -> f32 {
    let k = ((enter - delay) / (1.0 - delay)).clamp(0.0, 1.0);
    1.0 - (1.0 - k) * (1.0 - k)
}

/// Top left, where the match has the economy: the emblem and the name.
fn brand(ui: &mut Ui, enter: f32) {
    let k = arrive(enter, 0.0);
    ui.fade = k;
    ui.shift = Vec2::new(-24.0 * (1.0 - k), 0.0);
    let r = Rect::new(MARGIN, MARGIN, COLUMN_W, BRAND_H);
    ui.panel(r);
    ui.emblem(
        Vec2::new(r.x + 42.0, r.mid_y()),
        20.0,
        rgb(palette::TEXT, 1.0),
    );
    let x = r.x + 84.0;
    ui.text(x, r.y + 33.0, NAME_BOLD, rgb(0xFFFFFF, 1.0), "Meridian");
    ui.text(x, r.y + 63.0, NAME_LIGHT, rgb(palette::TEXT, 0.92), "Conflict");
    ui.text_right(
        r.right() - 16.0,
        r.y + 22.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Pre-Alpha",
    );
    ui.text_right(
        r.right() - 16.0,
        r.y + 40.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.9),
        env!("CARGO_PKG_VERSION"),
    );
}

const NAME_BOLD: super::Style = super::style(mc_render::Face::Bold, 28.0, 0.3);
const NAME_LIGHT: super::Style = super::style(mc_render::Face::Light, 28.0, 0.3);
const BLURB: super::Style = super::style(mc_render::Face::Medium, 12.5, 0.1);

/// The menu as a panel of HUD tiles: a stripe, a glyph, the name over a line
/// about it, and the key that picks it.
fn entries(ui: &mut Ui, state: &mut MenuState, enter: f32) -> Option<MenuAction> {
    let k = arrive(enter, 0.1);
    ui.fade = k;
    ui.shift = Vec2::new(-24.0 * (1.0 - k), 0.0);
    let n = ENTRIES.len() as f32;
    let panel = Rect::new(
        MARGIN,
        MARGIN + BRAND_H + 12.0,
        COLUMN_W,
        40.0 + n * (TILE_H + TILE_GAP) - TILE_GAP + 14.0,
    );
    ui.panel(panel);
    ui.section(panel.x + 12.0, panel.y + 20.0, panel.w - 24.0, "Main Menu");

    // Keyboard: up and down move the highlight, enter activates it, and each
    // entry's number picks it directly.
    let before = state.selected;
    if ui.input.key(Key::Down) {
        state.selected = (state.selected + 1) % ENTRIES.len();
    }
    if ui.input.key(Key::Up) {
        state.selected = (state.selected + ENTRIES.len() - 1) % ENTRIES.len();
    }
    let digit = ui
        .input
        .typed
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| d as usize)
        .find(|d| (1..=ENTRIES.len()).contains(d));
    if let Some(d) = digit {
        state.selected = d - 1;
    }
    if state.selected != before && digit.is_none() {
        ui.audio.play(Sfx::Hover);
    }

    let mut chosen = None;
    for (i, entry) in ENTRIES.iter().enumerate() {
        // Tiles arrive one after another.
        let t = arrive(enter, 0.15 + i as f32 * 0.06);
        ui.fade = t;
        ui.shift = Vec2::new(-16.0 * (1.0 - t), 0.0);

        let tr = Rect::new(
            panel.x + 12.0,
            panel.y + 40.0 + i as f32 * (TILE_H + TILE_GAP),
            panel.w - 24.0,
            TILE_H,
        );
        let available = entry.action.is_some();
        let res = ui.tile(id("menu", i), tr, state.selected == i, true);
        if res.hovered && state.selected != i {
            state.selected = i;
        }
        let picked = state.selected == i
            && ui.interactive
            && (ui.input.key(Key::Enter) || digit == Some(i + 1));
        if res.clicked || picked {
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

        let g = res.glow;
        let tone = match (available, entry.action) {
            (false, _) => palette::FAINT,
            (_, Some(MenuAction::Quit)) => palette::TEXT,
            _ => palette::ACCENT,
        };
        // As on the order card: the stripe down the left, a wash off it.
        ui.fill(
            Rect::new(tr.x + 1.0, tr.y + 7.0, 2.0, tr.h - 14.0),
            rgb(tone, 0.55 + 0.45 * g),
        );
        ui.gradient_h(
            Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w * 0.55, tr.h - 6.0),
            rgb(tone, 0.04 + 0.14 * g),
            rgb(tone, 0.0),
        );
        let ink_k: f32 = if available { 1.0 } else { 0.55 };
        glyph(
            ui,
            i,
            Vec2::new(tr.x + 26.0, tr.mid_y()),
            9.0,
            rgb(
                if g > 0.5 && available { 0xFFFFFF } else { tone },
                (0.8 + 0.2 * g) * ink_k.max(0.8),
            ),
        );
        let x = tr.x + 50.0;
        let end = ui.text(
            x,
            tr.mid_y() - 9.0,
            type_scale::ITEM,
            rgb(palette::TEXT, (0.82 + 0.18 * g) * ink_k),
            entry.label,
        );
        if !available {
            let tag = Rect::new(end + 10.0, tr.mid_y() - 17.0, 40.0, 16.0);
            ui.frame(tag, rgb(palette::DIM, 0.4));
            ui.text_centred(
                tag.x + tag.w * 0.5,
                tag.mid_y(),
                type_scale::MICRO,
                rgb(palette::DIM, 0.8),
                "Soon",
            );
        }
        ui.text_fit_left(
            x,
            tr.mid_y() + 12.0,
            tr.right() - x - 36.0,
            BLURB,
            rgb(palette::DIM, (0.75 + 0.25 * g) * ink_k),
            entry.blurb,
        );
        ui.key_cap(
            tr.right() - 23.0,
            tr.mid_y() - 7.5,
            &(i + 1).to_string(),
            state.selected == i,
        );
    }
    chosen
}

/// Each entry's glyph, drawn in the order card's line weight.
fn glyph(ui: &mut Ui, entry: usize, c: Vec2, r: f32, color: super::Color) {
    use crate::hud::icons::{self, Glyph};
    let t = (r * 0.17).max(1.4);
    match entry {
        0 => icons::glyph(ui, Glyph::Attack, c, r, color),
        1 => {
            // A held point under fire from three sides.
            ui.arc(c, r * 0.34, 0.0, TAU, t, color);
            ui.disc(c, t * 0.9, color);
            for k in 0..3 {
                let d = Vec2::from_angle(-std::f32::consts::FRAC_PI_2 + k as f32 * TAU / 3.0);
                icons::arrow_head(ui, c + d * r * 0.58, -d, r * 0.36, t, color);
                ui.stroke(c + d * r * 0.62, c + d * r, t, color);
            }
        }
        2 => icons::glyph(ui, Glyph::GroundAttack, c, r, color),
        3 => {
            // Two stations and the link between them.
            ui.arc(c - Vec2::X * r * 0.5, r * 0.42, 0.0, TAU, t, color);
            ui.arc(c + Vec2::X * r * 0.5, r * 0.42, 0.0, TAU, t, color);
            ui.stroke(
                c - Vec2::new(r * 0.9, -r * 0.85),
                c + Vec2::new(r * 0.9, r * 0.85),
                t,
                color,
            );
        }
        4 => {
            ui.arc(c, r * 0.9, 0.0, TAU, t, color);
            ui.triangle(
                c + Vec2::new(-r * 0.3, -r * 0.45),
                c + Vec2::new(r * 0.5, 0.0),
                c + Vec2::new(-r * 0.3, r * 0.45),
                color,
            );
        }
        5 => {
            // Three sliders.
            for (row, knob) in [(-0.6, 0.3), (0.0, -0.4), (0.6, 0.1)] {
                let y = c.y + row * r;
                ui.stroke(
                    Vec2::new(c.x - r, y),
                    Vec2::new(c.x + r, y),
                    t * 0.8,
                    color,
                );
                ui.disc(Vec2::new(c.x + knob * r, y), t * 1.4, color);
            }
        }
        _ => {
            // Out through a door.
            let door = [
                c + Vec2::new(r * 0.2, -r * 0.9),
                c + Vec2::new(-r * 0.8, -r * 0.9),
                c + Vec2::new(-r * 0.8, r * 0.9),
                c + Vec2::new(r * 0.2, r * 0.9),
            ];
            for pair in door.windows(2) {
                ui.stroke(pair[0], pair[1], t, color);
            }
            let (from, to) = (c + Vec2::new(-r * 0.2, 0.0), c + Vec2::new(r, 0.0));
            ui.stroke(from, to, t, color);
            icons::arrow_head(ui, to, Vec2::X, r * 0.4, t, color);
        }
    }
}

/// Top right, where the match keeps its clock: what the backdrop is.
fn readouts(ui: &mut Ui, telemetry: &Telemetry, enter: f32) {
    let k = arrive(enter, 0.05);
    ui.fade = k;
    ui.shift = Vec2::new(24.0 * (1.0 - k), 0.0);
    let r = Rect::new(ui.size.x - MARGIN - 500.0, MARGIN, 500.0, 56.0);
    ui.panel(r);
    let columns = [
        ("Sector", telemetry.map_name.to_owned()),
        (
            "Camera",
            format!(
                "{:05.0} : {:05.0}  \u{b7}  {:.0} m",
                telemetry.camera.x, telemetry.camera.y, telemetry.altitude
            ),
        ),
        ("Tick", format!("{:05}", telemetry.tick)),
        ("Units", format!("{}", telemetry.units)),
    ];
    let widths = [130.0, 170.0, 80.0, 60.0];
    let mut x = r.x + 16.0;
    for (i, ((label, value), cw)) in columns.iter().zip(widths).enumerate() {
        if i > 0 {
            ui.vline(x - 10.0, r.y + 12.0, r.h - 24.0, rgb(palette::LINE, 0.14));
        }
        ui.text(x, r.y + 18.0, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
        ui.text_fit_left(
            x,
            r.y + 38.0,
            cw - 16.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            value,
        );
        x += cw;
    }
    // Live: the backdrop is a real match running, not a video.
    let blink = (ui.time * 1.1).fract() < 0.6;
    ui.fill(
        Rect::new(r.right() - 22.0, r.y + 15.0, 6.0, 6.0),
        rgb(palette::ACCENT, if blink { 1.0 } else { 0.25 }),
    );
}

fn scenes_panel(ui: &mut Ui, director: &mut Director, telemetry: &Telemetry, enter: f32) {
    let panel = Rect::new(
        ui.size.x - MARGIN - 500.0,
        ui.size.y - MARGIN - 244.0,
        500.0,
        244.0,
    );
    let k = arrive(enter, 0.1);
    ui.fade = k;
    ui.shift = Vec2::new(0.0, 26.0 * (1.0 - k));
    ui.panel(panel);
    let (x, y, right) = (panel.x + 18.0, panel.y, panel.right() - 18.0);

    ui.text(
        x,
        y + 25.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, 0.85),
        "Background Scenes",
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
        if director.auto_advance { "On" } else { "Off" },
    );
    ui.text_right(
        track.x - 38.0,
        y + 25.0,
        type_scale::MICRO,
        rgb(palette::DIM, 0.7 + 0.3 * res.glow),
        "Auto-Advance",
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
        let res = ui.tile(id("transport", k), b, icon == "pause" && director.paused, true);
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
        &format!("Up next  \u{b7}  {}", next.name),
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
