//! What an aircraft flies at when no order names it: the nearest enemy for most, and
//! for a fighter the nearest that no friendly fighter is on yet (`air_spread_pick`).
//!
//! A fighter's pick counts the friends already on each mark. With tens of thousands
//! of fighters in one sky, finding them by searching round the fighter read every
//! friend within a kilometre for every pick; `Chasers` lists them by mark instead,
//! once a tick, so a pick reads only the few on its candidates.

use crate::spatial::kind;
use crate::tables::*;
use crate::World;
use mc_core::Fx;
use mc_data::cat;

/// How far a fighter looks first for a mark (`air_spread_pick`), metres, and
/// how much further each look after: a thin ring at a time is all it checks.
const SCRAMBLE_PROBE: Fx = Fx::from_int(128);

const END: u32 = u32::MAX;

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
            return self.air_spread_pick(row, mask, reach);
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
    fn air_spread_pick(&self, row: usize, mask: u32, reach: Fx) -> Option<usize> {
        if !self.fires_at_will(row) {
            return None;
        }
        let units = &self.state.units;
        let pos = units.pos[row];
        let crowd = self.bp(row).vision / 4;
        let friends = self.team_mask(units.owner[row]);
        // What it may go after further than `inner` and within `r` (and within
        // `reach` of its edge), with its distance.
        let within = |inner: Fx, r: Fx| {
            let mut near: Vec<(Fx, usize)> = Vec::new();
            let kinds = crate::combat::target_kinds(mask);
            self.index.query_foes(pos, r, kinds, friends, |e| {
                let t = e.row as usize;
                // In the ring, `inner < distance <= reach + e.radius`, decided
                // on the squared length: a wider ring reads the inner ones again.
                let d = pos - e.pos;
                if !d.shorter_than(Fx(inner.0 + 1))
                    && d.shorter_than(Fx(reach.0 + e.radius.0 + 1))
                    && self.unit_entry_is_current(e)
                    && self.air_can_harass(row, t)
                    && self.hittable(t, mask)
                {
                    near.push((d.length(), t));
                }
                true
            });
            near
        };
        // Marks are scored nearest first, and a score is the distance plus some
        // crowd, so once one is further than the best score none after it can
        // win. The search widens a ring at a time only while one could: a
        // fighter near an enemy army looks at its nearest marks, not the army.
        let mut best: Option<(Fx, usize)> = None;
        let mut inner = Fx::from_int(-1);
        let mut radius = SCRAMBLE_PROBE;
        loop {
            let r = radius.min(reach);
            let last = r >= reach;
            let mut ring: Vec<(Fx, usize)> = within(inner, r)
                .into_iter()
                .filter(|&(d, _)| last || d <= r)
                .collect();
            ring.sort_unstable();
            mc_core::perf_count!("scramble.candidates", ring.len());
            for (distance, t) in ring {
                if best.is_some_and(|(s, _)| distance > s) {
                    return best.map(|(_, t)| t);
                }
                let score = distance + crowd * self.air_pursuers(row, t);
                if best.is_none_or(|(s, b)| score < s || (score == s && t < b)) {
                    best = Some((score, t));
                }
            }
            if last || best.is_some_and(|(s, _)| s <= r) {
                break;
            }
            inner = r;
            radius = r + SCRAMBLE_PROBE;
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
                && units.pos[other].distance(at) <= vision + self.bp(other).radius
            {
                seen += 1;
                if seen == 3 {
                    break;
                }
            }
        }
        seen
    }
}
