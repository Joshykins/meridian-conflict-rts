//! A dash (`UnitBlueprint::dash`): a unit that sees enemies on the ground some way off
//! charges where it stands, is gone for a moment, and comes down in the middle of them,
//! and its `dash_only` weapons go off at its feet as it lands: the Seraph's is a wave
//! rolling out all round (a cone weapon, mc-sim `wake.rs`). The Regency Seraph, mc-sim
//! `dash.rs`.
//!
//! It dashes on its own while it is free to pick its fights: idle, on an attack-move or a
//! patrol, or under an attack order (whose mark picks the cluster), and only at fire at
//! will. A move order or hold fire keeps it where the player put it.

use mc_core::{Fx, StateHasher};
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDash {
    /// Metres: the farthest an enemy it sees may be for it to dash at.
    pub range: f64,
    /// Metres: an enemy nearer than this is already at its feet, and it fights where it
    /// stands; nor does it dash anywhere nearer than this.
    pub min: f64,
    /// Metres round the enemy it dashes at that count as its cluster: it lands in their
    /// middle (each one counted once, whatever its size).
    pub gather: f64,
    /// Seconds from one landing until it may wind up again.
    pub cooldown: f64,
    /// Seconds it charges where it stands before it goes.
    pub windup: f64,
    /// Seconds it is gone between leaving and landing: out of the world, not seen or hit.
    pub transit: f64,
    /// Seconds before it lands that the air where it comes down begins to crackle
    /// (`DashArriving`, drawn and heard only), at most the transit. Zero (the default): no
    /// warning.
    #[serde(default)]
    pub arrive: f64,
}

/// A dash, in ticks and fixed point (`RawDash`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dash {
    pub range: Fx,
    pub min: Fx,
    pub gather: Fx,
    pub cooldown_ticks: u16,
    pub windup_ticks: u16,
    pub transit_ticks: u16,
    pub arrive_ticks: u16,
}

impl RawDash {
    pub(crate) fn compile(
        &self,
        key: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Dash, DataError> {
        let ticks = |s: f64| (s * ticks_per_second as f64).round().clamp(0.0, 6000.0) as u16;
        let d = Dash {
            range: fx(self.range),
            min: fx(self.min),
            gather: fx(self.gather),
            cooldown_ticks: ticks(self.cooldown),
            windup_ticks: ticks(self.windup),
            transit_ticks: ticks(self.transit),
            arrive_ticks: ticks(self.arrive),
        };
        if !(self.min >= 0.0 && self.range > self.min)
            || self.gather <= 0.0
            || d.windup_ticks == 0
            || d.transit_ticks == 0
            || d.arrive_ticks > d.transit_ticks
            || d.cooldown_ticks < d.windup_ticks
        {
            return Err(DataError::Invalid(format!(
                "{key}: a dash needs a range past its min, a gather, a windup, a transit (no shorter than its arrive) and a cooldown at least as long as the windup"
            )));
        }
        Ok(d)
    }
}

impl Dash {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for v in [self.range, self.min, self.gather] {
            h.write_i64(v.0);
        }
        h.write_u64(
            self.cooldown_ticks as u64
                | (self.windup_ticks as u64) << 16
                | (self.transit_ticks as u64) << 32
                | (self.arrive_ticks as u64) << 48,
        );
    }
}
