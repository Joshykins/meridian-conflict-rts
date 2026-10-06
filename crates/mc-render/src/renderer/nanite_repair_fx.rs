//! Nanite repair in the world (`RenderFrame::mends`, `repair_fields`):
//!
//! - A hull the nanites are mending (its own nanite regen, or a friend's repair field) is
//!   crawled over by a haze of violet motes drifting in onto it and up it, soft swells of
//!   violet glowing in the haze.
//! - A mend by a field is fed: motes thrown from high on the field's carrier out to the
//!   hull, slowing as they reach it.
//! - The field itself: its edge stands on the ground as a low, faint violet curtain in
//!   slow streaks (`PUFF_VEIL`), and a thin nanite haze drifts up off the ground inside it.
//!
//! Presentation only, laid once a sim tick on the renderer's own clock; no state of its
//! own, so the field's edge is laid a stretch at a time at random.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};
use mc_sim::mirror::{MendView, RenderFrame, RepairFieldView};

use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::{nanite, puff};

const MOTE: f32 = puff::WARP_MOTE as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
/// How far a mote is carried in all on its `vel`, against the air (warp_puffs.wgsl).
const MOTE_DRAG: f32 = 1.8;
const VIOLET: Vec3 = Vec3::new(nanite::R, nanite::G, nanite::B);
/// Motes a tick on a hull mending itself, per metre of its radius, and at most.
const HULL_MOTES: f32 = 0.9;
const HULL_MOTES_MAX: f32 = 16.0;
/// How bright a mote burns (HDR, before its twinkle).
const MOTE_GLOW: f32 = 6.0;
/// The haze's soft glows on the hull: how big for its radius, and how bright.
const HAZE_SIZE: f32 = 0.45;
const HAZE_GLOW: f32 = 1.6;
/// The field's edge: metres of it a curtain covers, how tall it stands, how long it lives
/// and how bright it is.
const VEIL_STEP: f32 = 45.0;
const VEIL_TALL: f32 = 7.0;
const VEIL_LIFE: f32 = 3.5;
const VEIL_GLOW: f32 = 0.32;
/// Haze motes a tick per hundred metres of the field's radius.
const FIELD_MOTES: f32 = 2.0;

impl Renderer {
    /// Once a sim tick: the hulls the nanites mend and the repair fields near the eye.
    pub(super) fn nanite_repair_tick(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        if frame.mends.is_empty() && frame.repair_fields.is_empty() {
            return;
        }
        let focus = camera.focus.truncate();
        let reach = camera.distance * 2.5 + 600.0;
        let dt = self.tick_seconds.clamp(0.03, 0.25);
        for m in &frame.mends {
            if Vec2::new(m.pos[0], m.pos[1]).distance(focus) <= reach {
                self.mend_hull(m, time, dt);
            }
        }
        for f in &frame.repair_fields {
            if Vec2::new(f.pos[0], f.pos[1]).distance(focus) <= reach + f.radius {
                self.field_haze(f, time, dt, focus, reach);
            }
        }
    }

    /// The motes on one hull, and the stream feeding it when a field mends it.
    fn mend_hull(&mut self, m: &MendView, time: f32, dt: f32) {
        let at = Vec3::from(m.pos);
        let r = m.radius.max(0.5);
        let h = m.height.max(1.0);
        // A friend's field mends slower than a hull's own nanites: a thinner haze.
        let share = if m.from.is_some() { 0.5 } else { 1.0 };
        let count = (r * HULL_MOTES * share).clamp(1.0, HULL_MOTES_MAX);
        let mut n = count as usize;
        if self.scatter.unit() < count.fract() {
            n += 1;
        }
        let size = (0.065 * r).clamp(0.35, 1.0);
        for _ in 0..n {
            let a = self.scatter.unit() * TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let start = at
                + out * r * (0.45 + 0.4 * self.scatter.unit())
                + Vec3::Z * h * (0.08 + 0.85 * self.scatter.unit());
            // In onto the hull and up it.
            let vel = -out * r * 0.25 + Vec3::Z * h * (0.05 + 0.08 * self.scatter.unit());
            let life = 0.6 + 0.6 * self.scatter.unit();
            let start_at = time + self.scatter.unit() * dt;
            self.push_lit(
                MOTE,
                start,
                vel,
                start_at,
                life,
                (size * 0.5, size),
                VIOLET * MOTE_GLOW,
                0.0,
            );
        }
        // A soft swell of violet somewhere on the hull, the haze thick there.
        if self.scatter.unit() < 0.7 * share {
            let a = self.scatter.unit() * TAU;
            let on = at
                + Vec3::new(a.cos(), a.sin(), 0.0) * r * 0.35 * self.scatter.unit()
                + Vec3::Z * h * (0.2 + 0.7 * self.scatter.unit());
            let start_at = time + self.scatter.unit() * dt;
            let across = r * HAZE_SIZE * (0.7 + 0.6 * self.scatter.unit());
            self.push_lit(
                GLOW,
                on,
                Vec3::Z * h * 0.04,
                start_at,
                1.2,
                (across * 0.6, across),
                VIOLET * HAZE_GLOW,
                0.0,
            );
        }
        let Some(from) = m.from else {
            return;
        };
        if self.scatter.unit() < 0.6 {
            let from = Vec3::from(from);
            let to = at
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.4
                + Vec3::Z * h * (0.3 + 0.4 * self.scatter.unit());
            let life = 1.1;
            let carried = (1.0 - (-MOTE_DRAG * life).exp()) / MOTE_DRAG;
            let start_at = time + self.scatter.unit() * dt;
            self.push_lit(
                MOTE,
                from,
                (to - from) / carried,
                start_at,
                life,
                (0.6, 1.1),
                VIOLET * MOTE_GLOW,
                0.0,
            );
        }
    }

    /// A field's edge on the ground and the haze inside it, a little each tick.
    fn field_haze(&mut self, f: &RepairFieldView, time: f32, dt: f32, focus: Vec2, reach: f32) {
        let centre = Vec2::new(f.pos[0], f.pos[1]);
        let stretches = (TAU * f.radius / VEIL_STEP).max(8.0);
        let step = TAU / stretches;
        // As many a tick as keep the ring full over a curtain's life.
        let due = stretches * dt / VEIL_LIFE;
        let mut lay = due as usize;
        if self.scatter.unit() < due.fract() {
            lay += 1;
        }
        for _ in 0..lay {
            let a = (self.scatter.unit() * stretches).floor() * step;
            let at = centre + Vec2::new(a.cos(), a.sin()) * f.radius;
            if at.distance(focus) > reach {
                continue;
            }
            let run = Vec3::new(-a.sin(), a.cos(), 0.0) * f.radius * step * 1.4;
            let start_at = time + self.scatter.unit() * dt;
            self.push_drive(
                puff::VEIL as f32,
                at.extend(f.pos[2]),
                run,
                start_at,
                VEIL_LIFE,
                (VEIL_TALL, VEIL_TALL),
                VIOLET,
                VEIL_GLOW,
            );
        }
        let count = FIELD_MOTES * f.radius / 100.0;
        let mut n = count as usize;
        if self.scatter.unit() < count.fract() {
            n += 1;
        }
        for _ in 0..n {
            // Even over the disc's area.
            let a = self.scatter.unit() * TAU;
            let d = f.radius * self.scatter.unit().sqrt();
            let at = centre + Vec2::new(a.cos(), a.sin()) * d;
            let rise = Vec3::new(self.scatter.signed(), self.scatter.signed(), 2.0) * 2.0;
            let lift = 0.5 + 2.0 * self.scatter.unit();
            let start_at = time + self.scatter.unit() * dt;
            let life = 1.6 + self.scatter.unit();
            self.push_lit(
                MOTE,
                at.extend(f.pos[2] + lift),
                rise,
                start_at,
                life,
                (0.5, 0.9),
                VIOLET * MOTE_GLOW * 0.6,
                0.0,
            );
        }
    }
}
