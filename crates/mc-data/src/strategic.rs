//! Strategic launchers: the nuclear silo and the interceptor array (`docs/NUKES.md`).
//!
//! Neither carries a weapon in the usual sense. Each assembles rounds one at a time out
//! of its side's income, paid like a build (`missile` at `power`), and keeps up to `stock`.
//! A silo launches only when ordered, at a point anywhere on the map; an array fires by
//! itself at an enemy warhead coming down within `coverage` of it.

use mc_core::Fx;
use mc_core::StateHasher;
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum StrategicKind {
    /// Launches a nuclear warhead on order.
    Nuke,
    /// Shoots down enemy warheads on its own.
    Interceptor,
}

/// What one round costs to assemble, like a unit's `cost`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRoundCost {
    pub mass: f64,
    pub energy: f64,
    pub time: f64,
}

/// A nuclear blast: everything within `core` takes `damage`, falling to `edge` at
/// `radius`; the front runs out to `radius` over `front` seconds, so the far edge of it
/// is hit seconds after the middle.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNuclearBlast {
    pub radius: f64,
    pub core: f64,
    pub damage: f64,
    pub edge: f64,
    pub front: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStrategic {
    pub kind: StrategicKind,
    pub missile: RawRoundCost,
    pub power: f64,
    pub stock: u8,
    pub speed: f64,
    #[serde(default)]
    pub apogee: f64,
    #[serde(default)]
    pub blast: Option<RawNuclearBlast>,
    #[serde(default)]
    pub coverage: f64,
    /// The missile's drawn size against the Sunfall's round (1, the default): a boat's
    /// warhead is a smaller missile than a silo's.
    #[serde(default = "one")]
    pub missile_scale: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NuclearBlast {
    /// Metres out the blast does damage.
    pub radius: Fx,
    /// Metres out it does its full `damage`.
    pub core: Fx,
    pub damage: Fx,
    /// Damage at `radius`.
    pub edge: Fx,
    /// Ticks for the front to run out to `radius`.
    pub front_ticks: u32,
}

impl NuclearBlast {
    /// Damage `distance` metres from the middle: full in the core, falling off
    /// with the square of the way out to `edge`.
    pub fn damage_at(&self, distance: Fx) -> Fx {
        if distance <= self.core {
            return self.damage;
        }
        if distance >= self.radius || self.radius <= self.core {
            return self.edge;
        }
        let out = Fx::ONE - (distance - self.core) / (self.radius - self.core);
        self.edge + (self.damage - self.edge) * out * out
    }

    /// Metres the front has reached `ticks` after the detonation: fast at first, slowing
    /// as it goes, at `radius` after `front_ticks` (a square-root law, like a blast wave).
    pub fn front_at(&self, ticks: u32) -> Fx {
        if ticks >= self.front_ticks || self.front_ticks == 0 {
            return self.radius;
        }
        let t = Fx::from_int(ticks as i32) / Fx::from_int(self.front_ticks as i32);
        // A small head start so the fireball is felt the tick it forms.
        self.radius * (Fx::ratio(1, 12) + Fx::ratio(11, 12) * t.sqrt())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Strategic {
    pub kind: StrategicKind,
    pub round_mass: Fx,
    pub round_energy: Fx,
    /// Build-time units of one round: done at `power` a second in `round_time / power` s.
    pub round_time: Fx,
    pub power: Fx,
    pub stock: u8,
    /// Metres a tick along the flight.
    pub speed: Fx,
    /// Highest the arc goes, metres (a nuke).
    pub apogee: Fx,
    pub blast: Option<NuclearBlast>,
    /// Metres round the array a warhead must be coming down in to be shot at.
    pub coverage: Fx,
    /// Presentation only, and not hashed: the missile body, its plume and its trail are
    /// drawn this size against the Sunfall's (nuke_fx.rs, nuke.wgsl).
    pub missile_scale: Fx,
}

impl RawStrategic {
    pub(crate) fn compile(
        &self,
        key: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Strategic, DataError> {
        let bad = |what: &str| Err(DataError::Invalid(format!("{key}: strategic {what}")));
        if self.stock == 0 || self.power <= 0.0 || self.missile.time <= 0.0 || self.speed <= 0.0 {
            return bad("needs a stock, a power, a round time and a speed");
        }
        let blast = match (self.kind, &self.blast) {
            (StrategicKind::Nuke, Some(b)) => {
                if b.radius <= 0.0
                    || b.core < 0.0
                    || b.core > b.radius
                    || b.damage <= 0.0
                    || b.front <= 0.0
                {
                    return bad("blast needs radius >= core >= 0, damage and a front time");
                }
                Some(NuclearBlast {
                    radius: fx(b.radius),
                    core: fx(b.core),
                    damage: fx(b.damage),
                    edge: fx(b.edge.max(0.0)),
                    front_ticks: (b.front * ticks_per_second as f64).round().max(1.0) as u32,
                })
            }
            (StrategicKind::Nuke, None) => return bad("a nuke needs a blast"),
            (StrategicKind::Interceptor, Some(_)) => return bad("an interceptor has no blast"),
            (StrategicKind::Interceptor, None) => None,
        };
        if !(0.1..=4.0).contains(&self.missile_scale) {
            return bad("missile_scale must be between 0.1 and 4");
        }
        if self.kind == StrategicKind::Interceptor && self.coverage <= 0.0 {
            return bad("an interceptor needs a coverage");
        }
        Ok(Strategic {
            kind: self.kind,
            round_mass: fx(self.missile.mass),
            round_energy: fx(self.missile.energy),
            round_time: fx(self.missile.time),
            power: fx(self.power),
            stock: self.stock,
            speed: fx(self.speed / ticks_per_second as f64),
            apogee: fx(self.apogee.max(0.0)),
            blast,
            coverage: fx(self.coverage.max(0.0)),
            missile_scale: fx(self.missile_scale),
        })
    }
}

impl Strategic {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.kind as u64 | (self.stock as u64) << 8);
        for v in [
            self.round_mass,
            self.round_energy,
            self.round_time,
            self.power,
            self.speed,
            self.apogee,
            self.coverage,
        ] {
            h.write_i64(v.0);
        }
        match &self.blast {
            Some(b) => {
                for v in [b.radius, b.core, b.damage, b.edge] {
                    h.write_i64(v.0);
                }
                h.write_u64(b.front_ticks as u64);
            }
            None => h.write_u64(u64::MAX),
        }
    }

    /// Seconds one round takes at full supply.
    pub fn round_seconds(&self) -> f32 {
        self.round_time.to_f32() / self.power.to_f32().max(0.001)
    }
}
