//! What a shell, bullet or bolt does where it lands (not on a shield, not flak):
//! the blink of its flash, the fire of a bursting shell (blast_fx.rs), and what
//! comes off the spot. The spot tells hit from miss at a glance: a hit on a hull
//! throws sparks and a lick of fire off the armour; a miss kicks a spurt of dirt
//! and dust out of the ground; a bursting shell on the ground leaves a column of
//! smoke and dust over its crater that hangs for seconds, so where the bombs fell
//! can still be read after the flash is gone.

use glam::Vec3;
use mc_data::WeaponColor;

use super::{
    rail_fx, Renderer, PUFF_BOLT, PUFF_CLOD, PUFF_DUST, PUFF_FIRE, PUFF_SHATTER_BLAST, PUFF_SMOKE,
    PUFF_SPARK,
};

/// Smallest splash, metres, whose burst on the ground leaves a standing column.
const COLUMN_SPLASH: f32 = 4.0;

/// One landed shot, as the renderer draws it.
pub(super) struct ShellImpact {
    pub(super) at: Vec3,
    pub(super) start: f32,
    pub(super) splash: f32,
    /// The weapon's `impact`: how big its hit is drawn.
    pub(super) impact: f32,
    /// Square root of the damage.
    pub(super) power: f32,
    pub(super) shockwave: f32,
    pub(super) bolts: u8,
    pub(super) color: WeaponColor,
    pub(super) red: f32,
    /// A hitscan shot or a rail slug: strikes white-hot.
    pub(super) white_hot: bool,
    /// The commander's blue hitscan rail cannon: the heavy blue bloom.
    pub(super) rail: bool,
    /// A fire bomb: its napalm wave is the hit.
    pub(super) incendiary: bool,
    pub(super) on_unit: bool,
}

impl ShellImpact {
    fn shell(&self) -> bool {
        self.color == WeaponColor::Orange
    }

    /// A bursting shell: its colour is its fireball, the flash only the blink before it.
    fn burning(&self) -> bool {
        self.shell() && self.splash > 0.0 && !self.incendiary && !self.rail
    }
}

impl Renderer {
    pub(super) fn shell_impact(&mut self, hit: &ShellImpact) {
        let (at, start, splash, impact) = (hit.at, hit.start, hit.splash, hit.impact);
        let shell = hit.shell();
        let tint = if shell && hit.red > 0.5 {
            8.0
        } else {
            hit.color as u32 as f32
        };
        // A rail slug strikes white-hot (`rail_fx`).
        let tint = if hit.white_hot && shell {
            rail_fx::RAIL_FLASH
        } else {
            tint
        };
        let burning = hit.burning();
        let core = if burning {
            (1.4 + splash * 0.14) * impact
        } else {
            (1.6 + hit.power * 0.28 + splash * 0.15) * impact
        };
        let snap = hit.bolts > 0 || (hit.shockwave > 0.0 && splash <= 0.0);
        let (life, ring) = if burning {
            (0.24, 1.0)
        } else if splash > 0.0 {
            ((0.38 + splash * 0.01) * impact.min(2.0), 1.0)
        } else if snap {
            (0.2, 1.0)
        } else {
            (0.3, 0.0)
        };
        // A modest ball of light above the crater — the wide blast sits on the ground.
        self.push_effect(
            at.to_array(),
            start,
            core,
            if hit.rail {
                0.55
            } else {
                (life * 0.7).max(if snap { 0.12 } else { 0.2 })
            },
            tint,
            // Above 1 only for the Bulwark: the flash shader reads the extra as a brighter, bluer core.
            if hit.rail { 1.8 } else { ring },
        );
        if hit.rail {
            self.rail_bloom(at, start, core, hit.color);
        }
        if burning {
            let draped = (4.0 + splash * 0.5) * impact.min(2.4);
            self.push_effect(at.to_array(), start, draped, 0.24, 5.0, tint);
        } else if splash > 0.0 {
            let draped = (8.0 + splash * 1.15) * impact.min(2.4);
            self.push_effect(
                at.to_array(),
                start,
                draped,
                (life * 0.7).max(0.22),
                5.0,
                tint,
            );
            if splash > 16.0 {
                // A second beat of fire, not another sheet of haze.
                self.push_effect(
                    (at + Vec3::Z * 2.2).to_array(),
                    start + 0.06,
                    core * 1.15,
                    0.28,
                    2.0,
                    0.55,
                );
            }
        }
        if hit.shockwave > 0.0 {
            self.push_shockwave(
                at.to_array(),
                start,
                if splash > 0.0 {
                    (18.0 + splash * 2.1) * hit.shockwave
                } else {
                    (16.0 + hit.power * 2.6) * hit.shockwave
                },
                if splash > 0.0 {
                    (0.7 + splash * 0.018).min(2.0)
                } else {
                    0.55
                },
                hit.shockwave.min(1.0),
                hit.color as u32 as f32,
                Vec3::ZERO,
            );
        }
        if !shell && splash <= 0.0 {
            for _ in 0..hit.bolts {
                let vel = self.scatter.upward(0.08) * (16.0 + self.scatter.unit() * 22.0);
                let life = 0.16 + self.scatter.unit() * 0.12;
                self.push_puff(
                    PUFF_BOLT,
                    at + Vec3::Z * 0.2,
                    vel,
                    start,
                    life,
                    (0.22, 0.05),
                );
            }
            return;
        }
        if hit.on_unit {
            self.hull_struck(hit);
        } else {
            self.ground_struck(hit);
        }
        // Napalm stays as the ground wave. A normal blast still throws a short flame.
        if hit.incendiary && splash > 0.0 {
            return;
        }
        self.impact_smoke(hit);
        if burning {
            self.shell_blast(at, splash, impact, start);
        } else if splash > 0.0 {
            let big = (splash * 0.12) as usize;
            let fires = 2 + big / 3;
            for i in 0..fires {
                let vel =
                    self.scatter.upward(0.35) * (3.0 + self.scatter.unit() * 4.0 + splash * 0.05);
                let life = 0.4 + self.scatter.unit() * 0.25 + splash * 0.004;
                self.push_puff(
                    PUFF_FIRE,
                    at + Vec3::Z * 0.55,
                    vel,
                    start + i as f32 * 0.015,
                    life,
                    (0.7 + splash * 0.04, 1.6 + splash * 0.06),
                );
            }
        }
        if !hit.on_unit && splash >= COLUMN_SPLASH {
            self.impact_column(hit);
        }
    }

    /// The blue hitscan rail's hit: a second, slower blue bloom, then lobes and
    /// bolts around the core.
    fn rail_bloom(&mut self, at: Vec3, start: f32, core: f32, color: WeaponColor) {
        self.push_effect(
            (at + Vec3::Z * 0.6).to_array(),
            start + 0.04,
            core * 0.62,
            0.85,
            color as u32 as f32,
            1.8,
        );
        for _ in 0..7 {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.8,
            )
            .normalize_or_zero();
            let speed = 10.0 + self.scatter.unit() * 14.0;
            let life = 0.42 + self.scatter.unit() * 0.18;
            self.push_puff(
                PUFF_SHATTER_BLAST,
                at + dir * core * 0.18,
                dir * speed,
                start,
                life,
                (core * 0.16, core * 0.42),
            );
        }
        for _ in 0..8 {
            let vel = self.scatter.upward(0.2) * (18.0 + self.scatter.unit() * 26.0);
            let life = 0.28 + self.scatter.unit() * 0.22;
            self.push_puff(
                PUFF_BOLT,
                at + Vec3::Z * 0.3,
                vel,
                start,
                life,
                (0.45, 0.08),
            );
        }
    }

    /// A hit on a hull: a spray of sparks off the armour and, for a plain
    /// round, a small lick of fire on the plating (a bursting shell has its
    /// fireball), so a hit reads as a hit, not as a puff of dirt.
    fn hull_struck(&mut self, hit: &ShellImpact) {
        let (at, start, splash) = (hit.at, hit.start, hit.splash);
        let big = (splash * 0.12) as usize;
        let reach = 1.0 + splash * 0.12;
        for _ in 0..14 + big {
            let vel = self.scatter.upward(0.15) * (8.0 + self.scatter.unit() * 14.0) * reach;
            let life = 0.35 + self.scatter.unit() * 0.5 + splash * 0.01;
            self.push_puff(
                PUFF_SPARK,
                at + Vec3::Z * 0.2,
                vel,
                start,
                life,
                (0.22 + splash * 0.004, 0.05),
            );
        }
        self.clods(hit, 2 + big / 2, reach);
        if hit.shell() && splash <= 0.0 {
            let r = (0.45 + hit.power * 0.09) * hit.impact.clamp(0.6, 2.0);
            let burn = (0.16 + hit.power * 0.02).min(0.4);
            let lumps = if hit.power < 3.0 { 1 } else { 2 };
            self.fireball(at + Vec3::Z * 0.3, r, lumps, burn, 0.9, start);
        }
    }

    /// A miss into the ground: a few sparks, clods and a spurt of dust thrown
    /// up out of the dirt, brown, not fire.
    fn ground_struck(&mut self, hit: &ShellImpact) {
        let (at, start, splash) = (hit.at, hit.start, hit.splash);
        let big = (splash * 0.12) as usize;
        let reach = 1.0 + splash * 0.12;
        for _ in 0..3 + big {
            let vel = self.scatter.upward(0.15) * (8.0 + self.scatter.unit() * 14.0) * reach;
            let life = 0.25 + self.scatter.unit() * 0.35 + splash * 0.008;
            self.push_puff(
                PUFF_SPARK,
                at + Vec3::Z * 0.2,
                vel,
                start,
                life,
                (0.2 + splash * 0.004, 0.05),
            );
        }
        self.clods(hit, 8 + big, reach);
        // The spurt: thrown up steeply, then it hangs and settles.
        let spurts = (2 + big / 2).min(8);
        for _ in 0..spurts {
            let vel = (self.scatter.upward(0.75) + Vec3::Z * 0.6)
                * (3.0 + self.scatter.unit() * 4.0)
                * reach.sqrt();
            let life = 1.0 + self.scatter.unit() * 0.6 + splash * 0.03;
            self.push_puff(
                PUFF_DUST,
                at + Vec3::Z * 0.3,
                vel,
                start + 0.02,
                life,
                (
                    0.45 + hit.power * 0.06 + splash * 0.05,
                    1.4 + hit.power * 0.14 + splash * 0.12,
                ),
            );
        }
    }

    fn clods(&mut self, hit: &ShellImpact, n: usize, reach: f32) {
        let splash = hit.splash;
        for _ in 0..n {
            let vel = self.scatter.upward(0.45) * (6.0 + self.scatter.unit() * 9.0) * reach;
            let life = 0.9 + self.scatter.unit() * 0.7 + splash * 0.012;
            self.push_puff(
                PUFF_CLOD,
                hit.at + Vec3::Z * 0.2,
                vel,
                hit.start,
                life,
                (0.12 + hit.power * 0.025 + splash * 0.006, 0.08),
            );
        }
    }

    /// The smoke that hangs after the hit: soot off a hull, soot and dust off the ground.
    fn impact_smoke(&mut self, hit: &ShellImpact) {
        let splash = hit.splash;
        let big = (splash * 0.12) as usize;
        let smokes = 2 + (splash > 0.0) as usize * 2 + big / 3;
        for i in 0..smokes {
            let vel = self.scatter.upward(0.5) * (1.5 + self.scatter.unit() * 2.5 + splash * 0.12);
            let kind = if hit.on_unit || i % 2 == 0 {
                PUFF_SMOKE
            } else {
                PUFF_DUST
            };
            let life = 0.9 + self.scatter.unit() * 0.6 + splash * 0.02;
            self.push_puff(
                kind,
                hit.at + Vec3::Z * 0.4,
                vel,
                hit.start + 0.03,
                life,
                (
                    0.55 + hit.power * 0.05 + splash * 0.03,
                    1.2 + hit.power * 0.1 + splash * 0.08,
                ),
            );
        }
    }

    /// A bursting shell on the ground: a column of dust and smoke that stands
    /// over the crater for seconds after the fire is out.
    fn impact_column(&mut self, hit: &ShellImpact) {
        let (at, splash) = (hit.at, hit.splash);
        let ground = self.ground_height(at.truncate());
        // An air burst, or a burst on the sea (water_fx.rs draws that), leaves no column.
        if at.z - ground > 1.0 + splash * 0.25 || self.under_sea(at.truncate().extend(ground + 0.1))
        {
            return;
        }
        // Cosmetic: a few puffs make the column; a bigger shell makes it wider and
        // longer-lived, not denser past seven.
        let n = (3 + (splash / 10.0) as usize).min(7);
        for i in 0..n {
            let up = i as f32 / n as f32;
            let kind = if i % 2 == 0 { PUFF_DUST } else { PUFF_SMOKE };
            let vel = Vec3::new(
                self.scatter.signed() * 0.6,
                self.scatter.signed() * 0.6,
                1.5 + up * 2.5 + splash * 0.04,
            );
            let life = (3.0 + splash * 0.06 + self.scatter.unit() * 1.2).min(7.5);
            self.push_puff(
                kind,
                at + Vec3::Z * (0.6 + up * splash * 0.15),
                vel,
                hit.start + 0.12 + i as f32 * 0.1,
                life,
                (0.9 + splash * 0.14, 2.4 + splash * 0.4),
            );
        }
    }
}
