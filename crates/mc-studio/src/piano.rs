//! The piano roll: one pattern's notes against a keyboard, with the song's
//! scale shaded in, the other clips of the same section as faint ghosts, a
//! velocity lane and an automation lane underneath.
//!
//! Gestures follow the common DAW grammar: click empty space to draw a note of
//! the last length, drag a note to move it (snapped), drag its right edge to
//! resize, drag empty space to rubber-band select, Shift adds to the
//! selection, right-click erases. Every drag works from a copy of the notes
//! taken when it started, so it never accumulates rounding.

use crate::app::{Pane, PlayMode, Studio};
use crate::notes::{self, grid_ticks, snap_floor, snap_round, GRIDS};
use crate::songops;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT, WARN};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
};
use mc_music::song::{key_name, Automation, Target};
use mc_music::{Command, Note, PPQ};

pub struct State {
    pub fit_next: bool,
    ppb: f32,
    key_h: f32,
    scroll_tick: f32,
    /// The key whose row starts at the top edge.
    top_key: f32,
    pub grid: usize,
    last_len: u32,
    last_vel: u8,
    sel: Vec<usize>,
    /// The pattern the selection indexes into.
    sel_pattern: Option<usize>,
    drag: Option<Drag>,
    clipboard: Vec<Note>,
    /// A key sounding from a click, and when to release it.
    sounding: Option<(usize, u8, f64)>,
    paste_at: u32,
    auto_target: Target,
    auto_drag: Option<usize>,
    shown_pattern: Option<usize>,
    shown_audition: Option<String>,
    /// The grid size the view was last fitted to, and whether it has been moved by hand since.
    fit_size: egui::Vec2,
    user_view: bool,
    /// The last frame's mapping, for placing synthetic input in tests.
    last_view: Option<View>,
}

impl Default for State {
    fn default() -> State {
        State {
            fit_next: true,
            ppb: 64.0,
            key_h: 12.0,
            scroll_tick: 0.0,
            top_key: 84.0,
            grid: 3,
            last_len: PPQ / 4,
            last_vel: 100,
            sel: Vec::new(),
            sel_pattern: None,
            drag: None,
            clipboard: Vec::new(),
            sounding: None,
            paste_at: 0,
            auto_target: Target::Cutoff,
            auto_drag: None,
            shown_pattern: None,
            shown_audition: None,
            fit_size: egui::Vec2::ZERO,
            user_view: false,
            last_view: None,
        }
    }
}

enum Drag {
    Move {
        orig: Vec<Note>,
        anchor: usize,
        grab_tick: i64,
        grab_key: i32,
        last_key: i32,
    },
    Resize {
        orig: Vec<Note>,
        anchor: usize,
        grab_tick: i64,
    },
    Rubber {
        start: Pos2,
        before: Vec<usize>,
    },
    Velocity,
}

impl State {
    /// Where a tick and key were drawn last frame (the middle of the row).
    #[cfg(test)]
    pub fn screen_pos(&self, tick: u32, key: u8) -> Option<Pos2> {
        let v = self.last_view?;
        Some(pos2(v.x(tick as f32), v.y(key) + v.kh * 0.5))
    }

    /// Selected note indices into the open pattern.
    pub fn selected(&self) -> Vec<usize> {
        self.sel.clone()
    }
}

const KEYS_W: f32 = 58.0;
const VEL_H: f32 = 54.0;
const AUTO_H: f32 = 64.0;
const RULER_H: f32 = 18.0;

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(pi) = st.sel.pattern.filter(|&p| p < st.song.patterns.len()) else {
        empty(ui, st);
        return;
    };
    if st.piano.sel_pattern != Some(pi) {
        st.piano.sel.clear();
        st.piano.sel_pattern = Some(pi);
    }
    if st.piano.shown_pattern != Some(pi) || st.piano.shown_audition != st.collab.audition {
        st.piano.shown_pattern = Some(pi);
        st.piano.shown_audition = st.collab.audition.clone();
        st.piano.fit_next = true;
        st.piano.user_view = false;
    }
    toolbar(ui, st, pi);
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    if area.height() < 80.0 {
        return;
    }
    let keys = Rect::from_min_max(
        pos2(area.left(), area.top() + RULER_H),
        pos2(area.left() + KEYS_W, area.bottom() - VEL_H - AUTO_H),
    );
    let grid = Rect::from_min_max(
        pos2(keys.right(), keys.top()),
        pos2(area.right(), keys.bottom()),
    );
    let ruler = Rect::from_min_max(
        pos2(grid.left(), area.top()),
        pos2(grid.right(), grid.top()),
    );
    let vel = Rect::from_min_max(
        pos2(grid.left(), grid.bottom()),
        pos2(grid.right(), grid.bottom() + VEL_H),
    );
    let auto = Rect::from_min_max(
        pos2(grid.left(), vel.bottom()),
        pos2(grid.right(), area.bottom()),
    );
    let side_vel = Rect::from_min_max(
        pos2(area.left(), vel.top()),
        pos2(keys.right(), vel.bottom()),
    );
    let side_auto = Rect::from_min_max(
        pos2(area.left(), auto.top()),
        pos2(keys.right(), auto.bottom()),
    );

    let ticks = st.song.patterns[pi].ticks().max(PPQ);
    view_input(ui, st, area, grid, ticks, pi);
    let v = View {
        grid,
        ppt: st.piano.ppb / PPQ as f32,
        scroll: st.piano.scroll_tick,
        top: st.piano.top_key,
        kh: st.piano.key_h,
    };

    st.piano.last_view = Some(v);
    let p = ui.painter().with_clip_rect(area);
    p.rect_filled(area, CornerRadius::ZERO, theme::BG1);
    background(&p, st, &v, ticks, ruler, vel, auto);
    ghosts(&p, st, &v, pi, ticks);
    proposal_ghosts(&p, st, &v, pi);
    notes_layer(&p, st, &v, pi);
    grid_input(ui, st, &v, pi, ticks);
    keyboard(ui, st, keys, &v);
    velocity_lane(ui, st, vel, side_vel, &v, pi);
    automation_lane(ui, st, auto, side_auto, &v, pi, ticks);
    playhead(ui, st, &v, pi, ticks, area);
    shortcuts(ui, st, pi, ticks);
    release_sounding(ui, st);
}

fn empty(ui: &mut egui::Ui, st: &mut Studio) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("No pattern open")
                .font(theme::font_light(22.0))
                .color(DIM),
        );
        ui.label(
            egui::RichText::new(
                "Click a clip in the arrangement, or pick a pattern in the list on the left.",
            )
            .color(FAINT),
        );
        if ui.button("New pattern").clicked() {
            let bpb = st.song.beats_per_bar;
            let i = songops::new_pattern(&mut st.song, "Pattern", bpb);
            st.sel.pattern = Some(i);
        }
    });
}

#[derive(Clone, Copy)]
struct View {
    grid: Rect,
    ppt: f32,
    scroll: f32,
    top: f32,
    kh: f32,
}

impl View {
    fn x(&self, tick: f32) -> f32 {
        self.grid.left() + (tick - self.scroll) * self.ppt
    }
    fn tick(&self, x: f32) -> f32 {
        (x - self.grid.left()) / self.ppt + self.scroll
    }
    fn y(&self, key: u8) -> f32 {
        self.grid.top() + (self.top - key as f32) * self.kh
    }
    fn key(&self, y: f32) -> i32 {
        (self.top - (y - self.grid.top()) / self.kh).ceil() as i32
    }
    fn note_rect(&self, n: &Note) -> Rect {
        let x0 = self.x(n.0 as f32);
        let x1 = self.x(n.end() as f32).max(x0 + 3.0);
        let y = self.y(n.2);
        Rect::from_min_max(pos2(x0, y + 0.5), pos2(x1, y + self.kh - 0.5))
    }
}

fn toolbar(ui: &mut egui::Ui, st: &mut Studio, pi: usize) {
    egui::Frame::new().fill(BG2).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let names: Vec<String> = st.song.patterns.iter().map(|p| p.name.clone()).collect();
            let mut sel = pi;
            egui::ComboBox::from_id_salt("piano-pattern").width(130.0).selected_text(names[pi].clone()).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut sel, i, n);
                }
            });
            if sel != pi {
                st.sel.pattern = Some(sel);
            }
            let mut name = st.song.patterns[pi].name.clone();
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(110.0)).on_hover_text("Rename (clips follow)");
            if r.changed() && !name.trim().is_empty() {
                songops::rename_pattern(&mut st.song, pi, name.trim());
            }
            ui.label(egui::RichText::new("plays on").color(FAINT));
            let tnames: Vec<String> = st.song.tracks.iter().map(|t| t.name.clone()).collect();
            if !tnames.is_empty() {
                let cur = st.sel.pattern_track.min(tnames.len() - 1);
                egui::ComboBox::from_id_salt("piano-track").width(90.0).selected_text(tnames[cur].clone()).show_ui(ui, |ui| {
                    for (i, n) in tnames.iter().enumerate() {
                        ui.selectable_value(&mut st.sel.pattern_track, i, n);
                    }
                });
            }
            ui.label(egui::RichText::new("length").color(FAINT));
            ui.add(egui::DragValue::new(&mut st.song.patterns[pi].beats).range(1..=256).suffix(" beats"));
            ui.label(egui::RichText::new("snap").color(FAINT));
            egui::ComboBox::from_id_salt("piano-grid").width(92.0).selected_text(GRIDS[st.piano.grid].0).show_ui(ui, |ui| {
                for (i, g) in GRIDS.iter().enumerate() {
                    ui.selectable_value(&mut st.piano.grid, i, g.0);
                }
            });
            if ui.button("Quantize").on_hover_text("Snap the selected notes' starts to the grid (all notes if none selected); Ctrl+Q").clicked() {
                quantize(st, pi);
            }
            let hearing = st.mode == PlayMode::Pattern;
            if crate::widgets::toggle(ui, hearing, "Loop pattern", ACCENT).on_hover_text("Play just this pattern on its track").clicked() {
                st.mode = if hearing { PlayMode::Song } else { PlayMode::Pattern };
                if !hearing && !st.status.playing {
                    st.toggle_play();
                }
            }
            if ui.small_button("Duplicate").on_hover_text("A copy of the pattern to edit separately").clicked() {
                let d = songops::duplicate_pattern(&mut st.song, pi);
                st.sel.pattern = Some(d);
            }
            let n = st.song.patterns[pi].notes.len();
            let s = st.piano.sel.len();
            ui.label(egui::RichText::new(if s > 0 { format!("{s} of {n} notes selected") } else { format!("{n} notes") }).color(FAINT).small());
            if st.collab.changed_patterns.contains(&st.song.patterns[pi].name) {
                ui.label(egui::RichText::new("changed in the proposal").color(WARN).small());
            }
        });
    });
}

fn quantize(st: &mut Studio, pi: usize) {
    let g = grid_ticks(st.piano.grid, st.song.bar_ticks());
    let ticks = st.song.patterns[pi].ticks();
    let sel = st.piano.sel.clone();
    notes::quantize(&mut st.song.patterns[pi].notes, &sel, g, ticks);
    st.piano.sel = notes::normalise(&mut st.song.patterns[pi].notes, &sel);
}

/// Zoom and scroll: wheel scrolls keys, Shift+wheel time, Ctrl+wheel zooms
/// time, Alt+wheel zooms keys.
fn view_input(ui: &mut egui::Ui, st: &mut Studio, area: Rect, grid: Rect, ticks: u32, pi: usize) {
    let s = &mut st.piano;
    // The pane settles over the first frames (and when resized): fit again
    // until the view has been scrolled or zoomed by hand.
    if !s.user_view
        && ((grid.height() - s.fit_size.y).abs() > 30.0
            || (grid.width() - s.fit_size.x).abs() > 30.0)
    {
        s.fit_next = true;
    }
    if s.fit_next && grid.width() > 50.0 {
        s.fit_next = false;
        s.fit_size = grid.size();
        s.ppb = ((grid.width() - 20.0) / (ticks as f32 / PPQ as f32)).clamp(8.0, 400.0);
        s.scroll_tick = 0.0;
        // Fit the notes, and a proposal's version of them when one is heard.
        let name = &st.song.patterns[pi].name;
        let mut notes = st.song.patterns[pi].notes.clone();
        if let Some(p) = st
            .collab
            .audition_song()
            .and_then(|song| song.pattern(name).map(|i| &song.patterns[i]))
        {
            notes.extend(p.notes.iter().copied());
        }
        let rows = grid.height() / s.key_h;
        let centre = if notes.is_empty() {
            60.0
        } else {
            let (lo, hi) = notes
                .iter()
                .fold((127u8, 0u8), |(l, h), n| (l.min(n.2), h.max(n.2)));
            if (hi - lo) as f32 + 4.0 > rows {
                s.key_h = (grid.height() / ((hi - lo) as f32 + 6.0)).clamp(5.0, 12.0);
            }
            (lo as f32 + hi as f32) * 0.5
        };
        let rows = grid.height() / s.key_h;
        s.top_key = (centre + rows * 0.5).clamp(rows, 127.0);
    }
    if ui.rect_contains_pointer(area) {
        let (scroll, zoom, hover, alt) = ui.input(|i| {
            (
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.pointer.hover_pos(),
                i.modifiers.alt,
            )
        });
        if zoom != 1.0 || scroll != egui::Vec2::ZERO {
            s.user_view = true;
        }
        if zoom != 1.0 {
            if let Some(h) = hover {
                let t = s.scroll_tick + (h.x - grid.left()) / (s.ppb / PPQ as f32);
                s.ppb = (s.ppb * zoom).clamp(4.0, 1200.0);
                s.scroll_tick = t - (h.x - grid.left()) / (s.ppb / PPQ as f32);
            }
        } else if alt && scroll.y != 0.0 {
            let old = s.key_h;
            s.key_h = (s.key_h * (1.0 + scroll.y * 0.004)).clamp(5.0, 28.0);
            if let Some(h) = hover {
                let k = s.top_key - (h.y - grid.top()) / old;
                s.top_key = k + (h.y - grid.top()) / s.key_h;
            }
        } else {
            s.scroll_tick -= scroll.x / (s.ppb / PPQ as f32);
            s.top_key += scroll.y / s.key_h;
        }
    }
    let rows = grid.height() / s.key_h;
    s.top_key = s.top_key.clamp(rows.min(127.0), 128.0);
    let vis_ticks = grid.width() / (s.ppb / PPQ as f32);
    s.scroll_tick = s
        .scroll_tick
        .clamp(-(PPQ as f32), (ticks as f32 - vis_ticks * 0.5).max(0.0));
}

fn background(
    p: &egui::Painter,
    st: &Studio,
    v: &View,
    ticks: u32,
    ruler: Rect,
    vel: Rect,
    auto: Rect,
) {
    let g = v.grid;
    let root = st.song.root;
    let scale = st.song.scale;
    let lo = v.key(g.bottom()).max(0);
    let hi = v.key(g.top()).min(127);
    for k in lo..=hi {
        let k = k as u8;
        let r = Rect::from_min_size(pos2(g.left(), v.y(k)), vec2(g.width(), v.kh));
        let in_scale = scale.contains(root, k);
        let fill = if k % 12 == root % 12 {
            theme::mix(BG2, ACCENT, 0.08)
        } else if in_scale {
            Color32::from_rgb(0x19, 0x19, 0x1D)
        } else {
            Color32::from_rgb(0x12, 0x12, 0x15)
        };
        p.rect_filled(r, CornerRadius::ZERO, fill);
        if k % 12 == 0 {
            p.hline(g.x_range(), r.bottom(), Stroke::new(1.0, theme::line(26)));
        } else if v.kh >= 8.0 {
            p.hline(g.x_range(), r.bottom(), Stroke::new(1.0, theme::line(6)));
        }
    }
    // Past the pattern's end.
    let xe = v.x(ticks as f32);
    if xe < g.right() {
        let r = Rect::from_min_max(pos2(xe.max(g.left()), g.top()), g.max);
        p.rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(110));
        p.rect_filled(
            Rect::from_min_max(pos2(xe.max(g.left()), vel.top()), auto.max),
            CornerRadius::ZERO,
            Color32::from_black_alpha(110),
        );
    }
    // Time grid: bars, beats and the snap.
    let bar = st.song.bar_ticks();
    let snap = grid_ticks(st.piano.grid, bar);
    let step = if snap as f32 * v.ppt >= 6.0 {
        snap
    } else if (PPQ as f32) * v.ppt >= 6.0 {
        PPQ
    } else {
        bar
    };
    let t0 = snap_floor(v.tick(g.left()).max(0.0) as i64, step) as u32;
    let t1 = v.tick(g.right()) as u32;
    p.rect_filled(ruler, CornerRadius::ZERO, BG0);
    let mut t = t0;
    while t <= t1.min(ticks + bar * 4) {
        let x = v.x(t as f32);
        let a = if t % bar == 0 {
            34
        } else if t % PPQ == 0 {
            14
        } else {
            6
        };
        p.vline(x, g.top()..=auto.bottom(), Stroke::new(1.0, theme::line(a)));
        if t % bar == 0 {
            p.text(
                pos2(x + 3.0, ruler.center().y),
                Align2::LEFT_CENTER,
                format!("{}", t / bar + 1),
                theme::font_semi(11.0),
                DIM,
            );
        } else if t % PPQ == 0 && v.ppt * PPQ as f32 > 36.0 {
            let beat = (t % bar) / PPQ + 1;
            p.text(
                pos2(x + 3.0, ruler.center().y),
                Align2::LEFT_CENTER,
                format!("{}.{}", t / bar + 1, beat),
                theme::font_body(10.0),
                FAINT,
            );
        }
        t += step;
    }
    p.hline(g.x_range(), vel.top(), Stroke::new(1.0, theme::line(30)));
    p.hline(g.x_range(), auto.top(), Stroke::new(1.0, theme::line(30)));
    p.rect_filled(vel, CornerRadius::ZERO, Color32::from_black_alpha(40));
}

/// The other clips of the open clip's section, in their tracks' colours.
fn ghosts(p: &egui::Painter, st: &Studio, v: &View, pi: usize, ticks: u32) {
    let Some((si, ci)) = st.sel.clip else { return };
    let Some(sec) = st.song.sections.get(si) else {
        return;
    };
    let Some(this) = sec.clips.get(ci) else {
        return;
    };
    if st.song.pattern(&this.pattern) != Some(pi) {
        return;
    }
    let sec_ticks = st.song.section_ticks(sec);
    let clip_start = this.at * PPQ;
    let vis = v.grid;
    for (k, c) in sec.clips.iter().enumerate() {
        if k == ci {
            continue;
        }
        let (Some(opi), Some(ti)) = (st.song.pattern(&c.pattern), st.song.track(&c.track)) else {
            continue;
        };
        if st.is_kit(ti) && !st.is_kit(st.sel.pattern_track) {
            continue;
        }
        let pat = &st.song.patterns[opi];
        let pt = pat.ticks().max(1);
        let span = c.span(pt, sec_ticks);
        let colour = theme::with_alpha(theme::rgb(st.song.tracks[ti].colour), 46);
        let mut rep = 0;
        while rep * pt < span {
            let base = (c.at * PPQ + rep * pt) as i64 - clip_start as i64;
            if base >= ticks as i64 {
                break;
            }
            for n in &pat.notes {
                if n.0 >= pt {
                    continue;
                }
                let at = base + n.0 as i64;
                if at < 0 || at >= ticks as i64 {
                    continue;
                }
                let key =
                    (n.2 as i32 + c.transpose as i32 - this.transpose as i32).clamp(0, 127) as u8;
                let r = v.note_rect(&Note(at as u32, n.1, key, n.3));
                if r.intersects(vis) {
                    p.rect_filled(r.shrink(1.0), CornerRadius::same(1), colour);
                }
            }
            rep += 1;
        }
    }
}

/// While a proposal is heard (or waiting), its version of this pattern as outlines.
fn proposal_ghosts(p: &egui::Painter, st: &Studio, v: &View, pi: usize) {
    let name = &st.song.patterns[pi].name;
    let Some(song) = st.collab.audition_song() else {
        return;
    };
    let Some(other) = song.pattern(name).map(|i| &song.patterns[i]) else {
        return;
    };
    if *other == st.song.patterns[pi] {
        return;
    }
    for n in &other.notes {
        let r = v.note_rect(n);
        if r.intersects(v.grid) {
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.5, WARN),
                StrokeKind::Inside,
            );
        }
    }
}

fn notes_layer(p: &egui::Painter, st: &Studio, v: &View, pi: usize) {
    let pat = &st.song.patterns[pi];
    let colour = st
        .song
        .tracks
        .get(st.sel.pattern_track)
        .map(|t| theme::rgb(t.colour))
        .unwrap_or(ACCENT);
    let gp = p.with_clip_rect(v.grid);
    for (i, n) in pat.notes.iter().enumerate() {
        let r = v.note_rect(n);
        if !r.intersects(v.grid) {
            continue;
        }
        let selected = st.piano.sel.contains(&i);
        let vel = n.3 as f32 / 127.0;
        let base = if selected { ACCENT } else { colour };
        let fill = theme::mix(theme::mix(BG2, base, 0.55), base, vel * 0.6);
        gp.rect(
            r,
            CornerRadius::same(2),
            fill,
            Stroke::new(
                1.0,
                if selected {
                    TEXT
                } else {
                    theme::mix(base, Color32::BLACK, 0.4)
                },
            ),
            StrokeKind::Inside,
        );
        // A velocity tick along the top.
        let w = (r.width() - 4.0) * vel;
        if w > 1.0 && r.height() > 6.0 {
            gp.hline(
                r.left() + 2.0..=r.left() + 2.0 + w,
                r.top() + 2.0,
                Stroke::new(1.0, Color32::from_white_alpha(120)),
            );
        }
        if r.width() > 34.0 && v.kh >= 11.0 {
            gp.text(
                r.left_center() + vec2(3.0, 0.5),
                Align2::LEFT_CENTER,
                key_name(n.2),
                theme::font_body(9.5),
                Color32::from_black_alpha(200),
            );
        }
    }
    if let Some(Drag::Rubber { start, .. }) = &st.piano.drag {
        if let Some(cur) = p.ctx().input(|i| i.pointer.hover_pos()) {
            let r = Rect::from_two_pos(*start, cur);
            gp.rect(
                r,
                CornerRadius::ZERO,
                theme::with_alpha(ACCENT, 24),
                Stroke::new(1.0, ACCENT),
                StrokeKind::Inside,
            );
        }
    }
}

fn audition(st: &mut Studio, key: u8, vel: u8, now: f64, hold: f64) {
    let track = st.sel.pattern_track;
    if let Some((t, k, _)) = st.piano.sounding.take() {
        st.send(Command::NoteOff { track: t, key: k });
    }
    st.send(Command::NoteOn { track, key, vel });
    st.piano.sounding = Some((track, key, now + hold));
}

fn release_sounding(ui: &egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    let held = ui.input(|i| i.pointer.any_down());
    if let Some((t, k, until)) = st.piano.sounding {
        if now >= until && !held {
            st.send(Command::NoteOff { track: t, key: k });
            st.piano.sounding = None;
        }
    }
}

fn grid_input(ui: &mut egui::Ui, st: &mut Studio, v: &View, pi: usize, ticks: u32) {
    let resp = ui.interact(v.grid, ui.id().with("piano-grid"), Sense::click_and_drag());
    let now = ui.input(|i| i.time);
    let shift = ui.input(|i| i.modifiers.shift);
    let snap = !ui.input(|i| i.modifiers.alt);
    let g = if snap {
        grid_ticks(st.piano.grid, st.song.bar_ticks())
    } else {
        1
    };
    let hover = resp.hover_pos();
    // Hit test: the topmost note under the pointer, and whether it is its right edge.
    let hit = |st: &Studio, pos: Pos2| -> Option<(usize, bool)> {
        let pat = &st.song.patterns[pi];
        for (i, n) in pat.notes.iter().enumerate().rev() {
            let r = v.note_rect(n);
            if r.contains(pos) {
                let edge = pos.x > r.right() - (r.width() * 0.25).clamp(3.0, 8.0);
                return Some((i, edge));
            }
        }
        None
    };
    if let Some(h) = hover {
        match hit(st, h) {
            Some((_, true)) => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
            Some((_, false)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
            None => {}
        }
    }
    let press = resp.drag_started() || resp.clicked() || resp.secondary_clicked();
    let pos = resp.interact_pointer_pos();
    if let (true, Some(pos)) = (press, pos) {
        st.focus = Pane::Detail;
        st.piano.paste_at = snap_floor(v.tick(pos.x).max(0.0) as i64, g) as u32;
    }
    if resp.secondary_clicked() {
        if let Some(pos) = pos {
            match hit(st, pos) {
                Some((i, _)) => {
                    let del = if st.piano.sel.contains(&i) {
                        st.piano.sel.clone()
                    } else {
                        vec![i]
                    };
                    notes::delete(&mut st.song.patterns[pi].notes, &del);
                    st.piano.sel.clear();
                }
                None => st.piano.sel.clear(),
            }
        }
        return;
    }
    if resp.clicked() {
        if let Some(pos) = pos {
            match hit(st, pos) {
                Some((i, _)) => {
                    if shift {
                        if let Some(k) = st.piano.sel.iter().position(|&s| s == i) {
                            st.piano.sel.remove(k);
                        } else {
                            st.piano.sel.push(i);
                        }
                    } else {
                        st.piano.sel = vec![i];
                    }
                    let n = st.song.patterns[pi].notes[i];
                    st.piano.last_len = n.1;
                    st.piano.last_vel = n.3;
                    audition(st, n.2, n.3, now, 0.25);
                }
                None if !shift => {
                    let t = snap_floor(v.tick(pos.x).max(0.0) as i64, g) as u32;
                    let k = v.key(pos.y).clamp(0, 127) as u8;
                    if t < ticks {
                        let len = st.piano.last_len.max(1);
                        let pat = &mut st.song.patterns[pi];
                        pat.notes.push(Note(t, len, k, st.piano.last_vel));
                        let i = pat.notes.len() - 1;
                        st.piano.sel = notes::normalise(&mut pat.notes, &[i]);
                        audition(st, k, st.piano.last_vel, now, 0.25);
                    }
                }
                None => {}
            }
        }
    }
    if resp.drag_started() {
        if let Some(pos) = resp.interact_pointer_pos() {
            // Where the button went down, not where the drag was recognised:
            // otherwise the note lags the pointer by the drag threshold.
            let pos = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
            match hit(st, pos) {
                Some((i, edge)) => {
                    if !st.piano.sel.contains(&i) {
                        if shift {
                            st.piano.sel.push(i);
                        } else {
                            st.piano.sel = vec![i];
                        }
                    }
                    let ctrl = ui.input(|i| i.modifiers.command);
                    let mut anchor = i;
                    if ctrl && !edge {
                        // Ctrl-drag moves copies and leaves the originals.
                        let pat = &mut st.song.patterns[pi];
                        let copies: Vec<Note> =
                            st.piano.sel.iter().map(|&s| pat.notes[s]).collect();
                        let start = pat.notes.len();
                        let pos_in_sel = st.piano.sel.iter().position(|&s| s == i).unwrap_or(0);
                        pat.notes.extend(copies);
                        st.piano.sel = (start..pat.notes.len()).collect();
                        anchor = start + pos_in_sel;
                    }
                    let orig = st.song.patterns[pi].notes.clone();
                    let n = orig[anchor];
                    if edge {
                        st.piano.drag = Some(Drag::Resize {
                            orig,
                            anchor,
                            grab_tick: n.end() as i64,
                        });
                    } else {
                        let grab_tick = v.tick(pos.x) as i64;
                        let grab_key = n.2 as i32;
                        st.piano.drag = Some(Drag::Move {
                            orig,
                            anchor,
                            grab_tick,
                            grab_key,
                            last_key: grab_key,
                        });
                        audition(st, n.2, n.3, now, 0.05);
                    }
                }
                None => {
                    let before = if shift {
                        st.piano.sel.clone()
                    } else {
                        Vec::new()
                    };
                    st.piano.drag = Some(Drag::Rubber { start: pos, before });
                }
            }
        }
    }
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let sel = st.piano.sel.clone();
            match &mut st.piano.drag {
                Some(Drag::Move {
                    orig,
                    anchor,
                    grab_tick,
                    grab_key,
                    last_key,
                }) => {
                    let n = orig[*anchor];
                    let raw = v.tick(pos.x) as i64 - *grab_tick;
                    let new_at = if snap {
                        snap_round(n.0 as i64 + raw, g)
                    } else {
                        n.0 as i64 + raw
                    };
                    let dt = new_at - n.0 as i64;
                    let key = v.key(pos.y).clamp(0, 127);
                    let dk = key - *grab_key;
                    let mut moved = orig.clone();
                    notes::move_notes(&mut moved, &sel, dt, dk, ticks);
                    let sounding_key = moved[*anchor].2 as i32;
                    let vel = moved[*anchor].3;
                    let changed_key = sounding_key != *last_key;
                    *last_key = sounding_key;
                    st.song.patterns[pi].notes = moved;
                    if changed_key {
                        audition(st, sounding_key as u8, vel, now, 0.05);
                    }
                }
                Some(Drag::Resize {
                    orig,
                    anchor,
                    grab_tick,
                }) => {
                    let n = orig[*anchor];
                    let raw = v.tick(pos.x) as i64;
                    let new_end =
                        if snap { snap_round(raw, g) } else { raw }.max(n.0 as i64 + g as i64);
                    let dlen = new_end - *grab_tick;
                    let mut resized = orig.clone();
                    notes::resize_notes(&mut resized, &sel, dlen, g.min(PPQ / 8).max(1));
                    st.piano.last_len = resized[*anchor].1;
                    st.song.patterns[pi].notes = resized;
                }
                Some(Drag::Rubber { start, before }) => {
                    let r = Rect::from_two_pos(*start, pos);
                    let t0 = v.tick(r.left()).max(0.0) as u32;
                    let t1 = v.tick(r.right()).max(0.0) as u32;
                    let k1 = v.key(r.top()).clamp(0, 127) as u8;
                    let k0 = v.key(r.bottom()).clamp(0, 127) as u8;
                    let mut s = before.clone();
                    for i in notes::in_rect(&st.song.patterns[pi].notes, t0, t1.max(t0 + 1), k0, k1)
                    {
                        if !s.contains(&i) {
                            s.push(i);
                        }
                    }
                    st.piano.sel = s;
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        if matches!(st.piano.drag, Some(Drag::Move { .. } | Drag::Resize { .. })) {
            let sel = st.piano.sel.clone();
            st.piano.sel = notes::normalise(&mut st.song.patterns[pi].notes, &sel);
        }
        st.piano.drag = None;
    }
}

fn keyboard(ui: &mut egui::Ui, st: &mut Studio, keys: Rect, v: &View) {
    let p = ui.painter().with_clip_rect(keys);
    p.rect_filled(keys, CornerRadius::ZERO, BG0);
    let lo = v.key(keys.bottom()).max(0);
    let hi = v.key(keys.top()).min(127);
    let resp = ui.interact(keys, ui.id().with("piano-keys"), Sense::click_and_drag());
    let pressed_key = resp
        .interact_pointer_pos()
        .filter(|_| resp.is_pointer_button_down_on())
        .map(|p| v.key(p.y).clamp(0, 127) as u8);
    for k in lo..=hi {
        let k = k as u8;
        let y = v.y(k);
        let r = Rect::from_min_size(pos2(keys.left(), y), vec2(keys.width(), v.kh));
        let black = is_black(k);
        let held = pressed_key == Some(k);
        let fill = if held {
            ACCENT
        } else if black {
            Color32::from_rgb(0x14, 0x14, 0x17)
        } else {
            Color32::from_rgb(0x2C, 0x2C, 0x31)
        };
        let kr = if black {
            Rect::from_min_size(r.min, vec2(keys.width() * 0.62, r.height()))
        } else {
            r
        };
        p.rect_filled(kr.shrink2(vec2(0.0, 0.5)), CornerRadius::same(1), fill);
        if st.song.scale.contains(st.song.root, k) && !held {
            p.rect_filled(
                Rect::from_min_size(pos2(keys.right() - 4.0, y + 1.0), vec2(3.0, v.kh - 2.0)),
                CornerRadius::ZERO,
                theme::with_alpha(ACCENT, if k % 12 == st.song.root % 12 { 220 } else { 70 }),
            );
        }
        if k % 12 == 0 || v.kh >= 14.0 && !black {
            p.text(
                pos2(keys.right() - 7.0, y + v.kh * 0.5),
                Align2::RIGHT_CENTER,
                key_name(k),
                theme::font_body(9.5),
                if k % 12 == 0 { TEXT } else { FAINT },
            );
        }
    }
    p.vline(
        keys.right() - 0.5,
        keys.y_range(),
        Stroke::new(1.0, theme::line(30)),
    );
    let now = ui.input(|i| i.time);
    if let Some(k) = pressed_key {
        if st.piano.sounding.map(|s| s.1) != Some(k) {
            audition(st, k, 100, now, 0.0);
        }
    }
}

fn velocity_lane(ui: &mut egui::Ui, st: &mut Studio, lane: Rect, side: Rect, v: &View, pi: usize) {
    let p = ui.painter().with_clip_rect(lane);
    ui.painter().text(
        side.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        "Velocity",
        theme::font_body(10.5),
        FAINT,
    );
    let colour = st
        .song
        .tracks
        .get(st.sel.pattern_track)
        .map(|t| theme::rgb(t.colour))
        .unwrap_or(ACCENT);
    let inner = lane.shrink2(vec2(0.0, 4.0));
    for (i, n) in st.song.patterns[pi].notes.iter().enumerate() {
        let x = v.x(n.0 as f32);
        if x < lane.left() - 4.0 || x > lane.right() {
            continue;
        }
        let h = inner.height() * n.3 as f32 / 127.0;
        let selected = st.piano.sel.contains(&i);
        let c = if selected { ACCENT } else { colour };
        p.vline(
            x + 1.0,
            (inner.bottom() - h)..=inner.bottom(),
            Stroke::new(2.0, c),
        );
        p.circle_filled(pos2(x + 1.0, inner.bottom() - h), 2.5, c);
    }
    let resp = ui.interact(lane, ui.id().with("vel-lane"), Sense::click_and_drag());
    if resp.drag_started() || resp.clicked() {
        st.piano.drag = Some(Drag::Velocity);
        st.focus = Pane::Detail;
    }
    if matches!(st.piano.drag, Some(Drag::Velocity)) && (resp.dragged() || resp.clicked()) {
        if let Some(pos) = resp.interact_pointer_pos() {
            let vel = (((inner.bottom() - pos.y) / inner.height()) * 127.0)
                .round()
                .clamp(1.0, 127.0) as u8;
            let sel = st.piano.sel.clone();
            let pat = &mut st.song.patterns[pi];
            // Notes starting within a few pixels of the pointer (only selected ones when there is a selection).
            for (i, n) in pat.notes.iter_mut().enumerate() {
                let x = v.x(n.0 as f32);
                if (x - pos.x).abs() <= 4.0 && (sel.is_empty() || sel.contains(&i)) {
                    n.3 = vel;
                }
            }
            st.piano.last_vel = vel;
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        st.piano.drag = None;
    }
}

fn automation_lane(
    ui: &mut egui::Ui,
    st: &mut Studio,
    lane: Rect,
    side: Rect,
    v: &View,
    pi: usize,
    ticks: u32,
) {
    // The target picker sits in the key column beside the lane.
    ui.painter().text(
        side.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        "Automation",
        theme::font_body(10.5),
        FAINT,
    );
    let mut c = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                side.min + vec2(3.0, 20.0),
                side.max - vec2(3.0, 3.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let used: Vec<Target> = st.song.patterns[pi]
        .automation
        .iter()
        .map(|a| a.target)
        .collect();
    egui::ComboBox::from_id_salt("auto-target")
        .width(side.width() - 10.0)
        .selected_text(st.piano.auto_target.name())
        .show_ui(&mut c, |ui| {
            for t in Target::ALL {
                let mark = if used.contains(&t) { " \u{2022}" } else { "" };
                ui.selectable_value(&mut st.piano.auto_target, t, format!("{}{mark}", t.name()));
            }
        });
    let target = st.piano.auto_target;
    let inner = lane.shrink2(vec2(0.0, 6.0));
    let y_of = |val: f32| inner.bottom() - inner.height() * val.clamp(0.0, 1.0);
    let val_of = |y: f32| ((inner.bottom() - y) / inner.height()).clamp(0.0, 1.0);
    let p = ui.painter().with_clip_rect(lane);
    let idx = st.song.patterns[pi]
        .automation
        .iter()
        .position(|a| a.target == target);
    let neutral = y_of(target.neutral());
    p.hline(lane.x_range(), neutral, Stroke::new(1.0, theme::line(18)));
    let colour = ACCENT;
    if let Some(ai) = idx {
        let pts = &st.song.patterns[pi].automation[ai].points;
        if !pts.is_empty() {
            let mut line = vec![pos2(lane.left(), y_of(pts[0].1))];
            line.extend(pts.iter().map(|(t, val)| pos2(v.x(*t as f32), y_of(*val))));
            line.push(pos2(
                v.x(ticks as f32).max(lane.left()),
                y_of(pts.last().unwrap().1),
            ));
            p.add(egui::Shape::line(line, Stroke::new(1.5, colour)));
            for (t, val) in pts {
                p.circle_filled(pos2(v.x(*t as f32), y_of(*val)), 3.5, TEXT);
            }
        }
    } else {
        p.text(
            lane.center(),
            Align2::CENTER_CENTER,
            format!("Click to automate {}", target.name().to_lowercase()),
            theme::font_body(11.0),
            FAINT,
        );
    }
    let resp = ui.interact(lane, ui.id().with("auto-lane"), Sense::click_and_drag());
    let g = grid_ticks(st.piano.grid, st.song.bar_ticks());
    let near = |st: &Studio, pos: Pos2| -> Option<usize> {
        let ai = st.song.patterns[pi]
            .automation
            .iter()
            .position(|a| a.target == target)?;
        st.song.patterns[pi].automation[ai]
            .points
            .iter()
            .position(|(t, val)| (pos2(v.x(*t as f32), y_of(*val)) - pos).length() < 7.0)
    };
    if resp.secondary_clicked() {
        if let (Some(pos), Some(ai)) = (resp.interact_pointer_pos(), idx) {
            if let Some(k) = near(st, pos) {
                let a = &mut st.song.patterns[pi].automation[ai];
                a.points.remove(k);
                if a.points.is_empty() {
                    st.song.patterns[pi].automation.remove(ai);
                }
            }
        }
        return;
    }
    if resp.drag_started() || resp.clicked() {
        st.focus = Pane::Detail;
        if let Some(pos) = resp.interact_pointer_pos() {
            let pos = if resp.drag_started() {
                ui.input(|i| i.pointer.press_origin()).unwrap_or(pos)
            } else {
                pos
            };
            st.piano.auto_drag = near(st, pos);
            if st.piano.auto_drag.is_none() {
                let t = (snap_round(v.tick(pos.x).max(0.0) as i64, g) as u32).min(ticks);
                let val = val_of(pos.y);
                let pat = &mut st.song.patterns[pi];
                let ai = match pat.automation.iter().position(|a| a.target == target) {
                    Some(a) => a,
                    None => {
                        pat.automation.push(Automation {
                            target,
                            points: Vec::new(),
                        });
                        pat.automation.len() - 1
                    }
                };
                let pts = &mut pat.automation[ai].points;
                pts.retain(|p| p.0 != t);
                pts.push((t, val));
                pts.sort_by_key(|p| p.0);
                st.piano.auto_drag = pts.iter().position(|p| p.0 == t);
            }
        }
    }
    if resp.dragged() {
        if let (Some(pos), Some(k)) = (resp.interact_pointer_pos(), st.piano.auto_drag) {
            if let Some(ai) = st.song.patterns[pi]
                .automation
                .iter()
                .position(|a| a.target == target)
            {
                let pts = &mut st.song.patterns[pi].automation[ai].points;
                if k < pts.len() {
                    let t = (snap_round(v.tick(pos.x).max(0.0) as i64, g) as u32).min(ticks);
                    pts[k] = (t, val_of(pos.y));
                    let moved = pts[k];
                    pts.sort_by_key(|p| p.0);
                    st.piano.auto_drag = pts.iter().position(|p| *p == moved);
                }
            }
        }
    }
    if resp.drag_stopped() {
        st.piano.auto_drag = None;
    }
    if let Some(h) = resp.hover_pos() {
        let val = val_of(h.y);
        p.text(
            pos2(lane.right() - 6.0, lane.top() + 4.0),
            Align2::RIGHT_TOP,
            format!("{:.2}", val),
            theme::font_body(10.0),
            DIM,
        );
    }
}

fn playhead(ui: &mut egui::Ui, st: &Studio, v: &View, pi: usize, ticks: u32, area: Rect) {
    let s = &st.status;
    if !s.playing {
        return;
    }
    let t = match st.mode {
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
    };
    if let Some(t) = t {
        let x = v.x(t as f32);
        if x >= v.grid.left() && x <= v.grid.right() {
            ui.painter()
                .with_clip_rect(area)
                .vline(x, area.y_range(), Stroke::new(1.5, ACCENT));
        }
    }
}

fn shortcuts(ui: &mut egui::Ui, st: &mut Studio, pi: usize, ticks: u32) {
    if st.focus != Pane::Detail || Studio::text_focus(ui.ctx()) {
        return;
    }
    use egui::{Key, Modifiers};
    let g = grid_ticks(st.piano.grid, st.song.bar_ticks());
    let key = |ui: &egui::Ui, m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
    let sel = st.piano.sel.clone();
    if key(ui, Modifiers::NONE, Key::Delete) || key(ui, Modifiers::NONE, Key::Backspace) {
        notes::delete(&mut st.song.patterns[pi].notes, &sel);
        st.piano.sel.clear();
    }
    if key(ui, Modifiers::COMMAND, Key::A) {
        st.piano.sel = (0..st.song.patterns[pi].notes.len()).collect();
    }
    if key(ui, Modifiers::COMMAND, Key::D) {
        let new = notes::duplicate(&mut st.song.patterns[pi].notes, &sel, g, ticks);
        if !new.is_empty() {
            st.piano.sel = notes::normalise(&mut st.song.patterns[pi].notes, &new);
        }
    }
    if key(ui, Modifiers::COMMAND, Key::Q) {
        quantize(st, pi);
    }
    // Copy and paste arrive as events, not keys.
    let events = ui.input(|i| i.events.clone());
    for e in events {
        match e {
            egui::Event::Copy | egui::Event::Cut if !sel.is_empty() => {
                st.piano.clipboard = notes::copy(&st.song.patterns[pi].notes, &sel);
                if matches!(e, egui::Event::Cut) {
                    notes::delete(&mut st.song.patterns[pi].notes, &sel);
                    st.piano.sel.clear();
                }
            }
            egui::Event::Paste(_) if !st.piano.clipboard.is_empty() => {
                let clip = st.piano.clipboard.clone();
                let new = notes::paste(
                    &mut st.song.patterns[pi].notes,
                    &clip,
                    st.piano.paste_at,
                    ticks,
                );
                st.piano.sel = notes::normalise(&mut st.song.patterns[pi].notes, &new);
            }
            _ => {}
        }
    }
    if sel.is_empty() {
        return;
    }
    let mut dt = 0i64;
    let mut dk = 0i32;
    let mut dlen = 0i64;
    if key(ui, Modifiers::NONE, Key::ArrowLeft) {
        dt = -(g as i64);
    }
    if key(ui, Modifiers::NONE, Key::ArrowRight) {
        dt = g as i64;
    }
    if key(ui, Modifiers::SHIFT, Key::ArrowLeft) {
        dlen = -(g as i64);
    }
    if key(ui, Modifiers::SHIFT, Key::ArrowRight) {
        dlen = g as i64;
    }
    if key(ui, Modifiers::NONE, Key::ArrowUp) {
        dk = 1;
    }
    if key(ui, Modifiers::NONE, Key::ArrowDown) {
        dk = -1;
    }
    if key(ui, Modifiers::COMMAND, Key::ArrowUp) {
        dk = 12;
    }
    if key(ui, Modifiers::COMMAND, Key::ArrowDown) {
        dk = -12;
    }
    if dt != 0 || dk != 0 {
        notes::move_notes(&mut st.song.patterns[pi].notes, &sel, dt, dk, ticks);
        if dk != 0 {
            let n = st.song.patterns[pi].notes[sel[0]];
            let now = ui.input(|i| i.time);
            audition(st, n.2, n.3, now, 0.2);
        }
        st.piano.sel = notes::normalise(&mut st.song.patterns[pi].notes, &sel);
    }
    if dlen != 0 {
        notes::resize_notes(&mut st.song.patterns[pi].notes, &sel, dlen, g);
    }
}
