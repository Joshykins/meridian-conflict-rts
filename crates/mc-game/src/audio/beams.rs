//! Held beams (`Weapon::beam`, the Naga's Pinched-plasmeric Beam). The sim fires one every
//! tick it bears, so it is heard as its `hold` loop while the shots keep coming, not as a
//! shot and a strike a tick: one loop per sound, from every beam firing this tick.

use glam::Vec3;
use mc_data::{BlueprintId, Blueprints, SoundId};
use mc_sim::SimEvent;

/// Whether `event` is a held beam's shot or strike: heard only as its loop.
pub(crate) fn is_held_beam(event: &SimEvent, bps: &Blueprints) -> bool {
    match event {
        SimEvent::ShotFired {
            blueprint, weapon, ..
        }
        | SimEvent::Impact {
            blueprint, weapon, ..
        } => bps.unit(*blueprint).weapons[*weapon as usize].beam,
        _ => false,
    }
}

/// This tick's held-beam loops as (sound, gain, pan, pitch). `hold` names a weapon's loop;
/// `hear` places a point as (gain, pan).
pub(crate) fn held_beam_loops(
    events: &[SimEvent],
    bps: &Blueprints,
    hold: impl Fn(BlueprintId, u8) -> Option<SoundId>,
    hear: impl Fn(Vec3) -> (f32, f32),
) -> Vec<(SoundId, f32, f32, f32)> {
    // Per sound: summed power, and power-weighted pan.
    let mut sums: Vec<(SoundId, f32, f32)> = Vec::new();
    for event in events {
        let SimEvent::ShotFired {
            pos,
            blueprint,
            weapon,
            ..
        } = event
        else {
            continue;
        };
        if !bps.unit(*blueprint).weapons[*weapon as usize].beam {
            continue;
        }
        let Some(sound) = hold(*blueprint, *weapon) else {
            continue;
        };
        let (gain, pan) = hear(Vec3::from(pos.to_f32()));
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
        .map(|(sound, power, pan)| {
            (
                sound,
                (power.sqrt() * 0.7).min(0.95),
                pan / power.max(1e-9),
                1.0,
            )
        })
        .collect()
}
