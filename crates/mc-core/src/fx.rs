//! `Fx`: signed 47.16 fixed-point number in an `i64`.
//!
//! One unit is one metre. The 16 fractional bits give ~0.015 mm resolution and
//! the integer range covers an 80 km map many times over. Multiplication and
//! division widen to `i128`, so no product of two in-range values can overflow.

use core::fmt;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
pub struct Fx(pub i64);

impl Fx {
    pub const FRAC_BITS: u32 = 16;
    pub const ZERO: Fx = Fx(0);
    pub const ONE: Fx = Fx(1 << Self::FRAC_BITS);
    pub const HALF: Fx = Fx(1 << (Self::FRAC_BITS - 1));
    pub const EPSILON: Fx = Fx(1);
    pub const MAX: Fx = Fx(i64::MAX);
    pub const MIN: Fx = Fx(i64::MIN);

    #[inline]
    pub const fn from_int(v: i32) -> Fx {
        Fx((v as i64) << Self::FRAC_BITS)
    }

    /// `num / den` as a fixed-point value. Handy for data constants: `Fx::ratio(3, 10)`.
    #[inline]
    pub const fn ratio(num: i64, den: i64) -> Fx {
        Fx((num << Self::FRAC_BITS) / den)
    }

    /// Thousandths: `Fx::milli(1500)` is 1.5. Blueprint data is authored in this form.
    #[inline]
    pub const fn milli(v: i64) -> Fx {
        Self::ratio(v, 1000)
    }

    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }

    /// Rounds toward negative infinity.
    #[inline]
    pub const fn floor_int(self) -> i32 {
        (self.0 >> Self::FRAC_BITS) as i32
    }

    #[inline]
    pub const fn ceil_int(self) -> i32 {
        ((self.0 + (Self::ONE.0 - 1)) >> Self::FRAC_BITS) as i32
    }

    #[inline]
    pub const fn round_int(self) -> i32 {
        ((self.0 + Self::HALF.0) >> Self::FRAC_BITS) as i32
    }

    /// Fractional part in `[0, 1)`.
    #[inline]
    pub const fn fract(self) -> Fx {
        Fx(self.0 & (Self::ONE.0 - 1))
    }

    #[inline]
    pub const fn abs(self) -> Fx {
        Fx(self.0.abs())
    }

    #[inline]
    pub fn min(self, o: Fx) -> Fx {
        Fx(self.0.min(o.0))
    }

    #[inline]
    pub fn max(self, o: Fx) -> Fx {
        Fx(self.0.max(o.0))
    }

    #[inline]
    pub fn clamp(self, lo: Fx, hi: Fx) -> Fx {
        Fx(self.0.clamp(lo.0, hi.0))
    }

    #[inline]
    pub const fn signum(self) -> i64 {
        self.0.signum()
    }

    /// Square root. Negative inputs return zero.
    #[inline]
    pub fn sqrt(self) -> Fx {
        if self.0 <= 0 {
            return Fx::ZERO;
        }
        Fx(isqrt((self.0 as u128) << Self::FRAC_BITS) as i64)
    }

    /// `self + (to - self) * t`.
    #[inline]
    pub fn lerp(self, to: Fx, t: Fx) -> Fx {
        self + (to - self) * t
    }

    /// Moves `self` toward `target` by at most `max_delta`.
    #[inline]
    pub fn approach(self, target: Fx, max_delta: Fx) -> Fx {
        let d = target - self;
        if d.abs() <= max_delta {
            target
        } else if d.0 > 0 {
            self + max_delta
        } else {
            self - max_delta
        }
    }

    /// `self * num / den` without losing precision to an intermediate `Fx`.
    #[inline]
    pub fn mul_div(self, num: i64, den: i64) -> Fx {
        // The same quotient in 64 bits when the product fits: a 128-bit division
        // is a slow library call.
        match self.0.checked_mul(num).and_then(|p| p.checked_div(den)) {
            Some(q) => Fx(q),
            None => Fx(((self.0 as i128 * num as i128) / den as i128) as i64),
        }
    }

    /// Presentation only. Never feed the result back into the simulation.
    #[inline]
    pub fn to_f32(self) -> f32 {
        (self.0 as f64 / Self::ONE.0 as f64) as f32
    }

    /// Presentation only. Never feed the result back into the simulation.
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::ONE.0 as f64
    }

    /// For tools and UI input (e.g. a mouse pick). The result is only
    /// deterministic once it has been written into a command.
    #[inline]
    pub fn from_f32(v: f32) -> Fx {
        Fx((v as f64 * Self::ONE.0 as f64) as i64)
    }
}

impl Add for Fx {
    type Output = Fx;
    #[inline]
    fn add(self, o: Fx) -> Fx {
        Fx(self.0 + o.0)
    }
}

impl Sub for Fx {
    type Output = Fx;
    #[inline]
    fn sub(self, o: Fx) -> Fx {
        Fx(self.0 - o.0)
    }
}

impl Neg for Fx {
    type Output = Fx;
    #[inline]
    fn neg(self) -> Fx {
        Fx(-self.0)
    }
}

impl Mul for Fx {
    type Output = Fx;
    #[inline]
    fn mul(self, o: Fx) -> Fx {
        Fx(((self.0 as i128 * o.0 as i128) >> Self::FRAC_BITS) as i64)
    }
}

impl Mul<i32> for Fx {
    type Output = Fx;
    #[inline]
    fn mul(self, o: i32) -> Fx {
        Fx(self.0 * o as i64)
    }
}

impl Div for Fx {
    type Output = Fx;
    #[inline]
    fn div(self, o: Fx) -> Fx {
        // The same quotient in 64 bits while the shifted dividend fits: a
        // 128-bit division is a slow library call, and this one is everywhere.
        if self.0.unsigned_abs() < 1 << (63 - Self::FRAC_BITS) {
            Fx((self.0 << Self::FRAC_BITS) / o.0)
        } else {
            Fx((((self.0 as i128) << Self::FRAC_BITS) / o.0 as i128) as i64)
        }
    }
}

impl Div<i32> for Fx {
    type Output = Fx;
    #[inline]
    fn div(self, o: i32) -> Fx {
        Fx(self.0 / o as i64)
    }
}

impl AddAssign for Fx {
    #[inline]
    fn add_assign(&mut self, o: Fx) {
        self.0 += o.0;
    }
}

impl SubAssign for Fx {
    #[inline]
    fn sub_assign(&mut self, o: Fx) {
        self.0 -= o.0;
    }
}

impl MulAssign for Fx {
    #[inline]
    fn mul_assign(&mut self, o: Fx) {
        *self = *self * o;
    }
}

impl DivAssign for Fx {
    #[inline]
    fn div_assign(&mut self, o: Fx) {
        *self = *self / o;
    }
}

impl core::iter::Sum for Fx {
    fn sum<I: Iterator<Item = Fx>>(iter: I) -> Fx {
        Fx(iter.map(|f| f.0).sum())
    }
}

impl fmt::Debug for Fx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.to_f64())
    }
}

impl fmt::Display for Fx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_f64(), f)
    }
}

/// `n.isqrt()`, taken in 64 bits when `n` fits.
#[inline]
pub(crate) fn isqrt(n: u128) -> u128 {
    match u64::try_from(n) {
        Ok(n) => n.isqrt() as u128,
        Err(_) => n.isqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 64-bit short cuts give the 128-bit answers, at their edges too.
    #[test]
    fn short_cuts_match_wide_maths() {
        let edge = 1i64 << (63 - Fx::FRAC_BITS);
        let mut values = vec![
            0,
            1,
            -1,
            7,
            -7,
            65_535,
            65_536,
            -65_536,
            i64::MAX,
            i64::MIN + 1,
        ];
        values.extend([edge - 1, edge, edge + 1, -edge + 1, -edge, -edge - 1]);
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..2000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            values.push((x as i64) >> (x % 40));
        }
        for &a in &values {
            for &b in &values {
                if b != 0 {
                    let wide = (((a as i128) << Fx::FRAC_BITS) / b as i128) as i64;
                    assert_eq!((Fx(a) / Fx(b)).0, wide, "{a} / {b}");
                    let wide = ((a as i128 * 3) / b as i128) as i64;
                    assert_eq!(Fx(a).mul_div(3, b).0, wide, "{a} * 3 / {b}");
                    let wide = ((a as i128 * b as i128) / -1i128) as i64;
                    assert_eq!(Fx(a).mul_div(b, -1).0, wide, "{a} * {b} / -1");
                }
            }
            let n = (a as i128 * a as i128) as u128;
            assert_eq!(isqrt(n), n.isqrt());
            assert_eq!(isqrt(n + 1), (n + 1).isqrt());
            assert_eq!(
                isqrt(a.unsigned_abs() as u128),
                (a.unsigned_abs() as u128).isqrt()
            );
        }
        assert_eq!(isqrt(u64::MAX as u128), (u64::MAX as u128).isqrt());
        assert_eq!(isqrt(u64::MAX as u128 + 1), 1 << 32);
    }

    #[test]
    fn arithmetic() {
        let a = Fx::ratio(3, 2);
        let b = Fx::from_int(4);
        assert_eq!(a * b, Fx::from_int(6));
        assert_eq!(b / a, Fx::ratio(8, 3));
        assert_eq!((-a).abs(), a);
        assert_eq!(Fx::milli(2500), Fx::ratio(5, 2));
    }

    #[test]
    fn rounding() {
        assert_eq!(Fx::ratio(-1, 2).floor_int(), -1);
        assert_eq!(Fx::ratio(-1, 2).ceil_int(), 0);
        assert_eq!(Fx::ratio(5, 2).round_int(), 3);
        assert_eq!(Fx::ratio(7, 4).fract(), Fx::ratio(3, 4));
    }

    #[test]
    fn sqrt_is_exact_on_squares() {
        for i in 0..2000 {
            let v = Fx::from_int(i);
            assert_eq!((v * v).sqrt(), v);
        }
        assert_eq!(Fx::from_int(-4).sqrt(), Fx::ZERO);
    }

    #[test]
    fn map_scale_products_do_not_overflow() {
        let edge = Fx::from_int(81_920);
        let d2 = edge * edge + edge * edge;
        assert_eq!(d2.sqrt().floor_int(), 115_852);
    }
}
