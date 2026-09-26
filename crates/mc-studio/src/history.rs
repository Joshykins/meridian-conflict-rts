//! Undo and redo as whole-song snapshots.
//!
//! A song is small (a few thousand notes at most), so keeping copies is
//! simpler and safer than inverse operations for every kind of edit. Edits
//! made during one gesture (a knob drag, a note drag, typing into a field) are
//! folded into one step: the snapshot from before the gesture is pushed when
//! the first change is seen, and later changes in the same gesture only move
//! the working copy.

pub struct History<T: Clone + PartialEq> {
    undo: Vec<T>,
    redo: Vec<T>,
    /// The state the last step ended at.
    committed: T,
    /// A change is in progress and its step is already on the undo stack.
    pending: bool,
    limit: usize,
}

impl<T: Clone + PartialEq> History<T> {
    pub fn new(initial: &T) -> Self {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            committed: initial.clone(),
            pending: false,
            limit: 300,
        }
    }

    /// Forgets everything (a different file was opened).
    pub fn reset(&mut self, initial: &T) {
        *self = History::new(initial);
    }

    /// Called once per frame with the working copy. `gesture` is true while a
    /// pointer button is held or a text field has focus: the step stays open.
    pub fn observe(&mut self, current: &T, gesture: bool) {
        if !self.pending && *current != self.committed {
            self.undo.push(self.committed.clone());
            if self.undo.len() > self.limit {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.pending = true;
        }
        if self.pending && !gesture {
            self.committed = current.clone();
            self.pending = false;
        }
    }

    /// Closes an open step now (before an undo, a save or a reload).
    pub fn settle(&mut self, current: &T) {
        self.observe(current, false);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Steps back: returns the state to show.
    pub fn undo(&mut self, current: &T) -> Option<T> {
        self.settle(current);
        let prev = self.undo.pop()?;
        self.redo
            .push(std::mem::replace(&mut self.committed, prev.clone()));
        Some(prev)
    }

    pub fn redo(&mut self, current: &T) -> Option<T> {
        self.settle(current);
        let next = self.redo.pop()?;
        self.undo
            .push(std::mem::replace(&mut self.committed, next.clone()));
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_is_one_step() {
        let mut h = History::new(&0);
        h.observe(&0, false);
        h.observe(&1, true);
        h.observe(&2, true);
        h.observe(&3, true);
        h.observe(&3, false);
        assert_eq!(h.undo(&3), Some(0));
        assert!(!h.can_undo());
        assert_eq!(h.redo(&0), Some(3));
    }

    #[test]
    fn separate_edits_are_separate_steps() {
        let mut h = History::new(&0);
        h.observe(&1, false);
        h.observe(&2, false);
        assert_eq!(h.undo(&2), Some(1));
        assert_eq!(h.undo(&1), Some(0));
        assert_eq!(h.undo(&0), None);
        assert_eq!(h.redo(&0), Some(1));
        assert_eq!(h.redo(&1), Some(2));
        assert_eq!(h.redo(&2), None);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut h = History::new(&0);
        h.observe(&1, false);
        assert_eq!(h.undo(&1), Some(0));
        h.observe(&5, false);
        assert!(!h.can_redo());
        assert_eq!(h.undo(&5), Some(0));
    }

    #[test]
    fn undo_mid_gesture_closes_the_step_first() {
        let mut h = History::new(&0);
        h.observe(&1, true);
        h.observe(&2, true);
        // Ctrl+Z while the mouse is still down: the drag so far is one step.
        assert_eq!(h.undo(&2), Some(0));
        assert_eq!(h.redo(&0), Some(2));
    }

    #[test]
    fn nothing_changed_records_nothing() {
        let mut h = History::new(&7);
        for _ in 0..10 {
            h.observe(&7, true);
            h.observe(&7, false);
        }
        assert!(!h.can_undo());
    }
}
