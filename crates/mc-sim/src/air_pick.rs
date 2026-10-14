//! What an aircraft flies at when no order names it: the nearest enemy for most, and
//! for a fighter the nearest that no friendly fighter is on yet (`air_spread_pick`).
//!
//! A fighter's pick counts the friends already on each mark. With tens of thousands
//! of fighters in one sky, finding them by searching round the fighter read every
//! friend within a kilometre for every pick; `Chasers` lists them by mark instead,
//! once a tick, so a pick reads only the few on its candidates.
//!
//! Orders run one row after another, and a fighter's pick sees the picks made before
//! it this tick. With a hundred thousand fighters that one thread was most of the tick.
//! So the picks are first worked out on the pool from the tick's starting state
//! (`speculate_air_picks`), each noting the marks whose chasers it counted. As the
//! orders then run, every mark whose chasers change is noted (`note_chase_change`), and
//! a fighter takes its worked-out pick when none of the marks it counted changed. When
//! some did, it counts their chasers again and scores the marks it weighed anew; only
//! when that could let in a mark it never weighed does it pick again there and then.
//! The picks come out as if made in turn.

use crate::spatial::kind;
use crate::tables::*;
use crate::World;
use mc_core::fx::isqrt as isqrt_u128;
use mc_core::{Fx, FxVec2};
use mc_data::cat;

/// How far a fighter looks first for a mark (`air_spread_pick`), metres; each look
/// after reaches twice as far. Marks are weighed nearest first whatever the rings, so
/// they only set how many are measured: in a crowded sky a thin first ring holds the
/// few that decide.
const SCRAMBLE_PROBE: Fx = Fx::from_int(48);

/// Most marks one fighter's pick weighs (`air_spread_pick`), nearest first. A
/// deliberate cap, as an AI's budget: in a sky of tens of thousands every mark near a
/// fighter already has one of its friends on it, and scoring the crowd added in (up to
/// three times a quarter of its sight) meant weighing every enemy within most of a
/// kilometre, thousands a pick. Past this it takes the best of the nearest; a fight of
/// a few dozen never reaches it.
const PICK_BUDGET: usize = 48;

const END: u32 = u32::MAX;

/// `d`'s length squared, exactly (`FxVec2::length` is its square root rounded down).
fn squared(d: FxVec2) -> u128 {
    let (x, y) = (d.x.0 as i128, d.y.0 as i128);
    (x * x + y * y) as u128
}

/// Fighters' picks worked out ahead of the orders (`World::speculate_air_picks`).
#[derive(Default)]
pub(crate) struct AirSpecs {
    /// Per row: its pick's index in `picks`, or `END`.
    by_row: Vec<u32>,
    picks: Vec<Spec>,
    /// The marks each pick weighed, in the order it weighed them (`Spec::seen` ranges
    /// over it).
    seen: Vec<Weighed>,
    /// Per row: its chasers have changed since the picks were made.
    changed: Vec<bool>,
}

/// One fighter's pick, and what it rested on.
struct Spec {
    /// Its front order then (node), and what that named.
    order: u32,
    named: UnitId,
    pick: Option<u32>,
    seen: (u32, u32),
    stop: Stop,
}

/// A mark a pick weighed: its distance, and how many of the fighter's friends were on it.
#[derive(Clone, Copy)]
struct Weighed {
    mark: u32,
    distance: Fx,
    chasers: u8,
}

/// Why a pick stopped weighing marks, and so which it never weighed.
#[derive(Clone, Copy, Default)]
enum Stop {
    /// Every mark in reach was weighed, or as many as the budget lets one be.
    #[default]
    Weighed,
    /// Those it did not weigh are at least this far.
    Beyond(Fx),
}

/// Where a search for a mark takes up (`World::spread_from`): nothing weighed yet, or
/// what a worked-out pick weighed, scored anew.
struct Resume<'a> {
    weighed: &'a [Weighed],
    best: Option<(Fx, usize)>,
    /// Marks not yet weighed lie further than this.
    inner: Fx,
}

impl Default for Resume<'_> {
    fn default() -> Self {
        Resume {
            weighed: &[],
            best: None,
            inner: Fx::from_int(-1),
        }
    }
}

/// What a pick worked out ahead weighed, and why it stopped.
#[derive(Default)]
struct Trace {
    marks: Vec<Weighed>,
    stop: Stop,
}

/// What of a unit its chasers' counts read: the mark its front order names, and
/// whether it is on a run.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChaseState {
    target: Option<usize>,
    run: bool,
}

/// The aircraft whose front order names each unit, by the unit's row: listed at the
/// start of `run_orders` and added to as fighters take new marks (`World::air_fight`).
/// An entry may be stale (the aircraft has died or turned to another mark since), so
/// a reader checks each against the aircraft's order as it stands.
#[derive(Default)]
pub(crate) struct Chasers {
    /// The first link of each row's list, or `END`.
    head: Vec<u32>,
    /// (aircraft row, next link).
    links: Vec<(u32, u32)>,
}

impl Chasers {
    /// The aircraft that `target` may be the mark of; some may be stale.
    fn of(&self, target: usize) -> impl Iterator<Item = usize> + '_ {
        let mut at = self.head.get(target).copied().unwrap_or(END);
        std::iter::from_fn(move || {
            let (row, next) = *self.links.get(at as usize)?;
            at = next;
            Some(row as usize)
        })
    }

    pub(crate) fn add(&mut self, target: usize, row: usize) {
        if let Some(head) = self.head.get_mut(target) {
            self.links.push((row as u32, *head));
            *head = (self.links.len() - 1) as u32;
        }
    }
}

impl World {
    /// Lists every aircraft by the unit its front order names (`Chasers`).
    pub(crate) fn list_chasers(&mut self) {
        let mut chasers = std::mem::take(&mut self.scratch.chasers);
        let units = &self.state.units;
        chasers.head.clear();
        chasers.head.resize(units.slots.rows(), END);
        chasers.links.clear();
        for row in units.slots.iter() {
            if units.order_head[row] == NO_ORDER || !self.is_air(row) {
                continue;
            }
            if let Some(t) = self
                .state
                .orders
                .front(units, row)
                .and_then(|o| units.row(o.target))
            {
                chasers.add(t, row);
            }
        }
        self.scratch.chasers = chasers;
    }

    /// What the aircraft flies at: the target its order names, else a detected enemy
    /// within `reach`.
    pub(crate) fn air_engage_target_within(&self, row: usize, reach: Fx) -> Option<usize> {
        self.engage_within(row, reach, &mut None)
    }

    /// `air_engage_target_within`, noting in `trace` the marks it weighed.
    fn engage_within(
        &self,
        row: usize,
        reach: Fx,
        trace: &mut Option<&mut Trace>,
    ) -> Option<usize> {
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
        let fighter = bp.has(cat::ANTI_AIR) && bp.motion.is_some_and(|m| !m.hover);
        if !fighter {
            for id in units.weapon_target[row] {
                if let Some(t) = units.row(id) {
                    if self.air_can_harass(row, t) {
                        return Some(t);
                    }
                }
            }
        }
        let mask = bp.weapons.iter().fold(0u32, |acc, w| acc | w.target_mask);
        if mask == 0 {
            return None;
        }
        if fighter {
            return self.air_spread_pick(row, mask, reach, trace);
        }
        let friends = self.team_mask(units.owner[row]);
        self.index
            .nearest_foe(units.pos[row], reach, kind::UNIT, friends, |e| {
                self.unit_entry_is_current(e)
                    && self.air_can_harass(row, e.row as usize)
                    && self.hittable(e.row as usize, mask)
                    && self.fires_at_will(row)
            })
            .map(|e| e.row as usize)
    }

    /// A fighter looking for something to fight takes the nearest enemy that no
    /// friendly fighter is on yet, or one a few hundred metres further than a
    /// crowded one: a swarm meeting a swarm splits into pairs, each turning after
    /// its own opponent, rather than piling onto the first few it sees while the
    /// rest of the enemy flies free. Orders run in row order, so a fighter sees the
    /// picks the fighters before it made this tick.
    fn air_spread_pick(
        &self,
        row: usize,
        mask: u32,
        reach: Fx,
        trace: &mut Option<&mut Trace>,
    ) -> Option<usize> {
        if let Some(trace) = trace {
            trace.stop = Stop::Weighed;
        }
        if !self.fires_at_will(row) {
            return None;
        }
        mc_core::perf_count!("air_picks.spread");
        self.spread_from(row, mask, reach, Resume::default(), trace)
    }

    /// `air_spread_pick`'s search, from `from`: the marks weighed already, the best of
    /// them, and how far out the rest lie.
    fn spread_from(
        &self,
        row: usize,
        mask: u32,
        reach: Fx,
        from: Resume<'_>,
        trace: &mut Option<&mut Trace>,
    ) -> Option<usize> {
        let units = &self.state.units;
        let pos = units.pos[row];
        let crowd = self.bp(row).vision / 4;
        let friends = self.team_mask(units.owner[row]);
        // What it may go after further than `inner` and within `r` (and within
        // `reach` of its edge), by its squared distance (`squared`).
        let within = |inner: Fx, r: Fx| {
            let mut near: Vec<(u128, usize)> = Vec::new();
            let kinds = crate::combat::target_kinds(mask);
            self.index
                .query_foes_ring(pos, inner, r, kinds, friends, |e| {
                    let t = e.row as usize;
                    // In the ring, `inner < distance <= reach + e.radius`, decided
                    // on the squared length: a wider ring reads the inner ones again.
                    let d = pos - e.pos;
                    if !d.shorter_than(Fx(inner.0 + 1))
                        && d.shorter_than(Fx(reach.0 + e.radius.0 + 1))
                        && !from.weighed.iter().any(|w| w.mark as usize == t)
                        && self.unit_entry_is_current(e)
                        && self.air_can_harass(row, t)
                        && self.hittable(t, mask)
                    {
                        near.push((squared(d), t));
                    }
                    true
                });
            near
        };
        // Marks are scored nearest first, and a score is the distance plus some
        // crowd, so once one is further than the best score none after it can
        // win. The search widens a ring at a time only while one could: a
        // fighter near an enemy army looks at its nearest marks, not the army.
        let mut best = from.best;
        let mut inner = from.inner;
        // Each ring reaches as far again past the last as that one did past its own.
        let mut step = SCRAMBLE_PROBE;
        let mut radius = inner.max(Fx::ZERO) + step;
        let mut weighed = from.weighed.len();
        loop {
            let r = radius.min(reach);
            let last = r >= reach;
            // `distance <= r`: the length is the square root rounded down.
            let past = squared(FxVec2::new(Fx(r.0 + 1), Fx::ZERO));
            let mut ring: Vec<(u128, usize)> = within(inner, r)
                .into_iter()
                .filter(|&(sq, _)| last || sq < past)
                .collect();
            mc_core::perf_count!("scramble.candidates", ring.len());
            // Only as many as the budget has left can be weighed: those nearest, with any
            // as far as the last of them by the rounded length (the weighing order is by
            // length, then row). Only these are measured and sorted.
            let left = PICK_BUDGET - weighed;
            if ring.len() > left {
                ring.select_nth_unstable(left - 1);
                let edge = isqrt_u128(ring[left - 1].0) + 1;
                let tie = edge * edge;
                let mut k = left;
                for i in left..ring.len() {
                    if ring[i].0 < tie {
                        ring.swap(k, i);
                        k += 1;
                    }
                }
                ring.truncate(k);
            }
            let mut ring: Vec<(Fx, usize)> = ring
                .into_iter()
                .map(|(sq, t)| (Fx(isqrt_u128(sq) as i64), t))
                .collect();
            ring.sort_unstable();
            for (distance, t) in ring {
                if best.is_some_and(|(s, _)| distance > s) {
                    if let Some(trace) = trace {
                        trace.stop = Stop::Beyond(distance);
                    }
                    return best.map(|(_, t)| t);
                }
                if weighed == PICK_BUDGET {
                    return best.map(|(_, t)| t);
                }
                weighed += 1;
                let chasers = self.air_pursuers(row, t);
                if let Some(trace) = trace {
                    trace.marks.push(Weighed {
                        mark: t as u32,
                        distance,
                        chasers: chasers as u8,
                    });
                }
                let score = distance + crowd * chasers;
                if best.is_none_or(|(s, b)| score < s || (score == s && t < b)) {
                    best = Some((score, t));
                }
            }
            if last || weighed == PICK_BUDGET {
                break;
            }
            if best.is_some_and(|(s, _)| s <= r) {
                // What lies past this ring is further than `r`; `Beyond` takes it as at
                // least `r`, which only ever asks for a fresh pick needlessly.
                if let Some(trace) = trace {
                    trace.stop = Stop::Beyond(r);
                }
                break;
            }
            inner = r;
            step = step * 2;
            radius = r + step;
        }
        best.map(|(_, t)| t)
    }

    /// The other friendly fighters on an attack run at `target` within `row`'s sight
    /// of it, up to three.
    fn air_pursuers(&self, row: usize, target: usize) -> i32 {
        let units = &self.state.units;
        let owner = units.owner[row];
        let vision = self.bp(row).vision;
        let id = units.id(target);
        let at = units.pos[target];
        let mut seen = 0;
        for other in self.scratch.chasers.of(target) {
            if other != row
                && units.slots.is_alive(other)
                && units.has_flag(other, flag::AIR_RUN)
                && !units.has_flag(other, flag::IN_FACTORY)
                && !self.are_enemies(owner, units.owner[other])
                && self
                    .state
                    .orders
                    .front(units, other)
                    .is_some_and(|o| o.target == id)
                && (units.pos[other] - at).shorter_than(Fx(vision.0 + self.bp(other).radius.0 + 1))
            {
                seen += 1;
                if seen == 3 {
                    break;
                }
            }
        }
        seen
    }

    /// Works out on the pool the pick of every aircraft whose front order flies it at
    /// what it finds (attack-move, patrol), from the state the orders start from.
    pub(crate) fn speculate_air_picks(&mut self) {
        let _span = mc_core::perf_span!("orders.speculate");
        let mut specs = std::mem::take(&mut self.scratch.air_specs);
        let units = &self.state.units;
        let rows = units.slots.rows();
        let this = &*self;
        type Made = (u32, u32, UnitId, Option<u32>, u32, Stop);
        let made: Vec<(Vec<Made>, Vec<Weighed>)> =
            self.pool.parallel_map_chunks(rows, 256, |_, range| {
                let mut picks = Vec::new();
                let mut trace = Trace::default();
                for row in range {
                    if !units.slots.is_alive(row)
                        || !units.is_active(row)
                        || units.drone_parent[row] != Handle::NONE
                        || !this.is_air(row)
                        || this.stunned(row)
                    {
                        continue;
                    }
                    let Some(o) = this.state.orders.front(units, row) else {
                        continue;
                    };
                    if !matches!(o.kind, OrderKind::AttackMove | OrderKind::Patrol) {
                        continue;
                    }
                    let first = trace.marks.len() as u32;
                    let pick = this
                        .engage_within(row, this.bp(row).vision, &mut Some(&mut trace))
                        .map(|t| t as u32);
                    picks.push((
                        row as u32,
                        units.order_head[row],
                        o.target,
                        pick,
                        first,
                        trace.stop,
                    ));
                }
                (picks, trace.marks)
            });
        specs.by_row.clear();
        specs.by_row.resize(rows, END);
        specs.picks.clear();
        specs.seen.clear();
        specs.changed.clear();
        specs.changed.resize(rows, false);
        for (picks, seen) in made {
            let base = specs.seen.len() as u32;
            let ends: Vec<u32> = picks
                .iter()
                .skip(1)
                .map(|p| p.4)
                .chain(std::iter::once(seen.len() as u32))
                .collect();
            for ((row, order, named, pick, first, stop), end) in picks.into_iter().zip(ends) {
                specs.by_row[row as usize] = specs.picks.len() as u32;
                specs.picks.push(Spec {
                    order,
                    named,
                    pick,
                    seen: (base + first, base + end),
                    stop,
                });
            }
            specs.seen.extend(seen);
        }
        self.scratch.air_specs = specs;
    }

    /// What `air_engage_target` gives `row` now: its worked-out pick when nothing it
    /// rested on has changed since, else picked again.
    pub(crate) fn air_engage_target_specced(&self, row: usize) -> Option<usize> {
        let specs = &self.scratch.air_specs;
        let units = &self.state.units;
        if let Some(spec) = specs
            .by_row
            .get(row)
            .and_then(|&i| specs.picks.get(i as usize))
        {
            let (a, b) = spec.seen;
            let weighed = &specs.seen[a as usize..b as usize];
            if units.order_head[row] == spec.order
                && self
                    .state
                    .orders
                    .front(units, row)
                    .is_some_and(|o| o.target == spec.named)
            {
                if weighed.iter().all(|w| !specs.changed[w.mark as usize]) {
                    mc_core::perf_count!("air_picks.taken");
                    return spec.pick.map(|t| t as usize);
                }
                // Score what it weighed again, the changed marks' chasers counted anew, in
                // the order and by the rule `air_spread_pick` scores them.
                let crowd = self.bp(row).vision / 4;
                let mut best: Option<(Fx, usize)> = None;
                for w in weighed {
                    let t = w.mark as usize;
                    let chasers = if specs.changed[t] {
                        self.air_pursuers(row, t)
                    } else {
                        w.chasers as i32
                    };
                    let score = w.distance + crowd * chasers;
                    if best.is_none_or(|(s, b)| score < s || (score == s && t < b)) {
                        best = Some((score, t));
                    }
                }
                // A mark it never weighed is at least `beyond` off, so scores no better.
                let whole = match spec.stop {
                    Stop::Weighed => true,
                    Stop::Beyond(beyond) => best.is_some_and(|(s, _)| s < beyond),
                };
                if whole {
                    mc_core::perf_count!("air_picks.rescored");
                    return best.map(|(_, t)| t);
                }
                // A mark it never weighed could now win: the search goes on from where
                // it stopped, past what it weighed.
                if let (Stop::Beyond(beyond), false) = (spec.stop, weighed.is_empty()) {
                    mc_core::perf_count!("air_picks.resumed");
                    let mask = self
                        .bp(row)
                        .weapons
                        .iter()
                        .fold(0u32, |acc, w| acc | w.target_mask);
                    let from = Resume {
                        weighed,
                        best,
                        inner: Fx(beyond.0 - 1),
                    };
                    return self.spread_from(row, mask, self.bp(row).vision, from, &mut None);
                }
            }
            mc_core::perf_count!("air_picks.again");
        }
        self.air_engage_target(row)
    }

    /// What of `row` its chasers' counts read (see `ChaseState`).
    pub(crate) fn chase_state(&self, row: usize) -> Option<ChaseState> {
        let units = &self.state.units;
        if !units.slots.is_alive(row) || !self.is_air(row) {
            return None;
        }
        Some(ChaseState {
            target: self
                .state
                .orders
                .front(units, row)
                .and_then(|o| units.row(o.target)),
            run: units.has_flag(row, flag::AIR_RUN),
        })
    }

    /// Notes the marks whose chasers a row's orders changed: the one it was on and the
    /// one it is on now.
    pub(crate) fn note_chase_change(
        &mut self,
        before: Option<ChaseState>,
        after: Option<ChaseState>,
    ) {
        if before == after {
            return;
        }
        let changed = &mut self.scratch.air_specs.changed;
        for t in [before, after]
            .into_iter()
            .flatten()
            .filter_map(|s| s.target)
        {
            if let Some(c) = changed.get_mut(t) {
                *c = true;
            }
        }
    }
}
