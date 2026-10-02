//! Where a Regency plasma shot strikes (`regency_landed`): each grade's burst and the jet
//! pouring in after a Pinched shot's head (a Pinch-fusion strike is `fusion.rs`'s). See
//! the parent module for how each one looks.

use super::{
    ball, grade, Glow, Grade, ARC, BURST, GLOB, GLOW, HOT, MOTE, RED, STREAK, WAKE, WHITE,
};
use crate::renderer::{Renderer, PUFF_TREE_SMOKE};
use glam::Vec3;
use mc_data::BlueprintId;

impl Renderer {
    /// A Regency plasma shot struck (`Impact`, not on a shield). True when this drew the
    /// strike (and a shell's blast should not be).
    pub(in crate::renderer) fn regency_landed(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        on_unit: bool,
        start: f32,
    ) -> bool {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(grade) = grade(w) else {
            return false;
        };
        let (impact, splash, size) = (w.impact.max(0.3), w.splash.to_f32(), ball(w));
        // A squeezed strike on a hull standing on the ground sears the ground under it too.
        let height = at.z - self.ground_height(at.truncate());
        let ground = height < 1.5 && !on_unit;
        let under = height < 4.0;
        match grade {
            Grade::Bolt => self.bolt_splash(at, impact, ground, start),
            Grade::Mortar => self.mortar_burst(at, impact, splash, ground, start),
            Grade::Flak => self.plasma_flak_burst(at, impact, splash, ground, start),
            Grade::Pinched => self.pinched_burst(at, impact, size, under, start),
            Grade::Fusion => self.fusion_nova(at, impact, size * 1.5, splash, under, start),
        }
        true
    }

    /// A Plasmeric bolt splashing: it dumps its heat in one go. A white-hot flash, a
    /// ragged red bloom over it, a knot of plasma left clinging and frying where it hit,
    /// droplets of it spattered out (up off the ground, every way off a hull) and sparks
    /// thrown, the ground seared to a glowing spot. Hard-edged and short, no mist: a stream
    /// of them lands.
    fn bolt_splash(&mut self, at: Vec3, impact: f32, ground: bool, start: f32) {
        let s = 2.8 * impact;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.07,
            (s * 0.4, s * 0.7),
            HOT * 3.4,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.3,
            (s * 0.35, s * 1.15),
            RED * 3.4,
            0.0,
        );
        // What clings: a small hot knot that fries a while after the bloom is gone.
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start + 0.04,
            0.55,
            (s * 0.15, s * 0.42),
            RED.lerp(HOT, 0.45) * 2.6,
            0.0,
        );
        for _ in 0..6 {
            let rise = if ground {
                0.45 + self.scatter.unit()
            } else {
                self.scatter.signed()
            };
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), rise).normalize_or(Vec3::Z);
            let blob = s * (0.05 + 0.04 * self.scatter.unit());
            let speed = s * (2.5 + 3.5 * self.scatter.unit());
            let life = 0.4 + 0.25 * self.scatter.unit();
            self.push_lit(
                GLOB,
                at,
                out * speed,
                start,
                life,
                (blob, blob * 0.35),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        for _ in 0..6 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.3 + self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.15 + 0.12 * self.scatter.unit();
            let speed = 10.0 + 14.0 * self.scatter.unit();
            let life = 0.35 + 0.25 * self.scatter.unit();
            self.push_lit(
                MOTE,
                at,
                out * speed,
                start,
                life,
                (dot, dot * 0.4),
                HOT.lerp(RED, 0.5) * 4.5,
                0.0,
            );
        }
        if ground {
            self.ground_melt
                .melt(at.truncate(), 0.9 * impact, start, 3.0);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z,
            color: RED.lerp(HOT, 0.15) * 110.0 * impact,
            range: 13.0 * impact,
            start,
            life: 0.35,
            pulse: 0.0,
        });
    }

    /// A plasma flak bolt bursting: a wide red bloom throwing sparkles and streaks of
    /// plasma out through the air.
    fn plasma_flak_burst(&mut self, at: Vec3, impact: f32, splash: f32, ground: bool, start: f32) {
        let s = (splash * 1.4).max(4.0) * impact;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.45,
            (s * 0.25, s),
            RED * 3.2,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.12,
            (s * 0.3, s * 0.45),
            HOT * 3.0,
            0.0,
        );
        for _ in 0..7 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let reach = s * (0.4 + 0.4 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                out * reach,
                start,
                0.3,
                (0.12, 0.08),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        for _ in 0..8 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.25 + 0.2 * self.scatter.unit();
            let when = start + self.scatter.unit() * 0.1;
            self.push_lit(
                MOTE,
                at,
                out * s * 2.2,
                when,
                0.8,
                (dot, dot * 0.3),
                RED * 4.0,
                0.0,
            );
        }
        if ground {
            self.ground_melt.melt(at.truncate(), 0.25 * s, start, 2.5);
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * 100.0 * impact,
            range: s * 3.0,
            start,
            life: 0.35,
            pulse: 0.0,
        });
    }

    /// A Pinched-plasmeric bolt bursting: a hard red heart over a white one that cools
    /// slowly in lumps, a spout of plasma thrown up out of it, a skirt of it rolling out
    /// low, red plasma crackling over the ground, molten spatter and globs thrown out in
    /// place of a shock ring, and the ground seared, embers rising off it a while.
    fn pinched_burst(&mut self, at: Vec3, impact: f32, size: f32, ground: bool, start: f32) {
        let s = size * 2.0 * impact;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.2,
            (s * 0.5, s * 0.9),
            WHITE * 3.0,
            0.0,
        );
        self.push_lingering(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.6,
            (s * 0.35, s * 1.3),
            RED * 3.0,
            0.0,
            2,
        );
        // The red it cools to, lingering: it spreads slower and slower as it goes out.
        self.push_lingering(
            BURST,
            at + Vec3::Z * s * 0.2,
            Vec3::ZERO,
            start + 0.06,
            0.6,
            (s * 0.4, s * 1.1),
            RED.lerp(HOT, 0.2) * 2.2,
            0.0,
            2,
        );
        // Lumps of it bursting out at their own places and moments, so it billows.
        for _ in 0..5 {
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + 0.6 * self.scatter.unit(),
            ) * s
                * 0.4;
            let lump = s * (0.3 + 0.2 * self.scatter.unit());
            let when = start + 0.03 + self.scatter.unit() * 0.15;
            let life = 0.45 + 0.3 * self.scatter.unit();
            self.push_lingering(
                BURST,
                at + off,
                Vec3::ZERO,
                when,
                life,
                (lump * 0.4, lump * 1.1),
                RED.lerp(HOT, 0.4) * 2.6,
                0.0,
                2,
            );
        }
        // A spout of plasma thrown up out of it, cooling as it climbs.
        for k in 0..9 {
            let f = k as f32 / 9.0;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.25;
            let puff = s * (0.18 + 0.1 * self.scatter.unit());
            let roll0 = self.scatter.unit();
            self.push_lingering(
                WAKE,
                at,
                (Vec3::Z + lean) * s * (4.0 + 3.6 * f),
                start + f * 0.25,
                0.8 + 0.4 * roll0,
                (puff, puff * 1.4),
                RED.lerp(HOT, 0.5 - f * 0.4) * 3.0,
                0.0,
                2,
            );
        }
        // A skirt of plasma rolling out low over the ground, slowing to a stop.
        for _ in 0..10 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.06);
            let puff = s * (0.14 + 0.1 * self.scatter.unit());
            let when = start + 0.04 + self.scatter.unit() * 0.1;
            let life = 0.9 + 0.5 * self.scatter.unit();
            let speed = s * (2.2 + 2.2 * self.scatter.unit());
            let tint = RED.lerp(HOT, 0.3 * self.scatter.unit());
            self.push_lingering(
                WAKE,
                at + out * s * 0.2 + Vec3::Z * s * 0.05,
                out * speed,
                when,
                life,
                (puff, puff * 1.3),
                tint * 3.0,
                0.0,
                2,
            );
        }
        // Red plasma crackling out over the ground round it as the bind lets go.
        for _ in 0..7 {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let r = s * (0.5 + 0.6 * self.scatter.unit());
            let to = at + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            let to = to.with_z(self.ground_height(to.truncate()) + 0.2);
            let from = at + Vec3::Z * s * 0.15;
            let when = start + self.scatter.unit() * 0.6;
            let life = 0.12 + 0.08 * self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                to - from,
                when,
                life,
                (s * 0.05, s * 0.05),
                RED.lerp(HOT, 0.45) * 6.0,
                0.0,
            );
        }
        // Globs of plasma thrown out of it as the bind breaks, in place of a shock ring.
        for _ in 0..14 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.15 + self.scatter.unit() * 0.7,
            )
            .normalize_or(Vec3::Z);
            let speed = s * (2.5 + 3.0 * self.scatter.unit());
            let blob = s * (0.08 + 0.06 * self.scatter.unit());
            let roll0 = self.scatter.unit();
            self.push_lingering(
                GLOB,
                at,
                out * speed,
                start,
                1.2,
                (blob, blob * 0.35),
                RED.lerp(HOT, roll0 * 0.4) * 3.5,
                0.0,
                1,
            );
        }
        for _ in 0..24 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.15 + self.scatter.unit() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let speed = 16.0 + self.scatter.unit() * 34.0;
            let life = 0.7 + self.scatter.unit() * 0.8;
            self.push_lit(
                MOTE,
                at + Vec3::Z * 0.3,
                out * speed,
                start,
                life,
                (0.4, 0.14),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        for _ in 0..8 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.3 + self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let dot = 0.35 + 0.25 * self.scatter.unit();
            let when = start + self.scatter.unit() * 0.2;
            self.push_lit(
                MOTE,
                at,
                out * s * 1.6,
                when,
                1.6,
                (dot, dot * 0.3),
                RED * 3.5,
                0.0,
            );
        }
        if ground {
            // The ground seared: a glassed scorch that glows red and crusts over, embers
            // rising off it a while.
            self.ground_melt.melt(at.truncate(), s * 0.5, start, 12.0);
            for _ in 0..20 {
                let a = self.scatter.unit() * std::f32::consts::TAU;
                let r = s * 0.45 * self.scatter.unit().sqrt();
                let from = at + Vec3::new(a.cos() * r, a.sin() * r, 0.3);
                let drift = Vec3::new(
                    a.cos() * 0.5,
                    a.sin() * 0.5,
                    2.0 + 2.0 * self.scatter.unit(),
                );
                let when = start + 0.3 + self.scatter.unit() * 2.2;
                let dot = 0.3 + 0.3 * self.scatter.unit();
                let life = 1.0 + self.scatter.unit();
                self.push_lit(
                    MOTE,
                    from,
                    drift,
                    when,
                    life,
                    (dot, dot * 0.5),
                    RED * 4.5,
                    0.0,
                );
            }
            for k in 0..2 {
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    at + Vec3::Z * 0.8,
                    Vec3::Z * (2.0 + k as f32),
                    start + 0.3 + k as f32 * 0.3,
                    2.4,
                    (s * 0.25, s * 0.75),
                );
            }
            self.plasma_fx.guns.light(Glow {
                pos: at + Vec3::Z,
                color: RED * 40.0 * impact * size,
                range: s * 1.4,
                start: start + 0.4,
                life: 2.6,
                pulse: 3.0,
            });
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z * 2.0,
            color: RED * 70.0 * impact * size,
            range: s * 2.4,
            start,
            life: 1.0,
            pulse: 0.0,
        });
    }

    /// A round of a Pinched jet pouring in after its head struck (`Weapon::round_span`,
    /// from `stream_bursts`), `size` its drawn width: a red splash over a hot heart and a
    /// few sparkles thrown off. The strike itself is the head's (`pinched_burst`).
    pub(in crate::renderer) fn pinched_pour(&mut self, at: Vec3, size: f32, start: f32) {
        let s = size * 2.2;
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.3,
            (s * 0.4, s),
            RED * 3.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.09,
            (s * 0.4, s * 0.6),
            HOT * 3.0,
            0.0,
        );
        for _ in 0..3 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + self.scatter.unit() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let speed = 14.0 + self.scatter.unit() * 22.0;
            self.push_lit(
                MOTE,
                at,
                out * speed,
                start,
                0.5,
                (0.3, 0.1),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at + Vec3::Z,
            color: RED * 80.0 * size,
            range: s * 2.5,
            start,
            life: 0.15,
            pulse: 0.0,
        });
    }
}
