//! What a winged flier's rig (flight.wgsl) reads of how it moves, eased.
//!
//! The sim turns a hovering hull at its full rate or not at all, and speeds it up and
//! brakes it at its full rate or not at all, so its turn and its change of speed a tick
//! jump between nothing and the most it can do. Wings and legs posed straight from them
//! snapped over and back at the ticks where a turn or a surge began or ended (the
//! Seraph's jitter on the move). Here each flier's turn and speed follow the sim's
//! through a critically damped spring, a tick at a time on the sim's clock, so the rig
//! leans into a turn and settles out of it.
//!
//! The eased values go to the GPU in the copy of the units it draws: the turn's share
//! (-1 to 1, left positive) last tick and this in `UnitInstance::drive_swing` (a flier has
//! no stern drives), and the speed in `gait` y and z (metres a tick, this tick and last),
//! whose difference is the surge.

use std::borrow::Cow;

use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};

/// Seconds the turn and the speed take to settle on a new value, about.
const TURN_SETTLE: f32 = 0.4;
const SPEED_SETTLE: f32 = 0.3;
/// The sim's turn a tick, radians, times this is the rig's turn share (flight.wgsl reads
/// it as -1 to 1): a twelfth of a turn a second is a full one.
const TURN_GAIN: f32 = 6.0;
/// A flier that moved further than this in a tick jumped (spawned or landed from a dash):
/// it starts where it is.
const JUMP: f32 = 60.0;

/// Seconds of sim time a tick.
const DT: f32 = 1.0 / mc_core::TICKS_PER_SECOND as f32;

/// One value following its goal through a critically damped spring.
#[derive(Clone, Copy, Default)]
struct Spring {
    /// Last tick's value and this tick's.
    at: [f32; 2],
    rate: f32,
}

impl Spring {
    fn rest(goal: f32) -> Self {
        Self {
            at: [goal, goal],
            rate: 0.0,
        }
    }

    /// One tick toward `goal`, stable for any step (Game Programming Gems 4, 1.10).
    fn step(self, goal: f32, settle: f32) -> Self {
        let omega = 2.0 / settle;
        let x = omega * DT;
        let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
        let change = self.at[1] - goal;
        let temp = (self.rate + omega * change) * DT;
        Self {
            at: [self.at[1], goal + (change + temp) * decay],
            rate: (self.rate - omega * temp) * decay,
        }
    }
}

#[derive(Clone, Copy)]
struct Eased {
    /// Whose it is: the row is shared by every unit that ever stood in it.
    unit: u32,
    turn: Spring,
    speed: Spring,
    /// The upload this flier was last seen in.
    seen: u32,
}

#[derive(Default)]
pub(super) struct FlightEase {
    /// By the unit's row (`Handle::index`): a sky of a hundred thousand fliers is a
    /// lookup each, not a hash; a slot not seen in the last upload is a new flier's.
    fliers: Vec<Option<Eased>>,
    upload: u32,
}

/// The rig's turn share for a turn of `turn` radians in a tick, left positive.
fn turn_share(turn: f32) -> f32 {
    let turn =
        (turn + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    (turn * TURN_GAIN).clamp(-1.0, 1.0)
}

impl FlightEase {
    /// The units as the GPU should have them: every flier (`flies` of its model slot)
    /// carries its eased turn and speed. Called once a tick, as the tick is uploaded.
    pub(super) fn patch<'a>(
        &mut self,
        flies: impl Fn(u32) -> bool,
        units: Cow<'a, [UnitInstance]>,
    ) -> Cow<'a, [UnitInstance]> {
        let wanted = |u: &UnitInstance| {
            u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0 && flies(u.blueprint)
        };
        if !units.iter().any(wanted) {
            self.fliers.clear();
            return units;
        }
        self.upload = self.upload.wrapping_add(1);
        let upload = self.upload;
        let mut units = units.into_owned();
        for u in units.iter_mut().filter(|u| wanted(u)) {
            let turn = turn_share(u.heading - u.prev_heading);
            let speed = u.gait[1];
            let travel = glam::Vec3::from(u.pos).distance(glam::Vec3::from(u.prev_pos));
            let row = mc_sim::Handle(u.unit_id).index();
            if row >= self.fliers.len() {
                self.fliers.resize(row + 1, None);
            }
            let last = self.fliers[row].filter(|e| {
                e.unit == u.unit_id && e.seen == upload.wrapping_sub(1) && travel <= JUMP
            });
            let eased = match last {
                Some(e) => Eased {
                    unit: u.unit_id,
                    turn: e.turn.step(turn, TURN_SETTLE),
                    speed: e.speed.step(speed, SPEED_SETTLE),
                    seen: upload,
                },
                None => Eased {
                    unit: u.unit_id,
                    turn: Spring::rest(turn),
                    speed: Spring::rest(speed),
                    seen: upload,
                },
            };
            self.fliers[row] = Some(eased);
            u.drive_swing = eased.turn.at;
            u.gait[1] = eased.speed.at[1].max(0.0);
            u.gait[2] = eased.speed.at[0].max(0.0);
        }
        Cow::Owned(units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A turn at the full rate that starts and stops at once: the rig's turn never jumps
    /// more than a little a tick, reaches the full turn and comes back to rest.
    #[test]
    fn a_sudden_turn_is_eased_in_and_out() {
        let full = turn_share(0.2);
        assert_eq!(full, 1.0);
        let mut s = Spring::rest(0.0);
        let mut most = 0.0f32;
        for tick in 0..40 {
            let goal = if tick < 20 { full } else { 0.0 };
            let next = s.step(goal, TURN_SETTLE);
            assert!((next.at[1] - next.at[0]).abs() < 0.4, "tick {tick} jumped");
            most = most.max(next.at[1]);
            s = next;
        }
        assert!(most > 0.95, "reached {most}");
        assert!(s.at[1].abs() < 0.02, "settled at {}", s.at[1]);
    }
}
