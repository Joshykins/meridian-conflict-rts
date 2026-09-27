//! The Workbench: the studio's default, plain view.
//!
//! The whole piece at once: one row per instrument across the entire
//! arrangement, the parts marked along the top. Clicking a part glides the
//! view in until that part fills the width and loops it; "Whole song" glides
//! back out. Each row has a speaker (mute while listening), Solo and Edit,
//! which opens that pattern in a simple editor under the rows. Moments (the
//! short pieces the game plays over the song) can be played now or dropped on
//! the timeline to fire when the playhead gets there. Everything else lives
//! behind Advanced.
//!
//! Muting and soloing here never touch the song: the engine is sent a copy
//! with the flags set, so the file on disk stays as written.
//!
//! Claude works on the song file from a chat. When the file changes, the
//! studio reloads it and keeps the part, the loop and the mutes (all by name),
//! and says what changed with a way to undo it.

use crate::app::{Studio, Tone};
use crate::moments;
use crate::theme::{self, ACCENT, BG0, BG1, BG2, BG3, DIM, FAINT, TEXT, WARN};
use crate::widgets::{self, Icon};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind,
};
use mc_music::{Mode, SectionKind, Song, PPQ};
use std::collections::{BTreeSet, HashMap};

pub struct State {
    /// The part in view, by section name.
    pub focus: Option<String>,
    /// Loop the part (true) or play on through the whole song.
    pub loop_part: bool,
    /// Tracks muted for listening only.
    pub muted: BTreeSet<String>,
    pub solo: Option<String>,
    /// The row being edited: (track, pattern).
    pub edit: Option<(String, String)>,
    /// The last pattern edited, drawn while the editor closes.
    pub edit_last: Option<(String, String)>,
    pub notice: Option<Notice>,
    /// The last notice, drawn while it fades out.
    notice_last: Option<(String, Option<String>)>,
    /// The visible stretch of the timeline, ticks, gliding from `view_from` to `view_to`.
    view_from: (f32, f32),
    view_to: (f32, f32),
    view_t0: f64,
    pub view: (f32, f32),
    pub animating: bool,
    /// The last Claude revision announced.
    announced: Option<String>,
    /// When the file last changed under us (to look for Claude's revision).
    pub external_at: Option<f64>,
    /// Announce the head revision however old it is (the --notice check).
    announce_any: bool,
    pub editor: crate::edit::State,
    /// Where things were drawn last frame (tests).
    pub chip_at: HashMap<String, Rect>,
    pub row_at: HashMap<(String, &'static str), Rect>,
    pub advanced_at: Option<Rect>,
    pub library_at: Option<Rect>,
    pub moment_at: HashMap<String, Rect>,
    pub ruler_at: Option<Rect>,
    pub marker_at: Vec<Rect>,
}

impl Default for State {
    fn default() -> State {
        State {
            focus: None,
            loop_part: true,
            muted: BTreeSet::new(),
            solo: None,
            edit: None,
            edit_last: None,
            notice: None,
            notice_last: None,
            view_from: (0.0, 1.0),
            view_to: (0.0, 1.0),
            view_t0: -10.0,
            view: (0.0, 1.0),
            animating: false,
            announced: None,
            external_at: None,
            announce_any: false,
            editor: crate::edit::State::default(),
            chip_at: HashMap::new(),
            row_at: HashMap::new(),
            advanced_at: None,
            library_at: None,
            moment_at: HashMap::new(),
            ruler_at: None,
            marker_at: Vec::new(),
        }
    }
}

impl State {
    pub fn forget_announced(&mut self) {
        self.announced = None;
        self.announce_any = true;
    }
}

pub struct Notice {
    pub text: String,
    /// Revision to go back to.
    pub undo_to: Option<String>,
    pub at: f64,
}

/// "battle_a" as "Battle A", "commander_lost" as "Commander lost".
pub fn pretty(name: &str) -> String {
    let words: Vec<String> = name
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            if w.len() == 1 || i == 0 {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            } else {
                w.to_string()
            }
        })
        .collect();
    words.join(" ")
}

// -- engine ---------------------------------------------------------------------

/// What the engine should play: the song (or the version being heard) with the
/// listening mutes applied, and the mode for the part.
pub fn engine_plan(st: &Studio, base: &Song) -> (Song, Mode) {
    let wb = &st.wb;
    let mut song = base.clone();
    if !wb.muted.is_empty() || wb.solo.is_some() {
        for t in song.tracks.iter_mut() {
            if wb.muted.contains(&t.name) {
                t.mute = true;
            }
            if let Some(s) = &wb.solo {
                t.solo = t.name == *s;
                if t.solo {
                    t.mute = false;
                }
            }
        }
    }
    let idx = wb.focus.as_ref().and_then(|n| song.section(n));
    let mode = match idx {
        Some(i) if wb.loop_part => Mode::Section(i),
        _ => Mode::Song,
    };
    (song, mode)
}

fn section_start(song: &Song, name: &str) -> Option<u32> {
    let starts = song.arrangement_starts();
    starts
        .iter()
        .find(|(_, s)| song.sections[*s].name == name)
        .map(|(t, _)| *t)
}

/// The name of the section the engine is playing.
fn playing_section(st: &Studio) -> Option<String> {
    if !st.status.playing {
        return None;
    }
    st.status
        .section
        .and_then(|i| st.sent_song().sections.get(i))
        .map(|s| s.name.clone())
}

pub fn focus(st: &mut Studio, name: &str) {
    st.wb.focus = Some(name.to_string());
    if let Some(i) = st.song.section(name) {
        st.sel.section = Some(i);
    }
    if !st.wb.loop_part {
        if let Some(t) = section_start(&st.song, name) {
            st.pending_seek = Some(t);
        }
    }
}

pub fn set_loop(st: &mut Studio, loop_part: bool) {
    if st.wb.loop_part == loop_part {
        return;
    }
    let here = playing_section(st);
    let same = here.is_some() && here == st.wb.focus;
    let into = if same { st.status.section_tick } else { 0 };
    st.wb.loop_part = loop_part;
    if loop_part {
        if same {
            st.pending_seek = Some(into);
        }
    } else if let Some(t) = st
        .wb
        .focus
        .as_ref()
        .and_then(|f| section_start(&st.song, f))
    {
        st.pending_seek = Some(t + into);
    }
}

pub fn toggle_play(st: &mut Studio) {
    // "Whole song" plays on from the part in view.
    if !st.status.playing && !st.wb.loop_part {
        if let Some(t) = st
            .wb
            .focus
            .as_ref()
            .and_then(|f| section_start(&st.song, f))
        {
            st.send(mc_music::Command::Seek(t));
        }
    }
    st.toggle_play();
}

/// Once a frame, before drawing: keep the focus valid, follow the playhead
/// through the whole song, and look for the revision behind a file change.
pub fn tick(st: &mut Studio, now: f64) {
    let valid = st
        .wb
        .focus
        .as_ref()
        .is_some_and(|f| st.song.section(f).is_some());
    if !valid {
        let first = st
            .sel
            .section
            .and_then(|i| st.song.sections.get(i))
            .or_else(|| {
                st.song
                    .arrangement
                    .first()
                    .and_then(|n| st.song.section(n))
                    .map(|i| &st.song.sections[i])
            })
            .or(st.song.sections.first())
            .map(|s| s.name.clone());
        st.wb.focus = first;
    }
    if !st.wb.loop_part {
        if let Some(p) = playing_section(st) {
            if st.wb.focus.as_ref() != Some(&p) && st.song.section(&p).is_some() {
                st.wb.focus = Some(p);
            }
        }
    }
    if let Some((track, _)) = &st.wb.edit {
        if st.song.track(track).is_none() {
            st.wb.edit = None;
        }
    }
    if st.wb.edit.is_some() {
        st.wb.edit_last = st.wb.edit.clone();
    }
    // Instruments that join with the battle's intensity: each part sets it
    // from its own range, so the part sounds as the game plays it.
    let part = playing_section(st).or(st.wb.focus.clone());
    if let Some(i) = part.and_then(|p| st.song.section(&p)) {
        let (lo, hi) = st.song.sections[i].intensity;
        let v = ((lo + hi) * 0.5).clamp(0.0, 1.0);
        if (v - st.intensity).abs() > 0.01 {
            st.intensity = v;
            st.send(mc_music::Command::SetIntensity(v));
        }
    }
    // A file change: say what it was once Claude's revision is in the log
    // (Claude writes the file, then records the revision).
    if let Some(at) = st.wb.external_at {
        let head = st
            .collab
            .log
            .entries
            .iter()
            .rev()
            .find(|e| e.status == mc_music::history::Status::Revision)
            .cloned();
        let recent = |e: &mc_music::history::Entry| {
            mc_music::history::now().saturating_sub(e.time) < 60 || st.wb.announce_any
        };
        match head {
            Some(e)
                if e.author == "claude"
                    && st.wb.announced.as_ref() != Some(&e.id)
                    && recent(&e) =>
            {
                st.wb.announce_any = false;
                let prev = e.base.clone().or_else(|| {
                    let revs: Vec<&mc_music::history::Entry> = st
                        .collab
                        .log
                        .entries
                        .iter()
                        .filter(|x| x.status == mc_music::history::Status::Revision)
                        .collect();
                    revs.iter().rev().nth(1).map(|x| x.id.clone())
                });
                st.wb.announced = Some(e.id.clone());
                st.wb.notice = Some(Notice {
                    text: format!("Claude changed: {}", e.message),
                    undo_to: prev,
                    at: now,
                });
                st.wb.external_at = None;
            }
            _ if now - at > 8.0 => st.wb.external_at = None,
            _ => {}
        }
    }
    if st.wb.notice.as_ref().is_some_and(|n| now - n.at > 12.0) {
        st.wb.notice = None;
    }
}

/// Called when the file was reloaded because someone else wrote it.
pub fn external_change(st: &mut Studio, now: f64) {
    st.wb.external_at = Some(now);
    st.collab.force_poll();
    if st.wb.notice.is_none() {
        st.wb.notice = Some(Notice {
            text: "The song changed on disk and was reloaded".into(),
            undo_to: None,
            at: now,
        });
    }
}

pub fn undo_change(st: &mut Studio, to: &str) {
    let (Some(desk), Some(stem)) = (&st.collab.desk, st.collab.stem.clone()) else {
        return;
    };
    match desk.revert(&stem, to, "you") {
        Ok(_) => {
            st.reload();
            st.collab.force_poll();
            st.wb.notice = Some(Notice {
                text: "Undone: back to how it was".into(),
                undo_to: None,
                at: st.time_now(),
            });
        }
        Err(e) => st.say(format!("Could not undo: {e}"), Tone::Bad),
    }
}

// -- the timeline ---------------------------------------------------------------

/// A part as it lies on the timeline.
#[derive(Clone, Copy)]
pub struct Block {
    pub section: usize,
    pub start: u32,
    pub ticks: u32,
}

/// The arrangement's parts in order; a part not in it stands alone.
pub fn blocks(st: &Studio) -> Vec<Block> {
    let song = &st.song;
    let starts = song.arrangement_starts();
    let in_arr = st
        .wb
        .focus
        .as_ref()
        .is_none_or(|f| starts.iter().any(|(_, s)| song.sections[*s].name == *f));
    if !in_arr {
        if let Some(i) = st.wb.focus.as_ref().and_then(|f| song.section(f)) {
            return vec![Block {
                section: i,
                start: 0,
                ticks: song.section_ticks(&song.sections[i]),
            }];
        }
    }
    starts
        .into_iter()
        .map(|(start, s)| Block {
            section: s,
            start,
            ticks: song.section_ticks(&song.sections[s]),
        })
        .collect()
}

fn focus_block(st: &Studio, blocks: &[Block]) -> Option<Block> {
    let f = st.wb.focus.as_ref()?;
    blocks
        .iter()
        .copied()
        .find(|b| st.song.sections[b.section].name == *f)
}

/// Where the playhead is, in timeline ticks.
pub fn playhead(st: &Studio, blocks: &[Block]) -> Option<u32> {
    let s = &st.status;
    if !s.playing {
        return None;
    }
    if st.sent_mode_is_song() {
        return Some(s.song_tick);
    }
    let name = s
        .section
        .and_then(|i| st.sent_song().sections.get(i))
        .map(|x| x.name.clone())?;
    let b = blocks
        .iter()
        .find(|b| st.song.sections[b.section].name == name)?;
    Some(b.start + s.section_tick.min(b.ticks))
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Glides the visible stretch toward the part in view (or the whole song).
fn update_view(st: &mut Studio, blocks: &[Block], now: f64) {
    let total = blocks.last().map(|b| b.start + b.ticks).unwrap_or(1).max(1) as f32;
    let target = match focus_block(st, blocks) {
        Some(b) if st.wb.loop_part => {
            let pad = b.ticks as f32 * 0.035;
            (b.start as f32 - pad, (b.start + b.ticks) as f32 + pad)
        }
        _ => (0.0, total),
    };
    let wb = &mut st.wb;
    if wb.view_t0 < 0.0 {
        wb.view_from = target;
        wb.view_to = target;
        wb.view_t0 = now - 1.0;
    }
    if (target.0 - wb.view_to.0).abs() > 0.5 || (target.1 - wb.view_to.1).abs() > 0.5 {
        wb.view_from = wb.view;
        wb.view_to = target;
        wb.view_t0 = now;
    }
    let k = ease(((now - wb.view_t0) / 0.28) as f32);
    wb.view = (
        wb.view_from.0 + (wb.view_to.0 - wb.view_from.0) * k,
        wb.view_from.1 + (wb.view_to.1 - wb.view_from.1) * k,
    );
    wb.animating = k < 1.0;
}

struct Map {
    rect: Rect,
    v0: f32,
    v1: f32,
}

impl Map {
    fn x(&self, t: f32) -> f32 {
        self.rect.left() + (t - self.v0) / (self.v1 - self.v0).max(1.0) * self.rect.width()
    }
    fn tick(&self, x: f32) -> f32 {
        self.v0 + (x - self.rect.left()) / self.rect.width() * (self.v1 - self.v0)
    }
}

// -- drawing --------------------------------------------------------------------

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    tick(st, now);
    moments::scan(st, now);
    moments::load_markers(st);
    let bl = blocks(st);
    let pos = playhead(st, &bl);
    moments::follow(st, pos);
    update_view(st, &bl, now);
    st.wb.chip_at.clear();
    st.wb.row_at.clear();
    st.wb.moment_at.clear();
    st.wb.marker_at.clear();
    egui::Panel::top("wb-top")
        .frame(egui::Frame::new().fill(BG0).inner_margin(egui::Margin {
            left: 16,
            right: 16,
            top: 12,
            bottom: 10,
        }))
        .show(ui, |ui| {
            top_bar(ui, st);
            if !st.moments.list.is_empty() {
                ui.add_space(8.0);
                moments_strip(ui, st, now);
            }
        });
    let mut open = st.wb.edit.is_some();
    let h = ui.ctx().content_rect().height();
    egui::Panel::bottom("wb-edit")
        .resizable(true)
        .default_size((h * 0.44).max(260.0))
        .size_range(200.0..=(h - 240.0).max(240.0))
        .frame(
            egui::Frame::new()
                .fill(BG1)
                .inner_margin(egui::Margin::symmetric(16, 10))
                .stroke(Stroke::new(1.0, theme::line(18))),
        )
        .show_collapsible(ui, &mut open, |ui| crate::edit::show(ui, st));
    if !open && st.wb.edit.is_some() {
        st.wb.edit = None;
    }
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(BG1)
                .inner_margin(egui::Margin::symmetric(16, 10)),
        )
        .show(ui, |ui| {
            if st.collab.audition.is_some() {
                banner(ui, WARN, |ui| {
                    ui.label(
                        egui::RichText::new("Hearing Claude's suggestion")
                            .font(theme::font_semi(15.0))
                            .color(WARN),
                    );
                    if ui.button("Back to mine").clicked() {
                        st.collab.set_audition(None);
                    }
                });
            }
            if st.disk_changed {
                banner(ui, WARN, |ui| {
                    ui.label(
                        egui::RichText::new(
                            "The song changed on disk while you have unsaved edits.",
                        )
                        .size(15.0)
                        .color(WARN),
                    );
                    if ui.button("Take the new version").clicked() {
                        st.reload();
                    }
                    if ui.button("Keep mine").clicked() {
                        st.disk_changed = false;
                    }
                });
            }
            notice(ui, st, now);
            if let Some((text, tone, at)) = &st.message {
                if now - at < 6.0 && matches!(tone, Tone::Bad | Tone::Warn) {
                    ui.label(egui::RichText::new(text).size(14.0).color(WARN));
                }
            }
            title(ui, st, &bl);
            timeline(ui, st, &bl, pos);
        });
    drag_ghost(ui, st);
}

fn title(ui: &mut egui::Ui, st: &Studio, bl: &[Block]) {
    let bar = st.song.bar_ticks().max(1);
    ui.horizontal(|ui| {
        let (name, info) = match (
            st.wb.loop_part,
            focus_block(st, bl).or_else(|| {
                st.wb
                    .focus
                    .as_ref()
                    .and_then(|f| st.song.section(f))
                    .map(|i| Block {
                        section: i,
                        start: 0,
                        ticks: st.song.section_ticks(&st.song.sections[i]),
                    })
            }),
        ) {
            (true, Some(b)) => (
                pretty(&st.song.sections[b.section].name),
                format!("{} bars, looping", b.ticks / bar),
            ),
            _ => {
                let total = bl.last().map(|b| b.start + b.ticks).unwrap_or(0);
                let secs = total as f64 * st.song.samples_per_tick(48000.0) / 48000.0;
                (
                    pretty(&st.song.name),
                    format!(
                        "the whole song, {} parts, {}",
                        bl.len(),
                        crate::player::clock(secs as f32)
                    ),
                )
            }
        };
        ui.label(
            egui::RichText::new(name)
                .font(theme::font_light(28.0))
                .color(TEXT),
        );
        ui.label(egui::RichText::new(info).size(15.0).color(FAINT));
        if let Some(m) = &st.side.moment {
            ui.add_space(12.0);
            let t = ui.input(|i| i.time);
            let pulse = 0.6 + 0.4 * ((t * 4.0).sin() as f32 * 0.5 + 0.5);
            let text = format!("{} playing", pretty(m));
            let galley = ui
                .painter()
                .layout_no_wrap(text, theme::font_semi(14.0), BG0);
            let (r, _) = ui.allocate_exact_size(galley.size() + vec2(24.0, 12.0), Sense::hover());
            ui.painter().rect_filled(
                r,
                CornerRadius::same(12),
                theme::with_alpha(ACCENT, (255.0 * pulse) as u8),
            );
            ui.painter()
                .galley(r.center() - galley.size() * 0.5, galley, BG0);
            ui.label(
                egui::RichText::new("the song dips while it plays")
                    .size(13.0)
                    .color(FAINT),
            );
        }
    });
    ui.add_space(4.0);
}

fn top_bar(ui: &mut egui::Ui, st: &mut Studio) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        // The song's name goes back to the library.
        let name = if st.song.name.is_empty() {
            "Untitled".to_string()
        } else {
            st.song.name.clone()
        };
        let galley = ui.painter().layout_no_wrap(
            crate::transport::truncate(&name, 22),
            theme::font_light(22.0),
            TEXT,
        );
        let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, 44.0), Sense::click());
        st.wb.library_at = Some(r);
        let hov = ui
            .ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(6), theme::mix(BG0, BG2, hov));
        let chev = Rect::from_center_size(
            pos2(r.left() + 13.0 - 3.0 * hov, r.center().y),
            vec2(12.0, 16.0),
        );
        p.add(egui::Shape::line(
            vec![
                chev.right_top(),
                chev.left_center() + vec2(3.0, 0.0),
                chev.right_bottom(),
            ],
            Stroke::new(2.0, theme::mix(DIM, TEXT, hov)),
        ));
        p.galley(
            pos2(r.left() + 26.0, r.center().y - galley.size().y * 0.5),
            galley,
            TEXT,
        );
        if st.dirty {
            p.circle_filled(pos2(r.right() - 6.0, r.top() + 10.0), 3.0, WARN);
        }
        if resp
            .on_hover_text("Back to the library (songs, moments and audio)")
            .clicked()
        {
            st.leave_for_library();
            return;
        }
        let playing = st.status.playing;
        let (r, resp) = ui.allocate_exact_size(vec2(52.0, 44.0), Sense::click());
        let a = ui.ctx().animate_bool_with_time(resp.id, playing, 0.15);
        let hov = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
        let p = ui.painter();
        p.rect_filled(
            r.expand(hov * 1.5),
            CornerRadius::same(6),
            theme::mix(theme::mix(BG2, BG3, hov), ACCENT, a),
        );
        widgets::draw_icon(
            p,
            r.shrink(4.0),
            if playing { Icon::Stop } else { Icon::Play },
            theme::mix(TEXT, BG0, a),
        );
        if resp.on_hover_text("Play / stop (Space)").clicked() {
            toggle_play(st);
        }
        ui.add_space(6.0);
        chips(ui, st);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let adv = big_toggle(ui, false, "Advanced", DIM);
            st.wb.advanced_at = Some(adv.rect);
            if adv
                .on_hover_text("Every tool: mixer, instruments, effects, history")
                .clicked()
            {
                st.set_advanced(true);
            }
            let wav = match st.export.progress() {
                Some(f) => format!("Saving {:.0}%", f * 100.0),
                None => "Save WAV".to_string(),
            };
            let resp = big_toggle(ui, st.export.busy(), &wav, ACCENT);
            let resp = match &st.export.error {
                Some(e) => resp.on_hover_text(format!("Last try failed: {e}")),
                None => resp.on_hover_text(
                    "The whole song as a .wav file with every instrument, even ones muted \
                     here; the folder opens when it is done",
                ),
            };
            if resp.clicked() {
                crate::export::save_song(st);
            }
            if big_toggle(ui, false, "Save", DIM)
                .on_hover_text("Edits save by themselves; this also records a revision (Ctrl+S)")
                .clicked()
            {
                st.save();
            }
            ui.add_space(8.0);
            if big_toggle(ui, !st.wb.loop_part, "Whole song", ACCENT)
                .on_hover_text("Everything, start to end")
                .clicked()
            {
                set_loop(st, false);
            }
            if big_toggle(ui, st.wb.loop_part, "Loop this part", ACCENT)
                .on_hover_text("Repeat the part in view")
                .clicked()
            {
                set_loop(st, true);
            }
        });
    });
}

/// A larger latching button with animated states.
pub fn big_toggle(ui: &mut egui::Ui, on: bool, text: &str, colour: Color32) -> egui::Response {
    let font = theme::font_semi(15.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, TEXT);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, 36.0), Sense::click());
    let a = ui.ctx().animate_bool_with_time(resp.id, on, 0.18);
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let down = resp.is_pointer_button_down_on();
    let p = ui.painter();
    let base = theme::mix(BG2, BG3, hov);
    let fill = theme::mix(base, colour, a);
    let fg = theme::mix(if colour == DIM { DIM } else { TEXT }, BG0, a);
    let r = if down { rect.shrink(1.0) } else { rect };
    p.rect(
        r,
        CornerRadius::same(6),
        fill,
        Stroke::new(1.0, theme::line((16.0 * (1.0 - a)) as u8)),
        StrokeKind::Inside,
    );
    p.galley(r.center() - galley.size() * 0.5, galley, fg);
    resp
}

fn chips(ui: &mut egui::Ui, st: &mut Studio) {
    let song = &st.song;
    let mut parts: Vec<String> = Vec::new();
    for n in &song.arrangement {
        if song.section(n).is_some() && !parts.contains(n) {
            parts.push(n.clone());
        }
    }
    for s in &song.sections {
        if !parts.contains(&s.name) && !matches!(s.kind, SectionKind::Stinger | SectionKind::Ending)
        {
            parts.push(s.name.clone());
        }
    }
    let playing = playing_section(st);
    let mut clicked = None;
    for name in parts.iter() {
        let focused = st.wb.focus.as_ref() == Some(name) && st.wb.loop_part;
        let r = chip(
            ui,
            &pretty(name),
            focused,
            playing.as_ref() == Some(name),
            15.0,
        );
        st.wb.chip_at.insert(name.clone(), r.rect);
        if r.clicked() {
            clicked = Some(name.clone());
        }
    }
    if let Some(n) = clicked {
        focus(st, &n);
        set_loop(st, true);
    }
}

fn chip(ui: &mut egui::Ui, text: &str, focused: bool, playing: bool, size: f32) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), theme::font_semi(size), TEXT);
    let h = if size > 14.0 { 40.0 } else { 30.0 };
    let (rect, resp) =
        ui.allocate_exact_size(vec2(galley.size().x + 26.0, h), Sense::click_and_drag());
    let a = ui.ctx().animate_bool_with_time(resp.id, focused, 0.2);
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let p = ui.painter();
    let fill = theme::mix(theme::mix(BG2, BG3, hov), theme::mix(BG2, ACCENT, 0.3), a);
    p.rect(
        rect,
        CornerRadius::same(6),
        fill,
        Stroke::new(1.0 + 0.5 * a, theme::mix(theme::line(14), ACCENT, a)),
        StrokeKind::Inside,
    );
    p.galley(
        rect.center() - galley.size() * 0.5,
        galley,
        theme::mix(DIM, TEXT, a.max(hov)),
    );
    if playing {
        let t = ui.input(|i| i.time);
        let pulse = 0.55 + 0.45 * ((t * 3.0).sin() as f32 * 0.5 + 0.5);
        p.circle_filled(
            pos2(rect.right() - 8.0, rect.top() + 8.0),
            3.0,
            theme::with_alpha(ACCENT, (255.0 * pulse) as u8),
        );
    }
    resp
}

fn moments_strip(ui: &mut egui::Ui, st: &mut Studio, _now: f64) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(egui::RichText::new("Moments").size(14.0).color(FAINT))
            .on_hover_text("What the game plays over the song when something happens. Click to play one now; drag one onto the timeline to play it there.");
        let names: Vec<(String, bool)> = st.moments.list.iter().map(|m| (m.name.clone(), m.ending)).collect();
        let playing = st.side.moment.clone();
        for (name, ending) in names {
            let r = chip(ui, &pretty(&name), playing.as_deref() == Some(name.as_str()), false, 13.0);
            st.wb.moment_at.insert(name.clone(), r.rect);
            if r.drag_started() {
                st.moments.drag = Some(name.clone());
            }
            let tip = if ending { "An ending: the song stops under it. Click to play now, or drag onto the timeline." } else { "Click to play it now over the song, or drag it onto the timeline." };
            if r.on_hover_text(tip).clicked() {
                moments::play(st, &name);
            }
        }
    });
}

/// The moment chip under the pointer while it is dragged to the timeline.
fn drag_ghost(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(name) = st.moments.drag.clone() else {
        return;
    };
    let (pos, released) = ui.input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.any_released() || !i.pointer.any_down(),
        )
    });
    if let Some(p) = pos {
        let layer = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("moment-drag"),
        ));
        let galley = layer.layout_no_wrap(pretty(&name), theme::font_semi(13.0), BG0);
        let r = Rect::from_min_size(p + vec2(10.0, 10.0), galley.size() + vec2(18.0, 10.0));
        layer.rect_filled(r, CornerRadius::same(10), ACCENT);
        layer.galley(r.center() - galley.size() * 0.5, galley, BG0);
    }
    if released {
        st.moments.drag = None;
    }
}

fn notice(ui: &mut egui::Ui, st: &mut Studio, _now: f64) {
    if let Some(n) = &st.wb.notice {
        st.wb.notice_last = Some((n.text.clone(), n.undo_to.clone()));
    }
    let a =
        ui.ctx()
            .animate_bool_with_time(egui::Id::new("wb-notice"), st.wb.notice.is_some(), 0.3);
    if a <= 0.0 {
        st.wb.notice_last = None;
        return;
    }
    let Some((text, undo)) = st.wb.notice_last.clone() else {
        return;
    };
    let mut close = false;
    ui.scope(|ui| {
        ui.set_opacity(a);
        ui.add_space(-8.0 * (1.0 - a));
        banner(ui, ACCENT, |ui| {
            ui.label(egui::RichText::new(text).size(15.0).color(TEXT));
            if let Some(to) = &undo {
                if big_toggle(ui, false, "Undo", ACCENT)
                    .on_hover_text(format!("Go back to {to}"))
                    .clicked()
                {
                    undo_change(st, to);
                }
            }
            if big_toggle(ui, false, "Dismiss", DIM).clicked() {
                close = true;
            }
        });
    });
    if close {
        st.wb.notice = None;
    }
}

fn banner(ui: &mut egui::Ui, colour: Color32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::with_alpha(colour, 22))
        .stroke(Stroke::new(1.0, theme::with_alpha(colour, 110)))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(add);
        });
    ui.add_space(8.0);
}

/// The instruments that play anywhere in the song, in track order.
pub fn row_tracks(song: &Song, sec: usize, all: bool) -> Vec<usize> {
    (0..song.tracks.len())
        .filter(|&t| {
            all || song.sections[sec]
                .clips
                .iter()
                .any(|c| c.track == song.tracks[t].name)
        })
        .collect()
}

fn song_tracks(st: &Studio, bl: &[Block]) -> Vec<usize> {
    let song = &st.song;
    (0..song.tracks.len())
        .filter(|&t| {
            bl.iter().any(|b| {
                song.sections[b.section]
                    .clips
                    .iter()
                    .any(|c| c.track == song.tracks[t].name)
            })
        })
        .collect()
}

const HEAD_W: f32 = 330.0;
const ROW_H: f32 = 64.0;
const RULER_H: f32 = 30.0;

fn timeline(ui: &mut egui::Ui, st: &mut Studio, bl: &[Block], pos: Option<u32>) {
    let full = ui.available_rect_before_wrap();
    let tl = Rect::from_min_max(pos2(full.left() + HEAD_W, full.top()), full.right_bottom());
    let map = Map {
        rect: tl,
        v0: st.wb.view.0,
        v1: st.wb.view.1,
    };
    let looped = focus_block(st, bl).map(|b| b.section);
    // The ruler: parts, markers, and where moments are dropped.
    let (ruler, _) = ui.allocate_exact_size(vec2(full.width(), RULER_H), Sense::hover());
    let ruler = Rect::from_min_max(
        pos2(tl.left(), ruler.top()),
        pos2(tl.right(), ruler.bottom()),
    );
    st.wb.ruler_at = Some(ruler);
    ruler_ui(ui, st, bl, &map, ruler, looped);
    ui.add_space(6.0);
    let tracks = song_tracks(st, bl);
    let rows_top = ui.cursor().top();
    egui::ScrollArea::vertical()
        .id_salt("wb-rows")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for t in tracks {
                row(ui, st, bl, t, &map, looped, pos);
                ui.add_space(6.0);
            }
        });
    // The playhead over the ruler and the rows (down to the last row).
    let rows_bottom = st
        .wb
        .row_at
        .values()
        .map(|r| r.center().y + ROW_H * 0.5)
        .fold(rows_top, f32::max)
        .min(full.bottom());
    if let Some(t) = pos {
        let x = map.x(t as f32);
        if x >= tl.left() && x <= tl.right() {
            let p = ui.painter().with_clip_rect(Rect::from_min_max(
                pos2(tl.left(), ruler.top()),
                full.right_bottom(),
            ));
            p.vline(x, ruler.top()..=rows_bottom, Stroke::new(2.0, ACCENT));
            p.add(egui::Shape::convex_polygon(
                vec![
                    pos2(x - 6.0, ruler.top()),
                    pos2(x + 6.0, ruler.top()),
                    pos2(x, ruler.top() + 8.0),
                ],
                ACCENT,
                Stroke::NONE,
            ));
        }
    }
    // Dropping a moment onto the timeline.
    if let Some(name) = st.moments.drag.clone() {
        let area = Rect::from_min_max(
            pos2(tl.left(), ruler.top()),
            pos2(tl.right(), full.bottom().max(rows_top)),
        );
        if let Some(p) = ui
            .input(|i| i.pointer.interact_pos())
            .filter(|p| area.contains(*p))
        {
            let beat = PPQ as f32;
            let tick = ((map.tick(p.x) / beat).round() * beat).max(0.0) as u32;
            let x = map.x(tick as f32);
            ui.painter().vline(
                x,
                area.y_range(),
                Stroke::new(1.5, theme::with_alpha(ACCENT, 160)),
            );
            if ui.input(|i| i.pointer.any_released()) {
                moments::add_marker(st, tick, &name);
                st.moments.drag = None;
            }
        }
    }
}

fn ruler_ui(
    ui: &mut egui::Ui,
    st: &mut Studio,
    bl: &[Block],
    map: &Map,
    ruler: Rect,
    looped: Option<usize>,
) {
    let p = ui.painter().with_clip_rect(ruler.expand2(vec2(0.0, 2.0)));
    p.rect_filled(ruler, CornerRadius::same(4), BG0);
    let mut clicked = None;
    for (i, b) in bl.iter().enumerate() {
        let x0 = map.x(b.start as f32);
        let x1 = map.x((b.start + b.ticks) as f32);
        if x1 < ruler.left() || x0 > ruler.right() {
            continue;
        }
        let r = Rect::from_min_max(pos2(x0, ruler.top()), pos2(x1, ruler.bottom()));
        let resp = ui.interact(
            r.intersect(ruler),
            ui.id().with(("ruler-part", i)),
            Sense::click(),
        );
        let on = looped == Some(b.section) && st.wb.loop_part;
        let a = ui.ctx().animate_bool_with_time(resp.id, on, 0.2);
        let hov = ui
            .ctx()
            .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
        if a > 0.0 || hov > 0.0 {
            p.rect_filled(
                r.shrink2(vec2(1.0, 0.0)),
                CornerRadius::same(4),
                theme::with_alpha(ACCENT, (40.0 * a + 16.0 * hov) as u8),
            );
        }
        p.hline(
            (x0 + 1.0)..=(x1 - 1.0),
            ruler.bottom() - 1.0,
            Stroke::new(2.0 * a, ACCENT),
        );
        p.vline(x0, ruler.y_range(), Stroke::new(1.0, theme::line(40)));
        let name = pretty(&st.song.sections[b.section].name);
        if x1 - x0 > 30.0 {
            // Each label stays inside its own part.
            p.with_clip_rect(r.intersect(ruler).shrink2(vec2(4.0, 0.0)))
                .text(
                    pos2(x0.max(ruler.left()) + 8.0, ruler.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    theme::font_semi(13.5),
                    theme::mix(DIM, TEXT, a.max(hov)),
                );
        }
        if resp.on_hover_text("Loop this part").clicked() {
            clicked = Some(b.section);
        }
    }
    // Moments dropped on the timeline.
    let mut remove = None;
    for (i, m) in st.moments.markers.iter().enumerate() {
        let x = map.x(m.tick as f32);
        if x < ruler.left() - 4.0 || x > ruler.right() + 4.0 {
            continue;
        }
        let r = Rect::from_center_size(pos2(x, ruler.top() + 7.0), vec2(12.0, 12.0));
        st.wb.marker_at.push(r);
        let resp = ui.interact(r, ui.id().with(("marker", i)), Sense::click());
        let hov = resp.hovered();
        let c = if hov { TEXT } else { WARN };
        p.add(egui::Shape::convex_polygon(
            vec![
                r.center_top(),
                r.right_center(),
                r.center_bottom(),
                r.left_center(),
            ],
            c,
            Stroke::NONE,
        ));
        p.vline(
            x,
            (r.bottom())..=ruler.bottom(),
            Stroke::new(1.0, theme::with_alpha(WARN, 160)),
        );
        let label = pretty(&m.name);
        let resp = resp.on_hover_text(format!("{label} plays here. Right-click to remove."));
        if resp.secondary_clicked() {
            remove = Some(i);
        }
    }
    if let Some(i) = remove {
        moments::remove_marker(st, i);
    }
    if let Some(s) = clicked {
        let name = st.song.sections[s].name.clone();
        focus(st, &name);
        set_loop(st, true);
    }
}

fn row(
    ui: &mut egui::Ui,
    st: &mut Studio,
    bl: &[Block],
    t: usize,
    map: &Map,
    looped: Option<usize>,
    pos: Option<u32>,
) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::hover());
    let track = st.song.tracks[t].clone();
    let colour = theme::rgb(track.colour);
    let editing = st.wb.edit.as_ref().is_some_and(|e| e.0 == track.name);
    let muted = st.wb.muted.contains(&track.name);
    let soloed = st.wb.solo.as_ref() == Some(&track.name);
    let silent = muted || st.wb.solo.as_ref().is_some_and(|s| *s != track.name);
    let quiet =
        ui.ctx()
            .animate_bool_with_time(egui::Id::new(("wb-quiet", &track.name)), silent, 0.25);
    let ed =
        ui.ctx()
            .animate_bool_with_time(egui::Id::new(("wb-editing", &track.name)), editing, 0.2);
    let dip = 0.3 + 0.7 * st.side.song_level.clamp(0.0, 1.0);
    let fade = (1.0 - 0.6 * quiet) * dip;
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(8),
        theme::mix(BG2, theme::mix(BG2, ACCENT, 0.1), ed),
        Stroke::new(1.0, theme::mix(theme::line(10), ACCENT, ed)),
        StrokeKind::Inside,
    );
    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(5.0, rect.height())),
        CornerRadius {
            nw: 8,
            sw: 8,
            ne: 0,
            se: 0,
        },
        theme::mix(BG2, colour, 1.0 - 0.6 * quiet),
    );
    // Name.
    let clips_here: Vec<String> = looped
        .map(|s| {
            let mut v: Vec<String> = st.song.sections[s]
                .clips
                .iter()
                .filter(|c| c.track == track.name)
                .map(|c| c.pattern.clone())
                .collect();
            v.dedup();
            v
        })
        .unwrap_or_default();
    let name_x = rect.left() + 18.0;
    p.text(
        pos2(name_x, rect.top() + 13.0),
        Align2::LEFT_TOP,
        crate::transport::truncate(&pretty(&track.name), 14),
        theme::font_semi(17.0),
        theme::mix(FAINT, TEXT, 1.0 - 0.6 * quiet),
    );
    let here: Option<Vec<String>> = st
        .wb
        .loop_part
        .then(|| clips_here.iter().map(|p| pretty(p)).collect());
    let right = rect.left() + HEAD_W - 12.0;
    crate::swap::row_label(
        ui,
        st,
        t,
        pos2(name_x, rect.top() + 38.0),
        right - 54.0 * 2.0 - 12.0 - 38.0 - 8.0 - name_x,
        here.as_deref(),
    );
    // Speaker, Solo, Edit at the right of the header.
    let edit_r = Rect::from_min_size(pos2(right - 54.0, rect.center().y - 17.0), vec2(54.0, 34.0));
    let solo_r = Rect::from_min_size(
        pos2(edit_r.left() - 6.0 - 54.0, edit_r.top()),
        vec2(54.0, 34.0),
    );
    let spk_r = Rect::from_min_size(
        pos2(solo_r.left() - 6.0 - 38.0, edit_r.top()),
        vec2(38.0, 34.0),
    );
    st.wb.row_at.insert((track.name.clone(), "speaker"), spk_r);
    st.wb.row_at.insert((track.name.clone(), "solo"), solo_r);
    st.wb.row_at.insert((track.name.clone(), "edit"), edit_r);
    let spk = ui.interact(spk_r, ui.id().with(("spk", t)), Sense::click());
    let m = ui.ctx().animate_bool_with_time(spk.id, muted, 0.18);
    let hov = ui
        .ctx()
        .animate_bool_with_time(spk.id.with("h"), spk.hovered(), 0.12);
    let p = ui.painter();
    p.rect(
        spk_r,
        CornerRadius::same(6),
        theme::mix(theme::mix(BG1, BG3, hov), theme::with_alpha(WARN, 70), m),
        Stroke::new(1.0, theme::line(16)),
        StrokeKind::Inside,
    );
    widgets::draw_icon(
        p,
        spk_r.shrink(4.0),
        if muted {
            Icon::SpeakerOff
        } else {
            Icon::Speaker
        },
        theme::mix(TEXT, WARN, m),
    );
    if spk
        .on_hover_text(if muted {
            "Muted while you listen. Click to hear it."
        } else {
            "Mute while you listen (the song is not changed)"
        })
        .clicked()
    {
        if muted {
            st.wb.muted.remove(&track.name);
        } else {
            st.wb.muted.insert(track.name.clone());
        }
    }
    let solo = ui.interact(solo_r, ui.id().with(("solo", t)), Sense::click());
    button_face(ui, solo.id, solo_r, "Solo", soloed, solo.hovered());
    if solo
        .on_hover_text("Hear only this; click again for all")
        .clicked()
    {
        st.wb.solo = if soloed {
            None
        } else {
            Some(track.name.clone())
        };
    }
    let edit = ui.interact(edit_r, ui.id().with(("edit", t)), Sense::click());
    let can_edit = !clips_here.is_empty()
        || st
            .song
            .sections
            .iter()
            .any(|s| s.clips.iter().any(|c| c.track == track.name));
    button_face(
        ui,
        edit.id,
        edit_r,
        "Edit",
        editing,
        edit.hovered() && can_edit,
    );
    if edit.on_hover_text("Change its notes").clicked() && can_edit {
        if editing {
            st.wb.edit = None;
        } else {
            let pat = clips_here.first().cloned().or_else(|| {
                let at = pos.unwrap_or(0);
                let b = bl
                    .iter()
                    .find(|b| at >= b.start && at < b.start + b.ticks)
                    .or(bl.first())?;
                st.song.sections[b.section]
                    .clips
                    .iter()
                    .find(|c| c.track == track.name)
                    .map(|c| c.pattern.clone())
            });
            let pat = pat.or_else(|| {
                st.song
                    .sections
                    .iter()
                    .flat_map(|s| &s.clips)
                    .find(|c| c.track == track.name)
                    .map(|c| c.pattern.clone())
            });
            if let Some(pat) = pat {
                if let Some(pi) = st.song.pattern(&pat) {
                    st.sel.pattern = Some(pi);
                }
                st.wb.edit = Some((track.name.clone(), pat));
                st.sel.pattern_track = t;
                st.sel.track = t;
                st.wb.editor.fit_next = true;
            }
        }
    }
    // The notes across the timeline.
    let view = Rect::from_min_max(
        pos2(map.rect.left(), rect.top() + 6.0),
        pos2(map.rect.right(), rect.bottom() - 6.0),
    );
    let lane = Map {
        rect: view,
        v0: map.v0,
        v1: map.v1,
    };
    notes_lane(ui, st, bl, t, &lane, colour, fade, looped);
    let resp = ui.interact(view, ui.id().with(("lane", t)), Sense::click());
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let tick = lane.tick(p.x).max(0.0) as u32;
            if let Some(b) = bl
                .iter()
                .find(|b| tick >= b.start && tick < b.start + b.ticks)
            {
                let name = st.song.sections[b.section].name.clone();
                if !(st.wb.loop_part && st.wb.focus.as_deref() == Some(name.as_str())) {
                    focus(st, &name);
                    set_loop(st, true);
                }
            }
        }
    }
}

fn button_face(ui: &egui::Ui, id: egui::Id, r: Rect, text: &str, on: bool, hovered: bool) {
    let a = ui.ctx().animate_bool_with_time(id.with("on"), on, 0.18);
    let hov = ui.ctx().animate_bool_with_time(id.with("h"), hovered, 0.12);
    let p = ui.painter();
    p.rect(
        r,
        CornerRadius::same(6),
        theme::mix(theme::mix(BG1, BG3, hov), ACCENT, a),
        Stroke::new(1.0, theme::line((16.0 * (1.0 - a)) as u8)),
        StrokeKind::Inside,
    );
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        theme::font_semi(15.0),
        theme::mix(TEXT, BG0, a),
    );
}

fn notes_lane(
    ui: &egui::Ui,
    st: &Studio,
    bl: &[Block],
    t: usize,
    lane: &Map,
    colour: Color32,
    fade: f32,
    looped: Option<usize>,
) {
    let song = &st.song;
    let r = lane.rect;
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, CornerRadius::same(4), BG0);
    let track = &song.tracks[t].name;
    // Pitch range over everything the track plays.
    let (lo, hi) = bl
        .iter()
        .flat_map(|b| {
            song.sections[b.section]
                .clips
                .iter()
                .filter(|c| c.track == *track)
        })
        .filter_map(|c| song.pattern(&c.pattern).map(|pi| (pi, c.transpose)))
        .flat_map(|(pi, tr)| {
            song.patterns[pi]
                .notes
                .iter()
                .map(move |n| (n.2 as i32 + tr as i32).clamp(0, 127) as u8)
        })
        .fold((127u8, 0u8), |(l, h), k| (l.min(k), h.max(k)));
    let span = hi.saturating_sub(lo).max(8) as f32;
    let inner = r.shrink2(vec2(0.0, 4.0));
    for b in bl {
        let x0 = lane.x(b.start as f32);
        let x1 = lane.x((b.start + b.ticks) as f32);
        if x1 < r.left() || x0 > r.right() {
            continue;
        }
        // Parts other than the one looped are dimmed.
        let dim = if st.wb.loop_part && looped.is_some() && looped != Some(b.section) {
            0.35
        } else {
            1.0
        };
        p.vline(x0, r.y_range(), Stroke::new(1.0, theme::line(22)));
        let sec = &song.sections[b.section];
        for c in sec.clips.iter().filter(|c| c.track == *track) {
            let Some(pi) = song.pattern(&c.pattern) else {
                continue;
            };
            let pat = &song.patterns[pi];
            let pt = pat.ticks().max(1);
            let cs = b.start + c.at * PPQ;
            let spanc = c.span(pt, b.ticks);
            p.rect_filled(
                Rect::from_min_max(
                    pos2(lane.x(cs as f32), r.top()),
                    pos2(lane.x((cs + spanc) as f32), r.bottom()),
                ),
                CornerRadius::ZERO,
                theme::with_alpha(colour, (18.0 * fade * dim) as u8),
            );
            let note_c = theme::mix(BG0, colour, (0.3 + 0.7 * fade) * dim);
            let px_per_tick = (lane.x(1.0) - lane.x(0.0)).max(1e-6);
            let mut rep = 0;
            while rep * pt < spanc {
                let base = cs + rep * pt;
                let bx0 = lane.x(base as f32);
                if bx0 > r.right() {
                    break;
                }
                if lane.x((base + pt) as f32) >= r.left() {
                    for n in &pat.notes {
                        if n.0 >= pt || rep * pt + n.0 >= spanc {
                            continue;
                        }
                        let t0 = base + n.0;
                        let t1 = (t0 + n.1.min(pt - n.0)).min(cs + spanc);
                        let k = (n.2 as i32 + c.transpose as i32).clamp(0, 127) as u8;
                        let y = inner.bottom()
                            - (k.saturating_sub(lo)) as f32 / span * (inner.height() - 3.0)
                            - 3.0;
                        let xa = lane.x(t0 as f32);
                        let w = ((t1 - t0) as f32 * px_per_tick).max(2.0);
                        p.rect_filled(
                            Rect::from_min_size(pos2(xa, y), vec2(w, 3.0)),
                            CornerRadius::same(1),
                            note_c,
                        );
                    }
                }
                rep += 1;
            }
        }
    }
}

/// For the session: what is being listened to, in words.
pub fn describe(st: &Studio) -> String {
    let part = st.wb.focus.as_deref().map(pretty).unwrap_or_default();
    let mut s = if st.wb.loop_part {
        format!("looping {part}")
    } else {
        format!("whole song, now in {part}")
    };
    if let Some(solo) = &st.wb.solo {
        s.push_str(&format!(", {solo} alone"));
    }
    if !st.wb.muted.is_empty() {
        s.push_str(&format!(
            ", muted: {}",
            st.wb.muted.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(m) = &st.side.moment {
        s.push_str(&format!(", moment {m} playing"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_read_as_words() {
        assert_eq!(pretty("battle_a"), "Battle A");
        assert_eq!(pretty("commander_lost"), "Commander lost");
        assert_eq!(pretty("calm"), "Calm");
    }

    #[test]
    fn the_view_eases_out() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert!(ease(0.5) > 0.8, "most of the way by half time");
    }
}
