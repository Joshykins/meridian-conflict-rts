//! The start screen: pick a song, a moment or a piece of audio to work on.
//! Nothing opens by itself; `studio.sh <song>` still goes straight to a song.

use crate::app::{Screen, Studio};
use crate::theme::{self, ACCENT, BG2, BG3, DIM, FAINT, TEXT};
use crate::workbench::pretty;
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub seconds: Option<f32>,
    pub changed: Option<SystemTime>,
    /// Audio from the reference pipeline: its transcription as a song.
    pub transcription: Option<PathBuf>,
}

#[derive(Default)]
pub struct State {
    pub songs: Vec<Row>,
    pub moments: Vec<Row>,
    pub audio: Vec<Row>,
    at: Option<f64>,
    /// Where each row was drawn last frame (tests).
    pub row_at: HashMap<String, Rect>,
}

fn song_row(path: PathBuf) -> Row {
    let song = mc_music::Song::load(&path).ok();
    let seconds = song
        .as_ref()
        .map(|s| (s.arrangement_ticks() as f64 * s.samples_per_tick(48000.0) / 48000.0) as f32);
    let name = song
        .as_ref()
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    Row {
        changed: crate::files::mtime(&path),
        path,
        name,
        seconds,
        transcription: None,
    }
}

fn is_audio(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
        matches!(
            x.to_ascii_lowercase().as_str(),
            "wav" | "mp3" | "flac" | "ogg"
        )
    })
}

/// A WAV file's length from its header; other formats are not measured here.
fn wav_seconds(p: &Path) -> Option<f32> {
    use std::io::Read;
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = [0u8; 44];
    f.read_exact(&mut h).ok()?;
    if &h[0..4] != b"RIFF" {
        return None;
    }
    let rate = u32::from_le_bytes([h[24], h[25], h[26], h[27]]) as f32;
    let bytes_per_sec = u32::from_le_bytes([h[28], h[29], h[30], h[31]]) as f32;
    let len = std::fs::metadata(p).ok()?.len() as f32;
    (rate > 0.0 && bytes_per_sec > 0.0).then(|| (len - 44.0) / bytes_per_sec)
}

/// Audio in `references/`: loose files, and pipeline folders (their `source.wav`
/// and, when there is one, the `reference.ron` transcription).
fn audio_rows(dir: &Path) -> Vec<Row> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            let source = p.join("source.wav");
            if source.is_file() {
                let t = p.join("reference.ron");
                out.push(Row {
                    name: p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    seconds: wav_seconds(&source),
                    changed: crate::files::mtime(&source),
                    transcription: t.is_file().then_some(t),
                    path: source,
                });
            }
        } else if is_audio(&p) {
            out.push(Row {
                name: p
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                seconds: wav_seconds(&p),
                changed: crate::files::mtime(&p),
                transcription: None,
                path: p,
            });
        }
    }
    out
}

pub fn scan(st: &mut Studio, now: f64) {
    if st.picker.at.is_some_and(|t| now - t < 3.0) {
        return;
    }
    st.picker.at = Some(now);
    let Some(dir) = st.music_dir.clone() else {
        return;
    };
    st.picker.songs = mc_music::score::song_paths(&dir)
        .into_iter()
        .map(song_row)
        .collect();
    let mut moments: Vec<PathBuf> = std::fs::read_dir(dir.join("moments"))
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "ron"))
                .collect()
        })
        .unwrap_or_default();
    moments.sort();
    st.picker.moments = moments.into_iter().map(song_row).collect();
    st.picker.audio = audio_rows(&dir.join("references"));
}

fn length(s: Option<f32>) -> String {
    match s {
        Some(s) => format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32),
        None => String::new(),
    }
}

fn changed(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return String::new() };
    let d = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match d {
        0..=59 => "changed just now".into(),
        60..=3599 => format!("changed {} min ago", d / 60),
        3600..=86399 => format!("changed {} h ago", d / 3600),
        _ => format!("changed {} days ago", d / 86400),
    }
}

enum Pick {
    Song(PathBuf),
    Audio(Row),
    New,
    Sounds,
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    scan(st, now);
    st.picker.row_at.clear();
    let mut pick = None;
    let (songs, moments, audio) = (
        st.picker.songs.clone(),
        st.picker.moments.clone(),
        st.picker.audio.clone(),
    );
    egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG1)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let w = ui.available_width().min(880.0);
            let pad = ((ui.available_width() - w) * 0.5).max(16.0);
            ui.add_space(48.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.label(egui::RichText::new("Music").font(theme::font_light(40.0)).color(TEXT));
                    ui.label(egui::RichText::new("Pick something to work on.").size(16.0).color(DIM));
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        heading(ui, "Songs");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if crate::workbench::big_toggle(ui, false, "New song", ACCENT).clicked() {
                                pick = Some(Pick::New);
                            }
                        });
                    });
                    list(ui, st, &songs, "No songs yet.", &mut pick, false);
                    ui.add_space(20.0);
                    heading(ui, "Moments");
                    ui.label(egui::RichText::new("Short pieces the game plays over a song when something happens.").size(14.0).color(FAINT));
                    ui.add_space(4.0);
                    list(ui, st, &moments, "No moments yet.", &mut pick, false);
                    ui.add_space(20.0);
                    heading(ui, "Audio");
                    ui.label(egui::RichText::new("Recordings to listen to and learn from.").size(14.0).color(FAINT));
                    ui.add_space(4.0);
                    list(ui, st, &audio, "Put audio in data/music/references to see it here.", &mut pick, true);
                    ui.add_space(20.0);
                    heading(ui, "Game sounds");
                    ui.label(egui::RichText::new("Every sound the game makes: play them, take them apart, see what uses them.").size(14.0).color(FAINT));
                    ui.add_space(4.0);
                    let count = st.sounds.count(st.music_dir.as_ref(), now);
                    if sounds_row(ui, count) {
                        pick = Some(Pick::Sounds);
                    }
                    ui.add_space(40.0);
                });
            });
        });
    });
    match pick {
        Some(Pick::Song(p)) => {
            st.open(p);
            st.go(Screen::Song);
        }
        Some(Pick::Audio(r)) => crate::player::enter(st, r),
        Some(Pick::Sounds) => crate::sounds::enter(st),
        Some(Pick::New) => {
            st.new_song();
            st.go(Screen::Song);
        }
        None => {}
    }
}

/// The one row that opens the Sounds screen.
fn sounds_row(ui: &mut egui::Ui, count: usize) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(8),
        theme::mix(BG2, BG3, hover),
        Stroke::new(1.0, theme::mix(theme::line(10), ACCENT, hover * 0.6)),
        StrokeKind::Inside,
    );
    p.text(
        pos2(rect.left() + 18.0 + hover * 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        "All game sounds",
        theme::font_semi(17.0),
        TEXT,
    );
    p.text(
        pos2(rect.right() - 18.0, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{count} sounds"),
        theme::font_body(14.0),
        FAINT,
    );
    resp.clicked()
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::font_semi(20.0))
            .color(TEXT),
    );
}

fn list(
    ui: &mut egui::Ui,
    st: &mut Studio,
    rows: &[Row],
    empty: &str,
    pick: &mut Option<Pick>,
    audio: bool,
) {
    if rows.is_empty() {
        ui.label(egui::RichText::new(empty).size(15.0).color(FAINT));
        return;
    }
    for r in rows {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
        st.picker.row_at.insert(r.name.clone(), rect);
        let hover = ui
            .ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
        let p = ui.painter();
        p.rect(
            rect,
            CornerRadius::same(8),
            theme::mix(BG2, BG3, hover),
            Stroke::new(1.0, theme::mix(theme::line(10), ACCENT, hover * 0.6)),
            StrokeKind::Inside,
        );
        p.text(
            pos2(rect.left() + 18.0 + hover * 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            pretty(&r.name),
            theme::font_semi(17.0),
            TEXT,
        );
        let mut details = vec![length(r.seconds)];
        if r.transcription.is_some() {
            details.push("with a transcription".into());
        }
        details.push(changed(r.changed));
        details.retain(|d| !d.is_empty());
        p.text(
            pos2(rect.right() - 18.0, rect.center().y),
            Align2::RIGHT_CENTER,
            details.join("   "),
            theme::font_body(14.0),
            FAINT,
        );
        ui.add_space(6.0);
        if resp.clicked() {
            *pick = Some(if audio {
                Pick::Audio(r.clone())
            } else {
                Pick::Song(r.path.clone())
            });
        }
    }
}
