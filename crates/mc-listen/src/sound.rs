//! How it sounds as a mix: loudness, dynamics, spectrum, stereo, space, pump.

use crate::decode::Audio;
use crate::dsp;
use crate::features::{FramesA, FramesB};
use mc_music::dsp::filter::{Biquad, BiquadKind};
use serde::{Deserialize, Serialize};

/// The four stereo bands: (name, low Hz, high Hz; 0 = open).
pub const WIDTH_BANDS: [(&str, f32, f32); 4] =
    [("sub", 20.0, 80.0), ("bass", 80.0, 250.0), ("mids", 250.0, 4000.0), ("highs", 4000.0, 0.0)];

/// Third-octave centres, Hz.
pub const THIRDS: [f32; 29] = [
    25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0,
    1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0,
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pump {
    pub detected: bool,
    /// How far the mids (400 Hz-3 kHz, above the bass, below the hats) dip after a kick, dB (averaged over kicks).
    pub depth_db: f32,
    /// When the dip is deepest, ms after the kick.
    pub dip_ms: f32,
    /// Time back to within 1 dB of the level before the kick, ms.
    pub recovery_ms: f32,
    /// Share of kicks followed by a dip of at least 1.5 dB.
    pub consistency: f32,
    pub kicks: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Sound {
    pub lufs: f32,
    pub lufs_loudest_3s: f32,
    /// Loudness range (EBU R128 style: 10th to 95th percentile of 3 s loudness), LU.
    pub lra: f32,
    pub peak_db: f32,
    pub rms_db: f32,
    pub crest_db: f32,
    pub clipped: usize,
    /// Side/mid RMS ratio per band (sub, bass, mids, highs): 0 = mono, 1 = as wide as centred.
    pub width: [f32; 4],
    /// L/R correlation over the whole mix.
    pub correlation: f32,
    /// The mix is mono (side 20 dB under mid) below this frequency, Hz; 0 = not even the lowest band.
    pub mono_below_hz: f32,
    /// Side/mid below 120 Hz.
    pub low_width: f32,
    /// (centre Hz, dB) third-octave average spectrum; a full-scale sine reads about 0 dB.
    pub third_octave: Vec<(f32, f32)>,
    pub centroid_hz: f32,
    /// (seconds, Hz) spectral centroid in twelve slices.
    pub centroid_over_time: Vec<(f32, f32)>,
    pub onsets_per_s: f32,
    /// Rough RT60 of the decay after isolated hits, seconds (instrument release and reverb together).
    pub decay_rt60: Option<f32>,
    pub decay_events: usize,
    pub pump: Pump,
}

/// K-weighted power per 10 ms block (both channels summed), for loudness over time.
pub fn k_power(audio: &Audio) -> (Vec<f32>, f32) {
    let r = audio.rate as f32;
    let mut kf = [Biquad::default(); 4];
    for c in 0..2 {
        kf[c].set(BiquadKind::HighShelf, 1681.0, 0.707, 4.0, r);
        kf[2 + c].set(BiquadKind::HighPass, 38.0, 0.5, 0.0, r);
    }
    let block = (0.01 * r).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(audio.frames.len() / block + 1);
    let mut acc = 0.0f64;
    let mut n = 0;
    for f in &audio.frames {
        let l = {
            let x = kf[0].process(f[0]);
            kf[2].process(x)
        };
        let rr = {
            let x = kf[1].process(f[1]);
            kf[3].process(x)
        };
        acc += (l * l + rr * rr) as f64;
        n += 1;
        if n == block {
            out.push((acc / block as f64) as f32);
            acc = 0.0;
            n = 0;
        }
    }
    (out, block as f32 / r)
}

pub fn lufs_of(p: f32) -> f32 {
    -0.691 + 10.0 * p.max(1e-12).log10()
}

/// Mean loudness between two times from `k_power` blocks.
pub fn loudness_between(kp: &[f32], hop: f32, a: f32, b: f32) -> f32 {
    let i0 = ((a / hop).max(0.0) as usize).min(kp.len());
    let i1 = ((b / hop).max(0.0) as usize).clamp(i0, kp.len());
    if i1 <= i0 {
        return -70.0;
    }
    lufs_of(dsp::mean(&kp[i0..i1]))
}

/// `kick_times` / `snare_times` from the drum grid (kicks under a snare are not used for the pump).
pub fn measure(audio: &Audio, a: &FramesA, b: &FramesB, kick_times: &[f32], snare_times: &[f32], beat: f32) -> Sound {
    let base = mc_music::render::analyse(&audio.frames, audio.rate);
    let mut s = Sound {
        lufs: base.lufs,
        lufs_loudest_3s: base.lufs_short_max,
        peak_db: base.peak_db,
        rms_db: base.rms_db,
        crest_db: base.crest_db,
        clipped: base.clipped,
        ..Default::default()
    };
    let rate = audio.rate;
    // Loudness range from 3 s windows every second.
    let (kp, khop) = k_power(audio);
    let win = (3.0 / khop) as usize;
    let step = (1.0 / khop) as usize;
    let mut st = Vec::new();
    let mut i = 0;
    while i + win <= kp.len() {
        st.push(lufs_of(dsp::mean(&kp[i..i + win])));
        i += step.max(1);
    }
    let gated: Vec<f32> = st.iter().copied().filter(|&l| l > -70.0).collect();
    if !gated.is_empty() {
        let mean_p = dsp::mean(&gated.iter().map(|l| 10f32.powf((l + 0.691) / 10.0)).collect::<Vec<_>>());
        let rel = lufs_of(mean_p) - 20.0;
        let g2: Vec<f32> = gated.into_iter().filter(|&l| l > rel).collect();
        s.lra = dsp::percentile(&g2, 0.95) - dsp::percentile(&g2, 0.10);
    }

    // Stereo: width per band, correlation, the mono low end.
    let l: Vec<f32> = audio.frames.iter().map(|f| f[0]).collect();
    let r: Vec<f32> = audio.frames.iter().map(|f| f[1]).collect();
    let mid: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let side: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a - b)).collect();
    let rms = |x: &[f32]| (x.iter().map(|v| (v * v) as f64).sum::<f64>() / x.len().max(1) as f64).sqrt() as f32;
    for (k, &(_, lo, hi)) in WIDTH_BANDS.iter().enumerate() {
        let m = rms(&dsp::band(&mid, rate, lo, hi));
        let sd = rms(&dsp::band(&side, rate, lo, hi));
        s.width[k] = if m > 1e-6 { sd / m } else { 0.0 };
    }
    let (mut lr, mut ll, mut rr) = (0.0f64, 0.0f64, 0.0f64);
    for (a, b) in l.iter().zip(&r) {
        lr += (a * b) as f64;
        ll += (a * a) as f64;
        rr += (b * b) as f64;
    }
    s.correlation = (lr / (ll.sqrt() * rr.sqrt()).max(1e-12)) as f32;
    // Low end on a decimated copy: bands up to 400 Hz.
    let factor = ((rate as f32 / 3000.0).floor() as usize).max(1);
    let lr_rate = rate / factor as u32;
    let md = dsp::decimate(&mid, factor);
    let sdd = dsp::decimate(&side, factor);
    let edges = [20.0, 40.0, 60.0, 80.0, 100.0, 120.0, 160.0, 200.0, 250.0, 315.0, 400.0];
    s.mono_below_hz = 0.0;
    let mut still_mono = true;
    for w in edges.windows(2) {
        let m = rms(&dsp::band(&md, lr_rate, w[0], w[1]));
        let sd = rms(&dsp::band(&sdd, lr_rate, w[0], w[1]));
        let mono = m < 1e-5 || sd / m < 0.1;
        if still_mono && mono {
            s.mono_below_hz = w[1];
        } else {
            still_mono = false;
        }
    }
    {
        let m = rms(&dsp::band(&md, lr_rate, 20.0, 120.0));
        let sd = rms(&dsp::band(&sdd, lr_rate, 20.0, 120.0));
        s.low_width = if m > 1e-6 { sd / m } else { 0.0 };
    }

    // Third-octave spectrum from the long-term average.
    for &fc in THIRDS.iter() {
        if fc > rate as f32 * 0.45 {
            break;
        }
        let lo = fc / 2f32.powf(1.0 / 6.0);
        let hi = fc * 2f32.powf(1.0 / 6.0);
        let a0 = (lo / b.bin_hz).ceil() as usize;
        let a1 = ((hi / b.bin_hz).floor() as usize).max(a0);
        let p: f32 = b.mean_power[a0.min(b.mean_power.len() - 1)..=a1.min(b.mean_power.len() - 1)].iter().sum();
        s.third_octave.push((fc, (dsp::pow_db(p / 1.5) * 10.0).round() / 10.0));
    }

    // Brightness.
    let voiced: Vec<f32> = a.centroid.iter().zip(&a.level_db).filter(|(_, l)| **l > -60.0).map(|(c, _)| *c).collect();
    s.centroid_hz = dsp::median(&voiced);
    let n = a.centroid.len();
    for k in 0..12 {
        let i0 = n * k / 12;
        let i1 = (n * (k + 1) / 12).max(i0 + 1).min(n);
        let slice: Vec<f32> = (i0..i1).filter(|&i| a.level_db[i] > -60.0).map(|i| a.centroid[i]).collect();
        s.centroid_over_time.push((i0 as f32 * a.hop, dsp::median(&slice).round()));
    }

    // Transients: peaks of the onset envelope.
    let env = crate::tempo::prepare(&a.flux, a.hop);
    let mut onsets = Vec::new();
    for i in 3..env.len().saturating_sub(3) {
        if env[i] > 0.2 && (i - 3..=i + 3).all(|k| env[k] <= env[i]) && onsets.last().map(|&o: &usize| i - o >= 5).unwrap_or(true) {
            onsets.push(i);
        }
    }
    s.onsets_per_s = onsets.len() as f32 / (n as f32 * a.hop).max(1e-3);

    // Decay after isolated strong hits.
    let mut rts = Vec::new();
    for (j, &o) in onsets.iter().enumerate() {
        if env[o] < 0.5 {
            continue;
        }
        let next = onsets.get(j + 1).copied().unwrap_or(n).min(o + (1.5 / a.hop) as usize);
        if (next - o) as f32 * a.hop < 0.25 {
            continue;
        }
        let pk_i = (o..(o + 5).min(n)).max_by(|&x, &y| a.level_db[x].partial_cmp(&a.level_db[y]).unwrap()).unwrap_or(o);
        let pk = a.level_db[pk_i];
        let pts: Vec<(f32, f32)> = (pk_i..next)
            .map(|i| (i as f32 * a.hop, a.level_db[i]))
            .filter(|(_, l)| *l < pk - 3.0 && *l > pk - 35.0)
            .collect();
        if pts.len() < 8 {
            continue;
        }
        let mx = dsp::mean(&pts.iter().map(|p| p.0).collect::<Vec<_>>());
        let my = dsp::mean(&pts.iter().map(|p| p.1).collect::<Vec<_>>());
        let num: f32 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
        let den: f32 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
        let slope = num / den.max(1e-9);
        if slope < -5.0 {
            rts.push(-60.0 / slope);
        }
    }
    s.decay_events = rts.len();
    if rts.len() >= 3 {
        s.decay_rt60 = Some((dsp::median(&rts) * 100.0).round() / 100.0);
    }

    let alone: Vec<f32> = kick_times.iter().copied().filter(|k| !snare_times.iter().any(|s| (s - k).abs() < 0.03)).collect();
    s.pump = pump(audio, &alone, beat);
    s
}

fn pump(audio: &Audio, kicks: &[f32], beat: f32) -> Pump {
    let mut p = Pump { kicks: kicks.len(), ..Default::default() };
    if kicks.len() < 4 {
        return p;
    }
    let rate = audio.rate;
    let mono = audio.mono();
    let y = dsp::band(&mono, rate, 400.0, 3000.0);
    let block = (rate as f32 * 0.005).round().max(1.0) as usize;
    let env = dsp::rms_db_blocks(&y, block, block * 2);
    let bt = block as f32 / rate as f32;
    let pre_n = (0.1 / bt) as isize;
    let span = ((beat * 0.95).min(0.6) / bt) as isize;
    let mut sum = vec![0.0f32; (pre_n + span) as usize];
    let mut count = 0usize;
    let mut dipped = 0usize;
    for (j, &k) in kicks.iter().enumerate() {
        let c = (k / bt).round() as isize;
        let gap = kicks.get(j + 1).map(|n| n - k).unwrap_or(beat);
        if gap < beat * 0.45 || c - pre_n < 0 || c + span >= env.len() as isize {
            continue;
        }
        let curve: Vec<f32> = (c - pre_n..c + span).map(|i| env[i as usize]).collect();
        let pre = dsp::mean(&curve[(pre_n - (0.06 / bt) as isize) as usize..(pre_n - (0.01 / bt) as isize) as usize]);
        if pre < -70.0 {
            continue;
        }
        let a = (pre_n + (0.03 / bt) as isize) as usize;
        let b = (pre_n + (0.25 / bt) as isize).min(pre_n + span) as usize;
        let mn = curve[a..b].iter().cloned().fold(f32::MAX, f32::min);
        if pre - mn >= 1.5 {
            dipped += 1;
        }
        for (s, v) in sum.iter_mut().zip(&curve) {
            *s += v - pre;
        }
        count += 1;
    }
    if count < 4 {
        return p;
    }
    let avg: Vec<f32> = sum.iter().map(|s| s / count as f32).collect();
    let pre = dsp::mean(&avg[(pre_n - (0.06 / bt) as isize) as usize..(pre_n - (0.01 / bt) as isize) as usize]);
    let a = (pre_n + (0.03 / bt) as isize) as usize;
    let b = ((pre_n + (0.25 / bt) as isize) as usize).min(avg.len());
    let (mi, mn) = avg[a..b].iter().enumerate().fold((0, f32::MAX), |acc, (i, &v)| if v < acc.1 { (i, v) } else { acc });
    p.depth_db = ((pre - mn) * 10.0).round() / 10.0;
    p.dip_ms = ((a + mi) as f32 - pre_n as f32) * bt * 1000.0;
    let rec = avg[a + mi..].iter().position(|&v| v >= pre - 1.0);
    p.recovery_ms = rec.map(|r| ((a + mi + r) as f32 - pre_n as f32) * bt * 1000.0).unwrap_or(span as f32 * bt * 1000.0);
    p.consistency = dipped as f32 / count as f32;
    p.kicks = count;
    p.detected = p.depth_db >= 3.5 && p.consistency >= 0.6;
    p
}
