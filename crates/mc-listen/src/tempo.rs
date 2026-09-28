//! Tempo and beat grid from the onset-strength envelope.
//!
//! Coarse tempo: autocorrelation of the onset envelope, weighted by a gentle
//! prior around 120 bpm. Fine tempo and phase: "epoch folding" — the envelope
//! folded at a trial period gives a sharp peak only at the true period, which
//! pins the tempo to a small fraction of a bpm over a whole song. Downbeat:
//! of the beats in a bar, the one with the most low-end onsets and the most
//! harmonic change after it.

use crate::dsp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tempo {
    pub bpm: f32,
    /// 0..1: how periodic the onsets are at that tempo.
    pub confidence: f32,
    /// Other readings: (bpm, score relative to the chosen one).
    pub candidates: Vec<(f32, f32)>,
    /// Time of one beat (the phase), seconds.
    pub beat_phase: f32,
}

/// Onset envelope ready for periodicity: local mean removed, half-wave rectified, unit peak.
pub fn prepare(flux: &[f32], hop: f32) -> Vec<f32> {
    let avg = dsp::moving_average(flux, (1.0 / hop) as usize | 1);
    let mut e: Vec<f32> = flux
        .iter()
        .zip(&avg)
        .map(|(f, a)| (f - a).max(0.0))
        .collect();
    let p = dsp::percentile(&e, 0.995).max(1e-9);
    for v in e.iter_mut() {
        *v = (*v / p).min(1.5);
    }
    e
}

fn prior(bpm: f32) -> f32 {
    // Log-normal around 120 bpm, one octave wide: gentle, it only breaks near-ties.
    let x = (bpm / 120.0).log2();
    (-0.5 * (x / 0.9).powi(2)).exp()
}

/// Epoch folding: mean envelope per phase bin at period `p` frames; returns (sharpness, phase in frames).
fn fold(env: &[f32], p: f32) -> (f32, f32) {
    const BINS: usize = 48;
    let mut sum = [0.0f32; BINS];
    let mut cnt = [0u32; BINS];
    for (i, &v) in env.iter().enumerate() {
        let ph = (i as f32 / p).fract();
        let b = ((ph * BINS as f32) as usize).min(BINS - 1);
        sum[b] += v;
        cnt[b] += 1;
    }
    let prof: Vec<f32> = (0..BINS)
        .map(|b| {
            if cnt[b] > 0 {
                sum[b] / cnt[b] as f32
            } else {
                0.0
            }
        })
        .collect();
    let mean = dsp::mean(&prof).max(1e-9);
    // Three-bin smoothing so the peak is not a single lucky bin.
    let mut best = 0.0;
    let mut arg = 0;
    for b in 0..BINS {
        let v = prof[(b + BINS - 1) % BINS] * 0.25 + prof[b] * 0.5 + prof[(b + 1) % BINS] * 0.25;
        if v > best {
            best = v;
            arg = b;
        }
    }
    // Sub-bin phase from the neighbours.
    let (a, c) = (prof[(arg + BINS - 1) % BINS], prof[(arg + 1) % BINS]);
    let den = a - 2.0 * prof[arg] + c;
    let off = if den.abs() > 1e-9 {
        (0.5 * (a - c) / den).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    (best / mean, ((arg as f32 + 0.5 + off) / BINS as f32) * p)
}

/// Tempo from a prepared onset envelope at `hop` seconds per frame.
/// `hint` narrows the search to within 8% of a known tempo.
/// `low` is the same envelope for the low band (kicks): when the kick pulses
/// at twice the chosen tempo (four on the floor), that faster pulse is the beat.
pub fn estimate(env: &[f32], low: Option<&[f32]>, hop: f32, hint: Option<f32>) -> Tempo {
    if env.len() < 16 {
        return Tempo {
            bpm: 120.0,
            ..Default::default()
        };
    }
    let ac = dsp::autocorrelation(env);
    let at = |bpm: f32| -> f32 {
        let lag = 60.0 / (bpm * hop);
        if lag + 1.0 >= ac.len() as f32 {
            0.0
        } else {
            dsp::lerp_at(&ac, lag).max(0.0)
        }
    };
    // Peak-enhanced score: the lag and its double both line up for a true beat.
    let score = |bpm: f32| at(bpm) + 0.35 * at(bpm / 2.0);
    let (lo, hi) = match hint {
        Some(h) => (h * 0.92, h * 1.08),
        None => (50.0, 220.0),
    };
    let mut best = (0.0f32, 120.0f32);
    let mut bpm = lo;
    let mut curve = Vec::new();
    while bpm <= hi {
        let s = score(bpm) * if hint.is_some() { 1.0 } else { prior(bpm) };
        curve.push((bpm, s));
        if s > best.0 {
            best = (s, bpm);
        }
        bpm += 0.25;
    }
    if let (Some(low), None) = (low, hint) {
        let acl = dsp::autocorrelation(low);
        let atl = |bpm: f32| dsp::lerp_at(&acl, 60.0 / (bpm * hop)).max(0.0);
        let double = best.1 * 2.0;
        if double <= 185.0 && atl(double) > 0.3 && atl(double) >= 0.6 * atl(best.1) {
            best.1 = double;
        }
    }
    // Fine tempo by folding around the coarse peak.
    let p0 = 60.0 / (best.1 * hop);
    let mut fine = (0.0f32, p0, 0.0f32);
    let steps = 240;
    for k in 0..=steps {
        let p = p0 * (0.985 + 0.03 * k as f32 / steps as f32);
        let (s, ph) = fold(env, p);
        if s > fine.0 {
            fine = (s, p, ph);
        }
    }
    let bpm = 60.0 / (fine.1 * hop);
    let base = score(best.1).max(1e-6);
    let mut candidates = Vec::new();
    for mult in [0.5f32, 2.0, 2.0 / 3.0, 1.5] {
        let b = bpm * mult;
        if (40.0..=300.0).contains(&b) {
            candidates.push((
                (b * 10.0).round() / 10.0,
                (score(b) * prior(b) / (base * prior(best.1))).min(2.0),
            ));
        }
    }
    Tempo {
        bpm,
        confidence: (at(bpm) * 1.4).clamp(0.0, 1.0),
        candidates,
        beat_phase: fine.2 * hop,
    }
}

/// Which beat of the bar is the downbeat: the most low-end onsets on it and the
/// most harmonic change across it, the fewest mid (snare) onsets.
/// Returns the time of the first downbeat at or after -0.05 s.
pub fn downbeat(
    tempo: &Tempo,
    beats_per_bar: u32,
    hop_a: f32,
    flux_low: &[f32],
    flux_mid: &[f32],
    chroma: &[[f32; 12]],
    hop_b: f32,
    duration: f32,
) -> f32 {
    let beat = 60.0 / tempo.bpm.max(1.0);
    let bpb = beats_per_bar.max(1) as usize;
    let mut first = tempo.beat_phase % beat;
    if first > beat - 0.05 {
        first -= beat;
    }
    let n = ((duration - first) / beat).floor().max(0.0) as usize;
    let peak = |x: &[f32], t: f32| -> f32 {
        let c = (t / hop_a).round() as isize;
        (c - 3..=c + 3)
            .filter(|&i| i >= 0 && (i as usize) < x.len())
            .map(|i| x[i as usize])
            .fold(0.0, f32::max)
    };
    let avg_chroma = |a: f32, b: f32| -> [f32; 12] {
        let mut s = [0.0f32; 12];
        let i0 = (a / hop_b).max(0.0) as usize;
        let i1 = ((b / hop_b) as usize).min(chroma.len());
        for c in chroma.iter().take(i1).skip(i0) {
            for k in 0..12 {
                s[k] += c[k];
            }
        }
        s
    };
    let mut low = vec![0.0f32; bpb];
    let mut mid = vec![0.0f32; bpb];
    let mut change = vec![0.0f32; bpb];
    let mut count = vec![0.0f32; bpb];
    for k in 0..n {
        let t = first + k as f32 * beat;
        let m = k % bpb;
        low[m] += peak(flux_low, t);
        mid[m] += peak(flux_mid, t);
        let before = avg_chroma(t - beat, t);
        let after = avg_chroma(t, t + beat);
        change[m] += 1.0 - dsp::cosine(&before, &after);
        count[m] += 1.0;
    }
    let norm = |v: &mut Vec<f32>| {
        for (x, c) in v.iter_mut().zip(&count) {
            *x /= c.max(1.0);
        }
        let m = dsp::mean(v).max(1e-9);
        for x in v.iter_mut() {
            *x /= m;
        }
    };
    norm(&mut low);
    norm(&mut mid);
    norm(&mut change);
    let mut best = (f32::MIN, 0usize);
    for m in 0..bpb {
        let s = low[m] - 0.4 * mid[m] + 1.5 * change[m];
        if s > best.0 {
            best = (s, m);
        }
    }
    let mut d = first + best.1 as f32 * beat;
    let bar = beat * bpb as f32;
    while d - bar >= -0.05 {
        d -= bar;
    }
    d
}
