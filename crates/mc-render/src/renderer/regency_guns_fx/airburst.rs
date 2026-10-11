//! A Regency airburst gun (`Weapon::airburst`, the Gorget's) as it is drawn: made to read
//! apart from the point defences beside it (the Picket's boiling bolts, the Halberd's red
//! jet and billowing burst). No charge, no red bloom, no jet: a hard white snap at the
//! mouth with the gas vented out sideways; a small pearl ticking on its fuse in flight,
//! shedding sparks; and in the sky a hard ball: a white flash, a tight hot core, a ragged
//! shell of red lumps popping round it, shrapnel lines flung out, sparks falling.
//! Presentation only.

use super::{Glow, BURST, GLOW, HOT, MAX_TRAIL_SPARKS, MOTE, RED, STREAK, WHITE};
use crate::renderer::Renderer;
use glam::Vec3;

/// Lumps in a burst's ragged shell.
const SHELL: usize = 12;
/// Shrapnel lines a burst flings out.
const SHRAPNEL: usize = 5;

impl Renderer {
    /// An airburst bead leaving the bore at `at` along `dir`: a hard white snap, the gas
    /// vented out either side of the fork, a few sparks spat forward.
    pub(super) fn airburst_fired(&mut self, at: Vec3, dir: Vec3, flash: f32, time: f32) {
        let s = 2.2 * flash;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.06,
            (s * 0.5, s * 0.8),
            WHITE * 6.0,
            0.0,
        );
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        for k in 0..4 {
            let out = if k % 2 == 0 { side } else { -side };
            let vent = (out * 0.9 + dir * (0.2 + 0.2 * self.scatter.unit())).normalize_or(out);
            let reach = s * (1.3 + 0.6 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                vent * reach,
                time,
                0.09,
                (s * 0.06, s * 0.03),
                HOT * 5.0,
                0.0,
            );
        }
        for _ in 0..5 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.2;
            let vel =
                (dir + spread).normalize_or(dir) * s * 14.0 * (0.6 + 0.6 * self.scatter.unit());
            self.push_lit(MOTE, at, vel, time, 0.25, (0.12, 0.05), HOT * 4.0, 0.0);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: HOT * 120.0 * flash,
            range: s * 6.0,
            start: time,
            life: 0.08,
            pulse: 0.0,
        });
    }

    /// The stretch `from` to `to` an airburst bead flew this tick: the sparks it sheds, left
    /// where they were shed and sinking, and its light. `sparks` counts against the trail
    /// sparks' cap a tick (`MAX_TRAIL_SPARKS`, a deliberate cosmetic cap).
    pub(super) fn airburst_flight(
        &mut self,
        from: Vec3,
        to: Vec3,
        size: f32,
        time: f32,
        sparks: &mut usize,
    ) {
        let tick = self.tick_seconds.max(0.02);
        for _ in 0..2 {
            if *sparks >= MAX_TRAIL_SPARKS {
                break;
            }
            *sparks += 1;
            let f = self.scatter.unit();
            let drift = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            );
            let dot = 0.1 + 0.08 * size.min(2.0);
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                from.lerp(to, f),
                drift * 2.0 - Vec3::Z * 4.0,
                time + tick * f,
                0.35 + 0.2 * roll,
                (dot, dot * 0.4),
                HOT.lerp(RED, roll) * 4.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: to,
            color: HOT * 30.0 * size,
            range: 7.0,
            start: time,
            life: tick * 0.6,
            pulse: 0.0,
        });
    }

    /// An airburst bead going off at `at` (`splash` its blast's reach): a hard ball, not a
    /// bloom or a star. A white flash, a tight hot core, a ragged shell of red lumps popping
    /// round it a moment later, shrapnel lines flung out every way, sparks falling. Short
    /// and hard-edged: gone in half a second. Low over the ground it sears a spot and blows
    /// a little dirt out.
    pub(super) fn airburst_burst(
        &mut self,
        at: Vec3,
        impact: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let s = splash.max(6.0) * 0.5 * impact.sqrt();
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.05,
            (s * 0.2, s * 0.28),
            HOT * 5.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.3,
            (s * 0.2, s * 0.6),
            RED.lerp(HOT, 0.4) * 3.0,
            0.0,
        );
        // The shell: lumps popping out round the core, so the ball's edge is ragged.
        for _ in 0..SHELL {
            let out = self.scatter_dir();
            let r = s * (0.3 + 0.12 * self.scatter.unit());
            let when = start + 0.015 + 0.05 * self.scatter.unit();
            let lump = s * (0.22 + 0.08 * self.scatter.unit());
            let tint = RED.lerp(HOT, 0.35 * self.scatter.unit());
            self.push_lit(
                BURST,
                at + out * r,
                Vec3::ZERO,
                when,
                0.3,
                (lump * 0.4, lump),
                tint * 2.6,
                0.0,
            );
        }
        // Shrapnel: thin hot lines flung out every way, fast and short-lived.
        for _ in 0..SHRAPNEL {
            let out = self.scatter_dir();
            let reach = s * (0.5 + 0.3 * self.scatter.unit());
            let life = 0.07 + 0.03 * self.scatter.unit();
            self.push_lit(
                STREAK,
                at + out * s * 0.1,
                out * reach,
                start,
                life,
                (s * 0.012, s * 0.006),
                HOT * 2.5,
                0.0,
            );
        }
        // Sparks thrown out and falling as they cool.
        for _ in 0..12 {
            let out = self.scatter_dir();
            let dot = 0.14 + 0.08 * self.scatter.unit();
            let when = start + 0.05 * self.scatter.unit();
            let tint = HOT.lerp(RED, self.scatter.unit());
            self.push_lit(
                MOTE,
                at + out * s * 0.25,
                out * s * 2.0 - Vec3::Z * s * 0.8,
                when,
                0.7,
                (dot, dot * 0.4),
                tint * 3.5,
                0.0,
            );
        }
        if ground {
            self.ground_melt
                .melt(at.truncate(), 0.2 * s, 0.15, start, 2.0);
            self.plasma_dirt(at, s * 0.4, 5, start);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: HOT * 160.0 * impact,
            range: s * 2.5,
            start,
            life: 0.1,
            pulse: 0.0,
        });
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * 60.0 * impact,
            range: s * 1.8,
            start,
            life: 0.35,
            pulse: 0.0,
        });
    }

    /// A direction picked evenly over the sphere.
    fn scatter_dir(&mut self) -> Vec3 {
        let z = self.scatter.signed();
        let a = self.scatter.unit() * std::f32::consts::TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(a.cos() * r, a.sin() * r, z)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_airburst_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            crate::gpu_consts::plasma_look::AIRBURST,
            mc_sim::mirror::PLASMA_LOOK_AIRBURST
        );
    }
}
