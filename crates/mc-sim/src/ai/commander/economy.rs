//! The Commander's economy (`docs/AI_COMMANDER.md`, "Economy"): every think it
//! balances build power against income against power, and grows the mines.
//!
//! - **Never float**: a full store throws away what the mines make, so a store
//!   filling up calls for more sinks: mine upgrades first (they pay back), then
//!   more build power (engineers, then factories).
//! - **Never stall energy**: mines dig less while their upkeep goes unpaid, so
//!   power is kept a little ahead of what everything draws at the speed the
//!   materials pay for, and goes first when it runs out.
//! - **No idle build power, none waiting**: building far below full speed, no more
//!   builders or factories are made; building at full speed with income to spare,
//!   more are.
//! - **Grow**: engineers set apart as expanders claim free ore one mine after
//!   another while there is safe ore on the side's half; reclaim is fetched early;
//!   the commander itself works out in its half while no enemy army is near.
//!
//! The builders, factories and upgrades carry it out (`Directives`).
use super::state::{PlanKind, Stake};
use crate::ai::salvage::Field;
use crate::ai::{Census, Claim, Intel, Job, ENERGY_PER_MASS, HOME_RADIUS};
use crate::command::Command;
use crate::tables::UnitId;
use crate::tables::WreckId;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use mc_data::cat;
use serde::{Deserialize, Serialize};

/// Where the store and the build speed are read as filling and stalling.
const FLOAT_FILL: Fx = Fx::ratio(2, 5);
const STALL_SPEED: Fx = Fx::ratio(4, 5);
/// Materials a second a unit of builder power spends, on average over what
/// builders put up (a mine 1, a plant 0.6), a little under for the walking.
const DRAW_PER_POWER: Fx = Fx::ratio(7, 10);
/// Most engineers set apart to claim mines.
const MOST_EXPANDERS: usize = 3;
/// Engineers the side never goes below, besides its expanders.
const LEAST_ENGINEERS: usize = 2;
/// Deliberate cap on engineers wanted: past it more factories, not more builders.
const MOST_ENGINEERS: usize = 30;
/// The commander works out to this share of the way to the enemy, at most
/// `ROAM_MOST` metres, while no armed enemy is within `ROAM_SAFE` of it, until
/// `ROAM_UNTIL` ticks (twelve minutes): a commander out late was a lost game.
const ROAM_SHARE: Fx = Fx::ratio(2, 5);
const ROAM_MOST: i32 = 2500;
const ROAM_SAFE: i32 = 1600;
const ROAM_UNTIL: u32 = 7200;
/// Armed enemy mass near the commander that sends it home (a scout or two do not).
const ROAM_THREAT: i32 = 200;
/// A wreck field worth the commander's walk, in mass.
const ROAM_FIELD: i32 = 150;
/// Ticks a deposit no lot was found by is skipped (three minutes).
const BLOCKED_FOR: u32 = 1800;

/// What power the side should put up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub(in crate::ai) enum Power {
    /// Income covers the draw with room: no new plants.
    #[default]
    Enough = 0,
    /// Short of what it will draw: a plant when nothing more urgent is wanted.
    Want = 1,
    /// Mines digging less, or the store run dry: power before anything else.
    Urgent = 2,
}

/// The economy's state between thinks, and what it tells the builders.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct Eco {
    /// The materials store's fill and the build speed, smoothed over a few thinks.
    pub fill: Fx,
    pub speed: Fx,
    pub floating: bool,
    pub stalling: bool,
    pub power: Power,
    /// Energy a second the side lacks for what it draws, with room to spare.
    pub power_short: Fx,
    /// Engineers and factories the side should have.
    pub engineers: u16,
    pub factories: u16,
    /// Engineers that do nothing but claim mines.
    pub expanders: Vec<UnitId>,
    /// Free ore on the side's half when last counted.
    pub free_ore: u16,
    /// Mine upgrades at once, and the most seconds one may take to pay back.
    pub upgrades: u16,
    pub payback: u32,
    /// Metres from home the commander may work out to; zero keeps it home.
    pub roam: Fx,
    /// How far out builders claim mines.
    pub reach: Fx,
    /// Deposits no lot could be found by, and when: skipped for `BLOCKED_FOR`.
    pub blocked: Vec<(FxVec2, u32)>,
}

impl Eco {
    pub(in crate::ai) fn hash(&self, h: &mut StateHasher) {
        h.write_i64(self.fill.0);
        h.write_i64(self.speed.0);
        h.write_u64(
            self.floating as u64
                | (self.stalling as u64) << 1
                | (self.power as u64) << 2
                | (self.engineers as u64) << 8
                | (self.factories as u64) << 24
                | (self.free_ore as u64) << 40,
        );
        h.write_i64(self.power_short.0);
        h.write_u64(self.upgrades as u64 | (self.payback as u64) << 16);
        h.write_i64(self.roam.0);
        h.write_i64(self.reach.0);
        h.write_u64(self.blocked.len() as u64);
        for (p, t) in &self.blocked {
            h.write_i64(p.x.0);
            h.write_i64(p.y.0);
            h.write_u64(*t as u64);
        }
        h.write_u64(self.expanders.len() as u64);
        for u in &self.expanders {
            h.write_u64(u.0 as u64);
        }
    }
}

impl World {
    /// Reads the side's economy and sets what its builders, factories and upgrades
    /// should do this think. Runs before them.
    pub(in crate::ai) fn plan_economy(&mut self, player: u8, census: &Census, intel: &Intel) {
        let p = player as usize;
        let pl = &self.state.players[p];
        let income = pl.mass_income + pl.reclaim_income;
        let fill = if pl.mass_capacity > Fx::ZERO {
            pl.mass / pl.mass_capacity
        } else {
            Fx::ZERO
        };
        let first = self.state.ai[p].commander.eco.speed == Fx::ZERO;
        let (speed_now, mine_power) = (pl.build_speed, pl.mine_power);
        let (energy, energy_cap) = (pl.energy, pl.energy_capacity);
        let (energy_income, energy_spent) = (pl.energy_income, pl.energy_spent);
        let mass_spent = pl.mass_spent;
        let skill = self.state.ai[p].config.skill();
        let start = pl.start;

        let eco = &self.state.ai[p].commander.eco;
        let smooth = |old: Fx, now: Fx| {
            if first {
                now
            } else {
                (old * 3 + now) / 4
            }
        };
        let fill_avg = smooth(eco.fill, fill);
        let speed = smooth(eco.speed, speed_now);
        let floating = fill_avg > FLOAT_FILL && fill > FLOAT_FILL / 2;
        let stalling = !floating && speed < STALL_SPEED;

        // Power for spending the whole income (nearly everything costs energy and
        // mass in about one ratio) and the upkeep of all that stands or
        // goes up, with room to spare. Judged by builders' draw it read 900 a second
        // short of a side spending 110 materials, and the mines dug at half pay.
        let upkeep: Fx = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| self.state.units.owner[r] == player)
            .map(|r| self.bp(r).economy.energy_upkeep)
            .sum();
        // Spent, not earned, while the store pays for more: the opening spends its
        // starting materials, and reading income alone it ran its energy dry.
        let spending = income.max(mass_spent).max(Fx::from_int(5));
        // At the skill's ratio (8 on Hard): costs run 4 to 7 energy a unit of mass,
        // and at 6 the store ran dry a seventh of the game.
        let need = upkeep + spending * skill.power_ratio.max(ENERGY_PER_MASS);
        let short = (need * 11 / 10 - energy_income).max(Fx::ZERO);
        let dry = energy < energy_cap * Fx::ratio(3, 20);
        let draining = energy < energy_cap / 2 && energy_spent > energy_income;
        let power = if mine_power < Fx::ratio(49, 50) || dry {
            Power::Urgent
        } else if draining || (short > Fx::ZERO && energy <= energy_cap * Fx::ratio(9, 10)) {
            Power::Want
        } else {
            Power::Enough
        };

        // Build power against income, each in its share: the factories get the
        // army's (more with a push on, less while booming), builders the rest.
        // Builders counted against all of it crowded out factories: one factory at
        // 57 a second, the army never grew.
        let units = &self.state.units;
        let builder_power: Fx = units
            .slots
            .iter()
            .filter(|&r| units.owner[r] == player && units.is_active(r))
            .filter_map(|r| {
                let bp = self.bp(r);
                bp.builder
                    .as_ref()
                    .filter(|_| bp.is_mobile() && bp.has(cat::ENGINEER))
                    .map(|b| b.power)
            })
            .sum();
        let c = &self.state.ai[p].commander;
        let army_share = Fx::from_int(
            (50 + 10 * c.plan(PlanKind::Pressure) as i32 - 10 * c.plan(PlanKind::Boom) as i32)
                .clamp(30, 75),
        ) / 100;
        let spend = if floating {
            income * 13 / 10
        } else {
            income * 11 / 10
        };
        let factory_draw = self.factory_mass_draw(player);
        // A filling store always has room for more: what it cannot spend is lost.
        let room = |draw: Fx, share: Fx| floating || (!stalling && draw < spend * share);

        // Free ore on the side's half: what expanders are for.
        let half = intel
            .enemy_start
            .map_or(Fx::from_int(skill.mine_travel), |e| {
                e.distance(start) * 11 / 20
            })
            .max(Fx::from_int(skill.mine_travel));
        let blocked = self.blocked_claims(player);
        let free = self
            .free_ores(start, &blocked, half, intel)
            .len()
            .min(u16::MAX as usize);
        let boom = self.state.ai[p].commander.plan(PlanKind::Boom);
        let expanders_wanted = if free == 0 {
            0
        } else {
            (1 + (boom >= Stake::Invest) as usize + (free >= 6) as usize)
                .min(free.div_ceil(2))
                .min(MOST_EXPANDERS)
        };
        // A force the plans want with no factory to make it gets one even while
        // stalling: its plans were held all in for twenty minutes with no shipyard
        // while boats shelled its coast, since a stalling side builds no factory.
        let shares = self.force_shares(player);
        let missing = (shares[1] > 0 && census.air_factories == 0)
            || (shares[2] + shares[3] > 0 && census.naval_factories == 0);
        let factories_wanted = census.factories.len()
            + ((room(factory_draw, army_share) || missing)
                && census.factories.len() < skill.factory_cap as usize) as usize;
        // Engineers by what is waiting for them: one more while every builder is
        // busy and power, ore or a factory waits, never more while stalling, and
        // at most twice what the builders' share of the income could keep at full
        // speed (counted up from what it had, "two more" each think made 26).
        let per_engineer = if census.engineers > 0 {
            builder_power / Fx::from_int(census.engineers as i32)
        } else {
            Fx::from_int(5)
        }
        .max(Fx::ONE);
        let share =
            spend * (Fx::ONE - army_share) * if floating { Fx::ratio(4, 3) } else { Fx::ONE };
        let ceiling = (share * 2 / (per_engineer * DRAW_PER_POWER))
            .floor_int()
            .max(0) as usize;
        let busy = census
            .builders_idle
            .iter()
            .all(|&r| self.bp(r).has(cat::COMMANDER));
        let waiting = power != Power::Enough
            || free > expanders_wanted
            || floating
            || census.factories.len() < factories_wanted;
        let mut engineers = census.engineers;
        if busy && waiting && !stalling && engineers < ceiling {
            engineers += 1;
        }
        // The floor: two and the expanders, but in the first minutes no more than
        // the income feeds (five engineers before any mine stalled minute three).
        let floor = (LEAST_ENGINEERS + expanders_wanted + 2 * (power != Power::Enough) as usize)
            .min(2 + (income / 4).floor_int().max(0) as usize);
        let engineers = engineers.max(floor).min(MOST_ENGINEERS);
        let factories = factories_wanted;

        // Mine upgrades: the best sink when the store fills, held to a payback.
        let upgrades = 1 + (income / 30).floor_int().max(0) as usize + 2 * floating as usize;
        let mut payback = skill.upgrade_payback.max(0) as u32;
        if floating {
            payback *= 2;
        }
        if free == 0 {
            payback = payback * 3 / 2;
        }

        // The commander works out in its half early, while no armed enemy is near it.
        let commander = units.row(pl.commander).filter(|&r| units.is_active(r));
        let threat_near = |at: FxVec2| {
            self.state.ai[p]
                .contacts
                .iter()
                .filter(|c| {
                    let bp = self.blueprints.unit(c.blueprint);
                    bp.is_mobile()
                        && !bp.weapons.is_empty()
                        && !bp.has(cat::ENGINEER)
                        && c.pos.distance(at) < Fx::from_int(ROAM_SAFE)
                        && self.state.tick.saturating_sub(c.seen) < 600
                })
                .map(|c| self.blueprints.unit(c.blueprint).cost_mass)
                .sum::<Fx>()
                >= Fx::from_int(ROAM_THREAT)
        };
        let roam = match (commander, intel.enemy_start) {
            (Some(r), Some(e))
                if self.state.tick < ROAM_UNTIL
                    && units.health[r] * 10 >= self.bp(r).health * 7
                    && !threat_near(units.pos[r]) =>
            {
                (e.distance(start) * ROAM_SHARE).min(Fx::from_int(ROAM_MOST))
            }
            _ => Fx::ZERO,
        };

        // Expanders: keep the living, take more from the engineers (nearest home
        // first, lowest tier first: the best ones build the big things).
        let mut expanders: Vec<UnitId> = self.state.ai[p]
            .commander
            .eco
            .expanders
            .iter()
            .copied()
            .filter(|&u| units.row(u).is_some_and(|r| units.is_active(r)))
            .collect();
        expanders.truncate(expanders_wanted);
        if expanders.len() < expanders_wanted {
            let mut spare: Vec<(u8, Fx, UnitId)> = units
                .slots
                .iter()
                .filter(|&r| units.owner[r] == player && units.is_active(r))
                .filter(|&r| {
                    let bp = self.bp(r);
                    bp.is_mobile()
                        && bp.has(cat::ENGINEER)
                        && !bp.has(cat::COMMANDER)
                        && bp.builder.is_some()
                })
                .map(|r| (self.bp(r).tech, units.pos[r].distance(start), units.id(r)))
                .filter(|(_, _, u)| !expanders.contains(u))
                .collect();
            spare.sort_by_key(|&(tech, d, u)| (tech, d, u));
            for (_, _, u) in spare.into_iter().take(expanders_wanted - expanders.len()) {
                expanders.push(u);
            }
        }

        let eco = &mut self.state.ai[p].commander.eco;
        eco.fill = fill_avg;
        eco.speed = speed;
        eco.floating = floating;
        eco.stalling = stalling;
        eco.power = power;
        eco.power_short = if power == Power::Urgent {
            short.max(Fx::from_int(15))
        } else {
            short
        };
        eco.engineers = engineers as u16;
        eco.factories = factories as u16;
        eco.expanders = expanders;
        eco.free_ore = free as u16;
        eco.upgrades = upgrades.min(u16::MAX as usize) as u16;
        eco.payback = payback;
        eco.roam = roam;
        eco.reach = half;
    }

    /// The deposits lately found with no lot, as claims the mine search keeps off.
    pub(in crate::ai) fn blocked_claims(&self, player: u8) -> Vec<Claim> {
        self.state.ai[player as usize]
            .commander
            .eco
            .blocked
            .iter()
            .map(|&(pos, _)| Claim {
                pos,
                foot: 0,
                mine: false,
                factory: false,
                cover: Fx::ZERO,
            })
            .collect()
    }

    /// Remembers this think's failed deposits and forgets the old ones.
    pub(in crate::ai) fn note_failed_mines(&mut self, player: u8, failed: &[FxVec2]) {
        let tick = self.state.tick;
        let blocked = &mut self.state.ai[player as usize].commander.eco.blocked;
        blocked.retain(|&(_, t)| tick < t + BLOCKED_FOR);
        for &p in failed {
            if !blocked.iter().any(|&(b, _)| b == p) {
                blocked.push((p, tick));
            }
        }
    }

    /// Whether `row` is one of its side's expanders.
    pub(in crate::ai) fn is_expander(&self, row: usize) -> bool {
        let units = &self.state.units;
        self.state.ai[units.owner[row] as usize]
            .commander
            .eco
            .expanders
            .contains(&units.id(row))
    }

    /// An expander's next mine: the free ore on its side's half nearest to it, else
    /// the best bare ground nearest home.
    pub(in crate::ai) fn expander_job(
        &self,
        row: usize,
        start: FxVec2,
        facing: Angle,
        claimed: &[Claim],
        intel: &Intel,
    ) -> Option<Job> {
        let units = &self.state.units;
        let ai = &self.state.ai[units.owner[row] as usize];
        let reach = ai.commander.eco.reach;
        let bare = Fx::ratio(ai.config.skill().bare_mine_efficiency as i64, 100);
        let pos = units.pos[row];
        let spot = self
            .free_ores(start, claimed, reach, intel)
            .into_iter()
            .min_by_key(|d| (d.distance_sq(pos), d.x, d.y))
            .or_else(|| self.free_deposit(start, claimed, reach, intel, Some(bare)))?;
        self.job_structure(row, cat::EXTRACTOR, 1, spot, facing, Fx::ZERO, false)
    }

    /// For a commander out roaming with nothing to build: the nearest wreck of the
    /// richest wreck field within its roam.
    pub(in crate::ai) fn roam_wreck(&self, row: usize, fields: &[Field]) -> Option<WreckId> {
        let units = &self.state.units;
        if !self.bp(row).has(cat::COMMANDER) {
            return None;
        }
        let pl = &self.state.players[units.owner[row] as usize];
        let roam = self.commander_directives(units.owner[row]).roam;
        let field = fields
            .iter()
            .filter(|f| {
                f.mass >= Fx::from_int(ROAM_FIELD)
                    && f.at.distance(pl.start) <= roam.max(HOME_RADIUS)
            })
            .max_by_key(|f| (f.mass - f.at.distance(units.pos[row]) / 4, f.at.x, f.at.y))?;
        self.near_wreck(field.at, Fx::from_int(450))
    }

    /// The commander out of its home when it may no longer roam (an enemy near, or
    /// too late in the game) goes home: a commander lost far out loses the match.
    pub(in crate::ai) fn recall_commander(&self, player: u8, out: &mut Vec<Command>) {
        let units = &self.state.units;
        let pl = &self.state.players[player as usize];
        let Some(r) = units.row(pl.commander).filter(|&r| units.is_active(r)) else {
            return;
        };
        let c = &self.state.ai[player as usize].commander;
        // Keeping out of reach of what was shooting it (`king.rs`).
        if self.state.tick < c.king_fled {
            return;
        }
        let roam = c.eco.roam;
        let out_there = units.pos[r].distance(pl.start) > HOME_RADIUS + Fx::from_int(200);
        let homeward = self.state.orders.front(units, r).is_some_and(|o| {
            matches!(o.kind, crate::tables::OrderKind::Move)
                && o.pos.distance(pl.start) <= HOME_RADIUS
        });
        let allowed = units.pos[r].distance(pl.start) <= roam;
        if out_there && !allowed && !homeward {
            out.push(Command::Move {
                units: vec![units.id(r)],
                target: pl.start,
                queue: false,
            });
        }
    }

    /// Energy a second the plants going up will add.
    pub(in crate::ai) fn power_rising(&self, census: &Census) -> Fx {
        census
            .sites
            .iter()
            .filter(|&&s| self.bp(s).has(cat::POWER))
            .map(|&s| self.bp(s).economy.energy_income)
            .sum()
    }
}
