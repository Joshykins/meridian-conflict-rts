//! Missile defence heard (`SimEvent::MissileLased`). A laser faction's shot is a
//! one-shot zap from its emitter, each tick it fires; a counter-seeker faction's is a
//! low hum, one loop for all of them, while any holds on a missile. Either way the
//! casing giving way is a muffled pop out at the missile.

use glam::Vec3;
use mc_data::{AntiMissileLook, Blueprints};
use mc_sim::SimEvent;

/// What this frame's missile defence sounds like: the hum's summed power and
/// power-weighted pan, and the zaps and pops as (gain, pan, pitch), loudest first.
#[derive(Default)]
pub(crate) struct Heard {
    pub(crate) hum: (f32, f32),
    pub(crate) zaps: Vec<(f32, f32, f32)>,
    pub(crate) breaks: Vec<(f32, f32, f32)>,
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
        match look {
            AntiMissileLook::Laser => out.zaps.push((gain, pan, 0.95 + jitter(from) * 0.1)),
            AntiMissileLook::CounterSeeker => {
                out.hum = (out.hum.0 + gain * gain, out.hum.1 + gain * gain * pan);
            }
        }
        if *killed {
            let (gain, pan) = hear(to);
            out.breaks.push((gain * 0.8, pan, 0.92 + jitter(to) * 0.12));
        }
    }
    out.zaps.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.breaks.sort_by(|a, b| b.0.total_cmp(&a.0));
    out
}
