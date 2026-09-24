//! Commands become orders; orders drive units.
//!
//! `apply_command` validates a player's command and edits order queues.
//! `run_orders` then advances the order at the front of every unit's queue.
//! Orders never move a unit themselves, except a factory rolling a finished
//! product out of its blocked bay: they set a destination and flags, and the
//! movement, economy and combat phases do the work.

use crate::command::{
    Command, PlayerCommand, MAX_BOMBARD_RADIUS, MAX_ORBIT_RADIUS, MAX_PATROL_POINTS,
    MIN_ORBIT_RADIUS,
};
use crate::mirror::SimEvent;
use crate::nav::Route;
use crate::spatial::kind;
use crate::tables::*;
use crate::world::snap_to_build_grid;
use crate::{SimError, World};
use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::{cat, BlueprintId, MoveLayer};

/// Longest production queue per factory.
const MAX_FACTORY_QUEUE: usize = 64;
/// Chasing units re-path once their quarry has moved this far from the field's goal.
pub(crate) const CHASE_REPATH_DISTANCE: Fx = Fx::from_int(96);
/// Shortest leash, metres, for a unit that goes after an enemy on its own.
const CHASE_LEASH_MIN: i32 = 24;
/// Build power of structures that upgrade themselves but are not builders.
pub(crate) const SELF_UPGRADE_POWER: Fx = Fx::from_int(10);
/// A build arm works once it points within this of its target (~4 degrees).
const WORK_AIM_TOLERANCE: u16 = 728;
const DT: i32 = TICKS_PER_SECOND as i32;
/// How far a followed orbit's centre may have moved on from where a `RelocateOrder`
/// found it and still be the one meant, metres: a fast unit's travel over the command delay.
pub(crate) const ORBIT_FOLLOW_SLACK: Fx = Fx::from_int(64);
/// A member of an arrived block has this long to find its exact slot before
/// it settles for near it (movement counts the ticks it spends jostling).
const SLOT_GRACE_TICKS: u16 = 2 * TICKS_PER_SECOND as u16;

/// One block of a group order: who goes, and where each stands relative to the centre.
pub(crate) struct FormationLayout {
    pub rows: Vec<usize>,
    /// Each row's slot, turned to `facing`.
    pub offsets: Vec<FxVec2>,
    pub facing: Angle,
    /// Where the block starts from.
    pub centroid: FxVec2,
    /// Where it ends up: the target, moved clear of map edges and parked hulls.
    pub center: FxVec2,
}

/// The way a patrol leg runs from `from` to `to`; `fallback` when they are one place.
fn leg_heading(from: FxVec2, to: FxVec2, fallback: Angle) -> Angle {
    if from == to {
        fallback
    } else {
        (to - from).angle()
    }
}

impl World {
    pub(crate) fn apply_command(&mut self, pc: &PlayerCommand) -> Result<(), SimError> {
        if pc.player as usize >= self.state.players.len() || self.apply_debug(pc)? {
            return Ok(());
        }
        // Test range: a slot may be steering another side.
        let player = if self.state.cheats {
            self.state.players[pc.player as usize].acts_as
        } else {
            pc.player
        };
        if self.state.players[player as usize].defeated {
            return Ok(());
        }
        self.take_standing(player, &pc.command);
        self.apply_as(player, &pc.command)
    }

    /// Carries out `command` as `player`'s. A factory's products are given its standing
    /// orders through here too.
    pub(crate) fn apply_as(&mut self, player: u8, command: &Command) -> Result<(), SimError> {
        match command {
            Command::FormationMove {
                units,
                target,
                queue,
                attack_move,
                together,
                spacing,
            } => self.order_formation(
                player,
                units,
                if *attack_move {
                    OrderKind::AttackMove
                } else {
                    OrderKind::Move
                },
                *target,
                *queue,
                *together,
                *spacing,
            ),
            Command::Move {
                units,
                target,
                queue,
            } => self.order_group(player, units, OrderKind::Move, *target, *queue),
            Command::AttackMove {
                units,
                target,
                queue,
            } => self.order_group(player, units, OrderKind::AttackMove, *target, *queue),
            Command::Attack {
                units,
                target,
                queue,
            } => {
                let Some(t) = self.state.units.row(*target) else {
                    return Ok(());
                };
                let pos = self.state.units.pos[t];
                for row in self.owned(player, units, 0) {
                    if self.are_enemies(player, self.state.units.owner[t])
                        && self.bp(row).can_attack(self.bp(t))
                    {
                        self.give(row, order(OrderKind::Attack, pos, *target), *queue)?;
                    }
                }
                Ok(())
            }
            Command::Orbit {
                units,
                pos,
                target,
                radius,
                queue,
            } => {
                let pos = self.clamp_to_map(*pos);
                let radius = if *radius > Fx::ZERO {
                    (*radius).clamp(MIN_ORBIT_RADIUS, MAX_ORBIT_RADIUS)
                } else {
                    Fx::ZERO
                };
                let target = self
                    .state
                    .units
                    .row(*target)
                    .filter(|&t| !self.are_enemies(player, self.state.units.owner[t]))
                    .map_or(Handle::NONE, |_| *target);
                let (flock, followed): (Vec<usize>, Vec<usize>) = self
                    .owned(player, units, cat::MOBILE)
                    .into_iter()
                    .filter(|&row| self.bp(row).orbit_radius > Fx::ZERO)
                    .partition(|&row| self.state.units.id(row) != target);
                // The unit being followed cannot circle itself: it circles the point.
                self.order_orbit(followed, pos, Handle::NONE, radius, *queue)?;
                self.order_orbit(flock, pos, target, radius, *queue)
            }
            Command::Stop { units } => {
                for row in self.owned_or_rising(player, units) {
                    self.clear_orders(row)?;
                    // An airbase stops guarding too.
                    self.state.units.guard[row].1 = Fx::ZERO;
                }
                Ok(())
            }
            Command::Build {
                units,
                blueprint,
                pos,
                heading,
                queue,
            } => {
                if blueprint.index() >= self.blueprints.units.len() {
                    return Ok(());
                }
                let bp = self.blueprints.unit(*blueprint);
                if !bp.built_on_site() {
                    return Ok(());
                }
                let site = snap_to_build_grid(bp, *pos);
                let rows: Vec<usize> = self
                    .owned(player, units, cat::MOBILE)
                    .into_iter()
                    .filter(|&row| {
                        self.bp(row)
                            .builder
                            .as_ref()
                            .is_some_and(|b| b.builds.contains(blueprint))
                    })
                    .collect();
                // Plans these builders are about to drop are not in the way.
                if self.plan_blocks(player, *blueprint, site, |row, _| {
                    !*queue && rows.contains(&row)
                }) {
                    self.events.push(SimEvent::BuildRejected { player });
                    return Ok(());
                }
                for row in rows {
                    let held = *queue
                        && self.state.orders.iter(&self.state.units, row).any(|o| {
                            o.kind == OrderKind::Build && o.pos == site && o.blueprint == *blueprint
                        });
                    if !held {
                        let mut o = order(OrderKind::Build, site, Handle::NONE);
                        o.blueprint = *blueprint;
                        o.heading = *heading;
                        self.give(row, o, *queue)?;
                    }
                }
                Ok(())
            }
            Command::Assist {
                units,
                target,
                queue,
            } => {
                let Some(t) = self.state.units.row(*target) else {
                    return Ok(());
                };
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
            Command::ReclaimWreck {
                units,
                wreck,
                queue,
            } => {
                let Some(w) = self.state.wrecks.slots.resolve(*wreck) else {
                    return Ok(());
                };
                let pos = self.state.wrecks.pos[w];
                for row in self.owned(player, units, 0) {
                    if self.bp(row).sends_reclaimers() {
                        self.give(row, order(OrderKind::Reclaim, pos, *wreck), *queue)?;
                    }
                }
                Ok(())
            }
            Command::ReclaimUnit {
                units,
                target,
                queue,
            } => {
                let Some(t) = self.state.units.row(*target) else {
                    return Ok(());
                };
                let pos = self.state.units.pos[t];
                for row in self.owned(player, units, 0) {
                    let bp = self.bp(row);
                    if bp.sends_reclaimers()
                        && (bp.drone.is_some() || self.can_reclaim_unit(row, t))
                    {
                        self.give(row, order(OrderKind::ReclaimUnit, pos, *target), *queue)?;
                    }
                }
                Ok(())
            }
            Command::Produce {
                factories,
                blueprint,
                count,
            } => {
                for row in self.owned_factories(player, factories) {
                    // Against the tier it will be by then: a queued upgrade opens its units.
                    let can = self
                        .blueprints
                        .unit(self.planned_loadout(row))
                        .builder
                        .as_ref()
                        .is_some_and(|b| b.builds.contains(blueprint));
                    if !can {
                        continue;
                    }
                    let queued = self.state.orders.iter(&self.state.units, row).count();
                    let room = MAX_FACTORY_QUEUE.saturating_sub(queued);
                    for _ in 0..(*count as usize).min(room) {
                        let mut o =
                            order(OrderKind::Produce, self.state.units.pos[row], Handle::NONE);
                        o.blueprint = *blueprint;
                        self.state.orders.push_back(&mut self.state.units, row, o)?;
                    }
                }
                Ok(())
            }
            Command::CancelProduce {
                factories,
                blueprint,
            } => {
                for row in self.owned_factories(player, factories) {
                    self.cancel_last_produce(row, *blueprint)?;
                }
                Ok(())
            }
            Command::SetRepeat { factories, repeat } => {
                for row in self.owned_factories(player, factories) {
                    let f = &mut self.state.units.flags[row];
                    *f = if *repeat {
                        *f | flag::REPEAT
                    } else {
                        *f & !flag::REPEAT
                    };
                }
                Ok(())
            }
            Command::SetRally { factories, pos } => {
                let pos = self.clamp_to_map(*pos);
                for row in self.owned_factories(player, factories) {
                    self.state.units.rally[row] = pos;
                }
                Ok(())
            }
            Command::CopyFactoryOrders { factories, from } => {
                self.copy_standing(player, factories, *from);
                Ok(())
            }
            Command::Upgrade { units } => {
                for row in self.owned(player, units, 0) {
                    // Each one queues the tier after the last already queued, as refits chain.
                    let Some(next) = self.blueprints.unit(self.planned_loadout(row)).upgrades_to
                    else {
                        continue;
                    };
                    // Free building (the test range) skips the tech a side must have reached.
                    if !self.state.players[player as usize].free_build
                        && self.blueprints.upgrade_needs(self.blueprints.unit(next))
                            > self.side_tech(player)
                    {
                        continue;
                    }
                    self.drop_all_but_builds(row)?;
                    let mut o = order(OrderKind::Upgrade, self.state.units.pos[row], Handle::NONE);
                    o.blueprint = next;
                    self.state.orders.push_back(&mut self.state.units, row, o)?;
                }
                Ok(())
            }
            Command::CancelUpgrade { units } => {
                for row in self.owned(player, units, 0) {
                    self.cancel_upgrade(row)?;
                }
                Ok(())
            }
            Command::Refit { units, kit } => {
                if self.blueprints.kit(*kit).is_none() {
                    return Ok(());
                }
                for row in self.owned(player, units, 0) {
                    let planned = self.planned_loadout(row);
                    if self.blueprints.refit_result(planned, *kit).is_err() {
                        continue;
                    }
                    self.drop_all_but_builds(row)?;
                    let mut o = order(OrderKind::Upgrade, self.state.units.pos[row], Handle::NONE);
                    o.blueprint = *kit;
                    self.state.orders.push_back(&mut self.state.units, row, o)?;
                }
                Ok(())
            }
            Command::CancelRefit { units, kit } => {
                for row in self.owned(player, units, 0) {
                    self.cancel_refit(row, *kit)?;
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
            Command::RelocateOrder {
                units,
                kind,
                from,
                to,
            } => self.relocate_orders(player, units, *kind, *from, *to),
            Command::SetFireState { units, state } => {
                for row in self.owned_or_rising(player, units) {
                    self.state.units.fire_state[row] = *state;
                    // Hold position stops a unit where it stands. Work in hand
                    // (building, reclaiming, a refit) carries on.
                    let travelling =
                        self.state
                            .orders
                            .front(&self.state.units, row)
                            .is_some_and(|o| {
                                matches!(
                                    o.kind,
                                    OrderKind::Move
                                        | OrderKind::AttackMove
                                        | OrderKind::Patrol
                                        | OrderKind::Attack
                                        | OrderKind::Orbit
                                )
                            });
                    if *state == FireState::HoldPosition && self.bp(row).is_mobile() && travelling {
                        self.clear_orders(row)?;
                    }
                }
                Ok(())
            }
            Command::AttackGround { units, pos, queue } => self.order_ground(
                player,
                units,
                OrderKind::AttackGround,
                *pos,
                Fx::ZERO,
                *queue,
            ),
            Command::Bombard {
                units,
                pos,
                radius,
                queue,
            } => {
                let radius = (*radius).clamp(Fx::ZERO, MAX_BOMBARD_RADIUS);
                self.order_ground(player, units, OrderKind::Bombard, *pos, radius, *queue)
            }
            Command::Patrol {
                units,
                points,
                queue,
            } => self.order_patrol(player, units, points, *queue),
            Command::PatrolInsert {
                units,
                after,
                point,
            } => self.insert_patrol_post(player, units, *after, *point),
            Command::CancelOrder { units, kind, pos } => {
                self.cancel_orders(player, units, *kind, *pos)
            }
            Command::Reform {
                units,
                together,
                spacing,
            } => self.reform(player, units, *together, *spacing),
            Command::SetDive { units, dive } => {
                self.set_dive(player, units, *dive);
                Ok(())
            }
            Command::SetPaused { units, paused } => {
                self.set_paused(player, units, *paused);
                Ok(())
            }
            Command::Guard {
                units,
                pos,
                radius,
                queue,
            } => self.order_guard(player, units, *pos, *radius, *queue),
            Command::Dock { units, base, queue } => self.order_dock(player, units, *base, *queue),
            Command::Launch {
                units,
                blueprint,
                count,
            } => {
                self.order_launch(player, units, *blueprint, *count);
                Ok(())
            }
            Command::SetAutoLand { units, on } => {
                self.set_auto_land(player, units, *on);
                Ok(())
            }
            Command::Board {
                units,
                carrier,
                queue,
            } => self.order_board(player, units, *carrier, *queue),
            Command::Land {
                units,
                pos,
                unload,
                queue,
            } => self.order_land(player, units, *pos, *unload, *queue),
            Command::Unload { units } => self.order_unload(player, units),
            Command::TakeOff { units } => self.order_take_off(player, units),

            // Handled above, before the issuing slot is looked at.
            Command::DebugSpawn { .. }
            | Command::DebugDamage { .. }
            | Command::DebugRemove { .. }
            | Command::DebugSetFlags { .. }
            | Command::DebugSetBuild { .. }
            | Command::DebugClear
            | Command::DebugControl { .. }
            | Command::DebugFreeBuild { .. }
            | Command::DebugStock { .. }
            | Command::DebugIncome { .. }
            | Command::DebugStorage { .. }
            | Command::DebugWrecks { .. } => Ok(()),
        }
    }

    /// Live, complete units in `ids` that `player` owns and that have all of `categories`.
    pub(crate) fn owned(&self, player: u8, ids: &[UnitId], categories: u32) -> Vec<usize> {
        let units = &self.state.units;
        ids.iter()
            .filter_map(|id| units.row(*id))
            .filter(|&row| {
                units.owner[row] == player
                    // Aircraft below an airbase take orders too: they are fired out to carry them out.
                    && (units.is_active(row) || units.hangar[row] != Handle::NONE)
                    && units.drone_parent[row] == Handle::NONE
                    && self.bp(row).visual.mesh != "reclaim_drone"
                    && self.bp(row).has(categories)
            })
            .collect()
    }

    /// Sites of the structures `player`'s builders have been ordered to build and have not begun.
    pub(crate) fn planned_sites(&self, player: u8) -> impl Iterator<Item = (usize, &Order)> {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(move |&row| {
                units.owner[row] == player
                    && self.bp(row).is_mobile()
                    && self.bp(row).builder.is_some()
            })
            .flat_map(move |row| {
                let begun = units.row(units.build_target[row]).is_some();
                self.state
                    .orders
                    .iter(units, row)
                    .enumerate()
                    .filter(move |(i, o)| o.kind == OrderKind::Build && !(*i == 0 && begun))
                    .map(move |(_, o)| (row, o))
            })
    }

    /// Whether `blueprint` at `site` would overlap a structure `player` has planned.
    /// The very same plan in another queue is not in the way: whoever gets there
    /// second joins in. `skip` names plans that do not count.
    fn plan_blocks(
        &self,
        player: u8,
        blueprint: BlueprintId,
        site: FxVec2,
        skip: impl Fn(usize, &Order) -> bool,
    ) -> bool {
        let bp = self.blueprints.unit(blueprint);
        self.planned_sites(player).any(|(row, o)| {
            if skip(row, o) || (o.pos == site && o.blueprint == blueprint) {
                return false;
            }
            let other = self.blueprints.unit(o.blueprint);
            let reach =
                |a: u8, b: u8| Fx::from_int((a as i32 + b as i32) * mc_map::BUILD_CELL_M) / 2;
            let d = o.pos - site;
            d.x.abs() < reach(bp.footprint.0, other.footprint.0)
                && d.y.abs() < reach(bp.footprint.1, other.footprint.1)
        })
    }

    /// Drags queued orders somewhere else: every `kind` order these units hold at
    /// `from` goes to `to`. A structure that has been begun stays where it is.
    fn relocate_orders(
        &mut self,
        player: u8,
        ids: &[UnitId],
        kind: OrderKind,
        from: FxVec2,
        to: FxVec2,
    ) -> Result<(), SimError> {
        if !matches!(
            kind,
            OrderKind::Move
                | OrderKind::AttackMove
                | OrderKind::Build
                | OrderKind::Patrol
                | OrderKind::AttackGround
                | OrderKind::Bombard
                | OrderKind::Orbit
                | OrderKind::Guard
        ) {
            return Ok(());
        }
        if kind == OrderKind::Guard {
            // An airbase's guard area is dragged about like a unit's, and kept in reach.
            for b in self.owned(player, ids, 0) {
                let (at, radius) = self.state.units.guard[b];
                if self.bp(b).airbase.is_some() && radius > Fx::ZERO && at == from {
                    self.set_airbase_guard(b, self.clamp_to_map(to), radius);
                }
            }
        }
        let rows = self.owned(player, ids, cat::MOBILE);
        let units = &self.state.units;
        // An orbit round a unit moves with it: by the time the drag arrives its centre
        // has gone on a little from where the player picked it up.
        let found = |o: &Order| {
            o.pos == from
                || (kind == OrderKind::Orbit
                    && units.row(o.target).is_some()
                    && o.pos.distance(from) <= ORBIT_FOLLOW_SLACK)
        };
        // (row, order node, the order is the one being carried out)
        let mut nodes: Vec<(usize, usize, bool)> = Vec::new();
        for &row in &rows {
            let begun = kind == OrderKind::Build && units.row(units.build_target[row]).is_some();
            for (i, node) in self.state.orders.nodes(units, row).enumerate() {
                let o = &self.state.orders.order[node as usize];
                if o.kind == kind && found(o) && !(i == 0 && begun) {
                    nodes.push((row, node as usize, i == 0));
                }
            }
        }
        let Some(&(_, first, _)) = nodes.first() else {
            return Ok(());
        };
        let to = if kind == OrderKind::Build {
            let blueprint = self.state.orders.order[first].blueprint;
            nodes.retain(|&(_, node, _)| self.state.orders.order[node].blueprint == blueprint);
            let site = snap_to_build_grid(self.blueprints.unit(blueprint), to);
            // The plans being moved are not in their own way; the same plan in a queue left alone is.
            let moved = |row: usize, o: &Order| {
                o.pos == from && o.blueprint == blueprint && nodes.iter().any(|&(r, ..)| r == row)
            };
            let fits = site == from
                || (self.can_place(self.blueprints.unit(blueprint), site)
                    && !self.plan_blocks(player, blueprint, site, moved));
            if !fits {
                self.events.push(SimEvent::BuildRejected { player });
                return Ok(());
            }
            site
        } else {
            self.clamp_to_map(to)
        };
        let mut moved_groups = std::collections::BTreeMap::new();
        for (row, node, current) in nodes {
            let previous = self.state.orders.order[node];
            if previous.formation != 0 && to != from {
                let (id, heading) = *moved_groups.entry(previous.formation).or_insert_with(|| {
                    let anchor = self
                        .state
                        .formations
                        .get(&previous.formation)
                        .map_or(self.state.units.pos[row], |g| g.anchor);
                    let heading = (to - anchor).angle();
                    self.state.formation_serial += 1;
                    let id = self.state.formation_serial;
                    self.state.formations.insert(
                        id,
                        crate::formations::Group {
                            anchor,
                            heading,
                            phase: 0,
                            speed: Fx::ZERO,
                        },
                    );
                    (id, heading)
                });
                let o = &mut self.state.orders.order[node];
                o.formation = id;
                o.offset = previous.offset.rotate(heading - previous.heading);
                o.heading = heading;
            }
            self.state.orders.order[node].pos = to;
            // Put down somewhere, an orbit circles that spot and no longer follows anyone.
            if kind == OrderKind::Orbit {
                self.state.orders.order[node].target = Handle::NONE;
            }
            if current {
                // The way there is asked for again when the order next runs.
                self.state.units.stuck_ticks[row] = 0;
            }
        }
        Ok(())
    }

    /// `AttackGround` or `Bombard` for every unit with a weapon that can hit the ground.
    fn order_ground(
        &mut self,
        player: u8,
        ids: &[UnitId],
        kind: OrderKind,
        pos: FxVec2,
        radius: Fx,
        queue: bool,
    ) -> Result<(), SimError> {
        let pos = self.clamp_to_map(pos);
        for row in self.owned(player, ids, 0) {
            if self.bp(row).weapons.iter().any(crate::combat::hits_ground) {
                let mut o = order(kind, pos, Handle::NONE);
                o.radius = radius;
                self.give(row, o, queue)?;
            }
        }
        Ok(())
    }

    /// A patrol loop walked in formation: one `Patrol` order per post, each leg its own
    /// command group facing along the leg. With one post the loop runs out to it and
    /// back to where the group stands, or, queued, where its queue leaves it.
    fn order_patrol(
        &mut self,
        player: u8,
        ids: &[UnitId],
        points: &[FxVec2],
        queue: bool,
    ) -> Result<(), SimError> {
        let rows = self.owned(player, ids, cat::MOBILE);
        if rows.is_empty() || points.is_empty() {
            return Ok(());
        }
        let points: Vec<FxVec2> = points
            .iter()
            .take(MAX_PATROL_POINTS)
            .map(|&p| self.clamp_to_map(p))
            .collect();
        for layout in self.formation_layouts(rows, points[0], queue, 1) {
            let mut route = points.clone();
            if route.len() == 1 {
                route.push(layout.centroid);
            }
            let together = layout.rows.len() > 1;
            // Each leg faces the way it runs, from the post before it round the loop.
            let legs: Vec<(u64, Angle)> = (0..route.len())
                .map(|i| {
                    let from = route[(i + route.len() - 1) % route.len()];
                    let heading = leg_heading(from, route[i], layout.facing);
                    let formation = if together {
                        self.new_formation(from, heading)
                    } else {
                        0
                    };
                    (formation, heading)
                })
                .collect();
            for (&row, offset) in layout.rows.iter().zip(&layout.offsets) {
                // The slot as laid out facing the first post, turned with every leg.
                let slot = offset.rotate(-layout.facing);
                for (i, (&p, &(formation, heading))) in route.iter().zip(&legs).enumerate() {
                    let mut o = order(OrderKind::Patrol, p, Handle::NONE);
                    o.formation = formation;
                    o.heading = heading;
                    o.offset = slot.rotate(heading);
                    self.give(row, o, queue || i > 0)?;
                }
            }
        }
        Ok(())
    }

    /// Puts `point` into these units' patrol loops, right after their post at `after`.
    /// The leg out of the new post, to the post that followed `after`, turns to match.
    fn insert_patrol_post(
        &mut self,
        player: u8,
        ids: &[UnitId],
        after: FxVec2,
        point: FxVec2,
    ) -> Result<(), SimError> {
        let point = self.clamp_to_map(point);
        // The leg into `after`, by group: the new leg's group, shared the same way.
        let mut groups = std::collections::BTreeMap::<u64, u64>::new();
        for row in self.owned(player, ids, cat::MOBILE) {
            let mut queue: Vec<Order> = self
                .state
                .orders
                .iter(&self.state.units, row)
                .copied()
                .collect();
            let patrol: Vec<usize> = (0..queue.len())
                .filter(|&i| queue[i].kind == OrderKind::Patrol)
                .collect();
            if patrol.len() >= MAX_PATROL_POINTS {
                continue;
            }
            let Some(at) = patrol.iter().position(|&i| queue[i].pos == after) else {
                continue;
            };
            let (i, next) = (patrol[at], patrol[(at + 1) % patrol.len()]);
            let prev = queue[i];
            let slot = prev.offset.rotate(-prev.heading);
            let heading = leg_heading(after, point, prev.heading);
            let mut o = order(OrderKind::Patrol, point, Handle::NONE);
            o.heading = heading;
            o.offset = slot.rotate(heading);
            if prev.formation != 0 {
                o.formation = match groups.get(&prev.formation) {
                    Some(&id) => id,
                    None => {
                        let id = self.new_formation(after, heading);
                        groups.insert(prev.formation, id);
                        id
                    }
                };
            }
            if next != i {
                let n = &mut queue[next];
                let turned = leg_heading(point, n.pos, n.heading);
                n.offset = n.offset.rotate(-n.heading).rotate(turned);
                n.heading = turned;
            }
            queue.insert(i + 1, o);
            // The front order stays in front: whatever it is doing carries on.
            self.state.orders.clear(&mut self.state.units, row);
            for o in queue {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        Ok(())
    }

    /// Takes every `kind` order these units hold at exactly `pos` out of their queues.
    /// A structure already begun is left standing, as `Stop` leaves it; a product or
    /// refit under way is scrapped, as `CancelProduce` and `CancelUpgrade` scrap it.
    fn cancel_orders(
        &mut self,
        player: u8,
        ids: &[UnitId],
        kind: OrderKind,
        pos: FxVec2,
    ) -> Result<(), SimError> {
        let doomed = |o: &Order| o.kind == kind && o.pos == pos;
        if kind == OrderKind::Guard {
            // An airbase's guard, taken off.
            for row in self.owned(player, ids, 0) {
                if self.state.units.guard[row].0 == pos {
                    self.state.units.guard[row].1 = Fx::ZERO;
                }
            }
        }
        for row in self.owned_or_rising(player, ids) {
            let queue: Vec<Order> = self
                .state
                .orders
                .iter(&self.state.units, row)
                .copied()
                .collect();
            if !queue.iter().any(doomed) {
                continue;
            }
            if doomed(&queue[0]) {
                // The order being carried out: whatever comes next starts afresh.
                if matches!(kind, OrderKind::Produce | OrderKind::Upgrade) {
                    self.abort_product(row)?;
                }
                self.state.units.build_target[row] = Handle::NONE;
                self.stop_moving(row);
            }
            self.state.orders.clear(&mut self.state.units, row);
            for o in queue.into_iter().filter(|o| !doomed(o)) {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        Ok(())
    }

    pub(crate) fn give(&mut self, row: usize, o: Order, queue: bool) -> Result<(), SimError> {
        if !queue {
            // A unit being refitted is pinned until the refit is done: a new order replaces what
            // was queued behind the refit, not the refit. Only `Stop` and `CancelUpgrade` end one.
            let refit = self
                .state
                .orders
                .front(&self.state.units, row)
                .copied()
                .filter(|f| f.kind == OrderKind::Upgrade && self.upgrades_in_place(row));
            match refit {
                Some(refit) => {
                    self.state.orders.clear(&mut self.state.units, row);
                    self.state
                        .orders
                        .push_back(&mut self.state.units, row, refit)?;
                }
                None => self.clear_orders(row)?,
            }
        }
        self.state.orders.push_back(&mut self.state.units, row, o)
    }

    /// Independent blocks for ground layers, repeating Vs for each air altitude.
    /// Slot assignment is spatial and stable, independent of selection order.
    fn order_group(
        &mut self,
        player: u8,
        ids: &[UnitId],
        kind: OrderKind,
        target: FxVec2,
        queue: bool,
    ) -> Result<(), SimError> {
        self.order_formation(player, ids, kind, target, queue, true, 1)
    }

    fn order_formation(
        &mut self,
        player: u8,
        ids: &[UnitId],
        kind: OrderKind,
        target: FxVec2,
        queue: bool,
        together: bool,
        spacing_level: u8,
    ) -> Result<(), SimError> {
        let rows = self.owned(player, ids, cat::MOBILE);
        let target = self.clamp_to_map(target);
        for layout in self.formation_layouts(rows, target, queue, spacing_level) {
            let n = layout.rows.len();
            let formation = if together && n > 1 {
                self.new_formation(layout.centroid, layout.facing)
            } else {
                0
            };
            for (row, offset) in layout.rows.into_iter().zip(layout.offsets) {
                let mut o = order(kind, layout.center, Handle::NONE);
                o.formation = formation;
                o.offset = offset;
                o.heading = layout.facing;
                self.give(row, o, queue)?;
            }
        }
        Ok(())
    }

    /// A new command group, not yet formed up.
    fn new_formation(&mut self, anchor: FxVec2, heading: Angle) -> u64 {
        self.state.formation_serial += 1;
        let id = self.state.formation_serial;
        self.state.formations.insert(
            id,
            crate::formations::Group {
                anchor,
                heading,
                phase: 0,
                speed: Fx::ZERO,
            },
        );
        id
    }

    /// Where each of `rows` stands in a group ordered to `target`: independent blocks
    /// for ground layers, repeating Vs for each air altitude. Slot assignment is
    /// spatial and stable, independent of selection order. With `queue`, the group
    /// starts from where its queues end.
    pub(crate) fn formation_layouts(
        &self,
        rows: Vec<usize>,
        target: FxVec2,
        queue: bool,
        spacing_level: u8,
    ) -> Vec<FormationLayout> {
        let mut out = Vec::new();
        let mut groups = std::collections::BTreeMap::<(u8, i64), Vec<usize>>::new();
        for row in rows {
            let m = self.bp(row).motion.expect("mobile");
            groups
                .entry((
                    if m.layer == MoveLayer::Air {
                        2
                    } else if m.layer == MoveLayer::Naval {
                        1
                    } else {
                        0
                    },
                    if m.layer == MoveLayer::Air {
                        m.altitude.0
                    } else {
                        0
                    },
                ))
                .or_default()
                .push(row);
        }
        for (_, mut rows) in groups {
            rows.sort_unstable();
            rows.dedup();
            let n = rows.len() as i32;
            let source = |row: usize| {
                if queue {
                    self.state
                        .orders
                        .iter(&self.state.units, row)
                        .last()
                        .map(|o| o.pos + o.offset)
                        .unwrap_or(self.state.units.pos[row])
                } else {
                    self.state.units.pos[row]
                }
            };
            let mut centroid = FxVec2::ZERO;
            let mut spacing = Fx::ZERO;
            for &row in &rows {
                centroid += source(row);
                spacing = spacing.max(self.bp(row).radius * 2 + Fx::from_int(6));
            }
            centroid = FxVec2::new(centroid.x / n, centroid.y / n);
            let facing = if centroid == target {
                self.state.units.heading[rows[0]]
            } else {
                (target - centroid).angle()
            };
            let air = self.is_air(rows[0]);
            spacing = spacing
                * match spacing_level.min(2) {
                    0 => Fx::ONE,
                    1 => Fx::ratio(5, 4),
                    _ => Fx::ratio(7, 4),
                };
            let offsets = crate::formations::slots(n as usize, spacing, air);
            // Shift the whole layout at map edges instead of crushing individual slots.
            let rotated: Vec<_> = offsets.iter().map(|p| p.rotate(facing)).collect();
            let size = self.terrain.size_metres();
            let min_x = rotated.iter().map(|p| p.x).min().unwrap();
            let max_x = rotated.iter().map(|p| p.x).max().unwrap();
            let min_y = rotated.iter().map(|p| p.y).min().unwrap();
            let max_y = rotated.iter().map(|p| p.y).max().unwrap();
            let fit = |v: Fx, lo: Fx, hi: Fx, size: Fx| {
                let a = Fx::from_int(4) - lo;
                let b = size - Fx::from_int(4) - hi;
                if a <= b {
                    v.clamp(a, b)
                } else {
                    size / 2
                }
            };
            let center = FxVec2::new(
                fit(target.x, min_x, max_x, size.x),
                fit(target.y, min_y, max_y, size.y),
            );
            let center = self.clear_formation_destination(center, &rotated, &rows, spacing);
            // Sort ranks front-to-back, then left-to-right. This is O(n log n),
            // avoids selection-order crossings, and keeps large armies affordable.
            rows.sort_by_key(|&row| {
                let p = (source(row) - centroid).rotate(-facing);
                (-p.x.0.div_euclid(spacing.0), -p.y.0, row)
            });
            let mut slots: Vec<_> = (0..rotated.len()).collect();
            slots.sort_by_key(|&i| (-offsets[i].x.0, -offsets[i].y.0, i));
            // Remove crossing assignments before issuing the order. Pair swaps
            // strictly reduce squared travel, with fixed iteration order for replay.
            // Bound work for very large selections.
            if rows.len() <= 256 {
                for _ in 0..4 {
                    let mut changed = false;
                    for a in 0..rows.len() {
                        for b in a + 1..rows.len() {
                            let pa = source(rows[a]) - centroid;
                            let pb = source(rows[b]) - centroid;
                            let oa = rotated[slots[a]];
                            let ob = rotated[slots[b]];
                            if (pa - pb).dot(oa - ob) < Fx::ZERO {
                                slots.swap(a, b);
                                changed = true;
                            }
                        }
                    }
                    if !changed {
                        break;
                    }
                }
            }
            out.push(FormationLayout {
                offsets: slots.into_iter().map(|slot| rotated[slot]).collect(),
                rows,
                facing,
                centroid,
                center,
            });
        }
        out
    }

    /// Keep a layout intact when the clicked ground is occupied by a parked
    /// hull or a structure. Search whole-layout translations, not stacked slots.
    fn clear_formation_destination(
        &self,
        center: FxVec2,
        offsets: &[FxVec2],
        rows: &[usize],
        spacing: Fx,
    ) -> FxVec2 {
        let selected: std::collections::BTreeSet<_> = rows.iter().copied().collect();
        let radius = rows
            .iter()
            .map(|&r| self.bp(r).radius)
            .max()
            .unwrap_or(Fx::ONE);
        let size = rows
            .iter()
            .map(|&r| self.bp(r).motion.unwrap().size_class)
            .max()
            .unwrap_or(0);
        let first = self.bp(rows[0]).motion.unwrap();
        let layer = if rows
            .iter()
            .any(|&r| self.bp(r).motion.unwrap().layer == MoveLayer::Land)
        {
            MoveLayer::Land
        } else {
            first.layer
        };
        let air = layer == MoveLayer::Air;
        let free = |candidate: FxVec2| {
            offsets.iter().all(|offset| {
                let pos = candidate + *offset;
                if !self.terrain.in_bounds(pos) || !self.nav.passable(layer, size, pos) {
                    return false;
                }
                let mut clear = true;
                self.index
                    .query(pos, radius + Fx::from_int(6), kind::UNIT, |e| {
                        let other = e.row as usize;
                        if selected.contains(&other) || !self.unit_entry_is_current(e) {
                            return true;
                        }
                        let Some(m) = self.bp(other).motion else {
                            return true;
                        };
                        if (m.layer == MoveLayer::Air) != air
                            || self.state.units.has_flag(other, flag::IN_FACTORY)
                        {
                            return true;
                        }
                        if self.state.units.has_flag(other, flag::HAS_FIELD) {
                            return true;
                        }
                        // Ships and dived submarines may share a spot, one under the other.
                        if rows.iter().all(|&r| self.hulls_pass(r, other)) {
                            return true;
                        }
                        if air
                            && (m.altitude - first.altitude).abs() > self.bp(other).height + Fx::ONE
                        {
                            return true;
                        }
                        clear = pos.distance(e.pos) >= radius + e.radius + Fx::from_int(6);
                        clear
                    });
                clear
            })
        };
        if free(center) {
            return center;
        }
        let sum = rows
            .iter()
            .fold(FxVec2::ZERO, |p, &r| p + self.state.units.pos[r]);
        let from = FxVec2::new(sum.x / rows.len() as i32, sum.y / rows.len() as i32);
        for ring in 1..=10 {
            let mut best = None;
            for spoke in 0..16 {
                let shift = FxVec2::from_angle(Angle(spoke * 4096))
                    * (spacing.max(Fx::from_int(12)) * ring);
                let candidate = center + shift;
                if free(candidate)
                    && best.is_none_or(|p: FxVec2| candidate.distance(from) < p.distance(from))
                {
                    best = Some(candidate);
                }
            }
            if let Some(candidate) = best {
                return candidate;
            }
        }
        center
    }

    fn cancel_last_produce(&mut self, row: usize, blueprint: BlueprintId) -> Result<(), SimError> {
        let queue: Vec<Order> = self
            .state
            .orders
            .iter(&self.state.units, row)
            .copied()
            .collect();
        let Some(last) = queue
            .iter()
            .rposition(|o| o.kind == OrderKind::Produce && o.blueprint == blueprint)
        else {
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

    /// The blueprint a unit will have once the upgrades and refits in its queue are done.
    pub(crate) fn planned_loadout(&self, row: usize) -> BlueprintId {
        let mut at = self.state.units.blueprint[row];
        for o in self.state.orders.iter(&self.state.units, row) {
            if o.kind == OrderKind::Upgrade {
                at = self.after_upgrade(at, o.blueprint).unwrap_or(at);
            }
        }
        at
    }

    /// What `at` becomes through the queued upgrade to `to` (a tier or a refit kit), if it can take it.
    fn after_upgrade(&self, at: BlueprintId, to: BlueprintId) -> Option<BlueprintId> {
        if self.blueprints.kit(to).is_some() {
            self.blueprints.refit_result(at, to).ok()
        } else {
            (self.blueprints.unit(at).upgrades_to == Some(to)).then_some(to)
        }
    }

    /// A commander told to upgrade drops what it was doing (moves, attacks, assists...) and
    /// keeps only its build queue and the upgrades already queued, which the new one follows.
    fn drop_all_but_builds(&mut self, row: usize) -> Result<(), SimError> {
        if !self.bp(row).has(cat::COMMANDER) {
            return Ok(());
        }
        let keep = |o: &Order| matches!(o.kind, OrderKind::Build | OrderKind::Upgrade);
        let queue: Vec<Order> = self
            .state
            .orders
            .iter(&self.state.units, row)
            .copied()
            .collect();
        if queue.iter().all(keep) {
            return Ok(());
        }
        if !queue.first().is_some_and(keep) {
            // The order under way is dropped: stop walking, leave any site it was helping standing.
            self.stop_moving(row);
            self.state.units.build_target[row] = Handle::NONE;
        }
        self.state.orders.clear(&mut self.state.units, row);
        for o in queue.into_iter().filter(keep) {
            self.state.orders.push_back(&mut self.state.units, row, o)?;
        }
        Ok(())
    }

    fn cancel_refit(&mut self, row: usize, kit: BlueprintId) -> Result<(), SimError> {
        let Some(at) = self
            .state
            .orders
            .iter(&self.state.units, row)
            .position(|o| o.kind == OrderKind::Upgrade && o.blueprint == kit)
        else {
            return Ok(());
        };
        self.cancel_upgrade_at(row, at)
    }

    fn cancel_upgrade(&mut self, row: usize) -> Result<(), SimError> {
        let Some(at) = self
            .state
            .orders
            .iter(&self.state.units, row)
            .position(|o| o.kind == OrderKind::Upgrade)
        else {
            return Ok(());
        };
        self.cancel_upgrade_at(row, at)
    }

    /// Takes the upgrade at `at` in the unit's queue out, and every upgrade, refit and
    /// factory order after it that relied on what it would have made.
    fn cancel_upgrade_at(&mut self, row: usize, at: usize) -> Result<(), SimError> {
        let queue: Vec<Order> = self
            .state
            .orders
            .iter(&self.state.units, row)
            .copied()
            .collect();
        let mut loadout = self.state.units.blueprint[row];
        let mut keep = vec![true; queue.len()];
        keep[at] = false;
        for (i, o) in queue.iter().enumerate() {
            match o.kind {
                OrderKind::Upgrade => match self.after_upgrade(loadout, o.blueprint) {
                    Some(next) if keep[i] => loadout = next,
                    Some(_) => {}
                    None => keep[i] = false,
                },
                OrderKind::Produce => {
                    let can = self
                        .blueprints
                        .unit(loadout)
                        .builder
                        .as_ref()
                        .is_some_and(|b| b.builds.contains(&o.blueprint));
                    keep[i] &= can;
                }
                _ => {}
            }
        }
        if !keep[0] {
            self.abort_product(row)?;
        }
        self.state.orders.clear(&mut self.state.units, row);
        for (i, o) in queue.into_iter().enumerate() {
            if keep[i] {
                self.state.orders.push_back(&mut self.state.units, row, o)?;
            }
        }
        Ok(())
    }

    /// Removes the half-built unit or upgrade a factory, structure or refitting unit is working on.
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
        // An engineer's `build_target` is somebody's site and is left standing; a refit is its own.
        let refitting = self
            .state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| o.kind == OrderKind::Upgrade);
        self.state.orders.clear(&mut self.state.units, row);
        self.stop_moving(row);
        if self.bp(row).is_structure() || refitting {
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
        units.flags[row] &= !flag::AIR_RUN;
        units.air_turn_ticks[row] = 0;
        units.air_break_ticks[row] = 0;
        units.move_goal[row] = units.pos[row];
        units.stuck_ticks[row] = 0;
    }

    /// Points the unit at `own_goal`, following the shared field toward `field_goal`.
    pub(crate) fn ensure_moving(
        &mut self,
        row: usize,
        field_goal: FxVec2,
        own_goal: FxVec2,
    ) -> Result<(), SimError> {
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
        if motion.layer == MoveLayer::Air {
            // Air flies a straight line: the nav grid is for hulls that cannot
            // cross water, slopes or structures.
            let units = &mut self.state.units;
            units.field[row] = NO_FIELD;
            units.field_goal[row] = field_goal;
            units.flags[row] |= flag::HAS_FIELD;
            units.move_goal[row] = own_goal;
            return Ok(());
        }
        let id = match self.nav.request(
            motion.layer,
            motion.size_class,
            field_goal,
            self.state.units.pos[row],
        )? {
            Route::Field(id) => id,
            Route::Unreachable => {
                // Nowhere near the goal can be stood on; the order gives up next tick.
                self.state.units.stuck_ticks[row] = u16::MAX;
                return Ok(());
            }
            // No field to spare this tick: stand, and ask again on the next.
            Route::Busy => return Ok(()),
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
        FxVec2::new(
            p.x.clamp(margin, size.x - margin),
            p.y.clamp(margin, size.y - margin),
        )
    }

    pub(crate) fn run_orders(&mut self) -> Result<(), SimError> {
        // Units spawned while orders run start acting next tick.
        let rows = self.state.units.slots.rows();
        for row in 0..rows {
            if !self.state.units.slots.is_alive(row) || !self.state.units.is_active(row) {
                continue;
            }
            if self.state.units.drone_parent[row] != Handle::NONE {
                continue;
            }
            let Some(o) = self.state.orders.front(&self.state.units, row).copied() else {
                if self.is_air(row)
                    && !self.bp(row).weapons.is_empty()
                    && !self.state.units.has_flag(row, flag::PASSIVE)
                {
                    if let Some(target) = self.air_engage_target(row) {
                        // Idle armed aircraft launch a flight before using their weapons.
                        let mut home = order(
                            OrderKind::AttackMove,
                            self.state.units.pos[row],
                            Handle::NONE,
                        );
                        home.heading = self.state.units.heading[row];
                        self.give(row, home, false)?;
                        self.air_fight(row, target)?;
                        continue;
                    }
                }
                if self.state.units.has_flag(row, flag::HAS_FIELD) {
                    self.stop_moving(row);
                }
                if self.idle_chase(row)? || self.idle_air_land(row)? {
                    continue;
                }
                if !self.idle_repair(row)? {
                    self.idle_reclaim(row)?;
                }
                continue;
            };
            match o.kind {
                OrderKind::Move | OrderKind::AttackMove => self.run_move(row, &o)?,
                OrderKind::Attack => self.run_attack(row, &o)?,
                OrderKind::Build => self.run_build(row, &o)?,
                OrderKind::Assist => self.run_assist(row, &o)?,
                OrderKind::Reclaim if self.bp(row).drone.is_some() => {}
                OrderKind::Reclaim => self.run_reclaim(row, &o)?,
                OrderKind::ReclaimUnit if self.bp(row).drone.is_some() => {}
                OrderKind::ReclaimUnit => self.run_reclaim_unit(row, &o)?,
                OrderKind::Produce => self.run_produce(row, &o)?,
                OrderKind::Upgrade => self.run_upgrade(row, &o)?,
                OrderKind::Orbit => self.run_orbit(row, &o)?,
                OrderKind::AttackGround | OrderKind::Bombard => self.run_attack_ground(row, &o)?,
                OrderKind::Patrol => self.run_patrol(row, &o)?,
                OrderKind::Guard => self.run_guard(row, &o)?,
                OrderKind::Dock => self.run_dock(row, &o)?,
                OrderKind::Board => self.run_board(row, &o)?,
                OrderKind::Land | OrderKind::Unload => self.run_land(row, &o)?,
            }
        }
        Ok(())
    }

    pub(crate) fn finish_order(&mut self, row: usize) {
        self.state.orders.pop_front(&mut self.state.units, row);
        self.state.units.stuck_ticks[row] = 0;
        // Keep the field when the next order heads to the same place; otherwise drop it.
        let next_same = self
            .state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|n| {
                matches!(n.kind, OrderKind::Move | OrderKind::AttackMove)
                    && n.pos == self.state.units.field_goal[row]
            });
        if !next_same {
            self.stop_moving(row);
        }
    }

    fn run_move(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if self.is_air(row) {
            return self.run_air_move(row, o);
        }
        let goal = self.clamp_to_map(o.pos + o.offset);
        let units = &self.state.units;
        // Settle for "close" sooner the longer the unit has been jostling for its slot.
        let tolerance = self.bp(row).radius / 2
            + Fx::from_int(2)
            + Fx::from_int(units.stuck_ticks[row] as i32) / 2;
        if units.stuck_ticks[row] == u16::MAX {
            self.finish_order(row);
            return Ok(());
        }
        let group_arrived = o.formation == 0
            || self
                .state
                .formations
                .get(&o.formation)
                .is_none_or(|g| g.anchor.distance(o.pos) <= Fx::HALF);
        // A slot is exact, until jostling for it has gone on a couple of seconds.
        let tolerance = if o.formation != 0 {
            Fx::ONE
                + Fx::from_int(units.stuck_ticks[row].saturating_sub(SLOT_GRACE_TICKS) as i32) / 4
        } else {
            tolerance
        };
        if group_arrived && units.pos[row].distance(goal) <= tolerance {
            let turn = self.bp(row).motion.expect("mobile").turn_rate;
            self.state.units.flags[row] |= flag::HOLD;
            self.state.units.speed[row] = Fx::ZERO;
            self.state.units.heading[row] =
                self.state.units.heading[row].turn_toward(o.heading, turn);
            if self.state.units.heading[row] == o.heading {
                self.finish_order(row);
            }
            return Ok(());
        }
        if o.kind == OrderKind::AttackMove && self.has_clear_target(row) {
            self.state.units.flags[row] |= flag::HOLD;
        }
        self.ensure_moving(row, o.pos, goal)
    }

    /// In combat they fly a run or a pass; otherwise they settle at the waypoint.
    fn run_air_move(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if o.kind == OrderKind::AttackMove {
            if self.air_search_last_seen(row, o.target)? {
                return Ok(());
            }
            if let Some(t) = self.air_engage_target(row) {
                return self.air_fight(row, t);
            }
        }
        let goal = self.clamp_to_map(o.pos + o.offset);
        let motion = self.bp(row).motion.expect("air");
        // Dock into the assigned slot before dropping the order. Cruise-speed
        // waypoint tolerances leave a braking-distance-sized hole in a V.
        let group_arrived = o.formation == 0
            || self
                .state
                .formations
                .get(&o.formation)
                .is_none_or(|g| g.anchor.distance(o.pos) <= Fx::HALF);
        if group_arrived
            && self.state.units.pos[row].distance(goal) <= Fx::HALF
            && self.state.units.speed[row] <= motion.accel / DT
        {
            self.state.units.speed[row] = Fx::ZERO;
            self.state.units.heading[row] =
                self.state.units.heading[row].turn_toward(o.heading, motion.turn_rate);
            if self.state.units.heading[row] == o.heading {
                self.finish_order(row);
            }
            return Ok(());
        }
        self.state.units.flags[row] &= !flag::AIR_RUN;
        self.ensure_moving(row, o.pos, goal)
    }

    pub(crate) fn has_live_target(&self, row: usize) -> bool {
        let units = &self.state.units;
        units.weapon_target[row]
            .iter()
            .any(|t| units.row(*t).is_some())
    }

    pub(crate) fn is_air(&self, row: usize) -> bool {
        self.bp(row)
            .motion
            .is_some_and(|m| m.layer == MoveLayer::Air)
    }

    pub(crate) fn air_engage_target(&self, row: usize) -> Option<usize> {
        let bp = self.bp(row);
        // A lift ship never flies at anything: its guns shoot what comes in reach.
        if bp.weapons.is_empty() || bp.transport.is_some() {
            return None;
        }
        let units = &self.state.units;
        // Holding position, it only flies at what an order names: no sorties,
        // no breaking off a route.
        let chases = self.chases(row);
        if let Some(o) = self.state.orders.front(units, row) {
            if o.kind == OrderKind::Attack
                || (chases && matches!(o.kind, OrderKind::AttackMove | OrderKind::Patrol))
            {
                if let Some(t) = units.row(o.target) {
                    if self.air_can_harass(row, t) {
                        return Some(t);
                    }
                }
            }
        }
        if !chases {
            return None;
        }
        for id in units.weapon_target[row] {
            if let Some(t) = units.row(id) {
                if self.air_can_harass(row, t) {
                    return Some(t);
                }
            }
        }
        let mask = bp.weapons.iter().fold(0u32, |acc, w| acc | w.target_mask);
        if mask == 0 {
            return None;
        }
        self.index
            .nearest(units.pos[row], bp.vision, kind::UNIT, |e| {
                self.unit_entry_is_current(e)
                    && self.air_can_harass(row, e.row as usize)
                    && self.hittable(e.row as usize, mask)
                    && self.fires_at_will(row)
            })
            .map(|e| e.row as usize)
    }

    fn air_can_harass(&self, shooter: usize, target: usize) -> bool {
        let units = &self.state.units;
        if !units.slots.is_alive(target) || units.has_flag(target, flag::IN_FACTORY) {
            return false;
        }
        // `can_strike`: a torpedo bomber goes after a dived hull its sonar hears.
        self.are_enemies(units.owner[shooter], units.owner[target])
            && self.can_strike(shooter, target)
            && self.detects(units.owner[shooter], target)
    }

    pub(crate) fn run_orbit(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        // An orbit never ends by itself: anything queued behind it takes over at once.
        if self
            .state
            .orders
            .iter(&self.state.units, row)
            .nth(1)
            .is_some()
        {
            self.finish_order(row);
            return Ok(());
        }
        let center = self
            .state
            .units
            .row(o.target)
            .map_or(o.pos, |t| self.state.units.pos[t]);
        // Keep the last centre when the followed unit disappears.
        let head = self.state.units.order_head[row];
        if head != NO_ORDER {
            self.state.orders.order[head as usize].pos = center;
        }
        let radius = if o.radius > Fx::ZERO {
            o.radius
        } else {
            self.bp(row).orbit_radius
        };
        // As on patrol: break off for an enemy near the circle, then come back to it.
        let reach = radius + self.bp(row).vision;
        if let Some(t) = self
            .air_engage_target(row)
            .filter(|&t| self.state.units.pos[t].distance(center) <= reach)
        {
            return self.air_fight(row, t);
        }
        if o.formation != 0 {
            // The group's motion flies the circle; this keeps the member under way.
            let slot = self.orbit_slot(o).unwrap_or(center);
            self.ensure_moving(row, center, slot)?;
            self.state.units.flags[row] &= !flag::AIR_RUN;
            return Ok(());
        }
        let radial = self.state.units.pos[row] - center;
        let bearing = if radial.length() < Fx::ONE {
            self.state.units.heading[row]
        } else {
            radial.angle()
        };
        let lead = Angle::from_degrees(35);
        let goal = self.clamp_to_map(
            center + FxVec2::from_angle(bearing + lead) * crate::orbit::chase_radius(radius, lead),
        );
        self.ensure_moving(row, goal, goal)?;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    pub(crate) fn air_fight(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        self.state.units.stuck_ticks[row] = 0;
        self.state.units.air_aim[row] = self.state.units.pos[target];
        let head = self.state.units.order_head[row];
        if head != NO_ORDER
            && matches!(
                self.state.orders.order[head as usize].kind,
                OrderKind::AttackMove | OrderKind::Patrol
            )
        {
            // Keep the detected target through the full egress/return arc, even
            // outside acquisition range. Hidden targets are searched for at
            // their last detected position; their hidden movement is not read.
            self.state.orders.order[head as usize].target = self.state.units.id(target);
        }
        if self.bp(row).is_capital_ship() {
            self.capital_engage(row, target)
        } else if self.bp(row).motion.is_some_and(|m| m.hover) {
            self.air_hover_standoff(row, self.state.units.pos[target])
        } else if self.bp(row).has(cat::ANTI_AIR) {
            self.air_dogfight(row, target)
        } else {
            self.air_bomb_run(row, target)
        }
    }

    /// A capital ship (`is_capital_ship`) never wheels about a target: its turrets
    /// cover every side. It holds where it is once the target is within reach, and
    /// otherwise closes straight in until it is.
    fn capital_engage(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        self.state.units.flags[row] &= !flag::AIR_RUN;
        let (pos, at) = (self.state.units.pos[row], self.state.units.pos[target]);
        self.state.units.air_aim[row] = at;
        let reach = self.bp(row).max_weapon_range();
        if pos.distance(at) <= reach * Fx::ratio(4, 5) {
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            return Ok(());
        }
        let back = (pos - at).normalize();
        let goal = self.clamp_to_map(at + back * (reach * Fx::ratio(3, 5)));
        self.ensure_moving(row, goal, goal)
    }

    /// A gunship circles `center` at three fifths of its reach, guns on it.
    fn air_hover_standoff(&mut self, row: usize, center: FxVec2) -> Result<(), SimError> {
        self.state.units.flags[row] |= flag::AIR_RUN;
        let delta = self.state.units.pos[row] - center;
        let standoff = self.bp(row).max_weapon_range() * Fx::ratio(3, 5);
        let bearing = if delta.length() > Fx::ONE {
            delta.angle()
        } else {
            self.state.units.heading[row]
        };
        let side = if row % 2 == 0 { 18 } else { -18 };
        let goal = self.clamp_to_map(
            center + FxVec2::from_angle(bearing + Angle::from_degrees(side)) * standoff,
        );
        self.ensure_moving(row, goal, goal)?;
        self.state.units.air_aim[row] = center;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Leaving sight during a wide return must not cancel the flight.
    /// Search the recorded point once; do not track unseen target movement.
    fn air_search_last_seen(&mut self, row: usize, target: UnitId) -> Result<bool, SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(target) else {
            return Ok(false);
        };
        if !units.has_flag(row, flag::AIR_RUN) || self.detects(units.owner[row], t) {
            return Ok(false);
        }
        let aim = units.air_aim[row];
        if units.pos[row].distance(aim) <= self.bp(row).vision / 2 {
            return Ok(false);
        }
        self.air_fly_through(row, aim, self.air_run_distance(row))?;
        Ok(true)
    }

    /// Pursue the predicted intercept instead of orbiting a fixed contact point.
    /// Speed and turn authority respond to this pursuit bearing in movement.
    fn air_dogfight(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let motion = self.bp(row).motion.expect("air");
        // `prev_pos` is refreshed at the top of the tick, before orders run.
        let tvel = units.air_velocity[target].xy();
        let intercept_time = (pos.distance(units.pos[target]) / motion.speed)
            .clamp(Fx::ratio(1, 5), Fx::ratio(3, 2));
        let lead = units.pos[target] + tvel * Fx::from_int(DT) * intercept_time;
        let to = lead - pos;
        let nose = FxVec2::from_angle(units.heading[row]);
        let off = units.heading[row].delta_to(to.angle()).unsigned_abs();
        let mut turn_ticks = if off > 0x2000 {
            units.air_turn_ticks[row].saturating_add(1)
        } else {
            0
        };
        let mut break_ticks = units.air_break_ticks[row];
        let goal = if break_ticks > 0 {
            break_ticks -= 1;
            turn_ticks = 0;
            units.move_goal[row]
        } else if turn_ticks >= 30 + (row % 3) as u16 * 7 {
            // Sustained unsuccessful pursuit can settle into a mutual circle.
            // Unload the turn and regain separation before the next intercept.
            // Stagger the decision so matched opponents do not mirror forever.
            turn_ticks = 0;
            break_ticks = 18 + (row % 2) as u16 * 8;
            pos + nose * (motion.speed * 2)
        } else if to.length() < Fx::from_int(16) {
            // Finish the crossing before reversing; never pivot on the target.
            pos + nose * (motion.speed / 2)
        } else {
            lead
        };
        let goal = self.clamp_to_map(goal);
        self.ensure_moving(row, goal, goal)?;
        // Changing a movement goal clears old flight state; retain these
        // counters only for this continuing engagement.
        self.state.units.air_turn_ticks[row] = turn_ticks;
        self.state.units.air_break_ticks[row] = break_ticks;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Fly through the target, drop on the pass, then loop for another run.
    fn air_bomb_run(&mut self, row: usize, target: usize) -> Result<(), SimError> {
        let aim = self.air_bomb_aim(row, target);
        self.air_fly_through(row, aim, self.air_run_distance(row))
    }

    /// Where a moving target will be when bombs dropped on this line land.
    /// A bomb keeps the aircraft's ground speed, so it comes down where the
    /// aircraft would have been: time to impact is the flight time to that
    /// point. Flying at the target's present position instead leaves the
    /// release line beside a crossing target, and the bay never opens.
    fn air_bomb_aim(&self, row: usize, target: usize) -> FxVec2 {
        let units = &self.state.units;
        let tpos = units.pos[target];
        // Orders run before movement, where `prev_pos` still equals `pos`:
        // last tick's displacement is what says whether it is under way.
        if units.air_velocity[target].xy() == FxVec2::ZERO {
            return tpos;
        }
        // Heading and speed are steadier than one tick's displacement, which
        // jitters with crowding and would swing a far aim point about.
        let tvel = FxVec2::from_angle(units.heading[target]) * (units.speed[target] / DT);
        let motion = self.bp(row).motion.expect("air");
        let step = (motion.speed / DT).max(Fx::ONE);
        // Never less than the fall itself, so this agrees with the bomb sight
        // at the moment of release; never so far that a turn makes it absurd.
        let fall = ((motion.altitude * 2 / crate::combat::GRAVITY).sqrt()).max(Fx::ONE);
        let mut aim = tpos;
        for _ in 0..3 {
            let ticks = (units.pos[row].distance(aim) / step).clamp(fall, Fx::from_int(12 * DT));
            aim = tpos + tvel * ticks;
        }
        self.clamp_to_map(aim)
    }

    fn air_run_distance(&self, row: usize) -> Fx {
        let motion = self.bp(row).motion.expect("air");
        let turn_radius = motion
            .speed
            .mul_div(10430, (motion.turn_rate as i64 * DT as i64).max(1));
        let fall = ((motion.altitude * 2 / crate::combat::GRAVITY).sqrt()).ceil_int();
        let rack = self
            .bp(row)
            .weapons
            .iter()
            .filter(|w| w.trajectory == mc_data::Trajectory::Ballistic && !w.missile)
            .map(|w| {
                (w.salvo.saturating_sub(1) / w.salvo_batch) as i32 * w.salvo_delay_ticks as i32 / 2
            })
            .max()
            .unwrap_or(0);
        // Finish the reversal before the next release line, including half the carpet.
        (motion.speed * 4 + Fx::from_int(48))
            .max(turn_radius * 2 + motion.speed / DT * (fall + rack) + Fx::from_int(48))
    }

    /// Ticks from opening the bay at cruise height to the middle of the carpet landing.
    fn air_bomb_ticks(&self, row: usize) -> Option<Fx> {
        let motion = self.bp(row).motion?;
        let rack = self
            .bp(row)
            .weapons
            .iter()
            .filter(|w| w.trajectory == mc_data::Trajectory::Ballistic && !w.missile)
            .map(|w| {
                (w.salvo.saturating_sub(1) / w.salvo_batch) as i32 * w.salvo_delay_ticks as i32 / 2
            })
            .max()?;
        Some((motion.altitude * 2 / crate::combat::GRAVITY).sqrt() + Fx::from_int(rack))
    }

    /// One attack pass after another. `air_turn_ticks` is the latch between
    /// the two halves of a pass: set while the aircraft is lining up on the
    /// target, clear while it flies through and out to turning room.
    fn air_fly_through(&mut self, row: usize, aim: FxVec2, run: Fx) -> Result<(), SimError> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let motion = self.bp(row).motion.expect("air");
        let step = motion.speed / DT;
        let arrive = step + self.bp(row).radius + Fx::from_int(8);
        let nose = FxVec2::from_angle(units.heading[row]);
        let turn_radius = motion
            .speed
            .mul_div(10430, (motion.turn_rate as i64 * DT as i64).max(1));
        let margin = turn_radius * Fx::ratio(8, 5);
        let size = self.terrain.size_metres();
        let to = aim - pos;
        let gap = to.length();
        let inbound = nose.dot(to) > Fx::ZERO;
        // Inside this the line is flown, not steered: a bomber is committed at
        // its release point, a gun keeps correcting until the target is under it.
        let commit = self
            .air_bomb_ticks(row)
            .map_or(step * 2, |ticks| step * ticks);
        // Rolling in costs room, so plan on a wider circle than the steady turn.
        // The target must lie outside it with a straight leg left before commit.
        let wide = turn_radius * Fx::ratio(5, 4);
        let side = if nose.perp().dot(to) < Fx::ZERO {
            -nose.perp()
        } else {
            nose.perp()
        };
        let centre_gap = (aim - (pos + side * wide)).length();
        let can_line_up =
            inbound && gap > commit && centre_gap * centre_gap >= wide * wide + commit * commit;
        let engaged = units.has_flag(row, flag::AIR_RUN) && units.has_flag(row, flag::HAS_FIELD);
        let lining_up = engaged && units.air_turn_ticks[row] > 0;
        let positioning = engaged && units.field_goal[row] != units.move_goal[row];
        let goal = units.move_goal[row];
        // Short of room and about to run out of map: break off along the edge,
        // even with the target still ahead. One standing on the boundary has
        // been bombed by now, and flying on would pin the aircraft to the edge.
        let ahead = pos + nose * margin;
        let leaving = gap < run * Fx::ratio(9, 10)
            && (ahead.x < Fx::from_int(4)
                || ahead.y < Fx::from_int(4)
                || ahead.x > size.x - Fx::from_int(4)
                || ahead.y > size.y - Fx::from_int(4));

        if positioning && pos.distance(goal) > arrive && gap < run * Fx::ratio(9, 10) {
            // On the way to a setup point beside a map edge, which is only a
            // direction to find room in: the leg ends with the room, reached or
            // not. Give it up when its target has gone somewhere else.
            let room = goal.distance(aim);
            if room >= run * Fx::ratio(7, 10) && room <= run * 2 {
                return self.ensure_moving(row, units.field_goal[row], goal);
            }
        } else if leaving && !positioning && (inbound || !lining_up) {
            return self.air_edge_setup(row, aim, run, margin);
        } else if lining_up || (!positioning && can_line_up) {
            if can_line_up {
                // Correct the line through where the target will be.
                return self.air_run_goal(row, aim + to.normalize() * run, 1);
            }
            if inbound && engaged {
                // Committed, or too tight to make: fly this line out. Chasing
                // the target from here only winds the aircraft round it.
                self.state.units.air_turn_ticks[row] = 0;
                return Ok(());
            }
            if !inbound && lining_up {
                // Still coming round onto it.
                return self.air_turn_in(row, aim, run, margin);
            }
        } else if engaged && !positioning {
            // Flying out. The leg is measured from where the target is now,
            // not from where it was bombed: one driving the same way would
            // otherwise be too close behind to line up on again.
            if inbound || gap < run {
                if pos.distance(goal) <= arrive * 3 || nose.dot(goal - pos) <= Fx::ZERO {
                    return self.air_run_goal(row, pos + nose * run, 0);
                }
                return Ok(());
            }
        } else if !engaged && !can_line_up && gap < run {
            // Ordered onto something too close or behind: open the range first.
            return self.air_run_goal(row, pos + nose * run, 0);
        }
        // Far enough out, or arrived at a setup point: come round and aim
        // straight through the target. The flight controller rolls into one
        // continuous return arc.
        self.air_turn_in(row, aim, run, margin)
    }

    /// Come round onto the target the short way, unless that way runs out of
    /// map: then round the open side, however much further it is.
    fn air_turn_in(
        &mut self,
        row: usize,
        aim: FxVec2,
        run: Fx,
        margin: Fx,
    ) -> Result<(), SimError> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let nose = FxVec2::from_angle(units.heading[row]);
        let size = self.terrain.size_metres();
        let to = aim - pos;
        let short = if nose.perp().dot(to) < Fx::ZERO {
            -nose.perp()
        } else {
            nose.perp()
        };
        let outside = |p: FxVec2| {
            p.x < Fx::from_int(4)
                || p.y < Fx::from_int(4)
                || p.x > size.x - Fx::from_int(4)
                || p.y > size.y - Fx::from_int(4)
        };
        // The far side of the turn, and the corner it swings through.
        let swept = |side: FxVec2| {
            outside(pos + side * margin) || outside(pos + (side + nose) * (margin / 2))
        };
        if nose.dot(to) <= Fx::ZERO && swept(short) && !swept(-short) {
            return self.air_run_goal(row, pos - short * margin, 1);
        }
        let approach = if to.length() > Fx::ONE {
            to.normalize()
        } else {
            nose
        };
        self.air_run_goal(row, aim + approach * run, 1)
    }

    /// Changing a movement goal clears flight state; put the pass latch back.
    /// The goal may lie off the map: that keeps the attack line true, and the
    /// outbound leg turns back before the aircraft itself gets there.
    fn air_run_goal(&mut self, row: usize, goal: FxVec2, lining_up: u16) -> Result<(), SimError> {
        self.ensure_moving(row, goal, goal)?;
        self.state.units.air_turn_ticks[row] = lining_up;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    /// Out of airspace on the way out. Take the nearest point that still has
    /// a full approach to the target from inside the map, which beside an edge
    /// means running along it rather than at it.
    fn air_edge_setup(
        &mut self,
        row: usize,
        aim: FxVec2,
        run: Fx,
        margin: Fx,
    ) -> Result<(), SimError> {
        let size = self.terrain.size_metres();
        let pos = self.state.units.pos[row];
        let nose = FxVec2::from_angle(self.state.units.heading[row]);
        let wide = margin * Fx::ratio(25, 32);
        let inset = margin.min(size.x.min(size.y) / 6);
        let mut best: Option<(bool, Fx, FxVec2)> = None;
        for sixteenth in 0..16u32 {
            let direction = FxVec2::from_angle(Angle((sixteenth * 4096) as u16));
            let setup = aim + direction * run;
            let candidate = FxVec2::new(
                setup.x.clamp(inset, size.x - inset),
                setup.y.clamp(inset, size.y - inset),
            );
            let room = candidate.distance(aim);
            // A point inside either turning circle can only be orbited.
            let flyable = [nose.perp(), -nose.perp()]
                .iter()
                .all(|&side| candidate.distance(pos + side * wide) >= wide);
            let enough = flyable && room >= run * Fx::ratio(9, 10);
            // With room to spare the nearest wins; with none anywhere, the roomiest.
            let score = if enough {
                pos.distance(candidate)
            } else {
                -room
            };
            if best.is_none_or(|(had, s, _)| (enough && !had) || (enough == had && score < s)) {
                best = Some((enough, score, candidate));
            }
        }
        let ingress = best.map_or(size * Fx::HALF, |(_, _, p)| p);
        self.ensure_moving(row, aim, ingress)?;
        self.state.units.flags[row] |= flag::AIR_RUN;
        Ok(())
    }

    fn run_attack(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(o.target) else {
            self.finish_order(row);
            return Ok(());
        };
        // A chase the unit started itself (`idle_chase`) ends at the end of its
        // leash, when it gets stuck, or when its stance stops allowing it. The
        // walk home queued behind it comes next.
        if o.radius > Fx::ZERO
            && (!self.chases(row)
                || units.pos[row].distance(o.pos) > o.radius
                || units.stuck_ticks[row] == u16::MAX)
        {
            self.finish_order(row);
            return Ok(());
        }
        if self.is_air(row) && self.air_search_last_seen(row, o.target)? {
            return Ok(());
        }
        let units = &self.state.units;
        if !self.detects(units.owner[row], t) || !self.can_strike(row, t) {
            self.finish_order(row);
            return Ok(());
        }
        if self.is_air(row) {
            return self.air_fight(row, t);
        }
        let target_pos = units.pos[t];
        // A named Attack is a fire order: stand and shoot the moment any
        // gun can hit, at full range. A gun on it that the ground hides cannot:
        // the unit walks on until it can see it (`line_of_fire.rs`).
        let in_range = self.bp(row).weapons.iter().enumerate().any(|(w, weapon)| {
            self.is_valid_target(row, t, weapon)
                && !(units.weapon_target[row][w] == o.target && self.shot_blocked(row, w))
        });
        if in_range {
            self.state.units.flags[row] |= flag::HOLD;
            return Ok(());
        }
        let field_goal = if units.has_flag(row, flag::HAS_FIELD)
            && units.field_goal[row].distance(target_pos) <= CHASE_REPATH_DISTANCE
        {
            units.field_goal[row]
        } else {
            target_pos
        };
        self.ensure_moving(row, field_goal, target_pos)
    }

    /// Into range of the point, then hold there: the weapons phase shells it
    /// (`World::ground_mark`). Aircraft fly passes over it, gunships circle it.
    fn run_attack_ground(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        if self.is_air(row) {
            self.state.units.stuck_ticks[row] = 0;
            self.state.units.air_aim[row] = o.pos;
            if self.bp(row).motion.is_some_and(|m| m.hover) {
                return self.air_hover_standoff(row, o.pos);
            }
            // Bombarding: each run goes at its own point in the circle, picked
            // once the last run's bombs are away.
            let mut aim = o.pos;
            if let Some(w) = self.bombard_lead(row).filter(|_| o.radius > Fx::ZERO) {
                if self.state.units.ground_aim[row][w].distance(o.pos) > o.radius {
                    self.pick_bombard_aim(row, w);
                }
                aim = self.state.units.ground_aim[row][w];
                self.state.units.air_aim[row] = aim;
            }
            return self.air_fly_through(row, aim, self.air_run_distance(row));
        }
        let gap = self.state.units.pos[row].distance(o.pos);
        // Bombarding: close until most of the circle is in reach, not just its middle.
        let in_range = self.bp(row).weapons.iter().any(|w| {
            let reach = (w.range_max - o.radius).max(w.range_max / 2);
            crate::combat::hits_ground(w) && gap <= reach && gap >= w.range_min
        });
        if in_range {
            self.state.units.flags[row] |= flag::HOLD;
            return Ok(());
        }
        // A structure cannot close the distance, and a unit with no way there gives up.
        if self.bp(row).motion.is_none() || self.state.units.stuck_ticks[row] == u16::MAX {
            self.finish_order(row);
            return Ok(());
        }
        self.ensure_moving(row, o.pos, o.pos)
    }

    /// An idle armed land or naval unit that sees an enemy just out of range goes
    /// after it, then walks back. The chase is an `Attack` whose `pos` is home
    /// and whose `radius` is the leash (`run_attack` ends it there), followed by
    /// an attack-move home. Builders keep to their work, and a unit already
    /// shooting something stays put. True if it set off.
    fn idle_chase(&mut self, row: usize) -> Result<bool, SimError> {
        // Looked at a few times a second, spread over the rows.
        if (self.state.tick as usize + row) % 4 != 0 {
            return Ok(false);
        }
        let bp = self.bp(row);
        if bp.weapons.is_empty()
            || !bp.is_mobile()
            || bp.builder.is_some()
            || bp.drone.is_some()
            || self.is_air(row)
            || !self.chases(row)
            || self.state.units.has_flag(row, flag::PASSIVE)
            || self.has_clear_target(row)
        {
            return Ok(false);
        }
        let range = bp.max_weapon_range();
        let leash = range.max(Fx::from_int(CHASE_LEASH_MIN));
        let reach = (range + leash / 2).min(bp.vision);
        let units = &self.state.units;
        let (home, owner) = (units.pos[row], units.owner[row]);
        let Some(t) = self
            .index
            .nearest(home, reach, kind::UNIT, |e| {
                let t = e.row as usize;
                self.unit_entry_is_current(e)
                    && self.are_enemies(owner, units.owner[t])
                    && !units.has_flag(t, flag::IN_FACTORY)
                    && self.can_strike(row, t)
                    && self.detects(owner, t)
            })
            .map(|e| e.row as usize)
        else {
            return Ok(false);
        };
        let mut chase = order(OrderKind::Attack, home, self.state.units.id(t));
        chase.radius = leash;
        let mut back = order(OrderKind::AttackMove, home, Handle::NONE);
        back.heading = self.state.units.heading[row];
        self.give(row, chase, false)?;
        self.give(row, back, true)?;
        Ok(true)
    }

    /// Attack-move to the waypoint; there, send the order to the back of the queue.
    /// A mobile builder on patrol mends and reclaims what it passes, as it does idle.
    fn run_patrol(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let goal = self.clamp_to_map(o.pos + o.offset);
        // A group walks its loop together: it leaves a post once its anchor is on it,
        // or, held up where its ranks do not fit, as each member gets there.
        let group = self
            .state
            .formations
            .get(&o.formation)
            .map(|g| (g.phase, g.anchor));
        if group.is_some_and(|(phase, anchor)| {
            phase == crate::movement::PHASE_NEXT_LEG || (phase != 0 && anchor == o.pos)
        }) {
            return self.next_patrol_leg(row);
        }
        let on_its_own = group.is_none_or(|(phase, _)| phase == 3);
        if self.is_air(row) {
            if self.air_search_last_seen(row, o.target)? {
                return Ok(());
            }
            if let Some(t) = self.air_engage_target(row) {
                return self.air_fight(row, t);
            }
            // Aircraft sweep through a waypoint rather than settle on it, turning
            // in early enough to roll out on the next leg instead of overshooting.
            let motion = self.bp(row).motion.expect("air");
            let mut reach = motion.speed / 2 + self.bp(row).radius + Fx::from_int(8);
            if !motion.hover {
                let units = &self.state.units;
                let next = self
                    .state
                    .orders
                    .iter(units, row)
                    .filter(|p| p.kind == OrderKind::Patrol)
                    .nth(1)
                    .map(|p| self.clamp_to_map(p.pos + p.offset));
                if let Some(next) = next {
                    let turn_radius = units.speed[row]
                        .mul_div(10430, (motion.turn_rate as i64 * DT as i64).max(1));
                    reach = reach.max(crate::movement::patrol_lead(
                        units.heading[row],
                        goal,
                        next,
                        turn_radius,
                    ));
                }
            }
            if on_its_own && self.state.units.pos[row].distance(goal) <= reach {
                return self.next_patrol_leg(row);
            }
            self.ensure_moving(row, o.pos, goal)?;
            // A flight on patrol keeps its formation instead.
            if !motion.hover && group.is_none() {
                self.state.units.flags[row] |= flag::AIR_RUN;
            }
            return Ok(());
        }
        let units = &self.state.units;
        let tolerance = self.bp(row).radius / 2
            + Fx::from_int(2)
            + Fx::from_int(units.stuck_ticks[row] as i32) / 2;
        if units.stuck_ticks[row] == u16::MAX
            || (on_its_own && units.pos[row].distance(goal) <= tolerance)
        {
            return self.next_patrol_leg(row);
        }
        if self.bp(row).builder.is_some() {
            let working = self.idle_repair(row)? || {
                self.idle_reclaim(row)?;
                self.state.units.flags[row] & (flag::RECLAIMING | flag::WORKING) != 0
            };
            if working {
                self.state.units.flags[row] |= flag::HOLD;
                return Ok(());
            }
        }
        if self.has_clear_target(row) {
            self.state.units.flags[row] |= flag::HOLD;
        }
        self.ensure_moving(row, o.pos, goal)
    }

    /// The waypoint is reached: it goes to the back of the queue and the next one is taken.
    fn next_patrol_leg(&mut self, row: usize) -> Result<(), SimError> {
        let Some(mut o) = self.state.orders.pop_front(&mut self.state.units, row) else {
            return Ok(());
        };
        o.target = Handle::NONE;
        self.state.orders.push_back(&mut self.state.units, row, o)?;
        self.stop_moving(row);
        // A group back on a leg it walked last time round forms up afresh for it.
        if let Some(next) = self.state.orders.front(&self.state.units, row).copied() {
            if let Some(g) = self.state.formations.get_mut(&next.formation) {
                if g.phase == crate::movement::PHASE_NEXT_LEG
                    || (g.phase != 0 && g.anchor == next.pos)
                {
                    g.phase = 0;
                    g.speed = Fx::ZERO;
                }
            }
            // An aircraft flies straight on into the next leg: left without a
            // goal for the tick, it would shed speed and roll level mid-turn.
            if next.kind == OrderKind::Patrol && self.is_air(row) {
                let goal = self.clamp_to_map(next.pos + next.offset);
                self.ensure_moving(row, next.pos, goal)?;
                let hover = self.bp(row).motion.is_some_and(|m| m.hover);
                if !hover && !self.state.formations.contains_key(&next.formation) {
                    self.state.units.flags[row] |= flag::AIR_RUN;
                }
            }
        }
        Ok(())
    }

    /// Walks a builder into range of `pos`. True once it is close enough to work.
    pub(crate) fn approach(
        &mut self,
        row: usize,
        pos: FxVec2,
        target_radius: Fx,
    ) -> Result<bool, SimError> {
        let range = self.work_range(row) + target_radius;
        if self.state.units.pos[row].distance(pos) <= range {
            self.state.units.flags[row] |= flag::HOLD;
            if self.state.units.has_flag(row, flag::HAS_FIELD) {
                self.stop_moving(row);
            }
            return Ok(true);
        }
        // A structure cannot walk over: what is out of its reach is given up.
        if self.bp(row).motion.is_none() {
            self.state.units.stuck_ticks[row] = u16::MAX;
        }
        if self.state.units.stuck_ticks[row] == u16::MAX {
            return Ok(false);
        }
        self.ensure_moving(row, pos, pos)?;
        Ok(false)
    }

    /// Turns a build arm or a reclaimer turret toward its work at `pos`. True
    /// once it points there, and for a unit with nothing to turn. The arm aims
    /// a little above the ground there.
    pub(crate) fn face_work(&mut self, row: usize, pos: FxVec2) -> bool {
        let middle = self.terrain.height_at(pos) + Fx::from_int(2);
        self.face_work_at(row, pos, middle)
    }

    /// [`Self::face_work`] for work whose middle is at height `z`: the arm points up or down at it too.
    pub(crate) fn face_work_at(&mut self, row: usize, pos: FxVec2, z: Fx) -> bool {
        let bp = self.bp(row);
        let (turn, pivot) = match (bp.builder.as_ref().and_then(|b| b.arm), bp.reclaimer) {
            (Some(arm), _) => (arm.turn, arm.pivot),
            (None, Some(r)) if r.turn > 0 => (r.turn, None),
            _ => return true,
        };
        let has_shoulder = bp
            .builder
            .as_ref()
            .and_then(|b| b.arm)
            .is_some_and(|a| a.shoulder.is_some());
        let units = &mut self.state.units;
        units.flags[row] |= flag::WORKING;
        let offset = pos - units.pos[row];
        if offset.length_sq() < Fx::ONE {
            return true;
        }
        let pitched = if let Some(pivot) = pivot {
            let want = crate::world::pitch_to(offset.length(), z - (units.z[row] + pivot.z));
            if has_shoulder {
                // Boom unfolds from folded-up rest to level, then the forearm aims.
                units.arm_pitch[row][0] =
                    units.arm_pitch[row][0].turn_toward(Angle::ZERO, turn / 2);
                let boom_on = units.arm_pitch[row][0].delta_to(Angle::ZERO).unsigned_abs()
                    <= WORK_AIM_TOLERANCE;
                if boom_on {
                    units.arm_pitch[row][1] = units.arm_pitch[row][1].turn_toward(want, turn / 2);
                }
                boom_on
                    && units.arm_pitch[row][1].delta_to(want).unsigned_abs() <= WORK_AIM_TOLERANCE
            } else {
                units.arm_pitch[row][1] = units.arm_pitch[row][1].turn_toward(want, turn / 2);
                units.arm_pitch[row][1].delta_to(want).unsigned_abs() <= WORK_AIM_TOLERANCE
            }
        } else {
            true
        };
        let want = offset.angle() - units.heading[row];
        let yaw = units.weapon_yaw[row][0].turn_toward(want, turn);
        units.weapon_yaw[row][0] = yaw;
        pitched && yaw.delta_to(want).unsigned_abs() <= WORK_AIM_TOLERANCE
    }

    fn run_build(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        if let Some(t) = units.row(units.build_target[row]) {
            if units.pos[t] == o.pos && units.has_flag(t, flag::UNDER_CONSTRUCTION) {
                let middle = units.z[t] + self.bp(t).height / 2;
                self.state.units.flags[row] |= flag::HOLD;
                // Paused: it waits by the half-built site, arm down, so its guns stay free.
                if !self.work_paused(row) && self.face_work_at(row, o.pos, middle) {
                    self.state.units.flags[row] |= flag::BUILDING;
                }
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
        // Paused: it waits at the lot and starts nothing until resumed.
        if self.work_paused(row) {
            self.state.units.flags[row] |= flag::HOLD;
            return Ok(());
        }
        if !self.face_work(row, o.pos) {
            return Ok(());
        }
        // Someone may have started this exact structure already: join in.
        if let Some(existing) = self.joinable_site(row, o.blueprint, o.pos) {
            self.state.units.build_target[row] = self.state.units.id(existing);
            return Ok(());
        }
        if !self.can_place(&bp, o.pos) {
            // Same-tick start: the lot is blocked but the site is not in the
            // index yet. Join that, rather than bounce and drop the order.
            if let Some(existing) = self.unindexed_joinable_site(row, o.blueprint, o.pos) {
                self.state.units.build_target[row] = self.state.units.id(existing);
                return Ok(());
            }
            self.events.push(SimEvent::BuildRejected {
                player: self.state.units.owner[row],
            });
            self.finish_order(row);
            return Ok(());
        }
        if self.mobile_units_in_footprint(&bp, o.pos, row, o.blueprint) {
            // Friendly units standing on the site get a moment to clear off.
            if self.state.units.stuck_ticks[row] < 50 {
                self.state.units.stuck_ticks[row] += 1;
                return Ok(());
            }
            self.events.push(SimEvent::BuildRejected {
                player: self.state.units.owner[row],
            });
            self.finish_order(row);
            return Ok(());
        }
        // Trees on the lot go first: no mass in them, so it is quick.
        if !self.clear_lot(row, bp.footprint, o.pos) {
            return Ok(());
        }
        let owner = self.state.units.owner[row];
        // A unit raised on a lot lies along it, whatever the order said.
        let heading = if bp.is_site_built_unit() { bp.build_heading() } else { o.heading };
        let site = self.spawn_unit(o.blueprint, owner, o.pos, heading, false)?;
        self.state.units.build_target[row] = self.state.units.id(site);
        Ok(())
    }

    fn site_is_joinable(
        &self,
        builder: usize,
        site: usize,
        blueprint: BlueprintId,
        pos: FxVec2,
    ) -> bool {
        let units = &self.state.units;
        units.blueprint[site] == blueprint
            && units.pos[site] == pos
            && units.has_flag(site, flag::UNDER_CONSTRUCTION)
            && !self.are_enemies(units.owner[builder], units.owner[site])
    }

    /// A matching construction site at `pos` that the spatial index already knows.
    fn joinable_site(&self, builder: usize, blueprint: BlueprintId, pos: FxVec2) -> Option<usize> {
        // An experimental's site is a unit, not a structure: `structure_at` passes it by.
        if self.blueprints.unit(blueprint).is_site_built_unit() {
            return self.unindexed_joinable_site(builder, blueprint, pos);
        }
        let existing = self.structure_at(pos, 0)?;
        self.site_is_joinable(builder, existing, blueprint, pos)
            .then_some(existing)
    }

    /// Same as [`Self::joinable_site`], but walks the unit table. Needed when
    /// another builder spawned the site this tick, before `rebuild_index`.
    fn unindexed_joinable_site(
        &self,
        builder: usize,
        blueprint: BlueprintId,
        pos: FxVec2,
    ) -> Option<usize> {
        self.state
            .units
            .slots
            .iter()
            .find(|&r| r != builder && self.site_is_joinable(builder, r, blueprint, pos))
    }

    fn mobile_units_in_footprint(
        &self,
        bp: &mc_data::UnitBlueprint,
        pos: FxVec2,
        except: usize,
        blueprint: BlueprintId,
    ) -> bool {
        // The lot is a rectangle: a unit beside a long, narrow lot is not on it.
        let half = FxVec2::from_ints(
            bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2,
            bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2,
        );
        let mut found = false;
        self.index.query(pos, half.x.max(half.y), kind::UNIT, |e| {
            let r = e.row as usize;
            if r != except
                && self.unit_entry_is_current(e)
                && self.bp(r).is_mobile()
                && !self.building_this_site(r, blueprint, pos)
            {
                let d = e.pos - pos;
                if d.x.abs() < half.x && d.y.abs() < half.y {
                    found = true;
                    return false;
                }
            }
            true
        });
        found
    }

    /// True when `row` is already ordered to start or join this structure.
    fn building_this_site(&self, row: usize, blueprint: BlueprintId, pos: FxVec2) -> bool {
        self.state
            .orders
            .front(&self.state.units, row)
            .is_some_and(|o| o.kind == OrderKind::Build && o.pos == pos && o.blueprint == blueprint)
    }

    fn run_assist(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units.row(o.target) else {
            self.state.units.build_target[row] = Handle::NONE;
            self.finish_order(row);
            return Ok(());
        };
        // What there is to do, in priority order: finish the target itself,
        // help with whatever it is building, repair it, or feed a live shield.
        let work = if units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            Some((t, false))
        } else if let Some(product) = units
            .row(units.build_target[t])
            .filter(|p| units.has_flag(*p, flag::UNDER_CONSTRUCTION))
        {
            Some((product, false))
        } else if units.health[t] < self.unit_max_health(t) {
            Some((t, true))
        } else if self.shield_assistable(t) {
            Some((t, false))
        } else {
            None
        };
        // Nothing to help with right now (or only a paused builder's work), no
        // build lined up on the target, and more orders queued behind: the
        // assist gives way to them at once. A factory between products or a
        // builder walking to its next site still counts as work coming.
        let waiting = match work {
            None => !self.state.orders.iter(units, t).any(|n| {
                matches!(
                    n.kind,
                    OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade
                )
            }),
            Some((w, _)) => w != t && self.work_paused(t),
        };
        if waiting
            && !self.work_paused(row)
            && self
                .state
                .orders
                .iter(&self.state.units, row)
                .nth(1)
                .is_some()
        {
            self.state.units.build_target[row] = Handle::NONE;
            self.finish_order(row);
            return Ok(());
        }
        match work {
            // Paused, or helping a builder that is paused: wait by the work, spending nothing.
            Some((w, _)) if self.work_paused(row) || (w != t && self.work_paused(t)) => {
                self.state.units.build_target[row] = Handle::NONE;
                let (pos, radius) = (self.state.units.pos[w], self.bp(w).radius);
                self.approach(row, pos, radius)?;
            }
            Some((w, repairing)) => {
                let (pos, radius) = (self.state.units.pos[w], self.bp(w).radius);
                if self.approach(row, pos, radius)? {
                    self.state.units.build_target[row] = self.state.units.id(w);
                    let middle = self.state.units.z[w] + self.bp(w).height / 2;
                    if self.face_work_at(row, pos, middle) {
                        self.state.units.flags[row] |= flag::BUILDING;
                        if repairing {
                            self.state.units.flags[row] |= flag::REPAIRING;
                        }
                    }
                }
            }
            None => {
                // With nothing queued behind, stay on the assist until another
                // order is given, or the target is gone. A full shield, a
                // finished repair, an idle factory: wait nearby so work can
                // resume when it appears.
                self.state.units.build_target[row] = Handle::NONE;
                let (pos, radius) = (self.state.units.pos[t], self.bp(t).radius);
                self.approach(row, pos, radius)?;
            }
        }
        Ok(())
    }

    fn run_reclaim(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        // A wreck too deep for this beam is given up (`World::wreck_in_reach`).
        let Some(w) = self
            .state
            .wrecks
            .slots
            .resolve(o.target)
            .filter(|&w| self.wreck_in_reach(row, w))
        else {
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
        if self.reclaim_ready(row, pos) && self.drain_wreck(row, w) {
            self.finish_order(row);
        }
        Ok(())
    }

    fn run_produce(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(t) = self.state.units.row(self.state.units.build_target[row]) else {
            // Paused: nothing new goes on the floor until resumed.
            if self.work_paused(row) {
                return Ok(());
            }
            let owner = self.state.units.owner[row];
            let pos = self.state.units.pos[row];
            let heading = self.state.units.heading[row];
            // Print on the lot centre so the finished hull walks out +x.
            let pad = pos;
            let t = self.spawn_unit(o.blueprint, owner, pad, heading, false)?;
            self.state.units.flags[t] |= flag::IN_FACTORY;
            self.state.units.build_target[row] = self.state.units.id(t);
            return Ok(());
        };
        if self.state.units.has_flag(t, flag::UNDER_CONSTRUCTION) {
            if !self.work_paused(row) {
                self.state.units.flags[row] |= flag::BUILDING;
            }
            return Ok(());
        }
        // Finished: walk it out the factory's facing side. If that cell is a
        // building, a cliff or off the map, take the nearest ground the hull
        // can stand on instead of holding the finished unit forever.
        let reach = Fx::from_int(self.bp(row).footprint.0 as i32 * mc_map::BUILD_CELL_M / 2)
            + self.bp(t).radius
            + Fx::from_int(8);
        let motion = self.bp(t).motion;
        let pos = self.state.units.pos[row];
        let heading = self.state.units.heading[row];
        let want = pos + FxVec2::from_angle(heading) * reach;
        let exit = match motion {
            Some(m) if m.layer == MoveLayer::Air => want,
            Some(m) => self
                .nav
                .nearest_passable(m.layer, m.size_class, want)
                .or_else(|| self.nav.nearest_passable(m.layer, m.size_class, pos))
                .filter(|&p| self.terrain.in_bounds(p))
                .unwrap_or(want),
            None => want,
        };
        if !self.terrain.in_bounds(exit) {
            return Ok(());
        }
        if !self.roll_out_of_factory(t, exit, motion) {
            return Ok(());
        }
        self.state.units.flags[t] &= !flag::IN_FACTORY;
        self.state.units.build_target[row] = Handle::NONE;
        // Standing orders are the way out; without any, or none it can carry out, the rally point.
        if !self.inherit_standing(row, t)? {
            let rally = if self.state.units.rally[row] == pos {
                exit + (exit - pos).normalize() * Fx::from_int(40)
            } else {
                self.state.units.rally[row]
            };
            let rally = self.clamp_to_map(rally);
            let rally = self.clear_formation_destination(
                rally,
                &[FxVec2::ZERO],
                &[t],
                self.bp(t).radius * 2 + Fx::from_int(6),
            );
            let mut rally_order = order(OrderKind::Move, rally, Handle::NONE);
            rally_order.heading = (rally - exit).angle();
            self.state
                .orders
                .push_back(&mut self.state.units, t, rally_order)?;
        }
        self.state.orders.pop_front(&mut self.state.units, row);
        if self.state.units.has_flag(row, flag::REPEAT) {
            self.state
                .orders
                .push_back(&mut self.state.units, row, *o)?;
        }
        Ok(())
    }

    /// Advances a finished product toward `exit` at its own speed so walkers
    /// stride and tracks crawl. Returns true once it has cleared the bay.
    /// Movement cannot do this: the factory lot is blocked, and the unit is
    /// still `IN_FACTORY` so it is skipped by that phase.
    fn roll_out_of_factory(
        &mut self,
        row: usize,
        exit: FxVec2,
        motion: Option<mc_data::Motion>,
    ) -> bool {
        let pos = self.state.units.pos[row];
        let to = exit - pos;
        let dist = to.length();
        if dist <= Fx::from_int(2) {
            let z = self.ground_z(row, exit);
            let heading = to.angle();
            let units = &mut self.state.units;
            units.pos[row] = exit;
            units.heading[row] = heading;
            units.z[row] = z;
            units.speed[row] = Fx::ZERO;
            return true;
        }
        let dir = to.normalize();
        let speed = motion.map(|m| m.speed).unwrap_or(Fx::from_int(20));
        let step = (speed / Fx::from_int(TICKS_PER_SECOND as i32)).min(dist);
        let next = pos + dir * step;
        let heading = dir.angle();
        let ground = next.distance(pos);
        let turned = self.state.units.heading[row]
            .delta_to(heading)
            .unsigned_abs() as i32;
        let stride = ground + (self.bp(row).radius * turned).mul_div(355, 113 * 0x10000);
        let gait = (stride * 256).floor_int().clamp(0, u16::MAX as i32) as u16;
        let z = self.ground_z(row, next);
        let units = &mut self.state.units;
        units.gait[row] = units.gait[row].wrapping_add(gait as u32);
        units.gait_step[row] = [gait, units.gait_step[row][0]];
        units.flags[row] |= flag::MOVING;
        units.pos[row] = next;
        units.heading[row] = heading;
        units.speed[row] = speed;
        units.z[row] = z;
        false
    }

    fn ground_z(&self, row: usize, pos: FxVec2) -> Fx {
        // A submarine rides as far under as its dive has taken it.
        if let Some(z) = self.dive_z(row, pos) {
            return z;
        }
        let ground = self.terrain.height_at(pos);
        if self.bp(row).water_build {
            return ground.max(self.terrain.water_level());
        }
        match self.bp(row).motion.map(|m| m.layer) {
            Some(MoveLayer::Hover) | Some(MoveLayer::Naval) | Some(MoveLayer::Air) => {
                ground.max(self.terrain.water_level())
            }
            _ => ground,
        }
    }

    /// Aircraft cruise during orders, then descend onto a clear landing site.
    /// Factory-held products stay on the factory floor.
    pub(crate) fn stand_z(&self, row: usize, pos: FxVec2) -> Fx {
        let surface = self.ground_z(row, pos);
        match self.bp(row).motion {
            Some(m)
                if m.layer == MoveLayer::Air
                    && !self.state.units.has_flag(row, flag::IN_FACTORY)
                    && self.bp(row).transport.is_some() =>
            {
                // A lift ship keeps to the sky unless it is setting down (`transport.rs`).
                self.lift_stand_z(row, pos, surface, m.altitude)
            }
            Some(m)
                if m.layer == MoveLayer::Air
                    && !self.state.units.has_flag(row, flag::IN_FACTORY) =>
            {
                let units = &self.state.units;
                if units.drone_parent[row] == Handle::NONE
                    && self.bp(row).drone.is_none()
                    && self.bp(row).visual.mesh != "reclaim_drone"
                    && units.order_head[row] == NO_ORDER
                    && !units.has_flag(row, flag::AIR_RUN)
                    && units.speed[row] <= Fx::ONE
                    && self.air_can_land(row, pos)
                {
                    surface
                } else if self.descending_to_hatch(row) {
                    // Straight down an airbase's open hatch, to the lift at the bottom.
                    self.shaft_floor(row)
                } else {
                    surface + m.altitude
                }
            }
            _ => surface,
        }
    }

    /// An idle aircraft that cannot set down where it stopped (water, cliffs,
    /// buildings, a pad another aircraft took) flies to the nearest clear
    /// ground instead of hovering there for good. Carriers and drones stay up.
    /// True if it set off.
    fn idle_air_land(&mut self, row: usize) -> Result<bool, SimError> {
        // Looked at twice a second, spread over the rows.
        if (self.state.tick as usize + row) % 16 != 0 || !self.is_air(row) {
            return Ok(false);
        }
        let bp = self.bp(row);
        let Some(motion) = bp.motion else {
            return Ok(false);
        };
        let units = &self.state.units;
        let pos = units.pos[row];
        // Only once it has stopped at cruise height: a hull on the ground or
        // still settling is left to `stand_z`.
        if bp.drone.is_some()
            || bp.visual.mesh == "reclaim_drone"
            || bp.transport.is_some()
            || units.has_flag(row, flag::IN_FACTORY)
            || units.has_flag(row, flag::AIR_RUN)
            || units.speed[row] > Fx::ONE
            || units.z[row] < self.ground_z(row, pos) + motion.altitude / 2
        {
            return Ok(false);
        }
        // An airbase of its side with room, whose reach it is in, comes before any
        // open ground; one just fired out of a tunnel stays out a while first.
        if units.sortie[row] == 0 {
            if let Some(b) = self.airbase_to_land_at(row) {
                let base = self.state.units.id(b);
                let dock = order(OrderKind::Dock, self.state.units.pos[b], base);
                self.give(row, dock, false)?;
                return Ok(true);
            }
        }
        if self.air_can_land(row, pos) {
            return Ok(false);
        }
        let Some(site) = self.air_landing_site(row) else {
            return Ok(false);
        };
        let mut land = order(OrderKind::Move, site, Handle::NONE);
        land.heading = self.state.units.heading[row];
        self.give(row, land, false)?;
        Ok(true)
    }

    /// The nearest place the hull fits, searched in widening rings, nose side first.
    fn air_landing_site(&self, row: usize) -> Option<FxVec2> {
        let pos = self.state.units.pos[row];
        let nose = self.state.units.heading[row].0 as i32;
        let step = (self.bp(row).radius * 2 + Fx::from_int(8)).max(Fx::from_int(16));
        let mut dist = step;
        while dist <= Fx::from_int(AIR_LAND_SEARCH) {
            let n = (dist * 6 / step).floor_int().clamp(8, 48);
            for i in 0..n {
                // 0, +1, -1, +2, -2, ... around the ring from the nose.
                let k = (i + 1) / 2;
                let k = if i % 2 == 1 { k } else { -k };
                let a = Angle((nose + k * 0x10000 / n) as u16);
                let site = self.clamp_to_map(pos + FxVec2::from_angle(a) * dist);
                if self.air_can_land(row, site) {
                    return Some(site);
                }
            }
            dist += step.max(dist / 4);
        }
        None
    }

    /// Land only on dry, unblocked ground across the hull footprint.
    fn air_can_land(&self, row: usize, pos: FxVec2) -> bool {
        let bp = self.bp(row);
        let height = self.terrain.height_at(pos);
        let r = bp.radius;
        for offset in [
            FxVec2::ZERO,
            FxVec2::new(r, r),
            FxVec2::new(-r, r),
            FxVec2::new(r, -r),
            FxVec2::new(-r, -r),
        ] {
            let sample = pos + offset;
            let z = self.terrain.height_at(sample);
            if z <= self.terrain.water_level()
                || (z - height).abs() > Fx::from_int(2)
                || !self.nav.passable(MoveLayer::Land, 0, sample)
            {
                return false;
            }
        }
        let mut clear = true;
        self.index.query(pos, r, kind::UNIT, |e| {
            let other = e.row as usize;
            if other != row
                && self.unit_entry_is_current(e)
                && (!self.is_air(other)
                    || self.state.units.z[other] <= self.ground_z(other, e.pos) + Fx::from_int(16)
                    || (other < row
                        && self.state.units.order_head[other] == NO_ORDER
                        && self.state.units.speed[other] <= Fx::ONE
                        && !self.state.units.has_flag(other, flag::AIR_RUN)))
                && pos.distance(e.pos) < r + e.radius + Fx::from_int(2)
            {
                clear = false;
            }
            clear
        });
        clear
    }

    /// The highest tier `player` has reached: the best of what its finished units can build.
    pub fn side_tech(&self, player: u8) -> u8 {
        let units = &self.state.units;
        units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .map(|r| self.blueprints.builds_tech(self.bp(r)))
            .max()
            .unwrap_or(1)
    }

    /// The commander, a factory, an economy structure (mine, vault, reclaim tower),
    /// an intel tower, and a shield generator: the next tier is built onto the same
    /// unit. Other structures are replaced by their successor.
    pub(crate) fn upgrades_in_place(&self, row: usize) -> bool {
        let bp = self.bp(row);
        bp.is_mobile()
            || bp.has(cat::FACTORY)
            || bp.has(cat::ECONOMY)
            || bp.has(cat::INTEL)
            || bp.has(cat::SHIELD)
            || bp.airbase.is_some()
    }

    fn run_upgrade(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
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
        let units = &self.state.units;
        let Some(t) = units.row(units.build_target[row]) else {
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
            self.remove_unit_row(t, true)?;
            if self.bp(row).is_structure() {
                self.reshape_lot(row, old_id, new.id);
            }
            self.airbase_upgraded(row, old_id);
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

/// How far, in metres, an idle aircraft looks for ground it can land on.
const AIR_LAND_SEARCH: i32 = 1500;

pub(crate) fn order(kind: OrderKind, pos: FxVec2, target: Handle) -> Order {
    Order {
        formation: 0,
        kind,
        pos,
        target,
        blueprint: BlueprintId(0),
        heading: Angle::ZERO,
        offset: FxVec2::ZERO,
        radius: Fx::ZERO,
    }
}
