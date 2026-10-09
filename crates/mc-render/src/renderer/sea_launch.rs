//! A warhead out of a boat's tube at sea (`Strategic::tubes`): a cold launch. Gas throws
//! the missile up out of its tube with no fire; it boils the sea over the boat, breaks
//! the surface in a dome of white water and hangs a moment on the spray, and only clear
//! of the sea does its motor light, the exhaust beating the water flat into a ring of
//! steam that rolls out over the waves and a column of it standing round the climb.
//! Until then it lays no trail and shows no plume (`nuke_fx.rs` asks `unlit`).
//!
//! Presentation only: the sim flies the missile from the tube mouth the same either way.

use glam::Vec3;
use mc_sim::mirror::StrategicInstance;

use super::water_fx::{PUFF_COLUMN, PUFF_DROPLET, PUFF_SPRAY, PUFF_STEAM};
use super::{Renderer, PUFF_FIREBALL};
use crate::gpu_consts::missile::WARHEAD_LENGTH;

/// Metres the nozzle stands over the sea when the motor lights, at scale 1.
const LIGHT_CLEAR: f32 = 16.0;
/// The boat's missile (`missile_scale` 0.35) is the size these effects are tuned at.
const TUNED_SCALE: f32 = 0.35;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Thrown up out of the tube, under the water or just over the deck.
    Rising,
    /// Through the surface, not yet lit.
    Breached,
    Lit,
}

/// The warheads launched at sea whose motors have not lit yet, or lit lately.
#[derive(Default)]
pub(super) struct SeaLaunches {
    live: Vec<(u32, Stage)>,
}

impl SeaLaunches {
    /// Whether the warhead `serial` is out of a boat's tube and its motor not yet lit.
    pub(super) fn unlit(&self, serial: u32) -> bool {
        self.live
            .iter()
            .any(|&(s, stage)| s == serial && stage != Stage::Lit)
    }
}

impl Renderer {
    /// A warhead thrown out of a tube at `at`, at sea (`nuke_event`): the gas boils out
    /// of the mouth and up through the water.
    pub(super) fn sea_launch(&mut self, at: Vec3, serial: u32, time: f32) {
        if self.nuke_fx.sea.live.iter().any(|&(s, _)| s == serial) {
            return;
        }
        self.nuke_fx.sea.live.push((serial, Stage::Rising));
        let origin = self.effect_origin.replace(at);
        let water = self.sea_level();
        let surface = Vec3::new(at.x, at.y, water);
        if at.z < water - 0.3 {
            // Dived: a thump through the hull and a boil of gas over the tube.
            self.push_ripple(at, time, 9.0, 1.6, 2.5, 0.0);
            self.push_bubbles(at + Vec3::Z * 0.5, 2.4, 40, time, 0.4, 0.9);
            self.push_ripple(surface, time + 0.1, 8.0, 2.5, 0.0, 0.8);
        } else {
            // Surfaced: the gas bursts white out of the open mouth round the missile.
            for _ in 0..14 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 6.0
                    + Vec3::Z * (4.0 + self.scatter.unit() * 6.0);
                let r0 = self.scatter.unit();
                self.push_puff(
                    PUFF_STEAM,
                    at + Vec3::Z * 0.5,
                    out,
                    time + r0 * 0.3,
                    2.4 + r0,
                    (1.5, 5.0 + r0 * 2.0),
                );
            }
        }
        self.effect_origin = origin;
    }

    /// Every tick, for each strategic missile: a warhead launched at sea breaks the surface
    /// and then lights clear of it. Forgets the ones that are gone.
    pub(super) fn sea_launch_tick(&mut self, missiles: &[StrategicInstance], time: f32) {
        self.nuke_fx
            .sea
            .live
            .retain(|(s, _)| missiles.iter().any(|m| m.serial == *s));
        let water = self.sea_level();
        for m in missiles {
            let Some(i) = self.nuke_fx.sea.live.iter().position(|l| l.0 == m.serial) else {
                continue;
            };
            let nose = Vec3::from(m.pos);
            let axis = (nose - Vec3::from(m.prev_pos)).normalize_or(Vec3::Z);
            let nozzle = nose - axis * WARHEAD_LENGTH * m.scale;
            let size = (m.scale / TUNED_SCALE).clamp(0.5, 3.0);
            let stage = self.nuke_fx.sea.live[i].1;
            if stage == Stage::Rising && nose.z >= water {
                self.nuke_fx.sea.live[i].1 = Stage::Breached;
                // Only one that came up through the water breaks it.
                if Vec3::from(m.prev_pos).z < water + 1.0 {
                    self.warhead_breach(Vec3::new(nose.x, nose.y, water), size, time);
                }
            }
            if stage != Stage::Lit && nozzle.z >= water + LIGHT_CLEAR * size {
                self.nuke_fx.sea.live[i].1 = Stage::Lit;
                self.warhead_ignition(nozzle, axis, water, size, time);
            }
        }
    }

    /// The missile breaking the surface at `surface`: a dome of white water heaved up
    /// round it, torn sheets thrown off, a ring of foam running out.
    fn warhead_breach(&mut self, surface: Vec3, size: f32, time: f32) {
        let origin = self.effect_origin.replace(surface);
        self.water_splash(surface, time, 5.0 * size, 1.6);
        self.push_ripple(surface, time, 14.0 * size, 4.0, 0.0, 1.0);
        self.push_ripple(surface, time + 0.25, 24.0 * size, 6.0, 0.0, 0.6);
        // The dome: a ring of low, broad columns round the hull of the missile.
        for k in 0..8 {
            let a = (k as f32 + self.scatter.unit() * 0.5) / 8.0 * std::f32::consts::TAU;
            let at = surface + Vec3::new(a.cos(), a.sin(), 0.0) * 2.4 * size;
            let tall = (6.0 + self.scatter.unit() * 5.0) * size;
            let v0 = (2.0 * 11.0 * tall).sqrt();
            let late = self.scatter.unit() * 0.08;
            self.push_puff(
                PUFF_COLUMN,
                at,
                Vec3::Z * v0,
                time + late,
                2.0 * v0 / 11.0 + 0.8,
                (1.4 * size, 3.2 * size),
            );
        }
        // Sheets of water torn off and thrown out, and the fine spray they leave hanging.
        for _ in 0..18 {
            let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0)
                .normalize_or(Vec3::X)
                * (4.0 + self.scatter.unit() * 6.0)
                + Vec3::Z * (8.0 + self.scatter.unit() * 8.0);
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_DROPLET,
                surface + Vec3::Z * 1.0,
                out * size.sqrt(),
                time + r0 * 0.15,
                2.4,
                (0.5 * size, 1.2 * size),
            );
        }
        for _ in 0..10 {
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 3.0
                + Vec3::Z * (2.0 + self.scatter.unit() * 3.0);
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_SPRAY,
                surface + Vec3::Z * (2.0 + r0 * 6.0) * size,
                drift,
                time + 0.2 + r0 * 0.4,
                4.0 + r0 * 2.0,
                (2.0 * size, 7.0 * size),
            );
        }
        self.effect_origin = origin;
    }

    /// The motor lighting at `nozzle`, `water` beneath it: a flash, the exhaust driven
    /// down onto the sea and beating it flat into a ring of steam that rolls out over the
    /// waves, and a column of steam left standing round the first of the climb.
    fn warhead_ignition(&mut self, nozzle: Vec3, axis: Vec3, water: f32, size: f32, time: f32) {
        let surface = Vec3::new(nozzle.x, nozzle.y, water);
        let origin = self.effect_origin.replace(surface);
        self.push_effect(nozzle.to_array(), time, 22.0 * size, 1.0, 2.0, 0.0);
        self.push_shockwave(
            (surface + Vec3::Z * 1.5).to_array(),
            time + 0.05,
            70.0 * size,
            1.3,
            0.6,
            1.0,
            Vec3::ZERO,
        );
        // The sea lit by the motor and pressed flat under the exhaust.
        self.push_ripple(surface, time, 30.0 * size, 1.2, 4.0, 0.0);
        self.push_ripple(surface, time + 0.1, 40.0 * size, 5.0, 0.0, 1.0);
        // Fire hammered down out of the nozzle onto the water.
        for _ in 0..10 {
            let down = -axis * (14.0 + self.scatter.unit() * 10.0)
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 4.0;
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_FIREBALL,
                nozzle,
                down * size.sqrt(),
                time + r0 * 0.3,
                0.9,
                (2.5 * size, 6.0 * size),
            );
        }
        // The ring of steam rolling out over the sea from under the climb.
        for k in 0..36 {
            let a = (k as f32 + self.scatter.unit()) / 36.0 * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.08) * (10.0 + self.scatter.unit() * 12.0);
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_STEAM,
                surface + Vec3::Z * 1.5,
                out * size.sqrt(),
                time + 0.1 + r0 * 1.2,
                6.0 + r0 * 4.0,
                (5.0 * size, 16.0 * size),
            );
        }
        // The column standing round the first of the climb, slow to thin.
        for i in 0..10 {
            let up = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 1.5
                + Vec3::Z * (1.5 + self.scatter.unit() * 2.0);
            let r0 = self.scatter.unit();
            self.push_puff(
                PUFF_STEAM,
                surface + Vec3::Z * (i as f32 * 3.0 * size),
                up,
                time + 0.2 + i as f32 * 0.08,
                8.0 + r0 * 4.0,
                (4.0 * size, 11.0 * size),
            );
        }
        self.effect_origin = origin;
    }
}
