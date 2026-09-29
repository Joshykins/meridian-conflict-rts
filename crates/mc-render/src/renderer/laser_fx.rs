//! The missile-defence lasers (`SimEvent::MissileLased`, a tick of burn on a missile or
//! a lobbed shell): one steady red beam per emitter and round, held on it between ticks.
//! The emitter's head flares red while it holds, the casing glows white-hot where the
//! beam bites and throws sparks back along it, and when the casing fails the round
//! goes up red: a red flash, a red glowing ball, red embers, a little smoke.
//! Built to read at strategic zoom: the beam never thins below a few pixels.

use super::{blast_fx, FadeBeam, Renderer, PUFF_SMOKE, PUFF_SPARK};
use crate::gpu_consts::puff;
use glam::Vec3;
use mc_core::FxVec3;

/// A laser kill's embers (puffs.wgsl).
const PUFF_INTERCEPT: f32 = puff::INTERCEPT as f32;

/// A held beam's thickness in the world, metres.
const BEAM_WIDTH: f32 = 0.6;

/// An anti-missile laser held on a missile (`SimEvent::MissileLased` each tick it burns):
/// one steady beam from its emitter that follows the missile between ticks, not a flash
/// per tick. Cut the moment the casing fails, or when the ticks stop coming.
pub(super) struct HeldLaser {
    from: Vec3,
    /// Where the missile was on the tick before, and on the latest.
    prev_to: Vec3,
    to: Vec3,
    /// When the latest tick's burn came in.
    last: f32,
    /// When the missile went up; the beam cuts just after.
    killed: Option<f32>,
}

impl Renderer {
    /// A tick of burn from the emitter at `from` on the round at `to`.
    pub(super) fn missile_lased(&mut self, from: &FxVec3, to: &FxVec3, killed: bool, time: f32) {
        let origin = Vec3::from(from.to_f32());
        let at = Vec3::from(to.to_f32());
        // One steady beam per emitter and missile: the same emitter, and the missile
        // near where its last step says it would be.
        let tick = self.tick_seconds.max(0.02);
        let held = self.held_lasers.iter_mut().find(|l| {
            l.killed.is_none()
                && l.from.distance(origin) < 0.5
                && (l.to + (l.to - l.prev_to) * ((time - l.last) / tick).clamp(0.0, 2.0))
                    .distance(at)
                    < 60.0
        });
        let (motion, first) = match held {
            Some(l) => {
                let motion = (at - l.to) / tick;
                l.prev_to = l.to;
                l.to = at;
                l.last = time;
                if killed {
                    l.killed = Some(time);
                }
                (motion, false)
            }
            None => {
                self.held_lasers.push(HeldLaser {
                    from: origin,
                    prev_to: at,
                    to: at,
                    last: time,
                    killed: killed.then_some(time),
                });
                (Vec3::ZERO, true)
            }
        };
        // The emitter's head flares red while it holds, harder the tick it lights.
        let flare = if first { 3.6 } else { 2.4 };
        self.push_effect(origin.to_array(), time, flare, tick * 1.3, 8.0, 0.0);
        if killed {
            self.missile_killed(at, motion, time);
            return;
        }
        // Still burning: the casing white-hot where the beam holds, sparks thrown back
        // off it toward the emitter and carried along with the round.
        self.push_effect(at.to_array(), time, 2.6, tick * 1.2, 9.0, 0.0);
        self.push_effect(at.to_array(), time, 4.2, tick * 1.2, 1.0, 0.0);
        let back = (origin - at).normalize_or_zero();
        for _ in 0..3 {
            let spray = (back
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.8)
                .normalize_or_zero();
            let speed = 14.0 + self.scatter.unit() * 18.0;
            self.push_puff(
                PUFF_SPARK,
                at,
                spray * speed + motion * 0.6,
                time,
                0.3,
                (0.2, 0.05),
            );
        }
    }

    /// A round burnt down by a laser goes up red, the colour of the beam that killed it:
    /// a hard red flash, a glowing red ball carried on along its line that cools to a deep
    /// red, red embers thrown out and falling, and a little dark smoke left hanging.
    fn missile_killed(&mut self, at: Vec3, motion: Vec3, time: f32) {
        let carry = motion * 0.35;
        self.push_effect(at.to_array(), time, 5.0, 0.1, 8.0, 0.0);
        self.push_effect(at.to_array(), time, 12.0, 0.18, 8.0, 0.0);
        self.push_shockwave(at.to_array(), time, 22.0, 0.34, 0.7, 1.0, Vec3::ZERO);
        // The burst: the blast fireball's lumpy burning ball, in red (puffs.wgsl reads
        // appearance.y as the red switch), carried on along the round's line.
        let r = 5.5;
        let life = 0.7 / blast_fx::BLAST_BURN;
        for i in 0..4 {
            let dir = self.scatter.upward(-0.4);
            let (out, size) = if i == 0 {
                (0.0, 1.0)
            } else {
                (
                    0.35 + self.scatter.unit() * 0.4,
                    0.6 + self.scatter.unit() * 0.3,
                )
            };
            let grown = r * size / 0.62;
            let lasts = life * (0.85 + self.scatter.unit() * 0.35);
            self.push_puff_with_motion(
                blast_fx::PUFF_BLAST,
                at + dir * r * out * 0.3,
                dir * r * out * 3.4 + carry,
                time + i as f32 * 0.012,
                lasts,
                (grown * 0.3, grown),
                Vec3::new(1.0, 1.0, 0.0),
            );
        }
        for _ in 0..18 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.8 + 0.2,
            )
            .normalize_or_zero();
            let speed = 20.0 + self.scatter.unit() * 36.0;
            let life = 0.6 + self.scatter.unit() * 0.5;
            self.push_puff(
                PUFF_INTERCEPT,
                at,
                dir * speed + carry,
                time,
                life,
                (0.5, 0.2),
            );
        }
        for i in 0..2 {
            let dir =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.3).normalize_or_zero();
            self.push_puff(
                PUFF_SMOKE,
                at + dir,
                dir * 1.5 + carry * 0.3 + Vec3::Z * 0.8,
                time + 0.12 + i as f32 * 0.05,
                1.8,
                (1.4, 4.0),
            );
        }
    }

    /// The held anti-missile lasers as beams for this frame (`HeldLaser`): full strength while
    /// the burns keep coming, the far end led along the missile's last step; a quick cut once
    /// it is gone. Drops the finished ones.
    pub(super) fn held_laser_beams(&mut self, time: f32) -> Vec<FadeBeam> {
        let tick = self.tick_seconds.max(0.02);
        self.held_lasers.retain(|l| match l.killed {
            Some(k) => time < k + 0.12,
            None => time < l.last + tick * 1.6,
        });
        self.held_lasers
            .iter()
            .map(|l| {
                let lead = ((time - l.last) / tick).clamp(0.0, 1.5);
                let to = if l.killed.is_some() {
                    l.to
                } else {
                    l.to + (l.to - l.prev_to) * lead
                };
                // A live beam sits at the start of a long life so it never fades; a cut one fades fast.
                let (start, life) = match l.killed {
                    Some(k) => (k, 0.12),
                    None => (time, 1.0),
                };
                FadeBeam {
                    from: l.from,
                    to,
                    start,
                    life,
                    width: BEAM_WIDTH,
                    laser: true,
                    rail: false,
                }
            })
            .collect()
    }
}
