//! Orders for warships, scouts and landings: everything that warps.
use super::ops_ground::THREAT_MASS;
use super::profile::{role, Target};
use super::state::{OpKind, Operation, Phase};
use super::Ctx;
use crate::command::Command;
use crate::tables::{Handle, OrderKind, WarpPhase, NO_ORDER};
use crate::World;
use mc_core::{Fx, FxVec2};

/// Warships this close to their gathering point have gathered: hulls are long.
const SPACE_HOME: Fx = Fx::from_int(700);
/// A landing comes down this far short of its target.
const LANDING_STANDOFF: Fx = Fx::from_int(420);
/// Ticks units may take to board before the ship goes with what it has.
const BOARD_TICKS: u32 = 600;
/// A landing that has not got away in this long is given up.
const GIVE_UP: u32 = 2400;

impl World {
    /// Armed spaceships: gather, then jump as one at the target worth most against
    /// the anti-air there; come home as one, by warp, once the group is worn down.
    pub(in crate::ai) fn run_warships(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        if rows.is_empty() {
            return 0;
        }
        let tick = self.state.tick;
        let mass: Fx = rows.iter().map(|&r| self.bp(r).cost_mass).sum();
        let health: Fx = rows
            .iter()
            .map(|&r| {
                self.state.units.health[r] * self.bp(r).cost_mass / self.bp(r).health.max(Fx::ONE)
            })
            .sum();
        let centre = self.centre_of(&rows).unwrap_or(op.rally);
        match op.phase {
            Phase::Gathering => {
                let at_home: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&r| self.state.units.pos[r].distance(op.rally) <= SPACE_HOME)
                    .collect();
                let away: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|r| !at_home.contains(r) && self.state.units.order_head[*r] == NO_ORDER)
                    .collect();
                let mut orders = 0;
                if !away.is_empty() {
                    for &r in &away {
                        self.go_home(r, op.rally, out);
                    }
                    orders += 1;
                }
                let gathered: Fx = at_home.iter().map(|&r| self.bp(r).cost_mass).sum();
                if gathered >= op.want && op.want > Fx::ZERO {
                    let target = self.strike_target(ctx, op.rally, gathered).or_else(|| {
                        self.attack_target(
                            ctx.player,
                            op.rally,
                            ctx.intel,
                            super::super::Stance::Expand,
                        )
                    });
                    if let Some(t) = target {
                        op.target = t;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        op.want = gathered;
                        self.jump_then_attack(ctx.player, &at_home, t, op.rally, out);
                        orders += 1;
                    }
                }
                orders
            }
            Phase::Executing => {
                // Worn below half of what went: the group jumps home together.
                let aa = ctx.wm.threat_at(centre, Target::Space) * THREAT_MASS;
                if health * 2 < op.want || aa > mass * 3 {
                    op.phase = Phase::Withdrawing;
                    op.phase_since = tick;
                    for &r in &rows {
                        self.go_home(r, op.rally, out);
                    }
                    return 1;
                }
                if !rows
                    .iter()
                    .all(|&r| self.state.units.order_head[r] == NO_ORDER)
                {
                    return 0;
                }
                match self.strike_target(ctx, centre, mass) {
                    Some(t) if t.distance(centre) > Fx::from_int(300) => {
                        op.target = t;
                        self.jump_then_attack(ctx.player, &rows, t, centre, out);
                        1
                    }
                    _ => {
                        op.phase = Phase::Withdrawing;
                        for &r in &rows {
                            self.go_home(r, op.rally, out);
                        }
                        1
                    }
                }
            }
            Phase::Withdrawing => {
                if centre.distance(op.rally) < SPACE_HOME * 2 || tick > op.phase_since + 900 {
                    op.phase = Phase::Gathering;
                    op.phase_since = tick;
                }
                0
            }
            Phase::Done => 0,
        }
    }

    /// Scouts and sensor ships: each idle one goes where the side has looked least
    /// lately among the places that matter (enemy starts, the enemy's side of the
    /// map, where their army was seen). Sensor ships jump there.
    pub(in crate::ai) fn run_scouts(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        let mut orders = 0;
        let mut taken: Vec<FxVec2> = Vec::new();
        for &r in &rows {
            if self.state.units.order_head[r] != NO_ORDER {
                continue;
            }
            let Some(spot) = self.stalest(ctx, &taken) else {
                break;
            };
            taken.push(spot);
            let p = ctx.profiles.get(self.state.units.blueprint[r]);
            let id = vec![self.state.units.id(r)];
            let mark = self.safe_mark(
                ctx.player,
                super::super::offset_toward(spot, ctx.start, Fx::from_int(900)),
                ctx.start,
            );
            if p.has(role::WARP) && self.can_jump(r, mark) {
                out.push(Command::Warp {
                    units: id.clone(),
                    pos: mark,
                    queue: false,
                });
                out.push(Command::Move {
                    units: id,
                    target: spot,
                    queue: true,
                });
            } else {
                out.push(Command::Move {
                    units: id,
                    target: spot,
                    queue: false,
                });
            }
            orders += 1;
        }
        orders
    }

    /// The place most worth a look: least recently seen, weighted toward enemy
    /// starts and their side of the map, kept off anti-air and away from `taken`.
    fn stalest(&self, ctx: &Ctx, taken: &[FxVec2]) -> Option<FxVec2> {
        let tick = self.state.tick;
        let seen = &self.state.ai[ctx.player as usize].commander.seen;
        let enemy = ctx.enemy_start?;
        let span = enemy.distance(ctx.start).max(Fx::ONE);
        (0..seen.len())
            .map(|i| (i, ctx.wm.centre(i)))
            .filter(|(_, c)| !taken.iter().any(|t| t.distance(*c) < Fx::from_int(1500)))
            // Never where anti-air is known to be: three Vigils were lost over bases.
            .filter(|(_, c)| ctx.wm.threat_at(*c, Target::Air) == Fx::ZERO)
            .map(|(i, c)| {
                let age = tick.saturating_sub(seen[i]) as i64;
                // Their half counts double, their start four times.
                let theirs = (c.distance(ctx.start) * 2 > span) as i64;
                let home = (c.distance(enemy) < Fx::from_int(1200)) as i64;
                (age * (1 + theirs + 2 * home), c)
            })
            .max_by_key(|&(w, c)| (w, std::cmp::Reverse((c.x, c.y))))
            .filter(|&(w, _)| w > 900)
            .map(|(_, c)| c)
    }

    /// A landing: its lift ship takes cargo from the army gathering at staging,
    /// jumps short of the target, sets down and lets them out; ashore, the cargo
    /// fights as a land group, and the ship goes home to carry the next.
    pub(in crate::ai) fn run_landing(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let tick = self.state.tick;
        let units = &self.state.units;
        let ship = units.row(op.carrier);
        let age = tick.saturating_sub(op.phase_since);
        match (op.phase, ship) {
            // No ship yet: the plan has one built (`plans.rs`).
            (Phase::Gathering, None) => {
                if tick > op.since + GIVE_UP * 3 {
                    op.phase = Phase::Done;
                }
                0
            }
            (Phase::Gathering, Some(ship)) => {
                let aboard = self.cargo_used(ship);
                let boarding = op.units.iter().filter_map(|(u, _)| units.row(*u)).any(|r| {
                    units.hangar[r] == Handle::NONE
                        && self
                            .state
                            .orders
                            .front(units, r)
                            .is_some_and(|o| o.kind == OrderKind::Board)
                });
                // Cargo is taken before the operations run (`load_landings`).
                if op.units.is_empty() {
                    return 0;
                }
                if !(aboard > 0 && (!boarding || age > BOARD_TICKS)) {
                    if age > BOARD_TICKS * 3 {
                        op.phase = Phase::Done;
                    }
                    return 0;
                }
                // Aboard: where to put them down.
                let mass = op.mass();
                let Some(target) = self.landing_spot(ctx, mass) else {
                    return 0;
                };
                op.target = target;
                let from = self.state.units.pos[ship];
                let mark = self.safe_mark(
                    ctx.player,
                    super::super::offset_toward(target, from, LANDING_STANDOFF),
                    from,
                );
                let jump = self.can_jump(ship, mark);
                if jump {
                    out.push(Command::Warp {
                        units: vec![op.carrier],
                        pos: mark,
                        queue: false,
                    });
                }
                out.push(Command::Land {
                    units: vec![op.carrier],
                    pos: mark,
                    unload: true,
                    queue: jump,
                });
                op.phase = Phase::Executing;
                op.phase_since = tick;
                1
            }
            (Phase::Executing, Some(ship)) if self.cargo_used(ship) > 0 => {
                let busy = units.warp[ship].phase != WarpPhase::Idle
                    || self.state.orders.front(units, ship).is_some_and(|o| {
                        matches!(
                            o.kind,
                            OrderKind::Warp | OrderKind::Unload | OrderKind::Land
                        )
                    });
                if busy && age < GIVE_UP {
                    return 0;
                }
                // Stuck: let them out where it is.
                let cargo: Vec<crate::tables::UnitId> = units
                    .slots
                    .iter()
                    .filter(|&r| units.hangar[r] == op.carrier)
                    .map(|r| units.id(r))
                    .collect();
                out.push(Command::Unload { units: cargo });
                1
            }
            (Phase::Executing, carrier) => {
                // Ashore: the ship goes home for the next, the cargo fights on.
                if let Some(ship) = carrier {
                    let home = super::super::offset_toward(ctx.start, op.target, Fx::from_int(300));
                    out.push(Command::TakeOff {
                        units: vec![op.carrier],
                    });
                    let units = vec![op.carrier];
                    out.push(if self.can_jump(ship, home) {
                        Command::Warp {
                            units,
                            pos: home,
                            queue: true,
                        }
                    } else {
                        Command::Move {
                            units,
                            target: home,
                            queue: true,
                        }
                    });
                    op.carrier = Handle::NONE;
                }
                // Now a land group like any other, at its target.
                op.kind = OpKind::Raid;
                op.phase = Phase::Gathering;
                op.rally = self.centre_of(&self.op_rows(op)).unwrap_or(op.target);
                op.want = Fx::ZERO;
                let rows = self.op_rows(op);
                if rows.is_empty() {
                    return 1;
                }
                op.phase = Phase::Executing;
                op.phase_since = tick;
                out.push(Command::AttackMove {
                    units: self.ids_of(&rows),
                    target: op.target,
                    queue: false,
                });
                2
            }
            _ => 0,
        }
    }

    /// Land units of the gathering land operations that fit `ship`, nearest it,
    /// up to `capacity`: (id, mass, room).
    pub(in crate::ai) fn take_cargo(
        &self,
        ctx: &Ctx,
        ship: usize,
        capacity: u16,
    ) -> Vec<(crate::tables::UnitId, Fx, u16)> {
        let units = &self.state.units;
        let at = units.pos[ship];
        let mut cands: Vec<(crate::tables::UnitId, Fx, u16, Fx)> = Vec::new();
        for o in &self.state.ai[ctx.player as usize].commander.ops {
            if !matches!(o.kind, OpKind::Army) || o.phase != Phase::Gathering {
                continue;
            }
            for &(u, m) in &o.units {
                let Some(r) = units.row(u) else { continue };
                let room = self.bp(r).cargo_room().unwrap_or(0);
                if room == 0
                    || !self.cargo_fits(r, ship)
                    || units.pos[r].distance(at) > Fx::from_int(2500)
                {
                    continue;
                }
                cands.push((u, m, room, units.pos[r].distance(at)));
            }
        }
        cands.sort_by_key(|&(u, _, _, d)| (d, u));
        let mut left = capacity.saturating_sub(self.cargo_used(ship));
        let mut out = Vec::new();
        for (u, m, room, _) in cands {
            if room <= left {
                left -= room;
                out.push((u, m, room));
            }
        }
        out
    }

    /// Where a landing of `mass` goes: enemy value it can win at, on ground the
    /// enemy holds, the least anti-air and guns near. With nothing seen, the enemy
    /// start.
    fn landing_spot(&self, ctx: &Ctx, mass: Fx) -> Option<FxVec2> {
        let contacts = &self.state.ai[ctx.player as usize].contacts;
        contacts
            .iter()
            .filter(|c| self.blueprints.unit(c.blueprint).is_structure())
            .filter_map(|c| {
                let guns = self.enemy_strength_at(ctx, c.pos, Fx::from_int(800), Target::Land);
                let aa = ctx.wm.threat_at(c.pos, Target::Air) * THREAT_MASS;
                (guns * 2 <= mass * 3).then(|| {
                    let value: Fx = contacts
                        .iter()
                        .filter(|o| o.pos.distance(c.pos) < Fx::from_int(700))
                        .map(|o| self.blueprints.unit(o.blueprint).cost_mass)
                        .sum();
                    ((value - guns - aa).floor_int() as i64, c.pos)
                })
            })
            .max_by_key(|&(s, p)| (s, std::cmp::Reverse((p.x, p.y))))
            .map(|(_, p)| p)
            .or(ctx.enemy_start)
    }
}
