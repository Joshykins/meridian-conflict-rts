//! Missile defence: each defender's lasers burn through the casings of the enemy missiles
//! in its reach (`UnitBlueprint::anti_missile`), each laser on a missile of its own from an
//! emitter of its own (`anti_missile_mounts`), measured from the nearest emitter.

use crate::mirror::SimEvent;
use crate::tables::{flag, FireState};
use crate::World;
use mc_core::{Fx, FxVec3};

/// Hit points an intercept laser burns off a missile each tick.
const LASER_BITE: Fx = Fx::from_int(10);
/// Ticks after a kill before the laser acquires another missile.
const LASER_GAP: u16 = 3;

impl World {
    /// One tick of every defender's lasers on the projectiles `casings` (rows of the
    /// projectile table that can be lased); those burnt through go onto `killed`.
    pub(crate) fn lase_missiles(&mut self, casings: &[usize], killed: &mut Vec<usize>) {
        let mut gone = vec![false; self.state.projectiles.len()];
        let defenders: Vec<_> = self
            .state
            .units
            .slots
            .iter()
            .filter(|&r| {
                self.state.units.is_active(r)
                    && self.state.units.health[r] > Fx::ZERO
                    && self.bp(r).anti_missile > Fx::ZERO
                    && !self.powered_down(r)
                    && !self.state.units.has_flag(r, flag::PASSIVE)
                    && self.state.units.fire_state[r] != FireState::HoldFire
            })
            .collect();
        for r in defenders {
            if self.state.units.intercept_cooldown[r] > 0 {
                self.state.units.intercept_cooldown[r] -= 1;
                continue;
            }
            let center = self.state.units.pos[r].extend(self.state.units.z[r]);
            let owner = self.state.units.owner[r];
            let radius = self.bp(r).anti_missile;
            // Its emitters in the world (`anti_missile_mounts`); none: its middle.
            let heading = self.state.units.heading[r];
            let emitters: Vec<FxVec3> = self
                .bp(r)
                .anti_missile_mounts
                .iter()
                .map(|m| center + m.xy().rotate(heading).extend(m.z))
                .collect();
            // How far a missile at `at` is from the nearest emitter: each reaches round
            // itself, so a long hull's lenses guard its whole length, not a circle round
            // its middle.
            let nearest = |at: FxVec3| {
                emitters
                    .iter()
                    .map(|e| (at - *e).length_sq())
                    .min()
                    .unwrap_or((at - center).length_sq())
            };
            if self.state.players[owner as usize].efficiency <= Fx::ZERO {
                continue;
            }
            // Each laser (`anti_missile_lasers`) takes a missile of its own from a mount
            // of its own.
            let mut lased: Vec<usize> = Vec::new();
            let mut used: Vec<FxVec3> = Vec::new();
            for _ in 0..self.bp(r).anti_missile_lasers {
                // Stay on a casing already burning. Otherwise the nearest full one.
                let mut best: Option<(usize, bool, Fx, Fx)> = None;
                for &i in casings {
                    if gone[i] || lased.contains(&i) {
                        continue;
                    }
                    let p = &self.state.projectiles;
                    let w = &self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
                    let max = w.casing_hp();
                    if max <= Fx::ZERO || !self.are_enemies(owner, p.owner[i]) {
                        continue;
                    }
                    let d2 = nearest(p.pos[i]);
                    if d2 > radius * radius {
                        continue;
                    }
                    let hp = if p.hp[i] > Fx::ZERO { p.hp[i] } else { max };
                    let burning = hp < max;
                    let take = match best {
                        None => true,
                        Some((_, was, best_hp, best_d)) => {
                            (burning && !was)
                                || (burning == was
                                    && (hp < best_hp || (hp == best_hp && d2 < best_d)))
                        }
                    };
                    if take {
                        best = Some((i, burning, hp, d2));
                    }
                }
                let Some((i, _, hp, _)) = best else {
                    break;
                };
                lased.push(i);
                let left = hp - LASER_BITE;
                let dead = left <= Fx::ZERO;
                let to = self.state.projectiles.pos[i];
                // A hull with emitters of its own fires from the one nearest the missile.
                let from = emitters
                    .iter()
                    .copied()
                    .filter(|at| !used.contains(at))
                    .min_by_key(|at| (*at - to).length_sq())
                    .unwrap_or(center);
                used.push(from);
                self.events.push(SimEvent::MissileLased {
                    from,
                    to,
                    killed: dead,
                    blueprint: self.state.units.blueprint[r],
                });
                if dead {
                    killed.push(i);
                    gone[i] = true;
                    self.state.units.intercept_cooldown[r] = LASER_GAP;
                } else {
                    self.state.projectiles.hp[i] = left;
                }
            }
        }
    }
}
