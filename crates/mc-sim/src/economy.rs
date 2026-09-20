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
    /// Build-time units this builder adds per tick at full efficiency.
    pub rate: Fx,
    /// True when the target is complete and this is a repair.
    pub repair: bool,
}

impl World {
    pub(crate) fn run_economy(&mut self) -> Result<(), SimError> {
        let player_count = self.state.players.len();
        let mut income = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut demand = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut capacity = vec![(Fx::ZERO, Fx::ZERO); player_count];
        let mut upkeep = vec![Fx::ZERO; player_count];
        let mut spent = vec![(Fx::ZERO, Fx::ZERO); player_count];

        // Production, upkeep and storage from every active unit.
        for row in self.state.units.slots.iter() {
            if !self.state.units.is_active(row) {
                continue;
            }
            let e = &self.bp(row).economy;
            let p = self.state.units.owner[row] as usize;
            income[p].0 += e.mass_income / DT;
            income[p].1 += e.energy_income / DT;
            upkeep[p] += e.energy_upkeep / DT;
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
            let Some(target) = units.row(units.build_target[row]) else {
                continue;
            };
            // Extractors upgrade themselves without being builders.
            let power = self
                .bp(row)
                .builder
                .as_ref()
                .map_or(crate::orders::SELF_UPGRADE_POWER, |b| b.power);
            let tbp = self.bp(target);
            let repair = units.flags[target] & flag::UNDER_CONSTRUCTION == 0;
            let remaining = self.work_remaining(target, repair);
            if remaining <= Fx::ZERO {
                continue;
            }
            let rate = power / DT;
            let progress = rate.min(remaining);
            let p = units.owner[row] as usize;
            let scale = repair_scale(repair);
            demand[p].0 += tbp.cost_mass * progress / tbp.build_time * scale;
            demand[p].1 += tbp.cost_energy * progress / tbp.build_time * scale;
            jobs.push(BuildJob {
                builder: row,
                target,
                rate,
                repair,
            });
        }
        for p in 0..player_count {
            demand[p].1 += upkeep[p];
        }

        // How much of the demand each player can cover.
        let mut efficiency = vec![Fx::ONE; player_count];
        for p in 0..player_count {
            let pl = &self.state.players[p];
            let ratio = |have: Fx, want: Fx| {
                if want > have {
                    have.max(Fx::ZERO) / want
                } else {
                    Fx::ONE
                }
            };
            efficiency[p] = if pl.free_build {
                Fx::ONE
            } else {
                ratio(pl.mass + income[p].0, demand[p].0)
                    .min(ratio(pl.energy + income[p].1, demand[p].1))
            };
        }

        for job in &jobs {
            let p = self.state.units.owner[job.builder] as usize;
            // A stall slows the builder down, never the little that is left to do:
            // a remainder scaled by the stall shrinks for ever and then rounds to nothing.
            // Measured here, not when the job was gathered, so helpers share what is left.
            let remaining = self.work_remaining(job.target, job.repair);
            let step = (job.rate * efficiency[p]).min(remaining);
            let tbp = self.bp(job.target);
            let max_health = if job.repair {
                self.unit_max_health(job.target)
            } else {
                tbp.health
            };
            let build_time = tbp.build_time;
            let scale = repair_scale(job.repair);
            spent[p].0 += tbp.cost_mass * step / build_time * scale;
            spent[p].1 += tbp.cost_energy * step / build_time * scale;
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
                // Health grows with progress from the 10% a fresh site starts with.
                units.health[job.target] = (units.health[job.target]
                    + max_health * step / build_time * Fx::ratio(9, 10))
                .min(max_health);
            }
        }

        for p in 0..player_count {
            let pl = &mut self.state.players[p];
            let e = efficiency[p];
            pl.mass_capacity = capacity[p].0;
            pl.energy_capacity = capacity[p].1;
            if pl.free_build {
                spent[p] = (Fx::ZERO, Fx::ZERO);
                upkeep[p] = Fx::ZERO;
            }
            pl.mass = (pl.mass + income[p].0 - spent[p].0).clamp(Fx::ZERO, capacity[p].0);
            pl.energy = (pl.energy + income[p].1 - upkeep[p] * e - spent[p].1)
                .clamp(Fx::ZERO, capacity[p].1);
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
        Ok(())
    }

    /// Mass an engineer pulls out of a wreck per tick per point of build power.
    pub(crate) fn reclaim_rate() -> Fx {
        Fx::ratio(1, DT as i64)
    }
}

/// Repairs cost a fraction of building from scratch.
fn repair_scale(repair: bool) -> Fx {
    if repair {
        Fx::ratio(1, 2)
    } else {
        Fx::ONE
    }
}
