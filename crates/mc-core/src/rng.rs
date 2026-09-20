//! PCG32. The generator state is part of game state and is hashed every tick.

use crate::Fx;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Rng {
    state: u64,
}

const MULTIPLIER: u64 = 6364136223846793005;
const INCREMENT: u64 = 1442695040888963407;

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut rng = Rng {
            state: seed.wrapping_add(INCREMENT),
        };
        rng.next_u32();
        rng
    }

    #[inline]
    pub fn state(&self) -> u64 {
        self.state
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        xorshifted.rotate_right((old >> 59) as u32)
    }

    /// Uniform in `0..bound`. `bound` must be non-zero.
    #[inline]
    pub fn below(&mut self, bound: u32) -> u32 {
        ((self.next_u32() as u64 * bound as u64) >> 32) as u32
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn unit(&mut self) -> Fx {
        Fx((self.next_u32() >> 16) as i64)
    }

    /// Uniform in `[lo, hi)`.
    #[inline]
    pub fn range(&mut self, lo: Fx, hi: Fx) -> Fx {
        lo + (hi - lo) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        assert_ne!(Rng::new(1).next_u32(), Rng::new(2).next_u32());
    }

    #[test]
    fn bounds_hold() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            assert!(r.below(13) < 13);
            let u = r.unit();
            assert!(u >= Fx::ZERO && u < Fx::ONE);
        }
    }
}
