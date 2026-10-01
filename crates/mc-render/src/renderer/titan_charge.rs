//! The Behemoth's AEB light (`models::pattern::COIL`, `COIL_TURN`): the shader breathes
//! the coils idle, climbs them stage by stage through the charge, blinds them at the shot
//! and cools them after, and turns the capacitor rings' lugs faster as the charge builds.
//! The Sunspear's plasma coils light the same way, and its working gear
//! (`rig::CHARGE_GEAR_MASK`) opens through the charge and vents after the shot.
//! It needs each unit's charge on the clock, which the sim does not mirror: this keeps it
//! from the events (`SimEvent::StormCharging` or `WeaponCharging` starts a charge, the
//! weapon's `ShotFired` ends it) and hands it to the shader in `UnitInstance::mount`,
//! which a model with charge coils has no other use for (its weapons turn on gun houses
//! or on the turret's own arm, never a `rig::MOUNT`):
//! `[charge start, charge due, last shot, CHARGE_RECORD]`, seconds on the renderer's clock
//! (`FrameInput::time`, the shader's `globals.camera.w`).

use std::borrow::Cow;
use std::collections::HashMap;

use mc_data::Blueprints;
use mc_sim::mirror::{SimEvent, UnitInstance};

/// `UnitInstance::mount.w` on a unit whose mount carries its charge (`entity.wgsl`).
pub(super) const CHARGE_RECORD: f32 = -1000.0;
/// Long ago: a unit that has never charged or fired.
const NEVER: f32 = -1.0e4;

#[derive(Clone, Copy)]
struct Charge {
    start: f32,
    due: f32,
    shot: f32,
    /// The weapon that charges: its blueprint and index, to match its `ShotFired`.
    blueprint: u32,
    weapon: u8,
}

pub(super) struct TitanCharge {
    /// Per model slot (`UnitInstance::blueprint`): whether its mesh carries charge coils.
    coils: Vec<bool>,
    units: HashMap<u32, Charge>,
    /// Seen since the last upload: charges begun (unit, seconds, blueprint, weapon), and
    /// shots fired by a weapon that charges (blueprint, weapon, where).
    charges: Vec<(u32, f32, u32, u8)>,
    shots: Vec<(u32, u8, [f32; 2])>,
}

impl TitanCharge {
    pub(super) fn new(coils: Vec<bool>) -> Self {
        Self {
            coils,
            units: HashMap::new(),
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
                    *weapon,
                ));
            }
            SimEvent::StormCharging {
                unit,
                ticks,
                blueprint,
                weapon,
                ..
            } => {
                self.charges.push((
                    unit.0,
                    *ticks as f32 * tick_seconds,
                    blueprint.0 as u32,
                    *weapon,
                ));
            }
            SimEvent::ShotFired {
                pos,
                blueprint,
                weapon,
                ..
            } => {
                let (bp, w) = (blueprint.0 as u32, *weapon);
                if self
                    .units
                    .values()
                    .any(|c| c.blueprint == bp && c.weapon == w)
                    || self.charges.iter().any(|c| c.2 == bp && c.3 == w)
                {
                    let p = pos.to_f32();
                    self.shots.push((bp, w, [p[0], p[1]]));
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
            self.charges.clear();
            self.shots.clear();
            return Cow::Borrowed(units);
        }
        for (id, seconds, blueprint, weapon) in self.charges.drain(..) {
            let entry = self.units.entry(id).or_insert(Charge {
                start: NEVER,
                due: NEVER,
                shot: NEVER,
                blueprint,
                weapon,
            });
            *entry = Charge {
                start: time,
                due: time + seconds.max(0.1),
                blueprint,
                weapon,
                ..*entry
            };
        }
        for (blueprint, weapon, at) in self.shots.drain(..) {
            // The nearest unit charging that weapon fired it.
            let mut best: Option<(u32, f32)> = None;
            for u in units.iter().filter(|u| coil(u)) {
                let Some(c) = self.units.get(&u.unit_id) else {
                    continue;
                };
                if c.blueprint != blueprint || c.weapon != weapon || c.start <= c.shot {
                    continue;
                }
                let d = (u.pos[0] - at[0]).hypot(u.pos[1] - at[1]);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((u.unit_id, d));
                }
            }
            if let Some(c) = best.and_then(|(id, _)| self.units.get_mut(&id)) {
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
    }
}
