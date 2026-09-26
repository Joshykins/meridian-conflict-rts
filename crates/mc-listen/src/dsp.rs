//! Small signal helpers shared by the analysers: FFT frames, filters,
//! decimation, envelopes, statistics.

use mc_music::dsp::filter::{Biquad, BiquadKind};
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// Power in dB, floored at -120.
#[inline]
pub fn pow_db(p: f32) -> f32 {
    10.0 * p.max(1e-12).log10()
}

/// Amplitude in dB, floored at -120.
#[inline]
pub fn amp_db(a: f32) -> f32 {
    20.0 * a.abs().max(1e-6).log10()
}

#[inline]
pub fn hz_to_midi(f: f32) -> f32 {
    69.0 + 12.0 * (f.max(1e-3) / 440.0).log2()
}

#[inline]
pub fn midi_to_hz(m: f32) -> f32 {
    440.0 * 2f32.powf((m - 69.0) / 12.0)
}

pub fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
        .collect()
}

/// A filter chain of biquads run over a whole signal.
pub fn filter(signal: &[f32], rate: u32, stages: &[(BiquadKind, f32)]) -> Vec<f32> {
    let mut bq: Vec<Biquad> = stages
        .iter()
        .map(|&(k, f)| {
            let mut b = Biquad::default();
            b.set(k, f.min(rate as f32 * 0.45), 0.707, 0.0, rate as f32);
            b
        })
        .collect();
    signal
        .iter()
        .map(|&x| {
            let mut y = x;
            for b in bq.iter_mut() {
                y = b.process(y);
            }
            y
        })
        .collect()
}

/// Band-pass by a 4th-order high-pass at `lo` and 4th-order low-pass at `hi` (0 = none).
pub fn band(signal: &[f32], rate: u32, lo: f32, hi: f32) -> Vec<f32> {
    let mut st = Vec::new();
    if lo > 0.0 {
        st.push((BiquadKind::HighPass, lo));
        st.push((BiquadKind::HighPass, lo));
    }
    if hi > 0.0 && hi < rate as f32 * 0.45 {
        st.push((BiquadKind::LowPass, hi));
        st.push((BiquadKind::LowPass, hi));
    }
    filter(signal, rate, &st)
}

/// Keeps every `factor`-th sample after a windowed-sinc low-pass at 0.45 of the new Nyquist.
pub fn decimate(signal: &[f32], factor: usize) -> Vec<f32> {
    if factor <= 1 {
        return signal.to_vec();
    }
    let half = 12 * factor;
    let cutoff = 0.45 / factor as f32; // cycles per input sample
    let taps: Vec<f32> = (0..=2 * half)
        .map(|i| {
            let n = i as f32 - half as f32;
            let sinc = if n == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * std::f32::consts::PI * cutoff * n).sin() / (std::f32::consts::PI * n)
            };
            let w = 0.42 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (2 * half) as f32).cos()
                + 0.08 * (4.0 * std::f32::consts::PI * i as f32 / (2 * half) as f32).cos();
            sinc * w
        })
        .collect();
    let sum: f32 = taps.iter().sum();
    let taps: Vec<f32> = taps.iter().map(|t| t / sum).collect();
    let out_len = signal.len() / factor;
    let mut out = Vec::with_capacity(out_len);
    for o in 0..out_len {
        let c = (o * factor) as isize;
        let mut acc = 0.0;
        for (k, &t) in taps.iter().enumerate() {
            let i = c + k as isize - half as isize;
            if i >= 0 && (i as usize) < signal.len() {
                acc += t * signal[i as usize];
            }
        }
        out.push(acc);
    }
    out
}

/// A reusable real-input FFT of one size.
pub struct Spectrum {
    pub size: usize,
    fft: Arc<dyn Fft<f32>>,
    buf: Vec<Complex32>,
    window: Vec<f32>,
}

impl Spectrum {
    pub fn new(size: usize) -> Spectrum {
        let fft = FftPlanner::new().plan_fft_forward(size);
        Spectrum {
            size,
            fft,
            buf: vec![Complex32::new(0.0, 0.0); size],
            window: hann(size),
        }
    }

    /// Magnitudes of bins 0..=size/2 of the Hann-windowed frame starting at `start`
    /// (zero outside the signal), scaled so a full-scale sine peaks near 1.
    pub fn magnitudes(&mut self, signal: &[f32], start: isize, out: &mut Vec<f32>) {
        let scale = 4.0 / self.size as f32;
        for i in 0..self.size {
            let j = start + i as isize;
            let x = if j >= 0 && (j as usize) < signal.len() {
                signal[j as usize]
            } else {
                0.0
            };
            self.buf[i] = Complex32::new(x * self.window[i], 0.0);
        }
        self.fft.process(&mut self.buf);
        out.clear();
        out.extend(self.buf[..=self.size / 2].iter().map(|c| c.norm() * scale));
    }
}

/// Plain autocorrelation of a sequence (mean removed) by FFT, lags 0..len.
pub fn autocorrelation(x: &[f32]) -> Vec<f32> {
    let n = x.len();
    if n == 0 {
        return Vec::new();
    }
    let mean = x.iter().sum::<f32>() / n as f32;
    let size = (2 * n).next_power_of_two();
    let mut planner = FftPlanner::new();
    let f = planner.plan_fft_forward(size);
    let inv = planner.plan_fft_inverse(size);
    let mut buf: Vec<Complex32> = (0..size)
        .map(|i| Complex32::new(if i < n { x[i] - mean } else { 0.0 }, 0.0))
        .collect();
    f.process(&mut buf);
    for c in buf.iter_mut() {
        *c = Complex32::new(c.norm_sqr(), 0.0);
    }
    inv.process(&mut buf);
    let z = buf[0].re.max(1e-12);
    buf[..n].iter().map(|c| c.re / z).collect()
}

/// Linear interpolation into `x` at a fractional index (0 outside).
#[inline]
pub fn lerp_at(x: &[f32], i: f32) -> f32 {
    if i < 0.0 {
        return 0.0;
    }
    let k = i.floor() as usize;
    if k + 1 >= x.len() {
        return if k < x.len() { x[k] } else { 0.0 };
    }
    let f = i - k as f32;
    x[k] * (1.0 - f) + x[k + 1] * f
}

pub fn median(v: &[f32]) -> f32 {
    percentile(v, 0.5)
}

/// `q` in 0..1, linear between order statistics; 0 for an empty slice.
pub fn percentile(v: &[f32], q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s: Vec<f32> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return 0.0;
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = q.clamp(0.0, 1.0) * (s.len() - 1) as f32;
    let k = pos.floor() as usize;
    let f = pos - k as f32;
    if k + 1 < s.len() {
        s[k] * (1.0 - f) + s[k + 1] * f
    } else {
        s[k]
    }
}

pub fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}

pub fn std_dev(v: &[f32]) -> f32 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / v.len() as f32).sqrt()
}

/// Running mean over a centred window of `w` samples.
pub fn moving_average(x: &[f32], w: usize) -> Vec<f32> {
    let h = w / 2;
    let mut pre = vec![0.0f64; x.len() + 1];
    for (i, &v) in x.iter().enumerate() {
        pre[i + 1] = pre[i] + v as f64;
    }
    (0..x.len())
        .map(|i| {
            let a = i.saturating_sub(h);
            let b = (i + h + 1).min(x.len());
            ((pre[b] - pre[a]) / (b - a).max(1) as f64) as f32
        })
        .collect()
}

/// RMS in dB of consecutive blocks of `block` samples, each over a window of `win` samples centred on the block.
pub fn rms_db_blocks(x: &[f32], block: usize, win: usize) -> Vec<f32> {
    let n = x.len() / block.max(1);
    let mut pre = vec![0.0f64; x.len() + 1];
    for (i, &v) in x.iter().enumerate() {
        pre[i + 1] = pre[i] + (v * v) as f64;
    }
    (0..n)
        .map(|k| {
            let c = k * block + block / 2;
            let a = c.saturating_sub(win / 2);
            let b = (c + win / 2).min(x.len()).max(a + 1);
            pow_db(((pre[b] - pre[a]) / (b - a) as f64) as f32)
        })
        .collect()
}

/// Cosine similarity.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    ab / (aa.sqrt() * bb.sqrt()).max(1e-12)
}

/// Pearson correlation.
pub fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let ma = mean(a);
    let mb = mean(b);
    let (mut ab, mut aa, mut bb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        ab += (x - ma) * (y - mb);
        aa += (x - ma) * (x - ma);
        bb += (y - mb) * (y - mb);
    }
    ab / (aa.sqrt() * bb.sqrt()).max(1e-12)
}

/// "1:32.5" for 92.5 s.
pub fn clock(s: f32) -> String {
    let s = s.max(0.0);
    let m = (s / 60.0).floor();
    format!("{}:{:04.1}", m as u32, s - m * 60.0)
}
