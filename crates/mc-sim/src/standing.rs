//! A factory's standing orders: the moves, patrols, attacks and assists a player gives a
//! factory, finished or still going up. The factory keeps them as the commands they
//! came in as, and every unit it rolls out is given them again as its own, so they
//! are checked the way any order is: a product that cannot carry one out skips it.

use crate::command::{Command, MAX_PATROL_POINTS};
use crate::tables::*;
use crate::{SimError, World};
use mc_core::FxVec2;
use mc_data::cat;

/// Most commands one factory keeps for its products.
pub const MAX_STANDING: usize = 32;

/// A standing order as the interface draws it.
pub(crate) struct StandingView {
    pub kind: OrderKind,
    /// The command's own position: what `RelocateOrder` and `CancelOrder` find it by.
    pub at: FxVec2,
    /// Where it takes the products: `at`, or the unit it names, where that is now.
    pub pos: FxVec2,
    pub radius: mc_core::Fx,
}

/// The order kind a command becomes, and the positions it is found and moved by.
fn points_mut(command: &mut Command) -> Option<(OrderKind, Vec<&mut FxVec2>)> {
    Some(match command {
        Command::Move { target, .. } => (OrderKind::Move, vec![target]),
        Command::AttackMove { target, .. } => (OrderKind::AttackMove, vec![target]),
        Command::FormationMove {
            target,
            attack_move,
            ..
        } => (
            if *attack_move {
                OrderKind::AttackMove
            } else {
                OrderKind::Move
            },
            vec![target],
        ),
        Command::AttackGround { pos, .. } => (OrderKind::AttackGround, vec![pos]),
        Command::Bombard { pos, .. } => (OrderKind::Bombard, vec![pos]),
        Command::Orbit { pos, .. } => (OrderKind::Orbit, vec![pos]),
        Command::Guard { pos, .. } => (OrderKind::Guard, vec![pos]),
        Command::Patrol { points, .. } => (OrderKind::Patrol, points.iter_mut().collect()),
        Command::Attack { .. } => (OrderKind::Attack, Vec::new()),
        Command::Assist { .. } => (OrderKind::Assist, Vec::new()),
        _ => return None,
    })
}

/// The command's addressees and queue flag, if it is one a factory keeps for its products.
fn keepable(command: &mut Command) -> Option<(&mut Vec<UnitId>, &mut bool)> {
    match command {
        Command::FormationMove { units, queue, .. }
        | Command::Move { units, queue, .. }
        | Command::AttackMove { units, queue, .. }
        | Command::Attack { units, queue, .. }
        | Command::Orbit { units, queue, .. }
        | Command::Assist { units, queue, .. }
        | Command::AttackGround { units, queue, .. }
        | Command::Bombard { units, queue, .. }
        | Command::Patrol { units, queue, .. }
        | Command::Guard { units, queue, .. } => Some((units, queue)),
        _ => None,
    }
}

impl World {
    /// Factories in `ids` that `player` owns, finished or still being built.
    pub(crate) fn owned_factories(&self, player: u8, ids: &[UnitId]) -> Vec<usize> {
        let units = &self.state.units;
        ids.iter()
            .filter_map(|id| units.row(*id))
            .filter(|&row| {
                units.owner[row] == player
                    && !units.has_flag(row, flag::IN_FACTORY)
                    && self.bp(row).has(cat::FACTORY)
            })
            .collect()
    }

    /// `ids`' own units that carry orders, and the factories among them still being built.
    pub(crate) fn owned_or_rising(&self, player: u8, ids: &[UnitId]) -> Vec<usize> {
        let mut rows = self.owned(player, ids, 0);
        for row in self.owned_factories(player, ids) {
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
        rows
    }

    /// Keeps, edits or drops the standing orders of the factories `command` addresses.
    pub(crate) fn take_standing(&mut self, player: u8, command: &Command) {
        let ids: Vec<UnitId> = match command {
            Command::Stop { units }
            | Command::RelocateOrder { units, .. }
            | Command::CancelOrder { units, .. }
            | Command::PatrolInsert { units, .. } => units.clone(),
            Command::SetRally { factories, .. } => factories.clone(),
            other => match keepable(&mut other.clone()) {
                Some((units, _)) => units.clone(),
                None => return,
            },
        };
        let rows = self.owned_factories(player, &ids);
        if rows.is_empty() {
            return;
        }
        // Where a moved or added post lands, clamped the way the order itself would be.
        let clamped = match command {
            Command::RelocateOrder { to, .. } => self.clamp_to_map(*to),
            Command::PatrolInsert { point, .. } => self.clamp_to_map(*point),
            _ => FxVec2::ZERO,
        };
        // Attack and assist are found by where their unit is now.
        let target_at = |units: &Units, c: &Command| match c {
            Command::Attack { target, .. } | Command::Assist { target, .. } => {
                units.row(*target).map(|t| units.pos[t])
            }
            _ => None,
        };
        for row in rows {
            let mut standing = std::mem::take(&mut self.state.units.standing[row]);
            match command {
                // A new rally point is a plain move out; `run_produce` makes it.
                Command::Stop { .. } | Command::SetRally { .. } => standing.clear(),
                Command::RelocateOrder { kind, from, .. } => {
                    for c in standing.iter_mut() {
                        if let Some((k, points)) = points_mut(c) {
                            for p in points.into_iter().filter(|p| k == *kind && **p == *from) {
                                *p = clamped;
                            }
                        }
                    }
                }
                Command::CancelOrder { kind, pos, .. } => {
                    let units = &self.state.units;
                    standing.retain_mut(|c| {
                        let target = target_at(units, c);
                        match points_mut(c) {
                            Some((k, points)) if k == *kind => {
                                if points.is_empty() {
                                    return target != Some(*pos);
                                }
                            }
                            _ => return true,
                        }
                        match c {
                            Command::Patrol { points, .. } => {
                                points.retain(|p| p != pos);
                                !points.is_empty()
                            }
                            c => {
                                points_mut(c).is_some_and(|(_, ps)| ps.iter().all(|p| **p != *pos))
                            }
                        }
                    });
                }
                Command::PatrolInsert { after, .. } => {
                    for c in standing.iter_mut() {
                        if let Command::Patrol { points, .. } = c {
                            if points.len() < MAX_PATROL_POINTS {
                                if let Some(i) = points.iter().position(|p| p == after) {
                                    points.insert(i + 1, clamped);
                                    break;
                                }
                            }
                        }
                    }
                }
                other => {
                    let mut kept = other.clone();
                    if let Some((units, queue)) = keepable(&mut kept) {
                        units.clear();
                        // Products take them one after another, behind nothing.
                        if !std::mem::replace(queue, true) {
                            standing.clear();
                        }
                        if standing.len() < MAX_STANDING {
                            standing.push(kept);
                        }
                    }
                }
            }
            self.state.units.standing[row] = standing;
        }
    }

    /// `factories` drop their standing orders and rally point for `from`'s, so there is
    /// one patrol, not theirs and its one after the other. A rally point left on `from`
    /// itself (none set) stays none: each keeps its own default way out.
    pub(crate) fn copy_standing(&mut self, player: u8, factories: &[UnitId], from: UnitId) {
        let Some(&src) = self.owned_factories(player, &[from]).first() else {
            return;
        };
        let units = &mut self.state.units;
        let standing = units.standing[src].clone();
        let rally = (units.rally[src] != units.pos[src]).then_some(units.rally[src]);
        for row in self.owned_factories(player, factories) {
            if row == src {
                continue;
            }
            let units = &mut self.state.units;
            units.standing[row] = standing.clone();
            units.rally[row] = rally.unwrap_or(units.pos[row]);
        }
    }

    /// Gives a product that has just rolled out its factory's standing orders, and the
    /// factory's stance. False if it came away with nothing to do.
    pub(crate) fn inherit_standing(
        &mut self,
        factory: usize,
        product: usize,
    ) -> Result<bool, SimError> {
        let units = &mut self.state.units;
        units.fire_state[product] = units.fire_state[factory];
        if units.standing[factory].is_empty() {
            return Ok(false);
        }
        let owner = units.owner[factory];
        let id = units.id(product);
        for mut command in units.standing[factory].clone() {
            if let Some((units, _)) = keepable(&mut command) {
                *units = vec![id];
            }
            self.apply_as(owner, &command)?;
        }
        Ok(self
            .state
            .orders
            .front(&self.state.units, product)
            .is_some())
    }

    /// A factory's standing orders, front first, as the interface draws them.
    pub(crate) fn standing_view(&self, row: usize) -> Vec<StandingView> {
        let units = &self.state.units;
        let mut out = Vec::new();
        for c in &units.standing[row] {
            let mut c = c.clone();
            let (target, radius) = match &c {
                Command::Attack { target, .. } | Command::Assist { target, .. } => {
                    match units.row(*target) {
                        Some(t) => (Some(units.pos[t]), mc_core::Fx::ZERO),
                        None => continue,
                    }
                }
                Command::Orbit { radius, .. }
                | Command::Bombard { radius, .. }
                | Command::Guard { radius, .. } => (None, *radius),
                _ => (None, mc_core::Fx::ZERO),
            };
            let Some((kind, points)) = points_mut(&mut c) else {
                continue;
            };
            if let Some(pos) = target {
                out.push(StandingView {
                    kind,
                    at: pos,
                    pos,
                    radius,
                });
            }
            for p in points {
                out.push(StandingView {
                    kind,
                    at: *p,
                    pos: *p,
                    radius,
                });
            }
        }
        out
    }
}
