//! Cone weapons (`Weapon::cone`, the Regency Wake Tank's wake): a shot that rolls out
//! from the gun as one front across the fan ahead of it and strikes everything the front
//! reaches, rather than one target.
//!
//! - **The front:** let go from the muzzle on the tick the gun fires, it rolls out along
//!   the gun's facing at that moment, `Cone::speed` metres a second, to the gun's range.
//!   It is its own thing once fired (`State::wakes`): it rolls on if the gun dies or
//!   turns away, and a unit that outruns it, or leaves the fan before it arrives, is
//!   missed.
//! - **What it reaches:** every enemy the gun may shoot (`targets`, never under the water)
//!   whose hull the front passes over this tick: within the cone's half-angle of the
//!   facing, its radius counted, so a big hull at the edge is caught by its flank. Each
//!   unit is struck once a wake.
//! - **Falloff:** full damage at the muzzle, falling in a straight line with the distance
//!   along the facing to `Cone::edge` of it at full range.
//! - **Cover:** the wake rolls over the ground, so ground between the muzzle and a unit
//!   shields it (`clear_from`, the line every direct-fire gun needs). A shield dome
//!   between takes the hit for all it covers, once a wake, as a blast's is
//!   (`blast_blocker`); a hull shield takes its own unit's share.
//! - **The ground:** seared along the fan (stains) as the front passes, as a plasma
//!   strike sears it.
//!
//! The firing itself (cooldown, salvo, `ShotFired` for what is drawn and heard) is the
//! ordinary gun's (`combat::step_weapon`); only the projectile is left out.

use std::sync::Arc;

use crate::spatial::kind;
use crate::tables::{flag, UnitId};
use crate::world::State;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Metres past the band the front sweeps a tick that a hull's middle may stand and still
/// be reached: more than the widest hull's radius, so the search never misses one the
/// cone's own test takes.
const HULL_MARGIN: i32 = 40;
/// Searing stains laid along the fan's middle each wake.
const STAINS: i32 = 3;

/// A wake rolling out over the ground (`wake.rs`).
#[derive(Clone, Serialize, Deserialize)]
pub struct Wake {
    /// The muzzle it was let go from, and the gun's facing then.
    pub from: FxVec3,
    pub facing: Angle,
    pub owner: u8,
    pub source: UnitId,
    /// The gun that fired it: what it strikes, how hard, how far and how fast.
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Ticks rolled so far.
    pub age: u16,
    /// Units it has struck and shield domes it has charged: none of them twice.
    pub struck: Vec<UnitId>,
    pub shields: Vec<UnitId>,
}

impl World {
    /// Weapon `w` of `row` fired its cone from `muzzle`, the gun facing `facing`: the wake
    /// is let go, and rolls out from this tick on (`run_wakes`).
    pub(crate) fn wake(&mut self, row: usize, w: usize, muzzle: FxVec3, facing: Angle) {
        let units = &self.state.units;
        self.state.wakes.push(Wake {
            from: muzzle,
            facing,
            owner: units.owner[row],
            source: units.id(row),
            blueprint: units.blueprint[row],
            weapon: w as u8,
            age: 0,
            struck: Vec::new(),
            shields: Vec::new(),
        });
    }

    /// Every wake rolls a tick further, striking what its front passes over.
    pub(crate) fn run_wakes(&mut self) {
        let wakes = std::mem::take(&mut self.state.wakes);
        let mut left = Vec::with_capacity(wakes.len());
        for mut wake in wakes {
            if self.roll_wake(&mut wake) {
                left.push(wake);
            }
        }
        self.state.wakes = left;
    }

    /// One tick of `wake`: whether it still has ground ahead of it.
    fn roll_wake(&mut self, wake: &mut Wake) -> bool {
        let blueprints = Arc::clone(&self.blueprints);
        let Some(weapon) = blueprints
            .unit(wake.blueprint)
            .weapons
            .get(wake.weapon as usize)
        else {
            return false;
        };
        let Some(cone) = weapon.cone else {
            return false;
        };
        let range = weapon.range_max.max(Fx::ONE);
        let per_second = Fx::from_int(mc_core::TICKS_PER_SECOND as i32);
        let behind = (cone.speed * wake.age as i32 / per_second).min(range);
        wake.age = wake.age.saturating_add(1);
        let ahead = (cone.speed * wake.age as i32 / per_second).min(range);

        let dir = FxVec2::from_angle(wake.facing);
        let edge = FxVec2::from_angle(cone.half);
        let tan = edge.y / edge.x.max(Fx::EPSILON);
        let from = wake.from.xy();

        // Everything within reach of the band the front sweeps this tick: no further from
        // its middle than half its depth along plus the fan's half-width across.
        let middle = from + dir * ((behind + ahead) / 2);
        let reach = (ahead - behind) / 2 + ahead * tan + Fx::from_int(HULL_MARGIN);
        let mut near = Vec::new();
        self.index
            .query_foes(middle, reach, kind::UNIT, self.team_mask(wake.owner), |e| {
                if self.unit_entry_is_current(e) {
                    near.push(e.row as usize);
                }
                true
            });
        // The index hands rows over in its own order: struck in row order, every run alike.
        near.sort_unstable();
        near.dedup();

        let mut victims = Vec::new();
        for t in near {
            let units = &self.state.units;
            if units.id(t) == wake.source
                || wake.struck.contains(&units.id(t))
                || units.has_flag(t, flag::IN_FACTORY)
                || !self.are_enemies(wake.owner, units.owner[t])
                || !self.hittable(t, weapon.target_mask)
            {
                continue;
            }
            let bp = self.bp(t);
            let d = units.pos[t] - from;
            let along = d.dot(dir);
            let across = (d.x * dir.y - d.y * dir.x).abs();
            // The front passed over its hull this tick, and its hull reaches into the fan.
            if along + bp.radius < behind
                || along - bp.radius > ahead
                || across > along.max(Fx::ZERO) * tan + bp.radius
                || !self.clear_from(wake.from, t)
            {
                continue;
            }
            let out = along.clamp(Fx::ZERO, range) / range;
            let damage = weapon.damage * (Fx::ONE - (Fx::ONE - cone.edge) * out);
            let target = units.pos[t].extend(units.z[t] + bp.height / 2);
            // Every shield is read before any takes a hit: one the wake breaks still
            // covers the rest of what is under it this tick.
            let blocker = self.blast_blocker(wake.from, target, Some(t));
            victims.push((t, damage, blocker));
        }

        for (t, damage, blocker) in victims {
            wake.struck.push(self.state.units.id(t));
            if let Some(shield) = blocker.filter(|&s| s != t) {
                let id = self.state.units.id(shield);
                if !wake.shields.contains(&id) {
                    wake.shields.push(id);
                    self.damage_shield(shield, damage);
                }
                continue;
            }
            if self.shield_blocking(t) && self.bp(t).shield.is_some_and(|s| s.is_hull()) {
                self.damage_shield(t, damage);
            } else {
                self.damage_unit(t, damage, wake.owner, wake.source);
            }
        }

        // The ground it rolls over, seared down the fan's middle, wider as it spreads.
        for k in 1..=STAINS {
            let along = range * k / (STAINS + 1);
            if along <= behind || along > ahead {
                continue;
            }
            let at = from + dir * along;
            if self.terrain.height_at(at) > self.terrain.water_level() {
                self.add_stain(at, (along * tan).max(Fx::from_int(3)), 64);
            }
        }
        ahead < range
    }
}

/// The rolling wakes, for the state hash.
pub(crate) fn hash_wakes(s: &State, h: &mut StateHasher) {
    h.write_u64(s.wakes.len() as u64);
    for w in &s.wakes {
        for v in [w.from.x, w.from.y, w.from.z] {
            h.write_i64(v.0);
        }
        h.write_u64(
            w.facing.0 as u64
                | (w.owner as u64) << 16
                | (w.weapon as u64) << 24
                | (w.age as u64) << 32,
        );
        h.write_u64(w.source.0 as u64 | (w.blueprint.0 as u64) << 32);
        h.write_u64(w.struck.len() as u64 | (w.shields.len() as u64) << 32);
        for id in w.struck.iter().chain(&w.shields) {
            h.write_u64(id.0 as u64);
        }
    }
}
