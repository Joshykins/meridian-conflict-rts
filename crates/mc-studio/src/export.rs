//! Rendering to WAV on a background thread: the arrangement, one stem per
//! track (the song with only that track soloed), or the director at a fixed
//! intensity or a ramp. Each file is measured with `render::analyse` so
//! loudness and balance can be checked by number as well as by ear.

use crate::app::Studio;
use crate::files;
use crate::theme::{self, ACCENT, DIM, FAINT, TEXT};
use eframe::egui;
use mc_music::render::{analyse, write_wav, Analysis};
use mc_music::{Engine, Mode, Song};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

impl State {
    pub fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.done)
    }
}

pub struct State {
    pub open: bool,
    folder: String,
    tail: f32,
    intensity: (f32, f32),
    seconds: f32,
    job: Option<Job>,
    results: Vec<(PathBuf, Analysis)>,
    error: Option<String>,
}

impl Default for State {
    fn default() -> State {
        State {
            open: false,
            folder: String::new(),
            tail: 4.0,
            intensity: (0.2, 1.0),
            seconds: 90.0,
            job: None,
            results: Vec::new(),
            error: None,
        }
    }
}

struct Job {
    what: String,
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<(PathBuf, Analysis), String>>,
    done: bool,
}

const RATE: u32 = 48000;

/// Renders `seconds` of `song` in `mode`, the intensity moving linearly from
/// `intensity.0` to `.1`, reporting progress in thousandths from `from` to `to`.
fn render(
    song: &Song,
    seconds: f32,
    mode: Mode,
    intensity: (f32, f32),
    progress: &AtomicU32,
    cancel: &AtomicBool,
    span: (u32, u32),
) -> Option<Vec<[f32; 2]>> {
    let mut e = Engine::new(RATE as f32, Arc::new(song.clone()));
    e.command(mc_music::Command::ForceIntensity(intensity.0));
    e.set_mode(mode);
    e.play();
    let total = (seconds.max(0.1) * RATE as f32) as usize;
    let mut out = vec![[0.0f32; 2]; total];
    let mut at = 0;
    while at < total {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let n = 4096.min(total - at);
        let k = at as f32 / total as f32;
        e.set_intensity(intensity.0 + (intensity.1 - intensity.0) * k);
        e.render_frames(&mut out[at..at + n]);
        at += n;
        progress.store(
            span.0 + ((span.1 - span.0) as f32 * at as f32 / total as f32) as u32,
            Ordering::Relaxed,
        );
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_asked_length_and_reports_progress() {
        let song = crate::songops::starter_song("t");
        let (p, c) = (AtomicU32::new(0), AtomicBool::new(false));
        let f = render(&song, 0.5, Mode::Song, (1.0, 1.0), &p, &c, (0, 1000)).unwrap();
        assert_eq!(f.len(), RATE as usize / 2);
        assert_eq!(p.load(Ordering::Relaxed), 1000);
        assert!(
            f.iter().any(|x| x[0].abs() > 1e-4),
            "the starter song makes sound"
        );
        c.store(true, Ordering::Relaxed);
        assert!(render(&song, 0.5, Mode::Song, (1.0, 1.0), &p, &c, (0, 1000)).is_none());
        assert!(
            (arrangement_seconds(&song, 0.0) - 8.0).abs() < 0.01,
            "4 bars at 120 bpm"
        );
    }
}

fn arrangement_seconds(song: &Song, tail: f32) -> f32 {
    (song.arrangement_ticks() as f64 * song.samples_per_tick(RATE as f32) / RATE as f64) as f32
        + tail
}

enum Kind {
    Mix,
    Stems,
    Director,
}

fn start(st: &mut Studio, kind: Kind) {
    let folder = PathBuf::from(st.export.folder.trim());
    if let Err(e) = std::fs::create_dir_all(&folder) {
        st.export.error = Some(format!("{}: {e}", folder.display()));
        return;
    }
    let song = st.song.clone();
    let stem = st
        .collab
        .stem
        .clone()
        .unwrap_or_else(|| files::file_stem(&song.name));
    let tail = st.export.tail;
    let (intensity, seconds) = (st.export.intensity, st.export.seconds);
    let progress = Arc::new(AtomicU32::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();
    let what = match kind {
        Kind::Mix => "Rendering the arrangement".to_string(),
        Kind::Stems => "Rendering stems".to_string(),
        Kind::Director => format!(
            "Rendering the director at {:.2} to {:.2}",
            intensity.0, intensity.1
        ),
    };
    let (p, c) = (progress.clone(), cancel.clone());
    std::thread::spawn(move || {
        let write = |path: PathBuf, frames: Vec<[f32; 2]>| -> Result<(PathBuf, Analysis), String> {
            write_wav(&path, &frames, RATE).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok((path, analyse(&frames, RATE)))
        };
        match kind {
            Kind::Mix => {
                let secs = arrangement_seconds(&song, tail);
                if let Some(f) = render(&song, secs, Mode::Song, (1.0, 1.0), &p, &c, (0, 1000)) {
                    let _ = tx.send(write(folder.join(format!("{stem}.wav")), f));
                }
            }
            Kind::Stems => {
                let tracks: Vec<usize> = (0..song.tracks.len())
                    .filter(|&i| !song.tracks[i].mute)
                    .collect();
                let secs = arrangement_seconds(&song, tail);
                let n = tracks.len().max(1) as u32;
                for (k, &i) in tracks.iter().enumerate() {
                    let mut solo = song.clone();
                    for (j, t) in solo.tracks.iter_mut().enumerate() {
                        t.solo = j == i;
                    }
                    let span = (k as u32 * 1000 / n, (k as u32 + 1) * 1000 / n);
                    let Some(f) = render(&solo, secs, Mode::Song, (1.0, 1.0), &p, &c, span) else {
                        return;
                    };
                    let name = files::file_stem(&song.tracks[i].name);
                    let _ = tx.send(write(folder.join(format!("{stem}_{name}.wav")), f));
                }
            }
            Kind::Director => {
                if let Some(f) =
                    render(&song, seconds, Mode::Director, intensity, &p, &c, (0, 1000))
                {
                    let name = format!(
                        "{stem}_director_{:.0}-{:.0}.wav",
                        intensity.0 * 100.0,
                        intensity.1 * 100.0
                    );
                    let _ = tx.send(write(folder.join(name), f));
                }
            }
        }
        p.store(1000, Ordering::Relaxed);
    });
    st.export.results.clear();
    st.export.error = None;
    st.export.job = Some(Job {
        what,
        progress,
        cancel,
        rx,
        done: false,
    });
}

fn default_folder(st: &Studio) -> String {
    let stem = st
        .collab
        .stem
        .clone()
        .unwrap_or_else(|| files::file_stem(&st.song.name));
    let base = st
        .music_dir
        .as_deref()
        .and_then(|m| m.parent())
        .and_then(|d| d.parent())
        .map(|root| root.join("target").join("studio-export"))
        .unwrap_or_else(|| std::env::temp_dir().join("mc-studio-export"));
    base.join(stem).to_string_lossy().into_owned()
}

pub fn window(ctx: &egui::Context, st: &mut Studio) {
    // Collect finished work even when the window is closed.
    if let Some(job) = &mut st.export.job {
        while let Ok(r) = job.rx.try_recv() {
            match r {
                Ok(x) => st.export.results.push(x),
                Err(e) => st.export.error = Some(e),
            }
        }
        if job.progress.load(Ordering::Relaxed) >= 1000 || job.cancel.load(Ordering::Relaxed) {
            job.done = true;
        }
    }
    if !st.export.open {
        return;
    }
    if st.export.folder.is_empty() {
        st.export.folder = default_folder(st);
    }
    let mut open = st.export.open;
    egui::Window::new(egui::RichText::new("Export").font(theme::font_light(22.0)))
        .open(&mut open)
        .default_width(560.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Folder").color(FAINT));
                ui.add(egui::TextEdit::singleline(&mut st.export.folder).desired_width(380.0));
                if ui.small_button("Open").clicked() {
                    let _ = std::fs::create_dir_all(&st.export.folder);
                    files::reveal(Path::new(&st.export.folder));
                }
            });
            ui.add_space(6.0);
            let busy = st.export.job.as_ref().is_some_and(|j| !j.done);
            ui.add_enabled_ui(!busy, |ui| {
                crate::widgets::group(ui, "Arrangement", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Tail").color(FAINT));
                        ui.add(
                            egui::DragValue::new(&mut st.export.tail)
                                .range(0.0..=20.0)
                                .suffix(" s"),
                        );
                        let secs = arrangement_seconds(&st.song, st.export.tail);
                        ui.label(
                            egui::RichText::new(format!("{:.0} s at full intensity", secs))
                                .color(DIM),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Render the mix").clicked() {
                            start(st, Kind::Mix);
                        }
                        let n = st.song.tracks.iter().filter(|t| !t.mute).count();
                        if ui
                            .button(format!("Render {n} stems"))
                            .on_hover_text(
                                "Each unmuted track alone, through its own effects and sends",
                            )
                            .clicked()
                        {
                            start(st, Kind::Stems);
                        }
                    });
                });
                ui.add_space(6.0);
                crate::widgets::group(ui, "Director", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Intensity from").color(FAINT));
                        ui.add(
                            egui::DragValue::new(&mut st.export.intensity.0)
                                .range(0.0..=1.0)
                                .speed(0.01)
                                .fixed_decimals(2),
                        );
                        ui.label(egui::RichText::new("to").color(FAINT));
                        ui.add(
                            egui::DragValue::new(&mut st.export.intensity.1)
                                .range(0.0..=1.0)
                                .speed(0.01)
                                .fixed_decimals(2),
                        );
                        ui.label(egui::RichText::new("over").color(FAINT));
                        ui.add(
                            egui::DragValue::new(&mut st.export.seconds)
                                .range(5.0..=900.0)
                                .suffix(" s"),
                        );
                    });
                    if ui
                        .button("Render the director")
                        .on_hover_text("What the game plays as the battle moves through that range")
                        .clicked()
                    {
                        start(st, Kind::Director);
                    }
                });
            });
            if let Some(job) = &st.export.job {
                ui.add_space(8.0);
                let f = job.progress.load(Ordering::Relaxed) as f32 / 1000.0;
                ui.label(
                    egui::RichText::new(if job.done {
                        "Done".to_string()
                    } else {
                        job.what.clone()
                    })
                    .color(TEXT),
                );
                ui.add(egui::ProgressBar::new(f).fill(ACCENT).desired_height(8.0));
                if !job.done && ui.small_button("Cancel").clicked() {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            if let Some(e) = &st.export.error {
                ui.label(egui::RichText::new(e).color(theme::BAD));
            }
            if !st.export.results.is_empty() {
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (path, a) in &st.export.results {
                            ui.label(
                                egui::RichText::new(
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .font(theme::font_semi(13.0))
                                .color(TEXT),
                            );
                            ui.label(
                                egui::RichText::new(format!("{a}"))
                                    .monospace()
                                    .color(DIM)
                                    .size(11.5),
                            );
                            ui.add_space(4.0);
                        }
                    });
            }
        });
    st.export.open = open;
}
