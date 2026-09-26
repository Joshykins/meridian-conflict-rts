//! How the giants (`Motion::stride`, the Behemoth) and the biggest guns are heard beyond
//! the usual range:
//!
//! - A giant's footfall carries across the map (`UnitSounds::step_far`): near the camera
//!   the ordinary `step` plays (game.rs footfalls); further out this low boom takes over,
//!   cross-faded by how much of the near one is still heard.
//! - A weapon's `far` sound: the same for its hits (the AEB-3's strike).
//! - A rotary gun's `spin`: heard as its barrels start to turn (`Weapon::spin_up`).
//! - A giant rail gun's spent sabot bursting where it lands (`SimEvent::SabotLanded`).
//!
//! Heard the moment it happens, however far, like a nuclear blast (game.rs
//! `nuke_sounds`): the user wants a sound with its event, not the real lag of sound.

use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId};
use mc_sim::mirror::{UnitInstance, KIND_WRECK};
use mc_sim::SimEvent;
use std::collections::{HashMap, HashSet};

/// Ground distance at which a far sound is at half its level, metres.
const FAR_HALF: f32 = 3500.0;
/// The loudest a far sound plays, at the edge of the near sound's reach.
const FAR_PEAK: f32 = 0.55;

#[derive(Clone, Copy, Default)]
struct Ids {
    step_far: Option<SoundId>,
    /// The first rotary gun's spin-up.
    spin: Option<SoundId>,
    /// Per weapon, its far hit.
    far: [Option<SoundId>; mc_data::MAX_WEAPONS],
    /// Per weapon, a spent casing (`sabot`) landing: its `casing` sound, or failing
    /// that its ground hit.
    casing: [Option<SoundId>; mc_data::MAX_WEAPONS],
}

#[derive(Default)]
pub struct GiantSounds {
    /// Units whose rotary barrels were turning last tick.
    spinning: HashSet<u32>,
    /// A rotary gun's turn last tick (radians a tick), and whether its run-down was heard.
    spin_speed: HashMap<u32, (f32, bool)>,
    /// This tick's barrel whir loops (sound, gain, pan, pitch), for the game's loop mix.
    whirs: Vec<(SoundId, f32, f32, f32)>,
    /// `titan_gatling_whir`, `titan_gatling_spindown`.
    rotary: [Option<SoundId>; 2],
    /// Ground shocks the camera feels: where, how hard, when, and how long they ring.
    jolts: Vec<(Vec3, f32, std::time::Instant, f32)>,
    ids: HashMap<u32, Ids>,
    generation: Option<u32>,
}

impl GiantSounds {
    /// One tick's far footfalls, far hits and spin-ups. `hear` is the game's near hearing
    /// (gain, pan); `focus` the ground point the camera looks at; `tick_seconds` how long
    /// this tick lasts at the game's speed.
    pub fn tick(
        &mut self,
        units: &[UnitInstance],
        events: &[SimEvent],
        blueprints: &Blueprints,
        audio: &Audio,
        focus: Vec3,
        tick_seconds: f32,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) {
        let (library, generation) = audio.library();
        if self.generation != Some(generation) {
            self.ids.clear();
            self.generation = Some(generation);
            self.rotary =
                ["titan_gatling_whir", "titan_gatling_spindown"].map(|n| library.id_of(n));
        }
        let [whir, spindown] = self.rotary;
        self.whirs.clear();
        let mut speeds = HashMap::new();
        let mut ids = |blueprint: u32| -> Ids {
            *self.ids.entry(blueprint).or_insert_with(|| {
                let Some(bp) = blueprints.units.get(blueprint as usize) else {
                    return Ids::default();
                };
                let id = |name: &Option<String>| name.as_ref().and_then(|n| library.id_of(n));
                let mut far = [None; mc_data::MAX_WEAPONS];
                let mut casing = [None; mc_data::MAX_WEAPONS];
                for (i, w) in bp.weapons.iter().enumerate().take(mc_data::MAX_WEAPONS) {
                    far[i] = id(&w.sounds.far);
                    casing[i] = id(&w.sounds.casing)
                        .or_else(|| id(&w.sounds.ground))
                        .or_else(|| id(&w.sounds.impact));
                }
                Ids {
                    step_far: id(&bp.sounds.step_far),
                    spin: bp
                        .weapons
                        .iter()
                        .find(|w| w.spin_ticks > 0)
                        .and_then(|w| id(&w.sounds.spin)),
                    far,
                    casing,
                }
            })
        };
        // Far over the ground, and faded in as the near sound fades out.
        let far = |at: Vec3| -> (f32, f32) {
            let (near, pan) = hear(at);
            let d = (at - focus).truncate().length();
            let level = FAR_PEAK / (1.0 + (d / FAR_HALF).powi(2));
            (level * (1.0 - (near * 1.4).min(1.0)), pan)
        };

        let mut jolts = Vec::new();
        let mut spinning = HashSet::new();
        for u in units {
            if u.owner_flags & KIND_WRECK != 0 {
                continue;
            }
            let found = ids(u.blueprint);
            let at = Vec3::from(u.pos);
            if let Some(sound) = found.spin {
                let [before, now, ..] = u.spin_recoil;
                let speed = now - before;
                let (last, mut ran_down) = self
                    .spin_speed
                    .get(&u.unit_id)
                    .copied()
                    .unwrap_or((0.0, true));
                if speed > 1e-4 {
                    spinning.insert(u.unit_id);
                    let (gain, pan) = hear(at);
                    if !self.spinning.contains(&u.unit_id) {
                        audio.play_world(sound, gain, pan, 1.0);
                    }
                    // Coasting down: the barrels lose speed with nothing left to shoot.
                    if speed < last * 0.97 && !ran_down {
                        if let Some(down) = spindown {
                            audio.play_world(down, gain, pan, 1.0);
                        }
                        ran_down = true;
                    } else if speed > last * 1.001 {
                        ran_down = false;
                    }
                    // The cluster turning: a loop that rises with its speed.
                    if let Some(loop_id) = whir {
                        let top = speed.max(last).max(1e-4);
                        let share = (speed / top).clamp(0.0, 1.0);
                        self.whirs.push((
                            loop_id,
                            gain * (0.35 + 0.65 * share),
                            pan,
                            0.7 + 0.3 * share,
                        ));
                    }
                    speeds.insert(u.unit_id, (speed.max(last * 0.999), ran_down));
                }
            }
            let Some(sound) = found.step_far else {
                continue;
            };
            let Some(stomp) = blueprints
                .units
                .get(u.blueprint as usize)
                .and_then(|bp| bp.stomp)
            else {
                continue;
            };
            // A foot comes down as the ground covered passes another pace, as game.rs times
            // the near footfall.
            let pace = stomp.pace.to_f32();
            let [ground, moved, _] = u.gait;
            if moved <= 0.0 {
                continue;
            }
            let landing = (ground / pace).floor() * pace;
            if landing <= ground - moved {
                continue;
            }
            let (gain, pan) = far(at);
            if gain > 0.01 {
                let after = (landing - (ground - moved)) / moved * tick_seconds;
                audio.play_world_after(sound, gain, pan, 1.0, after);
            }
            jolts.push((at, 1.0, 0.7));
        }
        self.spinning = spinning;
        self.spin_speed = speeds;

        for event in events {
            if let SimEvent::SabotLanded {
                pos,
                blueprint,
                weapon,
            } = event
            {
                let sound = ids(blueprint.0 as u32)
                    .casing
                    .get(*weapon as usize)
                    .copied()
                    .flatten();
                if let Some(sound) = sound {
                    let (gain, pan) = hear(Vec3::from(pos.to_f32()));
                    audio.play_world(sound, gain * 0.6, pan, 1.15);
                }
                continue;
            }
            let SimEvent::Impact {
                pos,
                blueprint,
                weapon,
                after,
                ..
            } = event
            else {
                continue;
            };
            let Some(sound) = ids(blueprint.0 as u32)
                .far
                .get(*weapon as usize)
                .copied()
                .flatten()
            else {
                continue;
            };
            let (gain, pan) = far(Vec3::from(pos.to_f32()));
            if gain > 0.01 {
                audio.play_world_after(sound, gain, pan, 1.0, after.to_f32() * tick_seconds);
            }
            // A weapon heard across the map is felt too; a storm keeps the ground shaking.
            let storm = blueprints
                .units
                .get(blueprint.0 as usize)
                .and_then(|bp| bp.weapons.get(*weapon as usize))
                .and_then(|w| w.bore)
                .and_then(|b| b.storm);
            jolts.push((Vec3::from(pos.to_f32()), 3.0, 0.9));
            if let Some(s) = storm {
                jolts.push((Vec3::from(pos.to_f32()), 0.9, s.ticks as f32 * 0.1));
            }
        }
        for (at, strength, ring) in jolts {
            self.jolt(at, strength, ring);
        }
    }
}

impl GiantSounds {
    fn jolt(&mut self, at: Vec3, strength: f32, ring: f32) {
        self.jolts
            .retain(|j| j.2.elapsed().as_secs_f32() < j.3 * 3.0);
        if self.jolts.len() < 32 {
            self.jolts
                .push((at, strength, std::time::Instant::now(), ring));
        }
    }

    /// `camera` as the ground shocks near it shake it this frame: a giant's footfall
    /// nearby, a giant bore's strike and its storm. Nothing far from the view, and
    /// nothing at all once they have died away.
    pub fn shaken(&self, camera: &mc_render::camera::Camera) -> mc_render::camera::Camera {
        let mut shake = 0.0;
        for &(at, strength, when, ring) in &self.jolts {
            let age = when.elapsed().as_secs_f32();
            let d = (at - camera.focus).truncate().length();
            let near = 1.0 / (1.0 + (d / (camera.distance * 1.2 + 300.0)).powi(2));
            // Storms ring on at a level for their life; a blow dies away fast.
            let fade = if ring > 2.0 {
                (1.0 - age / ring).max(0.0)
            } else {
                (-age * 3.0 / ring).exp()
            };
            shake += strength * near * fade;
        }
        let mut out = camera.clone();
        if shake < 0.01 {
            return out;
        }
        let t = self
            .jolts
            .first()
            .map_or(0.0, |j| j.2.elapsed().as_secs_f32());
        let wobble = Vec3::new(
            (t * 23.0).sin() + 0.5 * (t * 37.0).sin(),
            (t * 29.0).cos() + 0.5 * (t * 41.0).sin(),
            0.6 * (t * 31.0).sin(),
        );
        out.focus += wobble * shake.min(3.0) * camera.distance * 0.0025;
        out
    }
}

impl GiantSounds {
    /// The rotary guns turning now, as loops for the game's mix: (sound, gain, pan, pitch).
    pub fn loops(&self) -> Vec<(SoundId, f32, f32, f32)> {
        self.whirs.clone()
    }
}
