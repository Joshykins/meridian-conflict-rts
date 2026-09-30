//! How far a spacecraft's stern drive nozzles are swung (`CapitalRig::drives`,
//! `gpu_consts::drive`): toward the side the nose turns to, by how fast it turns.
//!
//! The sim turns a hovering hull at its full rate or not at all, so its turn a tick jumps
//! between nothing and the most it can turn, and nozzles swung straight from it snapped
//! over and back. Here each ship's swing follows that turn through a critically damped
//! spring, a tick at a time and on the sim's clock, with its speed capped: the nozzles
//! lean into a turn and settle out of it. The eased swing goes to the GPU in
//! `UnitInstance::drive_swing` (last tick and this, so the shader glides between them),
//! and the plumes (`capital_fx.rs`) are laid along the same swing.

use std::borrow::Cow;
use std::collections::HashMap;

use mc_data::Blueprints;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};

use crate::gpu_consts::drive;

/// Seconds the swing takes to settle on a new turn, about.
const SETTLE: f32 = 0.45;
/// The fastest the nozzles swing, radians a second: across their whole travel in about
/// a second and a half.
const MOST_RATE: f32 = 0.4;
/// A ship that moved further than this in a tick jumped (spawned, warped, restaged): its
/// nozzles start where its turn puts them.
const JUMP: f32 = 60.0;

/// Seconds of sim time a tick.
const DT: f32 = 1.0 / mc_core::TICKS_PER_SECOND as f32;

#[derive(Clone, Copy)]
struct Swing {
    /// Last tick's swing and this tick's, radians.
    angle: [f32; 2],
    /// How fast it is swinging, radians a second.
    rate: f32,
    /// The upload this ship was last seen in.
    seen: u32,
}

#[derive(Default)]
pub(super) struct DriveSwing {
    /// Per model slot (`UnitInstance::blueprint`): whether its hull has stern drives.
    drives: Vec<bool>,
    ships: HashMap<u32, Swing>,
    upload: u32,
}

/// How far the nozzles would swing for a turn of `turn` radians this tick, left positive.
fn target(turn: f32) -> f32 {
    let turn =
        (turn + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    (turn * drive::VECTOR_GAIN).clamp(-drive::VECTOR_MAX, drive::VECTOR_MAX)
}

/// One tick of a critically damped spring from `angle` (swinging at `rate`) toward
/// `goal`, stable for any step (Game Programming Gems 4, 1.10), and never faster than
/// `MOST_RATE`.
fn ease(angle: f32, rate: f32, goal: f32) -> (f32, f32) {
    let omega = 2.0 / SETTLE;
    let x = omega * DT;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let most = MOST_RATE * SETTLE;
    let change = (angle - goal).clamp(-most, most);
    let aim = angle - change;
    let temp = (rate + omega * change) * DT;
    let mut rate = (rate - omega * temp) * decay;
    let mut next = aim + (change + temp) * decay;
    // It never swings past the goal it is closing on.
    if (goal - angle > 0.0) == (next > goal) {
        next = goal;
        rate = 0.0;
    }
    let step = (next - angle).clamp(-MOST_RATE * DT, MOST_RATE * DT);
    (angle + step, rate.clamp(-MOST_RATE, MOST_RATE))
}

impl DriveSwing {
    /// Unit `id`'s swing last tick and this, as `patch` last eased it; `None` for a ship
    /// it has not seen.
    pub(super) fn of(&self, id: u32) -> Option<[f32; 2]> {
        self.ships.get(&id).map(|s| s.angle)
    }

    /// The units as the GPU should have them: every spacecraft with stern drives carries
    /// its eased swing in `drive_swing`. Called once a tick, as the tick is uploaded.
    pub(super) fn patch<'a>(
        &mut self,
        blueprints: &Blueprints,
        units: Cow<'a, [UnitInstance]>,
    ) -> Cow<'a, [UnitInstance]> {
        if self.drives.len() < blueprints.units.len() {
            self.drives = blueprints
                .units
                .iter()
                .map(|bp| {
                    crate::models::capital_rig(&bp.visual.mesh).is_some_and(|rig| rig[4][0] != 0.0)
                })
                .collect();
        }
        let drives = &self.drives;
        let wanted = |u: &UnitInstance| {
            u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) == 0
                && drives.get(u.blueprint as usize).copied().unwrap_or(false)
        };
        if !units.iter().any(wanted) {
            self.ships.clear();
            return units;
        }
        self.upload = self.upload.wrapping_add(1);
        let upload = self.upload;
        let mut units = units.into_owned();
        for u in units.iter_mut().filter(|u| wanted(u)) {
            let goal = target(u.heading - u.prev_heading);
            let travel = glam::Vec3::from(u.pos).distance(glam::Vec3::from(u.prev_pos));
            let jumped = travel > JUMP || u.in_warp();
            let swing = match self.ships.get(&u.unit_id).filter(|_| !jumped) {
                Some(s) => {
                    let (angle, rate) = ease(s.angle[1], s.rate, goal);
                    Swing {
                        angle: [s.angle[1], angle],
                        rate,
                        seen: upload,
                    }
                }
                None => Swing {
                    angle: [goal, goal],
                    rate: 0.0,
                    seen: upload,
                },
            };
            self.ships.insert(u.unit_id, swing);
            u.drive_swing = swing.angle;
        }
        self.ships.retain(|_, s| s.seen == upload);
        Cow::Owned(units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hull that turns at its full rate for a while and then stops: the nozzles never
    /// jump, swing no faster than `MOST_RATE`, reach the full swing and come back to rest.
    #[test]
    fn nozzles_ease_into_and_out_of_a_turn() {
        let full = target(0.2);
        assert_eq!(full, drive::VECTOR_MAX);
        let (mut angle, mut rate) = (0.0, 0.0);
        let mut most = 0.0f32;
        for tick in 0..60 {
            let goal = if tick < 30 { full } else { 0.0 };
            let (next, r) = ease(angle, rate, goal);
            assert!((next - angle).abs() <= MOST_RATE * DT + 1e-6, "tick {tick}");
            most = most.max(next);
            (angle, rate) = (next, r);
        }
        assert!(most > 0.95 * full, "reached {most}");
        assert!(angle.abs() < 0.01, "settled at {angle}");
    }
}
