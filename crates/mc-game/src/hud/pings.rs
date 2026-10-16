//! Map pings on the HUD (`crate::pings`): a column of buttons beside the minimap, one
//! per kind, the minimap's ripples where pings stand, the line a message ping's words
//! are typed in, and allies' pings arriving.

use super::{build, minimap, Hud, HudAction, Scene, EDGE};
use crate::audio::Sfx;
use crate::game::Mode;
use crate::pings::{badge, glyph, Ping, PingKind, Pings};
use crate::ui::{id, ink, palette, rgb, type_scale, Key, Rect, Ui};
use glam::Vec2;

/// A button's side, beside the map and beside its folded tab (as tall as the tab),
/// and the most a message ping may say.
const BUTTON: f32 = 32.0;
const FOLDED: f32 = 26.0;
const MAX_CHARS: usize = 80;

impl Hud {
    /// A chat line or a message ping's words have the keyboard: the match's keys
    /// must leave it alone.
    pub fn typing(&self) -> bool {
        self.net.typing() || self.pings.typing()
    }

    /// The ping buttons, their top right corner at (`right`, `top`): a column down
    /// the minimap's side, or with the map folded a row beside its tab, clear of
    /// the battlefield the map leaves.
    pub(super) fn ping_bar(&mut self, ui: &mut Ui, s: &Scene, right: f32, top: f32) {
        if s.view.observing {
            return;
        }
        let across = self.minimap_hidden;
        let side = if across { FOLDED } else { BUTTON };
        let n = PingKind::ALL.len() as f32;
        for (i, kind) in PingKind::ALL.into_iter().enumerate() {
            let step = i as f32 * (side + 4.0);
            let r = if across {
                Rect::new(right - n * (side + 4.0) + 4.0 + step, top, side, side)
            } else {
                Rect::new(right - side, top + step, side, side)
            };
            let lit = s.view.mode == Mode::Ping(kind);
            let t = self.tile(ui, id("ping", i), r, lit, true);
            self.claim(ui, r);
            let tone = rgb(kind.tone(), 0.75 + 0.25 * t.glow);
            glyph(
                ui,
                kind,
                Vec2::new(r.x + r.w * 0.5, r.mid_y()),
                side * 0.28,
                tone,
            );
            if !across {
                ui.text_right(
                    r.right() - 4.0,
                    r.y + 8.0,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 0.9),
                    &kind.key().to_string(),
                );
            }
            if t.hovered {
                let tip = format!("Ping: {}  \u{b7}  H, then {}", kind.label(), kind.key());
                let w = ui.text_width(type_scale::MICRO, &tip) + 24.0;
                if across {
                    build::tip(ui, r.right() - w, r.bottom() + 6.0, &tip);
                } else {
                    build::tip(ui, r.x - w - 8.0, r.y + 3.0, &tip);
                }
            }
            if t.clicked {
                self.actions.push(HudAction::ArmPing(kind));
            }
        }
    }

    /// An ally's ping arriving, or this machine's going out, put up with its line in
    /// the chat feed (a click on the line looks at it).
    pub fn ping_up(&mut self, ping: Ping, audio: &crate::audio::Audio) {
        self.net
            .ping_line(ping.from, &ping.name, ping.kind, ping.at, &ping.text);
        self.pings.add(ping, audio);
    }

    pub(super) fn ping_arrived(
        &mut self,
        ui: &mut Ui,
        s: &Scene,
        from: u8,
        kind: PingKind,
        at: Vec2,
        text: &str,
    ) {
        let ping = Ping {
            kind,
            at,
            from,
            name: super::netplay::name_of(s, from),
            text: text.to_owned(),
            age: 0.0,
        };
        self.ping_up(ping, ui.audio);
    }

    /// A screenshot's pings and their lines in the feed (`Pings::stage`).
    pub fn stage_pings(&mut self, focus: Vec2, spread: f32, name: &str) {
        self.pings.stage(focus, spread, name);
        for p in self.pings.live() {
            self.net.ping_line(p.from, &p.name, p.kind, p.at, &p.text);
        }
    }

    /// Too many pings too fast.
    pub fn ping_refused(&mut self) {
        self.notices.note(
            "ping-slow",
            "Pinging too fast: wait a moment",
            palette::WARN,
            super::notices::Glyph::Bar,
            None,
        );
    }

    /// The line a message ping's words are typed in, over the chat, its bottom at
    /// `bottom`. Enter sends it, Escape drops it.
    pub(super) fn ping_draft(&mut self, ui: &mut Ui, bottom: f32) {
        let Some(draft) = &mut self.pings.draft else {
            return;
        };
        let paste = if ui.input.key(Key::Paste) {
            crate::clipboard::paste()
        } else {
            None
        };
        for ch in ui
            .input
            .typed
            .chars()
            .chain(paste.iter().flat_map(|p| p.chars()))
        {
            if !ch.is_control() && draft.text.chars().count() < MAX_CHARS {
                draft.text.push(ch);
            }
        }
        if ui.input.key(Key::Backspace) {
            draft.text.pop();
        }
        if ui.input.key(Key::Escape) {
            ui.audio.play(Sfx::Back);
            self.pings.draft = None;
            return;
        }
        if ui.input.key(Key::Enter) {
            let text = draft.text.trim().to_owned();
            let at = draft.at;
            self.pings.draft = None;
            if text.is_empty() {
                ui.audio.play(Sfx::Back);
            } else {
                self.actions.push(HudAction::Ping {
                    kind: PingKind::Message,
                    at,
                    text,
                });
            }
            return;
        }
        let typed = draft.text.clone();
        let r = Rect::new(EDGE, bottom - 38.0, 560.0, 38.0);
        self.claim(ui, r);
        ui.frost_cut(r, 6.0, 0.85);
        ui.fill(r, ink(0.5));
        let tone = PingKind::Message.tone();
        ui.frame(r, rgb(tone, 0.6));
        badge(
            ui,
            PingKind::Message,
            Vec2::new(r.x + 20.0, r.mid_y()),
            11.0,
        );
        let x = ui.text(
            r.x + 40.0,
            r.mid_y(),
            type_scale::CAPTION,
            rgb(tone, 1.0),
            "Ping Message",
        );
        let end = ui.text(
            x + 12.0,
            r.mid_y(),
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &typed,
        );
        if (ui.time * 1.6).fract() < 0.55 {
            ui.fill(
                Rect::new(end + 2.0, r.mid_y() - 8.0, 2.0, 16.0),
                rgb(tone, 1.0),
            );
        }
        ui.text_right(
            r.right() - 10.0,
            r.mid_y(),
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Enter Send  \u{b7}  Esc",
        );
    }
}

/// Pings on the minimap: ripples while new, then the kind's badge where it stands.
pub(super) fn chart(ui: &mut Ui, pings: &Pings, chart: Rect, at: &dyn Fn(Vec2) -> Vec2) {
    for p in pings.live() {
        let m = at(p.at);
        if !chart.contains(m) {
            continue;
        }
        let fade = ui.fade;
        ui.fade *= p.fade();
        let tone = p.kind.tone();
        if p.age < 3.0 {
            let t = p.age % 1.0;
            minimap::ring(ui, chart, m, 4.0 + 18.0 * t, rgb(tone, 1.0 - t));
        }
        badge(ui, p.kind, m, 6.0);
        ui.fade = fade;
    }
}
