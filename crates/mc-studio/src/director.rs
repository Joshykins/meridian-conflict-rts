//! The director: how the game picks sections by the battle's intensity.
//! Each section's settings on the left (kind, intensity range, where it may go
//! next, how often it may leave early), and on the right every section laid
//! out over the intensity axis with the live intensity and the section
//! playing now, a trace of the last minute, and scripted ramps to hear the
//! transitions without a battle.

use crate::app::{PlayMode, Ramp, Studio};
use crate::songops;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT};
use crate::widgets::{self, caption, range_slider};
use eframe::egui::{
    self, pos2, vec2, Align2, CornerRadius, Id, Rect, Sense, Shape, Stroke, StrokeKind,
};
use mc_music::{Command, SectionKind};

pub struct State {
    rename: String,
    /// (seconds, intensity, section) samples of the last minute.
    trace: Vec<(f64, f32, Option<usize>)>,
    ramp_secs: f32,
}

impl Default for State {
    fn default() -> State {
        State {
            rename: String::new(),
            trace: Vec::new(),
            ramp_secs: 60.0,
        }
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    let d = &mut st.director;
    if d.trace.last().is_none_or(|t| now - t.0 > 0.1) {
        d.trace.push((now, st.status.intensity, st.status.section));
        d.trace.retain(|t| now - t.0 < 60.0);
    }
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let left = Rect::from_min_size(
        area.min + vec2(10.0, 8.0),
        vec2(360.0, area.height() - 16.0),
    );
    let right = Rect::from_min_max(
        pos2(left.right() + 16.0, area.top() + 8.0),
        area.max - vec2(10.0, 8.0),
    );
    let mut l = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    egui::ScrollArea::vertical()
        .id_salt("director-left")
        .auto_shrink([false, false])
        .show(&mut l, |ui| {
            section_list(ui, st);
            ui.add_space(10.0);
            properties(ui, st);
        });
    let mut r = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(right)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    controls(&mut r, st, now);
    let rest = r.available_rect_before_wrap();
    let trace_h = 90.0;
    let diagram_r =
        Rect::from_min_max(rest.min, pos2(rest.right(), rest.bottom() - trace_h - 10.0));
    let trace_r = Rect::from_min_max(pos2(rest.left(), diagram_r.bottom() + 10.0), rest.max);
    diagram(&mut r, st, diagram_r);
    trace(&r, st, trace_r, now);
}

fn section_list(ui: &mut egui::Ui, st: &mut Studio) {
    ui.horizontal(|ui| {
        caption(ui, "Sections");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Add").clicked() {
                let i = songops::new_section(&mut st.song, "Section", 4);
                st.sel.section = Some(i);
            }
        });
    });
    for i in 0..st.song.sections.len() {
        let s = &st.song.sections[i];
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
        let p = ui.painter();
        let sel = st.sel.section == Some(i);
        let playing = st.status.playing && st.status.section == Some(i);
        if sel {
            p.rect_filled(r, CornerRadius::same(2), theme::with_alpha(ACCENT, 38));
        } else if resp.hovered() {
            p.rect_filled(r, CornerRadius::same(2), theme::line(8));
        }
        p.rect_filled(
            Rect::from_min_size(r.min + vec2(4.0, 6.0), vec2(6.0, 10.0)),
            CornerRadius::same(1),
            theme::section_colour(s.kind),
        );
        p.text(
            r.left_center() + vec2(16.0, 0.0),
            Align2::LEFT_CENTER,
            &s.name,
            theme::font_semi(12.5),
            if sel { TEXT } else { DIM },
        );
        let info = match s.kind {
            SectionKind::Stinger | SectionKind::Ending => {
                format!("{}  {} bars", s.kind.name(), s.bars)
            }
            _ => format!(
                "{}  {:.2}-{:.2}  {} bars",
                s.kind.name(),
                s.intensity.0,
                s.intensity.1,
                s.bars
            ),
        };
        p.text(
            r.right_center() - vec2(6.0, 0.0),
            Align2::RIGHT_CENTER,
            info,
            theme::font_body(10.5),
            FAINT,
        );
        if playing {
            p.rect_stroke(
                Rect::from_min_size(r.min + vec2(2.0, 4.0), vec2(10.0, 14.0)),
                CornerRadius::same(2),
                Stroke::new(1.5, ACCENT),
                StrokeKind::Outside,
            );
        }
        if resp.clicked() {
            st.sel.section = Some(i);
            st.director.rename = s.name.clone();
        }
    }
}

fn properties(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(i) = st.sel.section.filter(|&i| i < st.song.sections.len()) else {
        ui.label(egui::RichText::new("Select a section").color(FAINT));
        return;
    };
    if st.director.rename.is_empty() {
        st.director.rename = st.song.sections[i].name.clone();
    }
    widgets::group(ui, "Section", |ui| {
        egui::Grid::new("dir-props").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
            ui.label(egui::RichText::new("Name").color(FAINT));
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut st.director.rename).desired_width(150.0));
                if (r.lost_focus() || ui.small_button("Rename").clicked()) && st.director.rename.trim() != st.song.sections[i].name {
                    let n = st.director.rename.trim().to_string();
                    songops::rename_section(&mut st.song, i, &n);
                }
            });
            ui.end_row();
            let names: Vec<String> = st.song.sections.iter().map(|s| s.name.clone()).collect();
            let s = &mut st.song.sections[i];
            ui.label(egui::RichText::new("Kind").color(FAINT));
            egui::ComboBox::from_id_salt("dir-kind").selected_text(s.kind.name()).show_ui(ui, |ui| {
                for k in SectionKind::ALL {
                    ui.selectable_value(&mut s.kind, k, k.name());
                }
            });
            ui.end_row();
            ui.label(egui::RichText::new("Bars").color(FAINT));
            ui.add(egui::DragValue::new(&mut s.bars).range(1..=256));
            ui.end_row();
            ui.label(egui::RichText::new("Intensity").color(FAINT));
            ui.vertical(|ui| {
                range_slider(ui, Id::new(("dir-range", i)), &mut s.intensity, 220.0);
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut s.intensity.0).range(0.0..=1.0).speed(0.01).fixed_decimals(2));
                    ui.label(egui::RichText::new("to").color(FAINT));
                    ui.add(egui::DragValue::new(&mut s.intensity.1).range(0.0..=1.0).speed(0.01).fixed_decimals(2));
                });
                if s.intensity.0 > s.intensity.1 {
                    s.intensity.1 = s.intensity.0;
                }
            });
            ui.end_row();
            ui.label(egui::RichText::new("Leave early every").color(FAINT));
            ui.add(egui::DragValue::new(&mut s.exit_every).range(0..=64).custom_formatter(|v, _| if v == 0.0 { "only at the end".into() } else { format!("{v:.0} bars") }))
                .on_hover_text("When the intensity leaves the range, the director may move on at these bar lines instead of waiting for the end");
            ui.end_row();
            ui.label(egui::RichText::new("Next").color(FAINT));
            ui.vertical(|ui| {
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    for (k, n) in s.next.iter().enumerate() {
                        if widgets::toggle(ui, false, &format!("{n}  \u{00D7}"), ACCENT).on_hover_text("Remove").clicked() {
                            remove = Some(k);
                        }
                    }
                    if s.next.is_empty() {
                        ui.label(egui::RichText::new("Any loop that fits").color(FAINT).small());
                    }
                });
                if let Some(k) = remove {
                    s.next.remove(k);
                }
                egui::ComboBox::from_id_salt("dir-next-add").selected_text("Add...").show_ui(ui, |ui| {
                    for n in &names {
                        if *n != s.name && !s.next.contains(n) && ui.selectable_label(false, n).clicked() {
                            s.next.push(n.clone());
                        }
                    }
                });
            });
            ui.end_row();
        });
        ui.horizontal(|ui| {
            let name = st.song.sections[i].name.clone();
            match st.song.sections[i].kind {
                SectionKind::Stinger => {
                    if ui.button("Cue it now").clicked() {
                        st.send(Command::Cue(name));
                    }
                }
                SectionKind::Ending => {
                    if ui.button("Finish into it").clicked() {
                        st.send(Command::Finish(Some(name)));
                    }
                }
                _ => {
                    if ui.button("Loop this section").clicked() {
                        st.mode = PlayMode::Section;
                        if !st.status.playing {
                            st.toggle_play();
                        }
                    }
                }
            }
        });
    });
}

fn controls(ui: &mut egui::Ui, st: &mut Studio, now: f64) {
    ui.horizontal(|ui| {
        let on = st.mode == PlayMode::Director;
        if widgets::toggle(ui, on, "Director playing", ACCENT)
            .on_hover_text("Let the director pick sections by intensity, as the game does")
            .clicked()
        {
            st.mode = if on {
                PlayMode::Song
            } else {
                PlayMode::Director
            };
            if !on && !st.status.playing {
                st.toggle_play();
            }
        }
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Ramp over").color(FAINT));
        ui.add(
            egui::DragValue::new(&mut st.director.ramp_secs)
                .range(5.0..=600.0)
                .suffix(" s"),
        );
        let secs = st.director.ramp_secs as f64;
        let cur = st.status.intensity;
        let start = |points: Vec<(f64, f32)>, st: &mut Studio| {
            st.mode = PlayMode::Director;
            if !st.status.playing {
                st.toggle_play();
            }
            st.ramp = Some(Ramp {
                points,
                started: now,
            });
        };
        if ui.button("Rise to 1").clicked() {
            start(vec![(0.0, cur), (secs, 1.0)], st);
        }
        if ui.button("Fall to 0").clicked() {
            start(vec![(0.0, cur), (secs, 0.0)], st);
        }
        if ui.button("0 to 1 and back").clicked() {
            start(vec![(0.0, 0.0), (secs, 1.0), (secs * 2.0, 0.0)], st);
        }
        if ui
            .button("Battle")
            .on_hover_text("Calm, a sudden fight, a lull, a bigger fight, calm")
            .clicked()
        {
            let s = secs / 4.0;
            start(
                vec![
                    (0.0, 0.1),
                    (s, 0.15),
                    (s + 3.0, 0.8),
                    (s * 2.0, 0.7),
                    (s * 2.5, 0.35),
                    (s * 3.0, 0.4),
                    (s * 3.2, 1.0),
                    (s * 4.0, 0.9),
                    (s * 5.0, 0.1),
                ],
                st,
            );
        }
        if st.ramp.is_some() {
            let left = st
                .ramp
                .as_ref()
                .and_then(|r| r.points.last().map(|p| p.0 - (now - r.started)))
                .unwrap_or(0.0);
            if widgets::toggle(ui, true, &format!("Stop ramp ({left:.0} s)"), theme::WARN).clicked()
            {
                st.ramp = None;
            }
        }
    });
    ui.add_space(6.0);
}

fn diagram(ui: &mut egui::Ui, st: &mut Studio, rect: Rect) {
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, CornerRadius::same(4), BG0);
    let g = rect.shrink2(vec2(16.0, 24.0));
    let x_of = |v: f32| g.left() + g.width() * v.clamp(0.0, 1.0);
    for k in 0..=10 {
        let x = x_of(k as f32 / 10.0);
        p.vline(
            x,
            g.y_range(),
            Stroke::new(1.0, theme::line(if k % 5 == 0 { 20 } else { 8 })),
        );
        p.text(
            pos2(x, g.bottom() + 4.0),
            Align2::CENTER_TOP,
            format!("{:.1}", k as f32 / 10.0),
            theme::font_body(10.0),
            FAINT,
        );
    }
    p.text(
        rect.left_top() + vec2(10.0, 6.0),
        Align2::LEFT_TOP,
        "Sections by intensity",
        theme::font_semi(12.5),
        DIM,
    );
    let chosen: Vec<usize> = (0..st.song.sections.len())
        .filter(|&i| {
            !matches!(
                st.song.sections[i].kind,
                SectionKind::Stinger | SectionKind::Ending
            )
        })
        .collect();
    let cued: Vec<usize> = (0..st.song.sections.len())
        .filter(|&i| {
            matches!(
                st.song.sections[i].kind,
                SectionKind::Stinger | SectionKind::Ending
            )
        })
        .collect();
    let rows = chosen.len() + usize::from(!cued.is_empty());
    let row_h = (g.height() / rows.max(1) as f32).clamp(14.0, 34.0);
    let playing = if st.status.playing {
        st.status.section
    } else {
        None
    };
    let mut clicked = None;
    for (row, &i) in chosen.iter().enumerate() {
        let s = &st.song.sections[i];
        let y = g.top() + row as f32 * row_h;
        let (lo, hi) = s.intensity;
        let r = Rect::from_min_max(
            pos2(x_of(lo), y + 3.0),
            pos2(x_of(hi).max(x_of(lo) + 4.0), y + row_h - 3.0),
        );
        let c = theme::section_colour(s.kind);
        let is_playing = playing == Some(i);
        let sel = st.sel.section == Some(i);
        let fill = theme::mix(
            BG2,
            c,
            if is_playing {
                0.7
            } else if sel {
                0.45
            } else {
                0.25
            },
        );
        p.rect(
            r,
            CornerRadius::same(3),
            fill,
            Stroke::new(
                if is_playing { 2.0 } else { 1.0 },
                if is_playing {
                    ACCENT
                } else if sel {
                    TEXT
                } else {
                    theme::with_alpha(c, 140)
                },
            ),
            StrokeKind::Inside,
        );
        let label = if s.next.is_empty() {
            s.name.clone()
        } else {
            format!("{}   then {}", s.name, s.next.join(", "))
        };
        p.text(
            r.left_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            theme::font_semi(12.0),
            TEXT,
        );
        if ui
            .interact(r, Id::new(("dir-bar", i)), Sense::click())
            .clicked()
        {
            clicked = Some(i);
        }
    }
    if !cued.is_empty() {
        let y = g.top() + chosen.len() as f32 * row_h;
        let mut x = g.left();
        p.text(
            pos2(x, y + row_h * 0.5),
            Align2::LEFT_CENTER,
            "Cued:",
            theme::font_body(11.0),
            FAINT,
        );
        x += 44.0;
        for &i in &cued {
            let s = &st.song.sections[i];
            let w = 14.0 + s.name.len() as f32 * 7.0;
            let r = Rect::from_min_size(pos2(x, y + 3.0), vec2(w, row_h - 6.0));
            let c = theme::section_colour(s.kind);
            let is_playing = playing == Some(i);
            p.rect(
                r,
                CornerRadius::same(3),
                theme::mix(BG2, c, if is_playing { 0.7 } else { 0.25 }),
                Stroke::new(1.0, theme::with_alpha(c, 160)),
                StrokeKind::Inside,
            );
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                &s.name,
                theme::font_semi(11.5),
                TEXT,
            );
            if ui
                .interact(r, Id::new(("dir-cue", i)), Sense::click())
                .clicked()
            {
                clicked = Some(i);
            }
            x += w + 6.0;
        }
    }
    if let Some(i) = clicked {
        st.sel.section = Some(i);
        st.director.rename = st.song.sections[i].name.clone();
    }
    // The live intensity (smoothed) and its target.
    let xi = x_of(st.status.intensity);
    p.vline(xi, rect.top() + 18.0..=g.bottom(), Stroke::new(2.0, ACCENT));
    p.text(
        pos2(xi + 4.0, rect.top() + 20.0),
        Align2::LEFT_TOP,
        format!("{:.2}", st.status.intensity),
        theme::font_semi(12.0),
        ACCENT,
    );
    let xt = x_of(st.status.intensity_target);
    if (xt - xi).abs() > 2.0 {
        p.add(Shape::dashed_line(
            &[pos2(xt, rect.top() + 18.0), pos2(xt, g.bottom())],
            Stroke::new(1.0, TEXT),
            4.0,
            4.0,
        ));
    }
}

fn trace(ui: &egui::Ui, st: &Studio, rect: Rect, now: f64) {
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, CornerRadius::same(4), BG0);
    p.text(
        rect.left_top() + vec2(8.0, 4.0),
        Align2::LEFT_TOP,
        "Last minute",
        theme::font_body(10.5),
        FAINT,
    );
    let g = rect.shrink2(vec2(8.0, 16.0));
    let x_of = |t: f64| g.right() - ((now - t) / 60.0) as f32 * g.width();
    let y_of = |v: f32| g.bottom() - g.height() * v.clamp(0.0, 1.0);
    let tr = &st.director.trace;
    // Section changes as ticks along the bottom.
    for w in tr.windows(2) {
        if w[0].2 != w[1].2 {
            if let Some(s) = w[1].2.and_then(|i| st.song.sections.get(i)) {
                let x = x_of(w[1].0);
                p.vline(
                    x,
                    g.y_range(),
                    Stroke::new(1.0, theme::with_alpha(theme::section_colour(s.kind), 120)),
                );
                p.text(
                    pos2(x + 3.0, g.bottom()),
                    Align2::LEFT_BOTTOM,
                    &s.name,
                    theme::font_body(9.5),
                    DIM,
                );
            }
        }
    }
    let pts: Vec<egui::Pos2> = tr
        .iter()
        .map(|(t, v, _)| pos2(x_of(*t), y_of(*v)))
        .collect();
    if pts.len() > 1 {
        p.add(Shape::line(pts, Stroke::new(1.5, ACCENT)));
    }
}
