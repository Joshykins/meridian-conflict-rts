//! How a hover-flight aircraft (a VTOL gunship, a fan carrier, a drone) flies, not slides:
//! it can only drift sideways or backwards slowly, so to go anywhere fast it turns its nose
//! that way; and its hull leans the way its lift is pushing it. The nose drops to speed up
//! and to hold a cruise against the air, comes up to brake, and the hull banks into a turn
//! or a sideways drift. The lean is serialized flight state: roll in `Units::bank`, pitch in
//! slot 1 of `Units::arm_pitch` (a hover gunship has no build arm), so the renderer poses the
//! hull and tilts its engines from it.
use crate::tables::Units;
use mc_core::{Angle, Fx, FxVec2, TICKS_PER_SECOND};
use mc_data::Motion;

const DT: i32 = TICKS_PER_SECOND as i32;

/// The share of top speed a hover aircraft makes sideways or backwards.
const DRIFT_SHARE: Fx = Fx::ratio(3, 10);
/// Pitch at cruise, and the most that speeding up or braking at full thrust adds (angle
/// units, 65536 a turn): about 9 and 13 degrees.
const CRUISE_PITCH: i32 = 1640;
const THRUST_PITCH: i32 = 2370;
const MAX_PITCH: i32 = 3900;
/// Roll for a full-speed sideways drift, and for full sideways thrust (a hard turn at
/// speed): about 8 and 24 degrees.
const DRIFT_ROLL: i32 = 1460;
const THRUST_ROLL: i32 = 4370;
const MAX_ROLL: i32 = 5100;

/// The slot of `Units::arm_pitch` that carries a hover aircraft's hull pitch.
pub(crate) const PITCH_SLOT: usize = 1;

/// Whether a blueprint flies this way: hover flight, and not a lift ship (`transport.rs`
/// flies those) or a capital ship.
pub(crate) fn flies(motion: &Motion, transport: bool, capital: bool) -> bool {
    motion.layer == mc_data::MoveLayer::Air && motion.hover && !transport && !capital
}

/// `desired` (metres a tick) held to what the airframe can make facing `heading`: full
/// speed ahead, `DRIFT_SHARE` of it to the side or astern.
pub(crate) fn hold_to_airframe(desired: FxVec2, heading: Angle, motion: &Motion) -> FxVec2 {
    let nose = FxVec2::from_angle(heading);
    let side = nose.perp();
    let drift = motion.speed * DRIFT_SHARE / DT;
    let ahead = desired.dot(nose).max(-drift);
    let across = desired.dot(side).clamp(-drift, drift);
    nose * ahead + side * across
}

/// Eases the hull's lean toward where the lift must point for this tick's move: `before`
/// and `after` are the last tick's and this tick's travel (metres a tick).
pub(crate) fn lean(
    units: &mut Units,
    row: usize,
    motion: &Motion,
    heading: Angle,
    before: FxVec2,
    after: FxVec2,
) {
    let nose = FxVec2::from_angle(heading);
    let side = nose.perp();
    let top = (motion.speed / DT).max(Fx::ratio(1, 10));
    let thrust = (motion.accel / DT / DT).max(Fx::ratio(1, 100));
    let push = after - before;
    let share = |v: Fx, of: Fx| (v / of).clamp(-Fx::ONE, Fx::ONE);
    // Nose down (negative) to go and to hold speed, up to brake.
    let pitch = -(Fx::from_int(CRUISE_PITCH) * share(after.dot(nose), top)
        + Fx::from_int(THRUST_PITCH) * share(push.dot(nose), thrust))
    .round_int();
    // Left (+y) lowers the left side: negative roll, as a fixed wing's left turn.
    let roll = -(Fx::from_int(DRIFT_ROLL) * share(after.dot(side), top)
        + Fx::from_int(THRUST_ROLL) * share(push.dot(side), thrust))
    .round_int();
    let ease = |now: i32, want: i32, limit: i32| {
        let want = want.clamp(-limit, limit);
        let delta = want - now;
        now + if delta.abs() <= 6 { delta } else { delta / 5 }
    };
    let now = Angle::ZERO.delta_to(units.arm_pitch[row][PITCH_SLOT]) as i32;
    units.arm_pitch[row][PITCH_SLOT] = Angle(ease(now, pitch, MAX_PITCH) as i16 as u16);
    units.bank[row] = ease(units.bank[row] as i32, roll, MAX_ROLL) as i16;
}
