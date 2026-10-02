//! The Plasmeric grade's firing as it is drawn (docs/STYLE.md "The Regency suite"), the
//! lowest rung of the Regency's plasma, for `regency_guns_fx`, and the mortar's strike (the
//! bolts' and flak's strikes are in `strike.rs`):
//!
//! - **A bolt or a flak bolt leaving** the mouth: a small red bloom for each.
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
    /// `round_gap` seconds apart, each a small red bloom.
    pub(super) fn bolt_fired(
        &mut self,
        at: Vec3,
        dir: Vec3,
        rounds: u8,
        flash: f32,
        round_gap: f32,
        time: f32,
    ) {
        for k in 0..rounds {
            let when = time + k as f32 * round_gap;
            let mouth = at + dir * 0.4;
            let s = 1.6 * flash;
            self.push_lit(
                BURST,
                mouth,
                Vec3::ZERO,
                when,
                0.11,
                (s * 0.4, s),
                RED * 3.5,
                0.0,
            );
            self.push_lit(
                GLOW,
                mouth,
                Vec3::ZERO,
                when,
                0.08,
                (s * 0.5, s * 0.7),
                HOT * 2.0,
                0.0,
            );
            self.plasma_fx.guns.light(Glow {
                pos: mouth,
                color: RED * 60.0 * flash,
                range: 12.0,
                start: when,
                life: 0.1,
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
            self.bore_fx.melt(at.truncate(), 0.45 * s, start, 4.0);
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
