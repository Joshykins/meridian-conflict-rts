//! Network play on the match HUD: the chat feed and its input line, and the
//! link's news (drops, returns, pauses) as notices.
//!
//! Enter opens the line (Shift+Enter to your allies), Tab switches between
//! everyone and allies, Enter sends, Escape puts it away. Lines fade after a
//! while and come back while the line is open.

use super::notices::Glyph;
use super::{Hud, HudAction, Scene, EDGE};
use crate::audio::Sfx;
use crate::netplay::NetNotice;
use crate::ui::{id, ink, palette, rgb, type_scale, Key, Rect, Ui};
use std::collections::VecDeque;

/// Lines kept; the oldest go first.
const KEPT: usize = 60;
/// How long a line stays up with the input closed, and how long it takes to fade.
const LIFE: f32 = 14.0;
const FADE: f32 = 2.5;
/// Lines shown with the input closed and open.
const SHOWN: usize = 6;
const SHOWN_OPEN: usize = 12;
const WIDTH: f32 = 560.0;
const LINE_H: f32 = 22.0;
/// The most a message may say.
const MAX_CHARS: usize = 200;

struct Line {
    from: Option<u8>,
    name: String,
    private: bool,
    text: String,
    age: f32,
}

struct Draft {
    text: String,
    allies: bool,
}

#[derive(Default)]
pub struct NetHud {
    lines: VecDeque<Line>,
    draft: Option<Draft>,
}

impl NetHud {
    /// The chat line has the keyboard: the match's keys must leave it alone.
    pub fn typing(&self) -> bool {
        self.draft.is_some()
    }

    /// Opens the chat line with `text` typed (headless screenshots).
    pub fn stage_draft(&mut self, text: &str, allies: bool) {
        self.draft = Some(Draft {
            text: text.into(),
            allies,
        });
    }
}

/// A slot's name as the match knows it.
fn name_of(s: &Scene, slot: u8) -> String {
    s.view
        .status
        .players
        .get(slot as usize)
        .map_or_else(|| "A commander".to_owned(), |p| p.name.clone())
}

/// The local player's side, the sender included, as a slot mask; 0 with no allies.
fn allies_mask(s: &Scene) -> mc_core::PlayerMask {
    if s.view.observing {
        return 0;
    }
    let players = &s.view.status.players;
    let Some(me) = players.get(s.view.local as usize) else {
        return 0;
    };
    let mask = players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.team == me.team)
        .fold(0, |m, (i, _)| m | mc_core::player_bit(i as u8));
    // Alone on a team: nobody to whisper to.
    if mask.count_ones() > 1 {
        mask
    } else {
        0
    }
}

impl Hud {
    /// Turns this frame's news from the session into notices and chat lines.
    pub(super) fn net_news(&mut self, ui: &mut Ui, s: &Scene) {
        for notice in s.net_notices {
            match notice {
                NetNotice::Chat {
                    from,
                    name,
                    private,
                    text,
                } => {
                    ui.audio.play(Sfx::Tick);
                    self.net.lines.push_back(Line {
                        from: *from,
                        name: name.clone(),
                        private: *private,
                        text: text.clone(),
                        age: 0.0,
                    });
                    if self.net.lines.len() > KEPT {
                        self.net.lines.pop_front();
                    }
                }
                NetNotice::Dropped(slot) => self.notices.note(
                    format!("net-drop-{slot}"),
                    format!("{} lost connection", name_of(s, *slot)),
                    palette::WARN,
                    Glyph::Bar,
                    None,
                ),
                NetNotice::Rejoined(slot) => self.notices.note(
                    format!("net-back-{slot}"),
                    format!("{} is back", name_of(s, *slot)),
                    palette::TEXT,
                    Glyph::Bar,
                    None,
                ),
                NetNotice::Paused(by) => {
                    let who = by.map_or_else(|| "An observer".to_owned(), |p| name_of(s, p));
                    self.notices.note(
                        "net-pause",
                        format!("{who} paused the match"),
                        palette::TEXT,
                        Glyph::Bar,
                        None,
                    );
                }
                NetNotice::Resumed(by) => {
                    let who = by.map_or_else(|| "An observer".to_owned(), |p| name_of(s, p));
                    self.notices.note(
                        "net-pause",
                        format!("{who} resumed the match"),
                        palette::TEXT,
                        Glyph::Bar,
                        None,
                    );
                }
                NetNotice::Reconnected => self.notices.note(
                    "net-reconnected",
                    "Connection restored",
                    palette::TEXT,
                    Glyph::Bar,
                    None,
                ),
            }
        }
    }

    /// The chat feed and its input line, their bottom edge at `bottom`.
    pub(super) fn net_chat(&mut self, ui: &mut Ui, s: &Scene, bottom: f32, dt: f32) {
        if s.net.is_none() {
            return;
        }
        for line in &mut self.net.lines {
            line.age += dt;
        }
        let allies = allies_mask(s);
        let typing = ui.mem.editing.is_some() || ui.mem.popup.is_some();
        let mut sent = None;
        match &mut self.net.draft {
            None => {
                if ui.interactive && !typing && ui.input.key(Key::Enter) {
                    ui.audio.play(Sfx::Tick);
                    self.net.draft = Some(Draft {
                        text: String::new(),
                        allies: s.view.shift && allies != 0,
                    });
                    // This Enter opened the line; it must not also send it.
                    return self.feed(ui, s, bottom, true, allies);
                }
            }
            Some(draft) => {
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
                    self.net.draft = None;
                } else if ui.input.key(Key::Enter) {
                    let text = draft.text.trim().to_owned();
                    let to = if draft.allies { allies } else { 0 };
                    self.net.draft = None;
                    if !text.is_empty() {
                        ui.audio.play(Sfx::Select);
                        sent = Some(HudAction::Chat { text, to });
                    }
                }
            }
        }
        self.actions.extend(sent);
        let open = self.net.draft.is_some();
        self.feed(ui, s, bottom, open, allies);
    }

    fn feed(
        &mut self,
        ui: &mut Ui,
        s: &Scene,
        bottom: f32,
        open: bool,
        allies: mc_core::PlayerMask,
    ) {
        let k = ui.ease(id("chat-open", 0), if open { 1.0 } else { 0.0 }, 14.0);
        let input_h = 38.0 * k;
        let shown = if open { SHOWN_OPEN } else { SHOWN };
        let count = self
            .net
            .lines
            .iter()
            .rev()
            .take(shown)
            .filter(|l| open || l.age < LIFE)
            .count();
        let feed_h = count as f32 * LINE_H;
        let top = bottom - input_h - feed_h - if open { 12.0 } else { 0.0 };
        if open {
            let back = Rect::new(EDGE, top - 8.0, WIDTH, bottom - top + 8.0);
            self.claim(ui, back);
            ui.frost_cut(back, 6.0, 0.85 * k);
            ui.fill_cut(back, 6.0, ink(0.45 * k));
        }
        let lines = self
            .net
            .lines
            .iter()
            .rev()
            .take(shown)
            .filter(|l| open || l.age < LIFE);
        // Newest at the bottom, just over the input line.
        for (i, line) in lines.enumerate() {
            let y = bottom - input_h - if open { 8.0 } else { 0.0 } - (i as f32 + 0.5) * LINE_H;
            let fade = if open {
                1.0
            } else {
                ((LIFE - line.age) / FADE).clamp(0.0, 1.0)
            };
            if !open {
                // Lines over the open battlefield get a soft shade to read against.
                ui.gradient_h(
                    Rect::new(EDGE, y - LINE_H * 0.5, WIDTH, LINE_H),
                    ink(0.45 * fade),
                    ink(0.0),
                );
            }
            chat_line(ui, s, line, EDGE + 12.0, y, fade);
        }
        if k > 0.01 {
            self.chat_input(
                ui,
                Rect::new(EDGE, bottom - input_h, WIDTH, input_h),
                k,
                allies,
            );
        }
    }

    fn chat_input(&mut self, ui: &mut Ui, r: Rect, k: f32, allies: mc_core::PlayerMask) {
        let Some(draft) = &mut self.net.draft else {
            return;
        };
        let (fade, live) = (ui.fade, ui.interactive);
        ui.fade *= k;
        ui.fill(r, ink(0.5));
        ui.frame(r, rgb(palette::ACCENT, 0.6));
        // Who it goes to: a pill that Tab, or a click, switches.
        let label = if draft.allies { "To Allies" } else { "To All" };
        let pill = Rect::new(r.x + 8.0, r.y + 7.0, 92.0, r.h - 14.0);
        let res = ui.interact(id("chat-scope", 0), pill, allies != 0);
        if allies != 0 && (res.clicked || ui.input.key(Key::Tab)) {
            ui.audio.play(Sfx::Tick);
            draft.allies = !draft.allies;
        }
        let tone = if draft.allies {
            palette::ACCENT
        } else {
            palette::TEXT
        };
        ui.fill(pill, rgb(tone, 0.14 + 0.1 * res.glow));
        ui.text_centred(
            pill.x + pill.w * 0.5,
            pill.mid_y(),
            type_scale::CAPTION,
            rgb(tone, 1.0),
            label,
        );
        let end = ui.text(
            pill.right() + 12.0,
            r.mid_y(),
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &draft.text,
        );
        if (ui.time * 1.6).fract() < 0.55 {
            ui.fill(
                Rect::new(end + 2.0, r.mid_y() - 8.0, 2.0, 16.0),
                rgb(palette::ACCENT, 1.0),
            );
        }
        let hint = if allies != 0 {
            "Enter Send  \u{b7}  Tab Switch  \u{b7}  Esc"
        } else {
            "Enter Send  \u{b7}  Esc"
        };
        ui.text_right(
            r.right() - 10.0,
            r.mid_y(),
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            hint,
        );
        (ui.fade, ui.interactive) = (fade, live);
    }
}

/// One line of the feed: who (in their colour) and what, faded by `fade`.
fn chat_line(ui: &mut Ui, s: &Scene, line: &Line, x: f32, y: f32, fade: f32) {
    if fade <= 0.0 {
        return;
    }
    let mut x = x;
    // The server's own word, not a player's.
    if line.from.is_none() && line.name.is_empty() {
        ui.text(x, y, type_scale::BODY, rgb(palette::WARN, fade), &line.text);
        return;
    }
    if line.private {
        x = ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, fade), "Allies") + 8.0;
    }
    let tone = match line.from {
        Some(slot) => {
            let mut c = s.team_color(slot);
            c[3] = fade;
            c
        }
        None => rgb(palette::DIM, fade),
    };
    let who = match line.from {
        Some(_) => format!("{}:", line.name),
        None => format!("{} (watching):", line.name),
    };
    x = ui.text(x, y, type_scale::VALUE, tone, &who) + 8.0;
    let room = EDGE + WIDTH - 12.0 - x;
    ui.text_fit_left(
        x,
        y,
        room,
        type_scale::BODY,
        rgb(palette::TEXT, fade),
        &line.text,
    );
}
