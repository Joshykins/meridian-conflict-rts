//! The ground a gravitic engine breaks, felt (mc-sim `gravitic.rs`): its gathering
//! rumbles under it until the rift leaves, the rift tearing out and striking home jolt
//! the view near them, the well let go thumps, and an aircraft torn down lands hard.

use glam::Vec3;
use mc_core::TICKS_PER_SECOND;
use mc_sim::SimEvent;

/// How far over the ground each is felt, metres.
const GATHER_REACH: f32 = 1400.0;
const TEAR_REACH: f32 = 2600.0;
const RELEASE_REACH: f32 = 1500.0;
const DOWNED_REACH: f32 = 600.0;

/// The jolts (where, strength, seconds it rings, reach) of `event`, if it is the engine's.
pub(super) fn rift_jolt(event: &SimEvent) -> Option<(Vec3, f32, f32, f32)> {
    let at = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
    match event {
        // Rung for as long as it gathers: over 2 s a tremor holds its level, then stops.
        SimEvent::RiftGathering { core, ticks, .. } => Some((
            at(*core),
            0.3,
            *ticks as f32 / TICKS_PER_SECOND as f32,
            GATHER_REACH,
        )),
        SimEvent::RiftLaunched { from, .. } => Some((at(*from), 1.8, 1.4, TEAR_REACH)),
        SimEvent::RiftStruck { pos, .. } => Some((at(*pos), 2.0, 1.6, TEAR_REACH)),
        SimEvent::WellReleased { pos, radius, .. } if radius.0 > 0 => {
            Some((at(*pos), 0.7, 1.0, RELEASE_REACH))
        }
        SimEvent::AircraftDowned { pos, .. } => Some((at(*pos), 0.5, 0.6, DOWNED_REACH)),
        _ => None,
    }
}
