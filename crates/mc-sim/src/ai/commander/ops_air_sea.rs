//! Orders for the operations in the air and at sea: bomber strikes, fighter cover,
//! the surface fleet and the submarines.
use super::ops_ground::THREAT_MASS;
use super::profile::{role, Domain, Target};
use super::state::{Operation, Phase};
use super::Ctx;
use crate::command::Command;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::{cat, MoveLayer};

/// Aircraft this close to their gathering point have gathered.
const AIR_HOME: Fx = Fx::from_int(400);
/// Enemy aircraft this close to home or the army are hunted at once.
const AIR_COVER: Fx = Fx::from_int(1800);
/// Ships this close to the anchorage have gathered.
const FLEET_HOME: Fx = Fx::from_int(450);

impl World {
    /// The enemy economy most worth a strike from `from` for a wing that `aa`
    /// hurts: mines, power, factories and engineers, against the anti-air on the way.
    pub(in crate::ai) fn strike_target(&self, ctx: &Ctx, from: FxVec2, wing: Fx) -> Option<FxVec2> {
        let tick = self.state.tick;
        let strip = self.state.ai[ctx.player as usize]
            .commander
            .plan(super::state::PlanKind::Strategic)
            >= super::state::Stake::Invest;
        // Shelled lately: their artillery and map guns are a mark, and the first one.
        let shelled = self.state.ai[ctx.player as usize].commander.hurt
            [super::state::Hurt::Artillery as usize]
            > Fx::from_int(1000);
        let guns = |c: &crate::ai::adaptive::Contact| {
            let p = ctx.profiles.get(c.blueprint);
            p.has(role::ARTILLERY) || p.has(role::MAP_GUN)
        };
        self.state.ai[ctx.player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let bp = self.blueprints.unit(c.blueprint);
                (strip && ctx.profiles.get(c.blueprint).has(role::INTERCEPTOR))
                    || bp.has(cat::EXTRACTOR)
                    || bp.has(cat::POWER)
                    || bp.has(cat::FACTORY)
                    || ctx
                        .profiles
                        .get(c.blueprint)
                        .has(role::STRATEGIC | role::PROJECT)
                    || (bp.has(cat::ENGINEER) && tick.saturating_sub(c.seen) <= 300)
                    || (guns(c) && tick.saturating_sub(c.seen) <= 600)
            })
            .filter_map(|c| {
                let aa = ctx.wm.threat_along(from, c.pos, Target::Air) * THREAT_MASS;
                (aa <= wing).then(|| {
                    // While our warheads wait on their interceptors, those are the mark.
                    let strip = strip && ctx.profiles.get(c.blueprint).has(role::INTERCEPTOR);
                    let worth = self.blueprints.unit(c.blueprint).cost_mass
                        * if strip || (shelled && guns(c)) { 6 } else { 1 };
                    let score = (worth * 4 - aa).floor_int() as i64 * 1000
                        / (c.pos.distance(from).floor_int() as i64 + 2000);
                    (score, c.pos)
                })
            })
            .max_by_key(|&(s, p)| (s, std::cmp::Reverse((p.x, p.y))))
            .map(|(_, p)| p)
    }

    /// Bombers and gunships: gather at their hangar until the wing is big enough,
    /// strike as one, come home, and fall back as one if the anti-air is too much.
    pub(in crate::ai) fn run_strike(
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
        let centre = self.centre_of(&rows).unwrap_or(op.rally);
        match op.phase {
            Phase::Gathering => {
                let away: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&r| {
                        self.state.units.pos[r].distance(op.rally) > AIR_HOME
                            && self.state.units.order_head[r] == crate::tables::NO_ORDER
                    })
                    .collect();
                let mut orders = 0;
                if !away.is_empty() {
                    out.push(Command::Move {
                        units: self.ids_of(&away),
                        target: op.rally,
                        queue: false,
                    });
                    orders += 1;
                }
                let home: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&r| self.state.units.pos[r].distance(op.rally) <= AIR_HOME)
                    .collect();
                let gathered: Fx = home.iter().map(|&r| self.bp(r).cost_mass).sum();
                if gathered >= op.want && op.want > Fx::ZERO {
                    if let Some(target) = self.strike_target(ctx, op.rally, gathered) {
                        op.target = target;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: self.ids_of(&home),
                            target,
                            queue: false,
                        });
                        orders += 1;
                    }
                }
                orders
            }
            Phase::Executing => {
                let aa = ctx.wm.threat_at(centre, Target::Air) * THREAT_MASS;
                let done = rows
                    .iter()
                    .all(|&r| self.state.units.order_head[r] == crate::tables::NO_ORDER);
                if aa > mass * 2 || done || tick > op.phase_since + 1200 {
                    op.phase = Phase::Gathering;
                    op.phase_since = tick;
                    out.push(Command::Move {
                        units: self.ids_of(&rows),
                        target: op.rally,
                        queue: false,
                    });
                    return 1;
                }
                0
            }
            _ => {
                op.phase = Phase::Gathering;
                0
            }
        }
    }

    /// Fighters: hunt enemy aircraft over home or the army at once; fly cover over a
    /// strike going in; otherwise wait at the staging point.
    pub(in crate::ai) fn run_air_guard(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        escort: Option<FxVec2>,
        army: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        if rows.is_empty() {
            return 0;
        }
        let near = |p: FxVec2| {
            p.distance(ctx.start) <= AIR_COVER || army.is_some_and(|a| a.distance(p) <= AIR_COVER)
        };
        let intruder = self
            .visible_air_target(ctx.player, ctx.start)
            .filter(|&p| near(p))
            .or_else(|| {
                army.and_then(|a| self.visible_air_target(ctx.player, a))
                    .filter(|&p| near(p))
            });
        let idle: Vec<usize> = rows
            .iter()
            .copied()
            .filter(|&r| self.state.units.order_head[r] == crate::tables::NO_ORDER)
            .collect();
        let (target, all) = match (intruder, escort) {
            (Some(t), _) => (t, true),
            (None, Some(t)) => (t, false),
            (None, None) => (op.rally, false),
        };
        let send: Vec<usize> = if all { rows.clone() } else { idle };
        if send.is_empty() || (target == op.target && !all) {
            return 0;
        }
        op.target = target;
        let far: Vec<usize> = send
            .into_iter()
            .filter(|&r| all || self.state.units.pos[r].distance(target) > AIR_HOME)
            .collect();
        if far.is_empty() {
            return 0;
        }
        out.push(Command::AttackMove {
            units: self.ids_of(&far),
            target,
            queue: false,
        });
        1
    }

    /// Ships, surface or dived: gather at the anchorage, sail as one at the nearest
    /// contact they can reach, fall back as one when outmatched. Submarines go for
    /// ships and sea mines and keep off sonar where they can.
    pub(in crate::ai) fn run_fleet(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        let Some(&first) = rows.first() else {
            return 0;
        };
        let tick = self.state.tick;
        let motion = self.bp(first).motion.expect("a ship moves");
        let Some(sea) =
            self.sea_reach(motion.layer, motion.size_class, self.state.units.pos[first])
        else {
            return 0;
        };
        if op.rally == FxVec2::ZERO || !sea.reaches(op.rally) {
            if let Some(a) = sea.nearest(ctx.start) {
                op.rally = a;
            }
        }
        let mass: Fx = rows.iter().map(|&r| self.bp(r).cost_mass).sum();
        let centre = self.centre_of(&rows).unwrap_or(op.rally);
        let subs = ctx.profiles.get(self.state.units.blueprint[first]).domain == Some(Domain::Sub);
        let class = if subs {
            Target::Submerged
        } else {
            Target::Surface
        };
        let target = |w: &World, from: FxVec2| -> Option<FxVec2> {
            if subs {
                w.sub_target(ctx, &rows, &sea, from)
            } else {
                w.fleet_target(ctx.player, &rows, &sea, from)
                    .or_else(|| w.enemy_water(ctx.player, &sea, from))
            }
        };
        match op.phase {
            Phase::Gathering => {
                let away: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&r| {
                        self.state.units.pos[r].distance(op.rally) > FLEET_HOME
                            && self.idle_or_arrived(ctx, r, op.rally)
                    })
                    .collect();
                let mut orders = 0;
                if !away.is_empty() {
                    out.push(Command::Move {
                        units: self.ids_of(&away),
                        target: op.rally,
                        queue: false,
                    });
                    orders += 1;
                }
                let gathered: Fx = rows
                    .iter()
                    .filter(|&&r| self.state.units.pos[r].distance(op.rally) <= FLEET_HOME * 2)
                    .map(|&r| self.bp(r).cost_mass)
                    .sum();
                // A contact close to the anchorage is fought at once.
                let close =
                    target(self, op.rally).filter(|t| t.distance(op.rally) < Fx::from_int(900));
                if (gathered >= op.want && op.want > Fx::ZERO) || close.is_some() {
                    if let Some(t) = close.or_else(|| target(self, op.rally)) {
                        op.target = t;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: self.ids_of(&rows),
                            target: t,
                            queue: false,
                        });
                        orders += 1;
                    }
                }
                orders
            }
            Phase::Executing => {
                let facing = self.enemy_strength_at(ctx, centre, Fx::from_int(1200), class);
                if facing > mass * 2 {
                    op.phase = Phase::Withdrawing;
                    op.phase_since = tick;
                    out.push(Command::Move {
                        units: self.ids_of(&rows),
                        target: op.rally,
                        queue: false,
                    });
                    return 1;
                }
                if !self.mostly_idle(&rows, &ctx.arrived) {
                    return 0;
                }
                match target(self, centre).filter(|t| t.distance(centre) > Fx::from_int(200)) {
                    Some(t) => {
                        op.target = t;
                        out.push(Command::AttackMove {
                            units: self.ids_of(&rows),
                            target: t,
                            queue: false,
                        });
                        1
                    }
                    None => {
                        op.phase = Phase::Withdrawing;
                        out.push(Command::Move {
                            units: self.ids_of(&rows),
                            target: op.rally,
                            queue: false,
                        });
                        1
                    }
                }
            }
            Phase::Withdrawing => {
                if centre.distance(op.rally) < FLEET_HOME * 2 || tick > op.phase_since + 900 {
                    op.phase = Phase::Gathering;
                    op.phase_since = tick;
                }
                0
            }
            Phase::Done => 0,
        }
    }

    /// Submarines' prey: ships and sea mines on water they reach, the fewest sonar
    /// and torpedoes near first.
    fn sub_target(
        &self,
        ctx: &Ctx,
        rows: &[usize],
        sea: &crate::ai::sea::SeaReach,
        from: FxVec2,
    ) -> Option<FxVec2> {
        let bp = self.bp(rows[0]);
        let m = bp.motion?;
        let range = ctx.profiles.get(bp.id).reach[Target::Surface as usize];
        self.state.ai[ctx.player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let e = self.blueprints.unit(c.blueprint);
                let at_sea = e.motion.is_some_and(|m| m.layer == MoveLayer::Naval)
                    || (e.has(cat::EXTRACTOR) && e.water_build);
                at_sea && bp.can_attack(e)
            })
            .filter_map(|c| {
                let goal = self.nav.nearest_passable(m.layer, m.size_class, c.pos)?;
                (goal.distance(c.pos) <= range && sea.reaches(goal)).then(|| {
                    let sonar = ctx.wm.threat_at(goal, Target::Submerged) * THREAT_MASS;
                    let cost =
                        goal.distance(from).floor_int() as i64 / 4 + sonar.floor_int() as i64 * 3;
                    (cost, goal)
                })
            })
            .min_by_key(|&(c, p)| (c, p.x, p.y))
            .map(|(_, p)| p)
    }
}
