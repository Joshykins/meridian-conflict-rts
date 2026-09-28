//! Sketching an idea to show Claude: play it on the computer keyboard or a
//! MIDI keyboard in time with the song (a bar of count-in, the metronome on),
//! or hum or sing it into the microphone and let the pitch tracker write it
//! down. The result is a `Pattern` that can go with the next message or into
//! the song.

use crate::app::{Studio, Tone};
use crate::theme::{self, ACCENT, BAD, BG0, DIM, FAINT, TEXT};
use crate::widgets;
use eframe::egui::{self, pos2, vec2, CornerRadius, Rect, Sense};
use mc_music::{Command, Note, Pattern, PPQ};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
pub struct Sketch {
    /// Keys and MIDI being recorded.
    rec: Option<Rec>,
    /// A take waiting for a decision.
    pub take: Option<Pattern>,
    pub quantize: bool,
    mic: Mic,
    /// Humming being recorded (samples arrive on the input thread).
    humming: bool,
    /// A transcription running on a worker thread.
    hearing: Option<std::sync::mpsc::Receiver<Result<Vec<Note>, String>>>,
    was_metronome: bool,
}

struct Rec {
    /// Tick 0 of the take, after the count-in.
    zero: Instant,
    tempo: f32,
    /// (time, key, velocity; 0 = off).
    events: Vec<(Instant, u8, u8)>,
}

impl Sketch {
    pub fn new() -> Sketch {
        Sketch {
            quantize: true,
            ..Default::default()
        }
    }

    /// A note played on the computer keyboard (velocity 0 = released).
    pub fn note(&mut self, key: u8, vel: u8) {
        if let Some(r) = &mut self.rec {
            r.events.push((Instant::now(), key, vel));
        }
    }
}

/// Notes from note-on/off events: times in seconds from tick 0 at `tempo`.
pub fn events_to_notes(events: &[(f64, u8, u8)], tempo: f32, grid: Option<u32>) -> Vec<Note> {
    let tick = |s: f64| (s.max(0.0) * tempo as f64 / 60.0 * PPQ as f64) as i64;
    let mut open: Vec<(u8, f64, u8)> = Vec::new();
    let mut notes = Vec::new();
    let last = events.last().map(|e| e.0).unwrap_or(0.0);
    for &(t, key, vel) in events {
        if vel > 0 {
            open.push((key, t, vel));
        } else if let Some(i) = open.iter().position(|o| o.0 == key) {
            let (k, start, v) = open.remove(i);
            notes.push((k, start, t, v));
        }
    }
    for (k, start, v) in open {
        notes.push((k, start, last.max(start + 0.1), v));
    }
    let mut out: Vec<Note> = notes
        .into_iter()
        .map(|(k, a, b, v)| {
            let (mut t0, mut t1) = (tick(a), tick(b));
            if let Some(g) = grid {
                t0 = crate::notes::snap_round(t0, g);
                t1 = crate::notes::snap_round(t1, g).max(t0 + g as i64);
            }
            Note(t0.max(0) as u32, (t1 - t0).max(1) as u32, k, v)
        })
        .collect();
    out.sort_by_key(|n| (n.0, n.2));
    out
}

fn to_pattern(notes: Vec<Note>, beats_per_bar: u32) -> Pattern {
    let end = notes.iter().map(|n| n.end()).max().unwrap_or(0);
    let bar = beats_per_bar.max(1) * PPQ;
    let bars = end.div_ceil(bar).max(1);
    let stamp = mc_music::history::now() % 100000;
    Pattern {
        name: format!("sketch {stamp}"),
        beats: bars * beats_per_bar,
        notes,
        automation: Vec::new(),
    }
}

fn start_keys(st: &mut Studio) {
    let beat = 60.0 / st.song.tempo.max(1.0) as f64;
    let bar = st.song.bar_ticks().max(1);
    // Tick 0 of the take is a bar line of the transport: the next one after a
    // full bar of count-in (from where it is now when already playing).
    let into_bar = if st.status.playing {
        let s = &st.status;
        let t = match st.mode {
            crate::app::PlayMode::Song => s.song_tick,
            crate::app::PlayMode::Pattern => s.pattern_tick,
            _ => s.section_tick,
        };
        t % bar
    } else {
        0
    };
    // The first bar line at least a whole bar away.
    let count_in = beat * (bar + (bar - into_bar) % bar) as f64 / PPQ as f64;
    st.sketch.was_metronome = st.metronome;
    st.metronome = true;
    st.send(Command::Metronome(true));
    if !st.status.playing {
        st.toggle_play();
    }
    st.midi.tapping(true);
    st.sketch.rec = Some(Rec {
        zero: Instant::now() + std::time::Duration::from_secs_f64(count_in),
        tempo: st.song.tempo,
        events: Vec::new(),
    });
}

fn stop_keys(st: &mut Studio) {
    let Some(mut r) = st.sketch.rec.take() else {
        return;
    };
    r.events.extend(st.midi.take_tap());
    st.midi.tapping(false);
    if !st.sketch.was_metronome {
        st.metronome = false;
        st.send(Command::Metronome(false));
    }
    r.events.sort_by_key(|e| e.0);
    let secs: Vec<(f64, u8, u8)> = r
        .events
        .iter()
        .map(|(t, k, v)| {
            let s = if *t >= r.zero {
                t.duration_since(r.zero).as_secs_f64()
            } else {
                -(r.zero.duration_since(*t).as_secs_f64())
            };
            (s, *k, *v)
        })
        .collect();
    let grid = st.sketch.quantize.then_some(PPQ / 4);
    let notes = events_to_notes(&secs, r.tempo, grid);
    if notes.is_empty() {
        st.say("Nothing was played", Tone::Warn);
        return;
    }
    st.sketch.take = Some(to_pattern(notes, st.song.beats_per_bar));
}

/// The sketch controls, in the "Tell Claude" area.
pub fn controls(ui: &mut egui::Ui, st: &mut Studio) {
    poll_hearing(st);
    if st.sketch.hearing.is_some() {
        ui.label(
            egui::RichText::new("Listening to the take...")
                .color(DIM)
                .small(),
        );
    }
    if st.sketch.rec.is_some() {
        // Drain MIDI while recording so a long take does not pile up.
        let tap = st.midi.take_tap();
        if let Some(r) = &mut st.sketch.rec {
            r.events.extend(tap);
        }
    }
    ui.horizontal(|ui| {
        let now = Instant::now();
        match &st.sketch.rec {
            Some(r) if now < r.zero => {
                let beat = 60.0 / r.tempo.max(1.0) as f64;
                let left = (r.zero.duration_since(now).as_secs_f64() / beat).ceil() as u32;
                widgets::toggle(ui, true, &format!("Count-in {left}"), BAD);
            }
            Some(r) => {
                let n = r.events.iter().filter(|e| e.2 > 0).count();
                if widgets::toggle(ui, true, &format!("Stop ({n} notes)"), BAD).on_hover_text("Stop recording").clicked() {
                    stop_keys(st);
                }
            }
            None => {
                if widgets::toggle(ui, false, "Record keys", BAD)
                    .on_hover_text("Play an idea on the keyboard (Z..M, Q..P) or MIDI, in time with the song: one bar of count-in")
                    .clicked()
                {
                    start_keys(st);
                }
            }
        }
        let humming = st.sketch.humming;
        let label = if humming { format!("Stop humming ({:.1} s)", st.sketch.mic.seconds()) } else { "Hum".to_string() };
        if widgets::toggle(ui, humming, &label, BAD).on_hover_text("Hum or sing into the microphone; it is written down as notes at the song's tempo").clicked() {
            if humming {
                stop_humming(st);
            } else {
                match st.sketch.mic.start() {
                    Ok(()) => st.sketch.humming = true,
                    Err(e) => st.say(format!("No microphone: {e}"), Tone::Bad),
                }
            }
        }
        if widgets::toggle(ui, st.sketch.quantize, "Snap 1/16", ACCENT).clicked() {
            st.sketch.quantize = !st.sketch.quantize;
        }
    });
    let Some(take) = st.sketch.take.clone() else {
        return;
    };
    egui::Frame::new()
        .fill(BG0)
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!(
                    "{}: {} notes over {} beats",
                    take.name,
                    take.notes.len(),
                    take.beats
                ))
                .color(TEXT)
                .small(),
            );
            preview(ui, &take);
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Attach to message").clicked() {
                    st.collab.sketch = Some(take.clone());
                    st.sketch.take = None;
                }
                if ui.small_button("Hear it").clicked() {
                    st.song.patterns.retain(|p| p.name != take.name);
                    st.song.patterns.push(take.clone());
                    st.sel.pattern = Some(st.song.patterns.len() - 1);
                    st.mode = crate::app::PlayMode::Pattern;
                    if !st.status.playing {
                        st.toggle_play();
                    }
                }
                if ui
                    .small_button("Into the open pattern")
                    .on_hover_text("Adds the notes to the pattern in the piano roll")
                    .clicked()
                {
                    if let Some(p) = st.sel.pattern.and_then(|p| st.song.patterns.get_mut(p)) {
                        let ticks = p.ticks();
                        p.notes.extend(take.notes.iter().filter(|n| n.0 < ticks));
                        p.notes.sort_by_key(|n| (n.0, n.2));
                        st.sketch.take = None;
                    }
                }
                if ui.small_button("Discard").clicked() {
                    st.sketch.take = None;
                }
            });
        });
}

fn preview(ui: &mut egui::Ui, p: &Pattern) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(r, CornerRadius::same(2), theme::BG2);
    if p.notes.is_empty() {
        return;
    }
    let (lo, hi) = p
        .notes
        .iter()
        .fold((127u8, 0u8), |(l, h), n| (l.min(n.2), h.max(n.2)));
    let span = (hi - lo).max(5) as f32;
    let ticks = p.ticks().max(1) as f32;
    for n in &p.notes {
        let x = r.left() + r.width() * n.0 as f32 / ticks;
        let x1 = r.left() + r.width() * n.end() as f32 / ticks;
        let y = r.bottom() - 3.0 - (n.2 - lo) as f32 / span * (r.height() - 8.0);
        painter.rect_filled(
            Rect::from_min_max(pos2(x, y - 1.5), pos2(x1.max(x + 2.0), y + 1.5)),
            CornerRadius::ZERO,
            ACCENT,
        );
    }
    let _ = DIM;
    let _ = FAINT;
}

fn stop_humming(st: &mut Studio) {
    st.sketch.humming = false;
    let (samples, rate) = st.sketch.mic.stop();
    if samples.len() < rate as usize / 4 {
        st.say("Too short to hear", Tone::Warn);
        return;
    }
    // The pitch tracker takes a moment on a long take: keep the UI running.
    let (tx, rx) = std::sync::mpsc::channel();
    let tempo = st.song.tempo;
    std::thread::spawn(move || {
        let _ = tx.send(transcribe(&samples, rate, tempo));
    });
    st.sketch.hearing = Some(rx);
}

/// Picks up a finished transcription.
fn poll_hearing(st: &mut Studio) {
    let Some(rx) = &st.sketch.hearing else { return };
    let Ok(r) = rx.try_recv() else { return };
    st.sketch.hearing = None;
    match r {
        Ok(notes) if !notes.is_empty() => {
            st.sketch.take = Some(to_pattern(notes, st.song.beats_per_bar))
        }
        Ok(_) => st.say("No notes heard in that", Tone::Warn),
        Err(e) => st.say(e, Tone::Warn),
    }
}

/// Humming to notes.
#[cfg(feature = "listen")]
fn transcribe(samples: &[f32], rate: u32, tempo: f32) -> Result<Vec<Note>, String> {
    Ok(mc_listen::pitch::transcribe(samples, rate, tempo))
}

#[cfg(not(feature = "listen"))]
fn transcribe(_samples: &[f32], _rate: u32, _tempo: f32) -> Result<Vec<Note>, String> {
    Err("This build has no pitch tracker (build with the listen feature)".into())
}

/// The microphone: mono samples at the device's rate while recording.
#[derive(Default)]
struct Mic {
    buf: Arc<Mutex<Vec<f32>>>,
    rate: u32,
    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    stream: Option<cpal::Stream>,
}

impl Mic {
    fn seconds(&self) -> f32 {
        let n = self.buf.lock().map(|b| b.len()).unwrap_or(0);
        n as f32 / self.rate.max(1) as f32
    }

    #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
    fn start(&mut self) -> Result<(), String> {
        Err("built without audio".into())
    }

    #[cfg(not(any(not(target_os = "linux"), feature = "alsa")))]
    fn stop(&mut self) -> (Vec<f32>, u32) {
        (Vec::new(), 48000)
    }

    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    fn start(&mut self) -> Result<(), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use cpal::SampleFormat;
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("no input device")?;
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        self.rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        if let Ok(mut b) = self.buf.lock() {
            b.clear();
        }
        fn push<T: Copy>(
            buf: &Arc<Mutex<Vec<f32>>>,
            data: &[T],
            channels: usize,
            conv: impl Fn(T) -> f32,
        ) {
            if let Ok(mut b) = buf.lock() {
                for frame in data.chunks(channels.max(1)) {
                    let s: f32 = frame.iter().map(|&x| conv(x)).sum::<f32>() / frame.len() as f32;
                    b.push(s);
                }
            }
        }
        let err = |e| eprintln!("mc-studio: microphone error: {e}");
        let buf = self.buf.clone();
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |d: &[f32], _| push(&buf, d, channels, |x| x),
                err,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |d: &[i16], _| push(&buf, d, channels, |x| x as f32 / 32768.0),
                err,
                None,
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |d: &[u16], _| push(&buf, d, channels, |x| (x as f32 - 32768.0) / 32768.0),
                err,
                None,
            ),
            other => return Err(format!("unsupported microphone format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }

    #[cfg(any(not(target_os = "linux"), feature = "alsa"))]
    fn stop(&mut self) -> (Vec<f32>, u32) {
        if let Some(s) = self.stream.take() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(s)));
        }
        let samples = self
            .buf
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default();
        (samples, self.rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_notes_land_on_the_grid() {
        // 120 bpm: a beat is 0.5 s. A note at 0.51 s held to 0.74 s is the
        // second beat's sixteenths 0..2.
        let ev: Vec<(f64, u8, u8)> = vec![
            (0.51, 60, 100),
            (0.74, 60, 0),
            (-0.03, 64, 90),
            (0.2, 64, 0),
        ];
        let mut ev = ev;
        ev.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = events_to_notes(&ev, 120.0, Some(24));
        assert_eq!(
            n[0],
            Note(0, 48, 64, 90),
            "a note just before the downbeat snaps onto it"
        );
        assert_eq!(n[1].0, 96);
        assert_eq!(n[1].1, 48);
        assert_eq!(n[1].2, 60);
        let p = to_pattern(n, 4);
        assert_eq!(p.beats, 4);
    }

    #[test]
    fn a_held_note_ends_with_the_take() {
        let n = events_to_notes(&[(0.0, 50, 80), (1.0, 52, 80), (1.5, 52, 0)], 60.0, None);
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].2, 50);
        assert_eq!(n[0].1, 144);
    }
}
