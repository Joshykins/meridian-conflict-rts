//! Naval rules: submarines diving and surfacing, sonar, and torpedoes.
//!
//! A naval hull's origin is its waterline. A dived submarine sits lower by its
//! height plus `Dive::depth`, so water closes over it (`World::submerged`).
//! Under water only sonar finds a hull and only a torpedo reaches one; a torpedo
//! in turn only strikes what floats in the water.

use crate::tables::*;
use crate::World;
use mc_core::{Angle, Fx, FxVec2, FxVec3, TICKS_PER_SECOND};
use mc_data::{MoveLayer, Weapon};

const DT: i32 = TICKS_PER_SECOND as i32;
/// Share of its height a hull reaches below the waterline: the keel. A dived
/// submarine keeps this much water under it, and a torpedo strikes this deep.
pub(crate) const KEEL: Fx = Fx::ratio(3, 5);
/// Metres under the surface a torpedo runs at when its mark floats.
const RUN_DEPTH: Fx = Fx::ratio(6, 5);
/// Shallowest a torpedo may be: a metre under the surface.
const CEILING: Fx = Fx::ONE;
/// Angle steps a torpedo turns in a tick: 75 degrees a second.
const TORPEDO_TURN: u16 = Angle::from_degrees(75).0 / DT as u16;
/// Longest lead a torpedo takes on a moving mark, in ticks.
const LEAD_TICKS: i32 = 20;
/// Metres within which an air-dropped torpedo's own seeker holds its mark, once
/// the aircraft that dropped it (and its sonar) has flown on.
const SEEKER: Fx = Fx::from_int(260);
/// Share of its way a falling torpedo keeps each tick against the air.
const AIR_DRAG: Fx = Fx::ratio(49, 50);

/// Where a torpedo leaves its tube, its first step and its life in ticks. It
/// drops into the water under the tube and runs out along it, level; it steers
/// onto its mark from the next tick (`World::steer_torpedoes`). It lives long
/// enough to run half as far again as its range, for the turns on the way, and
/// bursts when that runs out. Fired at a `point`, it runs straight there and bursts.
///
/// Dropped from an aircraft (`carried` is the aircraft's way per tick) it falls
/// from the muzzle, keeping that way, and only starts to run once it is in the
/// water (`World::steer_torpedoes`); its life gains the fall.
pub(crate) fn torpedo_launch(
    muzzle: FxVec3,
    facing: Angle,
    weapon: &Weapon,
    water: Fx,
    point: Option<FxVec2>,
    carried: FxVec2,
) -> (FxVec3, FxVec3, i32) {
    let step = weapon.projectile_speed / DT;
    if muzzle.z > water {
        // Ticks to fall to the water: h = g n^2 / 2.
        let fall = ((muzzle.z - water) * 2 / crate::combat::GRAVITY)
            .sqrt()
            .ceil_int()
            + 1;
        let run = match point {
            Some(point) => {
                let landing = muzzle.xy() + carried * Fx::from_int(fall);
                (point.distance(landing) / step.max(Fx::EPSILON))
                    .ceil_int()
                    .max(1)
            }
            None => (weapon.range_max * Fx::ratio(3, 2) / step.max(Fx::EPSILON)).ceil_int() + DT,
        };
        return (muzzle, carried.extend(Fx::ZERO), fall + run);
    }
    let at = FxVec3::new(muzzle.x, muzzle.y, muzzle.z.min(water - CEILING));
    if let Some(point) = point {
        // Fired at a point: straight there, bursting on arrival.
        let ticks = (point.distance(at.xy()) / step.max(Fx::EPSILON))
            .ceil_int()
            .max(1);
        let dir = (point - at.xy()).normalize();
        let dir = if dir == FxVec2::ZERO {
            FxVec2::from_angle(facing)
        } else {
            dir
        };
        return (at, (dir * step).extend(Fx::ZERO), ticks);
    }
    let ticks = (weapon.range_max * Fx::ratio(3, 2) / step.max(Fx::EPSILON)).ceil_int() + DT;
    (
        at,
        (FxVec2::from_angle(facing) * step).extend(Fx::ZERO),
        ticks,
    )
}

impl World {
    /// Whether `row` floats in the water: a naval hull, or a structure standing on open water.
    pub(crate) fn in_water(&self, row: usize) -> bool {
        let bp = self.bp(row);
        match bp.motion {
            Some(m) => m.layer == MoveLayer::Naval,
            None => {
                bp.water_build
                    && self.terrain.height_at(self.state.units.pos[row])
                        < self.terrain.water_level()
            }
        }
    }

    /// Sonar range this unit listens over right now. Like radar, a station that
    /// draws energy is deaf while the grid cannot pay.
    pub(crate) fn live_sonar(&self, row: usize) -> Fx {
        let bp = self.bp(row);
        if bp.sonar <= Fx::ZERO || !self.state.units.is_active(row) {
            return Fx::ZERO;
        }
        if bp.economy.energy_upkeep > Fx::ZERO && self.energy_stalling(self.state.units.owner[row])
        {
            Fx::ZERO
        } else {
            bp.sonar
        }
    }

    /// Whether the players in `mask` detect `row` with vision, radar or sonar.
    /// A hull under water answers to sonar alone, except for the few seconds after a
    /// missile launch gave it away (`Units::revealed`), when radar and eyes find it too;
    /// sonar also picks up anything floating, the way radar would.
    pub(crate) fn detected_by(&self, row: usize, mask: u8) -> bool {
        let pos = self.state.units.pos[row];
        if self.submerged(row) {
            return self.fog.is_sonar(pos, mask)
                || (self.state.units.revealed[row] > 0 && self.fog.is_detected(pos, mask));
        }
        self.fog.is_detected(pos, mask) || (self.fog.is_sonar(pos, mask) && self.in_water(row))
    }

    /// Whether the players in `mask` see `row` with their eyes. Nobody sees under water.
    pub(crate) fn seen_by(&self, row: usize, mask: u8) -> bool {
        !self.submerged(row) && self.fog.is_visible(self.state.units.pos[row], mask)
    }

    /// Whether `weapon` can strike `target`: the right kind, and on the right side of
    /// the surface. A torpedo strikes only what is in the water, dived or not; every
    /// other weapon only what is not under it.
    pub(crate) fn weapon_reaches(&self, target: usize, weapon: &Weapon) -> bool {
        // A torpedo takes a walker on the seabed whatever it is (`seabed.rs`).
        (self.target_layers(target) & weapon.target_mask != 0
            || (weapon.torpedo && self.on_seabed(target)))
            && if weapon.torpedo {
                self.torpedo_can_mark(target)
            } else {
                !self.submerged(target)
            }
    }

    /// Whether any of `shooter`'s weapons can strike `target`. Interceptor tubes
    /// strike no unit, and a surfaced-only gun nothing while its hull is under.
    pub(crate) fn can_strike(&self, shooter: usize, target: usize) -> bool {
        let dived = self.submerged(shooter);
        self.bp(shooter)
            .weapons
            .iter()
            .any(|w| !w.intercepts && !(w.surfaced && dived) && self.weapon_reaches(target, w))
    }

    /// Whether hulls `a` and `b` pass one over the other instead of bumping: both ride
    /// the water, and one is dived while the other floats, or their spans from keel to
    /// top do not meet. A dived submarine's two metres of water do not clear a frigate's
    /// keel, so being dived is enough on its own; two dived hulls still bump.
    pub(crate) fn hulls_pass(&self, a: usize, b: usize) -> bool {
        let naval = |r: usize| {
            self.bp(r)
                .motion
                .is_some_and(|m| m.layer == MoveLayer::Naval)
        };
        if !naval(a) || !naval(b) {
            return false;
        }
        if self.submerged(a) != self.submerged(b) {
            return true;
        }
        let units = &self.state.units;
        let span = |r: usize| {
            let h = self.bp(r).height;
            (units.z[r] - h * KEEL, units.z[r] + h)
        };
        let ((a0, a1), (b0, b1)) = (span(a), span(b));
        a1 < b0 || b1 < a0
    }

    /// The waterline height of a submarine over `pos`, as far down as its dive has
    /// gone. None for anything that does not dive. Fully down it keeps a keel's
    /// depth of water under it, so over shallows it cannot get all the way under.
    pub(crate) fn dive_z(&self, row: usize, pos: FxVec2) -> Option<Fx> {
        let bp = self.bp(row);
        let dive = bp.dive?;
        let water = self.terrain.water_level();
        let ground = self.terrain.height_at(pos);
        let surface = ground.max(water);
        let down = (water - bp.height - dive.depth)
            .max(ground + bp.height * KEEL)
            .min(surface);
        let share = Fx::ratio(self.state.units.dive[row] as i64, 255);
        Some(surface + (down - surface) * share)
    }

    /// Submarines go down or come up toward their ordered depth, a full dive
    /// taking `Dive::ticks`. Movement then sets the hull's height (`dive_z`).
    pub(crate) fn run_dive(&mut self) {
        let units = &mut self.state.units;
        for row in 0..units.slots.rows() {
            if !units.slots.is_alive(row) || !units.is_active(row) {
                continue;
            }
            // A launch that gave the hull away fades (`naval_arms.rs`).
            units.revealed[row] = units.revealed[row].saturating_sub(1);
            let Some(dive) = self.blueprints.unit(units.blueprint[row]).dive else {
                continue;
            };
            let step = 255u16.div_ceil(dive.ticks.max(1)).min(255) as u8;
            units.dive[row] = if units.dive_goal[row] {
                units.dive[row].saturating_add(step)
            } else {
                units.dive[row].saturating_sub(step)
            };
        }
    }

    /// `Command::SetDive`: the player's submarines among `ids` go down or come up.
    pub(crate) fn set_dive(&mut self, player: u8, ids: &[UnitId], dive: bool) {
        for row in self.owned(player, ids, 0) {
            if self.bp(row).dive.is_some() {
                self.state.units.dive_goal[row] = dive;
            }
        }
    }

    /// When torpedo `i` bursts with no hull to hit, as a share of this tick's step: at
    /// once when the mark it was fired at is gone, dead, out of the water or no longer
    /// detected; at the end of the step on its last tick of running. One fired at a
    /// point in the water runs out there (`torpedo_launch`).
    pub(crate) fn torpedo_burst(&self, i: usize, weapon: &Weapon) -> Option<Fx> {
        if !weapon.torpedo {
            return None;
        }
        // An interceptor whose quarry is gone bursts where it is (`naval_arms.rs`).
        if self.quarry_lost(i) {
            return Some(Fx::ZERO);
        }
        let p = &self.state.projectiles;
        if p.target[i] != Handle::NONE {
            let units = &self.state.units;
            // One dropped from the air homes by itself on a mark close enough to hear.
            let dropped = self
                .blueprints
                .unit(p.blueprint[i])
                .motion
                .is_some_and(|m| m.layer == MoveLayer::Air);
            let lost = units.row(p.target[i]).is_none_or(|t| {
                units.health[t] <= Fx::ZERO
                    || !self.torpedo_can_mark(t)
                    || (!self.detects(p.owner[i], t)
                        && !(dropped && units.pos[t].distance(p.pos[i].xy()) <= SEEKER))
            });
            if lost {
                return Some(Fx::ZERO);
            }
        }
        (p.ticks_left[i] <= 1).then_some(Fx::ONE)
    }

    /// Torpedoes run level under the water at their own speed, turning onto a lead
    /// on their mark and easing to its depth: just under the surface for a floating
    /// hull, the middle of a dived one. With no mark left they run on straight.
    pub(crate) fn steer_torpedoes(&mut self) {
        let blueprints = self.blueprints.clone();
        let water = self.terrain.water_level();
        for i in 0..self.state.projectiles.len() {
            let p = &self.state.projectiles;
            let weapon = &blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize];
            if !weapon.torpedo {
                continue;
            }
            let step = weapon.projectile_speed / DT;
            let pos = p.pos[i];
            if pos.z > water {
                // Still falling from the aircraft that dropped it: no steering in the air.
                let fall = p.vel[i];
                let vel = (fall.xy() * AIR_DRAG).extend(fall.z - crate::combat::GRAVITY);
                let p = &mut self.state.projectiles;
                p.vel[i] = vel;
                p.aim[i] = vel.normalize();
                continue;
            }
            let heading = p.vel[i].xy().angle();
            let units = &self.state.units;
            let mark = units
                .row(p.target[i])
                .filter(|&t| units.health[t] > Fx::ZERO && self.torpedo_can_mark(t))
                .map(|t| {
                    let lead = (units.pos[t].distance(pos.xy()) / step.max(Fx::EPSILON))
                        .min(Fx::from_int(LEAD_TICKS));
                    let aim = units.pos[t] + (units.pos[t] - units.prev_pos[t]) * lead;
                    let depth = if self.submerged(t) {
                        // Halfway between the keel and the top of the hull.
                        units.z[t] + self.bp(t).height * (Fx::ONE - KEEL) / 2
                    } else {
                        water - RUN_DEPTH
                    };
                    ((aim - pos.xy()).angle(), depth)
                });
            let (want, depth) = mark.unwrap_or((heading, pos.z));
            let heading = heading.turn_toward(want, TORPEDO_TURN);
            let climb = ((depth - pos.z) / 4)
                .clamp(-step / 3, step / 3)
                .min(water - CEILING - pos.z);
            let vel = (FxVec2::from_angle(heading) * step).extend(climb);
            let p = &mut self.state.projectiles;
            p.vel[i] = vel;
            p.aim[i] = vel.normalize();
        }
    }
}
