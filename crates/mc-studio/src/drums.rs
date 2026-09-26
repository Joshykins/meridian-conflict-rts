//! The drum grid: a step sequencer view of a pattern played by a kit. Rows
//! are the kit's drums by name, columns are steps at the chosen resolution.
//! Click toggles a hit, dragging across empty steps paints them, dragging a
//! hit up or down sets its velocity, and clicking a drum's name plays it.

use crate::app::{Pane, PlayMode, Studio};
use crate::songops;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
};
use mc_music::song::key_name;
use mc_music::{Command, Instrument, Note, PPQ};

pub struct State {
    /// Last frame's grid origin, row height and step width, for tests.
    last: Option<(Pos2, f32, f32)>,
    res: usize,
    drag: Option<Drag>,
    sounding: Option<(usize, u8, f64)>,
    pub default_vel: u8,
}

impl Default for State {
    fn default() -> State {
        State {
            last: None,
            res: 1,
            drag: None,
            sounding: None,
            default_vel: 100,
        }
    }
}

impl State {
    /// The middle of a cell (row, step) as drawn last frame.
    #[cfg(test)]
    pub fn cell_pos(&self, row: usize, step: u32) -> Option<Pos2> {
        let (o, h, w) = self.last?;
        Some(pos2(
            o.x + (step as f32 + 0.5) * w,
            o.y + (row as f32 + 0.5) * h,
        ))
    }
}

enum Drag {
    Paint {
        key: u8,
    },
    Velocity {
        key: u8,
        step: u32,
        from: u8,
        y: f32,
    },
}

const RES: [(&str, u32); 5] = [
    ("1/8", PPQ / 2),
    ("1/16", PPQ / 4),
    ("1/32", PPQ / 8),
    ("1/8 triplet", PPQ / 3),
    ("1/16 triplet", PPQ / 6),
];
const NAMES_W: f32 = 150.0;
const RULER_H: f32 = 18.0;

/// Which steps of `key` hold a hit: the note index of the first hit starting in each step.
fn hit_in(notes: &[Note], key: u8, t0: u32, t1: u32) -> Option<usize> {
    notes
        .iter()
        .position(|n| n.2 == key && n.0 >= t0 && n.0 < t1)
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(pi) = st.sel.pattern.filter(|&p| p < st.song.patterns.len()) else {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No pattern open")
                    .font(theme::font_light(22.0))
                    .color(DIM),
            );
            ui.label(egui::RichText::new("Click a drum clip in the arrangement.").color(FAINT));
        });
        return;
    };
    toolbar(ui, st, pi);
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let track = st.sel.pattern_track;
    let kit = match st.song.tracks.get(track).map(|t| &t.instrument) {
        Some(Instrument::Kit(k)) => Some(k.clone()),
        _ => None,
    };
    // Rows: the kit's drums, then any other keys the pattern uses.
    let mut rows: Vec<(String, u8)> = kit
        .as_ref()
        .map(|k| k.drums.iter().map(|d| (d.name.clone(), d.key)).collect())
        .unwrap_or_default();
    let mut extra: Vec<u8> = st.song.patterns[pi]
        .notes
        .iter()
        .map(|n| n.2)
        .filter(|k| !rows.iter().any(|r| r.1 == *k))
        .collect();
    extra.sort_unstable();
    extra.dedup();
    rows.extend(
        extra
            .into_iter()
            .map(|k| (format!("Key {}", key_name(k)), k)),
    );
    if rows.is_empty() {
        ui.label(
            egui::RichText::new(
                "This track is not a drum kit and the pattern is empty: use the piano roll.",
            )
            .color(FAINT),
        );
        return;
    }
    let res = RES[st.drums.res].1;
    let ticks = st.song.patterns[pi].ticks().max(res);
    let steps = ticks.div_ceil(res);
    let grid_rect = Rect::from_min_max(pos2(area.left() + NAMES_W, area.top() + RULER_H), area.max);
    let row_h = ((grid_rect.height() - 4.0) / rows.len() as f32).clamp(16.0, 30.0);
    let step_w = ((grid_rect.width() - 8.0) / steps as f32).max(6.0);
    st.drums.last = Some((grid_rect.min, row_h, step_w));
    let colour = st
        .song
        .tracks
        .get(track)
        .map(|t| theme::rgb(t.colour))
        .unwrap_or(ACCENT);
    let p = ui.painter().with_clip_rect(area);
    p.rect_filled(area, CornerRadius::ZERO, theme::BG1);
    let bar = st.song.bar_ticks();
    let now = ui.input(|i| i.time);

    // Ruler.
    let ruler = Rect::from_min_max(
        pos2(grid_rect.left(), area.top()),
        pos2(grid_rect.right(), grid_rect.top()),
    );
    p.rect_filled(ruler, CornerRadius::ZERO, BG0);
    for s in 0..steps {
        let t = s * res;
        let x = grid_rect.left() + s as f32 * step_w;
        if t % PPQ == 0 {
            let label = if t % bar == 0 {
                format!("{}", t / bar + 1)
            } else {
                format!("{}.{}", t / bar + 1, (t % bar) / PPQ + 1)
            };
            p.text(
                pos2(x + 2.0, ruler.center().y),
                Align2::LEFT_CENTER,
                label,
                theme::font_body(10.0),
                if t % bar == 0 { DIM } else { FAINT },
            );
        }
    }
    let play_step = playhead_tick(st, pi, ticks).map(|t| t / res);

    let notes = st.song.patterns[pi].notes.clone();
    let mut toggle: Option<(u8, u32)> = None;
    let mut audition_key: Option<u8> = None;
    for (ri, (name, key)) in rows.iter().enumerate() {
        let y = grid_rect.top() + ri as f32 * row_h;
        let name_r = Rect::from_min_size(pos2(area.left(), y), vec2(NAMES_W, row_h));
        let nresp = ui.interact(name_r, ui.id().with(("drum-name", ri)), Sense::click());
        let fill = if nresp.is_pointer_button_down_on() {
            theme::with_alpha(ACCENT, 60)
        } else if nresp.hovered() {
            theme::line(10)
        } else {
            BG0
        };
        p.rect_filled(name_r, CornerRadius::ZERO, fill);
        p.text(
            name_r.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            crate::transport::truncate(name, 16),
            theme::font_semi(12.5),
            TEXT,
        );
        p.text(
            name_r.right_center() - vec2(8.0, 0.0),
            Align2::RIGHT_CENTER,
            key_name(*key),
            theme::font_body(10.0),
            FAINT,
        );
        if nresp.clicked() || nresp.drag_started() {
            audition_key = Some(*key);
            st.focus = Pane::Detail;
        }
        p.hline(
            area.left()..=grid_rect.right(),
            y + row_h,
            Stroke::new(1.0, theme::line(8)),
        );
        for s in 0..steps {
            let t0 = s * res;
            let cell = Rect::from_min_size(
                pos2(grid_rect.left() + s as f32 * step_w, y),
                vec2(step_w, row_h),
            )
            .shrink(1.5);
            let beat_shade = (t0 / PPQ) % 2 == 0;
            let base = if beat_shade {
                Color32::from_rgb(0x1B, 0x1B, 0x20)
            } else {
                Color32::from_rgb(0x15, 0x15, 0x19)
            };
            let hit = hit_in(&notes, *key, t0, t0 + res);
            let playing = play_step == Some(s);
            p.rect_filled(
                cell,
                CornerRadius::same(2),
                if playing {
                    theme::mix(base, TEXT, 0.08)
                } else {
                    base
                },
            );
            if let Some(i) = hit {
                let vel = notes[i].3 as f32 / 127.0;
                let c = theme::mix(theme::mix(BG2, colour, 0.5), colour, vel);
                let h = (cell.height() * (0.35 + 0.65 * vel)).max(3.0);
                let r = Rect::from_min_max(pos2(cell.left(), cell.bottom() - h), cell.max);
                p.rect_filled(r, CornerRadius::same(2), c);
                if playing {
                    p.rect_stroke(
                        cell,
                        CornerRadius::same(2),
                        Stroke::new(1.0, TEXT),
                        StrokeKind::Inside,
                    );
                }
            }
            if t0 % bar == 0 && s > 0 {
                p.vline(
                    cell.left() - 1.5,
                    (y)..=(y + row_h),
                    Stroke::new(1.0, theme::line(40)),
                );
            }
        }
    }
    // One interaction over the grid; the cell is found from the pointer.
    let gh = rows.len() as f32 * row_h;
    let active = Rect::from_min_size(grid_rect.min, vec2(steps as f32 * step_w, gh));
    let resp = ui.interact(active, ui.id().with("drum-grid"), Sense::click_and_drag());
    let cell_at = |pos: egui::Pos2| -> Option<(usize, u32)> {
        let r = ((pos.y - grid_rect.top()) / row_h).floor();
        let s = ((pos.x - grid_rect.left()) / step_w).floor();
        (r >= 0.0 && (r as usize) < rows.len() && s >= 0.0 && (s as u32) < steps)
            .then(|| (r as usize, s as u32))
    };
    if resp.clicked() {
        if let Some((r, s)) = resp.interact_pointer_pos().and_then(cell_at) {
            toggle = Some((rows[r].1, s));
            st.focus = Pane::Detail;
        }
    }
    if resp.drag_started() {
        // Where the button went down, not where the drag was recognised.
        let start = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some((r, s)) = start.and_then(cell_at) {
            let key = rows[r].1;
            st.focus = Pane::Detail;
            match hit_in(&notes, key, s * res, s * res + res) {
                Some(i) => {
                    st.drums.drag = Some(Drag::Velocity {
                        key,
                        step: s,
                        from: notes[i].3,
                        y: start.unwrap().y,
                    })
                }
                None => {
                    st.drums.drag = Some(Drag::Paint { key });
                    toggle = Some((key, s));
                }
            }
        }
    }
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            match st.drums.drag {
                Some(Drag::Paint { key }) => {
                    if let Some((_, s)) = cell_at(pos) {
                        if hit_in(&st.song.patterns[pi].notes, key, s * res, s * res + res)
                            .is_none()
                        {
                            toggle = Some((key, s));
                        }
                    }
                }
                Some(Drag::Velocity { key, step, from, y }) => {
                    let v = (from as f32 + (y - pos.y) * 0.8).round().clamp(1.0, 127.0) as u8;
                    if let Some(i) = hit_in(
                        &st.song.patterns[pi].notes,
                        key,
                        step * res,
                        step * res + res,
                    ) {
                        st.song.patterns[pi].notes[i].3 = v;
                        st.drums.default_vel = v;
                    }
                    ui.painter().text(
                        pos + vec2(12.0, -12.0),
                        Align2::LEFT_BOTTOM,
                        format!("{v}"),
                        theme::font_semi(12.0),
                        ACCENT,
                    );
                }
                None => {}
            }
        }
    }
    if resp.drag_stopped() {
        st.drums.drag = None;
    }
    if let Some((key, s)) = toggle {
        let t0 = s * res;
        let pat = &mut st.song.patterns[pi];
        let before = pat.notes.len();
        pat.notes
            .retain(|n| !(n.2 == key && n.0 >= t0 && n.0 < t0 + res));
        if pat.notes.len() == before {
            pat.notes.push(Note(t0, res, key, st.drums.default_vel));
            pat.notes.sort_by_key(|n| (n.0, n.2));
            audition_key = Some(key);
        }
    }
    if let Some(k) = audition_key {
        if let Some((t, old, _)) = st.drums.sounding.take() {
            st.send(Command::NoteOff { track: t, key: old });
        }
        st.send(Command::NoteOn {
            track,
            key: k,
            vel: st.drums.default_vel,
        });
        st.drums.sounding = Some((track, k, now + 0.3));
    }
    if let Some((t, k, until)) = st.drums.sounding {
        if now > until {
            st.send(Command::NoteOff { track: t, key: k });
            st.drums.sounding = None;
        }
    }
}

fn playhead_tick(st: &Studio, pi: usize, ticks: u32) -> Option<u32> {
    let s = &st.status;
    if !s.playing {
        return None;
    }
    match st.mode {
        PlayMode::Pattern if st.sel.pattern == Some(pi) => Some(s.pattern_tick % ticks.max(1)),
        _ => st.sel.clip.and_then(|(si, ci)| {
            if s.section != Some(si) {
                return None;
            }
            let c = st.song.sections.get(si)?.clips.get(ci)?;
            let into = s.section_tick.checked_sub(c.at * PPQ)?;
            let span = c.span(ticks, st.song.section_ticks(&st.song.sections[si]));
            (into < span).then_some(into % ticks.max(1))
        }),
    }
}

fn toolbar(ui: &mut egui::Ui, st: &mut Studio, pi: usize) {
    egui::Frame::new()
        .fill(BG2)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let names: Vec<String> = st.song.patterns.iter().map(|p| p.name.clone()).collect();
                let mut sel = pi;
                egui::ComboBox::from_id_salt("drum-pattern")
                    .width(130.0)
                    .selected_text(names[pi].clone())
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut sel, i, n);
                        }
                    });
                if sel != pi {
                    st.sel.pattern = Some(sel);
                }
                ui.label(egui::RichText::new("plays on").color(FAINT));
                let tnames: Vec<String> = st.song.tracks.iter().map(|t| t.name.clone()).collect();
                if !tnames.is_empty() {
                    let cur = st.sel.pattern_track.min(tnames.len() - 1);
                    egui::ComboBox::from_id_salt("drum-track")
                        .width(90.0)
                        .selected_text(tnames[cur].clone())
                        .show_ui(ui, |ui| {
                            for (i, n) in tnames.iter().enumerate() {
                                ui.selectable_value(&mut st.sel.pattern_track, i, n);
                            }
                        });
                }
                ui.label(egui::RichText::new("length").color(FAINT));
                ui.add(
                    egui::DragValue::new(&mut st.song.patterns[pi].beats)
                        .range(1..=256)
                        .suffix(" beats"),
                );
                ui.label(egui::RichText::new("steps").color(FAINT));
                egui::ComboBox::from_id_salt("drum-res")
                    .width(92.0)
                    .selected_text(RES[st.drums.res].0)
                    .show_ui(ui, |ui| {
                        for (i, r) in RES.iter().enumerate() {
                            ui.selectable_value(&mut st.drums.res, i, r.0);
                        }
                    });
                ui.label(egui::RichText::new("new hits").color(FAINT));
                ui.add(egui::DragValue::new(&mut st.drums.default_vel).range(1..=127))
                    .on_hover_text("Velocity of new hits; drag a hit up or down to change its own");
                let hearing = st.mode == PlayMode::Pattern;
                if crate::widgets::toggle(ui, hearing, "Loop pattern", ACCENT).clicked() {
                    st.mode = if hearing {
                        PlayMode::Song
                    } else {
                        PlayMode::Pattern
                    };
                    if !hearing && !st.status.playing {
                        st.toggle_play();
                    }
                }
                if ui.small_button("Duplicate").clicked() {
                    let d = songops::duplicate_pattern(&mut st.song, pi);
                    st.sel.pattern = Some(d);
                }
                if ui.small_button("Clear").clicked() {
                    st.song.patterns[pi].notes.clear();
                }
                ui.label(
                    egui::RichText::new(format!("{} hits", st.song.patterns[pi].notes.len()))
                        .color(FAINT)
                        .small(),
                );
            });
        });
}
