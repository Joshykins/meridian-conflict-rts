//! Reloads fed from the grid: a weapon with `reload_energy` (the artillery
//! installations and the map guns) draws that much energy a second while it reloads,
//! charge and all, and the reload runs only as fast as it is paid. Neighbouring power
//! plants (`adjacency.rs`) save it their share `s` of the draw and run the whole cycle
//! `1 + 2s` times as fast (`pace`): a gun ringed for a 60% saving fires 2.2 times as
//! often on 40% of the power, whatever its charge, so every faction's guns gain alike.
//!
//! The draw is paid with the rest of the side's energy-only spending, as upkeep and warp
//! drives are, so a stall slows every fed reload by the same ratio. The economy runs
//! before combat, so it moves the countdown itself and combat leaves a fed reload alone
//! but for its last tick (`Units::reload_carry` keeps the part of a tick the economy has
//! not yet counted down). A gun that charges before it fires is heard charging its
//! `charge_ticks` before the shot at its pace: the economy holds the countdown for a tick
//! at that mark (`charge_mark`) and says so, and a charge from rest is set out to it.
use crate::adjacency::Resource;
use crate::mirror::SimEvent;
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
/// `salvo_left` shots of its salvo still to come: a fed weapon in its reload, before its
/// last tick and not between the shots of a salvo.
pub(crate) fn fed(weapon: &Weapon, cooldown: u16, salvo_left: u8) -> bool {
    weapon.reload_energy > Fx::ZERO && salvo_left == 0 && cooldown > 1
}

/// How many times as fast a fed reload runs for a neighbours' saving of `saved`.
fn pace(saved: Fx) -> Fx {
    Fx::ONE + saved * 2
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

    /// Share of its fed reloads' draw the unit in `row` is saved by its neighbours.
    fn reload_saving(&self, row: usize) -> Fx {
        self.adjacency.saving(row, Resource::Energy).min(Fx::ONE)
    }

    /// Countdown at which `weapon` of the unit in `row` starts its charge: its
    /// `charge_ticks`, stretched by its pace when the reload is fed, so the charge takes
    /// as long as it is heard.
    pub(crate) fn charge_mark(&self, row: usize, weapon: &Weapon) -> u16 {
        if weapon.reload_energy <= Fx::ZERO || weapon.charge_ticks == 0 {
            return weapon.charge_ticks;
        }
        let ticks = Fx::from_int(weapon.charge_ticks as i32) * pace(self.reload_saving(row));
        ticks.round_int().clamp(1, u16::MAX as i32) as u16
    }

    /// Moves the fed reloads of the unit in `row` on by `paid` of a tick (the share of
    /// its draw its side covered) at its pace, holding each for a tick at its charge mark
    /// and leaving the last tick to combat.
    pub(crate) fn feed_reload(&mut self, row: usize, paid: Fx) {
        let saved = self.reload_saving(row);
        let bp = self.blueprints.clone();
        let bp = bp.unit(self.state.units.blueprint[row]);
        let carry = self.state.units.reload_carry[row] + paid * pace(saved);
        let steps = carry.floor_int().clamp(0, u16::MAX as i32) as u16;
        self.state.units.reload_carry[row] = carry - Fx::from_int(steps as i32);
        for (w, weapon) in bp.weapons.iter().enumerate() {
            let units = &self.state.units;
            let cooldown = units.weapon_cooldown[row][w];
            if !fed(weapon, cooldown, units.weapon_salvo_left[row][w]) {
                continue;
            }
            let mark = self.charge_mark(row, weapon);
            let mut next = cooldown.saturating_sub(steps).max(1);
            if weapon.charge_ticks > 0 && cooldown > mark && next <= mark {
                next = mark;
                self.events.push(SimEvent::WeaponCharging {
                    unit: units.id(row),
                    pos: units.pos[row].extend(units.z[row] + weapon.muzzle.z),
                    owner: units.owner[row],
                    blueprint: units.blueprint[row],
                    weapon: w as u8,
                    gun: bp.gun_slot(w) as u16,
                });
            }
            self.state.units.weapon_cooldown[row][w] = next;
        }
    }
}
