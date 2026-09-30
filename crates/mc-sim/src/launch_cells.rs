//! Missiles launched out of hatched cells (the Skyguard, the Atoll): the hatches open before a
//! salvo and shut while the cells reload (`Weapon::hatch_ticks`, counted in
//! `Units::deploy`); each missile is boosted straight up out of its cell, coasts while
//! its thrusters turn it over onto its mark, and only then lights its motor
//! (`Weapon::boost_ticks`); and a salvo spreads its missiles over the targets in range
//! (`Weapon::split`).

use crate::World;
use mc_core::{Fx, FxVec3};
use mc_data::{UnitBlueprint, Weapon};

/// Ticks the last missile of a salvo is given to climb clear before the hatches shut.
const CLEAR_TICKS: u16 = 5;

/// Ticks a unit's cell hatches take to open: those of its first weapon that has them.
pub(crate) fn hatch_ticks(bp: &UnitBlueprint) -> Option<u16> {
    bp.weapons.iter().map(|w| w.hatch_ticks).find(|&h| h > 0)
}

/// A boosted missile's climb this tick at `age`, metres a tick: the booster drives it up
/// to a sixth of its cruise `step`; spent, it coasts, slowing to a fifth of that by the time
/// its motor lights.
pub(crate) fn boost_climb(weapon: &Weapon, age: u16, step: Fx) -> Fx {
    let (boost, cold) = (weapon.boost_ticks, weapon.cold_launch_ticks);
    let top = step / 6;
    if age < boost {
        return top * Fx::ratio(age as i64 + 1, boost as i64);
    }
    let span = cold.saturating_sub(boost).max(1);
    let along = age.saturating_sub(boost).min(span);
    top - top * Fx::ratio(4 * along as i64, 5 * span as i64)
}

/// Nose and velocity of a boosted missile `age` ticks out, before its motor lights: nose
/// up on the booster, then eased over onto `desired` across the coast. `None`: not a
/// boosted missile, or its motor is already alight.
pub(crate) fn boosted(
    weapon: &Weapon,
    age: u16,
    desired: FxVec3,
    step: Fx,
) -> Option<(FxVec3, FxVec3)> {
    let (boost, cold) = (weapon.boost_ticks, weapon.cold_launch_ticks);
    if boost == 0 || age >= cold {
        return None;
    }
    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
    let span = cold.saturating_sub(boost).max(1);
    let along = age.saturating_sub(boost).min(span);
    let u = Fx::ratio(along as i64, span as i64);
    let ease = u * u * (Fx::from_int(3) - u * Fx::from_int(2));
    let pitched = (up * (Fx::ONE - ease) + desired * ease).normalize();
    let nose = if pitched.length_sq() > Fx::ZERO {
        pitched
    } else {
        desired
    };
    Some((nose, up * boost_climb(weapon, age, step)))
}

impl World {
    /// Opens a cell launcher's hatches a tick while a salvo is due (it has a target and
    /// its reload is nearly done), under way, or its last missile is still climbing out;
    /// shuts them a tick otherwise.
    pub(crate) fn step_hatches(&mut self, row: usize) {
        let bp = self.bp(row);
        let Some(w) = bp.weapons.iter().position(|w| w.hatch_ticks > 0) else {
            return;
        };
        let weapon = &bp.weapons[w];
        let (need, reload) = (weapon.hatch_ticks, weapon.reload_ticks);
        let units = &mut self.state.units;
        let cooldown = units.weapon_cooldown[row][w];
        let firing = units.weapon_salvo_left[row][w] > 0;
        let leaving = cooldown > 0 && reload.saturating_sub(cooldown) < CLEAR_TICKS;
        let due = cooldown <= need && units.row(units.weapon_target[row][w]).is_some();
        units.deploy[row] = if firing || leaving || due {
            (units.deploy[row] + 1).min(need)
        } else {
            units.deploy[row].saturating_sub(1)
        };
    }

    /// `UnitInstance::status[2]` for a cell launcher: a bit per cell still holding a
    /// missile, bit `k` for the cell under muzzle `k`. Mid-salvo the cells fired so far are
    /// empty; after it they stay empty until the hatches start to open for the next.
    /// `None` for a unit without hatches.
    pub(crate) fn loaded_cells(&self, row: usize) -> Option<u32> {
        let bp = self.bp(row);
        let w = bp.weapons.iter().position(|w| w.hatch_ticks > 0)?;
        let weapon = &bp.weapons[w];
        let cells = weapon.muzzles.len().clamp(1, 16) as u32;
        let all = (1u32 << cells) - 1;
        let left = self.state.units.weapon_salvo_left[row][w];
        Some(if left > 0 {
            let fired = (weapon.salvo - left).min(cells as u8) as u32;
            all & !((1u32 << fired) - 1)
        } else if self.state.units.weapon_cooldown[row][w] > weapon.hatch_ticks {
            0
        } else {
            all
        })
    }

    /// Lays weapon `w` of `row` on the target in range the fewest of its own missiles are
    /// chasing, the nearest of those first; with every one already chased, on the least.
    /// Nothing else in range: it stays on what it has. A unit ordered onto a mark
    /// (`OrderKind::Attack`) keeps the whole volley on it.
    pub(crate) fn split_target(&mut self, row: usize, w: usize) {
        let weapon = &self.bp(row).weapons[w];
        let units = &self.state.units;
        let ordered = self
            .state
            .orders
            .front(units, row)
            .is_some_and(|o| o.kind == crate::tables::OrderKind::Attack);
        if ordered {
            return;
        }
        let (at, id) = (units.pos[row], units.id(row));
        let p = &self.state.projectiles;
        let chased = |t: usize| {
            let target = units.id(t);
            (0..p.len())
                .filter(|&i| {
                    p.source[i] == id && p.weapon[i] as usize == w && p.target[i] == target
                })
                .count()
        };
        let mut near: Vec<(usize, Fx, usize)> = Vec::new();
        let friends = self.friends(units.owner[row]);
        self.index.query_foes(
            at,
            weapon.range_max,
            crate::spatial::kind::UNIT,
            friends,
            |e| {
                let t = e.row as usize;
                if self.unit_entry_is_current(e) && self.is_valid_target(row, t, weapon) {
                    near.push((chased(t), at.distance(units.pos[t]), t));
                }
                true
            },
        );
        // Rows are unique, so equal keys never tie.
        near.sort_unstable();
        let pick = near
            .into_iter()
            .map(|(_, _, t)| t)
            .find(|&t| !self.needs_line(row, weapon) || self.clear_shot(row, weapon, t));
        if let Some(t) = pick {
            self.state.units.weapon_target[row][w] = self.state.units.id(t);
        }
    }
}
