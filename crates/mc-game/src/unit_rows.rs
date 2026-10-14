//! Quick ways from a unit id to its instance in the frame and to its orders in the
//! status, the few units and queues each kind of mark looks for, and the selection
//! resolved against the frame once per pull.
//!
//! The interface asks "where is unit N?" of every selected unit, often several times a
//! frame (the deck, the order lines, the marks on the ground). With tens of thousands
//! selected, a hash map lookup or a binary search each was tens of milliseconds a frame.
//! A unit id is a sim handle, whose low bits are its table row (`Handle::index`), so a
//! table by row, holding each row's id and filled once per pull, answers in one step
//! without reading the frame. A row two ids share is asked of `View::index_of` (or the
//! binary search of the queues, or of the fuel list), and so is everything while the
//! frame is not the one the tables were filled from (a view filled by hand, before any
//! pull), where a row's answer is believed only once the list confirms it: the answers
//! are always those the slower ways give.
//!
//! The same pass lists each blueprint's units, and the units and queues the interface
//! otherwise looked through every frame for the few that matter (`UnitList`,
//! `QueueList`).

use crate::game::View;
use mc_sim::mirror::{FuelView, UnitInstance, UnitOrders, KIND_GHOST, KIND_PROP, KIND_WRECK};
use mc_sim::tables::OrderKind;
use mc_sim::Handle;
use std::cell::{Ref, RefCell};

/// An empty row.
const NONE: u32 = u32::MAX;
/// A row two ids share (a prop's id on a unit's row): asked of the slower way.
const SHARED: u32 = u32::MAX - 1;

/// One list's ids by handle row: each row's id and where it stands in the list (and the
/// unit's blueprint, for the frame's units), filled once per pull.
#[derive(Default)]
struct Table {
    slots: Vec<Slot>,
    /// The length of the list it was filled from: a list of another length (a frame
    /// filled by hand) is not what it describes.
    len: usize,
}

#[derive(Clone, Copy)]
struct Slot {
    id: u32,
    at: u32,
    blueprint: u32,
}

const EMPTY: Slot = Slot {
    id: NONE,
    at: NONE,
    blueprint: 0,
};

/// What a table says of an id.
enum Lookup {
    At(usize, u32),
    /// Not in the list.
    Absent,
    /// The table cannot tell: ask the list.
    Unknown,
}

impl Table {
    fn fill(&mut self, len: usize, entries: impl Iterator<Item = (usize, u32, u32)>) {
        self.slots.fill(EMPTY);
        self.len = len;
        for (at, id, blueprint) in entries {
            let row = Handle(id).index();
            if row >= self.slots.len() {
                self.slots.resize(row + 1, EMPTY);
            }
            let slot = &mut self.slots[row];
            // The later of one id, as `index_of` keeps it; two ids on one row, neither.
            *slot = if slot.at == NONE || slot.id == id {
                Slot {
                    id,
                    at: at as u32,
                    blueprint,
                }
            } else {
                Slot {
                    id: NONE,
                    at: SHARED,
                    blueprint: 0,
                }
            };
        }
    }

    /// Where `id` stands in a list of `len`. `current`: the list is the one the table was
    /// filled from; otherwise a row's entry is believed only when `check` confirms it.
    fn find(&self, id: u32, len: usize, current: bool, check: impl Fn(usize) -> bool) -> Lookup {
        let slot = self.slots.get(Handle(id).index()).copied().unwrap_or(EMPTY);
        if current && len == self.len {
            match slot {
                Slot { at: SHARED, .. } => Lookup::Unknown,
                Slot { at, .. } if at == NONE || slot.id != id => Lookup::Absent,
                Slot { at, blueprint, .. } => Lookup::At(at as usize, blueprint),
            }
        } else if slot.at < SHARED && slot.id == id && check(slot.at as usize) {
            Lookup::At(slot.at as usize, slot.blueprint)
        } else {
            Lookup::Unknown
        }
    }
}

/// Queues the interface looks through every frame for the few that matter: listed once
/// per pull (`View::listed_queues`).
#[derive(Clone, Copy)]
pub enum QueueList {
    /// Factories forming a batch.
    Batches = 0,
    /// Units whose target the ground hides (`line_of_fire.rs`).
    Hidden = 1,
    /// Factories and engineers making something, their bar off their queue.
    Working = 2,
}

impl QueueList {
    fn holds(self, q: &UnitOrders) -> bool {
        match self {
            QueueList::Batches => q.batch.is_some(),
            QueueList::Hidden => q.hidden_target.is_some(),
            QueueList::Working => {
                q.progress > 0.0
                    && q.orders.first().is_some_and(|o| {
                        matches!(
                            o.kind,
                            OrderKind::Build
                                | OrderKind::Produce
                                | OrderKind::Upgrade
                                | OrderKind::Assist
                        )
                    })
            }
        }
    }
}

const QUEUE_LISTS: [QueueList; 3] = [QueueList::Batches, QueueList::Hidden, QueueList::Working];

/// Units the interface looks through every frame for the few that matter: listed once
/// per pull (`View::listed_units`).
#[derive(Clone, Copy)]
pub enum UnitList {
    /// Sites going up, structures upgrading and launchers assembling: work their own bar
    /// shows (`unit_bar_work` without queues).
    Rising = 0,
    /// Stunned now or at the tick before (`UnitInstance::stun`).
    Stunned = 1,
}

impl UnitList {
    fn holds(self, u: &UnitInstance) -> bool {
        match self {
            UnitList::Rising => crate::game::unit_bar_work(u, &[], &[]) >= 0.0,
            UnitList::Stunned => {
                u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) == 0
                    && (u.fx[2] > STUN_SHOWN || u.fx[3] > STUN_SHOWN)
            }
        }
    }
}

const UNIT_LISTS: [UnitList; 2] = [UnitList::Rising, UnitList::Stunned];

/// The least stun that shows (`warp_marks::stuns`): a stun between two ticks is never
/// more than the larger of them.
pub const STUN_SHOWN: f32 = 0.01;

/// The tables, kept in `View::rows` and rebuilt each time a frame is pulled.
#[derive(Default)]
pub struct Rows {
    /// The frame's units (wrecks aside), as `View::index_of` has them.
    units: Table,
    /// The status's orders.
    queues: Table,
    /// The frame's tanks.
    fuel: Table,
    /// The frame's tick when they were filled.
    tick: u32,
    /// Indices in `status.queues`, by `QueueList`.
    queue_lists: [Vec<u32>; QUEUE_LISTS.len()],
    /// Indices in `frame.units`, by `UnitList`.
    unit_lists: [Vec<u32>; UNIT_LISTS.len()],
    /// By blueprint id: the indices in `frame.units` of its units (wrecks aside), in order.
    by_kind: Vec<Vec<u32>>,
    /// The blueprint ids with units in the frame, ascending.
    kinds: Vec<u32>,
    /// Counts the rebuilds: what `Picked` was resolved against. Zero: never filled, as
    /// in a view filled by hand, where every answer is checked against the lists.
    generation: u64,
    picked: RefCell<Picked>,
}

impl Rows {
    /// Fills the tables for a newly pulled frame and status.
    pub fn rebuild(
        &mut self,
        tick: u32,
        units: &[UnitInstance],
        queues: &[UnitOrders],
        fuel: &[FuelView],
    ) {
        self.generation += 1;
        self.tick = tick;
        let live = || (units.iter().enumerate()).filter(|(_, u)| u.owner_flags & KIND_WRECK == 0);
        self.units.fill(
            units.len(),
            live().map(|(i, u)| (i, u.unit_id, u.blueprint)),
        );
        (self.queues).fill(
            queues.len(),
            queues.iter().enumerate().map(|(i, q)| (i, q.unit_id, 0)),
        );
        (self.fuel).fill(
            fuel.len(),
            fuel.iter().enumerate().map(|(i, f)| (i, f.unit_id, 0)),
        );
        for &kind in &self.kinds {
            self.by_kind[kind as usize].clear();
        }
        self.kinds.clear();
        for (i, u) in live() {
            let bp = u.blueprint as usize;
            if bp >= self.by_kind.len() {
                self.by_kind.resize_with(bp + 1, Vec::new);
            }
            if self.by_kind[bp].is_empty() {
                self.kinds.push(bp as u32);
            }
            self.by_kind[bp].push(i as u32);
        }
        self.kinds.sort_unstable();
        for list in QUEUE_LISTS {
            let out = &mut self.queue_lists[list as usize];
            out.clear();
            out.extend(
                (queues.iter().enumerate())
                    .filter(|(_, q)| list.holds(q))
                    .map(|(i, _)| i as u32),
            );
        }
        for list in UNIT_LISTS {
            let out = &mut self.unit_lists[list as usize];
            out.clear();
            out.extend(
                (units.iter().enumerate())
                    .filter(|(_, u)| list.holds(u))
                    .map(|(i, _)| i as u32),
            );
        }
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
    /// Counts the changes of selection: what figures kept for the same units, a tick
    /// old, go by.
    units: u64,
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

    /// Whether the unit at `index` in `frame.units` is one of `rows`: for a unit found by
    /// `View::unit_index`, whether it is selected.
    pub fn has(&self, index: usize) -> bool {
        self.bits
            .get(index / 64)
            .is_some_and(|w| w & 1 << (index % 64) != 0)
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

    /// Changes each time the selection does, not with the frame.
    pub fn units(&self) -> u64 {
        self.units
    }

    fn rebuild(&mut self, view: &View, key: (u64, u32, usize, usize)) {
        self.key = Some(key);
        self.stamp += 1;
        if self.ids[..] != view.selection[..] {
            self.units += 1;
            self.ids.clear();
            self.ids.extend_from_slice(&view.selection);
        }
        self.rows.clear();
        self.bits.clear();
        self.bits.resize(view.frame.units.len().div_ceil(64), 0);
        self.kinds.clear();
        let mut last = None;
        for &id in &view.selection {
            let Some((i, bp)) = view.unit_entry(id) else {
                continue;
            };
            self.rows.push(i as u32);
            if let Some(w) = self.bits.get_mut(i / 64) {
                *w |= 1 << (i % 64);
            }
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
    /// The tables were filled from this frame and status.
    fn rows_current(&self) -> bool {
        self.rows.generation > 0 && self.rows.tick == self.frame.tick
    }

    /// Unit `id`'s index in `frame.units`, and its blueprint: what `index_of` holds,
    /// found in one step.
    fn unit_entry(&self, id: u32) -> Option<(usize, u32)> {
        let check = |i: usize| {
            (self.frame.units.get(i))
                .is_some_and(|u| u.unit_id == id && u.owner_flags & KIND_WRECK == 0)
        };
        let len = self.frame.units.len();
        match self.rows.units.find(id, len, self.rows_current(), check) {
            Lookup::At(i, bp) => Some((i, bp)),
            Lookup::Absent => None,
            Lookup::Unknown => {
                let i = *self.index_of.get(&id)?;
                Some((i, self.frame.units.get(i)?.blueprint))
            }
        }
    }

    /// Unit `id`'s index in `frame.units`: what `index_of` holds, found in one step.
    pub fn unit_index(&self, id: u32) -> Option<usize> {
        self.unit_entry(id).map(|(i, _)| i)
    }

    /// Unit `id`'s instance in the frame (`unit_index`).
    pub fn unit(&self, id: u32) -> Option<&UnitInstance> {
        self.unit_index(id).map(|i| &self.frame.units[i])
    }

    /// Unit `id`'s orders, when the status has them (`sim_thread::queue_of`).
    pub fn queue(&self, id: u32) -> Option<&UnitOrders> {
        let queues = &self.status.queues;
        let check = |i: usize| queues.get(i).is_some_and(|q| q.unit_id == id);
        match (self.rows.queues).find(id, queues.len(), self.rows_current(), check) {
            Lookup::At(i, _) => queues.get(i),
            Lookup::Absent => None,
            Lookup::Unknown => crate::sim_thread::queue_of(queues, id),
        }
    }

    /// Unit `id`'s tank, when it has one (`FuelView::of`).
    pub fn fuel(&self, id: u32) -> Option<&FuelView> {
        let fuel = &self.frame.fuel;
        let check = |i: usize| fuel.get(i).is_some_and(|f| f.unit_id == id);
        match (self.rows.fuel).find(id, fuel.len(), self.rows_current(), check) {
            Lookup::At(i, _) => fuel.get(i),
            Lookup::Absent => None,
            Lookup::Unknown => FuelView::of(fuel, id),
        }
    }

    /// Whether any unit in the frame (wrecks aside) is of a blueprint `test` takes: asked
    /// of each blueprint there once. Before the first pull, always (a view filled by hand).
    pub fn in_frame(&self, test: impl Fn(mc_data::BlueprintId) -> bool) -> bool {
        !self.rows_current()
            || (self.rows.kinds.iter()).any(|&k| test(mc_data::BlueprintId(k as u16)))
    }

    /// The indices in `frame.units` of the units (wrecks aside) of blueprints `test` takes,
    /// in order: a pass over the frame that meets only those.
    pub fn units_of(&self, test: impl Fn(mc_data::BlueprintId) -> bool) -> Vec<usize> {
        if !self.rows_current() {
            return (self.frame.units.iter().enumerate())
                .filter(|(_, u)| {
                    u.owner_flags & KIND_WRECK == 0
                        && test(mc_data::BlueprintId(u.blueprint as u16))
                })
                .map(|(i, _)| i)
                .collect();
        }
        let mut out: Vec<usize> = (self.rows.kinds.iter())
            .filter(|&&k| test(mc_data::BlueprintId(k as u16)))
            .flat_map(|&k| self.rows.by_kind[k as usize].iter().map(|&i| i as usize))
            .collect();
        out.sort_unstable();
        out
    }

    /// The queues `list` holds, in order: a few among tens of thousands.
    pub fn listed_queues(&self, list: QueueList) -> Box<dyn Iterator<Item = &UnitOrders> + '_> {
        let queues = &self.status.queues;
        if !self.rows_current() || queues.len() != self.rows.queues.len {
            // Not the lists' status (a view filled by hand): every queue is looked at.
            return Box::new(queues.iter().filter(move |q| list.holds(q)));
        }
        Box::new((self.rows.queue_lists[list as usize].iter()).map(|&i| &queues[i as usize]))
    }

    /// The indices in `frame.units` of the units `list` holds, in order.
    pub fn listed_units(&self, list: UnitList) -> Box<dyn Iterator<Item = usize> + '_> {
        let units = &self.frame.units;
        if !self.rows_current() || units.len() != self.rows.units.len {
            return Box::new(
                (units.iter().enumerate())
                    .filter(move |(_, u)| list.holds(u))
                    .map(|(i, _)| i),
            );
        }
        Box::new((self.rows.unit_lists[list as usize].iter()).map(|&i| i as usize))
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
        // Unit 9 is going up and stunned; its target is hidden.
        view.frame.units[1].owner_flags = (mc_sim::tables::flag::UNDER_CONSTRUCTION as u32) << 8;
        view.frame.units[1].fx[3] = 0.5;
        view.index_of = [(5, 0), (9, 1), (gen1, 2)].into_iter().collect();
        view.status.queues = vec![
            UnitOrders {
                unit_id: 9,
                hidden_target: Some([0.0; 3]),
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
        view.rows.rebuild(
            view.frame.tick,
            &view.frame.units,
            &view.status.queues,
            &view.frame.fuel,
        );
        assert_eq!(view.fuel(9).map(|f| f.fill), Some(0.5));
        // The wreck's blueprint 3 is not counted.
        assert!(view.in_frame(|bp| bp.0 == 2) && !view.in_frame(|bp| bp.0 == 3));
        let listed = |view: &View| {
            (
                view.listed_units(UnitList::Rising).collect::<Vec<_>>(),
                view.listed_units(UnitList::Stunned).collect::<Vec<_>>(),
                (view.listed_queues(QueueList::Hidden))
                    .map(|q| q.unit_id)
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(listed(&view), (vec![1], vec![1], vec![9]));
        // A view filled by hand since is looked through whole, with the same answers.
        view.frame.tick += 1;
        assert_eq!(listed(&view), (vec![1], vec![1], vec![9]));
        view.frame.tick -= 1;
        assert_eq!(view.units_of(|bp| bp.0 != 1), [1, 2]);
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
        assert!(picked.has(0) && picked.has(2) && !picked.has(3));
        drop(picked);
        view.selection = vec![9].into();
        assert_eq!(view.picked().rows(), &[1]);
        assert!(view.picks_any(|bp| bp.0 == 2) && !view.picks_any(|bp| bp.0 == 1));
    }
}
