//! A Lance (`UnitBlueprint::lance`): a structure's great beam that takes the
//! nearest enemy in range, charges on it for `charge` seconds, then strikes
//! where it stands: `damage` over everything of the enemy's within `splash`
//! (shields first), then cools for `reload` seconds. Survival's Shapers carry
//! one, its figures grown with their level (`growth.rs`). The sim's side is
//! `mc_sim::lance`.

use mc_core::{Fx, StateHasher};
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLance {
    /// Seconds it charges on its target before it strikes.
    pub charge: f64,
    /// Seconds it cools after a strike.
    pub reload: f64,
    pub damage: f64,
    /// Metres round the mark the strike reaches.
    pub splash: f64,
    pub range: f64,
    /// Categories it strikes.
    pub targets: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lance {
    pub charge_ticks: u32,
    pub reload_ticks: u32,
    pub damage: Fx,
    pub splash: Fx,
    pub range: Fx,
    pub target_mask: u32,
}

impl RawLance {
    pub(crate) fn compile(
        &self,
        key: &str,
        target_mask: u32,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Lance, DataError> {
        if self.charge <= 0.0
            || self.reload < 0.0
            || self.damage <= 0.0
            || self.splash < 0.0
            || self.range <= 0.0
            || target_mask == 0
        {
            return Err(DataError::Invalid(format!(
                "{key}: a lance needs a charge, damage, range and targets"
            )));
        }
        let ticks = |s: f64| (s * ticks_per_second as f64).round().max(1.0) as u32;
        Ok(Lance {
            charge_ticks: ticks(self.charge),
            reload_ticks: ticks(self.reload),
            damage: fx(self.damage),
            splash: fx(self.splash),
            range: fx(self.range),
            target_mask,
        })
    }
}

impl Lance {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.charge_ticks as u64 | (self.reload_ticks as u64) << 32);
        for v in [self.damage, self.splash, self.range] {
            h.write_i64(v.0);
        }
        h.write_u64(self.target_mask as u64);
    }
}
