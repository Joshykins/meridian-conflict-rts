//! The left column: the song files in `data/music`, the open song's
//! settings, every section and pattern (including the ones the arrangement
//! does not use: stingers, endings, bridges), and the song's notes.

use crate::app::{Detail, Studio, Tone};
use crate::songops;
use crate::theme::{self, ACCENT, DIM, FAINT, TEXT};
use crate::widgets::caption;
use eframe::egui::{self, vec2, CornerRadius, Sense, Stroke};
use mc_music::song::{Scale, NOTE_NAMES};
use std::path::PathBuf;

#[derive(Default)]
pub struct State {
    songs: Vec<PathBuf>,
    last_scan: f64,
    pub rescan: bool,
    pub midi_seen: u32,
    pub midi_flash: f64,
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    if st.browser.rescan
        || now - st.browser.last_scan > 2.0
        || st.browser.songs.is_empty() && st.browser.last_scan == 0.0
    {
        st.browser.rescan = false;
        st.browser.last_scan = now;
        st.browser.songs = st
            .music_dir
            .as_deref()
            .map(mc_music::score::song_paths)
            .unwrap_or_default();
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                caption(ui, "Songs");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Save as").clicked() {
                        st.dialog = Some(crate::app::Dialog::SaveAs {
                            name: st.song.name.clone(),
                        });
                    }
                    if ui.small_button("Save").on_hover_text("Ctrl+S").clicked() {
                        st.save();
                    }
                    if ui.small_button("New").clicked() {
                        st.request_open(None);
                    }
                });
            });
            if st.music_dir.is_none() {
                ui.label(
                    egui::RichText::new("No data/music folder found")
                        .color(FAINT)
                        .small(),
                );
            }
            let songs = st.browser.songs.clone();
            for p in songs {
                let open = st.path.as_ref().is_some_and(|o| same_file(o, &p));
                let name = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let label = if open && st.dirty {
                    format!("{name} *")
                } else {
                    name
                };
                if row(ui, open, &label, None).clicked() && !open {
                    st.request_open(Some(p.clone()));
                }
            }
            ui.add_space(10.0);
            song_settings(ui, st);
            ui.add_space(10.0);
            sections(ui, st);
            ui.add_space(10.0);
            patterns(ui, st);
            ui.add_space(10.0);
            caption(ui, "Notes");
            ui.add(
                egui::TextEdit::multiline(&mut st.song.notes)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text("Notes about the song for whoever edits it next"),
            );
        });
}

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// A selectable list row with an optional colour chip.
fn row(
    ui: &mut egui::Ui,
    selected: bool,
    text: &str,
    chip: Option<egui::Color32>,
) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(2), theme::with_alpha(ACCENT, 38));
        p.vline(rect.left() + 1.0, rect.y_range(), Stroke::new(2.0, ACCENT));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(2), theme::line(8));
    }
    let mut x = rect.left() + 8.0;
    if let Some(c) = chip {
        p.rect_filled(
            egui::Rect::from_center_size(egui::pos2(x + 3.0, rect.center().y), vec2(6.0, 10.0)),
            CornerRadius::same(1),
            c,
        );
        x += 12.0;
    }
    p.with_clip_rect(rect.shrink2(egui::vec2(2.0, 0.0))).text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::font_body(13.0),
        if selected { TEXT } else { DIM },
    );
    resp
}

fn song_settings(ui: &mut egui::Ui, st: &mut Studio) {
    caption(ui, "Song");
    egui::Grid::new("song-settings")
        .num_columns(2)
        .spacing(vec2(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Name").color(FAINT));
            ui.add(egui::TextEdit::singleline(&mut st.song.name).desired_width(140.0));
            ui.end_row();
            ui.label(egui::RichText::new("Tempo").color(FAINT));
            ui.add(
                egui::DragValue::new(&mut st.song.tempo)
                    .range(20.0..=300.0)
                    .speed(0.2)
                    .fixed_decimals(1)
                    .suffix(" bpm"),
            );
            ui.end_row();
            ui.label(egui::RichText::new("Beats per bar").color(FAINT));
            ui.add(egui::DragValue::new(&mut st.song.beats_per_bar).range(1..=16));
            ui.end_row();
            ui.label(egui::RichText::new("Key").color(FAINT));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("root")
                    .width(44.0)
                    .selected_text(NOTE_NAMES[st.song.root as usize % 12])
                    .show_ui(ui, |ui| {
                        for (i, n) in NOTE_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut st.song.root, i as u8, *n);
                        }
                    });
                egui::ComboBox::from_id_salt("scale")
                    .width(90.0)
                    .selected_text(st.song.scale.name())
                    .show_ui(ui, |ui| {
                        for s in Scale::ALL {
                            ui.selectable_value(&mut st.song.scale, s, s.name());
                        }
                    });
            });
            ui.end_row();
        });
}

fn sections(ui: &mut egui::Ui, st: &mut Studio) {
    ui.horizontal(|ui| {
        caption(ui, "Sections");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Add").clicked() {
                let bars = st
                    .sel
                    .section
                    .and_then(|s| st.song.sections.get(s))
                    .map(|s| s.bars)
                    .unwrap_or(4);
                let i = songops::new_section(&mut st.song, "Section", bars);
                st.sel.section = Some(i);
                st.sel.entry = None;
            }
        });
    });
    for i in 0..st.song.sections.len() {
        let s = &st.song.sections[i];
        let used = st.song.arrangement.contains(&s.name);
        let text = format!(
            "{}  {} bars{}",
            s.name,
            s.bars,
            if used { "" } else { "  (not arranged)" }
        );
        let chip = theme::section_colour(s.kind);
        let r = row(ui, st.sel.section == Some(i), &text, Some(chip)).on_hover_text(s.kind.name());
        if r.clicked() {
            st.sel.section = Some(i);
            st.sel.entry = st
                .song
                .arrangement
                .iter()
                .position(|a| *a == st.song.sections[i].name);
        }
        let name = st.song.sections[i].name.clone();
        r.context_menu(|ui| {
            if ui.button("Add to the arrangement").clicked() {
                st.song.arrangement.push(name.clone());
                ui.close();
            }
            if ui.button("Duplicate").clicked() {
                let d = songops::duplicate_section(&mut st.song, i);
                st.sel.section = Some(d);
                ui.close();
            }
            if ui.button("Delete").clicked() {
                songops::delete_section(&mut st.song, i);
                st.say(format!("Deleted section {name}"), Tone::Info);
                ui.close();
            }
        });
    }
}

fn patterns(ui: &mut egui::Ui, st: &mut Studio) {
    ui.horizontal(|ui| {
        caption(ui, "Patterns");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Add").clicked() {
                let i = songops::new_pattern(&mut st.song, "Pattern", 4);
                st.sel.pattern = Some(i);
                st.detail = if st.is_kit(st.sel.pattern_track) {
                    Detail::Drums
                } else {
                    Detail::Piano
                };
            }
        });
    });
    for i in 0..st.song.patterns.len() {
        let p = &st.song.patterns[i];
        let uses = st
            .song
            .sections
            .iter()
            .flat_map(|s| &s.clips)
            .filter(|c| c.pattern == p.name)
            .count();
        // Colour the chip by the first track that plays it.
        let chip = st
            .song
            .sections
            .iter()
            .flat_map(|s| &s.clips)
            .find(|c| c.pattern == p.name)
            .and_then(|c| st.song.track(&c.track))
            .map(|t| theme::rgb(st.song.tracks[t].colour))
            .unwrap_or(FAINT);
        let text = format!(
            "{}  {} beats  {} notes{}",
            p.name,
            p.beats,
            p.notes.len(),
            if uses == 0 { "  (unused)" } else { "" }
        );
        let r = row(ui, st.sel.pattern == Some(i), &text, Some(chip));
        if r.clicked() {
            st.sel.pattern = Some(i);
            let name = st.song.patterns[i].name.clone();
            if let Some(t) = st
                .song
                .sections
                .iter()
                .flat_map(|s| &s.clips)
                .find(|c| c.pattern == name)
                .and_then(|c| st.song.track(&c.track))
            {
                st.sel.pattern_track = t;
            }
            st.detail = if st.is_kit(st.sel.pattern_track) {
                crate::app::Detail::Drums
            } else {
                crate::app::Detail::Piano
            };
            st.detail_open = true;
        }
        r.context_menu(|ui| {
            if ui.button("Duplicate").clicked() {
                let d = songops::duplicate_pattern(&mut st.song, i);
                st.sel.pattern = Some(d);
                ui.close();
            }
            if ui.button("Delete (and its clips)").clicked() {
                songops::delete_pattern(&mut st.song, i);
                ui.close();
            }
        });
    }
    let _ = ACCENT;
}
