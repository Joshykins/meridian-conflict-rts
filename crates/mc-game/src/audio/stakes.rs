//! A siege unit's ground stakes striking home as it plants (`UnitSounds::stake`,
//! `mc_models::stakes`): once a stake, at the moment its spike strikes the ground as the
//! model draws it. The sim reports nothing of it; the strikes are read off the unit's
//! deploy tick by tick, as the renderer's `stake_fx.rs` does.

use super::Audio;
use glam::Vec3;
use mc_data::{Blueprints, SoundId};
use mc_render::models::stakes::STAKES;
use mc_sim::mirror::{UnitInstance, KIND_WRECK};
use std::collections::HashMap;

#[derive(Default)]
pub struct StakeSounds {
    /// Per blueprint, its stake sound.
    ids: HashMap<u32, Option<SoundId>>,
    generation: Option<u32>,
}

impl StakeSounds {
    /// One tick's stake strikes. `hear` is the game's hearing (gain, pan); `tick_seconds`
    /// how long this tick lasts at the game's speed.
    pub fn tick(
        &mut self,
        units: &[UnitInstance],
        blueprints: &Blueprints,
        audio: &Audio,
        tick_seconds: f32,
        hear: impl Fn(Vec3) -> (f32, f32),
    ) {
        let (library, generation) = audio.library();
        if self.generation != Some(generation) {
            self.ids.clear();
            self.generation = Some(generation);
        }
        for u in units {
            if u.owner_flags & KIND_WRECK != 0 || u.deploy <= u.prev_deploy {
                continue;
            }
            let sound = *self.ids.entry(u.blueprint).or_insert_with(|| {
                blueprints
                    .units
                    .get(u.blueprint as usize)
                    .and_then(|bp| bp.sounds.stake.as_ref())
                    .and_then(|n| library.id_of(n))
            });
            let Some(sound) = sound else {
                continue;
            };
            let (before, now) = (u.prev_deploy, u.deploy);
            for (k, stake) in STAKES.into_iter().enumerate() {
                let strikes = stake.strikes();
                if strikes <= before || strikes > now {
                    continue;
                }
                let after = (strikes - before) / (now - before) * tick_seconds;
                let (gain, pan) = hear(Vec3::from(u.pos));
                // No two quite alike.
                let pitch = [1.0, 0.94, 1.04, 0.97][k % 4];
                audio.play_world_after(sound, gain, pan, pitch, after);
            }
        }
    }
}
