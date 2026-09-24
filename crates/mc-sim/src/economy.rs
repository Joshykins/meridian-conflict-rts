//! Flow economy. Income and spending are rates; when a player cannot cover
//! what their builders ask for, builders slow down. Upkeep and the building of
//! power and mines are paid first, so a stall never starves its own way out;
//! everything else shares what is left, every builder slowed by the same ratio.

use crate::mirror::SimEvent;
use crate::tables::*;
use crate::{SimError, World};
use mc_core::{Fx, TICKS_PER_SECOND};
use mc_data::cat;

const DT: i32 = TICKS_PER_SECOND as i32;

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
    /// True when this builds or upgrades power or a mine: paid before the rest.
    pub priority: bool,
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
        // The part of `demand` paid first: upkeep, and building power and mines.
        let mut first = vec![(Fx::ZERO, Fx::ZERO); player_count];
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
            let e = self.bp(row).economy;
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
        self.mine_income(&mut income);
        // The test range can turn a side's income up or down, and give it stores.
        for (p, pl) in self.state.players.iter().enumerate() {
            capacity[p].0 += pl.bonus_storage[0];
            capacity[p].1 += pl.bonus_storage[1];
            let [m, e] = pl.income_permille;
            if m != 1000 {
                income[p].0 = income[p].0 * m as i32 / 1000;
            }
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
            let power = self
                .bp(row)
                .builder
                .as_ref()
                .map_or(crate::orders::SELF_UPGRADE_POWER, |b| b.power);
            let tbp = self.bp(target);
            let priority = constructing && tbp.categories & (cat::POWER | cat::EXTRACTOR) != 0;
            let rate = power / DT;
            let progress = rate.min(remaining);
            let p = self.state.units.owner[row] as usize;
            let want = if shield {
                [Fx::ZERO, tbp.cost_energy * progress / tbp.build_time]
            } else {
                let scale = repair_scale(repair);
                [
                    tbp.cost_mass * progress / tbp.build_time * scale,
                    tbp.cost_energy * progress / tbp.build_time * scale,
                ]
            };
            demand[p].0 += want[0];
            demand[p].1 += want[1];
            if priority {
                first[p].0 += want[0];
                first[p].1 += want[1];
            }
            let flow = &mut self.flows[row];
            flow.wanted[0] += want[0];
            flow.wanted[1] += want[1];
            jobs.push(BuildJob {
                builder: row,
                target,
                rate,
                repair,
                shield,
                priority,
            });
        }
        for p in 0..player_count {
            demand[p].1 += upkeep[p];
            first[p].1 += upkeep[p];
        }

        // How much of the demand each player can cover: first what is paid
        // first, then the rest out of what that leaves.
        let mut efficiency = vec![Fx::ONE; player_count];
        let mut efficiency_first = vec![Fx::ONE; player_count];
        for p in 0..player_count {
            let pl = &self.state.players[p];
            if pl.free_build {
                continue;
            }
            let ratio = |have: Fx, want: Fx| {
                if want > have {
                    have.max(Fx::ZERO) / want
                } else {
                    Fx::ONE
                }
            };
            let have = (pl.mass + income[p].0, pl.energy + income[p].1);
            let e = ratio(have.0, first[p].0).min(ratio(have.1, first[p].1));
            let left = (have.0 - first[p].0 * e, have.1 - first[p].1 * e);
            let rest = (demand[p].0 - first[p].0, demand[p].1 - first[p].1);
            efficiency_first[p] = e;
            efficiency[p] = ratio(left.0, rest.0).min(ratio(left.1, rest.1));
        }

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
            let e = if job.priority {
                efficiency_first[p]
            } else {
                efficiency[p]
            };
            let step = (job.rate * e).min(remaining);
            let tbp = self.bp(job.target);
            let max_health = if job.repair {
                self.unit_max_health(job.target)
            } else {
                tbp.health
            };
            let (build_time, cost_mass, cost_energy) =
                (tbp.build_time, tbp.cost_mass, tbp.cost_energy);
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
                    // Health grows with progress from the 10% a fresh site starts with.
                    units.health[job.target] = (units.health[job.target]
                        + max_health * step / build_time * Fx::ratio(9, 10))
                    .min(max_health);
                }
            }
        }

        // Upkeep is paid first, at the efficiency of what is paid first.
        for row in self.state.units.slots.iter() {
            if self.state.units.is_active(row) {
                let e = efficiency_first[self.state.units.owner[row] as usize];
                let upkeep = self.bp(row).economy.energy_upkeep / DT;
                self.flows[row].used[1] += upkeep * e;
            }
        }

        for p in 0..player_count {
            let pl = &mut self.state.players[p];
            let e = efficiency_first[p];
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
            pl.reclaim_income = (pl.reclaimed_mass - pl.reclaimed_counted) * DT;
            pl.reclaimed_counted = pl.reclaimed_mass;
            pl.energy_income = income[p].1 * DT;
            pl.mass_demand = demand[p].0 * DT;
            pl.energy_demand = demand[p].1 * DT;
            pl.efficiency = e.min(efficiency[p]);
            pl.upkeep_efficiency = e;
        }

        // Completions, in target row order so the result does not depend on job order.
        jobs.sort_unstable_by_key(|j| j.target);
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
