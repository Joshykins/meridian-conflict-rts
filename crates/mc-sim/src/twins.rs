//! Twin guns: weapons of one name on one unit that cover the same arc (the Paladin's two
//! bolt rifles, the Strider's two cannons). They take turns: the later one fires half a
//! reload after the earlier, never on the same tick.
//!
//! A gun that charges before it fires (`Weapon::charge_ticks`) is seen charging, so the
//! turn is kept from the start of the charge, not by holding back one that has already
//! charged. Twins that sat idle do not charge together: the later one starts its charge so
//! that it comes out half a reload after the earlier one's shot.
//!
//! Guns of one name that cover other arcs (a capital ship's turrets on the bow, the stern
//! and each flank) are separate guns: held back, one on the far side of the hull would sit
//! through a charge it can never fire. So are rotary guns (the Breacher's arms): each
//! keeps its own spin and fires as its own barrels come to the top (`Weapon::rotary`), and
//! held back on every shot of the other, the later would never fire at all.

use mc_data::Weapon;

use crate::world::World;

/// The ticks the later of two twins fires after the earlier.
fn stagger(weapon: &Weapon) -> u16 {
    weapon.reload_ticks / 2 + 1
}

/// Whether `a` and `b` are twins: one gun's name, covering the same arc, neither rotary.
fn twins(a: &Weapon, b: &Weapon) -> bool {
    a.name == b.name
        && a.facing == b.facing
        && a.half_arc == b.half_arc
        && a.spin_ticks == 0
        && b.spin_ticks == 0
}

impl World {
    /// The countdown weapon `w` of `row` starts on when it charges from idle: its charge,
    /// or longer when an earlier twin is already counting down to a shot, so that this one
    /// fires half a reload after it. Its charge is then seen from the countdown's last
    /// `charge_ticks` (the reload's charge in `step_weapon`).
    pub(crate) fn twin_charge(&self, row: usize, w: usize, weapon: &Weapon) -> u16 {
        let units = &self.state.units;
        let gap = stagger(weapon);
        self.bp(row)
            .weapons
            .iter()
            .enumerate()
            .take(w)
            .filter(|(i, other)| {
                twins(other, weapon)
                    && units.weapon_cooldown[row][*i] > 0
                    && units.weapon_salvo_left[row][*i] == 0
            })
            .map(|(i, _)| units.weapon_cooldown[row][i].saturating_add(gap))
            .fold(weapon.charge_ticks, u16::max)
    }

    /// Weapon `w` of `row` just let its last shot go: the later twins that are ready wait
    /// half a reload, so they do not fire on this tick or the next. One that charges waits
    /// at least a charge more, and is seen charging again for it.
    pub(crate) fn stagger_twins(&mut self, row: usize, w: usize, weapon: &Weapon) {
        let gap = stagger(weapon);
        if gap <= 1 {
            return;
        }
        let later: Vec<(usize, u16)> = self
            .bp(row)
            .weapons
            .iter()
            .enumerate()
            .skip(w + 1)
            .filter(|(_, other)| twins(other, weapon))
            .map(|(i, other)| {
                let wait = if other.charge_ticks > 0 {
                    gap.max(other.charge_ticks + 1)
                } else {
                    gap
                };
                (i, wait)
            })
            .collect();
        let units = &mut self.state.units;
        for (i, wait) in later {
            // Ready, or counting its last tick (a twin that charged beside this one would
            // otherwise fire on this very tick and stay in step for good).
            if units.weapon_cooldown[row][i] <= 1 && units.weapon_salvo_left[row][i] == 0 {
                units.weapon_cooldown[row][i] = wait;
            }
        }
    }
}
