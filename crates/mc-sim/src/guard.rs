//! Guard: hold a spot and go after enemies that come into an area around it.
//!
//! A `Guard` order's `pos` is the middle of the area and `radius` its size;
//! each unit holds `pos + offset`, so a group keeps its spread. A ground or
//! naval unit that sees an enemy it can strike inside the area goes after it:
//! an `Attack` pushed in front of the guard, leashed to the area and a gun's
//! reach beyond it (`run_attack` ends it there), after which the unit walks
//! back to its spot. Aircraft chase what is in the area the same way (leashed
//! wider, for a bombing pass carries them well out), then circle it
//! halfway out, a group in formation (`orbit.rs`); one that orbits
//! (`UnitBlueprint::orbit`, the Argus) circles on the edge itself, and takes up an
//! orbit of its own wherever it is left idle.
//!
//! With a friendly unit as `target`, the area goes with it: `pos` follows the
//! unit, and stays where it was last if the unit is lost. A guard never ends by
//! itself; anything queued behind it takes over at once.
//!
//! An engineer on guard works the area: it helps raise what goes up in it, mends
//! friends and reclaims wrecks there, and takes up work that appears later
//! (`area_work.rs`).
//!
//! Only a unit free to engage (`FireState::FireAtWill`) leaves its spot. Held
//! position, it stays and shoots what comes into range; on hold fire, it only stands.

use crate::spatial::kind;
use crate::tables::*;
use crate::{Handle, SimError, World};
use mc_core::{Fx, FxVec2};

/// How far past the guard area a chase may run: at least this, metres, or a gun's reach.
const GUARD_LEASH_MIN: i32 = 60;
/// Gap a unit on guard round another keeps from that unit's hull, metres.
const GUARD_CLEARANCE: i32 = 8;

impl World {
    /// `Command::Guard`: mobile units hold their places around `pos`, keeping their
    /// spread; aircraft circle it. `target`, if a friend, is followed.
    pub(crate) fn order_guard(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        target: UnitId,
        radius: Fx,
        queue: bool,
    ) -> Result<(), SimError> {
        use crate::command::{MAX_GUARD_RADIUS, MIN_GUARD_RADIUS};
        let pos = self.clamp_to_map(pos);
        let radius = radius.clamp(MIN_GUARD_RADIUS, MAX_GUARD_RADIUS);
        let target = self
            .state
            .units
            .row(target)
            .filter(|&t| !self.are_enemies(player, self.state.units.owner[t]))
            .map_or(Handle::NONE, |_| target);
        let (air, movers): (Vec<usize>, Vec<usize>) = self
            .owned(player, ids, mc_data::cat::MOBILE)
            .into_iter()
            .partition(|&row| self.is_air(row));
        // The unit being followed cannot guard itself: it guards the spot.
        let (followed, flock): (Vec<usize>, Vec<usize>) = air
            .into_iter()
            .partition(|&row| self.state.units.id(row) == target);
        self.order_air_guard(followed, pos, Handle::NONE, radius, queue)?;
        self.order_air_guard(flock, pos, target, radius, queue)?;
        for layout in self.formation_layouts(movers, pos, queue, None, 1, 0) {
            for (row, offset) in layout.rows.into_iter().zip(layout.offsets) {
                let anchor = if self.state.units.id(row) == target {
                    Handle::NONE
                } else {
                    target
                };
                let mut o = crate::orders::order(OrderKind::Guard, pos, anchor);
                // The spread, kept but never past the edge of the area.
                o.offset = (layout.center + offset - pos).clamp_length(radius * 3 / 4);
                o.heading = layout.facing;
                o.radius = radius;
                self.give(row, o, queue)?;
            }
        }
        Ok(())
    }

    /// `OrderKind::Guard`: see the module notes.
    pub(crate) fn run_guard(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if self.bp(row).motion.is_none() {
            // A structure has nowhere to go: its weapons look after themselves.
            return Ok(());
        }
        if self
            .state
            .orders
            .iter(&self.state.units, row)
            .nth(1)
            .is_some()
        {
            self.finish_order(row);
            return Ok(());
        }
        let centre = self
            .state
            .units
            .row(o.target)
            .map_or(o.pos, |t| self.state.units.pos[t]);
        // Kept, so the last centre stays when the followed unit is gone.
        let head = self.state.units.order_head[row];
        if head != NO_ORDER {
            self.state.orders.order[head as usize].pos = centre;
        }
        let o = &Order { pos: centre, ..*o };
        if self.is_air(row) {
            return self.air_guard(row, o);
        }
        let spot = self.clamp_to_map(o.pos + self.guard_offset(row, o));
        if let Some(t) =
            self.guard_intruder(row, o, (self.state.tick as usize + row).is_multiple_of(4))
        {
            let reach = self
                .bp(row)
                .max_weapon_range()
                .max(Fx::from_int(GUARD_LEASH_MIN));
            let mut chase = crate::orders::order(OrderKind::Attack, o.pos, self.state.units.id(t));
            chase.radius = o.radius + reach;
            self.state
                .orders
                .push_front(&mut self.state.units, row, chase)?;
            self.state.units.stuck_ticks[row] = 0;
            return Ok(());
        }
        // An engineer works the whole area (`area_work.rs`).
        if self.area_work(row, o)? {
            return Ok(());
        }
        let units = &self.state.units;
        let tolerance = self.bp(row).radius / 2 + Fx::from_int(3);
        if units.pos[row].distance(spot) > tolerance && units.stuck_ticks[row] != u16::MAX {
            if self.has_clear_target(row) {
                // Shooting on the way back, as an attack-move does.
                self.state.units.flags[row] |= flag::HOLD;
            }
            return self.ensure_moving(row, spot, spot);
        }
        // On its spot: stand, face out, and a builder mends what is near.
        if self.state.units.has_flag(row, flag::HAS_FIELD) {
            self.stop_moving(row);
        }
        let turn = self.bp(row).motion.expect("mobile").turn_rate;
        let units = &mut self.state.units;
        units.flags[row] |= flag::HOLD;
        units.speed[row] = Fx::ZERO;
        units.heading[row] = units.heading[row].turn_toward(o.heading, turn);
        if self.bp(row).builder.is_some() && !self.idle_repair(row)? {
            self.idle_reclaim(row)?;
        }
        Ok(())
    }

    /// Where a ground or naval unit on guard stands, from the centre: its place in the
    /// group, but round a unit it follows never on that unit's hull (it would shove
    /// the unit along for ever), so a lone escort keeps behind it.
    fn guard_offset(&self, row: usize, o: &Order) -> FxVec2 {
        let Some(t) = self.state.units.row(o.target) else {
            return o.offset;
        };
        let clear = self.bp(row).radius + self.bp(t).radius + Fx::from_int(GUARD_CLEARANCE);
        let len = o.offset.length();
        if len >= clear {
            o.offset
        } else if len >= Fx::ONE {
            o.offset * (clear / len)
        } else {
            FxVec2::from_angle(self.state.units.heading[t]) * -clear
        }
    }

    /// The enemy nearest this unit inside its guard area that it may go after,
    /// when it is free to and has nothing in its sights. `look`: whether to
    /// search this tick (a ground unit looks a few times a second).
    fn guard_intruder(&self, row: usize, o: &Order, look: bool) -> Option<usize> {
        let bp = self.bp(row);
        let units = &self.state.units;
        if !look
            || bp.weapons.is_empty()
            || !self.chases(row)
            || units.has_flag(row, flag::PASSIVE)
            || (!self.is_air(row) && self.has_clear_target(row))
        {
            return None;
        }
        let (pos, owner) = (units.pos[row], units.owner[row]);
        // A pass through the air carries an aircraft out past the edge and back.
        let slack = if self.is_air(row) {
            bp.vision / 2
        } else {
            Fx::ZERO
        };
        let span = pos.distance(o.pos) + o.radius + slack;
        self.index
            .nearest(pos, span + crate::reclaim::WIDEST_TARGET, kind::UNIT, |e| {
                let t = e.row as usize;
                self.unit_entry_is_current(e)
                    && e.pos.distance(o.pos) <= o.radius + e.radius + slack
                    && self.guard_may_strike(row, t, owner)
            })
            .map(|e| e.row as usize)
    }

    /// Whether the unit in `row` (of side `owner`) may go after `t` for a guard: an
    /// enemy it sees and has a weapon that reaches, a dived hull included for a
    /// torpedo.
    fn guard_may_strike(&self, row: usize, t: usize, owner: u8) -> bool {
        let units = &self.state.units;
        self.are_enemies(owner, units.owner[t])
            && !units.has_flag(t, flag::IN_FACTORY)
            && self.can_strike(row, t)
            && self.detects(owner, t)
    }

    /// An aircraft on guard: fights what is in the area; when it is clear, it circles.
    ///
    /// It goes after an intruder as an `Attack` pushed in front of the guard, as a
    /// ground unit does, so it keeps one mark from the run in to the release line
    /// and its weapons aim at that mark too (`run_targeting`). Picking the nearest
    /// enemy afresh every tick, a bomber over a crowd flew at one unit while its bay
    /// was on another, and never opened.
    fn air_guard(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(t) = self.guard_intruder(row, o, true) else {
            return self.fly_circle(row, o);
        };
        // The pass carries it out past the area's edge and round again; one still
        // flying in from further out (off the factory floor) is leashed from there.
        let units = &self.state.units;
        let leash = (o.radius + self.bp(row).vision / 2).max(units.pos[row].distance(o.pos))
            + self.air_run_distance(row, units.pos[t]);
        let mut chase = crate::orders::order(OrderKind::Attack, o.pos, self.state.units.id(t));
        chase.radius = leash;
        self.state
            .orders
            .push_front(&mut self.state.units, row, chase)?;
        self.air_fight(row, t)
    }
}
