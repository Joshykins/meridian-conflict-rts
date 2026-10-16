//! A gun beginning its charge (`SimEvent::WeaponCharging`), drawn by the effect its weapon
//! asks for: a gravitic shock mortar winding up its crystal (`gravitic_shell_fx`), a
//! squeezed plasma gun gathering its charge (`regency_guns_fx`), a thrown charge
//! (`plasma_fx`) or an ordinary gun's glow (`weapon_charging`), and a giant gun's bore.

use glam::Vec3;
use mc_sim::mirror::SimEvent;

use super::{titan_fx, Renderer};

impl Renderer {
    /// `event`, a `SimEvent::WeaponCharging`, drawn from `time`.
    pub(super) fn weapon_charging_event(&mut self, event: &SimEvent, time: f32) {
        let SimEvent::WeaponCharging {
            unit,
            pos,
            owner,
            blueprint,
            weapon,
            gun,
        } = event
        else {
            return;
        };
        // A squeezed plasma gun gathers its charge in front of the bore
        // (`regency_guns_fx`), not as an ordinary gun's glow nor as a thrown
        // charge held at its unpitched muzzle (the Springald lobs, but its
        // charge rides its barrel). A gravitic shock mortar winds up its crystal
        // between its tines (`gravitic_shell_fx`).
        if !self.gravitic_mortar_charging(unit.0, (*blueprint, *weapon, *gun), time)
            && !self.regency_charging(
                unit.0,
                (*blueprint, *weapon, *gun),
                Vec3::from(pos.to_f32()),
                time,
            )
        {
            self.plasma_charging(
                unit.0,
                *owner,
                *blueprint,
                *weapon,
                Vec3::from(pos.to_f32()),
                time,
            );
            self.weapon_charging(Vec3::from(pos.to_f32()), *blueprint, *weapon, time);
        }
        let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
        let seconds = w.charge_ticks as f32 * self.tick_seconds.max(0.02);
        if w.bore.is_some() && seconds >= titan_fx::GIANT_CHARGE {
            let (at, id) = (Vec3::from(pos.to_f32()), blueprint.0 as u32);
            self.bore_charge(at, id, seconds, time);
        }
    }
}
