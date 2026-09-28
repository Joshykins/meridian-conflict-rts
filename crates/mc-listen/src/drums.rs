//! The drum grid: band onsets read on the sixteenth grid.
//!
//! Kick = 30-110 Hz, snare = 1.8-6 kHz noise (with its 150-400 Hz body as a
//! second opinion), hats = above 7 kHz. Each band's level is followed at 1 ms;
//! a hit is a rise over the level 5-25 ms earlier. The grid is slid by up to
//! half a sixteenth to where the rises line up best (engine latency, a late
//! band), then each lane's rises are split into hits and non-hits by the
//! biggest gap in their distribution (Otsu), so no fixed threshold decides.
//! Written as the song scripts write drums: 'X' strong, 'x' normal, 'o' soft,
//! 'g' ghost, '.' rest.

use crate::dsp;
use crate::harmony::Grid;
use serde::{Deserialize, Serialize};

pub const LANES: [&str; 3] = ["kick", "snare", "hat"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrumBar {
    pub bar: usize,
    /// kick, snare, hat.
    pub lanes: [String; 3],
    /// Share of the bar's cells that match the most common bar, 0..1.
    pub consistency: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DrumReport {
    pub bars: Vec<DrumBar>,
    /// The most common bar and how many bars match it (hits, ignoring accents).
    pub common: [String; 3],
    pub common_count: usize,
    /// The most common two-bar phrase (bars 1-2, 3-4, ...).
    pub common2: [String; 3],
    pub common2_count: usize,
    /// How far the heard grid sits from the beat grid, ms (positive = late).
    pub offset_ms: f32,
    /// Seconds of each kick hit (used for pump detection).
    pub kick_times: Vec<f32>,
    /// Seconds of each snare hit.
    pub snare_times: Vec<f32>,
    /// Which lanes have any hits at all.
    pub present: [bool; 3],
}

struct Lane {
    level: Vec<f32>,
    rise: Vec<f32>,
}

/// Seconds per envelope block (about 1 ms; exact, so long songs do not drift).
fn block_secs(rate: u32) -> f32 {
    (rate as f32 / 1000.0).round().max(1.0) / rate as f32
}

/// A band's level (dB, ~1 ms blocks over `win_ms`) and its rise over the lowest level `back` ms (from..to) before.
fn lane(x: &[f32], rate: u32, lo: f32, hi: f32, win_ms: f32, back: (usize, usize)) -> Lane {
    let y = dsp::band(x, rate, lo, hi);
    let block = (rate as f32 / 1000.0).round().max(1.0) as usize;
    let level = dsp::rms_db_blocks(&y, block, (win_ms * rate as f32 / 1000.0) as usize);
    let rise = (0..level.len())
        .map(|t| {
            let a = t.saturating_sub(back.0);
            let b = t.saturating_sub(back.1).max(a + 1);
            let before = level[a..b].iter().cloned().fold(f32::MAX, f32::min);
            (level[t] - before).max(0.0)
        })
        .collect();
    Lane { level, rise }
}

fn max_in(v: &[f32], c: isize, w: isize) -> f32 {
    let a = (c - w).max(0) as usize;
    let b = ((c + w + 1).max(0) as usize).min(v.len());
    if a >= b {
        return 0.0;
    }
    v[a..b].iter().cloned().fold(f32::MIN, f32::max)
}

/// Otsu split: the threshold that best separates the values into two groups,
/// and the gap between the groups' means.
fn otsu(v: &[f32]) -> (f32, f32) {
    let mut s: Vec<f32> = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    if n < 4 {
        return (f32::MAX, 0.0);
    }
    let total: f32 = s.iter().sum();
    let mut best = (0.0f32, f32::MAX, 0.0f32);
    let mut acc = 0.0;
    for i in 1..n {
        acc += s[i - 1];
        let w0 = i as f32 / n as f32;
        let m0 = acc / i as f32;
        let m1 = (total - acc) / (n - i) as f32;
        let between = w0 * (1.0 - w0) * (m1 - m0) * (m1 - m0);
        if between > best.0 && s[i] > s[i - 1] {
            best = (between, 0.5 * (s[i - 1] + s[i]), m1 - m0);
        }
    }
    (best.1, best.2)
}

/// `flux_low` / `hop`: the band-wise spectral flux below 150 Hz from the
/// feature pass. It hears a kick's falling sweep even under a held bass note,
/// where the plain level of the low band barely moves.
pub fn analyse(x: &[f32], rate: u32, grid: &Grid, flux_low: &[f32], hop: f32) -> DrumReport {
    let lanes = [
        lane(x, rate, 30.0, 110.0, 12.0, (25, 5)),
        lane(x, rate, 1800.0, 6000.0, 6.0, (25, 5)),
        // Hats: short windows, so a tick shows even over a snare's noisy tail.
        lane(x, rate, 7000.0, 0.0, 2.0, (10, 2)),
    ];
    let body = lane(x, rate, 150.0, 400.0, 8.0, (25, 5));
    let steps = grid.steps();
    let n_steps = grid.bars * steps;
    let bs = block_secs(rate);
    let ms = |t: f32| (t / bs).round() as isize;
    let step_t = |i: usize| grid.downbeat + i as f32 * grid.step_len();
    // Slide the grid to where the rises line up.
    let half = (grid.step_len() * 1000.0 * 0.45) as isize;
    let mut best = (f32::MIN, 0isize);
    for o in -half..=half {
        let mut tot = 0.0;
        for i in 0..n_steps {
            let c = ms(step_t(i)) + o;
            for l in &lanes {
                tot += max_in(&l.rise, c, 4);
            }
        }
        // Prefer the smallest shift among near-equals.
        let tot = tot * (1.0 - 0.0005 * o.abs() as f32);
        if tot > best.0 {
            best = (tot, o);
        }
    }
    let off = best.1;
    let w = ((grid.step_len() * 1000.0 * 0.3) as isize).clamp(4, 20);
    let hat_neg: Vec<f32> = lanes[2].level.iter().map(|v| -v).collect();
    let mut strength = vec![vec![0.0f32; n_steps]; 3];
    let mut level = vec![vec![-120.0f32; n_steps]; 3];
    for i in 0..n_steps {
        let c = ms(step_t(i)) + off;
        for (k, l) in lanes.iter().enumerate() {
            strength[k][i] = max_in(&l.rise, c, w);
            level[k][i] = max_in(&l.level, c + 10, 15);
        }
        // Kick: the low-band flux (dB summed over the bands), scaled to read like a rise.
        let fc = (step_t(i) + off as f32 * bs) / hop;
        let fw = (w as f32 * bs / hop).ceil().max(1.0) as isize;
        strength[0][i] = (fc.round() as isize - 1..=fc.round() as isize + fw)
            .filter(|&j| j >= 0 && (j as usize) < flux_low.len())
            .map(|j| flux_low[j as usize])
            .fold(0.0, f32::max)
            * 0.5;
        // Hats also by contrast with where their tick has died away (a third of
        // a step later): a tick on top of a snare's slow tail barely rises, but it does fall.
        let later = c + (grid.step_len() / bs * 0.33) as isize;
        let pk = max_in(&lanes[2].level, c, w);
        let lo = -max_in(&hat_neg, later, 3);
        strength[2][i] = strength[2][i].max((pk - lo) * 1.5);
        // The snare's body backs up a weak noise rise.
        strength[1][i] += 0.25 * max_in(&body.rise, c, w);
    }
    let mut hit = vec![vec![false; n_steps]; 3];
    let min_rise = [6.0f32, 5.0, 5.0];
    let mut present = [false; 3];
    for k in 0..3 {
        // A few huge values (the very first hit out of silence) must not own the split.
        let cap = dsp::percentile(&strength[k], 0.97);
        let capped: Vec<f32> = strength[k].iter().map(|v| v.min(cap)).collect();
        let (thr, gap) = otsu(&capped);
        let loud = dsp::percentile(&level[k], 0.95);
        if gap < 5.0 {
            continue;
        }
        for i in 0..n_steps {
            hit[k][i] =
                strength[k][i] > thr && strength[k][i] >= min_rise[k] && level[k][i] > loud - 30.0;
        }
        present[k] = hit[k].iter().any(|&h| h);
    }
    // Accent marks from each lane's level relative to its typical hit.
    let mut sym = vec![vec!['.'; n_steps]; 3];
    for k in 0..3 {
        let lv: Vec<f32> = (0..n_steps)
            .filter(|&i| hit[k][i])
            .map(|i| level[k][i])
            .collect();
        let med = dsp::median(&lv);
        for i in 0..n_steps {
            if hit[k][i] {
                let rel = level[k][i] - med;
                sym[k][i] = if rel >= 3.0 {
                    'X'
                } else if rel <= -12.0 {
                    'g'
                } else if rel <= -6.0 {
                    'o'
                } else {
                    'x'
                };
            }
        }
    }
    let bar_str =
        |k: usize, b: usize| -> String { sym[k][b * steps..(b + 1) * steps].iter().collect() };
    let mask = |s: &str| -> String {
        s.chars()
            .map(|c| if c == '.' { '.' } else { 'x' })
            .collect()
    };
    let mut rep = DrumReport {
        offset_ms: off as f32 * bs * 1000.0,
        present,
        ..Default::default()
    };
    let masks: Vec<String> = (0..grid.bars)
        .map(|b| {
            (0..3)
                .map(|k| mask(&bar_str(k, b)))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect();
    let mut counts: std::collections::HashMap<&str, usize> = Default::default();
    for m in &masks {
        *counts.entry(m.as_str()).or_default() += 1;
    }
    let common_mask = counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(m, _)| m.to_string())
        .unwrap_or_default();
    rep.common_count = counts.get(common_mask.as_str()).copied().unwrap_or(0);
    if let Some(b) = masks.iter().position(|m| *m == common_mask) {
        rep.common = [bar_str(0, b), bar_str(1, b), bar_str(2, b)];
    }
    let pairs: Vec<String> = (0..grid.bars / 2)
        .map(|p| format!("{}#{}", masks[2 * p], masks[2 * p + 1]))
        .collect();
    let mut pc: std::collections::HashMap<&str, usize> = Default::default();
    for p in &pairs {
        *pc.entry(p.as_str()).or_default() += 1;
    }
    if let Some((m, n)) = pc.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) {
        rep.common2_count = *n;
        if let Some(p) = pairs.iter().position(|x| x == m) {
            rep.common2 =
                [0, 1, 2].map(|k| format!("{}{}", bar_str(k, 2 * p), bar_str(k, 2 * p + 1)));
        }
    }
    let cm: Vec<char> = common_mask.chars().filter(|c| *c != '|').collect();
    for (b, mask) in masks.iter().enumerate() {
        let m: Vec<char> = mask.chars().filter(|c| *c != '|').collect();
        let same = m.iter().zip(&cm).filter(|(a, b)| a == b).count();
        rep.bars.push(DrumBar {
            bar: b + 1,
            lanes: [bar_str(0, b), bar_str(1, b), bar_str(2, b)],
            consistency: same as f32 / m.len().max(1) as f32,
        });
    }
    rep.kick_times = (0..n_steps)
        .filter(|&i| hit[0][i])
        .map(|i| step_t(i) + off as f32 * bs)
        .collect();
    rep.snare_times = (0..n_steps)
        .filter(|&i| hit[1][i])
        .map(|i| step_t(i) + off as f32 * bs)
        .collect();
    rep
}
