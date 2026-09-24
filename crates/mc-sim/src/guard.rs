//! Guard: hold a spot and go after enemies that come into an area around it.
//!
//! A `Guard` order's `pos` is the middle of the area and `radius` its size;
//! each unit holds `pos + offset`, so a group keeps its spread. A ground or
//! naval unit that sees an enemy it can strike inside the area goes after it:
//! an `Attack` pushed in front of the guard, leashed to the area and a gun's
//! reach beyond it (`run_attack` ends it there), after which the unit walks
//! back to its spot. Aircraft fight whatever is in the area, then keep station
//! over the spot; those an airbase sent out (`target` is the base) go home and
//! down its hatch instead. An airbase's own guard is `Units::guard` (`airbase.rs`).
//!
//! Only a unit free to engage (`FireState::FireAtWill`) leaves its spot. Held
//! position, it stays and shoots what comes into range; on hold fire, it only stands.

use crate::spatial::kind;
use crate::tables::*;
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2};

/// How far past the guard area a chase may run: at least this, metres, or a gun's reach.
const GUARD_LEASH_MIN: i32 = 60;

impl World {
    /// `Command::Guard`: mobile units hold their places around `pos`, keeping their
    /// spread; an airbase keeps the area within its reach.
    pub(crate) fn order_guard(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        radius: Fx,
        queue: bool,
    ) -> Result<(), SimError> {
        use crate::command::{MAX_GUARD_RADIUS, MIN_GUARD_RADIUS};
        let pos = self.clamp_to_map(pos);
        let radius = radius.clamp(MIN_GUARD_RADIUS, MAX_GUARD_RADIUS);
        let rows = self.owned(player, ids, 0);
        // An airbase keeps its guard as a setting of its own, not an order: it can
        // still take an upgrade.
        for &b in &rows {
            if self.bp(b).airbase.is_some() {
                self.set_airbase_guard(b, pos, radius);
            }
        }
        let mobile: Vec<UnitId> = rows
            .iter()
            .filter(|&&r| self.bp(r).is_mobile())
            .map(|&r| self.state.units.id(r))
            .collect();
        let movers = self.owned(player, &mobile, mc_data::cat::MOBILE);
        for layout in self.formation_layouts(movers, pos, queue, 1) {
            for (row, offset) in layout.rows.into_iter().zip(layout.offsets) {
                let mut o = crate::orders::order(OrderKind::Guard, pos, Handle::NONE);
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
        if self.is_air(row) {
            return self.air_guard(row, o);
        }
        let spot = self.clamp_to_map(o.pos + o.offset);
        if let Some(t) = self.guard_intruder(row, o, (self.state.tick as usize + row) % 4 == 0) {
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
    /// torpedo. The airbase's guard asks the same of what it holds, so it never sends
    /// out aircraft that would find nothing to go for and fly straight home.
    pub(crate) fn guard_may_strike(&self, row: usize, t: usize, owner: u8) -> bool {
        let units = &self.state.units;
        self.are_enemies(owner, units.owner[t])
            && !units.has_flag(t, flag::IN_FACTORY)
            && self.can_strike(row, t)
            && self.detects(owner, t)
    }

    /// An aircraft on guard: fights what is in the area; when it is clear, a base's
    /// aircraft go home, others keep station over their spot.
    fn air_guard(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let owner = self.state.units.owner[row];
        let home = self.airbase_for(o.target, owner);
        if home.is_some() && self.guard_should_mend(row) {
            return self.go_home(row, o.target);
        }
        if let Some(t) = self.guard_intruder(row, o, true) {
            return self.air_fight(row, t);
        }
        if home.is_some() {
            return self.go_home(row, o.target);
        }
        let spot = self.clamp_to_map(o.pos + o.offset);
        if self.bp(row).motion.is_some_and(|m| m.hover) {
            self.state.units.flags[row] &= !flag::AIR_RUN;
            return self.ensure_moving(row, spot, spot);
        }
        // Fixed wings circle the spot.
        let radius = self.bp(row).orbit_radius.max(Fx::from_int(160));
        let radial = self.state.units.pos[row] - spot;
        let bearing = if radial.length() < Fx::ONE {
            self.state.units.heading[row]
        } else {
            radial.angle()
        };
        let lead = Angle::from_degrees(35);
        let goal = self.clamp_to_map(
            spot + FxVec2::from_angle(bearing + lead) * crate::orbit::chase_radius(radius, lead),
        );
        self.ensure_moving(row, goal, goal)?;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Back down the hatch of the base in `base`.
    fn go_home(&mut self, row: usize, base: UnitId) -> Result<(), SimError> {
        let Some(b) = self.airbase_for(base, self.state.units.owner[row]) else {
            return Ok(());
        };
        let o = crate::orders::order(OrderKind::Dock, self.state.units.pos[b], base);
        self.give(row, o, false)
    }
}
