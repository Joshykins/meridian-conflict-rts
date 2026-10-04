//! The missile-defence lasers (`SimEvent::MissileLased`, a tick of burn on a missile or
//! a lobbed shell). Each tick's burn is one shot: a red beam from the emitter that
//! strikes at full brightness and is gone a moment later, the emitter's head flashing,
//! the casing white-hot where it bit, a glint in the camera's glass (`lens_flare`), and
//! sparks thrown back along the beam. When the casing fails the round goes up red and
//! quick: a red flash, a short red ball, red embers, a bigger glint.
//! Built to read at strategic zoom: the beam never thins below a few pixels.

use super::lens_flare::Flare;
use super::{blast_fx, FadeBeam, Renderer, FADE_LASER, PUFF_SPARK};
use crate::gpu_consts::puff;
use glam::Vec3;
use mc_core::FxVec3;

/// A laser kill's embers (puffs.wgsl).
const PUFF_INTERCEPT: f32 = puff::INTERCEPT as f32;

/// A shot's thickness in the world, metres, and how long it takes to fade, seconds.
const BEAM_WIDTH: f32 = 0.6;
const BEAM_LIFE: f32 = 0.2;

/// The glint where a shot strikes, and when the round goes up.
const STRIKE_GLINT: Flare = Flare {
    color: Vec3::new(9.0, 3.4, 2.6),
    size: 30.0,
};
const KILL_GLINT: Flare = Flare {
    color: Vec3::new(14.0, 4.4, 2.6),
    size: 46.0,
};

impl Renderer {
    /// A tick of burn from the emitter at `from` on the round at `to`: one shot.
    pub(super) fn missile_lased(&mut self, from: &FxVec3, to: &FxVec3, killed: bool, time: f32) {
        let origin = Vec3::from(from.to_f32());
        let at = Vec3::from(to.to_f32());
        self.fade_beams.push(FadeBeam {
            from: origin,
            to: at,
            start: time,
            life: BEAM_LIFE,
            width: BEAM_WIDTH,
            kind: FADE_LASER,
        });
        // The emitter's head flashes as the bank dumps into it.
        self.push_effect(origin.to_array(), time, 3.6, BEAM_LIFE, 8.0, 0.0);
        if killed {
            self.missile_killed(at, time);
            return;
        }
        // The casing white-hot where the shot bit, a glint, sparks thrown back off it
        // toward the emitter.
        self.lens_flares.flash(at, STRIKE_GLINT, time, 0.22);
        self.push_effect(at.to_array(), time, 2.6, BEAM_LIFE, 9.0, 0.0);
        self.push_effect(at.to_array(), time, 4.2, BEAM_LIFE, 1.0, 0.0);
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
            self.push_puff(PUFF_SPARK, at, spray * speed, time, 0.25, (0.2, 0.05));
        }
    }

    /// A round burnt down by a laser goes up red, the colour of the beam that killed it,
    /// and is soon gone: a hard red flash and glint, a short glowing red ball, red embers
    /// thrown out. Nothing left hanging.
    fn missile_killed(&mut self, at: Vec3, time: f32) {
        self.lens_flares.flash(at, KILL_GLINT, time, 0.3);
        self.push_effect(at.to_array(), time, 5.0, 0.1, 8.0, 0.0);
        self.push_effect(at.to_array(), time, 12.0, 0.18, 8.0, 0.0);
        // The burst: the blast fireball's lumpy burning ball, in red (puffs.wgsl reads
        // appearance.y as the red switch), burnt out fast.
        let r = 5.5;
        let life = 0.4 / blast_fx::BLAST_BURN;
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
                dir * r * out * 3.4,
                time + i as f32 * 0.012,
                lasts,
                (grown * 0.3, grown),
                Vec3::new(1.0, 1.0, 0.0),
            );
        }
        for _ in 0..12 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.8 + 0.2,
            )
            .normalize_or_zero();
            let speed = 20.0 + self.scatter.unit() * 36.0;
            let life = 0.3 + self.scatter.unit() * 0.25;
            self.push_puff(PUFF_INTERCEPT, at, dir * speed, time, life, (0.5, 0.2));
        }
    }
}
