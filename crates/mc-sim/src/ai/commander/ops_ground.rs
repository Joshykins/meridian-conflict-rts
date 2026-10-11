//! Orders for the operations on the ground: the army, defence, raids and siege.
//! Every order goes to a group; units fight with their own behaviour.
use super::profile::{role, Target};
use super::state::{OpKind, Operation, Phase, PlanKind};
use super::Ctx;
use crate::command::Command;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::{cat, MoveLayer};

/// Enemy damage a second worth one materials of fighting strength: a T1 tank is about
/// 20 damage a second for 100 materials.
pub(in crate::ai) const THREAT_MATERIALS: i32 = 5;
/// A group falls back, as a group, once what it faces is this many times it.
pub(in crate::ai) const OUTMATCHED: Fx = Fx::from_int(2);
/// How near a target a hovercraft must be able to float for a hover army to go for it.
const HOVER_NEAR: i32 = 256;
/// How far from its rally point a gathering wave goes after enemy groups it beats.
const PREY_REACH: i32 = 2500;
/// Enemy materials below which an incursion is left to the turrets: a scout.
const SMALL_FRY: i32 = 120;
/// Enemies this close to what the side holds are in its ground.
const NEAR_HOLD: Fx = Fx::from_int(1200);
/// An engineer seen within this many ticks is still near where it was seen.
const FRESH: u32 = 300;

impl World {
    /// Enemy strength (in materials) around `at`: what we remember standing there, or the
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
                    bp.cost_materials
                }
            })
            .sum();
        seen.max(ctx.wm.threat_at(at, t) * THREAT_MATERIALS)
    }

    /// Where a land group of `materials` at `from` should go: the enemy value it can win
    /// at, most for the walk. Value near a target counts; fire on it counts against.
    pub(in crate::ai) fn land_objective(
        &self,
        ctx: &Ctx,
        from: FxVec2,
        materials: Fx,
        soft: bool,
    ) -> Option<FxVec2> {
        let tick = self.state.tick;
        let contacts = &self.state.ai[ctx.player as usize].contacts;
        // Put ashore off home's ground, a group takes whatever is about it.
        let ashore = !ctx.can_walk(from);
        // The fire on each zone's way from here, found once for every target.
        let fire = ctx
            .zones
            .of(from)
            .map_or_else(Vec::new, |a| ctx.zv.fire_along(ctx.zones, a));
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
                }) && (ashore || self.army_reaches(ctx, c.pos))
            })
            .filter_map(|c| {
                let threat = self.enemy_strength_at(ctx, c.pos, Fx::from_int(700), Target::Land);
                (threat * OUTMATCHED <= materials * 2).then(|| {
                    // What stands within 600 m of it, less the fire on the zones
                    // between. (Counting the target's whole zone, or its ore,
                    // sent waves at the strongest ground: 11 of 16 test matches
                    // were decided before, 6-9 after.)
                    let zone = ctx.zones.of(c.pos);
                    let value: Fx = contacts
                        .iter()
                        .filter(|o| o.pos.distance(c.pos) < Fx::from_int(600))
                        .map(|o| self.blueprints.unit(o.blueprint).cost_materials)
                        .sum();
                    let along = zone.map_or(Fx::ZERO, |b| {
                        fire.get(b as usize).copied().unwrap_or(Fx::ZERO)
                    });
                    // Near targets first, by the square of the walk: a wave sent
                    // 8 km at the enemy's base bled out on its defences on the way.
                    let walk = c.pos.distance(from).floor_int() as i64 / 100 + 15;
                    let score = (value - threat / 2 - along * THREAT_MATERIALS / 4).floor_int()
                        as i64
                        * 10_000
                        / (walk * walk);
                    (score, c.pos)
                })
            })
            .chain(self.remembered_objectives(ctx, from, materials, soft, &fire))
            .max_by_key(|&(s, p)| (s, std::cmp::Reverse((p.x, p.y))))
            .map(|(_, p)| p)
            .or_else(|| {
                (!soft)
                    .then_some(ctx.enemy_start)
                    .flatten()
                    .filter(|s| self.army_reaches(ctx, *s))
            })
    }

    /// Targets from the side's long memory (`knowledge.rs`): every zone where it knows
    /// the enemy has structures (mines and power only, for a raid), long after the
    /// contacts have forgotten them, scored as `land_objective` scores a contact. An
    /// expansion scouted five minutes ago was out of every wave's reckoning.
    fn remembered_objectives(
        &self,
        ctx: &Ctx,
        from: FxVec2,
        materials: Fx,
        soft: bool,
        fire: &[Fx],
    ) -> Vec<(i64, FxVec2)> {
        use super::knowledge::Built;
        let tick = self.state.tick;
        let ashore = !ctx.can_walk(from);
        let known = &self.state.ai[ctx.player as usize].commander.knowledge;
        let mut out = Vec::new();
        for (z, k) in known.iter().enumerate() {
            let Some(zone) = ctx.zones.zones.get(z) else {
                continue;
            };
            let at = zone.centre;
            if !(ashore || self.army_reaches(ctx, at)) {
                continue;
            }
            let (mut value, mut guarded) = (Fx::ZERO, Fx::ZERO);
            for h in &k.held {
                let sure = h.built_sure(tick);
                let b = |x: Built| h.built[x as usize] * sure / 100;
                value += if soft {
                    b(Built::Mine) + b(Built::Power)
                } else {
                    h.built_materials() * sure / 100
                };
                guarded += b(Built::Guns) + b(Built::Artillery) + h.army_now(tick);
            }
            if value == Fx::ZERO {
                continue;
            }
            let threat = self
                .enemy_strength_at(ctx, at, Fx::from_int(700), Target::Land)
                .max(guarded);
            if threat * OUTMATCHED > materials * 2 {
                continue;
            }
            let along = fire.get(z).copied().unwrap_or(Fx::ZERO);
            let walk = at.distance(from).floor_int() as i64 / 100 + 15;
            let score = (value - threat / 2 - along * THREAT_MATERIALS / 4).floor_int() as i64
                * 10_000
                / (walk * walk);
            out.push((score, at));
        }
        out
    }

    /// One think of a land operation (`Army`, `Raid`, `Siege`, `Defend`, and a landing
    /// once ashore): gather, go, judge, fall back. Returns the orders given.
    pub(in crate::ai) fn run_land_op(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        army_out: Option<FxVec2>,
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
        let materials: Fx = rows.iter().map(|&r| self.bp(r).cost_materials).sum();
        let tick = self.state.tick;
        let ids = |rows: &[usize]| self.ids_of(rows);
        let mut orders = 0;
        // Health lost since the last think, its units counted as they stand.
        let health: Fx = rows
            .iter()
            .map(|&r| self.state.units.health[r] * 100 / self.bp(r).health.max(Fx::ONE))
            .sum();
        let shelled = op.health > Fx::ZERO && health * 100 < op.health * 97;
        op.health = health;
        if op.phase == Phase::Gathering && shelled && op.kind != OpKind::Defend {
            // Under fire while gathering, from what it may not see: charge the guns
            // if it can win there, else the rally point steps back out of reach.
            // A wave sat idle under artillery and lost a unit every few seconds.
            // Hit from the air, by aircraft or a warship it cannot shoot back at:
            // home, under the base's anti-air. One corvette over a rally point
            // killed a wave of 44 tanks a unit at a time.
            let from_above = self.state.ai[ctx.player as usize].contacts.iter().any(|c| {
                let p = ctx.profiles.get(c.blueprint);
                matches!(
                    p.domain,
                    Some(super::profile::Domain::Air | super::profile::Domain::Space)
                ) && p.hits(Target::Land)
                    && c.pos.distance(centre) < Fx::from_int(1500)
            });
            let answers_air = rows.iter().any(|&r| {
                ctx.profiles
                    .get(self.state.units.blueprint[r])
                    .has(super::profile::role::ANTI_AIR)
            });
            if from_above && !answers_air {
                let units = self.ids_of(&rows);
                let c = &mut self.state.ai[ctx.player as usize].commander;
                c.rally_back = (c.rally_back + Fx::from_int(800)).min(Fx::from_int(2400));
                out.push(Command::Move {
                    units,
                    target: super::super::offset_toward(ctx.start, centre, Fx::from_int(200)),
                    queue: false,
                });
                return 1;
            }
            // Guns on the ground only: a wave cannot charge aircraft.
            let guns = self.state.ai[ctx.player as usize]
                .contacts
                .iter()
                .filter(|c| {
                    let p = ctx.profiles.get(c.blueprint);
                    p.hits(Target::Land)
                        && !matches!(
                            p.domain,
                            Some(super::profile::Domain::Air | super::profile::Domain::Space)
                        )
                        && c.pos.distance(centre) < Fx::from_int(3000)
                })
                .map(|c| c.pos)
                .min_by_key(|p| (p.distance_sq(centre), p.x, p.y));
            let charge = guns.filter(|g| {
                self.enemy_strength_at(ctx, *g, Fx::from_int(700), Target::Land) * 4
                    <= materials * 5
                    && self.army_reaches(ctx, *g)
            });
            match charge {
                Some(g) => {
                    op.target = g;
                    op.launched = materials;
                    op.phase = Phase::Executing;
                    op.phase_since = tick;
                    out.push(Command::AttackMove {
                        units: ids(&rows),
                        target: g,
                        queue: false,
                    });
                }
                None if op.kind == OpKind::Army => {
                    let c = &mut self.state.ai[ctx.player as usize].commander;
                    c.rally_back = (c.rally_back + Fx::from_int(400)).min(Fx::from_int(2400));
                }
                None => {}
            }
            return 1;
        }
        match op.phase {
            Phase::Gathering => {
                // Stragglers to the rally point; the blob's edge grows with its size.
                let r = Fx::from_int(200 + 12 * (rows.len() as i32).isqrt());
                let far: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|&row| {
                        let at = self.state.units.pos[row];
                        at.distance(op.rally) > r * 2
                            && self.idle_or_arrived(ctx, row, op.rally)
                            // One in a fight fights it out where it stands.
                            && self.enemy_strength_at(ctx, at, Fx::from_int(1000), Target::Land)
                                == Fx::ZERO
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
                    .map(|&row| self.bp(row).cost_materials)
                    .sum();
                // An enemy group in reach that it outnumbers: a wave in the field takes
                // it on, the way a player's army keeps the map clear around it.
                if op.kind == OpKind::Army && gathered > Fx::ZERO {
                    if let Some(prey) = self.prey_near(ctx, op.rally, gathered) {
                        op.target = prey;
                        op.launched = materials;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: ids(&rows),
                            target: prey,
                            queue: false,
                        });
                        return orders + 1;
                    }
                }
                // The guard waits for raids (`raise_defence`); it never sets out itself.
                if op.kind == OpKind::Defend
                    || (op.kind != OpKind::Guard && op.want > Fx::ZERO && gathered >= op.want)
                {
                    let soft = op.kind == OpKind::Raid;
                    let target = match op.kind {
                        OpKind::Defend | OpKind::Landing => Some(op.target),
                        OpKind::Siege => self.siege_spot(ctx, op.rally, &rows),
                        // A wave: the enemy's nearest extractor or factory, weighed by the
                        // guns scouted round it (`attack_target`), on ground it can walk.
                        // A probe at what is soft; a wave at the enemy's nearest
                        // extractor or factory, weighed by the guns scouted round it.
                        OpKind::Army
                            if self.state.ai[ctx.player as usize].commander.plan(op.plan)
                                == super::state::Stake::Probe =>
                        {
                            self.land_objective(ctx, op.rally, gathered, true)
                        }
                        // Never into a turret belt it cannot take: the guns round a
                        // target, turrets included, may weigh at most four fifths of it.
                        OpKind::Army => self
                            .attack_target(
                                ctx.player,
                                op.rally,
                                ctx.intel,
                                super::super::Stance::Raid,
                            )
                            .filter(|t| {
                                self.army_reaches(ctx, *t)
                                    && self.enemy_strength_at(
                                        ctx,
                                        *t,
                                        Fx::from_int(800),
                                        Target::Land,
                                    ) * 5
                                        <= gathered * 4
                            })
                            .or_else(|| self.land_objective(ctx, op.rally, gathered, soft)),

                        _ => self.land_objective(ctx, op.rally, gathered, soft),
                    };
                    // An army goes only as strong as the enemy army it believes in
                    // (a little less when all in), and not through stronger fire on
                    // the way: one sent at half the enemy's size met its main army
                    // on the road and lost nine units in ten seconds.
                    let stake = self.state.ai[ctx.player as usize].commander.plan(op.plan);
                    let enemy = ctx.beliefs.army[super::profile::Domain::Land as usize]
                        + ctx.beliefs.army[super::profile::Domain::Hover as usize];
                    // Fights near their base favour them (turrets, reinforcements): a
                    // wave goes at a third more than the army it believes in, all in at
                    // as much, and never blind; a probe goes only at soft targets. In a
                    // mirror match a wave that set out at once, unseeing, met the
                    // enemy by its own base and was wiped out.
                    // A side whose units come down free from its elevator goes as a
                    // probe does, and blind: with 50 000 materials of free tanks at its
                    // rally point, a side waited five minutes for a third more than
                    // the enemy's army.
                    let probe = stake == super::state::Stake::Probe || ctx.cheap_army;
                    let bold = match stake {
                        _ if probe => Fx::ratio(3, 5),
                        super::state::Stake::AllIn => Fx::ONE,
                        _ => Fx::ratio(13, 10),
                    };
                    // Not blind: their army has been seen, or their base lately with
                    // none there to see.
                    // Three times the army it believes in, gathered three minutes with
                    // the scouts finding nothing, goes all the same: it knows where
                    // their start is, and ten heavy bots sat at home all game waiting
                    // to see an enemy that had nothing.
                    let overwhelming = gathered >= enemy * 3 && tick > op.phase_since + 1800;
                    let seen = ctx.beliefs.army_at.is_some()
                        || ctx.beliefs.base_unseen < 1200
                        || probe
                        || overwhelming;
                    // And enough anti-air for the enemy's air and warships it believes
                    // in: a wave of tanks marched 22 km under corvettes and bombers
                    // and died without a shot.
                    let enemy_air = ctx.beliefs.army[super::profile::Domain::Air as usize]
                        + ctx.beliefs.army[super::profile::Domain::Space as usize];
                    let our_aa: Fx = rows
                        .iter()
                        .map(|&r| {
                            let p = ctx.profiles.get(self.state.units.blueprint[r]);
                            p.dps[Target::Air as usize].max(p.dps[Target::Space as usize])
                        })
                        .sum::<Fx>()
                        * THREAT_MATERIALS;
                    let covered = our_aa * 2 >= enemy_air;
                    // Artillery goes out behind a wave already out near its spot, or
                    // where little of the enemy stands: alone it walked up to the
                    // guns and was run down, trading 1:6.
                    let siege_covered = op.kind == OpKind::Siege
                        && target.is_some_and(|t| {
                            army_out.is_some_and(|a| a.distance(t) < Fx::from_int(2500))
                                || self.enemy_strength_at(ctx, t, Fx::from_int(1500), Target::Land)
                                    * 3
                                    <= gathered
                        });
                    let strong = matches!(op.kind, OpKind::Defend | OpKind::Landing | OpKind::Raid)
                        || siege_covered
                        || (op.kind != OpKind::Siege
                            && gathered >= enemy * bold
                            && seen
                            && covered
                            && target.is_some_and(|t| {
                                ctx.wm.threat_along(op.rally, t, Target::Land) * THREAT_MATERIALS
                                    <= gathered * 6 / 5
                            }));
                    if let Some(target) = target.filter(|_| strong) {
                        op.target = target;
                        op.launched = materials;
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
                // What it faces against what of it stands there: a straggler far
                // behind does not fight.
                let facing = self.enemy_strength_at(ctx, centre, Fx::from_int(900), Target::Land);
                let here: Fx = rows
                    .iter()
                    .filter(|&&r| self.state.units.pos[r].distance(centre) < Fx::from_int(700))
                    .map(|&r| self.bp(r).cost_materials)
                    .sum();
                let ashore = op.kind == OpKind::Landing && !ctx.land_route;
                // Half of what set out lost, and losing the trade: an even fight is
                // fought out. Breaking off one on the move (units on a move order do not
                // shoot) handed the enemy the rest of the wave as it chased.
                // Free units (`Ctx::cheap_army`) are spent at any trade: a side with
                // an elevator fell back from an even fight with 136 units standing.
                let bled = op.launched > Fx::ZERO
                    && !ctx.cheap_army
                    && materials * 2 < op.launched
                    && op.ledger.killed < op.ledger.lost * 7 / 10;
                // Under aircraft or a warship it has nothing to shoot back with.
                let helpless = shelled
                    && !rows.iter().any(|&r| {
                        ctx.profiles
                            .get(self.state.units.blueprint[r])
                            .has(super::profile::role::ANTI_AIR)
                    })
                    && self.state.ai[ctx.player as usize].contacts.iter().any(|c| {
                        let p = ctx.profiles.get(c.blueprint);
                        matches!(
                            p.domain,
                            Some(super::profile::Domain::Air | super::profile::Domain::Space)
                        ) && p.hits(Target::Land)
                            && c.pos.distance(centre) < Fx::from_int(1500)
                    });
                let bled = bled || helpless;
                // By its own rally point there is nowhere to fall back to: it fights
                // it out there. Falling back from beside it ended the operation on
                // the spot and dissolved the army in the middle of a battle.
                let home = centre.distance(op.rally) < Fx::from_int(1500);
                let outmatched = if ctx.cheap_army {
                    OUTMATCHED * 2
                } else {
                    OUTMATCHED
                };
                if (facing > here * outmatched || bled)
                    && op.kind != OpKind::Defend
                    && !ashore
                    && !home
                {
                    op.phase = Phase::Withdrawing;
                    op.phase_since = tick;
                    // A fighting withdrawal: units on a plain move do not shoot, and a
                    // wave falling back that way was chased down to the last tank.
                    out.push(Command::AttackMove {
                        units: ids(&rows),
                        target: op.rally,
                        queue: false,
                    });
                    return orders + 1;
                }
                // In a fight, the whole group aims at the enemy in front of it, not
                // at the building past them it set out for.
                if facing > Fx::ZERO {
                    let front = self.state.ai[ctx.player as usize]
                        .contacts
                        .iter()
                        .filter(|c| {
                            let p = ctx.profiles.get(c.blueprint);
                            tick.saturating_sub(c.seen) <= 30
                                && p.armed()
                                && p.mobile()
                                && c.pos.distance(centre) < Fx::from_int(1000)
                        })
                        .map(|c| c.pos)
                        .min_by_key(|p| (p.distance_sq(centre), p.x, p.y));
                    if let Some(front) = front.filter(|f| f.distance(op.target) > Fx::from_int(200))
                    {
                        op.target = front;
                        out.push(Command::AttackMove {
                            units: ids(&rows),
                            target: front,
                            queue: false,
                        });
                        return orders + 1;
                    }
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
                // Done there once mostly idle, or once the group's middle stands at the
                // target with nothing to fight, or five minutes out with nothing in
                // front of it: a wave of 340 never counted as mostly idle (a few of
                // so many always have somewhere to walk) and stood 1.4 km from home
                // for ten minutes while the side earned 2000 a second.
                let there = centre.distance(op.target) < Fx::from_int(600) && facing == Fx::ZERO;
                let stalled = facing == Fx::ZERO && tick > op.phase_since + 3000;
                if !(self.mostly_idle(&rows, &ctx.arrived) || there || stalled) {
                    return orders;
                }
                if stalled {
                    op.phase_since = tick;
                }
                // Done there: on to the next, or back to gather. A wave takes on only
                // groups nearby it beats; anything more is decided again at the rally
                // point with its checks (a wave that went on to the enemy start
                // every time marched into their base and was wiped out).
                let ashore = !ctx.can_walk(centre);
                let next = match op.kind {
                    OpKind::Defend => None,
                    OpKind::Siege => self.siege_spot(ctx, centre, &rows),
                    OpKind::Raid => self.land_objective(ctx, centre, materials, true),
                    OpKind::Army if !ashore => self.prey_near(ctx, centre, materials),
                    _ => self.land_objective(ctx, centre, materials, false),
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
                // What chases it is no stronger than what it has: turn and fight.
                let facing = self.enemy_strength_at(ctx, centre, Fx::from_int(900), Target::Land);
                if facing > Fx::ZERO && facing <= materials && op.kind != OpKind::Defend {
                    if let Some(prey) = self.prey_near(ctx, centre, materials) {
                        op.target = prey;
                        op.phase = Phase::Executing;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: ids(&rows),
                            target: prey,
                            queue: false,
                        });
                        return orders + 1;
                    }
                }
                if centre.distance(op.rally) < Fx::from_int(700)
                    || (self.mostly_idle(&rows, &ctx.arrived) && tick > op.phase_since + 300)
                {
                    // Back at the rally point an army's remnant joins the wave
                    // gathering there (its units are freed and taken up again).
                    op.phase = if op.kind == OpKind::Siege {
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

    /// The nearest enemy land group within `PREY_REACH` of `at` that `materials` beats by
    /// a fifth, off any turret's cover, on ground the army can walk to.
    pub(in crate::ai) fn prey_near(&self, ctx: &Ctx, at: FxVec2, materials: Fx) -> Option<FxVec2> {
        let tick = self.state.tick;
        self.state.ai[ctx.player as usize]
            .contacts
            .iter()
            .filter(|c| {
                let p = ctx.profiles.get(c.blueprint);
                tick.saturating_sub(c.seen) <= 100
                    && p.armed()
                    && matches!(
                        p.domain,
                        Some(super::profile::Domain::Land | super::profile::Domain::Hover)
                    )
                    && c.pos.distance(at) <= Fx::from_int(PREY_REACH)
                    && self.army_reaches(ctx, c.pos)
            })
            .map(|c| c.pos)
            .filter(|&p| {
                self.enemy_strength_at(ctx, p, Fx::from_int(700), Target::Land) * 6 <= materials * 5
            })
            .min_by_key(|p| (p.distance_sq(at), p.x, p.y))
    }

    /// Whether the land army can get to `p`: walk there from home or, on a side with
    /// no land route, whose army is its hovercraft alone (`ops::fits`), float there.
    /// Asked only whether home's ground reached them, a hover army of 663 on an
    /// island map never found a target and held at home for half an hour.
    pub(in crate::ai) fn army_reaches(&self, ctx: &Ctx, p: FxVec2) -> bool {
        ctx.can_walk(p)
            || (!ctx.land_route
                && self
                    .nav
                    .nearest_passable(MoveLayer::Hover, 0, p)
                    .is_some_and(|g| g.distance(p) <= Fx::from_int(HOVER_NEAR)))
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

    /// Armed enemies on the ground the side holds: within `NEAR_HOLD` of its
    /// structures or its rally point, seen in the last ten seconds, gathered into
    /// 600 m clusters: (what they are near, where they are). Artillery shelling the
    /// rally point from beyond the base's raid radius counts.
    pub(in crate::ai) fn incursions(&self, ctx: &Ctx) -> Vec<(FxVec2, FxVec2, bool)> {
        let tick = self.state.tick;
        let units = &self.state.units;
        let held: Vec<FxVec2> = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == ctx.player && self.bp(r).is_structure())
            .map(|r| units.pos[r])
            .chain([ctx.staging, ctx.start])
            .collect();
        let mut out: Vec<(FxVec2, FxVec2, bool)> = Vec::new();
        for c in &self.state.ai[ctx.player as usize].contacts {
            let p = ctx.profiles.get(c.blueprint);
            // Raiders from the air too: corvettes jumping onto the power farms and
            // gunships over the mines took more of the economy than tanks did.
            let air = matches!(
                p.domain,
                Some(super::profile::Domain::Air | super::profile::Domain::Space)
            );
            if tick.saturating_sub(c.seen) > 100
                || !p.mobile()
                || !p.hits(Target::Land) && !p.hits(Target::Structure)
                || !(air
                    || matches!(
                        p.domain,
                        Some(super::profile::Domain::Land | super::profile::Domain::Hover)
                    ))
            {
                continue;
            }
            let reach = p.reach[Target::Land as usize].max(p.reach[Target::Structure as usize]);
            let near = held
                .iter()
                .copied()
                .filter(|h| h.distance(c.pos) <= NEAR_HOLD.max(reach + Fx::from_int(100)))
                .min_by_key(|h| (h.distance_sq(c.pos), h.x, h.y));
            let Some(at) = near else { continue };
            if !air && !ctx.can_walk(c.pos) && !ctx.can_walk(at) {
                continue;
            }
            if !out
                .iter()
                .any(|(_, e, a)| *a == air && e.distance(c.pos) < Fx::from_int(600))
            {
                out.push((at, c.pos, air));
            }
        }
        out
    }

    /// What the incursion at `enemy` shows the side is short of: anti-air against
    /// aircraft, guns against shelling, quick units against raiders. Posted as a
    /// floor on that role's share of new units (`solver.rs`), fading with time
    /// (`commander.rs`), so the zone that was hit gets what it lacked.
    fn post_role_need(&mut self, ctx: &Ctx, enemy: FxVec2, air: bool, strength: Fx) {
        let (mut guns, mut raiders) = (Fx::ZERO, Fx::ZERO);
        for c in &self.state.ai[ctx.player as usize].contacts {
            let p = ctx.profiles.get(c.blueprint);
            if c.pos.distance(enemy) >= Fx::from_int(700) {
                continue;
            }
            if p.has(role::ARTILLERY) {
                guns += p.materials;
            } else if p.has(role::RAIDER) {
                raiders += p.materials;
            }
        }
        let share = |m: Fx| (m.floor_int() / 4).clamp(120, 450) as i16;
        let need = &mut self.state.ai[ctx.player as usize].commander.role_need;
        if air {
            need[0] = need[0].max(share(strength));
        } else if guns > strength / 4 {
            need[1] = need[1].max(share(guns));
        } else if raiders > strength / 2 {
            need[2] = need[2].max(share(raiders));
        }
    }

    /// Opens a defence for each raid on home or an outlying mine no defence answers,
    /// taking the nearest gathering units of the land operations and free ones.
    pub(in crate::ai) fn raise_defence(&mut self, ctx: &Ctx, threats: &[(FxVec2, FxVec2, bool)]) {
        for &(at, enemy, air) in threats {
            let c = &self.state.ai[ctx.player as usize].commander;
            if c.ops
                .iter()
                .any(|o| o.kind == OpKind::Defend && o.target.distance(enemy) < Fx::from_int(800))
            {
                continue;
            }
            let base = enemy.distance(ctx.start) < Fx::from_int(700);
            // Far from home and the front: a detachment, only what the raid needs,
            // nearest first (the guard before the wave). Letting outlying mines go
            // handed a raiding enemy the economy.
            let far = enemy.distance(ctx.staging) > Fx::from_int(2500)
                && enemy.distance(ctx.start) > Fx::from_int(2500);
            // From the air: what of theirs flies there; on the ground, their guns.
            let strength = if air {
                self.state.ai[ctx.player as usize]
                    .contacts
                    .iter()
                    .filter(|c| c.pos.distance(enemy) < Fx::from_int(700))
                    .map(|c| ctx.profiles.get(c.blueprint))
                    .filter(|p| {
                        matches!(
                            p.domain,
                            Some(super::profile::Domain::Air | super::profile::Domain::Space)
                        )
                    })
                    .map(|p| p.materials)
                    .sum()
            } else {
                self.enemy_strength_at(ctx, enemy, Fx::from_int(600), Target::Land)
            };
            // A scout passing by is the turrets' business: the whole army chased
            // them about the map and never gathered a wave.
            if strength < Fx::from_int(SMALL_FRY) {
                continue;
            }
            let need = strength * 13 / 10 + Fx::from_int(50);
            self.post_role_need(ctx, enemy, air, strength);
            let id = self.open_op(
                ctx.player,
                OpKind::Defend,
                PlanKind::Pressure,
                at,
                enemy,
                Fx::ZERO,
            );
            // Borrow from land operations that are gathering, nearest first; and
            // when the base itself is hit and that is not enough, from those out
            // there too. The commander died at home under corvettes while its
            // army stood at the enemy's front, and with it the match.
            let units = &self.state.units;
            let mut cands: Vec<(usize, usize, crate::tables::UnitId, Fx)> = Vec::new();
            let c = &self.state.ai[ctx.player as usize].commander;
            for recall in [false, true] {
                if recall && (!base || cands.iter().map(|c| c.3).sum::<Fx>() >= need) {
                    break;
                }
                for (oi, o) in c.ops.iter().enumerate() {
                    let guard = o.kind == OpKind::Guard || (air && o.kind == OpKind::AirGuard);
                    if !(guard || matches!(o.kind, OpKind::Army | OpKind::Siege | OpKind::Raid))
                        // An operation already out there is fighting: taking its
                        // units pulled an army out of a battle and turned it round.
                        || (o.phase == Phase::Gathering) == recall
                    {
                        continue;
                    }
                    for (ui, (u, cost)) in o.units.iter().enumerate() {
                        if let Some(r) = units.row(*u) {
                            // Against aircraft only what can shoot at them.
                            let p = ctx.profiles.get(units.blueprint[r]);
                            if air && !p.hits(Target::Air) && !p.hits(Target::Space) {
                                continue;
                            }
                            if guard
                                || base
                                || far
                                || units.pos[r].distance(enemy) < Fx::from_int(2500)
                            {
                                cands.push((oi, ui, *u, *cost));
                            }
                        }
                    }
                }
            }
            // The guard goes first, then the nearest.
            cands.sort_by_key(|&(oi, _, u, _)| {
                let guard = c.ops[oi].kind != OpKind::Guard;
                (
                    guard,
                    units
                        .row(u)
                        .map_or(Fx::from_int(1 << 30), |r| units.pos[r].distance(enemy)),
                    u,
                )
            });
            // Outmatched away from the base, the mine is let go: two tanks sent at
            // a raiding party of six were lost for nothing. A match is enough:
            // holding out for a quarter more let raids of four bots take eleven mines.
            let can: Fx = cands.iter().map(|c| c.3).sum();
            if !base && can < need {
                let c = &mut self.state.ai[ctx.player as usize].commander;
                if let Some(d) = c.ops.iter_mut().find(|o| o.id == id) {
                    d.phase = Phase::Done;
                }
                continue;
            }
            // Enough to win, nearest first: more than that leaves the wave short,
            // less was picked off a few at a time.
            let mut got = Fx::ZERO;
            let mut take: Vec<(crate::tables::UnitId, Fx)> = Vec::new();
            for &(_, _, u, cost) in &cands {
                if got >= need {
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
                    .any(|(_, e, _)| e.distance(o.target) < Fx::from_int(900))
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
