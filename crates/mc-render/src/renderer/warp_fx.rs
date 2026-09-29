//! A capital ship's warp jump (`mc_sim::warp`, `RenderFrame::warps`): everything round the
//! hull's own stretch into a streak of light, which entity.wgsl draws (warp_hull.wgsl).
//!
//! - **Charging** (`WarpPhase::Spool`, `WarpView::charge`): the drive glow builds in the
//!   hull and at its stern, motes of light are drawn in to it, a ripple of bent air forms
//!   ahead of the nose along the jump, the light on the hull pulses faster and faster, and
//!   near the end arcs crackle over the plating.
//! - **Going in** (the first tick of `Transit`, the ship still drawn where it left while
//!   it stretches out): as the streak snaps away, a blinding flash, a pressure ring round
//!   the bearing, a line of light that runs away along the bearing and fades, sparkle
//!   hanging where the ship was, and the clouds shoved aside.
//! - **In warp** (the rest of `Transit`): the rift forms where it will come out, a point
//!   of light that grows into a lens of bent light round a dark heart, arms winding in and
//!   motes drawn into it. On its last tick a streak runs in along the bearing to meet it.
//! - **Coming out** (the first tick of `Emerge`, the streak collapsing into the hull): a
//!   flash, a pressure ring, clouds and dust pushed out, motes thrown off, the hull lit
//!   while it settles.
//! - **Dampened** (`WarpView::dampened`): all of it in sickly violet with red through it
//!   rather than blue-white, torn (stuttering, jittering, guttering), lightning round the
//!   rift through its three-times-longer transit, and a violent exit: sparks and hot
//!   debris thrown off, arcs over the hull and light guttering over the thirty ticks it
//!   takes to be torn out of the streak.
//!
//! Every size is scaled by the hull's radius. Emitters are capped per ship per tick
//! (about 20 puffs while charging, 60 on a jump or exit tick), and ships far from the
//! camera draw only the glows, streaks, rifts and flashes.

use super::{Puff, Renderer, PUFF_RING, PUFF_SPARK};
use crate::camera::Camera;
use crate::gpu_consts::puff;
use glam::Vec3;
use mc_sim::mirror::{RenderFrame, WarpView};
use mc_sim::tables::WarpPhase;
use std::mem::size_of;

const PUFF_WARP_GLOW: f32 = puff::WARP_GLOW as f32;
const PUFF_WARP_STREAK: f32 = puff::WARP_STREAK as f32;
const PUFF_WARP_RIFT: f32 = puff::WARP_RIFT as f32;
const PUFF_WARP_ARC: f32 = puff::WARP_ARC as f32;
const PUFF_WARP_MOTE: f32 = puff::WARP_MOTE as f32;

/// Radius (m) of the hull the light's brightness is tuned for (the Resolute's).
const REFERENCE_RADIUS: f32 = 150.0;
/// Timed lights held at most (flashes and the glows renewed each tick).
const MAX_GLOWS: usize = 64;
/// Share of a mote's birth speed it covers in its life (`warp_puffs.wgsl`: drag 1.8 over 0.9 s).
const MOTE_REACH: f32 = 0.44;
/// A streak's head reaches its end at this share of its life (`warp_streak`).
const STREAK_RUN: f32 = 0.35;

/// The colours of a jump: clean blue-white, or dampened violet with red through it.
#[derive(Clone, Copy)]
struct Palette {
    /// The hottest light: the streak's core, the flash.
    core: Vec3,
    /// The glow round it.
    edge: Vec3,
    /// Sparks off a torn jump.
    accent: Vec3,
    /// How torn it looks (`appearance.w`): 0 clean, 1 dampened.
    torn: f32,
    /// The pressure ring's tint.
    shock: [f32; 3],
}

fn palette(dampened: bool) -> Palette {
    if dampened {
        Palette {
            core: Vec3::new(0.95, 0.6, 1.0),
            edge: Vec3::new(0.62, 0.12, 1.0),
            accent: Vec3::new(1.0, 0.1, 0.22),
            torn: 1.0,
            shock: [0.7, 0.2, 1.0],
        }
    } else {
        Palette {
            core: Vec3::new(0.82, 0.93, 1.0),
            edge: Vec3::new(0.22, 0.55, 1.0),
            accent: Vec3::new(0.5, 0.8, 1.0),
            torn: 0.0,
            shock: [0.35, 0.7, 1.0],
        }
    }
}

/// A light the jump throws: from `start` for `life` seconds, fading out, and pulsing
/// `pulse` times a second (0 steady; below zero it gutters, for a torn jump).
#[derive(Clone, Copy)]
struct Glow {
    pos: Vec3,
    color: Vec3,
    range: f32,
    start: f32,
    life: f32,
    pulse: f32,
}

#[derive(Default)]
pub(super) struct WarpFx {
    glows: Vec<Glow>,
}

impl WarpFx {
    fn light(&mut self, glow: Glow) {
        if self.glows.len() >= MAX_GLOWS {
            self.glows.remove(0);
        }
        self.glows.push(glow);
    }
}

/// One jump as this tick sees it, with what every emitter needs.
struct Jump {
    view: WarpView,
    pal: Palette,
    /// Hull radius, and that against the reference hull.
    r: f32,
    k: f32,
    /// Along the jump, and across it.
    fwd: Vec3,
    left: Vec3,
    from: Vec3,
    to: Vec3,
    /// Close enough to the camera for the fine emitters.
    near: bool,
}

impl Renderer {
    /// Every jump under way this tick (called from `upload_sim`).
    pub(super) fn warp_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let focus = camera.focus.truncate();
        let reach = camera.distance * 2.0 + 1500.0;
        for view in &frame.warps {
            let r = view.radius.max(8.0);
            let fwd = Vec3::new(view.bearing.cos(), view.bearing.sin(), 0.0);
            let from = Vec3::from(view.from);
            let to = Vec3::from(view.to);
            let at = if view.phase == WarpPhase::Spool {
                from
            } else {
                to
            };
            let jump = Jump {
                view: *view,
                pal: palette(view.dampened),
                r,
                k: r / REFERENCE_RADIUS,
                fwd,
                left: Vec3::new(-fwd.y, fwd.x, 0.0),
                from,
                to,
                near: at.truncate().distance(focus) < reach && camera.distance < 5000.0,
            };
            match view.phase {
                WarpPhase::Spool => {
                    // Where the hull is now, its nose still coming round.
                    let hull = frame
                        .units
                        .iter()
                        .find(|u| u.unit_id == view.unit_id)
                        .map_or((from, view.bearing), |u| (Vec3::from(u.pos), u.heading));
                    self.warp_charge(&jump, hull, time);
                }
                WarpPhase::Transit if view.ticks == 0 => self.warp_leave(&jump, time),
                WarpPhase::Transit => self.warp_rift(&jump, time),
                WarpPhase::Emerge if view.ticks == 0 => self.warp_arrive(&jump, time),
                WarpPhase::Emerge if view.dampened => self.warp_torn(&jump, time),
                _ => {}
            }
        }
    }

    /// The drive charging: glow in the hull and at the stern, motes drawn in, a ripple
    /// ahead of the nose, a light pulsing faster as it fills, arcs over the plating near
    /// the end.
    fn warp_charge(&mut self, j: &Jump, (pos, heading): (Vec3, f32), time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let c = j.view.charge.clamp(0.0, 1.0);
        let fwd = Vec3::new(heading.cos(), heading.sin(), 0.0);
        let left = Vec3::new(-fwd.y, fwd.x, 0.0);
        let life = dt * 1.8;
        let heart = 0.45 * r * (0.3 + c);
        self.push_warp(
            PUFF_WARP_GLOW,
            pos + fwd * 0.1 * r,
            Vec3::ZERO,
            time,
            life,
            (heart, heart * 1.1),
            pal.edge * (0.25 + 1.6 * c * c),
            pal.torn,
        );
        for side in [-1.0, 1.0] {
            let size = 0.28 * r * (0.4 + c);
            self.push_warp(
                PUFF_WARP_GLOW,
                pos - fwd * 0.82 * r + left * side * 0.28 * r,
                Vec3::ZERO,
                time,
                life,
                (size, size * 1.15),
                pal.core * (0.4 + 3.0 * c * c),
                pal.torn,
            );
        }
        // Space ahead of the nose, along the jump, begins to bend.
        if c > 0.2 {
            let size = r * (0.5 + 0.9 * c);
            let ahead = Vec3::new(j.fwd.x, j.fwd.y, 0.0) * (1.4 + 0.6 * c) * r;
            self.push_warp(
                PUFF_WARP_RIFT,
                pos + ahead,
                Vec3::new(0.0, 0.0, 0.0),
                time,
                life,
                (size, size),
                pal.edge * 0.9 * (c - 0.2),
                pal.torn,
            );
        }
        let rate = 3.0 + 14.0 * c * c;
        self.warp_fx.light(Glow {
            pos: pos + Vec3::Z * 0.3 * r,
            color: pal.edge * (0.15 + 1.2 * c * c) * 9000.0 * k * k,
            range: (2.5 + 1.5 * c) * r,
            start: time,
            life,
            pulse: rate,
        });
        if !j.near {
            return;
        }
        // Energy gathered in from all round the hull.
        let motes = (2.0 + 8.0 * c) as usize;
        for _ in 0..motes {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.5 * self.scatter.signed(),
            )
            .normalize_or_zero();
            let start = pos + dir * r * (1.5 + 0.9 * self.scatter.unit());
            let target = pos + fwd * self.scatter.signed() * 0.7 * r;
            let size = (0.035 + 0.02 * c) * r;
            let roll_1 = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_MOTE,
                start,
                (target - start) / MOTE_REACH,
                time + roll_1 * dt,
                0.9,
                (size * 0.6, size),
                pal.edge * (2.0 + 4.0 * c),
                pal.torn,
            );
        }
        // Near the end, the plating crackles.
        if c > 0.6 {
            let arcs = (((c - 0.6) * 9.0) as usize + 1).min(4);
            for _ in 0..arcs {
                let a = pos
                    + fwd * self.scatter.signed() * 0.8 * r
                    + left * self.scatter.signed() * 0.35 * r
                    + Vec3::Z * self.scatter.signed() * 0.15 * r;
                let span = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    0.4 * self.scatter.signed(),
                )
                .normalize_or_zero()
                    * r
                    * (0.3 + 0.4 * self.scatter.unit());
                let roll_1 = self.scatter.unit();
                self.push_warp(
                    PUFF_WARP_ARC,
                    a,
                    span,
                    time + roll_1 * dt,
                    0.16,
                    (0.09 * r, 0.09 * r),
                    pal.core * 4.0,
                    pal.torn,
                );
            }
        }
    }

    /// Going in: the flash as the streak snaps away, a pressure ring round the bearing, a
    /// line of light running off along it, sparkle where the ship was.
    fn warp_leave(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let from = j.from;
        // The streak is at full stretch as the tick ends.
        let gone = time + dt * 0.9;
        let distance = j.from.distance(j.to);
        self.warp_flash(from + j.fwd * 1.5 * r, gone, j, 1.0);
        self.push_warp(
            PUFF_WARP_GLOW,
            from,
            j.fwd * 6.0 * k,
            gone,
            1.4,
            (0.9 * r, 2.2 * r),
            pal.edge * 3.0,
            pal.torn,
        );
        // The line of light it leaves: a bright one running out, a long faint one after.
        let bright = (distance - r).clamp(r, 40.0 * r);
        self.push_warp(
            PUFF_WARP_STREAK,
            from + j.fwd * r,
            j.fwd * bright,
            gone - dt * 0.4,
            0.8,
            (0.16 * r, 0.05 * r),
            pal.core * 6.0,
            pal.torn,
        );
        let faint = distance.clamp(r, 90.0 * r);
        self.push_warp(
            PUFF_WARP_STREAK,
            from,
            j.fwd * faint,
            gone,
            1.7,
            (0.06 * r, 0.02 * r),
            pal.edge * 2.5,
            pal.torn,
        );
        if j.view.dampened {
            // Torn, it comes apart into strands.
            for side in [-1.0, 1.0] {
                self.push_warp(
                    PUFF_WARP_STREAK,
                    from + j.left * side * 0.3 * r,
                    (j.fwd + j.left * side * 0.04) * bright * 0.7,
                    gone - dt * 0.2,
                    0.9,
                    (0.1 * r, 0.04 * r),
                    pal.accent * 4.0,
                    pal.torn,
                );
            }
        }
        if !j.near {
            return;
        }
        // Ionised air left hanging where it was, drifting after it.
        for _ in 0..30 {
            let at = from
                + j.fwd * self.scatter.signed() * r
                + j.left * self.scatter.signed() * 0.4 * r
                + Vec3::Z * self.scatter.signed() * 0.2 * r;
            let vel = (Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 10.0
                + j.fwd * 22.0)
                * k.max(0.4);
            let size = 0.03 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            let roll_2 = self.scatter.unit();
            let roll_3 = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_MOTE,
                at,
                vel,
                gone + roll_1 * 0.2,
                1.5 + 1.5 * roll_2,
                (size, size * 0.3),
                pal.edge * (2.0 + 3.0 * roll_3),
                pal.torn,
            );
        }
    }

    /// In warp: the rift forms where it will come out; on the last tick a streak runs in
    /// along the bearing to meet it.
    fn warp_rift(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let v = &j.view;
        let p = (v.ticks as f32 / v.length.max(1) as f32).clamp(0.0, 1.0);
        let life = dt * 1.8;
        let to = j.to;
        let size = r * (0.3 + 1.3 * p.powf(1.3));
        let open = smoothstep(0.1, 0.85, p);
        self.push_warp(
            PUFF_WARP_RIFT,
            to,
            Vec3::new(open, 0.0, 0.0),
            time,
            life,
            (size, size),
            pal.edge * (1.0 + 2.0 * p),
            pal.torn,
        );
        let heart = r * (0.15 + 0.35 * p);
        self.push_warp(
            PUFF_WARP_GLOW,
            to,
            Vec3::ZERO,
            time,
            life,
            (heart, heart),
            pal.core * (0.4 + 2.5 * p * p),
            pal.torn,
        );
        self.warp_fx.light(Glow {
            pos: to,
            color: pal.edge * (0.1 + p * p) * 16000.0 * k * k,
            range: (1.5 + 2.0 * p) * r,
            start: time,
            life,
            pulse: if v.dampened { -1.0 } else { 2.0 + 6.0 * p },
        });
        if v.ticks + 1 >= v.length {
            // The streak coming in along the bearing reaches the rift as the tick ends.
            let run = j.from.distance(to).clamp(r, 40.0 * r);
            let life = dt * 1.4;
            self.push_warp(
                PUFF_WARP_STREAK,
                to - j.fwd * run,
                j.fwd * run,
                time + dt - life * STREAK_RUN,
                life,
                (0.16 * r, 0.07 * r),
                pal.core * 7.0,
                pal.torn,
            );
        }
        if !j.near {
            return;
        }
        // Light drawn in, wound round the heart.
        for _ in 0..(2.0 + 4.0 * p) as usize {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.6 * self.scatter.signed(),
            )
            .normalize_or_zero();
            let start = to + dir * r * (2.0 + 1.5 * self.scatter.unit());
            let swirl = dir.cross(Vec3::Z) * r * 1.2;
            let size = 0.04 * r;
            let roll_1 = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_MOTE,
                start,
                (to - start + swirl) / MOTE_REACH,
                time + roll_1 * dt,
                0.9,
                (size, size * 0.4),
                pal.edge * (2.0 + 3.0 * p),
                pal.torn,
            );
        }
        if v.dampened {
            // The knotted space round it throws lightning.
            for _ in 0..1 + (self.scatter.unit() * 3.0) as usize {
                let dir = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    0.5 * self.scatter.signed(),
                )
                .normalize_or_zero();
                let color = if self.scatter.unit() < 0.4 {
                    pal.accent
                } else {
                    pal.core
                };
                let roll_1 = self.scatter.unit();
                let roll_2 = self.scatter.unit();
                self.push_warp(
                    PUFF_WARP_ARC,
                    to + dir * size * 0.2,
                    dir * size * (0.5 + 0.7 * roll_1),
                    time + roll_2 * dt,
                    0.2,
                    (0.12 * r, 0.12 * r),
                    color * 5.0,
                    pal.torn,
                );
            }
        }
    }

    /// Coming out: the streak collapses into the hull, a flash, a pressure ring, motes
    /// thrown off; dampened, hot debris and sparks torn off it too.
    fn warp_arrive(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let to = j.to;
        // Clean, the collapse ends with the tick; torn, the blow lands at once.
        let at = if j.view.dampened {
            time
        } else {
            time + dt * 0.9
        };
        self.warp_flash(to, at, j, if j.view.dampened { 1.3 } else { 1.0 });
        // The hull, lit as it settles.
        self.warp_fx.light(Glow {
            pos: to + Vec3::Z * 0.3 * r,
            color: pal.core * 9000.0 * k * k,
            range: 2.2 * r,
            start: at,
            life: 1.8,
            pulse: if j.view.dampened { -1.0 } else { 0.0 },
        });
        if !j.near {
            return;
        }
        for _ in 0..24 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.5 * self.scatter.signed(),
            )
            .normalize_or_zero();
            let size = 0.035 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            let roll_2 = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_MOTE,
                to + dir * 0.6 * r,
                dir * (30.0 + 40.0 * roll_1) * k.max(0.4),
                at,
                1.0 + roll_2,
                (size, size * 0.3),
                pal.edge * 4.0,
                pal.torn,
            );
        }
        if j.view.dampened {
            // Plating and hot metal torn off as it is thrown out.
            for _ in 0..40 {
                let vel =
                    self.scatter.upward(0.1) * (25.0 + 45.0 * self.scatter.unit()) * k.max(0.5);
                let at_hull = to
                    + j.fwd * self.scatter.signed() * 0.8 * r
                    + j.left * self.scatter.signed() * 0.35 * r;
                let roll_1 = self.scatter.unit();
                let roll_2 = self.scatter.unit();
                self.push_puff(
                    PUFF_SPARK,
                    at_hull,
                    vel,
                    at + roll_1 * 0.1,
                    1.2 + roll_2,
                    (0.8 * k.max(0.5), 0.4 * k.max(0.5)),
                );
            }
        }
    }

    /// A dampened exit, tick by tick while it is torn out of the streak: arcs over the
    /// hull, sparks falling off it, light guttering.
    fn warp_torn(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let v = &j.view;
        let left_over = 1.0 - (v.ticks as f32 / v.length.max(1) as f32).clamp(0.0, 1.0);
        let to = j.to;
        let jitter =
            j.fwd * self.scatter.signed() * 0.5 * r + j.left * self.scatter.signed() * 0.3 * r;
        let size = r * (0.5 + 0.6 * left_over);
        self.push_warp(
            PUFF_WARP_GLOW,
            to + jitter,
            Vec3::ZERO,
            time,
            dt * 1.6,
            (size, size * 1.2),
            pal.edge * 2.5 * left_over,
            pal.torn,
        );
        self.warp_fx.light(Glow {
            pos: to,
            color: pal.edge * 7000.0 * k * k * left_over,
            range: 2.0 * r,
            start: time,
            life: dt * 1.6,
            pulse: -1.0,
        });
        if !j.near {
            return;
        }
        for _ in 0..2 {
            let a = to
                + j.fwd * self.scatter.signed() * 0.9 * r
                + j.left * self.scatter.signed() * 0.35 * r;
            let span = (j.fwd * self.scatter.signed()
                + j.left * self.scatter.signed() * 0.5
                + Vec3::Z * 0.3 * self.scatter.signed())
            .normalize_or_zero()
                * r
                * (0.4 + 0.5 * self.scatter.unit());
            let color = if self.scatter.unit() < 0.5 {
                pal.accent
            } else {
                pal.core
            };
            let roll_1 = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_ARC,
                a,
                span,
                time + roll_1 * dt,
                0.18,
                (0.1 * r, 0.1 * r),
                color * 5.0,
                pal.torn,
            );
        }
        for _ in 0..3 {
            let at = to
                + j.fwd * self.scatter.signed() * 0.8 * r
                + j.left * self.scatter.signed() * 0.35 * r;
            let vel = self.scatter.upward(0.0) * 20.0 * k.max(0.5) - Vec3::Z * 10.0;
            let roll_1 = self.scatter.unit();
            let roll_2 = self.scatter.unit();
            self.push_puff(
                PUFF_SPARK,
                at,
                vel,
                time + roll_1 * dt,
                1.0 + roll_2,
                (0.7 * k.max(0.5), 0.3 * k.max(0.5)),
            );
        }
    }

    /// The flash of a jump going or coming, at `at` from `start`: a burst of light, a
    /// pressure ring round the bearing, the clouds and ground dust pushed out.
    fn warp_flash(&mut self, at: Vec3, start: f32, j: &Jump, power: f32) {
        let (r, k, pal) = (j.r, j.k, j.pal);
        self.push_warp(
            PUFF_WARP_GLOW,
            at,
            Vec3::ZERO,
            start,
            0.28,
            (0.5 * r * power, 1.6 * r * power),
            pal.core * 4.0,
            pal.torn,
        );
        let saved = self.effect_settings;
        self.effect_settings.shockwave_color = Some(pal.shock);
        self.push_shockwave(at.to_array(), start, 5.0 * r * power, 0.9, 0.9, 0.0, j.fwd);
        self.push_shockwave(
            at.to_array(),
            start,
            2.6 * r * power,
            0.5,
            0.7,
            0.0,
            Vec3::ZERO,
        );
        self.effect_settings = saved;
        self.sky.blast(at, 4.0 * r * power, 1.2 * power, start);
        self.warp_fx.light(Glow {
            pos: at,
            color: pal.core * 40_000.0 * k * k * power,
            range: 7.0 * r,
            start,
            life: 0.55,
            pulse: 0.0,
        });
    }

    /// Every jump's light this frame (called from `upload_lights`).
    pub(super) fn warp_lights(&mut self, time: f32) {
        self.warp_fx.glows.retain(|g| time < g.start + g.life);
        for g in &self.warp_fx.glows {
            let age = (time - g.start) / g.life.max(0.01);
            if age < 0.0 {
                continue;
            }
            let fade = (1.0 - age).clamp(0.0, 1.0);
            let beat = if g.pulse > 0.0 {
                0.7 + 0.3 * (time * g.pulse * std::f32::consts::TAU).sin()
            } else if g.pulse < 0.0 {
                // Guttering: flaring and dropping out at random, fifteen times a second.
                let n = ((time * 15.0).floor() * 12.9898 + g.pos.x * 0.01).sin() * 43758.547;
                0.25 + 1.1 * (n - n.floor())
            } else {
                1.0
            };
            self.lights.lamp(
                g.pos,
                Vec3::NEG_Z,
                g.color * fade * beat,
                g.range,
                180.0,
                1.0,
            );
        }
    }

    /// One warp puff (`warp_puffs.wgsl`): `rgb` its colour and brightness, `torn` how
    /// torn a dampened jump makes it.
    fn push_warp(
        &mut self,
        kind: f32,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: (f32, f32),
        rgb: Vec3,
        torn: f32,
    ) {
        let p = Puff {
            appearance: [rgb.x, rgb.y, rgb.z, torn],
            origin: pos.to_array(),
            opacity: 1.0,
            pos: pos.to_array(),
            start,
            vel: vel.to_array(),
            life,
            params: [size.0, size.1, kind, self.scatter.unit()],
        };
        self.puffs.write(
            (self.puff_cursor * size_of::<Puff>()) as u64,
            bytemuck::bytes_of(&p),
        );
        self.puff_cursor = (self.puff_cursor + 1) % PUFF_RING;
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
