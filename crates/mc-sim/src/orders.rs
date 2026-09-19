//! Commands become orders; orders drive units.
//!
//! `apply_command` validates a player's command and edits order queues.
//! `run_orders` then advances the order at the front of every unit's queue.
//! Orders never move a unit themselves: they set a destination and flags, and
//! the movement, economy and combat phases do the work.

use crate::command::{Command, PlayerCommand};
use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{cat, BlueprintId};

/// Longest production queue per factory.
const MAX_FACTORY_QUEUE: usize = 64;
/// Chasing units re-path once their quarry has moved this far from the field's goal.
const CHASE_REPATH_DISTANCE: Fx = Fx::from_int(96);
/// Build power of structures that upgrade themselves but are not builders.
pub(crate) const SELF_UPGRADE_POWER: Fx = Fx::from_int(10);

impl World {
    pub(crate) fn apply_command(&mut self, pc: &PlayerCommand) -> Result<(), SimError> {
        let player = pc.player;
        let Some(p) = self.state.players.get(player as usize) else { return Ok(()) };
        if p.defeated {
            return Ok(());
        }
        match &pc.command {
            Command::Move { units, target, queue } => self.order_group(player, units, OrderKind::Move, *target, *queue),
            Command::AttackMove { units, target, queue } => self.order_group(player, units, OrderKind::AttackMove, *target, *queue),
            Command::Attack { units, target, queue } => {
                let Some(t) = self.state.units.row(*target) else { return Ok(()) };
                let pos = self.state.units.pos[t];
                for row in self.owned(player, units, cat::MOBILE) {
                    if !self.bp(row).weapons.is_empty() {
                        self.give(row, order(OrderKind::Attack, pos, *target), *queue)?;
                    }
                }
                Ok(())
            }
            Command::Stop { units } => {
                for row in self.owned(player, units, 0) {
                    self.clear_orders(row)?;
                }
                Ok(())
            }
            Command::Build { units, blueprint, pos, heading, queue } => {
                if blueprint.index() >= self.blueprints.units.len() {
                    return Ok(());
                }
                let bp = self.blueprints.unit(*blueprint);
                if !bp.is_structure() {
                    return Ok(());
                }
                let site = snap_to_build_grid(bp, *pos);
                for row in self.owned(player, units, cat::MOBILE) {
                    let can = self.bp(row).builder.as_ref().is_some_and(|b| b.builds.contains(blueprint));
                    if can {
                        let mut o = order(OrderKind::Build, site, Handle::NONE);
                        o.blueprint = *blueprint;
                        o.heading = *heading;
                        self.give(row, o, *queue)?;
                    }
                }
                Ok(())
            }
            Command::Assist { units, target, queue } => {
                let Some(t) = self.state.units.row(*target) else { return Ok(()) };
                if self.are_enemies(player, self.state.units.owner[t]) {
                    return Ok(());
                }
                let pos = self.state.units.pos[t];
                for row in self.owned(player, units, cat::MOBILE) {
                    if row != t && self.bp(row).builder.is_some() {
                        self.give(row, order(OrderKind::Assist, pos, *target), *queue)?;
                    }
                }
                Ok(())
            }
            Command::ReclaimWreck { units, wreck, queue } => {
                let Some(w) = self.state.wrecks.slots.resolve(*wreck) else { return Ok(()) };
                let pos = self.state.wrecks.pos[w];
                for row in self.owned(player, units, cat::MOBILE) {
                    if self.bp(row).builder.is_some() {
                        self.give(row, order(OrderKind::Reclaim, pos, *wreck), *queue)?;
                    }
                }
                Ok(())
            }
            Command::Produce { factories, blueprint, count } => {
                for row in self.owned(player, factories, cat::FACTORY) {
                    let can = self.bp(row).builder.as_ref().is_some_and(|b| b.builds.contains(blueprint));
                    if !can {
                        continue;
                    }
                    let queued = self.state.orders.iter(&self.state.units, row).count();
                    let room = MAX_FACTORY_QUEUE.saturating_sub(queued);
                    for _ in 0..(*count as usize).min(room) {
                        let mut o = order(OrderKind::Produce, self.state.units.pos[row], Handle::NONE);
                        o.blueprint = *blueprint;
                        self.state.orders.push_back(&mut self.state.units, row, o)?;
                    }
                }
                Ok(())
            }
            Command::CancelProduce { factories, blueprint } => {
                for row in self.owned(player, factories, cat::FACTORY) {
                    self.cancel_last_produce(row, *blueprint)?;
                }
                Ok(())
            }
            Command::SetRepeat { factories, repeat } => {
                for row in self.owned(player, factories, cat::FACTORY) {
                    let f = &mut self.state.units.flags[row];
                    *f = if *repeat { *f | flag::REPEAT } else { *f & !flag::REPEAT };
                }
                Ok(())
            }
            Command::SetRally { factories, pos } => {
                let pos = self.clamp_to_map(*pos);
                for row in self.owned(player, factories, cat::FACTORY) {
                    self.state.units.rally[row] = pos;
                }
                Ok(())
            }
            Command::Upgrade { units } => {
                for row in self.owned(player, units, cat::STRUCTURE) {
                    let Some(next) = self.bp(row).upgrades_to else { continue };
                    let already = self.state.orders.iter(&self.state.units, row).any(|o| o.kind == OrderKind::Upgrade);
                    if !already {
                        let mut o = order(OrderKind::Upgrade, self.state.units.pos[row], Handle::NONE);
                        o.blueprint = next;
                        self.state.orders.push_back(&mut self.state.units, row, o)?;
                    }
                }
                Ok(())
            }
            Command::SelfDestruct { units } => {
                for row in self.owned(player, units, 0) {
                    self.state.units.health[row] = Fx::ZERO;
                }
                Ok(())
            }
            Command::Resign => {
                self.defeat_player(player);
                Ok(())
            }
            Command::DebugSpawn { owner, blueprint, pos, heading, count } => {
                if !self.state.cheats || *owner as usize >= self.state.players.len() || blueprint.index() >= self.blueprints.units.len() {
                    return Ok(());
                }
                let bp = self.blueprints.unit(*blueprint).clone();
                if bp.is_structure() {
                    let site = snap_to_build_grid(&bp, *pos);
                    if self.can_place(&bp, site) {
                        self.spawn_unit(*blueprint, *owner, site, *heading, true)?;
                    }
                    return Ok(());
                }
                // A square block centred on `pos`.
                let n = (*count).clamp(1, 1024) as i32;
                let cols = (Fx::from_int(n).sqrt().ceil_int()).max(1);
                let spacing = bp.radius * 2 + Fx::from_int(3);
                for i in 0..n {
                    let offset = FxVec2::new(spacing * (i % cols - cols / 2), spacing * (i / cols - cols / 2));
                    let p = self.clamp_to_map(*pos + offset);
                    self.spawn_unit(*blueprint, *owner, p, *heading, true)?;
                }
                Ok(())
            }
        }
    }

    /// Live, complete units in `ids` that `player` owns and that have all of `categories`.
    fn owned(&self, player: u8, ids: &[UnitId], categories: u32) -> Vec<usize> {
        let units = &self.state.units;
        ids.iter()
            .filter_map(|id| units.row(*id))
            .filter(|&row| units.owner[row] == player && units.is_active(row) && self.bp(row).has(categories))
            .collect()
    }

    fn give(&mut self, row: usize, o: Order, queue: bool) -> Result<(), SimError> {
        if !queue {
            self.clear_orders(row)?;
        }
        self.state.orders.push_back(&mut self.state.units, row, o)
    }

    /// Move-type order for a group: everyone shares the target (and so the flow
    /// field) and gets a slot in a block formation facing the direction of travel.
    fn order_group(&mut self, player: u8, ids: &[UnitId], kind: OrderKind, target: FxVec2, queue: bool) -> Result<(), SimError> {
        let rows = self.owned(player, ids, cat::MOBILE);
        if rows.is_empty() {
            return Ok(());
        }
        let target = self.clamp_to_map(target);
        let n = rows.len() as i32;
        let mut centroid = FxVec2::ZERO;
        let mut spacing = Fx::ZERO;
        for &row in &rows {
            centroid += self.state.units.pos[row];
            spacing = spacing.max(self.bp(row).radius * 2 + Fx::from_int(4));
        }
        centroid = FxVec2::new(centroid.x / n, centroid.y / n);
        let facing = (target - centroid).angle();
        let cols = Fx::from_int(n).sqrt().ceil_int().max(1);
        let depth = (n + cols - 1) / cols;
        for (i, &row) in rows.iter().enumerate() {
            let (c, r) = (i as i32 % cols, i as i32 / cols);
            // Local frame: x forward, y left. Centre the block on the target.
            let local = FxVec2::new(
                spacing * (depth - 1) / 2 - spacing * r,
                spacing * (cols - 1) / 2 - spacing * c,
            );
            let mut o = order(kind, target, Handle::NONE);
            o.offset = if n == 1 { FxVec2::ZERO } else { local.rotate(facing) };
            o.heading = facing;
            self.give(row, o, queue)?;
        }
        Ok(())
    }

    fn cancel_last_produce(&mut self, row: usize, blueprint: BlueprintId) -> Result<(), SimError> {
        let queue: Vec<Order> = self.state.orders.iter(&self.state.units, row).copied().collect();
        let Some(last) = queue.iter().rposition(|o| o.kind == OrderKind::Produce && o.blueprint == blueprint) else {
            return Ok(());
        };
        if last == 0 {
            // The unit being assembled: scrap it.
            self.abort_product(row)?;
        }
        self.state.orders.clear(&mut self.state.units, row);
        for (i, o) in queue.into_iter().enumerate() {
            if i != last {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        Ok(())
    }

    /// Removes the half-built unit or upgrade a factory/structure is working on.
    fn abort_product(&mut self, row: usize) -> Result<(), SimError> {
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.has_flag(t, flag::IN_FACTORY) {
                self.despawn_unit(t, false)?;
            }
        }
        self.state.units.build_target[row] = Handle::NONE;
        Ok(())
    }

    pub(crate) fn clear_orders(&mut self, row: usize) -> Result<(), SimError> {
        self.state.orders.clear(&mut self.state.units, row);
        self.stop_moving(row);
        if self.bp(row).is_structure() {
            self.abort_product(row)?;
        }
        self.state.units.build_target[row] = Handle::NONE;
        Ok(())
    }

    pub(crate) fn stop_moving(&mut self, row: usize) {
        let units = &mut self.state.units;
        if units.flags[row] & flag::HAS_FIELD != 0 {
            self.nav.release(units.field[row]);
            units.field[row] = NO_FIELD;
            units.flags[row] &= !flag::HAS_FIELD;
        }
        units.move_goal[row] = units.pos[row];
        units.stuck_ticks[row] = 0;
    }

    /// Points the unit at `own_goal`, following the shared field toward `field_goal`.
    fn ensure_moving(&mut self, row: usize, field_goal: FxVec2, own_goal: FxVec2) -> Result<(), SimError> {
        let units = &self.state.units;
        let motion = match self.bp(row).motion {
            Some(m) => m,
            None => return Ok(()),
        };
        if units.flags[row] & flag::HAS_FIELD != 0 && units.field_goal[row] == field_goal {
            self.state.units.move_goal[row] = own_goal;
            return Ok(());
        }
        self.stop_moving(row);
        let Some(id) = self.nav.request(motion.layer, motion.size_class, field_goal, self.state.units.pos[row])? else {
            // Nowhere near the goal can be stood on; the order gives up next tick.
            self.state.units.stuck_ticks[row] = u16::MAX;
            return Ok(());
        };
        let units = &mut self.state.units;
        units.field[row] = id;
        units.field_goal[row] = field_goal;
        units.flags[row] |= flag::HAS_FIELD;
        units.move_goal[row] = own_goal;
        Ok(())
    }

    pub(crate) fn clamp_to_map(&self, p: FxVec2) -> FxVec2 {
        let size = self.terrain.size_metres();
        let margin = Fx::from_int(4);
        FxVec2::new(p.x.clamp(margin, size.x - margin), p.y.clamp(margin, size.y - margin))
    }

    pub(crate) fn run_orders(&mut self) -> Result<(), SimError> {
        // Units spawned while orders run start acting next tick.
        let rows = self.state.units.slots.rows();
        for row in 0..rows {
            if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
                continue;
            }
            let Some(o) = self.state.orders.front(&self.state.units, row).copied() else {
                if self.state.units.has_flag(row, flag::HAS_FIELD) {
                    self.stop_moving(row);
                }
                continue;
            };
            match o.kind {
                OrderKind::Move | OrderKind::AttackMove => self.run_move(row, &o)?,
                OrderKind::Attack => self.run_attack(row, &o)?,
                OrderKind::Build => self.run_build(row, &o)?,
                OrderKind::Assist => self.run_assist(row, &o)?,
                OrderKind::Reclaim => self.run_reclaim(row, &o)?,
                OrderKind::Produce => self.run_produce(row, &o)?,
                OrderKind::Upgrade => self.run_upgrade(row, &o)?,
            }
        }
        Ok(())
    }

    fn finish_order(&mut self, row: usize) {
        self.state.orders.pop_front(&mut self.state.units, row);
        self.state.units.stuck_ticks[row] = 0;
        // Keep the field when the next order heads to the same place; otherwise drop it.
        let next_same = self
            .state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|n| matches!(n.kind, OrderKind::Move | OrderKind::AttackMove) && n.pos == self.state.units.field_goal[row]);
        if !next_same {
            self.stop_moving(row);
        }
    }

    fn run_move(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let goal = self.clamp_to_map(o.pos + o.offset);
        let units = &self.state.units;
        // Settle for "close" sooner the longer the unit has been jostling for its slot.
        let tolerance = self.bp(row).radius / 2 + Fx::from_int(2) + Fx::from_int(units.stuck_ticks[row] as i32) / 2;
        if units.pos[row].distance(goal) <= tolerance || units.stuck_ticks[row] == u16::MAX {
            self.finish_order(row);
            return Ok(());
        }
        if o.kind == OrderKind::AttackMove && self.has_live_target(row) {
            self.state.units.flags[row] |= flag::HOLD;
        }
        self.ensure_moving(row, o.pos, goal)
    }

    fn has_live_target(&self, row: usize) -> bool {
        let units = &self.state.units;
        units.weapon_target[row].iter().any(|t| units.row(*t).is_some())
    }

    fn run_attack(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(o.target) else {
            self.finish_order(row);
            return Ok(());
        };
        if !self.detects(units.owner[row], t) {
            self.finish_order(row);
            return Ok(());
        }
        let range = self.bp(row).weapons.first().map_or(Fx::ZERO, |w| w.range_max);
        let target_pos = units.pos[t];
        let gap = units.pos[row].distance(target_pos) - self.bp(t).radius;
        if gap <= range * Fx::ratio(9, 10) {
            self.state.units.flags[row] |= flag::HOLD;
            return Ok(());
        }
        let field_goal = if units.has_flag(row, flag::HAS_FIELD) && units.field_goal[row].distance(target_pos) <= CHASE_REPATH_DISTANCE {
            units.field_goal[row]
        } else {
            target_pos
        };
        self.ensure_moving(row, field_goal, target_pos)
    }

    /// Walks a builder into range of `pos`. True once it is close enough to work.
    fn approach(&mut self, row: usize, pos: FxVec2, target_radius: Fx) -> Result<bool, SimError> {
        let range = self.bp(row).builder.as_ref().map_or(Fx::ZERO, |b| b.range) + target_radius;
        if self.state.units.pos[row].distance(pos) <= range {
            self.state.units.flags[row] |= flag::HOLD;
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            return Ok(true);
        }
        if self.state.units.stuck_ticks[row] == u16::MAX {
            return Ok(false);
        }
        self.ensure_moving(row, pos, pos)?;
        Ok(false)
    }

    fn run_build(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.pos[t] == o.pos && units.has_flag(t, flag::UNDER_CONSTRUCTION) {
                self.state.units.flags[row] |= flag::BUILDING | flag::HOLD;
            } else {
                self.state.units.build_target[row] = Handle::NONE;
                self.finish_order(row);
            }
            return Ok(());
        }
        let bp = self.blueprints.unit(o.blueprint).clone();
        if !self.approach(row, o.pos, bp.radius)? {
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
            }
            return Ok(());
        }
        // Someone may have started this exact structure already: join in.
        if let Some(existing) = self.structure_at(o.pos, 0) {
            let units = &self.state.units;
            let same = units.blueprint[existing] == o.blueprint
                && units.pos[existing] == o.pos
                && units.has_flag(existing, flag::UNDER_CONSTRUCTION)
                && !self.are_enemies(units.owner[row], units.owner[existing]);
            if same {
                self.state.units.build_target[row] = units.id(existing);
                return Ok(());
            }
        }
        if !self.can_place(&bp, o.pos) || self.mobile_units_in_footprint(&bp, o.pos, row) {
            // Friendly units standing on the site get a moment to clear off.
            if self.can_place(&bp, o.pos) && self.state.units.stuck_ticks[row] < 50 {
                self.state.units.stuck_ticks[row] += 1;
                return Ok(());
            }
            self.events.push(SimEvent::BuildRejected { player: self.state.units.owner[row] });
            self.finish_order(row);
            return Ok(());
        }
        let owner = self.state.units.owner[row];
        let site = self.spawn_unit(o.blueprint, owner, o.pos, o.heading, false)?;
        self.state.units.build_target[row] = self.state.units.id(site);
        Ok(())
    }

    fn mobile_units_in_footprint(&self, bp: &mc_data::UnitBlueprint, pos: FxVec2, except: usize) -> bool {
        let half = Fx::from_int(bp.footprint.0.max(bp.footprint.1) as i32 * mc_map::BUILD_CELL_M / 2);
        let mut found = false;
        self.index.query(pos, half, kind::UNIT, |e| {
            let r = e.row as usize;
            if r != except && self.unit_entry_is_current(e) && self.bp(r).is_mobile() {
                let d = e.pos - pos;
                if d.x.abs() < half && d.y.abs() < half {
                    found = true;
                    return false;
                }
            }
            true
        });
        found
    }

    fn run_assist(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(o.target) else {
            self.state.units.build_target[row] = Handle::NONE;
            self.finish_order(row);
            return Ok(());
        };
        // What there is to do, in priority order: finish the target itself,
        // help with whatever it is building, or repair it.
        let work = if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            Some(t)
        } else if let Some(product) = units.row(units.build_target[t]).filter(|p| units.has_flag(*p, flag::UNDER_CONSTRUCTION)) {
            Some(product)
        } else if units.health[t] < self.bp(t).health {
            Some(t)
        } else {
            None
        };
        match work {
            Some(w) => {
                let (pos, radius) = (self.state.units.pos[w], self.bp(w).radius);
                if self.approach(row, pos, radius)? {
                    self.state.units.build_target[row] = self.state.units.id(w);
                    self.state.units.flags[row] |= flag::BUILDING;
                }
            }
            None => {
                self.state.units.build_target[row] = Handle::NONE;
                let is_worker = self.bp(t).builder.is_some();
                if !is_worker {
                    self.finish_order(row);
                } else {
                    // Stay near the builder being assisted.
                    let (pos, radius) = (self.state.units.pos[t], self.bp(t).radius);
                    self.approach(row, pos, radius)?;
                }
            }
        }
        Ok(())
    }

    fn run_reclaim(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(w) = self.state.wrecks.slots.resolve(o.target) else {
            self.finish_order(row);
            return Ok(());
        };
        let pos = self.state.wrecks.pos[w];
        let radius = self.blueprints.unit(self.state.wrecks.blueprint[w]).radius;
        if !self.approach(row, pos, radius)? {
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
            }
            return Ok(());
        }
        let power = self.bp(row).builder.as_ref().map_or(Fx::ZERO, |b| b.power);
        let take = (power * World::reclaim_rate()).min(self.state.wrecks.mass[w]);
        self.state.wrecks.mass[w] -= take;
        let player = &mut self.state.players[self.state.units.owner[row] as usize];
        player.mass += take;
        player.reclaimed_mass += take;
        self.state.units.flags[row] |= flag::RECLAIMING;
        if self.state.wrecks.mass[w] <= Fx::ZERO {
            self.state.wrecks.slots.free(w);
            self.finish_order(row);
        }
        Ok(())
    }

    fn run_produce(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(units.build_target[row]) else {
            let (owner, pos, heading) = (units.owner[row], units.pos[row], units.heading[row]);
            let t = self.spawn_unit(o.blueprint, owner, pos, heading, false)?;
            self.state.units.flags[t] |= flag::IN_FACTORY;
            self.state.units.build_target[row] = self.state.units.id(t);
            return Ok(());
        };
        if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            self.state.units.flags[row] |= flag::BUILDING;
            return Ok(());
        }
        // Finished: roll it out of the first side that is clear.
        let reach = Fx::from_int(self.bp(row).footprint.0 as i32 * mc_map::BUILD_CELL_M / 2) + self.bp(t).radius + Fx::from_int(8);
        let motion = self.bp(t).motion;
        let (pos, heading) = (units.pos[row], units.heading[row]);
        let exit = (0..4).map(|side| pos + FxVec2::from_angle(heading + Angle(side * 0x4000)) * reach).find(|p| {
            motion.is_none_or(|m| self.nav.passable(m.layer, m.size_class, *p)) && self.terrain.in_bounds(*p)
        });
        let Some(exit) = exit else { return Ok(()) };
        let rally = if units.rally[row] == pos { exit + (exit - pos).normalize() * Fx::from_int(40) } else { units.rally[row] };
        let units = &mut self.state.units;
        units.flags[t] &= !flag::IN_FACTORY;
        units.pos[t] = exit;
        units.prev_pos[t] = exit;
        units.z[t] = self.terrain.height_at(exit);
        units.prev_z[t] = units.z[t];
        units.heading[t] = (exit - pos).angle();
        units.prev_heading[t] = units.heading[t];
        units.build_target[row] = Handle::NONE;
        let rally = self.clamp_to_map(rally);
        self.state.orders.push_back(&mut self.state.units, t, order(OrderKind::Move, rally, Handle::NONE))?;
        self.state.orders.pop_front(&mut self.state.units, row);
        if self.state.units.has_flag(row, flag::REPEAT) {
            self.state.orders.push_back(&mut self.state.units, row, *o)?;
        }
        Ok(())
    }

    fn run_upgrade(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(units.build_target[row]) else {
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
            self.state.units.flags[row] |= flag::BUILDING;
            return Ok(());
        }
        let units = &mut self.state.units;
        units.flags[t] &= !(flag::IN_FACTORY | flag::UPGRADE);
        units.flags[t] |= units.flags[row] & flag::REPEAT;
        units.rally[t] = units.rally[row];
        units.build_target[row] = Handle::NONE;
        // The footprint stays blocked: the successor stands exactly where this one stood.
        self.remove_unit_row(row, true)?;
        Ok(())
    }
}

fn order(kind: OrderKind, pos: FxVec2, target: Handle) -> Order {
    Order { kind, pos, target, blueprint: BlueprintId(0), heading: Angle::ZERO, offset: FxVec2::ZERO }
}
