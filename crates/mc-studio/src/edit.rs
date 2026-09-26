//! The Workbench's pattern editor: one instrument's notes in one part, with
//! only what changing them needs. A drum kit shows a grid of its drums; a
//! synth shows a small piano roll. Click empty space to add a note, click a
//! note to remove it, drag it to move it, drag its end to make it longer.
//! Everything else (velocity, automation, quantize, ghosts) is in Advanced.

use crate::app::Studio;
use crate::notes::{snap_floor, snap_round};
use crate::theme::{self, ACCENT, BG0, BG2, BG3, DIM, FAINT, TEXT};
use crate::workbench::pretty;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
};
use mc_music::song::key_name;
use mc_music::{Command, Instrument, Note, PPQ};

pub struct State {
    /// Grid step in ticks.
    pub snap: u32,
    last_len: u32,
    drag: Option<Drag>,
    pub fit_next: bool,
    /// The lowest key shown in the piano roll.
    low_key: i32,
    sounding: Option<(usize, u8, f64)>,
    last: Option<Layout>,
}

impl Default for State {
    fn default() -> State {
        State {
            snap: PPQ / 4,
            last_len: PPQ / 4,
            drag: None,
            fit_next: true,
            low_key: 48,
            sounding: None,
            last: None,
        }
    }
}

#[derive(Clone, Copy)]
enum Drag {
    Move { idx: usize, orig: Note, grab: i64 },
    Resize { idx: usize, orig: Note },
}

/// How the grid was laid out last frame.
#[derive(Clone)]
struct Layout {
    grid: Rect,
    ticks: u32,
    row_h: f32,
    /// Kit: the key of each row, top down. Empty for the piano roll.
    rows: Vec<u8>,
    low_key: i32,
    top_key: i32,
}

impl Layout {
    fn x(&self, t: u32) -> f32 {
        self.grid.left() + self.grid.width() * t as f32 / self.ticks.max(1) as f32
    }
    fn tick(&self, x: f32) -> i64 {
        (((x - self.grid.left()) / self.grid.width()) * self.ticks as f32).floor() as i64
    }
    fn row_of_key(&self, key: u8) -> Option<usize> {
        if self.rows.is_empty() {
            let k = key as i32;
            (k >= self.low_key && k <= self.top_key).then(|| (self.top_key - k) as usize)
        } else {
            self.rows.iter().position(|&r| r == key)
        }
    }
    fn key_of_row(&self, row: i64) -> Option<u8> {
        if row < 0 {
            return None;
        }
        if self.rows.is_empty() {
            let k = self.top_key - row as i32;
            (k >= self.low_key).then_some(k as u8)
        } else {
            self.rows.get(row as usize).copied()
        }
    }
    fn row_at(&self, y: f32) -> i64 {
        ((y - self.grid.top()) / self.row_h).floor() as i64
    }
    fn note_rect(&self, n: &Note, kit: bool, snap: u32) -> Option<Rect> {
        let row = self.row_of_key(n.2)?;
        let y = self.grid.top() + row as f32 * self.row_h;
        let len = if kit { snap } else { n.1 };
        let x0 = self.x(n.0);
        let x1 = self.x((n.0 + len).min(self.ticks)).max(x0 + 4.0);
        Some(Rect::from_min_max(
            pos2(x0 + 1.0, y + 1.5),
            pos2(x1 - 1.0, y + self.row_h - 1.5),
        ))
    }
}

impl State {
    /// The middle of the cell for a tick and key, as drawn last frame (tests).
    #[cfg(test)]
    pub fn cell_pos(&self, tick: u32, key: u8) -> Option<Pos2> {
        let l = self.last.as_ref()?;
        let row = l.row_of_key(key)?;
        Some(pos2(
            l.x(tick) + 3.0,
            l.grid.top() + (row as f32 + 0.5) * l.row_h,
        ))
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let Some((track_name, pattern_name)) = st.wb.edit.clone().or(st.wb.edit_last.clone()) else {
        return;
    };
    let (Some(t), Some(pi)) = (st.song.track(&track_name), st.song.pattern(&pattern_name)) else {
        st.wb.edit = None;
        return;
    };
    let kit = match &st.song.tracks[t].instrument {
        Instrument::Kit(k) => Some(k.clone()),
        Instrument::Synth(_) => None,
    };
    header(ui, st, t, pi, &track_name, &pattern_name);
    ui.add_space(6.0);
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let ticks = st.song.patterns[pi].ticks().max(PPQ);
    let colour = theme::rgb(st.song.tracks[t].colour);
    let names_w = if kit.is_some() { 130.0 } else { 48.0 };
    let grid = Rect::from_min_max(
        pos2(area.left() + names_w, area.top()),
        area.max - vec2(4.0, 4.0),
    );
    let layout = match &kit {
        Some(k) => {
            let mut rows: Vec<u8> = k.drums.iter().map(|d| d.key).collect();
            for n in &st.song.patterns[pi].notes {
                if !rows.contains(&n.2) {
                    rows.push(n.2);
                }
            }
            let row_h = (grid.height() / rows.len().max(1) as f32).clamp(14.0, 34.0);
            Layout {
                grid,
                ticks,
                row_h,
                rows,
                low_key: 0,
                top_key: 0,
            }
        }
        None => {
            let row_h = 15.0;
            let n_rows = ((grid.height() / row_h).floor() as i32).max(8);
            let es = &mut st.wb.editor;
            if es.fit_next {
                es.fit_next = false;
                let notes = &st.song.patterns[pi].notes;
                let mid = if notes.is_empty() {
                    60
                } else {
                    let (lo, hi) = notes
                        .iter()
                        .fold((127, 0), |(l, h), n| (l.min(n.2 as i32), h.max(n.2 as i32)));
                    (lo + hi) / 2
                };
                es.low_key = mid - n_rows / 2;
            }
            if ui.rect_contains_pointer(area) {
                let dy = ui.input(|i| i.smooth_scroll_delta.y);
                if dy != 0.0 {
                    es.low_key += (dy / row_h).round() as i32;
                }
            }
            es.low_key = es.low_key.clamp(0, (127 - n_rows + 1).max(0));
            Layout {
                grid,
                ticks,
                row_h,
                rows: Vec::new(),
                low_key: es.low_key,
                top_key: es.low_key + n_rows - 1,
            }
        }
    };
    draw(ui, st, &layout, area, pi, kit.as_ref(), colour);
    input(ui, st, &layout, pi, t, kit.is_some());
    st.wb.editor.last = Some(layout);
    let now = ui.input(|i| i.time);
    if let Some((tr, k, until)) = st.wb.editor.sounding {
        if now > until {
            st.send(Command::NoteOff { track: tr, key: k });
            st.wb.editor.sounding = None;
        }
    }
}

fn header(ui: &mut egui::Ui, st: &mut Studio, t: usize, pi: usize, track: &str, pattern: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(
            egui::RichText::new(format!("{}: {}", pretty(track), pretty(pattern)))
                .font(theme::font_light(24.0))
                .color(TEXT),
        );
        // Other patterns this instrument plays in the part.
        if let Some(sec) = st.wb.focus.as_ref().and_then(|f| st.song.section(f)) {
            let mut pats: Vec<String> = st.song.sections[sec]
                .clips
                .iter()
                .filter(|c| c.track == track)
                .map(|c| c.pattern.clone())
                .collect();
            pats.dedup();
            if pats.len() > 1 {
                for p in pats {
                    if crate::workbench::big_toggle(ui, p == pattern, &pretty(&p), ACCENT).clicked()
                    {
                        st.wb.edit = Some((track.to_string(), p));
                        st.wb.editor.fit_next = true;
                    }
                }
            }
        }
        let uses = st
            .song
            .sections
            .iter()
            .flat_map(|s| &s.clips)
            .filter(|c| c.pattern == pattern)
            .count();
        if uses > 1 {
            ui.label(
                egui::RichText::new(if uses == 2 {
                    "also plays in 1 other place".to_string()
                } else {
                    format!("also plays in {} other places", uses - 1)
                })
                .color(FAINT)
                .size(13.0),
            )
            .on_hover_text("The same notes are used there too, so a change here changes them all");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if crate::workbench::big_toggle(ui, false, "Close", DIM)
                .on_hover_text("Esc")
                .clicked()
            {
                st.wb.edit = None;
            }
            let alone = st.wb.solo.as_deref() == Some(track);
            if crate::workbench::big_toggle(ui, alone, "Hear this alone", ACCENT).clicked() {
                st.wb.solo = if alone { None } else { Some(track.to_string()) };
            }
            if crate::workbench::big_toggle(ui, false, "Undo", DIM)
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                st.undo();
            }
            ui.add_space(16.0);
            for (name, v) in [("1/16", PPQ / 4), ("1/8", PPQ / 2), ("1/4", PPQ)] {
                if crate::workbench::big_toggle(ui, st.wb.editor.snap == v, name, ACCENT).clicked()
                {
                    st.wb.editor.snap = v;
                    st.wb.editor.last_len = v;
                }
            }
            ui.label(egui::RichText::new("Grid").size(15.0).color(DIM));
        });
    });
    let _ = (t, pi);
}

fn playhead(st: &Studio, pattern: &str, ticks: u32) -> Option<u32> {
    let s = &st.status;
    if !s.playing {
        return None;
    }
    let sent = st.sent_song();
    let sec = sent.sections.get(s.section?)?;
    if st.wb.focus.as_deref() != Some(sec.name.as_str()) {
        return None;
    }
    let track = &st.wb.edit.as_ref()?.0;
    let c = sec
        .clips
        .iter()
        .find(|c| c.pattern == pattern && c.track == *track)?;
    let into = s.section_tick.checked_sub(c.at * PPQ)?;
    (into < c.span(ticks, sent.section_ticks(sec))).then_some(into % ticks.max(1))
}

fn draw(
    ui: &egui::Ui,
    st: &Studio,
    l: &Layout,
    area: Rect,
    pi: usize,
    kit: Option<&mc_music::Kit>,
    colour: Color32,
) {
    let p = ui.painter().with_clip_rect(area);
    let g = l.grid;
    p.rect_filled(g, CornerRadius::same(4), BG0);
    let n_rows = if l.rows.is_empty() {
        (l.top_key - l.low_key + 1) as usize
    } else {
        l.rows.len()
    };
    for r in 0..n_rows {
        let y = g.top() + r as f32 * l.row_h;
        let key = l.key_of_row(r as i64).unwrap_or(0);
        let name_r = Rect::from_min_max(pos2(area.left(), y), pos2(g.left() - 6.0, y + l.row_h));
        let label = match kit {
            Some(k) => k
                .drum_for(key)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| key_name(key)),
            None => key_name(key),
        };
        let in_scale = st.song.scale.contains(st.song.root, key);
        if kit.is_none() {
            let fill = if in_scale {
                Color32::from_rgb(0x19, 0x19, 0x1D)
            } else {
                BG0
            };
            p.rect_filled(
                Rect::from_min_max(pos2(g.left(), y), pos2(g.right(), y + l.row_h)),
                CornerRadius::ZERO,
                fill,
            );
        } else if r % 2 == 1 {
            p.rect_filled(
                Rect::from_min_max(pos2(g.left(), y), pos2(g.right(), y + l.row_h)),
                CornerRadius::ZERO,
                Color32::from_white_alpha(3),
            );
        }
        if kit.is_some() || key % 12 == 0 || l.row_h >= 15.0 && in_scale {
            let c = if kit.is_some() || key % 12 == 0 {
                TEXT
            } else {
                FAINT
            };
            let size = if kit.is_some() { 14.0 } else { 11.0 };
            p.text(
                pos2(name_r.right(), name_r.center().y),
                Align2::RIGHT_CENTER,
                label,
                theme::font_semi(size),
                c,
            );
        }
        if kit.is_none() && key % 12 == 0 {
            p.hline(g.x_range(), y + l.row_h, Stroke::new(1.0, theme::line(22)));
        }
    }
    let snap = st.wb.editor.snap.max(1);
    let bar = st.song.bar_ticks().max(1);
    let mut t = 0;
    while t <= l.ticks {
        let a = if t % bar == 0 {
            40
        } else if t % PPQ == 0 {
            18
        } else {
            7
        };
        p.vline(l.x(t), g.y_range(), Stroke::new(1.0, theme::line(a)));
        t += snap;
    }
    let notes = &st.song.patterns[pi].notes;
    for n in notes {
        if let Some(r) = l.note_rect(n, kit.is_some(), snap) {
            p.rect(
                r,
                CornerRadius::same(3),
                colour,
                Stroke::new(1.0, theme::mix(colour, TEXT, 0.4)),
                StrokeKind::Inside,
            );
            if kit.is_none() && r.width() > 12.0 {
                // The grab handle at the end.
                p.vline(
                    r.right() - 3.0,
                    r.y_range().shrink(3.0),
                    Stroke::new(1.5, theme::with_alpha(BG0, 150)),
                );
            }
        }
    }
    let name = &st.song.patterns[pi].name;
    if let Some(t) = playhead(st, name, l.ticks) {
        p.vline(l.x(t), g.y_range(), Stroke::new(2.0, ACCENT));
    }
    let _ = (BG2, BG3);
}

fn audition(st: &mut Studio, track: usize, key: u8, now: f64) {
    if let Some((t, k, _)) = st.wb.editor.sounding.take() {
        st.send(Command::NoteOff { track: t, key: k });
    }
    st.send(Command::NoteOn {
        track,
        key,
        vel: 100,
    });
    st.wb.editor.sounding = Some((track, key, now + 0.25));
}

fn input(ui: &mut egui::Ui, st: &mut Studio, l: &Layout, pi: usize, track: usize, kit: bool) {
    let resp = ui.interact(
        l.grid,
        ui.id().with("wb-edit-grid"),
        Sense::click_and_drag(),
    );
    let snap = st.wb.editor.snap.max(1);
    let now = ui.input(|i| i.time);
    let hit = |st: &Studio, pos: Pos2| -> Option<(usize, bool)> {
        let notes = &st.song.patterns[pi].notes;
        notes.iter().enumerate().rev().find_map(|(i, n)| {
            let r = l.note_rect(n, kit, snap)?;
            r.contains(pos)
                .then(|| (i, !kit && pos.x > r.right() - 8.0))
        })
    };
    if let Some(h) = resp.hover_pos() {
        match hit(st, h) {
            Some((_, true)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
            Some(_) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
            None => ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair),
        }
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            match hit(st, pos) {
                Some((i, _)) => {
                    st.song.patterns[pi].notes.remove(i);
                }
                None => {
                    let t = snap_floor(l.tick(pos.x), snap);
                    if let Some(key) = l.key_of_row(l.row_at(pos.y)) {
                        if t >= 0 && (t as u32) < l.ticks {
                            let len = if kit {
                                snap
                            } else {
                                st.wb.editor.last_len.max(1)
                            };
                            let notes = &mut st.song.patterns[pi].notes;
                            notes.push(Note(t as u32, len, key, 100));
                            notes.sort_by_key(|n| (n.0, n.2));
                            audition(st, track, key, now);
                        }
                    }
                }
            }
        }
    }
    if resp.drag_started() {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some(pos) = origin {
            if let Some((i, edge)) = hit(st, pos) {
                let orig = st.song.patterns[pi].notes[i];
                st.wb.editor.drag = Some(if edge {
                    Drag::Resize { idx: i, orig }
                } else {
                    Drag::Move {
                        idx: i,
                        orig,
                        grab: l.tick(pos.x) - orig.0 as i64,
                    }
                });
            }
        }
    }
    if resp.dragged() {
        if let (Some(pos), Some(d)) = (resp.interact_pointer_pos(), st.wb.editor.drag) {
            match d {
                Drag::Move { idx, orig, grab } => {
                    let t =
                        snap_round(l.tick(pos.x) - grab, snap).clamp(0, l.ticks as i64 - 1) as u32;
                    let key = l.key_of_row(l.row_at(pos.y)).unwrap_or(orig.2);
                    if let Some(n) = st.song.patterns[pi].notes.get_mut(idx) {
                        let changed_key = n.2 != key;
                        *n = Note(t, orig.1, key, orig.3);
                        if changed_key {
                            audition(st, track, key, now);
                        }
                    }
                }
                Drag::Resize { idx, orig } => {
                    let end =
                        snap_round(l.tick(pos.x), snap).max(orig.0 as i64 + snap as i64) as u32;
                    if let Some(n) = st.song.patterns[pi].notes.get_mut(idx) {
                        n.1 = end - orig.0;
                        st.wb.editor.last_len = n.1;
                    }
                }
            }
        }
    }
    if resp.drag_stopped() {
        st.wb.editor.drag = None;
        st.song.patterns[pi].notes.sort_by_key(|n| (n.0, n.2));
    }
}
