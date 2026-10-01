//! The Match History screen: every recorded match, newest first, with its map,
//! length, players and the marks made on it. Read its battle report
//! (`summary.rs`), or watch its replay from the start or from any mark.

mod summary;

use super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::hud::replay_clock as clock;
use crate::replay::Summary;
use glam::Vec2;
use mc_data::Blueprints;
use mc_jobs::Pool;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use summary::{SummaryAction, SummaryState};

/// A jump to a mark lands this far in front of it, as on the timeline: 5 s.
const LEAD_IN: u32 = 5 * mc_core::TICKS_PER_SECOND;
const ROW_H: f32 = 62.0;

pub enum HistoryAction {
    Back,
    /// Watch this replay, jumping to the tick if one is given.
    Watch(PathBuf, Option<u32>),
}

pub struct HistoryState {
    /// Filled by a background read of every file.
    found: Arc<Mutex<Option<Vec<Summary>>>>,
    selected: usize,
    /// First row shown.
    scroll: usize,
    /// What a battle report is worked out with.
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    /// The selected match's battle report, over the list.
    summary: Option<SummaryState>,
    /// A report had the image slots; what the other screens keep there must go back.
    slots_taken: bool,
}

impl HistoryState {
    pub fn new(blueprints: Arc<Blueprints>, pool: Arc<Pool>) -> HistoryState {
        let found: Arc<Mutex<Option<Vec<Summary>>>> = Arc::default();
        let into = found.clone();
        std::thread::spawn(move || {
            let list = crate::replay::summaries();
            *into.lock().unwrap() = Some(list);
        });
        HistoryState {
            found,
            selected: 0,
            scroll: 0,
            blueprints,
            pool,
            summary: None,
            slots_taken: false,
        }
    }

    /// True once after a battle report closed: it drew over the image slots the
    /// menu, set-up and map browser keep their pictures in.
    pub fn take_slots_back(&mut self) -> bool {
        self.summary.is_none() && std::mem::take(&mut self.slots_taken)
    }

    /// A headless shot of match `n`'s battle report (1 = newest) on `page`: waits for the list
    /// and for the report to be worked out (`MERIDIAN_HISTORY_REPORT`).
    pub fn open_report_now(&mut self, n: usize, page: &str) {
        let found = loop {
            if let Some(list) = self.found.lock().unwrap().take() {
                break list;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        self.selected = n.saturating_sub(1).min(found.len().saturating_sub(1));
        if let Some(s) = found.get(self.selected) {
            self.open_report(s);
        }
        *self.found.lock().unwrap() = Some(found);
        if let Some(summary) = &mut self.summary {
            summary.wait(page);
        }
    }

    fn open_report(&mut self, s: &Summary) {
        self.summary = Some(SummaryState::open(s, &self.blueprints, &self.pool));
        self.slots_taken = true;
    }
}

/// `20260926-223017` as `Sep 26 2026  22:30 UTC`.
fn when(id: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let digits: Vec<u32> = id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .filter_map(|c| c.to_digit(10))
        .collect();
    let num = |a: usize, n: usize| {
        digits
            .get(a..a + n)
            .map(|d| d.iter().fold(0, |t, v| t * 10 + v))
    };
    match (num(0, 4), num(4, 2), num(6, 2), num(8, 2), num(10, 2)) {
        (Some(y), Some(m @ 1..=12), Some(d), Some(hh), Some(mm)) => {
            format!("{} {d} {y}  {hh:02}:{mm:02} UTC", MONTHS[m as usize - 1])
        }
        _ => id.to_owned(),
    }
}

pub fn draw(ui: &mut Ui, state: &mut HistoryState, enter: f32) -> Option<HistoryAction> {
    // An open report has the pointer and keys; the list stands still under it.
    let interactive = ui.interactive;
    ui.interactive &= state.summary.is_none();
    let mut action = list(ui, state, enter);
    ui.interactive = interactive;
    if let Some(summary) = &mut state.summary {
        match summary.draw(ui) {
            Some(SummaryAction::Close) => state.summary = None,
            Some(SummaryAction::Watch(path)) => action = Some(HistoryAction::Watch(path, None)),
            None => {}
        }
    }
    action
}

fn list(ui: &mut Ui, state: &mut HistoryState, enter: f32) -> Option<HistoryAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut action = None;
    let mut report = None;
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
        "Match History",
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Battle Reports and Replays of Every Recorded Match",
    );
    ui.fill(Rect::new(left, 124.0, 58.0, 2.0), rgb(palette::ACCENT, 1.0));
    ui.gradient_h(
        Rect::new(left + 66.0, 124.0, w - 2.0 * left - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );

    let list_w = (w * 0.46).clamp(520.0, 820.0);
    let list = Rect::new(left, 164.0, list_w, h - 164.0 - 150.0);
    let detail = Rect::new(
        list.right() + 24.0,
        list.y,
        (w - left - list.right() - 24.0).min(620.0),
        list.h,
    );
    ui.panel(list);

    let found = state.found.clone();
    let guard = found.lock().unwrap();
    match guard.as_deref() {
        None => {
            ui.text(
                list.x + 28.0,
                list.y + 40.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "Reading match history\u{2026}",
            );
        }
        Some([]) => {
            ui.text(
                list.x + 28.0,
                list.y + 40.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "No matches yet: every skirmish and survival match is recorded.",
            );
        }
        Some(found) => {
            state.selected = state.selected.min(found.len() - 1);
            let fits = (((list.h - 24.0) / ROW_H) as usize).max(1);
            // The wheel scrolls; arrows move the selection and keep it in view.
            if ui.interactive && list.contains(ui.cursor) && ui.input.scroll != 0.0 {
                state.scroll = if ui.input.scroll > 0.0 {
                    state.scroll.saturating_sub(1)
                } else {
                    state.scroll + 1
                };
            }
            if ui.interactive && ui.input.key(Key::Down) {
                state.selected = (state.selected + 1).min(found.len() - 1);
            }
            if ui.interactive && ui.input.key(Key::Up) {
                state.selected = state.selected.saturating_sub(1);
            }
            if state.selected < state.scroll {
                state.scroll = state.selected;
            } else if state.selected >= state.scroll + fits {
                state.scroll = state.selected + 1 - fits;
            }
            state.scroll = state.scroll.min(found.len().saturating_sub(fits));
            for (i, s) in found.iter().enumerate().skip(state.scroll).take(fits) {
                let r = Rect::new(
                    list.x + 12.0,
                    list.y + 12.0 + (i - state.scroll) as f32 * ROW_H,
                    list.w - 24.0,
                    ROW_H - 4.0,
                );
                if row(ui, i, r, s, i == state.selected) {
                    if i == state.selected && s.playable() {
                        report = Some(i);
                    }
                    state.selected = i;
                }
            }
            if found.len() > fits {
                ui.text_right(
                    list.right() - 16.0,
                    list.bottom() + 16.0,
                    type_scale::MICRO,
                    rgb(palette::DIM, 1.0),
                    &format!(
                        "{}-{} of {}",
                        state.scroll + 1,
                        (state.scroll + fits).min(found.len()),
                        found.len()
                    ),
                );
            }
            if let Some(s) = found.get(state.selected) {
                match details(ui, detail, s) {
                    Some(Pick::Report) => report = Some(state.selected),
                    Some(Pick::Watch(at)) => {
                        action = Some(HistoryAction::Watch(s.path.clone(), at))
                    }
                    None => {}
                }
            }
            if let Some(s) = report.and_then(|i| found.get(i)) {
                state.open_report(s);
            }
        }
    }
    drop(guard);

    let back = ui.button(
        id("history-back", 0),
        Rect::new(left, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    if back || (ui.input.key(Key::Escape) && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(HistoryAction::Back);
    }
    ui.fade = 1.0;
    ui.shift.y = 0.0;
    action
}

/// One replay in the list; returns whether it was clicked.
fn row(ui: &mut Ui, i: usize, r: Rect, s: &Summary, lit: bool) -> bool {
    let res = ui.interact(id("replay-row", i), r, true);
    let k = ui.ease(id("replay-row-lit", i), if lit { 1.0 } else { 0.0 }, 16.0);
    ui.gradient_h(
        r,
        rgb(palette::ACCENT, 0.05 + 0.12 * k + 0.06 * res.glow),
        rgb(palette::ACCENT, 0.0),
    );
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(palette::ACCENT, k));
    ui.hline(r.x, r.bottom(), r.w, rgb(palette::LINE, 0.08));
    let tone = if s.playable() { 1.0 } else { 0.45 };
    let map = s.map.as_deref().unwrap_or("Unknown map");
    let end = ui.text(
        r.x + 16.0,
        r.y + 18.0,
        type_scale::VALUE,
        rgb(palette::TEXT, tone),
        map,
    );
    ui.text(
        end + 12.0,
        r.y + 18.0,
        type_scale::CAPTION,
        rgb(palette::DIM, tone),
        &when(&s.id),
    );
    if s.playable() {
        ui.text_right(
            r.right() - 16.0,
            r.y + 18.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &clock(s.length),
        );
    }
    let mut line = format!("{} commanders", s.players.len());
    if s.survival {
        line = "Survival".into();
    }
    if !s.marks.is_empty() {
        line += &format!("   \u{b7}   {} marked", s.marks.len());
    }
    if s.playable() && !s.complete {
        line += "   \u{b7}   cut short";
    }
    if let Some(p) = &s.problem {
        line = p.clone();
    }
    ui.text_fit_left(
        r.x + 16.0,
        r.y + 40.0,
        r.w - 32.0,
        type_scale::CAPTION,
        rgb(
            if s.problem.is_some() {
                palette::WARN
            } else if s.marks.is_empty() {
                palette::DIM
            } else {
                palette::TEXT
            },
            if s.problem.is_some() { 0.8 } else { 1.0 },
        ),
        &line,
    );
    if res.clicked {
        ui.audio.play(Sfx::Select);
    }
    res.clicked
}

/// What the selected match's card asks for.
enum Pick {
    Report,
    /// Watch the replay, from the tick if one is given.
    Watch(Option<u32>),
}

/// The selected match: who played, its marks, and the report and watch buttons.
fn details(ui: &mut Ui, r: Rect, s: &Summary) -> Option<Pick> {
    let mut action = None;
    ui.panel(r);
    let (x, cw) = (r.x + 28.0, r.w - 56.0);
    let mut y = r.y + 34.0;
    ui.section(x, y, cw, s.map.as_deref().unwrap_or("Unknown map"));
    y += 30.0;
    for (label, value) in [
        ("Recorded", when(&s.id)),
        ("Match", s.id.clone()),
        (
            "Build",
            s.build.clone().unwrap_or_else(|| "not recorded".into()),
        ),
        (
            "Length",
            if !s.playable() {
                "-".into()
            } else if s.complete {
                clock(s.length)
            } else {
                format!("{} (cut short)", clock(s.length))
            },
        ),
        (
            "Report",
            if !s.playable() {
                "-".into()
            } else if s.report_kept {
                "Ready".into()
            } else {
                "Read from the replay when first opened".into()
            },
        ),
    ] {
        ui.text(x, y, type_scale::CAPTION, rgb(palette::DIM, 1.0), label);
        ui.text(
            x + 110.0,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &value,
        );
        y += 24.0;
    }
    let other_build = s
        .build
        .as_deref()
        .filter(|b| *b != crate::replay::BUILD)
        .map(|b| {
            format!(
                "Recorded by {b}, this is {}: it may play out differently",
                crate::replay::BUILD
            )
        });
    if let Some(p) = s.problem.as_ref().or(other_build.as_ref()) {
        y += 8.0;
        ui.text_fit_left(x, y, cw, type_scale::CAPTION, rgb(palette::WARN, 1.0), p);
        y += 24.0;
    }

    y += 16.0;
    ui.section(x, y, cw, "Commanders");
    y += 26.0;
    for (name, human) in &s.players {
        ui.text(
            x,
            y,
            type_scale::BODY,
            rgb(if *human { palette::TEXT } else { palette::DIM }, 1.0),
            name,
        );
        y += 22.0;
    }

    let buttons_y = r.bottom() - 28.0 - 52.0;
    if !s.marks.is_empty() {
        y += 16.0;
        ui.section(x, y, cw, "Marks");
        y += 14.0;
        for (i, m) in s.marks.iter().enumerate() {
            let row = Rect::new(x, y, cw, 34.0);
            if row.bottom() > buttons_y - 12.0 {
                ui.text(
                    x,
                    y + 12.0,
                    type_scale::CAPTION,
                    rgb(palette::DIM, 1.0),
                    &format!("and {} more on the timeline", s.marks.len() - i),
                );
                break;
            }
            let res = ui.interact(id("replay-mark-row", i), row, s.playable());
            ui.gradient_h(
                row,
                rgb(palette::WARN, 0.04 + 0.12 * res.glow),
                rgb(palette::WARN, 0.0),
            );
            ui.fill(
                Rect::new(row.x, row.y + 13.0, 8.0, 8.0),
                rgb(palette::WARN, 0.9),
            );
            let end = ui.text(
                row.x + 18.0,
                row.mid_y(),
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                &format!("{}   {}", m.number, clock(m.tick)),
            );
            let note = if m.note.is_empty() {
                "No note"
            } else {
                &m.note
            };
            ui.text_fit_left(
                end + 16.0,
                row.mid_y(),
                row.right() - end - 24.0,
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                note,
            );
            if res.clicked {
                ui.audio.play(Sfx::Select);
                action = Some(Pick::Watch(Some(m.tick.saturating_sub(LEAD_IN))));
            }
            y += 36.0;
        }
    }

    if ui.button(
        id("replay-report", 0),
        Rect::new(x, buttons_y, 220.0, 52.0),
        "Battle Report",
        ButtonKind::Primary,
        s.playable(),
    ) || (ui.input.key(Key::Enter) && ui.interactive && s.playable())
    {
        ui.audio.play(Sfx::Select);
        action = Some(Pick::Report);
    }
    if ui.button(
        id("replay-watch", 0),
        Rect::new(x + 232.0, buttons_y, 220.0, 52.0),
        "Watch Replay",
        ButtonKind::Secondary,
        s.playable(),
    ) {
        ui.audio.play(Sfx::Select);
        action = Some(Pick::Watch(None));
    }
    action
}

#[cfg(test)]
mod tests {
    use super::when;

    #[test]
    fn ids_read_as_dates() {
        assert_eq!(when("20260926-223017"), "Sep 26 2026  22:30 UTC");
        assert_eq!(when("20260926-223017-2"), "Sep 26 2026  22:30 UTC");
        assert_eq!(when("odd"), "odd");
    }
}
