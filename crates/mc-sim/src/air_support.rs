//! Carrier-owned salvage drones, paid production and incendiary damage.
use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::{flag, OrderKind};
use crate::{Handle, SimError, World};
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::WeaponColor;

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
            if count >= 4 || self.state.units.slots.live() >= crate::tables::MAX_UNITS {
                continue;
            }
            let db = self.blueprints.unit(drone);
            let time = db.build_time;
            let work = Fx::ONE.min(time - self.state.units.drone_progress[row]);
            let mass = db.cost_mass * work / time;
            let energy = db.cost_energy * work / time;
            let player = &mut self.state.players[self.state.units.owner[row] as usize];
            if !player.free_build && (player.mass < mass || player.energy < energy) {
                continue;
            }
            if !player.free_build {
                player.mass -= mass;
                player.energy -= energy;
            }
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
                self.state.players[owner as usize].units_built += 1;
            }
        }
        let drones: Vec<_> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| self.state.units.drone_parent[r] != Handle::NONE)
            .collect();
        let mut claimed = std::collections::BTreeSet::new();
        let mut by_parent: std::collections::BTreeMap<usize, Vec<usize>> =
            std::collections::BTreeMap::new();
        for row in drones {
            let Some(parent) = self
                .state
                .units
                .row(self.state.units.drone_parent[row])
                .filter(|&p| self.state.units.health[p] > Fx::ZERO)
            else {
                self.state.units.flags[row] |= flag::RECLAIMED;
                self.state.units.health[row] = Fx::ZERO;
                continue;
            };
            by_parent.entry(parent).or_default().push(row);
        }
        for (parent, children) in by_parent {
            let task = self
                .state
                .orders
                .front(&self.state.units, parent)
                .filter(|o| matches!(o.kind, OrderKind::Reclaim | OrderKind::ReclaimUnit))
                .map(|o| (o.kind, o.target));
            let recalling = self.carrier_recalling(parent);
            let need = self.bp(parent).motion.map(|m| m.deploy_ticks).unwrap_or(0);
            let open = need > 0 && self.state.units.deploy[parent] >= need;
            let center = self.state.units.pos[parent];
            let heading = self.state.units.heading[parent];
            let altitude = self.state.units.z[parent];
            let reach = self.bp(parent).drone_radius;
            let owner = self.state.units.owner[parent];
            let full = self.state.players[owner as usize].mass
                >= self.state.players[owner as usize].mass_capacity;
            for (slot, row) in children.iter().copied().enumerate() {
                let dock = self.drone_socket(
                    center,
                    heading,
                    altitude,
                    slot,
                    self.state.units.deploy[parent],
                    need,
                );
                let pos = self.state.units.pos[row];
                let launched = self.state.units.deploy[row] > 0;
                if recalling || !open {
                    if launched && pos.distance(dock.xy) > Fx::from_int(5) {
                        self.ensure_moving(row, dock.xy, dock.xy)?;
                        self.state.units.flags[row] &= !flag::AIR_RUN;
                    } else {
                        self.pin_drone(row, dock.xy, dock.z, heading)?;
                    }
                    continue;
                }
                if let Some((kind, target)) = task {
                    if !self.carrier_target_live(row, kind, target) {
                        self.finish_order(parent);
                        continue;
                    }
                }
                if !launched {
                    self.pin_drone(row, dock.xy, dock.z, heading)?;
                    self.state.units.deploy[row] = 1;
                    if let Some(blueprint) = self.blueprints.id_of("aster_t1_interceptor") {
                        let at = dock.xy.extend(dock.z);
                        self.events.push(SimEvent::ShotFired {
                            pos: at,
                            vel: FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::from_int(2)),
                            travel: FxVec3::ZERO,
                            color: WeaponColor::Orange,
                            owner,
                            blueprint,
                            weapon: 0,
                        });
                    }
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
                        self.drain_wreck(row, w);
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
        let player = &self.state.players[owner as usize];
        if player.mass >= player.mass_capacity {
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

    /// Socket under a wing. `deploy` slides it from the bay to the launch rail.
    fn drone_socket(
        &self,
        center: FxVec2,
        heading: Angle,
        altitude: Fx,
        slot: usize,
        deploy: u16,
        need: u16,
    ) -> Socket {
        let open = if need == 0 {
            Fx::ZERO
        } else {
            Fx::from_int(deploy as i32) / Fx::from_int(need as i32)
        };
        // Stowed in the hold under the midbody, two abreast and two deep, hanging
        // from cradles 0.7 m up in the hull (the Osprey model's `CRADLES`). Opening
        // the hold lowers the cradles 1.9 m, so the flock drops out between the
        // door leaves and is clear of the hull before it flies.
        let along = [
            Fx::ratio(-3, 2),
            Fx::ratio(-3, 2),
            Fx::ratio(3, 2),
            Fx::ratio(3, 2),
        ][slot % 4];
        let across = [
            Fx::ratio(13, 10),
            Fx::ratio(-13, 10),
            Fx::ratio(13, 10),
            Fx::ratio(-13, 10),
        ][slot % 4];
        Socket {
            xy: center + FxVec2::new(along, across).rotate(heading),
            z: altitude + Fx::ratio(7, 10) - open * Fx::ratio(19, 10),
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
                    self.drain_wreck(row, w);
                }
            }
            OrderKind::ReclaimUnit => {
                if let Some(t) = self.state.units.row(target) {
                    self.drain_unit(row, t);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn pin_drone(
        &mut self,
        row: usize,
        pos: FxVec2,
        z: Fx,
        heading: Angle,
    ) -> Result<(), SimError> {
        self.clear_orders(row)?;
        let units = &mut self.state.units;
        units.pos[row] = pos;
        units.z[row] = z;
        units.heading[row] = heading;
        units.air_velocity[row] = FxVec3::ZERO;
        units.speed[row] = Fx::ZERO;
        units.deploy[row] = 0;
        units.flags[row] &= !flag::AIR_RUN;
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
}
