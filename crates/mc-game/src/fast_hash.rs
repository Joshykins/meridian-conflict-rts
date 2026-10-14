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

/// A map from unit ids (sim handles) to `V`, by the handle's row (`Handle::index`): one
/// step and no hashing while a row holds one id, and a side's units, met in id order,
/// are met in the order they lie. An id whose row another holds waits in a hash map.
pub(crate) struct RowMap<V> {
    slots: Vec<Option<(u32, V)>>,
    /// The rows written since the last clear, to clear and walk without the empty ones.
    used: Vec<u32>,
    spill: IdMap<u32, V>,
}

impl<V> Default for RowMap<V> {
    fn default() -> Self {
        RowMap {
            slots: Vec::new(),
            used: Vec::new(),
            spill: IdMap::default(),
        }
    }
}

impl<V: Copy> RowMap<V> {
    fn row(id: u32) -> usize {
        mc_sim::Handle(id).index()
    }

    pub(crate) fn get(&self, id: u32) -> Option<V> {
        match self.slots.get(Self::row(id)) {
            Some(&Some((held, v))) if held == id => Some(v),
            _ if self.spill.is_empty() => None,
            _ => self.spill.get(&id).copied(),
        }
    }

    /// Sets `id`'s value; returns the one it had.
    pub(crate) fn insert(&mut self, id: u32, v: V) -> Option<V> {
        let row = Self::row(id);
        if row >= self.slots.len() {
            self.slots.resize(row + 1, None);
        }
        match &mut self.slots[row] {
            Some((held, old)) if *held == id => Some(std::mem::replace(old, v)),
            Some(_) => self.spill.insert(id, v),
            empty => {
                *empty = Some((id, v));
                self.used.push(row as u32);
                if self.spill.is_empty() {
                    None
                } else {
                    self.spill.remove(&id)
                }
            }
        }
    }

    pub(crate) fn remove(&mut self, id: u32) -> Option<V> {
        match self.slots.get_mut(Self::row(id)) {
            Some(slot @ Some(_)) if slot.is_some_and(|(held, _)| held == id) => {
                slot.take().map(|(_, v)| v)
            }
            _ if self.spill.is_empty() => None,
            _ => self.spill.remove(&id),
        }
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(u32, V) -> bool) {
        for &row in &self.used {
            let slot = &mut self.slots[row as usize];
            if slot.is_some_and(|(id, v)| !keep(id, v)) {
                *slot = None;
            }
        }
        self.spill.retain(|&id, v| keep(id, *v));
    }

    pub(crate) fn clear(&mut self) {
        for &row in &self.used {
            self.slots[row as usize] = None;
        }
        self.used.clear();
        self.spill.clear();
    }
}

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

    #[test]
    fn a_row_map_is_a_map() {
        let mut map: RowMap<u64> = RowMap::default();
        // Row 5 for two generations: the second waits on the side.
        let (a, b) = (mc_sim::Handle::new(5, 0).0, mc_sim::Handle::new(5, 1).0);
        assert_eq!(map.insert(a, 1), None);
        assert_eq!(map.insert(b, 2), None);
        assert_eq!(map.insert(9, 3), None);
        assert_eq!(map.insert(a, 4), Some(1));
        assert_eq!(
            (map.get(a), map.get(b), map.get(9), map.get(7)),
            (Some(4), Some(2), Some(3), None)
        );
        assert_eq!(map.remove(a), Some(4));
        assert_eq!(map.get(a), None);
        // The row is free: `b` takes it, coming back from the side with its value.
        assert_eq!(map.insert(b, 5), Some(2));
        assert_eq!(map.get(b), Some(5));
        map.retain(|id, _| id != 9);
        assert_eq!((map.get(9), map.get(b)), (None, Some(5)));
        map.clear();
        assert_eq!(map.get(b), None);
    }
}
