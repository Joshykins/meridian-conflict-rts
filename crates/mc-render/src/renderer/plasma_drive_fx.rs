//! A Regency capital ship's plasma drives at the mouth (`models::plasma_drives`), over the
//! plumes `capital_fx.rs` lays (`puff::PLASMA_PLUME`): a violet glow swelling in each
//! mouth with the thrust, arcs crackling from the lip back along the plume where the
//! containment leaks, and motes of plasma shed into the wake. Near the eye only.
//!
//! The puffs are the warp's (`warp_puffs.wgsl`: `WARP_GLOW`, `WARP_ARC`, `WARP_MOTE`), as the
//! lift bells' are (`lift_fx.rs`). Presentation only.

use super::Renderer;
use crate::gpu_consts::puff;
use glam::Vec3;

const PUFF_GLOW: f32 = puff::WARP_GLOW as f32;
const PUFF_ARC: f32 = puff::WARP_ARC as f32;
const PUFF_MOTE: f32 = puff::WARP_MOTE as f32;

/// The drive's violet, and the rose-white of its hottest arcs.
pub(super) const VIOLET: Vec3 = Vec3::new(0.62, 0.16, 1.0);
const HOT: Vec3 = Vec3::new(1.0, 0.62, 0.9);

/// One drive mouth this tick, as `capital_fx` places it.
pub(super) struct Mouth {
    /// Its middle at the start and end of the tick.
    pub(super) at: [Vec3; 2],
    /// Aft along the plume, and the mouth's radius.
    pub(super) aft: Vec3,
    pub(super) radius: f32,
}

impl Renderer {
    /// This tick's glow, arcs and motes at `mouth`, the drive pushing `heat` (0 idle to 1
    /// flat out), the ship moving at `vel`.
    pub(super) fn plasma_drive_mouth(&mut self, mouth: &Mouth, heat: f32, vel: Vec3, time: f32) {
        let dt = self.tick_seconds.max(0.02);
        let r = mouth.radius;
        let at = |k: f32| mouth.at[0].lerp(mouth.at[1], k);
        let side = mouth.aft.any_orthonormal_vector();
        let up = mouth.aft.cross(side);

        // The glow filling the mouth, brighter with the thrust.
        let k = self.scatter.unit();
        self.push_warp(
            PUFF_GLOW,
            at(k) + mouth.aft * (r * 0.3),
            vel,
            time + k * dt,
            0.12,
            (r * 1.4, r * (1.9 + 0.9 * heat)),
            VIOLET * (1.2 + 2.4 * heat),
            0.0,
        );

        // Arcs from the lip, raked back down the plume, more of them the harder it pushes.
        let arcs = 1 + usize::from(self.scatter.unit() < 0.35 + 0.55 * heat);
        for _ in 0..arcs {
            let k = self.scatter.unit();
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let round = side * a.cos() + up * a.sin();
            let start = at(k) + round * r * (0.85 + 0.2 * self.scatter.unit());
            let reach = r * (1.5 + 2.5 * self.scatter.unit()) * (0.6 + 0.8 * heat);
            let dir = (mouth.aft * (0.8 + 0.6 * heat) + round * (0.35 - 0.5 * self.scatter.unit()))
                .normalize_or(mouth.aft);
            let color = if self.scatter.unit() < 0.3 {
                HOT
            } else {
                VIOLET
            };
            let life = 0.1 + 0.08 * self.scatter.unit();
            self.push_warp(
                PUFF_ARC,
                start,
                dir * reach + vel * dt,
                time + k * dt,
                life,
                (r * 0.45, r * 0.45),
                color * 5.0,
                0.0,
            );
        }

        // Motes of plasma shed into the wake, left behind as it goes.
        for _ in 0..1 + usize::from(heat > 0.5) {
            let k = self.scatter.unit();
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let round = side * a.cos() + up * a.sin();
            let start = at(k) + round * (r * 0.7 * self.scatter.unit());
            let shed = mouth.aft * (8.0 + 22.0 * heat) * (0.6 + 0.6 * self.scatter.unit())
                + round * (2.0 + 4.0 * self.scatter.unit());
            let color = VIOLET.lerp(HOT, self.scatter.unit() * 0.6) * 3.0;
            let life = 0.4 + 0.4 * self.scatter.unit();
            self.push_warp(
                PUFF_MOTE,
                start,
                vel * 0.6 + shed,
                time + k * dt,
                life,
                (r * 0.12, r * 0.04),
                color,
                0.0,
            );
        }
    }
}
