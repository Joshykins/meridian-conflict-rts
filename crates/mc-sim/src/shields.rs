//! Shield generators and hull wraps: open and close the field, regenerate it,
//! and drop it when the economy stalls. Every shield, dome or hull wrap, runs
//! on its side's energy: while the grid cannot pay, it is down. An in-place
//! refit keeps a dome up.
//!
//! A shattered dome starts filling at once, stays down while it fills, and
//! only rises again at full charge. Engineers can feed a live (damaged)
//! bubble extra regen for energy; they cannot hurry that recovery.

use crate::mirror::SimEvent;
use crate::World;
use mc_core::{Fx, FxVec3, TICKS_PER_SECOND};

const DT: i32 = TICKS_PER_SECOND as i32;
/// How far `shield_open` moves toward open each tick (~2 s for a full rise).
const OPEN_STEP: u8 = 13;
/// Closing on a stall is a little quicker than opening.
const CLOSE_STEP: u8 = 16;
/// A shattered dome peels in about 0.6 s: fast enough to read as a break,
/// slow enough that the glass is seen coming apart instead of popping off.
const BREAK_STEP: u8 = 42;
/// The bubble only stops shots once it is mostly there.
pub(crate) const SHIELD_BLOCKING_OPEN: u8 = 200;
/// A broken dome refills this many times faster than a live one regenerates,
/// so a generator is not out of the fight for long after it blips.
const BREAK_REGEN_MUL: i32 = 2;
/// Stored energy below this counts as empty. A stall pays each spender its share
/// rounded down, so a starved grid is left holding a few raw units, never zero.
const DRY: Fx = Fx::ONE;
/// Ticks an upgraded dome takes to swell out to its new radius once the refit is
/// done. It holds its old size while the upgrade is built.
pub(crate) const SHIELD_GROW_TICKS: u8 = 25;

impl World {
    /// A live, mostly-open field that still has hit points.
    pub(crate) fn shield_blocking(&self, row: usize) -> bool {
        self.bp(row).shield.is_some()
            && self.state.units.is_active(row)
            && self.state.units.shield_open[row] >= SHIELD_BLOCKING_OPEN
            && self.state.units.shield_hp[row] > Fx::ZERO
            && self.state.units.shield_recharge[row] == 0
            && !self.shield_off(row)
    }

    /// The dome's radius now: its blueprint's, or on the way there from the old
    /// one after an upgrade (`shield_grow`). Zero without a shield.
    pub(crate) fn dome_radius(&self, row: usize) -> Fx {
        self.dome_radius_at(row, self.state.units.shield_grow[row])
    }

    /// How low the wall under a dome's rim reaches: the sea surface. Below it the
    /// ground stops everything first (`combat::in_dome`).
    pub(crate) fn dome_floor(&self) -> Fx {
        self.terrain.water_level()
    }

    /// [`Self::dome_radius`] with `grow` ticks left, eased at both ends.
    pub(crate) fn dome_radius_at(&self, row: usize, grow: u8) -> Fx {
        let Some(spec) = self.bp(row).shield else {
            return Fx::ZERO;
        };
        if grow == 0 {
            return spec.radius;
        }
        let from = self.state.units.shield_from[row];
        let t = Fx::ratio(
            SHIELD_GROW_TICKS.saturating_sub(grow) as i64,
            SHIELD_GROW_TICKS as i64,
        );
        let ease = t * t * (Fx::from_int(3) - t * 2);
        from + (spec.radius - from) * ease
    }

    pub(crate) fn energy_stalling(&self, player: u8) -> bool {
        let p = &self.state.players[player as usize];
        !p.free_build && p.energy < DRY && p.efficiency < Fx::ONE
    }

    /// The side cannot pay its upkeep: every shield it owns is down. Upkeep is
    /// paid before building, so a side short only on construction keeps them.
    pub(crate) fn shields_unpowered(&self, player: u8) -> bool {
        let p = &self.state.players[player as usize];
        !p.free_build && p.energy < DRY && p.upkeep_efficiency < Fx::ONE
    }

    /// This unit's shield has no power: its side cannot pay upkeep, or it is paused.
    pub(crate) fn shield_off(&self, row: usize) -> bool {
        self.powered_down(row) || self.shields_unpowered(self.state.units.owner[row])
    }

    fn shield_wants_up(&self, row: usize) -> bool {
        self.bp(row).shield.is_some()
            && self.state.units.is_active(row)
            && self.state.units.shield_recharge[row] == 0
            && !self.shield_off(row)
    }

    /// A live dome that is missing charge: engineers may pump extra regen into
    /// it. A shattered bubble filling while down is not this.
    pub(crate) fn shield_assistable(&self, row: usize) -> bool {
        let Some(spec) = self.bp(row).shield else {
            return false;
        };
        let hp = self.state.units.shield_hp[row];
        self.state.units.is_active(row)
            && self.state.units.health[row] >= self.unit_max_health(row)
            && self.state.units.shield_recharge[row] == 0
            && hp > Fx::ZERO
            && hp < spec.health
            && !self.powered_down(row)
            && !self.energy_stalling(self.state.units.owner[row])
    }

    /// Build-time units left to restore a live dome, so several engineers share
    /// what remains the same way they share a repair.
    pub(crate) fn shield_work_remaining(&self, row: usize) -> Fx {
        let Some(spec) = self.bp(row).shield else {
            return Fx::ZERO;
        };
        let missing = spec.health - self.state.units.shield_hp[row];
        if missing > Fx::ZERO {
            let time = self.bp(row).build_time;
            (missing * time / spec.health.max(Fx::ONE)).max(Fx::EPSILON)
        } else {
            Fx::ZERO
        }
    }

    /// Raise, lower and regenerate every bubble. Called after the economy so a
    /// stall this tick drops shields the same tick, and so engineer assist
    /// lands before the generator's own regen.
    pub(crate) fn run_shields(&mut self) {
        let rows: Vec<usize> = self.state.units.slots.iter().collect();
        for row in rows {
            let Some(spec) = self.bp(row).shield else {
                continue;
            };
            if !self.state.units.is_active(row) {
                continue;
            }
            let grow = &mut self.state.units.shield_grow[row];
            *grow = grow.saturating_sub(1);
            if self.state.units.shield_recharge[row] > 0 {
                self.recover_shield(row, spec.health, spec.regen);
            }
            let want = self.shield_wants_up(row);
            let open = self.state.units.shield_open[row];
            self.state.units.shield_open[row] = if want {
                open.saturating_add(OPEN_STEP)
            } else if self.state.units.shield_recharge[row] > 0 {
                open.saturating_sub(BREAK_STEP)
            } else {
                open.saturating_sub(CLOSE_STEP)
            };
            if !want || self.state.units.shield_open[row] < SHIELD_BLOCKING_OPEN {
                continue;
            }
            let hp = self.state.units.shield_hp[row];
            if hp < spec.health {
                self.state.units.shield_hp[row] = (hp + spec.regen / DT).min(spec.health);
            }
        }
    }

    /// Fill after a break at `BREAK_REGEN_MUL` times the live rate. The dome
    /// stays down until it is full; a stall pauses the fill.
    /// `shield_recharge` stays set until then.
    fn recover_shield(&mut self, row: usize, max: Fx, regen: Fx) {
        if self.shield_off(row) {
            return;
        }
        let hp = self.state.units.shield_hp[row];
        if hp < max {
            self.state.units.shield_hp[row] = (hp + regen * BREAK_REGEN_MUL / DT).min(max);
        }
        if self.state.units.shield_hp[row] >= max {
            self.state.units.shield_recharge[row] = 0;
        }
    }

    pub(crate) fn arm_shield(&mut self, row: usize, instant: bool) {
        let Some(spec) = self.bp(row).shield else {
            return;
        };
        self.state.units.shield_hp[row] = spec.health;
        self.state.units.shield_recharge[row] = 0;
        let open = if instant { 255 } else { 0 };
        self.state.units.shield_open[row] = open;
        self.state.units.prev_shield_open[row] = open;
    }

    /// An upgrade that just finished keeps its parent's bubble: same opening,
    /// same charge delay, hit points scaled onto the new dome.
    pub(crate) fn inherit_shield(&mut self, child: usize) {
        if self.bp(child).shield.is_none() {
            return;
        }
        let id = self.state.units.id(child);
        let Some(parent) = self
            .state
            .units
            .slots
            .iter()
            .find(|&row| self.state.units.build_target[row] == id)
        else {
            self.arm_shield(child, false);
            return;
        };
        let Some(old) = self.bp(parent).shield else {
            self.arm_shield(child, false);
            return;
        };
        let spec = self.bp(child).shield.unwrap();
        let ratio =
            (self.state.units.shield_hp[parent] / old.health.max(Fx::ONE)).clamp(Fx::ZERO, Fx::ONE);
        self.state.units.shield_hp[child] = spec.health * ratio;
        self.state.units.shield_recharge[child] = self.state.units.shield_recharge[parent];
        self.state.units.shield_open[child] = self.state.units.shield_open[parent];
        self.state.units.prev_shield_open[child] = self.state.units.prev_shield_open[parent];
    }

    pub(crate) fn break_shield(&mut self, row: usize) {
        let Some(spec) = self.bp(row).shield else {
            return;
        };
        let units = &mut self.state.units;
        units.shield_hp[row] = Fx::ZERO;
        // Shots already pass (`hp` and `recharge`). Peel the glass this tick so
        // the collapse is seen interpolating, not snapped off between frames.
        units.shield_open[row] = units.shield_open[row].saturating_sub(BREAK_STEP * 2);
        units.shield_recharge[row] = 1;
        self.events.push(SimEvent::ShieldBroken {
            pos: units.pos[row].extend(units.z[row]),
            radius: spec.radius,
            owner: units.owner[row],
        });
    }

    pub(crate) fn damage_shield(&mut self, row: usize, damage: Fx) {
        if !self.shield_blocking(row) || damage <= Fx::ZERO {
            return;
        }
        // Survival's veil: it takes every hit and gives nothing.
        if self
            .state
            .units
            .has_flag(row, crate::tables::flag::INVULNERABLE)
        {
            return;
        }
        let hp = self.state.units.shield_hp[row];
        self.state.units.shield_hp[row] = hp - damage.min(hp);
        if self.state.units.shield_hp[row] <= Fx::ZERO {
            self.break_shield(row);
        }
    }
}

/// `offset` from a dome's centre with its height stretched to `radius`, so the
/// flattened dome (`mc_data::dome_height`) is a sphere of `radius` in this space.
fn dome_space(offset: FxVec3, radius: Fx) -> FxVec3 {
    let height = mc_data::dome_height(radius).max(Fx::EPSILON);
    FxVec3::new(offset.x, offset.y, offset.z * radius / height)
}

/// True when `p` is inside the dome at `center` with `radius`: under its cap, or
/// below the rim within the radius and above `floor`. A dome on high ground drops a
/// wall from its rim to the ground, so what stands under the cliff it sits on is
/// covered too. `floor` is the sea surface (`World::dome_floor`): the wall stops
/// there, and a torpedo still runs under it.
pub(crate) fn in_dome(p: FxVec3, center: FxVec3, radius: Fx, floor: Fx) -> bool {
    let offset = p - center;
    if p.z >= center.z {
        dome_space(offset, radius).length_sq() <= radius * radius
    } else {
        p.z >= floor && offset.xy().length_sq() <= radius * radius
    }
}

/// First time the segment `from + vel * t` (t in 0..=1) meets the dome at `center`
/// with `radius`: its cap, or the wall under the rim down to `floor` (`in_dome`).
/// None when it misses.
pub(crate) fn ray_dome(
    from: FxVec3,
    vel: FxVec3,
    center: FxVec3,
    radius: Fx,
    floor: Fx,
) -> Option<Fx> {
    let r2 = radius * radius;
    let (oc, vel_d) = (dome_space(from - center, radius), dome_space(vel, radius));
    let cap = quadratic_roots(vel_d.length_sq(), vel_d.dot(oc), oc.length_sq() - r2);
    let (oc, vel_xy) = ((from - center).xy(), vel.xy());
    let wall = quadratic_roots(vel_xy.length_sq(), vel_xy.dot(oc), oc.length_sq() - r2);
    let z = |t: Fx| (from + vel * t).z;
    let on_cap = cap.into_iter().flatten().filter(|&t| z(t) >= center.z);
    let on_wall = (wall.into_iter().flatten()).filter(|&t| z(t) < center.z && z(t) >= floor);
    on_cap
        .chain(on_wall)
        .filter(|&t| t >= Fx::ZERO && t <= Fx::ONE)
        .min()
}

/// Both roots of `a t^2 + 2 half_b t + c = 0`, least first. None when `a` is
/// about zero or there is no real root.
fn quadratic_roots(a: Fx, half_b: Fx, c: Fx) -> Option<[Fx; 2]> {
    if a <= Fx::EPSILON {
        return None;
    }
    let disc = half_b * half_b - a * c;
    if disc < Fx::ZERO {
        return None;
    }
    let root = disc.sqrt();
    Some([(-half_b - root) / a, (-half_b + root) / a])
}

#[cfg(test)]
mod tests {
    use super::{in_dome, ray_dome};
    use mc_core::{Fx, FxVec3};

    fn at(x: i32, z: i32) -> FxVec3 {
        FxVec3::new(Fx::from_int(x), Fx::ZERO, Fx::from_int(z))
    }

    #[test]
    fn a_dome_on_a_cliff_covers_the_ground_under_its_rim() {
        // A 100 m dome projected 60 m up a cliff, sea at 0.
        let (center, radius, floor) = (at(0, 60), Fx::from_int(100), Fx::ZERO);
        assert!(in_dome(at(80, 5), center, radius, floor), "under the cliff");
        assert!(in_dome(at(0, 100), center, radius, floor), "under the cap");
        assert!(
            !in_dome(at(120, 5), center, radius, floor),
            "outside the wall"
        );
        assert!(!in_dome(at(80, -5), center, radius, floor), "under the sea");
        // A shot along the lowland is stopped at the wall, at x = -100.
        let t = ray_dome(at(-150, 5), at(100, 0), center, radius, floor).unwrap();
        assert!((t - Fx::ratio(1, 2)).abs() < Fx::ratio(1, 100), "{t:?}");
        // One passing under the sea is not.
        assert!(ray_dome(at(-150, -5), at(100, 0), center, radius, floor).is_none());
        // Nor one from inside going deeper in.
        assert!(ray_dome(at(80, 5), at(-20, 0), center, radius, floor).is_none());
        // The cap still stops fire from above.
        assert!(ray_dome(at(0, 200), at(0, -100), center, radius, floor).is_some());
    }
}
