//! A cone weapon's wake as it is drawn (`Weapon::cone`, the Regency Wake's projector;
//! docs/STYLE.md "The Regency suite"): the plasma the projector gathered across its mouth
//! (drawn as a Pinched charge, `regency_guns_fx`) is let go as a shell of red light that
//! opens from the muzzle across the fan and rolls out over the ground, at the pace the
//! sim's front rolls (`Cone::speed`), to the end of its reach.
//!
//! - **The mouth:** a hard red flash where the charge hung, the charge collapsing into it.
//! - **The shell** (`wake_shell.rs`, wake_shell.wgsl): from the muzzle a cone across the
//!   fan, swelling and closing in a tall rounded dome at the front, half of it standing
//!   over the ground. Faint seen square on, bright at its edges, threaded with thin streaks,
//!   white-hot where it meets the ground; faint back at the muzzle, brightest at the front.
//! - **Where it meets the ground** the dome's foot kicks up clods, a little low dust and
//!   sparks as it passes; its light runs with it, and the trees bow as it goes by.
//! - **Where it runs out** it fades where it stands.
//!
//! Its ground effects are laid a tick at a time as the front gets there (`roll_wakes`,
//! from `regency_trails`); the shell itself is drawn where the front stands each frame
//! (`upload_wake_shells`). Presentation only; the renderer's own clock.

use super::regency_guns_fx::{BURST, GLOW, HOT, MOTE, RED};
use super::wake_shell::GpuWakeShell;
use super::{Renderer, PUFF_CLOD, PUFF_DUST};
use crate::gpu_consts::wake_shell::{CAP, MAX_SHELLS, SWELL};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;

/// Metres between the stretches of the front's foot laid at once.
const RING: f32 = 6.0;
/// Seconds the shell takes to fade where it stands once it has run out.
const FADE: f32 = 0.45;
/// The shell's height over its half-width.
const RISE: f32 = 0.85;

/// A wake rolling out.
#[derive(Clone, Copy)]
pub(super) struct RollingWake {
    blueprint: BlueprintId,
    weapon: u8,
    /// The muzzle as drawn, and which way over the ground it rolls.
    at: Vec3,
    ahead: Vec2,
    start: f32,
    /// Stretches of its foot laid so far.
    laid: usize,
}

/// What a wake's weapon says of how it is drawn.
#[derive(Clone, Copy)]
struct Reach {
    range: f32,
    speed: f32,
    /// The tangent of the fan's half-angle.
    spread: f32,
    impact: f32,
}

impl Renderer {
    /// A cone weapon fired (`ShotFired` of a `cone` weapon): `at` its muzzle as drawn, `dir`
    /// down the bore. The mouth flashes, and the wake starts rolling.
    pub(super) fn wake_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        if w.cone.is_none() {
            return;
        }
        let flash = w.flash.max(0.3);

        // The mouth: the charge collapses into a hard red flash as the plasma goes out.
        self.charge_spent(blueprint, weapon, at, 0.16, time);
        let s = 2.2 * flash;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            time,
            0.22,
            (s * 0.6, s * 2.4),
            RED * 4.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.12,
            (s * 0.8, s * 1.2),
            HOT * 3.0,
            0.0,
        );
        self.plasma_fx
            .guns
            .flare(at, RED * 260.0 * flash, s * 14.0, time, 0.3);

        // A deliberate cosmetic cap, as many as the shells drawn: the oldest goes first.
        let wakes = &mut self.plasma_fx.wakes;
        if wakes.len() >= MAX_SHELLS as usize {
            wakes.remove(0);
        }
        wakes.push(RollingWake {
            blueprint,
            weapon,
            at,
            ahead: dir.truncate().normalize_or(Vec2::X),
            start: time,
            laid: 0,
        });
    }

    fn wake_reach(&self, wake: &RollingWake) -> Option<Reach> {
        let w = &self.blueprints.unit(wake.blueprint).weapons[wake.weapon as usize];
        let cone = w.cone?;
        let half = cone.half.0 as f32 / 65536.0 * std::f32::consts::TAU;
        Some(Reach {
            range: w.range_max.to_f32().max(1.0),
            speed: cone.speed.to_f32().max(1.0),
            spread: half.tan(),
            impact: w.impact.max(0.3),
        })
    }

    /// Every rolling wake's foot laid out as far as its front gets by the next tick; one
    /// that has run out and faded is let go.
    pub(super) fn roll_wakes(&mut self, time: f32) {
        let until = time + self.tick_seconds.max(0.02);
        let mut i = 0;
        while i < self.plasma_fx.wakes.len() {
            let mut wake = self.plasma_fx.wakes[i];
            let Some(reach) = self.wake_reach(&wake) else {
                self.plasma_fx.wakes.remove(i);
                continue;
            };
            let end = wake.start + reach.range / reach.speed;
            if time > end + FADE {
                self.plasma_fx.wakes.remove(i);
                continue;
            }
            let front = ((until - wake.start) * reach.speed).min(reach.range);
            while ring_at(wake.laid) <= front {
                self.lay_foot(&wake, wake.laid, reach);
                wake.laid += 1;
            }
            self.plasma_fx.wakes[i] = wake;
            i += 1;
        }
    }

    /// The shells of the rolling wakes, where their fronts stand at `time`.
    pub(super) fn upload_wake_shells(&mut self, time: f32) {
        let water = self.map_info.water_level.to_f32();
        let mut shells = Vec::with_capacity(self.plasma_fx.wakes.len());
        for wake in &self.plasma_fx.wakes {
            let Some(reach) = self.wake_reach(wake) else {
                continue;
            };
            let age = time - wake.start;
            let end = reach.range / reach.speed;
            let front = (age * reach.speed).clamp(0.0, reach.range);
            let fade = (age / 0.05).clamp(0.0, 1.0) * (1.0 - ((age - end) / FADE).clamp(0.0, 1.0));
            let ground = |xy: Vec2| self.ground_height(xy).max(water);
            let tip = wake.at.truncate() + wake.ahead * front;
            shells.push(GpuWakeShell {
                apex: wake.at.to_array(),
                start: wake.start,
                ahead: wake.ahead.to_array(),
                spread: reach.spread,
                front,
                base: [ground(wake.at.truncate()), ground(tip)],
                rise: RISE,
                fade,
            });
        }
        self.wake_shells.upload(&shells);
    }

    /// Stretch `k` of `wake`'s foot, as the front reaches it: clods, a little low dust and
    /// sparks kicked up along the dome's foot, its light, the trees bowing.
    fn lay_foot(&mut self, wake: &RollingWake, k: usize, reach: Reach) {
        let r = ring_at(k);
        let when = wake.start + r / reach.speed;
        let water = self.map_info.water_level.to_f32();
        let side = Vec2::new(-wake.ahead.y, wake.ahead.x);
        // The dome's foot then: round its front from one side to the other.
        let foot = |me: &mut Renderer| -> (Vec3, Vec2) {
            let u = 0.55 + 0.45 * me.scatter.unit();
            let across = r * reach.spread * shell_width(u) * me.scatter.signed().signum();
            let xy = wake.at.truncate() + wake.ahead * (r * u) + side * across;
            let out = (wake.ahead * (1.0 - u + 0.15) + side * across.signum() * 0.6)
                .normalize_or(wake.ahead);
            (xy.extend(me.ground_height(xy).max(water)), out)
        };
        let width = r * reach.spread * (1.0 + SWELL);
        let kicks = ((width / 6.0).ceil() as usize).clamp(2, 10);
        for _ in 0..kicks {
            let (p, out) = foot(self);
            let wet = p.z <= water + 0.05;
            let speed = 6.0 + 9.0 * self.scatter.unit();
            let vel = (out * speed).extend(5.0 + 9.0 * self.scatter.unit()) * reach.impact.sqrt();
            let life = 0.9 + 0.6 * self.scatter.unit();
            if !wet {
                let size = 0.25 + 0.3 * self.scatter.unit();
                self.push_puff(PUFF_CLOD, p + Vec3::Z * 0.4, vel, when, life, (size, 0.3));
            }
            if self.scatter.unit() < 0.5 {
                let spark = (out * speed * 1.5).extend(4.0 + 8.0 * self.scatter.unit());
                let life = 0.3 + 0.3 * self.scatter.unit();
                self.push_lit(
                    MOTE,
                    p + Vec3::Z * 0.3,
                    spark,
                    when,
                    life,
                    (0.3, 0.1),
                    RED.lerp(HOT, 0.5) * 4.0,
                    0.0,
                );
            }
        }
        // A little low dust thrown off its foot, every other stretch.
        if k % 2 == 1 {
            let (p, out) = foot(self);
            if p.z > water + 0.05 {
                let vel = (out * 4.0).extend(1.5);
                let size = 2.0 + 1.5 * self.scatter.unit();
                self.push_puff(
                    PUFF_DUST,
                    p + Vec3::Z * 0.8,
                    vel,
                    when,
                    1.4,
                    (size, size * 2.2),
                );
            }
        }
        // Its light runs with the front; the trees bow as it goes by.
        let mid = wake.at.truncate() + wake.ahead * r;
        let mid = mid.extend(self.ground_height(mid).max(water));
        if k.is_multiple_of(2) {
            self.plasma_fx.guns.flare(
                mid + Vec3::Z * width * RISE * 0.5,
                RED * 140.0 * reach.impact,
                width * 1.4 + 10.0,
                when,
                0.25,
            );
        }
        if k % 4 == 1 {
            self.tree_blasts.record(mid, when, width + 12.0, 0.8, true);
        }
    }
}

/// How far out stretch `k` of a wake's foot is laid, metres.
fn ring_at(k: usize) -> f32 {
    4.0 + k as f32 * RING
}

/// The shell's half-width `u` of the way out, in fan half-widths at the front
/// (wake_shell.wgsl `shell_width`).
fn shell_width(u: f32) -> f32 {
    let close = if u > CAP {
        (1.0 - ((u - CAP) / (1.0 - CAP)).powi(2)).max(0.0).sqrt()
    } else {
        1.0
    };
    let swell = ((u - 0.45) / 0.4).clamp(0.0, 1.0);
    u * (1.0 + SWELL * swell * swell * (3.0 - 2.0 * swell)) * close
}
