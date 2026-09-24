//! The Argon Electric Bore (docs/STYLE.md "The electric bore").
//!
//! The gun fires an argon tracer round, an ordinary shot. Where it lands, the charge is
//! struck down the ionised channel it left: the tracer's own path from the muzzle. The
//! weapon's damage and splash land at the end as for any shot (`apply_impact`); a bore
//! with a `width` also sears everything within that distance of the channel on the way,
//! and scorches the ground under it. The glowing, cooling track is the renderer's.

use mc_core::{Fx, FxVec3};

use crate::mirror::SimEvent;
use crate::spatial::kind;
use crate::tables::flag;
use crate::{SimError, World};

/// Metres between the scorch marks laid along a channel that runs over the ground.
const SCORCH_STEP: i32 = 10;

impl World {
    /// The discharge for the bore tracer `projectile`, which has just landed at `to`.
    /// `struck`: the unit (or hull field) the tracer itself hit, which takes the shot's
    /// own damage and is not seared again. `after`: how far into the tick it landed.
    pub(crate) fn bore_discharge(
        &mut self,
        projectile: usize,
        to: FxVec3,
        struck: Option<usize>,
        after: Fx,
    ) -> Result<(), SimError> {
        let blueprints = self.blueprints.clone();
        let p = &self.state.projectiles;
        let (blueprint, w) = (p.blueprint[projectile], p.weapon[projectile]);
        let weapon = &blueprints.unit(blueprint).weapons[w as usize];
        let Some(bore) = weapon.bore else {
            return Ok(());
        };
        let (owner, source) = (p.owner[projectile], p.source[projectile]);
        // The tracer flies straight: back from the start of this tick's step by the whole
        // steps it flew before it, to the muzzle it left.
        let vel = p.vel[projectile];
        let flown = Fx::from_int(p.age[projectile].saturating_sub(1) as i32);
        let from = p.prev_pos[projectile] - vel * flown;
        self.events.push(SimEvent::BoreDischarge {
            from,
            to,
            width: bore.width,
            after,
            owner,
            blueprint,
            weapon: w,
        });
        // All electric bores ignite the ground corridor, including the T3's
        // otherwise single-target discharge. A capsule has no gaps between samples.
        let seg = to - from;
        let len_sq = seg.xy().length_sq().max(Fx::EPSILON);
        let burn_width = bore.width.max(Fx::from_int(4));
        let mid = from.xy() + seg.xy() * Fx::HALF;
        let mut trees = Vec::new();
        self.prop_index.query(mid, seg.xy().length() / 2 + burn_width, kind::PROP, |e| {
            let prop = e.row as usize;
            let t = ((e.pos - from.xy()).dot(seg.xy()) / len_sq).clamp(Fx::ZERO, Fx::ONE);
            if e.pos.distance_sq((from + seg * t).xy()) <= burn_width * burn_width
                && self.map.props[prop].kind.is_tree() && self.is_prop_alive(prop)
            {
                trees.push(prop);
            }
            true
        });
        trees.sort_unstable();
        for prop in trees {
            self.state.props_dead[prop / 64] |= 1 << (prop % 64);
        }
        if bore.width <= Fx::ZERO || bore.damage <= Fx::ZERO {
            return Ok(());
        }

        // Everything the channel passes within `width` of, between its feet and its top.
        let seg = to - from;
        let len_sq = seg.xy().length_sq().max(Fx::EPSILON);
        let mid = from.xy() + seg.xy() * Fx::HALF;
        let mut victims = Vec::new();
        self.index
            .query(mid, seg.xy().length() / 2 + bore.width, kind::UNIT, |e| {
                let row = e.row as usize;
                let units = &self.state.units;
                if Some(row) == struck
                    || !self.unit_entry_is_current(e)
                    || units.has_flag(row, flag::IN_FACTORY)
                    || !self.are_enemies(owner, units.owner[row])
                    || !self.weapon_reaches(row, weapon)
                {
                    return true;
                }
                let t = ((e.pos - from.xy()).dot(seg.xy()) / len_sq).clamp(Fx::ZERO, Fx::ONE);
                let at = from + seg * t;
                let reach = e.radius + bore.width;
                let (low, high) = (units.z[row], units.z[row] + self.bp(row).height);
                if at.xy().distance_sq(e.pos) <= reach * reach
                    && at.z >= low - bore.width
                    && at.z <= high + bore.width
                {
                    victims.push(row);
                }
                true
            });
        // Rows in table order, so the outcome does not depend on the index's walk.
        victims.sort_unstable();
        // A dome between the gun and a victim takes the charge meant for it, once.
        let victims: Vec<_> = victims
            .into_iter()
            .map(|r| {
                let units = &self.state.units;
                let middle = units.pos[r].extend(units.z[r] + self.bp(r).height / 2);
                (r, self.blast_blocker(from, middle, Some(r)))
            })
            .collect();
        let mut charged = Vec::new();
        for (r, blocker) in victims {
            if let Some(shield) = blocker {
                if !charged.contains(&shield) {
                    self.damage_shield(shield, bore.damage);
                    charged.push(shield);
                }
                continue;
            }
            self.damage_unit(r, bore.damage, owner, source);
        }

        // The ground under a low channel is scorched. Tree ignition above is independent
        // of altitude; a shot high over a valley leaves the valley floor unscorched.
        let length = seg.length();
        let steps = (length / Fx::from_int(SCORCH_STEP)).floor_int().clamp(1, 256);
        for i in 0..=steps {
            let at = from + seg * Fx::ratio(i as i64, steps as i64);
            let ground = self.terrain.height_at(at.xy());
            if at.z - ground > bore.width * 2 {
                continue;
            }
            self.add_stain(at.xy(), bore.width * Fx::ratio(3, 2), 80)?;
        }
        Ok(())
    }
}

/// Whether a bore's tracer flies on past `unit` rather than landing on it: a searing
/// bore's (`Bore::width`) tracer goes through to its mark, and what it passes on the way
/// is seared by the discharge. One fired at the ground lands on whatever it meets.
pub(crate) fn passes_through(
    weapon: &mc_data::Weapon,
    mark: crate::tables::UnitId,
    unit: crate::tables::UnitId,
) -> bool {
    weapon.bore.is_some_and(|b| b.width > Fx::ZERO)
        && mark != crate::tables::Handle::NONE
        && mark != unit
}
