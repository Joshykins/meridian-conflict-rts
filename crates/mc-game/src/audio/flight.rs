//! Shots heard while they fly (`WeaponSounds::flight`): a cruise missile's motor.
//! Each flight sound is heard from its loudest few shots, each a voice of its own
//! (`Audio::set_loops` keeps a voice on the shot nearest its pan), so a salvo
//! coming in passes across the ears one missile at a time, and a sky full of them
//! does not eat the mixer.

use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary};
use mc_sim::mirror::FlightSource;

/// Voices per flight sound: the loudest shots. Deliberately capped (cosmetic).
const VOICES: usize = 3;
/// Quieter than this a shot is not heard.
const FLOOR: f32 = 0.01;
/// Loudest one voice plays.
const PEAK: f32 = 0.8;

/// The loops for every shot in flight, as (sound, gain, pan, pitch). `hear` is the
/// game's hearing: gain and pan for a point in the world.
pub(crate) fn loops(
    flights: &[FlightSource],
    blueprints: &Blueprints,
    library: &SoundLibrary,
    hear: impl Fn(Vec3) -> (f32, f32),
) -> Vec<(SoundId, f32, f32, f32)> {
    let heard = flights
        .iter()
        .filter_map(|f| {
            let weapon = blueprints
                .units
                .get(f.blueprint as usize)?
                .weapons
                .get(f.weapon as usize)?;
            let sound = library.id_of(weapon.sounds.flight.as_ref()?)?;
            let (gain, pan) = hear(Vec3::from(f.pos));
            (gain >= FLOOR).then_some((sound, gain, pan))
        })
        .collect();
    voices(heard)
}

/// The loudest `VOICES` of each sound's (sound, gain, pan), as loops.
fn voices(mut heard: Vec<(SoundId, f32, f32)>) -> Vec<(SoundId, f32, f32, f32)> {
    heard.sort_by(|a, b| a.0 .0.cmp(&b.0 .0).then(b.1.total_cmp(&a.1)));
    let mut out = Vec::new();
    let mut run = (None, 0);
    for (sound, gain, pan) in heard {
        run = if run.0 == Some(sound) {
            (run.0, run.1 + 1)
        } else {
            (Some(sound), 1)
        };
        if run.1 <= VOICES {
            out.push((sound, (gain * PEAK).min(PEAK), pan, 1.0));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_sound_is_heard_from_its_loudest_few_shots() {
        let (a, b) = (SoundId(4), SoundId(2));
        let mut heard: Vec<_> = (0..6).map(|k| (a, 0.1 * k as f32, 0.0)).collect();
        heard.push((b, 0.05, -0.5));
        let out = voices(heard);
        let of = |s| {
            out.iter()
                .filter(|l| l.0 == s)
                .map(|l| l.1)
                .collect::<Vec<_>>()
        };
        assert_eq!(of(b).len(), 1);
        let loud = of(a);
        assert_eq!(loud.len(), VOICES);
        assert!(loud.iter().all(|&g| g >= 0.3 * PEAK - 1e-6), "{loud:?}");
    }
}
