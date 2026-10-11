//! A Regency airburst gun (`Weapon::airburst`, the Gorget's) as it is drawn: made to read
//! apart from the point defences beside it (the Picket's boiling bolts, the Halberd's red
//! jet and billowing burst). No charge, no red bloom, no jet: a hard white snap at the
//! mouth with the gas vented out sideways; a small pearl ticking on its fuse in flight,
//! shedding sparks; and in the sky a star: a pin-point flash, a shell of straight
//! white-hot needles thrown out every way, pops crackling out on that shell a moment later,
//! embers drifting down. Presentation only.

use super::{Glow, BURST, GLOW, HOT, MAX_TRAIL_SPARKS, MOTE, RED, STREAK, WHITE};
use crate::renderer::Renderer;
use glam::Vec3;

/// Needles in a burst's shell, laid evenly over the sphere.
const NEEDLES: usize = 22;
/// Pops crackling out on the shell after the burst.
const POPS: usize = 7;

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

    /// An airburst bead going off at `at` (`splash` its blast's reach): a star, not a
    /// bloom. A pin-point white flash, the pearl popping, a shell of straight needles
    /// thrown out evenly every way (white-hot, and slower red ones after them), pops
    /// crackling out on the shell a moment later, embers drifting down. Low over the
    /// ground it sears a spot and blows a little dirt out.
    pub(super) fn airburst_burst(
        &mut self,
        at: Vec3,
        impact: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let s = splash.max(6.0) * 0.7 * impact.sqrt();
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.07,
            (s * 0.12, s * 0.18),
            WHITE * 9.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.18,
            (s * 0.08, s * 0.3),
            HOT * 4.0,
            0.0,
        );
        // The shell of needles: evenly over the sphere (a golden-angle spiral), turned
        // and jittered a little so no two bursts line up.
        let turn = self.scatter.unit() * std::f32::consts::TAU;
        let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
        let mut dirs = [Vec3::ZERO; NEEDLES];
        for (k, d) in dirs.iter_mut().enumerate() {
            let z = 1.0 - 2.0 * (k as f32 + 0.5) / NEEDLES as f32;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let a = turn + golden * k as f32;
            let jitter = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.12;
            *d = (Vec3::new(a.cos() * r, a.sin() * r, z) + jitter).normalize_or(Vec3::Z);
        }
        for d in dirs {
            let reach = s * (0.85 + 0.25 * self.scatter.unit());
            let life = 0.16 + 0.06 * self.scatter.unit();
            self.push_lit(
                STREAK,
                at + d * s * 0.05,
                d * reach,
                start,
                life,
                (s * 0.014, s * 0.007),
                WHITE.lerp(HOT, 0.5) * 4.5,
                0.0,
            );
            self.push_lit(
                STREAK,
                at + d * s * 0.05,
                d * reach * 0.65,
                start + 0.03,
                0.4,
                (s * 0.01, s * 0.006),
                RED * 3.5,
                0.0,
            );
        }
        // Pops crackling out on the shell, each on a needle's end, one after another.
        for k in 0..POPS {
            let d = dirs[(k * 3 + (self.scatter.unit() * 3.0) as usize) % NEEDLES];
            let pos = at + d * s * (0.6 + 0.35 * self.scatter.unit());
            let when = start + 0.05 + 0.2 * (k as f32 + self.scatter.unit()) / POPS as f32;
            self.push_lit(
                GLOW,
                pos,
                Vec3::ZERO,
                when,
                0.05,
                (s * 0.05, s * 0.07),
                HOT * 7.0,
                0.0,
            );
            self.plasma_fx.guns.light(Glow {
                pos,
                color: HOT * 25.0 * impact,
                range: s * 0.5,
                start: when,
                life: 0.06,
                pulse: 0.0,
            });
        }
        // Embers off the shell, sinking as they cool.
        for _ in 0..10 {
            let d = dirs[(self.scatter.unit() * NEEDLES as f32) as usize % NEEDLES];
            let dot = 0.15 + 0.1 * self.scatter.unit();
            let when = start + 0.1 + 0.15 * self.scatter.unit();
            let tint = RED.lerp(HOT, 0.3 * self.scatter.unit());
            self.push_lit(
                MOTE,
                at + d * s * 0.7,
                d * 2.0 - Vec3::Z * 3.0,
                when,
                1.0,
                (dot, dot * 0.4),
                tint * 3.0,
                0.0,
            );
        }
        if ground {
            self.ground_melt
                .melt(at.truncate(), 0.15 * s, 0.15, start, 2.0);
            self.plasma_dirt(at, s * 0.3, 5, start);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: HOT * 160.0 * impact,
            range: s * 1.5,
            start,
            life: 0.12,
            pulse: 0.0,
        });
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * 60.0 * impact,
            range: s,
            start,
            life: 0.4,
            pulse: 0.0,
        });
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
