//! How a gravitic engine (mc-sim `gravitic.rs`, the Regency's The Fallen) is heard. Every
//! sound is the engine's own, named in its blueprint's `gravitic.sounds`:
//!
//! - `gather`, held at the engine's core while it gathers (`held.rs`). It is as long as the
//!   gathering, and it follows the gathering, not the event: re-aimed mid-gather it carries
//!   on, first seen part way through it joins at that point, and stalled (its side cannot
//!   pay) or dead it fades out.
//! - `launch` where the rift tears out of the ground; `rift` looped at its front as it runs
//!   (and, quieter, from the nearest point of the open crack behind it); `strike` where it
//!   lands, heard over the whole map; `hold` looped at the well; `hum` looped at the
//!   engine while its rift runs and its well holds; `release` when a well is let go (a
//!   rift or gathering that dies has none).
//! - A faltering engine (`WellView::powered` false) sags: its hold and hum drop in level
//!   and in pitch, and its rings coast while a stalled gathering waits.
//!
//! Like the giants' far sounds (`titan.rs`), the big moments carry across the map at once,
//! without the real lag of sound: the launch and, loudest, the strike.
//!
//! The gravitic lens, The Fallen's small cousin, is heard alongside (`lens.rs`), and so is
//! the Regent's Edict (`edict.rs`), whose held gather shares the engine's held voices.

use super::held::Held;
use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary};
use mc_sim::mirror::{WellStage, WellView};
use mc_sim::{RenderFrame, SimEvent};
use std::collections::HashMap;

/// An engine's sounds, from its blueprint's `gravitic.sounds`.
#[derive(Clone, Copy, Default)]
struct Ids {
    gather: Option<SoundId>,
    launch: Option<SoundId>,
    rift: Option<SoundId>,
    strike: Option<SoundId>,
    hold: Option<SoundId>,
    hum: Option<SoundId>,
    release: Option<SoundId>,
}

impl Ids {
    fn of(library: &SoundLibrary, blueprints: &Blueprints, blueprint: usize) -> Ids {
        let Some(s) = blueprints
            .units
            .get(blueprint)
            .and_then(|bp| bp.gravitic.as_ref())
            .map(|g| &g.sounds)
        else {
            return Ids::default();
        };
        let id = |name: &String| library.id_of(name);
        Ids {
            gather: id(&s.gather),
            launch: id(&s.launch),
            rift: id(&s.rift),
            strike: id(&s.strike),
            hold: id(&s.hold),
            hum: id(&s.hum),
            release: id(&s.release),
        }
    }
}

/// Every gravitic loop whose pitch follows the engine (`Audio::set_gliding`): a faltering
/// well and engine sag. For `CapitalSounds` to list with its own.
pub(crate) fn gliding(library: &SoundLibrary, blueprints: &Blueprints) -> Vec<SoundId> {
    let mut ids: Vec<SoundId> = (0..blueprints.units.len())
        .filter(|&b| blueprints.units[b].gravitic.is_some())
        .flat_map(|b| {
            let i = Ids::of(library, blueprints, b);
            [i.hold, i.hum]
        })
        .flatten()
        .collect();
    ids.sort_by_key(|s| s.0);
    ids.dedup();
    ids
}

/// A loop wanted, as `Audio::set_loops` takes it: (sound, gain, pan, pitch).
type LoopRow = (SoundId, f32, f32, f32);

/// Which of an engine's sounds an event plays.
type Pick = fn(Ids) -> Option<SoundId>;

/// How a far-carrying sound is heard: (peak level, distance at half of it in metres).
#[derive(Clone, Copy)]
struct Far(f32, f32);

/// The gathering, so an engine winding up is heard from a way off.
const GATHER_FAR: Far = Far(0.12, 3000.0);
/// The rift running, louder the nearer its front comes.
const RIFT_FAR: Far = Far(0.16, 3000.0);
const LAUNCH_FAR: Far = Far(0.3, 5000.0);
/// The strike, heard over the whole map: a fifth of full level 10 km away.
const STRIKE_FAR: Far = Far(0.55, 8000.0);
const RELEASE_FAR: Far = Far(0.12, 4000.0);
/// What is left of a faltering engine's hold and hum, and their pitch.
const FALTER_GAIN: f32 = 0.3;
const FALTER_PITCH: f32 = 0.86;
/// Ticks an engine's blueprint is remembered after its work was last seen, so the
/// sounds of a well let go as the engine dies still find it.
const REMEMBER: u64 = 600;

/// Keys of the held sounds (`Held::key`): the engine's unit id with this tag over it.
const GATHER_KEY: u64 = 1 << 32;

#[derive(Default)]
pub(crate) struct GraviticSounds {
    ids: HashMap<u32, Ids>,
    /// Engines seen at work: unit id to (blueprint, the tick last seen).
    engines: HashMap<u32, (u32, u64)>,
    generation: Option<u32>,
    tick: u64,
    /// Gravitic lenses' fronts, strikes and wells.
    lens: super::lens::LensSounds,
    /// The Regent's Edicts gathering and firing.
    edict: super::edict::EdictSounds,
}

impl GraviticSounds {
    /// One sim tick: plays the one-shots, holds the gatherings (`Audio::set_held`) and
    /// returns the loops wanted, as `Audio::set_loops` takes them. `hear` is the game's
    /// (gain, pan) for a place; `focus` the ground point the camera looks at.
    pub(crate) fn tick(
        &mut self,
        frame: &RenderFrame,
        blueprints: &Blueprints,
        audio: &Audio,
        focus: Vec3,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<LoopRow> {
        self.tick += 1;
        let (library, generation) = audio.library();
        let fresh = self.generation != Some(generation);
        if fresh {
            self.generation = Some(generation);
            self.ids.clear();
        }
        let now = self.tick;
        for w in &frame.wells {
            self.engines.insert(w.unit_id, (w.blueprint.0 as u32, now));
        }
        self.engines.retain(|_, e| now - e.1 < REMEMBER);
        let ids_cache = &mut self.ids;
        let mut ids = |blueprint: u32| -> Ids {
            *ids_cache
                .entry(blueprint)
                .or_insert_with(|| Ids::of(&library, blueprints, blueprint as usize))
        };
        let far = |at: Vec3, far: Far, near: f32| -> (f32, f32) {
            let (gain, pan) = hear(at);
            let d = (at - focus).truncate().length();
            ((gain * near).max(far.0 / (1.0 + (d / far.1).powi(2))), pan)
        };
        // The engine an event is about: seen at work, else among the units.
        let engine = |unit: u32| {
            self.engines.get(&unit).map(|e| e.0).or_else(|| {
                frame
                    .units
                    .iter()
                    .find(|u| u.unit_id == unit)
                    .map(|u| u.blueprint)
            })
        };

        for event in &frame.events {
            let (unit, sound, at, reach, near): (_, Pick, [f32; 3], Far, f32) = match event {
                SimEvent::RiftLaunched { unit, from, .. } => {
                    (unit, |i: Ids| i.launch, from.to_f32(), LAUNCH_FAR, 1.3)
                }
                SimEvent::RiftStruck { unit, pos, .. } => {
                    (unit, |i: Ids| i.strike, pos.to_f32(), STRIKE_FAR, 1.4)
                }
                // A rift or gathering that dies (radius zero) only goes quiet: its loop or
                // held gathering is no longer wanted.
                SimEvent::WellReleased { unit, pos, radius } if radius.to_f32() > 0.0 => {
                    (unit, |i: Ids| i.release, pos.to_f32(), RELEASE_FAR, 1.1)
                }
                _ => continue,
            };
            let Some(sound) = engine(unit.0).and_then(|b| sound(ids(b))) else {
                continue;
            };
            let (gain, pan) = far(Vec3::from(at), reach, near);
            audio.play_world(sound, gain.min(1.0), pan, 1.0);
        }

        let (mut held, mut loops) = wanted(&frame.wells, &mut ids, |at, reach| {
            far(at, reach.unwrap_or(Far(0.0, 1.0)), 1.0)
        });
        held.extend(
            self.edict
                .tick(frame, blueprints, (&library, fresh), audio, focus, &hear),
        );
        audio.set_held(&held);
        loops.extend(
            self.lens
                .tick(frame, blueprints, (&library, generation), audio, &hear),
        );
        loops
    }
}

/// What every engine at work wants sounding this tick: its held gathering, and its loops
/// as (sound, gain, pan, pitch). `hear` places a point, carrying as far as `Far` says.
fn wanted(
    wells: &[WellView],
    mut ids: impl FnMut(u32) -> Ids,
    hear: impl Fn(Vec3, Option<Far>) -> (f32, f32),
) -> (Vec<Held>, Vec<LoopRow>) {
    let mut held = Vec::new();
    let mut loops = Vec::new();
    let mut want = |sound: Option<SoundId>, (gain, pan): (f32, f32), loud: f32, pitch: f32| {
        if let Some(sound) = sound {
            loops.push((sound, (gain * loud).min(0.9), pan, pitch));
        }
    };
    for w in wells {
        let i = ids(w.blueprint.0 as u32);
        let core = Vec3::from(w.core);
        let (sag, pitch) = if w.powered {
            (1.0, 1.0)
        } else {
            (FALTER_GAIN, FALTER_PITCH)
        };
        match w.stage {
            WellStage::Gathering if w.powered => {
                if let Some(sound) = i.gather {
                    let (gain, pan) = hear(core, Some(GATHER_FAR));
                    held.push(Held {
                        key: GATHER_KEY | w.unit_id as u64,
                        sound,
                        gain: gain.min(1.0),
                        pan,
                        from: w.seconds,
                    });
                }
            }
            // Stalled: the gathering waits, and the rings coast.
            WellStage::Gathering => want(i.hum, hear(core, None), 0.35, 0.8),
            WellStage::Running => {
                // Heard from its front, and from the nearest point of the crack behind it.
                let (from, front) = (Vec3::from(w.from), Vec3::from(w.front));
                let ahead = hear(front, Some(RIFT_FAR));
                let behind = (0..=16)
                    .map(|k| {
                        let (gain, pan) = hear(from.lerp(front, k as f32 / 16.0), None);
                        (gain * 0.45, pan)
                    })
                    .fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
                want(
                    i.rift,
                    if behind.0 > ahead.0 { behind } else { ahead },
                    1.0,
                    1.0,
                );
                want(i.hum, hear(core, None), 0.7 * sag, pitch);
            }
            WellStage::Holding => {
                want(i.hold, hear(Vec3::from(w.mark), None), sag, pitch);
                want(i.hum, hear(core, None), 0.9 * sag, pitch);
            }
        }
    }
    (held, loops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::BlueprintId;

    fn view(stage: WellStage, powered: bool) -> WellView {
        WellView {
            unit_id: 9,
            owner: 0,
            blueprint: BlueprintId(0),
            stage,
            progress: 0.5,
            seconds: 6.5,
            left: 8.5,
            core: [0.0, 0.0, 120.0],
            from: [0.0; 3],
            front: [4000.0, 0.0, 0.0],
            mark: [8000.0, 0.0, 0.0],
            radius: 300.0,
            powered,
        }
    }

    fn ids(_: u32) -> Ids {
        Ids {
            gather: Some(SoundId(1)),
            rift: Some(SoundId(3)),
            hold: Some(SoundId(5)),
            hum: Some(SoundId(6)),
            ..Ids::default()
        }
    }

    /// Heard from the origin: loud there, nothing 2 km off, unless it carries.
    fn hear(at: Vec3, far: Option<Far>) -> (f32, f32) {
        let near = (1.0 - at.truncate().length() / 2000.0).max(0.0);
        let carry = far.map_or(0.0, |f| f.0);
        (near.max(carry), at.x.signum() * 0.5)
    }

    #[test]
    fn a_gathering_is_held_from_where_it_has_got_to() {
        let (held, loops) = wanted(&[view(WellStage::Gathering, true)], ids, hear);
        assert!(loops.is_empty());
        assert_eq!(held.len(), 1);
        assert_eq!((held[0].sound, held[0].from), (SoundId(1), 6.5));
        assert_eq!(held[0].key, GATHER_KEY | 9);
    }

    #[test]
    fn a_stalled_gathering_lets_its_sound_go_and_the_rings_coast() {
        let (held, loops) = wanted(&[view(WellStage::Gathering, false)], ids, hear);
        assert!(held.is_empty());
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].0, SoundId(6));
        assert!(loops[0].1 < 0.5 && loops[0].3 < 1.0);
    }

    #[test]
    fn a_rift_is_heard_from_its_front_or_the_crack_behind_it() {
        let (held, loops) = wanted(&[view(WellStage::Running, true)], ids, hear);
        assert!(held.is_empty());
        let rift = loops.iter().find(|l| l.0 == SoundId(3)).unwrap();
        // The front is out of hearing; the crack runs past the ear, at under half.
        assert!(rift.1 > 0.3 && rift.1 <= 0.45, "{}", rift.1);
        assert!(loops.iter().any(|l| l.0 == SoundId(6)));
    }

    #[test]
    fn a_faltering_well_sags() {
        let mut near = view(WellStage::Holding, true);
        near.mark = [100.0, 0.0, 0.0];
        let mut weak = near;
        weak.powered = false;
        let (_, strong) = wanted(&[near], ids, hear);
        let (_, sagging) = wanted(&[weak], ids, hear);
        for sound in [SoundId(5), SoundId(6)] {
            let a = strong.iter().find(|l| l.0 == sound).unwrap();
            let b = sagging.iter().find(|l| l.0 == sound).unwrap();
            assert!(b.1 < a.1 * 0.5 && b.3 < a.3, "{sound:?}");
        }
    }

    /// The engine's sounds are its own, all named and in the library.
    #[test]
    fn the_fallen_has_all_its_sounds() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let library = SoundLibrary::load(&data).unwrap();
        let blueprints = Blueprints::load(&data).unwrap();
        let engines: Vec<usize> = (0..blueprints.units.len())
            .filter(|&b| blueprints.units[b].gravitic.is_some())
            .collect();
        assert!(!engines.is_empty());
        for b in engines {
            let i = Ids::of(&library, &blueprints, b);
            for s in [
                i.gather, i.launch, i.rift, i.strike, i.hold, i.hum, i.release,
            ] {
                let s = library.sound(s.expect("named and in the library"));
                assert!(s.like.is_none(), "{} is a stand-in", s.name);
            }
            let loops = [i.rift, i.hold, i.hum].map(|s| library.sound(s.unwrap()).looped);
            assert_eq!(loops, [true; 3]);
            let gather = library.sound(i.gather.unwrap());
            let ticks = blueprints.units[b].gravitic.as_ref().unwrap().gather_ticks;
            assert!(
                (gather.length - ticks as f32 / mc_core::TICKS_PER_SECOND as f32).abs() < 0.01,
                "the gathering sound lasts as long as the gathering"
            );
        }
        assert_eq!(gliding(&library, &blueprints).len(), 2);
    }
}
