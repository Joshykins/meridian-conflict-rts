//! Dragging queued orders: a waypoint, a patrol post, a guard's circle or a planned
//! structure picked up and put down somewhere else (`Command::RelocateOrder`).

use crate::mirror::Refusal;
use crate::orders::GUARD_FOLLOW_SLACK;
use crate::tables::*;
use crate::world::snap_turned_to_build_grid;
use crate::{SimError, World};
use mc_core::{Fx, FxVec2};
use mc_data::cat;

impl World {
    /// Drags queued orders somewhere else: every `kind` order these units hold at
    /// `from` goes to `to`. A structure that has been begun stays where it is.
    /// A planned structure moves for every builder holding it, engineering stations
    /// too; a station it is dragged out of reach of lets it go.
    pub(crate) fn relocate_orders(
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
                | OrderKind::Strike
                | OrderKind::Bombard
                | OrderKind::Guard
                | OrderKind::ReclaimArea
        ) {
            return Ok(());
        }
        // A station's plans are among the side's plans (`planned_sites`): left behind,
        // its copy would stand in the way of the drop and stay where it was.
        let rows: Vec<usize> = (self.owned(player, ids, 0).into_iter())
            .filter(|&row| {
                let bp = self.bp(row);
                bp.has(cat::MOBILE) || (kind == OrderKind::Build && bp.works_in_field())
            })
            .collect();
        let units = &self.state.units;
        // A guard round a unit moves with it: by the time the drag arrives its centre
        // has gone on a little from where the player picked it up.
        let found = |o: &Order| {
            o.pos == from
                || (kind == OrderKind::Guard
                    && units.row(o.target).is_some()
                    && o.pos.distance(from) <= GUARD_FOLLOW_SLACK)
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
        // Stations the plan is dragged out of reach of: they let it go.
        let mut dropped: Vec<usize> = Vec::new();
        let to = if kind == OrderKind::Build {
            let (blueprint, heading) = {
                let o = &self.state.orders.order[first];
                (o.blueprint, o.heading)
            };
            nodes.retain(|&(_, node, _)| self.state.orders.order[node].blueprint == blueprint);
            let bp = self.blueprints.unit(blueprint);
            let site = snap_turned_to_build_grid(bp, to, heading);
            // The plans being moved are not in their own way; the same plan in a queue left alone is.
            let moved = |row: usize, o: &Order| {
                o.pos == from && o.blueprint == blueprint && nodes.iter().any(|&(r, ..)| r == row)
            };
            let fits = site == from
                || (self.can_place_turned(bp, site, heading)
                    && !self.plan_blocks(player, blueprint, site, heading, moved));
            if !fits {
                self.reject_site(player, blueprint, site);
                return Ok(());
            }
            let radius = bp.radius;
            dropped = (nodes.iter())
                .map(|&(row, ..)| row)
                .filter(|&row| !self.reaches_work(row, site, radius))
                .collect();
            if dropped.len() == nodes.len() {
                self.refuse(player, Refusal::OutOfReach);
                return Ok(());
            }
            nodes.retain(|(row, ..)| !dropped.contains(row));
            site
        } else {
            self.clamp_to_map(to)
        };
        let blueprint = self.state.orders.order[first].blueprint;
        for row in dropped {
            self.drop_plan(row, blueprint, from)?;
        }
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
            // Put down somewhere, a guard holds that spot and no longer follows anyone,
            // and a build is a fresh plan there, not the site it may have lost.
            if matches!(kind, OrderKind::Guard | OrderKind::Build) {
                self.state.orders.order[node].target = Handle::NONE;
            }
            if current {
                // The way there is asked for again when the order next runs.
                self.state.units.stuck_ticks[row] = 0;
            }
        }
        Ok(())
    }

    /// Takes the plan of `blueprint` at `from` out of the queue in `row`. It has not
    /// been begun: nothing else needs undoing.
    fn drop_plan(
        &mut self,
        row: usize,
        blueprint: mc_data::BlueprintId,
        from: FxVec2,
    ) -> Result<(), SimError> {
        let plan =
            |o: &Order| o.kind == OrderKind::Build && o.pos == from && o.blueprint == blueprint;
        let queue: Vec<Order> = (self.state.orders.iter(&self.state.units, row).copied()).collect();
        if queue.first().is_some_and(plan) {
            self.state.units.build_target[row] = Handle::NONE;
        }
        self.state.orders.clear(&mut self.state.units, row);
        for o in queue.into_iter().filter(|o| !plan(o)) {
            self.state.orders.push_back(&mut self.state.units, row, o)?;
        }
        Ok(())
    }
}
