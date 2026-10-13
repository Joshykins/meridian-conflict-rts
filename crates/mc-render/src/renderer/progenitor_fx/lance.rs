//! A Shaper's Lance (`mc_sim::lance`): its charge at the Lens, drawn every frame from
//! the Shaper's `lance_charge`, and its strike (`SimEvent::LanceStrike`).
//!
//! - **An auxiliary Shaper** gathering its next volley or launch: a column of light
//!   standing up out of its Lens over the last stretch, brighter as it comes due.
//! - **Charging**: a crystal of hard light grows at the Lens, lines are drawn in to it
//!   from a ring of points turning round it that closes as it fills, and rings of light
//!   close round it (one, then two crossed, then three). From the third tier lines climb
//!   to it from the collars below. Past half charged a thin line runs from the Lens to
//!   the mark, and the mark turns on the ground, the size of the last strike's splash.
//! - **The strike**: the Lance itself, from the Lens to the mark, in the look the user
//!   chose (`LanceLook`), a ring kicked out round the Lens, and at the mark a flash, a
//!   ring of arcs out to the sim's splash, lines of light flung up, and a mark burnt into
//!   the ground that cools from white to blue. From the second tier beams converge on the
//!   mark from the collars, from the third a lattice dome stands over it, from the fourth
//!   a pillar of light rises from it, from the fifth a second ring runs far out.

use super::{lens_of, tier_of, LanceLook, Piece, HOT, LIGHT};
use crate::gpu_consts::progenitor_fx::{
    ARCS, BRAID, DOME, DRAPE, LIGHT as SHARD, LINE, MARK, PULSES, TARGET,
};
use crate::gpu_consts::puff;
use crate::renderer::Renderer;
use glam::Vec3;
use mc_sim::survival::ShaperInstance;
use std::f32::consts::TAU;

const STREAK: f32 = puff::WARP_STREAK as f32;
/// The mark's size before a Lance has struck once.
const FIRST_SPLASH: f32 = 20.0;

/// Points on a Shaper's collars, below its Lens, that its lesser beams and lines come
/// from: `n` of them round it, `foot` the ground it stands on.
fn collars(s: &ShaperInstance, foot: Vec3, n: usize, turn: f32) -> Vec<Vec3> {
    let h = s.height.max(20.0);
    let reach = (s.foot * 0.22).clamp(4.0, 40.0);
    (0..n)
        .map(|k| {
            let a = turn + k as f32 / n as f32 * TAU;
            let up = [0.5, 0.62, 0.72][k % 3];
            foot + Vec3::new(a.cos() * reach, a.sin() * reach, h * up)
        })
        .collect()
}

/// An auxiliary Shaper gathering its next volley or launch (`fire_charge`): over its
/// last stretch a column of light stands up out of its Lens, brighter as it comes due.
fn fire_charge(out: &mut Vec<Piece>, s: &ShaperInstance, foot: Vec3, time: f32) {
    let f = ((s.fire_charge - 0.6) / 0.4).clamp(0.0, 1.0);
    if s.kind == 0 || f <= 0.0 {
        return;
    }
    let lens = lens_of(s, foot);
    let tier = tier_of(s).min(6.0);
    out.push(
        Piece::prism(
            lens - Vec3::Z * 10.0,
            Vec3::Z * (30.0 + 15.0 * tier) * (0.3 + 0.7 * f),
        )
        .size(5.0 + tier, 1.5)
        .power(0.3 + 1.7 * f * f)
        .seed((s.unit % 89) as f32 * 0.41)
        .now(time),
    );
}

impl Renderer {
    /// Every frame, every Shaper charging its Lance: the crystal, the lines and rings,
    /// and its mark.
    pub(super) fn lance_charges(&mut self, out: &mut Vec<Piece>, time: f32) {
        let shapers = self.progenitor_fx.shapers.clone();
        for (s, foot) in &shapers {
            fire_charge(out, s, *foot, time);
            let f = s.lance_charge.clamp(0.0, 1.0);
            if f <= 0.0 {
                continue;
            }
            let tier = tier_of(s).min(6.0);
            let lens = lens_of(s, *foot);
            let seed = (s.unit % 97) as f32 * 0.37;
            // The crystal: a shard of light standing in the Lens, growing as it fills.
            let len = (8.0 + 2.5 * tier) * (0.35 + 0.65 * f);
            out.push(
                Piece::body(lens + Vec3::Z * len * 0.5, Vec3::Z, Vec3::X)
                    .size((1.6 + 0.5 * tier) * (0.4 + 0.6 * f), len)
                    .power(0.6 + 2.4 * f)
                    .flags(SHARD)
                    .now(time),
            );
            // Lines drawn in from a ring of points turning round it, closing as it fills.
            let reach = s.height.max(20.0) * 0.1 * (1.25 - f) + 12.0;
            let n = 3 + 2 * tier as usize;
            for k in 0..n {
                let a = seed + time * 0.5 + k as f32 / n as f32 * TAU;
                let from = lens + Vec3::new(a.cos(), a.sin(), -0.4) * reach;
                out.push(
                    Piece::beam(from, lens)
                        .size(0.5 + 0.12 * tier, 0.2)
                        .power(0.4 + 2.4 * f * f)
                        .flags(LINE)
                        .now(time),
                );
            }
            // Rings closing round it: one, then two crossed, then three.
            let rings = 1 + (tier as usize - 1).min(2);
            for k in 0..rings {
                let tilt = [0.0, 0.6, -0.6][k];
                let spin = time * (0.7 + 0.3 * k as f32) * if k == 1 { -1.0 } else { 1.0 };
                let normal = Vec3::new(tilt * spin.cos(), tilt * spin.sin(), 1.0).normalize();
                let r = reach * (0.75 - 0.12 * k as f32);
                out.push(
                    Piece::ring(lens, normal)
                        .size(r, r)
                        .width(1.2 + 0.3 * tier)
                        .power(0.5 + 2.0 * f)
                        .flags(ARCS)
                        .tier(tier)
                        .seed(seed + k as f32 * 0.31)
                        .now(time),
                );
            }
            if tier >= 3.0 && f > 0.25 {
                // From the collars below, lines climbing to the Lens.
                let up = (f - 0.25) / 0.75;
                for from in collars(s, *foot, 2 + tier as usize, seed) {
                    out.push(
                        Piece::beam(from, from.lerp(lens, up))
                            .size(0.8 + 0.2 * tier, 0.6)
                            .power(0.8 + 1.6 * up)
                            .flags(LINE)
                            .now(time),
                    );
                }
            }
            let target = Vec3::from(s.lance_target);
            if target != Vec3::ZERO {
                let splash = self
                    .progenitor_fx
                    .splashes
                    .iter()
                    .find(|p| p.0 == s.unit)
                    .map_or(FIRST_SPLASH, |p| p.1);
                // Its mark turning on the ground, and past half charged a line to it.
                out.push(
                    Piece::ring(target, Vec3::Z)
                        .size(splash, splash)
                        .power(0.4 + 1.4 * f)
                        .flags(DRAPE | TARGET)
                        .now(time),
                );
                if f > 0.5 {
                    let flicker = 0.75 + 0.25 * (time * 31.0 + seed).sin();
                    out.push(
                        Piece::beam(lens, target)
                            .size(0.35, 0.5)
                            .power((f - 0.5) * 2.2 * flicker)
                            .flags(LINE)
                            .now(time),
                    );
                }
            }
        }
    }

    /// The Lance of `unit` struck `to`, `splash` metres round it.
    pub(super) fn lance_strike(&mut self, unit: u32, to: Vec3, splash: f32, time: f32) {
        let fx = &mut self.progenitor_fx;
        match fx.splashes.iter_mut().find(|p| p.0 == unit) {
            Some(p) => p.1 = splash,
            None => fx.splashes.push((unit, splash)),
        }
        let shaper = fx.shaper(unit).cloned();
        let (lens, tier, collar) = match &shaper {
            Some((s, foot)) => {
                let tier = tier_of(s).min(6.0);
                let n = (tier as usize).saturating_sub(1);
                (lens_of(s, *foot), tier, collars(s, *foot, n, time * 0.7))
            }
            // Not a Shaper: from straight over the mark.
            None => (to + Vec3::Z * 400.0, 1.0, Vec::new()),
        };
        let look = fx.lance;
        let width = (3.0 + 1.2 * tier).min(12.0);
        let life = 0.55 + 0.08 * tier;
        let seed = self.scatter.unit();
        match look {
            LanceLook::Spear => {
                self.lay_progenitor(
                    Piece::beam(lens, to)
                        .size(width * 0.7, width)
                        .time(time, life, 0.1)
                        .power(3.0)
                        .flags(PULSES),
                );
                self.lay_progenitor(
                    Piece::beam(lens, to)
                        .size(width * 2.2, width * 2.6)
                        .time(time, life * 0.8, 0.1)
                        .power(0.45),
                );
            }
            LanceLook::Braid => {
                self.lay_progenitor(
                    Piece::beam(lens, to)
                        .size(width, width * 1.2)
                        .time(time, life, 0.1)
                        .power(2.6)
                        .flags(BRAID),
                );
            }
            LanceLook::Gates => {
                self.lay_progenitor(
                    Piece::beam(lens, to)
                        .size(width * 0.35, width * 0.45)
                        .time(time, life, 0.08)
                        .power(3.5)
                        .flags(LINE),
                );
                // Rings flying down the beam to the mark, one after another.
                let dir = (to - lens).normalize_or(Vec3::NEG_Z);
                let n = 6 + 2 * tier as usize;
                for k in 0..n {
                    let s = k as f32 / n as f32;
                    self.lay_progenitor(
                        Piece::ring(lens.lerp(to, s), dir)
                            .size(width * 1.9, width * 1.3)
                            .width(width * 0.3)
                            .time(time + s * 0.16, 0.3, 0.5)
                            .power(2.2)
                            .flags(ARCS)
                            .tier(tier)
                            .seed(seed + s),
                    );
                }
            }
        }
        // Beams down from the collars, converging on the mark.
        for from in collar {
            self.lay_progenitor(
                Piece::beam(from, to)
                    .size(width * 0.25, width * 0.4)
                    .time(time + 0.04, life * 0.8, 0.12)
                    .power(1.8)
                    .flags(if look == LanceLook::Spear {
                        PULSES
                    } else {
                        LINE
                    }),
            );
        }
        // The ring kicked out round the Lens.
        self.lay_progenitor(
            Piece::ring(lens, Vec3::Z)
                .size(6.0, 30.0 + 6.0 * tier)
                .width(2.0 + tier * 0.5)
                .time(time, 0.5, 0.6)
                .power(2.0)
                .flags(ARCS)
                .tier(tier)
                .seed(seed),
        );
        self.lance_impact(to, splash, tier, time, seed);
        self.progenitor_fx.flash(
            lens,
            HOT * (6.0e4 * (1.0 + tier * 0.3)),
            150.0 + 30.0 * tier,
            time,
            life,
        );
    }

    /// What the Lance leaves where it strikes.
    fn lance_impact(&mut self, to: Vec3, splash: f32, tier: f32, time: f32, seed: f32) {
        let splash = splash.max(4.0);
        let ground = Vec3::new(to.x, to.y, self.progenitor_ground(to));
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(splash * 0.3, splash)
                .width(splash * 0.25 + 2.0)
                .time(time, 0.7, 0.4)
                .power(2.5)
                .flags(DRAPE | ARCS)
                .tier(tier)
                .seed(seed),
        );
        self.lay_progenitor(
            Piece::ring(ground, Vec3::Z)
                .size(splash, splash)
                .time(time, 10.0 + tier, 0.0)
                .power(1.5)
                .flags(DRAPE | MARK)
                .tier(tier)
                .seed(seed),
        );
        if tier >= 3.0 {
            // A lattice dome standing over the strike, breaking into lines.
            self.lay_progenitor(
                Piece::shell(ground, 0.6)
                    .size(splash * 0.5, splash * 0.85)
                    .time(time + 0.03, 0.9, 0.35)
                    .power(1.6)
                    .flags(DOME)
                    .seed(seed),
            );
        }
        if tier >= 4.0 {
            // A pillar of light rising from it.
            self.lay_progenitor(
                Piece::prism(ground, Vec3::Z * (80.0 + 40.0 * tier))
                    .size(splash * 0.22, splash * 0.08)
                    .time(time + 0.05, 1.2, 0.25)
                    .power(1.5)
                    .seed(seed),
            );
        }
        if tier >= 5.0 {
            self.lay_progenitor(
                Piece::ring(ground, Vec3::Z)
                    .size(splash, splash * 2.6)
                    .width(3.0)
                    .time(time, 1.0, 0.8)
                    .power(1.2)
                    .flags(DRAPE),
            );
        }
        // Lines of light flung up off it.
        for _ in 0..8 + 2 * tier as usize {
            let a = self.scatter.unit() * TAU;
            let up = 0.6 + 0.8 * self.scatter.unit();
            let out =
                Vec3::new(a.cos(), a.sin(), up).normalize() * splash * (1.5 + self.scatter.unit());
            let lasts = 0.35 + 0.3 * self.scatter.unit();
            let tint = LIGHT.lerp(HOT, self.scatter.unit());
            self.push_lit(
                STREAK,
                ground + Vec3::Z * 1.5,
                out,
                time,
                lasts,
                (1.2 + 0.2 * tier, 0.4),
                tint * 5.0,
                0.0,
            );
        }
        self.progenitor_fx.flash(
            ground + Vec3::Z * (4.0 + splash * 0.2),
            HOT * (1.2e5 * (1.0 + tier * 0.4)),
            splash * 6.0 + 60.0,
            time,
            0.6,
        );
    }

    /// The ground (or the sea) under `at`.
    pub(super) fn progenitor_ground(&self, at: Vec3) -> f32 {
        self.ground_height(at.truncate())
            .max(self.map_info.water_level.to_f32())
    }

    pub(super) fn lay_progenitor(&mut self, p: Piece) {
        self.progenitor_fx.lay(p);
    }
}
