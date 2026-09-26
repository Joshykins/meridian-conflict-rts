//! An ARC rail gun leaving the muzzle (docs/STYLE.md "ARC fires no plasma").
//!
//! A slug driven out on a current: the rails' last contact arcs white-hot at the
//! bore, the air in front of it is thrown out as a white blast, and the rail
//! spatter and vapour blown out after it hang a moment. White, not the orange of a
//! powder gun. The slug is a real, very fast shot (`Weapon::rail`): its path is
//! `rail_wakes`; the smoke and sparks every gun leaves are added by the `ShotFired` handler.

use super::water_fx::PUFF_STEAM;
use super::{FadeBeam, Renderer, PUFF_SPARK};
use crate::camera::Camera;
use glam::Vec3;
use mc_sim::mirror::{
    ProjectileInstance, PROJECTILE_BEAM, PROJECTILE_ENDS_SHIFT, PROJECTILE_FADE_BEAM, PROJECTILE_RAIL,
    PROJECTILE_STARTS_SHIFT,
};

/// Effect kind of a rail gun's flash: white-hot (sprites.wgsl `weapon_color`, lights.rs).
pub(super) const RAIL_FLASH: f32 = 9.0;

impl Renderer {
    /// `size` is the gun's flash radius as every gun works it out.
    pub(super) fn rail_muzzle(&mut self, at: Vec3, dir: Vec3, size: f32, time: f32) {
        // The Paladin fires from inside its own shield: the blast goes out through it.
        let outbound = std::mem::replace(&mut self.effect_outbound, true);
        self.rail_muzzle_inner(at, dir, size, time);
        self.effect_outbound = outbound;
    }

    fn rail_muzzle_inner(&mut self, at: Vec3, dir: Vec3, size: f32, time: f32) {
        // The arc at the bore, held long enough to be seen, then a hot point just ahead.
        self.push_effect(at.to_array(), time, size * 0.55, 0.16, RAIL_FLASH, 0.55);
        self.push_effect((at + dir * size * 0.08).to_array(), time, size * 0.3, 0.1, RAIL_FLASH, 0.0);
        let right = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = right.cross(dir).normalize_or(Vec3::Z);
        let puff = (size * 0.05).clamp(0.3, 2.2);
        // The blast out ahead of the slug: a jet of white vapour along the bore.
        for i in 0..5 {
            let speed = size * (0.6 + 0.45 * i as f32);
            let jitter = (right * self.scatter.signed() + up * self.scatter.signed()) * size * 0.1;
            let life = 0.55 + 0.12 * i as f32 + self.scatter.unit() * 0.2;
            self.push_puff(
                PUFF_STEAM,
                at + dir * (0.4 + i as f32 * 0.3),
                dir * speed + jitter,
                time + 0.008 * i as f32,
                life,
                (puff * 0.7, puff * (2.2 + 0.4 * i as f32)),
            );
        }
        // And a ring of it thrown out sideways round the muzzle.
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + self.scatter.signed() * 0.3;
            let out = right * a.cos() + up * a.sin();
            let speed = size * (0.55 + 0.3 * self.scatter.unit());
            let life = 0.45 + self.scatter.unit() * 0.25;
            self.push_puff(
                PUFF_STEAM,
                at + dir * 0.3,
                (out * 0.9 + dir * 0.35) * speed,
                time,
                life,
                (puff * 0.5, puff * 1.7),
            );
        }
        // Rail spatter: metal torn off the rails, thrown out fast in a tight cone.
        for _ in 0..10 {
            let spray = (dir * 2.2
                + Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()) * 0.45)
                .normalize_or_zero();
            let speed = 40.0 + self.scatter.unit() * 50.0;
            let life = 0.14 + self.scatter.unit() * 0.16;
            self.push_puff(PUFF_SPARK, at + dir * 0.3, spray * speed, time, life, (0.2, 0.05));
        }
    }
}

impl Renderer {
    /// The stretch each rail slug (`PROJECTILE_RAIL`) flies this tick: the air it tore
    /// glows white-hot just behind it and cools, and a thin trail of white vapour is
    /// left hanging to drift off on the wind. Laid piece by piece as the slug passes, so
    /// the streak is seen to travel; the slug itself is the projectile's own trace.
    pub(super) fn rail_wakes(&mut self, projectiles: &[ProjectileInstance], time: f32, camera: &Camera) {
        if camera.distance > 2200.0 {
            return;
        }
        let reach = camera.distance * 2.8 + 360.0;
        let focus = camera.focus.truncate();
        let wind = self.sky.wind_heading();
        for p in projectiles {
            if p.color & (PROJECTILE_RAIL | PROJECTILE_FADE_BEAM | PROJECTILE_BEAM) != PROJECTILE_RAIL {
                continue;
            }
            // A capital rail's slug is drawn by `heavy_rail_fx`.
            if self.heavy_rail_owns(p) {
                continue;
            }
            let (prev, pos) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
            if pos.truncate().distance(focus) > reach && prev.truncate().distance(focus) > reach {
                continue;
            }
            // As sprites.wgsl flies it: over part of the tick on its last stretch, and
            // from part of the way along for a shot that leaves during the tick.
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let span = if ends > 0.0 { ends } else { 1.0 };
            let s0 = (((p.color >> PROJECTILE_STARTS_SHIFT) & 0xFF) as f32 / 255.0 / span).min(1.0);
            let from = prev.lerp(pos, s0);
            let length = from.distance(pos);
            if length < 0.5 {
                continue;
            }
            let tick = self.tick_seconds;
            let passed = |t: f32| time + (s0 + (1.0 - s0) * t) * span * tick;
            // Thin: a slug's path, not a beam weapon.
            let line = (p.size * 0.3).clamp(0.2, 0.7);
            let pieces = ((length / 24.0).ceil() as usize).clamp(1, 10);
            for k in 0..pieces {
                let (t0, t1) = (k as f32 / pieces as f32, (k + 1) as f32 / pieces as f32);
                let (a, b) = (from.lerp(pos, t0), from.lerp(pos, t1));
                let start = passed(t1);
                self.fade_beams.push(FadeBeam { from: a, to: b, start, life: 0.12, width: line * 1.4, laser: false, rail: true });
                self.fade_beams.push(FadeBeam { from: a, to: b, start, life: 0.5, width: line * 0.6, laser: false, rail: true });
            }
            let vapour = 0.45 + p.size * 0.4;
            let n = ((length / 7.0) as usize).clamp(2, 40);
            for k in 1..=n {
                let t = k as f32 / n as f32;
                let drift = (wind * (1.0 + self.scatter.unit())).extend(0.3);
                let life = 1.4 + self.scatter.unit() * 1.2;
                let grow = vapour * (1.6 + self.scatter.unit());
                self.push_puff(PUFF_STEAM, from.lerp(pos, t), drift, passed(t), life, (vapour * 0.5, grow));
            }
        }
    }
}
