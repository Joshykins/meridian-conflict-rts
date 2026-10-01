//! Area reclaim: a reclaimer sent to a point clears the wrecks it passes on the way,
//! and one given a circle clears every wreck inside it.
//!
//! A `ReclaimArea` order's `pos` is the point (or the middle of the circle) and
//! `radius` the circle's size, zero for a point; each unit heads for `pos + offset`,
//! so a group keeps its spread. A few times a second the unit looks for a wreck
//! within a short step of its way, or, once it has come up to the circle, anywhere
//! inside it. It goes and takes it with a
//! `Reclaim` put in front of the area order, which carries on when that is done. A
//! wreck another reclaimer nearby is already on is left to it while there are others.
//!
//! On a point, the order ends once the unit stands there. On a circle, it ends once
//! the unit is inside and nothing is left in it to take.
//!
//! A drone carrier takes nothing itself: it hovers over the wrecks it comes to while
//! its drones clear them, and on a circle goes from wreck to wreck inside it, its
//! drones taking only what is inside the circle.

use crate::orders::order;
use crate::reclaim::WIDEST_TARGET;
use crate::spatial::{kind, Entry};
use crate::tables::*;
use crate::{Handle, SimError, World};
use mc_core::{Fx, FxVec2};

/// Ticks between a reclaimer's looks for the next wreck.
const LOOK_TICKS: usize = 5;
/// How far aside from its way, past its reach, a reclaimer steps for a wreck, metres.
const SWEEP_MARGIN: i32 = 40;
/// A carrier hovers while a wreck it is after lies within this share of its drones'
/// reach (a quarter: 150 m on the Osprey), so they work close to it. Farther ones its
/// drones still take as it flies.
const CARRIER_HOLD_SHARE: i32 = 4;

impl World {
    /// `Command::ReclaimArea`: mobile reclaimers head for `pos` in a group's spread.
    pub(crate) fn order_reclaim_area(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        radius: Fx,
        queue: bool,
    ) -> Result<(), SimError> {
        use crate::command::MAX_RECLAIM_RADIUS;
        let pos = self.clamp_to_map(pos);
        let radius = radius.clamp(Fx::ZERO, MAX_RECLAIM_RADIUS);
        let rows: Vec<usize> = self
            .owned(player, ids, mc_data::cat::MOBILE)
            .into_iter()
            .filter(|&row| self.bp(row).sends_reclaimers())
            .collect();
        for layout in self.formation_layouts(rows, pos, queue, None, 1, 0) {
            for (row, offset) in layout.rows.into_iter().zip(layout.offsets) {
                let mut o = order(OrderKind::ReclaimArea, pos, Handle::NONE);
                o.offset = layout.center + offset - pos;
                if radius > Fx::ZERO {
                    // The spread, kept but never past the edge of the circle.
                    o.offset = o.offset.clamp_length(radius * 3 / 4);
                }
                o.heading = layout.facing;
                o.radius = radius;
                self.give(row, o, queue)?;
            }
        }
        Ok(())
    }

    /// `OrderKind::ReclaimArea`: see the module notes.
    pub(crate) fn run_reclaim_area(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let looks = (self.state.tick as usize + row).is_multiple_of(LOOK_TICKS);
        if self.bp(row).drone_carrier() {
            return self.run_carrier_area(row, o, looks);
        }
        if looks {
            if let Some(w) = self.next_area_wreck(row, o) {
                return self.take_area_wreck(row, w);
            }
        }
        let units = &self.state.units;
        let pos = units.pos[row];
        let spot = self.clamp_to_map(o.pos + o.offset);
        let stuck = units.stuck_ticks[row] == u16::MAX;
        if o.radius > Fx::ZERO {
            // Nothing left in the circle, seen from inside it: done.
            if looks && (pos.distance(o.pos) <= o.radius || stuck) {
                self.finish_order(row);
                return Ok(());
            }
        } else {
            let tolerance = self.bp(row).radius / 2 + Fx::from_int(3);
            if pos.distance(spot) <= tolerance || stuck {
                self.finish_order(row);
                return Ok(());
            }
        }
        if stuck {
            return Ok(());
        }
        self.ensure_moving(row, spot, spot)?;
        // Anything in reach is taken without stopping, as on an ordered reclaim.
        self.reclaim_on_the_way(row)
    }

    /// A carrier on an area order hovers over the wrecks it comes to while its drones
    /// take them (`run_air_support`, which keeps them inside a circle), then goes on: to
    /// its spot on a point, or to the nearest wreck left on a circle. A circle is done
    /// once nothing is left in it to take.
    fn run_carrier_area(&mut self, row: usize, o: &Order, looks: bool) -> Result<(), SimError> {
        let pos = self.state.units.pos[row];
        let stuck = self.state.units.stuck_ticks[row] == u16::MAX;
        let hold = self.bp(row).drone_radius / CARRIER_HOLD_SHARE;
        let wanted: Vec<FxVec2> = self
            .state
            .wrecks
            .slots
            .iter()
            .filter(|&w| self.carrier_wants(row, w))
            .map(|w| self.state.wrecks.pos[w])
            .collect();
        if wanted.iter().any(|at| at.distance(pos) <= hold) {
            self.carrier_hold(row);
            return Ok(());
        }
        let spot = if o.radius > Fx::ZERO {
            match wanted.into_iter().min_by_key(|at| at.distance_sq(pos)) {
                Some(at) if !stuck => at,
                _ => {
                    if looks || stuck {
                        self.finish_order(row);
                    }
                    return Ok(());
                }
            }
        } else {
            let spot = self.clamp_to_map(o.pos + o.offset);
            if pos.distance(spot) <= self.bp(row).radius / 2 + Fx::from_int(3) || stuck {
                self.finish_order(row);
                return Ok(());
            }
            spot
        };
        self.ensure_moving(row, spot, spot)
    }

    /// `Reclaim`/`ReclaimUnit` on a carrier: it flies in over the target and hovers there
    /// while its drones take it apart; they end the order once it is gone
    /// (`run_air_support`).
    pub(crate) fn run_carrier_reclaim(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let at = if o.kind == OrderKind::Reclaim {
            self.state
                .wrecks
                .slots
                .resolve(o.target)
                .map(|w| self.state.wrecks.pos[w])
        } else {
            self.state
                .units
                .row(o.target)
                .map(|t| self.state.units.pos[t])
        };
        let Some(at) = at else {
            self.finish_order(row);
            return Ok(());
        };
        let hold = self.bp(row).drone_radius / CARRIER_HOLD_SHARE;
        if self.state.units.pos[row].distance(at) <= hold {
            self.carrier_hold(row);
            Ok(())
        } else {
            self.ensure_moving(row, at, at)
        }
    }

    /// A carrier stops where it is and hovers while its drones work.
    fn carrier_hold(&mut self, row: usize) {
        if self.state.units.has_flag(row, flag::HAS_FIELD) {
            self.stop_moving(row);
        }
    }

    /// The wreck this reclaimer should take next, nearest first: in the circle, or a
    /// short step off its way. One another reclaimer nearby is on is left to it while
    /// there is another; the one it last gave up on (`o.target`) is left alone.
    fn next_area_wreck(&self, row: usize, o: &Order) -> Option<usize> {
        let bp = self.bp(row);
        let (_, reach) = bp.reclaims().unwrap_or((Fx::ZERO, self.work_range(row)));
        let units = &self.state.units;
        let (pos, owner) = (units.pos[row], units.owner[row]);
        let sweep = reach + Fx::from_int(SWEEP_MARGIN);
        // The circle's wrecks count once the unit has come up to its edge; before
        // that, only what lies along its way.
        let (centre, radius) = (o.pos, o.radius);
        let circle = radius > Fx::ZERO && pos.distance(centre) <= radius + sweep;
        let span = if circle {
            pos.distance(centre) + radius + WIDEST_TARGET
        } else {
            sweep + WIDEST_TARGET
        };
        // Wrecks the owner's other reclaimers about here are on.
        let mut claimed: Vec<Handle> = Vec::new();
        self.index.query(pos, span, kind::UNIT, |e| {
            let t = e.row as usize;
            if t != row && self.unit_entry_is_current(e) && units.owner[t] == owner {
                if let Some(f) = self.state.orders.front(units, t) {
                    if f.kind == OrderKind::Reclaim {
                        claimed.push(f.target);
                    }
                }
            }
            true
        });
        let wrecks = &self.state.wrecks;
        let given_up = wrecks.slots.resolve(o.target);
        let motion = bp.motion;
        let reachable = |e: &Entry| {
            motion.is_none_or(|m| {
                self.nav
                    .nearest_passable(m.layer, m.size_class, e.pos)
                    .is_some_and(|p| p.distance(e.pos) <= reach + e.radius)
            })
        };
        let wanted = |e: &Entry| {
            let w = e.row as usize;
            wrecks.slots.is_alive(w)
                && wrecks.pos[w] == e.pos
                && wrecks.mass[w] > Fx::ZERO
                && given_up != Some(w)
                && self.wreck_known(w, owner)
                && (e.pos.distance(pos) <= sweep + e.radius
                    || (circle && e.pos.distance(centre) <= radius + e.radius))
        };
        let free = self.index.nearest(pos, span, kind::WRECK, |e| {
            wanted(e) && !claimed.contains(&wrecks.slots.handle(e.row as usize)) && reachable(e)
        });
        free.or_else(|| {
            self.index
                .nearest(pos, span, kind::WRECK, |e| wanted(e) && reachable(e))
        })
        .map(|e| e.row as usize)
    }

    /// Puts a `Reclaim` of wreck `w` in front of the area order, and notes it on the
    /// area order: if that wreck is still there when the order next runs, it was given up.
    fn take_area_wreck(&mut self, row: usize, w: usize) -> Result<(), SimError> {
        let handle = self.state.wrecks.slots.handle(w);
        let head = self.state.units.order_head[row];
        if head != NO_ORDER {
            self.state.orders.order[head as usize].target = handle;
        }
        let take = order(OrderKind::Reclaim, self.state.wrecks.pos[w], handle);
        self.state
            .orders
            .push_front(&mut self.state.units, row, take)?;
        self.state.units.stuck_ticks[row] = 0;
        Ok(())
    }
}
