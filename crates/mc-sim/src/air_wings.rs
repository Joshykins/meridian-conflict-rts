//! Air wings (`UnitBlueprint::air_wings`): a carrier builds a wing at a time aboard,
//! `count` aircraft paid and built as one job with the carrier's own build power, like a
//! launcher's round (`nukes.rs`). Done, the wing's aircraft are stowed aboard as on a
//! staging base (`staging.rs`), go off the deck one after another, and once the last is
//! up they form up and fly guard round the carrier (`Command::Guard` with the carrier as
//! its target), in formation, until given other orders.
//!
//! The queue is the carrier's own (`Command::QueueWing`, `Command::CancelWing`), apart
//! from its orders, so sailing it about never touches what it is building. A queue
//! holds at most `MAX_WING_QUEUE` wings; more is refused (`Refusal::WingQueueFull`).
//!
//! As a factory's: a carrier on Repeat (`Command::SetRepeat`, `flag::REPEAT`) puts each
//! wing it finishes back at the end of its queue; a paused one (`Command::SetPaused`)
//! builds nothing; and its priority (`Command::SetPriority`) is the tier its wing is paid
//! in during a stall.

use crate::mirror::{Refusal, SimEvent};
use crate::tables::{flag, Handle, UnitId};
use crate::{SimError, World};
use mc_core::{Fx, StateHasher, TICKS_PER_SECOND};
use mc_data::Wing;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Most wings waiting in one carrier's queue.
pub const MAX_WING_QUEUE: usize = 8;

/// Every carrier's wing queue, by the carrier.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct WingBays {
    pub bays: BTreeMap<UnitId, WingBay>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct WingBay {
    /// Wings to build, by their index in the carrier's `air_wings`; the first is under way.
    pub queue: Vec<u8>,
    /// Build time put into the first, out of the whole wing's.
    pub progress: Fx,
    /// The last wing built, aboard or on its way off the deck; formed up once all are out.
    pub launching: Vec<UnitId>,
}

impl WingBays {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.bays.len() as u64);
        for (id, b) in &self.bays {
            h.write_u64(id.0 as u64);
            h.write_u64(b.queue.len() as u64);
            for &w in &b.queue {
                h.write_u64(w as u64);
            }
            h.write_i64(b.progress.0);
            h.write_u64(b.launching.len() as u64);
            for u in &b.launching {
                h.write_u64(u.0 as u64);
            }
        }
    }
}

/// A wing job paid this tick: the carrier, its rate, what it wants, its tier.
pub(crate) type WingJob = (usize, Fx, [Fx; 2], usize);

impl World {
    /// The wing at `index` of the carrier in `row`'s list.
    fn wing(&self, row: usize, index: u8) -> Option<&Wing> {
        self.bp(row)
            .air_wings
            .as_ref()
            .and_then(|a| a.wings.get(index as usize))
    }

    /// Build time of the wing at `index` of the carrier in `row`'s list.
    pub(crate) fn wing_time(&self, row: usize, index: u8) -> Option<Fx> {
        self.wing(row, index).map(|w| self.wing_cost(w)[0])
    }

    /// Build time, mass and energy of a whole wing.
    fn wing_cost(&self, wing: &Wing) -> [Fx; 3] {
        let a = self.blueprints.unit(wing.aircraft);
        let n = wing.count as i32;
        [a.build_time * n, a.cost_mass * n, a.cost_energy * n]
    }

    /// `Command::QueueWing`: `count` more of wing `index` on each carrier among `ids`.
    pub(crate) fn queue_wing(
        &mut self,
        player: u8,
        ids: &[UnitId],
        index: u8,
        count: u8,
    ) -> Result<(), SimError> {
        for row in self.owned_carriers(player, ids) {
            if self.wing(row, index).is_none() {
                continue;
            }
            let id = self.state.units.id(row);
            let bay = self.state.wings.bays.entry(id).or_default();
            for _ in 0..count {
                if bay.queue.len() >= MAX_WING_QUEUE {
                    self.events.push(SimEvent::CommandRefused {
                        player,
                        reason: Refusal::WingQueueFull,
                    });
                    break;
                }
                bay.queue.push(index);
            }
        }
        Ok(())
    }

    /// The carriers (units with `air_wings`) among `ids` that `player` owns.
    pub(crate) fn owned_carriers(&self, player: u8, ids: &[UnitId]) -> Vec<usize> {
        let mut rows = self.owned(player, ids, 0);
        rows.retain(|&r| self.bp(r).air_wings.is_some());
        rows
    }

    /// `Command::CancelWing`: the last queued wing `index` off each carrier among `ids`.
    /// Cancelling the one under way loses what was put into it.
    pub(crate) fn cancel_wing(&mut self, player: u8, ids: &[UnitId], index: u8) {
        for row in self.owned_carriers(player, ids) {
            let id = self.state.units.id(row);
            let Some(bay) = self.state.wings.bays.get_mut(&id) else {
                continue;
            };
            if let Some(at) = bay.queue.iter().rposition(|&w| w == index) {
                bay.queue.remove(at);
                if at == 0 {
                    bay.progress = Fx::ZERO;
                }
            }
        }
    }

    /// The wing each carrier is building this tick, with its power, what it would spend
    /// and the tier it is paid in. Nothing while the last wing is still going off the
    /// deck, or while the carrier is paused.
    pub(crate) fn wing_jobs(&self) -> Vec<WingJob> {
        let mut jobs = Vec::new();
        for (&id, bay) in &self.state.wings.bays {
            let Some(row) = self.state.units.row(id) else {
                continue;
            };
            let units = &self.state.units;
            if !units.is_active(row) || units.paused[row] || !bay.launching.is_empty() {
                continue;
            }
            let (Some(&index), Some(a)) = (bay.queue.first(), self.bp(row).air_wings.as_ref())
            else {
                continue;
            };
            let Some(wing) = self.wing(row, index) else {
                continue;
            };
            let [time, mass, energy] = self.wing_cost(wing);
            let rate = a.power / TICKS_PER_SECOND as i32;
            let left = rate.min(time - bay.progress);
            if left <= Fx::ZERO || time <= Fx::ZERO {
                continue;
            }
            let want = [mass * left / time, energy * left / time];
            jobs.push((row, rate, want, units.priority[row].tier()));
        }
        jobs
    }

    /// The economy's answer to `wing_jobs`: `step` of build time into the wing.
    pub(crate) fn advance_wing(&mut self, row: usize, step: Fx) {
        let id = self.state.units.id(row);
        let Some(index) = self
            .state
            .wings
            .bays
            .get(&id)
            .and_then(|b| b.queue.first().copied())
        else {
            return;
        };
        let Some(time) = self.wing(row, index).map(|w| self.wing_cost(w)[0]) else {
            return;
        };
        let bay = self.state.wings.bays.get_mut(&id).expect("bay");
        // As for launchers: never left one step short of done by a stall.
        bay.progress = (bay.progress + step).min(time);
    }

    /// Every tick, before staging: a wing that is done comes aboard, and one that is all
    /// off the deck forms up round its carrier. A carrier that is gone takes its queue
    /// with it.
    pub(crate) fn run_air_wings(&mut self) -> Result<(), SimError> {
        let ids: Vec<UnitId> = self.state.wings.bays.keys().copied().collect();
        for id in ids {
            let Some(row) = self.state.units.row(id).filter(|&r| {
                self.bp(r).air_wings.is_some() && self.state.units.health[r] > Fx::ZERO
            }) else {
                self.state.wings.bays.remove(&id);
                continue;
            };
            self.form_up(row, id)?;
            self.finish_wing(row, id)?;
        }
        Ok(())
    }

    /// The wing done in full: its aircraft spawned aboard, ready to go off the deck, and
    /// on Repeat the same wing queued again at the back (it has just left the front, so
    /// the queue has room). Held at done while the unit table has no room for all of it.
    fn finish_wing(&mut self, row: usize, id: UnitId) -> Result<(), SimError> {
        let bay = &self.state.wings.bays[&id];
        let Some(&index) = bay.queue.first() else {
            return Ok(());
        };
        let Some(wing) = self.wing(row, index).cloned() else {
            let bay = self.state.wings.bays.get_mut(&id).expect("bay");
            bay.queue.remove(0);
            bay.progress = Fx::ZERO;
            return Ok(());
        };
        if bay.progress < self.wing_cost(&wing)[0] || !bay.launching.is_empty() {
            return Ok(());
        }
        let slots = &self.state.units.slots;
        if slots.capacity() - slots.live() < wing.count as usize {
            return Ok(());
        }
        let owner = self.state.units.owner[row];
        let (pos, heading) = (self.state.units.pos[row], self.state.units.heading[row]);
        let mut launching = Vec::with_capacity(wing.count as usize);
        for _ in 0..wing.count {
            let t = self.spawn_unit(wing.aircraft, owner, pos, heading, true)?;
            self.stow_aboard(t, row);
            self.state.players[owner as usize].units_built += 1;
            let unit = self.state.units.id(t);
            self.events.push(SimEvent::UnitCompleted { unit, owner });
            launching.push(unit);
        }
        let repeat = self.state.units.has_flag(row, flag::REPEAT);
        let bay = self.state.wings.bays.get_mut(&id).expect("bay");
        bay.queue.remove(0);
        if repeat {
            bay.queue.push(index);
        }
        bay.progress = Fx::ZERO;
        bay.launching = launching;
        Ok(())
    }

    /// Once every aircraft of the last wing is off the deck (or lost), they fly guard
    /// round the carrier together, in formation.
    fn form_up(&mut self, row: usize, id: UnitId) -> Result<(), SimError> {
        let units = &self.state.units;
        let bay = &self.state.wings.bays[&id];
        if bay.launching.is_empty()
            || bay.launching.iter().any(|&u| {
                units
                    .row(u)
                    .is_some_and(|r| units.hangar[r] != Handle::NONE)
            })
        {
            return Ok(());
        }
        let wing: Vec<UnitId> = bay
            .launching
            .iter()
            .copied()
            .filter(|&u| {
                units
                    .row(u)
                    .is_some_and(|r| !units.has_flag(r, flag::IN_FACTORY))
            })
            .collect();
        self.state
            .wings
            .bays
            .get_mut(&id)
            .expect("bay")
            .launching
            .clear();
        let Some(radius) = self.bp(row).air_wings.as_ref().map(|a| a.guard) else {
            return Ok(());
        };
        let (owner, pos) = (self.state.units.owner[row], self.state.units.pos[row]);
        self.order_guard(owner, &wing, pos, id, radius, false)
    }
}
