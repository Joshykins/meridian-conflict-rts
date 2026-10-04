//! The crash screen: the game's own interface, telling a player what happened
//! and getting the report to us in one press. The brand panel where the menu
//! has it; on the left what happened and the report itself (scrolls); on the
//! right what to do, as the menu's tiles: Copy details, Open folder, Restart,
//! Quit. No keys: four buttons, pressed with the pointer. Drawn by the reporter
//! process (`reporter.rs`) on the splash presenter, which has no glass: panels
//! stand on black.
//! Presentation only: every float here ends on the screen.

use crate::audio::Sfx;
use crate::ui::{self, id, ink, palette, rgb, type_scale, Rect, Style, Ui};
use glam::Vec2;
use mc_render::Face;
use std::path::PathBuf;

/// What the crashed run said, and where its report is.
pub(crate) struct Summary {
    /// "The game crashed", "The game had to stop".
    pub title: String,
    /// What went wrong; a panic's ends with "at FILE:LINE, thread NAME".
    pub message: String,
    /// What the player can do about it, when it is known.
    pub hint: Option<String>,
    pub path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    Copy,
    OpenFolder,
    Restart,
    Quit,
}

const ACTIONS: [(Action, &str, &str); 4] = [
    (
        Action::Copy,
        "Copy details",
        "The whole report, ready to paste to us",
    ),
    (
        Action::OpenFolder,
        "Open folder",
        "The saved report, in Explorer",
    ),
    (Action::Restart, "Restart", "Start Meridian Conflict again"),
    (Action::Quit, "Quit", "Close this window"),
];

pub(crate) struct Screen {
    summary: Summary,
    /// The report's lines, tabs opened out.
    lines: Vec<String>,
    /// The first report line shown, in lines (eased toward `scroll_to`).
    scroll: f32,
    scroll_to: f32,
    /// When the report was last copied, on the screen's clock, and whether it worked.
    copied: Option<(f32, bool)>,
}

const MARGIN: f32 = 24.0;
const GAP: f32 = 14.0;
const SIDE_W: f32 = 330.0;
const BRAND_H: f32 = 84.0;
const TILE_H: f32 = 62.0;
const TILE_GAP: f32 = 8.0;
const LINE_H: f32 = 17.0;

const HEADLINE: Style = ui::style(Face::Light, 40.0, 0.4);
const LEAD: Style = ui::style(Face::Medium, 15.5, 0.0);
const MESSAGE: Style = ui::style(Face::Bold, 17.0, 0.1);
const REPORT: Style = ui::style(Face::Medium, 12.5, 0.0);
const NAME_BOLD: Style = ui::style(Face::Bold, 24.0, 0.3);
const NAME_LIGHT: Style = ui::style(Face::Light, 24.0, 0.3);
const BLURB: Style = ui::style(Face::Medium, 12.5, 0.1);

/// How far along a part is in arriving, `delay` seconds after the screen came up.
fn arrive(t: f32, delay: f32) -> f32 {
    let k = ((t - delay) / 0.45).clamp(0.0, 1.0);
    1.0 - (1.0 - k) * (1.0 - k)
}

impl Screen {
    pub(crate) fn new(summary: Summary, report: &str) -> Screen {
        Screen {
            summary,
            lines: report.lines().map(|l| l.replace('\t', "    ")).collect(),
            scroll: 0.0,
            scroll_to: 0.0,
            copied: None,
        }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.summary.path
    }

    /// The report was put on the clipboard (or could not be).
    pub(crate) fn copied(&mut self, at: f32, ok: bool) {
        self.copied = Some((at, ok));
    }

    /// Draws the screen; returns what the player chose this frame.
    pub(crate) fn draw(&mut self, ui: &mut Ui) -> Option<Action> {
        let (w, h) = (ui.size.x, ui.size.y);
        ui.fill(Rect::new(0.0, 0.0, w, h), ink(1.0));
        self.backdrop(ui);
        let t = ui.time;
        let side_x = w - MARGIN - SIDE_W;
        let main_w = side_x - GAP - MARGIN;

        self.with_arrival(ui, t, 0.0, -1.0, |s, ui| s.brand(ui, main_w));
        let story_bottom = self.with_arrival(ui, t, 0.08, -1.0, |s, ui| s.story(ui, main_w)) + GAP;
        self.with_arrival(ui, t, 0.18, -1.0, |s, ui| {
            s.report(
                ui,
                Rect::new(
                    MARGIN,
                    story_bottom,
                    main_w,
                    h - MARGIN - 22.0 - story_bottom,
                ),
            )
        });
        let action = self.with_arrival(ui, t, 0.12, 1.0, |s, ui| {
            s.actions(
                ui,
                Rect::new(side_x, MARGIN, SIDE_W, h - 2.0 * MARGIN - 22.0),
            )
        });
        self.with_arrival(ui, t, 0.22, 1.0, |s, ui| {
            s.readouts(ui, Rect::new(side_x, 0.0, SIDE_W, h - MARGIN - 22.0))
        });
        self.footer(ui);
        ui.fade = 1.0;
        ui.shift = Vec2::ZERO;
        action
    }

    /// Runs `draw` faded and slid in from `side` as it arrives.
    fn with_arrival<T>(
        &mut self,
        ui: &mut Ui,
        t: f32,
        delay: f32,
        side: f32,
        draw: impl FnOnce(&mut Screen, &mut Ui) -> T,
    ) -> T {
        let k = arrive(t, delay);
        ui.fade = k;
        ui.shift = Vec2::new(side * 24.0 * (1.0 - k), 0.0);
        draw(self, ui)
    }

    /// Black, with the meridian down the left of the page and the horizon's
    /// warm haze along the foot, as the opening screen has them.
    fn backdrop(&self, ui: &mut Ui) {
        let (w, h) = (ui.size.x, ui.size.y);
        let warm = rgb(palette::ACCENT_DEEP, 0.16);
        ui.gradient_v(
            Rect::new(0.0, h * 0.55, w, h * 0.45),
            rgb(palette::ACCENT_DEEP, 0.0),
            warm,
        );
        let x = MARGIN * 0.5;
        ui.gradient_v(
            Rect::new(x, 0.0, 1.0, h),
            rgb(palette::ACCENT, 0.0),
            rgb(palette::ACCENT, 0.45),
        );
    }

    /// Top left, as on the menu: the emblem, the name and the build.
    fn brand(&mut self, ui: &mut Ui, w: f32) {
        let r = Rect::new(MARGIN, MARGIN, w, BRAND_H);
        ui.panel(r);
        ui.emblem(
            Vec2::new(r.x + 40.0, r.mid_y()),
            18.0,
            rgb(palette::TEXT, 1.0),
        );
        let x = r.x + 78.0;
        let end = ui.text(x, r.mid_y(), NAME_BOLD, rgb(0xFFFFFF, 1.0), "Meridian");
        ui.text(
            end + 7.0,
            r.mid_y(),
            NAME_LIGHT,
            rgb(palette::TEXT, 0.92),
            "Conflict",
        );
        ui.text_right(
            r.right() - 18.0,
            r.mid_y() - 10.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &crate::build_label(),
        );
        ui.text_right(
            r.right() - 18.0,
            r.mid_y() + 9.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 0.9),
            env!("MERIDIAN_BUILD"),
        );
    }

    /// What happened: the headline, a line for the player, then the message
    /// itself on its own glass. Returns the bottom of what it drew.
    fn story(&mut self, ui: &mut Ui, w: f32) -> f32 {
        let x = MARGIN;
        let mut y = MARGIN + BRAND_H + 34.0;
        // The overline: a warm mark and what this screen is.
        ui.fill(Rect::new(x, y - 5.0, 10.0, 10.0), rgb(palette::ACCENT, 1.0));
        ui.text(
            x + 18.0,
            y,
            type_scale::OVERLINE,
            rgb(palette::ACCENT, 1.0),
            "Crash report",
        );
        y += 38.0;
        ui.text(
            x - 2.0,
            y,
            HEADLINE,
            rgb(0xFFFFFF, 1.0),
            &self.summary.title,
        );
        y += 38.0;
        let lead = "Sorry about this. A report was saved; sending it to us is the quickest \
                    way to get it fixed. Press Copy details, then paste it into a message.";
        for line in ui.wrap(LEAD, lead, w.min(640.0)) {
            ui.text(x, y, LEAD, rgb(palette::DIM, 1.0), &line);
            y += 22.0;
        }
        y += 12.0;

        // The message, its place on a line of its own.
        let (message, place) = match self.summary.message.rsplit_once("\n\nat ") {
            Some((m, at)) => (m.trim().to_owned(), Some(at.trim().to_owned())),
            None => (self.summary.message.trim().to_owned(), None),
        };
        let inner = w - 48.0;
        let mut lines: Vec<String> = message
            .lines()
            .flat_map(|l| ui.wrap(MESSAGE, l, inner))
            .collect();
        // Deliberate cap (presentation): a long message is cut on screen; the
        // report below and Copy details have all of it.
        const MOST_LINES: usize = 4;
        if lines.len() > MOST_LINES {
            lines.truncate(MOST_LINES);
            if let Some(last) = lines.last_mut() {
                last.push_str(" …");
            }
        }
        let hint = self
            .summary
            .hint
            .as_deref()
            .map(|h| ui.wrap(BLURB, h, inner - 20.0));
        let h = 48.0
            + lines.len() as f32 * 24.0
            + if place.is_some() { 22.0 } else { 0.0 }
            + hint.as_ref().map_or(0.0, |l| 14.0 + l.len() as f32 * 18.0);
        let r = Rect::new(x, y, w, h);
        ui.panel(r);
        ui.section(r.x + 14.0, r.y + 20.0, r.w - 28.0, "What happened");
        // The warm stripe down the message, as the menu marks its live tile.
        ui.fill(
            Rect::new(r.x + 14.0, r.y + 38.0, 2.0, h - 52.0),
            rgb(palette::ACCENT, 0.9),
        );
        let mut ly = r.y + 50.0;
        for line in &lines {
            ui.text(r.x + 28.0, ly, MESSAGE, rgb(palette::TEXT, 1.0), line);
            ly += 24.0;
        }
        if let Some(place) = &place {
            ui.text_fit_left(
                r.x + 28.0,
                ly - 2.0,
                inner,
                type_scale::CAPTION,
                rgb(palette::DIM, 0.9),
                &format!("at {place}"),
            );
            ly += 22.0;
        }
        if let Some(hint) = hint {
            ly += 4.0;
            ui.triangle(
                Vec2::new(r.x + 34.0, ly - 6.0),
                Vec2::new(r.x + 40.0, ly + 5.0),
                Vec2::new(r.x + 28.0, ly + 5.0),
                rgb(palette::WARN, 1.0),
            );
            for line in hint {
                ui.text(r.x + 48.0, ly, BLURB, rgb(palette::WARN, 1.0), &line);
                ly += 18.0;
            }
        }
        r.bottom()
    }

    /// The report itself, on its own glass: as much as fits, and the wheel
    /// scrolls the rest.
    fn report(&mut self, ui: &mut Ui, r: Rect) {
        if r.h < 80.0 {
            return;
        }
        ui.panel(r);
        let title = format!("Report  \u{b7}  {} lines", self.lines.len());
        ui.section(r.x + 14.0, r.y + 20.0, r.w - 28.0, &title);
        let body = Rect::new(r.x + 16.0, r.y + 36.0, r.w - 40.0, r.h - 48.0);
        let shown = (body.h / LINE_H).floor().max(1.0);
        let most = (self.lines.len() as f32 - shown).max(0.0);
        let over = body.contains(ui.cursor - ui.shift);
        if over && ui.input.scroll != 0.0 {
            self.scroll_to = (self.scroll_to - ui.input.scroll * 3.0).clamp(0.0, most);
        }
        self.scroll += (self.scroll_to - self.scroll) * (1.0 - (-18.0 * ui.dt).exp());
        let first = self.scroll.floor() as usize;
        let offset = (self.scroll - first as f32) * LINE_H;
        for (i, line) in self.lines.iter().enumerate().skip(first) {
            let y = body.y + LINE_H * 0.5 + (i - first) as f32 * LINE_H - offset;
            if y > body.bottom() - LINE_H * 0.5 {
                break;
            }
            if y < body.y {
                continue;
            }
            // The lines that say what happened stand out from the stack.
            let tone = if i < 6 || line.starts_with("panicked") {
                rgb(palette::TEXT, 0.95)
            } else {
                rgb(palette::DIM, 0.85)
            };
            let line = cut_to(ui, REPORT, line, body.w);
            ui.text(body.x, y, REPORT, tone, &line);
        }
        // A fade at the foot when there is more below, and the scroll bar.
        if self.scroll < most - 0.01 {
            ui.scrim(
                Rect::new(r.x + 2.0, body.bottom() - 30.0, r.w - 4.0, 30.0),
                0.0,
                0.9,
                false,
            );
        }
        if most > 0.0 {
            let track = Rect::new(r.right() - 14.0, body.y, 3.0, body.h);
            ui.fill(track, rgb(palette::LINE, 0.08));
            let thumb_h = (track.h * shown / self.lines.len() as f32).max(24.0);
            let thumb_y = track.y + (track.h - thumb_h) * (self.scroll / most);
            ui.fill(
                Rect::new(track.x, thumb_y, track.w, thumb_h),
                rgb(palette::LINE, if over { 0.6 } else { 0.35 }),
            );
        }
    }

    /// What the player can do, as the menu's tiles; and once the report is
    /// copied, what to do with it.
    fn actions(&mut self, ui: &mut Ui, panel: Rect) -> Option<Action> {
        let n = ACTIONS.len() as f32;
        let tiles_h = 40.0 + n * (TILE_H + TILE_GAP) - TILE_GAP + 14.0;
        let r = Rect::new(panel.x, panel.y, panel.w, tiles_h);
        ui.panel(r);
        ui.section(r.x + 12.0, r.y + 20.0, r.w - 24.0, "What now");

        let mut chosen = None;
        let copied_k = self.copied.map_or(0.0, |(at, _)| {
            let age = ui.time - at;
            (1.0 - ((age - 3.5) / 0.6).clamp(0.0, 1.0)) * (age / 0.12).clamp(0.0, 1.0)
        });
        for (i, &(action, label, blurb)) in ACTIONS.iter().enumerate() {
            let tr = Rect::new(
                r.x + 12.0,
                r.y + 40.0 + i as f32 * (TILE_H + TILE_GAP),
                r.w - 24.0,
                TILE_H,
            );
            // Copy details is the one to press: it stands lit.
            let res = ui.tile(id("crash-action", i), tr, action == Action::Copy, true);
            if res.clicked {
                chosen = Some(action);
            }
            let g = res.glow;
            let tone = match action {
                Action::Copy => palette::ACCENT,
                Action::Quit => palette::TEXT,
                _ => palette::DIM,
            };
            ui.fill(
                Rect::new(tr.x + 1.0, tr.y + 7.0, 2.0, tr.h - 14.0),
                rgb(tone, 0.55 + 0.45 * g),
            );
            ui.gradient_h(
                Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w * 0.55, tr.h - 6.0),
                rgb(tone, 0.04 + 0.14 * g),
                rgb(tone, 0.0),
            );
            let c = Vec2::new(tr.x + 26.0, tr.mid_y());
            let glyph_tone = rgb(if g > 0.5 { 0xFFFFFF } else { tone }, 0.8 + 0.2 * g);
            let done = action == Action::Copy && copied_k > 0.0;
            glyph(ui, action, done, c, glyph_tone);
            let x = tr.x + 50.0;
            let (name, line) = match (action, self.copied) {
                (Action::Copy, Some((_, true))) if done => {
                    ("Copied", "Now paste it into a message to us")
                }
                (Action::Copy, Some((_, false))) if done => {
                    ("Could not copy", "Open folder has the file instead")
                }
                _ => (label, blurb),
            };
            ui.text(
                x,
                tr.mid_y() - 9.0,
                type_scale::ITEM,
                rgb(palette::TEXT, 0.82 + 0.18 * g),
                name,
            );
            ui.text_fit_left(
                x,
                tr.mid_y() + 12.0,
                tr.right() - x - 16.0,
                BLURB,
                rgb(palette::DIM, 0.75 + 0.25 * g),
                line,
            );
        }

        // Where to send it: under the tiles, while the copy is fresh.
        if copied_k > 0.0 && self.copied.is_some_and(|(_, ok)| ok) {
            let fade = ui.fade;
            ui.fade *= copied_k;
            let y = r.bottom() + GAP;
            let note = Rect::new(r.x, y, r.w, 84.0);
            ui.panel(note);
            ui.text(
                note.x + 18.0,
                note.y + 24.0,
                type_scale::CAPTION,
                rgb(palette::ACCENT, 1.0),
                "On the clipboard",
            );
            for (k, line) in ui
                .wrap(
                    BLURB,
                    "Paste it (Ctrl+V) into a message to the developers, with a line \
                     about what you were doing.",
                    note.w - 36.0,
                )
                .iter()
                .enumerate()
            {
                ui.text(
                    note.x + 18.0,
                    note.y + 46.0 + k as f32 * 17.0,
                    BLURB,
                    rgb(palette::TEXT, 0.85),
                    line,
                );
            }
            ui.fade = fade;
        }
        if chosen.is_some() {
            ui.audio.play(if chosen == Some(Action::Quit) {
                Sfx::Back
            } else {
                Sfx::Select
            });
        }
        chosen
    }

    /// The report's particulars, at the foot of the right-hand column: what to
    /// quote when asked which crash it was.
    fn readouts(&mut self, ui: &mut Ui, column: Rect) {
        let file = self
            .summary
            .path
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        let when = self
            .summary
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split('-').nth(1))
            .and_then(|s| s.parse::<u64>().ok())
            .map_or_else(|| "-".to_owned(), utc);
        let rows = [
            ("Report", file),
            ("When", when),
            (
                "Build",
                format!(
                    "{}  \u{b7}  {}",
                    crate::build_label(),
                    env!("MERIDIAN_BUILD")
                ),
            ),
            ("Log", "meridian.log, beside it".to_owned()),
        ];
        let row_h = 26.0;
        let h = 40.0 + rows.len() as f32 * row_h + 6.0;
        let r = Rect::new(column.x, column.bottom() - h, column.w, h);
        ui.panel(r);
        ui.section(r.x + 12.0, r.y + 20.0, r.w - 24.0, "Particulars");
        for (i, (label, value)) in rows.iter().enumerate() {
            let y = r.y + 46.0 + i as f32 * row_h;
            if i > 0 {
                ui.hline(
                    r.x + 14.0,
                    y - row_h * 0.5,
                    r.w - 28.0,
                    rgb(palette::LINE, 0.07),
                );
            }
            ui.text(
                r.x + 16.0,
                y,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                label,
            );
            let x = r.x + 84.0;
            ui.text_fit_left(
                x,
                y,
                r.right() - 16.0 - x,
                type_scale::VALUE,
                rgb(palette::TEXT, 0.92),
                value,
            );
        }
    }

    /// Where the report is, along the foot.
    fn footer(&mut self, ui: &mut Ui) {
        let k = arrive(ui.time, 0.3);
        ui.fade = k;
        ui.shift = Vec2::ZERO;
        let y = ui.size.y - MARGIN * 0.5 - 6.0;
        let path = self.summary.path.display().to_string();
        let w = ui.size.x - 2.0 * MARGIN;
        ui.text_fit_left(
            MARGIN,
            y,
            w,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("Saved as {path}"),
        );
    }
}

/// `text` cut short with an ellipsis to fit `width`, at its own size: report
/// lines stay readable, where `Ui::fitted` would shrink a long path to nothing.
fn cut_to(ui: &mut Ui, st: Style, text: &str, width: f32) -> String {
    if ui.text_width(st, text) <= width {
        return text.to_owned();
    }
    // Halve toward the longest prefix that fits with its ellipsis.
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let trial: String = chars[..mid].iter().chain(['…'].iter()).collect();
        if ui.text_width(st, &trial) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().chain(['…'].iter()).collect()
}

/// Each action's mark, in the menu glyphs' line weight.
fn glyph(ui: &mut Ui, action: Action, done: bool, c: Vec2, color: ui::Color) {
    let line = 1.6;
    if done {
        ui.stroke(
            c + Vec2::new(-7.0, 0.0),
            c + Vec2::new(-2.0, 5.0),
            2.0,
            color,
        );
        ui.stroke(
            c + Vec2::new(-2.0, 5.0),
            c + Vec2::new(8.0, -6.0),
            2.0,
            color,
        );
        return;
    }
    match action {
        // Two sheets, one over the other.
        Action::Copy => {
            ui.frame(Rect::new(c.x - 7.0, c.y - 4.0, 10.0, 12.0), color);
            ui.frame(Rect::new(c.x - 3.0, c.y - 8.0, 10.0, 12.0), color);
        }
        // A folder.
        Action::OpenFolder => {
            let pts = [
                c + Vec2::new(-8.0, -6.0),
                c + Vec2::new(-3.0, -6.0),
                c + Vec2::new(-1.0, -3.0),
                c + Vec2::new(8.0, -3.0),
                c + Vec2::new(8.0, 7.0),
                c + Vec2::new(-8.0, 7.0),
            ];
            ui.polyline(&pts, line, color, true);
        }
        // A ring with an arrow on its end.
        Action::Restart => {
            ui.arc(c, 7.0, -2.2, 3.3, line, color);
            let end = c + Vec2::new(7.0 * 3.3f32.cos(), 7.0 * 3.3f32.sin());
            ui.stroke(end, end + Vec2::new(-4.0, -3.0), line, color);
            ui.stroke(end, end + Vec2::new(3.0, -4.0), line, color);
        }
        // A cross.
        Action::Quit => {
            ui.stroke(
                c + Vec2::new(-6.0, -6.0),
                c + Vec2::new(6.0, 6.0),
                line,
                color,
            );
            ui.stroke(
                c + Vec2::new(6.0, -6.0),
                c + Vec2::new(-6.0, 6.0),
                line,
                color,
            );
        }
    }
}

/// Unix seconds as a date and time, UTC: "4 Oct 2026, 03:34 UTC".
fn utc(secs: u64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = (secs / 86_400) as i64;
    let (hour, minute) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Days to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day} {} {year}, {hour:02}:{minute:02} UTC",
        MONTHS[(month - 1) as usize]
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_dates() {
        assert_eq!(super::utc(0), "1 Jan 1970, 00:00 UTC");
        assert_eq!(super::utc(1_791_084_864), "4 Oct 2026, 03:34 UTC");
        assert_eq!(super::utc(951_782_400), "29 Feb 2000, 00:00 UTC");
    }
}
