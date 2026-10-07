//! The Regency's warp (`WarpStyle::Nanite`): the hull comes apart into the swarm its
//! builders are made of, and the swarm goes where the ship is going.
//!
//! - **Charging**: red heat builds in the hull and at its drives, the plating bleeds red
//!   arcs (`warp_charge_arcs`) and flakes: black specks lift off the hull and drift away,
//!   more as it fills. Energy is drawn in as red motes, and ahead of the nose the space the
//!   jump will cross dimples into rings that run in (the gravity well's closed form).
//! - **Going in** (the one tick the hull is still drawn): the hull cut into cells that
//!   leave stern first, flung down the bearing, each at its own speed (`warp_unmake` in
//!   warp_hull.wgsl); a dark cloud of specks goes with it, a few red filaments run ahead,
//!   a red ring spreads round the bearing. Nothing white: the swarm eats the light.
//! - **In warp**: the well opens where it will come out: a dark heart with a hard red ring,
//!   rings of bent space running in, beads of swarm falling along fixed lines, a violet
//!   point. Specks stream in from the way it came, and on the last tick the whole swarm
//!   arrives together.
//! - **Coming out** (all of `Emerge`, the hull condensing over `WarpView::length` ticks):
//!   the swarm runs in from behind and the hull re-forms cell by cell, nose first, violet
//!   at the edge of the join. A dampened exit is torn: the cells shudder, the light gutters
//!   and hot debris is thrown off, as the ARC drive's is.
//!
//! The specks are `PUFF_WARP_SWARM` (warp_puffs.wgsl): black shards with a glint at the rim.
//! Every size is scaled by the hull's radius. Emitters are capped per ship per tick (about
//! 14 specks while charging, 110 on the jump and 80 on the exit tick), and ships far from the
//! camera draw only the glows, rifts and a thin swarm.

use super::{
    Glow, Jump, Palette, MOTE_REACH, PUFF_WARP_ARC, PUFF_WARP_GLOW, PUFF_WARP_MOTE, PUFF_WARP_RIFT,
    PUFF_WARP_STREAK, PUFF_WARP_SWARM,
};
use crate::renderer::{Renderer, PUFF_SPARK};
use glam::Vec3;
use mc_sim::mirror::RenderFrame;
use mc_sim::tables::WarpPhase;

/// How far a speck gets in `t` seconds on a birth speed of 1 (warp_puffs.wgsl: drag 1.8).
fn reach_in(t: f32) -> f32 {
    (1.0 - (-1.8 * t).exp()) / 1.8
}

/// Red and violet, never white. `torn` carries the nanite mark (2) and whether a dampener
/// has the jump (1): a dampened jump is violet with red through it.
pub(super) fn palette(dampened: bool) -> Palette {
    if dampened {
        Palette {
            core: Vec3::new(0.75, 0.3, 1.0),
            edge: Vec3::new(0.5, 0.08, 1.0),
            accent: Vec3::new(1.0, 0.06, 0.12),
            torn: 3.0,
            shock: [0.6, 0.12, 1.0],
        }
    } else {
        Palette {
            core: Vec3::new(1.0, 0.32, 0.5),
            edge: Vec3::new(1.0, 0.07, 0.04),
            accent: Vec3::new(0.6, 0.2, 1.0),
            torn: 2.0,
            shock: [0.9, 0.1, 0.12],
        }
    }
}

impl Renderer {
    /// One jump of the nanite drive this tick.
    pub(super) fn nanite_tick(&mut self, j: &Jump, frame: &RenderFrame, time: f32) {
        let v = &j.view;
        match v.phase {
            WarpPhase::Spool => {
                let hull = frame
                    .units
                    .iter()
                    .find(|u| u.unit_id == v.unit_id)
                    .map_or((j.from, v.bearing), |u| (Vec3::from(u.pos), u.heading));
                self.nanite_charge(j, hull, time);
            }
            WarpPhase::Transit if v.ticks == 0 => self.nanite_leave(j, time),
            WarpPhase::Transit => self.nanite_well(j, time),
            WarpPhase::Emerge => self.nanite_condense(j, time),
            WarpPhase::Idle => {}
        }
    }

    /// One speck of the swarm: `size` its half-width, `glint` the colour at its rim.
    fn speck(
        &mut self,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: f32,
        glint: Vec3,
        torn: f32,
    ) {
        self.push_warp(
            PUFF_WARP_SWARM,
            pos,
            vel,
            start,
            life,
            (size, size * 0.8),
            glint,
            torn,
        );
    }

    /// A random point in the hull's volume, for a speck to leave from or arrive at.
    fn in_hull(&mut self, at: Vec3, j: &Jump, fwd: Vec3, left: Vec3) -> Vec3 {
        at + fwd * self.scatter.signed() * j.r
            + left * self.scatter.signed() * 0.4 * j.r
            + Vec3::Z * self.scatter.signed() * 0.2 * j.r
    }

    /// The drive charging: heat in the hull and at the drives, specks flaking off, energy
    /// drawn in, rings running in ahead of the nose.
    fn nanite_charge(&mut self, j: &Jump, (pos, heading): (Vec3, f32), time: f32) {
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
            pal.edge * (0.25 + 1.5 * c * c),
            pal.torn,
        );
        for side in [-1.0, 1.0] {
            let size = 0.2 * r * (0.4 + c);
            self.push_warp(
                PUFF_WARP_GLOW,
                pos - fwd * 0.82 * r + left * side * 0.28 * r,
                Vec3::ZERO,
                time,
                life,
                (size, size * 1.15),
                pal.accent * (0.4 + 2.6 * c * c),
                pal.torn,
            );
        }
        // The space ahead of the nose dimples: rings of bent air running in.
        if c > 0.2 {
            let size = r * (0.5 + 0.9 * c);
            let ahead = j.fwd * (1.4 + 0.6 * c) * r;
            self.push_warp(
                PUFF_WARP_RIFT,
                pos + ahead,
                Vec3::new(0.0, 1.0, 0.0),
                time,
                life,
                (size, size),
                pal.edge * 0.9 * (c - 0.2),
                pal.torn,
            );
        }
        self.warp_fx.light(Glow {
            pos: pos + Vec3::Z * 0.3 * r,
            color: pal.edge * (0.15 + 1.2 * c * c) * 9000.0 * k * k,
            range: (2.5 + 1.5 * c) * r,
            start: time,
            life,
            pulse: 3.0 + 12.0 * c * c,
        });
        if !j.near {
            return;
        }
        // The plating flakes: specks lift off and drift up and away, more as it fills.
        for _ in 0..(1.0 + 9.0 * c * c) as usize {
            let at = pos
                + fwd * self.scatter.signed() * 0.85 * r
                + left * self.scatter.signed() * 0.35 * r
                + Vec3::Z * 0.1 * r;
            let vel = (self.scatter.upward(0.4) * (5.0 + 14.0 * self.scatter.unit())
                + left * self.scatter.signed() * 4.0)
                * k.max(0.5);
            let size = 0.022 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            let roll_2 = self.scatter.unit();
            self.speck(
                at,
                vel,
                time + roll_1 * dt,
                1.1 + roll_2,
                size,
                pal.edge * 2.5,
                pal.torn,
            );
        }
        // Energy gathered in from all round the hull, red.
        for _ in 0..(1.0 + 5.0 * c) as usize {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.5 * self.scatter.signed(),
            )
            .normalize_or_zero();
            let start = pos + dir * r * (1.4 + 0.8 * self.scatter.unit());
            let target = pos + fwd * self.scatter.signed() * 0.7 * r;
            let size = (0.03 + 0.02 * c) * r;
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
    }

    /// Going in: the hull flung down the bearing as a dark cloud, a few filaments of red
    /// light running ahead of it, a ring spreading round the bearing.
    fn nanite_leave(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let from = j.from;
        let distance = from.distance(j.to);
        let gone = time + dt * 0.9;
        // A small hot flash at the break, red, and the ring.
        let flash = (0.4 * r).max(j.floor);
        self.push_warp(
            PUFF_WARP_GLOW,
            from + j.fwd * 0.4 * r,
            Vec3::ZERO,
            time,
            0.3,
            (flash, flash * 3.0),
            pal.edge * 3.0,
            pal.torn,
        );
        let saved = self.effect_settings;
        self.effect_settings.shockwave_color = Some(pal.shock);
        self.push_shockwave(from.to_array(), time, 3.6 * r, 0.9, 0.8, 0.0, j.fwd);
        self.effect_settings = saved;
        self.warp_fx.light(Glow {
            pos: from,
            color: pal.edge * 26_000.0 * k * k,
            range: 5.0 * r,
            start: time,
            life: 0.5,
            pulse: 0.0,
        });
        // Filaments of red running out along the bearing: the hot edge of the break.
        let bright = (distance - r).clamp(r, 36.0 * r);
        for side in [-0.4_f32, 0.0, 0.4] {
            let long = 0.5 + 0.5 * self.scatter.unit();
            let roll = self.scatter.unit();
            self.push_warp(
                PUFF_WARP_STREAK,
                from + j.fwd * r * 0.6 + j.left * side * r,
                j.fwd * bright * long,
                gone - dt * (0.6 + 0.4 * roll),
                0.7,
                (0.03 * r, 0.012 * r),
                pal.edge * 3.0,
                pal.torn,
            );
        }
        // The swarm: specks thrown from all through the hull, down the bearing, each as far
        // as its own speed takes it, the farthest almost to the mark.
        let count = if j.near { 110 } else { 28 };
        for _ in 0..count {
            let at = self.in_hull(from, j, j.fwd, j.left);
            let reach =
                (distance * (0.15 + 0.85 * self.scatter.unit().powi(2))).clamp(4.0 * r, 36.0 * r);
            let speed = reach / MOTE_REACH;
            let side = (j.left * self.scatter.signed() + Vec3::Z * 0.5 * self.scatter.signed())
                * speed
                * 0.04;
            let size = 0.03 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            let roll_2 = self.scatter.unit();
            self.speck(
                at,
                j.fwd * speed + side,
                time + roll_1 * dt * 0.8,
                0.9 + 0.8 * roll_2,
                size,
                pal.edge * 3.0,
                pal.torn,
            );
        }
        if j.view.dampened {
            // Torn, the swarm throws off sparks of violet and red.
            for _ in 0..16 {
                let vel =
                    self.scatter.upward(0.2) * (20.0 + 30.0 * self.scatter.unit()) * k.max(0.5);
                let at = self.in_hull(from, j, j.fwd, j.left);
                self.push_warp(
                    PUFF_WARP_MOTE,
                    at,
                    vel,
                    time,
                    1.0,
                    (0.04 * r, 0.015 * r),
                    pal.accent * 5.0,
                    pal.torn,
                );
            }
        }
    }

    /// In warp: the well opens where it will come out, specks stream in along the way it
    /// came, and on the last tick the swarm arrives all at once.
    fn nanite_well(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let v = &j.view;
        let p = (v.ticks as f32 / v.length.max(1) as f32).clamp(0.0, 1.0);
        let life = dt * 1.8;
        let to = j.to;
        let size = (r * (0.3 + 1.4 * p.powf(1.3))).max(j.floor * (0.6 + p));
        let open = super::smoothstep(0.1, 0.85, p);
        self.push_warp(
            PUFF_WARP_RIFT,
            to,
            Vec3::new(open, 1.0, 0.0),
            time,
            life,
            (size, size),
            pal.edge * (1.5 + 2.2 * p),
            pal.torn,
        );
        self.warp_fx.light(Glow {
            pos: to,
            color: pal.edge * (0.1 + p * p) * 15000.0 * k * k,
            range: (1.5 + 2.0 * p) * r,
            start: time,
            life,
            pulse: if v.dampened { -1.0 } else { 2.0 + 5.0 * p },
        });
        let last = v.ticks + 1 >= v.length;
        if last {
            // The whole swarm comes in along the bearing, reaching the well as the tick ends.
            let run = j.from.distance(to).clamp(4.0 * r, 36.0 * r);
            let flight = dt * 0.95;
            let count = if j.near { 90 } else { 24 };
            for _ in 0..count {
                let back = run * (0.1 + 0.9 * self.scatter.unit());
                let start = to - j.fwd * back
                    + j.left * self.scatter.signed() * 0.35 * r
                    + Vec3::Z * self.scatter.signed() * 0.2 * r;
                let vel = (to - start) / reach_in(flight);
                let size = 0.03 * r * (0.6 + self.scatter.unit());
                self.speck(start, vel, time, dt * 1.3, size, pal.edge * 3.0, pal.torn);
            }
        }
        if !j.near {
            return;
        }
        // A thin stream falling in from behind, and from all round.
        for n in 0..(2.0 + 5.0 * p) as usize {
            let start = if n % 2 == 0 {
                to - j.fwd * r * (2.5 + 3.0 * self.scatter.unit())
                    + j.left * self.scatter.signed() * 0.5 * r
                    + Vec3::Z * self.scatter.signed() * 0.3 * r
            } else {
                let d = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    0.6 * self.scatter.signed(),
                )
                .normalize_or_zero();
                to + d * r * (1.8 + 1.4 * self.scatter.unit())
            };
            let size = 0.028 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            self.speck(
                start,
                (to - start) / MOTE_REACH,
                time + roll_1 * dt,
                0.9,
                size,
                pal.edge * (2.0 + 2.0 * p),
                pal.torn,
            );
        }
        if v.dampened {
            // The knotted space round it throws red lightning.
            for _ in 0..1 + (self.scatter.unit() * 3.0) as usize {
                let dir = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    0.5 * self.scatter.signed(),
                )
                .normalize_or_zero();
                let roll_1 = self.scatter.unit();
                let roll_2 = self.scatter.unit();
                self.push_warp(
                    PUFF_WARP_ARC,
                    to + dir * size * 0.36,
                    dir * size * (0.4 + 0.6 * roll_1),
                    time + roll_2 * dt,
                    0.2,
                    (0.12 * r, 0.12 * r),
                    pal.accent * 5.0,
                    pal.torn,
                );
            }
        }
    }

    /// Coming out, every tick the hull condenses (the first also the blow of arrival): the
    /// swarm runs in from behind and the hull re-forms, violet along the join.
    fn nanite_condense(&mut self, j: &Jump, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let (r, k, pal) = (j.r, j.k, j.pal);
        let v = &j.view;
        let to = j.to;
        let left_over = 1.0 - (v.ticks as f32 / v.length.max(1) as f32).clamp(0.0, 1.0);
        if v.ticks == 0 {
            // A soft violet flash as the first of it lands, and a ring round the bearing.
            let flash = (0.5 * r).max(j.floor);
            self.push_warp(
                PUFF_WARP_GLOW,
                to,
                Vec3::ZERO,
                time,
                0.35,
                (flash, flash * 2.4),
                pal.accent * 2.5,
                pal.torn,
            );
            let saved = self.effect_settings;
            self.effect_settings.shockwave_color = Some([0.55, 0.2, 1.0]);
            self.push_shockwave(to.to_array(), time, 4.4 * r, 0.9, 0.8, 0.0, j.fwd);
            self.effect_settings = saved;
            self.sky.blast(to, 3.0 * r, 0.8, time);
        }
        // The hull lit as the swarm lands on it, easing off as it is whole.
        self.warp_fx.light(Glow {
            pos: to + Vec3::Z * 0.3 * r,
            color: pal.accent * 4_500.0 * k * k * (0.2 + left_over),
            range: 2.2 * r,
            start: time,
            life: dt * 1.8,
            pulse: if v.dampened { -1.0 } else { 0.0 },
        });
        if !j.near {
            return;
        }
        // The swarm running in from behind to land on the hull, nose first.
        let count = if v.ticks == 0 {
            80
        } else {
            (3.0 + 12.0 * left_over) as usize
        };
        let flight = 0.4;
        for _ in 0..count {
            let target = to
                + j.fwd * self.scatter.signed() * 0.8 * r
                + j.left * self.scatter.signed() * 0.35 * r;
            let back = r * (2.0 + 9.0 * self.scatter.unit());
            let start = target - j.fwd * back
                + j.left * self.scatter.signed() * 0.4 * r
                + Vec3::Z * self.scatter.signed() * 0.25 * r;
            let vel = (target - start) / reach_in(flight);
            let size = 0.03 * r * (0.6 + self.scatter.unit());
            let roll_1 = self.scatter.unit();
            self.speck(
                start,
                vel,
                time + roll_1 * dt,
                flight + 0.05,
                size,
                pal.accent * 3.0,
                pal.torn,
            );
        }
        if v.dampened {
            // Torn out of the swarm: the light gutters and hot metal falls off the hull.
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
    }
}
