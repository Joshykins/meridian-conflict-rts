//! The energy core a held beam is laid from (`Model::beam_core`, the Scourge's pod under
//! the keel), charging, firing and winding down. What drives it is how far the beam's
//! projector has run up (`UnitInstance::recoil`, last tick's in `prev_recoil`: 0 cold to 1
//! lit; `mirror::beam_brace`), so it keeps time with the sim's charge and with the
//! charge sound:
//!
//! - **Charging** (rising): a ball of plasma forms under the lens and swells, rose going
//!   over to white; arcs crawl in to it from round the aperture and motes are drawn in
//!   spiralling, more and faster as it fills; in the last quarter a glow gathers under
//!   the pod, and the core lights the hull round it brighter and brighter.
//! - **Ignition** (the tick it is full): a white flash under the pod and lightning
//!   thrown out round it, as the beam lights (`plasma_fx.rs` draws the beam).
//! - **Firing**: the ball held white-hot at the lens, beating, arcs snapping off it.
//! - **Winding down** (falling): the ball shrinks and dims, and the core vents motes.
//!
//! Presentation only; the renderer's own clock.

use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

use super::star_core_fx::{LAVENDER, ROSE, WHITE};
use super::Renderer;
use crate::camera::Camera;
use crate::gpu_consts::puff;

const ORB: f32 = puff::PLASMA_ORB as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
const ARC: f32 = puff::WARP_ARC as f32;
const MOTE: f32 = puff::WARP_MOTE as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
/// What the vented plasma cools to.
const VIOLET: Vec3 = Vec3::new(0.6, 0.3, 1.0);

/// A core lit this tick, for its light.
#[derive(Clone, Copy)]
struct Lit {
    centre: Vec3,
    r: f32,
    level: f32,
    seed: f32,
}

/// Each blueprint's core (`None`: it has none), and the cores lit this tick.
#[derive(Default)]
pub(super) struct LanceCores {
    cores: Vec<Option<[f32; 4]>>,
    lit: Vec<Lit>,
}

impl LanceCores {
    /// `cores`: per blueprint, in id order.
    pub(super) fn new(cores: Vec<Option<[f32; 4]>>) -> Self {
        Self {
            cores,
            lit: Vec::new(),
        }
    }

    /// Forgets this tick's lit cores, keeping the blueprints' cores.
    pub(super) fn clear(&mut self) {
        self.lit.clear();
    }
}

impl Renderer {
    /// Once a sim tick: every core that is charging, firing or winding down.
    pub(super) fn lance_core_tick(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        self.plasma_fx.cores.lit.clear();
        if self.plasma_fx.cores.cores.iter().all(Option::is_none) {
            return;
        }
        let hidden = KIND_WRECK | KIND_GHOST | STATE_RADAR;
        let reach = camera.distance * 2.5 + 600.0;
        let focus = camera.focus.truncate();
        for u in units {
            if u.owner_flags & hidden != 0 || u.recoil.max(u.prev_recoil) < 0.01 {
                continue;
            }
            let Some(Some(core)) = self.plasma_fx.cores.cores.get(u.blueprint as usize) else {
                continue;
            };
            let core = *core;
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            let (s, c) = u.heading.sin_cos();
            let centre = at
                + Vec3::new(
                    core[0] * c - core[1] * s,
                    core[0] * s + core[1] * c,
                    core[2],
                );
            let lit = Lit {
                centre,
                r: core[3],
                level: u.recoil.clamp(0.0, 1.0),
                seed: (u.unit_id % 97) as f32 * 0.613,
            };
            self.plasma_fx.cores.lit.push(lit);
            let before = u.prev_recoil.clamp(0.0, 1.0);
            if lit.level >= 1.0 && before < 1.0 {
                self.core_ignites(&lit, time);
            }
            if lit.level >= 1.0 {
                self.core_fires(&lit, time);
            } else if lit.level >= before {
                self.core_charges(&lit, time);
            } else {
                self.core_winds_down(&lit, time);
            }
        }
    }

    /// The ball under the lens, `across` wide.
    fn core_ball(&mut self, lit: &Lit, across: f32, rgb: Vec3, fusion: f32, time: f32) {
        let life = self.tick_seconds.clamp(0.03, 0.25) * 2.0;
        self.push_lit(
            ORB,
            lit.centre - Vec3::Z * (across * 0.35),
            Vec3::ZERO,
            time,
            life,
            (across, across * 1.04),
            rgb,
            fusion,
        );
    }

    /// A point round the aperture at angle `a`, `k` lens radii out from its middle.
    fn round_core(lit: &Lit, a: f32, k: f32) -> Vec3 {
        lit.centre + Vec3::new(a.cos(), a.sin(), 0.0) * lit.r * k
    }

    fn core_charges(&mut self, lit: &Lit, time: f32) {
        let f = lit.level;
        let dt = self.tick_seconds.max(0.02);
        let across = lit.r * (0.5 + 1.7 * f.powf(1.5));
        self.core_ball(
            lit,
            across,
            ROSE.lerp(WHITE, f) * (1.4 + 5.0 * f * f),
            f,
            time,
        );
        // Arcs crawling in from round the aperture.
        for _ in 0..1 + (f * 4.0) as usize {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let start = Self::round_core(lit, a, 2.4 + 0.4 * self.scatter.unit());
            let to = lit.centre - Vec3::Z * across * 0.3;
            let tint = LAVENDER.lerp(WHITE, f * self.scatter.unit());
            let lasts = 0.08 + 0.06 * self.scatter.unit();
            let k = self.scatter.unit();
            self.push_lit(
                ARC,
                start,
                (to - start) * 0.95,
                time + k * dt,
                lasts,
                (lit.r * 0.22, lit.r * 0.22),
                tint * (3.0 + 3.0 * f),
                0.0,
            );
        }
        // Motes drawn in, spiralling, faster as it fills.
        for _ in 0..2 + (f * 6.0) as usize {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let start = Self::round_core(lit, a, 3.2 + 1.2 * self.scatter.unit())
                - Vec3::Z * (lit.r * 0.8 * self.scatter.unit());
            let lasts = 0.5 - 0.2 * f;
            let swirl = Vec3::new(-a.sin(), a.cos(), 0.0) * lit.r * (2.0 + 4.0 * f);
            let k = self.scatter.unit();
            self.push_lit(
                MOTE,
                start,
                (lit.centre - start) / lasts + swirl,
                time + k * dt,
                lasts,
                (lit.r * 0.09, lit.r * 0.03),
                ROSE * (2.0 + 2.0 * f),
                0.0,
            );
        }
        if f > 0.75 {
            // The glow gathering under the pod before it lights.
            let g = (f - 0.75) * 4.0;
            self.push_lit(
                GLOW,
                lit.centre - Vec3::Z * lit.r,
                Vec3::ZERO,
                time,
                dt * 2.0,
                (lit.r * 2.5, lit.r * (3.0 + 2.0 * g)),
                ROSE.lerp(WHITE, g) * (2.0 + 6.0 * g),
                0.0,
            );
        }
    }

    fn core_ignites(&mut self, lit: &Lit, time: f32) {
        self.push_lit(
            GLOW,
            lit.centre - Vec3::Z * lit.r,
            Vec3::ZERO,
            time,
            0.3,
            (lit.r * 4.0, lit.r * 10.0),
            WHITE * 10.0,
            0.0,
        );
        // Lightning thrown out round the pod as the beam lights.
        for i in 0..14 {
            let a = std::f32::consts::TAU * (i as f32 + self.scatter.unit()) / 14.0;
            let out = Vec3::new(a.cos(), a.sin(), -0.35 - 0.4 * self.scatter.unit());
            let tint = WHITE.lerp(LAVENDER, self.scatter.unit());
            let thrown = out * lit.r * (12.0 + 8.0 * self.scatter.unit());
            self.push_lit(
                STREAK,
                lit.centre,
                thrown,
                time,
                0.35,
                (lit.r * 0.3, lit.r * 0.1),
                tint * 6.0,
                0.0,
            );
        }
    }

    fn core_fires(&mut self, lit: &Lit, time: f32) {
        let beat =
            1.0 + 0.12 * (time * 9.0 + lit.seed).sin() + 0.06 * (time * 23.0 + lit.seed).sin();
        self.core_ball(lit, lit.r * 2.2 * beat, WHITE * 6.0 * beat, 1.0, time);
        if self.scatter.unit() < 0.45 {
            // An arc snapping off the ball to the aperture's rim.
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let to = Self::round_core(lit, a, 2.3);
            let from = lit.centre - Vec3::Z * lit.r * 0.6;
            let lasts = 0.06 + 0.05 * self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                to - from,
                time,
                lasts,
                (lit.r * 0.2, lit.r * 0.2),
                WHITE.lerp(LAVENDER, 0.5) * 5.0,
                0.0,
            );
        }
    }

    fn core_winds_down(&mut self, lit: &Lit, time: f32) {
        let f = lit.level;
        let across = lit.r * (0.5 + 1.7 * f.powf(1.5));
        self.core_ball(lit, across, ROSE * (1.0 + 4.0 * f * f), f * 0.5, time);
        // The core venting: motes thrown out and down, cooling to violet.
        for _ in 0..2 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let lasts = 0.6 + 0.5 * self.scatter.unit();
            let vent = out * lit.r * (3.0 + 3.0 * self.scatter.unit()) - Vec3::Z * lit.r * 2.0;
            self.push_lit(
                MOTE,
                Self::round_core(lit, a, 1.0),
                vent,
                time,
                lasts,
                (lit.r * 0.1, lit.r * 0.03),
                ROSE.lerp(VIOLET, 0.6) * 2.5,
                0.0,
            );
        }
    }

    /// Every frame: the lit cores light the hull and the air round them.
    pub(super) fn lance_core_lights(&mut self, time: f32) {
        for lit in &self.plasma_fx.cores.lit {
            let beat = 0.9 + 0.1 * (time * 9.0 + lit.seed).sin();
            let f = lit.level * lit.level;
            self.lights.lamp(
                lit.centre - Vec3::Z * lit.r,
                Vec3::NEG_Z,
                ROSE.lerp(WHITE, lit.level) * (14.0 * lit.r * f * beat),
                lit.r * (6.0 + 10.0 * lit.level),
                180.0,
                1.0,
            );
        }
    }
}
