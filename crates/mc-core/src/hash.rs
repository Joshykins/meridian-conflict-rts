//! State hashing for desync detection.
//!
//! The hasher consumes integers, never raw memory, so the result does not
//! depend on endianness, padding or layout.

#[derive(Clone, Copy, Debug)]
pub struct StateHasher {
    h: u64,
}

const K: u64 = 0x9E37_79B9_7F4A_7C15;

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHasher {
    pub const fn new() -> Self {
        Self {
            h: 0xCBF2_9CE4_8422_2325,
        }
    }

    #[inline]
    pub fn write_u64(&mut self, v: u64) {
        let x = (self.h ^ v).wrapping_mul(K);
        self.h = x ^ (x >> 29);
    }

    #[inline]
    pub fn write_i64(&mut self, v: i64) {
        self.write_u64(v as u64);
    }

    #[inline]
    pub fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }

    pub fn write_u64s(&mut self, vs: &[u64]) {
        self.write_u64(vs.len() as u64);
        for &v in vs {
            self.write_u64(v);
        }
    }

    pub fn write_i64s(&mut self, vs: &[i64]) {
        self.write_u64(vs.len() as u64);
        for &v in vs {
            self.write_u64(v as u64);
        }
    }

    pub fn write_u32s(&mut self, vs: &[u32]) {
        self.write_u64(vs.len() as u64);
        for &v in vs {
            self.write_u64(v as u64);
        }
    }

    pub fn write_u16s(&mut self, vs: &[u16]) {
        self.write_u64(vs.len() as u64);
        for &v in vs {
            self.write_u64(v as u64);
        }
    }

    pub fn write_u8s(&mut self, vs: &[u8]) {
        self.write_u64(vs.len() as u64);
        let mut chunks = vs.chunks_exact(8);
        for c in &mut chunks {
            self.write_u64(u64::from_le_bytes(c.try_into().unwrap()));
        }
        let mut tail = [0u8; 8];
        let rest = chunks.remainder();
        tail[..rest.len()].copy_from_slice(rest);
        self.write_u64(u64::from_le_bytes(tail));
    }

    #[inline]
    pub fn finish(&self) -> u64 {
        let mut x = self.h;
        x ^= x >> 32;
        x = x.wrapping_mul(K);
        x ^ (x >> 29)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_length_matter() {
        let mut a = StateHasher::new();
        a.write_u32s(&[1, 2]);
        let mut b = StateHasher::new();
        b.write_u32s(&[2, 1]);
        let mut c = StateHasher::new();
        c.write_u32s(&[1]);
        c.write_u32s(&[2]);
        assert_ne!(a.finish(), b.finish());
        assert_ne!(a.finish(), c.finish());
    }
}
