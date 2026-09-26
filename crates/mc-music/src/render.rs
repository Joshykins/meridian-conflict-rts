//! Offline rendering, WAV files and measurement.
//!
//! Music is judged by ear, but a lot can be checked by numbers first: peak and
//! loudness against the game's other sounds, how the energy spreads over the
//! bands (a mix with nothing between 200 and 2k Hz sounds hollow), clipping,
//! DC, and whether each intensity step is actually louder or busier.

use crate::engine::{Engine, Mode};
use crate::song::Song;
use std::path::Path;
use std::sync::Arc;

/// Renders `seconds` of `song` from the start in `mode`, at a fixed `intensity`.
/// An `intensity_ramp` of (from, to) moves it linearly over the render instead.
pub fn render(
    song: &Song,
    rate: u32,
    seconds: f32,
    mode: Mode,
    intensity: (f32, f32),
) -> Vec<[f32; 2]> {
    render_profiled(song, rate, seconds, mode, intensity, false).0
}

/// `render`, and when `profile` is set, the seconds each stage took.
pub fn render_profiled(
    song: &Song,
    rate: u32,
    seconds: f32,
    mode: Mode,
    intensity: (f32, f32),
    profile: bool,
) -> (Vec<[f32; 2]>, Option<crate::engine::Profile>) {
    let mut e = Engine::new(rate as f32, Arc::new(song.clone()));
    if profile {
        e.profile = Some(Default::default());
    }
    e.command(crate::engine::Command::ForceIntensity(intensity.0));
    e.set_mode(mode);
    e.play();
    let total = (seconds * rate as f32) as usize;
    let mut out = vec![[0.0f32; 2]; total];
    let step = 1024;
    let mut at = 0;
    while at < total {
        let n = step.min(total - at);
        let k = at as f32 / total.max(1) as f32;
        e.set_intensity(intensity.0 + (intensity.1 - intensity.0) * k);
        e.render_frames(&mut out[at..at + n]);
        at += n;
    }
    (out, e.profile.take())
}

/// Renders the whole arrangement plus `tail` seconds of release.
pub fn render_arrangement(song: &Song, rate: u32, tail: f32) -> Vec<[f32; 2]> {
    let secs = song.arrangement_ticks() as f64 * song.samples_per_tick(rate as f32) / rate as f64;
    render(song, rate, secs as f32 + tail, Mode::Song, (1.0, 1.0))
}

pub fn write_wav(path: &Path, frames: &[[f32; 2]], rate: u32) -> std::io::Result<()> {
    let mut bytes = Vec::with_capacity(44 + frames.len() * 4);
    let data_len = (frames.len() * 4) as u32;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for f in frames {
        for c in f {
            let v = (c.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    std::fs::write(path, bytes)
}

/// Numbers about a render.
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub seconds: f32,
    pub peak_db: f32,
    pub rms_db: f32,
    /// Integrated loudness, gated like BS.1770 (absolute -70, relative -10).
    pub lufs: f32,
    /// Loudest 3 s window.
    pub lufs_short_max: f32,
    /// Peak over RMS, dB: small = squashed, large = spiky.
    pub crest_db: f32,
    /// Samples at or above full scale.
    pub clipped: usize,
    pub dc: f32,
    /// Share of energy per band, percent: sub <60, bass 60-250, low-mid 250-2k, high-mid 2k-6k, air >6k.
    pub bands: [f32; 5],
    /// Side over mid energy: 0 = mono, 1 = as wide as it is centred.
    pub width: f32,
    /// Seconds that are near silent (below -50 dBFS RMS over 100 ms).
    pub silent_seconds: f32,
}

pub fn analyse(frames: &[[f32; 2]], rate: u32) -> Analysis {
    use crate::dsp::filter::{Biquad, BiquadKind};
    let r = rate as f32;
    let mut a = Analysis {
        seconds: frames.len() as f32 / r,
        ..Default::default()
    };
    if frames.is_empty() {
        return a;
    }
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    let mut dc = 0.0f64;
    let (mut mid_e, mut side_e) = (0.0f64, 0.0f64);
    for f in frames {
        for &c in f {
            peak = peak.max(c.abs());
            sum += (c * c) as f64;
            dc += c as f64;
            if c.abs() >= 0.999 {
                a.clipped += 1;
            }
        }
        let m = (f[0] + f[1]) * 0.5;
        let s = (f[0] - f[1]) * 0.5;
        mid_e += (m * m) as f64;
        side_e += (s * s) as f64;
    }
    let n = (frames.len() * 2) as f64;
    a.peak_db = crate::dsp::gain_to_db(peak);
    let rms = (sum / n).sqrt() as f32;
    a.rms_db = crate::dsp::gain_to_db(rms);
    a.crest_db = a.peak_db - a.rms_db;
    a.dc = (dc / n) as f32;
    a.width = if mid_e > 0.0 {
        (side_e / mid_e) as f32
    } else {
        0.0
    };

    // Loudness: K-weighted power in 400 ms blocks, 75% overlap.
    let mut kf = [Biquad::default(); 4];
    for c in 0..2 {
        kf[c].set(BiquadKind::HighShelf, 1681.0, 0.707, 4.0, r);
        kf[2 + c].set(BiquadKind::HighPass, 38.0, 0.5, 0.0, r);
    }
    let kpow: Vec<f32> = frames
        .iter()
        .map(|f| {
            let l = {
                let x = kf[0].process(f[0]);
                kf[2].process(x)
            };
            let rr = {
                let x = kf[1].process(f[1]);
                kf[3].process(x)
            };
            l * l + rr * rr
        })
        .collect();
    let block = (0.4 * r) as usize;
    let hop = block / 4;
    let mut blocks = Vec::new();
    let mut i = 0;
    while i + block <= kpow.len() {
        let p: f64 = kpow[i..i + block].iter().map(|&x| x as f64).sum::<f64>() / block as f64;
        blocks.push(p);
        i += hop;
    }
    let lufs = |p: f64| -0.691 + 10.0 * p.max(1e-12).log10();
    let gated: Vec<f64> = blocks
        .iter()
        .copied()
        .filter(|&p| lufs(p) > -70.0)
        .collect();
    if !gated.is_empty() {
        let mean = gated.iter().sum::<f64>() / gated.len() as f64;
        let rel = lufs(mean) - 10.0;
        let g2: Vec<f64> = gated.iter().copied().filter(|&p| lufs(p) > rel).collect();
        let m2 = g2.iter().sum::<f64>() / g2.len().max(1) as f64;
        a.lufs = lufs(m2) as f32;
    } else {
        a.lufs = -70.0;
    }
    let short = (3.0 * r) as usize;
    let mut best = -70.0f32;
    let mut j = 0;
    while j + short <= kpow.len() {
        let p: f64 = kpow[j..j + short].iter().map(|&x| x as f64).sum::<f64>() / short as f64;
        best = best.max(lufs(p) as f32);
        j += (r * 0.5) as usize;
    }
    a.lufs_short_max = best;

    // Bands, by splitting with 4th-order crossovers.
    let edges = [60.0, 250.0, 2000.0, 6000.0];
    let mut energies = [0.0f64; 5];
    let mono: Vec<f32> = frames.iter().map(|f| (f[0] + f[1]) * 0.5).collect();
    let mut lows = Vec::new();
    for &e in &edges {
        let mut lp = [Biquad::default(); 2];
        lp[0].set(BiquadKind::LowPass, e, 0.707, 0.0, r);
        lp[1].set(BiquadKind::LowPass, e, 0.707, 0.0, r);
        let energy: f64 = mono
            .iter()
            .map(|&x| {
                let y = lp[0].process(x);
                lp[1].process(y)
            })
            .map(|y| (y * y) as f64)
            .sum();
        lows.push(energy);
    }
    let total: f64 = mono.iter().map(|&x| (x * x) as f64).sum();
    energies[0] = lows[0];
    for k in 1..4 {
        energies[k] = (lows[k] - lows[k - 1]).max(0.0);
    }
    energies[4] = (total - lows[3]).max(0.0);
    let esum: f64 = energies.iter().sum::<f64>().max(1e-12);
    for (band, &e) in a.bands.iter_mut().zip(&energies) {
        *band = (e / esum * 100.0) as f32;
    }

    let win = (0.1 * r) as usize;
    let mut silent = 0;
    for chunk in frames.chunks(win) {
        let p: f32 =
            chunk.iter().map(|f| f[0] * f[0] + f[1] * f[1]).sum::<f32>() / (chunk.len() * 2) as f32;
        if crate::dsp::gain_to_db(p.sqrt()) < -50.0 {
            silent += 1;
        }
    }
    a.silent_seconds = silent as f32 * 0.1;
    a
}

impl std::fmt::Display for Analysis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.1} s  peak {:.1} dBFS  rms {:.1}  LUFS {:.1} (loudest 3 s {:.1})  crest {:.1} dB  clipped {}  dc {:.4}\n  bands sub {:.0}% bass {:.0}% lowmid {:.0}% highmid {:.0}% air {:.0}%  width {:.2}  silent {:.1} s",
            self.seconds,
            self.peak_db,
            self.rms_db,
            self.lufs,
            self.lufs_short_max,
            self.crest_db,
            self.clipped,
            self.dc,
            self.bands[0],
            self.bands[1],
            self.bands[2],
            self.bands[3],
            self.bands[4],
            self.width,
            self.silent_seconds
        )
    }
}
