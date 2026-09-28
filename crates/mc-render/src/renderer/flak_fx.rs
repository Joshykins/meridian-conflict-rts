//! Flak (`Weapon::flak`, docs/STYLE.md "Flak"): a slow shell on a timed fuse that
//! bursts among aircraft.
//!
//! The shell is seen all the way up (a hot round trailing a thin wake, `mirror.rs`).
//! The burst is the signature: a quick knot of burning gas, the charge burning on for
//! a moment inside a hard-edged ball of black smoke that then hangs in the sky, drifting
//! on the wind; a sphere of hot shrapnel streaks flung out to the edge of the splash, so
//! the reach of the burst is seen, not guessed; and burning scraps falling away with
//! thin smoke behind them.

use glam::Vec3;

use super::{Renderer, PUFF_FIRE, PUFF_SMOKE, PUFF_SPARK};
use crate::gpu_consts::puff;

/// A casing shard flung out of the burst (puffs.wgsl).
pub(super) const PUFF_SHRAPNEL: f32 = puff::SHRAPNEL as f32;
/// The burst's black smoke; its appearance.x is heat (puffs.wgsl `flak_smoke`).
pub(super) const PUFF_FLAK: f32 = puff::FLAK as f32;
/// How hard the air slows shrapnel, 1/s: puffs.wgsl `SHRAPNEL_DRAG`.
const SHRAPNEL_DRAG: f32 = 4.0;
/// How hard the air stops the burst's smoke, 1/s: puffs.wgsl, `PUFF_FLAK` motion.
const SMOKE_DRAG: f32 = 5.5;
/// Light kind of a detonation's white blink (lights.rs, sprites.wgsl effect kinds).
const FLASH_WHITE: f32 = 2.0;
/// How much of its card a puff's ball fills (puffs.wgsl `flak_smoke` body).
const BALL_FILL: f32 = 0.62;

/// One flak shell bursting, from its `SimEvent::Impact`.
pub(super) struct FlakBurst {
    pub at: Vec3,
    /// How fast what it burst on was going, m/s; zero for a burst on its fuse.
    pub motion: Vec3,
    /// The weapon's splash radius, metres: what the burst is drawn to fill.
    pub splash: f32,
    /// `Weapon::impact`: how big its flash and smoke are drawn.
    pub impact: f32,
    /// It burst on a hull, not beside one.
    pub direct: bool,
}

impl Renderer {
    pub(super) fn flak_burst(&mut self, b: &FlakBurst, start: f32) {
        let s = b.splash.max(8.0);
        let k = b.impact.clamp(0.6, 2.0);
        // More pieces for a bigger burst, never a sparse one.
        let detail = (s / 36.0).clamp(0.7, 1.4);
        let at = b.at;

        // The fuse: a quick knot of burning gas (blast_fx) and the light it throws on
        // the hulls and the sky round it. The charge burning on inside the smoke carries
        // the moment after.
        self.fireball(at, s * 0.16 * k, 4, 0.14, 1.0, start);
        self.lights
            .effect(at.to_array(), start, s * 0.9 * k, 0.35, FLASH_WHITE);

        // The black puff: a heart that burns, and lumps of soot bulging out of it.
        let ball = s * 0.4 * k.sqrt();
        let lumps = 5 + (detail * 2.0) as usize;
        for i in 0..lumps {
            let dir = self.random_dir();
            let out = if i == 0 {
                0.0
            } else {
                0.3 + self.scatter.unit() * 0.3
            };
            let card = ball * (0.75 + self.scatter.unit() * 0.25) / BALL_FILL;
            let life = (4.6 + self.scatter.unit() * 1.6) * (0.85 + k * 0.15);
            let heat = if i < 3 { 1.0 } else { 0.55 };
            self.push_puff_with_motion(
                PUFF_FLAK,
                at,
                dir * ball * out * SMOKE_DRAG,
                start + i as f32 * 0.005,
                life,
                (card * 0.45, card),
                Vec3::X * heat,
            );
        }
        // Shrapnel: the casing torn into shards and flung out through the whole
        // splash, spread evenly over the sphere so the burst reads as round.
        let shards = (24.0 * detail) as usize;
        let spin = self.scatter.unit() * std::f32::consts::TAU;
        for i in 0..shards {
            let z = 1.0 - 2.0 * (i as f32 + 0.5) / shards as f32;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let a = spin + i as f32 * 2.399_963 + self.scatter.signed() * 0.3;
            let dir = Vec3::new(a.cos() * r, a.sin() * r, z);
            let reach = s * (0.8 + self.scatter.unit() * 0.3);
            let life = 0.5 + self.scatter.unit() * 0.3;
            let speed = reach * SHRAPNEL_DRAG / (1.0 - (-SHRAPNEL_DRAG * life).exp());
            self.push_puff(
                PUFF_SHRAPNEL,
                at,
                dir * speed,
                start,
                life,
                (0.35 * detail, 0.12),
            );
        }

        // Burning scraps falling away, a few trailing thin smoke.
        let scraps = (10.0 * detail) as usize;
        for i in 0..scraps {
            let vel = self.random_dir() * (14.0 + self.scatter.unit() * 26.0) + Vec3::Z * 6.0;
            let life = 0.9 + self.scatter.unit() * 0.8;
            self.push_puff(PUFF_SPARK, at, vel, start, life, (0.4 * detail, 0.1));
            if i < 3 {
                self.scrap_smoke(at, vel, start, life);
            }
        }

        if b.direct {
            // It burst on the hull: metal sparks off the plating, carried with it.
            for _ in 0..8 {
                let vel = self.random_dir() * (18.0 + self.scatter.unit() * 16.0) + b.motion;
                let life = 0.25 + self.scatter.unit() * 0.2;
                self.push_puff(PUFF_SPARK, at, vel, start, life, (0.25, 0.06));
            }
        }
    }

    /// Thin smoke behind a burning scrap along the arc it falls (puffs.wgsl: a spark
    /// flies `p + v t - 14 t²`).
    fn scrap_smoke(&mut self, from: Vec3, vel: Vec3, start: f32, life: f32) {
        for step in 1..=6 {
            let t = life * step as f32 / 6.0;
            let at = from + vel * t - Vec3::Z * 14.0 * t * t;
            let lasts = 1.4 + self.scatter.unit() * 0.6;
            self.push_puff(PUFF_SMOKE, at, Vec3::Z * 0.4, start + t, lasts, (0.5, 2.2));
        }
    }

    /// A flak gun's report: a tongue of flame, a ring of powder smoke punched out round
    /// the muzzle and a jet of it ahead, and sparks down the line of fire. `power` is
    /// the square root of its damage.
    pub(super) fn flak_muzzle(&mut self, at: Vec3, dir: Vec3, power: f32, time: f32) {
        let right = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = right.cross(dir).normalize_or(Vec3::Z);
        self.push_puff(
            PUFF_FIRE,
            at,
            dir * 14.0,
            time,
            0.12,
            (0.5 + power * 0.04, 1.2 + power * 0.08),
        );
        let ring = 8;
        let phase = self.scatter.unit() * std::f32::consts::TAU;
        for i in 0..ring {
            let a = phase + i as f32 / ring as f32 * std::f32::consts::TAU;
            let out = right * a.cos() + up * a.sin();
            let life = 0.9 + self.scatter.unit() * 0.4;
            self.push_puff(
                PUFF_SMOKE,
                at + dir * 0.8,
                out * (4.0 + power * 0.15) + dir * (6.0 + power * 0.3),
                time + 0.01,
                life,
                (0.4 + power * 0.03, 1.4 + power * 0.1),
            );
        }
        for i in 0..3 {
            let life = 1.0 + 0.3 * i as f32 + self.scatter.unit() * 0.3;
            self.push_puff(
                PUFF_SMOKE,
                at + dir * 0.4,
                dir * (10.0 + 6.0 * i as f32 + power * 0.5),
                time + 0.015 * i as f32,
                life,
                (0.5 + power * 0.05, 1.8 + power * 0.16),
            );
        }
        for _ in 0..5 {
            let spray = (dir * 1.5 + self.random_dir() * 0.45).normalize_or_zero();
            let speed = 18.0 + self.scatter.unit() * 18.0;
            let life = 0.12 + self.scatter.unit() * 0.12;
            self.push_puff(PUFF_SPARK, at, spray * speed, time, life, (0.18, 0.05));
        }
    }

    fn random_dir(&mut self) -> Vec3 {
        Vec3::new(
            self.scatter.signed(),
            self.scatter.signed(),
            self.scatter.signed(),
        )
        .normalize_or(Vec3::Z)
    }
}

#[cfg(test)]
mod tests;
