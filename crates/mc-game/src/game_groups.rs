//! Control groups, keys 1-9 and 0:
//!
//! - Ctrl+N makes the selection group N.
//! - Ctrl+Shift+N adds the selection to group N.
//! - N selects group N, and N twice quickly also brings the camera to it.
//! - Shift+N adds group N to the selection.
//!
//! A unit is in one group at most: putting it in another takes it out of the
//! one it was in, so the number drawn by it (`hud/groups.rs`) is never in doubt.

use super::Game;
use crate::audio::{Audio, Sfx};
use std::time::Instant;

/// A group's key pressed twice within this long also brings the camera.
const DOUBLE_TAP: f32 = 0.35;

/// The groups' members, by key (index 0 is the 0 key, the last in the row).
#[derive(Default, Clone)]
pub struct ControlGroups {
    members: [Vec<u32>; 10],
}

impl ControlGroups {
    /// Group `n` becomes `units`, which leave any other group.
    pub fn set(&mut self, n: usize, units: &[u32]) {
        self.take_out(units);
        self.members[n] = units.to_vec();
    }

    /// `units` join group `n`, leaving any other group.
    pub fn add(&mut self, n: usize, units: &[u32]) {
        self.take_out(units);
        self.members[n].extend_from_slice(units);
    }

    fn take_out(&mut self, units: &[u32]) {
        for group in &mut self.members {
            group.retain(|id| !units.contains(id));
        }
    }

    pub fn get(&self, n: usize) -> &[u32] {
        &self.members[n]
    }

    /// The groups that hold something, in keyboard order: 1..9, then 0.
    pub fn filled(&self) -> impl Iterator<Item = (usize, &[u32])> {
        (1..10)
            .chain([0])
            .map(|n| (n, self.members[n].as_slice()))
            .filter(|(_, m)| !m.is_empty())
    }

    /// Keeps the members `keep` says yes to (the rest died or went away).
    pub fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        for group in &mut self.members {
            group.retain(|&id| keep(id));
        }
    }
}

impl Game {
    /// Digit `n` pressed, with the modifiers held now.
    pub(super) fn group_key(&mut self, n: usize, audio: &Audio) {
        let groups = &mut self.view.groups;
        match (self.ctrl, self.shift) {
            (true, false) => {
                groups.set(n, &self.view.selection);
                audio.play(Sfx::Tick);
            }
            (true, true) => {
                if !self.view.selection.is_empty() {
                    groups.add(n, &self.view.selection);
                    audio.play(Sfx::Tick);
                }
            }
            (false, true) => {
                let mut added = false;
                for &id in groups.get(n) {
                    if !self.view.selection.contains(&id) {
                        self.view.selection.push(id);
                        added = true;
                    }
                }
                if added {
                    audio.play(Sfx::Select);
                }
            }
            (false, false) => {
                if groups.get(n).is_empty() {
                    return;
                }
                self.view.selection = groups.get(n).to_vec();
                audio.play(Sfx::Select);
                let now = Instant::now();
                if self
                    .last_group
                    .is_some_and(|(key, at)| key == n && (now - at).as_secs_f32() < DOUBLE_TAP)
                {
                    self.focus_selection();
                }
                self.last_group = Some((n, now));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ControlGroups;

    #[test]
    fn a_unit_is_in_one_group_at_most() {
        let mut g = ControlGroups::default();
        g.set(1, &[10, 11, 12]);
        g.set(2, &[12, 13]);
        assert_eq!(g.get(1), &[10, 11]);
        assert_eq!(g.get(2), &[12, 13]);
        g.add(1, &[13, 14]);
        assert_eq!(g.get(1), &[10, 11, 13, 14]);
        assert_eq!(g.get(2), &[12]);
        // Setting a group to nothing empties it.
        g.set(2, &[]);
        assert!(g.get(2).is_empty());
        g.set(0, &[20]);
        let keys: Vec<usize> = g.filled().map(|(n, _)| n).collect();
        assert_eq!(keys, [1, 0]);
        g.retain(|id| id != 10);
        assert_eq!(g.get(1), &[11, 13, 14]);
    }
}
