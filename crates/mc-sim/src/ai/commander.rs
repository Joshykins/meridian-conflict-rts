//! The Commander: the planning AI (`docs/AI_COMMANDER.md`). It holds game plans at a
//! stake, runs operations of grouped units, and reads the roster from its data.
//! Chosen per side by `AiConfig::brain`.
//!
//! One think: profiles from the data, the world model and beliefs from what the
//! side has seen, plans reviewed, operations kept and given units, then each
//! operation's orders as far as the side's attention goes.
mod beliefs;
mod matchup;
mod ops;
mod ops_air_sea;
mod ops_ground;
mod ops_space;
mod plans;
mod profile;
mod reach;
mod solver;
pub(super) mod state;
mod world_model;

use super::{offset_toward, Census, Intel};
use crate::command::Command;
use crate::tables::UnitId;
use crate::{Brain, Difficulty, World};
use beliefs::Beliefs;
use mc_core::{Fx, FxVec2};
use profile::Profiles;
use state::{OpKind, Operation, Phase};
use world_model::WorldModel;

/// Kills waiting to be credited are dropped past this many: a think comes long before.
const MOST_KILLS: usize = 512;

/// What every operation reads in one think.
pub(in crate::ai) struct Ctx<'a> {
    pub player: u8,
    pub start: FxVec2,
    pub enemy_start: Option<FxVec2>,
    /// Where the land army gathers.
    pub staging: FxVec2,
    pub profiles: &'a Profiles,
    pub wm: &'a WorldModel,
    pub beliefs: &'a Beliefs,
    pub intel: &'a Intel,
    /// Land units and ships standing where they were sent (`arrival.rs`), sorted.
    pub arrived: Vec<usize>,
    /// The land army can walk to an enemy still in the game.
    pub land_route: bool,
    /// Ground walkable from home (`reach.rs`).
    pub reach: &'a reach::Reach,
}

impl Ctx<'_> {
    /// Whether the land army can walk to `p` from home.
    pub(in crate::ai) fn can_walk(&self, p: FxVec2) -> bool {
        self.reach.reaches(p)
    }
}

/// Orders a side may give a minute, by difficulty: its attention.
fn orders_a_minute(d: Difficulty) -> i32 {
    match d {
        Difficulty::Easy => 24,
        Difficulty::Normal => 50,
        Difficulty::Hard => 100,
    }
}

impl World {
    /// A unit of `player` killed `mass` of the enemy: credited to its operation at the
    /// side's next think.
    pub(crate) fn note_kill_for_ai(&mut self, player: u8, killer: UnitId, mass: Fx) {
        let Some(ai) = self.state.ai.get_mut(player as usize) else {
            return;
        };
        // A deliberate cap: the oldest kills are lost if a side somehow never thinks.
        if ai.config.brain == Brain::Commander && ai.commander.kills.len() < MOST_KILLS {
            ai.commander.kills.push((killer, mass));
        }
    }

    /// One think of the Commander for `player`: everything but the builders, which
    /// it steers (`plans.rs`, `solver.rs`) and the classic code places.
    pub(super) fn command(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        out: &mut Vec<Command>,
    ) {
        let blueprints = self.blueprints.clone();
        let profiles = Profiles::build(&blueprints);
        let reach = self.home_reach(player);
        let wm = self.world_model(player, &profiles);
        let beliefs = self.beliefs(player, &profiles);
        self.ops_upkeep(player);
        let start = self.state.players[player as usize].start;
        let enemy_start = intel.enemy_start;
        let staging = {
            let want = offset_toward(start, enemy_start.unwrap_or(start), Fx::from_int(260));
            self.home_ground(start, 0)
                .map_or(want, |g| self.reachable_staging(start, want, &g))
        };
        let mut arrived = self.arrived_army(player);
        arrived.sort_unstable();
        let ctx = Ctx {
            player,
            start,
            enemy_start,
            staging,
            profiles: &profiles,
            wm: &wm,
            beliefs: &beliefs,
            intel,
            arrived,
            land_route: census.land_route,
            reach: &reach,
        };
        self.review_plans(&ctx, census);
        self.keep_ops(&ctx);
        let free = self.free_units(player, &profiles);
        self.assign(player, &profiles, &free, census.land_route);
        self.raise_defence(&ctx, &intel.threats);
        self.load_landings(&ctx, out);
        self.run_ops(&ctx, out);
        self.direct_strategic(&ctx, out);
    }

    /// Each operation's orders, the most urgent first, while the side's attention
    /// lasts. The rest keep their last orders until a later think.
    fn run_ops(&mut self, ctx: &Ctx, out: &mut Vec<Command>) {
        let player = ctx.player as usize;
        let config = self.state.ai[player].config;
        let per_think =
            Fx::from_int(orders_a_minute(config.difficulty)) * config.think_period() as i32 / 600;
        let c = &mut self.state.ai[player].commander;
        c.attention = (c.attention + per_think).min(per_think * 3 + Fx::from_int(2));
        let mut ops = std::mem::take(&mut c.ops);
        // Urgency: defence, then anything going in, then the rest (a stable sort).
        ops.sort_by_key(|o| match (o.kind, o.phase) {
            (OpKind::Defend, _) => 0,
            (_, Phase::Executing) | (_, Phase::Withdrawing) => 1,
            _ => 2,
        });
        let escort = ops
            .iter()
            .find(|o| o.kind == OpKind::Strike && o.phase == Phase::Executing)
            .map(|o| o.target);
        let army: Option<FxVec2> = ops
            .iter()
            .find(|o| o.kind == OpKind::Army && o.phase == Phase::Executing)
            .and_then(|o| self.centre_of(&self.op_rows(o)));
        for op in &mut ops {
            if self.state.ai[player].commander.attention < Fx::ONE {
                break;
            }
            let given = self.run_op(ctx, op, escort, army, out);
            self.state.ai[player].commander.attention -= Fx::from_int(given as i32);
            if given > 0 {
                op.ordered = self.state.tick;
            }
        }
        let c = &mut self.state.ai[player].commander;
        let opened = std::mem::take(&mut c.ops);
        ops.extend(opened);
        ops.sort_by_key(|o| o.id);
        c.ops = ops;
    }

    fn run_op(
        &mut self,
        ctx: &Ctx,
        op: &mut Operation,
        escort: Option<FxVec2>,
        army: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) -> u32 {
        match op.kind {
            OpKind::Army | OpKind::Defend | OpKind::Raid | OpKind::Siege => {
                self.run_land_op(ctx, op, out)
            }
            OpKind::Landing => self.run_landing(ctx, op, out),
            OpKind::Strike => self.run_strike(ctx, op, out),
            OpKind::AirGuard => self.run_air_guard(ctx, op, escort, army, out),
            OpKind::Fleet | OpKind::Wolfpack => self.run_fleet(ctx, op, out),
            OpKind::Warships => self.run_warships(ctx, op, out),
            OpKind::Scout => self.run_scouts(ctx, op, out),
        }
    }

    /// Landings with a ship at home and nothing aboard take their cargo from the
    /// army gathering at staging. Done before the operations run, while every
    /// operation is in place to be taken from.
    fn load_landings(&mut self, ctx: &Ctx, out: &mut Vec<Command>) {
        let player = ctx.player as usize;
        let ids: Vec<u32> = self.state.ai[player]
            .commander
            .ops
            .iter()
            .filter(|o| {
                o.kind == OpKind::Landing && o.phase == Phase::Gathering && o.units.is_empty()
            })
            .map(|o| o.id)
            .collect();
        for id in ids {
            let Some(op) = self.state.ai[player]
                .commander
                .ops
                .iter()
                .find(|o| o.id == id)
                .cloned()
            else {
                continue;
            };
            let Some(ship) = self.state.units.row(op.carrier) else {
                continue;
            };
            let units = &self.state.units;
            if units.order_head[ship] != crate::tables::NO_ORDER
                || units.warp[ship].phase != crate::tables::WarpPhase::Idle
                || units.pos[ship].distance(ctx.start) > Fx::from_int(2500)
            {
                continue;
            }
            let Some(t) = self.bp(ship).transport else {
                continue;
            };
            let cargo = self.take_cargo(ctx, ship, t.capacity);
            let room: u16 = cargo.iter().map(|(_, _, r)| *r).sum();
            let enough = (t.capacity * 2 / 3)
                .min(op.want.floor_int().max(2) as u16)
                .max(2);
            if cargo.len() < 2 || room < enough {
                continue;
            }
            let ids: Vec<UnitId> = cargo.iter().map(|(u, _, _)| *u).collect();
            let tick = self.state.tick;
            let c = &mut self.state.ai[player].commander;
            for o in &mut c.ops {
                o.units.retain(|(u, _)| !ids.contains(u));
            }
            if let Some(o) = c.ops.iter_mut().find(|o| o.id == id) {
                o.units = cargo.iter().map(|(u, m, _)| (*u, *m)).collect();
                o.ledger.committed += o.mass();
                o.phase_since = tick;
            }
            out.push(Command::Board {
                units: ids,
                carrier: op.carrier,
                queue: false,
            });
        }
    }
}

#[cfg(test)]
#[path = "commander/tests.rs"]
mod tests;
