//! Carrier-owned salvage drones, paid production and incendiary damage.
use crate::spatial::kind;
use crate::tables::{flag, OrderKind};
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};

impl World {
    pub(crate) fn run_air_support(&mut self) -> Result<(), SimError> {
        self.tick_fires();
        let rows: Vec<_> = self.state.units.slots.iter().collect();
        for row in rows {
            if self.state.units.health[row] <= Fx::ZERO || !self.state.units.is_active(row) {
                continue;
            }
            let Some(drone) = self.bp(row).drone else {
                continue;
            };
            let parent = self.state.units.id(row);
            let flock: Vec<usize> = self
                .state
                .units
                .slots
                .iter()
                .filter(|&r| self.state.units.drone_parent[r] == parent)
                .collect();
            // One drone at a time goes up, on the first socket without one.
            if flock
                .iter()
                .any(|&r| self.state.units.has_flag(r, flag::UNDER_CONSTRUCTION))
            {
                continue;
            }
            let sockets = self.bp(row).drone_sockets.len();
            let Some(slot) = (0..sockets).find(|&k| {
                !flock
                    .iter()
                    .any(|&r| self.state.units.drone_socket[r] as usize == k)
            }) else {
                continue;
            };
            // Deliberate: no drone is started while the unit table is full, so drones
            // never take the last rows from real production.
            if self.state.units.slots.live() >= crate::tables::MAX_UNITS {
                continue;
            }
            // It is built on its socket (paid for by the economy, `drone_jobs`) and let go
            // from there like any docked drone once it is done.
            let socket = self.drone_socket(row, slot);
            let owner = self.state.units.owner[row];
            let child = self.spawn_unit(drone, owner, socket.xy, socket.heading, false)?;
            let units = &mut self.state.units;
            units.drone_parent[child] = parent;
            units.drone_socket[child] = slot as u8;
            units.deploy[child] = dock::DOCKED;
            units.z[child] = socket.z;
            units.prev_z[child] = socket.z;
        }
        // A drone whose carrier is gone goes with it.
        let orphans: Vec<_> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| {
                self.state.units.drone_parent[r] != Handle::NONE && self.drone_carrier(r).is_none()
            })
            .collect();
        for row in orphans {
            self.state.units.flags[row] |= flag::RECLAIMED;
            self.state.units.health[row] = Fx::ZERO;
        }
        let mut claimed = std::collections::BTreeSet::new();
        for (parent, children) in self.drone_flocks() {
            let carrier = self.bp(parent).drone_carrier();
            // A carrier's drones work the wreck it is told to; a port's only salvage round it.
            let task = self
                .state
                .orders
                .front(&self.state.units, parent)
                .filter(|o| {
                    carrier && matches!(o.kind, OrderKind::Reclaim | OrderKind::ReclaimUnit)
                })
                .map(|o| (o.kind, o.target));
            let recalling = if carrier {
                task.is_none() && !self.carrier_has_work(parent)
            } else {
                !self.carrier_has_work(parent)
            };
            if carrier && !recalling {
                // Its drones are its tools: while they have work it is busy, not idle
                // (the HUD's idle Reclaimers card).
                self.state.units.flags[parent] |= flag::WORKING;
            }
            let need = self.bp(parent).motion.map(|m| m.deploy_ticks).unwrap_or(0);
            // A carrier lets its drones go once it has settled into a hover; a port
            // has nothing to wait for.
            let open = need == 0 || self.state.units.deploy[parent] >= need;
            let center = self.state.units.pos[parent];
            let reach = self.bp(parent).drone_radius;
            let owner = self.state.units.owner[parent];
            let full = self.no_room_for_salvage(owner);
            for row in children {
                if !self.state.units.is_active(row) {
                    // Still going up on its socket (`seat_drones` holds it there).
                    continue;
                }
                let slot = self.state.units.drone_socket[row] as usize;
                let dock = self.drone_socket(parent, slot);
                let pos = self.state.units.pos[row];
                let state = self.state.units.deploy[row];
                // A drone left outside its carrier's reach (the carrier flew on while it
                // worked) is called home; back inside, it picks up work again. A drone
                // sent to an ordered wreck goes however far that is.
                let astray = task.is_none()
                    && state == dock::FLYING
                    && pos.distance(center) > reach + self.work_range(row);
                if recalling || !open || astray {
                    match state {
                        dock::FLYING if pos.distance(dock.xy) <= DOCK_CAPTURE => {
                            // Close enough to line up under (or over) its socket: the
                            // glide in is flown after movement (`seat_drones`).
                            self.clear_orders(row)?;
                            self.state.units.deploy[row] = dock::DOCKING;
                        }
                        dock::FLYING => self.ensure_moving(row, dock.xy, dock.xy)?,
                        dock::RELEASING => self.state.units.deploy[row] = dock::DOCKING,
                        _ => {}
                    }
                    self.state.units.flags[row] &= !flag::AIR_RUN;
                    continue;
                }
                if let Some((kind, target)) = task {
                    if !self.carrier_target_live(row, kind, target) {
                        self.finish_order(parent);
                        continue;
                    }
                }
                if state != dock::FLYING {
                    // Work to do: let go of the socket and drop clear before flying.
                    self.state.units.deploy[row] = dock::RELEASING;
                    continue;
                }
                if let Some((kind, target)) = task {
                    self.drone_reclaim_ordered(row, kind, target)?;
                    continue;
                }
                let target = self
                    .state
                    .wrecks
                    .slots
                    .iter()
                    .filter(|&w| !full && self.drones_may_take(parent, w))
                    .min_by_key(|&w| {
                        (
                            claimed.contains(&w),
                            self.state.wrecks.pos[w].distance_sq(pos),
                        )
                    });
                if let Some(w) = target {
                    claimed.insert(w);
                    let wreck_at = self.state.wrecks.pos[w];
                    let radial = pos - wreck_at;
                    let radius = self.work_range(row) * Fx::ratio(3, 5);
                    let bearing = if radial.length() > Fx::ONE {
                        radial.angle()
                    } else {
                        self.state.units.heading[row]
                    };
                    let goal = self.clamp_to_map(
                        wreck_at + FxVec2::from_angle(bearing + Angle::from_degrees(45)) * radius,
                    );
                    self.ensure_moving(row, goal, goal)?;
                    self.state.units.air_aim[row] = wreck_at;
                    self.state.units.flags[row] |= flag::AIR_RUN;
                    if pos.distance(wreck_at) <= self.work_range(row) {
                        let power = self.tool_power(row);
                        self.drain_wreck(row, w, power, 0);
                        self.state.units.flags[row] |= flag::RECLAIMING;
                    }
                } else {
                    self.ensure_moving(row, dock.xy, dock.xy)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn carrier_reclaim_ordered(&self, row: usize) -> bool {
        self.state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| matches!(o.kind, OrderKind::Reclaim | OrderKind::ReclaimUnit))
    }

    pub(crate) fn carrier_drones_home(&self, row: usize) -> bool {
        let parent = self.state.units.id(row);
        let center = self.state.units.pos[row];
        self.state
            .units
            .slots
            .iter()
            .filter(|&r| self.state.units.drone_parent[r] == parent)
            .all(|r| {
                self.state.units.deploy[r] == 0
                    && self.state.units.pos[r].distance(center) <= Fx::from_int(14)
            })
    }

    pub(crate) fn carrier_has_work(&self, row: usize) -> bool {
        !self.no_room_for_salvage(self.state.units.owner[row])
            && self
                .state
                .wrecks
                .slots
                .iter()
                .any(|w| self.drones_may_take(row, w))
    }

    /// Whether the drones of carrier `row` may go for wreck `w` unordered: one it is
    /// after (`carrier_wants`) within the drones' reach of the carrier.
    fn drones_may_take(&self, row: usize, w: usize) -> bool {
        self.state.wrecks.pos[w].distance(self.state.units.pos[row]) <= self.bp(row).drone_radius
            && self.carrier_wants(row, w)
    }

    /// Whether carrier `row` is after wreck `w` at all: one with mass left that its side
    /// has seen and, while it is clearing a circle (`ReclaimArea`), inside that circle.
    pub(crate) fn carrier_wants(&self, row: usize, w: usize) -> bool {
        let units = &self.state.units;
        let wrecks = &self.state.wrecks;
        let at = wrecks.pos[w];
        let circle = self
            .state
            .orders
            .front(units, row)
            .filter(|o| o.kind == OrderKind::ReclaimArea && o.radius > Fx::ZERO);
        circle.is_none_or(|o| at.distance(o.pos) <= o.radius)
            && wrecks.mass[w] > Fx::ZERO
            && (!self.state.fog_enabled
                || self.fog.is_detected(at, self.team_mask(units.owner[row])))
    }

    /// Where drone `slot` of `parent` sits when home (`drone_sockets`): on a pylon, a
    /// pad or a clamp. On a unit with a torso the sockets turn with it.
    fn drone_socket(&self, parent: usize, slot: usize) -> Socket {
        let bp = self.bp(parent);
        let units = &self.state.units;
        let at = bp.drone_sockets[slot % bp.drone_sockets.len()];
        let heading = units.heading[parent];
        let facing = if bp.weapons.is_empty() {
            heading
        } else {
            heading + units.weapon_yaw[parent][0]
        };
        Socket {
            xy: units.pos[parent] + bp.turret_point(at.xy(), heading, facing),
            z: units.z[parent] + at.z,
            heading: facing,
        }
    }

    /// The live carrier (or port) a drone belongs to.
    fn drone_carrier(&self, drone: usize) -> Option<usize> {
        self.state
            .units
            .row(self.state.units.drone_parent[drone])
            .filter(|&p| self.state.units.health[p] > Fx::ZERO)
    }

    /// Each live carrier's drones, in row order.
    fn drone_flocks(&self) -> std::collections::BTreeMap<usize, Vec<usize>> {
        let mut flocks: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for row in self.state.units.slots.iter() {
            if self.state.units.drone_parent[row] == Handle::NONE {
                continue;
            }
            if let Some(parent) = self.drone_carrier(row) {
                flocks.entry(parent).or_default().push(row);
            }
        }
        flocks
    }

    /// Drones going up on their sockets, for the economy to pay for like any build:
    /// (carrier, drone, build-time units a tick at full speed, mass and energy that
    /// asks for, the tier it is paid in). A drone makes materials, so the side's
    /// materials priority (`Focus::mines`) says when it is paid in a stall.
    pub(crate) fn drone_jobs(&self) -> Vec<DroneJob> {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&r| {
                units.drone_parent[r] != Handle::NONE && units.has_flag(r, flag::UNDER_CONSTRUCTION)
            })
            .filter_map(|drone| {
                let carrier = self.drone_carrier(drone)?;
                let bp = self.bp(drone);
                let rate = Fx::ONE.min(bp.build_time - units.build_progress[drone]);
                let want = [
                    bp.cost_mass * rate / bp.build_time,
                    bp.cost_energy * rate / bp.build_time,
                ];
                let focus = self.state.players[units.owner[drone] as usize].focus;
                Some(DroneJob {
                    carrier,
                    drone,
                    rate,
                    want,
                    tier: focus.mines.tier(),
                })
            })
            .collect()
    }

    /// The aircraft a drone is docked on, which it rides as that is drawn (`mirror`).
    pub(crate) fn drone_riding(&self, drone: usize) -> Option<usize> {
        let units = &self.state.units;
        if units.drone_parent[drone] == Handle::NONE || units.deploy[drone] != dock::DOCKED {
            return None;
        }
        self.drone_carrier(drone).filter(|&c| {
            self.bp(c)
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
        })
    }

    /// Whether a drone is on or gliding to or from its socket, which `seat_drones` flies
    /// for it, not movement.
    pub(crate) fn drone_seated(&self, row: usize) -> bool {
        self.state.units.drone_parent[row] != Handle::NONE
            && self.state.units.deploy[row] != dock::FLYING
    }

    /// After movement, so a docked drone rides its carrier in step instead of a tick
    /// behind: seat each docked drone on its socket, and fly the glides between the
    /// socket and its line-up point (`drone_approach` below or above it). Letting go, a
    /// drone drops clear to the line-up point and flies from there; coming home, it
    /// slides in to the line-up point, then rises (or settles) onto the socket, slowing
    /// all the way.
    pub(crate) fn seat_drones(&mut self) {
        for (parent, children) in self.drone_flocks() {
            let approach = self.bp(parent).drone_approach;
            // How far the carrier flew this tick: a gliding drone is carried with it, so
            // it closes on its pylon even under a carrier flying faster than it eases.
            let carried = (self.state.units.pos[parent] - self.state.units.prev_pos[parent])
                .extend(self.state.units.z[parent] - self.state.units.prev_z[parent]);
            for row in children {
                let slot = self.state.units.drone_socket[row] as usize;
                let state = self.state.units.deploy[row];
                if state == dock::FLYING {
                    continue;
                }
                let socket = self.drone_socket(parent, slot);
                let seat = socket.xy.extend(socket.z);
                let line_up = socket.xy.extend(socket.z + approach);
                let here = self.state.units.pos[row].extend(self.state.units.z[row]) + carried;
                let (goal, next) = match state {
                    dock::RELEASING => (line_up, dock::FLYING),
                    dock::DOCKING if here.xy().distance(socket.xy) > DOCK_ALIGNED => {
                        (line_up, dock::DOCKING)
                    }
                    dock::DOCKING => (seat, dock::DOCKED),
                    _ => {
                        // Docked: it rides the socket exactly.
                        let units = &mut self.state.units;
                        units.pos[row] = socket.xy;
                        units.z[row] = socket.z;
                        units.heading[row] = socket.heading;
                        units.air_velocity[row] = FxVec3::ZERO;
                        units.speed[row] = Fx::ZERO;
                        continue;
                    }
                };
                let motion = self.bp(row).motion;
                let fastest = motion.map_or(Fx::ONE, |m| m.speed) / (2 * TICKS_PER_SECOND as i32);
                let left = here.distance(goal);
                // Ease in: a fifth of the way a tick, never slower than a creep.
                let step = (left / 5).clamp(DOCK_CREEP, fastest);
                let at = if left <= step {
                    self.state.units.deploy[row] = next;
                    goal
                } else {
                    here.lerp(goal, step / left)
                };
                let turn = motion.map_or(0, |m| m.turn_rate);
                let units = &mut self.state.units;
                units.pos[row] = at.xy();
                units.z[row] = at.z;
                units.heading[row] = units.heading[row].turn_toward(socket.heading, turn);
                units.air_velocity[row] = FxVec3::ZERO;
                units.speed[row] = Fx::ZERO;
            }
        }
    }

    fn carrier_target_live(&self, drone: usize, kind: OrderKind, target: Handle) -> bool {
        match kind {
            OrderKind::Reclaim => self
                .state
                .wrecks
                .slots
                .resolve(target)
                .is_some_and(|w| self.state.wrecks.mass[w] > Fx::ZERO),
            OrderKind::ReclaimUnit => self
                .state
                .units
                .row(target)
                .is_some_and(|t| self.can_reclaim_unit(drone, t)),
            _ => false,
        }
    }

    /// Every drone on the carrier works the ordered wreck or unit, however far it is.
    fn drone_reclaim_ordered(
        &mut self,
        row: usize,
        kind: OrderKind,
        target: Handle,
    ) -> Result<(), SimError> {
        let (at, radius) = match kind {
            OrderKind::Reclaim => {
                let Some(w) = self.state.wrecks.slots.resolve(target) else {
                    return Ok(());
                };
                let bp = self.blueprints.unit(self.state.wrecks.blueprint[w]);
                (self.state.wrecks.pos[w], bp.radius)
            }
            OrderKind::ReclaimUnit => {
                let Some(t) = self.state.units.row(target) else {
                    return Ok(());
                };
                (self.state.units.pos[t], self.bp(t).radius)
            }
            _ => return Ok(()),
        };
        let pos = self.state.units.pos[row];
        let radial = pos - at;
        let hold = self.work_range(row) * Fx::ratio(3, 5);
        let bearing = if radial.length() > Fx::ONE {
            radial.angle()
        } else {
            self.state.units.heading[row]
        };
        let goal =
            self.clamp_to_map(at + FxVec2::from_angle(bearing + Angle::from_degrees(45)) * hold);
        self.ensure_moving(row, goal, goal)?;
        self.state.units.air_aim[row] = at;
        self.state.units.flags[row] |= flag::AIR_RUN;
        if pos.distance(at) > self.work_range(row) + radius {
            return Ok(());
        }
        match kind {
            OrderKind::Reclaim => {
                if let Some(w) = self.state.wrecks.slots.resolve(target) {
                    let power = self.tool_power(row);
                    self.drain_wreck(row, w, power, 0);
                }
            }
            OrderKind::ReclaimUnit => {
                if let Some(t) = self.state.units.row(target) {
                    let power = self.tool_power(row);
                    self.drain_unit(row, t, power, 0);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Each live patch deals its weapon's `burn_dps` to enemies standing in it.
    /// Two bombs on the same ground are two patches, so the rate adds up.
    fn tick_fires(&mut self) {
        let n = self.state.fires.len();
        let mut burning = Vec::new();
        for i in 0..n {
            if self.state.fires.ticks[i] == 0 {
                continue;
            }
            let pos = self.state.fires.pos[i];
            let radius = self.state.fires.radius[i];
            let owner = self.state.fires.owner[i];
            let source = self.state.fires.source[i];
            let mask = self.state.fires.target_mask[i];
            let damage = self.state.fires.dps[i] / TICKS_PER_SECOND as i32;
            let point = pos.extend(self.state.fires.z[i]);
            let mut victims = Vec::new();
            self.index.query(pos, radius, kind::UNIT, |e| {
                if !self.unit_entry_is_current(e) {
                    return true;
                }
                let r = e.row as usize;
                let bp = self.bp(r);
                if self.state.units.is_active(r)
                    && self.are_enemies(owner, self.state.units.owner[r])
                    && self.hittable(r, mask)
                    && self.state.units.pos[r].distance(pos) <= radius + bp.radius
                {
                    victims.push(r);
                }
                true
            });
            let open: Vec<_> = victims
                .into_iter()
                .filter(|&r| {
                    let bp = self.bp(r);
                    let target =
                        self.state.units.pos[r].extend(self.state.units.z[r] + bp.height / 2);
                    self.blast_blocker(point, target, Some(r)).is_none()
                        && !(self.shield_blocking(r) && bp.shield.is_some_and(|s| s.is_hull()))
                })
                .collect();
            for r in open {
                self.damage_unit(r, damage, owner, source);
                if self.state.units.slots.is_alive(r) && self.state.units.health[r] > Fx::ZERO {
                    self.state.units.burn_owner[r] = owner;
                    self.state.units.burn_source[r] = source;
                    burning.push(r);
                }
            }
            self.state.fires.ticks[i] -= 1;
        }
        let mut i = 0;
        while i < self.state.fires.len() {
            if self.state.fires.ticks[i] == 0 {
                self.state.fires.swap_remove(i);
            } else {
                i += 1;
            }
        }
        for row in self.state.units.slots.iter() {
            self.state.units.burn_ticks[row] = 0;
        }
        for row in burning {
            if self.state.units.slots.is_alive(row) {
                self.state.units.burn_ticks[row] = 1;
            }
        }
    }
}

/// A drone going up on its socket this tick (`World::drone_jobs`).
pub(crate) struct DroneJob {
    pub carrier: usize,
    pub drone: usize,
    pub rate: Fx,
    pub want: [Fx; 2],
    pub tier: usize,
}

struct Socket {
    xy: FxVec2,
    z: Fx,
    heading: Angle,
}

/// A drone's `deploy` word: where it is between its socket and free flight.
mod dock {
    /// On its socket, riding the carrier.
    pub(super) const DOCKED: u16 = 0;
    pub(super) const FLYING: u16 = 1;
    /// Dropping clear of the socket to its line-up point.
    pub(super) const RELEASING: u16 = 2;
    /// Gliding in to the line-up point, then onto the socket.
    pub(super) const DOCKING: u16 = 3;
}

/// How near its socket (across the ground) a drone coming home starts its glide in.
const DOCK_CAPTURE: Fx = Fx::from_int(12);
/// How near under (or over) its socket a docking drone is before it rises onto it.
const DOCK_ALIGNED: Fx = Fx::ratio(1, 5);
/// The slowest a glide moves in a tick, so it never crawls to a stop short of the socket.
const DOCK_CREEP: Fx = Fx::ratio(1, 20);
