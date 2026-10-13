//! Replication shells (`mc_sim::survival` artillery, `RenderFrame::shells`): a
//! Seedcaster's shell that prints a squad where it lands. Bigger and brighter the higher
//! the Seedcaster's level when it fired.
//!
//! - **Fired**: a ring of arcs kicked out round the Seedcaster's crown and a few lines
//!   of light thrown up.
//! - **In flight**: a solid body of pale alloy seamed with light, nose cut on a slant,
//!   on a ribbon of hard light along the arc it has flown; drawn every frame where the
//!   sim's arc puts it (`ShellInstance::at`). Its mark turns on the ground where it will
//!   land, its radius the burst's, beating faster and brighter as it comes down, and in
//!   its last seconds a line of light stands up out of the mark to meet it.
//! - **Landed**: a flash, a burst ring of arcs out to its radius and a fainter one past
//!   it, a column of light the squad is printed in with bands climbing it, lines of
//!   light rising off the ground inside it, and a mark burnt into the ground.

use super::{Piece, HOT, LIGHT};
use crate::gpu_consts::progenitor_fx::{ARCS, DRAPE, LINE, MARK, PRINT, TARGET};
use crate::renderer::Renderer;
use glam::Vec3;
use std::f32::consts::TAU;

/// Seconds of its flight its ribbon reaches back, and the steps it is drawn in.
const RIBBON: f32 = 1.2;
const RIBBON_STEPS: usize = 10;

/// A shell's body length at `level`.
fn shell_length(level: u16) -> f32 {
    (9.0 + 0.5 * f32::from(level)).min(28.0)
}

impl Renderer {
    /// Every frame: each shell in the air, its ribbon and its mark.
    pub(super) fn shells_in_flight(&mut self, out: &mut Vec<Piece>, time: f32, alpha: f32) {
        let tick = self.tick_seconds.max(0.02);
        for s in &self.progenitor_fx.shells {
            let flight = s.flight.max(0.1);
            let now = s.age + alpha * tick;
            let t = (now / flight).clamp(0.0, 1.0);
            let at = Vec3::from(s.at(t));
            let ahead = Vec3::from(s.at((t + 0.002).min(1.0)));
            let behind = Vec3::from(s.at((t - 0.002).max(0.0)));
            let axis = (ahead - behind).normalize_or(Vec3::NEG_Z);
            let side = axis.cross(Vec3::Z).normalize_or(Vec3::X);
            let up = side.cross(axis);
            let len = shell_length(s.level);
            let level = f32::from(s.level);
            out.push(
                Piece::body(at + axis * len * 0.5, axis, up)
                    .size(len * 0.2, len)
                    .power(1.0 + level * 0.05)
                    .now(time),
            );
            // The ribbon along the arc it has flown.
            let width = len * 0.12;
            for k in 0..RIBBON_STEPS {
                let s0 = (now - RIBBON * (k + 1) as f32 / RIBBON_STEPS as f32) / flight;
                let s1 = (now - RIBBON * k as f32 / RIBBON_STEPS as f32) / flight;
                if s1 <= 0.0 {
                    break;
                }
                let fade = 1.0 - k as f32 / RIBBON_STEPS as f32;
                let fade0 = 1.0 - (k + 1) as f32 / RIBBON_STEPS as f32;
                let mut to = Vec3::from(s.at(s1.max(0.0)));
                if k == 0 {
                    to = at - axis * len * 0.5;
                }
                out.push(
                    Piece::beam(Vec3::from(s.at(s0.max(0.0))), to)
                        .size(width * fade0, width * fade)
                        .power(0.9 * fade)
                        .now(time),
                );
            }
            // Its mark, beating faster and brighter as it comes down.
            let left = (s.eta - alpha * tick).max(0.0);
            let near = 1.0 - (left / flight).clamp(0.0, 1.0);
            let beat = 0.75 + 0.25 * (time * (3.0 + 12.0 * near * near)).sin();
            let mark = Vec3::from(s.to);
            out.push(
                Piece::ring(mark, Vec3::Z)
                    .size(s.radius, s.radius)
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

    /// Shell `serial` left the Seedcaster's crown `from` for `to`.
    pub(super) fn shell_fired(&mut self, serial: u32, from: Vec3, to: Vec3, time: f32) {
        let level = self.shell_level(serial);
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

    /// Shell `serial` landed at `at`, its burst `radius` metres, printing `units`.
    pub(super) fn shell_landed(
        &mut self,
        serial: u32,
        at: Vec3,
        radius: f32,
        units: u16,
        time: f32,
    ) {
        let level = self.shell_level(serial);
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
        // The column the squad is printed in.
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

    /// The level shell `serial`'s Seedcaster was at when it fired (zero unknown).
    fn shell_level(&self, serial: u32) -> f32 {
        self.progenitor_fx
            .shell_levels
            .iter()
            .find(|l| l.0 == serial)
            .map_or(0.0, |l| f32::from(l.1))
    }
}
