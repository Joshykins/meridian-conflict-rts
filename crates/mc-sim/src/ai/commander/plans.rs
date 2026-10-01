//! The Commander's game plans (`docs/AI_COMMANDER.md`, "Plans"): a portfolio, each
//! held at a stake (probe, invest, all-in), reviewed every minute against what the
//! side believes, what it can build, its personality, and how the plan's own
//! operations have traded. A plan escalates while it works, is cut back while it
//! loses, and hands its budget to another when the enemy answers it.
use super::profile::{role, Domain, Profile};
use super::state::{Note, OpKind, Phase, Plan, PlanKind, Stake, Why, NOTES, PLANS};
use super::Ctx;
use crate::ai::Census;
use crate::{Difficulty, Doctrine, World};
use mc_core::{Fx, FxVec2};
use mc_data::BlueprintId;

/// Ticks between reviews: a minute. The first comes at two minutes.
const REVIEW: u32 = 600;
const FIRST_REVIEW: u32 = 1200;
/// Appeal a plan needs for each stake.
const PROBE_AT: i32 = 55;
const INVEST_AT: i32 = 95;
const ALL_IN_AT: i32 = 150;
/// Appeal a held plan keeps over a new one.
const HOLD: i32 = 15;
/// How far past a stake's mark a plan must go to take it up, or below to give it up.
const MARGIN: i32 = 12;
/// Mass traded before a plan's record counts for or against it.
const EVIDENCE: Fx = Fx::from_int(1200);

/// A personality, as tastes over the plans (`docs/AI_COMMANDER.md`, "Personalities").
struct Taste {
    /// Appeal added to each plan, by `PlanKind as usize`.
    bias: [i32; 14],
    /// Lowers (or raises) the appeal needed to go all in.
    risk: i32,
}

fn taste(d: Doctrine) -> Taste {
    use PlanKind as P;
    let mut bias = [0; 14];
    let mut set = |k: P, v: i32| bias[k as usize] = v;
    let risk = match d {
        Doctrine::Adaptive => 0,
        Doctrine::Aggressive => {
            set(P::Pressure, 20);
            set(P::Raid, 40);
            set(P::Landing, 20);
            set(P::AirPower, 15);
            set(P::Warships, 15);
            set(P::Boom, -20);
            set(P::Fortify, -25);
            20
        }
        Doctrine::Economic => {
            set(P::Boom, 40);
            set(P::Strategic, 40);
            set(P::Titan, 50);
            set(P::Landing, 15);
            set(P::Pressure, -10);
            30
        }
        Doctrine::Defensive => {
            set(P::Fortify, 50);
            set(P::Siege, 50);
            set(P::AirDefense, 20);
            set(P::Strategic, 10);
            set(P::Raid, -20);
            set(P::Landing, -20);
            -20
        }
    };
    Taste { bias, risk }
}

/// Minutes of income a project may cost at each stake: a probe is cheap.
fn minutes(s: Stake) -> i32 {
    match s {
        Stake::Off => 0,
        Stake::Probe => 2,
        Stake::Invest => 4,
        Stake::AllIn => 8,
    }
}

fn points(s: Stake) -> i32 {
    match s {
        Stake::Off => 0,
        Stake::Probe => 1,
        Stake::Invest => 3,
        Stake::AllIn => 6,
    }
}

/// How many stake points a side spreads over its plans.
fn budget(d: Difficulty) -> i32 {
    match d {
        Difficulty::Easy => 4,
        Difficulty::Normal => 7,
        Difficulty::Hard => 9,
    }
}

/// What the side can build: the profiles of everything in its builders' and
/// factories' menus.
pub(in crate::ai) struct Menu<'a> {
    pub items: Vec<&'a Profile>,
}

impl Menu<'_> {
    pub(in crate::ai) fn any(&self, f: impl Fn(&Profile) -> bool) -> bool {
        self.items.iter().any(|p| f(p))
    }
}

impl World {
    pub(in crate::ai) fn menu<'a>(&self, ctx: &Ctx<'a>) -> Menu<'a> {
        Menu {
            items: self
                .side_menu(ctx.player)
                .ids
                .iter()
                .map(|&id| ctx.profiles.get(id))
                .collect(),
        }
    }

    /// Our own armed mobile mass, by domain.
    pub(in crate::ai) fn our_army(&self, ctx: &Ctx) -> [Fx; 7] {
        let units = &self.state.units;
        let mut by = [Fx::ZERO; 7];
        for r in units.slots.iter() {
            if units.owner[r] != ctx.player || !units.is_active(r) {
                continue;
            }
            let p = ctx.profiles.get(units.blueprint[r]);
            if p.armed() && p.mobile() && p.roles & role::BUILDER == 0 {
                if let Some(d) = p.domain {
                    by[d as usize] += p.mass;
                }
            }
        }
        by
    }

    /// How much plan `k` suits the side now, before its record and its taste.
    fn appeal(&self, ctx: &Ctx, k: PlanKind, menu: &Menu, census: &Census) -> i32 {
        let b = ctx.beliefs;
        let pl = &self.state.players[ctx.player as usize];
        let income = pl.mass_income.floor_int();
        let minutes = (self.state.tick / 600) as i32;
        let ours = self.our_army(ctx);
        let our_land = ours[Domain::Land as usize] + ours[Domain::Hover as usize];
        let their_land = b.army[Domain::Land as usize] + b.army[Domain::Hover as usize];
        let island = !ctx.land_route;
        let scouted = !ctx.intel.enemy_factories.is_empty();
        let weak_aa = scouted && b.anti_air * 5 < Fx::from_int(income.max(5) * 60);
        let has = |f: &dyn Fn(&Profile) -> bool| menu.any(f);
        // What has been killing the side lately, in mass.
        let hurt = self.state.ai[ctx.player as usize].commander.hurt;
        let hurt_by = |k: super::state::Hurt| hurt[k as usize].floor_int();
        let from_above = hurt_by(super::state::Hurt::Air) + hurt_by(super::state::Hurt::Space);
        let by_guns = hurt_by(super::state::Hurt::Artillery) + hurt_by(super::state::Hurt::Static);
        let fortified = b.fortified.floor_int() + by_guns;
        match k {
            PlanKind::Pressure => {
                if island && !has(&|p| p.domain == Some(Domain::Hover) && p.armed()) {
                    return 0;
                }
                let ahead = (our_land * 10 > their_land * 13) as i32;
                // The land army is the main effort wherever it can walk to the enemy.
                let base = if island { 25 } else { 80 };
                // Waves into an artillery park behind turrets lose, however big.
                base + 40 * ahead - (fortified / 2000).min(50)
            }
            PlanKind::Raid => {
                if !has(&|p| p.has(role::RAIDER)) || island || census.factories.is_empty() {
                    return 0;
                }
                let soft = self.state.ai[ctx.player as usize]
                    .contacts
                    .iter()
                    .filter(|c| {
                        let bp = self.blueprints.unit(c.blueprint);
                        bp.has(mc_data::cat::EXTRACTOR) || bp.has(mc_data::cat::ENGINEER)
                    })
                    .count() as i32;
                // Early on, raids take the map: engineers and mines on the edges.
                60 + soft.min(6) * 6
            }
            PlanKind::Landing => {
                if !has(&|p| p.has(role::TRANSPORT)) || census.factories.len() < 2 {
                    return 0;
                }
                30 + 120 * island as i32 + 30 * (fortified > our_land.floor_int()) as i32
            }
            PlanKind::AirPower => {
                if !has(&|p| p.has(role::STRIKE)) && census.air_factories == 0 {
                    return 0;
                }
                let heavy_aa = b.anti_air * 5 > Fx::from_int(income.max(5) * 240);
                // Bombers are the answer to ground that is held but not covered.
                let entrenched = (fortified / 2000).min(40) * weak_aa as i32;
                55 + 50 * weak_aa as i32 + 20 * island as i32 - 60 * heavy_aa as i32 + entrenched
            }
            PlanKind::SeaControl => {
                // A shipyard its builders can put up will do.
                let yard = |p: &Profile| {
                    let bp = self.blueprints.unit(p.id);
                    bp.has(mc_data::cat::FACTORY) && crate::ai::adaptive::domain(bp) == 2
                };
                if !has(&|p| p.domain == Some(Domain::Naval) && p.armed())
                    && census.naval_factories == 0
                    && !has(&yard)
                {
                    return 0;
                }
                // Their ships seen, water between us and them, or ships killing our
                // coast: boats shelling sea mines took a side's economy from 26 to 20.
                let ships =
                    (b.army[Domain::Naval as usize] + b.army[Domain::Sub as usize]).floor_int();
                let by_sea = hurt_by(super::state::Hurt::Sea);
                if !island && ships == 0 && by_sea == 0 {
                    return 0;
                }
                25 + (ships / 20).min(70) + 50 * island as i32 + (by_sea / 10).min(80)
            }
            PlanKind::SubWar => {
                let ships = b.army[Domain::Naval as usize].floor_int();
                if !has(&|p| p.domain == Some(Domain::Sub) && p.armed()) || (ships == 0 && !island)
                {
                    return 0;
                }
                (30 + (ships / 100).min(50) + 15 * island as i32 - 20 * b.sonar.min(3) as i32)
                    .max(0)
            }
            PlanKind::Warships => {
                if !has(&|p| {
                    p.domain == Some(Domain::Space) && p.armed() && !p.has(role::TRANSPORT)
                }) || income < 15
                {
                    return 0;
                }
                50 + 40 * weak_aa as i32 + 30 * island as i32
            }
            PlanKind::Siege => {
                let map_gun = has(&|p| p.has(role::MAP_GUN));
                if !map_gun && !has(&|p| p.has(role::ARTILLERY)) {
                    return 0;
                }
                20 + (fortified / 800).min(60) + 30 * (map_gun && income >= 40) as i32
            }
            PlanKind::Strategic => {
                if !has(&|p| p.has(role::STRATEGIC)) || income < 30 {
                    return 0;
                }
                let open = b.interceptors.is_empty() && b.base_unseen < 6000;
                20 + 40 * open as i32 + 25 * (income >= 100) as i32
            }
            PlanKind::Titan => {
                let big = |p: &Profile| {
                    p.has(role::PROJECT)
                        && p.mobile()
                        && p.armed()
                        && p.domain != Some(Domain::Space)
                };
                if !has(&big) || income < 40 {
                    return 0;
                }
                20 + 40 * (income >= 80) as i32 + 20 * (our_land * 10 >= their_land * 8) as i32
            }
            PlanKind::Fortify => {
                // A hedge: an interceptor slowly while their nukes are a maybe.
                let raided = !ctx.intel.threats.is_empty();
                15 + b.nukes * 4 / 5 + 20 * raided as i32 + b.air / 5
            }
            PlanKind::Boom => (90 - minutes * 5).max(10) + 20 * (our_land > their_land * 2) as i32,
            PlanKind::Intel => {
                let blind = b.base_unseen > 3000;
                let unsure = (25..=70).contains(&b.nukes);
                30 + 40 * blind as i32 + 30 * unsure as i32
            }
            PlanKind::AirDefense => {
                let air =
                    (b.army[Domain::Air as usize] + b.army[Domain::Space as usize]).floor_int();
                (b.air.max(b.space) * 2 / 3 + (air / 100).min(90) + (from_above / 50).min(80))
                    .max(0)
            }
        }
    }

    /// Reviews the side's plans when one is due (`REVIEW`).
    pub(in crate::ai) fn review_plans(&mut self, ctx: &Ctx, census: &Census) {
        let tick = self.state.tick;
        let player = ctx.player as usize;
        if self.state.ai[player].commander.plans.is_empty() {
            // The opening, before anything is known: grow, scout, keep an army.
            let c = &mut self.state.ai[player].commander;
            for (kind, stake) in [
                (PlanKind::Boom, Stake::Invest),
                (PlanKind::Pressure, Stake::Probe),
                (PlanKind::Intel, Stake::Probe),
            ] {
                c.plans.push(Plan {
                    kind,
                    stake,
                    since: tick,
                    changed: tick,
                    ledger: Default::default(),
                    strikes: 0,
                });
            }
            c.next_review = FIRST_REVIEW;
        }
        if tick < self.state.ai[player].commander.next_review {
            return;
        }
        let config = self.state.ai[player].config;
        let taste = taste(config.doctrine);
        let menu = self.menu(ctx);
        // Each plan's appeal: the situation, its taste, and its own record.
        let mut scored: Vec<(PlanKind, i32, Stake, Option<Why>)> = Vec::new();
        for k in PLANS {
            let held = self.state.ai[player]
                .commander
                .plans
                .iter()
                .find(|p| p.kind == k)
                .cloned();
            let base = self.appeal(ctx, k, &menu, census);
            if base == 0 {
                scored.push((k, 0, Stake::Off, Some(Why::Answered)));
                continue;
            }
            let mut a = base + taste.bias[k as usize];
            let mut why = None;
            if let Some(p) = &held {
                if p.stake > Stake::Off {
                    a += HOLD;
                }
                let l = p.ledger;
                if l.killed + l.lost >= EVIDENCE {
                    if l.trade() >= Fx::ratio(3, 2) {
                        a += 40;
                        why = Some(Why::Working);
                    } else if l.trade() < Fx::ratio(3, 5) {
                        a -= 35 * (p.strikes as i32 + 1);
                        why = Some(Why::Failing);
                    }
                } else if p.stake >= Stake::Invest && tick > p.changed + 6000 {
                    // Ten minutes held at a real stake with nothing to show.
                    a -= 30;
                    why = Some(Why::Stale);
                }
            }
            if k == PlanKind::Fortify && (25..=80).contains(&ctx.beliefs.nukes) {
                why = why.or(Some(Why::Hedge));
            }
            // A stake is taken up past its mark and given up below it, by a margin
            // either way: plans near a mark used to flip every review.
            let now = held.as_ref().map_or(Stake::Off, |p| p.stake);
            let level = |mark: i32, stake: Stake| {
                if now >= stake {
                    a >= mark - MARGIN
                } else {
                    a >= mark + MARGIN
                }
            };
            let want = if level(ALL_IN_AT - taste.risk, Stake::AllIn) {
                Stake::AllIn
            } else if level(INVEST_AT, Stake::Invest) {
                Stake::Invest
            } else if level(PROBE_AT, Stake::Probe) {
                Stake::Probe
            } else {
                Stake::Off
            };
            // One step a review: commitment, not flipping.
            let next = match want.cmp(&now) {
                std::cmp::Ordering::Greater => now.up(),
                std::cmp::Ordering::Less => now.down(),
                std::cmp::Ordering::Equal => now,
            };
            scored.push((k, a, next, why.or(Some(Why::Appeal))));
        }
        let mut appeals = [0i16; PLANS.len()];
        for &(k, a, _, _) in &scored {
            appeals[k as usize] = a.clamp(-999, 999) as i16;
        }
        // One plan all in at most, and the stakes within the side's budget: the
        // least appealing give way.
        let mut all_in: Vec<usize> = (0..scored.len())
            .filter(|&i| scored[i].2 == Stake::AllIn)
            .collect();
        all_in.sort_by_key(|&i| (std::cmp::Reverse(scored[i].1), i));
        for &i in all_in.iter().skip(1) {
            scored[i].2 = Stake::Invest;
        }
        let cap = budget(config.difficulty);
        loop {
            let total: i32 = scored.iter().map(|s| points(s.2)).sum();
            if total <= cap {
                break;
            }
            let Some(i) = (0..scored.len())
                .filter(|&i| scored[i].2 > Stake::Off)
                .min_by_key(|&i| (scored[i].1, i))
            else {
                break;
            };
            scored[i].2 = scored[i].2.down();
        }
        let c = &mut self.state.ai[player].commander;
        for (k, _, stake, why) in scored {
            match c.plans.iter_mut().find(|p| p.kind == k) {
                Some(p) if p.stake != stake => {
                    if stake < p.stake && why == Some(Why::Failing) {
                        p.strikes = p.strikes.saturating_add(1);
                    } else if stake > p.stake {
                        p.strikes = 0;
                    }
                    p.stake = stake;
                    p.changed = tick;
                    p.ledger = Default::default();
                    c.notes.push(Note {
                        tick,
                        plan: k,
                        stake,
                        why: why.unwrap_or(Why::Appeal),
                    });
                }
                Some(_) => {}
                None if stake > Stake::Off => {
                    c.plans.push(Plan {
                        kind: k,
                        stake,
                        since: tick,
                        changed: tick,
                        ledger: Default::default(),
                        strikes: 0,
                    });
                    c.notes.push(Note {
                        tick,
                        plan: k,
                        stake,
                        why: why.unwrap_or(Why::Appeal),
                    });
                }
                None => {}
            }
        }
        c.plans
            .retain(|p| p.stake > Stake::Off || tick < p.changed + 3000);
        c.plans.sort_by_key(|p| p.kind);
        let excess = c.notes.len().saturating_sub(NOTES);
        c.notes.drain(..excess);
        c.next_review = tick + REVIEW;
        c.appeal = appeals;
        self.choose_wants(ctx, &menu);
    }

    /// Opens the operations the plans and the side's units call for, closes the
    /// ones no plan holds any more, and sets how much each waits for.
    pub(in crate::ai) fn keep_ops(&mut self, ctx: &Ctx) {
        let player = ctx.player as usize;
        let tick = self.state.tick;
        let stake = |w: &World, k: PlanKind| w.state.ai[player].commander.plan(k);
        let income = self.state.players[player].mass_income;
        let back = super::super::offset_toward(
            ctx.start,
            ctx.start + (ctx.start - ctx.staging),
            Fx::from_int(380),
        );
        let hangar = self.clamp_to_map(back);
        let space_rally = super::super::offset_toward(
            ctx.start,
            ctx.enemy_start.unwrap_or(ctx.start),
            Fx::from_int(450),
        );
        // Standing operations for every kind of unit the side has.
        let free = self.free_units(ctx.player, ctx.profiles);
        for kind in [
            OpKind::Army,
            OpKind::AirGuard,
            OpKind::Strike,
            OpKind::Fleet,
            OpKind::Wolfpack,
            OpKind::Warships,
            OpKind::Scout,
        ] {
            // The army: one wave always gathering at staging, whatever is out.
            let exists =
                self.state.ai[player].commander.ops.iter().any(|o| {
                    o.kind == kind && (kind != OpKind::Army || o.phase == Phase::Gathering)
                });
            let needed = free.iter().any(|&r| {
                super::ops::fits(
                    kind,
                    ctx.profiles.get(self.state.units.blueprint[r]),
                    ctx.land_route,
                )
            });
            if !exists && needed {
                let (plan, rally) = match kind {
                    OpKind::Army => (PlanKind::Pressure, ctx.staging),
                    OpKind::AirGuard => (PlanKind::AirDefense, ctx.staging),
                    OpKind::Strike => (PlanKind::AirPower, hangar),
                    OpKind::Fleet => (PlanKind::SeaControl, FxVec2::ZERO),
                    OpKind::Wolfpack => (PlanKind::SubWar, FxVec2::ZERO),
                    OpKind::Warships => (PlanKind::Warships, space_rally),
                    _ => (PlanKind::Intel, ctx.start),
                };
                self.open_op(ctx.player, kind, plan, rally, rally, Fx::ZERO);
            }
        }
        // The home guard: fighters held among the mines while the army is out,
        // sized by income.
        let raiders = free.iter().any(|&r| {
            super::ops::fits(
                OpKind::Guard,
                ctx.profiles.get(self.state.units.blueprint[r]),
                ctx.land_route,
            )
        });
        let has_guard = self.state.ai[player]
            .commander
            .ops
            .iter()
            .any(|o| o.kind == OpKind::Guard);
        if raiders && !has_guard {
            self.open_op(
                ctx.player,
                OpKind::Guard,
                PlanKind::Pressure,
                ctx.start,
                ctx.start,
                Fx::ZERO,
            );
        }
        // Plan operations, as many as each stake holds.
        let count = |w: &World, k: OpKind| {
            w.state.ai[player]
                .commander
                .ops
                .iter()
                .filter(|o| o.kind == k && o.phase != Phase::Done)
                .count()
        };
        let by_stake = |s: Stake, n: [usize; 3]| match s {
            Stake::Off => 0,
            Stake::Probe => n[0],
            Stake::Invest => n[1],
            Stake::AllIn => n[2],
        };
        let raid = stake(self, PlanKind::Raid);
        for _ in count(self, OpKind::Raid)..by_stake(raid, [1, 2, 3]) {
            let want = Fx::from_int(by_stake(raid, [300, 600, 900]) as i32);
            self.open_op(
                ctx.player,
                OpKind::Raid,
                PlanKind::Raid,
                ctx.staging,
                ctx.staging,
                want,
            );
        }
        let landing = stake(self, PlanKind::Landing);
        for _ in count(self, OpKind::Landing)..by_stake(landing, [1, 1, 2]) {
            let want = Fx::from_int(by_stake(landing, [4, 12, 30]) as i32);
            self.open_op(
                ctx.player,
                OpKind::Landing,
                PlanKind::Landing,
                ctx.staging,
                ctx.staging,
                want,
            );
        }
        let siege = stake(self, PlanKind::Siege);
        for _ in count(self, OpKind::Siege)..by_stake(siege, [1, 1, 2]) {
            self.open_op(
                ctx.player,
                OpKind::Siege,
                PlanKind::Siege,
                ctx.staging,
                ctx.staging,
                Fx::from_int(900),
            );
        }
        // The guard waits among the mines, a third of the way back toward home.
        let guard_post = {
            let units = &self.state.units;
            let mines: Vec<FxVec2> = units
                .slots
                .iter()
                .filter(|&r| units.owner[r] == ctx.player && self.bp(r).mine.is_some())
                .map(|r| units.pos[r])
                .collect();
            if mines.is_empty() {
                ctx.start
            } else {
                let base = mines[0];
                let sum = mines.iter().fold(FxVec2::ZERO, |s, m| s + (*m - base));
                let mid =
                    base + FxVec2::new(sum.x / mines.len() as i32, sum.y / mines.len() as i32);
                let at = mid.lerp(ctx.start, Fx::ratio(1, 3));
                if ctx.can_walk(at) {
                    at
                } else {
                    ctx.start
                }
            }
        };
        // Plans dropped: their gathering operations end, their units go back.
        let c = &mut self.state.ai[player].commander;
        let held: Vec<(PlanKind, Stake)> = c.plans.iter().map(|p| (p.kind, p.stake)).collect();
        let off = |k: PlanKind| {
            held.iter()
                .find(|(p, _)| *p == k)
                .is_none_or(|(_, s)| *s == Stake::Off)
        };
        for o in &mut c.ops {
            let special = matches!(o.kind, OpKind::Raid | OpKind::Landing | OpKind::Siege);
            if special && off(o.plan) && o.phase == Phase::Gathering {
                o.phase = Phase::Done;
            }
        }
        // How much each waits for before it goes.
        let pressure = c.plan(PlanKind::Pressure);
        let air = c.plan(PlanKind::AirPower);
        let sea = c.plan(PlanKind::SeaControl);
        let subs = c.plan(PlanKind::SubWar);
        let warships = c.plan(PlanKind::Warships);
        let i = income.floor_int();
        for o in &mut c.ops {
            let stale = tick > o.phase_since + 3000 && o.phase == Phase::Gathering;
            let want = match o.kind {
                // Off: the army holds at home and answers raids. Otherwise a wave is
                // so many seconds of income: sized by what had been seen, the first
                // wave waited nine minutes while the enemy took the map.
                OpKind::Army => match pressure {
                    Stake::Off => Fx::ZERO,
                    Stake::Probe => Fx::from_int(300 + i * 10),
                    Stake::Invest => Fx::from_int(500 + i * 18),
                    Stake::AllIn => Fx::from_int(1000 + i * 40),
                },
                OpKind::Strike => Fx::from_int(by_stake(air, [600, 2000, 5000]).max(600) as i32),
                OpKind::Fleet => Fx::from_int(by_stake(sea, [1500, 4000, 9000]).max(1500) as i32),
                OpKind::Wolfpack => {
                    Fx::from_int(by_stake(subs, [800, 2500, 6000]).max(1000) as i32)
                }
                OpKind::Warships => {
                    Fx::from_int(by_stake(warships, [2000, 5000, 12000]).max(4000) as i32)
                }
                _ => o.want,
            };
            // Waited five minutes: go with three quarters.
            o.want = if stale { want * 3 / 4 } else { want };
            if o.kind == OpKind::Army {
                o.rally = ctx.staging;
            }
            if o.kind == OpKind::Guard {
                o.want = Fx::from_int((i * 8).clamp(300, 1200));
                o.rally = guard_post;
            }
        }
    }

    /// The structures and site-built units the plans want, best first, for the
    /// builders (`commander_job`): lift ships, warships, map guns, silos,
    /// interceptors, shields, anti-air, sensor ships and titans.
    fn choose_wants(&mut self, ctx: &Ctx, menu: &Menu) {
        let player = ctx.player as usize;
        let c = &self.state.ai[player].commander;
        let stake = |k: PlanKind| c.plan(k);
        let pl = &self.state.players[player];
        let income = pl.mass_income;
        let held = |f: &dyn Fn(&Profile) -> bool| -> usize {
            let units = &self.state.units;
            units
                .slots
                .iter()
                .filter(|&r| {
                    units.owner[r] == ctx.player && f(ctx.profiles.get(units.blueprint[r]))
                })
                .count()
        };
        // The dearest of a kind the income carries in `minutes`, or the cheapest.
        let pick = |f: &dyn Fn(&Profile) -> bool, minutes: i32, big: bool| -> Option<BlueprintId> {
            let budget = income * Fx::from_int(minutes * 60);
            let fits = menu
                .items
                .iter()
                .filter(|p| f(p) && p.mass <= budget.max(Fx::from_int(300)));
            if big {
                fits.max_by_key(|p| (p.mass, std::cmp::Reverse(p.id.0)))
                    .map(|p| p.id)
            } else {
                fits.min_by_key(|p| (p.mass, p.id.0)).map(|p| p.id)
            }
        };
        let mut wants: Vec<BlueprintId> = Vec::new();
        let mut want = |id: Option<BlueprintId>| {
            if let Some(id) = id {
                if !wants.contains(&id) {
                    wants.push(id);
                }
            }
        };
        let b = ctx.beliefs;
        // Answers first: interceptors against their nukes, by how sure we are.
        let interceptor = |p: &Profile| p.has(role::INTERCEPTOR);
        let need_int = match b.nukes {
            90.. => 1 + (b.nukes >= 100) as usize,
            40..=89 if stake(PlanKind::Fortify) >= Stake::Probe => 1,
            _ => 0,
        };
        if held(&interceptor) < need_int {
            want(pick(&interceptor, 8, false));
        }
        let landing = stake(PlanKind::Landing);
        if landing > Stake::Off {
            let lift = |p: &Profile| p.has(role::TRANSPORT) && p.has(role::WARP);
            let biggest_held = {
                let units = &self.state.units;
                units
                    .slots
                    .iter()
                    .filter(|&r| units.owner[r] == ctx.player)
                    .map(|r| ctx.profiles.get(units.blueprint[r]).carry)
                    .max()
                    .unwrap_or(0)
            };
            let best = pick(&lift, minutes(landing), landing >= Stake::Invest);
            if best.is_some_and(|id| ctx.profiles.get(id).carry > biggest_held) && held(&lift) < 2 {
                want(best);
            }
        }
        let strategic = stake(PlanKind::Strategic);
        let silo = |p: &Profile| p.has(role::STRATEGIC);
        if held(&silo) < [0, 0, 1, 2][strategic as usize] {
            want(pick(&silo, minutes(strategic), false));
        }
        let siege = stake(PlanKind::Siege);
        let gun = |p: &Profile| p.has(role::MAP_GUN);
        if held(&gun) < [0, 0, 1, 3][siege as usize] {
            want(pick(&gun, minutes(siege), true));
        }
        let titan = stake(PlanKind::Titan);
        let big = |p: &Profile| {
            p.has(role::PROJECT)
                && p.mobile()
                && p.armed()
                && !p.has(role::TRANSPORT)
                && p.domain != Some(Domain::Space)
        };
        if held(&big) < [0, 0, 1, 2][titan as usize] {
            want(pick(&big, minutes(titan) + 2, true));
        }
        let warships = stake(PlanKind::Warships);
        let warship = |p: &Profile| {
            p.domain == Some(Domain::Space)
                && p.armed()
                && !p.has(role::TRANSPORT)
                && p.has(role::PROJECT)
        };
        if held(&warship) < [0, 1, 3, 6][warships as usize] {
            want(pick(&warship, minutes(warships), warships >= Stake::Invest));
        }
        let shield = |p: &Profile| p.has(role::SHIELD) && !p.mobile();
        let fortify = stake(PlanKind::Fortify);
        let shields_for = held(&gun) + held(&silo) + [0, 0, 1, 2][fortify as usize];
        if held(&shield) < shields_for {
            want(pick(&shield, 4, false));
        }
        // Ships killing the coast: a torpedo or coastal gun by the water.
        let by_sea = self.state.ai[player].commander.hurt[super::state::Hurt::Sea as usize];
        let coast = |p: &Profile| p.has(role::DEFENSE) && p.has(role::ANTI_SHIP) && !p.mobile();
        if by_sea > Fx::from_int(300) && held(&coast) < 2 + (by_sea / 1500).floor_int() as usize {
            want(pick(&coast, 2, by_sea > Fx::from_int(2000)));
        }
        let aa = |p: &Profile| p.has(role::ANTI_AIR) && p.has(role::DEFENSE);
        // Bombed lately: anti-air where it hit, one more for each 1500 mass lost.
        let c = &self.state.ai[player].commander;
        let above =
            c.hurt[super::state::Hurt::Air as usize] + c.hurt[super::state::Hurt::Space as usize];
        let bombed = c
            .hit_from_above
            .is_some_and(|(_, t)| self.state.tick < t + 1800);
        if bombed
            && above > Fx::from_int(400)
            && held(&aa) < 2 + (above / 1500).floor_int() as usize
        {
            want(pick(&aa, 2, above > Fx::from_int(3000)));
        }
        let air_def =
            stake(PlanKind::AirDefense).max(if b.air > 60 { Stake::Probe } else { Stake::Off });
        if held(&aa) < [0, 2, 4, 7][air_def as usize] {
            want(pick(&aa, 2, air_def >= Stake::Invest));
        }
        if stake(PlanKind::Intel) >= Stake::Probe && income >= Fx::from_int(25) {
            let sensor = |p: &Profile| p.has(role::SENSOR) && p.has(role::WARP);
            if held(&sensor) == 0 {
                want(pick(&sensor, 2, false));
            }
        }
        self.state.ai[player].commander.wants = wants;
    }
}
