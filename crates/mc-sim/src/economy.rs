//! Flow economy. Income and spending are rates; when a player cannot cover
//! what their builders ask for, everything slows down by the same ratio: builders,
//! factories, upkeep and the mines alike. The one order is the player's own
//! (`focus.rs`): new power or new mines can be paid in full first, or only out of
//! what the rest leaves over. Mines dig only as hard as their
//! energy is paid: a side out of energy makes less mass, which is what makes an
//! energy stall bad. A mass stall does not slow them, or it would feed itself.

use crate::mirror::SimEvent;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;
/// The tiers a side's spending is paid in, in turn (`Priority::tier`): what it puts
/// first, the rest, and what it puts last.
const TIERS: usize = 3;
const FIRST: usize = 0;
const REST: usize = 1;

/// What one tier of a side's spending asks for this tick, per tick.
#[derive(Clone, Copy, Default)]
struct Tier {
    mass: Fx,
    /// All the energy it asks for, `energy_only` included.
    energy: Fx,
    /// The part of `energy` asked for by draws that take no mass.
    energy_only: Fx,
}

impl Tier {
    fn add(&mut self, want: [Fx; 2], energy_only: bool) {
        self.mass += want[0];
        self.energy += want[1];
        if energy_only {
            self.energy_only += want[1];
        }
    }
}

/// The share of `want` that `have` covers, zero to one.
fn ratio(have: Fx, want: Fx) -> Fx {
    if want > have {
        have.max(Fx::ZERO) / want
    } else {
        Fx::ONE
    }
}

/// One unit's part in its side's economy this tick, in units per tick: `[mass, energy]`.
/// Not state; the mirror reports it to the interface.
#[derive(Clone, Copy, Default, Debug)]
pub struct UnitFlow {
    /// What it produced: generator and mine output, and materials it reclaimed.
    pub made: [Fx; 2],
    /// What its building, repairs and upkeep asked for at the full rate.
    pub wanted: [Fx; 2],
    /// What it was given of that, after the side's efficiency.
    pub used: [Fx; 2],
    /// Build time it gained as a site or product under construction, from every builder
    /// on it together: what the interface times its finish by.
    pub built: Fx,
}

/// One builder working on one target this tick.
#[derive(Clone, Copy)]
pub(crate) struct BuildJob {
    pub builder: usize,
    pub target: usize,
    /// Build-time units this builder adds per tick at full efficiency.
    pub rate: Fx,
    /// True when the target is complete and this is a repair.
    pub repair: bool,
    /// True when this is extra regen on a live shield bubble. Energy only;
    /// recovery after a break is not this.
    pub shield: bool,
    /// The tier it is paid in (`Priority::tier`).
    pub tier: usize,
}

impl World {
    /// This tick's flows of the unit in `row`, made room for if it is new.
    pub(crate) fn flow(&mut self, row: usize) -> &mut UnitFlow {
        if row >= self.flows.len() {
            self.flows.resize(row + 1, UnitFlow::default());
        }
        &mut self.flows[row]
    }

    pub(crate) fn run_economy(&mut self) -> Result<(), SimError> {
        let player_count = self.state.players.len();
        let mut income = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut demand = vec![(Fx::ZERO, Fx::ZERO); player_count];
        // `demand` split by the tier it is paid in.
        let mut tiers = vec![[Tier::default(); TIERS]; player_count];
        let mut capacity = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut upkeep = vec![Fx::ZERO; player_count];
        let mut spent = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let rows = self.state.units.slots.rows();
        if self.flows.len() < rows {
            self.flows.resize(rows, UnitFlow::default());
        }

        // Production, upkeep and storage from every active unit.
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row) {
                continue;
            }
            let mut e = self.bp(row).economy;
            if self.powered_down(row) {
                e.energy_upkeep = Fx::ZERO;
            }
            let p = self.state.units.owner[row] as usize;
            income[p].0 += e.mass_income / DT;
            income[p].1 += e.energy_income / DT;
            upkeep[p] += e.energy_upkeep / DT;
            capacity[p].0 += e.mass_storage;
            capacity[p].1 += e.energy_storage;
            let flow = &mut self.flows[row];
            flow.made[0] += e.mass_income / DT;
            flow.made[1] += e.energy_income / DT;
            flow.wanted[1] += e.energy_upkeep / DT;
        }
        // The test range can turn a side's income up or down, and give it stores.
        // Mass is turned once the mines are counted, below.
        for (p, pl) in self.state.players.iter().enumerate() {
            capacity[p].0 += pl.bonus_storage[0];
            capacity[p].1 += pl.bonus_storage[1];
            let e = pl.income_permille[1];
            if e != 1000 {
                income[p].1 = income[p].1 * e as i32 / 1000;
            }
        }

        // Builders: gather what each wants to do this tick.
        let mut jobs = std::mem::take(&mut self.scratch.build_jobs);
        jobs.clear();
        for row in self.state.units.slots.iter() {
            if self.state.units.flags[row] & flag::BUILDING == 0 {
                continue;
            }
            let Some(target) = self.state.units.row(self.state.units.build_target[row]) else {
                continue;
            };
            // Extractors, intel towers and shield generators upgrade themselves without being builders.
            let constructing = self.state.units.has_flag(target, flag::UNDER_CONSTRUCTION);
            let shield = !constructing && self.shield_assistable(target);
            let repair = !constructing && !shield;
            let remaining = if shield {
                self.shield_work_remaining(target)
            } else {
                self.work_remaining(target, repair)
            };
            if remaining <= Fx::ZERO {
                continue;
            }
            let tbp = self.bp(target);
            // Its own upgrade (the successor is a unit of its own) at the power
            // `upgrade_power` gives it; anything else at its own.
            let own_upgrade = self.state.units.has_flag(target, flag::UPGRADE)
                && self.bp(row).upgrades_to == Some(tbp.id);
            let power = if own_upgrade {
                self.blueprints
                    .upgrade_power(self.bp(row), tbp, crate::orders::SELF_UPGRADE_POWER)
            } else {
                self.bp(row)
                    .builder
                    .as_ref()
                    .map_or(crate::orders::SELF_UPGRADE_POWER, |b| b.power)
            };
            let focus = self.state.players[self.state.units.owner[row] as usize].focus;
            let tier = if constructing {
                focus.priority(tbp, &self.blueprints).tier()
            } else {
                REST
            };
            let rate = power / DT;
            let progress = rate.min(remaining);
            let p = self.state.units.owner[row] as usize;
            let want = if shield {
                [Fx::ZERO, tbp.cost_energy * progress / tbp.build_time]
            } else {
                let scale = repair_scale(repair);
                let (mass, energy) = self.work_cost(target, repair);
                [
                    mass * progress / tbp.build_time * scale,
                    energy * progress / tbp.build_time * scale,
                ]
            };
            demand[p].0 += want[0];
            demand[p].1 += want[1];
            tiers[p][tier].add(want, shield);
            let flow = &mut self.flows[row];
            flow.wanted[0] += want[0];
            flow.wanted[1] += want[1];
            jobs.push(BuildJob {
                builder: row,
                target,
                rate,
                repair,
                shield,
                tier,
            });
        }
        for p in 0..player_count {
            demand[p].1 += upkeep[p];
            tiers[p][REST].add([Fx::ZERO, upkeep[p]], true);
        }
        // Strategic launchers assembling rounds (`nukes.rs`): paid like any other build.
        let launchers = self.launcher_jobs();
        for &(row, _, want) in &launchers {
            let p = self.state.units.owner[row] as usize;
            demand[p].0 += want[0];
            demand[p].1 += want[1];
            tiers[p][REST].add(want, false);
            self.flows[row].wanted[0] += want[0];
            self.flows[row].wanted[1] += want[1];
        }
        // Drones going up on their carriers (`air_support.rs`): paid in the tier the
        // side's materials priority puts them in, and shown on the carrier.
        let drones = self.drone_jobs();
        for job in &drones {
            let p = self.state.units.owner[job.drone] as usize;
            demand[p].0 += job.want[0];
            demand[p].1 += job.want[1];
            tiers[p][job.tier].add(job.want, false);
            self.flows[job.carrier].wanted[0] += job.want[0];
            self.flows[job.carrier].wanted[1] += job.want[1];
        }

        // Warp drives charging (`warp.rs`): energy only, paid with the rest.
        let mut warps = Vec::new();
        for row in self.state.units.slots.iter() {
            if let Some((rate, left)) = self.warp_draw(row) {
                let want = rate.min(left);
                let p = self.state.units.owner[row] as usize;
                demand[p].1 += want;
                tiers[p][REST].add([Fx::ZERO, want], true);
                self.flows[row].wanted[1] += want;
                warps.push((row, rate, left));
            }
        }

        // Mines draw their upkeep with the rest, and dig as hard as the rest's energy
        // is covered. Mass is not known yet (the mines make it), so what is put first is
        // taken to spend all the energy it asks for: never more than it does.
        let mut powered = vec![Fx::ONE; player_count];
        for (p, pl) in self.state.players.iter().enumerate() {
            if pl.free_build {
                continue;
            }
            let mut have = pl.energy + income[p].1;
            have = (have - tiers[p][FIRST].energy).max(Fx::ZERO);
            powered[p] = ratio(have, tiers[p][REST].energy);
        }
        let mine_lost = self.mine_income(&mut income, &powered);
        self.fabricator_income(&mut income, &powered);
        for (p, pl) in self.state.players.iter().enumerate() {
            let m = pl.income_permille[0];
            if m != 1000 {
                income[p].0 = income[p].0 * m as i32 / 1000;
            }
        }

        // How much of each tier each player can cover, in turn, each out of what the
        // tiers before it left. `paid` is for work that takes mass and energy, `paid_energy`
        // for energy-only draws (upkeep, shield regen), which a mass stall does not slow.
        let mut paid = vec![[Fx::ONE; TIERS]; player_count];
        let mut paid_energy = vec![[Fx::ONE; TIERS]; player_count];
        for p in 0..player_count {
            let pl = &self.state.players[p];
            if pl.free_build {
                continue;
            }
            let mut left = (pl.mass + income[p].0, pl.energy + income[p].1);
            for (t, tier) in tiers[p].iter().enumerate() {
                let energy = ratio(left.1, tier.energy);
                let both = ratio(left.0, tier.mass).min(energy);
                paid[p][t] = both;
                paid_energy[p][t] = energy;
                left.0 = (left.0 - tier.mass * both).max(Fx::ZERO);
                left.1 =
                    (left.1 - tier.energy_only * energy - (tier.energy - tier.energy_only) * both)
                        .max(Fx::ZERO);
            }
        }

        // Build power asked for and delivered, for the stall readout, per tier.
        let mut power = vec![[(Fx::ZERO, Fx::ZERO); TIERS]; player_count];
        for job in &jobs {
            let p = self.state.units.owner[job.builder] as usize;
            // A stall slows the builder down, never the little that is left to do:
            // a remainder scaled by the stall shrinks for ever and then rounds to nothing.
            // Measured here, not when the job was gathered, so helpers share what is left.
            let remaining = if job.shield {
                self.shield_work_remaining(job.target)
            } else {
                self.work_remaining(job.target, job.repair)
            };
            let e = if job.shield {
                paid_energy[p][job.tier]
            } else {
                paid[p][job.tier]
            };
            let step = (job.rate * e).min(remaining);
            power[p][job.tier].0 += job.rate;
            power[p][job.tier].1 += job.rate * e;
            let tbp = self.bp(job.target);
            let max_health = if job.repair {
                self.unit_max_health(job.target)
            } else {
                tbp.health
            };
            let (cost_mass, cost_energy) = self.work_cost(job.target, job.repair);
            let build_time = tbp.build_time;
            let shield_max = tbp.shield.map(|s| s.health);
            if job.shield {
                spent[p].1 += cost_energy * step / build_time;
                self.flows[job.builder].used[1] += cost_energy * step / build_time;
                let max = shield_max.unwrap_or(Fx::ZERO);
                let units = &mut self.state.units;
                let filled = if step >= remaining {
                    max
                } else {
                    units.shield_hp[job.target] + max * step / build_time
                };
                units.shield_hp[job.target] = filled.min(max);
            } else {
                let scale = repair_scale(job.repair);
                let got = [
                    cost_mass * step / build_time * scale,
                    cost_energy * step / build_time * scale,
                ];
                spent[p].0 += got[0];
                spent[p].1 += got[1];
                let flow = &mut self.flows[job.builder];
                flow.used[0] += got[0];
                flow.used[1] += got[1];
                let units = &mut self.state.units;
                if job.repair {
                    // The last step lands on full health exactly, whatever the division dropped.
                    let healed = if step >= remaining {
                        max_health
                    } else {
                        units.health[job.target] + max_health * step / build_time
                    };
                    units.health[job.target] = healed.min(max_health);
                } else {
                    units.build_progress[job.target] =
                        (units.build_progress[job.target] + step).min(build_time);
                    self.flows[job.target].built += step;
                    // Health grows with progress from the 10% a fresh site starts with.
                    units.health[job.target] = (units.health[job.target]
                        + max_health * step / build_time * Fx::ratio(9, 10))
                    .min(max_health);
                }
            }
        }

        for &(row, rate, want) in &launchers {
            let p = self.state.units.owner[row] as usize;
            let e = paid[p][REST];
            power[p][REST].0 += rate;
            power[p][REST].1 += rate * e;
            spent[p].0 += want[0] * e;
            spent[p].1 += want[1] * e;
            self.flows[row].used[0] += want[0] * e;
            self.flows[row].used[1] += want[1] * e;
            self.advance_launchers(row, rate * e);
        }
        for job in &drones {
            let p = self.state.units.owner[job.drone] as usize;
            let e = paid[p][job.tier];
            power[p][job.tier].0 += job.rate;
            power[p][job.tier].1 += job.rate * e;
            spent[p].0 += job.want[0] * e;
            spent[p].1 += job.want[1] * e;
            self.flows[job.carrier].used[0] += job.want[0] * e;
            self.flows[job.carrier].used[1] += job.want[1] * e;
            let bp = self.bp(job.drone);
            let (time, health) = (bp.build_time, bp.health);
            let units = &mut self.state.units;
            // A stall slows it down, never the little that is left (as for builders).
            let step = e.min(time - units.build_progress[job.drone]);
            units.build_progress[job.drone] = (units.build_progress[job.drone] + step).min(time);
            self.flows[job.drone].built += step;
            units.health[job.drone] =
                (units.health[job.drone] + health * step / time * Fx::ratio(9, 10)).min(health);
        }

        for &(row, rate, left) in &warps {
            let p = self.state.units.owner[row] as usize;
            let got = (rate * paid_energy[p][REST]).min(left);
            spent[p].1 += got;
            self.flows[row].used[1] += got;
            self.state.units.warp[row].charge += got;
        }

        // Upkeep is paid with the rest, as far as its energy goes.
        for row in self.state.units.slots.iter() {
            if self.state.units.is_active(row) && !self.powered_down(row) {
                let e = paid_energy[self.state.units.owner[row] as usize][REST];
                let upkeep = self.bp(row).economy.energy_upkeep / DT;
                self.flows[row].used[1] += upkeep * e;
            }
        }

        for p in 0..player_count {
            let pl = &mut self.state.players[p];
            let e = paid_energy[p][REST];
            pl.mass_capacity = capacity[p].0;
            pl.energy_capacity = capacity[p].1;
            if pl.free_build {
                spent[p] = (Fx::ZERO, Fx::ZERO);
                upkeep[p] = Fx::ZERO;
            }
            let energy_spent = upkeep[p] * e + spent[p].1;
            pl.mass = (pl.mass + income[p].0 - spent[p].0).clamp(Fx::ZERO, capacity[p].0);
            pl.energy = (pl.energy + income[p].1 - energy_spent).clamp(Fx::ZERO, capacity[p].1);
            pl.mass_spent = spent[p].0 * DT;
            pl.energy_spent = energy_spent * DT;
            pl.mass_income = income[p].0 * DT;
            pl.reclaim_income = (pl.reclaimed_mass - pl.reclaimed_counted) * DT;
            pl.reclaimed_counted = pl.reclaimed_mass;
            pl.energy_income = income[p].1 * DT;
            pl.mass_demand = demand[p].0 * DT;
            pl.energy_demand = demand[p].1 * DT;
            pl.efficiency = paid[p].iter().fold(Fx::ONE, |a, &b| a.min(b));
            pl.upkeep_efficiency = e;
            pl.mine_power = powered[p];
            pl.mine_lost = mine_lost[p] * DT;
            let speed = |(asked, got): (Fx, Fx)| (got / asked).min(Fx::ONE);
            let all = power[p]
                .iter()
                .fold((Fx::ZERO, Fx::ZERO), |a, b| (a.0 + b.0, a.1 + b.1));
            pl.build_speed = if pl.free_build || all.0 <= Fx::ZERO {
                Fx::ONE
            } else {
                speed(all)
            };
            pl.tier_speed = std::array::from_fn(|t| {
                (power[p][t].0 > Fx::ZERO).then(|| {
                    if pl.free_build {
                        Fx::ONE
                    } else {
                        speed(power[p][t])
                    }
                })
            });
        }

        // Completions, in target row order so the result does not depend on job order. A
        // stable sort, so which of several jobs on one target is kept does not depend on
        // how the standard library breaks ties either.
        jobs.sort_by_key(|j| j.target);
        jobs.dedup_by_key(|j| j.target);
        for job in &jobs {
            let units = &self.state.units;
            if !job.repair
                && !job.shield
                && units.build_progress[job.target] >= self.bp(job.target).build_time
            {
                self.complete_unit(job.target)?;
            }
        }
        for job in &drones {
            if self.state.units.build_progress[job.drone] >= self.bp(job.drone).build_time {
                self.complete_unit(job.drone)?;
            }
        }
        self.scratch.build_jobs = jobs;
        Ok(())
    }

    /// What building (or mending) `target` in full costs, (materials, energy): an
    /// upgrade is priced by `Blueprints::upgrade_cost`, anything else by its blueprint.
    fn work_cost(&self, target: usize, repair: bool) -> (Fx, Fx) {
        let tbp = self.bp(target);
        if !repair && self.state.units.has_flag(target, flag::UPGRADE) {
            self.blueprints.upgrade_cost(tbp)
        } else {
            (tbp.cost_mass, tbp.cost_energy)
        }
    }

    /// What is left to do on a build or a repair, in build-time units so slow
    /// builds keep full precision.
    fn work_remaining(&self, target: usize, repair: bool) -> Fx {
        let (units, tbp) = (&self.state.units, self.bp(target));
        if repair {
            // Any missing health is work, even when it is too little to survive the division.
            let max = self.unit_max_health(target);
            let missing = max - units.health[target];
            if missing > Fx::ZERO {
                (missing * tbp.build_time / max.max(Fx::ONE)).max(Fx::EPSILON)
            } else {
                Fx::ZERO
            }
        } else {
            tbp.build_time - units.build_progress[target]
        }
    }

    /// A unit finished construction: it becomes active. Factory products and
    /// upgrades are released by their parent's order logic on the next tick.
    pub(crate) fn complete_unit(&mut self, row: usize) -> Result<(), SimError> {
        self.state.units.flags[row] &= !flag::UNDER_CONSTRUCTION;
        self.state.units.health[row] = self.unit_max_health(row);
        let owner = self.state.units.owner[row];
        self.state.players[owner as usize].units_built += 1;
        self.events.push(SimEvent::UnitCompleted {
            unit: self.state.units.id(row),
            owner,
        });
        let bp = self
            .blueprints
            .unit(self.state.units.blueprint[row])
            .clone();
        let pos = self.state.units.pos[row];
        self.remember_structure_pad(&bp, pos, owner)?;
        // An experimental raised on a lot drives off it: the lot is free again.
        if bp.is_site_built_unit() {
            let heading = self.state.units.heading[row];
            self.release_lot(row, &bp, pos, heading);
        }
        if self.state.units.has_flag(row, flag::UPGRADE) {
            self.inherit_shield(row);
        } else {
            self.arm_shield(row, false);
        }
        Ok(())
    }

    /// Mass an engineer pulls out of a wreck per tick per point of build power.
    pub(crate) fn reclaim_rate() -> Fx {
        Fx::ratio(1, DT as i64)
    }
}

/// Repairs cost a quarter of building from scratch.
fn repair_scale(repair: bool) -> Fx {
    if repair {
        Fx::ratio(1, 4)
    } else {
        Fx::ONE
    }
}
