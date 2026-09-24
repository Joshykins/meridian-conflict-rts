//! Lift ships: huge aircraft that carry land units (`UnitBlueprint::transport`).
//!
//! A lift ship keeps station high up, in the cloud layer, and never goes after anything
//! by itself: its guns shoot what comes within reach. Told to set down (`OrderKind::Land`,
//! or `Unload`), it flies to the ground the order found for it (`landing_site`), gliding
//! down on the way in (`lift_stand_z`, `lift_speed_cap`, `lift_vertical`) to settle over
//! it, and once down lowers the ramp beneath its belly (`Units::deploy`, up to
//! `Motion::deploy_ticks`). Idle on the ground it stays down with the ramp open; idle in
//! the sky it stays up. Given somewhere to go, it raises the ramp, rises clear, and only
//! then moves off.
//!
//! Land units told to board (`OrderKind::Board`) walk round the hull to its centre line
//! behind the stern, then straight up the ramp to the far end of the hold, where they are
//! stowed as an airbase stores aircraft: the row is kept, `IN_FACTORY`, with `hangar`
//! naming the ship, and it rides with the ship. An idle ship up in the sky comes down for
//! them where it is. `Unload` lets the hold out (or, with a target, that one unit) one unit
//! every `Transport::unload_ticks`, each walking down the centre line to behind the stern
//! and then to a place behind the ship; the order is done once none is left to let out.
//! What is in the hold dies with the ship (`run_airbases`).
//!
//! Its guns are `Weapon::slant`: they reach the ground only along the line of sight, so
//! from the clouds they cannot, and as it comes down to land they begin to clear the site.
//! Each reaches from its own pivot (`gun_origin`).

use crate::orders::order;
use crate::spatial::kind;
use crate::tables::*;
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::{cat, MoveLayer, Transport, Weapon};

/// Most the ground may rise or fall across the hull and still take a lift ship.
const LAND_RISE: Fx = Fx::from_int(6);
/// How far round a landing point to look for ground that takes a lift ship, metres.
const LAND_SEARCH: i32 = 1600;
/// Within this of the ground (metres), a lift ship is down.
const DOWN: Fx = Fx::ONE;
/// Within this of its landing point (metres), a lift ship settles onto the ground.
const OVER_SITE: Fx = Fx::from_int(3);
/// A lift ship starts down its glide this many cruise heights short of the site.
const GLIDE_LENGTH: i32 = 3;
/// Most height over the ground a lift ship's glide levels out at, metres: it brakes
/// to a near stop there, just over the site, and then settles.
const GLIDE_FLOOR: i32 = 12;
/// Below this height, metres, a lift ship taking off only climbs; it moves off once clear.
const LIFT_CLEAR: i32 = 40;
/// Boarding units head for a point this far ahead of them on the ramp's centre line.
const CARROT: i32 = 24;
/// Units spaced out along the centre line wait this far apart, metres (more their size).
const FILE_GAP: i32 = 6;
/// Boarding units wait this far behind the lip of a closed ramp, metres.
const WAIT_BEHIND: i32 = 18;
/// A unit this near the far end of the hold (plus its radius) is stowed.
const STOW_REACH: Fx = Fx::from_int(4);
/// Nothing may stand this near the far end of the hold when the next unit is let out.
const OUT_CLEAR: Fx = Fx::from_int(5);

impl World {
    /// The lift ship spec of `row`, if it is one.
    pub(crate) fn transport(&self, row: usize) -> Option<Transport> {
        self.bp(row).transport
    }

    /// A point in the ship's frame (x along its heading), on the map.
    pub(crate) fn ship_point(&self, ship: usize, local: FxVec2) -> FxVec2 {
        let units = &self.state.units;
        units.pos[ship] + local.rotate(units.heading[ship])
    }

    /// A map point in the ship's frame.
    fn ship_local(&self, ship: usize, p: FxVec2) -> FxVec2 {
        let units = &self.state.units;
        (p - units.pos[ship]).rotate(Angle(units.heading[ship].0.wrapping_neg()))
    }

    /// Whether the lift ship in `row` has set down (a site being built stands on the ground too).
    pub(crate) fn set_down(&self, row: usize) -> bool {
        let units = &self.state.units;
        units.z[row] <= self.terrain.height_at(units.pos[row]) + DOWN
    }

    /// Whether the lift ship in `row` is down with its ramp all the way open.
    pub fn ramp_down(&self, row: usize) -> bool {
        let need = self.bp(row).motion.map_or(0, |m| m.deploy_ticks);
        self.set_down(row) && self.state.units.deploy[row] >= need
    }

    /// Target categories of `row` as it is now: a lift ship on the ground can be hit
    /// by what hits land units, not only by anti-air.
    pub(crate) fn target_layers(&self, row: usize) -> u32 {
        let bp = self.bp(row);
        let layers = bp.target_categories();
        if bp.transport.is_some() && self.set_down(row) {
            layers | cat::LAND
        } else {
            layers
        }
    }

    /// Whether weapon `weapon` of `shooter` reaches `target`, `gap` metres away across
    /// the map (hull to hull): a `slant` gun reaches what is not in the air along the
    /// line of sight, so it cannot reach down to the ground from high up.
    pub(crate) fn slant_reaches(&self, shooter: usize, target: usize, weapon: &Weapon, gap: Fx) -> bool {
        if !weapon.slant {
            return true;
        }
        let units = &self.state.units;
        let aloft = self.is_air(target) && !(self.bp(target).transport.is_some() && self.set_down(target));
        if aloft {
            return true;
        }
        let drop = units.z[shooter] + weapon.muzzle.z - units.z[target] - self.bp(target).height / 2;
        let gap = gap.max(Fx::ZERO);
        // Squares of a few hundred metres fit Fx easily; `sqrt` of a sum is exact enough.
        (gap * gap + drop * drop).sqrt() <= weapon.range_max
    }

    /// Where the reach of `weapon` on `shooter` is measured from across the map: a lift
    /// ship is so long that each of its gun houses reaches from its own pivot; anything
    /// else from its middle.
    pub(crate) fn gun_origin(&self, shooter: usize, weapon: &Weapon) -> FxVec2 {
        let units = &self.state.units;
        match weapon.pivot {
            Some(p) if weapon.mount && self.bp(shooter).transport.is_some() => {
                units.pos[shooter] + p.xy().rotate(units.heading[shooter])
            }
            _ => units.pos[shooter],
        }
    }

    /// How far `gun_origin` lies from the middle of `shooter`, metres.
    pub(crate) fn gun_offset(&self, shooter: usize, weapon: &Weapon) -> Fx {
        match weapon.pivot {
            Some(p) if weapon.mount && self.bp(shooter).transport.is_some() => p.xy().length(),
            _ => Fx::ZERO,
        }
    }

    /// Room taken in the hold of the ship `ship`.
    pub fn cargo_used(&self, ship: usize) -> u16 {
        let units = &self.state.units;
        let id = units.id(ship);
        units
            .slots
            .iter()
            .filter(|&r| units.hangar[r] == id)
            .map(|r| self.bp(r).cargo_room().unwrap_or(1))
            .sum()
    }

    /// Whether the ground round `pos` takes the lift ship in `row`: dry, open land
    /// that rises or falls no more than `LAND_RISE` under the hull, not where
    /// another lift ship is down or setting down.
    fn lift_can_land(&self, row: usize, pos: FxVec2) -> bool {
        let bp = self.bp(row);
        let reach = bp.hull.0.max(bp.hull.1);
        let middle = self.terrain.height_at(pos);
        let size = self.terrain.size_metres();
        if pos.x < reach || pos.y < reach || pos.x > size.x - reach || pos.y > size.y - reach {
            return false;
        }
        for i in 0..17 {
            let sample = if i == 0 {
                pos
            } else {
                let a = Angle(((i - 1) as u16) << 13);
                let r = if i <= 8 { reach } else { reach / 2 };
                pos + FxVec2::from_angle(a) * r
            };
            let z = self.terrain.height_at(sample);
            if z <= self.terrain.water_level()
                || (z - middle).abs() > LAND_RISE
                || !self.nav.passable(MoveLayer::Land, 0, sample)
            {
                return false;
            }
        }
        let units = &self.state.units;
        !units.slots.iter().any(|other| {
            if other == row || self.bp(other).transport.is_none() || !units.is_active(other) {
                return false;
            }
            let apart = reach + self.bp(other).hull.0.max(self.bp(other).hull.1) + Fx::from_int(8);
            let landing_at = self
                .state
                .orders
                .iter(units, other)
                .filter(|o| matches!(o.kind, OrderKind::Land | OrderKind::Unload))
                .any(|o| o.pos.distance(pos) < apart);
            landing_at || (self.set_down(other) && units.pos[other].distance(pos) < apart)
        })
    }

    /// Ground that takes the lift ship in `row` nearest `want`, searched in widening rings.
    fn landing_site(&self, row: usize, want: FxVec2) -> Option<FxVec2> {
        let want = self.clamp_to_map(want);
        if self.lift_can_land(row, want) {
            return Some(want);
        }
        let bp = self.bp(row);
        let step = (bp.hull.1.max(Fx::from_int(8)) / 2).max(Fx::from_int(16));
        let mut dist = step;
        while dist <= Fx::from_int(LAND_SEARCH) {
            let n = (dist * 6 / step).floor_int().clamp(8, 64);
            for i in 0..n {
                let a = Angle((i * 0x10000 / n) as u16);
                let site = self.clamp_to_map(want + FxVec2::from_angle(a) * dist);
                if self.lift_can_land(row, site) {
                    return Some(site);
                }
            }
            dist += step.max(dist / 4);
        }
        None
    }

    /// `Command::Land`: lift ships among `ids` set down at the ground nearest `pos` that
    /// takes them, and with `unload` let everything out.
    pub(crate) fn order_land(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        unload: bool,
        queue: bool,
    ) -> Result<(), SimError> {
        let kind = if unload { OrderKind::Unload } else { OrderKind::Land };
        for row in self.owned(player, ids, 0) {
            if self.transport(row).is_none() {
                continue;
            }
            // Orders already given count: each ship of a group finds ground of its own.
            let Some(site) = self.landing_site(row, pos) else {
                continue;
            };
            let mut o = order(kind, site, Handle::NONE);
            o.heading = self.state.units.heading[row];
            self.give(row, o, queue)?;
        }
        Ok(())
    }

    /// `Command::Unload`: units among `ids` in a lift ship's hold walk out. The ship
    /// lets them out after whatever landing it is making (queued behind a `Land` or an
    /// `Unload` in hand); otherwise it sets down where it is to do so.
    pub(crate) fn order_unload(&mut self, player: u8, ids: &[UnitId]) -> Result<(), SimError> {
        for row in self.owned(player, ids, 0) {
            let units = &self.state.units;
            let Some(ship) = units.row(units.hangar[row]).filter(|&s| self.transport(s).is_some()) else {
                continue;
            };
            let cargo = units.id(row);
            let landing = self
                .state
                .orders
                .front(units, ship)
                .filter(|o| matches!(o.kind, OrderKind::Land | OrderKind::Unload))
                .map(|o| (o.pos, o.kind == OrderKind::Unload && o.target == Handle::NONE));
            match landing {
                // Everything is coming out already.
                Some((_, true)) => {}
                Some((site, false)) => {
                    let mut o = order(OrderKind::Unload, site, cargo);
                    o.heading = self.state.units.heading[ship];
                    self.give(ship, o, true)?;
                }
                None => {
                    let here = self.state.units.pos[ship];
                    let site = if self.set_down(ship) {
                        Some(here)
                    } else {
                        self.landing_site(ship, here)
                    };
                    if let Some(site) = site {
                        let mut o = order(OrderKind::Unload, site, cargo);
                        o.heading = self.state.units.heading[ship];
                        self.give(ship, o, false)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `Command::TakeOff`: lift ships among `ids` that are down, or coming down, raise
    /// the ramp and climb back to cruise height, drifting a little ahead as they go.
    pub(crate) fn order_take_off(&mut self, player: u8, ids: &[UnitId]) -> Result<(), SimError> {
        for row in self.owned(player, ids, 0) {
            let landing = self
                .state
                .orders
                .front(&self.state.units, row)
                .is_some_and(|o| matches!(o.kind, OrderKind::Land | OrderKind::Unload));
            if self.transport(row).is_none() || !(landing || self.set_down(row)) {
                continue;
            }
            let units = &self.state.units;
            let ahead = FxVec2::from_angle(units.heading[row]) * (self.bp(row).hull.0 / 4);
            let mut o = order(OrderKind::Move, self.clamp_to_map(units.pos[row] + ahead), Handle::NONE);
            o.heading = units.heading[row];
            self.give(row, o, false)?;
        }
        Ok(())
    }

    /// Sends the idle lift ship in `ship` down to the nearest ground under it, for units
    /// that want to board. Does nothing if it is busy or already down.
    fn call_down(&mut self, ship: usize) -> Result<(), SimError> {
        if self.state.units.order_head[ship] != NO_ORDER || self.set_down(ship) {
            return Ok(());
        }
        if let Some(site) = self.landing_site(ship, self.state.units.pos[ship]) {
            let mut o = order(OrderKind::Land, site, Handle::NONE);
            o.heading = self.state.units.heading[ship];
            self.give(ship, o, false)?;
        }
        Ok(())
    }

    fn cargo_fits(&self, row: usize, ship: usize) -> bool {
        let t = self.transport(ship).expect("transport");
        let bp = self.bp(row);
        bp.cargo_room().is_some_and(|room| room <= t.capacity)
            && bp.radius * 2 <= t.width
            && bp.height <= t.clearance
    }

    /// `Command::Board`: land units among `ids` walk up the ramp of `carrier`.
    pub(crate) fn order_board(
        &mut self,
        player: u8,
        ids: &[UnitId],
        carrier: UnitId,
        queue: bool,
    ) -> Result<(), SimError> {
        let Some(ship) = self.state.units.row(carrier).filter(|&s| {
            let units = &self.state.units;
            units.owner[s] == player && units.is_active(s) && self.transport(s).is_some()
        }) else {
            return Ok(());
        };
        let pos = self.state.units.pos[ship];
        let mut any = false;
        for row in self.owned(player, ids, 0) {
            let fits = self.cargo_fits(row, ship);
            if fits && row != ship && self.state.units.hangar[row] == Handle::NONE {
                self.give(row, order(OrderKind::Board, pos, carrier), queue)?;
                any = true;
            }
        }
        if any {
            self.call_down(ship)?;
        }
        Ok(())
    }

    /// `OrderKind::Board`: to the foot of the ramp, then, once it is down, up it into the hold.
    pub(crate) fn run_board(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let owner = self.state.units.owner[row];
        let ship = self.state.units.row(o.target).filter(|&s| {
            let units = &self.state.units;
            units.owner[s] == owner && units.is_active(s) && self.transport(s).is_some()
        });
        let Some(ship) = ship else {
            self.finish_order(row);
            return Ok(());
        };
        let t = self.transport(ship).expect("lift ship");
        let room = self.bp(row).cargo_room().unwrap_or(u16::MAX);
        if !self.cargo_fits(row, ship) || self.cargo_used(ship).saturating_add(room) > t.capacity {
            // Full: it waits where it is for other orders.
            self.finish_order(row);
            return Ok(());
        }
        let radius = self.bp(row).radius;
        let (half_len, half_wide) = self.bp(ship).hull;
        let pos = self.state.units.pos[row];
        let local = self.ship_local(ship, pos);
        // Every lift ship is boarded along its ramp's centre line: units line up behind
        // the stern, then walk straight in over the lip. From anywhere else they go round
        // the hull to the line first, not in under it from the side.
        let stern = t.lip.min(-half_len) - radius;
        let line_x = stern - Fx::from_int(12);
        let lane_half = (t.width / 2 - radius).max(Fx::ONE);
        if o.radius == Fx::ZERO {
            let in_lane = local.y.abs() <= lane_half && local.x <= t.hold.x + STOW_REACH;
            if in_lane {
                let head = self.state.units.order_head[row];
                self.state.orders.order[head as usize].radius = Fx::ONE;
            } else if local.x <= stern {
                // Clear of the stern: onto the centre line.
                let line = self.ship_point(ship, FxVec2::new(line_x, Fx::ZERO));
                return self.ensure_moving(row, line, line);
            } else {
                let side = if local.y < Fx::ZERO { -1 } else { 1 };
                let outer = half_wide + radius + Fx::from_int(8);
                let x = if local.y.abs() < outer - Fx::from_int(2) {
                    // Beside or under the hull: step out past its side, or round the nose.
                    if local.x > half_len {
                        half_len + radius + Fx::from_int(8)
                    } else {
                        local.x
                    }
                } else {
                    // Along the flank to the stern corner.
                    line_x
                };
                let around = self.ship_point(ship, FxVec2::new(x, outer * side));
                return self.ensure_moving(row, around, around);
            }
        }
        if !self.ramp_down(ship) {
            // Wait on the centre line behind the lip, spaced out by how near the ramp each is.
            self.call_down(ship)?;
            let ahead = self.boarders_ahead(ship, row, local.x);
            let x = t.lip - Fx::from_int(WAIT_BEHIND) - radius
                - Fx::from_int(ahead as i32) * (radius * 2 + Fx::from_int(FILE_GAP));
            let wait = self.ship_point(ship, FxVec2::new(x, Fx::ZERO));
            if pos.distance(wait) > radius + Fx::from_int(2) {
                let carrot = self.lane_carrot(ship, local, x);
                self.ensure_moving(row, wait, carrot)?;
            } else if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            return Ok(());
        }
        if local.x > t.lip - Fx::from_int(2) && local.y.abs() > lane_half + Fx::from_int(2) {
            // Pushed off the side of the ramp: back out behind the lip and come in again.
            let back = self.ship_point(ship, FxVec2::new(t.lip - radius - Fx::from_int(8), Fx::ZERO));
            return self.ensure_moving(row, back, back);
        }
        let hold = self.ship_point(ship, t.hold);
        if pos.distance(hold) <= STOW_REACH + radius {
            return self.stow(row, ship);
        }
        // Straight up the centre line: a point a little ahead on it pulls the unit onto it.
        let carrot = self.lane_carrot(ship, local, t.hold.x);
        self.ensure_moving(row, hold, carrot)
    }

    /// A point `CARROT` metres ahead of `local` on the ship's centre line, no further
    /// forward than `until` (and no further back, when `local` is past it).
    fn lane_carrot(&self, ship: usize, local: FxVec2, until: Fx) -> FxVec2 {
        let x = if local.x < until {
            (local.x + Fx::from_int(CARROT)).min(until)
        } else {
            (local.x - Fx::from_int(CARROT)).max(until)
        };
        self.ship_point(ship, FxVec2::new(x, Fx::ZERO))
    }

    /// Units boarding `ship`, lined up on its centre line, that stand nearer its ramp
    /// than `x` (ties go to the lower row): where `row` waits in the file.
    fn boarders_ahead(&self, ship: usize, row: usize, x: Fx) -> usize {
        let units = &self.state.units;
        let id = units.id(ship);
        units
            .slots
            .iter()
            .filter(|&r| {
                r != row
                    && self.state.orders.front(units, r).is_some_and(|o| {
                        o.kind == OrderKind::Board && o.target == id && o.radius != Fx::ZERO
                    })
                    && {
                        let other = self.ship_local(ship, units.pos[r]).x;
                        other > x || (other == x && r < row)
                    }
            })
            .count()
    }

    /// Takes the unit in `row` into the hold of `ship`.
    fn stow(&mut self, row: usize, ship: usize) -> Result<(), SimError> {
        self.clear_orders(row)?;
        self.stop_moving(row);
        let t = self.transport(ship).expect("lift ship");
        let id = self.state.units.id(ship);
        let pos = self.ship_point(ship, t.hold);
        let z = self.state.units.z[ship] + t.floor;
        let units = &mut self.state.units;
        units.flags[row] |= flag::IN_FACTORY;
        units.flags[row] &= !flag::MOVING;
        units.hangar[row] = id;
        units.pos[row] = pos;
        units.prev_pos[row] = pos;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.speed[row] = Fx::ZERO;
        units.air_velocity[row] = FxVec3::ZERO;
        units.weapon_target[row] = [Handle::NONE; mc_data::MAX_WEAPONS];
        units.burn_ticks[row] = 0;
        Ok(())
    }

    /// `OrderKind::Land` and `Unload`: fly to the site, come down (`stand_z`), and once
    /// the ramp is down, `Land` is done; `Unload` lets the hold out and is done once it is empty.
    pub(crate) fn run_land(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if self.transport(row).is_none() {
            self.finish_order(row);
            return Ok(());
        }
        let units = &self.state.units;
        if units.pos[row].distance(o.pos) > OVER_SITE || !self.set_down(row) {
            return self.ensure_moving(row, o.pos, o.pos);
        }
        if self.state.units.has_flag(row, flag::HAS_FIELD) {
            self.stop_moving(row);
        }
        if !self.ramp_down(row) {
            return Ok(());
        }
        if o.kind == OrderKind::Land || !self.unload_next(row, o.target)? {
            self.finish_order(row);
        }
        Ok(())
    }

    /// Lets the next unit out of the hold of `ship` when its time comes and the way is
    /// clear: `only` that one when it names one. False once there is none left to let out.
    fn unload_next(&mut self, ship: usize, only: Handle) -> Result<bool, SimError> {
        let t = self.transport(ship).expect("lift ship");
        let id = self.state.units.id(ship);
        let cargo: Vec<usize> = {
            let units = &self.state.units;
            units
                .slots
                .iter()
                .filter(|&r| units.hangar[r] == id && (only == Handle::NONE || units.id(r) == only))
                .collect()
        };
        let Some(&row) = cargo.first() else {
            return Ok(false);
        };
        if self.state.tick % t.unload_ticks as u32 != 0 {
            return Ok(true);
        }
        let out = self.ship_point(ship, t.hold);
        let mut clear = true;
        self.index.query(out, OUT_CLEAR + Fx::from_int(4), kind::UNIT, |e| {
            let r = e.row as usize;
            if self.unit_entry_is_current(e)
                && !self.is_air(r)
                && e.pos.distance(out) < OUT_CLEAR + e.radius
            {
                clear = false;
            }
            clear
        });
        if !clear {
            return Ok(true);
        }
        // Straight down the ramp's centre line and out from under the stern, then off to
        // rows of four behind the ship, the later ones further back.
        let radius = self.bp(row).radius;
        let (half_len, half_wide) = self.bp(ship).hull;
        let exit_x = t.lip.min(-half_len) - radius - Fx::from_int(8);
        let left = {
            let units = &self.state.units;
            units.slots.iter().filter(|&r| units.hangar[r] == id).count()
        };
        let k = (left - 1) as i32;
        let back = exit_x - Fx::from_int(12 + (k / 4) * 12);
        let side = Fx::from_int(((k % 4) * 2 - 3) * 9);
        let goal = self.clamp_to_map(self.ship_point(ship, FxVec2::new(back, side)));
        let heading = Angle(self.state.units.heading[ship].0.wrapping_add(0x8000));
        let ground = self.terrain.height_at(out);
        let units = &mut self.state.units;
        units.flags[row] &= !flag::IN_FACTORY;
        units.hangar[row] = Handle::NONE;
        units.pos[row] = out;
        units.prev_pos[row] = out;
        units.z[row] = ground;
        units.prev_z[row] = ground;
        units.heading[row] = heading;
        units.prev_heading[row] = heading;
        if units.order_head[row] == NO_ORDER {
            let mut o = order(OrderKind::Move, goal, Handle::NONE);
            o.heading = heading;
            self.give(row, o, false)?;
        }
        // An order given in transit that lies ahead of the stern: round the ship's side
        // to it, not back in under the hull.
        if let Some(next) = self.state.orders.front(&self.state.units, row) {
            let destination = self.ship_local(ship, next.pos);
            if destination.x > exit_x {
                let side = if destination.y < Fx::ZERO { -1 } else { 1 };
                let outer = half_wide + radius + Fx::from_int(8);
                let mut corner = order(
                    OrderKind::Move,
                    self.ship_point(ship, FxVec2::new(exit_x, outer * side)),
                    Handle::NONE,
                );
                corner.heading = heading;
                self.state.orders.push_front(&mut self.state.units, row, corner)?;
            }
        }
        // Down the centre line in steps short enough to walk straight.
        let run = t.hold.x - exit_x;
        let steps = (run / Fx::from_int(64)).ceil_int().max(1);
        for i in (1..=steps).rev() {
            let x = t.hold.x - run * i / steps;
            let mut o = order(OrderKind::Move, self.ship_point(ship, FxVec2::new(x, Fx::ZERO)), Handle::NONE);
            o.heading = heading;
            self.state.orders.push_front(&mut self.state.units, row, o)?;
        }
        Ok(true)
    }

    /// Where a lift ship stands (`stand_z`). Setting down, it glides in: from its cruise
    /// height `GLIDE_LENGTH` cruise heights out, easing down (steepest half-way, flat at
    /// both ends) to `GLIDE_FLOOR` over the site, where it settles once over it. Down, or
    /// idle on the ground, or while its ramp is still coming up, the ground; its cruise
    /// height otherwise.
    pub(crate) fn lift_stand_z(&self, row: usize, pos: FxVec2, surface: Fx, altitude: Fx) -> Fx {
        let units = &self.state.units;
        let on_ground = units.z[row] <= surface + DOWN;
        match self.state.orders.front(units, row) {
            Some(o) if matches!(o.kind, OrderKind::Land | OrderKind::Unload) => {
                let d = pos.distance(o.pos);
                if d <= OVER_SITE || (on_ground && units.deploy[row] > 0) {
                    surface
                } else {
                    surface + glide_height(altitude, d)
                }
            }
            None if on_ground => surface,
            Some(_) if on_ground && units.deploy[row] > 0 => surface,
            _ => surface + altitude,
        }
    }

    /// Fastest a lift ship in `row` may fly across the map this tick (`move_unit`), if
    /// less than its top speed. Lifting off, it rises clear before it moves off and gathers
    /// way as it climbs. Setting down, it slows so that it is over the site no sooner than
    /// it can come down to its glide floor there, at half its `descent` rate.
    pub(crate) fn lift_speed_cap(&self, row: usize, pos: FxVec2, top: Fx) -> Option<Fx> {
        let t = self.transport(row)?;
        let motion = self.bp(row).motion?;
        let units = &self.state.units;
        let surface = self.terrain.height_at(pos).max(self.terrain.water_level());
        let height = units.z[row] - surface;
        let want = self.lift_stand_z(row, pos, surface, motion.altitude) - surface;
        let mut cap: Option<Fx> = None;
        let clear = Fx::from_int(LIFT_CLEAR).min(motion.altitude / 4);
        let full = motion.altitude / 2;
        if want > height + Fx::from_int(2) && height < full {
            let share = ((height - clear) / (full - clear).max(Fx::ONE)).clamp(Fx::ZERO, Fx::ONE);
            cap = Some(top * share);
        }
        if let Some(o) = self
            .state
            .orders
            .front(units, row)
            .filter(|o| matches!(o.kind, OrderKind::Land | OrderKind::Unload))
        {
            let excess = height - Fx::from_int(GLIDE_FLOOR).min(motion.altitude / 4);
            if excess > Fx::ONE {
                let slow = pos.distance(o.pos) * (t.descent / 2) / excess;
                cap = Some(cap.map_or(slow, |c| c.min(slow)));
            }
        }
        cap
    }

    /// A lift ship's climb or descent this tick toward `want_z`, metres (`run_movement`).
    /// It gathers vertical speed at an eighth of its `descent` rate per second, tops out
    /// at `descent`, and brakes so as to arrive at rest; near the ground it comes down
    /// slower still, so it settles rather than drops.
    pub(crate) fn lift_vertical(&self, row: usize, pos: FxVec2, want_z: Fx) -> Fx {
        let t = self.transport(row).expect("lift ship");
        let units = &self.state.units;
        let dt = Fx::from_int(mc_core::TICKS_PER_SECOND as i32);
        let z = units.z[row];
        let remaining = want_z - z;
        let dist = remaining.abs();
        let accel = t.descent / 8;
        // What it can still stop in, with a fifth in hand.
        let mut speed = t.descent.min((accel * dist * Fx::ratio(8, 5)).sqrt());
        if remaining < Fx::ZERO {
            let above = z - self.terrain.height_at(pos).max(self.terrain.water_level());
            speed = speed.min(above.max(Fx::ZERO) / 2 + Fx::ONE);
        }
        let step = speed / dt;
        let desired = if remaining < Fx::ZERO { -step } else { step };
        let previous = units.air_velocity[row].z;
        let velocity = previous.approach(desired, accel / dt / dt);
        if (velocity.abs() >= dist && (velocity < Fx::ZERO) == (remaining < Fx::ZERO))
            || (dist < Fx::ratio(1, 100) && velocity.abs() < Fx::ratio(1, 100))
        {
            remaining
        } else {
            velocity
        }
    }

    /// Every tick, after orders: each lift ship's ramp, and its hold riding with it.
    pub(crate) fn run_transports(&mut self) {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        for &ship in &rows {
            let Some(t) = self.transport(ship) else {
                continue;
            };
            if !self.state.units.is_active(ship) {
                continue;
            }
            let need = self.bp(ship).motion.map_or(0, |m| m.deploy_ticks);
            // Open only where it means to be: not with a site elsewhere to fly to.
            let staying = self.state.orders.front(&self.state.units, ship).is_none_or(|o| {
                matches!(o.kind, OrderKind::Land | OrderKind::Unload)
                    && self.state.units.pos[ship].distance(o.pos) <= OVER_SITE
            });
            let open = self.set_down(ship) && staying;
            let units = &mut self.state.units;
            if open {
                units.deploy[ship] = (units.deploy[ship] + 1).min(need);
            } else if units.deploy[ship] > 0 {
                units.deploy[ship] -= 1;
                units.flags[ship] |= flag::HOLD;
            }
            // The hold rides with the ship.
            let id = units.id(ship);
            let at = units.pos[ship] + t.hold.rotate(units.heading[ship]);
            let z = units.z[ship] + t.floor;
            for &r in &rows {
                if units.hangar[r] == id {
                    units.pos[r] = at;
                    units.prev_pos[r] = at;
                    units.z[r] = z;
                    units.prev_z[r] = z;
                }
            }
        }
    }
}

/// Height over the ground of a lift ship's glide `d` metres short of its landing site,
/// for a cruise height of `altitude`: a smoothstep from `altitude` at `GLIDE_LENGTH`
/// cruise heights out down to the glide floor over the site.
fn glide_height(altitude: Fx, d: Fx) -> Fx {
    let floor = Fx::from_int(GLIDE_FLOOR).min(altitude / 4);
    let length = (altitude * GLIDE_LENGTH).max(Fx::ONE);
    let s = (d / length).clamp(Fx::ZERO, Fx::ONE);
    floor + (altitude - floor) * s * s * (Fx::from_int(3) - s * 2)
}

/// Presentation of a lift ship that is down (`World::lift_decks`): enough to raise what
/// walks its ramp and hold onto the deck, since the sim keeps land units on the ground.
#[derive(Clone, Copy, Debug)]
pub struct Deck {
    pub pos: [f32; 2],
    /// Its heading as a unit vector.
    pub dir: [f32; 2],
    pub ground: f32,
    pub hinge: f32,
    pub lip: f32,
    pub front: f32,
    pub half_width: f32,
    pub floor: f32,
    /// How far the ramp is down, zero to one.
    pub open: f32,
}

impl Deck {
    /// Metres over the ground of the deck at `p`, where a unit there stands; zero off it.
    pub fn lift(&self, p: [f32; 2]) -> f32 {
        let d = [p[0] - self.pos[0], p[1] - self.pos[1]];
        let x = d[0] * self.dir[0] + d[1] * self.dir[1];
        let y = -d[0] * self.dir[1] + d[1] * self.dir[0];
        if y.abs() > self.half_width || x < self.lip || x > self.front {
            return 0.0;
        }
        if x >= self.hinge {
            return self.floor;
        }
        // Down the ramp: it lies from the hinge to the lip once open, and swings up closed.
        let along = (x - self.lip) / (self.hinge - self.lip);
        self.floor * along * self.open
    }

    /// Which way is up for a unit standing on the deck at `p` (world, unit length): tilted
    /// with the ramp's slope on the ramp, easing in over its foot and its top so a unit
    /// does not snap onto it; straight up on the hold floor. `None` off the deck.
    pub fn up(&self, p: [f32; 2]) -> Option<[f32; 3]> {
        let d = [p[0] - self.pos[0], p[1] - self.pos[1]];
        let x = d[0] * self.dir[0] + d[1] * self.dir[1];
        let y = -d[0] * self.dir[1] + d[1] * self.dir[0];
        if y.abs() > self.half_width || x < self.lip || x > self.front {
            return None;
        }
        let run = (self.hinge - self.lip).max(1.0);
        let ease = |e: f32| {
            let e = e.clamp(0.0, 1.0);
            e * e * (3.0 - 2.0 * e)
        };
        let on_ramp = ease((x - self.lip) / 6.0) * (1.0 - ease((x - self.hinge + 6.0) / 6.0));
        let slope = self.floor * self.open / run * on_ramp;
        let n = (slope * slope + 1.0).sqrt();
        Some([-slope * self.dir[0] / n, -slope * self.dir[1] / n, 1.0 / n])
    }
}

/// Metres over the ground below which a lift ship's legs are all the way out.
const GEAR_HEIGHT: i32 = 60;

impl World {
    /// Lift ships that are down, for raising what walks on their decks.
    pub fn lift_decks(&self) -> Vec<Deck> {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter_map(|r| {
                let t = self.transport(r)?;
                if !units.is_active(r) || !self.set_down(r) {
                    return None;
                }
                let need = self.bp(r).motion.map_or(1, |m| m.deploy_ticks.max(1));
                let h = units.heading[r].to_radians_f32();
                Some(Deck {
                    pos: [units.pos[r].x.to_f32(), units.pos[r].y.to_f32()],
                    dir: [h.cos(), h.sin()],
                    ground: self.terrain.height_at(units.pos[r]).to_f32(),
                    hinge: t.hinge.to_f32(),
                    lip: t.lip.to_f32(),
                    front: (t.hold.x + STOW_REACH * 2).to_f32(),
                    half_width: (t.width / 2).to_f32(),
                    floor: t.floor.to_f32(),
                    open: units.deploy[r] as f32 / need as f32,
                })
            })
            .collect()
    }

    /// A lift ship's landing gear, 0 stowed to 255 all the way out: out near the ground.
    pub fn lift_gear(&self, row: usize) -> u32 {
        if self.transport(row).is_none() {
            return 0;
        }
        let units = &self.state.units;
        let height = units.z[row] - self.terrain.height_at(units.pos[row]);
        let out = Fx::ONE - (height / GEAR_HEIGHT).clamp(Fx::ZERO, Fx::ONE);
        (out * 255).round_int().clamp(0, 255) as u32
    }

    /// What the lift ship in `row` carries, for the interface. `None` for anything else.
    pub fn cargo_view(&self, row: usize) -> Option<crate::mirror::CargoView> {
        let t = self.transport(row)?;
        let units = &self.state.units;
        let id = units.id(row);
        let stored: Vec<crate::mirror::CargoUnit> = units
            .slots
            .iter()
            .filter(|&r| units.hangar[r] == id)
            .map(|r| crate::mirror::CargoUnit {
                unit_id: units.id(r).0,
                blueprint: units.blueprint[r],
                health: (units.health[r] / self.unit_max_health(r))
                    .to_f32()
                    .clamp(0.0, 1.0),
                room: self.bp(r).cargo_room().unwrap_or(1) as u8,
            })
            .collect();
        let boarding = units
            .slots
            .iter()
            .filter(|&r| {
                self.state
                    .orders
                    .front(units, r)
                    .is_some_and(|o| o.kind == OrderKind::Board && o.target == id)
            })
            .count() as u16;
        let front = self.state.orders.front(units, row);
        let unloading = front.is_some_and(|o| o.kind == OrderKind::Unload);
        // What its unload orders will let out: the lot, or the units they name.
        let in_hold = |h: Handle| units.row(h).is_some_and(|r| units.hangar[r] == id);
        let mut to_unload = 0u16;
        for o in self.state.orders.iter(units, row) {
            if o.kind != OrderKind::Unload {
                continue;
            }
            if o.target == Handle::NONE {
                to_unload = stored.len() as u16;
                break;
            }
            to_unload += u16::from(in_hold(o.target));
        }
        let to_unload = to_unload.min(stored.len() as u16);
        use crate::mirror::LiftPhase;
        let need = self.bp(row).motion.map_or(0, |m| m.deploy_ticks);
        let landing = front.filter(|o| matches!(o.kind, OrderKind::Land | OrderKind::Unload));
        let phase = if self.set_down(row) {
            let staying = front.is_none() || landing.is_some_and(|o| units.pos[row].distance(o.pos) <= OVER_SITE);
            if !staying {
                LiftPhase::RampClosing
            } else if units.deploy[row] < need {
                LiftPhase::RampOpening
            } else if unloading && to_unload > 0 {
                LiftPhase::Unloading
            } else {
                LiftPhase::Ready
            }
        } else if landing.is_some() {
            LiftPhase::Descending
        } else {
            let cruise = self.bp(row).motion.map_or(Fx::ZERO, |m| m.altitude);
            let ground = self.terrain.height_at(units.pos[row]).max(self.terrain.water_level());
            if units.z[row] < ground + cruise - Fx::from_int(8) && units.air_velocity[row].z > Fx::ZERO {
                LiftPhase::TakingOff
            } else {
                LiftPhase::InFlight
            }
        };
        Some(crate::mirror::CargoView {
            capacity: t.capacity,
            used: self.cargo_used(row),
            stored,
            boarding,
            ramp_down: self.ramp_down(row),
            unloading,
            phase,
            to_unload,
        })
    }
}
