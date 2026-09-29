//! Area assist: an engineer on guard works its whole ring, not only what is in reach of
//! its spot. A few times a second it looks for work inside the area, nearest first by
//! kind: a structure going up (or one upgrading) to help raise, then a friend to mend,
//! then a wreck to reclaim. It goes and does it with an order put in front of the guard,
//! which carries on when that is done, so work that turns up later is picked up too.

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
        let units = &self.state.units;
        if self.bp(row).builder.is_none()
            || !(self.state.tick as usize + row).is_multiple_of(LOOK_TICKS)
            || units.has_flag(row, flag::PASSIVE)
            || self.work_paused(row)
        {
            return Ok(false);
        }
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
        if let Some(e) = raising.or_else(mending) {
            let t = e.row as usize;
            let mut help = order(OrderKind::Assist, e.pos, self.state.units.id(t));
            // A site: done once it stands. Anything else: done once there is no work.
            if self.state.units.has_flag(t, flag::UNDER_CONSTRUCTION) {
                help.radius = Fx::ONE;
            }
            return self.area_take(row, help);
        }
        if self.no_room_for_salvage(owner) {
            return Ok(false);
        }
        let wrecks = &self.state.wrecks;
        let wreck = self.index.nearest(pos, span, kind::WRECK, |e| {
            let w = e.row as usize;
            wrecks.slots.is_alive(w)
                && wrecks.pos[w] == e.pos
                && wrecks.mass[w] > Fx::ZERO
                && inside(e)
        });
        match wreck {
            Some(e) => {
                let handle = self.state.wrecks.slots.handle(e.row as usize);
                self.area_take(row, order(OrderKind::Reclaim, e.pos, handle))
            }
            None => Ok(false),
        }
    }

    /// Whether a builder of `owner` helps raise `t` from its area: a friendly site going
    /// up, or a friendly structure putting its next tier on.
    fn area_raise(&self, owner: u8, t: usize) -> bool {
        let units = &self.state.units;
        if self.are_enemies(owner, units.owner[t]) || units.has_flag(t, flag::IN_FACTORY) {
            return false;
        }
        units.has_flag(t, flag::UNDER_CONSTRUCTION)
            || (!self.bp(t).is_mobile()
                && units
                    .row(units.build_target[t])
                    .is_some_and(|u| units.has_flag(u, flag::UPGRADE)))
    }

    fn area_take(&mut self, row: usize, work: Order) -> Result<bool, SimError> {
        self.state
            .orders
            .push_front(&mut self.state.units, row, work)?;
        self.state.units.stuck_ticks[row] = 0;
        Ok(true)
    }
}
