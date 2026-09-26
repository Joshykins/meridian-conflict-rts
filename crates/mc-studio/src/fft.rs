//! A small in-place radix-2 FFT for the spectrum analyser. The scope holds
//! 4096 frames, so one power-of-two transform a frame is all that is needed;
//! not worth a dependency.

use std::f32::consts::PI;

/// Transforms `re`/`im` in place. Lengths must be equal and a power of two.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    assert!(n.is_power_of_two() && im.len() == n);
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

/// Magnitude spectrum in dBFS of a mono signal (Hann window), `n/2` bins;
/// a full-scale sine reads about 0 dB.
pub fn spectrum_db(signal: &[f32]) -> Vec<f32> {
    let n = signal.len().next_power_of_two().max(2);
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for (i, &x) in signal.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f32 / (signal.len().max(2) - 1) as f32).cos();
        re[i] = x * w;
    }
    fft(&mut re, &mut im);
    // Hann's coherent gain is 0.5; a real sine splits across +-f.
    let scale = 4.0 / signal.len().max(1) as f32;
    (0..n / 2)
        .map(|k| {
            let m = (re[k] * re[k] + im[k] * im[k]).sqrt() * scale;
            if m <= 1e-7 {
                -140.0
            } else {
                20.0 * m.log10()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impulse_is_flat() {
        let mut re = vec![0.0; 8];
        let mut im = vec![0.0; 8];
        re[0] = 1.0;
        fft(&mut re, &mut im);
        for k in 0..8 {
            assert!((re[k] - 1.0).abs() < 1e-6 && im[k].abs() < 1e-6);
        }
    }

    #[test]
    fn matches_a_direct_dft() {
        let n = 16;
        let x: Vec<f32> = (0..n).map(|i| ((i * 7 % 5) as f32 - 2.0) * 0.3).collect();
        let mut re = x.clone();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        for k in 0..n {
            let (mut dr, mut di) = (0.0f32, 0.0f32);
            for (i, v) in x.iter().enumerate() {
                let a = -2.0 * PI * (k * i) as f32 / n as f32;
                dr += v * a.cos();
                di += v * a.sin();
            }
            assert!((re[k] - dr).abs() < 1e-4, "bin {k}");
            assert!((im[k] - di).abs() < 1e-4, "bin {k}");
        }
    }

    #[test]
    fn a_sine_peaks_in_its_bin() {
        let n = 1024;
        let bin = 64;
        let x: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * bin as f32 * i as f32 / n as f32).sin())
            .collect();
        let s = spectrum_db(&x);
        let peak = (0..s.len()).max_by(|&a, &b| s[a].total_cmp(&s[b])).unwrap();
        assert_eq!(peak, bin);
        assert!(s[bin].abs() < 1.0, "{} dB", s[bin]);
        assert!(s[bin * 3] < -60.0);
    }
}
