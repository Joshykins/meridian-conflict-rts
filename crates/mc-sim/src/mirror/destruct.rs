//! What the presentation sees of timed self-destructs (`crate::destruct`).

use crate::World;

/// A unit counting down to its self-destruct, for the interface to show round it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestructView {
    pub unit_id: u32,
    /// Ticks left, and the whole countdown (`destruct::COUNTDOWN`).
    pub ticks_left: u16,
    pub length: u16,
}

impl World {
    /// Every countdown running on `viewer`'s side and its allies' (everyone's with no
    /// viewer): an enemy is not told which of its units are about to go.
    pub(super) fn write_destructs(&self, viewer: Option<u8>, out: &mut Vec<DestructView>) {
        out.clear();
        let units = &self.state.units;
        for row in units.slots.iter() {
            let left = units.destruct[row];
            if left == 0 || viewer.is_some_and(|v| self.are_enemies(v, units.owner[row])) {
                continue;
            }
            out.push(DestructView {
                unit_id: units.id(row).0,
                ticks_left: left,
                length: crate::destruct::COUNTDOWN,
            });
        }
    }
}
