//! Reclaim: taking wrecks and live units apart for their mass.
//!
//! A wreck gives up its mass at the reclaimer's power. A live unit is unbuilt
//! at the same power it would take to build: it loses health as it goes, and
//! when the last of it is taken it is simply gone (`flag::RECLAIMED`). Reclaimers
//! with nothing to do clear the wrecks within their reach; a live unit is only
//! ever reclaimed on an order.

use crate::mirror::SimEvent;
use crate::orders::CHASE_REPATH_DISTANCE;
use crate::spatial::kind;
use crate::tables::*;
use crate::{SimError, World};
use bytemuck::{Pod, Zeroable};
use mc_core::{Fx, FxVec2, FxVec3, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;
/// Share of a live unit's mass that comes back. Less than a repair costs, so
/// mending a unit and taking it apart again never turns a profit.
const UNIT_YIELD: Fx = Fx::ratio(2, 5);
/// An armed builder leaves wrecks alone while an enemy is within this many times its guns' reach.
const GUN_RANGE_MARGIN: Fx = Fx::ratio(5, 4);
/// No wreck is wider than this: how far past its reach a reclaimer looks for one.
const WIDEST_TARGET: Fx = Fx::from_int(48);

/// One reclaimer working on one thing this tick.
#[derive(Clone, Copy, Debug)]
pub struct ReclaimWork {
    pub source: UnitId,
    /// The unit being taken apart, `Handle::NONE` for a wreck.
    pub unit: UnitId,
    /// Where the target stood when it was worked on, on the ground.
    pub at: FxVec3,
    pub radius: Fx,
    pub height: Fx,
}

/// `BeamInstance::kind` of a reclaim beam. One is kept for construction.
pub const BEAM_RECLAIM: u32 = 0;

/// A beam between a unit and its work, for the renderer. The far end glides
/// from `to_prev` to `to` over the tick, as the unit it is on does.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct BeamInstance {
    /// The emitter.
    pub from: [f32; 3],
    pub kind: u32,
    /// Foot of the target a tick ago.
    pub to_prev: [f32; 3],
    /// Size of the target: what is torn off it comes from all over this.
    pub radius: f32,
    pub to: [f32; 3],
    pub height: f32,
}

impl World {
    /// This tick's reclaim beams. Left out when the viewer can see neither end.
    pub(crate) fn write_reclaim_beams(
        &self,
        viewer: Option<u8>,
        out: &mut Vec<BeamInstance>,
        sources: &mut Vec<u32>,
    ) {
        let s = &self.state;
        out.clear();
        sources.clear();
        let seen = |p: FxVec2| {
            viewer.is_none_or(|v| !s.fog_enabled || self.fog.is_detected(p, self.team_mask(v)))
        };
        for work in &self.reclaims {
            let Some(row) = s.units.row(work.source) else {
                continue;
            };
            // Where it is now if it is still there; where it was when the last of it went.
            let (to_prev, to) = match s.units.row(work.unit) {
                Some(t) => (
                    s.units.prev_pos[t].extend(s.units.prev_z[t]),
                    s.units.pos[t].extend(s.units.z[t]),
                ),
                None => (work.at, work.at),
            };
            if !seen(s.units.pos[row]) && !seen(to.xy()) {
                continue;
            }
            let bp = self.bp(row);
            let (emitter, facing) = match (bp.reclaimer, bp.builder.as_ref().and_then(|b| b.arm)) {
                (Some(r), _) => (r.emitter, s.units.heading[row] + s.units.weapon_yaw[row][0]),
                // The build arm is pitched at its work: the beam leaves from where its tip has swung to.
                (None, Some(arm)) => (
                    crate::world::pitched(arm.emitter, arm.pivot, s.units.arm_pitch[row][1]),
                    s.units.heading[row] + s.units.weapon_yaw[row][0],
                ),
                (None, None) => (
                    FxVec3::new(Fx::ZERO, Fx::ZERO, bp.height),
                    s.units.heading[row],
                ),
            };
            let from = (s.units.pos[row] + FxVec2::new(emitter.x, emitter.y).rotate(facing))
                .extend(s.units.z[row] + emitter.z);
            sources.push(work.source.0);
            out.push(BeamInstance {
                from: from.to_f32(),
                kind: BEAM_RECLAIM,
                to_prev: to_prev.to_f32(),
                radius: work.radius.to_f32(),
                to: to.to_f32(),
                height: work.height.to_f32(),
            });
        }
    }

    /// How far this unit's tools reach: a builder's, or else a reclaimer's.
    pub(crate) fn work_range(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        bp.builder
            .as_ref()
            .map(|b| b.range)
            .or(bp.reclaimer.map(|r| r.range))
            .unwrap_or(Fx::ZERO)
    }

    /// Whether the unit in `row` may turn a reclaim beam on the unit in `t`:
    /// its owner's own units, and enemies it can see. Never an ally's.
    pub(crate) fn can_reclaim_unit(&self, row: usize, t: usize) -> bool {
        let units = &self.state.units;
        if t == row
            || !units.slots.is_alive(t)
            || units.health[t] <= Fx::ZERO
            || units.flags[t] & (flag::IN_FACTORY | flag::INVULNERABLE) != 0
        {
            return false;
        }
        let (owner, theirs) = (units.owner[row], units.owner[t]);
        owner == theirs || (self.are_enemies(owner, theirs) && self.detects(owner, t))
    }

    /// One tick of pulling mass out of a wreck. True once there is none left.
    pub(crate) fn drain_wreck(&mut self, row: usize, w: usize) -> bool {
        let power = self.bp(row).reclaims().map_or(Fx::ZERO, |(power, _)| power);
        let wrecks = &mut self.state.wrecks;
        let take = (power * World::reclaim_rate()).min(wrecks.mass[w]);
        wrecks.mass[w] -= take;
        let bp = self.blueprints.unit(wrecks.blueprint[w]);
        let at = wrecks.pos[w].extend(wrecks.z[w]);
        let gone = wrecks.mass[w] <= Fx::ZERO;
        if gone {
            wrecks.slots.free(w);
            self.events.push(SimEvent::Reclaimed {
                pos: at,
                blueprint: bp.id,
                wreck: true,
            });
        }
        self.reclaims.push(ReclaimWork {
            source: self.state.units.id(row),
            unit: Handle::NONE,
            at,
            radius: bp.radius,
            height: bp.height / 2,
        });
        self.credit(row, take);
        gone
    }

    /// One tick of unbuilding a live unit. True once it has nothing left to give.
    fn drain_unit(&mut self, row: usize, t: usize) -> bool {
        let power = self.bp(row).reclaims().map_or(Fx::ZERO, |(power, _)| power);
        let tbp = self.bp(t);
        let (full, time, cost, radius, height) = (
            tbp.health,
            tbp.build_time,
            tbp.cost_mass,
            tbp.radius,
            tbp.height,
        );
        let (by, of, source) = (
            self.state.units.owner[row],
            self.state.units.owner[t],
            self.state.units.id(row),
        );
        let enemies = self.are_enemies(by, of);
        let taken = (full * (power / DT) / time)
            .max(Fx::EPSILON)
            .min(self.state.units.health[t]);
        self.state.units.health[t] -= taken;
        self.state.units.flags[t] |= flag::HURT;
        // A site is worth what has been put into it: a fresh one gives nothing back.
        let built = self.state.units.build_progress[t] / time;
        let mass = cost * taken / full * built * UNIT_YIELD;
        let gone = self.state.units.health[t] <= Fx::ZERO;
        if enemies {
            self.record_damage(t, source, taken);
        }
        if gone {
            self.state.units.flags[t] |= flag::RECLAIMED;
            if enemies {
                self.settle_kill(t, source, by);
            }
        }
        let units = &self.state.units;
        self.reclaims.push(ReclaimWork {
            source: units.id(row),
            unit: units.id(t),
            at: units.pos[t].extend(units.z[t]),
            radius,
            height,
        });
        self.credit(row, mass);
        gone
    }

    fn credit(&mut self, row: usize, mass: Fx) {
        let player = &mut self.state.players[self.state.units.owner[row] as usize];
        player.mass += mass;
        player.reclaimed_mass += mass;
        self.state.units.flags[row] |= flag::RECLAIMING;
    }

    pub(crate) fn run_reclaim_unit(&mut self, row: usize, o: &Order) -> Result<(), SimError> {
        let units = &self.state.units;
        let Some(t) = units
            .row(o.target)
            .filter(|&t| self.can_reclaim_unit(row, t))
        else {
            self.finish_order(row);
            return Ok(());
        };
        let (pos, radius) = (units.pos[t], self.bp(t).radius);
        let reach = self.work_range(row) + radius;
        if units.pos[row].distance(pos) > reach
            && self.bp(row).motion.is_some()
            && units.stuck_ticks[row] != u16::MAX
        {
            // After something that may be driving off: the way there is only asked for again once it has got far from it.
            let goal = if units.has_flag(row, flag::HAS_FIELD)
                && units.field_goal[row].distance(pos) <= CHASE_REPATH_DISTANCE
            {
                units.field_goal[row]
            } else {
                pos
            };
            return self.ensure_moving(row, goal, pos);
        }
        if !self.approach(row, pos, radius)? {
            if self.state.units.stuck_ticks[row] == u16::MAX {
                self.finish_order(row);
            }
            return Ok(());
        }
        if self.reclaim_ready(row, pos) && self.drain_unit(row, t) {
            self.finish_order(row);
        }
        Ok(())
    }

    /// Turns onto `pos` and, for a reclaimer turret, waits out its charge.
    /// True once the beam may come on. Losing the aim dumps the charge.
    pub(crate) fn reclaim_ready(&mut self, row: usize, pos: FxVec2) -> bool {
        if !self.face_work(row, pos) {
            self.state.units.reclaim_charge[row] = 0;
            return false;
        }
        let need = self.bp(row).reclaimer.map_or(0, |r| r.charge_ticks);
        let charged = self.state.units.reclaim_charge[row];
        if charged < need {
            self.state.units.reclaim_charge[row] = charged + 1;
            return false;
        }
        true
    }

    /// What a reclaimer with no orders does by itself: it clears the wrecks within
    /// its reach while there is room to store the mass. It never leaves where it
    /// stands, and it never turns its beam on a live unit unasked: that takes an
    /// order. A builder that carries weapons leaves wrecks alone while there is
    /// anything about to shoot at, so its torso is never on a wreck between two targets.
    pub(crate) fn idle_reclaim(&mut self, row: usize) -> Result<(), SimError> {
        let bp = self.bp(row);
        let Some((_, range)) = bp.reclaims() else {
            return Ok(());
        };
        let units = &self.state.units;
        if units.has_flag(row, flag::PASSIVE)
            || self.has_live_target(row)
            || self.enemy_in_gun_range(row)
        {
            return Ok(());
        }
        let (pos, owner) = (units.pos[row], units.owner[row]);
        let player = &self.state.players[owner as usize];
        if player.mass >= player.mass_capacity {
            return Ok(());
        }
        let wrecks = &self.state.wrecks;
        let found = self
            .index
            .nearest(pos, range + WIDEST_TARGET, kind::WRECK, |e| {
                let w = e.row as usize;
                wrecks.slots.is_alive(w)
                    && wrecks.pos[w] == e.pos
                    && e.pos.distance(pos) <= range + e.radius
            });
        if let Some(e) = found {
            if self.reclaim_ready(row, e.pos) {
                self.drain_wreck(row, e.row as usize);
            }
        } else {
            self.state.units.reclaim_charge[row] = 0;
        }
        Ok(())
    }

    /// Whether an enemy this unit can see stands within its guns' reach, or near enough to be there soon.
    fn enemy_in_gun_range(&self, row: usize) -> bool {
        let reach = self.bp(row).max_weapon_range();
        if reach <= Fx::ZERO {
            return false;
        }
        let units = &self.state.units;
        let (pos, owner, reach) = (units.pos[row], units.owner[row], reach * GUN_RANGE_MARGIN);
        let mut found = false;
        self.index.query(pos, reach, kind::UNIT, |e| {
            let t = e.row as usize;
            found = self.unit_entry_is_current(e)
                && self.are_enemies(owner, units.owner[t])
                && !units.has_flag(t, flag::IN_FACTORY)
                && self.detects(owner, t);
            !found
        });
        found
    }
}
