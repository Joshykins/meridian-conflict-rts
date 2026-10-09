//! The dash (`UnitBlueprint::dash`, the Regency Seraph): a unit that sees enemies on the
//! ground or the water some way off winds up where it stands, is gone, and comes down in
//! the middle of them, and its landing goes off as a shockwave.
//!
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
//! - **Windup**: it stops and turns to the mark for `Dash::windup_ticks` (`warp_held`
//!   keeps it out of movement). A stun, or anything that takes away its freedom (a move
//!   order, hold fire), calls the dash off at no cost.
//! - **Landing**: on the windup's last tick it is at the mark: no transit, a teleport
//!   (`prev_pos` too, so nothing is drawn sliding across the map). The landing strikes every
//!   enemy unit and structure on the ground within `Dash::blast` (`Dash::falloff`), through
//!   the shields as any blast is, and stuns all but experimentals for `Dash::stun_ticks`;
//!   friends are not touched. Its `on_dash` weapons fire a whole salvo at once, however
//!   far through their reload they were. A chase it began on its own ends there: where it
//!   lands is its new post.

use mc_core::{Fx, FxVec2};
use mc_data::{cat, dash::Dash, MoveLayer};
use serde::{Deserialize, Serialize};

use crate::city::Blow;
use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::{flag, FireState, OrderKind, UnitId};
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashState {
    pub phase: DashPhase,
    /// Ticks into the windup.
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
            }
        }
        Ok(())
    }

    /// Winding up a dash: held out of movement (`warp_held`).
    pub(crate) fn dash_held(&self, row: usize) -> bool {
        self.state.units.dash[row].phase == DashPhase::Windup
    }

    /// Whether `row` may dash, and why.
    fn dash_leave(&self, row: usize) -> Option<Leave> {
        let units = &self.state.units;
        if !units.is_active(row)
            || self.stunned(row)
            || units.has_flag(row, flag::PASSIVE)
            || units.fire_state[row] != FireState::FireAtWill
            || units.hangar[row] != crate::Handle::NONE
        {
            return None;
        }
        let Some(o) = self.state.orders.front(units, row) else {
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
        let Some(leave) = self.dash_leave(row) else {
            return;
        };
        let Some(to) = self.dash_mark(row, dash, leave) else {
            return;
        };
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
        self.land_dash(row, dash, to, leave)
    }

    fn land_dash(
        &mut self,
        row: usize,
        dash: &Dash,
        to: FxVec2,
        leave: Leave,
    ) -> Result<(), SimError> {
        if leave == (Leave::Free { chase: true }) {
            // Its own chase ends where it lands, and the walk home behind it with it.
            self.clear_orders(row)?;
        }
        self.stop_moving(row);
        let z = self.stand_z(row, to);
        let units = &mut self.state.units;
        let from = units.pos[row].extend(units.z[row]);
        units.pos[row] = to;
        units.prev_pos[row] = to;
        units.z[row] = z;
        units.prev_z[row] = z;
        units.move_goal[row] = to;
        units.speed[row] = Fx::ZERO;
        units.flags[row] |= flag::MOVING;
        units.dash[row] = DashState {
            phase: DashPhase::Ready,
            ticks: 0,
            cooldown: dash.cooldown_ticks,
            from: from.xy(),
            to,
        };
        let (unit, owner, blueprint) = (units.id(row), units.owner[row], units.blueprint[row]);
        let at = to.extend(z);
        self.events.push(SimEvent::DashLanded {
            unit,
            from,
            to: at,
            blueprint,
            owner,
        });
        self.dash_shockwave(row, dash, to, unit, owner);
        self.events.push(SimEvent::DashShockwave {
            unit,
            pos: at,
            radius: dash.blast,
            blueprint,
            owner,
        });
        // The volley that goes up as it lands, whatever its reload.
        let units = &mut self.state.units;
        for (w, weapon) in self.blueprints.unit(blueprint).weapons.iter().enumerate() {
            if weapon.on_dash {
                units.weapon_cooldown[row][w] = 0;
                units.weapon_salvo_left[row][w] = 0;
            }
        }
        Ok(())
    }

    /// The landing's blow: every enemy on the ground within reach takes it, less toward
    /// the edge, and all but experimentals are stunned. Shields between take it instead.
    fn dash_shockwave(&mut self, row: usize, dash: &Dash, at: FxVec2, by: UnitId, owner: u8) {
        let mut struck = Vec::new();
        self.index
            .query_foes(at, dash.blast, kind::UNIT, self.team_mask(owner), |e| {
                let t = e.row as usize;
                if t != row
                    && self.unit_entry_is_current(e)
                    && self.are_enemies(owner, self.state.units.owner[t])
                    && !self.state.units.has_flag(t, flag::IN_FACTORY)
                    && self.target_layers(t) & GROUND != 0
                    && self.target_layers(t) & cat::AIR == 0
                    && !self.submerged(t)
                {
                    struck.push(t);
                }
                true
            });
        struck.sort_unstable();
        struck.dedup();
        let origin = at.extend(self.ground_z(row, at) + Fx::ONE);
        let mut charged = Vec::new();
        for t in struck {
            let bp = self.bp(t);
            let (radius, height, giant) = (bp.radius, bp.height, bp.has(cat::EXPERIMENTAL));
            let units = &self.state.units;
            let reach = (units.pos[t].distance(at) - radius).max(Fx::ZERO);
            if reach > dash.blast {
                continue;
            }
            let damage = dash.falloff(reach);
            let target = units.pos[t].extend(units.z[t] + height / 2);
            if let Some(shield) = self.blast_blocker(origin, target, Some(t)) {
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                continue;
            }
            self.damage_unit(t, damage, owner, by);
            if dash.stun_ticks > 0 && !giant && self.state.units.slots.is_alive(t) {
                self.stun(t, dash.stun_ticks);
            }
        }
        self.wear_wrecks(origin, dash.blast, |reach| dash.falloff(reach));
        self.blast_structures(origin, dash.blast, Blow::BLAST, |reach| dash.falloff(reach));
        let mut felled = Vec::new();
        self.prop_index
            .query(at, dash.blast * Fx::ratio(3, 4), kind::PROP, |e| {
                felled.push(e.row as usize);
                true
            });
        for prop in felled {
            if self.map.props[prop].kind.is_tree() {
                self.state.props_dead[prop / 64] |= 1 << (prop % 64);
            }
        }
        self.add_stain(at, dash.blast * Fx::ratio(3, 4), 140);
    }
}
