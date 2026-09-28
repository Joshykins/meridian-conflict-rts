//! The Replays screen: every recorded match, newest first, with its map,
//! length, players and the marks made on it. Watch one from the start or
//! from any mark.

use super::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use crate::audio::Sfx;
use crate::hud::replay_clock as clock;
use crate::replay::Summary;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A jump to a mark lands this far in front of it, as on the timeline: 5 s.
const LEAD_IN: u32 = 5 * mc_core::TICKS_PER_SECOND;
const ROW_H: f32 = 62.0;

pub enum ReplaysAction {
    Back,
    /// Watch this replay, jumping to the tick if one is given.
    Watch(PathBuf, Option<u32>),
}

pub struct ReplaysState {
    /// Filled by a background read of every file.
    found: Arc<Mutex<Option<Vec<Summary>>>>,
    selected: usize,
    /// First row shown.
    scroll: usize,
}

impl ReplaysState {
    pub fn new() -> ReplaysState {
        let found: Arc<Mutex<Option<Vec<Summary>>>> = Arc::default();
        let into = found.clone();
        std::thread::spawn(move || {
            let list = crate::replay::summaries();
            *into.lock().unwrap() = Some(list);
        });
        ReplaysState {
            found,
            selected: 0,
            scroll: 0,
        }
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

pub fn draw(ui: &mut Ui, state: &mut ReplaysState, enter: f32) -> Option<ReplaysAction> {
    let (w, h) = (ui.size.x, ui.size.y);
    let mut action = None;
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
        "Replays",
    );
    ui.text(
        end + 18.0,
        90.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Recorded Matches and Their Marks",
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

    let guard = state.found.lock().unwrap();
    match guard.as_deref() {
        None => {
            ui.text(
                list.x + 28.0,
                list.y + 40.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "Reading replays\u{2026}",
            );
        }
        Some([]) => {
            ui.text(
                list.x + 28.0,
                list.y + 40.0,
                type_scale::BODY,
                rgb(palette::DIM, 1.0),
                "No replays yet: every skirmish and survival match is recorded.",
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
                        action = Some(ReplaysAction::Watch(s.path.clone(), None));
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
                action = action.or(details(ui, detail, s));
            }
        }
    }
    drop(guard);

    let back = ui.button(
        id("replays-back", 0),
        Rect::new(left, h - 64.0 - 52.0, 200.0, 52.0),
        "Back",
        ButtonKind::Secondary,
        true,
    );
    if back || (ui.input.key(Key::Escape) && ui.interactive) {
        ui.audio.play(Sfx::Back);
        action = Some(ReplaysAction::Back);
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

/// The selected replay: who played, its marks, and the watch buttons.
fn details(ui: &mut Ui, r: Rect, s: &Summary) -> Option<ReplaysAction> {
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
                action = Some(ReplaysAction::Watch(
                    s.path.clone(),
                    Some(m.tick.saturating_sub(LEAD_IN)),
                ));
            }
            y += 36.0;
        }
    }

    if ui.button(
        id("replay-watch", 0),
        Rect::new(x, buttons_y, 220.0, 52.0),
        "Watch",
        ButtonKind::Primary,
        s.playable(),
    ) || (ui.input.key(Key::Enter) && ui.interactive && s.playable())
    {
        ui.audio.play(Sfx::Select);
        action = Some(ReplaysAction::Watch(s.path.clone(), None));
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
