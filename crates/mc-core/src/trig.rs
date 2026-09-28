//! Integer-only trigonometry.
//!
//! Angles are binary: a full turn is 65536 steps, so wrapping arithmetic on the
//! `u16` is exactly angle arithmetic. Zero points along +X and angles grow
//! counter-clockwise (toward +Y). The sine table is built at first use with an
//! integer Taylor series, so it is bit-identical on every machine.

use crate::Fx;
use std::sync::LazyLock;

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Default, Debug, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
pub struct Angle(pub u16);

const QUARTER: u32 = 0x4000;
const TABLE_SHIFT: u32 = 4;
const TABLE_LEN: usize = (QUARTER >> TABLE_SHIFT) as usize + 1;

/// pi in Q60.
const PI_Q60: i128 = 3_622_009_729_038_561_421;

/// atan(2^-i) where a full turn is 2^32.
const ATAN_TABLE: [u32; 24] = [
    536870912, 316933406, 167458907, 85004756, 42667331, 21354465, 10679838, 5340245, 2670163,
    1335087, 667544, 333772, 166886, 83443, 41722, 20861, 10430, 5215, 2608, 1304, 652, 326, 163,
    81,
];

/// Quarter-wave sine in Q16, `TABLE_LEN` samples over `[0, pi/2]` inclusive.
static SIN_TABLE: LazyLock<[i32; TABLE_LEN]> = LazyLock::new(|| {
    let mut table = [0i32; TABLE_LEN];
    for (i, out) in table.iter_mut().enumerate() {
        let x = PI_Q60 * i as i128 / (2 * (TABLE_LEN as i128 - 1));
        let x2 = (x * x) >> 60;
        let mut term = x;
        let mut sum = x;
        let mut n = 1i128;
        while term != 0 {
            term = -((term * x2) >> 60) / ((2 * n) * (2 * n + 1));
            sum += term;
            n += 1;
        }
        *out = ((sum + (1 << 43)) >> 44) as i32;
    }
    table
});

impl Angle {
    pub const ZERO: Angle = Angle(0);
    pub const QUARTER_TURN: Angle = Angle(0x4000);
    pub const HALF_TURN: Angle = Angle(0x8000);

    /// Whole degrees, any sign or magnitude.
    pub const fn from_degrees(deg: i32) -> Angle {
        Angle(((deg as i64 * 65536).div_euclid(360)) as u16)
    }

    pub fn sin(self) -> Fx {
        let phase = self.0 as u32;
        let quadrant = phase >> 14;
        let mut idx = phase & (QUARTER - 1);
        if quadrant & 1 == 1 {
            idx = QUARTER - idx;
        }
        let i = (idx >> TABLE_SHIFT) as usize;
        let frac = (idx & ((1 << TABLE_SHIFT) - 1)) as i64;
        let a = SIN_TABLE[i] as i64;
        let v = if frac == 0 {
            a
        } else {
            let b = SIN_TABLE[i + 1] as i64;
            a + (((b - a) * frac) >> TABLE_SHIFT)
        };
        Fx(if quadrant >= 2 { -v } else { v })
    }

    #[inline]
    pub fn cos(self) -> Fx {
        Angle(self.0.wrapping_add(0x4000)).sin()
    }

    /// Angle of the vector `(x, y)`. `atan2(0, 0)` is zero.
    pub fn atan2(y: Fx, x: Fx) -> Angle {
        let (mut x, mut y) = (x.0, y.0);
        if x == 0 && y == 0 {
            return Angle::ZERO;
        }
        // Bring the larger component to ~2^40 so every input keeps the same
        // relative precision and the CORDIC gain has headroom.
        let mag = x.unsigned_abs().max(y.unsigned_abs());
        let bits = 64 - mag.leading_zeros() as i32;
        let shift = 40 - bits;
        if shift >= 0 {
            x <<= shift;
            y <<= shift;
        } else {
            x >>= -shift;
            y >>= -shift;
        }
        let mut angle: u32 = 0;
        if x < 0 {
            x = -x;
            y = -y;
            angle = 0x8000_0000;
        }
        for (i, step) in ATAN_TABLE.iter().enumerate() {
            let (dx, dy) = (x >> i, y >> i);
            if y > 0 {
                x += dy;
                y -= dx;
                angle = angle.wrapping_add(*step);
            } else {
                x -= dy;
                y += dx;
                angle = angle.wrapping_sub(*step);
            }
        }
        Angle((angle.wrapping_add(0x8000) >> 16) as u16)
    }

    /// Signed shortest rotation from `self` to `to`, in angle steps.
    #[inline]
    pub fn delta_to(self, to: Angle) -> i16 {
        to.0.wrapping_sub(self.0) as i16
    }

    /// Rotates toward `target` by at most `max_step` angle steps.
    pub fn turn_toward(self, target: Angle, max_step: u16) -> Angle {
        let d = self.delta_to(target);
        if d.unsigned_abs() <= max_step {
            target
        } else if d > 0 {
            Angle(self.0.wrapping_add(max_step))
        } else {
            Angle(self.0.wrapping_sub(max_step))
        }
    }

    /// Presentation only.
    #[inline]
    pub fn to_radians_f32(self) -> f32 {
        self.0 as f32 * (core::f32::consts::TAU / 65536.0)
    }
}

impl core::ops::Add for Angle {
    type Output = Angle;
    #[inline]
    fn add(self, o: Angle) -> Angle {
        Angle(self.0.wrapping_add(o.0))
    }
}

impl core::ops::Sub for Angle {
    type Output = Angle;
    #[inline]
    fn sub(self, o: Angle) -> Angle {
        Angle(self.0.wrapping_sub(o.0))
    }
}

impl core::ops::Neg for Angle {
    type Output = Angle;
    #[inline]
    fn neg(self) -> Angle {
        Angle(self.0.wrapping_neg())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_values_are_exact() {
        assert_eq!(Angle::ZERO.sin(), Fx::ZERO);
        assert_eq!(Angle::ZERO.cos(), Fx::ONE);
        assert_eq!(Angle::QUARTER_TURN.sin(), Fx::ONE);
        assert_eq!(Angle::HALF_TURN.cos(), -Fx::ONE);
        assert_eq!(Angle::from_degrees(270).sin(), -Fx::ONE);
        assert_eq!(Angle::from_degrees(-90), Angle::from_degrees(270));
    }

    #[test]
    fn sin_matches_reference() {
        for step in (0..65536u32).step_by(7) {
            let a = Angle(step as u16);
            let want = (step as f64 * core::f64::consts::TAU / 65536.0).sin();
            assert!((a.sin().to_f64() - want).abs() < 4e-5, "sin({step})");
        }
    }

    #[test]
    fn atan2_round_trips() {
        for step in (0..65536u32).step_by(13) {
            let a = Angle(step as u16);
            for scale in [Fx::ratio(1, 8), Fx::from_int(3), Fx::from_int(50_000)] {
                let got = Angle::atan2(a.sin() * scale, a.cos() * scale);
                assert!(
                    a.delta_to(got).abs() <= 2,
                    "atan2 at {step} scale {scale:?}: {got:?}"
                );
            }
        }
        assert_eq!(Angle::atan2(Fx::ZERO, Fx::ZERO), Angle::ZERO);
        assert_eq!(Angle::atan2(Fx::ONE, Fx::ZERO), Angle::QUARTER_TURN);
        assert_eq!(Angle::atan2(Fx::ZERO, -Fx::ONE), Angle::HALF_TURN);
    }

    #[test]
    fn turning() {
        let a = Angle::from_degrees(350);
        let b = Angle::from_degrees(10);
        assert!(a.delta_to(b) > 0);
        assert_eq!(a.turn_toward(b, 0x4000), b);
        assert_eq!(a.turn_toward(b, 100), Angle(a.0.wrapping_add(100)));
    }
}
