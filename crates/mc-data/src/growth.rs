//! Units that grow by level (`UnitBlueprint::growth`): survival's Shapers. The
//! blueprint's own figures are its level 1; each stat named here grows from
//! that figure as the unit's level rises, the level set by whoever owns the
//! unit's growth (the survival engine). A stat not named stays as it is.
//!
//! A stat at level `L` is `base * (1 + percent/100)^(L-1) + per_level * (L-1)`,
//! held within `least` and `most` where they are given. Levels have no
//! ceiling; a figure saturates far past anything a match reaches
//! (`Grow::LIMIT`).
//!
//! ```ron
//! growth: (
//!     health: (percent: 15),
//!     shield_radius: (per_level: 20, most: 900),
//!     lance_reload: (percent: -4, least: 1.5),
//! ),
//! ```

use mc_core::{Fx, StateHasher};
use serde::Deserialize;

use crate::DataError;

/// How one stat grows, as written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RawGrow {
    /// Percent more (less, when negative) each level, compounding.
    pub percent: f64,
    /// Added each level after the first (seconds for a time, metres for a length).
    pub per_level: f64,
    pub most: Option<f64>,
    pub least: Option<f64>,
}

/// How one stat grows, compiled: `percent` in thousandths, the rest in the
/// stat's own fixed-point units (ticks for a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grow {
    pub permille: i32,
    pub per_level: Fx,
    pub most: Fx,
    pub least: Fx,
}

impl Default for Grow {
    fn default() -> Self {
        Grow {
            permille: 0,
            per_level: Fx::ZERO,
            most: Grow::LIMIT,
            least: Fx::ZERO,
        }
    }
}

impl Grow {
    /// The most any grown figure reaches: a billion, far past any match.
    pub const LIMIT: Fx = Fx(1_000_000_000 << Fx::FRAC_BITS);

    /// Whether this stat grows at all.
    pub fn grows(&self) -> bool {
        *self != Grow::default()
    }

    /// The stat at `level` from its level-1 figure `base`.
    pub fn at(&self, base: Fx, level: u16) -> Fx {
        let steps = level.max(1) as u32 - 1;
        // (1000 + permille)^steps / 1000^steps by squaring, in 32.32 fixed point.
        let mut factor: i128 = 1 << 32;
        let mut sq: i128 = ((1000 + self.permille.max(-999)) as i128) << 32;
        sq = (sq + 500) / 1000;
        let mut n = steps;
        // A factor of 2^28 saturates any stat; kept there so no product overflows.
        let cap: i128 = 1 << 60;
        while n > 0 {
            if n & 1 == 1 {
                factor = ((factor * sq) >> 32).min(cap);
            }
            sq = ((sq * sq) >> 32).min(cap);
            n >>= 1;
        }
        let grown = (base.0 as i128 * factor) >> 32;
        let added = self.per_level.0 as i128 * steps as i128;
        let v = (grown + added).clamp(0, Grow::LIMIT.0 as i128) as i64;
        Fx(v).clamp(self.least, self.most)
    }

    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.permille as u32 as u64);
        for v in [self.per_level, self.most, self.least] {
            h.write_i64(v.0);
        }
    }
}

impl RawGrow {
    /// `scale` turns the written figures into the stat's units (ticks a second for a time).
    fn compile(&self, scale: f64) -> Grow {
        let fx = |v: f64| Fx((v * scale * Fx::ONE.0 as f64).round() as i64);
        Grow {
            permille: (self.percent * 10.0).round().clamp(-999.0, 100_000.0) as i32,
            per_level: fx(self.per_level),
            most: self.most.map_or(Grow::LIMIT, |m| fx(m).min(Grow::LIMIT)),
            least: self.least.map_or(Fx::ZERO, |l| fx(l).max(Fx::ZERO)),
        }
    }
}

/// What grows with a unit's level, as written. Each names a stat of another
/// block of the blueprint (`health`, `shield`, `lance`, `strategic`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RawGrowth {
    pub health: RawGrow,
    pub shield_radius: RawGrow,
    pub shield_health: RawGrow,
    pub shield_regen: RawGrow,
    pub lance_damage: RawGrow,
    pub lance_splash: RawGrow,
    pub lance_range: RawGrow,
    /// Seconds.
    pub lance_charge: RawGrow,
    /// Seconds.
    pub lance_reload: RawGrow,
    /// Interceptors held; the figure is rounded down.
    pub interceptor_stock: RawGrow,
    pub interceptor_coverage: RawGrow,
    /// Build-time units of one interceptor.
    pub interceptor_round: RawGrow,
}

/// What grows with a unit's level (`UnitBlueprint::growth`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Growth {
    pub health: Grow,
    pub shield_radius: Grow,
    pub shield_health: Grow,
    pub shield_regen: Grow,
    pub lance_damage: Grow,
    pub lance_splash: Grow,
    pub lance_range: Grow,
    /// Ticks.
    pub lance_charge: Grow,
    /// Ticks.
    pub lance_reload: Grow,
    pub interceptor_stock: Grow,
    pub interceptor_coverage: Grow,
    pub interceptor_round: Grow,
}

/// Which of a blueprint's blocks a growth block may name.
pub(crate) struct Has {
    pub shield: bool,
    pub lance: bool,
    pub interceptor: bool,
}

impl RawGrowth {
    pub(crate) fn compile(
        &self,
        key: &str,
        has: Has,
        ticks_per_second: u32,
    ) -> Result<Growth, DataError> {
        let tps = ticks_per_second as f64;
        let g = Growth {
            health: self.health.compile(1.0),
            shield_radius: self.shield_radius.compile(1.0),
            shield_health: self.shield_health.compile(1.0),
            shield_regen: self.shield_regen.compile(1.0),
            lance_damage: self.lance_damage.compile(1.0),
            lance_splash: self.lance_splash.compile(1.0),
            lance_range: self.lance_range.compile(1.0),
            lance_charge: self.lance_charge.compile(tps),
            lance_reload: self.lance_reload.compile(tps),
            interceptor_stock: self.interceptor_stock.compile(1.0),
            interceptor_coverage: self.interceptor_coverage.compile(1.0),
            interceptor_round: self.interceptor_round.compile(1.0),
        };
        let shield = [g.shield_radius, g.shield_health, g.shield_regen];
        let lance = [
            g.lance_damage,
            g.lance_splash,
            g.lance_range,
            g.lance_charge,
            g.lance_reload,
        ];
        let interceptor = [
            g.interceptor_stock,
            g.interceptor_coverage,
            g.interceptor_round,
        ];
        let bad = |what: &str| {
            Err(DataError::Invalid(format!(
                "{key}: growth names {what} stats but the unit has no {what}"
            )))
        };
        if !has.shield && shield.iter().any(Grow::grows) {
            return bad("shield");
        }
        if !has.lance && lance.iter().any(Grow::grows) {
            return bad("lance");
        }
        if !has.interceptor && interceptor.iter().any(Grow::grows) {
            return bad("interceptor");
        }
        Ok(g)
    }
}

impl Growth {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for g in [
            self.health,
            self.shield_radius,
            self.shield_health,
            self.shield_regen,
            self.lance_damage,
            self.lance_splash,
            self.lance_range,
            self.lance_charge,
            self.lance_reload,
            self.interceptor_stock,
            self.interceptor_coverage,
            self.interceptor_round,
        ] {
            g.hash(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stat_compounds_adds_and_stays_within_its_bounds() {
        let base = Fx::from_int(1000);
        let flat = Grow::default();
        assert_eq!(flat.at(base, 50), base);
        let pct = RawGrow {
            percent: 10.0,
            ..RawGrow::default()
        }
        .compile(1.0);
        assert_eq!(pct.at(base, 1), base);
        assert!((pct.at(base, 2) - Fx::from_int(1100)).abs() < Fx::ratio(1, 100));
        let l3 = pct.at(base, 3);
        assert!(
            (l3 - Fx::from_int(1210)).abs() < Fx::ratio(1, 100),
            "{l3:?}"
        );
        // No ceiling on the level: it saturates, never wraps.
        assert_eq!(pct.at(base, u16::MAX), Grow::LIMIT);
        let add = RawGrow {
            per_level: 20.0,
            most: Some(1100.0),
            ..RawGrow::default()
        }
        .compile(1.0);
        assert_eq!(add.at(base, 4), Fx::from_int(1060));
        assert_eq!(add.at(base, 40), Fx::from_int(1100));
        let shrink = RawGrow {
            percent: -50.0,
            least: Some(2.0),
            ..RawGrow::default()
        }
        .compile(10.0);
        assert!((shrink.at(Fx::from_int(80), 2) - Fx::from_int(40)).abs() < Fx::ratio(1, 100));
        assert_eq!(shrink.at(Fx::from_int(80), 30), Fx::from_int(20));
    }
}
