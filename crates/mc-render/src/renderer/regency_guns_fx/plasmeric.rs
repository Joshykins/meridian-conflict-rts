//! The Plasmeric grade's firing as it is drawn (docs/STYLE.md "The Regency suite"), the
//! lowest rung of the Regency's plasma, for `regency_guns_fx`, and the mortar's strike (the
//! bolts' and flak's strikes are in `strike.rs`):
//!
//! - **A bolt or a flak bolt leaving** the mouth: spat out hard, a white-hot snap, a red
//!   bloom thrown forward, droplets of plasma and sparks flung after it, a flash of red light.
//! - **A Plasmeric Mortar** (a lobbed Plasmeric shot, `Weapon::trajectory` Ballistic):
//!   it leaves the bore with a heavy red bloom and a few globs of plasma thrown up out
//!   of the mouth after it; it flies as the bolt's fat teardrop on its high arc
//!   (sprites.wgsl `plasma_look` 4), and where it comes down it bursts in a hard red heart
//!   over a white one as wide as its splash, a short spout of plasma thrown up, globs
//!   and spatter thrown out low (no shock ring, no dust) and the ground seared into a
//!   glowing pool that crusts over. Hard-edged and short-lived, no mist.

use glam::Vec3;

use super::super::Renderer;
use super::{Glow, BURST, GLOB, GLOW, HOT, MOTE, RED, WHITE};

impl Renderer {
    /// Plasmeric bolts leaving the mouth at `at` down `dir`: `rounds` of them
    /// `round_gap` seconds apart. Each is spat out hard: a white-hot snap at the mouth,
    /// a red bloom thrown forward off it, a few droplets of plasma flung out after the
    /// bolt and sparks, the ground and hull lit red for an instant. All of it gone in a
    /// fifth of a second, so a moving gun leaves nothing hanging behind it.
    pub(super) fn bolt_fired(
        &mut self,
        at: Vec3,
        dir: Vec3,
        rounds: u8,
        flash: f32,
        round_gap: f32,
        time: f32,
    ) {
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        for k in 0..rounds {
            let when = time + k as f32 * round_gap;
            let mouth = at + dir * 0.3;
            let s = 1.8 * flash;
            // The snap: the field letting go, white-hot and gone in a blink.
            self.push_lit(
                GLOW,
                mouth,
                Vec3::ZERO,
                when,
                0.06,
                (s * 0.3, s * 0.55),
                HOT * 3.2,
                0.0,
            );
            // The bloom thrown forward off the mouth with the bolt.
            self.push_lit(
                BURST,
                mouth + dir * s * 0.45,
                dir * s * 6.0,
                when,
                0.13,
                (s * 0.35, s * 1.25),
                RED * 3.0,
                0.0,
            );
            // Droplets of plasma flung out down the line of fire after it.
            for _ in 0..3 {
                let spread = side * self.scatter.signed() + up * self.scatter.signed();
                let out = (dir + spread * 0.3).normalize_or(dir);
                let blob = s * (0.07 + 0.05 * self.scatter.unit());
                let speed = s * (12.0 + 10.0 * self.scatter.unit());
                self.push_lit(
                    GLOB,
                    mouth,
                    out * speed,
                    when,
                    0.16,
                    (blob, blob * 0.4),
                    RED.lerp(HOT, 0.35) * 4.0,
                    0.0,
                );
            }
            for _ in 0..3 {
                let spread = side * self.scatter.signed() + up * self.scatter.signed();
                let out = (dir + spread * 0.6).normalize_or(dir);
                let dot = 0.12 + 0.08 * self.scatter.unit();
                let speed = s * (10.0 + 12.0 * self.scatter.unit());
                self.push_lit(
                    MOTE,
                    mouth,
                    out * speed,
                    when,
                    0.2,
                    (dot, dot * 0.4),
                    HOT * 4.0,
                    0.0,
                );
            }
            self.plasma_fx.guns.light(Glow {
                pos: mouth,
                color: RED.lerp(HOT, 0.2) * 95.0 * flash,
                range: 16.0 * flash.max(0.6),
                start: when,
                life: 0.12,
                pulse: 0.0,
            });
        }
    }

    /// A Plasmeric Mortar firing from the mouth at `at` up `dir`: a heavy red bloom over a
    /// hot heart, and a few globs of plasma thrown up out of the bore after the shot.
    pub(super) fn mortar_fired(&mut self, at: Vec3, dir: Vec3, flash: f32, time: f32) {
        let s = 2.4 * flash;
        let mouth = at + dir * 0.5;
        self.push_lit(
            BURST,
            mouth,
            dir * 3.0,
            time,
            0.22,
            (s * 0.4, s),
            RED * 3.5,
            0.0,
        );
        self.push_lit(
            GLOW,
            mouth,
            Vec3::ZERO,
            time,
            0.12,
            (s * 0.5, s * 0.8),
            HOT * 2.5,
            0.0,
        );
        for _ in 0..5 {
            let out = (dir
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.35)
                .normalize_or(dir);
            let blob = s * (0.08 + 0.05 * self.scatter.unit());
            let speed = 6.0 + 6.0 * self.scatter.unit();
            let when = time + self.scatter.unit() * 0.05;
            self.push_lit(
                GLOB,
                mouth,
                out * speed,
                when,
                0.4,
                (blob, blob * 0.35),
                RED.lerp(HOT, 0.3) * 3.5,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: mouth,
            color: RED * 90.0 * flash,
            range: 14.0,
            start: time,
            life: 0.2,
            pulse: 0.0,
        });
    }

    /// A Plasmeric Mortar shot coming down at `at`, `splash` metres its blast: a hard red
    /// heart over a white one, a short spout of plasma, globs and spatter thrown out low in
    /// place of a shock ring, and on the ground a seared pool.
    pub(super) fn mortar_burst(
        &mut self,
        at: Vec3,
        impact: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let s = splash.max(3.0) * impact;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.12,
            (s * 0.35, s * 0.55),
            WHITE * 2.2,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.38,
            (s * 0.25, s * 0.9),
            RED * 3.0,
            0.0,
        );
        // A short spout thrown up out of it: globs, not puffs, so it does not hang as mist.
        for k in 0..4 {
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.3;
            let blob = s * (0.07 + 0.04 * self.scatter.unit());
            self.push_lit(
                GLOB,
                at,
                (Vec3::Z + lean) * s * (1.6 + 0.6 * k as f32),
                start + k as f32 * 0.025,
                0.55,
                (blob, blob * 0.35),
                RED.lerp(HOT, 0.45 - k as f32 * 0.08) * 3.5,
                0.0,
            );
        }
        // Globs thrown out low as the shot lets go.
        for _ in 0..8 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.15 + self.scatter.unit() * 0.5,
            )
            .normalize_or(Vec3::Z);
            let blob = s * (0.06 + 0.05 * self.scatter.unit());
            let roll0 = self.scatter.unit();
            self.push_lit(
                GLOB,
                at,
                out * s * (2.0 + 2.5 * roll0),
                start,
                0.7,
                (blob, blob * 0.35),
                RED.lerp(HOT, roll0 * 0.4) * 3.5,
                0.0,
            );
        }
        // Spatter: small hot sparks thrown wide.
        for _ in 0..12 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + self.scatter.unit() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let dot = 0.22 + 0.12 * self.scatter.unit();
            let speed = 12.0 + 18.0 * self.scatter.unit();
            let life = 0.5 + 0.4 * self.scatter.unit();
            self.push_lit(
                MOTE,
                at + Vec3::Z * 0.3,
                out * speed,
                start,
                life,
                (dot, dot * 0.35),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        if ground {
            self.ground_melt.melt(at.truncate(), 0.45 * s, start, 4.0);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z,
            color: RED * 160.0 * impact,
            range: s * 3.0,
            start,
            life: 0.4,
            pulse: 0.0,
        });
    }
}
