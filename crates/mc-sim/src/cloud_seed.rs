//! Storm seeds (`Weapon::cloud`, the Tempest's Storm Seeder): a shot that is a cloud.
//!
//! The seed is aimed at nothing. It leaves the muzzle at the tail while any foe is in
//! reach, and hangs where it was let go, carried on a little by the aircraft's way and
//! slowing to a stop as it grows (`path`, toward the `mark` the weapons phase gives it), so
//! a run lays a trail of clouds over what it crosses. It hits nothing as it drifts
//! (`impacts.rs` passes it over).
//! Grown, it lets fall its bolts, spread evenly over the `strike` time
//! (`CloudSeed::bolts_by`), then it is gone.
//!
//! Each bolt is a sub-shot (`Projectiles::sub`, `Weapon::sub_shot`: an even share of the
//! damage and the bolt's own splash) let go from the cloud's base, over what it strikes,
//! straight at it, fast enough to land the tick it is let go. It strikes the foes under the cloud
//! within its `radius`, nearest first, one a bolt and round again when there are more bolts
//! than foes; the ground under the cloud when there are none, on a sunflower over the
//! disc (`cluster::offset`). A dome over a foe takes the bolt, as it would a shell. No
//! randomness: which bolt falls where follows from the seed's flight and what is under it.

use mc_core::{Fx, FxVec2, FxVec3};

use crate::spatial::kind;
use crate::tables::{Handle, UnitId, MAX_PROJECTILES};
use crate::{SimError, SimEvent, Table, World};

/// Metres below a cloud's middle its bolts leave from: the base of the cloud as drawn.
const BASE_DROP: i32 = 20;
/// How far out from the cloud's middle toward what it strikes a bolt leaves: two thirds
/// of the way, inside the cloud as it is drawn.
const BASE_OUT_NUM: i64 = 2;
const BASE_OUT_DEN: i64 = 3;
/// Ticks a bolt is given: it lands on the first, one more in case it was let go from just
/// over a rise.
const BOLT_TICKS: u16 = 2;
/// Metres past what it strikes a bolt is thrown, so its one tick's sweep reaches into the
/// ground (or through the unit) rather than stopping just short.
const BOLT_OVERRUN: i32 = 6;

/// Where a seed let go at `origin` drifting to `mark` is after `age` ticks of a `grow`-tick
/// drift: on toward the mark at its own height, easing in (`1 - (1 - s)^2`), and there after.
pub(crate) fn path(origin: FxVec3, mark: FxVec2, age: u16, grow: u16) -> FxVec3 {
    let s = Fx::ratio(age.min(grow) as i64, grow.max(1) as i64);
    let left = Fx::ONE - s;
    let eased = Fx::ONE - left * left;
    (origin.xy() + (mark - origin.xy()) * eased).extend(origin.z)
}

impl World {
    /// Each seed's drift for this tick's step, and the bolts it lets fall this tick.
    /// `SimError::TableFull` if the projectile table cannot take them.
    pub(crate) fn steer_cloud_seeds(&mut self) -> Result<(), SimError> {
        let blueprints = self.blueprints.clone();
        let mut falling = Vec::new();
        let p = &mut self.state.projectiles;
        for i in 0..p.len() {
            if p.sub[i] > 0 {
                continue;
            }
            let Some(cloud) = blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].cloud
            else {
                continue;
            };
            let age = p.age[i];
            let (origin, mark) = (p.origin[i], p.mark[i].xy());
            p.vel[i] = path(origin, mark, age + 1, cloud.grow_ticks)
                - path(origin, mark, age, cloud.grow_ticks);
            let before = if age == 0 { 0 } else { cloud.bolts_by(age - 1) };
            for k in before..cloud.bolts_by(age) {
                falling.push((i, k));
            }
        }
        if self.state.projectiles.len() + falling.len() > MAX_PROJECTILES {
            return Err(SimError::TableFull(Table::Projectiles));
        }
        for (i, k) in falling {
            self.let_fall_bolt(i, k)?;
        }
        Ok(())
    }

    /// Bolt `k` (from zero) of seed `i`, let go from the cloud's base onto what it strikes.
    fn let_fall_bolt(&mut self, i: usize, k: u8) -> Result<(), SimError> {
        let p = &self.state.projectiles;
        let (owner, source, blueprint, slot) =
            (p.owner[i], p.source[i], p.blueprint[i], p.weapon[i]);
        let (centre, serial) = (p.pos[i], p.serial[i]);
        let weapon = &self.blueprints.unit(blueprint).weapons[slot as usize];
        let Some(cloud) = weapon.cloud else {
            return Ok(());
        };
        let mask = weapon.target_mask;
        // The foes under it, nearest first (the id breaks a tie, so the order is total).
        let mut under: Vec<(Fx, UnitId, usize)> = Vec::new();
        let around = cloud.radius + self.index.max_radius();
        self.index.query(centre.xy(), around, kind::UNIT, |e| {
            let r = e.row as usize;
            if self.unit_entry_is_current(e)
                && self.state.units.is_active(r)
                && self.are_enemies(owner, self.state.units.owner[r])
                && self.blast_hittable(r, mask)
            {
                let d = self.state.units.pos[r].distance(centre.xy());
                if d <= cloud.radius + self.bp(r).radius {
                    under.push((d, self.state.units.id(r), r));
                }
            }
            true
        });
        under.sort_by_key(|&(d, id, _)| (d, id.0));
        let (to, target) = match under.get(k as usize % under.len().max(1)) {
            Some(&(_, id, r)) => {
                let u = &self.state.units;
                let middle = u.z[r] + self.bp(r).height / 2;
                (u.pos[r].extend(middle), id)
            }
            None => {
                let spin = mc_core::Angle((serial.wrapping_mul(2_654_435_761) >> 16) as u16);
                let xy = self.clamp_to_map(
                    centre.xy() + crate::cluster::offset(k, cloud.bolts, cloud.radius, spin),
                );
                let ground = self.terrain.height_at(xy).max(self.terrain.water_level());
                (xy.extend(ground), Handle::NONE)
            }
        };
        // It leaves the part of the cloud's base over what it strikes, two thirds of the way out
        // toward it, so it falls nearly straight down rather than slanting from the middle.
        let out = (to.xy() - centre.xy()) * Fx::ratio(BASE_OUT_NUM, BASE_OUT_DEN);
        let from = (centre.xy() + out).extend(centre.z - Fx::from_int(BASE_DROP));
        let fall = to - from;
        let length = fall.length().max(Fx::ONE);
        let vel = fall * ((length + Fx::from_int(BOLT_OVERRUN)) / length);
        self.state
            .projectiles
            .spawn(from, vel, owner, source, blueprint, slot, BOLT_TICKS)?;
        let p = &mut self.state.projectiles;
        let b = p.len() - 1;
        p.sub[b] = k + 1;
        p.target[b] = target;
        p.mark[b] = to;
        p.origin[b] = from;
        self.events.push(SimEvent::CloudBolt {
            from,
            to,
            owner,
            blueprint,
            weapon: slot,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_drifts_out_over_its_mark_and_stays_there() {
        let origin = FxVec3::new(Fx::from_int(100), Fx::from_int(50), Fx::from_int(300));
        let mark = FxVec2::new(Fx::from_int(700), Fx::from_int(50));
        let grow = 50;
        assert_eq!(path(origin, mark, 0, grow), origin);
        let mut last = Fx::ZERO;
        for age in 1..=grow {
            let step = path(origin, mark, age, grow).x - path(origin, mark, age - 1, grow).x;
            assert!(step > Fx::ZERO, "it keeps going out");
            assert!(age == 1 || step <= last, "and slows as it comes");
            last = step;
        }
        assert_eq!(path(origin, mark, grow, grow).xy(), mark);
        assert_eq!(
            path(origin, mark, grow + 9, grow),
            path(origin, mark, grow, grow)
        );
        assert_eq!(path(origin, mark, grow + 9, grow).z, origin.z);
    }

    #[test]
    fn every_bolt_falls_once_over_the_strike() {
        let cloud = mc_data::CloudSeed {
            grow_ticks: 40,
            strike_ticks: 15,
            bolts: 6,
            radius: Fx::from_int(150),
            splash: Fx::from_int(25),
        };
        assert_eq!(cloud.bolts_by(39), 0);
        assert_eq!(cloud.bolts_by(40), 1, "the first falls as it is grown");
        let mut fell = 0;
        for age in 0..cloud.life_ticks() {
            let before = if age == 0 { 0 } else { cloud.bolts_by(age - 1) };
            assert!(cloud.bolts_by(age) >= before);
            fell += cloud.bolts_by(age) - before;
        }
        assert_eq!(fell, 6);
        assert_eq!(
            cloud.bolts_by(cloud.life_ticks() - 1),
            6,
            "all before it is gone"
        );
    }
}
