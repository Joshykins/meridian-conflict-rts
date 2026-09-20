//! Seeded 2D gradient noise for the baker. Floating point: never used by the sim.
//!
//! Lattice gradients come from an integer hash of the cell coordinates, so a
//! field is a pure function of position. That is what lets tiles bake
//! independently on any thread and still meet exactly at their shared edges.

const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// cos and sin of 22.5 degrees.
const C: f64 = 0.923_879_532_511_286_7;
const S: f64 = 0.382_683_432_365_089_8;

/// 16 unit gradients, evenly spaced. More directions than the classic 8 hide
/// the axis-aligned streaks that show up in large flat areas.
const GRADIENTS: [(f64, f64); 16] = [
    (1.0, 0.0),
    (C, S),
    (D, D),
    (S, C),
    (0.0, 1.0),
    (-S, C),
    (-D, D),
    (-C, S),
    (-1.0, 0.0),
    (-C, -S),
    (-D, -D),
    (-S, -C),
    (0.0, -1.0),
    (S, -C),
    (D, -D),
    (C, -S),
];

#[inline]
fn mix(mut h: u64) -> u64 {
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^= h >> 32;
    h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
    h ^ (h >> 32)
}

/// Hash of a lattice point. Also used directly for per-cell prop decisions.
#[inline]
pub fn hash2(seed: u64, x: i64, y: i64) -> u64 {
    let h = mix(seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    mix(h.wrapping_add((y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)))
}

/// Uniform in `[0, 1)` from 24 bits of a hash, starting at bit `shift`.
#[inline]
pub fn unit(hash: u64, shift: u32) -> f64 {
    ((hash >> shift) & 0xFF_FFFF) as f64 / (1u64 << 24) as f64
}

#[derive(Clone, Copy)]
pub struct Noise {
    seed: u64,
}

impl Noise {
    /// `channel` separates the fields of one bake from each other.
    pub fn new(seed: u64, channel: u64) -> Noise {
        Noise {
            seed: mix(seed ^ channel.wrapping_mul(0xA24B_AED4_963E_E407)),
        }
    }

    /// Roughly `[-1, 1]`, zero at lattice points, one feature per unit.
    pub fn get(&self, x: f64, y: f64) -> f64 {
        let (xf, yf) = (x.floor(), y.floor());
        let (ix, iy) = (xf as i64, yf as i64);
        let (fx, fy) = (x - xf, y - yf);
        let corner = |dx: i64, dy: i64| {
            let (gx, gy) = GRADIENTS[(hash2(self.seed, ix + dx, iy + dy) >> 60) as usize];
            gx * (fx - dx as f64) + gy * (fy - dy as f64)
        };
        let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let (u, v) = (fade(fx), fade(fy));
        let (n00, n10, n01, n11) = (corner(0, 0), corner(1, 0), corner(0, 1), corner(1, 1));
        let bottom = n00 + (n10 - n00) * u;
        let top = n01 + (n11 - n01) * u;
        // Unit gradients peak at sqrt(1/2); rescale to fill [-1, 1].
        (bottom + (top - bottom) * v) * std::f64::consts::SQRT_2
    }

    /// Fractal sum, normalised back to roughly `[-1, 1]`. The lacunarity is
    /// slightly off 2 so octave lattices do not line up.
    pub fn fbm(&self, x: f64, y: f64, octaves: u32, gain: f64) -> f64 {
        let (mut sum, mut norm, mut amp, mut freq) = (0.0, 0.0, 1.0, 1.0);
        for o in 0..octaves {
            // Offset each octave so their zero crossings at the origin do not coincide.
            let shift = o as f64 * 17.37;
            sum += amp * self.get(x * freq + shift, y * freq - shift);
            norm += amp;
            amp *= gain;
            freq *= 1.97;
        }
        sum / norm
    }

    /// Ridged fractal in `[0, 1]`: sharp crests where the base noise crosses zero.
    pub fn ridged(&self, x: f64, y: f64, octaves: u32, gain: f64) -> f64 {
        let (mut sum, mut norm, mut amp, mut freq) = (0.0, 0.0, 1.0, 1.0);
        for o in 0..octaves {
            let shift = o as f64 * 31.91;
            let r = 1.0 - self.get(x * freq + shift, y * freq - shift).abs();
            sum += amp * r * r;
            norm += amp;
            amp *= gain;
            freq *= 2.03;
        }
        sum / norm
    }
}

#[inline]
pub fn smoothstep(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1 at `t = 0` falling smoothly to 0 at `t >= 1`.
#[inline]
pub fn bump(t: f64) -> f64 {
    if t >= 1.0 {
        0.0
    } else {
        let s = 1.0 - t * t;
        s * s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_continuous_and_seeded() {
        let n = Noise::new(1, 0);
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        let mut prev = n.get(0.0, 0.3);
        for i in 1..20_000 {
            let v = n.get(i as f64 * 0.013, 0.3 + i as f64 * 0.007);
            assert!((v - prev).abs() < 0.08, "jump at {i}");
            (lo, hi, prev) = (lo.min(v), hi.max(v), v);
        }
        assert!(
            lo >= -1.0 && hi <= 1.0 && lo < -0.5 && hi > 0.5,
            "{lo}..{hi}"
        );
        assert_eq!(n.get(12.5, -7.25), Noise::new(1, 0).get(12.5, -7.25));
        assert_ne!(n.get(12.5, -7.25), Noise::new(1, 1).get(12.5, -7.25));
        assert_ne!(n.get(12.5, -7.25), Noise::new(2, 0).get(12.5, -7.25));
        let r = n.ridged(3.3, 4.4, 4, 0.5);
        assert!((0.0..=1.0).contains(&r));
    }

    #[test]
    fn shaping_functions() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.5), 0.5);
        assert_eq!(smoothstep(2.0, 4.0, 9.0), 1.0);
        assert_eq!(bump(0.0), 1.0);
        assert_eq!(bump(1.5), 0.0);
        assert!(bump(0.5) > bump(0.6));
    }
}
