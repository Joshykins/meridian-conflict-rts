//! A guess at the melody: the most salient pitched line with its fundamental
//! between ~260 Hz and 2 kHz. Per frame, every spectral peak votes for the
//! fundamentals it could be a harmonic of (harmonic sum, 0.8^h weights); the
//! winner is kept when it clearly stands out, then the frames are segmented
//! into notes exactly like humming. Best effort: pads and chords in the same
//! range confuse it, so it carries its own confidence.

use crate::features::FramesB;
use crate::harmony::Grid;
use crate::pitch::{self, F0Track, NoteEvent, TranscribeOpts};
use crate::{dsp, theory};
use mc_music::Note;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Melody {
    /// 0..1; below ~0.5 treat the notes as a sketch.
    pub confidence: f32,
    /// Share of the audio where a line was heard.
    pub voiced: f32,
    /// Notes at the detected tempo, tick 0 = the first downbeat.
    pub notes: Vec<Note>,
    pub events: Vec<NoteEvent>,
}

const LO: f32 = 60.0; // C4
const HI: f32 = 96.0; // C7
const RES: f32 = 10.0; // bins per semitone

pub fn guess(b: &FramesB, tuning: f32, grid: &Grid) -> Melody {
    let nbins = ((HI - LO) * RES) as usize + 1;
    let mut f0 = Vec::with_capacity(b.peaks.len());
    let mut conf = Vec::with_capacity(b.peaks.len());
    let mut level = Vec::with_capacity(b.peaks.len());
    let mut sal = vec![0.0f32; nbins];
    for fr in &b.peaks {
        sal.iter_mut().for_each(|s| *s = 0.0);
        let mut total = 0.0f32;
        let mut direct = vec![0.0f32; nbins];
        for p in fr {
            if p.hz < 250.0 || p.hz > 6000.0 {
                continue;
            }
            let a = p.mag.sqrt();
            total += a;
            for h in 1..=6 {
                let m = dsp::hz_to_midi(p.hz / h as f32) - tuning;
                if !(LO..=HI).contains(&m) {
                    continue;
                }
                let c = (m - LO) * RES;
                let w = a * 0.8f32.powi(h - 1);
                for k in -3i32..=3 {
                    let i = c.round() as i32 + k;
                    if i >= 0 && (i as usize) < nbins {
                        let tri = 1.0 - (i as f32 - c).abs() / 4.0;
                        if tri > 0.0 {
                            sal[i as usize] += w * tri;
                            if h == 1 {
                                direct[i as usize] += a * tri;
                            }
                        }
                    }
                }
            }
        }
        // The fundamental itself must be present (no phantom sub-octaves).
        let (mut bi, mut bv) = (0usize, 0.0f32);
        for i in 0..nbins {
            if direct[i] > 0.0 && sal[i] > bv {
                bv = sal[i];
                bi = i;
            }
        }
        level.push(dsp::amp_db(total / 20.0));
        let ratio = if total > 0.0 { bv / (total * 1.6) } else { 0.0 };
        if ratio > 0.35 && bv > 0.0 {
            f0.push(dsp::midi_to_hz(LO + bi as f32 / RES + tuning));
            conf.push(ratio.min(1.0));
        } else {
            f0.push(0.0);
            conf.push(0.0);
        }
    }
    let track = F0Track {
        hop: b.hop,
        f0,
        confidence: conf,
        level_db: level,
    };
    let mut opts = TranscribeOpts::voice(grid.bpm);
    opts.min_note = 0.09;
    opts.adapt_tuning = false;
    opts.origin = grid.downbeat;
    let (events, _) = pitch::segment(&track, &opts);
    let (notes, _, _) = pitch::quantise(&events, &opts);
    let voiced_frames = track.f0.iter().filter(|&&f| f > 0.0).count();
    let voiced = voiced_frames as f32 / track.f0.len().max(1) as f32;
    let mean_conf = dsp::mean(
        &track
            .confidence
            .iter()
            .copied()
            .filter(|&c| c > 0.0)
            .collect::<Vec<_>>(),
    );
    Melody {
        confidence: (mean_conf * voiced.sqrt() * 1.5).clamp(0.0, 1.0),
        voiced,
        notes,
        events,
    }
}

/// "rises to a peak then falls" style description of a note line.
pub fn contour(keys: &[u8]) -> String {
    if keys.len() < 2 {
        return "single note".into();
    }
    let lo = *keys.iter().min().unwrap();
    let hi = *keys.iter().max().unwrap();
    let peak_at = keys.iter().position(|&k| k == hi).unwrap() as f32 / (keys.len() - 1) as f32;
    let first = keys[0] as i32;
    let last = *keys.last().unwrap() as i32;
    let steps: Vec<i32> = keys.windows(2).map(|w| w[1] as i32 - w[0] as i32).collect();
    let leaps = steps.iter().filter(|s| s.abs() > 2).count();
    let repeats = steps.iter().filter(|s| **s == 0).count();
    let shape = if hi - lo <= 2 {
        "flat (near one pitch)"
    } else if peak_at > 0.2 && peak_at < 0.8 && (hi as i32 - first) >= 3 && (hi as i32 - last) >= 3
    {
        "arch (rises to a peak, falls back)"
    } else if last - first >= 3 {
        "rising"
    } else if first - last >= 3 {
        "falling"
    } else {
        "wave (up and down around a centre)"
    };
    format!(
        "{shape}; range {} semitones; {}% steps, {}% leaps, {}% repeats",
        hi - lo,
        (100 * (steps.len() - leaps - repeats) / steps.len()),
        100 * leaps / steps.len(),
        100 * repeats / steps.len()
    )
}

pub fn names(notes: &[Note], flats: bool) -> String {
    notes
        .iter()
        .map(|n| theory::note_name(n.key(), flats))
        .collect::<Vec<_>>()
        .join(" ")
}
