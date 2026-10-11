//! Orders for the titans (`ops::titan`): fighting units built on a lot, a tier 4 or 5,
//! that walk or float. A titan is worth a whole wave, and it does what a player does
//! with one: sends it in alone, on an attack-move, so its own guns take what they
//! outrange on the way (a Behemoth razed two shields, four tech 3 turrets and a
//! factory from 2.4 km in ten seconds). Held in the guard or behind the army's
//! checks, the side's titans stood at home for whole matches.
use super::ops_ground::OUTMATCHED;
use super::profile::Target;
use super::state::{Operation, Phase};
use super::Ctx;
use crate::command::Command;
use crate::World;
use mc_core::{Fx, FxVec2};

/// Per cent of their health the titans fall back below, when what they face also
/// outmatches them; and how much they come back up to before they go again.
const FALL_BACK_AT: i32 = 30;
const READY_AT: i32 = 60;
/// Ticks a fallen-back titan waits to mend before it goes all the same (three minutes).
const MEND_TICKS: u32 = 1800;

impl World {
    /// One think of the titans' operation.
    pub(in crate::ai) fn run_titan(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        out: &mut Vec<Command>,
    ) -> u32 {
        let rows = self.op_rows(op);
        let Some(centre) = self.centre_of(&rows) else {
            if op.phase != Phase::Gathering {
                op.phase = Phase::Done;
            }
            return 0;
        };
        let tick = self.state.tick;
        let materials: Fx = rows.iter().map(|&r| self.bp(r).cost_materials).sum();
        let health = {
            let (now, most) = rows.iter().fold((Fx::ZERO, Fx::ZERO), |(n, m), &r| {
                (n + self.state.units.health[r], m + self.bp(r).health)
            });
            (now * 100 / most.max(Fx::ONE)).floor_int()
        };
        let ids = self.ids_of(&rows);
        // What it faces where it stands, and what of it stands there.
        let facing = self.enemy_strength_at(ctx, centre, Fx::from_int(900), Target::Land);
        match op.phase {
            Phase::Gathering => {
                // Mended, or waited long enough: out again. There is nothing to gather.
                if health < READY_AT && tick < op.phase_since + MEND_TICKS {
                    return 0;
                }
                self.titan_go(ctx, op, &rows, centre, materials, out)
            }
            Phase::Executing => {
                // Badly hurt and outmatched: home, to mend. Anything less it fights out.
                if health < FALL_BACK_AT && facing > materials * OUTMATCHED {
                    op.phase = Phase::Withdrawing;
                    op.phase_since = tick;
                    out.push(Command::AttackMove {
                        units: ids,
                        target: ctx.start,
                        queue: false,
                    });
                    return 1;
                }
                // An enemy army in reach that it beats: that first, wherever it was
                // going.
                if facing > Fx::ZERO {
                    if let Some(prey) = self
                        .prey_near(ctx, centre, materials)
                        .filter(|p| p.distance(op.target) > Fx::from_int(200))
                    {
                        op.target = prey;
                        op.phase_since = tick;
                        out.push(Command::AttackMove {
                            units: ids,
                            target: prey,
                            queue: false,
                        });
                        return 1;
                    }
                }
                // Something worth more found nearer on the way: that first. Sent at
                // their start knowing nothing else, it walked past a guarded factory.
                if let Some(t) = self
                    .land_objective(ctx, centre, materials, false)
                    .filter(|t| {
                        t.distance(op.target) > Fx::from_int(600)
                            && t.distance(centre) < op.target.distance(centre)
                    })
                {
                    op.target = t;
                    op.phase_since = tick;
                    out.push(Command::AttackMove {
                        units: ids,
                        target: t,
                        queue: false,
                    });
                    return 1;
                }
                let there = centre.distance(op.target) < Fx::from_int(600) && facing == Fx::ZERO;
                if !(there || self.mostly_idle(&rows, &ctx.arrived)) {
                    return 0;
                }
                self.titan_go(ctx, op, &rows, centre, materials, out)
            }
            Phase::Withdrawing => {
                if centre.distance(ctx.start) < Fx::from_int(700)
                    || (self.mostly_idle(&rows, &ctx.arrived) && tick > op.phase_since + 300)
                {
                    op.phase = Phase::Gathering;
                    op.phase_since = tick;
                }
                0
            }
            Phase::Done => 0,
        }
    }

    /// Sends the titans at their next target: an enemy army nearby they beat, else what
    /// of the enemy's they can win at (`land_objective`); nothing left that it knows
    /// of, their start. On an attack-move, so they fight whatever they meet on the way.
    fn titan_go(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        rows: &[usize],
        centre: FxVec2,
        materials: Fx,
        out: &mut Vec<Command>,
    ) -> u32 {
        let Some(target) = self
            .prey_near(ctx, centre, materials)
            .or_else(|| self.land_objective(ctx, centre, materials, false))
        else {
            return 0;
        };
        op.target = target;
        op.launched = materials;
        op.phase = Phase::Executing;
        op.phase_since = self.state.tick;
        out.push(Command::AttackMove {
            units: self.ids_of(rows),
            target,
            queue: false,
        });
        1
    }
}
