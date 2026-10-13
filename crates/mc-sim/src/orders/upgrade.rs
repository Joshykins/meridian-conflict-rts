//! A unit's own upgrade or refit coming to an end (`World::run_upgrade`), out of
//! `orders.rs`, whose private helpers it shares.

use super::*;

impl World {
    pub(super) fn run_upgrade(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        // A refit to a module: what the unit becomes once the kit is on. One that no longer
        // fits (the tier it went over was cancelled) is dropped.
        let refit = match self.blueprints.kit(o.blueprint) {
            Some(_) => match self.blueprints.refit_result(self.bp(row).id, o.blueprint) {
                Ok(to) => Some(to),
                Err(_) => {
                    self.abort_product(row)?;
                    self.finish_order(row);
                    return Ok(());
                }
            },
            None => None,
        };
        let in_place = self.upgrades_in_place(row);
        if self.bp(row).is_mobile() {
            // Refitted where it stands: it neither walks nor builds until it is done or told to
            // stop (orders given meanwhile wait behind the refit), but it still shoots at what comes near.
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            self.state.units.flags[row] |= flag::HOLD;
        }
        // Only the successor this order spawned counts: a builder's `build_target` can still
        // name the ally it last mended, which would pass as a finished upgrade (and be removed).
        let units = &self.state.units;
        let product = units
            .row(units.build_target[row])
            .filter(|&t| units.has_flag(t, flag::UPGRADE) && units.blueprint[t] == o.blueprint);
        if product.is_none() {
            self.state.units.build_target[row] = Handle::NONE;
        }
        let units = &self.state.units;
        let Some(t) = product else {
            // Paused before it began: nothing is started until resumed.
            if self.work_paused(row) {
                return Ok(());
            }
            // The successor is assembled in place, hidden inside the old structure.
            let z = units.z[row];
            let t = self.state.units.spawn(UnitSpawn {
                blueprint: o.blueprint,
                owner: units.owner[row],
                pos: units.pos[row],
                z,
                heading: units.heading[row],
                health: self.blueprints.unit(o.blueprint).health / 10,
                flags: flag::UNDER_CONSTRUCTION | flag::IN_FACTORY | flag::UPGRADE,
                build_progress: Fx::ZERO,
            })?;
            self.state.units.build_target[row] = self.state.units.id(t);
            return Ok(());
        };
        if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            if !self.work_paused(row) {
                self.state.units.flags[row] |= flag::BUILDING;
            }
            return Ok(());
        }
        if in_place {
            // The same unit under the same id, so selections, groups and a commander's standing survive.
            let rank = self.state.units.veterancy[row];
            let old_max = crate::veterancy_health(self.bp(row).health, rank);
            let had_shield = self.bp(row).shield.is_some();
            let old_radius = self.dome_radius(row);
            let new = match refit {
                Some(to) => self.blueprints.unit(to).clone(),
                None => self.bp(t).clone(),
            };
            let new_max = crate::veterancy_health(new.health, rank);
            let old_id = self.state.units.blueprint[row];
            // `inherit_shield` already scaled the bubble onto the successor. A refit keeps
            // the field it has, or raises the one it has just been given.
            let shield = new.shield.filter(|_| refit.is_none()).map(|_| {
                (
                    self.state.units.shield_hp[t],
                    self.state.units.shield_open[t],
                    self.state.units.prev_shield_open[t],
                    self.state.units.shield_recharge[t],
                )
            });
            let units = &mut self.state.units;
            units.health[row] = (units.health[row] + new_max - old_max).clamp(Fx::ONE, new_max);
            units.blueprint[row] = new.id;
            units.build_progress[row] = new.build_time;
            units.build_target[row] = Handle::NONE;
            if let Some((hp, open, prev_open, recharge)) = shield {
                units.shield_hp[row] = hp;
                units.shield_open[row] = open;
                units.prev_shield_open[row] = prev_open;
                units.shield_recharge[row] = recharge;
                // The dome kept its old size while the upgrade was built; it swells
                // out to the new one now that it is done.
                if had_shield && new.shield.is_some_and(|s| s.radius != old_radius) {
                    units.shield_from[row] = old_radius;
                    units.shield_grow[row] = crate::shields::SHIELD_GROW_TICKS;
                }
            }
            if refit.is_some() && new.shield.is_some() != had_shield {
                // Raised from nothing, or taken off: `arm_shield` clears what has no field.
                units.shield_hp[row] = Fx::ZERO;
                units.shield_open[row] = 0;
                units.prev_shield_open[row] = 0;
                units.shield_recharge[row] = 0;
                self.arm_shield(row, false);
            }
            if refit.is_some() && self.deploy_span(row) == 0 {
                self.state.units.deploy[row] = 0;
                self.state.units.prev_deploy[row] = 0;
            }
            self.fit_mounts(row);
            self.remove_unit_row(t, true)?;
            if self.bp(row).is_structure() {
                self.reshape_lot(row, old_id, new.id);
            }
            self.events.push(SimEvent::UnitCompleted {
                unit: self.state.units.id(row),
                owner: self.state.units.owner[row],
            });
            self.finish_order(row);
            return Ok(());
        }
        let units = &mut self.state.units;
        units.flags[t] &= !(flag::IN_FACTORY | flag::UPGRADE);
        units.flags[t] |= units.flags[row] & flag::REPEAT;
        units.fire_state[t] = units.fire_state[row];
        units.paused[t] = units.paused[row];
        units.priority[t] = units.priority[row];
        units.rally[t] = units.rally[row];

        units.build_target[row] = Handle::NONE;
        // The footprint stays blocked: the successor stands exactly where this one stood.
        let (old_id, new_id) = (
            self.state.units.blueprint[row],
            self.state.units.blueprint[t],
        );
        self.remove_unit_row(row, true)?;
        if self.bp(t).is_structure() {
            self.reshape_lot(t, old_id, new_id);
        }
        Ok(())
    }
}
