//! Factory batches (`Command::SetBatch`). A batch is one or more factories linked
//! together: what each rolls out forms up in rows just outside its own door and waits.
//! The batch leaves when it is full, all at once: by default once every factory's queue
//! is out (one lap of a repeating queue, all of a plain one), or, with a size set
//! (`Command::SetBatchSize`), once that many are formed up. Units whose factories share
//! the same standing orders leave on them as one group; the rest go to their factory's
//! rally point, or stay formed up. Turning batch off for a factory, or
//! `Command::ReleaseBatch`, sends whoever is waiting at once.
//!
//! The units waiting at a factory are one group to the player: a unit ordered away from
//! its place leaves the batch, and those left close ranks, so taking some away leaves two
//! tidy groups, the batch and the ones taken.

use crate::tables::{flag, OrderKind, UnitId};
use crate::{Command, SimError, World};
use mc_core::{Angle, Fx, FxVec2};
use serde::{Deserialize, Serialize};

/// Most units one batch holds, and the largest size it may be set to. A batch that
/// reaches it is sent off at once: a queue far past this (up to `MAX_FACTORY_QUEUE`)
/// would otherwise leave hundreds of units parked with nothing to show why.
pub const MAX_BATCH: u16 = 200;
/// Places in each row of a muster block.
const MUSTER_COLUMNS: i32 = 5;
/// Clear ground between the factory's lot and the muster block, metres: the lane the
/// next product rolls out along.
const MUSTER_GAP: i32 = 24;

/// One batch: the factories linked in it and the units waiting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Batch {
    pub owner: u8,
    /// The factories in it, first linked first, each with the products it has rolled out
    /// since the batch last left (those lost since counted too).
    pub factories: Vec<(UnitId, u16)>,
    /// The units formed up or on their way to it.
    pub held: Vec<Held>,
    /// Leave once this many are formed up; `None`: once every factory's queue is out.
    pub size: Option<u16>,
}

/// A unit waiting in a batch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Held {
    pub unit: UnitId,
    /// Its place in its factory's block.
    pub at: FxVec2,
    pub factory: UnitId,
}

/// Where one factory of a batch musters, in the sim's own numbers (`mirror::BatchView`
/// shows it).
pub(crate) struct Muster {
    pub group: u32,
    /// The batch's count so far and what it leaves at: products out and the laps' total,
    /// or, with a size set, units waiting and the size.
    pub count: u16,
    pub size: u16,
    pub fixed: bool,
    /// The units waiting at this factory, in place order.
    pub units: Vec<UnitId>,
    /// Where the next one out will stand, while more are to come.
    pub next: Option<FxVec2>,
    /// Where each linked factory's block starts, in the batch's order, and which is this one.
    pub linked: Vec<FxVec2>,
    pub index: usize,
}

impl World {
    /// The batch `factory` is in.
    pub(crate) fn batch_of(&self, factory: UnitId) -> Option<u32> {
        self.state
            .batches
            .iter()
            .find(|(_, b)| b.factories.iter().any(|&(f, _)| f == factory))
            .map(|(&g, _)| g)
    }

    /// `Command::SetBatch`. On: the factories become one batch, bringing their waiting
    /// units along (a set size comes too). Off: each leaves its batch, and its waiting
    /// units leave at once.
    pub(crate) fn set_batch(
        &mut self,
        player: u8,
        factories: &[UnitId],
        on: bool,
    ) -> Result<(), SimError> {
        let ids: Vec<UnitId> = self
            .owned_factories(player, factories)
            .into_iter()
            .map(|r| self.state.units.id(r))
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        if !on {
            for &f in &ids {
                if let Some(g) = self.batch_of(f) {
                    self.release(g, Some(f))?;
                    if let Some(b) = self.state.batches.get_mut(&g) {
                        b.factories.retain(|&(m, _)| m != f);
                    }
                }
            }
            self.state.batches.retain(|_, b| !b.factories.is_empty());
            return Ok(());
        }
        // Already one batch of exactly these: nothing to do.
        if let Some(g) = self.batch_of(ids[0]) {
            let b = &self.state.batches[&g];
            if b.factories.len() == ids.len() && ids.iter().all(|&f| self.batch_of(f) == Some(g)) {
                return Ok(());
            }
        }
        let mut batch = Batch {
            owner: player,
            ..Batch::default()
        };
        for &f in &ids {
            let mut made = 0;
            if let Some(g) = self.batch_of(f) {
                if let Some(old) = self.state.batches.get_mut(&g) {
                    made = old
                        .factories
                        .iter()
                        .find(|&&(m, _)| m == f)
                        .map_or(0, |m| m.1);
                    old.factories.retain(|&(m, _)| m != f);
                    batch
                        .held
                        .extend(old.held.iter().filter(|h| h.factory == f));
                    old.held.retain(|h| h.factory != f);
                    batch.size = batch.size.max(old.size);
                }
            }
            batch.factories.push((f, made));
        }
        self.state.batches.retain(|_, b| !b.factories.is_empty());
        let g = self.state.next_batch;
        self.state.next_batch = g.wrapping_add(1);
        self.state.batches.insert(g, batch);
        Ok(())
    }

    /// `Command::SetBatchSize`: the batches these factories are in leave at `size`
    /// (up to `MAX_BATCH`), or, `None`, once every factory's queue is out.
    pub(crate) fn set_batch_size(&mut self, player: u8, factories: &[UnitId], size: Option<u16>) {
        let size = size.map(|n| n.clamp(1, MAX_BATCH));
        for row in self.owned_factories(player, factories) {
            let id = self.state.units.id(row);
            if let Some(b) = self
                .batch_of(id)
                .and_then(|g| self.state.batches.get_mut(&g))
            {
                b.size = size;
            }
        }
    }

    /// `Command::ReleaseBatch`: each batch these factories are in leaves as it stands.
    pub(crate) fn send_batches(
        &mut self,
        player: u8,
        factories: &[UnitId],
    ) -> Result<(), SimError> {
        for row in self.owned_factories(player, factories) {
            if let Some(g) = self.batch_of(self.state.units.id(row)) {
                self.release(g, None)?;
            }
        }
        Ok(())
    }

    /// A product out of `factory`'s bay joins its batch, if it has one: it is sent to its
    /// place in the factory's block and waits. False when there is no batch to join.
    pub(crate) fn join_batch(&mut self, factory: usize, product: usize) -> Result<bool, SimError> {
        let id = self.state.units.id(factory);
        let Some(g) = self.batch_of(id) else {
            return Ok(false);
        };
        let b = &self.state.batches[&g];
        let made = b.factories.iter().find(|m| m.0 == id).map_or(0, |m| m.1);
        // The next free place: those waiting stand closed up in the first ones.
        let waiting = b.held.iter().filter(|h| h.factory == id).count();
        let places = self.block_size(factory, b, made + 1);
        let spacing = self.muster_spacing(factory, Some(product));
        let at = self.standing_place(factory, product, waiting, places, spacing);
        let units = &mut self.state.units;
        units.fire_state[product] = units.fire_state[factory];
        let mut o = crate::orders::order(OrderKind::Move, at, crate::Handle::NONE);
        o.heading = units.heading[factory];
        self.state
            .orders
            .push_back(&mut self.state.units, product, o)?;
        let unit = self.state.units.id(product);
        if let Some(b) = self.state.batches.get_mut(&g) {
            b.held.push(Held {
                unit,
                at,
                factory: id,
            });
            for m in b.factories.iter_mut().filter(|m| m.0 == id) {
                m.1 = m.1.saturating_add(1);
            }
        }
        if self.batch_full(g) {
            self.release(g, None)?;
        }
        Ok(true)
    }

    /// Every tick: factories that are gone leave their batches (a batch left with none
    /// is dropped, its units staying formed up), units lost or ordered away leave theirs,
    /// and a batch that is full, or waits on queues that have run out, is sent.
    pub(crate) fn run_batches(&mut self) -> Result<(), SimError> {
        if self.state.batches.is_empty() {
            return Ok(());
        }
        let groups: Vec<u32> = self.state.batches.keys().copied().collect();
        for g in groups {
            let Some(mut b) = self.state.batches.remove(&g) else {
                continue;
            };
            let units = &self.state.units;
            b.factories
                .retain(|&(f, _)| units.row(f).is_some_and(|r| units.owner[r] == b.owner));
            if b.factories.is_empty() {
                continue;
            }
            let before = b.held.clone();
            b.held.retain(|h| self.still_held(b.owner, h));
            let thinned = b.held.len() < before.len();
            self.state.batches.insert(g, b);
            if thinned {
                self.close_ranks(g, &before)?;
            }
            if self.batch_full(g) {
                self.release(g, None)?;
            }
        }
        Ok(())
    }

    /// Batch `g`'s waiting units move up into the first places of their blocks (`before`:
    /// who held which place, in order), so a block some were taken from stays one tidy group.
    fn close_ranks(&mut self, g: u32, before: &[Held]) -> Result<(), SimError> {
        let Some(b) = self.state.batches.get(&g) else {
            return Ok(());
        };
        let mut moves = Vec::new();
        for &(f, _) in &b.factories {
            let mut places = before.iter().filter(|h| h.factory == f).map(|h| h.at);
            let waiting = b.held.iter().enumerate().filter(|(_, h)| h.factory == f);
            for ((i, h), at) in waiting.zip(places.by_ref()) {
                if at != h.at {
                    moves.push((i, at));
                }
            }
        }
        for (i, at) in moves {
            let h = self.state.batches[&g].held[i];
            let Some(row) = self.state.units.row(h.unit) else {
                continue;
            };
            let heading = self
                .state
                .units
                .row(h.factory)
                .map_or(self.state.units.heading[row], |f| {
                    self.state.units.heading[f]
                });
            let mut o = crate::orders::order(OrderKind::Move, at, crate::Handle::NONE);
            o.heading = heading;
            self.give(row, o, false)?;
            if let Some(b) = self.state.batches.get_mut(&g) {
                b.held[i].at = at;
            }
        }
        Ok(())
    }

    /// Place `k` of `factory`'s block for `unit`: on ground it can stand on, on the map.
    fn standing_place(
        &self,
        factory: usize,
        unit: usize,
        k: usize,
        places: u16,
        spacing: Fx,
    ) -> FxVec2 {
        let mut at = self.muster_place(factory, k, places, spacing);
        if let Some(m) = self.bp(unit).motion {
            at = self
                .nav
                .nearest_passable(m.layer, m.size_class, at)
                .filter(|&p| self.terrain.in_bounds(p))
                .unwrap_or(at);
        }
        self.clamp_to_map(at)
    }

    /// Whether `h` is still waiting: alive, its side's, and either standing with no
    /// orders or still on its way to its place.
    fn still_held(&self, owner: u8, h: &Held) -> bool {
        let units = &self.state.units;
        let Some(row) = units.row(h.unit) else {
            return false;
        };
        if units.owner[row] != owner {
            return false;
        }
        let mut orders = self.state.orders.iter(units, row);
        match (orders.next(), orders.next()) {
            (None, _) => true,
            (Some(o), None) => o.kind == OrderKind::Move && o.pos == h.at,
            _ => false,
        }
    }

    /// Whether batch `g` has what it waits for: its size formed up, or with none set every
    /// factory's lap out; or all its factories' queues have run dry, so nothing more comes.
    fn batch_full(&self, g: u32) -> bool {
        let Some(b) = self.state.batches.get(&g) else {
            return false;
        };
        if b.held.is_empty() {
            return false;
        }
        let rows: Vec<(usize, u16)> = b
            .factories
            .iter()
            .filter_map(|&(f, made)| self.state.units.row(f).map(|r| (r, made)))
            .collect();
        let dry = rows.iter().all(|&(r, _)| !self.producing(r));
        let full = match b.size {
            Some(n) => b.held.len() >= usize::from(n),
            None => rows.iter().all(|&(r, made)| made >= self.lap(r, made)),
        };
        full || dry || b.held.len() >= usize::from(MAX_BATCH)
    }

    /// Whether `factory` has anything queued or still leaving its bay.
    fn producing(&self, factory: usize) -> bool {
        let units = &self.state.units;
        let id = units.id(factory);
        self.state
            .orders
            .iter(units, factory)
            .any(|o| o.kind == OrderKind::Produce)
            || self.state.rollouts.values().any(|r| r.factory == id)
    }

    /// How many products one lap of `factory`'s queue makes, with `made` already out:
    /// a repeating queue's length, or, for a plain one, those out and those still to come.
    fn lap(&self, factory: usize, made: u16) -> u16 {
        let units = &self.state.units;
        let queued = self
            .state
            .orders
            .iter(units, factory)
            .filter(|o| o.kind == OrderKind::Produce)
            .count();
        let queued = u16::try_from(queued).unwrap_or(u16::MAX);
        if units.has_flag(factory, flag::REPEAT) {
            // A product rolling out has already gone round to the back of the queue.
            queued
        } else {
            let id = units.id(factory);
            let rolling = self
                .state
                .rollouts
                .values()
                .filter(|r| r.factory == id)
                .count();
            let rolling = u16::try_from(rolling).unwrap_or(u16::MAX);
            made.saturating_add(queued).saturating_add(rolling)
        }
    }

    /// How many places `factory`'s block is laid out for, with `made` out: its lap, or
    /// with a size set, room for all of it.
    fn block_size(&self, factory: usize, b: &Batch, made: u16) -> u16 {
        match b.size {
            Some(n) => n.max(made),
            None => self.lap(factory, made).max(made),
        }
    }

    /// Sends batch `g`'s waiting units off (only `factory`'s, when given), and starts its
    /// next round. Units whose factories share standing orders leave on them as one group;
    /// the rest go to their factory's rally point, or stay where they are.
    fn release(&mut self, g: u32, factory: Option<UnitId>) -> Result<(), SimError> {
        let Some(b) = self.state.batches.get_mut(&g) else {
            return Ok(());
        };
        let owner = b.owner;
        let (going, staying): (Vec<Held>, Vec<Held>) = std::mem::take(&mut b.held)
            .into_iter()
            .partition(|h| factory.is_none_or(|f| h.factory == f));
        b.held = staying;
        for m in b.factories.iter_mut() {
            if factory.is_none_or(|f| m.0 == f) {
                m.1 = 0;
            }
        }
        // One group for each way out: factories with the same standing orders, or without
        // any, the same rally point.
        let mut groups: Vec<(usize, Vec<UnitId>)> = Vec::new();
        for h in going.iter().filter(|h| self.still_held(owner, h)) {
            let Some(f) = self.state.units.row(h.factory) else {
                continue;
            };
            match groups.iter_mut().find(|(o, _)| self.same_way_out(*o, f)) {
                Some((_, ids)) => ids.push(h.unit),
                None => groups.push((f, vec![h.unit])),
            }
        }
        for (f, ids) in groups {
            let units = &self.state.units;
            let (rally, pos) = (units.rally[f], units.pos[f]);
            if self.give_standing(f, &ids, true)? {
                continue;
            }
            // Nowhere to go: they march out as one group, clear of where the next forms up.
            let target = if rally != pos {
                rally
            } else {
                self.clear_of_muster(f, ids.len())
            };
            self.apply_as(
                owner,
                &Command::Move {
                    units: ids,
                    target,
                    queue: false,
                },
            )?;
        }
        Ok(())
    }

    /// Where `n` units a batch sends off with no orders and no rally point stand: ahead of
    /// `factory`'s muster block, with room for them to form up short of it.
    fn clear_of_muster(&self, factory: usize, n: usize) -> FxVec2 {
        let spacing = self.muster_spacing(factory, None);
        let size = self
            .batch_of(self.state.units.id(factory))
            .and_then(|g| self.state.batches.get(&g))
            .map_or(1, |b| self.block_size(factory, b, 0).max(1));
        let far = self.muster_place(factory, 0, size, spacing);
        let n = i32::try_from(n).unwrap_or(i32::MAX);
        let rows = (n + MUSTER_COLUMNS - 1) / MUSTER_COLUMNS;
        let ahead = FxVec2::from_angle(self.state.units.heading[factory]);
        let at = far + ahead * (Fx::from_int(MUSTER_GAP) + spacing * (rows / 2 + 1));
        self.clamp_to_map(at)
    }

    /// Whether factories `a` and `b` send their units the same way: the same standing
    /// orders, or with none, the same rally point (or neither has one).
    fn same_way_out(&self, a: usize, b: usize) -> bool {
        let units = &self.state.units;
        let rally = |f: usize| (units.rally[f] != units.pos[f]).then_some(units.rally[f]);
        units.standing[a] == units.standing[b]
            && (!units.standing[a].is_empty() || rally(a) == rally(b))
    }

    /// How far apart a muster block's places are: room for the widest hull `factory` has
    /// queued, or `product`.
    fn muster_spacing(&self, factory: usize, product: Option<usize>) -> Fx {
        let units = &self.state.units;
        let queued = self
            .state
            .orders
            .iter(units, factory)
            .filter(|o| o.kind == OrderKind::Produce)
            .map(|o| self.blueprints.unit(o.blueprint).radius);
        let widest = queued
            .chain(product.map(|p| self.bp(p).radius))
            .max()
            .unwrap_or(Fx::from_int(4));
        widest * 2 + Fx::from_int(6)
    }

    /// Place `k` of a muster block of `size` in front of `factory`. The block's rows run
    /// across the factory's facing, and the far row fills first, so each product walks
    /// up through open ground to its place.
    fn muster_place(&self, factory: usize, k: usize, size: u16, spacing: Fx) -> FxVec2 {
        let units = &self.state.units;
        let heading = units.heading[factory];
        let (ahead, across) = (
            FxVec2::from_angle(heading),
            FxVec2::from_angle(heading + Angle::QUARTER_TURN),
        );
        let cols = MUSTER_COLUMNS;
        let size = i32::from(size.clamp(1, MAX_BATCH));
        let rows = (size + cols - 1) / cols;
        let k = i32::try_from(k).unwrap_or(0).min(size - 1);
        let (row, col) = (rows - 1 - k / cols, k % cols);
        // The last row may be short: it stays centred.
        let in_row = if k / cols == rows - 1 && size % cols != 0 {
            size % cols
        } else {
            cols
        };
        let half_lot = Fx::from_int(self.bp(factory).footprint.0 as i32 * mc_map::BUILD_CELL_M / 2);
        let front = half_lot + Fx::from_int(MUSTER_GAP) + spacing / 2;
        let side = spacing * (col * 2 - (in_row - 1)) / 2;
        units.pos[factory] + ahead * (front + spacing * row) + across * side
    }

    /// Where `factory` musters for its batch and who stands there, while it is in one.
    pub(crate) fn muster(&self, factory: usize) -> Option<Muster> {
        let id = self.state.units.id(factory);
        let g = self.batch_of(id)?;
        let b = &self.state.batches[&g];
        let units = &self.state.units;
        let made = b.factories.iter().find(|m| m.0 == id).map_or(0, |m| m.1);
        let spacing = self.muster_spacing(factory, None);
        let waiting: Vec<UnitId> = b
            .held
            .iter()
            .filter(|h| h.factory == id)
            .map(|h| h.unit)
            .collect();
        let places = self.block_size(factory, b, made);
        let more = match b.size {
            Some(n) => b.held.len() < usize::from(n),
            None => made < places,
        };
        let next = (more && waiting.len() < usize::from(MAX_BATCH))
            .then(|| self.muster_place(factory, waiting.len(), places.max(1), spacing));
        let (count, size) = match b.size {
            Some(n) => (u16::try_from(b.held.len()).unwrap_or(u16::MAX), n),
            None => b
                .factories
                .iter()
                .filter_map(|&(f, made)| units.row(f).map(|r| (made, self.lap(r, made).max(made))))
                .fold((0u16, 0u16), |(c, s), (m, l)| {
                    (c.saturating_add(m), s.saturating_add(l))
                }),
        };
        let linked = b
            .factories
            .iter()
            .filter_map(|&(f, _)| units.row(f))
            .map(|r| {
                let s = self.muster_spacing(r, None);
                self.muster_place(r, 0, 1, s)
            })
            .collect();
        Some(Muster {
            group: g,
            count,
            size,
            fixed: b.size.is_some(),
            units: waiting,
            next,
            linked,
            index: b.factories.iter().position(|m| m.0 == id).unwrap_or(0),
        })
    }
}
