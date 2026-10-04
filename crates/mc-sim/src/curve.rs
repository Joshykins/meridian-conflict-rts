//! Thrown charges that curve onto their mark (`Weapon::curve`, the Regency's Gravitic Bomb).
//!
//! A salvo's shots leave fanned out off the line to the mark: the first swung out to the
//! thrower's side of it, the last across to the other, the middle ones lofted over it.
//! Each then bends back onto its target at its own speed, turning harder the nearer it
//! gets, so every one of them arrives, and each from its own angle: they come in on the
//! mark from around it rather than down one line. A shot that has passed its mark stops
//! turning and flies on through, until it strikes something or runs out.
//!
//! No gravity, no randomness: the path follows from where it left, the target and the
//! shot's number in its salvo.

use mc_core::{Angle, Fx, FxVec2, FxVec3};
use mc_data::Weapon;

use crate::World;

/// How hard a curving charge turns onto its mark: each tick it swings this many times
/// a tick's flight over the distance left of the way from its heading to the mark. At one
/// its heading's error falls off with the distance left, so it closes on the mark in a
/// steady arc and never circles; more pulls it in sooner and straighter.
const PULL: i32 = 1;
/// Of the fan's angle, how much every shot is lofted (quarters), and how much more the
/// middle of the salvo is (quarters again, times how near the middle it is).
const LOFT_ALL: i32 = 1;
const LOFT_MIDDLE: i32 = 3;

/// Which way shot `tube` of a curving `weapon`'s salvo leaves a muzzle at `muzzle`, thrown
/// at `aim`: fanned off the line to it by the weapon's `curve`, to the side the gun stands
/// on (`side`: the muzzle's own side of the unit, positive left) first.
pub(crate) fn launch_dir(
    weapon: &Weapon,
    muzzle: FxVec3,
    aim: FxVec3,
    tube: usize,
    side: Fx,
) -> FxVec3 {
    let to = aim - muzzle;
    if weapon.corkscrew > Fx::ZERO {
        return to.normalize();
    }
    let bearing = to.xy().angle();
    let elevation = FxVec2::new(to.xy().length(), to.z).angle();
    let fan = weapon.curve.0 as i32;
    let salvo = weapon.salvo.max(1) as i32;
    // From +1 (the first) to -1 (the last) across the salvo, in thousandths.
    let along = if salvo > 1 {
        1000 - 2000 * (tube as i32 % salvo) / (salvo - 1)
    } else {
        0
    };
    let toward = if side < Fx::ZERO { -1 } else { 1 };
    let yaw = fan * along * toward / 1000;
    let loft = fan * LOFT_ALL / 4 + fan * LOFT_MIDDLE * (1000 - along.abs()) / 4000;
    let up = FxVec2::from_angle(elevation + Angle(loft as i16 as u16));
    (FxVec2::from_angle(bearing + Angle(yaw as i16 as u16)) * up.x).extend(up.y)
}

/// Ticks a curving shot may fly: its reach twice over, and a margin.
pub(crate) fn flight_ticks(weapon: &Weapon, step: Fx) -> i32 {
    (weapon.range_max * 2 / step.max(Fx::ONE)).ceil_int() + 20
}

impl World {
    /// Turns every curving charge (`Weapon::curve`) onto its mark: the middle of the unit
    /// it was thrown at while that stands, else the point it was thrown at.
    pub(crate) fn steer_curving_shots(&mut self) {
        let blueprints = self.blueprints.clone();
        for i in 0..self.state.projectiles.len() {
            let p = &self.state.projectiles;
            let weapon = &blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            if weapon.curve.0 == 0 {
                continue;
            }
            if weapon.corkscrew > Fx::ZERO {
                let next = corkscrew_next(
                    p.origin[i],
                    p.mark[i],
                    p.pos[i],
                    p.age[i],
                    p.serial[i],
                    weapon.projectile_speed / mc_core::TICKS_PER_SECOND as i32,
                    weapon.corkscrew,
                );
                let velocity = next - p.pos[i];
                self.state.projectiles.aim[i] = velocity.normalize();
                self.state.projectiles.vel[i] = velocity;
                continue;
            }
            let units = &self.state.units;
            let mark = units
                .row(p.target[i])
                .filter(|&t| units.health[t] > Fx::ZERO)
                .map_or(p.mark[i], |t| {
                    units.pos[t].extend(units.z[t] + self.bp(t).height / 2)
                });
            let step = p.vel[i].length();
            let flight = p.vel[i].normalize();
            let to = mark - p.pos[i];
            // Past it (or on it): no more turning, it flies on through.
            if flight == FxVec3::ZERO || to.dot(flight) <= Fx::ZERO {
                continue;
            }
            let dist = to.length().max(Fx::EPSILON);
            let pull = (step * PULL / dist).min(Fx::ONE);
            let dir = (flight * (Fx::ONE - pull) + to.normalize() * pull).normalize();
            if dir != FxVec3::ZERO {
                self.state.projectiles.aim[i] = dir;
                self.state.projectiles.vel[i] = dir * step;
            }
        }
    }
}

/// A helix about the immutable launch-to-area line. The first tick is straight;
/// the orbit opens smoothly above the firing line, rotates, then closes over the
/// last quarter of the flight. Its lower turns clear the ground. Projecting
/// onto the axis makes progress independent of the lateral orbit and of tick rate.
fn corkscrew_next(
    origin: FxVec3,
    mark: FxVec3,
    pos: FxVec3,
    age: u16,
    serial: u32,
    step: Fx,
    radius: Fx,
) -> FxVec3 {
    let line = mark - origin;
    let length = line.length().max(Fx::ONE);
    let axis = line.normalize();
    let run = ((pos - origin).dot(axis) + step).min(length + step);
    let forward = origin + axis * run;
    let lateral = FxVec3::new(axis.y, -axis.x, Fx::ZERO).normalize();
    let up = FxVec3::new(
        lateral.y * axis.z,
        -lateral.x * axis.z,
        lateral.x * axis.y - lateral.y * axis.x,
    )
    .normalize();
    let opening = Fx::ratio(age.min(3) as i64, 3);
    let opening = opening * opening * (Fx::from_int(3) - opening * 2);
    let closing = ((length - run) / (length / 4).max(step)).clamp(Fx::ZERO, Fx::ONE);
    let phase = Angle::from_degrees(age as i32 * 80 + (serial % 6) as i32 * 60);
    forward + (lateral * phase.cos() + up * (Fx::ONE + phase.sin())) * (radius * opening * closing)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helix_leaves_straight_rotates_and_closes_onto_the_area() {
        for (distance, step) in [(90, 18), (240, 18)] {
            let origin = FxVec2::from_ints(0, 0).extend(Fx::from_int(20));
            let mark = FxVec2::from_ints(distance, 0).extend(Fx::ZERO);
            let axis = (mark - origin).normalize();
            let mut pos = origin;
            let mut sides = [false; 2];
            let mut high = Fx::ZERO;
            for age in 0..45 {
                let next = corkscrew_next(
                    origin,
                    mark,
                    pos,
                    age,
                    1,
                    Fx::from_int(step),
                    Fx::from_int(18),
                );
                let offset = next - origin - axis * (next - origin).dot(axis);
                if age == 0 {
                    assert!(offset.length() < Fx::ratio(1, 20));
                }
                sides[0] |= offset.y > Fx::from_int(6);
                sides[1] |= offset.y < Fx::from_int(-6);
                high = high.max(offset.z.abs());
                pos = next;
                if (pos - origin).dot(axis) >= (mark - origin).length() {
                    break;
                }
            }
            assert!(sides.into_iter().all(|v| v));
            assert!(high > Fx::from_int(6));
            assert!(pos.distance(mark) <= Fx::from_int(step + 1));
        }
    }
}
