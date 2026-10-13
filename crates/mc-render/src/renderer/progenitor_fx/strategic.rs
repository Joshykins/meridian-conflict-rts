//! The Progenitor's strategic missiles (`StrategicLook::Progenitor`, docs/NUKES.md): every
//! Shaper's anti-nuke and the Unmaker's warheads. Nothing here burns: no motor fire, no
//! smoke trail, no fireball.
//!
//! - **An interceptor** is a shard of hard light. It lifts out of a column of light with
//!   a ring of arcs round its cell, leaves a line of light behind it that breaks into
//!   dashes, and where it meets a warhead it closes a cage of light round it (three rings
//!   of arcs square to each other) and unmakes it: the cage falls in, and the warhead
//!   comes apart into lines of light flung every way, two rings of arcs opening round
//!   them. A spent one unmakes itself, smaller.
//! - **A warhead** is a solid body of pale alloy seamed with light, nose cut on a slant,
//!   pushed by a spear of light from its tail, and trailing a line of light that hangs
//!   long after it. It climbs out of a pillar of light. It goes off as an unmaking: a
//!   sphere of hard light opens to the sim's blast radius and breaks into lines, then a
//!   second one falls in on the middle; a ring of arcs runs out far over the ground, a
//!   pillar of light stands up out of it, lines of light lift off the ground inside it,
//!   two rings turn above it, and it leaves a burnt mark cooling for most of a minute.
//!   One broken open by an interceptor flashes and comes apart into lines.

use super::{Piece, HOT, LIGHT};
use crate::gpu_consts::nuke_look;
use crate::gpu_consts::progenitor_fx::{ARCS, DRAPE, LIGHT as SHARD, LINE, MARK, PULSES, TRAIL};
use crate::renderer::Renderer;
use glam::Vec3;
use mc_sim::mirror::{RenderFrame, StrategicInstance, STRATEGIC_WARHEAD};
use std::f32::consts::TAU;

/// A warhead's body at scale 1, nose to tail, and its half-width (as the Sunfall's round).
const WARHEAD_LENGTH: f32 = crate::gpu_consts::missile::WARHEAD_LENGTH;
const WARHEAD_RADIUS: f32 = crate::gpu_consts::missile::WARHEAD_RADIUS;
/// An interceptor's shard, nose to tail, and its half-width.
const SHARD_LENGTH: f32 = 22.0;
const SHARD_RADIUS: f32 = 2.6;

impl Renderer {
    /// Once a tick: the Progenitor's missiles now, and their trails laid as far as they
    /// have flown.
    pub(super) fn progenitor_missiles(&mut self, frame: &RenderFrame, time: f32) {
        let ours: Vec<StrategicInstance> = frame
            .strategic
            .iter()
            .filter(|m| m.look == nuke_look::PROGENITOR)
            .copied()
            .collect();
        self.progenitor_fx
            .trails
            .retain(|(s, _)| ours.iter().any(|m| m.serial == *s));
        for m in &ours {
            let warhead = m.kind == STRATEGIC_WARHEAD;
            let now = Vec3::from(m.pos);
            let last = self
                .progenitor_fx
                .trails
                .iter()
                .find(|(s, _)| *s == m.serial)
                .map_or(Vec3::from(m.prev_pos), |t| t.1);
            let step = if warhead { 60.0 } else { 35.0 };
            if now.distance(last) < step {
                if !self.progenitor_fx.trails.iter().any(|t| t.0 == m.serial) {
                    self.progenitor_fx.trails.push((m.serial, last));
                }
                continue;
            }
            // The trail stops a body's length behind the nose: the body covers the rest.
            let axis = (now - Vec3::from(m.prev_pos)).normalize_or(Vec3::Z);
            let body = if warhead {
                WARHEAD_LENGTH * m.scale
            } else {
                SHARD_LENGTH
            };
            let to = now - axis * body;
            let (width, life) = if warhead {
                (2.4 * m.scale.max(0.5), 18.0)
            } else {
                (2.4, 4.0)
            };
            if to.distance(last) > 1.0 {
                let seed = self.scatter.unit();
                self.lay_progenitor(
                    Piece::beam(last, to)
                        .size(width, width * 1.2)
                        .time(time, life, 0.0)
                        .power(if warhead { 1.2 } else { 1.0 })
                        .flags(TRAIL)
                        .seed(seed),
                );
            }
            match self
                .progenitor_fx
                .trails
                .iter_mut()
                .find(|t| t.0 == m.serial)
            {
                Some(t) => t.1 = to,
                None => self.progenitor_fx.trails.push((m.serial, to)),
            }
        }
        self.progenitor_fx.missiles = ours;
    }

    /// Every frame: the bodies in flight, between the last two ticks at `alpha`.
    pub(super) fn missiles_in_flight(&mut self, out: &mut Vec<Piece>, time: f32, alpha: f32) {
        for m in &self.progenitor_fx.missiles {
            let a = Vec3::from(m.prev_pos);
            let b = Vec3::from(m.pos);
            let at = a.lerp(b, alpha);
            let axis = (b - a).normalize_or(Vec3::Z);
            let side = axis.cross(Vec3::Z).normalize_or(Vec3::X);
            let up = side.cross(axis);
            if m.kind == STRATEGIC_WARHEAD {
                let len = WARHEAD_LENGTH * m.scale;
                let half = WARHEAD_RADIUS * m.scale * 1.4;
                out.push(
                    Piece::body(at, axis, up)
                        .size(half, len)
                        .power(1.0 + 1.5 * m.boost)
                        .now(time),
                );
                // The spear of light that drives it, longer while it boosts; none once it
                // falls.
                let climbing = axis.z > -0.05;
                if climbing {
                    let tail = at - axis * len;
                    let spear = len * (0.8 + 0.8 * m.boost);
                    out.push(
                        Piece::beam(tail, tail - axis * spear)
                            .size(half * 1.6, 0.2)
                            .power(1.8 + 1.2 * m.boost)
                            .flags(PULSES)
                            .now(time),
                    );
                }
            } else {
                out.push(
                    Piece::body(at, axis, up)
                        .size(SHARD_RADIUS, SHARD_LENGTH)
                        .power(2.2)
                        .flags(SHARD)
                        .now(time),
                );
            }
        }
    }

    /// An interceptor lifts out of its cell at `at`.
    pub(super) fn shard_launch(&mut self, at: Vec3, time: f32) {
        let seed = self.scatter.unit();
        self.lay_progenitor(
            Piece::ring(at, Vec3::Z)
                .size(6.0, 40.0)
                .width(3.0)
                .time(time, 0.8, 0.5)
                .power(2.0)
                .flags(DRAPE | ARCS)
                .tier(3.0)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::prism(at, Vec3::Z * 140.0)
                .size(3.5, 1.2)
                .time(time, 0.9, 0.3)
                .power(1.8)
                .seed(seed),
        );
        self.progenitor_fx
            .flash(at + Vec3::Z * 10.0, HOT * 4.0e4, 140.0, time, 0.6);
    }

    /// An interceptor met a warhead at `at`: a cage of light closes on it and it comes
    /// apart into lines.
    pub(super) fn warhead_unmade(&mut self, at: Vec3, time: f32) {
        let seed = self.scatter.unit();
        // The cage: three rings of arcs, square to each other, closing on it.
        for (k, normal) in [Vec3::X, Vec3::Y, Vec3::new(0.0, 0.6, 1.0).normalize()]
            .into_iter()
            .enumerate()
        {
            self.lay_progenitor(
                Piece::ring(at, normal)
                    .size(160.0, 6.0)
                    .width(6.0)
                    .time(time, 0.62, 0.9)
                    .power(2.4)
                    .flags(ARCS)
                    .tier(3.0)
                    .seed(seed + k as f32 * 0.29),
            );
        }
        self.unmade_lines(at, 40, (150.0, 450.0), time + 0.55);
        for k in 0..2 {
            let normal =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.6 + k as f32).normalize();
            self.lay_progenitor(
                Piece::ring(at, normal)
                    .size(20.0, 420.0)
                    .width(9.0)
                    .time(time + 0.55, 1.2, 0.7)
                    .power(1.8)
                    .flags(ARCS)
                    .tier(4.0)
                    .seed(seed + k as f32 * 0.4),
            );
        }
        self.progenitor_fx
            .flash(at, HOT * 4.0e6, 900.0, time + 0.5, 1.0);
    }

    /// A spent interceptor unmakes itself at `at`.
    pub(super) fn shard_spent(&mut self, at: Vec3, time: f32) {
        self.unmade_lines(at, 10, (20.0, 45.0), time);
        let normal = Vec3::new(self.scatter.signed(), self.scatter.signed(), 1.0).normalize();
        self.lay_progenitor(
            Piece::ring(at, normal)
                .size(3.0, 40.0)
                .width(2.0)
                .time(time, 0.6, 0.7)
                .power(1.4)
                .flags(ARCS)
                .tier(2.0),
        );
        self.progenitor_fx.flash(at, HOT * 2.0e5, 200.0, time, 0.4);
    }

    /// A Progenitor warhead broken open by an interceptor at `at`: its seams flash and
    /// it comes apart into lines.
    pub(super) fn warhead_broken(&mut self, at: Vec3, time: f32) {
        let seed = self.scatter.unit();
        self.lay_progenitor(
            Piece::shell(at, 1.0)
                .size(4.0, 80.0)
                .time(time, 0.9, 0.5)
                .power(1.8)
                .seed(seed),
        );
        self.unmade_lines(at, 24, (40.0, 140.0), time);
        self.progenitor_fx.flash(at, HOT * 3.0e6, 800.0, time, 0.9);
    }

    /// `n` lines of light flung out every way from `at`, `reach` metres long.
    fn unmade_lines(&mut self, at: Vec3, n: usize, reach: (f32, f32), start: f32) {
        for _ in 0..n {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let len = reach.0 + (reach.1 - reach.0) * self.scatter.unit();
            let from = at + dir * len * 0.1;
            let begin = start + self.scatter.unit() * 0.1;
            let life = 1.2 + 0.5 * self.scatter.unit();
            self.lay_progenitor(
                Piece::beam(from, at + dir * len)
                    .size(len * 0.012 + 0.6, 0.5)
                    .time(begin, life, 0.3)
                    .power(2.0)
                    .flags(LINE),
            );
        }
    }

    /// A warhead climbs out of the silo at `at`.
    pub(super) fn warhead_launch(&mut self, at: Vec3, time: f32) {
        let seed = self.scatter.unit();
        self.lay_progenitor(
            Piece::ring(at, Vec3::Z)
                .size(10.0, 120.0)
                .width(8.0)
                .time(time, 1.4, 0.5)
                .power(2.2)
                .flags(DRAPE | ARCS)
                .tier(5.0)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::prism(at, Vec3::Z * 700.0)
                .size(14.0, 4.0)
                .time(time, 2.6, 0.3)
                .power(2.0)
                .seed(seed),
        );
        self.progenitor_fx
            .flash(at + Vec3::Z * 30.0, HOT * 6.0e5, 500.0, time, 1.5);
    }

    /// A Progenitor warhead goes off at `at`, its blast `radius` metres: an unmaking.
    pub(super) fn unmaking(&mut self, at: Vec3, radius: f32, time: f32) {
        let fx = &mut self.progenitor_fx;
        if fx.heard.iter().any(|h| h.0.distance(at) < 1.0) {
            return;
        }
        fx.heard.push((at, time));
        let r = radius.max(50.0);
        let ground = Vec3::new(at.x, at.y, self.progenitor_ground(at));
        let seed = self.scatter.unit();
        // The sphere opening to the blast radius and breaking into lines, and the one
        // falling in on the middle after it.
        self.lay_progenitor(
            Piece::shell(at, 0.7)
                .size(r * 0.05, r)
                .time(time, 4.0, 0.25)
                .power(1.3)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::shell(at, 0.7)
                .size(r * 0.9, r * 0.05)
                .time(time + 1.6, 1.6, 0.9)
                .power(2.5)
                .seed(seed + 0.5),
        );
        // Over the ground: the ring of arcs running far out, and the mark.
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r * 0.2, r * 2.4)
                .width(r * 0.08)
                .time(time, 3.0, 0.8)
                .power(2.0)
                .flags(DRAPE | ARCS)
                .tier(6.0)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r)
                .time(time, 45.0, 0.0)
                .power(2.0)
                .flags(DRAPE | MARK)
                .tier(6.0)
                .seed(seed),
        );
        // The pillar standing up out of it.
        self.lay_progenitor(
            Piece::prism(ground, Vec3::Z * r * 4.0)
                .size(r * 0.1, r * 0.04)
                .time(time, 7.0, 0.15)
                .power(2.2)
                .seed(seed),
        );
        // Two rings turning over it.
        for k in 0..2 {
            let tilt = if k == 0 { 0.35 } else { -0.35 };
            let normal = Vec3::new(tilt, tilt * 0.5, 1.0).normalize();
            self.lay_progenitor(
                Piece::ring(at + Vec3::Z * r * 0.4, normal)
                    .size(r * 0.6, r * 1.3)
                    .width(r * 0.03)
                    .time(time + 0.2, 3.5, 0.6)
                    .power(1.6)
                    .flags(ARCS)
                    .tier(5.0)
                    .seed(seed + k as f32 * 0.37),
            );
        }
        // Lines of light lifting off the ground inside it: what it unmade.
        for _ in 0..48 {
            let a = self.scatter.unit() * TAU;
            let d = r * self.scatter.unit().sqrt();
            let xy = ground.truncate() + glam::Vec2::new(a.cos(), a.sin()) * d;
            let foot = xy.extend(self.progenitor_ground(xy.extend(0.0)));
            let rise = 150.0 + 450.0 * self.scatter.unit();
            let start = time + 0.3 + self.scatter.unit() * 1.5;
            self.lay_progenitor(
                Piece::beam(foot, foot + Vec3::Z * rise)
                    .size(1.5, 0.6)
                    .time(start, 2.0, 0.5)
                    .power(1.6)
                    .flags(LINE),
            );
        }
        self.progenitor_fx
            .flash(at + Vec3::Z * r * 0.3, HOT * 3.0e6, r * 4.0, time, 1.5);
        self.progenitor_fx.flash(
            ground + Vec3::Z * 20.0,
            LIGHT * 4.0e6,
            r * 2.5,
            time + 1.6,
            2.0,
        );
        // It still flattens the trees and throws back the clouds over it.
        let scale = r / 520.0;
        self.tree_blasts
            .record_wide(at, time, 2600.0 * scale, 5.0 * scale.sqrt(), 330.0);
        self.sky
            .blast(at + Vec3::Z * 400.0 * scale, 1400.0 * scale, 2.0, time);
    }
}
