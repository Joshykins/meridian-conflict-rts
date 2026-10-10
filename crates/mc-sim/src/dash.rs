//! The dash (`UnitBlueprint::dash`, the Regency Seraph): a unit that sees enemies on the
//! ground or the water some way off charges where it stands, is gone for a moment, and
//! comes down in the middle of them, and its landing lets go a wave that rolls out over
//! everything round it.
//!
//! - **By hand** (`Command::Dash`, `OrderKind::Dash`): the order waits for the dash to be
//!   ready, then it charges for the point itself, as far toward it as it reaches, on the
//!   nearest ground it can stand on: no cluster is gathered. A point nearer than
//!   `Dash::min` is refused (`Refusal::DashTooClose`). It is the player's order, so it
//!   goes at hold fire and hold position too. The order is done as it goes.
//! - **When**: its dash is ready (`Dash::cooldown_ticks` after its last landing), it is
//!   free to pick its fights (idle, chasing on its own, on an attack-move or patrol, or
//!   under an attack order; fire at will; not stunned), and an enemy it sees on the ground
//!   or the water lies between `Dash::min` and `Dash::range` of it. Idle, the enemy is
//!   the nearest one it sees, and none may be nearer than `min`: one at its feet means it
//!   is in the fight already. Under an attack order it is the order's mark. A move order,
//!   hold fire or hold position never dash: the player keeps the unit where it was put.
//!   Looked at twice a second.
//! - **Where**: the middle of the enemies within `Dash::gather` of the one it dashes at,
//!   each counted once whatever its size (so it comes down among them, not on the one
//!   biggest hull), then put clear of any big hull there (one at least half its own
//!   radius: it never lands inside a giant) and onto the nearest ground its layer can
//!   stand on (`Nav::nearest_passable`: never a cliff, nor a structure's hull).
//! - **Windup**: it stops and turns to the mark for `Dash::windup_ticks`, charging
//!   (`warp_held` keeps it out of movement). A stun, or anything that takes away its
//!   freedom (a move order, hold fire), calls the dash off at no cost.
//! - **Transit**: its charge spent, it is out of the world (`IN_FACTORY`: not drawn, seen,
//!   hit or ordered) for `Dash::transit_ticks`, already at the mark (`prev_pos` too, so
//!   nothing is drawn sliding across the map). Nothing calls it off now.
//!   `Dash::arrive_ticks` before it lands, the air where it comes down begins to crackle
//!   (`DashArriving`, a warning drawn and heard only).
//! - **Landing**: back in the world at the mark. Its `dash_only` weapons, cones, go off at
//!   its feet: each a wake rolling out from where it stands (`wake.rs`), striking the
//!   enemies its front passes over and stunning them if the cone says so; friends are not
//!   touched. Its `on_dash` weapons fire a whole salvo at once, however far through their
//!   reload they were. A chase it began on its own ends there: where it lands is its new
//!   post.

use mc_core::{Fx, FxVec2};
use mc_data::{cat, dash::Dash, MoveLayer};
use serde::{Deserialize, Serialize};

use crate::mirror::{Refusal, SimEvent};
use crate::spatial::kind;
use crate::tables::{flag, FireState, Order, OrderKind, UnitId};
use crate::Handle;
use crate::{SimError, World};

/// What a dash may strike and be drawn to: anything on the ground or the water.
const GROUND: u32 = cat::LAND | cat::NAVAL | cat::STRUCTURE;
/// Ticks between looks for something to dash at, spread over the rows.
const LOOK_EVERY: usize = 5;

/// Where a unit with a dash is in it (`Units::dash`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DashPhase {
    /// Recharging (`DashState::cooldown` above zero), or ready and looking.
    #[default]
    Ready,
    /// Winding up where it stands, bound for `DashState::to`.
    Windup,
    /// Out of the world, already at `DashState::to`, until it lands.
    Transit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashState {
    pub phase: DashPhase,
    /// Ticks into the windup, or the transit.
    pub ticks: u16,
    /// Ticks until it may dash again.
    pub cooldown: u16,
    /// Where the windup began, and where it comes down.
    pub from: FxVec2,
    pub to: FxVec2,
}

impl DashState {
    pub(crate) fn hash(&self, h: &mut mc_core::StateHasher) {
        h.write_u64(self.phase as u64 | (self.ticks as u64) << 8 | (self.cooldown as u64) << 24);
        for v in [self.from.x, self.from.y, self.to.x, self.to.y] {
            h.write_i64(v.0);
        }
    }
}

/// Why it may dash: on its own, or at the mark of an attack order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leave {
    /// Idle, chasing on its own (`chase`), or on an attack-move or patrol.
    Free { chase: bool },
    /// An attack order on this row.
    On(usize),
    /// A dash by hand (`OrderKind::Dash`): to the point it was given.
    Ordered,
}

impl World {
    /// Every tick, after orders: windups under way, recharges, and looks for a dash.
    pub(crate) fn run_dashes(&mut self) -> Result<(), SimError> {
        let rows: Vec<usize> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| self.bp(r).dash.is_some())
            .collect();
        for row in rows {
            let Some(dash) = self.bp(row).dash else {
                continue;
            };
            if !self.state.units.slots.is_alive(row) {
                continue;
            }
            let s = self.state.units.dash[row];
            match s.phase {
                DashPhase::Ready if s.cooldown > 0 => {
                    self.state.units.dash[row].cooldown -= 1;
                }
                DashPhase::Ready => {
                    if (self.state.tick as usize + row).is_multiple_of(LOOK_EVERY) {
                        self.look_for_dash(row, &dash);
                    }
                }
                DashPhase::Windup => self.step_windup(row, &dash, s)?,
                DashPhase::Transit => self.step_dash_transit(row, &dash, s)?,
            }
        }
        Ok(())
    }

    /// Winding up a dash, or gone in one: held out of movement (`warp_held`).
    pub(crate) fn dash_held(&self, row: usize) -> bool {
        self.state.units.dash[row].phase != DashPhase::Ready
    }

    /// Whether `row` may dash, and why.
    fn dash_leave(&self, row: usize) -> Option<Leave> {
        let units = &self.state.units;
        if !units.is_active(row) || self.stunned(row) || units.hangar[row] != crate::Handle::NONE {
            return None;
        }
        let front = self.state.orders.front(units, row);
        // The player's own dash goes whatever the stance.
        if front.is_some_and(|o| o.kind == OrderKind::Dash) {
            return Some(Leave::Ordered);
        }
        if units.has_flag(row, flag::PASSIVE) || units.fire_state[row] != FireState::FireAtWill {
            return None;
        }
        let Some(o) = front else {
            return Some(Leave::Free { chase: false });
        };
        match o.kind {
            // A chase it began itself (`idle_chase`) carries its leash in `radius`.
            OrderKind::Attack if o.radius > Fx::ZERO => Some(Leave::Free { chase: true }),
            OrderKind::Attack => units.row(o.target).map(Leave::On),
            OrderKind::AttackMove | OrderKind::Patrol => Some(Leave::Free { chase: false }),
            _ => None,
        }
    }

    /// An enemy of `owner` on the ground or the water that it sees: one a dash may go for.
    fn dash_foe(&self, owner: u8, t: usize) -> bool {
        let units = &self.state.units;
        units.slots.is_alive(t)
            && self.are_enemies(owner, units.owner[t])
            && !units.has_flag(t, flag::IN_FACTORY)
            && self.target_layers(t) & GROUND != 0
            && self.target_layers(t) & cat::AIR == 0
            && !self.submerged(t)
            && self.detects(owner, t)
    }

    fn look_for_dash(&mut self, row: usize, dash: &Dash) {
        let Some(leave) = self.dash_leave(row).filter(|&l| l != Leave::Ordered) else {
            return;
        };
        let Some(to) = self.dash_mark(row, dash, leave) else {
            return;
        };
        self.begin_windup(row, dash, to);
    }

    /// `Command::Dash`: an order to each unit among `ids` with a dash, refused once for a
    /// point nearer than its least reach.
    pub(crate) fn order_dash(
        &mut self,
        player: u8,
        ids: &[UnitId],
        pos: FxVec2,
        queue: bool,
    ) -> Result<(), SimError> {
        let pos = self.clamp_to_map(pos);
        let rows: Vec<usize> = self
            .owned(player, ids, 0)
            .into_iter()
            .filter(|&r| self.bp(r).dash.is_some())
            .collect();
        let mut refused = false;
        for row in rows {
            let Some(dash) = self.bp(row).dash else {
                continue;
            };
            if !queue && self.state.units.pos[row].distance(pos) < dash.min {
                refused = true;
                continue;
            }
            self.give(
                row,
                crate::orders::order(OrderKind::Dash, pos, Handle::NONE),
                queue,
            )?;
        }
        if refused {
            self.refuse(player, Refusal::DashTooClose);
        }
        Ok(())
    }

    /// `OrderKind::Dash` at the front of the queue: waits (holding still) for the dash to
    /// be ready, then charges for the point, as far toward it as it reaches.
    pub(crate) fn run_dash_order(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let Some(dash) = self.bp(row).dash else {
            self.finish_order(row);
            return Ok(());
        };
        let s = self.state.units.dash[row];
        if s.phase != DashPhase::Ready {
            return Ok(());
        }
        self.stop_moving(row);
        if s.cooldown > 0 || self.stunned(row) || !self.state.units.is_active(row) {
            return Ok(());
        }
        let from = self.state.units.pos[row];
        let gap = from.distance(o.pos);
        let aim = if gap > dash.range {
            from + (o.pos - from).normalize() * dash.range
        } else {
            o.pos
        };
        match self.dash_ground(row, aim) {
            Some(to) if to.distance(from) >= dash.min => self.begin_windup(row, &dash, to),
            // Queued from farther off, it ended up too close: nothing to dash to.
            _ => self.finish_order(row),
        }
        Ok(())
    }

    /// It stops and begins its charge for `to`.
    fn begin_windup(&mut self, row: usize, dash: &Dash, to: FxVec2) {
        let from = self.state.units.pos[row];
        self.stop_moving(row);
        let units = &mut self.state.units;
        units.speed[row] = Fx::ZERO;
        units.dash[row] = DashState {
            phase: DashPhase::Windup,
            ticks: 0,
            cooldown: 0,
            from,
            to,
        };
        self.events.push(SimEvent::DashCharging {
            unit: units.id(row),
            from: from.extend(units.z[row]),
            to,
            ticks: dash.windup_ticks,
            blueprint: units.blueprint[row],
            owner: units.owner[row],
        });
    }

    /// Where `row` would come down, if it has something to dash at.
    fn dash_mark(&self, row: usize, dash: &Dash, leave: Leave) -> Option<FxVec2> {
        let units = &self.state.units;
        let (pos, owner) = (units.pos[row], units.owner[row]);
        let target = match leave {
            // A dash by hand goes to its point (`run_dash_order`), never a cluster.
            Leave::Ordered => return None,
            Leave::On(t) => {
                let gap = units.pos[t].distance(pos);
                (self.dash_foe(owner, t) && gap >= dash.min && gap <= dash.range).then_some(t)?
            }
            Leave::Free { .. } => {
                let t = self
                    .index
                    .nearest_foe(pos, dash.range, kind::UNIT, self.team_mask(owner), |e| {
                        self.unit_entry_is_current(e) && self.dash_foe(owner, e.row as usize)
                    })?
                    .row as usize;
                (units.pos[t].distance(pos) >= dash.min).then_some(t)?
            }
        };
        let centre = units.pos[target];
        let mut cluster = Vec::new();
        self.index.query_foes(
            centre,
            dash.gather,
            kind::UNIT,
            self.team_mask(owner),
            |e| {
                let t = e.row as usize;
                if self.unit_entry_is_current(e)
                    && e.pos.distance(centre) <= dash.gather
                    && self.dash_foe(owner, t)
                {
                    cluster.push(t);
                }
                true
            },
        );
        cluster.sort_unstable();
        cluster.dedup();
        if !cluster.contains(&target) {
            cluster.push(target);
        }
        // Their middle, summed about the mark so the sum stays small.
        let sum = cluster
            .iter()
            .fold(FxVec2::ZERO, |sum, &t| sum + (units.pos[t] - centre));
        let n = cluster.len() as i32;
        let mut to = centre + FxVec2::new(sum.x / n, sum.y / n);
        // Clear of the big hulls there: it lands beside a giant, not inside it.
        let own = self.bp(row).radius;
        for &t in &cluster {
            let r = self.bp(t).radius;
            if r * 2 < own {
                continue;
            }
            let clear = r + own;
            let at = units.pos[t];
            if to.distance(at) >= clear {
                continue;
            }
            let away = if to == at { pos - at } else { to - at };
            if away == FxVec2::ZERO {
                continue;
            }
            to = at + away.normalize() * clear;
        }
        let to = self.dash_ground(row, to)?;
        (to.distance(pos) >= dash.min).then_some(to)
    }

    /// The nearest point to `at` on the map that `row`'s hull can stand on.
    fn dash_ground(&self, row: usize, at: FxVec2) -> Option<FxVec2> {
        let m = self.bp(row).motion?;
        if m.layer == MoveLayer::Air {
            return None;
        }
        let at = self.clamp_to_map(at);
        self.nav
            .nearest_passable(m.layer, m.size_class, at)
            .map(|p| self.clamp_to_map(p))
    }

    fn step_windup(&mut self, row: usize, dash: &Dash, s: DashState) -> Result<(), SimError> {
        let leave = self.dash_leave(row);
        let motion = self.bp(row).motion;
        let (Some(leave), Some(motion)) = (leave, motion) else {
            self.state.units.dash[row] = DashState::default();
            self.events.push(SimEvent::DashCancelled {
                unit: self.state.units.id(row),
            });
            return Ok(());
        };
        let units = &mut self.state.units;
        let bearing = (s.to - units.pos[row]).angle();
        units.heading[row] = units.heading[row].turn_toward(bearing, motion.turn_rate);
        units.speed[row] = Fx::ZERO;
        units.flags[row] |= flag::HOLD;
        let ticks = s.ticks + 1;
        units.dash[row].ticks = ticks;
        if ticks < dash.windup_ticks {
            return Ok(());
        }
        // Something may have been built on the mark since it wound up.
        let to = self
            .dash_ground(row, s.to)
            .unwrap_or(self.state.units.pos[row]);
        match leave {
            // Its own chase ends where it lands, and the walk home behind it with it.
            Leave::Free { chase: true } => self.clear_orders(row)?,
            // The player's dash is done as it goes: whatever comes next waits for it to
            // land (`dash_held`).
            Leave::Ordered => self.finish_order(row),
            _ => {}
        }
        self.vanish(row, dash, to);
        Ok(())
    }

    /// Its charge spent: out of the world at once, already at `to`, until it lands.
    fn vanish(&mut self, row: usize, dash: &Dash, to: FxVec2) {
        self.stop_moving(row);
        let z = self.stand_z(row, to);
        let units = &mut self.state.units;
        let from = units.pos[row].extend(units.z[row]);
        units.flags[row] |= flag::IN_FACTORY;
        units.pos[row] = to;
        units.prev_pos[row] = to;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.move_goal[row] = to;
        units.speed[row] = Fx::ZERO;
        units.dash[row] = DashState {
            phase: DashPhase::Transit,
            ticks: 0,
            cooldown: 0,
            from: from.xy(),
            to,
        };
        self.events.push(SimEvent::DashVanished {
            unit: units.id(row),
            from,
            to: to.extend(z),
            ticks: dash.transit_ticks,
            blueprint: units.blueprint[row],
            owner: units.owner[row],
        });
    }

    fn step_dash_transit(&mut self, row: usize, dash: &Dash, s: DashState) -> Result<(), SimError> {
        let ticks = s.ticks + 1;
        self.state.units.dash[row].ticks = ticks;
        if dash.arrive_ticks > 0 && ticks + dash.arrive_ticks == dash.transit_ticks {
            let units = &self.state.units;
            self.events.push(SimEvent::DashArriving {
                unit: units.id(row),
                to: s.to.extend(units.z[row]),
                ticks: dash.arrive_ticks,
                blueprint: units.blueprint[row],
                owner: units.owner[row],
            });
        }
        if ticks < dash.transit_ticks {
            return Ok(());
        }
        self.land_dash(row, dash, s)
    }

    /// Back in the world where it came down: its `dash_only` weapons go off at its feet,
    /// and its `on_dash` ones fire whatever their reload.
    fn land_dash(&mut self, row: usize, dash: &Dash, s: DashState) -> Result<(), SimError> {
        let to = s.to;
        let units = &mut self.state.units;
        units.flags[row] &= !flag::IN_FACTORY;
        units.flags[row] |= flag::MOVING;
        units.dash[row] = DashState {
            phase: DashPhase::Ready,
            ticks: 0,
            cooldown: dash.cooldown_ticks,
            ..s
        };
        let (unit, owner, blueprint) = (units.id(row), units.owner[row], units.blueprint[row]);
        let (z, facing) = (units.z[row], units.heading[row]);
        let from = s.from.extend(z);
        self.events.push(SimEvent::DashLanded {
            unit,
            from,
            to: to.extend(z),
            blueprint,
            owner,
        });
        let feet = to.extend(self.ground_z(row, to) + Fx::ONE);
        let blueprints = std::sync::Arc::clone(&self.blueprints);
        for (w, weapon) in blueprints.unit(blueprint).weapons.iter().enumerate() {
            if weapon.dash_only && weapon.cone.is_some() {
                // The wave, let go at its feet the way it faces; a full circle rolls out all
                // round whatever the facing.
                self.wake(row, w, feet, facing);
                self.events.push(SimEvent::ShotFired {
                    pos: feet,
                    vel: FxVec2::from_angle(facing).extend(Fx::ZERO),
                    travel: mc_core::FxVec3::ZERO,
                    color: weapon.color,
                    owner,
                    blueprint,
                    weapon: w as u8,
                });
            }
            if weapon.on_dash {
                // The volley that goes up as it lands, whatever its reload.
                let units = &mut self.state.units;
                units.weapon_cooldown[row][w] = 0;
                units.weapon_salvo_left[row][w] = 0;
            }
        }
        Ok(())
    }
}
