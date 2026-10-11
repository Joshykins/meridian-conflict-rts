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
/// How far astern of a directional deck's landing point its final approach starts: the
/// gate.
const APPROACH: i32 = 900;
/// Height over the deck an aircraft comes through the gate at, let down to from cruise
/// over the last `DESCENT` metres to it.
const PATTERN: i32 = 60;
const DESCENT: i32 = 1500;
/// Its circuit round a directional deck: downwind this far abeam of the ship, to a
/// corner `BASE_TURN` aft of the gate, so it never crosses over the ship and turns in on
/// the gate rather than reversing on it.
const ABEAM: i32 = 450;
const BASE_TURN: i32 = 300;
/// Through the gate within this much of it, along and across: on final.
const ON_APPROACH: i32 = 140;
/// The leg of a directional deck's circuit an aircraft on a `Stage` is flying, kept in
/// the order's `radius`: downwind to the corner, the base leg to the gate, final.
const DOWNWIND: Fx = Fx::ZERO;
const BASE_LEG: Fx = Fx::ONE;
const FINAL: Fx = Fx::from_int(2);
/// The last metres to a directional deck's landing point are flown level, `SKIM` over
/// the deck: in through the stern mouth.
const FLARE: i32 = 40;
const SKIM: Fx = Fx::ONE;
/// On final, the speed it still has over the deck's at the landing point, metres a
/// second.
const CLOSING: i32 = 12;
/// On final, past the landing point by this much (not taken aboard), it goes round again.
const OVERSHOOT: i32 = 15;
/// Close enough over a pad (not a directional deck) to come down on it.
const TOUCHDOWN: i32 = 60;
/// Low enough over the deck to be taken aboard.
const TOUCHDOWN_HEIGHT: i32 = 4;
/// On a pad, the glide down from cruise starts this far out.
const GLIDE: i32 = 900;
/// Down its glide an aircraft slows with the share of it left to fly, to this share of
/// its top speed.
const LANDING_PACE: Fx = Fx::ratio(2, 5);
/// Looked at twice a second, spread over the rows.
const LOOK_EVERY: usize = 16;

impl World {
    /// `motion` as this aircraft flies it now: slower and wider once its tank is dry, and
    /// slowing to land on its glide down to a staging base.
    pub(crate) fn flown_motion(&self, row: usize, motion: &Motion) -> Motion {
        let mut m = *motion;
        if self.bp(row).fuel.is_some() && self.state.units.fuel[row] <= Fx::ZERO {
            m.speed = m.speed * DRY.0 / DRY.1;
            m.accel = m.accel * DRY.0 / DRY.1;
            m.turn_rate = (m.turn_rate as i32 * DRY.0 / DRY.1).max(1) as u16;
        }
        if let Some((left, base)) = self.glide_left(row).filter(|_| !m.hover) {
            // Never slower than the deck it is catching.
            let deck = self.state.units.speed[base] + Fx::from_int(CLOSING);
            m.speed = (m.speed * left.max(LANDING_PACE)).max(deck.min(m.speed));
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

    /// `OrderKind::Stage`: fly to the base and come down on it once a pad is free. A
    /// directional deck is flown into from astern round a circuit: downwind beside the
    /// ship, the base leg in to the gate at pattern height, then final, down the glide and
    /// in level; past the landing point, or the gate with no pad free, round again.
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
        let head = self.state.units.order_head[row] as usize;
        let ready = self.pads_free(base) && self.state.units.deploy[base] >= s.door_ticks;
        let low = self.state.units.z[row]
            <= self.state.units.z[base] + s.land.z + Fx::from_int(TOUCHDOWN_HEIGHT);
        if !s.directional() {
            if pos.distance(land) <= Fx::from_int(TOUCHDOWN) && ready && low {
                self.stow_aboard(row, base);
                return Ok(());
            }
            return self.ensure_moving(row, land, land);
        }
        let local = (pos - self.state.units.pos[base]).rotate(-self.state.units.heading[base]);
        let gate = FxVec2::new(s.land.x - Fx::from_int(APPROACH), Fx::ZERO);
        let near = Fx::from_int(ON_APPROACH);
        let leg = match o.radius {
            FINAL if local.x > s.land.x + Fx::from_int(OVERSHOOT) || local.y.abs() > near * 3 => {
                DOWNWIND
            }
            FINAL => {
                // Taken aboard where this tick's flight carries it over the landing point.
                let step = self.state.units.speed[row] / TICKS_PER_SECOND as i32 + Fx::from_int(2);
                if pos.distance(land) <= step && ready && low {
                    self.stow_aboard(row, base);
                    return Ok(());
                }
                // Braking for a point a little up the deck, it closes on a ship under way.
                let lead = self.state.units.speed[base] / 2 + Fx::from_int(CLOSING / 2);
                let aim = land + FxVec2::from_angle(self.state.units.heading[base]) * lead;
                return self.ensure_moving(row, aim, aim);
            }
            // Past the gate without a free pad: round again, a holding racetrack.
            BASE_LEG if local.x > gate.x + near => DOWNWIND,
            BASE_LEG
                if local.x > gate.x - near && local.y.abs() < near * 2 && self.pads_free(base) =>
            {
                FINAL
            }
            BASE_LEG => BASE_LEG,
            _ if local.x < gate.x - near => BASE_LEG,
            _ => DOWNWIND,
        };
        self.state.orders.order[head].radius = leg;
        let to = match leg {
            FINAL => land,
            BASE_LEG => self.ship_point(base, gate),
            _ => {
                let side = if local.y < Fx::ZERO { -ABEAM } else { ABEAM };
                let corner = FxVec2::new(gate.x - Fx::from_int(BASE_TURN), Fx::from_int(side));
                self.ship_point(base, corner)
            }
        };
        self.ensure_moving(row, to, to)
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

    /// Height an aircraft on a `Stage` flies at over `pos` near its base, `None` when it
    /// keeps to cruise. Onto a pad: down from cruise over the last `GLIDE` metres. Onto a
    /// directional deck: down to `PATTERN` on the way to the gate, then the glide from
    /// there to the deck, level over the last `FLARE` metres.
    pub(crate) fn stage_glide_z(&self, row: usize, pos: FxVec2, cruise: Fx) -> Option<Fx> {
        let (base, s, final_leg) = self.staging_to(row)?;
        let land = self.ship_point(base, s.land.xy());
        let deck = self.state.units.z[base] + s.land.z;
        let d = pos.distance(land);
        let top = (cruise - deck).max(Fx::ZERO);
        if !s.directional() {
            let glide = Fx::from_int(GLIDE);
            return (d < glide).then(|| deck + top * d / glide);
        }
        let pattern = Fx::from_int(PATTERN);
        if final_leg {
            let along = (d - Fx::from_int(FLARE)).max(Fx::ZERO);
            let slope = (pattern - SKIM) / Fx::from_int(APPROACH - FLARE);
            return Some(deck + SKIM + (along * slope).min(top));
        }
        let gate = self.ship_point(
            base,
            FxVec2::new(s.land.x - Fx::from_int(APPROACH), Fx::ZERO),
        );
        let descent = Fx::from_int(DESCENT);
        let dg = pos.distance(gate);
        (dg < descent && top > pattern).then(|| deck + pattern + (top - pattern) * dg / descent)
    }

    /// The base an aircraft on a `Stage` is going to, its staging, and whether it is on
    /// final (a directional deck's glide, past the gate).
    fn staging_to(&self, row: usize) -> Option<(usize, mc_data::Staging, bool)> {
        let o = self.state.orders.front(&self.state.units, row)?;
        if o.kind != OrderKind::Stage || self.state.units.hangar[row] != Handle::NONE {
            return None;
        }
        let base = self.stage_base(row, o.target)?;
        Some((base, self.bp(base).staging?, o.radius == FINAL))
    }

    /// On a directional deck's circuit, short of final: it flies through its corners
    /// rather than braking for them.
    pub(crate) fn on_circuit(&self, row: usize) -> bool {
        self.staging_to(row)
            .is_some_and(|(_, s, final_leg)| s.directional() && !final_leg)
    }

    /// How far along its glide to a staging base an aircraft has still to go, as a share
    /// of the glide, and the base; `None` off it.
    pub(crate) fn glide_left(&self, row: usize) -> Option<(Fx, usize)> {
        let (base, s, final_leg) = self.staging_to(row)?;
        let glide = Fx::from_int(if s.directional() { APPROACH } else { GLIDE });
        if s.directional() && !final_leg {
            return None;
        }
        let d = self.state.units.pos[row].distance(self.ship_point(base, s.land.xy()));
        (d < glide).then(|| (d / glide, base))
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
