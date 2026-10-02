//! Cone weapons (`Weapon::cone`, the Regency Wake Tank's wake): a shot that strikes the
//! whole fan ahead of the gun at once rather than one target.
//!
//! - **What it reaches:** every enemy the gun may shoot (`targets`, never under the water)
//!   whose hull reaches into the fan: within range of the muzzle, and within the cone's
//!   half-angle of the gun's facing, its radius counted, so a big hull at the edge is
//!   caught by its flank.
//! - **Falloff:** full damage at the muzzle, falling in a straight line with the distance
//!   along the gun's facing to `Cone::edge` of it at full range.
//! - **Cover:** the wake rolls over the ground, so ground between the muzzle and a unit
//!   shields it (`clear_shot`, the line every direct-fire gun needs). A shield dome
//!   between takes the hit for all it covers, once a shot, as a blast's is
//!   (`blast_blocker`); a hull shield takes its own unit's share.
//! - **The ground:** seared along the fan (stains), as a plasma strike sears it.
//!
//! The firing itself (cooldown, salvo, `ShotFired` for what is drawn and heard) is the
//! ordinary gun's (`combat::step_weapon`); only the projectile is left out.

use crate::spatial::kind;
use crate::tables::flag;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Weapon;

/// Metres past the range a hull's middle may stand and still reach into the fan: more
/// than the widest hull's radius, so the search never misses one the cone's own test takes.
const HULL_MARGIN: i32 = 40;
/// Searing stains laid along the fan's middle each shot.
const STAINS: i32 = 3;

impl World {
    /// Weapon `weapon` of `row` fired its cone from `muzzle`, the gun facing `facing`:
    /// everything it may shoot in the fan takes its share of the damage at once.
    pub(crate) fn wake(&mut self, row: usize, weapon: &Weapon, muzzle: FxVec3, facing: Angle) {
        let Some(cone) = weapon.cone else {
            return;
        };
        let units = &self.state.units;
        let (owner, source) = (units.owner[row], units.id(row));
        let range = weapon.range_max.max(Fx::ONE);
        let dir = FxVec2::from_angle(facing);
        let edge = FxVec2::from_angle(cone.half);
        let tan = edge.y / edge.x.max(Fx::EPSILON);
        let from = muzzle.xy();

        let mut near = Vec::new();
        self.index.query_foes(
            from,
            range + Fx::from_int(HULL_MARGIN),
            kind::UNIT,
            self.team_mask(owner),
            |e| {
                if self.unit_entry_is_current(e) {
                    near.push(e.row as usize);
                }
                true
            },
        );
        // The index hands rows over in its own order: struck in row order, every run alike.
        near.sort_unstable();
        near.dedup();

        let mut victims = Vec::new();
        for t in near {
            let units = &self.state.units;
            if t == row
                || units.has_flag(t, flag::IN_FACTORY)
                || !self.are_enemies(owner, units.owner[t])
                || !self.hittable(t, weapon.target_mask)
            {
                continue;
            }
            let bp = self.bp(t);
            let d = units.pos[t] - from;
            let along = d.dot(dir);
            let across = (d.x * dir.y - d.y * dir.x).abs();
            if along < -bp.radius
                || along > range + bp.radius
                || across > along.max(Fx::ZERO) * tan + bp.radius
                || !self.clear_shot(row, weapon, t)
            {
                continue;
            }
            let out = along.clamp(Fx::ZERO, range) / range;
            let damage = weapon.damage * (Fx::ONE - (Fx::ONE - cone.edge) * out);
            let target = units.pos[t].extend(units.z[t] + bp.height / 2);
            // Every shield is read before any takes a hit: one the wake breaks still
            // covers the rest of what is under it this shot.
            let blocker = self.blast_blocker(muzzle, target, Some(t));
            victims.push((t, damage, blocker));
        }

        let mut charged = Vec::new();
        for (t, damage, blocker) in victims {
            if let Some(shield) = blocker.filter(|&s| s != t) {
                if !charged.contains(&shield) {
                    charged.push(shield);
                    self.damage_shield(shield, damage);
                }
                continue;
            }
            if self.shield_blocking(t) && self.bp(t).shield.is_some_and(|s| s.is_hull()) {
                self.damage_shield(t, damage);
            } else {
                self.damage_unit(t, damage, owner, source);
            }
        }

        // The ground it rolled over, seared down the fan's middle, wider as it spreads.
        for k in 1..=STAINS {
            let along = range * k / (STAINS + 1);
            let at = from + dir * along;
            if self.terrain.height_at(at) > self.terrain.water_level() {
                self.add_stain(at, (along * tan).max(Fx::from_int(3)), 64);
            }
        }
    }
}
