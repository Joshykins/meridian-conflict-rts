//! How a gravitic lens (mc-sim `lens.rs`, the Regency's Lodestone) is heard. Its charge
//! and launch are its weapon's `charge` and `fire`, heard as any gun's are (game.rs). The
//! rest is here, every sound named in the weapon's `sounds`:
//!
//! - `flight` looped at the front while it runs out to its target;
//! - `impact` where the front strikes (`LensStruck`);
//! - `hold` looped on the well, carried with its target, until it is let go;
//! - `release` where the well collapses (`LensReleased`): only a well that held has one (a
//!   front let go before it struck goes quiet), heard where the well was last seen when
//!   its target was crushed to nothing.
//!
//! Many lenses at once: all the fronts sharing a sound are one voice and all the wells
//! another, their power summed (as held beams' are, `beams.rs`) and capped, so a line of
//! lenses holding is a heavier pressure, not a wall of hums.

use glam::Vec3;
use mc_data::{BlueprintId, Blueprints, SoundId, SoundLibrary};
use mc_sim::mirror::LensView;
use mc_sim::{RenderFrame, SimEvent};
use std::collections::HashMap;

use super::Audio;

/// A lens weapon's sounds, from its `sounds`.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Ids {
    front: Option<SoundId>,
    strike: Option<SoundId>,
    hold: Option<SoundId>,
    release: Option<SoundId>,
}

impl Ids {
    fn of(library: &SoundLibrary, blueprints: &Blueprints, blueprint: u16, weapon: u8) -> Ids {
        let Some(s) = blueprints
            .units
            .get(blueprint as usize)
            .and_then(|bp| bp.weapons.get(weapon as usize))
            .filter(|w| w.lens.is_some())
            .map(|w| &w.sounds)
        else {
            return Ids::default();
        };
        let id = |name: &Option<String>| name.as_deref().and_then(|n| library.id_of(n));
        Ids {
            front: id(&s.flight),
            strike: id(&s.impact),
            hold: id(&s.hold),
            release: id(&s.release),
        }
    }
}

/// A loop wanted, as `Audio::set_loops` takes it: (sound, gain, pan, pitch).
type LoopRow = (SoundId, f32, f32, f32);

/// The level of all the fronts sharing a sound, and of all the wells, on the summed power
/// of each (one lens heard as `hear` places it), and the most either may reach.
const FRONT_LEVEL: f32 = 0.8;
const HOLD_LEVEL: f32 = 0.7;
const LOOP_MAX: f32 = 0.85;
/// Strikes and releases heard in one tick, the loudest: a line of lenses firing together
/// is a few heavy blows, not a pile of them. A deliberate cap on a cosmetic.
const BLOWS_PER_TICK: usize = 3;
/// Ticks a well is remembered after it was last seen (a lens lost in the fog), so the
/// release of one that held still finds where it was.
const REMEMBER: u32 = 20;

/// A lens's front or well as last seen.
#[derive(Clone, Copy)]
struct Seen {
    blueprint: u16,
    /// The well's foot (`LensView::ground`).
    ground: Vec3,
    struck: bool,
    tick: u32,
}

#[derive(Default)]
pub(crate) struct LensSounds {
    /// By (blueprint, weapon).
    ids: HashMap<(u16, u8), Ids>,
    /// By (lens unit id, weapon).
    seen: HashMap<(u32, u8), Seen>,
    generation: Option<u32>,
}

impl LensSounds {
    /// One sim tick: plays the strikes and releases and returns the loops wanted, as
    /// `Audio::set_loops` takes them. `hear` is the game's (gain, pan) for a place.
    pub(crate) fn tick(
        &mut self,
        frame: &RenderFrame,
        blueprints: &Blueprints,
        (library, generation): (&SoundLibrary, u32),
        audio: &Audio,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<LoopRow> {
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.ids.clear();
        }
        let now = frame.tick;
        for v in &frame.lenses {
            self.seen.insert(
                (v.unit_id, v.weapon),
                Seen {
                    blueprint: v.blueprint.0,
                    ground: Vec3::from(v.ground),
                    struck: v.struck,
                    tick: now,
                },
            );
        }
        let ids_cache = &mut self.ids;
        let mut ids = |blueprint: u16, weapon: u8| -> Ids {
            *ids_cache
                .entry((blueprint, weapon))
                .or_insert_with(|| Ids::of(library, blueprints, blueprint, weapon))
        };

        let mut blows = Vec::new();
        for event in &frame.events {
            match event {
                SimEvent::LensStruck {
                    unit, weapon, pos, ..
                } => {
                    let blueprint = frame
                        .lenses
                        .iter()
                        .find(|v| v.unit_id == unit.0 && v.weapon == *weapon)
                        .map(|v| v.blueprint.0)
                        .or_else(|| self.seen.get(&(unit.0, *weapon)).map(|s| s.blueprint));
                    if let Some(sound) = blueprint.and_then(|b| ids(b, *weapon).strike) {
                        blows.push((sound, Vec3::from(pos.to_f32())));
                    }
                }
                SimEvent::LensReleased { unit, weapon, pos } => {
                    let Some(seen) = self.seen.remove(&(unit.0, *weapon)) else {
                        continue;
                    };
                    if !seen.struck {
                        continue;
                    }
                    if let Some(sound) = ids(seen.blueprint, *weapon).release {
                        let at = pos.map_or(seen.ground, |p| Vec3::from(p.to_f32()));
                        blows.push((sound, at));
                    }
                }
                _ => {}
            }
        }
        let mut blows: Vec<(SoundId, f32, f32)> = blows
            .into_iter()
            .map(|(sound, at)| {
                let (gain, pan) = hear(at);
                (sound, gain, pan)
            })
            .collect();
        blows.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (sound, gain, pan) in blows.into_iter().take(BLOWS_PER_TICK) {
            audio.play_world(sound, gain.min(1.0), pan, 1.0);
        }
        self.seen.retain(|_, s| now.wrapping_sub(s.tick) < REMEMBER);

        loops(&frame.lenses, |b, w| ids(b.0, w), hear)
    }
}

/// The loops every lens at work wants this tick: one per sound, its fronts' or wells'
/// power summed and its pan weighted by it.
fn loops(
    lenses: &[LensView],
    mut ids: impl FnMut(BlueprintId, u8) -> Ids,
    hear: impl Fn(Vec3) -> (f32, f32),
) -> Vec<LoopRow> {
    // Per sound: its level, summed power, power-weighted pan.
    let mut sums: Vec<(SoundId, f32, f32, f32)> = Vec::new();
    for v in lenses {
        let i = ids(v.blueprint, v.weapon);
        let (sound, at, level) = if v.struck {
            (i.hold, v.ground, HOLD_LEVEL)
        } else {
            (i.front, v.front, FRONT_LEVEL)
        };
        let Some(sound) = sound else {
            continue;
        };
        let (gain, pan) = hear(Vec3::from(at));
        let power = gain * gain;
        match sums.iter_mut().find(|s| s.0 == sound) {
            Some(s) => {
                s.2 += power;
                s.3 += power * pan;
            }
            None => sums.push((sound, level, power, power * pan)),
        }
    }
    sums.into_iter()
        .filter(|s| s.2 > 0.0)
        .map(|(sound, level, power, pan)| {
            (
                sound,
                (power.sqrt() * level).min(LOOP_MAX),
                pan / power,
                1.0,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(unit_id: u32, struck: bool, at: [f32; 3]) -> LensView {
        LensView {
            unit_id,
            owner: 0,
            blueprint: BlueprintId(0),
            weapon: 0,
            struck,
            seconds: 1.0,
            emitter: [0.0; 3],
            front: at,
            ground: at,
            radius: 30.0,
        }
    }

    fn ids(_: BlueprintId, _: u8) -> Ids {
        Ids {
            front: Some(SoundId(1)),
            hold: Some(SoundId(2)),
            ..Ids::default()
        }
    }

    /// Heard from the origin: loud there, nothing 1 km off, panned by side.
    fn hear(at: Vec3) -> (f32, f32) {
        ((1.0 - at.x.abs() / 1000.0).max(0.0), at.x.signum() * 0.5)
    }

    #[test]
    fn a_front_is_heard_running_and_its_well_holding() {
        let running = loops(&[view(1, false, [100.0, 0.0, 0.0])], ids, hear);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].0, SoundId(1));
        let held = loops(&[view(1, true, [100.0, 0.0, 0.0])], ids, hear);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].0, SoundId(2));
        assert!(held[0].2 > 0.0);
    }

    /// Four wells together are one voice, louder than one but nowhere near four times.
    #[test]
    fn many_wells_are_one_voice_that_does_not_pile_up() {
        let one = loops(&[view(1, true, [0.0; 3])], ids, hear);
        let four: Vec<LensView> = (0..4).map(|u| view(u, true, [0.0; 3])).collect();
        let four = loops(&four, ids, hear);
        assert_eq!(four.len(), 1);
        assert!(four[0].1 > one[0].1 && four[0].1 <= one[0].1 * 2.0 + 1e-6);
        assert!(four[0].1 <= LOOP_MAX);
    }

    /// The Lodestone's sounds are its own, all named and in the library, the front and
    /// well looping.
    #[test]
    fn the_lodestone_has_all_its_sounds() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let library = SoundLibrary::load(&data).unwrap();
        let blueprints = Blueprints::load(&data).unwrap();
        let mut lenses = 0;
        for (b, bp) in blueprints.units.iter().enumerate() {
            for (w, weapon) in bp.weapons.iter().enumerate() {
                if weapon.lens.is_none() {
                    continue;
                }
                lenses += 1;
                let i = Ids::of(&library, &blueprints, b as u16, w as u8);
                for s in [i.front, i.strike, i.hold, i.release] {
                    let s = library.sound(s.expect("named and in the library"));
                    assert!(s.like.is_none(), "{} is a stand-in", s.name);
                }
                let looped = [i.front, i.hold].map(|s| library.sound(s.unwrap()).looped);
                assert_eq!(looped, [true; 2]);
                let charge = weapon
                    .sounds
                    .charge
                    .as_deref()
                    .and_then(|n| library.id_of(n));
                let charge = library.sound(charge.expect("a charge"));
                assert!(
                    (charge.length - weapon.sounds.charge_time as f32).abs() < 0.01,
                    "the charge sound lasts as long as the charge"
                );
            }
        }
        assert!(lenses > 0);
    }
}
