//! The Progenitor's warp-ins (`mc_sim::survival`, `RenderFrame::warp_ins`): a
//! Seedcaster's squad or a Gatewright's fleet marked to come in beside the defenders.
//! Bigger and brighter the higher its Shaper's level.
//!
//! - **Marked**: a ring of arcs kicked out round its Shaper's crown and a few lines of
//!   light thrown up toward the mark.
//! - **Coming**: its mark turns on the ground where it will come in, its radius the
//!   shock's (the fleet's reach for a fleet), beating faster and brighter as it comes,
//!   and in its last seconds a line of light stands up out of the mark.
//! - **Arrived** (a squad): a flash, a ring of arcs out to the shock's radius and a
//!   fainter one past it, a column of light the squad comes out in with bands climbing
//!   it, lines of light rising off the ground inside it, and a mark burnt into the
//!   ground. A fleet's jump is a warp's own (`warp_fx.rs`).
//!
//! The warp exit itself is still to be drawn.

use super::{Piece, HOT, LIGHT};
use crate::gpu_consts::progenitor_fx::{ARCS, DRAPE, LINE, MARK, PRINT, TARGET};
use crate::renderer::Renderer;
use glam::Vec3;
use mc_sim::survival::WarpInKind;
use std::f32::consts::TAU;

impl Renderer {
    /// Every frame: each warp-in's mark.
    pub(super) fn warp_in_marks(&mut self, out: &mut Vec<Piece>, time: f32, alpha: f32) {
        let tick = self.tick_seconds.max(0.02);
        for s in &self.progenitor_fx.warp_ins {
            let whole = (s.age + s.eta).max(0.1);
            let left = (s.eta - alpha * tick).max(0.0);
            let near = 1.0 - (left / whole).clamp(0.0, 1.0);
            let beat = 0.75 + 0.25 * (time * (3.0 + 12.0 * near * near)).sin();
            let mark = Vec3::from(s.at);
            let r = match s.kind {
                WarpInKind::Squad => s.radius,
                WarpInKind::Fleet => s.reach,
            };
            out.push(
                Piece::ring(mark, Vec3::Z)
                    .size(r, r)
                    .power((0.6 + 1.8 * near * near) * beat)
                    .flags(DRAPE | TARGET)
                    .now(time),
            );
            if left < 3.0 {
                // A line of light standing up out of the mark to meet it.
                let k = 1.0 - left / 3.0;
                out.push(
                    Piece::beam(mark, mark + Vec3::Z * (120.0 + 380.0 * k))
                        .size(1.2 + 2.0 * k, 0.3)
                        .power(0.6 + 1.6 * k)
                        .flags(LINE)
                        .now(time),
                );
            }
        }
    }

    /// A warp-in was marked by the Shaper whose crown is `from`, at `to`, its Shaper at
    /// `level`.
    pub(super) fn warp_in_marked(&mut self, level: u16, from: Vec3, to: Vec3, time: f32) {
        let level = f32::from(level);
        let seed = self.scatter.unit();
        let big = 1.0 + level * 0.04;
        self.lay_progenitor(
            Piece::ring(from, Vec3::Z)
                .size(8.0 * big, 55.0 * big)
                .width(4.0 * big)
                .time(time, 0.7, 0.5)
                .power(2.2)
                .flags(ARCS)
                .tier(3.0 + level / 4.0)
                .seed(seed),
        );
        let toward = (to - from).truncate().normalize_or(glam::Vec2::X);
        for k in 0..6 {
            let a = k as f32 / 6.0 * TAU + seed * TAU;
            let out = (toward * 0.6 + glam::Vec2::new(a.cos(), a.sin()) * 0.4).extend(1.0);
            self.lay_progenitor(
                Piece::beam(from, from + out.normalize() * 60.0 * big)
                    .size(1.6 * big, 0.4)
                    .time(time, 0.5, 0.3)
                    .power(1.8)
                    .flags(LINE),
            );
        }
        self.progenitor_fx
            .flash(from, HOT * 1.5e5 * big, 220.0 * big, time, 0.5);
    }

    /// Squad `serial` warped in at `at`, its shock `radius` metres, `units` coming out.
    pub(super) fn warp_in_arrived(
        &mut self,
        serial: u32,
        at: Vec3,
        radius: f32,
        units: u16,
        time: f32,
    ) {
        let level = self.warp_in_level(serial);
        let big = 1.0 + level * 0.04;
        let r = radius.max(10.0);
        let ground = Vec3::new(at.x, at.y, self.progenitor_ground(at));
        let seed = self.scatter.unit();
        let tier = 3.0 + (level / 4.0).min(3.0);
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r * 0.2, r)
                .width(r * 0.2)
                .time(time, 0.8, 0.5)
                .power(2.6 * big)
                .flags(DRAPE | ARCS)
                .tier(tier)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r * 1.8)
                .width(2.5)
                .time(time, 1.0, 0.9)
                .power(1.2 * big)
                .flags(DRAPE),
        );
        // The column the squad comes out in.
        self.lay_progenitor(
            Piece::prism(ground, Vec3::Z * (30.0 + r * 0.8 + level * 2.0))
                .size(r * 0.45, r * 0.3)
                .time(time, 6.0, 0.12)
                .power(1.4 * big)
                .flags(PRINT)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r)
                .time(time, 25.0, 0.0)
                .power(1.6 * big)
                .flags(DRAPE | MARK)
                .tier(tier)
                .seed(seed),
        );
        // Lines of light rising off the ground inside it as the squad unfolds.
        for _ in 0..12 + usize::from(units).min(24) {
            let a = self.scatter.unit() * TAU;
            let d = r * 0.45 * self.scatter.unit().sqrt();
            let xy = ground.truncate() + glam::Vec2::new(a.cos(), a.sin()) * d;
            let foot = xy.extend(self.progenitor_ground(xy.extend(0.0)));
            let rise = 40.0 + 50.0 * self.scatter.unit();
            let start = time + self.scatter.unit() * 2.0;
            self.lay_progenitor(
                Piece::beam(foot, foot + Vec3::Z * rise)
                    .size(1.0, 0.3)
                    .time(start, 1.5, 0.4)
                    .power(1.4)
                    .flags(LINE),
            );
        }
        self.progenitor_fx.flash(
            ground + Vec3::Z * (8.0 + r * 0.2),
            LIGHT * 6.0e4 * big,
            r * 5.0 + 80.0,
            time,
            0.9,
        );
        self.progenitor_fx.flash(
            ground + Vec3::Z * (20.0 + r * 0.4),
            LIGHT * 2.0e4 * big,
            r * 2.0 + 40.0,
            time,
            6.0,
        );
    }

    /// The level warp-in `serial`'s Shaper was at when it marked it (zero unknown).
    fn warp_in_level(&self, serial: u32) -> f32 {
        self.progenitor_fx
            .warp_in_levels
            .iter()
            .find(|l| l.0 == serial)
            .map_or(0.0, |l| f32::from(l.1))
    }
}
