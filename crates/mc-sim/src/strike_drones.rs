//! Drones that are shells (`Weapon::launches`, the Regency Quiver's Wicks): the carrier's
//! launcher lets one go from its socket at the mark, the drone flies at it, diving onto it
//! over its last couple of hundred metres, and bursts there with the launcher's charge. The
//! burst is a shot of that weapon fired from the drone over its last metres (a tick's flight
//! at most), so its blast, splash, shields and impact are every other shot's; the drone is
//! spent with it, with no wreck and no death. A drone whose mark dies first takes the
//! nearest one it may strike near it, or flies home to its socket for the next launch.
//!
//! The sockets are the magazine: a launch waits for a drone sitting on one, and the
//! carrier builds the spent ones again where they sat (`air_support.rs`, `drone_jobs`). The
//! docked drones ride the hull where the player can see them, so an empty rack shows why a
//! carrier has stopped launching.
//!
//! A striking drone is `FLYING` with `AIR_RUN` set: its mark is `weapon_target[0]` (a unit;
//! the drone has no weapons of its own, so targeting never writes it) or, with no unit,
//! the point `air_aim` on the ground.
use crate::air_support::dock;
use crate::tables::flag;
use crate::{Handle, SimError, World};
use mc_core::{Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::Weapon;

/// How far past its mark a striking drone is steered, so it closes at speed instead of
/// braking into a hover on it.
const OVERSHOOT: Fx = Fx::from_int(80);
/// How near (beyond a tick's flight) the drone comes before it bursts.
const BURST_REACH: Fx = Fx::from_int(6);
/// How far round its lost mark a drone looks for another.
const RETARGET: Fx = Fx::from_int(160);
/// The dive: the drone comes down to its mark's middle along this slope, starting where
/// the slope meets its cruise height.
const DIVE_SLOPE: Fx = Fx::ratio(1, 3);
/// Ticks the burst flies at most.
const BURST_TICKS: u16 = 3;

/// A striking drone's mark: where it is, how high its middle stands, how wide it is, and
/// the unit (if it is one).
#[derive(Clone, Copy)]
pub(crate) struct StrikeMark {
    pub(crate) unit: Option<usize>,
    pub(crate) pos: FxVec2,
    pub(crate) middle: Fx,
    pub(crate) radius: Fx,
}

impl World {
    /// Weapon `w` of `row` launches its drones (`Weapon::launches`) in place of shots:
    /// on a mark in reach, with its countdown run out, it lets the first drone on its
    /// socket go at it. With none home it waits, salvo and countdown kept, until one is
    /// built or comes back.
    pub(crate) fn step_launcher(
        &mut self,
        row: usize,
        w: usize,
        mark: Option<(Option<usize>, FxVec2, Fx)>,
    ) -> Result<(), SimError> {
        let weapon = &self.bp(row).weapons[w];
        let (reach, salvo, delay, reload) = (
            weapon.range_max,
            weapon.salvo,
            weapon.salvo_delay_ticks,
            weapon.reload_ticks,
        );
        let units = &mut self.state.units;
        if units.weapon_cooldown[row][w] > 0 {
            units.weapon_cooldown[row][w] -= 1;
            return Ok(());
        }
        let Some((unit, at, radius)) = mark else {
            return Ok(());
        };
        if units.pos[row].distance(at) - radius > reach {
            return Ok(());
        }
        let Some(drone) = self.drone_home(row) else {
            return Ok(());
        };
        let units = &mut self.state.units;
        if units.weapon_salvo_left[row][w] == 0 {
            units.weapon_salvo_left[row][w] = salvo;
        }
        units.weapon_salvo_left[row][w] -= 1;
        units.weapon_cooldown[row][w] = if units.weapon_salvo_left[row][w] > 0 {
            delay.max(1) as u16
        } else {
            reload
        };
        units.weapon_target[drone][0] = unit.map_or(Handle::NONE, |t| units.id(t));
        units.air_aim[drone] = at;
        units.flags[drone] |= flag::AIR_RUN;
        units.deploy[drone] = dock::RELEASING;
        Ok(())
    }

    /// The first finished drone sitting on its socket, in socket order.
    fn drone_home(&self, carrier: usize) -> Option<usize> {
        let units = &self.state.units;
        let parent = units.id(carrier);
        units
            .slots
            .iter()
            .filter(|&r| {
                units.drone_parent[r] == parent
                    && units.is_active(r)
                    && units.deploy[r] == dock::DOCKED
            })
            .min_by_key(|&r| units.drone_socket[r])
    }

    /// Whether `row` is a drone flying at a mark (`StrikeMark`).
    pub(crate) fn drone_striking(&self, row: usize) -> bool {
        let units = &self.state.units;
        units.drone_parent[row] != Handle::NONE
            && units.deploy[row] == dock::FLYING
            && units.has_flag(row, flag::AIR_RUN)
    }

    /// What striking drone `row` flies at, or `None` when its unit is gone or no longer
    /// seen. A strike on the ground keeps its point.
    pub(crate) fn strike_mark(&self, row: usize) -> Option<StrikeMark> {
        let units = &self.state.units;
        let id = units.weapon_target[row][0];
        if id == Handle::NONE {
            let at = units.air_aim[row];
            return Some(StrikeMark {
                unit: None,
                pos: at,
                middle: self.terrain.height_at(at).max(self.terrain.water_level()),
                radius: Fx::ZERO,
            });
        }
        let t = units.row(id)?;
        let bp = self.bp(t);
        (units.health[t] > Fx::ZERO && self.detects(units.owner[row], t)).then(|| StrikeMark {
            unit: Some(t),
            pos: units.pos[t],
            middle: units.z[t] + bp.height / 2,
            radius: bp.radius,
        })
    }

    /// Flies the drones of `carrier`, a launcher (`Weapon::launches`): each striking one
    /// at its mark, bursting it when it is there; each with nothing to strike home to its
    /// socket. Docked and gliding drones are `seat_drones`' to fly.
    pub(crate) fn fly_strike_drones(
        &mut self,
        carrier: usize,
        drones: Vec<usize>,
    ) -> Result<(), SimError> {
        let Some(w) = self.bp(carrier).weapons.iter().position(|w| w.launches) else {
            return Ok(());
        };
        for row in drones {
            if !self.state.units.is_active(row) || self.state.units.deploy[row] != dock::FLYING {
                continue;
            }
            if self.state.units.has_flag(row, flag::AIR_RUN) {
                let mark = self
                    .strike_mark(row)
                    .or_else(|| self.next_strike_mark(carrier, w, row));
                if let Some(mark) = mark {
                    self.fly_strike(carrier, w, row, mark)?;
                    continue;
                }
                let units = &mut self.state.units;
                units.flags[row] &= !flag::AIR_RUN;
                units.weapon_target[row][0] = Handle::NONE;
            }
            self.fly_drone_home(carrier, row)?;
        }
        Ok(())
    }

    /// A new mark for a drone whose own is gone: the nearest unit its launcher may strike
    /// within `RETARGET` of the drone, which it takes as its own.
    fn next_strike_mark(&mut self, carrier: usize, w: usize, row: usize) -> Option<StrikeMark> {
        let units = &self.state.units;
        let weapon: &Weapon = &self.bp(carrier).weapons[w];
        let owner = units.owner[row];
        let next = self
            .index
            .nearest_foe(
                units.pos[row],
                RETARGET,
                crate::combat::target_kinds(weapon.target_mask),
                self.team_mask(owner),
                |e| {
                    let t = e.row as usize;
                    self.unit_entry_is_current(e)
                        && units.health[t] > Fx::ZERO
                        && !units.has_flag(t, flag::IN_FACTORY)
                        && self.are_enemies(owner, units.owner[t])
                        && self.weapon_reaches(t, weapon)
                        && self.detects(owner, t)
                },
            )
            .map(|e| e.row as usize)?;
        let id = self.state.units.id(next);
        self.state.units.weapon_target[row][0] = id;
        self.strike_mark(row)
    }

    /// Steers striking drone `row` through its mark, and bursts it once the mark is within
    /// a tick's flight.
    fn fly_strike(
        &mut self,
        carrier: usize,
        w: usize,
        row: usize,
        mark: StrikeMark,
    ) -> Result<(), SimError> {
        let units = &self.state.units;
        let here = units.pos[row].extend(units.z[row] + self.bp(row).height / 2);
        let aim = mark.pos.extend(mark.middle);
        let step = units.air_velocity[row].length();
        if here.distance(aim) - mark.radius <= step + BURST_REACH {
            return self.burst(carrier, w, row, here, aim, mark.unit);
        }
        let ahead = (mark.pos - units.pos[row]).normalize();
        let goal = self.clamp_to_map(mark.pos + ahead * OVERSHOOT);
        // A new goal stops the old flight first, which ends the run: it is still on one.
        self.ensure_moving(row, goal, goal)?;
        let units = &mut self.state.units;
        units.air_aim[row] = mark.pos;
        units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// The drone goes off: a shot of the carrier's launcher from where the drone is,
    /// straight into the mark within a tick, and the drone is spent.
    fn burst(
        &mut self,
        carrier: usize,
        w: usize,
        row: usize,
        from: FxVec3,
        aim: FxVec3,
        target: Option<usize>,
    ) -> Result<(), SimError> {
        let units = &self.state.units;
        let gap = aim - from;
        let reach = gap.length();
        let speed = self.bp(carrier).weapons[w].projectile_speed / TICKS_PER_SECOND as i32;
        // Through the mark within a tick, however short the dash.
        let vel = if reach > Fx::ONE {
            gap * ((reach + Fx::from_int(2)).max(speed) / reach)
        } else {
            FxVec3::new(Fx::ZERO, Fx::ZERO, -speed.max(Fx::ONE))
        };
        let (owner, source, blueprint) = (
            units.owner[carrier],
            units.id(carrier),
            units.blueprint[carrier],
        );
        let target = target.map_or(Handle::NONE, |t| units.id(t));
        self.state
            .projectiles
            .spawn(from, vel, owner, source, blueprint, w as u8, BURST_TICKS)?;
        let shot = self.state.projectiles.len() - 1;
        self.state.projectiles.target[shot] = target;
        self.state.projectiles.mark[shot] = aim;
        self.remove_unit_row(row, false)
    }

    /// A drone with nothing to strike flies back to its socket and lines up to settle on it.
    fn fly_drone_home(&mut self, carrier: usize, row: usize) -> Result<(), SimError> {
        let slot = self.state.units.drone_socket[row] as usize;
        let dock_xy = self.drone_socket(carrier, slot).xy;
        if self.state.units.pos[row].distance(dock_xy) <= dock::DOCK_CAPTURE {
            self.clear_orders(row)?;
            self.state.units.deploy[row] = dock::DOCKING;
            return Ok(());
        }
        self.ensure_moving(row, dock_xy, dock_xy)
    }

    /// The height a striking drone wants this tick (`movement.rs`): its cruise until the
    /// dive, then down the dive's slope onto its mark's middle.
    pub(crate) fn strike_height(&self, row: usize, at: FxVec2, cruise: Fx) -> Fx {
        let Some(mark) = self.strike_mark(row) else {
            return cruise;
        };
        let out = (at.distance(mark.pos) - mark.radius).max(Fx::ZERO);
        let floor = self.terrain.height_at(at).max(self.terrain.water_level()) + Fx::ONE;
        (mark.middle + out * DIVE_SLOPE).min(cruise).max(floor)
    }
}
