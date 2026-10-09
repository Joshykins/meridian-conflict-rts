//! A dash as it is drawn (mc-sim `dash.rs`, the Regency Seraph), in the Regency's
//! light: red containment, white hearts; no orange, no violet (nothing here is nanites).
//!
//! - **Charge** (`SimEvent::DashCharging`, then each tick of it): the charge burns in its
//!   feathers (flight.wgsl, `wing_fx.rs`) and the light it gathers grows round it, whiter
//!   and more restless as it fills (`dash_charge_light`); under it the ground's dust is
//!   drawn in.
//! - **Vanish** (`DashVanished`, at `from`): it goes in a flicker of white light, and
//!   while it is gone flickers of light run along its path, one after another, out to
//!   where it comes down, arriving as it does. Everything is laid at once, each piece
//!   timed to its moment.
//! - **Landing** (`DashLanded`): a blinding white light where it comes down, dust driven
//!   out over the ground and a red-glassed crater; no ball or bubble of plasma at its
//!   feet: the wave is the shockwave. The wave that rolls out from it is its `dash_only` cone's
//!   (`ShotFired`, `wake_fx.rs`).
//!
//! Presentation only; the renderer's own clock.

use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

use super::craters::CraterStyle;
use super::lens_flare::Flare;
use super::stun_fx::Flash;
use super::{Renderer, PUFF_DUST};
use crate::gpu_consts::puff;
use mc_sim::mirror::UnitInstance;
use mc_sim::SimEvent;

const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);
/// `push_effect` kind: a white-hot flash (lights.rs `effect`).
const WHITE_FLASH: f32 = 9.0;

/// Metres between the flickers along its path, about, and the most of them: a deliberate
/// cosmetic cap, so a long dash stays a line of flickers and does not flood the puffs.
const FLICKER_STEP: f32 = 45.0;
const MOST_FLICKERS: usize = 48;

impl Renderer {
    /// The dash's part of a sim event. Everything else about it is drawn as usual.
    pub(super) fn dash_event(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::DashCharging {
                from,
                ticks,
                blueprint,
                ..
            } => {
                let (r, _) = self.dash_hull(*blueprint);
                let ground = Vec3::from(from.to_f32());
                let ground = ground
                    .truncate()
                    .extend(self.ground_height(ground.truncate()));
                let seconds = *ticks as f32 * self.tick_seconds.max(0.02);
                self.dust_drawn_in(ground, r, seconds, time);
            }
            SimEvent::DashVanished {
                from,
                to,
                ticks,
                blueprint,
                ..
            } => {
                let (r, h) = self.dash_hull(*blueprint);
                let seconds = *ticks as f32 * self.tick_seconds.max(0.02);
                let (from, to) = (Vec3::from(from.to_f32()), Vec3::from(to.to_f32()));
                self.vanish(from, r, h, time);
                self.path_flickers(from, to, r, h, seconds, time);
            }
            SimEvent::DashLanded { to, blueprint, .. } => {
                let (r, h) = self.dash_hull(*blueprint);
                let reach = self.dash_reach(*blueprint).unwrap_or(r * 3.0);
                self.land(Vec3::from(to.to_f32()), r, h, reach, time);
            }
            _ => {}
        }
    }

    /// Radius and height of the hull that dashed.
    fn dash_hull(&self, blueprint: mc_data::BlueprintId) -> (f32, f32) {
        let bp = self.blueprints.unit(blueprint);
        (bp.radius.to_f32(), bp.height.to_f32())
    }

    /// How far the wave of its landing reaches: its farthest `dash_only` weapon's range.
    fn dash_reach(&self, blueprint: mc_data::BlueprintId) -> Option<f32> {
        self.blueprints
            .unit(blueprint)
            .weapons
            .iter()
            .filter(|w| w.dash_only)
            .map(|w| w.range_max.to_f32())
            .reduce(f32::max)
    }

    /// The ground's dust round `ground` drawn in toward it over `seconds`.
    fn dust_drawn_in(&mut self, ground: Vec3, r: f32, seconds: f32, time: f32) {
        for i in 0..14 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 14.0;
            let dir = Vec3::new(a.cos(), a.sin(), 0.0);
            let out = r * (1.8 + 0.8 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                ground + dir * out,
                -dir * out / seconds.max(0.2) * 0.8,
                time,
                seconds + 0.2,
                (r * 0.25, r * 0.12),
            );
        }
    }

    /// This tick of a flier's charge (`charge` its share, 0 to 1): the light it gathers,
    /// round its chest, growing and whitening and flickering harder as it fills, with a
    /// glint at its heart over the last of it. Called a tick at a time, so a charge called
    /// off stops with it.
    pub(super) fn dash_charge_light(&mut self, u: &UnitInstance, charge: f32, time: f32) {
        let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
            return;
        };
        let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
        let chest = Vec3::from(u.pos) + Vec3::Z * h * 0.62;
        let dt = self.tick_seconds.max(0.02);
        let c = charge.clamp(0.0, 1.0);
        let k = c * c;
        self.emp_fx.flashes.push(Flash {
            pos: chest,
            start: time,
            life: dt * 1.6,
            color: RED.lerp(WHITE, k) * (6.0 + 90.0 * k),
            range: r * (2.0 + 4.0 * c),
            flicker: 0.2 + 0.6 * c,
        });
        // Short threads of light drawn in to its heart from all round, more and quicker
        // as it fills.
        let n = 1 + (5.0 * c) as usize;
        for _ in 0..n {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.6 * self.scatter.signed(),
            )
            .normalize_or(Vec3::X);
            let out = r * (0.5 + 0.7 * self.scatter.unit()) * (1.2 - 0.5 * c);
            let life = 0.3 + 0.2 * self.scatter.unit() - 0.15 * c;
            let start = time + dt * self.scatter.unit();
            self.push_lit(
                STREAK,
                chest + dir * out,
                -dir * out / life,
                start,
                life,
                (r * 0.004 * (1.0 + c), r * 0.003),
                RED.lerp(HOT, c) * (3.0 + 6.0 * c),
                0.0,
            );
        }
        if c > 0.6 {
            let g = (c - 0.6) / 0.4;
            self.lens_flares.flash(
                chest,
                Flare {
                    color: RED.lerp(WHITE, g) * (3.0 + 9.0 * g),
                    size: r * (0.4 + 0.8 * g),
                },
                time,
                dt * 1.5,
            );
        }
    }

    /// It goes from `at` (its feet) in a flicker of white light.
    fn vanish(&mut self, at: Vec3, r: f32, h: f32, time: f32) {
        let heart = at + Vec3::Z * h * 0.55;
        self.push_effect(heart.to_array(), time, r * 1.6, 0.22, WHITE_FLASH, 0.0);
        self.push_lit(
            GLOW,
            heart,
            Vec3::ZERO,
            time,
            0.18,
            (r * 1.2, r * 0.05),
            WHITE * 6.0,
            0.0,
        );
        // It flickers as it goes: three snaps of light, each fainter.
        for (i, k) in [1.0f32, 0.55, 0.3].into_iter().enumerate() {
            let start = time + i as f32 * 0.06;
            self.lens_flares.flash(
                heart,
                Flare {
                    color: WHITE * 26.0 * k,
                    size: r * 2.2 * k,
                },
                start,
                0.12,
            );
            self.emp_fx.flashes.push(Flash {
                pos: heart,
                start,
                life: 0.1,
                color: WHITE * 260.0 * k,
                range: r * 8.0,
                flicker: 0.6,
            });
        }
        // The dust it stood in drawn in after it.
        for i in 0..16 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 16.0;
            let dir = Vec3::new(a.cos(), a.sin(), 0.0);
            let out = r * (1.6 + 0.6 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                at + dir * out,
                -dir * out * 1.6,
                time,
                0.6,
                (r * 0.3, r * 0.15),
            );
        }
    }

    /// While it is gone (`seconds`), flickers of light along its path from `from` to `to`
    /// (feet), one after another out to where it comes down, the last as it lands. The
    /// path bows up a little between them.
    fn path_flickers(&mut self, from: Vec3, to: Vec3, r: f32, h: f32, seconds: f32, time: f32) {
        let lift = Vec3::Z * h * 0.55;
        let (a, b) = (from + lift, to + lift);
        let span = a.distance(b);
        let along = (b - a).normalize_or(Vec3::X);
        let bow = span * 0.05;
        let at = |k: f32| a.lerp(b, k) + Vec3::Z * bow * (PI * k).sin();
        let n = ((span / FLICKER_STEP) as usize).clamp(6, MOST_FLICKERS);
        for i in 0..n {
            let k = (i as f32 + 0.5 + 0.4 * self.scatter.signed()) / n as f32;
            let k = k.clamp(0.0, 1.0);
            // Out along the path at the pace it is gone, a little ahead and behind.
            let start = time + seconds * (0.08 + 0.9 * k) + 0.03 * self.scatter.signed();
            let side = Vec3::new(-along.y, along.x, 0.0) * r * 0.5 * self.scatter.signed();
            let p = at(k) + side + Vec3::Z * r * 0.4 * self.scatter.signed();
            let heat = self.scatter.unit();
            // A snap of light, and a short thread of it along the way it goes.
            self.push_lit(
                GLOW,
                p,
                Vec3::ZERO,
                start,
                0.1 + 0.08 * heat,
                (r * (0.35 + 0.35 * heat), r * 0.02),
                HOT.lerp(WHITE, heat) * (5.0 + 5.0 * heat),
                0.0,
            );
            let reach = FLICKER_STEP * (0.8 + 0.8 * self.scatter.unit());
            self.push_lit(
                STREAK,
                p,
                along * reach / 0.14,
                start,
                0.14,
                (r * 0.012, r * 0.012),
                RED.lerp(WHITE, 0.4 + 0.6 * heat) * 9.0,
                0.0,
            );
            // Every third one lights the ground under it and glints.
            if i % 3 == 0 {
                self.emp_fx.flashes.push(Flash {
                    pos: p,
                    start,
                    life: 0.12,
                    color: HOT.lerp(WHITE, heat) * 120.0,
                    range: r * 5.0,
                    flicker: 0.7,
                });
                self.lens_flares.flash(
                    p,
                    Flare {
                        color: WHITE * (6.0 + 8.0 * heat),
                        size: r * 0.9,
                    },
                    start,
                    0.1,
                );
            }
        }
    }

    /// It comes down at `at` (its feet) in a blinding light; its wave reaches `reach`.
    fn land(&mut self, at: Vec3, r: f32, h: f32, reach: f32, time: f32) {
        let ground = at
            .truncate()
            .extend(self.ground_height(at.truncate()).max(at.z));
        let heart = ground + Vec3::Z * h * 0.45;
        // The light it lands in: white past the screen, a broad glare, then a red afterglow.
        self.push_effect(heart.to_array(), time, reach * 0.55, 0.35, WHITE_FLASH, 0.0);
        self.push_effect(
            heart.to_array(),
            time + 0.05,
            reach * 0.35,
            0.8,
            WHITE_FLASH,
            0.0,
        );
        self.push_lit(
            GLOW,
            heart,
            Vec3::ZERO,
            time,
            0.35,
            (r * 2.5, r * 0.4),
            WHITE * 10.0,
            0.0,
        );
        self.lens_flares.flash(
            heart,
            Flare {
                color: WHITE * 40.0,
                size: reach * 1.4,
            },
            time,
            0.9,
        );
        self.emp_fx.flashes.push(Flash {
            pos: heart,
            start: time,
            life: 1.0,
            color: WHITE * 600.0,
            range: (reach * 2.5).min(900.0),
            flicker: 0.25,
        });
        self.emp_fx.flashes.push(Flash {
            pos: heart,
            start: time + 0.25,
            life: 2.0,
            color: RED * 160.0,
            range: (reach * 2.0).min(900.0),
            flicker: 0.35,
        });
        // Dust driven out over the ground in a ring.
        for i in 0..32 {
            let a = TAU * (i as f32 + self.scatter.unit() * 0.5) / 32.0;
            let dir = Vec2::new(a.cos(), a.sin()).extend(0.08);
            let from = ground + dir * reach * 0.15;
            let speed = reach * (0.9 + 0.5 * self.scatter.unit());
            self.push_puff(
                PUFF_DUST,
                from,
                dir * speed,
                time,
                1.3,
                (reach * 0.06, reach * 0.14),
            );
        }
        self.add_crater_styled(
            ground.truncate(),
            reach * 0.3,
            0.6,
            time,
            CraterStyle::Plasma,
        );
    }
}
