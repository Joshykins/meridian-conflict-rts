//! A cone weapon's wake as it is drawn (`Weapon::cone`, the Regency Wake's projector;
//! docs/STYLE.md "The Regency suite"): the plasma the projector gathered across its mouth
//! (drawn as a Pinched charge, `regency_guns_fx`) is let go as a wave of red plasma that
//! rolls out over the ground across the fan, at the pace the sim's front rolls
//! (`Cone::speed`), to the end of its reach, and leaves a trail of cooling light behind it.
//!
//! - **The mouth:** a white-hot flash where the charge hung, the charge collapsing into
//!   it, and plasma thrown out down the bore.
//! - **The front** (`wake_shell.rs`, wake_shell.wgsl): a wall of plasma standing on the
//!   arc where the front stands, across the fan, white-hot along its face and crest,
//!   orange over its body, red down its back. Its foot throws up clods, dust and sparks of
//!   every heat as it passes; its light runs with it, and the trees bow as it goes by. Run
//!   out, it rolls on a little way, slowing, slumps and breaks up, throwing off the last of
//!   its plasma; it never stops dead.
//! - **The trail:** the fan laid low over the ground from the muzzle out to the wall, in
//!   threads of light running out to it, hot where the front has just passed and cooling
//!   behind it to red and a deep ember, eaten away from the muzzle out. It leaves clumps
//!   of plasma burning on the ground and embers rising off it.
//!
//! Its ground effects are laid a tick at a time as the front gets there (`roll_wakes`,
//! from `regency_trails`); the front and trail themselves are drawn where the front stands
//! each frame (`upload_wake_shells`). Presentation only; the renderer's own clock.

use super::ground_melt::Burn;
use super::regency_guns_fx::{BURST, GLOW, HOT, MOTE, RED, WAKE, WHITE};
use super::wake_shell::GpuWakeShell;
use super::{Renderer, PUFF_CLOD, PUFF_DUST};
use crate::gpu_consts::wake_shell::{BREAK, LINGER, MAX_SHELLS, WIDTH};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use std::f32::consts::{FRAC_PI_2, PI};

/// Metres between the stretches of the front's foot laid at once.
const RING: f32 = 6.0;
/// The front's light: hotter than the trail, a blue-white (wake_shell.wgsl `ICE`).
const ICE: Vec3 = Vec3::new(0.72, 0.86, 1.0);
/// Metres apart along the arc the ground is scorched as the front passes, and how far
/// each scorch reaches; how hot it gets (under the melt: it scorches and glows, never
/// melts) and seconds its glow takes to die.
const SCORCH_STEP: f32 = 14.0;
const SCORCH_RADIUS: f32 = 7.0;
const SCORCH_HEAT: f32 = 0.16;
const SCORCH_COOL: f32 = 1.5;

/// The wake's plasma between red and white-hot (wake_shell.wgsl `HOT`): more orange
/// than the rest of the suite's pink-hot, so a wake runs red, orange and white.
const FIRE: Vec3 = Vec3::new(1.0, 0.42, 0.16);

/// A wake rolling out.
#[derive(Clone, Copy)]
pub(super) struct RollingWake {
    blueprint: BlueprintId,
    weapon: u8,
    /// The muzzle as drawn, and which way over the ground it rolls.
    at: Vec3,
    ahead: Vec2,
    start: f32,
    /// Stretches of its foot laid so far, and whether its front has broken up.
    laid: usize,
    broken: bool,
}

/// What a wake's weapon says of how it is drawn.
#[derive(Clone, Copy)]
struct Reach {
    range: f32,
    speed: f32,
    /// The fan's half-angle, radians, and the drawn arc's (wider by `WIDTH`, at most a
    /// half turn: a full circle round the gun).
    half: f32,
    arc: f32,
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

        // The mouth: the charge collapses into a white-hot flash as the plasma goes out.
        self.charge_spent(blueprint, weapon, at, 0.16, time);
        let s = 2.2 * flash;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            time,
            0.22,
            (s * 0.6, s * 2.4),
            RED.lerp(HOT, 0.4) * 4.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.12,
            (s * 0.8, s * 1.2),
            WHITE * 3.0,
            0.0,
        );
        self.plasma_jet(at, dir, s, false, time);
        self.plasma_fx
            .guns
            .flare(at, HOT * 260.0 * flash, s * 14.0, time, 0.3);

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
            broken: false,
        });
    }

    fn wake_reach(&self, wake: &RollingWake) -> Option<Reach> {
        let w = &self.blueprints.unit(wake.blueprint).weapons[wake.weapon as usize];
        let cone = w.cone?;
        let half = cone.half.0 as f32 / 65536.0 * std::f32::consts::TAU;
        Some(Reach {
            range: w.range_max.to_f32().max(1.0),
            speed: cone.speed.to_f32().max(1.0),
            half,
            arc: (half * WIDTH).min(PI),
            impact: w.impact.max(0.3),
        })
    }

    /// Every rolling wake's foot laid out as far as its front gets by the next tick, and
    /// its front broken up once it has run out; one whose trail has gone out is let go.
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
            if time > end + LINGER {
                self.plasma_fx.wakes.remove(i);
                continue;
            }
            let front = ((until - wake.start) * reach.speed).min(reach.range);
            while ring_at(wake.laid) <= front {
                self.lay_foot(&wake, wake.laid, reach);
                wake.laid += 1;
            }
            if !wake.broken && until >= end {
                self.break_front(&wake, reach, end);
                wake.broken = true;
            }
            self.plasma_fx.wakes[i] = wake;
            i += 1;
        }
    }

    /// The fronts and trails of the rolling wakes, where their fronts stand at `time`.
    pub(super) fn upload_wake_shells(&mut self, time: f32) {
        let mut shells = Vec::with_capacity(self.plasma_fx.wakes.len());
        for wake in &self.plasma_fx.wakes {
            let Some(reach) = self.wake_reach(wake) else {
                continue;
            };
            let age = time - wake.start;
            let end = reach.range / reach.speed;
            // Run out, it rolls on, slowing to half its pace as it breaks up.
            let over = (age - end).clamp(0.0, BREAK);
            let front = (age * reach.speed).clamp(0.0, reach.range)
                + reach.speed * over * (1.0 - 0.25 * over / BREAK);
            shells.push(GpuWakeShell {
                apex: wake.at.to_array(),
                start: wake.start,
                ahead: wake.ahead.to_array(),
                half: reach.half,
                front,
                speed: reach.speed,
                height: wall_height(reach.half_width(reach.range.min(age * reach.speed))),
                fade: (age / 0.05).clamp(0.0, 1.0),
                spent: age - end,
            });
        }
        self.wake_shells.upload(&shells);
    }

    /// Whether a rolling wake's front has passed over `at` by `time`: a tree felled there
    /// burns (`tree_fires`).
    pub(super) fn wake_seared(&self, at: Vec2, time: f32) -> bool {
        self.plasma_fx.wakes.iter().any(|wake| {
            let Some(reach) = self.wake_reach(wake) else {
                return false;
            };
            let d = at - wake.at.truncate();
            let out = d.length();
            let front = ((time - wake.start + self.tick_seconds) * reach.speed).min(reach.range);
            out <= front + 8.0 && d.angle_to(wake.ahead).abs() <= reach.arc + 4.0 / out.max(1.0)
        })
    }

    /// A point on the ground along the wall's foot when it stands `r` out: `across` of
    /// the way along its arc from its middle (0) to one end (±1), and which way it throws
    /// things off (wake_shell.wgsl `radial`).
    fn front_foot(&self, wake: &RollingWake, reach: Reach, r: f32, across: f32) -> (Vec3, Vec2) {
        let water = self.map_info.water_level.to_f32();
        let out = Vec2::from_angle(across * reach.arc).rotate(wake.ahead);
        let xy = wake.at.truncate() + out * r;
        (xy.extend(self.ground_height(xy).max(water)), out)
    }

    /// Stretch `k` of `wake`'s foot, as the front reaches it: clods, a little low dust and
    /// sparks of every heat kicked up along the front's feet, plasma left burning at the
    /// trail's feet and embers rising off it, its light, the trees bowing.
    fn lay_foot(&mut self, wake: &RollingWake, k: usize, reach: Reach) {
        let r = ring_at(k);
        let when = wake.start + r / reach.speed;
        let water = self.map_info.water_level.to_f32();
        let width = reach.half_width(r);
        let kicks = ((r * reach.arc / 6.0).ceil() as usize).clamp(2, 24);
        // The ground it rolls over scorched all along the arc, glowing a moment. A
        // deliberate cosmetic cap on how many a stretch lays.
        let scorches = ((2.0 * r * reach.arc / SCORCH_STEP).ceil() as usize).clamp(1, 64);
        for n in 0..scorches {
            let across = 2.0 * (n as f32 + self.scatter.unit()) / scorches as f32 - 1.0;
            let (p, _) = self.front_foot(wake, reach, r, across);
            if p.z > water + 0.05 {
                self.ground_melt.burn(Burn {
                    pos: p.truncate(),
                    radius: SCORCH_RADIUS * (0.8 + 0.4 * self.scatter.unit()),
                    peak: SCORCH_HEAT,
                    start: when,
                    rise: 0.0,
                    cool: SCORCH_COOL,
                });
            }
        }
        for _ in 0..kicks {
            let across = self.scatter.signed();
            let (p, out) = self.front_foot(wake, reach, r, across);
            let wet = p.z <= water + 0.05;
            let speed = 6.0 + 9.0 * self.scatter.unit();
            let vel = (out * speed).extend(5.0 + 9.0 * self.scatter.unit()) * reach.impact.sqrt();
            let life = 0.9 + 0.6 * self.scatter.unit();
            if !wet {
                let size = 0.25 + 0.3 * self.scatter.unit();
                self.push_puff(PUFF_CLOD, p + Vec3::Z * 0.4, vel, when, life, (size, 0.3));
            }
            if self.scatter.unit() < 0.6 {
                let spark = (out * speed * 1.5).extend(4.0 + 8.0 * self.scatter.unit());
                let life = 0.3 + 0.4 * self.scatter.unit();
                let heat = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    p + Vec3::Z * 0.3,
                    spark,
                    when,
                    life,
                    (0.3, 0.1),
                    plasma_heat(heat) * 4.0,
                    0.0,
                );
            }
        }
        // A little low dust thrown off its feet, every other stretch.
        if k % 2 == 1 {
            let across = self.scatter.signed().signum();
            let (p, out) = self.front_foot(wake, reach, r, across);
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
        // Behind it, a clump of plasma left burning on the ground, and embers rising
        // slowly off the trail as it cools.
        if k.is_multiple_of(2) && r > 20.0 {
            let across = 0.9 * self.scatter.signed();
            let (p, _) = self.front_foot(wake, reach, r, across);
            let size = 0.6 + 0.05 * width.min(30.0) * (0.5 + self.scatter.unit());
            let heat = 0.1 + 0.3 * self.scatter.unit();
            let life = 2.4 + 1.2 * self.scatter.unit();
            self.push_lit(
                WAKE,
                p + Vec3::Z * size * 0.3,
                Vec3::Z * 0.6,
                when + 0.05,
                life,
                (size, size * 1.4),
                plasma_heat(heat) * 1.2,
                0.0,
            );
        }
        for _ in 0..2 {
            let across = self.scatter.signed();
            let (p, _) = self.front_foot(wake, reach, r, across);
            let up = wall_height(width) * reach.taper(across) * self.scatter.unit();
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 1.5;
            let heat = 0.2 + 0.6 * self.scatter.unit();
            let vel = drift + Vec3::Z * (2.0 + 3.0 * self.scatter.unit());
            let start = when + 0.1 + 0.3 * self.scatter.unit();
            let life = 0.8 + 0.8 * self.scatter.unit();
            self.push_lit(
                MOTE,
                p + Vec3::Z * up,
                vel,
                start,
                life,
                (0.22, 0.08),
                plasma_heat(heat) * 3.0,
                0.0,
            );
        }
        // Its light runs with the front; the trees bow as it goes by.
        let mid = wake.at.truncate() + wake.ahead * r;
        let mid = mid.extend(self.ground_height(mid).max(water));
        if k.is_multiple_of(2) {
            self.plasma_fx.guns.flare(
                mid + Vec3::Z * wall_height(width) * 0.5,
                WHITE.lerp(ICE, 0.5) * 170.0 * reach.impact,
                (width * 1.4 + 10.0).min(90.0),
                when,
                0.25,
            );
        }
        if k % 4 == 1 {
            // Bowed all along the arc, a stretch of it at a time.
            let bends = ((r * reach.arc / 30.0).ceil() as usize).clamp(1, 12);
            let reach_out = (r * reach.arc / bends as f32 + 12.0).min(width + 12.0);
            for n in 0..bends {
                let across = 2.0 * (n as f32 + 0.5) / bends as f32 - 1.0;
                let (p, _) = self.front_foot(wake, reach, r, across);
                self.tree_blasts.record(p, when, reach_out, 0.8, true);
            }
        }
    }

    /// The front run out at `when`: it breaks up as it rolls on, throwing the last of its
    /// plasma off its crest and on ahead.
    fn break_front(&mut self, wake: &RollingWake, reach: Reach, when: f32) {
        let r = reach.range;
        let width = reach.half_width(r);
        let pieces = ((r * reach.arc / 4.0).ceil() as usize).clamp(4, 32);
        for n in 0..pieces {
            let across = 2.0 * (n as f32 + self.scatter.unit()) / pieces as f32 - 1.0;
            let (p, out) = self.front_foot(wake, reach, r, across);
            let up = wall_height(width) * reach.taper(across) * (0.4 + 0.6 * self.scatter.unit());
            let size = 1.2 + 0.06 * width.min(40.0) * (0.6 + 0.6 * self.scatter.unit());
            let pace = reach.speed * (0.3 + 0.3 * self.scatter.unit());
            let vel = (out * pace).extend(1.0 + 3.0 * self.scatter.unit());
            let heat = 0.5 + 0.4 * self.scatter.unit();
            let start = when + 0.06 * self.scatter.unit();
            let life = 0.6 + 0.4 * self.scatter.unit();
            self.push_lit(
                WAKE,
                p + Vec3::Z * up,
                vel,
                start,
                life,
                (size, size * 1.6),
                plasma_heat(heat) * 2.0,
                0.0,
            );
        }
        let mid = wake.at.truncate() + wake.ahead * r;
        let mid = mid.extend(self.ground_height(mid) + wall_height(width) * 0.5);
        self.plasma_fx.guns.flare(
            mid,
            WHITE.lerp(ICE, 0.4) * 140.0 * reach.impact,
            (width * 1.6 + 10.0).min(90.0),
            when,
            BREAK,
        );
    }
}

impl Reach {
    /// Metres the drawn fan reaches out to one side of its middle line `r` out (the
    /// whole of `r` for a fan of a quarter turn or more).
    fn half_width(self, r: f32) -> f32 {
        r * self.arc.min(FRAC_PI_2).sin()
    }

    /// How much of the wall's height stands `across` of the way along its arc to one end:
    /// it falls away to nothing at its ends, and has none round a full circle.
    fn taper(self, across: f32) -> f32 {
        if self.arc >= PI {
            1.0
        } else {
            (1.0 - across.powi(4)).max(0.0).sqrt()
        }
    }
}

/// The wall's height over the middle of the fan when the fan is `half` metres wide each
/// side of its middle line (wake_shell.wgsl `wall_tall`, before it breaks up): it grows
/// with the fan up to a point.
fn wall_height(half: f32) -> f32 {
    2.5 + 0.22 * half.min(40.0)
}

/// How far out stretch `k` of a wake's foot is laid, metres.
fn ring_at(k: usize) -> f32 {
    4.0 + k as f32 * RING
}

/// The Regency's plasma by heat, 0 a deep red to 1 white-hot (wake_shell.wgsl
/// `wake_heat`, without its ember end: a spark or clump cools in its own shader).
fn plasma_heat(h: f32) -> Vec3 {
    if h < 0.5 {
        RED.lerp(FIRE, h * 2.0)
    } else {
        FIRE.lerp(WHITE, (h - 0.5) * 2.0)
    }
}
