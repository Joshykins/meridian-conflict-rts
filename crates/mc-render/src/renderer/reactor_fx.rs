//! An ARC reactor's held charge in the world (`models::Discharge`): while the plant runs,
//! near the eye, arcs crackle from the core's skin out to the electrode tips round it
//! and jump the gaps between its rings,
//! blue-white and short-lived, a few a tick, now and then a heavier one that lights the
//! plant for a moment and throws sparks off the tip it strikes; and now and then a wisp
//! of steam rises off a heat sink (its `Exhaust`). The core's churn, the turning blades
//! and the heat in the sinks are the entity shader's (`gpu_consts::reactor`).
//!
//! The arcs are lightning strokes among the EMP's (`stun_fx.rs`, sprites.wgsl colour 3).
//! Presentation only.

use super::stun_fx::{Flash, BOLT};
use super::water_fx::PUFF_STEAM;
use super::{Renderer, PUFF_SPARK};
use crate::models::{Discharge, Exhaust};
use glam::Vec3;
use mc_sim::mirror::UnitInstance;

/// The arcs' light: blue-white, as the lightning is.
const ARC_LIGHT: Vec3 = Vec3::new(0.5, 0.68, 1.0);
/// Wisps of steam a heat sink vents a second, on average.
const STEAM_CHANCE: f32 = 0.7;
/// Strokes every plant together may lay in one tick: a deliberate cosmetic cap, so a
/// yard of plants by the eye cannot crowd out a battle's lightning. Past it the plants
/// further down the list go quiet for that tick.
const TICK_BUDGET: usize = 360;

pub(super) struct ReactorFx {
    /// Per model slot (`UnitInstance::blueprint`): the core and its electrodes, and the
    /// heat sinks' ports, where steam vents.
    discharges: Vec<Option<Discharge>>,
    vents: Vec<Vec<Exhaust>>,
    /// Strokes left in this tick's budget.
    left: usize,
    /// Craters of plants gone up, still crackling (`reactor_blast.rs`).
    pub(super) aftermath: Vec<Aftermath>,
}

/// A plant's crater crackling after it went up.
#[derive(Clone, Copy)]
pub(super) struct Aftermath {
    pub(super) at: Vec3,
    /// How far from the middle its arcs land.
    pub(super) reach: f32,
    pub(super) start: f32,
    pub(super) until: f32,
}

impl ReactorFx {
    pub(super) fn new(discharges: Vec<Option<Discharge>>, vents: Vec<Vec<Exhaust>>) -> Self {
        Self {
            discharges,
            vents,
            left: 0,
            aftermath: Vec::new(),
        }
    }

    /// The height of the model's held charge over its foot, and its radius.
    pub(super) fn core_of(&self, blueprint: u32) -> Option<(f32, f32)> {
        let d = self.discharges.get(blueprint as usize)?.as_ref()?;
        Some((d.core[2], d.radius))
    }

    /// A new tick: the budget is full again.
    pub(super) fn new_tick(&mut self) {
        self.left = TICK_BUDGET;
    }

    /// The unit's model holds a charge.
    pub(super) fn holds(&self, blueprint: u32) -> bool {
        self.discharges
            .get(blueprint as usize)
            .is_some_and(Option::is_some)
    }
}

impl Renderer {
    /// This tick's arcs on a running plant.
    pub(super) fn reactor_arcs(&mut self, u: &UnitInstance, time: f32) {
        let Some(d) = self
            .reactor_fx
            .discharges
            .get(u.blueprint as usize)
            .cloned()
            .flatten()
        else {
            return;
        };
        if d.terminals.is_empty() && d.bridges.is_empty() {
            return;
        }
        let dt = self.tick_seconds.max(0.02);
        let (s, c) = u.heading.sin_cos();
        let at = Vec3::from(u.pos);
        let world = |p: [f32; 3]| at + Vec3::new(p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]);
        // Now and then a wisp of steam off a heat sink, rising and drifting.
        let vents = self
            .reactor_fx
            .vents
            .get(u.blueprint as usize)
            .cloned()
            .unwrap_or_default();
        for v in &vents {
            if self.scatter.unit() > STEAM_CHANCE * dt {
                continue;
            }
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * v.radius;
            let rise = Vec3::new(
                self.scatter.signed() * 0.4,
                self.scatter.signed() * 0.4,
                1.4,
            ) * (1.0 + v.radius * 0.6);
            let life = 1.6 + self.scatter.unit() * 1.4;
            let start = time + self.scatter.unit() * dt;
            self.push_puff(
                PUFF_STEAM,
                world(v.at) + off,
                rise,
                start,
                life,
                (v.radius * 0.5, v.radius * 2.2),
            );
        }
        let core = world(d.core);
        let r = d.radius;
        // A bigger core holds more charge: more arcs at once, and fatter.
        let arcs = 1 + (r * 0.5) as usize + usize::from(self.scatter.unit() < 0.5);
        for _ in 0..arcs {
            if self.reactor_fx.left < 8 {
                return;
            }
            let ends = d.terminals.len() + d.bridges.len();
            let pick = ((self.scatter.unit() * ends as f32) as usize).min(ends - 1);
            let (from, tip) = if let Some(&(a, b)) = pick
                .checked_sub(d.terminals.len())
                .and_then(|i| d.bridges.get(i))
            {
                // Across a gap between rings.
                (world(a), world(b))
            } else {
                let tip = world(d.terminals[pick]);
                let to_tip = (tip - core).normalize_or(Vec3::Z);
                // Off the core's skin on the near side, wandering a little round it.
                let off = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.45;
                (core + (to_tip + off).normalize_or(to_tip) * r * 0.92, tip)
            };
            let to_tip = (tip - from).normalize_or(Vec3::Z);
            let side = to_tip.cross(Vec3::Z).normalize_or(Vec3::X);
            let up = side.cross(to_tip).normalize_or(Vec3::Z);
            let len = from.distance(tip);
            let heavy = self.scatter.unit() < 0.12;
            let start = time + self.scatter.unit() * dt;
            let life = if heavy {
                0.22
            } else {
                0.07 + 0.09 * self.scatter.unit()
            };
            let width = r * if heavy {
                0.09
            } else {
                0.035 + 0.025 * self.scatter.unit()
            };
            let kinks = 5 + (len / (r * 0.6).max(0.5)) as usize;
            self.emp_kinks(
                from,
                tip,
                len * 0.16,
                side,
                up,
                start,
                life,
                width,
                kinks.min(10),
                BOLT,
            );
            self.reactor_fx.left -= kinks.min(10);
            if heavy {
                // A fork off it, and a flash of light and sparks where it strikes.
                let mid = from.lerp(tip, 0.4 + 0.3 * self.scatter.unit());
                let stray =
                    mid + (side * self.scatter.signed() + up * self.scatter.signed()) * len * 0.35;
                self.emp_kinks(
                    mid,
                    stray,
                    len * 0.08,
                    side,
                    up,
                    start,
                    life,
                    width * 0.6,
                    4,
                    BOLT,
                );
                self.reactor_fx.left = self.reactor_fx.left.saturating_sub(4);
                self.emp_fx.flashes.push(Flash {
                    pos: core.lerp(tip, 0.5),
                    start,
                    life: 0.25,
                    color: ARC_LIGHT * (1.0 + r * 0.2),
                    range: r * 4.0 + 4.0,
                    flicker: 1.0,
                });
                for _ in 0..3 {
                    let vel = (self.scatter.upward(0.2) - to_tip * 0.5)
                        * (3.0 + 5.0 * self.scatter.unit());
                    let life = 0.3 + 0.3 * self.scatter.unit();
                    self.push_puff(PUFF_SPARK, tip, vel, start, life, (0.12 + r * 0.02, 0.05));
                }
            }
        }
    }
}
