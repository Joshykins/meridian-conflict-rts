//! A fusion lance's charge, burn and bite as they are drawn (`Grade::Lance`: a held
//! Pinch-fusion beam that charges, the Sarissa's Skyward Lance), for `regency_guns_fx`.
//! The stream itself is `plasma_fx`'s held fusion beam; this draws what is round it, in
//! fusion's colours (white, the prism's rose and lavender, cooling to violet), never red.
//!
//! - **The charge:** fusion poured up the bore: rose-white streaks running down the core
//!   from the breech in pulses, faster as it fills, while a star kindles at the lens. Two
//!   tethers of lightning hold it from the lens heads over and under the mouth (a split
//!   bore's jaws), crawling at first,
//!   white and steady at the end. Over the last fifth the star is squeezed down to a
//!   hard point and its light strobes: the pinch.
//! - **Ignition** (the first tick of the burn): a blinding white flash, a pulse of light
//!   running up the lance to the hull, streamers thrown off round the mouth in a loose
//!   band, arcs snapping forward, a kick of heat off the turret's back.
//! - **The burn:** the star held white-hot at the lens, beating; the tethers held taut;
//!   sparks shed off the mouth falling; a pulse running up the lance every so often.
//! - **The bite** (`lance_bite`, each tick it holds on a hull): a white-hot spot in a rose
//!   bloom on the plating, molten globs and sparks falling off the hull, dark smoke torn
//!   off it.
//! - **Winding down** (`lance_spent`): the star collapses into a violet glow, lavender motes
//!   vented out round the mouth, heat rising off the turret's back for a few seconds.
//!
//! Presentation only; the renderer's own clock. Hard-edged and short-lived (no mist).

use std::f32::consts::TAU;

use glam::Vec3;
use mc_data::{PlasmaGrade, Weapon};

use super::super::star_core_fx::{LAVENDER, ROSE};
use super::super::{Renderer, PUFF_SPARK, PUFF_TREE_SMOKE};
use super::{Glow, ARC, GLOB, GLOW, MOTE, MOTE_LIFE, MOTE_REACH, STAR, STREAK, WHITE, WISP};
use crate::gpu_consts::puff;

/// What fusion cools to.
const VIOLET: Vec3 = Vec3::new(0.6, 0.4, 1.0);
/// The star's face across at full charge, at the gun's `flash` of 1, and how far over and
/// under the mouth the tethers reach out to the lens heads, metres.
const STAR_FACE: f32 = 3.6;
const TETHER: f32 = 2.6;
/// The share of the charge after which the star is squeezed down (the pinch).
const PINCH: f32 = 0.8;
/// Seconds between the pulses that run up the lance while it burns, and how fast they go.
const PULSE_GAP: f32 = 0.55;
const PULSE_SPEED: f32 = 2400.0;

/// A held Pinch-fusion beam that charges (`Weapon::charge_ticks`): a fusion lance. The
/// star beam (`Weapon::star_beam`, the Scourge's) is drawn as the star instead.
pub(super) fn lance(w: &Weapon) -> bool {
    w.beam
        && w.charge_ticks > 0
        && !w.star_beam
        && w.plasma_shot() == Some(PlasmaGrade::PinchFusion)
}

/// The gun's look scale: its `flash`.
fn scale(w: &Weapon) -> f32 {
    w.flash.clamp(0.5, 3.0)
}

/// Across and up off the bore `dir`.
fn bore_frame(dir: Vec3) -> (Vec3, Vec3) {
    let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
    (side, side.cross(dir))
}

/// The gun's length from its trunnion to its muzzle.
fn reach(w: &Weapon) -> f32 {
    (Vec3::from(w.muzzle.to_f32()) - w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32())))
        .length()
        .max(4.0)
}

/// The two lens heads over and under the mouth, on the jaws of a split bore.
fn heads(at: Vec3, dir: Vec3, r: f32) -> [Vec3; 2] {
    let (_, up) = bore_frame(dir);
    [up, -up].map(|u| at + u * r - dir * 0.8)
}

impl Renderer {
    /// `span` seconds of a lance's charge `f` of the way done, or of its burn (`burning`):
    /// `at` the lens, `dir` down the bore.
    pub(super) fn lance_step(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        f: f32,
        burning: bool,
        when: f32,
        span: f32,
    ) {
        let s = scale(w);
        let life = span * 2.0;
        let seed = (w.damage.to_f32() * 0.017).fract() * 40.0;
        let heads = heads(at, dir, TETHER * s * 0.8);
        if burning {
            self.lance_burns(at, dir, w, &heads, when, span);
            return;
        }
        let pinch = super::smooth(PINCH, 1.0, f);
        // The star at the lens: grows with the charge, squeezed down to a point at the end.
        let face = STAR_FACE * s * (0.2 + 0.8 * f.sqrt()) * (1.0 - 0.55 * pinch);
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            when,
            life,
            (face / 0.42, face / 0.42),
            Vec3::splat(0.8 + 1.6 * f + 1.6 * pinch),
            seed,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            when,
            life,
            (face * 3.0, face * 3.0),
            ROSE.lerp(WHITE, pinch) * (0.6 + 1.4 * f),
            0.0,
        );
        // Fusion poured up the bore: streaks down the core in pulses, faster as it fills.
        let len = reach(w);
        let pour = (f * 3.0 + self.scatter.unit()) as usize;
        for _ in 0..pour {
            let back = len * (0.3 + 0.55 * self.scatter.unit());
            let from = at - dir * back;
            let roll = self.scatter.unit();
            self.push_lit(
                STREAK,
                from,
                dir * back * (0.5 + 0.4 * f),
                when + roll * span,
                0.25 - 0.1 * f,
                (0.8 * s, 0.25 * s),
                ROSE.lerp(WHITE, roll * f) * (3.5 + 5.0 * f),
                0.0,
            );
        }
        // Tethers from the lens heads, crawling at first, white and steady at the end.
        let odds = 0.25 + 0.75 * f;
        for &h in &heads {
            if self.scatter.unit() > odds {
                continue;
            }
            let tint = LAVENDER.lerp(WHITE, pinch.max(f * self.scatter.unit()));
            self.push_lit(
                ARC,
                h,
                (at - h) * 0.95,
                when,
                life * (1.0 + 2.0 * pinch),
                (0.55 * s, 0.55 * s),
                tint * (3.5 + 5.0 * f),
                0.0,
            );
        }
        // Motes drawn in to the star from round the mouth.
        if self.scatter.unit() < 0.4 + f {
            let (side, up) = bore_frame(dir);
            let a = self.scatter.unit() * TAU;
            let from = at + (side * a.cos() + up * a.sin()) * TETHER * s * 2.2 - dir * 1.5;
            self.push_lit(
                MOTE,
                from,
                (at - from) / MOTE_REACH,
                when,
                MOTE_LIFE,
                (0.2 * s, 0.08 * s),
                LAVENDER.lerp(WHITE, 0.4) * (3.0 + 3.0 * f),
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: ROSE.lerp(WHITE, f) * (12.0 + 220.0 * f * f) * s,
            range: (10.0 + 30.0 * f) * s,
            start: when,
            life: life * 1.2,
            pulse: 2.0 + 18.0 * pinch,
        });
    }

    /// One step of the burn: the star held white-hot, beating; the tethers taut; sparks
    /// falling off the mouth; a pulse up the lance now and then.
    fn lance_burns(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        heads: &[Vec3; 2],
        when: f32,
        span: f32,
    ) {
        let s = scale(w);
        let life = span * 2.0;
        let beat = 0.5 + 0.5 * (when * TAU * 6.0).sin();
        let face = STAR_FACE * s * (0.75 + 0.2 * beat);
        let seed = (w.damage.to_f32() * 0.017).fract() * 40.0;
        self.push_lit(
            STAR,
            at,
            Vec3::ZERO,
            when,
            life,
            (face / 0.42, face / 0.42),
            Vec3::splat(3.2 + 1.2 * beat),
            seed,
        );
        self.push_lit(
            GLOW,
            at + dir * face,
            Vec3::ZERO,
            when,
            life,
            (face * 3.0, face * 3.6),
            WHITE.lerp(ROSE, 0.3) * (1.0 + 0.6 * beat),
            0.0,
        );
        for &h in heads {
            if self.scatter.unit() < 0.6 {
                self.push_lit(
                    ARC,
                    h,
                    (at - h) * 0.95,
                    when,
                    life,
                    (0.32 * s, 0.32 * s),
                    WHITE * 5.0,
                    0.0,
                );
            }
        }
        // Sparks shed off the mouth, falling.
        if self.scatter.unit() < 0.7 {
            let (side, up) = bore_frame(dir);
            let out = (side * self.scatter.signed() + up * self.scatter.signed() + dir * 0.4)
                .normalize_or(dir);
            let speed = 10.0 + self.scatter.unit() * 16.0;
            self.push_puff(PUFF_SPARK, at, out * speed, when, 0.9, (0.3, 0.1));
        }
        // A pulse running up the lance: once a gap, at a phase of its own.
        let phase = ((when + seed) / PULSE_GAP).fract();
        if phase < span / PULSE_GAP {
            self.push_lit(
                GLOW,
                at,
                dir * PULSE_SPEED,
                when,
                (w.range_max.to_f32() / PULSE_SPEED).min(1.5),
                (face * 1.6, face * 2.2),
                WHITE * 3.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE.lerp(ROSE, 0.25) * (240.0 + 80.0 * beat) * s,
            range: 40.0 * s,
            start: when,
            life: life * 1.2,
            pulse: 0.0,
        });
    }

    /// The first tick of a lance's burn: `at` the lens, `dir` down the bore.
    pub(super) fn lance_ignites(&mut self, at: Vec3, dir: Vec3, w: &Weapon, time: f32) {
        let s = scale(w);
        let face = STAR_FACE * s;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.35,
            (face * 2.2, face * 4.0),
            WHITE * 12.0,
            0.0,
        );
        // A pulse running up the lance to whatever it strikes.
        self.push_lit(
            GLOW,
            at,
            dir * PULSE_SPEED,
            time,
            (w.range_max.to_f32() / PULSE_SPEED).min(1.5),
            (face * 2.6, face * 3.4),
            WHITE * 6.0,
            0.0,
        );
        // Streamers thrown off round the mouth in a loose band, leaning forward.
        let (side, up) = bore_frame(dir);
        for k in 0..16 {
            let a = TAU * (k as f32 + self.scatter.unit() * 0.8) / 16.0;
            let out = (side * a.cos() + up * a.sin() + dir * 0.6).normalize_or(dir);
            let thick = face * (0.12 + 0.06 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                at,
                out * face * (2.5 + 2.5 * roll) * puff::NOVA_WISP_DRAG,
                time,
                0.8,
                (thick, thick * 3.0),
                Vec3::splat(2.4),
                0.0,
            );
        }
        for _ in 0..8 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.35;
            let late = self.scatter.unit() * 0.12;
            let far = 4.0 + 5.0 * self.scatter.unit();
            self.push_lit(
                ARC,
                at,
                (dir + spread).normalize_or(dir) * face * far,
                time + late,
                0.16,
                (face * 0.25, face * 0.25),
                WHITE * 6.0,
                0.0,
            );
        }
        self.lance_heat(at, dir, w, time, 10);
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE * 900.0 * s,
            range: 70.0 * s,
            start: time,
            life: 0.5,
            pulse: 0.0,
        });
    }

    /// The burn ended: the star collapses into a violet glow, lavender motes vented out
    /// round the mouth, heat rising off the turret's back.
    pub(super) fn lance_spent(&mut self, at: Vec3, dir: Vec3, w: &Weapon, time: f32) {
        let s = scale(w);
        let face = STAR_FACE * s;
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.9,
            (face * 2.4, face * 0.6),
            VIOLET * 3.0,
            0.0,
        );
        let (side, up) = bore_frame(dir);
        for k in 0..14 {
            let a = TAU * (k as f32 + self.scatter.unit()) / 14.0;
            let out = side * a.cos() + up * a.sin();
            let (late, tint) = (self.scatter.unit() * 0.2, self.scatter.unit());
            self.push_lit(
                MOTE,
                at,
                (out * 1.4 + dir * 0.3) * face * 4.0,
                time + late,
                0.9,
                (0.22 * s, 0.08 * s),
                LAVENDER.lerp(VIOLET, tint) * 3.0,
                0.0,
            );
        }
        self.lance_heat(at, dir, w, time, 18);
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: VIOLET * 120.0 * s,
            range: 30.0 * s,
            start: time,
            life: 0.8,
            pulse: 0.0,
        });
    }

    /// Heat rising off the turret's back, behind the trunnion, either side: `n` faint
    /// lavender streamers over a few seconds.
    fn lance_heat(&mut self, at: Vec3, dir: Vec3, w: &Weapon, time: f32, n: usize) {
        let len = reach(w);
        let flat = dir.with_z(0.0).normalize_or(Vec3::X);
        let across = flat.cross(Vec3::Z);
        let back = at - dir * len - flat * len * 0.3;
        for k in 0..n {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let from = back + across * side * len * 0.12 + flat * self.scatter.signed() * len * 0.1;
            let when = time + 0.1 + k as f32 * 0.15 + self.scatter.unit() * 0.1;
            let thick = len * (0.03 + 0.015 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                from,
                (Vec3::Z * (2.0 + 2.0 * roll) + across * side * 0.5) * puff::NOVA_WISP_DRAG,
                when,
                1.1,
                (thick, thick * 2.5),
                Vec3::splat(0.7 * (1.0 - k as f32 / (n as f32 * 1.3))),
                0.0,
            );
        }
    }

    /// A tick of a lance holding on a hull at `at`, `dir` the way it came: a white-hot spot
    /// in a rose bloom, molten globs and sparks falling off the plating, dark smoke torn
    /// off it.
    pub(in crate::renderer) fn lance_bite(&mut self, at: Vec3, dir: Vec3, width: f32, time: f32) {
        let back = at - dir * width * 0.5;
        self.push_lit(
            GLOW,
            back,
            Vec3::ZERO,
            time,
            0.15,
            (width * 1.6, width * 2.4),
            WHITE * 10.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            back,
            Vec3::ZERO,
            time,
            0.3,
            (width * 3.0, width * 4.6),
            ROSE * 3.0,
            0.0,
        );
        // Molten plating dripping off the hull.
        for _ in 0..3 {
            let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), -0.4)
                .normalize_or(-Vec3::Z);
            let speed = 8.0 + self.scatter.unit() * 14.0;
            self.push_lit(
                GLOB,
                back,
                out * speed - Vec3::Z * 18.0,
                time,
                1.8,
                (width * 0.4, width * 0.15),
                ROSE.lerp(WHITE, 0.4) * 3.0,
                0.6,
            );
        }
        for _ in 0..3 {
            let out = Vec3::new(self.scatter.signed(), self.scatter.signed(), -0.6)
                .normalize_or(-Vec3::Z);
            let speed = 20.0 + self.scatter.unit() * 30.0;
            self.push_puff(PUFF_SPARK, back, out * speed, time, 1.2, (0.35, 0.12));
        }
        {
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.5) * 6.0;
            self.push_puff(
                PUFF_TREE_SMOKE,
                back,
                drift,
                time,
                4.5,
                (width * 1.5, width * 5.0),
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: back,
            color: WHITE.lerp(ROSE, 0.5) * 400.0,
            range: width * 6.0,
            start: time,
            life: 0.2,
            pulse: 0.0,
        });
    }
}
