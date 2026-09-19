//! Flow economy. Income and spending are rates; when a player cannot cover
//! what their builders ask for, every builder slows by the same ratio.

use crate::mirror::SimEvent;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;

/// One builder working on one target this tick.
#[derive(Clone, Copy)]
pub(crate) struct BuildJob {
    pub builder: usize,
    pub target: usize,
    /// Build-time units this builder would add at full efficiency.
    pub progress: Fx,
    /// True when the target is complete and this is a repair.
    pub repair: bool,
}

impl World {
    pub(crate) fn run_economy(&mut self) -> Result<(), SimError> {
        let player_count = self.state.players.len();
        let mut income = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut demand = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut capacity = vec![(Fx::ZERO, Fx::ZERO); player_count];

        // Production, upkeep and storage from every active unit.
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row) {
                continue;
            }
            let e = &self.bp(row).economy;
            let p = self.state.units.owner[row] as usize;
            income[p].0 += e.mass_income / DT;
            income[p].1 += e.energy_income / DT;
            demand[p].1 += e.energy_upkeep / DT;
            capacity[p].0 += e.mass_storage;
            capacity[p].1 += e.energy_storage;
        }

        // Builders: gather what each wants to do this tick.
        let mut jobs = std::mem::take(&mut self.scratch.build_jobs);
        jobs.clear();
        for row in self.state.units.slots.iter() {
            let units = &self.state.units;
            if units.flags[row] & flag::BUILDING == 0 {
                continue;
            }
            let Some(target) = units.row(units.build_target[row]) else { continue };
            // Extractors upgrade themselves without being builders.
            let power = self.bp(row).builder.as_ref().map_or(crate::orders::SELF_UPGRADE_POWER, |b| b.power);
            let tbp = self.bp(target);
            let repair = units.flags[target] & flag::UNDER_CONSTRUCTION == 0;
            // Progress is counted in build-time units so slow builds keep full precision.
            let remaining = if repair {
                (tbp.health - units.health[target]) * tbp.build_time / tbp.health
            } else {
                tbp.build_time - units.build_progress[target]
            };
            if remaining <= Fx::ZERO {
                continue;
            }
            let progress = (power / DT).min(remaining);
            let p = units.owner[row] as usize;
            // Repairs cost a fraction of building from scratch.
            let scale = if repair { Fx::ratio(1, 2) } else { Fx::ONE };
            demand[p].0 += tbp.cost_mass * progress / tbp.build_time * scale;
            demand[p].1 += tbp.cost_energy * progress / tbp.build_time * scale;
            jobs.push(BuildJob { builder: row, target, progress, repair });
        }

        // How much of the demand each player can cover.
        let mut efficiency = vec![Fx::ONE; player_count];
        for p in 0..player_count {
            let pl = &self.state.players[p];
            let ratio = |have: Fx, want: Fx| if want > have { have.max(Fx::ZERO) / want } else { Fx::ONE };
            efficiency[p] = ratio(pl.mass + income[p].0, demand[p].0).min(ratio(pl.energy + income[p].1, demand[p].1));
        }

        for job in &jobs {
            let p = self.state.units.owner[job.builder] as usize;
            let step = job.progress * efficiency[p];
            let (max_health, build_time) = (self.bp(job.target).health, self.bp(job.target).build_time);
            let units = &mut self.state.units;
            if job.repair {
                units.health[job.target] = (units.health[job.target] + max_health * step / build_time).min(max_health);
            } else {
                units.build_progress[job.target] = (units.build_progress[job.target] + step).min(build_time);
                // Health grows with progress from the 10% a fresh site starts with.
                units.health[job.target] = (units.health[job.target] + max_health * step / build_time * Fx::ratio(9, 10)).min(max_health);
            }
        }

        for p in 0..player_count {
            let pl = &mut self.state.players[p];
            let e = efficiency[p];
            pl.mass_capacity = capacity[p].0;
            pl.energy_capacity = capacity[p].1;
            pl.mass = (pl.mass + income[p].0 - demand[p].0 * e).clamp(Fx::ZERO, capacity[p].0);
            pl.energy = (pl.energy + income[p].1 - demand[p].1 * e).clamp(Fx::ZERO, capacity[p].1);
            pl.mass_income = income[p].0 * DT;
            pl.energy_income = income[p].1 * DT;
            pl.mass_demand = demand[p].0 * DT;
            pl.energy_demand = demand[p].1 * DT;
            pl.efficiency = e;
        }

        // Completions, in target row order so the result does not depend on job order.
        jobs.sort_unstable_by_key(|j| j.target);
        jobs.dedup_by_key(|j| j.target);
        for job in &jobs {
            let units = &self.state.units;
            if !job.repair && units.build_progress[job.target] >= self.bp(job.target).build_time {
                self.complete_unit(job.target)?;
            }
        }
        self.scratch.build_jobs = jobs;
        Ok(())
    }

    /// A unit finished construction: it becomes active. Factory products and
    /// upgrades are released by their parent's order logic on the next tick.
    fn complete_unit(&mut self, row: usize) -> Result<(), SimError> {
        let units = &mut self.state.units;
        units.flags[row] &= !flag::UNDER_CONSTRUCTION;
        units.health[row] = self.blueprints.unit(units.blueprint[row]).health;
        let owner = units.owner[row];
        self.state.players[owner as usize].units_built += 1;
        self.events.push(SimEvent::UnitCompleted { unit: self.state.units.id(row), owner });
        Ok(())
    }

    /// Mass an engineer pulls out of a wreck per tick per point of build power.
    pub(crate) fn reclaim_rate() -> Fx {
        Fx::ratio(1, DT as i64)
    }
}
