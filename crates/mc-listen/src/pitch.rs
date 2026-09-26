//! Monophonic pitch: humming, singing, bass lines.
//!
//! 1. f0 per ~10 ms hop by YIN (the cumulative-mean-normalised difference
//!    function, computed by FFT), keeping up to six candidate periods a frame.
//! 2. A pYIN-style Viterbi path through the candidates plus an "unvoiced"
//!    state: jumps cost by the semitone and octave jumps cost extra, so a
//!    breathy frame or a strong second harmonic does not flip the octave.
//! 3. Median smoothing, then notes: a new note where the pitch moves more than
//!    `split_semitones` for longer than `min_note`, where the level dips (a
//!    repeated note), or across a gap. Each note's pitch is the median of its
//!    middle frames, rounded to the nearest semitone after the singer's global
//!    tuning is taken out (and optionally snapped to a key).
//! 4. Quantised to a grid at the given tempo (1/16 by default), velocity from
//!    the note's level. The unquantised notes are kept too.

use crate::dsp::{self, hz_to_midi};
use mc_music::song::Scale;
use mc_music::{Note, PPQ};
use rustfft::num_complex::Complex32;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

/// How to hear a line.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscribeOpts {
    /// Beats per minute for quantising.
    pub tempo: f32,
    /// Lowest and highest f0 looked for, Hz.
    pub fmin: f32,
    pub fmax: f32,
    /// Seconds between pitch frames.
    pub hop: f32,
    /// Quantisation grid in ticks (`PPQ` = 96 to the beat; 24 = a sixteenth).
    pub grid: u32,
    /// Snap notes into this key (root 0 = C, scale).
    pub key: Option<(u8, Scale)>,
    /// Notes shorter than this (seconds) are dropped.
    pub min_note: f32,
    /// A pitch move bigger than this (semitones), held for `min_note`, starts a new note.
    pub split_semitones: f32,
    /// Cost of calling a frame unvoiced (0..1): higher hears more frames as pitched.
    pub voicing: f32,
    /// A level dip this deep (dB) inside a note splits it (repeated notes).
    pub dip_db: f32,
    /// Seconds into the recording that is tick 0.
    pub origin: f32,
    /// Put tick 0 at the first note's (quantised) beat instead of at `origin`.
    pub align_first: bool,
    /// Take the singer's overall sharp/flat offset out before rounding.
    pub adapt_tuning: bool,
    /// Frames quieter than this far below the loudest are silence (dB).
    pub floor_db: f32,
    /// Move notes that fall far under (or over) the line around them by an
    /// octave: right for bass lines (period doubling under a kick), off for
    /// voices, whose leaps are real.
    #[serde(default)]
    pub octave_fix: bool,
}

impl TranscribeOpts {
    /// Humming and singing: 65-1100 Hz.
    pub fn voice(tempo: f32) -> TranscribeOpts {
        TranscribeOpts {
            tempo,
            fmin: 65.0,
            fmax: 1100.0,
            hop: 0.01,
            grid: PPQ / 4,
            key: None,
            min_note: 0.06,
            split_semitones: 0.7,
            voicing: 0.45,
            dip_db: 6.0,
            origin: 0.0,
            align_first: false,
            adapt_tuning: true,
            floor_db: 40.0,
            octave_fix: false,
        }
    }

    /// Bass lines (already low-passed): 27-420 Hz, no tuning adaptation.
    pub fn bass(tempo: f32) -> TranscribeOpts {
        TranscribeOpts {
            fmin: 27.0,
            fmax: 420.0,
            adapt_tuning: false,
            min_note: 0.07,
            dip_db: 3.0,
            octave_fix: true,
            ..TranscribeOpts::voice(tempo)
        }
    }
}

/// A frame-by-frame pitch track.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct F0Track {
    /// Seconds between frames; frame `i` is centred at `i * hop`.
    pub hop: f32,
    /// Hz, 0 = unvoiced.
    pub f0: Vec<f32>,
    /// 0..1 (1 - the YIN dip).
    pub confidence: Vec<f32>,
    /// Frame level, dBFS.
    pub level_db: Vec<f32>,
}

/// A note as heard, in seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub start: f32,
    pub end: f32,
    /// Median pitch, fractional MIDI (before tuning correction).
    pub pitch: f32,
    /// The MIDI key it was rounded (and snapped) to.
    pub key: u8,
    /// Mean level, dBFS.
    pub level_db: f32,
    /// Mean voicing confidence 0..1.
    pub confidence: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Transcription {
    pub tempo: f32,
    /// Quantised to the grid.
    pub notes: Vec<Note>,
    /// Same notes, timing to the tick.
    pub raw: Vec<Note>,
    pub events: Vec<NoteEvent>,
    /// The singer's overall offset from A440 that was taken out, cents.
    pub tuning_cents: f32,
    /// Seconds that became tick 0.
    pub origin: f32,
    pub track: F0Track,
}

/// Monophonic transcription of humming or singing at `tempo`, quantised to sixteenths.
pub fn transcribe(samples: &[f32], rate: u32, tempo: f32) -> Vec<Note> {
    transcribe_with(samples, rate, &TranscribeOpts::voice(tempo)).notes
}

/// Transcription with every option and everything it found.
pub fn transcribe_with(samples: &[f32], rate: u32, opts: &TranscribeOpts) -> Transcription {
    let track = track_f0(samples, rate, opts);
    let (events, tuning) = segment(&track, opts);
    let (notes, raw, origin) = quantise(&events, opts);
    Transcription { tempo: opts.tempo, notes, raw, events, tuning_cents: tuning * 100.0, origin, track }
}

struct Cand {
    midi: f32,
    cost: f32,
    conf: f32,
}

/// YIN candidates per frame and the Viterbi path through them.
pub fn track_f0(samples: &[f32], rate: u32, opts: &TranscribeOpts) -> F0Track {
    // Work at the lowest rate that keeps ~8 samples per period at fmax.
    let target = (opts.fmax * 8.0).max(4000.0);
    let factor = ((rate as f32 / target).floor() as usize).max(1);
    let hp = dsp::band(samples, rate, opts.fmin * 0.6, 0.0);
    let x = dsp::decimate(&hp, factor);
    let r = rate as f32 / factor as f32;
    let hop_n = (opts.hop * r).round().max(1.0) as usize;
    let hop = hop_n as f32 / r;
    let max_lag = ((r / opts.fmin).ceil() as usize + 2).max(4);
    let min_lag = ((r / opts.fmax).floor() as usize).max(2);
    let w = (2 * max_lag).max((0.025 * r) as usize);
    let n_frames = x.len() / hop_n + 1;
    let size = (w + max_lag + 1).next_power_of_two() * 2;
    let mut planner = FftPlanner::new();
    let fwd = planner.plan_fft_forward(size);
    let inv = planner.plan_fft_inverse(size);
    let mut pre = vec![0.0f64; x.len() + 1];
    for (i, &v) in x.iter().enumerate() {
        pre[i + 1] = pre[i] + (v * v) as f64;
    }
    let sample = |i: isize| -> f32 {
        if i >= 0 && (i as usize) < x.len() {
            x[i as usize]
        } else {
            0.0
        }
    };
    let energy = |a: isize, len: usize| -> f64 {
        let lo = a.clamp(0, x.len() as isize) as usize;
        let hi = (a + len as isize).clamp(0, x.len() as isize) as usize;
        pre[hi] - pre[lo]
    };

    let mut level = Vec::with_capacity(n_frames);
    let mut cands: Vec<Vec<Cand>> = Vec::with_capacity(n_frames);
    let mut abuf = vec![Complex32::new(0.0, 0.0); size];
    let mut sbuf = vec![Complex32::new(0.0, 0.0); size];
    let mut d = vec![0.0f32; max_lag + 2];
    for k in 0..n_frames {
        let c = (k * hop_n) as isize;
        let start = c - (w as isize) / 2 - (max_lag as isize) / 2;
        let e0 = energy(start, w);
        // Level over a short window (20 ms, or one period of fmin if longer) so gaps between repeated notes show.
        let lw = ((0.02 * r) as usize).max((r / opts.fmin) as usize).min(w);
        level.push(dsp::pow_db((energy(c - lw as isize / 2, lw) / lw as f64) as f32));
        if e0 <= 1e-10 {
            cands.push(Vec::new());
            continue;
        }
        for i in 0..size {
            let s = if i < w + max_lag + 1 { sample(start + i as isize) } else { 0.0 };
            sbuf[i] = Complex32::new(s, 0.0);
            abuf[i] = Complex32::new(if i < w { s } else { 0.0 }, 0.0);
        }
        fwd.process(&mut abuf);
        fwd.process(&mut sbuf);
        for i in 0..size {
            abuf[i] = abuf[i].conj() * sbuf[i];
        }
        inv.process(&mut abuf);
        let norm = 1.0 / size as f32;
        // d(tau) = e(0) + e(tau) - 2 r(tau); cumulative-mean normalised.
        d[0] = 1.0;
        let mut run = 0.0f64;
        for tau in 1..=max_lag + 1 {
            let et = energy(start + tau as isize, w);
            let diff = (e0 + et - 2.0 * (abuf[tau].re * norm) as f64).max(0.0);
            run += diff;
            d[tau] = if run > 0.0 { (diff * tau as f64 / run) as f32 } else { 1.0 };
        }
        let mut found: Vec<(f32, f32)> = Vec::new(); // (lag, d')
        for tau in min_lag.max(2)..=max_lag {
            if d[tau] < d[tau - 1] && d[tau] <= d[tau + 1] && d[tau] < 0.9 {
                // Parabolic interpolation of the dip.
                let (a, b, cc) = (d[tau - 1], d[tau], d[tau + 1]);
                let den = a - 2.0 * b + cc;
                let off = if den.abs() > 1e-9 { (0.5 * (a - cc) / den).clamp(-0.5, 0.5) } else { 0.0 };
                let val = b - 0.25 * (a - cc) * off;
                found.push((tau as f32 + off, val.max(0.0)));
            }
        }
        found.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        found.truncate(6);
        let best = found.first().map(|f| f.1).unwrap_or(1.0);
        // YIN's rule as a preference: the shortest period that is nearly as good.
        let short = found
            .iter()
            .filter(|f| f.1 < (best + 0.2).max(0.25))
            .map(|f| f.0)
            .fold(f32::MAX, f32::min);
        let fr: Vec<Cand> = found
            .iter()
            .map(|&(lag, v)| Cand {
                midi: hz_to_midi(r / lag),
                cost: v + 0.3 * (lag / short).log2().max(0.0),
                conf: (1.0 - v).clamp(0.0, 1.0),
            })
            .collect();
        cands.push(fr);
    }

    // Silence gate from the loudest frames.
    let loud = dsp::percentile(&level, 0.98);
    let gate = (loud - opts.floor_db).max(-75.0);

    // Viterbi: state 0 = unvoiced, 1.. = candidates.
    let n = cands.len();
    let jump = |a: f32, b: f32| -> f32 {
        let dd = (a - b).abs();
        let mut c = 0.03 * dd.min(24.0);
        if (dd - 12.0).abs() < 1.5 || (dd - 19.0).abs() < 1.0 || (dd - 24.0).abs() < 1.5 {
            c += 0.35;
        }
        c
    };
    let voice_switch = 0.15;
    let mut cost_prev: Vec<f32> = Vec::new();
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(n);
    for k in 0..n {
        let quiet = level[k] < gate;
        let em: Vec<f32> = std::iter::once(opts.voicing)
            .chain(cands[k].iter().map(|c| c.cost + if quiet { 2.0 } else { 0.0 }))
            .collect();
        let mut cost = vec![0.0f32; em.len()];
        let mut bk = vec![0usize; em.len()];
        if k == 0 {
            cost.copy_from_slice(&em);
        } else {
            for s in 0..em.len() {
                let mut best = f32::MAX;
                let mut arg = 0;
                for (p, &cp) in cost_prev.iter().enumerate() {
                    let t = match (p, s) {
                        (0, 0) => 0.0,
                        (0, _) | (_, 0) => voice_switch,
                        (p, s) => jump(cands[k - 1][p - 1].midi, cands[k][s - 1].midi),
                    };
                    if cp + t < best {
                        best = cp + t;
                        arg = p;
                    }
                }
                cost[s] = best + em[s];
                bk[s] = arg;
            }
        }
        back.push(bk);
        cost_prev = cost;
    }
    let mut path = vec![0usize; n];
    if n > 0 {
        let mut s = cost_prev
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        for k in (0..n).rev() {
            path[k] = s;
            s = back[k][s];
        }
    }
    let mut midi: Vec<f32> = vec![0.0; n];
    let mut conf = vec![0.0; n];
    for k in 0..n {
        if path[k] > 0 {
            let c = &cands[k][path[k] - 1];
            midi[k] = c.midi;
            conf[k] = c.conf;
        }
    }
    // Median of 5 inside voiced runs.
    let mut smooth = midi.clone();
    for k in 0..n {
        if midi[k] == 0.0 {
            continue;
        }
        let a = k.saturating_sub(2);
        let b = (k + 3).min(n);
        let v: Vec<f32> = midi[a..b].iter().copied().filter(|&m| m > 0.0).collect();
        smooth[k] = dsp::median(&v);
    }
    F0Track {
        hop,
        f0: smooth.iter().map(|&m| if m > 0.0 { dsp::midi_to_hz(m) } else { 0.0 }).collect(),
        confidence: conf,
        level_db: level,
    }
}

/// Splits a pitch track into notes. Returns the notes and the tuning offset (semitones) taken out.
pub fn segment(track: &F0Track, opts: &TranscribeOpts) -> (Vec<NoteEvent>, f32) {
    let n = track.f0.len();
    let hop = track.hop.max(1e-4);
    let midi: Vec<f32> = track.f0.iter().map(|&f| if f > 0.0 { hz_to_midi(f) } else { 0.0 }).collect();
    let min_frames = ((opts.min_note / hop).ceil() as usize).max(2);

    // Global tuning: the weighted circular mean of each frame's offset from the nearest semitone.
    let tuning = if opts.adapt_tuning {
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        for (k, &m) in midi.iter().enumerate() {
            if m > 0.0 {
                let ph = 2.0 * std::f32::consts::PI * (m - m.round());
                sx += ph.cos() * track.confidence[k];
                sy += ph.sin() * track.confidence[k];
            }
        }
        if sx.abs() + sy.abs() > 1e-6 {
            sy.atan2(sx) / (2.0 * std::f32::consts::PI)
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Voiced runs, bridging gaps of up to two frames at the same pitch.
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < n {
        if midi[k] == 0.0 {
            k += 1;
            continue;
        }
        let a = k;
        let mut b = k + 1;
        loop {
            while b < n && midi[b] > 0.0 {
                b += 1;
            }
            let mut g = b;
            while g < n && g < b + 3 && midi[g] == 0.0 {
                g += 1;
            }
            if g < n && g > b && g - b <= 2 && midi[g] > 0.0 && (midi[g] - midi[b - 1]).abs() < 1.0 {
                b = g;
                continue;
            }
            break;
        }
        runs.push((a, b));
        k = b;
    }

    // (start, end, split_by_pitch_only)
    let mut segs: Vec<(usize, usize, bool)> = Vec::new();
    for &(a, b) in &runs {
        // Moving average of 5 voiced frames: vibrato and jitter calm down, steps stay.
        let sm: Vec<f32> = (a..b)
            .map(|i| {
                let lo = i.saturating_sub(2).max(a);
                let hi = (i + 3).min(b);
                let v: Vec<f32> = midi[lo..hi].iter().copied().filter(|&m| m > 0.0).collect();
                if v.is_empty() {
                    0.0
                } else {
                    dsp::mean(&v)
                }
            })
            .collect();
        let at = |i: usize| sm[i - a];
        let mut cuts: Vec<(usize, bool)> = Vec::new();
        let mut seg_start = a;
        let mut streak: Option<(usize, f32)> = None; // (first frame, sign)
        for i in a..b {
            if at(i) == 0.0 {
                continue;
            }
            let lo = seg_start;
            let hist: Vec<f32> = (lo..i).map(at).filter(|&m| m > 0.0).collect();
            let reference = if hist.len() >= 3 { dsp::median(&hist) } else { at(lo).max(at(i)) };
            let dev = at(i) - reference;
            if dev.abs() > opts.split_semitones {
                let sign = dev.signum();
                match streak {
                    Some((s0, sg)) if sg == sign => {
                        if i + 1 - s0 >= min_frames {
                            cuts.push((s0, true));
                            seg_start = s0;
                            streak = None;
                        }
                    }
                    _ => streak = Some((i, sign)),
                }
            } else {
                streak = None;
            }
        }
        // Level dips inside the run: repeated notes.
        let lv = &track.level_db;
        let span = (0.15 / hop) as usize;
        for i in a + min_frames..b.saturating_sub(min_frames) {
            let l = lv[i];
            if l > lv[i - 1] || l > lv[i + 1] {
                continue;
            }
            let before = lv[i.saturating_sub(span).max(a)..i].iter().cloned().fold(f32::MIN, f32::max);
            let after = lv[i + 1..(i + 1 + span).min(b)].iter().cloned().fold(f32::MIN, f32::max);
            if before - l >= opts.dip_db && after - l >= opts.dip_db
                && !cuts.iter().any(|c| (c.0 as isize - i as isize).abs() < min_frames as isize) {
                    cuts.push((i, false));
                }
        }
        // Sudden rises (a re-attack on a held pitch).
        for i in a + min_frames..b {
            let lo = i.saturating_sub(4).max(a);
            let floor = lv[lo..i].iter().cloned().fold(f32::MAX, f32::min);
            if lv[i] - floor >= opts.dip_db + 3.0
                && !cuts.iter().any(|c| (c.0 as isize - i as isize).abs() < min_frames as isize)
            {
                cuts.push((i.saturating_sub(1).max(lo), false));
            }
        }
        cuts.sort_by_key(|c| c.0);
        let mut s = a;
        let mut s_pitch_only = false;
        for (c, pitch_only) in cuts {
            if c > s {
                segs.push((s, c, s_pitch_only));
                s = c;
                s_pitch_only = pitch_only;
            }
        }
        segs.push((s, b, s_pitch_only));
    }

    let mut events: Vec<(NoteEvent, bool)> = Vec::new();
    for (a, b, pitch_only) in segs {
        let voiced: Vec<usize> = (a..b).filter(|&i| midi[i] > 0.0).collect();
        if voiced.len() < min_frames {
            continue;
        }
        let trim = if voiced.len() >= 8 { voiced.len() * 15 / 100 } else { 0 };
        let core: Vec<f32> = voiced[trim..voiced.len() - trim].iter().map(|&i| midi[i]).collect();
        let pitch = dsp::median(&core);
        let mut key = (pitch - tuning).round();
        if let Some((root, scale)) = opts.key {
            let raw = pitch - tuning;
            let mut best = key;
            let mut bd = f32::MAX;
            for cand in [key - 1.0, key, key + 1.0] {
                if cand >= 0.0 && scale.contains(root, cand as u8) && (cand - raw).abs() < bd {
                    bd = (cand - raw).abs();
                    best = cand;
                }
            }
            key = best;
        }
        let p: f32 = (a..b).map(|i| 10f32.powf(track.level_db[i] / 10.0)).sum::<f32>() / (b - a) as f32;
        let confidence = dsp::mean(&voiced.iter().map(|&i| track.confidence[i]).collect::<Vec<_>>());
        let ev = NoteEvent {
            start: a as f32 * hop,
            end: b as f32 * hop,
            pitch,
            key: key.clamp(0.0, 127.0) as u8,
            level_db: dsp::pow_db(p),
            confidence,
        };
        // A pitch-only split that rounds to the same key was vibrato or drift: merge back.
        if pitch_only {
            if let Some((last, _)) = events.last_mut() {
                if last.key == ev.key && ev.start - last.end < 2.5 * hop {
                    last.end = ev.end;
                    continue;
                }
            }
        }
        events.push((ev, pitch_only));
    }
    let mut events: Vec<NoteEvent> = events.into_iter().map(|e| e.0).collect();
    // Glide residue: a sliver at the old pitch at the start of a slide into
    // the next note, glued to both neighbours.
    let mut i = 1;
    while i + 1 < events.len() {
        let e = &events[i];
        let glued = events[i + 1].start - e.end < 1.5 * hop;
        let residue = e.end - e.start < 0.09 && events[i - 1].key == e.key && e.start - events[i - 1].end < 3.5 * hop;
        // The tracker holding the sub-octave of a leap up an octave for a few frames.
        let late_leap = e.end - e.start < 0.15 && events[i + 1].key as i32 - e.key as i32 == 12;
        if glued && (residue || late_leap) {
            let start = e.start;
            events[i + 1].start = start;
            events.remove(i);
            continue;
        }
        i += 1;
    }
    if opts.octave_fix {
        octave_fix(&mut events);
    } else {
        lone_octave_fix(&mut events);
    }
    (events, tuning)
}

/// For voices: a note an octave-ish (10+ semitones) under both of its
/// neighbours, which sit close to each other and to it in time, is a period
/// doubling in a breathy frame; a real leap down comes back up less neatly.
fn lone_octave_fix(events: &mut [NoteEvent]) {
    for i in 1..events.len().saturating_sub(1) {
        let (p, n) = (events[i - 1].key as i32, events[i + 1].key as i32);
        let k = events[i].key as i32;
        let close = events[i].start - events[i - 1].end < 0.3 && events[i + 1].start - events[i].end < 0.3;
        if close && p - k >= 10 && n - k >= 10 && (p - n).abs() <= 4 && k + 12 <= 127 {
            events[i].key += 12;
            events[i].pitch += 12.0;
        }
    }
}

/// A note 8.5 semitones or more under the line around it (the median of the
/// notes within two seconds) is almost always a period-doubling error: move it
/// up an octave. Likewise more than 15 above: down.
fn octave_fix(events: &mut [NoteEvent]) {
    let keys: Vec<(f32, f32)> = events.iter().map(|e| (0.5 * (e.start + e.end), e.key as f32)).collect();
    for (i, e) in events.iter_mut().enumerate() {
        let t = keys[i].0;
        let near: Vec<f32> = keys.iter().enumerate().filter(|(j, k)| *j != i && (k.0 - t).abs() < 2.0).map(|(_, k)| k.1).collect();
        if near.len() < 3 {
            continue;
        }
        let m = dsp::median(&near);
        let k = e.key as f32;
        if k <= m - 8.5 && k + 12.0 <= 127.0 {
            e.key += 12;
            e.pitch += 12.0;
        } else if k > m + 15.0 && k >= 12.0 {
            e.key -= 12;
            e.pitch -= 12.0;
        }
    }
}

/// Notes at `opts.tempo`: (quantised, unquantised, origin seconds).
pub fn quantise(events: &[NoteEvent], opts: &TranscribeOpts) -> (Vec<Note>, Vec<Note>, f32) {
    let tps = opts.tempo.max(1.0) / 60.0 * PPQ as f32;
    let grid = opts.grid.max(1) as f32;
    let mut origin = opts.origin;
    if opts.align_first {
        if let Some(e) = events.first() {
            origin = e.start;
        }
    }
    let levels: Vec<f32> = events.iter().map(|e| e.level_db).collect();
    let mid = dsp::median(&levels);
    let vel = |db: f32| (100.0 + (db - mid) * 2.5).round().clamp(30.0, 127.0) as u8;
    let mut q: Vec<Note> = Vec::new();
    let mut raw: Vec<Note> = Vec::new();
    for e in events {
        let s = (e.start - origin) * tps;
        let en = (e.end - origin) * tps;
        if s < -grid * 0.5 {
            continue;
        }
        let v = vel(e.level_db);
        raw.push(Note(s.max(0.0).round() as u32, (en - s).round().max(1.0) as u32, e.key, v));
        let qs = ((s / grid).round() * grid).max(0.0) as u32;
        let qe = ((en / grid).round() * grid) as u32;
        let len = qe.saturating_sub(qs).max(grid as u32);
        if let Some(last) = q.last_mut() {
            if last.0 == qs {
                // Two notes on one step: keep the longer.
                if len > last.1 {
                    *last = Note(qs, len, e.key, v);
                }
                continue;
            }
            if last.0 + last.1 > qs {
                last.1 = qs - last.0;
            }
        }
        q.push(Note(qs, len, e.key, v));
    }
    (q, raw, origin)
}

/// A pattern of `notes`, long enough to hold them (whole bars of `beats_per_bar`).
pub fn to_pattern(name: &str, notes: &[Note], beats_per_bar: u32) -> mc_music::Pattern {
    let end = notes.iter().map(|n| n.end()).max().unwrap_or(0);
    let bar = beats_per_bar.max(1) * PPQ;
    let bars = end.div_ceil(bar).max(1);
    mc_music::Pattern { name: name.to_string(), beats: bars * beats_per_bar, notes: notes.to_vec(), automation: Vec::new() }
}

/// The pattern as RON, the way song files write it.
pub fn pattern_ron(p: &mc_music::Pattern) -> String {
    let config = ron::ser::PrettyConfig::new().struct_names(false).compact_arrays(true);
    ron::ser::to_string_pretty(p, config).unwrap_or_default()
}
