//! The arrangement: a bar ruler, the sections in playing order as coloured
//! blocks, and one lane per track showing each section's clips with a small
//! picture of their notes. Everything here is laid out in beats and drawn with
//! the painter, so zooming and scrolling stay smooth on long songs.

use crate::app::{PlayMode, Studio};
use crate::songops;
use crate::theme::{self, ACCENT, BG0, BG2, BG3, DIM, FAINT, TEXT, WARN};
use crate::transport::truncate;
use crate::widgets::{self, caption, Icon};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind,
};
use mc_music::{Clip, Command, Instrument, SectionKind, PPQ};

pub struct State {
    pub fit_next: bool,
    px_per_beat: f32,
    scroll_beats: f32,
    scroll_y: f32,
    lane_h: f32,
    drag: Option<Drag>,
    add: Option<AddClip>,
    rename: String,
    /// Show only the selected section instead of the whole arrangement.
    pub focus_section: bool,
    /// Last frame's timeline rect, section row and lane top, for tests.
    last: Option<(Rect, Rect, f32)>,
}

impl Default for State {
    fn default() -> State {
        State {
            fit_next: true,
            px_per_beat: 16.0,
            scroll_beats: 0.0,
            scroll_y: 0.0,
            lane_h: 46.0,
            drag: None,
            add: None,
            rename: String::new(),
            focus_section: false,
            last: None,
        }
    }
}

impl State {
    /// Where a tick of the timeline and a track's lane were drawn last frame.
    #[cfg(test)]
    pub fn screen_pos(&self, tick: u32, track: usize) -> Option<Pos2> {
        let (tl, _, top) = self.last?;
        let x = tl.left() + (tick as f32 / PPQ as f32 - self.scroll_beats) * self.px_per_beat;
        Some(pos2(x, top + (track as f32 + 0.5) * self.lane_h))
    }

    /// The middle of the section row at a tick.
    #[cfg(test)]
    pub fn section_pos(&self, tick: u32) -> Option<Pos2> {
        let (tl, row, _) = self.last?;
        let x = tl.left() + (tick as f32 / PPQ as f32 - self.scroll_beats) * self.px_per_beat;
        Some(pos2(x, row.center().y))
    }

    #[cfg(test)]
    pub fn adding(&self) -> bool {
        self.add.is_some()
    }
}

enum Drag {
    Entry {
        from: usize,
    },
    Clip {
        section: usize,
        clip: usize,
        at: u32,
        track: usize,
        origin: Pos2,
    },
    Seek,
    Loop {
        anchor: u32,
    },
}

struct AddClip {
    section: usize,
    track: usize,
    beat: u32,
    pos: Pos2,
}

/// A section as it appears on the timeline.
#[derive(Clone, Copy)]
struct Block {
    entry: Option<usize>,
    section: usize,
    start: u32,
    ticks: u32,
}

const HEADER_W: f32 = 196.0;
const RULER_H: f32 = 22.0;
const SECTION_H: f32 = 28.0;

fn blocks(st: &Studio) -> Vec<Block> {
    let song = &st.song;
    let in_arrangement = st
        .sel
        .section
        .is_some_and(|s| song.arrangement.iter().any(|a| *a == song.sections[s].name));
    if st.arrange.focus_section || (st.sel.section.is_some() && !in_arrangement) {
        if let Some(s) = st.sel.section {
            return vec![Block {
                entry: None,
                section: s,
                start: 0,
                ticks: song.section_ticks(&song.sections[s]),
            }];
        }
    }
    song.arrangement_starts()
        .into_iter()
        .enumerate()
        .map(|(e, (start, s))| Block {
            entry: Some(e),
            section: s,
            start,
            ticks: song.section_ticks(&song.sections[s]),
        })
        .collect()
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    toolbar(ui, st);
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let blocks = blocks(st);
    let total = blocks.last().map(|b| b.start + b.ticks).unwrap_or(0);
    let head = Rect::from_min_size(area.min, vec2(HEADER_W, area.height()));
    let tl = Rect::from_min_max(pos2(area.left() + HEADER_W, area.top()), area.max);
    let ruler = Rect::from_min_size(tl.min, vec2(tl.width(), RULER_H));
    let secrow = Rect::from_min_size(pos2(tl.left(), ruler.bottom()), vec2(tl.width(), SECTION_H));
    let lanes = Rect::from_min_max(pos2(tl.left(), secrow.bottom()), tl.max);
    let lane_heads = Rect::from_min_max(
        pos2(head.left(), lanes.top()),
        pos2(head.right(), lanes.bottom()),
    );

    let a = &mut st.arrange;
    if a.fit_next && total > 0 {
        a.fit_next = false;
        let beats = total as f32 / PPQ as f32;
        a.px_per_beat = ((tl.width() - 40.0) / beats).clamp(2.0, 60.0);
        a.scroll_beats = 0.0;
    }
    // Wheel: scroll lanes, Shift scrolls time, Ctrl zooms time around the pointer.
    if ui.rect_contains_pointer(area) {
        let (scroll, zoom, hover) =
            ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
        if zoom != 1.0 {
            if let Some(h) = hover {
                let beat_at = a.scroll_beats + (h.x - tl.left()) / a.px_per_beat;
                a.px_per_beat = (a.px_per_beat * zoom).clamp(1.0, 200.0);
                a.scroll_beats = beat_at - (h.x - tl.left()) / a.px_per_beat;
            }
        }
        a.scroll_beats -= scroll.x / a.px_per_beat;
        a.scroll_y -= scroll.y;
    }
    let tracks = st.song.tracks.len();
    let content_h = tracks as f32 * a.lane_h + 40.0;
    a.scroll_y = a.scroll_y.clamp(0.0, (content_h - lanes.height()).max(0.0));
    let max_beats = (total as f32 / PPQ as f32 - 4.0).max(0.0);
    a.scroll_beats = a.scroll_beats.clamp(-2.0, max_beats.max(0.0));

    let ppb = a.px_per_beat;
    let sb = a.scroll_beats;
    a.last = Some((tl, secrow, lanes.top() - a.scroll_y));
    let x_of = move |tick: u32| tl.left() + (tick as f32 / PPQ as f32 - sb) * ppb;
    let tick_of = move |x: f32| (((x - tl.left()) / ppb + sb) * PPQ as f32).max(0.0) as u32;

    let p = ui.painter().with_clip_rect(area);
    p.rect_filled(area, CornerRadius::ZERO, theme::BG1);
    p.rect_filled(ruler, CornerRadius::ZERO, BG0);
    p.rect_filled(secrow, CornerRadius::ZERO, theme::BG1);
    grid(&p, st, tl, lanes, ruler, &x_of, total);
    ruler_ui(ui, st, ruler, &blocks, &x_of, &tick_of, total);
    sections_ui(ui, st, secrow, &blocks, &x_of);
    lanes_ui(ui, st, lanes, &blocks, &x_of, &tick_of);
    headers_ui(ui, st, head, lane_heads);
    playhead(ui, st, tl, &blocks, &x_of);
    add_popup(ui, st);
    keys(ui, st);
}

fn toolbar(ui: &mut egui::Ui, st: &mut Studio) {
    egui::Frame::new()
        .fill(BG0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                caption(ui, "Arrangement");
                let in_arr = st.sel.section.is_some_and(|s| {
                    st.song
                        .arrangement
                        .iter()
                        .any(|a| *a == st.song.sections[s].name)
                });
                let focus = st.arrange.focus_section || (st.sel.section.is_some() && !in_arr);
                if widgets::toggle(ui, !focus, "Whole song", ACCENT).clicked() {
                    st.arrange.focus_section = false;
                    if !in_arr {
                        st.sel.section =
                            st.song.arrangement.first().and_then(|n| st.song.section(n));
                        st.sel.entry = st.sel.section.map(|_| 0);
                    }
                    st.arrange.fit_next = true;
                }
                if widgets::toggle(ui, focus, "Selected section", ACCENT).clicked()
                    && st.sel.section.is_some()
                {
                    st.arrange.focus_section = true;
                    st.arrange.fit_next = true;
                }
                ui.add_space(8.0);
                if ui
                    .small_button("Fit")
                    .on_hover_text("Zoom to show everything")
                    .clicked()
                {
                    st.arrange.fit_next = true;
                }
                ui.add_space(8.0);
                clip_inspector(ui, st);
            });
        });
}

/// The selected clip's pattern, repeats and transpose.
fn clip_inspector(ui: &mut egui::Ui, st: &mut Studio) {
    let Some((si, ci)) = st.sel.clip else {
        ui.label(
            egui::RichText::new(
                "Double-click a lane to add a clip; drag clips to move them (Shift snaps to bars)",
            )
            .color(FAINT)
            .small(),
        );
        return;
    };
    let names: Vec<String> = st.song.patterns.iter().map(|p| p.name.clone()).collect();
    let section_beats = st.song.sections[si].bars * st.song.beats_per_bar;
    let Some(clip) = st
        .song
        .sections
        .get_mut(si)
        .and_then(|s| s.clips.get_mut(ci))
    else {
        return;
    };
    ui.label(egui::RichText::new("Clip").color(DIM));
    egui::ComboBox::from_id_salt("clip-pattern")
        .width(120.0)
        .selected_text(clip.pattern.clone())
        .show_ui(ui, |ui| {
            for n in &names {
                ui.selectable_value(&mut clip.pattern, n.clone(), n);
            }
        });
    ui.label(egui::RichText::new("at beat").color(FAINT));
    ui.add(egui::DragValue::new(&mut clip.at).range(0..=section_beats.saturating_sub(1)));
    ui.label(egui::RichText::new("times").color(FAINT));
    ui.add(
        egui::DragValue::new(&mut clip.times)
            .range(0..=64)
            .custom_formatter(|v, _| {
                if v == 0.0 {
                    "fill".into()
                } else {
                    format!("{v:.0}")
                }
            }),
    )
    .on_hover_text("Played this many times back to back; 0 fills to the section's end");
    ui.label(egui::RichText::new("transpose").color(FAINT));
    ui.add(
        egui::DragValue::new(&mut clip.transpose)
            .range(-48..=48)
            .suffix(" st"),
    );
    let pattern = clip.pattern.clone();
    if ui
        .small_button("Make unique")
        .on_hover_text("Give this clip its own copy of the pattern")
        .clicked()
    {
        if let Some(pi) = st.song.pattern(&pattern) {
            let d = songops::duplicate_pattern(&mut st.song, pi);
            let new_name = st.song.patterns[d].name.clone();
            st.song.sections[si].clips[ci].pattern = new_name;
            st.sel.pattern = Some(d);
        }
    }
    if ui.small_button("Delete").clicked() {
        st.song.sections[si].clips.remove(ci);
        st.sel.clip = None;
    }
}

fn grid(
    p: &egui::Painter,
    st: &Studio,
    tl: Rect,
    lanes: Rect,
    ruler: Rect,
    x_of: &dyn Fn(u32) -> f32,
    total: u32,
) {
    let bar = st.song.bar_ticks().max(1);
    let ppb = st.arrange.px_per_beat;
    // Label every bar when there is room, else every 2, 4, 8...
    let mut every = 1;
    while (bar as f32 / PPQ as f32) * ppb * every as f32 <= 34.0 {
        every *= 2;
    }
    let first = (st.arrange.scroll_beats.max(0.0) * PPQ as f32) as u32 / bar;
    let end = total.max(bar * 8) + bar * 16;
    let mut b = first;
    while b * bar <= end {
        let x = x_of(b * bar);
        if x > tl.right() {
            break;
        }
        if x >= tl.left() {
            let strong = b % every == 0;
            p.vline(
                x,
                lanes.y_range(),
                Stroke::new(1.0, theme::line(if strong { 14 } else { 7 })),
            );
            if strong {
                p.vline(
                    x,
                    ruler.bottom() - 6.0..=ruler.bottom(),
                    Stroke::new(1.0, theme::line(60)),
                );
                p.text(
                    pos2(x + 3.0, ruler.center().y - 1.0),
                    Align2::LEFT_CENTER,
                    format!("{}", b + 1),
                    theme::font_semi(11.5),
                    DIM,
                );
            } else {
                p.vline(
                    x,
                    ruler.bottom() - 3.0..=ruler.bottom(),
                    Stroke::new(1.0, theme::line(30)),
                );
            }
        }
        // Beat lines when zoomed in far enough.
        if ppb > 14.0 {
            for k in 1..st.song.beats_per_bar {
                let xb = x_of(b * bar + k * PPQ);
                if xb >= tl.left() && xb <= tl.right() {
                    p.vline(xb, lanes.y_range(), Stroke::new(1.0, theme::line(4)));
                }
            }
        }
        b += 1;
    }
    let xe = x_of(total);
    if xe >= tl.left() && xe <= tl.right() {
        p.vline(xe, tl.y_range(), Stroke::new(1.0, theme::line(50)));
    }
}

fn ruler_ui(
    ui: &mut egui::Ui,
    st: &mut Studio,
    ruler: Rect,
    blocks: &[Block],
    x_of: &dyn Fn(u32) -> f32,
    tick_of: &dyn Fn(f32) -> u32,
    total: u32,
) {
    let resp = ui.interact(ruler, ui.id().with("ruler"), Sense::click_and_drag());
    let bar = st.song.bar_ticks();
    let focus = blocks.len() == 1 && blocks[0].entry.is_none();
    if let Some(pos) = resp.interact_pointer_pos() {
        let t = tick_of(pos.x).min(total.saturating_sub(1));
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.drag_started() || (resp.clicked() && !shift) {
            if shift {
                let a = (t / bar) * bar;
                st.arrange.drag = Some(Drag::Loop { anchor: a });
            } else {
                st.arrange.drag = Some(Drag::Seek);
            }
        }
        match st.arrange.drag {
            Some(Drag::Seek) if resp.dragged() || resp.clicked() => {
                let snapped = (t / (PPQ / 4)) * (PPQ / 4);
                if focus {
                    st.sel.section = Some(blocks[0].section);
                    if st.mode != PlayMode::Section {
                        st.mode = PlayMode::Section;
                    }
                } else if st.mode != PlayMode::Song {
                    st.mode = PlayMode::Song;
                }
                st.send(Command::Seek(snapped));
            }
            Some(Drag::Loop { anchor }) if resp.dragged() => {
                let here = ((t + bar / 2) / bar) * bar;
                let (a, b) = if here >= anchor {
                    (anchor, here.max(anchor + bar))
                } else {
                    (here, anchor)
                };
                st.loop_range = (a, b.min(total.max(bar)));
                st.loop_on = true;
            }
            _ => {}
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        st.arrange.drag = None;
    }
    // Loop range.
    if st.loop_range.1 > st.loop_range.0 && !focus {
        let (x0, x1) = (x_of(st.loop_range.0), x_of(st.loop_range.1));
        let c = if st.loop_on { ACCENT } else { FAINT };
        let r = Rect::from_min_max(pos2(x0, ruler.top() + 2.0), pos2(x1, ruler.top() + 7.0));
        let p = ui.painter().with_clip_rect(ruler);
        p.rect_filled(
            r,
            CornerRadius::same(1),
            theme::with_alpha(c, if st.loop_on { 200 } else { 90 }),
        );
        p.rect_filled(
            Rect::from_min_max(pos2(x0, ruler.top()), pos2(x1, ruler.bottom())),
            CornerRadius::ZERO,
            theme::with_alpha(c, 18),
        );
    }
    resp.on_hover_text("Click to move the playhead; Shift+drag sets the loop range");
}

fn sections_ui(
    ui: &mut egui::Ui,
    st: &mut Studio,
    row: Rect,
    blocks: &[Block],
    x_of: &dyn Fn(u32) -> f32,
) {
    let p = ui.painter().with_clip_rect(row);
    let playing = st.status.section;
    let mut hovered_drop: Option<usize> = None;
    let dragging_entry = matches!(st.arrange.drag, Some(Drag::Entry { .. }));
    for (bi, b) in blocks.iter().enumerate() {
        let x0 = x_of(b.start);
        let x1 = x_of(b.start + b.ticks);
        if x1 < row.left() || x0 > row.right() {
            continue;
        }
        let r = Rect::from_min_max(
            pos2(x0 + 1.0, row.top() + 3.0),
            pos2(x1 - 1.0, row.bottom() - 3.0),
        );
        let sec = &st.song.sections[b.section];
        let c = theme::section_colour(sec.kind);
        let selected = if b.entry.is_some() {
            st.sel.entry == b.entry
        } else {
            st.sel.section == Some(b.section)
        };
        let id = ui.id().with(("block", bi));
        let resp = ui.interact(r, id, Sense::click_and_drag());
        let is_playing = playing == Some(b.section) && st.status.playing;
        let fill = theme::mix(
            BG2,
            c,
            if selected {
                0.45
            } else if resp.hovered() {
                0.3
            } else {
                0.2
            },
        );
        p.rect(
            r,
            CornerRadius::same(3),
            fill,
            Stroke::new(
                1.0,
                if selected {
                    ACCENT
                } else {
                    theme::with_alpha(c, 120)
                },
            ),
            StrokeKind::Inside,
        );
        if is_playing {
            p.rect_filled(
                Rect::from_min_size(r.left_bottom() - vec2(0.0, 3.0), vec2(r.width(), 3.0)),
                CornerRadius::ZERO,
                ACCENT,
            );
        }
        let label = format!("{}  {}", sec.name, sec.bars);
        let text = truncate(&label, ((r.width() - 10.0) / 6.5).max(1.0) as usize);
        p.text(
            r.left_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            theme::font_semi(12.5),
            TEXT,
        );

        if resp.clicked() || resp.drag_started() {
            st.sel.section = Some(b.section);
            st.sel.entry = b.entry;
            st.sel.clip = None;
        }
        if resp.double_clicked() {
            st.mode = PlayMode::Section;
            st.sel.section = Some(b.section);
        }
        if resp.drag_started() {
            if let Some(e) = b.entry {
                st.arrange.drag = Some(Drag::Entry { from: e });
            }
        }
        if dragging_entry {
            if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                if ptr.x >= x0 && ptr.x < x1 {
                    let mid = (x0 + x1) * 0.5;
                    hovered_drop = Some(if ptr.x < mid { bi } else { bi + 1 });
                }
            }
        }
        section_menu(st, &resp, *b);
    }
    // The end of the arrangement: a button to add a section.
    let end_x = blocks
        .last()
        .map(|b| x_of(b.start + b.ticks))
        .unwrap_or(row.left());
    if blocks.first().is_some_and(|b| b.entry.is_some()) || blocks.is_empty() {
        let r = Rect::from_min_size(
            pos2(end_x + 4.0, row.top() + 4.0),
            vec2(22.0, row.height() - 8.0),
        );
        let resp = ui.interact(r, ui.id().with("add-section"), Sense::click());
        widgets::draw_icon(&p, r, Icon::Plus, if resp.hovered() { TEXT } else { FAINT });
        p.rect_stroke(
            r,
            CornerRadius::same(3),
            Stroke::new(1.0, theme::line(if resp.hovered() { 60 } else { 24 })),
            StrokeKind::Inside,
        );
        if resp.on_hover_text("Add a section to the end").clicked() {
            let bars = st
                .sel
                .section
                .and_then(|s| st.song.sections.get(s))
                .map(|s| s.bars)
                .unwrap_or(4);
            let i = songops::new_section(&mut st.song, "Section", bars);
            st.song.arrangement.push(st.song.sections[i].name.clone());
            st.sel.section = Some(i);
            st.sel.entry = Some(st.song.arrangement.len() - 1);
        }
    }
    if let Some(Drag::Entry { from }) = st.arrange.drag {
        let released = ui.input(|i| i.pointer.any_released());
        if let Some(to) = hovered_drop {
            let x = if to < blocks.len() {
                x_of(blocks[to].start)
            } else {
                end_x
            };
            p.vline(x, row.y_range(), Stroke::new(2.0, ACCENT));
            if released && to != from && to != from + 1 {
                let name = st.song.arrangement.remove(from);
                let to = if to > from { to - 1 } else { to };
                st.song.arrangement.insert(to, name);
                st.sel.entry = Some(to);
            }
        }
        if released {
            st.arrange.drag = None;
        }
    }
}

fn section_menu(st: &mut Studio, resp: &egui::Response, b: Block) {
    if resp.secondary_clicked() {
        st.arrange.rename = st.song.sections[b.section].name.clone();
        st.sel.section = Some(b.section);
        st.sel.entry = b.entry;
    }
    resp.context_menu(|ui| {
        ui.set_min_width(200.0);
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut st.arrange.rename).desired_width(120.0));
            if ui.button("Rename").clicked()
                || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let name = st.arrange.rename.trim().to_string();
                songops::rename_section(&mut st.song, b.section, &name);
                ui.close();
            }
        });
        let sec = &mut st.song.sections[b.section];
        ui.horizontal(|ui| {
            ui.label("Bars");
            ui.add(egui::DragValue::new(&mut sec.bars).range(1..=256));
            egui::ComboBox::from_id_salt("sec-kind")
                .selected_text(sec.kind.name())
                .show_ui(ui, |ui| {
                    for k in SectionKind::ALL {
                        ui.selectable_value(&mut sec.kind, k, k.name());
                    }
                });
        });
        ui.separator();
        if ui.button("Play this section").clicked() {
            st.mode = PlayMode::Section;
            ui.close();
        }
        if let Some(e) = b.entry {
            if ui
                .button("Repeat it here")
                .on_hover_text("Play the same section again after this one")
                .clicked()
            {
                let name = st.song.arrangement[e].clone();
                st.song.arrangement.insert(e + 1, name);
                ui.close();
            }
            if ui.button("Duplicate as a new section").clicked() {
                let d = songops::duplicate_section(&mut st.song, b.section);
                let name = st.song.sections[d].name.clone();
                st.song.arrangement.insert(e + 1, name);
                st.sel.section = Some(d);
                st.sel.entry = Some(e + 1);
                ui.close();
            }
            if ui.button("Remove from the arrangement").clicked() {
                st.song.arrangement.remove(e);
                st.sel.entry = None;
                ui.close();
            }
        } else if ui.button("Add to the arrangement").clicked() {
            let name = st.song.sections[b.section].name.clone();
            st.song.arrangement.push(name);
            ui.close();
        }
        if ui.button("Delete the section").clicked() {
            songops::delete_section(&mut st.song, b.section);
            st.sel.section = None;
            st.sel.entry = None;
            ui.close();
        }
    });
}

/// A clip's pattern ticks, looked up once.
fn pattern_ticks(st: &Studio, name: &str) -> Option<u32> {
    st.song
        .pattern(name)
        .map(|i| st.song.patterns[i].ticks().max(1))
}

fn lanes_ui(
    ui: &mut egui::Ui,
    st: &mut Studio,
    lanes: Rect,
    blocks: &[Block],
    x_of: &dyn Fn(u32) -> f32,
    tick_of: &dyn Fn(f32) -> u32,
) {
    let lane_h = st.arrange.lane_h;
    let top = lanes.top() - st.arrange.scroll_y;
    let p = ui.painter().with_clip_rect(lanes);
    let changed = st.collab_changed_tracks();
    // Lane backgrounds and the empty-space interaction (double-click to add).
    for t in 0..st.song.tracks.len() {
        let y = top + t as f32 * lane_h;
        let r = Rect::from_min_size(pos2(lanes.left(), y), vec2(lanes.width(), lane_h));
        if r.bottom() < lanes.top() || r.top() > lanes.bottom() {
            continue;
        }
        if t == st.sel.track {
            p.rect_filled(r, CornerRadius::ZERO, theme::line(5));
        }
        p.hline(r.x_range(), r.bottom(), Stroke::new(1.0, theme::line(10)));
        let resp = ui.interact(
            r.intersect(lanes),
            ui.id().with(("lane", t)),
            Sense::click(),
        );
        if resp.clicked() {
            st.sel.track = t;
            st.sel.clip = None;
        }
        if resp.double_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let tick = tick_of(pos.x);
                if let Some(b) = blocks
                    .iter()
                    .find(|b| tick >= b.start && tick < b.start + b.ticks)
                {
                    let beat = (tick - b.start) / PPQ;
                    let bpb = st.song.beats_per_bar.max(1);
                    let beat = (beat / bpb) * bpb;
                    st.arrange.add = Some(AddClip {
                        section: b.section,
                        track: t,
                        beat,
                        pos,
                    });
                }
            }
        }
    }
    // Clips.
    let mut clicked: Option<(usize, usize)> = None;
    for b in blocks {
        let x0 = x_of(b.start);
        let x1 = x_of(b.start + b.ticks);
        if x1 < lanes.left() || x0 > lanes.right() {
            continue;
        }
        let sec_ticks = b.ticks;
        for ci in 0..st.song.sections[b.section].clips.len() {
            let clip = st.song.sections[b.section].clips[ci].clone();
            let Some(ti) = st.song.track(&clip.track) else {
                continue;
            };
            let Some(pt) = pattern_ticks(st, &clip.pattern) else {
                continue;
            };
            let span = clip.span(pt, sec_ticks);
            if span == 0 {
                continue;
            }
            let y = top + ti as f32 * lane_h;
            let cx0 = x_of(b.start + clip.at * PPQ);
            let cx1 = x_of(b.start + clip.at * PPQ + span);
            let r = Rect::from_min_max(pos2(cx0 + 0.5, y + 3.0), pos2(cx1 - 0.5, y + lane_h - 3.0));
            if r.bottom() < lanes.top()
                || r.top() > lanes.bottom()
                || r.right() < lanes.left()
                || r.left() > lanes.right()
            {
                continue;
            }
            let selected = st.sel.clip == Some((b.section, ci));
            let colour = theme::rgb(st.song.tracks[ti].colour);
            let id = ui.id().with(("clip", b.entry, b.section, ci));
            let resp = ui.interact(r.intersect(lanes), id, Sense::click_and_drag());
            draw_clip(
                &p,
                st,
                r,
                &clip,
                pt,
                colour,
                selected,
                resp.hovered(),
                changed.contains(&clip.track),
            );
            if resp.clicked() || resp.drag_started() {
                clicked = Some((b.section, ci));
            }
            if resp.drag_started() {
                let origin = ui.input(|i| i.pointer.press_origin());
                if let Some(o) = origin.or(resp.interact_pointer_pos()) {
                    st.arrange.drag = Some(Drag::Clip {
                        section: b.section,
                        clip: ci,
                        at: clip.at,
                        track: ti,
                        origin: o,
                    });
                }
            }
            if resp.secondary_clicked() {
                clicked = Some((b.section, ci));
            }
            resp.context_menu(|ui| {
                if ui.button("Duplicate after itself").clicked() {
                    let mut c = clip.clone();
                    c.at += (pt / PPQ) * clip.times.max(1);
                    if c.at < sec_ticks / PPQ {
                        st.song.sections[b.section].clips.push(c);
                    }
                    ui.close();
                }
                if ui.button("Fill the section").clicked() {
                    st.song.sections[b.section].clips[ci].times = 0;
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    st.song.sections[b.section].clips.remove(ci);
                    st.sel.clip = None;
                    ui.close();
                }
            });
        }
        // Section boundaries in the lanes.
        p.vline(x0, lanes.y_range(), Stroke::new(1.0, theme::line(24)));
    }
    if let Some((s, c)) = clicked {
        st.select_clip(s, c);
        st.detail_open = true;
    }
    // Moving a clip: beats (Shift: bars), and across lanes to another track.
    if let Some(Drag::Clip {
        section,
        clip,
        at,
        track,
        origin,
    }) = st.arrange.drag
    {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let bpb = st.song.beats_per_bar.max(1) as i64;
            let shift = ui.input(|i| i.modifiers.shift);
            let mut d = ((pos.x - origin.x) / st.arrange.px_per_beat).round() as i64;
            if shift {
                d = (d as f64 / bpb as f64).round() as i64 * bpb;
            }
            let beats = (st.song.sections[section].bars * st.song.beats_per_bar) as i64;
            let new_at = (at as i64 + d).clamp(0, (beats - 1).max(0)) as u32;
            let lane = ((pos.y - top) / lane_h).floor() as i64;
            let new_track = if lane >= 0 && (lane as usize) < st.song.tracks.len() {
                lane as usize
            } else {
                track
            };
            let name = st.song.tracks[new_track].name.clone();
            if let Some(c) = st.song.sections[section].clips.get_mut(clip) {
                c.at = new_at;
                c.track = name;
            }
            if new_track != track {
                st.sel.track = new_track;
                st.sel.pattern_track = new_track;
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            st.arrange.drag = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_clip(
    p: &egui::Painter,
    st: &Studio,
    r: Rect,
    clip: &Clip,
    pt: u32,
    colour: Color32,
    selected: bool,
    hovered: bool,
    changed: bool,
) {
    let fill = theme::mix(
        BG3,
        colour,
        if selected {
            0.5
        } else if hovered {
            0.38
        } else {
            0.3
        },
    );
    p.rect(
        r,
        CornerRadius::same(2),
        fill,
        Stroke::new(
            1.0,
            if selected {
                TEXT
            } else {
                theme::with_alpha(colour, 160)
            },
        ),
        StrokeKind::Inside,
    );
    p.rect_filled(
        Rect::from_min_size(r.min, vec2(r.width(), 3.0)),
        CornerRadius::same(1),
        colour,
    );
    if changed {
        p.rect_stroke(
            r.expand(1.0),
            CornerRadius::same(3),
            Stroke::new(1.5, WARN),
            StrokeKind::Outside,
        );
    }
    let px_per_tick = st.arrange.px_per_beat / PPQ as f32;
    // Repeat boundaries.
    let reps = ((r.width() / px_per_tick) / pt as f32).ceil() as u32;
    for k in 1..reps {
        let x = r.left() + (k * pt) as f32 * px_per_tick;
        p.vline(
            x,
            r.y_range().shrink(4.0),
            Stroke::new(1.0, Color32::from_black_alpha(90)),
        );
    }
    let Some(pi) = st.song.pattern(&clip.pattern) else {
        return;
    };
    let pat = &st.song.patterns[pi];
    let label = if clip.transpose != 0 {
        format!("{} {:+}", pat.name, clip.transpose)
    } else {
        pat.name.clone()
    };
    if r.width() > 24.0 {
        p.with_clip_rect(r.intersect(p.clip_rect())).text(
            r.left_top() + vec2(4.0, 5.0),
            Align2::LEFT_TOP,
            label,
            theme::font_semi(11.0),
            TEXT,
        );
    }
    // Mini notes, pitched within the pattern's own range.
    if pat.notes.is_empty() || r.height() < 20.0 {
        return;
    }
    let (lo, hi) = pat
        .notes
        .iter()
        .fold((127u8, 0u8), |(lo, hi), n| (lo.min(n.2), hi.max(n.2)));
    let span_k = (hi - lo).max(6) as f32;
    let area = Rect::from_min_max(
        pos2(r.left(), r.top() + 17.0),
        pos2(r.right(), r.bottom() - 3.0),
    );
    let clip_p = p.with_clip_rect(area.intersect(p.clip_rect()));
    let vis = p.clip_rect();
    let note_c = theme::mix(colour, TEXT, 0.35);
    for k in 0..reps {
        let base = r.left() + (k * pt) as f32 * px_per_tick;
        if base > vis.right() {
            break;
        }
        if base + pt as f32 * px_per_tick < vis.left() {
            continue;
        }
        for n in &pat.notes {
            if n.0 >= pt {
                continue;
            }
            let x = base + n.0 as f32 * px_per_tick;
            let w = (n.1.min(pt - n.0) as f32 * px_per_tick).max(1.0);
            let y = area.bottom() - (n.2 - lo) as f32 / span_k * (area.height() - 2.0) - 2.0;
            clip_p.rect_filled(
                Rect::from_min_size(pos2(x, y), vec2(w, 2.0)),
                CornerRadius::ZERO,
                note_c,
            );
        }
    }
}

fn headers_ui(ui: &mut egui::Ui, st: &mut Studio, head: Rect, lanes: Rect) {
    let p = ui.painter().with_clip_rect(head);
    p.rect_filled(head, CornerRadius::ZERO, BG0);
    p.vline(
        head.right(),
        head.y_range(),
        Stroke::new(1.0, theme::line(20)),
    );
    // Above the lanes: track buttons.
    let top_area = Rect::from_min_max(head.min, pos2(head.right(), lanes.top()));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(top_area.shrink2(vec2(8.0, 6.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.label(
        egui::RichText::new("Tracks")
            .font(theme::font_semi(12.5))
            .color(DIM),
    );
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui
            .small_button("+ Kit")
            .on_hover_text("Add a drum kit track")
            .clicked()
        {
            let t = songops::new_track(&mut st.song, true);
            st.sel.track = t;
        }
        if ui
            .small_button("+ Synth")
            .on_hover_text("Add a synth track")
            .clicked()
        {
            let t = songops::new_track(&mut st.song, false);
            st.sel.track = t;
        }
    });
    let lane_h = st.arrange.lane_h;
    let top = lanes.top() - st.arrange.scroll_y;
    let changed = st.collab_changed_tracks();
    let mut action: Option<(usize, TrackAction)> = None;
    for t in 0..st.song.tracks.len() {
        let r = Rect::from_min_size(
            pos2(head.left(), top + t as f32 * lane_h),
            vec2(head.width(), lane_h),
        );
        if r.bottom() < lanes.top() || r.top() > lanes.bottom() {
            continue;
        }
        let vis = r.intersect(lanes);
        let selected = st.sel.track == t;
        let lp = ui.painter().with_clip_rect(vis);
        if selected {
            lp.rect_filled(r, CornerRadius::ZERO, theme::line(10));
        }
        lp.hline(r.x_range(), r.bottom(), Stroke::new(1.0, theme::line(10)));
        let track = &st.song.tracks[t];
        let colour = theme::rgb(track.colour);
        lp.rect_filled(
            Rect::from_min_size(r.min + vec2(0.0, 1.0), vec2(4.0, r.height() - 2.0)),
            CornerRadius::ZERO,
            colour,
        );
        let resp = ui.interact(vis, ui.id().with(("track-head", t)), Sense::click());
        if resp.clicked() {
            st.sel.track = t;
        }
        if resp.double_clicked() {
            st.sel.track = t;
            st.detail = crate::app::Detail::Instrument;
            st.detail_open = true;
        }
        lp.text(
            r.left_top() + vec2(12.0, 7.0),
            Align2::LEFT_TOP,
            truncate(&track.name, 16),
            theme::font_semi(13.5),
            if selected { TEXT } else { DIM },
        );
        let kind = match &track.instrument {
            Instrument::Synth(s) => format!("Synth, {} osc", s.oscs.len()),
            Instrument::Kit(k) => format!("Kit, {} drums", k.drums.len()),
        };
        lp.text(
            r.left_top() + vec2(12.0, 26.0),
            Align2::LEFT_TOP,
            kind,
            theme::font_body(10.5),
            FAINT,
        );
        if changed.contains(&track.name) {
            lp.circle_filled(pos2(r.right() - 64.0, r.top() + 14.0), 3.5, WARN);
        }
        // Mute and solo.
        let m_r = Rect::from_min_size(pos2(r.right() - 52.0, r.top() + 6.0), vec2(20.0, 18.0));
        let s_r = Rect::from_min_size(pos2(r.right() - 28.0, r.top() + 6.0), vec2(20.0, 18.0));
        if vis.contains_rect(m_r) {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(m_r));
            if widgets::toggle(&mut c, track.mute, "M", WARN)
                .on_hover_text("Mute")
                .clicked()
            {
                action = Some((t, TrackAction::Mute));
            }
        }
        if vis.contains_rect(s_r) {
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(s_r));
            if widgets::toggle(&mut c, track.solo, "S", ACCENT)
                .on_hover_text("Solo")
                .clicked()
            {
                action = Some((t, TrackAction::Solo));
            }
        }
        // A thin level meter along the bottom.
        if let Some(m) = st.meters.tracks.get(t) {
            let mr =
                Rect::from_min_size(pos2(r.right() - 52.0, r.bottom() - 13.0), vec2(44.0, 6.0));
            if vis.contains_rect(mr) {
                widgets::meter_horizontal(ui, mr, ui.id().with(("lane-meter", t)), m.peak);
            }
        }
        if resp.secondary_clicked() {
            st.arrange.rename = track.name.clone();
        }
        resp.context_menu(|ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.arrange.rename).desired_width(120.0));
                if ui.button("Rename").clicked() {
                    action = Some((t, TrackAction::Rename));
                    ui.close();
                }
            });
            let mut c = theme::rgb(st.song.tracks[t].colour);
            ui.horizontal(|ui| {
                ui.label("Colour");
                for &pc in theme::TRACK_COLOURS.iter() {
                    let (cr, cresp) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
                    ui.painter()
                        .rect_filled(cr, CornerRadius::same(2), theme::rgb(pc));
                    if cresp.clicked() {
                        c = theme::rgb(pc);
                    }
                }
            });
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut c,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
                || c != theme::rgb(st.song.tracks[t].colour)
            {
                st.song.tracks[t].colour = theme::to_rgb(c);
            }
            ui.separator();
            if ui.button("Move up").clicked() {
                action = Some((t, TrackAction::Up));
                ui.close();
            }
            if ui.button("Move down").clicked() {
                action = Some((t, TrackAction::Down));
                ui.close();
            }
            if ui.button("Duplicate").clicked() {
                action = Some((t, TrackAction::Duplicate));
                ui.close();
            }
            if ui.button("Delete (and its clips)").clicked() {
                action = Some((t, TrackAction::Delete));
                ui.close();
            }
        });
    }
    if let Some((t, a)) = action {
        match a {
            TrackAction::Mute => st.song.tracks[t].mute = !st.song.tracks[t].mute,
            TrackAction::Solo => st.song.tracks[t].solo = !st.song.tracks[t].solo,
            TrackAction::Rename => {
                let n = st.arrange.rename.trim().to_string();
                songops::rename_track(&mut st.song, t, &n);
            }
            TrackAction::Up if t > 0 => st.song.tracks.swap(t, t - 1),
            TrackAction::Down if t + 1 < st.song.tracks.len() => st.song.tracks.swap(t, t + 1),
            TrackAction::Duplicate => st.sel.track = songops::duplicate_track(&mut st.song, t),
            TrackAction::Delete => songops::delete_track(&mut st.song, t),
            _ => {}
        }
    }
}

enum TrackAction {
    Mute,
    Solo,
    Rename,
    Up,
    Down,
    Duplicate,
    Delete,
}

fn playhead(ui: &mut egui::Ui, st: &Studio, tl: Rect, blocks: &[Block], x_of: &dyn Fn(u32) -> f32) {
    let s = &st.status;
    let tick = match st.mode {
        PlayMode::Song if blocks.first().is_some_and(|b| b.entry.is_some()) => Some(s.song_tick),
        PlayMode::Pattern => None,
        _ => {
            // Section or director: on the selected entry of that section, else its first.
            s.section.and_then(|sec| {
                let pick = blocks
                    .iter()
                    .find(|b| b.section == sec && b.entry == st.sel.entry)
                    .or_else(|| blocks.iter().find(|b| b.section == sec));
                pick.map(|b| b.start + s.section_tick)
            })
        }
    };
    let Some(t) = tick else { return };
    if !s.playing && t == 0 {
        return;
    }
    let x = x_of(t);
    if x < tl.left() || x > tl.right() {
        return;
    }
    let p = ui.painter().with_clip_rect(tl);
    p.vline(x, tl.y_range(), Stroke::new(1.5, ACCENT));
    p.add(egui::Shape::convex_polygon(
        vec![
            pos2(x - 5.0, tl.top()),
            pos2(x + 5.0, tl.top()),
            pos2(x, tl.top() + 7.0),
        ],
        ACCENT,
        Stroke::NONE,
    ));
}

fn add_popup(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(add) = &st.arrange.add else { return };
    let (section, track, beat, pos) = (add.section, add.track, add.beat, add.pos);
    let mut close = false;
    let area = egui::Area::new(ui.id().with("add-clip"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Add a clip on {} at beat {}",
                        st.song.tracks[track].name,
                        beat + 1
                    ))
                    .color(DIM)
                    .small(),
                );
                let kit = st.is_kit(track);
                if ui.button("New pattern").clicked() {
                    let beats = st.song.beats_per_bar * 1.max(if kit { 1 } else { 2 });
                    let base = format!(
                        "{} {}",
                        st.song.tracks[track].name, st.song.sections[section].name
                    );
                    let pi = songops::new_pattern(&mut st.song, &base, beats);
                    push_clip(st, section, track, pi, beat);
                    close = true;
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for pi in 0..st.song.patterns.len() {
                            let pat = &st.song.patterns[pi];
                            if ui
                                .button(format!(
                                    "{}  ({} beats, {} notes)",
                                    pat.name,
                                    pat.beats,
                                    pat.notes.len()
                                ))
                                .clicked()
                            {
                                push_clip(st, section, track, pi, beat);
                                close = true;
                            }
                        }
                    });
            });
        });
    let clicked_outside =
        ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
    if close || clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.arrange.add = None;
    }
}

fn push_clip(st: &mut Studio, section: usize, track: usize, pattern: usize, beat: u32) {
    let clip = Clip {
        track: st.song.tracks[track].name.clone(),
        pattern: st.song.patterns[pattern].name.clone(),
        at: beat,
        times: 1,
        transpose: 0,
    };
    st.song.sections[section].clips.push(clip);
    let ci = st.song.sections[section].clips.len() - 1;
    st.select_clip(section, ci);
    st.detail_open = true;
}

fn keys(ui: &mut egui::Ui, st: &mut Studio) {
    if st.focus != crate::app::Pane::Arrange || Studio::text_focus(ui.ctx()) {
        return;
    }
    let del = ui.input_mut(|i| {
        i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
            || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
    });
    if del {
        if let Some((s, c)) = st.sel.clip.take() {
            if c < st.song.sections[s].clips.len() {
                st.song.sections[s].clips.remove(c);
            }
        }
    }
    let dup = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D));
    if dup {
        if let Some((s, c)) = st.sel.clip {
            let clip = st.song.sections[s].clips[c].clone();
            if let Some(pt) = pattern_ticks(st, &clip.pattern) {
                let mut n = clip.clone();
                n.at += (pt / PPQ) * clip.times.max(1);
                if n.at < st.song.sections[s].bars * st.song.beats_per_bar {
                    st.song.sections[s].clips.push(n);
                    let ci = st.song.sections[s].clips.len() - 1;
                    st.select_clip(s, ci);
                }
            }
        }
    }
}
