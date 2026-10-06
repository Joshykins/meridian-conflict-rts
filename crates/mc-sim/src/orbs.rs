//! Orb batteries (`Weapon::orbs`, the Regency's Carillon): a gun that makes plasma orbs a
//! batch at a time while it stands idle and holds them, up to one for each of its
//! muzzles, until something comes into reach. Each shot spends an orb and leaves from
//! the muzzle where that orb floated, so the stored orbs are stored damage: a battery
//! that has waited long enough empties its whole store onto the field in one salvo.
//!
//! The store is `Units::orb_stock`, filled from slot 0 up and spent from the top slot
//! down, so a batch is the next `orbs` slots and the renderer is told which slots are
//! held (`UnitInstance::status[2]`, `World::held_orbs`).

use crate::{SimEvent, World};
use mc_data::Weapon;

impl World {
    /// The orb battery among `row`'s weapons, with its slot.
    pub(crate) fn orb_battery(&self, row: usize) -> Option<(usize, &Weapon)> {
        self.bp(row)
            .weapons
            .iter()
            .enumerate()
            .find(|(_, w)| w.orbs > 0)
    }

    /// Makes `row`'s next batch once its clock runs out, while the store has room for it.
    /// A full store stops the clock, so the first batch after a salvo takes a whole period.
    pub(crate) fn step_orbs(&mut self, row: usize) {
        let Some((slot, weapon)) = self.orb_battery(row) else {
            return;
        };
        let (batch, period, cap) = (weapon.orbs, weapon.orb_ticks, weapon.muzzles.len());
        let at = self.state.units.pos[row].extend(self.state.units.z[row]);
        let blueprint = self.state.units.blueprint[row];
        let units = &mut self.state.units;
        if units.orb_stock[row] as usize + batch as usize > cap {
            units.orb_clock[row] = 0;
            return;
        }
        units.orb_clock[row] += 1;
        if units.orb_clock[row] >= period {
            units.orb_clock[row] = 0;
            units.orb_stock[row] += batch;
            let held = units.orb_stock[row];
            self.events.push(SimEvent::OrbsMade {
                pos: at,
                blueprint,
                weapon: slot as u8,
                held,
            });
        }
    }

    /// `UnitInstance::status[2]` for an orb battery: a bit for each slot holding an orb.
    /// `None` for a unit without one.
    pub(crate) fn held_orbs(&self, row: usize) -> Option<u32> {
        self.orb_battery(row)?;
        let held = self.state.units.orb_stock[row] as u32;
        Some(if held >= 32 {
            u32::MAX
        } else {
            (1u32 << held) - 1
        })
    }
}
