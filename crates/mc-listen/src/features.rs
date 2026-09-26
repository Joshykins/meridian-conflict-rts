//! Frame features every analyser shares, from two STFT passes over the mono mix.
//!
//! Pass A (~46 ms Hann, 10 ms hop): log band energies (40 bands, 30 Hz-16 kHz),
//! spectral flux onsets (all, low, mid, high), MFCC-ish cepstra, centroid, level.
//! Pass B (~170 ms Hann, 40 ms hop): spectral peaks (with parabolic frequency
//! interpolation) for chroma, tuning and melody salience, and the long-term
//! average spectrum.

use crate::dsp::{self, Spectrum};

pub const BANDS: usize = 40;
pub const MFCC: usize = 13;

/// Pass A.
#[derive(Clone, Debug, Default)]
pub struct FramesA {
    /// Seconds between frames; frame `i` is centred at `i * hop`.
    pub hop: f32,
    pub band_hz: Vec<f32>,
    /// Log band energy (dB) per frame.
    pub bands: Vec<[f32; BANDS]>,
    /// Spectral flux, all bands / below 150 Hz / 150 Hz-2 kHz / above 5 kHz.
    pub flux: Vec<f32>,
    pub flux_low: Vec<f32>,
    pub flux_mid: Vec<f32>,
    pub flux_high: Vec<f32>,
    pub mfcc: Vec<[f32; MFCC]>,
    pub centroid: Vec<f32>,
    pub level_db: Vec<f32>,
}

/// One spectral peak: Hz and linear magnitude.
#[derive(Clone, Copy, Debug)]
pub struct Peak {
    pub hz: f32,
    pub mag: f32,
}

/// Pass B.
#[derive(Clone, Debug, Default)]
pub struct FramesB {
    pub hop: f32,
    pub peaks: Vec<Vec<Peak>>,
    /// Mean power per FFT bin over the whole signal.
    pub mean_power: Vec<f32>,
    pub bin_hz: f32,
}

pub fn pass_a(x: &[f32], rate: u32) -> FramesA {
    let r = rate as f32;
    let size = ((0.046 * r) as usize).next_power_of_two();
    let hop_n = (0.01 * r).round().max(1.0) as usize;
    let bin_hz = r / size as f32;
    let top = (r * 0.45).min(16000.0);
    let edges: Vec<f32> = (0..=BANDS).map(|i| 30.0 * (top / 30.0).powf(i as f32 / BANDS as f32)).collect();
    let band_hz: Vec<f32> = (0..BANDS).map(|i| (edges[i] * edges[i + 1]).sqrt()).collect();
    // Bin ranges per band; a band narrower than a bin takes the nearest bin.
    let ranges: Vec<(usize, usize)> = (0..BANDS)
        .map(|i| {
            let a = (edges[i] / bin_hz).ceil() as usize;
            let b = (edges[i + 1] / bin_hz).ceil() as usize;
            if b <= a {
                let c = (band_hz[i] / bin_hz).round() as usize;
                (c, c + 1)
            } else {
                (a, b)
            }
        })
        .collect();
    let n = x.len() / hop_n + 1;
    let mut sp = Spectrum::new(size);
    let mut mags = Vec::new();
    let mut out = FramesA { hop: hop_n as f32 / r, band_hz: band_hz.clone(), ..Default::default() };
    let mut prev = [-100.0f32; BANDS];
    for i in 0..n {
        let start = (i * hop_n) as isize - (size / 2) as isize;
        sp.magnitudes(x, start, &mut mags);
        let mut b = [0.0f32; BANDS];
        let mut total = 0.0f32;
        let mut cw = 0.0f32;
        for (k, m) in mags.iter().enumerate() {
            let p = m * m;
            total += p;
            cw += p * k as f32 * bin_hz;
        }
        for (j, &(a, e)) in ranges.iter().enumerate() {
            let e = e.min(mags.len());
            let p: f32 = mags[a.min(e)..e].iter().map(|m| m * m).sum();
            b[j] = dsp::pow_db(p).max(-100.0);
        }
        let (mut f, mut fl, mut fm, mut fh) = (0.0, 0.0, 0.0, 0.0);
        for j in 0..BANDS {
            let d = (b[j].max(-80.0) - prev[j].max(-80.0)).max(0.0);
            f += d;
            if band_hz[j] < 150.0 {
                fl += d;
            } else if band_hz[j] < 2000.0 {
                fm += d;
            } else if band_hz[j] > 5000.0 {
                fh += d;
            }
        }
        prev = b;
        let mut c = [0.0f32; MFCC];
        for (q, cq) in c.iter_mut().enumerate() {
            let mut s = 0.0;
            for (j, bj) in b.iter().enumerate() {
                s += bj.max(-100.0) * (std::f32::consts::PI * q as f32 * (j as f32 + 0.5) / BANDS as f32).cos();
            }
            *cq = s / BANDS as f32;
        }
        out.bands.push(b);
        out.flux.push(f);
        out.flux_low.push(fl);
        out.flux_mid.push(fm);
        out.flux_high.push(fh);
        out.mfcc.push(c);
        out.centroid.push(if total > 1e-10 { cw / total } else { 0.0 });
        out.level_db.push(dsp::pow_db(total * 0.5));
    }
    out
}

pub fn pass_b(x: &[f32], rate: u32, hop: f32) -> FramesB {
    let r = rate as f32;
    let size = ((0.17 * r) as usize).next_power_of_two();
    let hop_n = (hop * r).round().max(1.0) as usize;
    let bin_hz = r / size as f32;
    let n = x.len() / hop_n + 1;
    let mut sp = Spectrum::new(size);
    let mut mags = Vec::new();
    let mut mean_power = vec![0.0f64; size / 2 + 1];
    let mut peaks = Vec::with_capacity(n);
    let lo_bin = (25.0 / bin_hz).floor().max(2.0) as usize;
    let hi_bin = ((8000.0f32).min(r * 0.45) / bin_hz) as usize;
    for i in 0..n {
        let start = (i * hop_n) as isize - (size / 2) as isize;
        sp.magnitudes(x, start, &mut mags);
        for (k, m) in mags.iter().enumerate() {
            mean_power[k] += (m * m) as f64;
        }
        let top = mags[lo_bin..hi_bin.min(mags.len() - 1)].iter().cloned().fold(0.0f32, f32::max);
        let floor = top * 10f32.powf(-60.0 / 20.0);
        let mut fr: Vec<Peak> = Vec::new();
        if top > 1e-6 {
            for k in lo_bin..hi_bin.min(mags.len() - 2) {
                let m = mags[k];
                if m > floor && m > mags[k - 1] && m >= mags[k + 1] {
                    // Local whitening: a peak must stand above its neighbourhood.
                    let a = k.saturating_sub(12);
                    let b = (k + 13).min(mags.len());
                    let local = mags[a..b].iter().sum::<f32>() / (b - a) as f32;
                    if m < local * 1.8 {
                        continue;
                    }
                    let (la, lb, lc) = (dsp::amp_db(mags[k - 1]), dsp::amp_db(m), dsp::amp_db(mags[k + 1]));
                    let den = la - 2.0 * lb + lc;
                    let off = if den.abs() > 1e-9 { (0.5 * (la - lc) / den).clamp(-0.5, 0.5) } else { 0.0 };
                    let peak_db = lb - 0.25 * (la - lc) * off;
                    fr.push(Peak { hz: (k as f32 + off) * bin_hz, mag: 10f32.powf(peak_db / 20.0) });
                }
            }
        }
        fr.sort_by(|a, b| b.mag.partial_cmp(&a.mag).unwrap());
        fr.truncate(48);
        peaks.push(fr);
    }
    let nf = n.max(1) as f64;
    FramesB {
        hop: hop_n as f32 / r,
        peaks,
        mean_power: mean_power.iter().map(|p| (p / nf) as f32).collect(),
        bin_hz,
    }
}

/// Global tuning offset (semitones, -0.5..0.5) from all peaks between 80 Hz and 2 kHz.
pub fn tuning(b: &FramesB) -> f32 {
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    for fr in &b.peaks {
        for p in fr.iter().take(16) {
            if p.hz < 80.0 || p.hz > 2000.0 {
                continue;
            }
            let m = dsp::hz_to_midi(p.hz);
            let ph = 2.0 * std::f64::consts::PI * (m - m.round()) as f64;
            let w = p.mag as f64;
            sx += ph.cos() * w;
            sy += ph.sin() * w;
        }
    }
    if sx.abs() + sy.abs() < 1e-9 {
        0.0
    } else {
        (sy.atan2(sx) / (2.0 * std::f64::consts::PI)) as f32
    }
}

/// Chroma per pass-B frame from peaks between `lo` and `hi` Hz, after taking out `tuning`.
/// Magnitudes are square-rooted so one loud partial does not own the frame.
pub fn chroma(b: &FramesB, tuning: f32, lo: f32, hi: f32) -> Vec<[f32; 12]> {
    b.peaks
        .iter()
        .map(|fr| {
            let mut c = [0.0f32; 12];
            for p in fr {
                if p.hz < lo || p.hz > hi {
                    continue;
                }
                let m = dsp::hz_to_midi(p.hz) - tuning;
                let dev = (m - m.round()).abs();
                if dev > 0.4 {
                    continue;
                }
                let w = p.mag.sqrt() * (1.0 - dev * 1.5).max(0.0);
                // Upper partials count less: they are mostly harmonics.
                let tilt = if p.hz > 1000.0 { (1000.0 / p.hz).sqrt() } else { 1.0 };
                c[(m.round() as i32).rem_euclid(12) as usize] += w * tilt;
            }
            c
        })
        .collect()
}
