//! A dash (`UnitBlueprint::dash`): a unit that sees enemies on the ground some way off
//! winds up where it stands and is gone, coming down in the middle of them in one tick,
//! and the landing goes off as a shockwave round it (the Regency Seraph, mc-sim `dash.rs`).
//!
//! It dashes on its own while it is free to pick its fights: idle, on an attack-move or a
//! patrol, or under an attack order (whose mark picks the cluster), and only at fire at
//! will. A move order, hold fire or hold position keeps it where the player put it.

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
    /// Seconds it winds up where it stands before it goes.
    pub windup: f64,
    /// Metres the landing's shockwave reaches.
    pub blast: f64,
    /// The shockwave's damage at its heart, falling in a straight line to `edge` of it at
    /// `blast`. Only enemies take it.
    pub damage: f64,
    /// Share of `damage` left at the edge of the blast (default a quarter).
    #[serde(default = "RawDash::default_edge")]
    pub edge: f64,
    /// Seconds the shockwave stuns what it hits (an EMP: no orders, fire or way on), all
    /// but experimentals. Zero (the default): no stun.
    #[serde(default)]
    pub stun: f64,
}

impl RawDash {
    fn default_edge() -> f64 {
        0.25
    }
}

/// A dash, in ticks and fixed point (`RawDash`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dash {
    pub range: Fx,
    pub min: Fx,
    pub gather: Fx,
    pub cooldown_ticks: u16,
    pub windup_ticks: u16,
    pub blast: Fx,
    pub damage: Fx,
    pub edge: Fx,
    pub stun_ticks: u16,
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
            blast: fx(self.blast),
            damage: fx(self.damage),
            edge: fx(self.edge),
            stun_ticks: ticks(self.stun),
        };
        if !(self.min >= 0.0 && self.range > self.min)
            || self.gather <= 0.0
            || self.blast <= 0.0
            || self.damage < 0.0
            || !(0.0..=1.0).contains(&self.edge)
            || d.windup_ticks == 0
            || d.cooldown_ticks < d.windup_ticks
        {
            return Err(DataError::Invalid(format!(
                "{key}: a dash needs a range past its min, a gather, a blast, damage of at least 0, an edge 0 to 1, a windup and a cooldown at least as long"
            )));
        }
        Ok(d)
    }
}

impl Dash {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        for v in [
            self.range,
            self.min,
            self.gather,
            self.blast,
            self.damage,
            self.edge,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(
            self.cooldown_ticks as u64
                | (self.windup_ticks as u64) << 16
                | (self.stun_ticks as u64) << 32,
        );
    }

    /// The shockwave's damage `reach` metres from its heart (to the near side of a hull).
    pub fn falloff(&self, reach: Fx) -> Fx {
        let f = (reach / self.blast.max(Fx::ONE)).clamp(Fx::ZERO, Fx::ONE);
        self.damage * (Fx::ONE - (Fx::ONE - self.edge) * f)
    }
}
