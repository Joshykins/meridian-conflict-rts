//! A core mine's pile driver striking its pipe. The vertex shader drops the
//! driver and drives the string down (`pipe_offset` in entity.wgsl) on the beat
//! the mirror publishes in `UnitInstance::gait` (`mc_sim::mines::hammer_gait`);
//! this puts what the blow throws up on the same beat. Every tier coughs dust
//! and sparks out of the pit. The deep core (tech 4) hits hard enough to flash
//! and send a small shockwave out across the ground.

use super::{Renderer, PUFF_DUST, PUFF_SPARK};
use crate::camera::Camera;
use glam::Vec3;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK, STATE_RADAR};

/// The model's authored radius (`core_mine` in models/aster/mod.rs); the lengths below
/// are at that scale and shrink with the blueprint's radius.
const AUTHORED_R: f32 = 40.5;
/// Height of the pit's opening over the mine's origin (`DECK` in models/aster/mine.rs).
const MOUTH_Z: f32 = 2.4;
/// Radius of the pit at its opening.
const MOUTH_R: f32 = 17.0;
/// How far the driver strikes over the opening (`STRING_TOP`).
const STRIKE_Z: f32 = 1.0;

impl Renderer {
    /// Called once a sim tick, like the footfalls: a blow lands this tick where the
    /// mine's count of blows passes a whole number, part of the way through it.
    pub(super) fn mine_blows(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        if camera.distance > 3000.0 {
            return;
        }
        let reach = camera.distance * 2.5 + 400.0;
        let focus = camera.focus.truncate();
        let previous = (self.effect_origin, self.effect_settings);
        for u in units {
            let [blows, step, _] = u.gait;
            if step <= 0.0 || u.owner_flags & (KIND_WRECK | KIND_GHOST | STATE_RADAR) != 0 {
                continue;
            }
            let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
                continue;
            };
            if bp.mine.is_none() || blows.floor() == (blows - step).floor() {
                continue;
            }
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            // Where in the coming tick's glide the shader shows the hammer land.
            let into = 1.0 - (blows.fract() / step).clamp(0.0, 1.0);
            let start = time + into * self.tick_seconds;
            self.effect_origin = Some(at);
            self.effect_settings = bp.visual.effects;
            let scale = bp.radius.to_f32() / AUTHORED_R;
            self.mine_blow(at, bp.tech, scale, start, camera.distance < 1400.0);
        }
        (self.effect_origin, self.effect_settings) = previous;
    }

    fn mine_blow(&mut self, at: Vec3, tech: u8, scale: f32, start: f32, close: bool) {
        let deep = tech >= 4;
        let mouth = at + Vec3::Z * (MOUTH_Z * scale);
        // Dust boiling up out of the pit and rolling over the lip.
        let puffs = if close { 5 + tech as usize } else { 3 };
        for _ in 0..puffs {
            let (x, y) = (self.scatter.signed(), self.scatter.signed());
            let off = Vec3::new(x, y, 0.0).normalize_or_zero()
                * (self.scatter.unit() * MOUTH_R * scale * 0.7);
            let rise = 5.0 + self.scatter.unit() * 5.0 + if deep { 4.0 } else { 0.0 };
            let life = 1.8 + self.scatter.unit() * 1.4;
            let late = self.scatter.unit() * 0.12;
            self.push_puff(
                PUFF_DUST,
                mouth + off + Vec3::Z * 1.0,
                off * 0.25 + Vec3::Z * rise,
                start + late,
                life,
                (
                    (3.0 + tech as f32) * scale,
                    (9.0 + tech as f32 * 2.5) * scale,
                ),
            );
        }
        if close {
            // Chips of hot rock thrown up past the rod.
            for _ in 0..4 + tech as usize * 2 {
                let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 5.0;
                let (up, life) = (
                    14.0 + self.scatter.unit() * 12.0,
                    0.7 + self.scatter.unit() * 0.5,
                );
                self.push_puff(
                    PUFF_SPARK,
                    mouth + out * 0.5 + Vec3::Z * STRIKE_Z * scale,
                    out + Vec3::Z * up,
                    start,
                    life,
                    (0.35, 0.1),
                );
            }
        }
        if deep {
            // A flash down the shaft that lights the rig, and the blow going out
            // through the ground: a low, wide pressure ring that lifts dust as it passes.
            self.push_effect(mouth.to_array(), start, 16.0, 0.3, 1.0, 0.0);
            self.push_shockwave(mouth.to_array(), start, 120.0, 1.1, 0.4, 1.0, Vec3::ZERO);
        }
    }
}
