//! Missile defence heard (`SimEvent::MissileLased`), one sound a tick of burn from the
//! emitter, by the faction's look: a laser faction's shot is a zap (`intercept_laser`), a
//! gravity crush's grip a low thrum (`intercept_grip`). The round giving way is heard out at
//! the missile: a laser's kill a muffled pop (`intercept_break`), a crush's a short indrawn
//! breath into a dull thump (`intercept_implode`).

use crate::audio::Audio;
use glam::Vec3;
use mc_data::{AntiMissileLook, Blueprints, SoundId, SoundLibrary};
use mc_sim::SimEvent;

/// One sound heard: its gain, pan and pitch, and the look it is the sound of.
#[derive(Clone, Copy)]
pub(crate) struct Hit {
    gain: f32,
    pan: f32,
    pitch: f32,
    look: AntiMissileLook,
}

/// What this frame's missile defence sounds like: the shots and kills, loudest first.
#[derive(Default)]
pub(crate) struct Heard {
    shots: Vec<Hit>,
    kills: Vec<Hit>,
}

/// The sounds missile defence plays, by look.
pub(crate) struct Sounds {
    laser: Option<SoundId>,
    grip: Option<SoundId>,
    laser_kill: Option<SoundId>,
    crush_kill: Option<SoundId>,
}

impl Sounds {
    pub(crate) fn new(library: &SoundLibrary) -> Sounds {
        Sounds {
            laser: library.id_of("intercept_laser"),
            grip: library.id_of("intercept_grip"),
            laser_kill: library.id_of("intercept_break"),
            crush_kill: library.id_of("intercept_implode"),
        }
    }

    /// Plays this frame's missile defence. Deliberately capped: past the loudest few, more
    /// shots in one tick only smear into noise.
    pub(crate) fn play(&self, heard: &Heard, audio: &Audio) {
        for h in heard.shots.iter().take(3) {
            let sound = match h.look {
                AntiMissileLook::Laser => self.laser,
                AntiMissileLook::Gravitic => self.grip,
            };
            if let Some(sound) = sound {
                audio.play_world(sound, h.gain, h.pan, h.pitch);
            }
        }
        for h in heard.kills.iter().take(2) {
            let sound = match h.look {
                AntiMissileLook::Laser => self.laser_kill,
                AntiMissileLook::Gravitic => self.crush_kill,
            };
            if let Some(sound) = sound {
                audio.play_world(sound, h.gain, h.pan, h.pitch);
            }
        }
    }
}

/// A little pitch spread, from where it happened, so a battery does not ring as one.
fn jitter(at: Vec3) -> f32 {
    ((at.x * 12.9898 + at.y * 78.233).sin() * 43_758.547)
        .fract()
        .abs()
}

/// `hear` is the game's hearing: gain and pan for a point in the world.
pub(crate) fn heard(
    events: &[SimEvent],
    blueprints: &Blueprints,
    hear: impl Fn(Vec3) -> (f32, f32),
) -> Heard {
    let mut out = Heard::default();
    for event in events {
        let SimEvent::MissileLased {
            from,
            to,
            killed,
            blueprint,
        } = event
        else {
            continue;
        };
        let from = Vec3::from(from.to_f32());
        let to = Vec3::from(to.to_f32());
        let look = blueprints
            .factions
            .get(blueprints.unit(*blueprint).faction.0 as usize)
            .map_or(AntiMissileLook::Laser, |f| f.anti_missile_look);
        let (gain, pan) = hear(from);
        out.shots.push(Hit {
            gain,
            pan,
            pitch: 0.95 + jitter(from) * 0.1,
            look,
        });
        if *killed {
            let (gain, pan) = hear(to);
            out.kills.push(Hit {
                gain: gain * 0.8,
                pan,
                pitch: 0.92 + jitter(to) * 0.12,
                look,
            });
        }
    }
    out.shots.sort_by(|a, b| b.gain.total_cmp(&a.gain));
    out.kills.sort_by(|a, b| b.gain.total_cmp(&a.gain));
    out
}
