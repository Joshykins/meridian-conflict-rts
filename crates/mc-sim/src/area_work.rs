//! Area assist: an engineer on guard works its whole ring, not only what is in reach of
//! its spot. A few times a second it looks for work inside the area, nearest first by
//! kind: a structure going up (or a unit upgrading or refitting, or a factory's product) to help
//! raise, then a friend to mend,
//! then, only while there is room to store the mass, a wreck to reclaim. It goes and does
//! it with an order put in front of the guard, which carries on when that is done, so
//! work that turns up later is picked up too. Reclaim is the least of it: a wreck in hand
//! is dropped as soon as there is raising or mending to do, or the store is full.

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
        if self.bp(row).builder.is_none()
            || !self.area_looks(row)
            || self.state.units.has_flag(row, flag::PASSIVE)
            || self.work_paused(row)
        {
            return Ok(false);
        }
        if let Some(help) = self.area_help(row, o) {
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
        match self.area_help(row, &guard) {
            Some(help) => {
                self.finish_order(row);
                self.area_take(row, help)
            }
            None => Ok(false),
        }
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

    /// Whether a builder of `owner` helps raise `t` from its area: a friendly site going
    /// up, or a friendly unit putting its next tier on (a structure's upgrade, or a
    /// commander's refit, or a factory's product).
    fn area_raise(&self, owner: u8, t: usize) -> bool {
        let units = &self.state.units;
        if self.are_enemies(owner, units.owner[t]) || units.has_flag(t, flag::IN_FACTORY) {
            return false;
        }
        units.has_flag(t, flag::UNDER_CONSTRUCTION)
            || units
                .row(units.build_target[t])
                .is_some_and(|u| units.has_flag(u, flag::UPGRADE))
            || self.factory_product(t).is_some()
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
        self.state.units.stuck_ticks[row] = 0;
        Ok(true)
    }
}
