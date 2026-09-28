//! The transport bar: pages, play/stop, mode, position, tempo, the battle's
//! intensity, stingers and endings for the director, history, and the master
//! meter with loudness, engine load and voice count.

use crate::app::{tab, Page, PlayMode, Studio};
use crate::notes::position_text;
use crate::theme::{self, ACCENT, BG0, DIM, FAINT, TEXT, WARN};
use crate::widgets::{self, icon_button, Icon};
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind};
use mc_music::{Command, SectionKind, PPQ};

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    // Narrow windows drop what is also elsewhere: the title and tempo (the
    // song panel), then the stinger and ending buttons (the director page).
    let width = ui.available_width();
    let roomy = width > 1760.0;
    let wide = width > 1480.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let name = if st.song.name.is_empty() { "Untitled".to_string() } else { st.song.name.clone() };
        if roomy {
            ui.label(egui::RichText::new(truncate(&name, 18)).font(theme::font_light(20.0)).color(TEXT));
        }
        let (dot, tip) = if st.dirty { (WARN, "Unsaved changes (Ctrl+S saves)") } else { (theme::GOOD, "Saved") };
        let (r, resp) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.5, dot);
        resp.on_hover_text(tip);
        ui.add_space(8.0);
        for (p, n) in [(Page::Arrange, "Arrange"), (Page::Mixer, "Mixer"), (Page::Director, "Director")] {
            if tab(ui, st.page == p, n).clicked() {
                st.page = p;
            }
        }
        ui.add_space(10.0);
        sep(ui);

        if icon_button(ui, Icon::Home, false, 26.0, "Return to start (Home)").clicked() {
            st.send(Command::Seek(0));
        }
        let playing = st.status.playing;
        if icon_button(ui, if playing { Icon::Stop } else { Icon::Play }, playing, 26.0, "Play / stop (Space)").clicked() {
            st.toggle_play();
        }
        let has_loop = st.loop_range.1 > st.loop_range.0;
        let tip = if has_loop { "Loop the range (Shift+drag in the ruler sets it)" } else { "Shift+drag in the ruler to set a loop range" };
        if icon_button(ui, Icon::Loop, st.loop_on, 26.0, tip).clicked() {
            st.loop_on = !st.loop_on;
            if st.loop_on && !has_loop {
                // No range yet: loop the selected arrangement entry.
                let starts = st.song.arrangement_starts();
                if let Some(e) = st.sel.entry.and_then(|e| starts.get(e)) {
                    st.loop_range = (e.0, e.0 + st.song.section_ticks(&st.song.sections[e.1]));
                }
            }
        }
        if icon_button(ui, Icon::Metronome, st.metronome, 26.0, "Metronome").clicked() {
            st.metronome = !st.metronome;
            st.send(Command::Metronome(st.metronome));
        }
        ui.add_space(4.0);
        egui::ComboBox::from_id_salt("play-mode")
            .width(86.0)
            .selected_text(st.mode.name())
            .show_ui(ui, |ui| {
                for m in PlayMode::ALL {
                    ui.selectable_value(&mut st.mode, m, m.name());
                }
            })
            .response
            .on_hover_text("Song plays the arrangement; Section loops the selected section; Pattern loops the open pattern; Director picks sections by intensity like the game");

        position(ui, st);

        let mut tempo = st.song.tempo;
        if roomy {
        let r = ui
            .add(egui::DragValue::new(&mut tempo).range(20.0..=300.0).speed(0.2).fixed_decimals(1).suffix(" bpm"))
            .on_hover_text("Tempo");
        if r.changed() {
            st.song.tempo = tempo;
        }
        }
        sep(ui);
        intensity(ui, st);
        if wide {
            director_buttons(ui, st);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let n = st.collab.pending().len();
            let label = match (&st.collab.audition, n) {
                (Some(id), _) => format!("Hearing {}", st.collab.entry_label(id)),
                (None, 0) => "Claude".to_string(),
                (None, n) => format!("Claude ({n})"),
            };
            let flash = ui.input(|i| i.time) - st.collab.flash < 3.0 && (ui.input(|i| i.time) * 3.0).fract() < 0.5;
            if widgets::toggle(ui, st.collab.open || flash || st.collab.audition.is_some(), &label, if n > 0 || st.collab.audition.is_some() { WARN } else { ACCENT })
                .on_hover_text("Proposals, history and messages to Claude (/ to type a message)")
                .clicked()
            {
                st.collab.open = !st.collab.open;
            }
            if widgets::toggle(ui, false, "Workbench", ACCENT).on_hover_text("Back to the simple view").clicked() {
                st.set_advanced(false);
            }
            if ui.button("Export").on_hover_text("Render WAV files").clicked() {
                st.export.open = true;
            }
            let (can_undo, can_redo) = (st.history.can_undo(), st.history.can_redo());
            if ui.add_enabled_ui(can_redo, |ui| icon_button(ui, Icon::Redo, false, 24.0, "Redo (Ctrl+Shift+Z, Ctrl+Y)")).inner.clicked() {
                st.redo();
            }
            if ui.add_enabled_ui(can_undo, |ui| icon_button(ui, Icon::Undo, false, 24.0, "Undo (Ctrl+Z)")).inner.clicked() {
                st.undo();
            }
            sep(ui);
            master(ui, st);
        });
    });
}

fn sep(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 26.0), Sense::hover());
    ui.painter()
        .vline(r.center().x, r.y_range(), Stroke::new(1.0, theme::line(20)));
}

/// Bars, beats and ticks of whatever the mode is playing, with the section's name.
fn position(ui: &mut egui::Ui, st: &Studio) {
    let s = &st.status;
    let bpb = st.song.beats_per_bar;
    let (tick, what) = match st.mode {
        PlayMode::Song => (s.song_tick, String::new()),
        PlayMode::Pattern => (
            s.pattern_tick,
            st.sel
                .pattern
                .and_then(|p| st.song.patterns.get(p))
                .map(|p| p.name.clone())
                .unwrap_or_default(),
        ),
        _ => (
            s.section_tick,
            s.section
                .and_then(|i| st.song.sections.get(i))
                .map(|x| x.name.clone())
                .unwrap_or_default(),
        ),
    };
    let what = if what.is_empty() && st.mode == PlayMode::Song {
        s.section
            .and_then(|i| st.song.sections.get(i))
            .map(|x| x.name.clone())
            .unwrap_or_default()
    } else {
        what
    };
    let (rect, _) = ui.allocate_exact_size(vec2(132.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(2),
        BG0,
        Stroke::new(1.0, theme::line(16)),
        StrokeKind::Inside,
    );
    p.text(
        pos2(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        position_text(tick, bpb),
        theme::font_light(22.0),
        if s.playing { ACCENT } else { TEXT },
    );
    let secs = tick as f64 / PPQ as f64 * 60.0 / st.song.tempo.max(1.0) as f64;
    p.text(
        pos2(rect.right() - 6.0, rect.top() + 7.0),
        Align2::RIGHT_CENTER,
        format!("{}:{:04.1}", (secs / 60.0) as u32, secs % 60.0),
        theme::font_body(10.5),
        FAINT,
    );
    p.text(
        pos2(rect.right() - 6.0, rect.bottom() - 7.0),
        Align2::RIGHT_CENTER,
        truncate(&what, 12),
        theme::font_body(10.5),
        DIM,
    );
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('\u{2026}');
        t
    }
}

/// The intensity slider: the handle is the target the director glides to,
/// the fill is where the smoothed value is now.
fn intensity(ui: &mut egui::Ui, st: &mut Studio) {
    let (rect, resp) = ui.allocate_exact_size(vec2(124.0, 26.0), Sense::click_and_drag());
    let (label_rect, _) = ui.allocate_exact_size(vec2(30.0, 26.0), Sense::hover());
    let track = rect.shrink2(vec2(6.0, 9.0));
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            st.intensity = (v * 100.0).round() / 100.0;
            st.ramp = None;
            let force = ui.input(|i| i.modifiers.shift);
            st.send(if force {
                Command::ForceIntensity(st.intensity)
            } else {
                Command::SetIntensity(st.intensity)
            });
        }
    }
    let p = ui.painter();
    p.rect_filled(track, CornerRadius::same(2), BG0);
    let now = st.status.intensity.clamp(0.0, 1.0);
    p.rect_filled(
        Rect::from_min_size(track.min, vec2(track.width() * now, track.height())),
        CornerRadius::same(2),
        theme::mix(theme::ACCENT_DEEP, ACCENT, now),
    );
    for k in 1..4 {
        let x = track.left() + track.width() * k as f32 * 0.25;
        p.vline(
            x,
            track.y_range(),
            Stroke::new(1.0, egui::Color32::from_black_alpha(140)),
        );
    }
    let x = track.left() + track.width() * st.intensity;
    p.rect(
        Rect::from_center_size(pos2(x, track.center().y), vec2(6.0, 18.0)),
        CornerRadius::same(1),
        TEXT,
        Stroke::new(1.0, BG0),
        StrokeKind::Outside,
    );
    p.text(
        label_rect.left_center(),
        Align2::LEFT_CENTER,
        format!("{:.2}", now),
        theme::font_semi(13.0),
        TEXT,
    );
    resp.on_hover_text("Intensity. Drag to set the target (Shift snaps without the glide); the fill is the smoothed value the director hears.");
}

fn director_buttons(ui: &mut egui::Ui, st: &mut Studio) {
    let stingers: Vec<String> = st
        .song
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Stinger)
        .map(|s| s.name.clone())
        .collect();
    let endings: Vec<String> = st
        .song
        .sections
        .iter()
        .filter(|s| s.kind == SectionKind::Ending)
        .map(|s| s.name.clone())
        .collect();
    if stingers.is_empty() && endings.is_empty() {
        return;
    }
    let inline = stingers.len() + endings.len() <= 4;
    if inline {
        for s in &stingers {
            if widgets::toggle(ui, false, s, WARN)
                .on_hover_text(format!("Cue the stinger \"{s}\" over the music"))
                .clicked()
            {
                st.send(Command::Cue(s.clone()));
            }
        }
        for e in &endings {
            if widgets::toggle(ui, false, &format!("End: {e}"), ACCENT)
                .on_hover_text(format!("Finish into \"{e}\""))
                .clicked()
            {
                st.send(Command::Finish(Some(e.clone())));
            }
        }
    } else {
        ui.menu_button("Cue", |ui| {
            for s in &stingers {
                if ui.button(s).clicked() {
                    st.send(Command::Cue(s.clone()));
                }
            }
        });
        ui.menu_button("Finish", |ui| {
            for e in &endings {
                if ui.button(e).clicked() {
                    st.send(Command::Finish(Some(e.clone())));
                }
            }
            if ui.button("Fade out").clicked() {
                st.send(Command::Finish(None));
            }
        });
    }
}

/// The master meter with loudness, engine load and voices under it, in one block.
fn master(ui: &mut egui::Ui, st: &mut Studio) {
    let (rect, resp) = ui.allocate_exact_size(vec2(176.0, 30.0), Sense::hover());
    let meter = Rect::from_min_size(rect.min + vec2(0.0, 2.0), vec2(rect.width(), 11.0));
    widgets::meter_horizontal(ui, meter, egui::Id::new("master-h"), st.meters.master.peak);
    let s = &st.status;
    let p = ui.painter();
    let lufs = st.meters.loudness;
    let y = rect.bottom() - 7.0;
    p.text(
        pos2(rect.left(), y),
        Align2::LEFT_CENTER,
        if lufs < -69.0 {
            "-inf LUFS".to_string()
        } else {
            format!("{lufs:.1} LUFS")
        },
        theme::font_semi(12.5),
        TEXT,
    );
    p.text(
        pos2(rect.right(), y),
        Align2::RIGHT_CENTER,
        format!("CPU {:.0}%   {} voices", s.load * 100.0, s.voices),
        theme::font_body(11.0),
        if s.load > 0.7 { WARN } else { FAINT },
    );
    resp.on_hover_text("Master peak; short-term loudness (400 ms, K-weighted); the share of real time the engine takes; voices sounding");
}
