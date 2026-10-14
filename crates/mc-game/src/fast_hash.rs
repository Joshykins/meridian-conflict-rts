//! Hash maps for the interface's passes over tens of thousands of units, keyed by unit
//! ids, formations and order positions: the standard library's hasher resists crafted
//! keys, which these are not, and costs several times more per key. Interface state only;
//! the simulation keeps to its own ordered tables.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// One multiply a word (the multiplier of rustc's own `FxHasher`).
#[derive(Default, Clone, Copy)]
pub(crate) struct IdHasher(u64);

const SEED: u64 = 0xf135_7aea_2e62_a9c5;

impl IdHasher {
    fn add(&mut self, word: u64) {
        self.0 = self.0.wrapping_add(word).wrapping_mul(SEED);
    }
}

impl Hasher for IdHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }

    fn write_u16(&mut self, n: u16) {
        self.add(n as u64);
    }

    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }

    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }

    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }

    fn finish(&self) -> u64 {
        // The multiply leaves the high bits best mixed; the table takes the low ones.
        self.0.rotate_left(26)
    }
}

pub(crate) type IdMap<K, V> = HashMap<K, V, BuildHasherDefault<IdHasher>>;
pub(crate) type IdSet<K> = HashSet<K, BuildHasherDefault<IdHasher>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_stay_apart() {
        let mut set: IdSet<(u32, u64)> = IdSet::default();
        for id in 0..10_000u32 {
            assert!(set.insert((id, u64::from(id % 7))));
        }
        assert!(!set.insert((5, 5)));
        assert!(set.contains(&(9_999, 9_999 % 7)) && !set.contains(&(9_999, 0)));
        let mut map: IdMap<u32, usize> = IdMap::default();
        map.extend((0..1000u32).map(|i| (i << 18, i as usize)));
        assert_eq!(map.get(&(999 << 18)), Some(&999));
    }
}
