//! The race picker: every playable race, and Random, as cards down the left;
//! the chosen one's full card beside them, with its crest, its other marks,
//! what its name means, what the crest stands for, who it is and how it
//! fights. Skirmish and survival open it from a seat's race cell.

use super::emblem::{self, Mark};
use super::faction::{races, Pick, Race};
use super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use glam::Vec2;

/// What the random pick's card says.
const RANDOM_NAME: &str = "Random";
const RANDOM_MOTTO: &str = "Let the draw decide";
const RANDOM_ABOUT: &str = "A race is drawn for this seat when the match starts, from every race on this list. The loading screen shows who you are leading, or who you are facing.";

/// The picker's state: open or not, for which seat, what is picked.
#[derive(Default)]
pub struct RacePicker {
    open: bool,
    /// Presence, 0..1: the picker fades and rises in.
    shown: f32,
    seat: usize,
    /// Who the seat is, under the title: "ARC AI 1".
    who: String,
    chosen: Pick,
    /// When a card was last clicked, for a double click.
    last_click: Option<(Pick, f32)>,
    /// Card centres drawn last frame (tests click them).
    pub cards: Vec<(Pick, Vec2)>,
    /// The Choose Race button's centre last frame (tests click it).
    pub choose_at: Vec2,
}

/// A race picked for a seat.
pub struct Chosen {
    pub seat: usize,
    pub pick: Pick,
}

/// Every choice in the order the cards show them: the races, then Random.
fn choices() -> Vec<Pick> {
    (0..races().len() as u8)
        .map(Pick::Race)
        .chain([Pick::Random])
        .collect()
}

impl RacePicker {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens for `seat` (`who` names it) on its current pick.
    pub fn open(&mut self, seat: usize, who: &str, current: Pick) {
        self.open = true;
        self.seat = seat;
        self.who = who.to_owned();
        self.chosen = current;
        self.last_click = None;
    }

    /// Opens fully arrived (tests).
    #[cfg(test)]
    pub fn open_now(&mut self, seat: usize, who: &str, current: Pick) {
        self.open(seat, who, current);
        self.shown = 1.0;
    }

    /// Draws the picker over the screen when it is open. The screen underneath
    /// must have been drawn with `ui.interactive` off while `is_open`.
    pub fn draw(&mut self, ui: &mut Ui) -> Option<Chosen> {
        self.shown = if self.open {
            (self.shown + ui.dt * 6.0).min(1.0)
        } else {
            (self.shown - ui.dt * 8.0).max(0.0)
        };
        if self.shown <= 0.0 {
            self.cards.clear();
            return None;
        }
        let k = 1.0 - (1.0 - self.shown) * (1.0 - self.shown);
        let (fade, shift) = (ui.fade, ui.shift);
        ui.fade = fade * k;
        ui.shift = shift + Vec2::new(0.0, 16.0 * (1.0 - k));
        let action = self.body(ui);
        ui.fade = fade;
        ui.shift = shift;
        match action {
            Some(Some(pick)) => {
                self.open = false;
                Some(Chosen {
                    seat: self.seat,
                    pick,
                })
            }
            Some(None) => {
                self.open = false;
                None
            }
            None => None,
        }
    }

    /// `Some(Some(pick))` when a race is chosen, `Some(None)` when cancelled.
    fn body(&mut self, ui: &mut Ui) -> Option<Option<Pick>> {
        let (w, h) = (ui.size.x, ui.size.y);
        let live = self.open && ui.interactive;
        ui.fill(Rect::new(-ui.shift.x, -ui.shift.y, w, h), ink(0.6));
        let pw = (w - 160.0).min(1640.0);
        let ph = (h - 168.0).min(900.0);
        let panel = Rect::new((w - pw) * 0.5, (h - ph) * 0.5, pw, ph);
        ui.panel(panel);
        let inner = panel.inset(36.0);

        // Header.
        let end = ui.text(
            inner.x,
            inner.y + 14.0,
            type_scale::TITLE,
            rgb(0xFFFFFF, 1.0),
            "Choose a Race",
        );
        if !self.who.is_empty() {
            ui.text(
                end + 18.0,
                inner.y + 20.0,
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                &format!("for {}", self.who),
            );
        }
        ui.fill(
            Rect::new(inner.x, inner.y + 46.0, 58.0, 2.0),
            rgb(palette::ACCENT, 1.0),
        );
        ui.gradient_h(
            Rect::new(inner.x + 66.0, inner.y + 46.0, inner.w - 66.0, 1.0),
            rgb(palette::LINE, 0.35),
            rgb(palette::LINE, 0.04),
        );

        let body_top = inner.y + 78.0;
        let footer = 64.0;
        let list = Rect::new(inner.x, body_top, 320.0, inner.bottom() - body_top - footer);
        let detail = Rect::new(
            list.right() + 44.0,
            body_top,
            inner.right() - list.right() - 44.0,
            list.h,
        );

        // The cards.
        let all = choices();
        let before = self.chosen;
        let mut confirm = false;
        self.cards.clear();
        const CARD_H: f32 = 86.0;
        let mut y = list.y;
        for (n, &pick) in all.iter().enumerate() {
            if pick == Pick::Random {
                // Set apart from the races.
                y += 14.0;
                ui.hline(list.x, y - 8.0, list.w, rgb(palette::LINE, 0.12));
            }
            let r = Rect::new(list.x, y, list.w, CARD_H);
            y += CARD_H + 10.0;
            if r.bottom() > list.bottom() {
                break;
            }
            self.cards
                .push((pick, Vec2::new(r.x + r.w * 0.5, r.mid_y()) + ui.shift));
            let res = ui.interact(id("race-card", n), r, live);
            if res.clicked {
                let now = ui.time;
                if self
                    .last_click
                    .is_some_and(|(p, t)| p == pick && now - t < 0.4)
                {
                    confirm = true;
                } else {
                    ui.audio.play(Sfx::Select);
                }
                self.chosen = pick;
                self.last_click = Some((pick, now));
            }
            card(ui, pick, r, self.chosen == pick, res.glow, n);
        }
        // Up and down step through the cards.
        if live && ui.mem.editing.is_none() {
            let at = all.iter().position(|p| *p == self.chosen).unwrap_or(0);
            let step = if ui.input.key(Key::Down) {
                1
            } else if ui.input.key(Key::Up) {
                all.len() - 1
            } else {
                0
            };
            if step != 0 {
                self.chosen = all[(at + step) % all.len()];
                ui.audio.play(Sfx::Tick);
            }
        }
        if self.chosen != before {
            ui.snap(id("race-reveal", 0), 0.0);
        }

        self.detail(ui, detail);

        // Footer.
        let fy = inner.bottom() - 48.0;
        let cancel = ui.button(
            id("race-cancel", 0),
            Rect::new(inner.x, fy, 180.0, 48.0),
            "Cancel",
            ButtonKind::Secondary,
            live,
        );
        let pick_r = Rect::new(inner.right() - 260.0, fy, 260.0, 48.0);
        self.choose_at = Vec2::new(pick_r.x + pick_r.w * 0.5, pick_r.mid_y()) + ui.shift;
        let pick = ui.button(
            id("race-pick", 0),
            pick_r,
            "Choose Race",
            ButtonKind::Primary,
            live,
        );
        let name = self.chosen.race().map_or(RANDOM_NAME, |r| r.name.as_str());
        ui.text_right(
            pick_r.x - 24.0,
            pick_r.mid_y(),
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            name,
        );
        ui.text(
            inner.x + 204.0,
            fy + 24.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Double-click a race to choose it  \u{b7}  Esc to go back",
        );

        if !live {
            return None;
        }
        let typing = ui.mem.editing.is_some();
        if cancel || (ui.input.key(Key::Escape) && !typing) {
            ui.audio.play(Sfx::Back);
            return Some(None);
        }
        if pick || confirm || (ui.input.key(Key::Enter) && !typing) {
            ui.audio.play(Sfx::Select);
            return Some(Some(self.chosen));
        }
        None
    }

    /// The chosen race's card: its crest and marks on the left, its words on the right.
    fn detail(&mut self, ui: &mut Ui, area: Rect) {
        let race = self.chosen.race();
        let reveal = ui.ease(id("race-reveal", 0), 1.0, 5.0);
        let marks: &[Mark] = race.map_or(&[], |r| r.codex.marks.as_slice());
        let strip_h = if marks.is_empty() { 0.0 } else { 118.0 };
        let side = (area.h - strip_h).min(area.w * 0.42).clamp(120.0, 520.0);
        let art = Rect::new(area.x, area.y, side, side);

        // The strip of other marks first: the one under the pointer takes the big frame.
        let mut shown = Mark::Crest;
        if !marks.is_empty() {
            let n = marks.len() as f32;
            let gap = 10.0;
            let tw = ((side - gap * (n - 1.0)) / n).min(110.0);
            let sy = art.bottom() + 18.0;
            for (i, &mark) in marks.iter().enumerate() {
                let t = Rect::new(art.x + i as f32 * (tw + gap), sy, tw, tw * 0.72);
                let res = ui.interact(id("race-mark", i), t, self.open && ui.interactive);
                if res.hovered {
                    shown = mark;
                }
                ui.fill(t, ink(0.55));
                ui.frame(
                    t,
                    rgb(
                        if res.glow > 0.05 {
                            palette::ACCENT
                        } else {
                            palette::LINE
                        },
                        0.14 + 0.5 * res.glow,
                    ),
                );
                if let Some(race) = race {
                    emblem::draw(ui, race, mark, t.inset(8.0), race.tint(1.0));
                }
                ui.text_centred(
                    t.x + t.w * 0.5,
                    t.bottom() + 12.0,
                    type_scale::MICRO,
                    rgb(palette::DIM, 0.8 + 0.2 * res.glow),
                    mark.label(),
                );
            }
        }

        // The big frame.
        ui.fill(art, ink(0.35));
        ui.brackets(art.inset(-6.0), 16.0, rgb(palette::ACCENT, 0.7));
        let lift = Rect::new(art.x, art.y + 10.0 * (1.0 - reveal), art.w, art.h);
        match race {
            Some(race) => {
                let pad = if shown == Mark::Crest { 14.0 } else { 40.0 };
                emblem::draw(ui, race, shown, lift.inset(pad), race.tint(reveal));
            }
            None => emblem::unknown(ui, lift.inset(side * 0.18), reveal),
        }

        // The words.
        let tx = art.right() + 44.0;
        let tw = area.right() - tx;
        if tw < 160.0 {
            return;
        }
        let mut col = Column {
            x: tx,
            y: area.y + 22.0,
            w: tw,
            bottom: area.bottom(),
            alpha: reveal,
        };
        let name = race.map_or(RANDOM_NAME, |r| r.name.as_str());
        ui.text_fit_left(
            tx,
            col.y,
            tw,
            type_scale::TITLE,
            rgb(0xFFFFFF, reveal),
            name,
        );
        col.y += 38.0;
        let motto = race.map_or(RANDOM_MOTTO, |r| r.codex.motto.as_str());
        let short = race.map_or("", |r| r.abbreviation.as_str());
        let mut x = tx;
        if !short.is_empty() {
            x = ui.text(
                tx,
                col.y,
                type_scale::VALUE,
                rgb(palette::ACCENT, reveal),
                short,
            ) + 14.0;
        }
        if !motto.is_empty() {
            ui.text_fit_left(
                x,
                col.y,
                tx + tw - x,
                type_scale::CAPTION,
                rgb(palette::DIM, reveal),
                &format!("\u{201c}{motto}\u{201d}"),
            );
        }
        col.y += 26.0;
        let Some(race) = race else {
            col.paragraph(ui, "How It Works", RANDOM_ABOUT);
            return;
        };
        let codex = &race.codex;
        col.paragraph(ui, "The Name", &codex.meaning);
        col.paragraph(ui, "Who They Are", &codex.about);
        col.pieces(ui, "The Crest", &codex.crest);
        let field = match race.borrowed_roster() {
            Some(roster) if codex.field.is_empty() => {
                format!("Fields {roster} units until its own army is ready.")
            }
            _ => codex.field.clone(),
        };
        col.paragraph(ui, "In the Field", &field);
    }
}

/// A column of headed text filling down to `bottom`; what does not fit is left out.
struct Column {
    x: f32,
    y: f32,
    w: f32,
    bottom: f32,
    alpha: f32,
}

impl Column {
    /// Room for a heading and a line or two under it.
    fn heading(&mut self, ui: &mut Ui, title: &str) -> bool {
        if self.y + 60.0 > self.bottom {
            return false;
        }
        self.y += 14.0;
        ui.section(self.x, self.y, self.w, title);
        self.y += 26.0;
        true
    }

    fn paragraph(&mut self, ui: &mut Ui, title: &str, text: &str) {
        if text.is_empty() || !self.heading(ui, title) {
            return;
        }
        for line in ui.wrap(type_scale::BODY, text, self.w) {
            if self.y + 10.0 > self.bottom {
                return;
            }
            ui.text(
                self.x,
                self.y,
                type_scale::BODY,
                rgb(palette::TEXT, 0.9 * self.alpha),
                &line,
            );
            self.y += 21.0;
        }
    }

    /// Named pieces, the name in a column of its own and what it means beside it.
    fn pieces(&mut self, ui: &mut Ui, title: &str, pieces: &[(String, String)]) {
        if pieces.is_empty() || !self.heading(ui, title) {
            return;
        }
        let label_w = 118.0;
        for (piece, meaning) in pieces {
            let lines = ui.wrap(type_scale::BODY, meaning, self.w - label_w);
            if self.y + 20.0 * lines.len() as f32 > self.bottom {
                return;
            }
            ui.text(
                self.x,
                self.y,
                type_scale::VALUE,
                rgb(palette::TEXT, self.alpha),
                piece,
            );
            for line in &lines {
                ui.text(
                    self.x + label_w,
                    self.y,
                    type_scale::BODY,
                    rgb(palette::DIM, self.alpha),
                    line,
                );
                self.y += 20.0;
            }
            self.y += 6.0;
        }
    }
}

/// One choice in the list: its badge, its short and full name, and whose army it fields.
fn card(ui: &mut Ui, pick: Pick, r: Rect, chosen: bool, glow: f32, n: usize) {
    let lit = ui.ease(id("race-card-lit", n), if chosen { 1.0 } else { 0.0 }, 12.0);
    let g = lit.max(glow * 0.6);
    ui.fill(r, ink(0.5));
    ui.gradient_h(
        r,
        rgb(palette::ACCENT, 0.18 * g),
        rgb(palette::ACCENT, 0.01),
    );
    ui.frame(
        r,
        rgb(
            if chosen {
                palette::ACCENT
            } else {
                palette::LINE
            },
            0.14 + 0.4 * g,
        ),
    );
    ui.fill(Rect::new(r.x, r.y, 4.0, r.h), rgb(palette::ACCENT, lit));
    let mark = Rect::new(r.x + 14.0, r.y + 11.0, r.h - 22.0, r.h - 22.0);
    let x = mark.right() + 16.0 + 3.0 * g;
    let tw = r.right() - x - 12.0;
    let title = rgb(
        if chosen {
            palette::ACCENT
        } else {
            palette::TEXT
        },
        0.85 + 0.15 * g,
    );
    match pick.race() {
        Some(race) => {
            emblem::draw(ui, race, Mark::Badge, mark, race.tint(0.8 + 0.2 * g));
            ui.text_fit_left(
                x,
                r.y + 24.0,
                tw,
                type_scale::ITEM,
                title,
                &race.abbreviation,
            );
            ui.text_fit_left(
                x + 1.0,
                r.y + 46.0,
                tw,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &race.name,
            );
            ui.text_fit_left(
                x + 1.0,
                r.y + 64.0,
                tw,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                &roster_note(race),
            );
        }
        None => {
            emblem::unknown(ui, mark, 0.8 + 0.2 * g);
            ui.text_fit_left(x, r.y + 24.0, tw, type_scale::ITEM, title, RANDOM_NAME);
            ui.text_fit_left(
                x + 1.0,
                r.y + 46.0,
                tw,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                "Drawn when the match starts",
            );
        }
    }
}

/// "Its own army", or whose army it borrows.
fn roster_note(race: &Race) -> String {
    race.borrowed_roster().map_or_else(
        || "Its own army".to_owned(),
        |roster| format!("Fields {roster} units"),
    )
}

/// A seat's race cell: its badge and short name, a caret saying it opens the
/// picker. True when clicked.
pub fn race_cell(ui: &mut Ui, cell: super::Id, r: Rect, pick: Pick, enabled: bool) -> bool {
    let res = ui.tile(cell, r, false, enabled);
    let live = if enabled { 1.0 } else { 0.4 };
    let s = (r.h - 8.0).min(24.0);
    let mark = Rect::new(r.x + 8.0, r.mid_y() - s * 0.5, s, s);
    match pick.race() {
        Some(race) => emblem::draw(ui, race, Mark::Badge, mark, race.tint(live)),
        None => emblem::unknown(ui, mark, live),
    }
    let tone = rgb(palette::TEXT, (0.8 + 0.2 * res.glow) * live);
    ui.text_fit_left(
        mark.right() + 8.0,
        r.mid_y(),
        r.right() - mark.right() - 28.0,
        type_scale::BUTTON,
        tone,
        pick.label(),
    );
    let c = Vec2::new(r.right() - 12.0, r.mid_y());
    ui.triangle(
        c + Vec2::new(-3.5, -2.0),
        c + Vec2::new(3.5, -2.0),
        c + Vec2::new(0.0, 2.5),
        rgb(palette::DIM, live),
    );
    if res.clicked {
        ui.audio.play(Sfx::Select);
    }
    res.clicked
}
