//! Orders for the operations on the ground: the army, defence, raids and siege.
//! Every order goes to a group; units fight with their own behaviour.
use super::profile::{role, Target};
use super::state::{OpKind, Operation, Phase, PlanKind};
use super::Ctx;
use crate::command::Command;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::cat;

/// Enemy damage a second worth one mass of fighting strength: a T1 tank is about
/// 20 damage a second for 100 mass.
pub(in crate::ai) const THREAT_MASS: i32 = 5;
/// A group falls back, as a group, once what it faces is this many times it.
const OUTMATCHED: Fx = Fx::from_int(2);
/// An engineer seen within this many ticks is still near where it was seen.
const FRESH: u32 = 300;

impl World {
    /// Enemy strength (in mass) around `at`: what we remember standing there, or the
    /// fire that reaches it, whichever says more.
    pub(in crate::ai) fn enemy_strength_at(&self, ctx: &Ctx, at: FxVec2, r: Fx, t: Target) -> Fx {
        let seen: Fx = self.state.ai[ctx.player as usize]
            .contacts
            .iter()
            .filter(|c| c.pos.distance(at) <= r)
            .map(|c| (c, ctx.profiles.get(c.blueprint)))
            .filter(|(_, p)| p.hits(t))
            .map(|(c, _)| {
                let bp = self.blueprints.unit(c.blueprint);
                if bp.has(cat::COMMANDER) {
                    Fx::from_int(2000)
                } else {
                    bp.cost_mass
                }
            })
            .sum();
        seen.max(ctx.wm.threat_at(at, t) * THREAT_MASS)
    }

    /// Where a land group of `mass` at `from` should go: the enemy value it can win
    /// at, most for the walk. Value near a target counts; fire on it counts against.
    pub(in crate::ai) fn land_objective(
        &self,
        ctx: &Ctx,
        from: FxVec2,
        mass: Fx,
        soft: bool,
    ) -> Option<FxVec2> {
        let tick = self.state.tick;
        let contacts = &self.state.ai[ctx.player as usize].contacts;
        // Put ashore off home's ground, a group takes whatever is about it.
        let ashore = !ctx.can_walk(from);
        contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                let p = ctx.profiles.get(c.blueprint);
                let soft_kind = bp.has(cat::EXTRACTOR)
                    || bp.has(cat::POWER)
                    || (bp.has(cat::ENGINEER)
                        && !bp.has(cat::COMMANDER)
                        && tick.saturating_sub(c.seen) <= FRESH);
                let structure = bp.is_structure() && !p.has(role::DEFENSE);
                (if soft {
                    soft_kind
                } else {
                    structure || bp.has(cat::COMMANDER)
                }) && (ashore || ctx.can_walk(c.pos))
            })
            .filter_map(|c| {
                let threat = self.enemy_strength_at(ctx, c.pos, Fx::from_int(700), Target::Land);
                (threat * OUTMATCHED <= mass * 2).then(|| {
                    let value: Fx = contacts
                        .iter()
                        .filter(|o| o.pos.distance(c.pos) < Fx::from_int(600))
                        .map(|o| self.blueprints.unit(o.blueprint).cost_mass)
                        .sum();
                    let walk = c.pos.distance(from).floor_int() as i64 + 1500;
                    let score = (value - threat / 2).floor_int() as i64 * 1000 / walk;
                    (score, c.pos)
                })
            })
            .max_by_key(|&(s, p)| (s, std::cmp::Reverse((p.x, p.y))))
            .map(|(_, p)| p)
            .or_else(|| {
                (!soft)
                    .then_some(ctx.enemy_start)
                    .flatten()
                    .filter(|s| ctx.can_walk(*s))
            })
    }

    /// One think of a land operation (`Army`, `Raid`, `Siege`, `Defend`, and a landing
    /// once ashore): gather, go, judge, fall back. Returns the orders given.
    pub(in crate::ai) fn run_land_op(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        if rows.is_empty() {
            if op.kind != OpKind::Army && op.phase != Phase::Gathering {
                op.phase = Phase::Done;
            }
            return 0;
        }
        let centre = self.centre_of(&rows).unwrap_or(op.rally);
        let mass: Fx = rows.iter().map(|&r| self.bp(r).cost_mass).sum();
        let tick = self.state.tick;
        let ids = |rows: &[usize]| self.ids_of(rows);
        let mut orders = 0;
        match op.phase {
            Phase::Gathering => {
                // Stragglers to the rally point; the blob's edge grows with its size.
                let r = Fx::from_int(200 + 12 * (rows.len() as i32).isqrt());
                let far: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&row| {
                        self.state.units.pos[row].distance(op.rally) > r * 2
                            && self.idle_or_arrived(ctx, row, op.rally)
                    })
                    .collect();
                if !far.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids(&far),
                        target: op.rally,
                        queue: false,
                    });
                    orders += 1;
                }
                let gathered: Fx = rows
                    .iter()
                    .filter(|&&row| self.state.units.pos[row].distance(op.rally) <= r * 3)
                    .map(|&row| self.bp(row).cost_mass)
                    .sum();
                if op.kind == OpKind::Defend || (op.want > Fx::ZERO && gathered >= op.want) {
                    let soft = op.kind == OpKind::Raid;
                    let target = match op.kind {
                        OpKind::Defend | OpKind::Landing => Some(op.target),
                        OpKind::Siege => self.siege_spot(ctx, op.rally, &rows),
                        _ => self.land_objective(ctx, op.rally, gathered, soft),
                    };
                    if let Some(target) = target {
                        op.target = target;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: ids(&rows),
                            target,
                            queue: false,
                        });
                        orders += 1;
                    }
                }
            }
            Phase::Executing => {
                let facing = self.enemy_strength_at(ctx, centre, Fx::from_int(900), Target::Land);
                let ashore = op.kind == OpKind::Landing && !ctx.land_route;
                if facing > mass * OUTMATCHED && op.kind != OpKind::Defend && !ashore {
                    op.phase = Phase::Withdrawing;
                    op.phase_since = tick;
                    out.push(Command::Move {
                        units: ids(&rows),
                        target: op.rally,
                        queue: false,
                    });
                    return orders + 1;
                }
                // Units that joined on the way, still far behind: to the group.
                let behind: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&row| {
                        self.state.units.pos[row].distance(centre) > Fx::from_int(1500)
                            && self.idle_or_arrived(ctx, row, centre)
                    })
                    .collect();
                if !behind.is_empty() {
                    out.push(Command::AttackMove {
                        units: ids(&behind),
                        target: centre,
                        queue: false,
                    });
                    orders += 1;
                }
                if !self.mostly_idle(&rows, &ctx.arrived) {
                    return orders;
                }
                // Done there: on to the next, or back to gather.
                let next = match op.kind {
                    OpKind::Defend => None,
                    OpKind::Siege => self.siege_spot(ctx, centre, &rows),
                    OpKind::Raid => self.land_objective(ctx, centre, mass, true),
                    _ => self.land_objective(ctx, centre, mass, false),
                }
                .filter(|t| t.distance(centre) > Fx::from_int(150));
                match next {
                    Some(target) => {
                        op.target = target;
                        out.push(Command::AttackMove {
                            units: ids(&rows),
                            target,
                            queue: false,
                        });
                        orders += 1;
                    }
                    None if op.kind == OpKind::Army || op.kind == OpKind::Siege => {
                        op.phase = Phase::Withdrawing;
                        op.phase_since = tick;
                        out.push(Command::Move {
                            units: ids(&rows),
                            target: op.rally,
                            queue: false,
                        });
                        orders += 1;
                    }
                    None => op.phase = Phase::Done,
                }
            }
            Phase::Withdrawing => {
                if centre.distance(op.rally) < Fx::from_int(700)
                    || (self.mostly_idle(&rows, &ctx.arrived) && tick > op.phase_since + 300)
                {
                    op.phase = if matches!(op.kind, OpKind::Army | OpKind::Siege) {
                        Phase::Gathering
                    } else {
                        Phase::Done
                    };
                    op.phase_since = tick;
                }
            }
            Phase::Done => {}
        }
        orders
    }

    /// Whether `row` has nothing more to do, or stands where it was sent near `to`.
    pub(in crate::ai) fn idle_or_arrived(&self, ctx: &Ctx, row: usize, _to: FxVec2) -> bool {
        self.state.units.order_head[row] == crate::tables::NO_ORDER
            || ctx.arrived.binary_search(&row).is_ok()
    }

    /// Where artillery stands to shell the enemy's defences from beyond their reach:
    /// short of the nearest fortified spot by most of the guns' range.
    pub(in crate::ai) fn siege_spot(
        &self,
        ctx: &Ctx,
        from: FxVec2,
        rows: &[usize],
    ) -> Option<FxVec2> {
        let range = rows
            .iter()
            .map(|&r| {
                ctx.profiles.get(self.state.units.blueprint[r]).reach[Target::Structure as usize]
            })
            .filter(|r| *r > Fx::ZERO)
            .min()?;
        let target = self.state.ai[ctx.player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let p = ctx.profiles.get(c.blueprint);
                p.has(role::DEFENSE) || self.blueprints.unit(c.blueprint).has(cat::FACTORY)
            })
            .map(|c| c.pos)
            .filter(|p| ctx.can_walk(*p))
            .min_by_key(|p| (p.distance_sq(from), p.x, p.y))?;
        Some(super::super::offset_toward(
            target,
            from,
            range * Fx::ratio(17, 20),
        ))
    }

    /// Opens a defence for each raid on home or an outlying mine no defence answers,
    /// taking the nearest gathering units of the land operations and free ones.
    pub(in crate::ai) fn raise_defence(&mut self, ctx: &Ctx, threats: &[(FxVec2, FxVec2)]) {
        for &(at, enemy) in threats {
            let c = &self.state.ai[ctx.player as usize].commander;
            if c.ops
                .iter()
                .any(|o| o.kind == OpKind::Defend && o.target.distance(enemy) < Fx::from_int(800))
            {
                continue;
            }
            let base = enemy.distance(ctx.start) < Fx::from_int(700);
            let need = self.enemy_strength_at(ctx, enemy, Fx::from_int(600), Target::Land) * 3 / 2
                + Fx::from_int(200);
            let id = self.open_op(
                ctx.player,
                OpKind::Defend,
                PlanKind::Pressure,
                at,
                enemy,
                Fx::ZERO,
            );
            // Borrow from land operations that are gathering, nearest first.
            let units = &self.state.units;
            let mut cands: Vec<(usize, usize, crate::tables::UnitId, Fx)> = Vec::new();
            let c = &self.state.ai[ctx.player as usize].commander;
            for (oi, o) in c.ops.iter().enumerate() {
                if !matches!(o.kind, OpKind::Army | OpKind::Siege | OpKind::Raid)
                    || (o.phase != Phase::Gathering && !base)
                {
                    continue;
                }
                for (ui, (u, cost)) in o.units.iter().enumerate() {
                    if let Some(r) = units.row(*u) {
                        if base || units.pos[r].distance(enemy) < Fx::from_int(2500) {
                            cands.push((oi, ui, *u, *cost));
                        }
                    }
                }
            }
            cands.sort_by_key(|&(_, _, u, _)| {
                (
                    units
                        .row(u)
                        .map_or(Fx::from_int(1 << 30), |r| units.pos[r].distance(enemy)),
                    u,
                )
            });
            // The army near enough goes as a whole, the way a player sends it: a
            // few units at a time were picked off. Far off, only what the raid needs.
            // Outmatched away from the base, the mine is let go: two tanks sent
            // at a raiding party of six were lost for nothing.
            let can: Fx = cands.iter().map(|c| c.3).sum();
            if !base && can * 4 < need * 5 {
                let c = &mut self.state.ai[ctx.player as usize].commander;
                if let Some(d) = c.ops.iter_mut().find(|o| o.id == id) {
                    d.phase = Phase::Done;
                }
                continue;
            }
            let mut got = Fx::ZERO;
            let mut take: Vec<(crate::tables::UnitId, Fx)> = Vec::new();
            for &(_, _, u, cost) in &cands {
                let near = units
                    .row(u)
                    .is_some_and(|r| units.pos[r].distance(enemy) < Fx::from_int(3000));
                if got >= need && !base && !near {
                    break;
                }
                got += cost;
                take.push((u, cost));
            }
            let c = &mut self.state.ai[ctx.player as usize].commander;
            for o in &mut c.ops {
                o.units.retain(|(u, _)| !take.iter().any(|(t, _)| t == u));
            }
            if let Some(d) = c.ops.iter_mut().find(|o| o.id == id) {
                d.units = take;
                if d.units.is_empty() {
                    d.phase = Phase::Done;
                }
            }
        }
        // A defence with nothing left to answer is over: its units go back to the pool.
        let player = ctx.player;
        let done: Vec<u32> = self.state.ai[player as usize]
            .commander
            .ops
            .iter()
            .filter(|o| o.kind == OpKind::Defend)
            .filter(|o| {
                !threats
                    .iter()
                    .any(|(_, e)| e.distance(o.target) < Fx::from_int(900))
            })
            .map(|o| o.id)
            .collect();
        for o in &mut self.state.ai[player as usize].commander.ops {
            if done.contains(&o.id) {
                o.phase = Phase::Done;
            }
        }
    }
}
