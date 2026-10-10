//! A gravitic engine (`UnitBlueprint::gravitic`, the Regency's tier 5 "The Fallen"): a
//! structure that, given a point anywhere on the map, gathers, then tears the ground open
//! in a rift that runs out to the point, and where the rift strikes the ground shatters
//! into a gravity well. The well holds until the engine is given another point, is
//! stopped or dies: everything of the enemy's inside it is crushed (the same
//! anywhere in it) and slowed, and an aircraft that is not an experimental is torn out of the sky.
//! A shield dome over it takes the crush instead of what is under it, and its own hit
//! points go as a unit's would. mc-sim `gravitic.rs`.

use mc_core::{Fx, FxVec3, StateHasher};
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGravitic {
    /// Seconds it gathers where it stands before the rift leaves.
    pub gather: f64,
    /// Metres a second the rift runs through the ground.
    pub speed: f64,
    /// Metres: the well's reach round the point.
    pub radius: f64,
    /// A second, the same anywhere in the well, while it holds.
    pub damage: f64,
    /// The share of its speed a unit in the well keeps.
    pub slow: f64,
    /// Aircraft of this tech and under are torn out of the sky in it; above, they are
    /// crushed and slowed like everything else.
    pub downs: u8,
    /// Energy a second while it gathers, runs a rift or holds a well.
    pub upkeep: f64,
    /// Where the held gravity sits in the model (x forward, y left, z up, metres): the
    /// rift leaves the ground under it.
    pub core: (f64, f64, f64),
    pub sounds: GraviticSounds,
}

/// What a gravitic engine sounds like, every one a library sound.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraviticSounds {
    /// Its gathering, at the engine.
    pub gather: String,
    /// The rift tearing out of the ground at its foot.
    pub launch: String,
    /// The rift running, heard where its front is (a loop).
    pub rift: String,
    /// The rift striking home: the ground shattering.
    pub strike: String,
    /// The well holding (a loop).
    pub hold: String,
    /// The engine itself while it holds a well (a loop).
    pub hum: String,
    /// The well let go: the ground falling back.
    pub release: String,
    /// An aircraft torn out of the sky.
    pub downed: String,
}

/// A gravitic engine, in ticks and fixed point (`RawGravitic`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gravitic {
    pub gather_ticks: u16,
    /// Metres a tick.
    pub step: Fx,
    pub radius: Fx,
    /// A second.
    pub damage: Fx,
    pub slow: Fx,
    pub downs: u8,
    /// A second.
    pub upkeep: Fx,
    pub core: FxVec3,
    pub sounds: GraviticSounds,
}

impl RawGravitic {
    pub(crate) fn compile(
        &self,
        key: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Gravitic, DataError> {
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        let share = |v: f64| ok(v) && v <= 1.0;
        if !(self.gather > 0.0 && self.speed > 0.0 && self.radius > 0.0)
            || ![
                self.gather,
                self.speed,
                self.radius,
                self.damage,
                self.upkeep,
            ]
            .into_iter()
            .all(ok)
            || !share(self.slow)
        {
            return Err(DataError::Invalid(format!(
                "{key}: a gravitic engine needs a gather, a speed and a radius, none of its numbers negative, and its slow share from 0 to 1"
            )));
        }
        let (x, y, z) = self.core;
        Ok(Gravitic {
            gather_ticks: (self.gather * ticks_per_second as f64)
                .round()
                .clamp(1.0, 6000.0) as u16,
            step: fx(self.speed / ticks_per_second as f64),
            radius: fx(self.radius),
            damage: fx(self.damage),
            slow: fx(self.slow),
            downs: self.downs,
            upkeep: fx(self.upkeep),
            core: FxVec3::new(fx(x), fx(y), fx(z)),
            sounds: self.sounds.clone(),
        })
    }
}

impl Gravitic {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for v in [
            self.step,
            self.radius,
            self.damage,
            self.slow,
            self.upkeep,
            self.core.x,
            self.core.y,
            self.core.z,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(self.gather_ticks as u64 | (self.downs as u64) << 16);
    }

    /// Every sound it names, for the library's check.
    pub fn sound_names(&self) -> [&String; 8] {
        let s = &self.sounds;
        [
            &s.gather, &s.launch, &s.rift, &s.strike, &s.hold, &s.hum, &s.release, &s.downed,
        ]
    }
}
