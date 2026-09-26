//! Listening to a recording: its whole waveform, play and pause, click to
//! move, drag to loop a stretch. When it came through the reference pipeline
//! its transcription's parts show as chips, and the transcription can be
//! opened as a song.

use crate::app::{Screen, Studio};
use crate::audio::RefCmd;
use crate::picker::Row;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT};
use crate::widgets::{self, Icon};
use crate::workbench::{big_toggle, pretty};
use eframe::egui::{self, pos2, vec2, CornerRadius, Rect, Sense, Stroke};
use mc_music::{Song, PPQ};

#[derive(Default)]
pub struct State {
    pub row: Option<Row>,
    transcription: Option<Song>,
    /// Parts of the transcription: (name, from, to) in seconds.
    parts: Vec<(String, f32, f32)>,
    drag_from: Option<f32>,
    pub span: Option<(f32, f32)>,
    /// Where the waveform was drawn last frame (tests).
    pub wave_at: Option<Rect>,
}

pub fn enter(st: &mut Studio, row: Row) {
    st.send(mc_music::Command::Stop);
    st.reference.match_loudness = false;
    crate::reference::load(st, row.path.clone());
    let t = row.transcription.as_ref().and_then(|p| Song::load(p).ok());
    st.player.parts = t
        .as_ref()
        .map(|s| {
            let sec =
                |ticks: u32| (ticks as f64 * 60.0 / (s.tempo.max(1.0) as f64 * PPQ as f64)) as f32;
            s.arrangement_starts()
                .into_iter()
                .map(|(start, i)| {
                    let end = start + s.section_ticks(&s.sections[i]);
                    (s.sections[i].name.clone(), sec(start), sec(end))
                })
                .collect()
        })
        .unwrap_or_default();
    st.player.transcription = t;
    st.player.row = Some(row);
    st.player.span = None;
    st.go(Screen::Audio);
}

pub fn leave(st: &mut Studio) {
    st.audio.send_ref(RefCmd::Play(false));
    st.audio.send_ref(RefCmd::Audible(false));
    st.audio.send_ref(RefCmd::Unload);
    st.audio.send_ref(RefCmd::Span(None));
    st.reference.loaded = None;
    st.reference.audible = false;
    st.reference.match_loudness = true;
    st.player = State::default();
    st.go(Screen::Picker);
}

fn rate(st: &Studio) -> f32 {
    st.reference
        .loaded
        .as_ref()
        .map(|l| l.rate as f32)
        .unwrap_or(48000.0)
}

pub fn toggle_play(st: &mut Studio) {
    if st.reference.loaded.is_none() {
        return;
    }
    let play = !st.reference.playing;
    st.audio.send_ref(RefCmd::Audible(true));
    st.reference.audible = true;
    st.audio.send_ref(RefCmd::Play(play));
    st.reference.playing = play;
}

fn set_span(st: &mut Studio, span: Option<(f32, f32)>) {
    st.player.span = span;
    let r = rate(st);
    st.audio.send_ref(RefCmd::Span(
        span.map(|(a, b)| ((a * r) as usize, (b * r) as usize))
            .filter(|(a, b)| b > a),
    ));
    if let Some((a, _)) = span {
        st.audio.send_ref(RefCmd::Seek(a as f64 * r as f64));
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(row) = st.player.row.clone() else {
        st.go(Screen::Picker);
        return;
    };
    let mut back = false;
    egui::Panel::top("player-top").frame(egui::Frame::new().fill(BG0).inner_margin(egui::Margin::symmetric(16, 12))).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if big_toggle(ui, false, "Library", DIM).on_hover_text("Back to the list of songs and audio").clicked() {
                back = true;
            }
            let playing = st.reference.playing;
            let (r, resp) = ui.allocate_exact_size(vec2(52.0, 44.0), Sense::click());
            let a = ui.ctx().animate_bool_with_time(resp.id, playing, 0.15);
            let p = ui.painter();
            p.rect_filled(r, CornerRadius::same(6), theme::mix(BG2, ACCENT, a));
            widgets::draw_icon(p, r.shrink(4.0), if playing { Icon::Stop } else { Icon::Play }, theme::mix(TEXT, BG0, a));
            if resp.on_hover_text("Play / pause (Space)").clicked() {
                toggle_play(st);
            }
            ui.label(egui::RichText::new(pretty(&row.name)).font(theme::font_light(26.0)).color(TEXT));
            let secs = st.reference.seconds();
            let at = st.reference.pos as f32 / rate(st);
            ui.label(egui::RichText::new(format!("{}  /  {}", clock(at), clock(secs))).size(16.0).color(DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(t) = row.transcription.clone() {
                    if big_toggle(ui, false, "Open the transcription as a song", ACCENT)
                        .on_hover_text("The notes worked out from this recording, as a song you can play and edit")
                        .clicked()
                    {
                        leave(st);
                        st.open(t);
                        st.go(Screen::Song);
                    }
                }
            });
        });
    });
    if back {
        leave(st);
        return;
    }
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG1)
                .inner_margin(egui::Margin::symmetric(16, 16)),
        )
        .show(ui, |ui| {
            if st.reference.loaded.is_none() {
                ui.label(egui::RichText::new("Loading...").size(18.0).color(DIM));
                return;
            }
            let secs = st.reference.seconds().max(0.01);
            // Parts as chips.
            if !st.player.parts.is_empty() {
                let mut picked = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for (i, (name, a, b)) in st.player.parts.iter().enumerate() {
                        let on = st.player.span == Some((*a, *b));
                        if big_toggle(ui, on, &pretty(name), ACCENT)
                            .on_hover_text(format!("Loop {} to {}", clock(*a), clock(*b)))
                            .clicked()
                        {
                            picked = Some((i, on));
                        }
                    }
                });
                if let Some((i, on)) = picked {
                    let (_, a, b) = st.player.parts[i].clone();
                    set_span(st, if on { None } else { Some((a, b)) });
                    if !st.reference.playing {
                        toggle_play(st);
                    }
                }
                ui.add_space(10.0);
            }
            let w = ui.available_width();
            let h = (ui.available_height() - 40.0).clamp(160.0, 420.0);
            let (wr, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
            st.player.wave_at = Some(wr);
            let p = ui.painter();
            p.rect_filled(wr, CornerRadius::same(8), BG0);
            let x_of = |s: f32| wr.left() + wr.width() * (s / secs);
            let s_of = |x: f32| ((x - wr.left()) / wr.width()).clamp(0.0, 1.0) * secs;
            for (i, (name, a, _)) in st.player.parts.iter().enumerate() {
                if i > 0 {
                    p.vline(x_of(*a), wr.y_range(), Stroke::new(1.0, theme::line(26)));
                }
                p.text(
                    pos2(x_of(*a) + 6.0, wr.top() + 6.0),
                    egui::Align2::LEFT_TOP,
                    pretty(name),
                    theme::font_semi(13.0),
                    FAINT,
                );
            }
            if let Some((a, b)) = st.player.span {
                p.rect_filled(
                    Rect::from_min_max(pos2(x_of(a), wr.top()), pos2(x_of(b), wr.bottom())),
                    CornerRadius::ZERO,
                    theme::with_alpha(ACCENT, 34),
                );
            }
            let at = st.reference.pos as f32 / rate(st);
            if let Some(l) = &st.reference.loaded {
                let n = l.overview.len().max(1);
                let c = wr.center().y;
                for (i, (lo, hi)) in l.overview.iter().enumerate() {
                    let x = wr.left() + wr.width() * i as f32 / n as f32;
                    let played = x < x_of(at);
                    let col = if played {
                        theme::with_alpha(ACCENT, 200)
                    } else {
                        theme::with_alpha(TEXT, 110)
                    };
                    p.vline(
                        x,
                        (c - hi * wr.height() * 0.46)..=(c - lo * wr.height() * 0.46),
                        Stroke::new(1.0, col),
                    );
                }
            }
            p.vline(x_of(at), wr.y_range(), Stroke::new(2.0, ACCENT));
            if resp.drag_started() {
                st.player.drag_from = ui.input(|i| i.pointer.press_origin()).map(|p| s_of(p.x));
            }
            if resp.dragged() {
                if let (Some(a), Some(pos)) = (st.player.drag_from, resp.interact_pointer_pos()) {
                    let b = s_of(pos.x);
                    st.player.span = Some((a.min(b), a.max(b)));
                }
            }
            if resp.drag_stopped() {
                st.player.drag_from = None;
                let span = st.player.span;
                set_span(st, span);
            }
            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let r = rate(st);
                    st.audio
                        .send_ref(RefCmd::Seek(s_of(pos.x) as f64 * r as f64));
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let text = match st.player.span {
                    Some((a, b)) => format!("Looping {} to {}", clock(a), clock(b)),
                    None => "Click to move the playhead. Drag across to loop a stretch.".into(),
                };
                ui.label(egui::RichText::new(text).size(15.0).color(FAINT));
                if st.player.span.is_some() && ui.button("Stop looping").clicked() {
                    set_span(st, None);
                }
            });
        });
}

pub fn clock(s: f32) -> String {
    let s = s.max(0.0);
    format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32)
}
