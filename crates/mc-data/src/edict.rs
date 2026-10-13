//! The Edict (`UnitBlueprint::edict`, the main gun of the Regency's tech 5 Regent): a
//! weapon with no barrel. Ordered on a point within `range`, the ship gathers for
//! `gather` seconds while every other gun on it is silent, drawing `energy` over the
//! gather (it stalls while its side cannot pay), then fires from the point on its hull
//! nearest the mark: a nuclear-scale `blast` there (falling off from its core to its
//! radius, every side's units hurt alike), then a molten `wave` rolling out over the
//! ground that burns a while, weaker with distance. It is ready again `reload` seconds
//! after the shot. mc-sim `edict.rs`.

use mc_core::{Fx, StateHasher};
use serde::Deserialize;

use crate::strategic::{NuclearBlast, RawNuclearBlast};
use crate::DataError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEdict {
    /// Seconds it gathers before it fires.
    pub gather: f64,
    /// Metres from the point on the hull nearest the mark to the mark, to begin a gather.
    pub range: f64,
    /// Seconds from one shot until it may gather again.
    pub reload: f64,
    /// Energy one gather draws, spread over `gather`.
    pub energy: f64,
    /// Seconds the other guns stay silent after the shot.
    pub quiet: f64,
    pub blast: RawNuclearBlast,
    pub wave: RawEdictWave,
    pub sounds: EdictSounds,
}

/// The molten wave (`RawEdict::wave`): it rolls out `speed` metres a second to `radius`
/// and burns what it has rolled over, `damage` a second at the mark falling to `edge` of
/// that at its rim, until `linger` seconds after it has reached `radius`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEdictWave {
    pub radius: f64,
    pub speed: f64,
    pub damage: f64,
    pub edge: f64,
    pub linger: f64,
}

/// What the Edict sounds like, every one a library sound.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdictSounds {
    /// The gather, at the ship, from its start.
    pub gather: String,
    /// The last moments of the gather, laid over its end.
    pub peak: String,
    /// The shot leaving the hull, near and far.
    pub fire: String,
    pub fire_far: String,
    /// The strike and the wave rolling out, near and far.
    pub impact: String,
    pub impact_far: String,
    /// A gather let go before it fired.
    pub cancel: String,
}

/// The molten wave, in ticks and fixed point (`RawEdictWave`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdictWave {
    pub radius: Fx,
    /// Damage a second at the mark.
    pub damage: Fx,
    /// The share of `damage` dealt at `radius`.
    pub edge: Fx,
    pub roll_ticks: u16,
    pub linger_ticks: u16,
}

impl EdictWave {
    /// Ticks from the shot until the wave is gone.
    pub fn life_ticks(&self) -> u16 {
        self.roll_ticks.saturating_add(self.linger_ticks)
    }

    /// Metres out the wave has rolled `age` ticks after the shot.
    pub fn reach(&self, age: u16) -> Fx {
        if age >= self.roll_ticks {
            return self.radius;
        }
        self.radius * Fx::from_int(age as i32) / Fx::from_int(self.roll_ticks.max(1) as i32)
    }
}

/// The Edict, in ticks and fixed point (`RawEdict`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edict {
    pub gather_ticks: u16,
    pub range: Fx,
    pub reload_ticks: u16,
    /// Energy a second while it gathers.
    pub upkeep: Fx,
    pub quiet_ticks: u16,
    pub blast: NuclearBlast,
    pub wave: EdictWave,
    pub sounds: EdictSounds,
}

impl RawEdict {
    pub(crate) fn compile(
        &self,
        key: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Edict, DataError> {
        let tps = ticks_per_second as f64;
        let ticks = |s: f64| (s * tps).round().clamp(0.0, 6000.0) as u16;
        let (b, w) = (&self.blast, &self.wave);
        let positive = [
            self.gather,
            self.range,
            self.reload,
            b.radius,
            b.damage,
            b.front,
            w.radius,
            w.speed,
            w.damage,
        ];
        if !positive.iter().all(|v| v.is_finite() && *v > 0.0)
            || ![self.energy, self.quiet, b.core, b.edge, w.linger]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
            || b.core > b.radius
            || !(0.0..=1.0).contains(&w.edge)
            || self.reload < self.gather
        {
            return Err(DataError::Invalid(format!(
                "{key}: an Edict needs a gather, a range, a reload no shorter than its gather, \
                 a blast (radius >= core) and a wave over zero, none of its numbers negative, \
                 and a wave edge share from 0 to 1"
            )));
        }
        Ok(Edict {
            gather_ticks: ticks(self.gather).max(1),
            range: fx(self.range),
            reload_ticks: ticks(self.reload).max(1),
            upkeep: fx(self.energy / self.gather),
            quiet_ticks: ticks(self.quiet),
            blast: NuclearBlast {
                radius: fx(b.radius),
                core: fx(b.core),
                damage: fx(b.damage),
                edge: fx(b.edge),
                front_ticks: (b.front * tps).round().max(1.0) as u32,
            },
            wave: EdictWave {
                radius: fx(w.radius),
                damage: fx(w.damage),
                edge: fx(w.edge),
                roll_ticks: ticks(w.radius / w.speed).max(1),
                linger_ticks: ticks(w.linger),
            },
            sounds: self.sounds.clone(),
        })
    }
}

impl Edict {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        let b = &self.blast;
        let w = &self.wave;
        for v in [
            self.range,
            self.upkeep,
            b.radius,
            b.core,
            b.damage,
            b.edge,
            w.radius,
            w.damage,
            w.edge,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(
            self.gather_ticks as u64
                | (self.reload_ticks as u64) << 16
                | (self.quiet_ticks as u64) << 32
                | (w.roll_ticks as u64) << 48,
        );
        h.write_u64(w.linger_ticks as u64 | (b.front_ticks as u64) << 16);
    }

    /// Every sound it names, for the library's check.
    pub fn sound_names(&self) -> [&String; 7] {
        let s = &self.sounds;
        [
            &s.gather,
            &s.peak,
            &s.fire,
            &s.fire_far,
            &s.impact,
            &s.impact_far,
            &s.cancel,
        ]
    }
}
