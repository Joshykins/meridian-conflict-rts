//! Arrangement: where the sections change and which ones repeat.
//!
//! One feature vector per bar (chroma for harmony, cepstra for timbre, level
//! for energy), z-normalised across the song; a self-similarity matrix of the
//! bars; a checkerboard kernel slid down its diagonal gives a novelty curve
//! whose peaks are the section boundaries (with a small preference for
//! 4-bar lines). Sections are then labelled A, B, C... by similarity to the
//! first section of each label.

use crate::dsp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionInfo {
    pub label: String,
    /// 1-based first bar.
    pub start_bar: usize,
    pub bars: usize,
    pub start: f32,
    pub end: f32,
    /// Mean K-weighted loudness, LUFS-ish (ungated).
    pub loudness: f32,
    /// Mean spectral centroid, Hz.
    pub brightness: f32,
}

/// Boundaries (0-based bar indices where a new section starts, always including 0).
pub fn boundaries(features: &[Vec<f32>]) -> Vec<usize> {
    let n = features.len();
    if n < 4 {
        return vec![0];
    }
    let dim = features[0].len();
    // z-normalise every dimension.
    let mut z = features.to_vec();
    for d in 0..dim {
        let col: Vec<f32> = features.iter().map(|f| f[d]).collect();
        let m = dsp::mean(&col);
        let s = dsp::std_dev(&col).max(1e-6);
        for f in z.iter_mut() {
            f[d] = (f[d] - m) / s;
        }
    }
    let sim: Vec<Vec<f32>> = (0..n).map(|i| (0..n).map(|j| dsp::cosine(&z[i], &z[j])).collect()).collect();
    let l: isize = if n >= 24 { 4 } else { 2 };
    let mut nov = vec![0.0f32; n];
    for (b, nv) in nov.iter_mut().enumerate().skip(1) {
        let mut s = 0.0;
        let mut wsum = 0.0;
        for i in -l..l {
            for j in -l..l {
                let (bi, bj) = (b as isize + i, b as isize + j);
                if bi < 0 || bj < 0 || bi >= n as isize || bj >= n as isize {
                    continue;
                }
                let sign = if (i < 0) == (j < 0) { -1.0 } else { 1.0 };
                let g = (-((i as f32 + 0.5).powi(2) + (j as f32 + 0.5).powi(2)) / (2.0 * (l as f32 * 0.7).powi(2))).exp();
                s += sign * g * sim[bi as usize][bj as usize];
                wsum += g;
            }
        }
        *nv = s / wsum.max(1e-6);
        if b % 4 == 0 {
            *nv *= 1.15;
        }
    }
    let m = dsp::mean(&nov[1..]);
    let sd = dsp::std_dev(&nov[1..]);
    let thr = (m + 0.6 * sd).max(0.15);
    let mut out = vec![0usize];
    for b in 2..n.saturating_sub(1) {
        let lo = b.saturating_sub(2).max(1);
        let hi = (b + 3).min(n);
        let is_max = (lo..hi).all(|k| nov[k] <= nov[b]);
        if is_max && nov[b] > thr && b - out.last().unwrap() >= 2 {
            out.push(b);
        }
    }
    out
}

/// Labels sections by similarity of their mean features: A, B, C...
pub fn labels(features: &[Vec<f32>], bounds: &[usize]) -> Vec<String> {
    let n = features.len();
    if n == 0 {
        return Vec::new();
    }
    let dim = features[0].len();
    let mut z = features.to_vec();
    for d in 0..dim {
        let col: Vec<f32> = features.iter().map(|f| f[d]).collect();
        let m = dsp::mean(&col);
        let s = dsp::std_dev(&col).max(1e-6);
        for f in z.iter_mut() {
            f[d] = (f[d] - m) / s;
        }
    }
    let means: Vec<Vec<f32>> = bounds
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let b = bounds.get(i + 1).copied().unwrap_or(n);
            (0..dim).map(|d| dsp::mean(&z[a..b].iter().map(|f| f[d]).collect::<Vec<_>>())).collect()
        })
        .collect();
    let mut reps: Vec<(char, Vec<f32>)> = Vec::new();
    let mut out = Vec::new();
    for m in &means {
        let best = reps.iter().map(|(c, r)| (*c, dsp::cosine(m, r))).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        match best {
            Some((c, s)) if s > 0.6 => out.push(c.to_string()),
            _ => {
                let c = (b'A' + reps.len().min(25) as u8) as char;
                reps.push((c, m.clone()));
                out.push(c.to_string());
            }
        }
    }
    out
}
