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
            let count = self
                .state
                .units
                .slots
                .iter()
                .filter(|&r| self.state.units.drone_parent[r] == parent)
                .count();
            // Deliberate: a unit keeps one drone for each socket, and rebuilds none while
            // the unit table is full, so drones never take the last rows from real production.
            if count >= self.bp(row).drone_sockets.len()
                || self.state.units.slots.live() >= crate::tables::MAX_UNITS
            {
                continue;
            }
            // Drones cost time alone (checked on load): paid out of what was left in store
            // after the economy's own spending, a drone that cost mass waited for a surplus
            // that a new side, the one that most needs its salvage, never has.
            let time = self.blueprints.unit(drone).build_time;
            let work = Fx::ONE.min(time - self.state.units.drone_progress[row]);
            self.state.units.drone_progress[row] += work;
            if self.state.units.drone_progress[row] >= time {
                self.state.units.drone_progress[row] = Fx::ZERO;
                let owner = self.state.units.owner[row];
                let pos = self.state.units.pos[row];
                let heading = self.state.units.heading[row];
                let child = self.spawn_unit(drone, owner, pos, heading, true)?;
                self.state.units.z[child] = self.state.units.z[row];
                self.state.units.prev_z[child] = self.state.units.z[row];
                self.state.units.drone_parent[child] = parent;
                // Made inside the hull, it comes out underneath and flies onto its socket.
                self.state.units.deploy[child] = dock::DOCKING;
                self.state.players[owner as usize].units_built += 1;
            }
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
                self.carrier_recalling(parent)
            } else {
                !self.carrier_has_work(parent)
            };
            let need = self.bp(parent).motion.map(|m| m.deploy_ticks).unwrap_or(0);
            // A carrier lets its drones go once it has settled into a hover; a port
            // has nothing to wait for.
            let open = need == 0 || self.state.units.deploy[parent] >= need;
            let center = self.state.units.pos[parent];
            let reach = self.bp(parent).drone_radius;
            let owner = self.state.units.owner[parent];
            let full = self.no_room_for_salvage(owner);
            for (slot, row) in children.iter().copied().enumerate() {
                let dock = self.drone_socket(parent, slot);
                let pos = self.state.units.pos[row];
                let state = self.state.units.deploy[row];
                if recalling || !open {
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
                    .filter(|&w| {
                        if full {
                            return false;
                        }
                        self.state.wrecks.pos[w].distance(center) <= reach
                            && self.state.wrecks.mass[w] > Fx::ZERO
                            && (!self.state.fog_enabled
                                || self
                                    .fog
                                    .is_detected(self.state.wrecks.pos[w], self.team_mask(owner)))
                    })
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

    /// A move order, or nothing left to salvage, brings the wing home.
    pub(crate) fn carrier_recalling(&self, row: usize) -> bool {
        let front = self.state.orders.front(&self.state.units, row);
        let ordered = self.carrier_reclaim_ordered(row);
        let moving =
            front.is_some_and(|o| matches!(o.kind, OrderKind::Move | OrderKind::AttackMove));
        moving || (!ordered && !self.carrier_has_work(row))
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
        let center = self.state.units.pos[row];
        let reach = self.bp(row).drone_radius;
        let owner = self.state.units.owner[row];
        if self.no_room_for_salvage(owner) {
            return false;
        }
        self.state.wrecks.slots.iter().any(|w| {
            self.state.wrecks.pos[w].distance(center) <= reach
                && self.state.wrecks.mass[w] > Fx::ZERO
                && (!self.state.fog_enabled
                    || self
                        .fog
                        .is_detected(self.state.wrecks.pos[w], self.team_mask(owner)))
        })
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

    /// Each live carrier's drones, in row order: a drone's place in the list is its
    /// socket.
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
            for (slot, row) in children.iter().copied().enumerate() {
                let state = self.state.units.deploy[row];
                if state == dock::FLYING {
                    continue;
                }
                let socket = self.drone_socket(parent, slot);
                let seat = socket.xy.extend(socket.z);
                let line_up = socket.xy.extend(socket.z + approach);
                let here = self.state.units.pos[row].extend(self.state.units.z[row]);
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

    /// Each live patch deals 30 damage a second to enemies standing in it.
    /// Two bombs on the same ground are two patches, so the rate is 30 times the count.
    fn tick_fires(&mut self) {
        let damage = Fx::ratio(30, TICKS_PER_SECOND as i64);
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
