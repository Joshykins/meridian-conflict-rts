//! The Progenitor's strategic missiles (`StrategicLook::Progenitor`, docs/NUKES.md): every
//! Shaper's anti-nuke and the Unmaker's warheads. Nothing here burns: no motor fire, no
//! smoke trail, no fireball.
//!
//! - **An interceptor** is a shard of hard light: on a Shaper, the blade that lifts off
//!   its halo (it leaves from that slot and eases onto the sim's path), with a ring of
//!   arcs kicked off round it. It leaves a line of light behind it that breaks into
//!   dashes. Where it meets a warhead a cage of five crossed rings of arcs, hundreds of
//!   metres across, closes on it over a second, and the warhead is unmade: lines of
//!   light flung out a kilometre every way, three rings of arcs opening round them, and a
//!   broken lattice spreading after. A spent one unmakes itself, smaller.
//! - **A warhead** is a solid body of pale alloy seamed with light, nose cut on a slant,
//!   pushed by a spear of light from its tail, and trailing a line of light that hangs
//!   long after it. It climbs out of a pillar of light. It goes off as an unmaking: a
//!   sphere of hard light (dimmer the bigger it is) opens to the sim's blast radius and
//!   breaks into lines, then a
//!   second one falls in on the middle; a ring of arcs runs out far over the ground, a
//!   pillar of light stands up out of it, lines of light lift off the ground inside it,
//!   two rings turn above it, and it leaves a burnt mark cooling for most of a minute.
//!   One broken open by an interceptor flashes and comes apart into lines.

use super::{Piece, HOT, LIGHT};
use crate::gpu_consts::nuke_look;
use crate::gpu_consts::progenitor_fx::{
    ARCS, CLOSE, DRAPE, LIGHT as SHARD, LINE, MARK, PULSES, TRAIL,
};
use crate::renderer::Renderer;
use glam::Vec3;
use mc_sim::mirror::{RenderFrame, StrategicInstance, STRATEGIC_WARHEAD};
use std::f32::consts::TAU;

/// A warhead's body at scale 1, nose to tail, and its half-width (as the Sunfall's round).
const WARHEAD_LENGTH: f32 = crate::gpu_consts::missile::WARHEAD_LENGTH;
const WARHEAD_RADIUS: f32 = crate::gpu_consts::missile::WARHEAD_RADIUS;
/// An interceptor's shard, nose to tail, and its half-width.
const SHARD_LENGTH: f32 = 22.0;
/// Seconds a Shaper's shard is held lifting off its halo slot, and then takes to come
/// onto the sim's path.
const SHARD_HOLD: f32 = 0.5;
const SHARD_EASE: f32 = 2.5;

/// A shard launched off a Shaper's halo: its slot, how far that is off the sim's launch
/// point, and the blade's length.
#[derive(Clone, Copy)]
pub(super) struct Launch {
    serial: u32,
    slot: Vec3,
    offset: Vec3,
    blade: f32,
}
/// Seconds the cage takes to close before the warhead is unmade.
const UNMADE_AT: f32 = 1.15;
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
        let tick = self.tick_seconds.max(0.02);
        for m in &ours {
            let warhead = m.kind == STRATEGIC_WARHEAD;
            let (prev, now) = self.drawn_path(m, tick);
            let last = self
                .progenitor_fx
                .trails
                .iter()
                .find(|(s, _)| *s == m.serial)
                .map_or(prev, |t| t.1);
            let step = if warhead { 60.0 } else { 35.0 };
            if now.distance(last) < step {
                if !self.progenitor_fx.trails.iter().any(|t| t.0 == m.serial) {
                    self.progenitor_fx.trails.push((m.serial, last));
                }
                continue;
            }
            // The trail stops a body's length behind the nose: the body covers the rest.
            let axis = (now - prev).normalize_or(Vec3::Z);
            let body = if warhead {
                WARHEAD_LENGTH * m.scale
            } else {
                self.shard_length(m.serial)
            };
            let to = now - axis * body;
            let (width, life) = if warhead {
                (2.4 * m.scale.max(0.5), 18.0)
            } else {
                ((body * 0.05).max(2.4), 4.0)
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
        self.progenitor_fx
            .launches
            .retain(|l| ours.iter().any(|m| m.serial == l.serial));
        self.progenitor_fx.missiles = ours;
    }

    /// Where missile `m` is drawn last tick and this: on the sim's path, but for a
    /// Shaper's shard leaving its halo slot (`shard_offset`).
    fn drawn_path(&self, m: &StrategicInstance, tick: f32) -> (Vec3, Vec3) {
        (
            self.shard_drawn(m.serial, m.age - tick, Vec3::from(m.prev_pos)),
            self.shard_drawn(m.serial, m.age, Vec3::from(m.pos)),
        )
    }

    /// Every frame: the bodies in flight, between the last two ticks at `alpha`.
    pub(super) fn missiles_in_flight(&mut self, out: &mut Vec<Piece>, time: f32, alpha: f32) {
        let tick = self.tick_seconds.max(0.02);
        for m in &self.progenitor_fx.missiles {
            let (a, b) = self.drawn_path(m, tick);
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
                let len = self.shard_length(m.serial);
                out.push(
                    Piece::body(at, axis, up)
                        .size(SHARD_RADIUS * len / SHARD_LENGTH, len)
                        .power(2.2)
                        .flags(SHARD)
                        .now(time),
                );
            }
        }
    }

    /// Interceptor `serial` leaves the cell at `from`. A Shaper's is the blade lifting
    /// off its halo (entity.wgsl `halo_slot`): the shard is drawn from that slot, eased
    /// onto the sim's path over its first seconds (`shard_offset`).
    pub(super) fn shard_launch(&mut self, serial: u32, from: Vec3, time: f32) {
        let at = match self.halo_slot_spent(from) {
            Some((slot, blade)) => {
                let fx = &mut self.progenitor_fx;
                // A cosmetic cap on launches remembered: the oldest goes first.
                if fx.launches.len() >= 64 {
                    fx.launches.remove(0);
                }
                fx.launches.push(Launch {
                    serial,
                    slot,
                    offset: slot - from,
                    blade,
                });
                // Its trail starts at the slot, not where the sim launched it.
                fx.trails.retain(|t| t.0 != serial);
                fx.trails.push((serial, slot));
                slot
            }
            None => from,
        };
        let seed = self.scatter.unit();
        // A ring of arcs kicked off the halo round the blade, and lines thrown up with it.
        self.lay_progenitor(
            Piece::ring(at, Vec3::Z)
                .size(4.0, 45.0)
                .width(3.0)
                .time(time, 0.9, 0.5)
                .power(2.2)
                .flags(ARCS)
                .tier(3.0)
                .seed(seed),
        );
        for k in 0..6 {
            let a = (k as f32 + seed) / 6.0 * TAU;
            let out = Vec3::new(a.cos() * 0.5, a.sin() * 0.5, 1.0).normalize();
            self.lay_progenitor(
                Piece::beam(at, at + out * 70.0)
                    .size(1.2, 0.3)
                    .time(time, 0.6, 0.4)
                    .power(1.6)
                    .flags(LINE),
            );
        }
        self.progenitor_fx.flash(at, HOT * 6.0e4, 180.0, time, 0.6);
    }

    /// Where the blade a Shaper standing over `from` just spent hung: the slot its halo
    /// emptied (entity.wgsl `halo_slot`, mc-models `shaper::halo_slot`), in the world.
    fn halo_slot_spent(&self, from: Vec3) -> Option<(Vec3, f32)> {
        use mc_sim::nukes::{LAUNCHER_CAPACITY_SHIFT, LAUNCHER_MARK, LAUNCHER_STOCK_MASK};
        let fx = &self.progenitor_fx;
        let (s, foot) = fx
            .shapers
            .iter()
            .filter(|(s, foot)| foot.truncate().distance(from.truncate()) < s.foot + 60.0)
            .min_by(|a, b| {
                let d = |f: &Vec3| f.truncate().distance_squared(from.truncate());
                d(&a.1).total_cmp(&d(&b.1))
            })?;
        let (heading, word) = fx
            .halos
            .iter()
            .find(|h| h.0 == s.unit)
            .map(|h| (h.1, h.2))?;
        if word & LAUNCHER_MARK == 0 {
            return None;
        }
        let n = crate::models::shaper::halo_slot_count(s.kind, s.level);
        let capacity = ((word >> LAUNCHER_CAPACITY_SHIFT) & 0xFF) as usize;
        let stock = (word & LAUNCHER_STOCK_MASK) as f32;
        // As the halo shares its slots out (one each, or a share each past its count).
        let per = n as f32 / capacity.max(n) as f32;
        let k = ((stock * per - 0.001).ceil().max(0.0) as usize).min(n - 1);
        let local = Vec3::from(crate::models::shaper::halo_slot(s.kind, s.level, k));
        let (sin, cos) = heading.sin_cos();
        let turned = Vec3::new(
            local.x * cos - local.y * sin,
            local.x * sin + local.y * cos,
            local.z,
        );
        let blade = crate::models::shaper::halo_blade(s.kind, s.level);
        Some((*foot + turned, blade))
    }

    /// Where a Shaper's shard is drawn `age` seconds after launch, `sim` where the sim
    /// has it: held at its halo slot, lifting off as its blade does (entity.wgsl), then
    /// let go onto the sim's path, the offset from the slot easing out by `SHARD_EASE`.
    fn shard_drawn(&self, serial: u32, age: f32, sim: Vec3) -> Vec3 {
        let Some(l) = self
            .progenitor_fx
            .launches
            .iter()
            .find(|l| l.serial == serial)
        else {
            return sim;
        };
        let age = age.max(0.0);
        let lifted = l.slot + Vec3::Z * l.blade * 0.6 * age * age;
        let t = (age / SHARD_EASE).clamp(0.0, 1.0);
        let path = sim + l.offset * (1.0 - t * t * (3.0 - 2.0 * t));
        let go = ((age - SHARD_HOLD) / 0.6).clamp(0.0, 1.0);
        lifted.lerp(path, go * go * (3.0 - 2.0 * go))
    }

    /// A Shaper's shard is its halo's blade: as long as it.
    fn shard_length(&self, serial: u32) -> f32 {
        self.progenitor_fx
            .launches
            .iter()
            .find(|l| l.serial == serial)
            .map_or(SHARD_LENGTH, |l| l.blade)
    }

    /// An interceptor met a warhead at `at`: a cage of rings closes on it and it is
    /// unmade into lines of light flung out a kilometre, rings of arcs opening round them
    /// and a broken lattice spreading after.
    pub(super) fn warhead_unmade(&mut self, at: Vec3, time: f32) {
        let seed = self.scatter.unit();
        // The cage: five rings of arcs, crossed, closing on it.
        let normals = [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            Vec3::new(1.0, 1.0, 0.8).normalize(),
            Vec3::new(-1.0, 1.0, 0.8).normalize(),
        ];
        for (k, normal) in normals.into_iter().enumerate() {
            self.lay_progenitor(
                Piece::ring(at, normal)
                    .size(380.0, 10.0)
                    .width(14.0)
                    .time(time, UNMADE_AT + 0.15, 0.88)
                    .power(2.6)
                    .flags(ARCS | CLOSE)
                    .tier(4.0)
                    .seed(seed + k as f32 * 0.29),
            );
        }
        let at_once = time + UNMADE_AT;
        self.unmade_lines(at, 70, (350.0, 1100.0), at_once);
        for k in 0..3 {
            let normal =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.4 + k as f32).normalize();
            self.lay_progenitor(
                Piece::ring(at, normal)
                    .size(40.0, 1100.0)
                    .width(20.0)
                    .time(at_once, 2.8, 0.7)
                    .power(2.0)
                    .flags(ARCS)
                    .tier(5.0)
                    .seed(seed + k as f32 * 0.4),
            );
        }
        // The broken lattice it leaves spreading.
        self.lay_progenitor(
            Piece::shell(at, 1.0)
                .size(60.0, 700.0)
                .time(at_once, 3.2, 0.45)
                .power(0.45)
                .seed(seed),
        );
        self.progenitor_fx
            .flash(at, HOT * 4.0e6, 1500.0, at_once, 1.6);
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
            let life = 2.4 + 0.8 * self.scatter.unit();
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
        // Bigger blasts are drawn no brighter as a whole: their lattice covers more.
        let dim = (420.0 / r).sqrt().min(1.0);
        // The sphere opening to the blast radius and breaking into lines, and the one
        // falling in on the middle after it.
        self.lay_progenitor(
            Piece::shell(at, 0.7)
                .size(r * 0.05, r)
                .time(time, 4.0, 0.25)
                .power(0.75 * dim)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::shell(at, 0.7)
                .size(r * 0.9, r * 0.05)
                .time(time + 1.6, 1.6, 0.9)
                .power(1.2 * dim)
                .seed(seed + 0.5),
        );
        // Over the ground: the ring of arcs running far out, and the mark.
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r * 0.2, r * 2.4)
                .width(r * 0.08)
                .time(time, 3.0, 0.8)
                .power(1.5 * dim)
                .flags(DRAPE | ARCS)
                .tier(6.0)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(r, r)
                .time(time, 45.0, 0.0)
                .power(1.4 * dim)
                .flags(DRAPE | MARK)
                .tier(6.0)
                .seed(seed),
        );
        // The pillar standing up out of it.
        self.lay_progenitor(
            Piece::prism(ground, Vec3::Z * r * 4.0)
                .size(r * 0.1, r * 0.04)
                .time(time, 7.0, 0.15)
                .power(1.4 * dim)
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
                    .power(1.2 * dim)
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
                    .power(1.1)
                    .flags(LINE),
            );
        }
        self.progenitor_fx.flash(
            at + Vec3::Z * r * 0.3,
            HOT * 1.2e6 * dim,
            r * 4.0,
            time,
            1.5,
        );
        self.progenitor_fx.flash(
            ground + Vec3::Z * 20.0,
            LIGHT * 1.5e6 * dim,
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
