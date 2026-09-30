//! A factory's batch (`Command::SetBatch`): with it on, what the factory rolls out forms
//! up in rows just outside it and waits there for the rest of the queue — one lap of a
//! repeating queue, or all of a plain one. Then the whole batch is given the factory's
//! standing orders together, as one group, or sent to its rally point; with neither it
//! stays formed up where it is. Turning batch off, or `Command::ReleaseBatch`, sends
//! whoever is waiting at once.
//!
//! A unit the player orders away from its place leaves the batch.

use crate::tables::{flag, OrderKind, UnitId};
use crate::{Command, SimError, World};
use mc_core::{Angle, Fx, FxVec2};
use serde::{Deserialize, Serialize};

/// Most units one batch holds. A batch that reaches it is sent off at once, as if its lap
/// were done: a queue far past this (up to `MAX_FACTORY_QUEUE`) would otherwise leave
/// hundreds of units parked by the factory with nothing to show why.
pub(crate) const MAX_BATCH: usize = 200;
/// Places in each row of the muster block.
const MUSTER_COLUMNS: i32 = 5;
/// Clear ground between the factory's lot and the muster block, metres: the lane the
/// next product rolls out along.
const MUSTER_GAP: i32 = 24;

/// One factory's batch while it is on.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Batch {
    /// The units formed up or on their way to it, each with its place.
    pub held: Vec<(UnitId, FxVec2)>,
    /// Products rolled out since the last batch left, those lost since counted too:
    /// a repeating queue's lap is done when this reaches the queue's length.
    pub made: u16,
}

/// Where a factory's batch musters, in the sim's own numbers (`mirror::BatchView` shows it).
pub(crate) struct Muster {
    /// Products this batch has had so far, and how many it will have.
    pub made: u16,
    pub size: u16,
    /// Each place in the block, and whether a unit stands in it.
    pub places: Vec<(FxVec2, bool)>,
    /// Distance between places, metres.
    pub spacing: Fx,
}

impl World {
    /// `Command::SetBatch`. Turning it off sends whoever is waiting.
    pub(crate) fn set_batch(
        &mut self,
        player: u8,
        factories: &[UnitId],
        on: bool,
    ) -> Result<(), SimError> {
        for row in self.owned_factories(player, factories) {
            let id = self.state.units.id(row);
            if on {
                self.state.batches.entry(id).or_default();
            } else {
                self.release_batch(row)?;
                self.state.batches.remove(&id);
            }
        }
        Ok(())
    }

    /// `Command::ReleaseBatch`: each factory's batch leaves as it stands.
    pub(crate) fn send_batches(
        &mut self,
        player: u8,
        factories: &[UnitId],
    ) -> Result<(), SimError> {
        for row in self.owned_factories(player, factories) {
            self.release_batch(row)?;
        }
        Ok(())
    }

    /// A product out of `factory`'s bay joins its batch, if it has one on: it is sent to
    /// its place in the block and waits. False when there is no batch to join.
    pub(crate) fn join_batch(&mut self, factory: usize, product: usize) -> Result<bool, SimError> {
        let id = self.state.units.id(factory);
        let Some(batch) = self.state.batches.get(&id) else {
            return Ok(false);
        };
        // Numbered by products out, not units waiting, so a lost one's place stays empty.
        let place = usize::from(batch.made);
        let size = self.batch_size(factory, batch.made + 1);
        let spacing = self.muster_spacing(factory, Some(product));
        let mut at = self.muster_place(factory, place, size, spacing);
        if let Some(m) = self.bp(product).motion {
            at = self
                .nav
                .nearest_passable(m.layer, m.size_class, at)
                .filter(|&p| self.terrain.in_bounds(p))
                .unwrap_or(at);
        }
        let at = self.clamp_to_map(at);
        let units = &mut self.state.units;
        units.fire_state[product] = units.fire_state[factory];
        let mut o = crate::orders::order(OrderKind::Move, at, crate::Handle::NONE);
        o.heading = units.heading[factory];
        self.state
            .orders
            .push_back(&mut self.state.units, product, o)?;
        let product_id = self.state.units.id(product);
        if let Some(batch) = self.state.batches.get_mut(&id) {
            batch.held.push((product_id, at));
            batch.made = batch.made.saturating_add(1);
        }
        if self.batch_done(factory) {
            self.release_batch(factory)?;
        }
        Ok(true)
    }

    /// Every tick: batches of factories that are gone are dropped (their units stay
    /// formed up), units lost or ordered away leave theirs, and a batch whose queue
    /// has run out while it waited (cancelled) is sent.
    pub(crate) fn run_batches(&mut self) -> Result<(), SimError> {
        if self.state.batches.is_empty() {
            return Ok(());
        }
        let ids: Vec<UnitId> = self.state.batches.keys().copied().collect();
        for id in ids {
            let Some(row) = self.state.units.row(id) else {
                self.state.batches.remove(&id);
                continue;
            };
            let Some(batch) = self.state.batches.get_mut(&id) else {
                continue;
            };
            let mut held = std::mem::take(&mut batch.held);
            held.retain(|&(u, at)| self.still_held(row, u, at));
            if let Some(batch) = self.state.batches.get_mut(&id) {
                batch.held = held;
            }
            if self.batch_done(row) {
                self.release_batch(row)?;
            }
        }
        Ok(())
    }

    /// Whether `unit` is still in `factory`'s batch: alive, its side's, and either
    /// waiting with no orders or still on its way to `at`.
    fn still_held(&self, factory: usize, unit: UnitId, at: FxVec2) -> bool {
        let units = &self.state.units;
        let Some(row) = units.row(unit) else {
            return false;
        };
        if units.owner[row] != units.owner[factory] {
            return false;
        }
        let mut orders = self.state.orders.iter(units, row);
        match (orders.next(), orders.next()) {
            (None, _) => true,
            (Some(o), None) => o.kind == OrderKind::Move && o.pos == at,
            _ => false,
        }
    }

    /// Whether `factory`'s batch has all it is waiting for (or as many as it may hold).
    fn batch_done(&self, factory: usize) -> bool {
        let id = self.state.units.id(factory);
        let Some(batch) = self.state.batches.get(&id) else {
            return false;
        };
        if batch.held.is_empty() {
            return false;
        }
        batch.held.len() >= MAX_BATCH || batch.made >= self.batch_size(factory, batch.made)
    }

    /// How many products this lap of `factory`'s queue makes, with `made` already out:
    /// a repeating queue's length, or, for a plain one, those out and those still to come.
    fn batch_size(&self, factory: usize, made: u16) -> u16 {
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

    /// Sends `factory`'s waiting units off together, and starts its next batch.
    fn release_batch(&mut self, factory: usize) -> Result<(), SimError> {
        let id = self.state.units.id(factory);
        let Some(batch) = self.state.batches.get_mut(&id) else {
            return Ok(());
        };
        let held = std::mem::take(&mut batch.held);
        batch.made = 0;
        let ids: Vec<UnitId> = held
            .iter()
            .filter(|&&(u, at)| self.still_held(factory, u, at))
            .map(|&(u, _)| u)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let units = &self.state.units;
        let (owner, rally, pos) = (
            units.owner[factory],
            units.rally[factory],
            units.pos[factory],
        );
        if !self.give_standing(factory, &ids, true)? && rally != pos {
            let target = rally;
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

    /// How far apart the muster block's places are: room for the widest hull `factory`
    /// has queued, or `product`.
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
        let size = i32::from(size.max(1)).min(MAX_BATCH as i32);
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

    /// Where `factory`'s batch musters and who stands there, while it has one on.
    pub(crate) fn muster(&self, factory: usize) -> Option<Muster> {
        let batch = self.state.batches.get(&self.state.units.id(factory))?;
        let size = self.batch_size(factory, batch.made).max(batch.made);
        let spacing = self.muster_spacing(factory, None);
        let units = &self.state.units;
        let held = batch.held.iter().map(|&(u, at)| {
            let here = units
                .row(u)
                .is_some_and(|r| units.pos[r].distance(at) < self.bp(r).radius + Fx::from_int(6));
            (at, here)
        });
        // The places still to fill; a lost unit's place is not shown.
        let to_come = (usize::from(batch.made)..usize::from(size).min(MAX_BATCH))
            .map(|k| (self.muster_place(factory, k, size, spacing), false));
        let places = held.chain(to_come).collect();
        Some(Muster {
            made: batch.made,
            size,
            places,
            spacing,
        })
    }
}
