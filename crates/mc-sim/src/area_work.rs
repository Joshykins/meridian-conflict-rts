//! Area assist: an engineer on guard works its whole ring, not only what is in reach of
//! its spot. A few times a second it looks for work inside the area, nearest first by
//! kind: work to help with (a structure going up, a unit upgrading or refitting, a
//! factory's product, a launcher's next round, a dome missing charge), then a friend to mend,
//! then, only while there is room to store the mass, a wreck to reclaim. It goes and does
//! it with an order put in front of the guard, which carries on when that is done, so
//! work that turns up later is picked up too. Reclaim is the least of it: a wreck in hand
//! is dropped as soon as there is raising or mending to do, or the store is full.
//! Paused units are left alone: nothing paused is raised or mended from the ring (nor a
//! paused factory's product), and work in hand is given up when it is paused.
//!
//! A salvager on guard (a unit whose reclaimer works on the move) works its ring the same
//! way, wrecks only: guarding a friendly unit it follows it and takes apart what falls
//! round it as the fight moves (the Scythe). An aircraft does this between circles.

use crate::assist_work::AssistWork;
use crate::orders::order;
use crate::reclaim::WIDEST_TARGET;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::Fx;

/// Ticks between an engineer's looks round its area for work.
const LOOK_TICKS: usize = 5;

impl World {
    /// A builder on guard with nothing to do at hand takes up the next work in its
    /// area. True when it has: an order now stands in front of the guard.
    pub(crate) fn area_work(&mut self, row: usize, o: &Order) -> Result<bool, SimError> {
        let bp = self.bp(row);
        let builds = bp.builder.is_some();
        if !(builds || self.salvages_on_guard(row))
            || !self.area_looks(row)
            || self.state.units.has_flag(row, flag::PASSIVE)
            || self.work_paused(row)
        {
            return Ok(false);
        }
        if let Some(help) = builds.then(|| self.area_help(row, o)).flatten() {
            return self.area_take(row, help);
        }
        let owner = self.state.units.owner[row];
        if self.no_room_for_salvage(owner) {
            return Ok(false);
        }
        let (pos, centre, radius) = (self.state.units.pos[row], o.pos, o.radius);
        let span = pos.distance(centre) + radius + WIDEST_TARGET;
        let wrecks = &self.state.wrecks;
        let wreck = self.index.nearest(pos, span, kind::WRECK, |e| {
            let w = e.row as usize;
            wrecks.slots.is_alive(w)
                && wrecks.pos[w] == e.pos
                && wrecks.mass[w] > Fx::ZERO
                && e.pos.distance(centre) <= radius + e.radius
                && self.wreck_known(w, owner)
        });
        match wreck {
            Some(e) => {
                let handle = self.state.wrecks.slots.handle(e.row as usize);
                let mut take = order(OrderKind::Reclaim, e.pos, handle);
                take.radius = Fx::ONE;
                self.area_take(row, take)
            }
            None => Ok(false),
        }
    }

    /// An engineer reclaiming a wreck it took up from its area gives the wreck up when
    /// there is no room left for the mass, or when something in the area wants raising
    /// or mending, which it then goes to. True when it gave the wreck up.
    pub(crate) fn area_reclaim_yields(&mut self, row: usize) -> Result<bool, SimError> {
        if !self.area_looks(row) {
            return Ok(false);
        }
        let units = &self.state.units;
        let Some(guard) = self
            .state
            .orders
            .iter(units, row)
            .nth(1)
            .copied()
            .filter(|g| g.kind == OrderKind::Guard)
        else {
            return Ok(false);
        };
        if self.no_room_for_salvage(units.owner[row]) {
            self.finish_order(row);
            return Ok(true);
        }
        let help = self
            .bp(row)
            .builder
            .is_some()
            .then(|| self.area_help(row, &guard))
            .flatten();
        match help {
            Some(help) => {
                self.finish_order(row);
                self.area_take(row, help)
            }
            None => Ok(false),
        }
    }

    /// An engineer helping with `t`, work it took up from its area, gives it up when that
    /// work is paused: an upgrade or refit on hold, or the product of a paused factory.
    /// It goes back on guard and looks round for other work. True when it gave it up.
    pub(crate) fn area_assist_yields(&mut self, row: usize, t: usize) -> bool {
        let units = &self.state.units;
        let on_guard = self
            .state
            .orders
            .iter(units, row)
            .nth(1)
            .is_some_and(|g| g.kind == OrderKind::Guard);
        if !on_guard || !self.raising_paused(t) {
            return false;
        }
        self.state.units.build_target[row] = Handle::NONE;
        self.finish_order(row);
        true
    }

    /// Whether the raising of `t` is on hold: `t` itself is paused, or it is a product
    /// in a factory that is.
    fn raising_paused(&self, t: usize) -> bool {
        self.work_paused(t)
            || (self.state.units.has_flag(t, flag::IN_FACTORY)
                && self.product_factory(t).is_some_and(|f| self.work_paused(f)))
    }

    /// The factory printing the product in `p`: it stands on the factory's lot centre.
    fn product_factory(&self, p: usize) -> Option<usize> {
        let units = &self.state.units;
        let id = units.id(p);
        self.index
            .nearest(units.pos[p], WIDEST_TARGET, kind::UNIT, |e| {
                self.unit_entry_is_current(e) && units.build_target[e.row as usize] == id
            })
            .map(|e| e.row as usize)
    }

    /// Whether `row` works its guard area as a salvager: it has a reclaimer that works on
    /// the move (the Scythe that follows an army, a salvage drone), not a builder's.
    fn salvages_on_guard(&self, row: usize) -> bool {
        self.bp(row).reclaimer.is_some_and(|r| r.mobile)
    }

    /// Whether this is one of the ticks an engineer looks round its area.
    fn area_looks(&self, row: usize) -> bool {
        (self.state.tick as usize + row).is_multiple_of(LOOK_TICKS)
    }

    /// The nearest raising in the area `o` rings, else the nearest mending, as an order.
    fn area_help(&self, row: usize, o: &Order) -> Option<Order> {
        let units = &self.state.units;
        let (pos, owner) = (units.pos[row], units.owner[row]);
        let (centre, radius) = (o.pos, o.radius);
        let span = pos.distance(centre) + radius + WIDEST_TARGET;
        let inside = |e: &crate::spatial::Entry| e.pos.distance(centre) <= radius + e.radius;
        let raising = self.index.nearest(pos, span, kind::UNIT, |e| {
            let t = e.row as usize;
            self.unit_entry_is_current(e) && inside(e) && self.area_raise(owner, t)
        });
        let mending = || {
            self.index.nearest(pos, span, kind::UNIT, |e| {
                self.unit_entry_is_current(e)
                    && inside(e)
                    && !self.work_paused(e.row as usize)
                    && self.can_repair_unit(row, e.row as usize)
            })
        };
        let e = raising.or_else(mending)?;
        // A factory at work: help with its product, done once that rolls out, so the
        // engineer looks round again between products rather than staying on the factory.
        let t = self
            .factory_product(e.row as usize)
            .unwrap_or(e.row as usize);
        let mut help = order(OrderKind::Assist, e.pos, units.id(t));
        // A site: done once it stands. Anything else: done once there is no work.
        if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            help.radius = Fx::ONE;
        }
        Some(help)
    }

    /// Whether a builder of `owner` takes up work on `t` from its area: whatever an
    /// assist would do for it (`assist_work`) bar mending, which is looked for after,
    /// and bar a builder's site, which is helped where it stands if that is in the ring.
    fn area_raise(&self, owner: u8, t: usize) -> bool {
        let units = &self.state.units;
        if self.are_enemies(owner, units.owner[t])
            || units.has_flag(t, flag::IN_FACTORY)
            || self.work_paused(t)
        {
            return false;
        }
        match self.assist_work(t) {
            Some(AssistWork::Raise | AssistWork::Round | AssistWork::Shield) => true,
            Some(AssistWork::Product(p)) => {
                units.has_flag(p, flag::IN_FACTORY) || units.has_flag(p, flag::UPGRADE)
            }
            Some(AssistWork::Mend) | None => false,
        }
    }

    /// The product a factory in `t` is building, while it is not paused. An upgrade's or
    /// a refit's new body is helped through the unit itself, not here.
    fn factory_product(&self, t: usize) -> Option<usize> {
        let units = &self.state.units;
        if units.has_flag(t, flag::UNDER_CONSTRUCTION) || self.work_paused(t) {
            return None;
        }
        units.row(units.build_target[t]).filter(|&p| {
            units.has_flag(p, flag::IN_FACTORY)
                && units.has_flag(p, flag::UNDER_CONSTRUCTION)
                && !units.has_flag(p, flag::UPGRADE)
        })
    }

    fn area_take(&mut self, row: usize, work: Order) -> Result<bool, SimError> {
        self.state
            .orders
            .push_front(&mut self.state.units, row, work)?;
        // As in `take_area_wreck`: a failed walk must not carry over to the new work.
        self.stop_moving(row);
        Ok(true)
    }
}
