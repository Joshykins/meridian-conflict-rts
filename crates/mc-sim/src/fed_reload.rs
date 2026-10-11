//! Reloads fed from the grid: a weapon with `reload_energy` (the artillery
//! installations and the map guns) draws that much energy a second while it reloads,
//! and the reload runs only as fast as it is paid. Neighbouring power plants
//! (`adjacency.rs`) save it their share of the draw and speed the reload by as much: a
//! gun ringed for a 60% saving reloads at 1.6 times the pace on 40% of the power.
//!
//! The draw is paid with the rest of the side's energy-only spending, as upkeep and warp
//! drives are, so a stall slows every fed reload by the same ratio. The economy runs
//! before combat, so it moves the countdown itself and combat leaves a fed reload alone
//! (`Units::reload_carry` keeps the part of a tick it has not yet counted down). It
//! stops a tick short of the charge (`Weapon::charge_ticks`, none for most guns), which
//! combat then counts down a tick a tick as ever, so a charge is heard on time.

use crate::adjacency::Resource;
use crate::World;
use mc_core::{Fx, TICKS_PER_SECOND};
use mc_data::{UnitBlueprint, Weapon};

const DT: i32 = TICKS_PER_SECOND as i32;

/// Whether any of `bp`'s weapons draws power to reload.
pub fn feeds(bp: &UnitBlueprint) -> bool {
    bp.weapons.iter().any(|w| w.reload_energy > Fx::ZERO)
}

/// Energy a second `bp`'s guns draw with every one of them reloading: what a side plans
/// its power for, since a gun in a fight is always reloading.
pub(crate) fn full_draw(bp: &UnitBlueprint) -> Fx {
    bp.weapons
        .iter()
        .fold(Fx::ZERO, |sum, w| sum + w.reload_energy)
}

/// Whether the economy, not combat, counts down `weapon` at `cooldown` ticks with
/// `salvo_left` shots of its salvo still to come: a fed weapon in its reload, more than a
/// tick above its charge and not between the shots of a salvo.
pub(crate) fn fed(weapon: &Weapon, cooldown: u16, salvo_left: u8) -> bool {
    weapon.reload_energy > Fx::ZERO && salvo_left == 0 && cooldown > last(weapon)
}

/// Where the economy leaves a fed reload for combat to finish.
fn last(weapon: &Weapon) -> u16 {
    weapon.charge_ticks + 1
}

impl World {
    /// Energy the unit in `row` asks for this tick, per tick, to feed its reloads, or
    /// `None` when none is running.
    pub(crate) fn reload_draw(&self, row: usize) -> Option<Fx> {
        let units = &self.state.units;
        if !units.is_active(row) {
            return None;
        }
        let mut running = self
            .bp(row)
            .weapons
            .iter()
            .enumerate()
            .filter(|&(w, weapon)| {
                fed(
                    weapon,
                    units.weapon_cooldown[row][w],
                    units.weapon_salvo_left[row][w],
                )
            })
            .peekable();
        running.peek()?;
        let full = running.fold(Fx::ZERO, |sum, (_, weapon)| sum + weapon.reload_energy);
        let saved = self.adjacency.saving(row, Resource::Energy).min(Fx::ONE);
        Some((full - full * saved) / DT)
    }

    /// Moves the fed reloads of the unit in `row` on by `paid` of a tick (the share of
    /// its draw its side covered), quickened by what its neighbours save it.
    pub(crate) fn feed_reload(&mut self, row: usize, paid: Fx) {
        let saved = self.adjacency.saving(row, Resource::Energy).min(Fx::ONE);
        let bp = self.blueprints.unit(self.state.units.blueprint[row]);
        let units = &mut self.state.units;
        let carry = units.reload_carry[row] + paid + paid * saved;
        let steps = carry.floor_int().max(0);
        units.reload_carry[row] = carry - Fx::from_int(steps);
        for (w, weapon) in bp.weapons.iter().enumerate() {
            let cooldown = units.weapon_cooldown[row][w];
            if fed(weapon, cooldown, units.weapon_salvo_left[row][w]) {
                units.weapon_cooldown[row][w] = cooldown
                    .saturating_sub(steps.min(u16::MAX as i32) as u16)
                    .max(last(weapon));
            }
        }
    }
}
