//! Explosions that burn (puffs.wgsl `blast_fireball`). A unit or a shell going
//! up is a short white flash, then a lumpy ball of burning gas that turns to
//! soot from the rim in and climbs off as smoke, not a disc of orange light.

use glam::Vec3;

use super::{Renderer, PUFF_CLOD, PUFF_DUST, PUFF_SMOKE, PUFF_SPARK};
use crate::gpu_consts::puff;

pub(super) const PUFF_BLAST: f32 = puff::BLAST as f32;

/// How much of a blast puff's life it burns: puffs.wgsl `BLAST_BURN`.
const BLAST_BURN: f32 = 0.34;

impl Renderer {
    /// A ball of burning gas about `r` metres in radius at its biggest, around
    /// `at`: `lumps` puffs thrown a little apart, so the outline is lumpy and
    /// not one round disc. It burns for about `burn` seconds and the smoke it
    /// leaves lives about twice as long again. `heat` 1 starts white-yellow at
    /// the heart; less starts orange, for fire that is not a detonation.
    pub(super) fn fireball(
        &mut self,
        at: Vec3,
        r: f32,
        lumps: u32,
        burn: f32,
        heat: f32,
        start: f32,
    ) {
        // The fire lights the ground and the hulls round it while it burns.
        self.lights
            .effect(at.to_array(), start, r * 0.8 * heat, burn * 0.7, 2.0);
        let life = burn / BLAST_BURN;
        for i in 0..lumps {
            let dir = self.scatter.upward(-0.25);
            // The first lump is the heart of it, on the blast; the rest bulge out of it.
            let (out, size) = if i == 0 {
                (0.0, 1.0)
            } else {
                (
                    0.35 + self.scatter.unit() * 0.4,
                    0.6 + self.scatter.unit() * 0.3,
                )
            };
            // The shader's ball fills about 0.62 of its card.
            let grown = r * size / 0.62;
            let lasts = life * (0.85 + self.scatter.unit() * 0.35);
            self.push_puff_with_motion(
                PUFF_BLAST,
                at + dir * r * out * 0.3,
                dir * r * out * 3.4,
                start + i as f32 * 0.012,
                lasts,
                (grown * 0.3, grown),
                Vec3::X * heat,
            );
        }
    }

    /// A land unit or structure going up: the detonation, secondary blasts
    /// walking across the hull, burning fragments thrown wide, a ring of dust
    /// along the ground, and fire that turns into a column of black smoke over
    /// the wreck. `r` and `h` are the hull's radius and height.
    pub(super) fn unit_blast(&mut self, at: Vec3, r: f32, h: f32, time: f32) {
        let core = at + Vec3::Z * h * 0.45;
        // A blink of white and a lick of light on the ground; the fireball
        // itself carries the colour after that.
        self.push_effect(core.to_array(), time, r * 2.4, 0.14, 2.0, 0.0);
        self.push_effect(at.to_array(), time, r * 2.2, 0.18, 5.0, 1.0);
        self.push_shockwave(
            at.to_array(),
            time,
            22.0 + r * 7.0,
            0.7,
            0.85,
            1.0,
            Vec3::ZERO,
        );
        // A little under full heat: burning twice as long, full heat held the
        // white-yellow heart too long (white blow-out).
        self.fireball(core, r * 1.8, 10, 1.05 + r * 0.04, 0.85, time + 0.02);
        for i in 0..4 {
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.6,
            ) * r
                * 0.85;
            let delay = 0.1 + 0.11 * i as f32 + self.scatter.unit() * 0.06;
            let size = r * (0.6 + self.scatter.unit() * 0.35);
            self.fireball(core + off, size, 3, 0.6, 0.75, time + delay);
        }
        for _ in 0..40 {
            let vel =
                self.scatter.upward(0.1) * (12.0 + self.scatter.unit() * 28.0) * (0.85 + r * 0.07);
            let life = 0.8 + self.scatter.unit() * 1.4;
            self.push_puff(PUFF_SPARK, core, vel, time, life, (0.22 + r * 0.04, 0.05));
        }
        for _ in 0..16 {
            let vel = self.scatter.upward(0.32) * (8.0 + self.scatter.unit() * 14.0);
            let life = 1.0 + self.scatter.unit() * 0.9;
            self.push_puff(PUFF_CLOD, core, vel, time, life, (0.22 + r * 0.05, 0.14));
        }
        // The dust ring: pushed out flat from the foot of the blast.
        for i in 0..18 {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 18.0;
            let out = Vec3::new(a.cos(), a.sin(), 0.06);
            let life = 2.2 + self.scatter.unit() * 1.2;
            self.push_puff(
                PUFF_DUST,
                at + out * r * 0.7 + Vec3::Z * 0.4,
                out * (12.0 + r * 1.6),
                time + 0.03,
                life,
                (r * 0.4, r * 1.25),
            );
        }
        // Fire licking out of the wreck once the fireball has lifted off it:
        // small burning puffs that climb and go to smoke, not glowing orbs.
        for i in 0..8 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.5
                + Vec3::Z * h * 0.4;
            let start = time + 0.6 + i as f32 * 0.24 + self.scatter.unit() * 0.1;
            let size = r * (0.3 + self.scatter.unit() * 0.2);
            self.fireball(at + off, size, 2, 0.55, 0.45, start);
        }
        for i in 0..16 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.45
                + Vec3::Z * h * 0.5;
            let vel = Vec3::new(
                self.scatter.signed() * 0.9 + 0.9,
                self.scatter.signed() * 0.9,
                3.4 + self.scatter.unit() * 2.2,
            );
            let start = time + 0.5 + i as f32 * 0.28;
            let life = 3.4 + self.scatter.unit() * 1.6;
            self.push_puff(PUFF_SMOKE, at + off, vel, start, life, (r * 0.5, r * 2.1));
        }
    }

    /// An aircraft blown apart in the air. Lingering fire and smoke follow the
    /// falling hull, not the death point.
    pub(super) fn air_blast(&mut self, at: Vec3, r: f32, time: f32) {
        self.push_effect(at.to_array(), time, r * 2.6, 0.14, 1.0, 0.0);
        self.push_shockwave(at.to_array(), time, r * 5.0, 0.5, 0.65, 0.0, Vec3::ZERO);
        self.fireball(at, r * 1.35, 8, 0.95, 0.9, time + 0.01);
        for _ in 0..24 {
            let vel = self.scatter.upward(0.1) * (12.0 + r * 2.0);
            self.push_puff(PUFF_SPARK, at, vel, time, 1.2, (0.35, 0.05));
        }
        for _ in 0..8 {
            let vel = self.scatter.upward(0.2) * 15.0;
            self.push_puff(PUFF_CLOD, at, vel, time, 1.3, (0.4, 0.2));
        }
    }

    /// The fire of a shell that bursts with `splash` metres of blast: a ball
    /// of burning gas over the crater, bigger and longer for a heavier shell.
    /// It burns long enough to see it boil (0.6 s for a light shell, over a
    /// second for a heavy one), then climbs off as soot.
    pub(super) fn shell_blast(&mut self, at: Vec3, splash: f32, impact: f32, start: f32) {
        let r = (1.5 + splash * 0.25) * impact.clamp(0.6, 2.0);
        let lumps = (4 + (splash * 0.1) as u32).min(9);
        let burn = (0.55 + splash * 0.02).min(1.4);
        self.fireball(at + Vec3::Z * r * 0.4, r, lumps, burn, 0.9, start);
    }
}
