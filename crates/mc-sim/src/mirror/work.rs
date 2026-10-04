//! Work going up, as the presentation sees it: which work a drawn unit's progress is
//! (its own site, or the hidden successor of an upgrade or a refit), and how long that
//! work has left. One answer for the unit's bar, its tag and a builder's status line,
//! keyed by the unit as drawn, so the time left never depends on who is building it.

use super::*;
use crate::tables::flag;

/// A unit going up or upgrading with its work moving, for the interface's time left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkLeft {
    /// The unit as drawn: the site itself, or the unit being upgraded or refitted.
    pub unit_id: u32,
    /// Seconds of game time before the work is done, at last tick's pace.
    pub seconds: f32,
}

impl World {
    /// The hidden successor a structure is assembling in place, when there is one.
    /// Factories, extractors, intel towers and shield generators are refitted
    /// like a mobile unit: the next kit is built onto them.
    pub(super) fn structure_upgrade(&self, row: usize) -> Option<usize> {
        if self.upgrades_in_place(row) {
            return None;
        }
        let units = &self.state.units;
        units.row(units.build_target[row]).filter(|&t| {
            units.has_flag(t, flag::UPGRADE) && units.has_flag(t, flag::UNDER_CONSTRUCTION)
        })
    }

    /// The successor `row` is becoming, a structure's upgrade or a refit's new body,
    /// drawn as `row` itself: its progress is `UnitInstance::upgrade`.
    pub(super) fn upgrade_work(&self, row: usize) -> Option<usize> {
        let s = &self.state;
        self.structure_upgrade(row).or_else(|| {
            s.orders
                .front(&s.units, row)
                .filter(|o| o.kind == crate::tables::OrderKind::Upgrade)
                .filter(|_| self.upgrades_in_place(row))
                .and_then(|_| s.units.row(s.units.build_target[row]))
        })
    }

    /// Seconds before work `t` is done at last tick's pace, every builder on it
    /// counted; `None` while it stands still.
    pub(super) fn work_left(&self, t: usize) -> Option<f32> {
        let pace = self.flows.get(t).map_or(Fx::ZERO, |f| f.built) * TICKS_PER_SECOND as i32;
        let left = self.bp(t).build_time - self.state.units.build_progress[t];
        (pace > Fx::ZERO && left > Fx::ZERO).then(|| (left / pace).to_f32())
    }

    /// The time left on every site and upgrade of `viewer`'s side and its allies'
    /// (everyone's with no viewer) whose work is moving: an enemy is not told when.
    pub(super) fn write_work_left(&self, viewer: Option<u8>, out: &mut Vec<WorkLeft>) {
        out.clear();
        let units = &self.state.units;
        for row in units.slots.iter() {
            // A hidden successor is shown on the unit it replaces, which lists it.
            if units.has_flag(row, flag::UPGRADE)
                || viewer.is_some_and(|v| self.are_enemies(v, units.owner[row]))
            {
                continue;
            }
            let work = if units.has_flag(row, flag::UNDER_CONSTRUCTION) {
                Some(row)
            } else {
                self.upgrade_work(row)
            };
            if let Some(seconds) = work.and_then(|t| self.work_left(t)) {
                out.push(WorkLeft {
                    unit_id: units.id(row).0,
                    seconds,
                });
            }
        }
    }
}
