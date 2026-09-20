//! Row allocation for the state tables.
//!
//! A table's rows never move: a handle's index is its row. Freed rows are
//! reused most-recently-freed first, and every reuse bumps the row's
//! generation so stale handles stop resolving. Allocation order is a pure
//! function of the spawn/despawn history, so it is identical on every machine.

use serde::{Deserialize, Serialize};

/// `index | generation << 16`. `Handle::NONE` never resolves.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Handle(pub u32);

impl Default for Handle {
    fn default() -> Self {
        Handle::NONE
    }
}

impl Handle {
    pub const NONE: Handle = Handle(u32::MAX);

    #[inline]
    pub fn new(index: usize, generation: u16) -> Handle {
        Handle(index as u32 | (generation as u32) << 16)
    }

    #[inline]
    pub fn index(self) -> usize {
        (self.0 & 0xFFFF) as usize
    }

    #[inline]
    pub fn generation(self) -> u16 {
        (self.0 >> 16) as u16
    }

    #[inline]
    pub fn is_none(self) -> bool {
        self == Handle::NONE
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slots {
    generation: Vec<u16>,
    alive: Vec<bool>,
    free: Vec<u32>,
    live: u32,
    capacity: u32,
}

impl Slots {
    pub fn new(capacity: usize) -> Slots {
        assert!(capacity <= 0xFFFF, "handle index is 16 bits");
        Slots {
            generation: Vec::new(),
            alive: Vec::new(),
            free: Vec::new(),
            live: 0,
            capacity: capacity as u32,
        }
    }

    /// Returns the row to fill, or `None` when the table is full.
    pub fn alloc(&mut self) -> Option<usize> {
        let row = match self.free.pop() {
            Some(row) => row as usize,
            None => {
                if self.alive.len() as u32 >= self.capacity {
                    return None;
                }
                self.alive.push(false);
                self.generation.push(0);
                self.alive.len() - 1
            }
        };
        self.alive[row] = true;
        self.live += 1;
        Some(row)
    }

    pub fn free(&mut self, row: usize) {
        debug_assert!(self.alive[row]);
        self.alive[row] = false;
        // Generation 0xFFFF is skipped so no live handle can equal `Handle::NONE`.
        self.generation[row] = (self.generation[row] + 1) % 0xFFFF;
        self.free.push(row as u32);
        self.live -= 1;
    }

    #[inline]
    pub fn handle(&self, row: usize) -> Handle {
        Handle::new(row, self.generation[row])
    }

    #[inline]
    pub fn resolve(&self, h: Handle) -> Option<usize> {
        let row = h.index();
        (row < self.alive.len() && self.alive[row] && self.generation[row] == h.generation())
            .then_some(row)
    }

    #[inline]
    pub fn is_alive(&self, row: usize) -> bool {
        self.alive[row]
    }

    /// Rows ever allocated; columns are this long.
    #[inline]
    pub fn rows(&self) -> usize {
        self.alive.len()
    }

    #[inline]
    pub fn live(&self) -> usize {
        self.live as usize
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity as usize
    }

    /// Live rows in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.alive
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.then_some(i))
    }

    pub fn hash(&self, h: &mut mc_core::StateHasher) {
        h.write_u16s(&self.generation);
        h.write_u32s(&self.free);
        h.write_u64(self.live as u64);
    }
}

/// Writes `value` into `column[row]`, growing the column when the row is new.
#[inline]
pub fn put<T>(column: &mut Vec<T>, row: usize, value: T) {
    if row == column.len() {
        column.push(value);
    } else {
        column[row] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handles_do_not_resolve() {
        let mut s = Slots::new(4);
        let a = s.alloc().unwrap();
        let ha = s.handle(a);
        assert_eq!(s.resolve(ha), Some(a));
        s.free(a);
        assert_eq!(s.resolve(ha), None);
        let b = s.alloc().unwrap();
        assert_eq!(a, b, "most recently freed row is reused");
        assert_ne!(s.handle(b), ha);
        assert_eq!(s.resolve(Handle::NONE), None);
    }

    #[test]
    fn capacity_is_enforced() {
        let mut s = Slots::new(2);
        assert!(s.alloc().is_some());
        assert!(s.alloc().is_some());
        assert!(s.alloc().is_none());
        assert_eq!(s.live(), 2);
    }
}
