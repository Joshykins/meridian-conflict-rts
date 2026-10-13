//! The Behemoth's AEB light (`models::pattern::COIL`, `COIL_TURN`): the shader breathes
//! the coils idle, climbs them stage by stage through the charge, blinds them at the shot
//! and cools them after, and turns the capacitor rings' lugs faster as the charge builds.
//! The Regency fusion guns' plasma coils light the same way, and their working gear
//! (`rig::CHARGE_GEAR_MASK`) opens through the charge and vents after the shot.
//! It needs each unit's charge on the clock, which the sim does not mirror: this keeps it
//! from the events (`SimEvent::StormCharging` or `WeaponCharging` starts a charge, the
//! weapon's `ShotFired` ends it) and hands it to the shader in `UnitInstance::mount`,
//! which a model with charge coils has no other use for (its weapons turn on gun houses
//! or on the turret's own arm, never a `rig::MOUNT`):
//! `[charge start, charge due, last shot, CHARGE_RECORD]`, seconds on the renderer's clock
//! (`FrameInput::time`, the shader's `globals.camera.w`). A unit whose guns turn on houses
//! of their own (the Flamberge's) also carries each gun's own charge in its `HousePose`
//! (`patch_houses`), so each house's gear opens on its own charge, not the last one heard.

use std::borrow::Cow;
use std::collections::HashMap;

use mc_data::{Blueprints, MAX_HOUSES};
use mc_sim::mirror::{HousePose, SimEvent, UnitInstance, UNIT_HOUSE_SHIFT};

/// `UnitInstance::mount.w` on a unit whose mount carries its charge (`entity.wgsl`).
pub(super) const CHARGE_RECORD: f32 = -1000.0;
/// Long ago: a unit that has never charged or fired.
const NEVER: f32 = -1.0e4;
/// A shot this soon (seconds) after the last one carries on the same burn: a held beam
/// (the Colossus's lance) fires every tick, and its gear stays open until the last.
const HOLD_GAP: f32 = 0.5;

#[derive(Clone, Copy)]
struct Charge {
    start: f32,
    due: f32,
    shot: f32,
    /// The gun that charges: its blueprint and gun slot (`UnitBlueprint::gun_slot`), to
    /// match its `ShotFired`.
    blueprint: u32,
    gun: u16,
}

pub(super) struct TitanCharge {
    /// Per model slot (`UnitInstance::blueprint`): whether its mesh carries charge coils.
    coils: Vec<bool>,
    /// Each unit's last charge begun, whichever gun.
    units: HashMap<u32, Charge>,
    /// Each gun's own (unit, gun slot), for guns on houses of their own.
    guns: HashMap<(u32, u16), Charge>,
    /// Seen since the last upload: charges begun (unit, seconds, blueprint, gun), and
    /// shots fired by a gun that charges (blueprint, gun, where).
    charges: Vec<(u32, f32, u32, u16)>,
    shots: Vec<(u32, u16, [f32; 2])>,
}

impl TitanCharge {
    pub(super) fn new(coils: Vec<bool>) -> Self {
        Self {
            coils,
            units: HashMap::new(),
            guns: HashMap::new(),
            charges: Vec::new(),
            shots: Vec::new(),
        }
    }

    fn any(&self) -> bool {
        self.coils.iter().any(|&c| c)
    }

    /// Hears the events that start and end a charge.
    pub(super) fn note(&mut self, event: &SimEvent, tick_seconds: f32, blueprints: &Blueprints) {
        if !self.any() {
            return;
        }
        match event {
            SimEvent::WeaponCharging {
                unit,
                blueprint,
                weapon,
                gun,
                ..
            } if self
                .coils
                .get(blueprint.0 as usize)
                .copied()
                .unwrap_or(false) =>
            {
                let w = &blueprints.unit(*blueprint).weapons[*weapon as usize];
                self.charges.push((
                    unit.0,
                    w.charge_ticks as f32 * tick_seconds,
                    blueprint.0 as u32,
                    *gun,
                ));
            }
            SimEvent::StormCharging {
                unit,
                ticks,
                blueprint,
                weapon,
                ..
            } => {
                // A storm's bore is an ordinary weapon: its gun is the weapon's one.
                self.charges.push((
                    unit.0,
                    *ticks as f32 * tick_seconds,
                    blueprint.0 as u32,
                    blueprints.unit(*blueprint).gun_slot(*weapon as usize) as u16,
                ));
            }
            SimEvent::ShotFired {
                pos,
                blueprint,
                gun,
                ..
            } => {
                let (bp, g) = (blueprint.0 as u32, *gun);
                if self.guns.values().any(|c| c.blueprint == bp && c.gun == g)
                    || self.charges.iter().any(|c| c.2 == bp && c.3 == g)
                {
                    let p = pos.to_f32();
                    self.shots.push((bp, g, [p[0], p[1]]));
                }
            }
            _ => {}
        }
    }

    /// The units as the GPU should have them: those whose model has charge coils carry
    /// their charge in `mount`. Borrowed as they are when there are none.
    pub(super) fn patch<'a>(
        &mut self,
        units: &'a [UnitInstance],
        time: f32,
    ) -> Cow<'a, [UnitInstance]> {
        let coil = |u: &UnitInstance| {
            self.coils
                .get(u.blueprint as usize)
                .copied()
                .unwrap_or(false)
        };
        if !units.iter().any(coil) {
            self.units.clear();
            self.guns.clear();
            self.charges.clear();
            self.shots.clear();
            return Cow::Borrowed(units);
        }
        for (id, seconds, blueprint, slot) in self.charges.drain(..) {
            let gun = self.guns.entry((id, slot)).or_insert(Charge {
                start: NEVER,
                due: NEVER,
                shot: NEVER,
                blueprint,
                gun: slot,
            });
            *gun = Charge {
                start: time,
                due: time + seconds.max(0.1),
                blueprint,
                gun: slot,
                ..*gun
            };
            // The unit's own record follows the last charge begun, keeping its last shot.
            let shot = self.units.get(&id).map_or(NEVER, |c| c.shot);
            self.units.insert(id, Charge { shot, ..*gun });
        }
        for (blueprint, gun, at) in self.shots.drain(..) {
            // The nearest unit charging that gun, or still burning it, fired it.
            let mut best: Option<(u32, f32)> = None;
            for u in units.iter().filter(|u| coil(u)) {
                let Some(c) = self.guns.get(&(u.unit_id, gun)) else {
                    continue;
                };
                let burning = c.start <= c.shot && time - c.shot < HOLD_GAP;
                if c.blueprint != blueprint || !(c.start > c.shot || burning) {
                    continue;
                }
                let d = (u.pos[0] - at[0]).hypot(u.pos[1] - at[1]);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((u.unit_id, d));
                }
            }
            let Some((id, _)) = best else {
                continue;
            };
            if let Some(c) = self.guns.get_mut(&(id, gun)) {
                c.shot = time;
            }
            if let Some(c) = self.units.get_mut(&id).filter(|c| c.gun == gun) {
                c.shot = time;
            }
        }
        let mut out = units.to_vec();
        // (A frame that already carries a charge record, a hand-posed shot's, is left be.)
        for u in out.iter_mut().filter(|u| {
            self.coils
                .get(u.blueprint as usize)
                .copied()
                .unwrap_or(false)
                && u.mount[3] != CHARGE_RECORD
        }) {
            let c = self.units.get(&u.unit_id).copied();
            u.mount = c.map_or([NEVER, NEVER, NEVER, CHARGE_RECORD], |c| {
                [c.start, c.due, c.shot, CHARGE_RECORD]
            });
        }
        if self.units.len() > 64 {
            let live: std::collections::HashSet<u32> = units.iter().map(|u| u.unit_id).collect();
            self.units.retain(|id, _| live.contains(id));
            self.guns.retain(|(id, _), _| live.contains(id));
        }
        Cow::Owned(out)
    }

    /// The gun-house poses as the GPU should have them (after `patch`): each house of a
    /// unit whose model has charge coils carries its own gun's charge
    /// (`HousePose::charge`). Borrowed as they are when there are none.
    pub(super) fn patch_houses<'a>(
        &self,
        units: &[UnitInstance],
        houses: &'a [HousePose],
        blueprints: &Blueprints,
    ) -> Cow<'a, [HousePose]> {
        if self.guns.is_empty() {
            return Cow::Borrowed(houses);
        }
        let mut out = houses.to_vec();
        for u in units {
            let Some(first) = (u.status[1] >> UNIT_HOUSE_SHIFT).checked_sub(1) else {
                continue;
            };
            // Its guns' records, eight guns to a record, in a row from its first.
            let guns = blueprints
                .units
                .get(u.blueprint as usize)
                .map_or(MAX_HOUSES, |bp| bp.gun_count())
                .div_ceil(MAX_HOUSES)
                .max(1)
                * MAX_HOUSES;
            for g in 0..guns {
                let Some(pose) = out.get_mut(first as usize + g / MAX_HOUSES) else {
                    break;
                };
                let record = self
                    .guns
                    .get(&(u.unit_id, g as u16))
                    .map_or([NEVER, 0.0], |c| {
                        if c.shot >= c.start {
                            [c.start, c.shot]
                        } else {
                            [c.start, -c.due]
                        }
                    });
                let k = g % MAX_HOUSES;
                pose.charge[2 * k..2 * k + 2].copy_from_slice(&record);
            }
        }
        Cow::Owned(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_charge_then_a_shot_reach_the_mount() {
        let mut tc = TitanCharge::new(vec![false, true]);
        let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
        u.blueprint = 1;
        u.unit_id = 7;
        u.pos = [100.0, 50.0, 0.0];
        let m = tc.patch(&[u], 1.0)[0].mount;
        assert_eq!(m[3], CHARGE_RECORD);
        assert!(m[0] < 0.0 && m[2] < 0.0);
        tc.charges.push((7, 6.0, 1, 1));
        let m = tc.patch(&[u], 2.0)[0].mount;
        assert_eq!((m[0], m[1]), (2.0, 8.0));
        tc.shots.push((1, 1, [120.0, 40.0]));
        let m = tc.patch(&[u], 8.1)[0].mount;
        assert_eq!(m[2], 8.1);
        // A held beam's next tick carries the burn on: the last shot moves with it.
        tc.shots.push((1, 1, [120.0, 40.0]));
        let m = tc.patch(&[u], 8.2)[0].mount;
        assert_eq!(m[2], 8.2);
        // A shot long after is no part of it, and wants a new charge.
        tc.shots.push((1, 1, [120.0, 40.0]));
        let m = tc.patch(&[u], 12.0)[0].mount;
        assert_eq!(m[2], 8.2);
    }

    #[test]
    fn each_house_carries_its_own_guns_charge() {
        let mut tc = TitanCharge::new(vec![false, true]);
        let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
        u.blueprint = 1;
        u.unit_id = 7;
        u.status[1] = 1 << UNIT_HOUSE_SHIFT;
        let houses = [HousePose::default()];
        // Houses 0 and 2 charge a second apart; house 0 fires.
        tc.charges.push((7, 2.0, 1, 0));
        tc.patch(&[u], 1.0);
        tc.charges.push((7, 2.0, 1, 2));
        tc.patch(&[u], 2.0);
        tc.shots.push((1, 0, [0.0, 0.0]));
        tc.patch(&[u], 3.0);
        let c = tc.patch_houses(&[u], &houses, &Blueprints::default())[0].charge;
        assert_eq!((c[0], c[1]), (1.0, 3.0), "house 0 fired at 3");
        assert_eq!(
            (c[4], c[5]),
            (2.0, -4.0),
            "house 2 still charging, due at 4"
        );
        assert!(c[2] < -9000.0, "house 1 never charged");
        // The unit's own record is the last charge begun, and its shot is not house 0's.
        let m = tc.patch(&[u], 3.1)[0].mount;
        assert_eq!((m[0], m[1]), (2.0, 4.0));
        assert!(m[2] < 0.0);
    }
}
