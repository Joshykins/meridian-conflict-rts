//! Key and chords from chroma, on the beat grid.

use crate::dsp;
use crate::theory::{self, Chord, Key};
use serde::{Deserialize, Serialize};

/// The bar/beat grid everything is laid on.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Grid {
    pub bpm: f32,
    pub beats_per_bar: u32,
    /// Seconds of the first downbeat (bar 1 starts here; anything earlier is a pickup).
    pub downbeat: f32,
    /// Whole bars in the analysed audio.
    pub bars: usize,
}

impl Grid {
    pub fn beat(&self) -> f32 {
        60.0 / self.bpm.max(1.0)
    }
    pub fn bar_len(&self) -> f32 {
        self.beat() * self.beats_per_bar as f32
    }
    /// Start of bar `b` (0-based), seconds.
    pub fn bar_start(&self, b: usize) -> f32 {
        self.downbeat + b as f32 * self.bar_len()
    }
    /// Sixteenth steps per bar.
    pub fn steps(&self) -> usize {
        self.beats_per_bar as usize * 4
    }
    pub fn step_len(&self) -> f32 {
        self.beat() / 4.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyReport {
    pub key: Key,
    pub name: String,
    /// 0..1.
    pub confidence: f32,
    pub runner_up: String,
    pub runner_up_score: f32,
    pub score: f32,
    /// Offset of the recording from A440, cents.
    pub tuning_cents: f32,
    /// Strength of each pitch class C..B relative to the strongest (1.0).
    pub pitch_classes: [f32; 12],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChordAt {
    /// Beats into the bar.
    pub beat: f32,
    pub chord: Option<Chord>,
    /// "Cm", "Ab", "G7"; "N" for no chord.
    pub name: String,
    /// 0..1 template match.
    pub score: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BarChords {
    /// 1-based bar number.
    pub bar: usize,
    pub start: f32,
    pub chords: Vec<ChordAt>,
}

fn sum_frames(ch: &[[f32; 12]], hop: f32, a: f32, b: f32) -> [f32; 12] {
    let mut s = [0.0f32; 12];
    // Skip the smeared edges of the long analysis window.
    let i0 = ((a + 0.07) / hop).ceil().max(0.0) as usize;
    let i1 = (((b - 0.05) / hop).floor().max(0.0) as usize + 1).min(ch.len());
    for c in ch.iter().take(i1).skip(i0) {
        for k in 0..12 {
            s[k] += c[k];
        }
    }
    s
}

/// Chords bar by bar, splitting a bar into halves or beats when the harmony
/// clearly changes inside it. Names are spelled with sharps until the key is
/// known (see `respell`).
pub fn chords(chroma: &[[f32; 12]], bass: &[[f32; 12]], hop: f32, grid: &Grid) -> Vec<BarChords> {
    let tpl = theory::templates(true);
    let energy: Vec<f32> = (0..grid.bars)
        .map(|b| sum_frames(chroma, hop, grid.bar_start(b), grid.bar_start(b + 1)).iter().sum())
        .collect();
    let typical = dsp::median(&energy).max(1e-9);
    let best = |a: f32, b: f32| -> (Option<Chord>, f32) {
        let c = sum_frames(chroma, hop, a, b);
        if c.iter().sum::<f32>() / (b - a).max(1e-3) < 0.08 * typical / grid.bar_len() {
            return (None, 0.0);
        }
        let bs = sum_frames(bass, hop, a, b);
        let m = theory::match_chords(&c, Some(&bs), &tpl);
        (Some(m[0].0), m[0].1)
    };
    let beat = grid.beat();
    let bpb = grid.beats_per_bar as usize;
    let mut out = Vec::new();
    for b in 0..grid.bars {
        let s = grid.bar_start(b);
        let parts = split_bar(bpb, &|x, y| best(s + x * beat, s + y * beat));
        out.push(BarChords {
            bar: b + 1,
            start: s,
            chords: parts
                .into_iter()
                .map(|(bt, c, sc)| ChordAt {
                    beat: bt,
                    chord: c,
                    name: c.map(|c| c.name(false)).unwrap_or_else(|| "N".into()),
                    score: sc,
                })
                .collect(),
        });
    }
    out
}

/// One bar's chords: the best chord for the whole bar, or for each half, or
/// for each beat, whichever is clearly the better fit. `best(from_beat, to_beat)`
/// scores a span. Returns (beat, chord, score).
pub fn split_bar(bpb: usize, best: &dyn Fn(f32, f32) -> (Option<Chord>, f32)) -> Vec<(f32, Option<Chord>, f32)> {
    let whole = best(0.0, bpb as f32);
    let mut parts: Vec<(f32, (Option<Chord>, f32))> = vec![(0.0, whole)];
    if bpb >= 2 && bpb.is_multiple_of(2) {
        let h = (bpb / 2) as f32;
        let (h1, h2) = (best(0.0, h), best(h, bpb as f32));
        if h1.0 != h2.0 && h1.0.is_some() && h2.0.is_some() && (h1.1 + h2.1) * 0.5 > whole.1 + 0.03 && h1.1.min(h2.1) > 0.7 {
            parts = vec![(0.0, h1), (h, h2)];
            // Faster still: one per beat when that is clearly better again.
            let per: Vec<(Option<Chord>, f32)> = (0..bpb).map(|k| best(k as f32, (k + 1) as f32)).collect();
            let mean_beat = per.iter().map(|p| p.1).sum::<f32>() / bpb as f32;
            let distinct = per.windows(2).filter(|w| w[0].0 != w[1].0).count();
            if mean_beat > (h1.1 + h2.1) * 0.5 + 0.04 && distinct >= 2 && per.iter().all(|p| p.1 > 0.7) {
                parts = per.into_iter().enumerate().map(|(k, p)| (k as f32, p)).collect();
                parts.dedup_by(|b2, a2| a2.1 .0 == b2.1 .0);
            }
        }
    }
    parts.into_iter().map(|(b, (c, s))| (b, c, s)).collect()
}

/// Key from the chroma histogram, re-ranked among the best few by how well the
/// bar chords fit it (diatonic chords, the tonic chord first/last/most often).
pub fn key(chroma: &[[f32; 12]], bass: &[[f32; 12]], bars: &[BarChords], tuning: f32) -> KeyReport {
    let mut hist = [0.0f32; 12];
    for (c, b) in chroma.iter().zip(bass) {
        let s: f32 = c.iter().sum();
        let sb: f32 = b.iter().sum();
        if s > 1e-9 {
            for k in 0..12 {
                hist[k] += c[k] / s * s.sqrt();
            }
        }
        if sb > 1e-9 {
            for k in 0..12 {
                hist[k] += 0.5 * b[k] / sb * sb.sqrt();
            }
        }
    }
    let chords: Vec<Chord> = bars.iter().flat_map(|b| b.chords.iter().filter_map(|c| c.chord)).collect();
    // Raw chroma leans on the partials of synths (a saw's fifth and third
    // harmonics); the chords heard are a cleaner census of the pitch classes.
    let raw_sum: f32 = hist.iter().sum::<f32>().max(1e-9);
    let mut census = [0.0f32; 12];
    for b in bars {
        for (i, c) in b.chords.iter().enumerate() {
            let Some(ch) = c.chord else { continue };
            let next = b.chords.get(i + 1).map(|n| n.beat).unwrap_or(4.0);
            let w = (next - c.beat).max(0.5) * c.score.max(0.0);
            for (j, iv) in ch.quality.intervals().iter().enumerate() {
                census[((ch.root + iv) % 12) as usize] += w * if j == 0 { 1.3 } else { 1.0 };
            }
        }
    }
    let census_sum: f32 = census.iter().sum::<f32>();
    if census_sum > 0.0 {
        for k in 0..12 {
            hist[k] = 0.35 * hist[k] / raw_sum + 0.65 * census[k] / census_sum;
        }
    }
    let starts: Vec<Chord> = bars.iter().step_by(4).filter_map(|b| b.chords.first().and_then(|c| c.chord)).collect();
    let ((k, s), (k2, s2)) = choose_key(&hist, &chords, &starts);
    let m = hist.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    KeyReport {
        key: k,
        name: k.name(),
        confidence: ((s - s2) * 4.0 + 0.3 * s).clamp(0.0, 1.0),
        runner_up: k2.name(),
        runner_up_score: s2,
        score: s,
        tuning_cents: tuning * 100.0,
        pitch_classes: hist.map(|h| ((h / m) * 100.0).round() / 100.0),
    }
}

/// The best key and the runner-up for a pitch-class histogram and the chords
/// heard: profile correlation, re-ranked by how the chords
/// fit (diatonic share; the tonic chord first, last and most often).
/// `phrase_starts` are the chords at the top of each 4-bar phrase: the one heard
/// there most often counts as where the music starts from.
pub fn choose_key(hist: &[f32; 12], chords: &[Chord], phrase_starts: &[Chord]) -> ((Key, f32), (Key, f32)) {
    let mut all: Vec<(Key, f32)> = theory::key_scores(hist).into_iter().map(|(k, _)| (k, key_fit(hist, chords, phrase_starts, k))).collect();
    all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    (all[0], all[1])
}

/// How well one key explains the histogram and the chords (the score `choose_key` ranks by).
pub fn key_fit(hist: &[f32; 12], chords: &[Chord], phrase_starts: &[Chord], key: Key) -> f32 {
    let profile = theory::key_scores(hist).into_iter().find(|(k, _)| *k == key).map(|(_, s)| s).unwrap_or(0.0);
    profile + chord_fit(chords, phrase_starts, &key)
}

fn chord_fit(chords: &[Chord], phrase_starts: &[Chord], k: &Key) -> f32 {
    let mut counts: std::collections::HashMap<Chord, usize> = Default::default();
    for c in chords {
        *counts.entry(*c).or_default() += 1;
    }
    let most = counts.iter().max_by(|a, b| a.1.cmp(b.1).then((b.0.root, b.0.quality as u8).cmp(&(a.0.root, a.0.quality as u8)))).map(|(c, _)| *c);
    let first = {
        let mut best: Option<(Chord, usize)> = None;
        for c in phrase_starts {
            let n = phrase_starts.iter().filter(|x| *x == c).count();
            if best.map(|b| n > b.1).unwrap_or(true) {
                best = Some((*c, n));
            }
        }
        best.map(|b| b.0).or(chords.first().copied())
    };
    let last = chords.last().copied();
    let fit = |k: &Key| -> f32 {
        let steps = k.steps();
        let diatonic = chords
            .iter()
            .filter(|c| {
                c.quality.intervals().iter().all(|iv| steps.contains(&(((c.root + iv) as i32 - k.root as i32).rem_euclid(12) as u8)))
                    || (k.minor && (c.root + 12 - k.root) % 12 == 7)
            })
            .count() as f32
            / chords.len() as f32;
        let tonic = |c: Option<Chord>| -> f32 {
            match c {
                Some(c) if c.root == k.root && c.quality.intervals().contains(&(if k.minor { 3 } else { 4 })) => 1.0,
                Some(c) if c.root == k.root => 0.5,
                _ => 0.0,
            }
        };
        // A loop's first chord is usually home: it breaks the relative major/minor tie.
        0.25 * diatonic + 0.35 * tonic(first) + 0.06 * (tonic(last) + tonic(most))
    };
    if chords.is_empty() {
        0.0
    } else {
        fit(k)
    }
}

/// Spell the chord names for the key (flats in flat keys).
pub fn respell(bars: &mut [BarChords], key: &Key) {
    for b in bars.iter_mut() {
        for c in b.chords.iter_mut() {
            if let Some(ch) = c.chord {
                c.name = ch.name(key.flats());
            }
        }
    }
}
