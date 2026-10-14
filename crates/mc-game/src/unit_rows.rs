//! Quick ways from a unit id to its instance in the frame and to its orders in the
//! status, and the selection resolved against the frame once per pull.
//!
//! The interface asks "where is unit N?" of every selected unit, often several times a
//! frame (the deck, the order lines, the marks on the ground). With tens of thousands
//! selected, a hash map lookup or a binary search each was tens of milliseconds a frame.
//! A unit id is a sim handle, whose low bits are its table row (`Handle::index`), so a
//! plain array by row, filled once per pull, finds it in one step; each hit is checked
//! against the id, and a miss falls back to `View::index_of` (or the binary search of the
//! queues, or of the fuel list), so the answers are always those the slower ways give.

use crate::game::View;
use mc_sim::mirror::{FuelView, UnitInstance, UnitOrders, KIND_WRECK};
use mc_sim::Handle;
use std::cell::{Ref, RefCell};

/// No unit has this row.
const NONE: u32 = u32::MAX;

/// The tables, kept in `View::rows` and rebuilt each time a frame is pulled.
#[derive(Default)]
pub struct Rows {
    /// By handle row: the index in `frame.units` of the last unit (not wreck) with that
    /// row, as `View::index_of` keeps the last of an id.
    units: Vec<u32>,
    /// By handle row: the index in `status.queues` of that unit's orders.
    queues: Vec<u32>,
    /// By handle row: the index in `frame.fuel` of that unit's tank.
    fuel: Vec<u32>,
    /// Counts the rebuilds: what `Picked` was resolved against.
    generation: u64,
    picked: RefCell<Picked>,
}

impl Rows {
    /// Fills the tables for a newly pulled frame and status.
    pub fn rebuild(&mut self, units: &[UnitInstance], queues: &[UnitOrders], fuel: &[FuelView]) {
        self.generation += 1;
        let fill = |table: &mut Vec<u32>, ids: &mut dyn Iterator<Item = (usize, u32)>| {
            table.fill(NONE);
            for (i, id) in ids {
                let row = Handle(id).index();
                if row >= table.len() {
                    table.resize(row + 1, NONE);
                }
                table[row] = i as u32;
            }
        };
        fill(
            &mut self.units,
            &mut units
                .iter()
                .enumerate()
                .filter(|(_, u)| u.owner_flags & KIND_WRECK == 0)
                .map(|(i, u)| (i, u.unit_id)),
        );
        fill(
            &mut self.queues,
            &mut queues.iter().enumerate().map(|(i, q)| (i, q.unit_id)),
        );
        fill(
            &mut self.fuel,
            &mut fuel.iter().enumerate().map(|(i, f)| (i, f.unit_id)),
        );
    }
}

/// The selection as found in the frame: built on first use after a pull or a change of
/// selection, then shared by everything drawn until the next.
#[derive(Default)]
pub struct Picked {
    /// What it was built for: `Rows::generation`, the frame's tick and size and
    /// `index_of`'s size (tests fill the frame by hand), and the selection.
    key: Option<(u64, u32, usize, usize)>,
    /// Counts the rebuilds: what a panel's figures kept for this selection go by.
    stamp: u64,
    ids: Vec<u32>,
    /// Index in `frame.units` of each selected unit found there, in selection order.
    rows: Vec<u32>,
    /// One bit per index in `frame.units`: set for those in `rows`.
    bits: Vec<u64>,
    /// The blueprints among them, each once, ascending.
    kinds: Vec<u32>,
}

impl Picked {
    /// The selected units' indices in `frame.units`, in selection order: the units
    /// `selection.iter().filter_map(|id| index_of.get(id))` finds.
    pub fn rows(&self) -> &[u32] {
        &self.rows
    }

    /// `rows` in the order of `frame.units`: as a pass over the frame meets the
    /// selection, without looking at the units that are not in it.
    pub fn in_frame_order(&self) -> impl Iterator<Item = usize> + '_ {
        self.bits.iter().enumerate().flat_map(|(at, &word)| {
            let mut left = word;
            std::iter::from_fn(move || {
                (left != 0).then(|| {
                    let bit = left.trailing_zeros() as usize;
                    left &= left - 1;
                    at * 64 + bit
                })
            })
        })
    }

    /// The blueprints of the selected units, each once: a selection of 60 000 fighters
    /// is one. A question about blueprints alone asks these, not every unit.
    pub fn kinds(&self) -> &[u32] {
        &self.kinds
    }

    /// Changes each time the selection or the frame it was found in does.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    fn rebuild(&mut self, view: &View, key: (u64, u32, usize, usize)) {
        self.key = Some(key);
        self.stamp += 1;
        self.ids.clear();
        self.ids.extend_from_slice(&view.selection);
        self.rows.clear();
        self.bits.clear();
        self.bits.resize(view.frame.units.len().div_ceil(64), 0);
        self.kinds.clear();
        let mut last = None;
        for &id in &view.selection {
            let Some(i) = view.unit_index(id) else {
                continue;
            };
            self.rows.push(i as u32);
            if let Some(w) = self.bits.get_mut(i / 64) {
                *w |= 1 << (i % 64);
            }
            let bp = view.frame.units[i].blueprint;
            if last != Some(bp) {
                last = Some(bp);
                self.kinds.push(bp);
            }
        }
        self.kinds.sort_unstable();
        self.kinds.dedup();
    }
}

impl View {
    /// Unit `id`'s index in `frame.units`: what `index_of` holds, found in one step.
    pub fn unit_index(&self, id: u32) -> Option<usize> {
        let found = self
            .rows
            .units
            .get(Handle(id).index())
            .map(|&i| i as usize)
            .filter(|&i| {
                self.frame
                    .units
                    .get(i)
                    .is_some_and(|u| u.unit_id == id && u.owner_flags & KIND_WRECK == 0)
            });
        found.or_else(|| self.index_of.get(&id).copied())
    }

    /// Unit `id`'s instance in the frame (`unit_index`).
    pub fn unit(&self, id: u32) -> Option<&UnitInstance> {
        self.unit_index(id).map(|i| &self.frame.units[i])
    }

    /// Unit `id`'s orders, when the status has them (`sim_thread::queue_of`).
    pub fn queue(&self, id: u32) -> Option<&UnitOrders> {
        let found = self
            .rows
            .queues
            .get(Handle(id).index())
            .and_then(|&i| self.status.queues.get(i as usize))
            .filter(|q| q.unit_id == id);
        found.or_else(|| crate::sim_thread::queue_of(&self.status.queues, id))
    }

    /// Unit `id`'s tank, when it has one (`FuelView::of`).
    pub fn fuel(&self, id: u32) -> Option<&FuelView> {
        let found = self
            .rows
            .fuel
            .get(Handle(id).index())
            .and_then(|&i| self.frame.fuel.get(i as usize))
            .filter(|f| f.unit_id == id);
        found.or_else(|| FuelView::of(&self.frame.fuel, id))
    }

    /// Changes with each pull: what figures kept until the next one go by.
    pub fn pulls(&self) -> u64 {
        self.rows.generation
    }

    /// The selection resolved against the frame (`Picked`), worked out once per pull and
    /// selection however many ask.
    pub fn picked(&self) -> Ref<'_, Picked> {
        let key = (
            self.rows.generation,
            self.frame.tick,
            self.frame.units.len(),
            self.index_of.len(),
        );
        {
            let picked = self.rows.picked.borrow();
            if picked.key == Some(key) && picked.ids[..] == self.selection[..] {
                return picked;
            }
        }
        // Nothing else holds it: the key only changes with the view, which a holder
        // would have borrowed.
        self.rows.picked.borrow_mut().rebuild(self, key);
        self.rows.picked.borrow()
    }

    /// The selected units found in the frame, in selection order.
    pub fn picked_units(&self) -> Vec<&UnitInstance> {
        let picked = self.picked();
        picked
            .rows()
            .iter()
            .map(|&i| &self.frame.units[i as usize])
            .collect()
    }

    /// Whether any selected unit's blueprint passes `test`: asked of each blueprint once.
    pub fn picks_any(&self, test: impl Fn(mc_data::BlueprintId) -> bool) -> bool {
        self.picked()
            .kinds()
            .iter()
            .any(|&bp| test(mc_data::BlueprintId(bp as u16)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: u32, blueprint: u32) -> UnitInstance {
        UnitInstance {
            unit_id: id,
            blueprint,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn finds_what_index_of_finds() {
        let mut view = View::new(0, [[1.0; 3]; mc_core::MAX_PLAYERS], false);
        // Row 5 twice (generations 0 and 1): the table keeps the later; a wreck is skipped.
        let gen1 = Handle::new(5, 1).0;
        view.frame.units = vec![unit(5, 1), unit(9, 2), unit(gen1, 2), unit(7, 3)];
        view.frame.units[3].owner_flags = KIND_WRECK;
        view.index_of = [(5, 0), (9, 1), (gen1, 2)].into_iter().collect();
        view.status.queues = vec![
            UnitOrders {
                unit_id: 9,
                ..Default::default()
            },
            UnitOrders {
                unit_id: gen1,
                ..Default::default()
            },
        ];
        view.frame.fuel = vec![FuelView {
            unit_id: 9,
            fill: 0.5,
            dry_in: None,
        }];
        view.rows
            .rebuild(&view.frame.units, &view.status.queues, &view.frame.fuel);
        assert_eq!(view.fuel(9).map(|f| f.fill), Some(0.5));
        assert!(view.fuel(gen1).is_none());
        assert_eq!(view.unit_index(9), Some(1));
        assert_eq!(view.unit_index(gen1), Some(2));
        // Its row now names another unit: found through `index_of`.
        assert_eq!(view.unit_index(5), Some(0));
        assert_eq!(view.unit_index(7), None);
        assert_eq!(view.queue(gen1).map(|q| q.unit_id), Some(gen1));
        assert!(view.queue(5).is_none());

        view.selection = vec![gen1, 7, 9, 5].into();
        let picked = view.picked();
        assert_eq!(picked.rows(), &[2, 1, 0]);
        assert_eq!(picked.kinds(), &[1, 2]);
        assert_eq!(picked.in_frame_order().collect::<Vec<_>>(), [0, 1, 2]);
        drop(picked);
        view.selection = vec![9].into();
        assert_eq!(view.picked().rows(), &[1]);
        assert!(view.picks_any(|bp| bp.0 == 2) && !view.picks_any(|bp| bp.0 == 1));
    }
}
