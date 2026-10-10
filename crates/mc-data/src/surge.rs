//! A surge shell (`Weapon::surge`, the Regency Saker's mortar): an ordinary artillery
//! shell that strikes hard where it lands, the weapon's `damage` over its `splash`, and
//! then lets go a wake that rolls out all round over the ground from the crater, `speed`
//! metres a second, to `radius`. Everything of the enemy's the wake has rolled over (never
//! aircraft or spacecraft) takes a light `damage` a second, the same anywhere in it, until
//! `linger` seconds after the wake reached its full radius. mc-sim `surge.rs`.

use mc_core::Fx;
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSurge {
    /// Metres out from the crater the wake rolls.
    pub radius: f64,
    /// Metres a second it rolls out.
    pub speed: f64,
    /// Damage a second to everything in it.
    pub damage: f64,
    /// Seconds the ground it rolled over keeps burning once it has reached `radius`.
    pub linger: f64,
}

/// A surge, in ticks and fixed point (`RawSurge`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surge {
    pub radius: Fx,
    pub speed: Fx,
    /// Damage a second.
    pub damage: Fx,
    /// Ticks the wake takes to reach `radius`, and ticks it burns after.
    pub roll_ticks: u16,
    pub linger_ticks: u16,
}

impl Surge {
    /// Ticks from the shell landing until the wake is gone.
    pub fn life_ticks(&self) -> u16 {
        self.roll_ticks + self.linger_ticks
    }

    /// Metres out the wake has rolled `age` ticks after the shell landed.
    pub fn reach(&self, age: u16) -> Fx {
        if age >= self.roll_ticks {
            return self.radius;
        }
        self.radius * Fx::from_int(age as i32) / Fx::from_int(self.roll_ticks.max(1) as i32)
    }
}

impl RawSurge {
    pub(crate) fn compile(
        &self,
        ctx: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Surge, DataError> {
        let ticks = |s: f64| (s * ticks_per_second as f64).round().clamp(0.0, 6000.0) as u16;
        if !(self.radius > 0.0 && self.speed > 0.0 && self.damage > 0.0 && self.linger >= 0.0) {
            return Err(DataError::Invalid(format!(
                "{ctx}: a surge needs a radius, a speed and a damage over zero and a linger of \
                 zero or more"
            )));
        }
        Ok(Surge {
            radius: fx(self.radius),
            speed: fx(self.speed),
            damage: fx(self.damage),
            roll_ticks: ticks(self.radius / self.speed).max(1),
            linger_ticks: ticks(self.linger),
        })
    }
}
