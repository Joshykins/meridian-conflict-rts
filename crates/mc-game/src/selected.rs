//! The selection: unit ids in the order they were picked, with a sorted copy so
//! "is this unit selected?" is a binary search. The interface asks that of every
//! unit on the map each frame (the minimap, the marks, the orders), and a linear
//! search there made a few thousand selected units cost tens of milliseconds.

use std::ops::Deref;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// In the order picked: the first leads (the details card, the camera's track).
    ids: Vec<u32>,
    /// The same ids, ascending.
    sorted: Vec<u32>,
}

impl Selection {
    pub fn contains(&self, id: &u32) -> bool {
        self.sorted.binary_search(id).is_ok()
    }

    /// Adds `id` at the end; an id already selected stays where it is.
    pub fn push(&mut self, id: u32) {
        if let Err(at) = self.sorted.binary_search(&id) {
            self.sorted.insert(at, id);
            self.ids.push(id);
        }
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.sorted.clear();
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&u32) -> bool) {
        self.ids.retain(|id| keep(id));
        self.resort();
    }

    pub fn truncate(&mut self, n: usize) {
        self.ids.truncate(n);
        self.resort();
    }

    fn resort(&mut self) {
        self.sorted.clone_from(&self.ids);
        self.sorted.sort_unstable();
    }
}

impl Deref for Selection {
    type Target = [u32];
    fn deref(&self) -> &[u32] {
        &self.ids
    }
}

impl<'a> IntoIterator for &'a Selection {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;
    fn into_iter(self) -> Self::IntoIter {
        self.ids.iter()
    }
}

/// Duplicates are dropped, the first kept.
impl From<Vec<u32>> for Selection {
    fn from(ids: Vec<u32>) -> Selection {
        ids.into_iter().collect()
    }
}

impl FromIterator<u32> for Selection {
    fn from_iter<I: IntoIterator<Item = u32>>(ids: I) -> Selection {
        let mut out = Selection::default();
        for id in ids {
            out.push(id);
        }
        out
    }
}

impl PartialEq<Vec<u32>> for Selection {
    fn eq(&self, other: &Vec<u32>) -> bool {
        self.ids == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_pick_order_and_answers_membership() {
        let mut s: Selection = vec![9, 3, 7, 3].into();
        assert_eq!(s, vec![9, 3, 7]);
        assert!(s.contains(&7) && !s.contains(&4));
        s.push(4);
        s.push(9);
        assert_eq!(s, vec![9, 3, 7, 4]);
        s.retain(|&id| id != 3);
        assert_eq!(s, vec![9, 7, 4]);
        assert!(!s.contains(&3) && s.contains(&4));
        s.truncate(1);
        assert_eq!(s, vec![9]);
        assert!(!s.contains(&7));
        s.clear();
        assert!(s.is_empty() && !s.contains(&9));
    }
}
