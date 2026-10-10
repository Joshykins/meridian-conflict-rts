//! A storm seeder at work (`Weapon::cloud`, the Nimbus): heard as its `hold` loop for as
//! long as it is seeding, from the seeds it has just let go, and not as a shot a second.
//! The loop fades out a little after the last seed, once it is no longer young.

use glam::Vec3;
use mc_data::{BlueprintId, Blueprints, SoundId};
use mc_sim::mirror::CloudView;
use mc_sim::SimEvent;

/// A seed counts as just let go for this many of its gun's reloads: one and a half, so the
/// loop holds over the gap between seeds and lets go soon after the last.
const YOUNG_RELOADS: f32 = 1.5;

/// Whether `event` is a seed let go by a seeder that is heard as its loop instead.
pub(crate) fn is_seed_release(
    event: &SimEvent,
    bps: &Blueprints,
    hold: impl Fn(BlueprintId, u8) -> Option<SoundId>,
) -> bool {
    match event {
        SimEvent::ShotFired {
            blueprint, weapon, ..
        } => {
            bps.unit(*blueprint).weapons[*weapon as usize]
                .cloud
                .is_some()
                && hold(*blueprint, *weapon).is_some()
        }
        _ => false,
    }
}

/// This tick's seeding loops as (sound, gain, pan, pitch): one per sound, heard from every
/// seed just let go. `hold` names a weapon's loop; `hear` places a point as (gain, pan).
pub(crate) fn seeding_loops(
    clouds: &[CloudView],
    bps: &Blueprints,
    hold: impl Fn(BlueprintId, u8) -> Option<SoundId>,
    hear: impl Fn(Vec3) -> (f32, f32),
) -> Vec<(SoundId, f32, f32, f32)> {
    // Per sound: summed power, and power-weighted pan.
    let mut sums: Vec<(SoundId, f32, f32)> = Vec::new();
    for c in clouds {
        let reload = bps.unit(c.blueprint).weapons[c.weapon as usize].reload_ticks as f32;
        if c.age as f32 > reload.max(1.0) * YOUNG_RELOADS {
            continue;
        }
        let Some(sound) = hold(c.blueprint, c.weapon) else {
            continue;
        };
        let (gain, pan) = hear(Vec3::from(c.pos));
        let power = gain * gain;
        match sums.iter_mut().find(|s| s.0 == sound) {
            Some(s) => {
                s.1 += power;
                s.2 += power * pan;
            }
            None => sums.push((sound, power, power * pan)),
        }
    }
    sums.into_iter()
        .filter(|s| s.1 > 0.0)
        .map(|(sound, power, pan)| (sound, (power.sqrt() * 0.7).min(0.9), pan / power, 1.0))
        .collect()
}
