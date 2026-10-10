//! Air staging: an aircraft with a tank (`UnitBlueprint::fuel`) burns it while it is up,
//! and flies slower and turns wider once it is dry. It fills again, and mends, aboard a
//! staging base (`UnitBlueprint::staging`): an airfield, a carrier.
//!
//! Set down on bare ground (an idle aircraft lands where it can), it fills slowly by
//! itself, a full tank in three minutes.
//!
//! A free aircraft (no order, or guarding or patrolling) goes down on its own:
//! - within a base's radius once it is under `NEEDY_FUEL` of a tank or `NEEDY_HEALTH` of
//!   its health;
//! - from anywhere once what is left would only just bring it to the nearest base
//!   (`RESERVE` to spare).
//!
//! It is sent with an `OrderKind::Stage` pushed in front of what it was doing, so it goes
//! back to that once it leaves. On a base whose launch point is not its landing point (a
//! carrier: down on the stern, off the bow) it comes in from astern along the deck. A base
//! takes `pads` aircraft at a time, the rest wait over it; aboard, an aircraft is stowed
//! like a lift ship's cargo (`Units::hangar`, `IN_FACTORY`), rides with the base, and dies
//! with it. Full and mended it is let off the launch point, one every `launch_ticks`.

use crate::orders::order;
use crate::tables::{flag, Handle, Order, OrderKind, UnitId};
use crate::{SimError, World};
use mc_core::{Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::{Motion, MoveLayer};

/// Under this share of a full tank an aircraft in a base's radius goes down to fill up.
const NEEDY_FUEL: (i32, i32) = (3, 5);
/// Under this share of its health an aircraft in a base's radius goes down to mend.
const NEEDY_HEALTH: (i32, i32) = (7, 10);
/// Metres of flight an aircraft keeps in hand over the way to the nearest base.
const RESERVE: i32 = 2000;
/// A loitering aircraft burns at least this share of what it burns at full speed.
const IDLE_BURN: (i32, i32) = (2, 5);
/// Ticks an empty tank takes to fill on bare ground: three minutes, against seconds on
/// a staging base.
const GROUND_REFUEL_TICKS: i32 = 180 * TICKS_PER_SECOND as i32;
/// Dry: the share of its speed, acceleration and turn an aircraft keeps.
const DRY: (i32, i32) = (1, 2);
/// How far astern of a directional deck the approach starts.
const APPROACH: i32 = 500;
/// Close enough to the approach point to turn in.
const ON_APPROACH: i32 = 140;
/// Close enough over the landing point to come down on it.
const TOUCHDOWN: i32 = 60;
/// On a deck flown into from astern (a carrier's tunnel): close enough to be inside the
/// stern mouth, so it is not taken aboard out over the water.
const TOUCHDOWN_ASTERN: i32 = 20;
/// The glide down to the deck starts this far out.
const GLIDE: i32 = 450;
/// Looked at twice a second, spread over the rows.
const LOOK_EVERY: usize = 16;

impl World {
    /// `motion` as this aircraft flies it now: slower and wider once its tank is dry.
    pub(crate) fn fuelled_motion(&self, row: usize, motion: &Motion) -> Motion {
        let mut m = *motion;
        if self.bp(row).fuel.is_some() && self.state.units.fuel[row] <= Fx::ZERO {
            m.speed = m.speed * DRY.0 / DRY.1;
            m.accel = m.accel * DRY.0 / DRY.1;
            m.turn_rate = (m.turn_rate as i32 * DRY.0 / DRY.1).max(1) as u16;
        }
        m
    }

    /// Every tick, after orders: aircraft up burn fuel; those aboard a base fill, mend and
    /// leave; free ones that need it are sent down.
    pub(crate) fn run_staging(&mut self) -> Result<(), SimError> {
        let bases: Vec<usize> = {
            let units = &self.state.units;
            units
                .slots
                .iter()
                .filter(|&r| {
                    self.bp(r).staging.is_some() && units.is_active(r) && units.health[r] > Fx::ZERO
                })
                .collect()
        };
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        // One aircraft off each deck a tick.
        let mut launched: Vec<Handle> = Vec::new();
        for &row in &rows {
            let Some(tank) = self.bp(row).fuel else {
                continue;
            };
            let aboard = self.state.units.hangar[row];
            if aboard != Handle::NONE {
                if let Some(base) = self.state.units.row(aboard) {
                    if self.bp(base).staging.is_some()
                        && self.service(row, base, tank.range)
                        && !launched.contains(&aboard)
                    {
                        launched.push(aboard);
                        self.launch(row, base);
                    }
                }
                continue;
            }
            if !self.state.units.is_active(row) {
                continue;
            }
            self.burn(row, tank.range);
            if (self.state.tick as usize + row).is_multiple_of(LOOK_EVERY) {
                self.send_down(row, &bases, tank.range)?;
            }
        }
        self.work_doors(&bases);
        Ok(())
    }

    /// A base's hatch opens while an aircraft is coming down on it, and shuts after.
    fn work_doors(&mut self, bases: &[usize]) {
        for &base in bases {
            let Some(s) = self.bp(base).staging.filter(|s| s.door_ticks > 0) else {
                continue;
            };
            let land = self.ship_point(base, s.land.xy());
            let id = self.state.units.id(base);
            let units = &self.state.units;
            let incoming = units.slots.iter().any(|r| {
                units.hangar[r] == Handle::NONE
                    && units.pos[r].distance(land) < Fx::from_int(GLIDE)
                    && self
                        .state
                        .orders
                        .front(units, r)
                        .is_some_and(|o| o.kind == OrderKind::Stage && o.target == id)
            });
            let units = &mut self.state.units;
            units.deploy[base] = if incoming {
                (units.deploy[base] + 1).min(s.door_ticks)
            } else {
                units.deploy[base].saturating_sub(1)
            };
        }
    }

    /// Burns this tick's fuel while it is up; set down on the ground, it fills slowly
    /// (`GROUND_REFUEL_TICKS` from empty).
    fn burn(&mut self, row: usize, range: Fx) {
        let Some(motion) = self.bp(row).motion else {
            return;
        };
        let units = &self.state.units;
        if units.has_flag(row, flag::IN_FACTORY) {
            return;
        }
        if units.z[row] <= self.ground_z(row, units.pos[row]) + Fx::from_int(2) {
            let fuel = &mut self.state.units.fuel[row];
            *fuel = (*fuel + range / GROUND_REFUEL_TICKS).min(range);
            return;
        }
        let pace = units.speed[row].max(motion.speed * IDLE_BURN.0 / IDLE_BURN.1);
        let fuel = &mut self.state.units.fuel[row];
        *fuel = (*fuel - pace / TICKS_PER_SECOND as i32).max(Fx::ZERO);
    }

    /// Free and in need: a `Stage` on the nearest base that will have it.
    fn send_down(&mut self, row: usize, bases: &[usize], range: Fx) -> Result<(), SimError> {
        let units = &self.state.units;
        // A fight is pushed in front of a guard or a patrol, so one in a fight is not free.
        let free = units.drone_parent[row] == Handle::NONE
            && self
                .state
                .orders
                .front(units, row)
                .is_none_or(|o| matches!(o.kind, OrderKind::Guard | OrderKind::Patrol));
        if !free {
            return Ok(());
        }
        let (pos, owner) = (units.pos[row], units.owner[row]);
        let mut nearest: Option<(usize, Fx)> = None;
        for &b in bases {
            if b == row || self.are_enemies(owner, units.owner[b]) {
                continue;
            }
            let d = units.pos[b].distance(pos);
            if nearest.is_none_or(|(_, n)| d < n) {
                nearest = Some((b, d));
            }
        }
        let Some((base, dist)) = nearest else {
            return Ok(());
        };
        let fuel = units.fuel[row];
        let radius = self.bp(base).staging.map_or(Fx::ZERO, |s| s.radius);
        let max = self.unit_max_health(row);
        let needy = fuel * NEEDY_FUEL.1 < range * NEEDY_FUEL.0
            || units.health[row] * NEEDY_HEALTH.1 < max * NEEDY_HEALTH.0;
        let bingo = fuel < dist + Fx::from_int(RESERVE);
        if !(bingo || needy && dist <= radius) {
            return Ok(());
        }
        let stage = order(OrderKind::Stage, units.pos[base], units.id(base));
        self.stop_moving(row);
        self.state.units.flags[row] &= !flag::AIR_RUN;
        self.state
            .orders
            .push_front(&mut self.state.units, row, stage)
    }

    /// `Command::Stage`: aircraft with a tank among `ids` go down on `base`.
    pub(crate) fn order_stage(
        &mut self,
        player: u8,
        ids: &[UnitId],
        base: UnitId,
        queue: bool,
    ) -> Result<(), SimError> {
        let Some(b) = self.state.units.row(base) else {
            return Ok(());
        };
        let pos = self.state.units.pos[b];
        for row in self.owned(player, ids, 0) {
            if self.bp(row).fuel.is_some()
                && self.state.units.hangar[row] == Handle::NONE
                && self.stage_base(row, base).is_some()
            {
                self.give(row, order(OrderKind::Stage, pos, base), queue)?;
            }
        }
        Ok(())
    }

    /// `OrderKind::Stage`: fly to the base (round to astern of a directional deck first),
    /// come down, and go aboard once a pad is free.
    pub(crate) fn run_stage(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if self.state.units.hangar[row] != Handle::NONE {
            return Ok(());
        }
        let Some(base) = self.stage_base(row, o.target) else {
            self.finish_order(row);
            return Ok(());
        };
        let s = self.bp(base).staging.expect("staging base");
        let land = self.ship_point(base, s.land.xy());
        let pos = self.state.units.pos[row];
        if s.directional() && o.radius == Fx::ZERO {
            let astern =
                land - FxVec2::from_angle(self.state.units.heading[base]) * Fx::from_int(APPROACH);
            if pos.distance(astern) > Fx::from_int(ON_APPROACH) {
                return self.ensure_moving(row, astern, astern);
            }
            let head = self.state.units.order_head[row];
            self.state.orders.order[head as usize].radius = Fx::ONE;
        }
        let touchdown = if s.directional() {
            TOUCHDOWN_ASTERN
        } else {
            TOUCHDOWN
        };
        if pos.distance(land) <= Fx::from_int(touchdown)
            && self.pads_free(base)
            && self.state.units.deploy[base] >= s.door_ticks
        {
            self.stow_aboard(row, base);
            return Ok(());
        }
        self.ensure_moving(row, land, land)
    }

    /// The base a `Stage` names, while it is still up and friendly.
    fn stage_base(&self, row: usize, target: Handle) -> Option<usize> {
        let units = &self.state.units;
        units.row(target).filter(|&b| {
            self.bp(b).staging.is_some()
                && units.is_active(b)
                && units.health[b] > Fx::ZERO
                && !self.are_enemies(units.owner[row], units.owner[b])
        })
    }

    fn aboard(&self, base: usize) -> usize {
        let units = &self.state.units;
        let id = units.id(base);
        units
            .slots
            .iter()
            .filter(|&r| units.hangar[r] == id)
            .count()
    }

    fn pads_free(&self, base: usize) -> bool {
        self.bp(base)
            .staging
            .is_some_and(|s| self.aboard(base) < s.pads as usize)
    }

    /// Height an aircraft on a `Stage` glides at over `pos`: down to the deck over the
    /// last `GLIDE` metres to the landing point. `None` when it is not coming down.
    pub(crate) fn stage_glide_z(&self, row: usize, pos: FxVec2, cruise: Fx) -> Option<Fx> {
        let units = &self.state.units;
        let o = self.state.orders.front(units, row)?;
        if o.kind != OrderKind::Stage {
            return None;
        }
        let base = self.stage_base(row, o.target)?;
        let s = self.bp(base).staging?;
        if s.directional() && o.radius == Fx::ZERO {
            return None;
        }
        let d = pos.distance(self.ship_point(base, s.land.xy()));
        let glide = Fx::from_int(GLIDE);
        if d >= glide {
            return None;
        }
        let deck = units.z[base] + s.land.z;
        Some(deck + (cruise - deck).max(Fx::ZERO) * d / glide)
    }

    /// Stows the aircraft in `row` aboard `base`, keeping its queue for when it leaves.
    pub(crate) fn stow_aboard(&mut self, row: usize, base: usize) {
        let Some(land) = self.bp(base).staging.map(|s| s.land) else {
            return;
        };
        self.stop_moving(row);
        let at = self.ship_point(base, land.xy());
        let z = self.state.units.z[base] + land.z;
        let id = self.state.units.id(base);
        let units = &mut self.state.units;
        units.flags[row] |= flag::IN_FACTORY;
        units.flags[row] &= !(flag::MOVING | flag::AIR_RUN);
        units.hangar[row] = id;
        units.pos[row] = at;
        units.prev_pos[row] = at;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.speed[row] = Fx::ZERO;
        units.air_velocity[row] = FxVec3::ZERO;
        units.weapon_target[row] = [Handle::NONE; mc_data::MAX_WEAPONS];
    }

    /// Aboard: ride with the base, fill and mend. True once it is full, mended and the
    /// catapult's turn has come.
    fn service(&mut self, row: usize, base: usize, range: Fx) -> bool {
        let s = self.bp(base).staging.expect("staging base");
        let max = self.unit_max_health(row);
        let units = &mut self.state.units;
        let at = units.pos[base] + s.land.xy().rotate(units.heading[base]);
        let z = units.z[base] + s.land.z;
        units.pos[row] = at;
        units.prev_pos[row] = at;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.heading[row] = units.heading[base];
        units.fuel[row] = (units.fuel[row] + range / s.refuel_ticks as i32).min(range);
        units.health[row] = (units.health[row] + max * s.repair / TICKS_PER_SECOND as i32).min(max);
        units.fuel[row] >= range
            && units.health[row] >= max
            && self.state.tick.is_multiple_of(s.launch_ticks as u32)
    }

    /// Off the launch point at speed, nose along the base, back to what it was doing.
    fn launch(&mut self, row: usize, base: usize) {
        let Some(motion) = self.bp(row).motion.filter(|m| m.layer == MoveLayer::Air) else {
            return;
        };
        let s = self.bp(base).staging.expect("staging base");
        let turn = self.state.tick / s.launch_ticks as u32 % s.launches as u32;
        let (launch, off) = s.launch[turn as usize];
        let units = &mut self.state.units;
        let at = units.pos[base] + launch.xy().rotate(units.heading[base]);
        let heading = units.heading[base] + off;
        let z = units.z[base] + launch.z;
        units.flags[row] &= !flag::IN_FACTORY;
        units.hangar[row] = Handle::NONE;
        units.pos[row] = at;
        units.prev_pos[row] = at;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.heading[row] = heading;
        units.prev_heading[row] = heading;
        let speed = if motion.hover { Fx::ZERO } else { motion.speed };
        units.speed[row] = speed;
        let fwd = FxVec2::from_angle(heading) * speed;
        units.air_velocity[row] = FxVec3::new(fwd.x, fwd.y, Fx::ZERO);
        if self
            .state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| o.kind == OrderKind::Stage)
        {
            self.finish_order(row);
        }
    }
}
