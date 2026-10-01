//! Operations (`docs/AI_COMMANDER.md`, "Operations"): groups of units with one goal.
//! This file keeps them: drops dead members (their cost to the ledger), credits
//! kills, frees the units of finished operations, and hands free units to the
//! operations that want them. The orders each kind gives are in `ops_*.rs`.
use super::profile::{role, Domain, Profile, Profiles};
use super::state::{Ledger, OpKind, Operation, Phase, PlanKind};
use crate::tables::{flag, Handle, UnitId};
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::cat;

/// Most units one operation takes: one order's worth.
pub(in crate::ai) const MOST_UNITS: usize = crate::command::MAX_COMMAND_UNITS;

/// Whether a unit of profile `p` belongs in an operation of `kind`.
pub(in crate::ai) fn fits(kind: OpKind, p: &Profile, land_route: bool) -> bool {
    let ground = matches!(p.domain, Some(Domain::Land | Domain::Hover));
    let crosses = p.domain == Some(Domain::Hover);
    match kind {
        OpKind::Army | OpKind::Defend => {
            ground
                && (land_route || crosses)
                && p.roles
                    & (role::LINE | role::ARTILLERY | role::ANTI_AIR | role::SHIELD | role::SENSOR)
                    != 0
        }
        OpKind::Raid => ground && (land_route || crosses) && p.has(role::RAIDER),
        OpKind::Siege => ground && p.has(role::ARTILLERY),
        OpKind::Landing => (ground && p.armed() && p.room > 0) || p.has(role::TRANSPORT),
        OpKind::Strike => p.domain == Some(Domain::Air) && p.has(role::STRIKE),
        OpKind::AirGuard => p.domain == Some(Domain::Air) && p.has(role::ANTI_AIR),
        OpKind::Fleet => p.domain == Some(Domain::Naval) && p.armed(),
        OpKind::Wolfpack => p.domain == Some(Domain::Sub) && p.armed(),
        OpKind::Warships => p.domain == Some(Domain::Space) && p.armed() && !p.has(role::TRANSPORT),
        OpKind::Scout => {
            p.roles & (role::SCOUT | role::SENSOR) != 0 && !p.armed() || p.has(role::SCOUT)
        }
    }
}

impl World {
    /// A new operation for `plan`, gathering at `rally`.
    pub(in crate::ai) fn open_op(
        &mut self,
        player: u8,
        kind: OpKind,
        plan: PlanKind,
        rally: FxVec2,
        target: FxVec2,
        want: Fx,
    ) -> u32 {
        let tick = self.state.tick;
        let c = &mut self.state.ai[player as usize].commander;
        c.next_op = c.next_op.wrapping_add(1);
        let id = c.next_op;
        c.ops.push(Operation {
            id,
            kind,
            plan,
            phase: Phase::Gathering,
            units: Vec::new(),
            target,
            rally,
            since: tick,
            phase_since: tick,
            want,
            ledger: Ledger::default(),
            carrier: Handle::NONE,
            ordered: 0,
        });
        id
    }

    /// Drops dead members (their cost lost) and finished operations, and credits
    /// the kills made since the last think to the operations whose units made them.
    /// The trades of finished operations go to their plans.
    pub(in crate::ai) fn ops_upkeep(&mut self, player: u8) {
        let units = &self.state.units;
        let alive = |id: UnitId| units.row(id).is_some();
        let c = &mut self.state.ai[player as usize].commander;
        let kills = std::mem::take(&mut c.kills);
        for (killer, mass) in kills {
            if let Some(op) = c
                .ops
                .iter_mut()
                .find(|o| o.has(killer) || o.carrier == killer)
            {
                op.ledger.killed += mass;
            }
        }
        for op in &mut c.ops {
            let before = op.units.len();
            let mut lost = Fx::ZERO;
            op.units.retain(|&(id, cost)| {
                let keep = alive(id);
                if !keep {
                    lost += cost;
                }
                keep
            });
            op.ledger.lost += lost;
            if !alive(op.carrier) {
                op.carrier = Handle::NONE;
            }
            // An operation that has gone and lost everyone is over.
            if op.units.is_empty() && before > 0 && op.phase != Phase::Gathering {
                op.phase = Phase::Done;
            }
        }
        let mut finished = Vec::new();
        c.ops.retain(|o| {
            if o.phase == Phase::Done {
                finished.push((o.plan, o.ledger));
                false
            } else {
                true
            }
        });
        for (plan, ledger) in finished {
            if let Some(p) = c.plans.iter_mut().find(|p| p.kind == plan) {
                p.ledger.add(&ledger);
            }
        }
    }

    /// The side's own units no operation holds, that an operation could use: armed
    /// mobile units, transports, scouts and sensors, finished and out of factories.
    pub(in crate::ai) fn free_units(&self, player: u8, profiles: &Profiles) -> Vec<usize> {
        let units = &self.state.units;
        let ops = &self.state.ai[player as usize].commander.ops;
        let held = |id: UnitId| ops.iter().any(|o| o.has(id) || o.carrier == id);
        units
            .slots
            .iter()
            .filter(|&r| {
                let bp = self.bp(r);
                let p = profiles.get(units.blueprint[r]);
                units.owner[r] == player
                    && units.is_active(r)
                    && !units.has_flag(r, flag::IN_FACTORY)
                    && units.drone_parent[r] == Handle::NONE
                    && !bp.has(cat::COMMANDER | cat::ENGINEER)
                    && bp.builder.is_none()
                    && !bp.is_salvager()
                    && p.mobile()
                    && (p.armed()
                        || p.roles & (role::TRANSPORT | role::SCOUT | role::SENSOR | role::SHIELD)
                            != 0)
                    && !held(units.id(r))
            })
            .collect()
    }

    /// Hands `free` units to the operations that want them, in the order the
    /// operations stand (the plans put the most urgent first). Units no operation
    /// wants stay free; the plans open an operation for them next think.
    pub(in crate::ai) fn assign(
        &mut self,
        player: u8,
        profiles: &Profiles,
        free: &[usize],
        land_route: bool,
    ) {
        let units = &self.state.units;
        let mut free: Vec<usize> = free.to_vec();
        let c = &mut self.state.ai[player as usize].commander;
        for op in &mut c.ops {
            if matches!(op.phase, Phase::Done | Phase::Withdrawing) {
                continue;
            }
            // A landing takes one ship, then cargo while it boards.
            if op.kind == OpKind::Landing {
                if op.carrier == Handle::NONE {
                    if let Some(i) = free
                        .iter()
                        .position(|&r| profiles.get(units.blueprint[r]).has(role::TRANSPORT))
                    {
                        op.carrier = units.id(free.remove(i));
                    }
                }
                continue;
            }
            // Only gathering operations take new units, except the standing ones
            // that soak up every unit of their kind.
            let soaks = matches!(
                op.kind,
                OpKind::Army
                    | OpKind::AirGuard
                    | OpKind::Fleet
                    | OpKind::Wolfpack
                    | OpKind::Warships
                    | OpKind::Scout
                    | OpKind::Strike
            );
            if op.phase != Phase::Gathering && !soaks {
                continue;
            }
            let room = MOST_UNITS.saturating_sub(op.units.len());
            let limit = match op.kind {
                OpKind::Raid => (op.want / Fx::from_int(60)).floor_int().clamp(2, 12) as usize,
                OpKind::Scout => 4,
                _ => room,
            }
            .min(room);
            let mut took = 0;
            free.retain(|&r| {
                if took >= limit || op.units.len() >= MOST_UNITS {
                    return true;
                }
                let p = profiles.get(units.blueprint[r]);
                if !fits(op.kind, p, land_route) || p.has(role::TRANSPORT) {
                    return true;
                }
                op.units.push((units.id(r), p.mass));
                op.ledger.committed += p.mass;
                took += 1;
                false
            });
        }
    }

    /// Rows of `op`'s units still alive and in the world (not in a hold or in warp).
    pub(in crate::ai) fn op_rows(&self, op: &Operation) -> Vec<usize> {
        let units = &self.state.units;
        op.units
            .iter()
            .filter_map(|(id, _)| units.row(*id))
            .filter(|&r| units.is_active(r))
            .collect()
    }

    /// The middle of `rows`, and their mass.
    pub(in crate::ai) fn centre_of(&self, rows: &[usize]) -> Option<FxVec2> {
        let first = *rows.first()?;
        let base = self.state.units.pos[first];
        let sum = rows
            .iter()
            .fold(FxVec2::ZERO, |s, &r| s + (self.state.units.pos[r] - base));
        let n = rows.len() as i32;
        Some(base + FxVec2::new(sum.x / n, sum.y / n))
    }

    /// Whether most of `rows` have no orders left, or stand where they were sent.
    pub(in crate::ai) fn mostly_idle(&self, rows: &[usize], arrived: &[usize]) -> bool {
        let units = &self.state.units;
        let idle = rows
            .iter()
            .filter(|&&r| {
                units.order_head[r] == crate::tables::NO_ORDER || arrived.binary_search(&r).is_ok()
            })
            .count();
        idle * 3 >= rows.len() * 2
    }
}
