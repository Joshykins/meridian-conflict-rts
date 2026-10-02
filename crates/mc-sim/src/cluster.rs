//! Cluster shots (`Weapon::cluster`, the Regency's Heavy Gravitic Seeker): an unguided
//! missile lobbed on a high arc that breaks into sub-shots on its way down.
//!
//! The whole shot splits on the first tick it is coming down within `height` metres of
//! its mark, or at the top of its arc if that is lower. Each sub-shot leaves from the
//! split point with the parent's way on it plus a sideways drift that carries it, by the
//! tick it comes down to the mark's height, to its own point on a disc `radius` metres
//! round where the parent would have landed: a sunflower pattern (each point a golden
//! angle round from the last, out by the square root of its number), turned by the shot's
//! serial. No randomness: where every sub-shot lands follows from the parent's flight.
//!
//! Each sub-shot lands with an even share of the damage and its own splash
//! (`Weapon::sub_shot`), and is a missile like its parent: intercept lasers can take it,
//! each with its share of the casing that was left. Kill the parent before it splits and
//! the whole volley is saved; after, each piece must be taken on its own.

use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Weapon;

use crate::combat::GRAVITY;
use crate::tables::MAX_PROJECTILES;
use crate::{SimError, SimEvent, Table, World};

/// A golden angle in angle steps (65536 a turn): 137.5 degrees.
const GOLDEN: u16 = 25_033;
/// The longest fall, in ticks, a split works out to its mark's height. A shot still above
/// it after this many ticks (it was split far over a deep valley) takes this flight.
const MAX_FALL: i32 = 4096;

/// Whether a whole cluster shot at `pos` flying at `vel` breaks this tick: it is coming
/// down (or level, at the top of its arc) and within the weapon's split height of `mark`.
pub(crate) fn splits(weapon: &Weapon, pos: FxVec3, vel: FxVec3, mark: FxVec3) -> bool {
    weapon
        .cluster
        .is_some_and(|c| vel.z - GRAVITY <= Fx::ZERO && pos.z - mark.z <= c.height)
}

/// Ticks a shot at height `z` with climb `vz` (metres a tick) takes to fall to `floor`,
/// stepped as `run_projectiles` steps it: gravity, then the move.
pub(crate) fn fall_ticks(z: Fx, vz: Fx, floor: Fx) -> i32 {
    let (mut z, mut vz) = (z, vz);
    for n in 1..=MAX_FALL {
        vz -= GRAVITY;
        z += vz;
        if z <= floor {
            return n;
        }
    }
    MAX_FALL
}

/// Where sub-shot `k` of `count` lands off the parent's landing point: a sunflower over
/// a disc `radius` across, turned by `spin`.
pub(crate) fn offset(k: u8, count: u8, radius: Fx, spin: Angle) -> FxVec2 {
    let out = radius * Fx::ratio(2 * k as i64 + 1, 2 * count.max(1) as i64).sqrt();
    let turn = Angle(spin.0.wrapping_add(GOLDEN.wrapping_mul(k as u16)));
    FxVec2::from_angle(turn) * out
}

/// A turn from a shot's serial, so successive volleys do not lay the same pattern.
fn spin(serial: u32) -> Angle {
    Angle((serial.wrapping_mul(2_654_435_761) >> 16) as u16)
}

impl World {
    /// Breaks every whole cluster shot that has come down to its split height into its
    /// sub-shots. `SimError::TableFull` if the projectile table cannot take them.
    pub(crate) fn split_cluster_shots(&mut self) -> Result<(), SimError> {
        let blueprints = self.blueprints.clone();
        let split: Vec<usize> = {
            let p = &self.state.projectiles;
            (0..p.len())
                .filter(|&i| {
                    p.sub[i] == 0
                        && splits(
                            &blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize],
                            p.pos[i],
                            p.vel[i],
                            p.mark[i],
                        )
                })
                .collect()
        };
        for &i in &split {
            let p = &self.state.projectiles;
            let weapon = &blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            let Some(cluster) = weapon.cluster else {
                continue;
            };
            if p.len() + cluster.count as usize > MAX_PROJECTILES {
                return Err(SimError::TableFull(Table::Projectiles));
            }
            let (pos, vel, mark) = (p.pos[i], p.vel[i], p.mark[i]);
            let (owner, source, blueprint, slot) =
                (p.owner[i], p.source[i], p.blueprint[i], p.weapon[i]);
            let (age, origin, serial) = (p.age[i], p.origin[i], p.serial[i]);
            let full = weapon.casing_hp();
            let left = if p.hp[i] > Fx::ZERO { p.hp[i] } else { full };
            let casing = (left / cluster.count as i32).max(Fx::EPSILON);
            let n = fall_ticks(pos.z, vel.z, mark.z);
            let lands = pos.xy() + vel.xy() * Fx::from_int(n);
            let turn = spin(serial);
            for k in 0..cluster.count {
                let off = offset(k, cluster.count, cluster.radius, turn);
                let child = vel + FxVec2::new(off.x / n, off.y / n).extend(Fx::ZERO);
                self.state.projectiles.spawn(
                    pos,
                    child,
                    owner,
                    source,
                    blueprint,
                    slot,
                    (n + 20).clamp(1, u16::MAX as i32) as u16,
                )?;
                let p = &mut self.state.projectiles;
                let c = p.len() - 1;
                p.sub[c] = k + 1;
                p.age[c] = age;
                p.origin[c] = origin;
                p.hp[c] = casing;
                p.mark[c] = (lands + off).extend(mark.z);
            }
            self.events.push(SimEvent::ClusterSplit {
                pos,
                vel,
                count: cluster.count,
                owner,
                blueprint,
                weapon: slot,
            });
        }
        for &i in split.iter().rev() {
            self.state.projectiles.swap_remove(i);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fall_counts_ticks_the_way_shots_are_stepped() {
        // Dropped from rest 100 m up: g/2 n(n+1) >= 100.
        let n = fall_ticks(Fx::from_int(100), Fx::ZERO, Fx::ZERO);
        let fallen = |n: i32| GRAVITY * (n * (n + 1) / 2);
        assert!(fallen(n) >= Fx::from_int(100));
        assert!(fallen(n - 1) < Fx::from_int(100));
    }

    #[test]
    fn the_pattern_fills_the_disc_without_doubling_up() {
        let radius = Fx::from_int(30);
        let pts: Vec<FxVec2> = (0..8).map(|k| offset(k, 8, radius, Angle(1234))).collect();
        for (a, p) in pts.iter().enumerate() {
            assert!(p.length() <= radius + Fx::ONE, "{a} outside the disc");
            for q in &pts[a + 1..] {
                assert!(p.distance(*q) > Fx::from_int(5), "{a} too close to another");
            }
        }
        // The outermost reaches most of the way out.
        assert!(pts[7].length() > radius * Fx::ratio(8, 10));
    }
}
