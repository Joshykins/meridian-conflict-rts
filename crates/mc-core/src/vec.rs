//! Fixed-point vectors. The ground plane is XY; Z is up.

use crate::{Angle, Fx};
use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct FxVec2 {
    pub x: Fx,
    pub y: Fx,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct FxVec3 {
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
}

impl FxVec2 {
    pub const ZERO: FxVec2 = FxVec2 { x: Fx::ZERO, y: Fx::ZERO };

    #[inline]
    pub const fn new(x: Fx, y: Fx) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn from_ints(x: i32, y: i32) -> Self {
        Self { x: Fx::from_int(x), y: Fx::from_int(y) }
    }

    /// Unit vector pointing along `angle`.
    #[inline]
    pub fn from_angle(angle: Angle) -> Self {
        Self { x: angle.cos(), y: angle.sin() }
    }

    #[inline]
    pub fn dot(self, o: Self) -> Fx {
        self.x * o.x + self.y * o.y
    }

    /// Z component of the 3D cross product; positive when `o` is counter-clockwise of `self`.
    #[inline]
    pub fn cross(self, o: Self) -> Fx {
        self.x * o.y - self.y * o.x
    }

    #[inline]
    pub fn length_sq(self) -> Fx {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> Fx {
        let (x, y) = (self.x.0 as i128, self.y.0 as i128);
        Fx(((x * x + y * y) as u128).isqrt() as i64)
    }

    #[inline]
    pub fn distance(self, o: Self) -> Fx {
        (self - o).length()
    }

    #[inline]
    pub fn distance_sq(self, o: Self) -> Fx {
        (self - o).length_sq()
    }

    /// Zero stays zero.
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == Fx::ZERO {
            Self::ZERO
        } else {
            Self { x: self.x / len, y: self.y / len }
        }
    }

    /// Scales the vector down so its length is at most `max`.
    pub fn clamp_length(self, max: Fx) -> Self {
        let len = self.length();
        if len <= max || len == Fx::ZERO {
            self
        } else {
            Self { x: self.x * max / len, y: self.y * max / len }
        }
    }

    #[inline]
    pub fn angle(self) -> Angle {
        Angle::atan2(self.y, self.x)
    }

    pub fn rotate(self, angle: Angle) -> Self {
        let (s, c) = (angle.sin(), angle.cos());
        Self { x: self.x * c - self.y * s, y: self.x * s + self.y * c }
    }

    /// Counter-clockwise perpendicular.
    #[inline]
    pub fn perp(self) -> Self {
        Self { x: -self.y, y: self.x }
    }

    #[inline]
    pub fn lerp(self, to: Self, t: Fx) -> Self {
        Self { x: self.x.lerp(to.x, t), y: self.y.lerp(to.y, t) }
    }

    #[inline]
    pub fn extend(self, z: Fx) -> FxVec3 {
        FxVec3 { x: self.x, y: self.y, z }
    }

    /// Presentation only.
    #[inline]
    pub fn to_f32(self) -> [f32; 2] {
        [self.x.to_f32(), self.y.to_f32()]
    }
}

impl FxVec3 {
    pub const ZERO: FxVec3 = FxVec3 { x: Fx::ZERO, y: Fx::ZERO, z: Fx::ZERO };

    #[inline]
    pub const fn new(x: Fx, y: Fx, z: Fx) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn xy(self) -> FxVec2 {
        FxVec2 { x: self.x, y: self.y }
    }

    #[inline]
    pub fn dot(self, o: Self) -> Fx {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn length_sq(self) -> Fx {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> Fx {
        let (x, y, z) = (self.x.0 as i128, self.y.0 as i128, self.z.0 as i128);
        Fx(((x * x + y * y + z * z) as u128).isqrt() as i64)
    }

    #[inline]
    pub fn distance(self, o: Self) -> Fx {
        (self - o).length()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len == Fx::ZERO {
            Self::ZERO
        } else {
            Self { x: self.x / len, y: self.y / len, z: self.z / len }
        }
    }

    #[inline]
    pub fn lerp(self, to: Self, t: Fx) -> Self {
        Self { x: self.x.lerp(to.x, t), y: self.y.lerp(to.y, t), z: self.z.lerp(to.z, t) }
    }

    /// Presentation only.
    #[inline]
    pub fn to_f32(self) -> [f32; 3] {
        [self.x.to_f32(), self.y.to_f32(), self.z.to_f32()]
    }
}

macro_rules! vec_ops {
    ($t:ident, $($f:ident),+) => {
        impl Add for $t {
            type Output = $t;
            #[inline]
            fn add(self, o: $t) -> $t {
                $t { $($f: self.$f + o.$f),+ }
            }
        }
        impl Sub for $t {
            type Output = $t;
            #[inline]
            fn sub(self, o: $t) -> $t {
                $t { $($f: self.$f - o.$f),+ }
            }
        }
        impl Neg for $t {
            type Output = $t;
            #[inline]
            fn neg(self) -> $t {
                $t { $($f: -self.$f),+ }
            }
        }
        impl Mul<Fx> for $t {
            type Output = $t;
            #[inline]
            fn mul(self, s: Fx) -> $t {
                $t { $($f: self.$f * s),+ }
            }
        }
        impl AddAssign for $t {
            #[inline]
            fn add_assign(&mut self, o: $t) {
                $(self.$f += o.$f;)+
            }
        }
        impl SubAssign for $t {
            #[inline]
            fn sub_assign(&mut self, o: $t) {
                $(self.$f -= o.$f;)+
            }
        }
    };
}

vec_ops!(FxVec2, x, y);
vec_ops!(FxVec3, x, y, z);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        let v = FxVec2::from_ints(3, 4);
        assert_eq!(v.length(), Fx::from_int(5));
        assert_eq!(v.clamp_length(Fx::from_int(10)), v);
        let n = v.normalize();
        assert!((n.length() - Fx::ONE).abs() <= Fx(2));
        assert_eq!(FxVec2::ZERO.normalize(), FxVec2::ZERO);
    }

    #[test]
    fn rotation() {
        let v = FxVec2::from_ints(1, 0).rotate(Angle::QUARTER_TURN);
        assert_eq!(v, FxVec2::from_ints(0, 1));
        assert_eq!(FxVec2::from_ints(0, 2).angle(), Angle::QUARTER_TURN);
        assert!(FxVec2::from_ints(1, 0).cross(FxVec2::from_ints(0, 1)) > Fx::ZERO);
    }
}
