//! Warp: capital ships with a drive (`UnitBlueprint::warp`) jump across the map, and warp
//! dampeners (`UnitBlueprint::warp_damper`) drag enemy jumps down.
//!
//! A jump (`Command::Warp`, `OrderKind::Warp`) runs through `Units::warp`:
//!
//! - **Spool**: the ship waits for its drive to recharge and to be up at cruise height,
//!   then stops where it is, charges the drive and brings its nose onto the mark. The
//!   charge is `Warp::energy`, drawn off the grid over `Warp::spool_ticks` with the rest of
//!   the side's upkeep (`economy.rs`), and slower by as much as the grid falls short. Its
//!   turrets keep firing. Any other order, or a stun, calls the jump off, and what was
//!   charged is lost.
//! - **Transit**: it drops out of the world at once (`IN_FACTORY`: not drawn, seen, hit or
//!   ordered) and is already where it will come out. The jump lasts as long as it is far
//!   at `Warp::speed`, and never less than `MIN_TRANSIT`.
//! - **Emerge**: back in the world, holding still while the drive winds down; then the
//!   drive recharges for `Warp::cooldown_ticks`. The order was done when it jumped, so
//!   the next one is taken up as it comes out.
//!
//! A jump that comes out within an enemy dampener's field, one that is finished and
//! powered, is snagged: the transit drags on `WarpDamper::drag` times longer, and if the
//! dampener still stands and has power when the ship comes out, the ship loses
//! `WarpDamper::damage` of its health and is stunned for `WarpDamper::stun_ticks`.
//! Destroying or starving the dampener before then saves the ship the blow.
//!
//! A stun (`Units::stun`) is an EMP: the unit does nothing (no orders, no fire, no way
//! on) until it runs out; a capital ship heels over, dips its nose and sinks a little
//! while its systems are down, and rights itself after.

use crate::mirror::SimEvent;
use crate::orders::order;
use crate::tables::{flag, Order, OrderKind, UnitId, WarpPhase, WarpState};
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2};

/// A jump never lasts less than this, however short: 1.5 s in warp.
const MIN_TRANSIT: u16 = 15;
/// Ticks a ship takes to come out of a clean jump, and out of a dampened one.
pub const EMERGE_TICKS: u16 = 12;
pub const EMERGE_DAMPED_TICKS: u16 = 30;
/// How near the mark its nose must be before it jumps: 3 degrees.
const ALIGNED: u16 = 546;
/// A mark nearer than this many hull radii is not worth a jump.
const MIN_JUMP_RADII: i32 = 2;
/// Share of its cruise height a ship must have before it spools.
const SPOOL_HEIGHT: Fx = Fx::ratio(3, 4);
/// A stunned capital ship heels over this far (binary angle steps: 14 degrees) and dips
/// its nose this far (4 degrees), a sixteenth of the way a tick.
const LIST_ROLL: i32 = 2549;
const LIST_PITCH: i32 = 728;
const LIST_EASE: i32 = 16;
/// It sinks toward this share of its cruise height, at most this many metres a tick.
const SAG_HEIGHT: Fx = Fx::ratio(17, 20);
const SAG_RATE: Fx = Fx::ratio(3, 2);
/// Its way comes off by this share a tick.
const DRIFT_KEEP: Fx = Fx::ratio(9, 10);

impl World {
    /// `Command::Warp`: ships among `ids` with a drive are given a jump to `pos`.
    pub(crate) fn order_warp(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        queue: bool,
    ) -> Result<(), SimError> {
        for row in self.owned(player, ids, 0) {
            if self.bp(row).warp.is_none() {
                continue;
            }
            let mut o = order(OrderKind::Warp, self.clamp_to_map(pos), Handle::NONE);
            o.heading = self.state.units.heading[row];
            self.give(row, o, queue)?;
        }
        Ok(())
    }

    /// `OrderKind::Warp` at the front of the queue, with the drive idle: waits for the
    /// drive and the height, then starts the spool (`run_warps` carries it on).
    pub(crate) fn run_warp_order(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(drive) = self.bp(row).warp else {
            self.finish_order(row);
            return Ok(());
        };
        if self.state.units.warp[row].phase != WarpPhase::Idle {
            return Ok(());
        }
        let from = self.state.units.pos[row];
        let to = self.clamp_to_map(from + (o.pos - from).clamp_length(drive.range));
        if from.distance(to) < self.bp(row).radius * MIN_JUMP_RADII {
            self.finish_order(row);
            return Ok(());
        }
        // Hover where it is (climbing back up if it was set down) until it may spool.
        self.stop_moving(row);
        if self.state.units.warp[row].recharge > 0 || !self.warp_height(row) {
            return Ok(());
        }
        let units = &mut self.state.units;
        units.warp[row] = WarpState {
            phase: WarpPhase::Spool,
            ticks: 0,
            length: drive.spool_ticks,
            from,
            to,
            damper: Handle::NONE,
            recharge: 0,
            charge: Fx::ZERO,
        };
        units.flags[row] |= flag::HOLD;
        self.events.push(SimEvent::WarpSpooling {
            unit: units.id(row),
            from: from.extend(units.z[row]),
            to,
            ticks: drive.spool_ticks,
            blueprint: units.blueprint[row],
            owner: units.owner[row],
        });
        Ok(())
    }

    /// Up at most of its cruise height: a ship spools only in the sky.
    fn warp_height(&self, row: usize) -> bool {
        let Some(m) = self.bp(row).motion else {
            return false;
        };
        let pos = self.state.units.pos[row];
        let surface = self.terrain.height_at(pos).max(self.terrain.water_level());
        self.state.units.z[row] >= surface + m.altitude * SPOOL_HEIGHT
    }

    /// Stunned by an EMP: does nothing until it wears off.
    #[inline]
    pub(crate) fn stunned(&self, row: usize) -> bool {
        self.state.units.stun[row][0] > 0
    }

    /// Held out of movement: stunned, or spooling or coming out of a jump (the jump
    /// holds it still and turns it). A ship in transit is out of the world altogether.
    #[inline]
    pub(crate) fn warp_held(&self, row: usize) -> bool {
        self.stunned(row)
            || matches!(
                self.state.units.warp[row].phase,
                WarpPhase::Spool | WarpPhase::Emerge
            )
    }

    /// A finished, powered dampener with its field up.
    pub(crate) fn damper_live(&self, row: usize) -> bool {
        self.bp(row).warp_damper.is_some()
            && self.state.units.is_active(row)
            && !self.powered_down(row)
            && !self.shields_unpowered(self.state.units.owner[row])
    }

    /// The first live dampener, by row, of an enemy of `owner` whose field covers `pos`.
    pub(crate) fn damper_at(&self, pos: FxVec2, owner: u8) -> Option<usize> {
        self.state.units.slots.iter().find(|&r| {
            self.bp(r)
                .warp_damper
                .is_some_and(|d| self.state.units.pos[r].distance_sq(pos) <= d.radius * d.radius)
                && self.are_enemies(owner, self.state.units.owner[r])
                && self.damper_live(r)
        })
    }

    /// Every tick, after orders: drives' recharge, jumps under way, and stuns.
    pub(crate) fn run_warps(&mut self) -> Result<(), SimError> {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        for row in rows {
            if !self.state.units.slots.is_alive(row) {
                continue;
            }
            if self.stunned(row) {
                self.step_stun(row);
            }
            let w = self.state.units.warp[row];
            match w.phase {
                WarpPhase::Idle => {
                    let units = &mut self.state.units;
                    units.warp[row].recharge = w.recharge.saturating_sub(1);
                }
                WarpPhase::Spool => self.step_spool(row, w),
                WarpPhase::Transit => self.step_transit(row, w),
                WarpPhase::Emerge => {
                    let units = &mut self.state.units;
                    units.warp[row].ticks += 1;
                    units.speed[row] = Fx::ZERO;
                    if units.warp[row].ticks >= w.length {
                        let cooldown = self.bp(row).warp.map_or(0, |d| d.cooldown_ticks);
                        self.state.units.warp[row] = WarpState {
                            recharge: cooldown,
                            ..WarpState::default()
                        };
                    }
                }
            }
        }
        Ok(())
    }

    /// Spooling: holds still and turns onto the mark while the economy charges the drive
    /// (`warp_draw`), then jumps. Called off when the jump is no longer the order in hand,
    /// or the ship is stunned.
    fn step_spool(&mut self, row: usize, w: WarpState) {
        let ordered = self
            .state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| o.kind == OrderKind::Warp);
        let (Some(drive), Some(motion)) = (self.bp(row).warp, self.bp(row).motion) else {
            return;
        };
        if !ordered || self.stunned(row) || !self.state.units.is_active(row) {
            self.state.units.warp[row].phase = WarpPhase::Idle;
            return;
        }
        let units = &mut self.state.units;
        let bearing = (w.to - units.pos[row]).angle();
        units.heading[row] = units.heading[row].turn_toward(bearing, motion.turn_rate);
        units.speed[row] = Fx::ZERO;
        units.flags[row] |= flag::HOLD;
        units.warp[row].ticks = w.ticks.saturating_add(1);
        let aligned = units.heading[row].delta_to(bearing).unsigned_abs() <= ALIGNED;
        if w.charge < drive.energy || !aligned {
            return;
        }
        // Into warp: out of the world at once, already where it comes out.
        let transit = ((w.from.distance(w.to) / drive.speed).ceil_int().max(0) as u32)
            .clamp(MIN_TRANSIT as u32, u16::MAX as u32) as u16;
        let owner = units.owner[row];
        let damper = self.damper_at(w.to, owner);
        let (length, damper) = match damper {
            Some(d) => (self.dragged(transit, 0, d), self.state.units.id(d)),
            None => (transit, Handle::NONE),
        };
        let surface = self.terrain.height_at(w.to).max(self.terrain.water_level());
        let z = surface + motion.altitude;
        let units = &mut self.state.units;
        units.flags[row] |= flag::IN_FACTORY;
        // `prev_pos` and `prev_z` stay where it left: the mirror draws it there streaking out.
        units.pos[row] = w.to;
        units.move_goal[row] = w.to;
        units.z[row] = z;
        units.speed[row] = Fx::ZERO;
        units.air_velocity[row] = mc_core::FxVec3::ZERO;
        units.warp[row] = WarpState {
            phase: WarpPhase::Transit,
            ticks: 0,
            length,
            damper,
            ..w
        };
        // The order is done: whatever comes next waits for it to come out (`warp_held`).
        self.finish_order(row);
        let units = &self.state.units;
        self.events.push(SimEvent::WarpJumped {
            unit: units.id(row),
            from: w.from.extend(units.prev_z[row]),
            to: w.to.extend(z),
            ticks: length,
            dampened: damper != Handle::NONE,
            blueprint: units.blueprint[row],
            owner: units.owner[row],
        });
    }

    /// The drive in `row` charging: the energy a tick it draws at full power, and what
    /// is left to charge. `None` when it is not charging. A stall slows the draw, never
    /// the little that is left, or that remainder would shrink for ever (`economy.rs`).
    pub(crate) fn warp_draw(&self, row: usize) -> Option<(Fx, Fx)> {
        let w = &self.state.units.warp[row];
        let d = self.bp(row).warp?;
        let left = d.energy - w.charge;
        (w.phase == WarpPhase::Spool && self.state.units.is_active(row) && left > Fx::ZERO)
            .then(|| (d.energy / d.spool_ticks.max(1) as i32, left))
    }

    /// A transit of `length` ticks, `done` of them gone, snagged by the dampener in `row`:
    /// what is left drags on its `drag` times longer.
    fn dragged(&self, length: u16, done: u16, row: usize) -> u16 {
        let drag = self.bp(row).warp_damper.map_or(Fx::ONE, |d| d.drag);
        let left = Fx::from_int(length.saturating_sub(done) as i32) * drag;
        (done as u32 + left.ceil_int().max(0) as u32).min(u16::MAX as u32) as u16
    }

    /// In warp: a field raised over where it comes out snags it part way; at the end it
    /// comes out, hurt and stunned if the dampener that snagged it still stands.
    fn step_transit(&mut self, row: usize, w: WarpState) {
        let owner = self.state.units.owner[row];
        let mut w = WarpState {
            ticks: w.ticks + 1,
            ..w
        };
        // Looked at twice a second: a dampener finished while the ship is in warp.
        if w.damper == Handle::NONE && w.ticks.is_multiple_of(5) {
            if let Some(d) = self.damper_at(w.to, owner) {
                w.length = self.dragged(w.length, w.ticks, d);
                w.damper = self.state.units.id(d);
                self.events.push(SimEvent::WarpSnagged {
                    unit: self.state.units.id(row),
                    at: w.to,
                });
            }
        }
        if w.ticks < w.length {
            self.state.units.warp[row] = w;
            return;
        }
        let damper = self
            .state
            .units
            .row(w.damper)
            .filter(|&d| self.damper_live(d));
        let units = &mut self.state.units;
        units.flags[row] &= !flag::IN_FACTORY;
        units.warp[row] = WarpState {
            phase: WarpPhase::Emerge,
            ticks: 0,
            length: if damper.is_some() {
                EMERGE_DAMPED_TICKS
            } else {
                EMERGE_TICKS
            },
            ..w
        };
        self.events.push(SimEvent::WarpArrived {
            unit: units.id(row),
            at: w.to.extend(units.z[row]),
            dampened: damper.is_some(),
            blueprint: units.blueprint[row],
            owner,
        });
        let Some(d) = damper else {
            return;
        };
        let spec = self.bp(d).warp_damper.expect("a dampener");
        let by = self.state.units.owner[d];
        self.stun(row, spec.stun_ticks);
        let blow = self.unit_max_health(row) * spec.damage;
        self.damage_unit(row, blow, by, w.damper);
    }

    /// Stuns `row` for `ticks` (EMP), or for longer if it already is.
    pub(crate) fn stun(&mut self, row: usize, ticks: u16) {
        let stun = &mut self.state.units.stun[row];
        if ticks > stun[0] {
            *stun = [ticks, ticks];
        }
        self.stop_moving(row);
        self.events.push(SimEvent::Stunned {
            unit: self.state.units.id(row),
            ticks,
        });
    }

    /// One stunned tick: no way on; a capital ship heels over, dips its nose and sinks.
    fn step_stun(&mut self, row: usize) {
        let capital = self.bp(row).is_capital_ship();
        let altitude = self.bp(row).motion.map(|m| m.altitude);
        let side = if row.is_multiple_of(2) { 1 } else { -1 };
        let units = &mut self.state.units;
        units.stun[row][0] -= 1;
        units.speed[row] = Fx::ZERO;
        units.move_goal[row] = units.pos[row];
        if units.has_flag(row, flag::IN_FACTORY) {
            return;
        }
        // What way it had drifts off.
        let drift = units.air_velocity[row].xy() * DRIFT_KEEP;
        units.air_velocity[row] = drift.extend(Fx::ZERO);
        let at = units.pos[row] + drift;
        if !capital {
            return;
        }
        let bank = units.bank[row] as i32;
        units.bank[row] = (bank + (side * LIST_ROLL - bank) / LIST_EASE) as i16;
        let pitch = Angle::ZERO.delta_to(units.arm_pitch[row][0]) as i32;
        units.arm_pitch[row][0] = Angle((pitch + (-LIST_PITCH - pitch) / LIST_EASE) as u16);
        let pos = self.clamp_to_map(at);
        let surface = self.terrain.height_at(pos).max(self.terrain.water_level());
        let units = &mut self.state.units;
        if units.pos[row] != pos {
            units.pos[row] = pos;
            units.flags[row] |= flag::MOVING;
        }
        if let Some(altitude) = altitude {
            let floor = surface + altitude * SAG_HEIGHT;
            let z = units.z[row];
            if z > floor {
                units.z[row] = z - ((z - floor) / 40).clamp(Fx::ZERO, SAG_RATE);
            }
        }
    }
}
