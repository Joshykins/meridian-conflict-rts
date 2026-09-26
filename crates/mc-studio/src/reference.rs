//! Reference recordings (`data/music/references/`): play a track the song is
//! meant to feel like through the same output, matched in loudness so the
//! louder one does not win by being louder, flip between it and the song with
//! one key while both keep their place, compare their spectra, and mark a span
//! of it that "this part" in a chat message then refers to.
//!
//! Files decode through `mc-listen` (WAV, MP3, FLAC, Ogg) when the studio is
//! built with it, and through a small WAV reader otherwise.

use crate::app::{Studio, Tone};
use crate::audio::RefCmd;
use crate::fft;
use crate::instrument::draw_spectrum;
use crate::theme::{self, ACCENT, BG0, BG2, DIM, FAINT, TEXT, WARN};
use crate::widgets;
use eframe::egui::{self, pos2, vec2, Align2, CornerRadius, Rect, Sense, Stroke};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

pub struct Reference {
    files: Vec<PathBuf>,
    files_at: f64,
    pub loaded: Option<Loaded>,
    loading: Option<(PathBuf, Receiver<Result<Loaded, String>>)>,
    /// Marked span, seconds.
    pub span: Option<(f32, f32)>,
    pub loop_span: bool,
    pub audible: bool,
    pub playing: bool,
    pub pos: f64,
    pub match_loudness: bool,
    /// The song's loudness, averaged while it plays.
    song_lufs: f32,
    drag_from: Option<f32>,
    ours: Vec<f32>,
    theirs: Vec<f32>,
    sent_gain: f32,
}

impl Default for Reference {
    fn default() -> Reference {
        Reference {
            files: Vec::new(),
            files_at: -10.0,
            loaded: None,
            loading: None,
            span: None,
            loop_span: true,
            audible: false,
            playing: false,
            pos: 0.0,
            match_loudness: true,
            song_lufs: -18.0,
            drag_from: None,
            ours: Vec::new(),
            theirs: Vec::new(),
            sent_gain: -1.0,
        }
    }
}

pub struct Loaded {
    pub path: PathBuf,
    /// At the output rate.
    pub frames: Arc<Vec<[f32; 2]>>,
    pub rate: u32,
    pub lufs: f32,
    pub peak_db: f32,
    /// Min and max per column of the overview.
    pub overview: Vec<(f32, f32)>,
}

impl Reference {
    pub fn current_name(&self) -> Option<String> {
        self.loaded.as_ref().map(|l| {
            l.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    pub fn seconds(&self) -> f32 {
        self.loaded
            .as_ref()
            .map(|l| l.frames.len() as f32 / l.rate as f32)
            .unwrap_or(0.0)
    }

    /// The gain that brings the reference to the song's loudness, dB.
    pub fn offset_db(&self) -> f32 {
        match &self.loaded {
            Some(l) if self.match_loudness => (self.song_lufs - l.lufs).clamp(-30.0, 30.0),
            _ => 0.0,
        }
    }
}

fn list(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && depth < 3 {
            list(&p, out, depth + 1);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
            matches!(
                x.to_ascii_lowercase().as_str(),
                "wav" | "mp3" | "flac" | "ogg"
            )
        }) {
            out.push(p);
        }
    }
}

#[cfg(feature = "listen")]
fn decode(path: &Path) -> Result<(u32, Vec<[f32; 2]>), String> {
    let a = mc_listen::decode::load(path)?;
    Ok((a.rate, a.frames))
}

#[cfg(not(feature = "listen"))]
fn decode(path: &Path) -> Result<(u32, Vec<[f32; 2]>), String> {
    let is_wav = path
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case("wav"));
    if !is_wav {
        return Err("only WAV without mc-listen".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    read_wav(&bytes)
}

/// PCM 16/24/32-bit and float WAV to stereo frames.
#[cfg_attr(feature = "listen", allow(dead_code))]
pub fn read_wav(b: &[u8]) -> Result<(u32, Vec<[f32; 2]>), String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut i = 12;
    let (mut fmt, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32_at(i + 4) as usize;
        let body = i + 8;
        if id == b"fmt " && body + 16 <= b.len() {
            fmt = u16_at(body);
            channels = u16_at(body + 2);
            rate = u32_at(body + 4);
            bits = u16_at(body + 14);
            if fmt == 0xFFFE && body + 26 <= b.len() {
                fmt = u16_at(body + 24);
            }
        } else if id == b"data" {
            let data = &b[body..(body + len).min(b.len())];
            let ch = channels.max(1) as usize;
            let width = (bits / 8) as usize;
            if width == 0 {
                return Err("bad WAV header".into());
            }
            let sample = |k: usize| -> f32 {
                let s = &data[k * width..k * width + width];
                match (fmt, bits) {
                    (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
                    (1, 24) => {
                        ((s[0] as i32) << 8 | (s[1] as i32) << 16 | (s[2] as i32) << 24) as f32
                            / 2147483648.0
                    }
                    (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2147483648.0,
                    (1, 8) => (s[0] as f32 - 128.0) / 128.0,
                    (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
                    _ => 0.0,
                }
            };
            if !matches!((fmt, bits), (1, 8 | 16 | 24 | 32) | (3, 32)) {
                return Err(format!("unsupported WAV format {fmt}/{bits}"));
            }
            let frames = data.len() / (width * ch);
            let out = (0..frames)
                .map(|f| {
                    let l = sample(f * ch);
                    let r = if ch > 1 { sample(f * ch + 1) } else { l };
                    [l, r]
                })
                .collect();
            return Ok((rate, out));
        }
        i = body + len + (len & 1);
    }
    Err("no audio data".into())
}

/// Linear resampling to the output rate.
pub fn resample(frames: &[[f32; 2]], from: u32, to: u32) -> Vec<[f32; 2]> {
    if from == to || frames.is_empty() {
        return frames.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (frames.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * ratio;
            let a = x.floor() as usize;
            let t = (x - a as f64) as f32;
            let p = frames[a.min(frames.len() - 1)];
            let q = frames[(a + 1).min(frames.len() - 1)];
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        })
        .collect()
}

pub fn load(st: &mut Studio, path: PathBuf) {
    let (tx, rx) = channel();
    let rate = st.audio.rate;
    let p = path.clone();
    std::thread::spawn(move || {
        let r = decode(&p).map(|(from, frames)| {
            let frames = resample(&frames, from, rate);
            let a = mc_music::render::analyse(&frames, rate);
            let cols = 1600;
            let per = (frames.len() / cols).max(1);
            let overview = frames
                .chunks(per)
                .map(|c| {
                    c.iter().fold((0.0f32, 0.0f32), |(lo, hi), f| {
                        let m = (f[0] + f[1]) * 0.5;
                        (lo.min(m), hi.max(m))
                    })
                })
                .collect();
            Loaded {
                path: p.clone(),
                frames: Arc::new(frames),
                rate,
                lufs: a.lufs,
                peak_db: a.peak_db,
                overview,
            }
        });
        let _ = tx.send(r);
    });
    st.reference.loading = Some((path, rx));
}

/// Once a frame: finish loads, follow the player, keep the gain matched.
pub fn tick(st: &mut Studio, ref_pos: f64, ref_playing: bool) {
    st.reference.pos = ref_pos;
    st.reference.playing = ref_playing;
    if let Some((_, rx)) = &st.reference.loading {
        if let Ok(r) = rx.try_recv() {
            st.reference.loading = None;
            match r {
                Ok(l) => {
                    st.audio.send_ref(RefCmd::Load(l.frames.clone()));
                    st.say(
                        format!(
                            "Reference {} loaded: {:.1} LUFS",
                            l.path.file_name().unwrap_or_default().to_string_lossy(),
                            l.lufs
                        ),
                        Tone::Info,
                    );
                    st.reference.loaded = Some(l);
                    st.reference.span = None;
                    st.audio.send_ref(RefCmd::Span(None));
                    st.reference.sent_gain = -1.0;
                }
                Err(e) => st.say(format!("Could not load the reference: {e}"), Tone::Bad),
            }
        }
    }
    // The song's loudness: follow it slowly while it plays and is heard.
    let l = st.meters.loudness;
    if st.status.playing && !st.reference.audible && l > -50.0 {
        st.reference.song_lufs += (l - st.reference.song_lufs) * 0.01;
    }
    let gain = 10f32.powf(st.reference.offset_db() / 20.0);
    if (gain - st.reference.sent_gain).abs() > 0.005 {
        st.reference.sent_gain = gain;
        st.audio.send_ref(RefCmd::Gain(gain));
    }
}

/// Backtick flips between the reference and the song.
pub fn flip(st: &mut Studio) {
    if st.reference.loaded.is_none() {
        return;
    }
    st.reference.audible = !st.reference.audible;
    st.audio.send_ref(RefCmd::Audible(st.reference.audible));
    if st.reference.audible && !st.reference.playing {
        st.audio.send_ref(RefCmd::Play(true));
    }
}

pub fn show(ui: &mut egui::Ui, st: &mut Studio) {
    let now = ui.input(|i| i.time);
    if now - st.reference.files_at > 3.0 {
        st.reference.files_at = now;
        st.reference.files.clear();
        if let Some(d) = &st.music_dir {
            let mut v = Vec::new();
            list(&d.join("references"), &mut v, 0);
            v.sort();
            st.reference.files = v;
        }
    }
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let list_r = Rect::from_min_size(area.min + vec2(8.0, 6.0), vec2(230.0, area.height() - 12.0));
    let main = Rect::from_min_max(
        pos2(list_r.right() + 10.0, area.top() + 6.0),
        area.max - vec2(8.0, 6.0),
    );
    let mut l = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(list_r)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    widgets::caption(&mut l, "References");
    if st.reference.files.is_empty() {
        l.label(
            egui::RichText::new("Put WAV, MP3, FLAC or Ogg files in data/music/references/.")
                .color(FAINT)
                .small(),
        );
    }
    egui::ScrollArea::vertical()
        .id_salt("ref-files")
        .show(&mut l, |ui| {
            for p in st.reference.files.clone() {
                let name = p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let current = st.reference.loaded.as_ref().is_some_and(|x| x.path == p);
                let loading = st.reference.loading.as_ref().is_some_and(|x| x.0 == p);
                let text = if loading {
                    format!("{name} (loading)")
                } else {
                    name
                };
                if ui.selectable_label(current, text).clicked() && !current && !loading {
                    load(st, p.clone());
                }
            }
        });
    let mut m = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(main)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let m = &mut m;
    let Some(info) = st
        .reference
        .loaded
        .as_ref()
        .map(|l| (l.lufs, l.peak_db, l.rate))
    else {
        m.add_space(20.0);
        m.label(
            egui::RichText::new("No reference loaded")
                .font(theme::font_light(22.0))
                .color(DIM),
        );
        m.label(
            egui::RichText::new(
                "Pick a file on the left. ` (backtick) flips between it and the song.",
            )
            .color(FAINT),
        );
        return;
    };
    let secs = st.reference.seconds();
    m.horizontal_wrapped(|ui| {
        let playing = st.reference.playing;
        if widgets::icon_button(
            ui,
            if playing {
                widgets::Icon::Stop
            } else {
                widgets::Icon::Play
            },
            playing,
            24.0,
            "Play the reference",
        )
        .clicked()
        {
            st.audio.send_ref(RefCmd::Play(!playing));
        }
        let hearing_ref = st.reference.audible;
        if widgets::toggle(
            ui,
            hearing_ref,
            if hearing_ref {
                "Hearing the reference"
            } else {
                "Hearing the song"
            },
            WARN,
        )
        .on_hover_text("` (backtick) flips; both keep playing so they stay in place")
        .clicked()
        {
            flip(st);
        }
        if ui.small_button("Unload").clicked() {
            st.audio.send_ref(RefCmd::Unload);
            st.reference.loaded = None;
            st.reference.audible = false;
            st.reference.span = None;
        }
        if widgets::toggle(ui, st.reference.match_loudness, "Match loudness", ACCENT).clicked() {
            st.reference.match_loudness = !st.reference.match_loudness;
        }
        let off = st.reference.offset_db();
        ui.label(
            egui::RichText::new(format!(
                "{:.1} LUFS, peak {:.1} dB; played {:+.1} dB against the song's {:.1} LUFS",
                info.0, info.1, off, st.reference.song_lufs
            ))
            .color(DIM)
            .small(),
        );
    });
    m.add_space(4.0);
    // The waveform: click to move, drag to mark a span.
    let w = m.available_width();
    let (wr, resp) = m.allocate_exact_size(vec2(w, 90.0), Sense::click_and_drag());
    let p = m.painter();
    p.rect_filled(wr, CornerRadius::same(3), BG0);
    if let Some(l) = &st.reference.loaded {
        let n = l.overview.len().max(1);
        for (i, (lo, hi)) in l.overview.iter().enumerate() {
            let x = wr.left() + wr.width() * i as f32 / n as f32;
            let c = wr.center().y;
            p.vline(
                x,
                (c - hi * wr.height() * 0.48)..=(c - lo * wr.height() * 0.48),
                Stroke::new(1.0, theme::with_alpha(TEXT, 90)),
            );
        }
    }
    let x_of = |s: f32| wr.left() + wr.width() * (s / secs.max(0.001));
    let s_of = |x: f32| ((x - wr.left()) / wr.width()).clamp(0.0, 1.0) * secs;
    if let Some((a, b)) = st.reference.span {
        p.rect_filled(
            Rect::from_min_max(pos2(x_of(a), wr.top()), pos2(x_of(b), wr.bottom())),
            CornerRadius::ZERO,
            theme::with_alpha(ACCENT, 40),
        );
        p.text(
            pos2(x_of(a) + 3.0, wr.top() + 3.0),
            Align2::LEFT_TOP,
            format!("{a:.1}-{b:.1} s"),
            theme::font_body(10.0),
            ACCENT,
        );
    }
    let pos_s = st.reference.pos as f32 / info.2 as f32;
    p.vline(
        x_of(pos_s),
        wr.y_range(),
        Stroke::new(1.5, if st.reference.audible { WARN } else { DIM }),
    );
    if resp.drag_started() {
        st.reference.drag_from = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
            .map(|p| s_of(p.x));
    }
    if resp.dragged() {
        if let (Some(a), Some(pos)) = (st.reference.drag_from, resp.interact_pointer_pos()) {
            let b = s_of(pos.x);
            st.reference.span = Some((a.min(b), a.max(b)));
        }
    }
    if resp.drag_stopped() {
        st.reference.drag_from = None;
        send_span(st, info.2);
        if let Some((a, _)) = st.reference.span {
            st.audio.send_ref(RefCmd::Seek(a as f64 * info.2 as f64));
        }
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            st.audio
                .send_ref(RefCmd::Seek(s_of(pos.x) as f64 * info.2 as f64));
        }
    }
    m.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{:.1} / {:.1} s", pos_s, secs))
                .color(FAINT)
                .small(),
        );
        if st.reference.span.is_some() {
            if widgets::toggle(ui, st.reference.loop_span, "Loop the span", ACCENT).clicked() {
                st.reference.loop_span = !st.reference.loop_span;
                send_span(st, info.2);
            }
            if ui.small_button("Clear the span").clicked() {
                st.reference.span = None;
                send_span(st, info.2);
            }
            ui.label(
                egui::RichText::new(
                    "The span goes with your messages to Claude as \"this part\" of the reference",
                )
                .color(FAINT)
                .small(),
            );
        } else {
            ui.label(
                egui::RichText::new("Drag on the waveform to mark a span")
                    .color(FAINT)
                    .small(),
            );
        }
    });
    m.add_space(4.0);
    // Spectra: the song now against the reference at its playhead.
    let rest = m.available_rect_before_wrap();
    let spec_r = Rect::from_min_size(
        rest.min,
        vec2(rest.width(), (rest.height() - 4.0).max(60.0)),
    );
    spectra(st, info.2);
    draw_spectrum(
        m.painter(),
        spec_r,
        &st.reference.ours,
        Some(&st.reference.theirs),
        "Spectrum: song (fill) and reference (line)",
    );
    m.painter().rect_stroke(
        spec_r,
        CornerRadius::same(3),
        Stroke::new(1.0, theme::line(12)),
        egui::StrokeKind::Inside,
    );
    let _ = BG2;
}

fn send_span(st: &mut Studio, rate: u32) {
    let span = if st.reference.loop_span {
        st.reference
            .span
            .map(|(a, b)| ((a * rate as f32) as usize, (b * rate as f32) as usize))
    } else {
        None
    };
    st.audio.send_ref(RefCmd::Span(span.filter(|(a, b)| b > a)));
}

/// Log-spaced columns of a signal's spectrum, dB.
fn columns(mono: &[f32], rate: f32, cols: usize) -> Vec<f32> {
    let spec = fft::spectrum_db(mono);
    let bins = spec.len().max(1);
    let (lo, hi) = (20.0f32.ln(), (rate * 0.5).min(20000.0).ln());
    (0..cols)
        .map(|c| {
            let f0 = (lo + (hi - lo) * c as f32 / cols as f32).exp();
            let f1 = (lo + (hi - lo) * (c + 1) as f32 / cols as f32).exp();
            let b0 = ((f0 / (rate * 0.5)) * bins as f32) as usize;
            let b1 = (((f1 / (rate * 0.5)) * bins as f32) as usize)
                .max(b0 + 1)
                .min(bins);
            spec[b0.min(bins - 1)..b1]
                .iter()
                .copied()
                .fold(-140.0, f32::max)
        })
        .collect()
}

fn spectra(st: &mut Studio, rate: u32) {
    let cols = 240;
    let scope = st.meters.scope_ordered();
    let ours: Vec<f32> = scope.iter().map(|f| (f[0] + f[1]) * 0.5).collect();
    let now_ours = columns(&ours, rate as f32, cols);
    let theirs_now = st.reference.loaded.as_ref().map(|l| {
        let at = (st.reference.pos as usize).min(l.frames.len());
        let from = at.saturating_sub(4096);
        let mono: Vec<f32> = l.frames[from..at]
            .iter()
            .map(|f| (f[0] + f[1]) * 0.5 * st.reference.sent_gain.max(0.0))
            .collect();
        if mono.len() < 256 {
            vec![-120.0; cols]
        } else {
            columns(&mono, rate as f32, cols)
        }
    });
    let smooth = |acc: &mut Vec<f32>, cur: &[f32]| {
        if acc.len() != cur.len() {
            *acc = cur.to_vec();
        }
        for (a, c) in acc.iter_mut().zip(cur) {
            *a += (c - *a) * 0.2;
        }
    };
    smooth(&mut st.reference.ours, &now_ours);
    if let Some(t) = theirs_now {
        smooth(&mut st.reference.theirs, &t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_mc_music_writes() {
        let frames: Vec<[f32; 2]> = (0..480)
            .map(|i| [((i as f32) * 0.05).sin() * 0.5, -0.25])
            .collect();
        let path = std::env::temp_dir().join(format!("mc-studio-wav-{}.wav", std::process::id()));
        mc_music::render::write_wav(&path, &frames, 48000).unwrap();
        let (rate, back) = read_wav(&std::fs::read(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(rate, 48000);
        assert_eq!(back.len(), 480);
        assert!((back[10][0] - frames[10][0]).abs() < 1e-3);
        assert!((back[10][1] + 0.25).abs() < 1e-3);
    }

    #[test]
    fn resampling_keeps_duration() {
        let f = vec![[0.5f32, 0.5]; 44100];
        let r = resample(&f, 44100, 48000);
        assert_eq!(r.len(), 48000);
        assert!((r[1000][0] - 0.5).abs() < 1e-6);
    }
}
