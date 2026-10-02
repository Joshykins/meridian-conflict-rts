//! The Regency's strategic weapons on screen (`nuke_look::PLASMA`; docs/NUKES.md "The
//! Regency's warhead"): what is not the volume (nova.wgsl draws the star, the shell and
//! the plasma cloud).
//!
//! - The burst: a white flash past what the screen shows, red after it; the light of it
//!   over the country, white, then red; streamers of plasma flung out round the star's
//!   waist and up from its pole (the power generators' supernova, `NOVA_WISP`, a warhead's
//!   size), and hard globs of red plasma thrown out and falling.
//! - A launch: the containment letting the warhead go, a red-white flash in the tube and
//!   globs of plasma, the tube smoking dark.
//! - In flight: dark smoke (puffs.wgsl reads the trail's negative strength as soot) and,
//!   at the tail, the white-hot streak of a Sunspear round (`RegencyGunFx::streak`).
//! - A kill: the containment breaking high up, a white flash with the prism's streamers
//!   and red globs falling; no nuclear yield.

use super::super::regency_guns_fx::{GLOB, GLOW, MOTE, RED};
use super::super::stun_fx::Flash;
use super::super::{Renderer, PUFF_SMOKE, PUFF_SPARK};
use super::{Blast, WARHEAD_LENGTH};
use crate::gpu_consts::puff;
use glam::{Vec2, Vec3};
use mc_sim::mirror::{StrategicInstance, STRATEGIC_WARHEAD};
use std::f32::consts::TAU;

const WISP: f32 = puff::NOVA_WISP as f32;
const WHITE: Vec3 = Vec3::new(1.0, 0.95, 1.0);
const ROSE: Vec3 = Vec3::new(1.0, 0.45, 0.5);
/// Metres of streak between two of its pieces, and the seconds it glows behind a warhead
/// and behind an interceptor.
const STREAK_STEP: f32 = 30.0;
const WARHEAD_STREAK: f32 = 0.9;
const INTERCEPTOR_STREAK: f32 = 0.5;

impl Renderer {
    /// One stroke of a blast's lightning: the storm's (`color` 0) or another fading beam's.
    pub(super) fn bolt_stroke(
        &mut self,
        from: Vec3,
        to: Vec3,
        start: f32,
        life: f32,
        width: f32,
        color: u32,
    ) {
        if color == 0 {
            self.bore_fx.lightning(from, to, start, life, width);
        } else {
            self.bore_fx.arc(from, to, start, life, width, color);
        }
    }

    /// What goes with a Regency nova's volume the moment it bursts.
    pub(super) fn nova_burst(&mut self, b: &Blast, time: f32) {
        let at = b.at;
        let s = b.scale;
        let origin = self.effect_origin.replace(at);
        // The flash: white past the screen, a broader red one under it.
        self.push_effect(at.to_array(), time, 260.0 * s, 0.5, 9.0, 0.0);
        self.push_effect(at.to_array(), time + 0.05, 260.0 * s, 1.2, 8.0, 0.0);
        // Its light over the country: white, then a long red.
        self.emp_fx.flashes.push(Flash {
            pos: at + Vec3::Z * 120.0 * s,
            start: time,
            life: 1.6 * s.sqrt(),
            color: WHITE * 1.5e4 * s,
            range: 3200.0 * s,
            flicker: 0.1,
        });
        self.emp_fx.flashes.push(Flash {
            pos: at + Vec3::Z * 160.0 * s,
            start: time + 0.3,
            life: 9.0 * s.sqrt(),
            color: RED * 6.0e3 * s,
            range: 2600.0 * s,
            flicker: 0.25,
        });
        // Streamers round its waist, the ring tilted a little its own way, none past the
        // shell (nova.wgsl `nova_shell_radius`, about 640 m).
        let reach = 560.0 * s;
        let tilt = 0.25 * self.scatter.signed();
        let (ts, tc) = tilt.sin_cos();
        let yaw = self.scatter.unit() * TAU;
        for i in 0..72 {
            let a = (i as f32 + 0.5 * self.scatter.unit()) * TAU / 72.0;
            let flat = Vec2::from_angle(a + yaw);
            let off = 0.25 * self.scatter.signed();
            let dir = Vec3::new(flat.x, flat.y * tc, flat.y * ts + off + 0.12).normalize();
            let go = reach * (0.55 + 0.35 * self.scatter.unit());
            let size = s * (16.0 + 14.0 * self.scatter.unit());
            let (r0, r1) = (self.scatter.unit(), self.scatter.unit());
            self.push_lit(
                WISP,
                at + dir * 40.0 * s,
                dir * go * puff::NOVA_WISP_DRAG,
                time + 0.04 * r0,
                3.5 + 1.5 * r1,
                (size, size * 3.4),
                Vec3::splat(1.6),
                0.0,
            );
        }
        // A jet up from its pole, laid over half a second, the first highest.
        for i in 0..28 {
            let k = i as f32 / 28.0;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.07;
            let dir = (Vec3::Z + lean).normalize();
            let go = reach * 1.3 * (1.0 - 0.45 * k) * (0.9 + 0.2 * self.scatter.unit());
            let size = s * (14.0 + 10.0 * self.scatter.unit());
            self.push_lit(
                WISP,
                at + dir * 40.0 * s,
                dir * go * puff::NOVA_WISP_DRAG,
                time + 0.5 * k,
                3.0 + 1.2 * k,
                (size, size * 2.8),
                Vec3::splat(2.4),
                0.0,
            );
        }
        // Hard globs of plasma thrown out every way that arc over and fall, cooling.
        for _ in 0..40 {
            let dir = self.scatter.upward(0.15);
            let speed = s * (90.0 + 160.0 * self.scatter.unit());
            let size = s * (6.0 + 8.0 * self.scatter.unit());
            let r0 = self.scatter.unit();
            self.push_lingering(
                GLOB,
                at + dir * 60.0 * s,
                dir * speed,
                time + 0.1 * r0,
                3.0 + 2.0 * r0,
                (size, size * 0.6),
                RED.lerp(ROSE, r0) * 6.0,
                0.0,
                1,
            );
        }
        // Sparks sprayed out of the star.
        for _ in 0..90 {
            let vel = self.scatter.upward(0.0) * s * (60.0 + 160.0 * self.scatter.unit());
            let life = 2.0 + 2.0 * self.scatter.unit();
            let size = 1.5 + 1.5 * self.scatter.unit();
            self.push_puff(PUFF_SPARK, at, vel, time, life, (size, 0.3));
        }
        self.effect_origin = origin;
    }

    /// A Regency silo letting its warhead go: the containment's flash in the tube, globs
    /// of plasma, dark smoke rolling out over the deck.
    pub(super) fn nova_launch(&mut self, at: Vec3, time: f32) {
        let origin = self.effect_origin.replace(at);
        self.push_effect((at + Vec3::Z * 4.0).to_array(), time, 22.0, 0.6, 9.0, 0.0);
        self.push_effect((at + Vec3::Z * 4.0).to_array(), time, 34.0, 1.6, 8.0, 0.0);
        self.push_shockwave(
            (at + Vec3::Z * 3.0).to_array(),
            time,
            90.0,
            1.4,
            0.7,
            1.0,
            Vec3::ZERO,
        );
        self.emp_fx.flashes.push(Flash {
            pos: at + Vec3::Z * 12.0,
            start: time,
            life: 2.5,
            color: RED * 900.0,
            range: 220.0,
            flicker: 0.4,
        });
        for k in 0..40 {
            let a = (k as f32 + self.scatter.unit()) / 40.0 * TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.15) * (10.0 + self.scatter.unit() * 16.0);
            let start = time + self.scatter.unit() * 1.5;
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_SMOKE,
                at + Vec3::Z * 3.0,
                out,
                start,
                7.0 + r0 * 5.0,
                (8.0, 30.0),
            );
        }
        // Plasma thrown up out of the tube with the round.
        for _ in 0..16 {
            let up = Vec3::new(
                self.scatter.signed() * 3.0,
                self.scatter.signed() * 3.0,
                24.0 + self.scatter.unit() * 22.0,
            );
            let r0 = self.scatter.unit();
            self.push_lit(
                GLOB,
                at + Vec3::Z * 4.0,
                up,
                time + r0 * 0.5,
                1.2 + r0,
                (2.6, 1.6),
                RED.lerp(ROSE, r0) * 5.0,
                0.0,
            );
        }
        self.effect_origin = origin;
    }

    /// A Gravitic Interceptor out of its cell: a red snap and a few motes.
    pub(super) fn nova_cell_launch(&mut self, at: Vec3, time: f32) {
        let origin = self.effect_origin.replace(at);
        self.push_effect(at.to_array(), time, 6.0, 0.3, 9.0, 0.0);
        self.push_effect(at.to_array(), time, 10.0, 0.6, 8.0, 0.0);
        for _ in 0..8 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            ) * 14.0;
            let r0 = self.scatter.unit();
            self.push_lit(
                MOTE,
                at,
                out,
                time,
                0.6 + 0.4 * r0,
                (0.6, 0.3),
                WHITE.lerp(RED, r0) * 5.0,
                0.0,
            );
        }
        for s in 0..8 {
            let a = s as f32 * TAU / 8.0 + self.scatter.unit();
            let out = Vec3::new(a.cos(), a.sin(), 0.1) * (8.0 + 6.0 * self.scatter.unit());
            self.push_puff(PUFF_SMOKE, at, out, time + 0.02, 2.4, (1.2, 6.0));
        }
        self.effect_origin = origin;
    }

    /// A Regency warhead broken open high up (`killed`): its containment fails in a white
    /// flash, the prism's streamers fling out and red plasma falls burning; no yield. Else
    /// a spent interceptor bursting.
    pub(super) fn nova_intercept_burst(&mut self, at: Vec3, killed: bool, time: f32) {
        let origin = self.effect_origin.replace(at);
        if !killed {
            self.push_effect(at.to_array(), time, 12.0, 0.4, 9.0, 0.0);
            self.push_effect(at.to_array(), time, 18.0, 0.9, 8.0, 0.0);
            self.effect_origin = origin;
            return;
        }
        self.push_effect(at.to_array(), time, 150.0, 0.6, 9.0, 0.0);
        self.push_effect(at.to_array(), time + 0.04, 220.0, 2.0, 8.0, 0.0);
        self.push_shockwave(at.to_array(), time, 520.0, 2.5, 1.0, 1.0, Vec3::ZERO);
        self.emp_fx.flashes.push(Flash {
            pos: at,
            start: time,
            life: 2.5,
            color: ROSE * 8.0e3,
            range: 1800.0,
            flicker: 0.2,
        });
        for i in 0..36 {
            let a = (i as f32 + self.scatter.unit()) * TAU / 36.0;
            let dir = Vec3::new(a.cos(), a.sin(), self.scatter.signed() * 0.35).normalize();
            let size = 8.0 + 8.0 * self.scatter.unit();
            let r0 = self.scatter.unit();
            self.push_lit(
                WISP,
                at,
                dir * 260.0 * (0.6 + 0.4 * r0) * puff::NOVA_WISP_DRAG,
                time,
                2.4 + r0,
                (size, size * 3.0),
                Vec3::splat(2.0),
                0.0,
            );
        }
        for _ in 0..30 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let speed = 40.0 + self.scatter.unit() * 90.0;
            let r0 = self.scatter.unit();
            self.push_lingering(
                GLOB,
                at,
                dir * speed,
                time,
                3.0 + 2.0 * r0,
                (4.0, 2.4),
                RED.lerp(ROSE, r0) * 5.0,
                0.0,
                1,
            );
        }
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            1.4,
            (60.0, 120.0),
            ROSE * 6.0,
            0.0,
        );
        self.sky.blast(at, 700.0, 2.0, time);
        self.effect_origin = origin;
    }

    /// The white-hot streak behind a Regency missile's tail, laid over the stretch it flew
    /// this tick.
    pub(super) fn nova_streak(&mut self, m: &StrategicInstance, time: f32) {
        let warhead = m.kind == STRATEGIC_WARHEAD;
        let to = Vec3::from(m.pos);
        let from = Vec3::from(m.prev_pos);
        let span = to - from;
        let len = span.length();
        if len < 0.01 {
            return;
        }
        let axis = span / len;
        let behind = axis
            * if warhead {
                WARHEAD_LENGTH * m.scale
            } else {
                9.0
            };
        let (life, width) = if warhead {
            (WARHEAD_STREAK, 2.6 * m.scale.max(0.4))
        } else {
            (INTERCEPTOR_STREAK, 1.0)
        };
        let tick = self.tick_seconds.max(0.02);
        // A deliberate cosmetic cap on one tick's pieces: far over a tick's flight.
        let n = ((len / STREAK_STEP).ceil() as usize).clamp(1, 24);
        for k in 0..n {
            let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let a = from.lerp(to, f0) - behind;
            let b = from.lerp(to, f1) - behind;
            if b.z < self.ground_height(b.truncate()) + 1.0 {
                continue;
            }
            let when = time - tick * (1.0 - f1);
            let fx = &mut self.plasma_fx.guns;
            // The thread where it passed, and the sheath round it that goes out sooner.
            fx.streak(a, b, when, life, width);
            fx.streak(a, b, when, life * 0.35, width * 2.6);
        }
    }
}
