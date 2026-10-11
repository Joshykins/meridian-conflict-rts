//! The Commander: the planning AI (`docs/AI_COMMANDER.md`). It holds game plans at a
//! stake, runs operations of grouped units, and reads the roster from its data.
//!
//! One think: profiles from the data, the world model and beliefs from what the
//! side has seen, plans reviewed, operations kept and given units, then each
//! operation's orders as far as the side's attention goes.
mod beliefs;
pub(in crate::ai) mod economy;
pub(in crate::ai) mod fronts;
mod king;
pub(in crate::ai) mod knowledge;
mod matchup;
pub(in crate::ai) mod mind;
pub(in crate::ai) mod mind_zones;
mod ops;
mod ops_air_sea;
mod ops_ground;
mod ops_space;
mod ops_titan;
mod plans;
pub(in crate::ai) mod profile;
mod reach;
mod scratch;
mod solver;
pub(super) mod state;
mod world_model;
pub(in crate::ai) mod zones;

use super::{offset_toward, Census, Intel};
use crate::command::Command;
use crate::tables::Controller;
use crate::tables::UnitId;
use crate::{Difficulty, World};
use beliefs::Beliefs;
use mc_core::{Fx, FxVec2};
use profile::Profiles;
use state::{OpKind, Operation, Phase};
use world_model::WorldModel;
use zones::{ZoneView, Zones};

/// Kills waiting to be credited are dropped past this many: a think comes long before.
const MOST_KILLS: usize = 512;
/// A side thinks one period slower for every this many units it owns...
const UNITS_PER_SLOWDOWN: usize = 800;
/// ...and four times slower at the most.
const MOST_SLOWDOWN: usize = 4;

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
    /// The ground split into zones, and what the side believes of each (`zones.rs`).
    pub zones: &'a Zones,
    pub zv: &'a ZoneView,
    /// A finished space elevator of the side's stands (`role::ELEVATOR`): its units
    /// come down from orbit free, so its land army is cheap to spend.
    pub cheap_army: bool,
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
    /// A unit of `player` killed `materials` of the enemy: credited to its operation at the
    /// side's next think.
    pub(crate) fn note_kill_for_ai(&mut self, player: u8, killer: UnitId, materials: Fx) {
        if self
            .state
            .players
            .get(player as usize)
            .map(|p| p.controller)
            != Some(Controller::Ai)
        {
            return;
        }
        let Some(ai) = self.state.ai.get_mut(player as usize) else {
            return;
        };
        // A deliberate cap: the oldest kills are lost if a side somehow never thinks.
        if ai.commander.kills.len() < MOST_KILLS {
            ai.commander.kills.push((killer, materials));
        }
        ai.commander.killed += materials;
    }

    /// A unit of `player` of blueprint `lost`, worth `materials`, was killed by a unit of
    /// blueprint `by`: what hurts the side, by the killer's kind (`state::Hurt`),
    /// steers production; a mine or plant lost to a ground raid calls for a turret
    /// there (`plans.rs`).
    pub(crate) fn note_loss_for_ai(
        &mut self,
        player: u8,
        by: mc_data::BlueprintId,
        lost: mc_data::BlueprintId,
        materials: Fx,
        at: FxVec2,
    ) {
        let kind = state::Hurt::of(self.blueprints.unit(by));
        let economy = {
            let bp = self.blueprints.unit(lost);
            bp.is_structure() && (bp.mine.is_some() || bp.has(mc_data::cat::POWER))
        };
        let tick = self.state.tick;
        if self
            .state
            .players
            .get(player as usize)
            .map(|p| p.controller)
            != Some(Controller::Ai)
        {
            return;
        }
        let Some(ai) = self.state.ai.get_mut(player as usize) else {
            return;
        };
        {
            ai.commander.hurt[kind as usize] += materials;
            // Where the bombers keep hitting: anti-air goes there (`plans.rs`).
            if matches!(kind, state::Hurt::Air | state::Hurt::Space) {
                ai.commander.hit_from_above = Some((at, tick));
            }
            if economy && matches!(kind, state::Hurt::Land | state::Hurt::Artillery) {
                ai.commander.raided = Some((at, tick));
            }
        }
    }

    /// One think of the Commander for `player`: everything but the builders, which
    /// it steers (`plans.rs`, `solver.rs`) and `builders.rs` places.
    pub(super) fn command(
        &mut self,
        player: u8,
        census: &Census,
        intel: &Intel,
        out: &mut Vec<Command>,
    ) {
        let profiles = self.profiles();
        let reach = self.home_reach(player);
        let zones = self.zones();
        // Past a few hundred units a side thinks less often (a deliberate cap on its
        // cost, kept in the state so a restored match thinks on the same ticks).
        let owned = {
            let units = &self.state.units;
            units
                .slots
                .iter()
                .filter(|&r| units.owner[r] == player)
                .count()
        };
        self.state.ai[player as usize].commander.think_scale =
            (1 + owned / UNITS_PER_SLOWDOWN).min(MOST_SLOWDOWN) as u8;
        let wm = self.world_model(player, &profiles);
        let zv = self.zone_view(player, &zones, &wm);
        let beliefs = self.beliefs(player, &profiles, &zones);
        let span = mc_core::perf_span!("ai.fronts");
        self.find_fronts(player, &zones, &zv, &profiles);
        drop(span);
        // Behind, for spending on the next tier: what it fields against the army it
        // believes in, and never less than two minutes of their income. Under fog
        // it believed in a 1.2k army that stood at 4.3k.
        // Its forces are what its operations hold: the world model counts the
        // commander too, worth more than any army at this point.
        let ours: Fx = self.state.ai[player as usize]
            .commander
            .ops
            .iter()
            .map(|o| o.materials())
            .sum();
        let theirs = beliefs
            .army_materials()
            .max(beliefs.income * Fx::from_int(120));
        self.state.ai[player as usize].commander.behind = ours < theirs;
        self.ops_upkeep(player);
        self.recall_commander(player, out);
        // What its defences found missing fades over a few minutes.
        for n in &mut self.state.ai[player as usize].commander.role_need {
            *n = *n * 127 / 128;
        }
        // What hurt it fades: a minute and a half's losses count most.
        for h in &mut self.state.ai[player as usize].commander.hurt {
            *h -= *h / 60;
        }
        let start = self.state.players[player as usize].start;
        let enemy_start = intel.enemy_start;
        // The rally point creeps back up to the front while the wave is not shelled.
        let c = &mut self.state.ai[player as usize].commander;
        c.rally_back = (c.rally_back - Fx::from_int(30)).max(Fx::ZERO);
        // The rally point holds still unless the front moves half a kilometre or
        // fire reaches it: moving it every think kept the wave walking after it.
        let front = self
            .front_rally(player, &zones, &reach, &wm)
            .unwrap_or_else(|| self.front_line(player, start, enemy_start, census, &reach, &wm));
        let staging = match self.state.ai[player as usize].commander.rally {
            Some(r)
                if r.distance(front) < Fx::from_int(500)
                    && wm.threat_at(r, profile::Target::Land) == Fx::ZERO
                    && reach.reaches(r) =>
            {
                r
            }
            _ => front,
        };
        self.state.ai[player as usize].commander.rally = Some(staging);
        let mut arrived = self.arrived_army(player);
        arrived.sort_unstable();
        let units = &self.state.units;
        let cheap_army = units.slots.iter().any(|r| {
            units.owner[r] == player
                && units.is_active(r)
                && profiles
                    .get(units.blueprint[r])
                    .has(profile::role::ELEVATOR)
        });
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
            zones: &zones,
            zv: &zv,
            cheap_army,
        };
        self.review_plans(&ctx, census);
        self.keep_ops(&ctx);
        let free = self.free_units(player, &profiles);
        self.assign(player, &profiles, &free, census.land_route);
        let mut incursions = self.incursions(&ctx);
        // The most valuable ground is answered first (a stable sort).
        incursions.sort_by_key(|&(at, ..)| {
            std::cmp::Reverse(
                ctx.zones
                    .of(at)
                    .map_or(0, |z| ctx.zv.value(ctx.zones, z).floor_int()),
            )
        });
        self.raise_defence(&ctx, &incursions);
        self.load_landings(&ctx, out);
        self.run_ops(&ctx, out);
        self.guard_commander(&ctx, out);
        self.direct_strategic(&ctx, out);
    }

    /// Where the land army gathers: on the front line, a little behind the furthest
    /// of the side's own mines toward the enemy (never more than 45% of the way),
    /// so a raid on them runs into it, but out of the enemy's known reach; by the
    /// base when nothing is out yet.
    fn front_line(
        &self,
        player: u8,
        start: FxVec2,
        enemy: Option<FxVec2>,
        census: &Census,
        reach: &reach::Reach,
        wm: &WorldModel,
    ) -> FxVec2 {
        let near = offset_toward(start, enemy.unwrap_or(start), Fx::from_int(260));
        let home = self
            .home_ground(start, 0)
            .map_or(near, |g| self.reachable_staging(start, near, &g));
        let Some(enemy) = enemy else {
            return home;
        };
        let span = enemy.distance(start).max(Fx::ONE);
        let dir = (enemy - start).normalize();
        let furthest = census
            .extractor_pos
            .iter()
            .map(|m| (*m - start).dot(dir))
            .filter(|t| *t > Fx::ZERO)
            .max()
            .unwrap_or(Fx::ZERO);
        let back = self.state.ai[player as usize].commander.rally_back;
        let t =
            (furthest - Fx::from_int(150) - back).clamp(Fx::from_int(260), span * Fx::ratio(9, 20));
        // Back toward home until the army can walk there and no enemy fire known
        // reaches it: units walking up one by one to a rally under the enemy's
        // artillery were picked off one every few seconds.
        let safe =
            |p: &FxVec2| reach.reaches(*p) && wm.threat_at(*p, profile::Target::Land) == Fx::ZERO;
        (0..12)
            .map(|k| start + dir * (t - Fx::from_int(200 * k)).max(Fx::from_int(260)))
            .find(safe)
            .unwrap_or(home)
    }

    /// Where the army gathers when the side has fronts: behind the land front with most
    /// at stake (what it holds there, and half what it could take), in our zone at its
    /// back, when the army can walk there and no enemy fire reaches it. Gathered on the
    /// line between the two starts, an army stood far from the border its mines were
    /// lost along.
    fn front_rally(
        &self,
        player: u8,
        zones: &Zones,
        reach: &reach::Reach,
        wm: &WorldModel,
    ) -> Option<FxVec2> {
        let c = &self.state.ai[player as usize].commander;
        c.scratch
            .fronts
            .iter()
            .filter(|f| !f.water)
            .filter_map(|f| {
                let at = zones.zones.get(f.rear? as usize)?.centre;
                (reach.reaches(at) && wm.threat_at(at, profile::Target::Land) == Fx::ZERO)
                    .then_some((f.stake_us + f.stake_them / 2, f.id, at))
            })
            .max_by_key(|&(stake, id, _)| (stake, std::cmp::Reverse(id)))
            .map(|(.., at)| at)
    }

    /// Each operation's orders, the most urgent first, while the side's attention
    /// lasts. The rest keep their last orders until a later think.
    fn run_ops(&mut self, ctx: &Ctx, out: &mut Vec<Command>) {
        let player = ctx.player as usize;
        let config = self.state.ai[player].config;
        let period =
            config.think_period() * self.state.ai[player].commander.think_scale.max(1) as u32;
        let per_think = Fx::from_int(orders_a_minute(config.difficulty)) * period as i32 / 600;
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
        // Fighters cover the army: the one on the move, else the one gathering.
        let army: Option<FxVec2> = ops
            .iter()
            .filter(|o| o.kind == OpKind::Army)
            .min_by_key(|o| o.phase != Phase::Executing)
            .and_then(|o| self.centre_of(&self.op_rows(o)));
        // The wave out in the field, which a siege may go out behind.
        let army_out: Option<FxVec2> = ops
            .iter()
            .find(|o| o.kind == OpKind::Army && o.phase == Phase::Executing)
            .and_then(|o| self.centre_of(&self.op_rows(o)));
        for op in &mut ops {
            if self.state.ai[player].commander.attention < Fx::ONE {
                break;
            }
            let given = self.run_op(ctx, op, escort, army, army_out, out);
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
        army_out: Option<FxVec2>,
        out: &mut Vec<Command>,
    ) -> u32 {
        match op.kind {
            OpKind::Army | OpKind::Defend | OpKind::Raid | OpKind::Siege | OpKind::Guard => {
                self.run_land_op(ctx, op, army_out, out)
            }
            OpKind::Landing => self.run_landing(ctx, op, out),
            OpKind::Strike => self.run_strike(ctx, op, out),
            OpKind::AirGuard => self.run_air_guard(ctx, op, escort, army, out),
            OpKind::Fleet | OpKind::Wolfpack => self.run_fleet(ctx, op, out),
            OpKind::Warships => self.run_warships(ctx, op, out),
            OpKind::Scout => self.run_scouts(ctx, op, out),
            OpKind::Titan => self.run_titan(ctx, op, out),
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
                o.ledger.committed += o.materials();
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
