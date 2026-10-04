//! A braided shot as it is drawn (`Weapon::braid`): the sim flies one lobbed shot and it
//! strikes once, but it is seen as strands wound round its line of flight like the
//! strands of a rope. They part off the muzzle, corkscrew round each other all the way up
//! and over, and close into one over the last stretch, so what lands is the one shot.
//!
//! Each strand is drawn as a shot of its own (`ProjectileInstance`), so each leaves its own
//! trail and the trails hang in the sky as a twisted rope.

use super::ProjectileInstance;
use crate::combat::BALLISTIC_OVERRUN;
use mc_core::TICKS_PER_SECOND;
use std::f32::consts::TAU;

/// Metres from the line of flight the strands wind at, fully parted. A map gun's shot
/// covers 40 to 70 m a tick, so a narrower braid is stretched into three near-straight lines.
const RADIUS: f32 = 16.0;
/// Seconds the strands take to part off the muzzle.
const PART_SECONDS: f32 = 1.2;
/// Seconds before it lands that the strands start closing into one.
const CLOSE_SECONDS: f32 = 2.5;
/// Seconds a strand takes to go once round the line of flight: eight ticks, so each turn
/// is drawn in eight straight pieces (a tick each) and still reads as round.
const TURN_SECONDS: f32 = 0.8;
/// How much faster they wind as they close, at the end: a skater pulling in her arms.
const CLOSE_SPIN: f32 = 2.0;
/// Each strand's share of the whole shot's size: together they are no brighter than it.
const STRAND_SIZE: f32 = 0.72;

/// A smooth 0-to-1 step over `x` from zero to one.
fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Where each strand is this tick, off the shot: the offset of strand `k` of `count`,
/// `t` ticks out of the muzzle, `left` ticks before it lands, flying down `fwd` (unit).
/// `phase` turns the whole braid so shots fired together do not wind in step.
fn offset(k: u8, count: u8, t: f32, left: f32, fwd: [f32; 3], phase: f32) -> [f32; 3] {
    let tps = TICKS_PER_SECOND as f32;
    let parted = ease(t / (PART_SECONDS * tps));
    let open = ease(left / (CLOSE_SECONDS * tps));
    let r = RADIUS * parted * open;
    // Wound once a turn, and faster through the close: the extra turn it makes over the
    // last stretch grows as it closes.
    let turns = t / (TURN_SECONDS * tps) + CLOSE_SPIN * (1.0 - open) * CLOSE_SECONDS / TURN_SECONDS;
    let a = phase + TAU * (turns + k as f32 / count as f32);
    // Across the line of flight: level, and then the one up off it.
    let flat = (fwd[0] * fwd[0] + fwd[1] * fwd[1]).sqrt().max(1e-4);
    let side = [-fwd[1] / flat, fwd[0] / flat, 0.0];
    let up = [
        side[1] * fwd[2] - side[2] * fwd[1],
        side[2] * fwd[0] - side[0] * fwd[2],
        side[0] * fwd[1] - side[1] * fwd[0],
    ];
    let (c, s) = (a.cos() * r, a.sin() * r);
    std::array::from_fn(|i| side[i] * c + up[i] * s)
}

/// The strands `shot` (a braided shot as it would be drawn alone) is seen as instead:
/// `count` of them, `age` ticks out of the muzzle with `ticks_left` on its clock, flying at
/// `vel`; `serial` turns its braid.
pub(super) fn strands(
    shot: &ProjectileInstance,
    count: u8,
    age: u16,
    ticks_left: u16,
    vel: [f32; 3],
    serial: u32,
    out: &mut Vec<ProjectileInstance>,
) {
    let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    if speed <= 0.0 {
        out.push(*shot);
        return;
    }
    let fwd = vel.map(|v| v / speed);
    let left = (ticks_left as i32 - BALLISTIC_OVERRUN).max(0) as f32;
    let t = age as f32;
    let phase = (serial.wrapping_mul(0x9E37_79B9) >> 8) as f32 / (1u32 << 24) as f32 * TAU;
    for k in 0..count {
        let now = offset(k, count, t, left, fwd, phase);
        let before = offset(k, count, t - 1.0, left + 1.0, fwd, phase);
        out.push(ProjectileInstance {
            prev_pos: std::array::from_fn(|i| shot.prev_pos[i] + before[i]),
            pos: std::array::from_fn(|i| shot.pos[i] + now[i]),
            size: shot.size * STRAND_SIZE,
            ..*shot
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// The strands leave the muzzle as one, wind apart, and land as one.
    #[test]
    fn the_strands_part_and_close_into_one() {
        let fwd = [0.6, 0.0, 0.8];
        let tps = TICKS_PER_SECOND as f32;
        for k in 0..3 {
            assert!(len(offset(k, 3, 0.0, 300.0, fwd, 0.0)) < 1e-3);
            assert!(len(offset(k, 3, 0.0 + 5.0 * tps, 0.0, fwd, 0.0)) < 1e-3);
            let mid = offset(k, 3, 5.0 * tps, 5.0 * tps, fwd, 0.0);
            assert!(
                (len(mid) - RADIUS).abs() < 1e-3,
                "strand {k} at {}",
                len(mid)
            );
            // Across the line of flight, never along it.
            let along: f32 = (0..3).map(|i| mid[i] * fwd[i]).sum();
            assert!(along.abs() < 1e-3);
        }
    }

    /// Three strands stand a third of a turn apart, so the braid is even.
    #[test]
    fn the_strands_are_evenly_spaced() {
        let fwd = [0.0, 1.0, 0.0];
        let t = 4.0 * TICKS_PER_SECOND as f32;
        let at: Vec<[f32; 3]> = (0..3).map(|k| offset(k, 3, t, t, fwd, 0.4)).collect();
        for k in 0..3 {
            let (a, b) = (at[k], at[(k + 1) % 3]);
            let gap = len(std::array::from_fn(|i| a[i] - b[i]));
            assert!((gap - RADIUS * 3f32.sqrt()).abs() < 1e-2, "gap {gap}");
        }
    }
}
