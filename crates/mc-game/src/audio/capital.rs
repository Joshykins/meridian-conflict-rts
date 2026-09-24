//! How a capital ship (`UnitBlueprint::is_capital_ship`: the Bastion, and the
//! spacecraft to come) is heard. The sim reports nothing about a landing or a
//! take-off, so this reads them off the ship's motion tick by tick, the way the
//! renderer's drive effects do (mc-render `renderer/capital_fx.rs`):
//!
//! - Loops, levels and pitch following the drives: the reactor hum (the unit's
//!   `moving` sound) whenever it is off the ground, the stern engines' roar with
//!   throttle, the lift jets' roar swelling as the ground comes up.
//! - One-shots at the turns: the drives spooling up as the ramp swings shut for
//!   take-off, the lift-off, the touchdown (a thud, the legs taking the weight, the
//!   drives winding down), and the ramp's motor opening and closing.
//!
//! Each sound is looked up per blueprint as `<moving>_<part>` (the unit's `moving`
//! sound name, e.g. `aster_lift_ship_thrust`), falling back to `capital_<part>`
//! (`data/sounds/capital_ship.ron`), so a new ship can take the defaults or name its
//! own. Parts: `thrust`, `lift_jets` (loops), `spool`, `liftoff`, `touchdown`,
//! `ramp_open`, `ramp_close`. A missing sound is simply not played.
//!
//! Height over the ground comes from the landing gear the mirror carries (all out
//! at the ground, in by 60 m; transports only), and above that from the last ground
//! it measured. A ship without gear is always heard as airborne.

use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId, SoundLibrary, UnitBlueprint};
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_RADAR, UNIT_GEAR_SHIFT};
use std::collections::HashMap;

/// Seconds a sim tick covers at normal speed.
const TICK: f32 = 0.1;
/// The gear is all the way in this many metres up (mc-sim `transport::GEAR_HEIGHT`).
const GEAR_HEIGHT: f32 = 60.0;

#[derive(Clone, Copy, Default)]
struct Ids {
    drive: Option<SoundId>,
    thrust: Option<SoundId>,
    jets: Option<SoundId>,
    spool: Option<SoundId>,
    liftoff: Option<SoundId>,
    touchdown: Option<SoundId>,
    ramp_open: Option<SoundId>,
    ramp_close: Option<SoundId>,
}

#[derive(Clone, Copy)]
struct Ship {
    /// Where it was, and its velocity, last tick.
    pos: Vec3,
    vel: Vec3,
    /// Ground under it as last measured by the gear (it moves slowly for a ship this size).
    ground: Option<f32>,
    landed: bool,
    deploy: f32,
    /// The take-off spool has been heard for this stay on the ground.
    spooled: bool,
    /// Smoothed 0..1.
    throttle: f32,
    lift: f32,
    /// Tick counter it was last seen on.
    seen: u64,
}

/// What a ship's drives are doing this tick, for the loops.
struct Drives {
    pos: Vec3,
    airborne: bool,
    throttle: f32,
    lift: f32,
    ids: Ids,
}

#[derive(Default)]
pub struct CapitalSounds {
    ships: HashMap<u32, Ship>,
    /// Sound ids by blueprint, for the library generation they were looked up in.
    ids: HashMap<u32, Ids>,
    generation: Option<u32>,
    tick: u64,
}

/// Whether `u` is heard here and not as an ordinary mover.
pub fn is_capital(u: &UnitInstance, blueprints: &Blueprints) -> bool {
    blueprints.units.get(u.blueprint as usize).is_some_and(|bp| bp.is_capital_ship())
}

impl CapitalSounds {
    /// One sim tick. Plays the one-shots and returns the loops wanted this tick,
    /// as `Audio::set_loops` takes them. `hear` is the game's (gain, pan) for a place.
    pub fn tick(
        &mut self,
        units: &[UnitInstance],
        blueprints: &Blueprints,
        audio: &Audio,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) -> Vec<(SoundId, f32, f32, f32)> {
        self.tick += 1;
        let (library, generation) = audio.library();
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.ids.clear();
            // Every capital ship's loops glide in pitch with its drives.
            let gliding: Vec<SoundId> = blueprints
                .units
                .iter()
                .filter(|bp| bp.is_capital_ship())
                .flat_map(|bp| {
                    let ids = Ids::of(&library, bp);
                    [ids.drive, ids.thrust, ids.jets]
                })
                .flatten()
                .collect();
            audio.set_gliding(&gliding);
        }
        let mut drives = Vec::new();
        for u in units {
            if u.owner_flags & (KIND_WRECK | STATE_RADAR) != 0 || u.build < 1.0 {
                continue;
            }
            let Some(bp) = blueprints.units.get(u.blueprint as usize).filter(|bp| bp.is_capital_ship()) else {
                continue;
            };
            let ids = *self.ids.entry(u.blueprint).or_insert_with(|| Ids::of(&library, bp));
            let cruise = bp.motion.map_or(78.0, |m| m.speed.to_f32()).max(1.0);
            if let Some(d) = self.ship(u, &ids, cruise, audio, &hear) {
                drives.push(d);
            }
        }
        let now = self.tick;
        self.ships.retain(|_, s| now - s.seen < 50);
        loops(&drives, &hear)
    }

    fn ship(&mut self, u: &UnitInstance, ids: &Ids, cruise: f32, audio: &Audio, hear: &impl Fn(Vec3) -> (f32, f32)) -> Option<Drives> {
        let pos = Vec3::from(u.pos);
        let gear = ((u._pad3[0] >> UNIT_GEAR_SHIFT) & 0xFF) as f32 / 255.0;
        let tick = self.tick;
        let fresh = !self.ships.contains_key(&u.unit_id);
        let ship = self.ships.entry(u.unit_id).or_insert(Ship {
            pos,
            vel: Vec3::ZERO,
            ground: None,
            landed: gear >= 0.999,
            deploy: u.deploy,
            spooled: false,
            throttle: 0.0,
            lift: 0.0,
            seen: tick,
        });
        let skipped = tick - ship.seen > 1;
        let vel = if fresh || skipped { Vec3::ZERO } else { (pos - ship.pos) / TICK };
        let accel = (vel - ship.vel).length() / TICK;
        if gear > 0.0 {
            ship.ground = Some(pos.z - (1.0 - gear) * GEAR_HEIGHT);
        }
        let height = match ship.ground {
            Some(g) if gear > 0.0 || (pos.z - g) < 400.0 => (pos.z - g).max(0.0),
            _ => f32::INFINITY,
        };
        let landed = gear >= 0.985 && vel.z.abs() < 2.0;
        let (gain, pan) = hear(pos);
        let pitch = 0.97 + (u.unit_id % 7) as f32 * 0.01;
        if !fresh && !skipped {
            // Touchdown: the ground taken, the legs loaded, the drives let go.
            if landed && !ship.landed {
                play(audio, ids.touchdown, gain * 1.1, pan, pitch);
                ship.spooled = false;
            }
            // Lift-off: the ground let go of.
            if !landed && ship.landed {
                if !ship.spooled {
                    play(audio, ids.spool, gain * 0.9, pan, pitch);
                }
                play(audio, ids.liftoff, gain * 1.1, pan, pitch);
                ship.spooled = false;
            }
            // The ramp: opening from shut, closing from open. It only swings shut on the
            // ground when the ship is about to leave, so the drives spool up with it.
            let (was, now) = (ship.deploy, u.deploy);
            if now > was + 1e-4 && was <= 0.02 {
                play(audio, ids.ramp_open, gain * 0.8, pan, pitch);
            }
            if now < was - 1e-4 && was >= 0.98 {
                play(audio, ids.ramp_close, gain * 0.8, pan, pitch);
                if landed && !ship.spooled {
                    play(audio, ids.spool, gain * 0.9, pan, pitch);
                    ship.spooled = true;
                }
            }
        }
        let closing = landed && u.deploy < ship.deploy - 1e-4;
        let push = (vel.truncate().length() / cruise * 0.75 + accel / 9.0 * 0.5 + vel.z.abs() / 30.0 * 0.3).clamp(0.0, 1.0);
        let near = (1.0 - (height - 30.0) / 150.0).clamp(0.0, 1.0);
        let lift_goal = if landed {
            if closing { 0.4 * (1.0 - u.deploy) } else { 0.0 }
        } else {
            near * (0.55 + 0.45 * (vel.z.abs() / 14.0).min(1.0))
        };
        let ease = |from: f32, to: f32, up: f32, down: f32| from + (to - from) * if to > from { up } else { down };
        ship.throttle = ease(ship.throttle, push, 0.25, 0.1);
        ship.lift = ease(ship.lift, lift_goal, 0.35, 0.12);
        ship.pos = pos;
        ship.vel = vel;
        ship.landed = landed;
        ship.deploy = u.deploy;
        ship.seen = tick;
        let airborne = !landed || closing;
        (airborne || ship.lift > 0.02).then_some(Drives { pos, airborne, throttle: ship.throttle, lift: ship.lift, ids: *ids })
    }
}

impl Ids {
    /// `bp`'s sounds: `<moving>_<part>` where the library has it, else `capital_<part>`.
    fn of(library: &SoundLibrary, bp: &UnitBlueprint) -> Ids {
        let own = bp.sounds.moving.as_deref();
        let part = |name: &str| {
            own.and_then(|m| library.id_of(&format!("{m}_{name}")))
                .or_else(|| library.id_of(&format!("capital_{name}")))
        };
        Ids {
            drive: own.and_then(|m| library.id_of(m)),
            thrust: part("thrust"),
            jets: part("lift_jets"),
            spool: part("spool"),
            liftoff: part("liftoff"),
            touchdown: part("touchdown"),
            ramp_open: part("ramp_open"),
            ramp_close: part("ramp_close"),
        }
    }
}

fn play(audio: &Audio, sound: Option<SoundId>, gain: f32, pan: f32, pitch: f32) {
    if let Some(sound) = sound {
        audio.play_world(sound, gain.min(1.0), pan, pitch);
    }
}

/// The loops for every ship with its drives alight: each sound heard once, from the
/// loudest ship playing it, a little louder for each other one.
fn loops(drives: &[Drives], hear: &impl Fn(Vec3) -> (f32, f32)) -> Vec<(SoundId, f32, f32, f32)> {
    // Per sound: (gain, pitch, pan, the others' gain summed).
    let mut best: Vec<(SoundId, f32, f32, f32, f32)> = Vec::new();
    for d in drives {
        let (gain, pan) = hear(d.pos);
        let t = d.throttle;
        let rows = [
            // The reactor under it all, a little brighter as the drives work.
            (d.ids.drive, if d.airborne { gain * (0.3 + 0.15 * t) } else { 0.0 }, 0.95 + 0.1 * t),
            // Stern engines: a low burn hanging still, a roar under way.
            (d.ids.thrust, if d.airborne { gain * (0.15 + 0.65 * t) } else { 0.0 }, 0.86 + 0.28 * t),
            // Lift jets: swell as the ground comes up.
            (d.ids.jets, gain * 0.85 * d.lift, 0.9 + 0.16 * d.lift),
        ];
        for (sound, g, p) in rows {
            let Some(sound) = sound.filter(|_| g >= 0.004) else { continue };
            match best.iter_mut().find(|b| b.0 == sound) {
                Some(b) if b.1 >= g => b.4 += g,
                Some(b) => *b = (sound, g, p, pan, b.4 + b.1),
                None => best.push((sound, g, p, pan, 0.0)),
            }
        }
    }
    best.into_iter().map(|(sound, g, p, pan, others)| (sound, (g + others * 0.3).min(0.7), pan, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drives(throttle: f32, lift: f32) -> Drives {
        let ids = Ids { drive: Some(SoundId(1)), thrust: Some(SoundId(2)), jets: Some(SoundId(3)), ..Ids::default() };
        Drives { pos: Vec3::ZERO, airborne: true, throttle, lift, ids }
    }

    #[test]
    fn loops_follow_the_drives() {
        let hear = |_: Vec3| (1.0, 0.0);
        let idle = loops(&[drives(0.0, 0.0)], &hear);
        let full = loops(&[drives(1.0, 0.0)], &hear);
        let low = loops(&[drives(0.3, 1.0)], &hear);
        let thrust = |l: &[(SoundId, f32, f32, f32)]| l.iter().find(|r| r.0 == SoundId(2)).copied();
        let (i, f) = (thrust(&idle).unwrap(), thrust(&full).unwrap());
        assert!(f.1 > i.1 * 3.0 && f.3 > i.3, "throttle raises the roar and its pitch");
        assert!(idle.iter().all(|r| r.0 != SoundId(3)), "no lift jets high up and idle");
        assert!(low.iter().any(|r| r.0 == SoundId(3) && r.1 > 0.5), "lift jets roar near the ground");
        // Several ships make one voice per sound, not a wall.
        let fleet = loops(&[drives(1.0, 1.0), drives(1.0, 1.0), drives(1.0, 1.0)], &hear);
        assert_eq!(fleet.len(), 3);
        assert!(fleet.iter().all(|r| r.1 <= 0.7));
    }
}
