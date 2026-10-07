//! The space elevator (`docs/ELEVATOR.md`): a T5 structure whose cable reaches orbit.
//!
//! Once it is finished its cable is lowered from orbit and clamped. From then on a set of
//! cargo crates falls down the cable every `period` seconds carrying `payload` mass: the
//! units queued on the elevator (like a factory's queue, but free), and whatever of the
//! payload the queue leaves as plain mass for its side's store. The crates only ever come
//! down: they land, open and let their units drive out, then sink into the ground.

use mc_core::{Fx, StateHasher};
use serde::Deserialize;

use crate::DataError;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawElevator {
    /// Seconds from one platform's arrival to the next.
    pub period: f64,
    /// Mass each platform brings down.
    pub payload: f64,
    /// Seconds the cable takes to come down once the elevator is finished, clamps and all.
    pub tether: f64,
    /// Seconds the crates take to come down the cable, to stand landed (doors opening, their
    /// units driving out), and to sink into the ground.
    pub descent: f64,
    pub dock: f64,
    pub sink: f64,
}

/// A space elevator, in ticks and fixed point. A cycle runs: descent, dock (units let off
/// as the crates land), sink, then the crates are away for the rest of the period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elevator {
    pub period_ticks: u32,
    pub payload: Fx,
    pub tether_ticks: u32,
    pub descent_ticks: u32,
    pub dock_ticks: u32,
    pub sink_ticks: u32,
}

impl RawElevator {
    pub(crate) fn compile(
        &self,
        key: &str,
        fx: impl Fn(f64) -> Fx,
        ticks_per_second: u32,
    ) -> Result<Elevator, DataError> {
        let ticks = |s: f64| (s * ticks_per_second as f64).round().max(0.0) as u32;
        let e = Elevator {
            period_ticks: ticks(self.period),
            payload: fx(self.payload),
            tether_ticks: ticks(self.tether),
            descent_ticks: ticks(self.descent),
            dock_ticks: ticks(self.dock),
            sink_ticks: ticks(self.sink),
        };
        let trip = e.descent_ticks + e.dock_ticks + e.sink_ticks;
        if self.payload <= 0.0
            || e.tether_ticks == 0
            || e.descent_ticks == 0
            || e.dock_ticks == 0
            || e.sink_ticks == 0
            || trip > e.period_ticks
            || e.period_ticks > 36_000
        {
            return Err(DataError::Invalid(format!(
                "{key}: an elevator needs a payload, a tether time, and a descent, dock and sink that fit in its period"
            )));
        }
        Ok(e)
    }
}

impl Elevator {
    pub(crate) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.period_ticks as u64 | (self.tether_ticks as u64) << 32);
        h.write_u64(
            self.descent_ticks as u64
                | (self.dock_ticks as u64) << 21
                | (self.sink_ticks as u64) << 42,
        );
        h.write_i64(self.payload.0);
    }

    /// Ticks into a cycle the crates touch down: units come off and leftover mass is paid.
    pub fn touchdown(&self) -> u32 {
        self.descent_ticks
    }
}
