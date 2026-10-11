//! A surge gun's charge and firing as they are drawn (`Grade::Surge`: a plasma gun with a
//! `Weapon::surge`, the Saker's Surge Mortar), for `regency_guns_fx`. The shell carries
//! the ground wake it lets go where it lands (`surge_fx`), and everything about the gun
//! foreshadows it: crimson and the Regency's bent violet air, pressure and the ground,
//! never a fusion star's white.
//!
//! - **The charge:** pressure runs down the bore from the breech, crimson motes streaming
//!   along the gap between the halves (which draw apart, `charge_gear::PART`, the slots in
//!   their flanks rippling out toward the muzzle) into a knot held between the tips: a
//!   dark-hearted crimson core that tightens rather than swells, red arcs snapping across
//!   the gap, a halo of bent violet air pulled in round it, its light thudding faster as it
//!   fills. Crimson motes skim in over the ground toward the gun's feet, as if the ground
//!   were being drawn up into it.
//! - **The shot:** the halves slam shut and the bore kicks back hard; the knot lets go in a
//!   hard crimson bloom (not a white-out), a cone of red plasma driven out down the line of
//!   fire with violet air torn off round it, and the gun's own surge goes into the ground
//!   through its legs: a small wake rolls out from under it (`surge_fx`). Heat bleeds up
//!   off the bore's slots.
//! - **In flight** (`regency_trails`, sprites.wgsl `plasma_look::SURGE_SHELL`): a solid
//!   graphite shell, crimson in its seams, behind it a crimson plume that cools to violet,
//!   pressure pulses running back down it and beading its trail, crimson embers falling.
//!
//! Presentation only; the renderer's own clock. Hard-edged and short-lived (no mist).

use glam::Vec3;
use mc_data::Weapon;
use mc_sim::mirror::ProjectileInstance;

use crate::gpu_consts::plasma_look;

use super::super::Renderer;
use super::{ball, smooth, Glow, ARC, BURST, GLOW, HOT, MOTE, MOTE_LIFE, MOTE_REACH, ORB, RED};
use super::{Trail, TrailKind, SURGE_TRAIL, WAKE};

/// The Regency's violet, as the surge's bent air (`surge_fx`).
const VIOLET: Vec3 = Vec3::new(0.55, 0.16, 1.0);
/// Deep crimson: the knot's skin and the shell's embers.
const CRIMSON: Vec3 = Vec3::new(0.8, 0.02, 0.03);
/// Metres off the bore's line the halves' tips stand while it charges: the arcs' reach.
const TIPS: f32 = 1.7;
/// Metres out from the gun the ground's motes start skimming in from.
const DRAW_REACH: f32 = 46.0;
/// The gun's own wake into the ground as it fires: metres out, metres a second, seconds it
/// burns after.
const KICK: (f32, f32, f32) = (64.0, 120.0, 0.4);
/// Metres back down the bore the shell sits at the start of the charge's last tick, before
/// the rails shunt it out to the muzzle for the shot (`surge_shunt`).
const SHUNT: f32 = 24.0;
/// Pressure pulses a second running back down a shell's plume, each beading its trail.
const PULSE_RATE: f32 = 5.0;

/// A surge gun's knot across at full charge, metres: held tight between the tips, far
/// smaller than a fusion star of the same shot.
pub(super) fn knot(w: &Weapon) -> f32 {
    ball(w) * 0.2
}

impl Renderer {
    /// `span` seconds of a surge gun's charge `f` of the way done: `at` the middle of the
    /// charge between the tips, `dir` down the bore, `foot` the ground under the gun.
    pub(super) fn surge_charge_step(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        f: f32,
        foot: Vec3,
        when: f32,
        span: f32,
    ) {
        let size = knot(w);
        let life = span * 2.0;
        // The knot tightens as it fills: wide and dull at first, then small and hard.
        let tight = smooth(0.1, 0.95, f);
        let across = size * (1.3 - 0.55 * tight) * (0.35 + 0.65 * f.sqrt());
        self.push_lit(
            ORB,
            at,
            Vec3::ZERO,
            when,
            life,
            (across, across * 1.04),
            CRIMSON.lerp(RED, f) * (0.7 + 1.8 * f * f),
            0.0,
        );
        // Its heart, hotter the tighter it is held.
        if f > 0.35 {
            let s = across * 0.35;
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (s, s),
                RED.lerp(HOT, 0.35) * (0.5 + 1.0 * tight),
                0.0,
            );
        }
        // Bent violet air round it, pulled in tighter as it fills.
        if f > 0.3 {
            let s = size * (3.6 - 1.4 * tight);
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (s, s * 0.8),
                VIOLET * (0.08 + 0.14 * f),
                0.0,
            );
        }
        // Pressure running down the bore: crimson motes streaming along the gap between
        // the halves from the breech into the knot.
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        let bore = (Vec3::from(w.muzzle.to_f32())
            - w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32())))
        .length();
        let motes = (1.0 + f * 4.0 + self.scatter.unit()) as usize;
        for _ in 0..motes {
            let back = bore * (0.15 + 0.7 * self.scatter.unit());
            let from = at - dir * back + up * self.scatter.signed() * 0.25;
            let rgb = RED.lerp(HOT, self.scatter.unit() * 0.4 * f) * (3.0 + 4.0 * f);
            let dot = size * 0.07;
            let late = self.scatter.unit() * span;
            self.push_lit(
                MOTE,
                from,
                (at - from) / MOTE_REACH,
                when + late,
                MOTE_LIFE,
                (dot, dot * 0.4),
                rgb,
                0.0,
            );
        }
        // Red arcs snapping across the gap at the tips, more often the fuller it gets.
        if f > 0.2 && self.scatter.unit() < 0.2 + f * 0.9 {
            let hold = TIPS * (0.9 + 0.2 * self.scatter.unit());
            let from = at + up * hold - dir * self.scatter.unit() * TIPS;
            let to = at - up * hold - dir * self.scatter.unit() * TIPS;
            let roll = self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                to - from,
                when,
                0.06 + 0.05 * roll,
                (0.35, 0.35),
                RED.lerp(HOT, 0.2 + 0.3 * roll) * (3.0 + 4.0 * f),
                0.0,
            );
        }
        // The ground drawn in toward the gun's feet: crimson motes skimming in low over it
        // once the charge has taken hold.
        for _ in 0..(f * 3.0) as usize {
            let turn = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(turn.cos(), turn.sin(), 0.0);
            let r = DRAW_REACH * (0.7 + 0.5 * self.scatter.unit());
            let xy = (foot + out * r).truncate();
            let water = self.map_info.water_level.to_f32();
            let ground = self.ground_height(xy);
            if ground > water + 0.05 {
                let p = xy.extend(ground + 0.4);
                let pull = -out * r * (0.9 + 0.4 * f);
                let roll = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    p,
                    pull,
                    when,
                    0.8,
                    (0.3, 0.1),
                    CRIMSON.lerp(HOT, roll * 0.5) * 3.0,
                    0.0,
                );
            }
        }
        // Its light, thudding faster as it fills.
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * (5.0 + 55.0 * f * f) * size,
            range: size * (2.5 + 3.0 * f),
            start: when,
            life: life * 1.2,
            pulse: 1.5 + 6.0 * f,
        });
    }

    /// A surge gun fired: `at` the middle of its charge as drawn, `dir` the shell's way out,
    /// `reach` the gun's length from trunnion to charge.
    pub(super) fn surge_fired(&mut self, w: &Weapon, at: Vec3, dir: Vec3, reach: f32, time: f32) {
        let s = knot(w) * 1.4;
        let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        let up = side.cross(dir);
        // The knot lets go: a hard crimson bloom over a short hot heart, no white-out.
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            time,
            0.32,
            (s * 1.4, s * 3.4),
            RED * 3.0,
            0.0,
        );
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.12,
            (s * 1.0, s * 1.5),
            HOT * 2.2,
            0.0,
        );
        // A cone of red plasma driven out down the line of fire.
        for _ in 0..12 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.22;
            let v = (dir + spread).normalize_or(dir) * s * 10.0 * (0.5 + 0.7 * self.scatter.unit());
            let puff = s * (0.3 + 0.25 * self.scatter.unit());
            let late = self.scatter.unit() * 0.05;
            let roll = self.scatter.unit();
            self.push_lit(
                WAKE,
                at + dir * s * 0.5,
                v,
                time + late,
                0.4 * (0.7 + 0.6 * roll),
                (puff, puff * 1.6),
                CRIMSON.lerp(HOT, roll * 0.35) * 3.2,
                0.0,
            );
        }
        // Violet air torn off round the mouth, thrown out across the line of fire.
        for k in 0..10 {
            let a = std::f32::consts::TAU * (k as f32 + self.scatter.unit() * 0.6) / 10.0;
            let out = side * a.cos() + up * a.sin();
            let puff = s * (0.5 + 0.3 * self.scatter.unit());
            self.push_lit(
                GLOW,
                at + dir * s * 0.3,
                out * s * 7.0 + dir * s * 2.0,
                time,
                0.3,
                (puff, puff * 0.6),
                VIOLET * 1.4,
                0.0,
            );
        }
        // The gun's own surge goes into the ground through its legs: a small wake rolls out
        // from under it.
        let flat = dir.with_z(0.0).normalize_or(Vec3::X);
        let foot = (at - flat * reach).truncate();
        let water = self.map_info.water_level.to_f32();
        let ground = foot.extend(self.ground_height(foot).max(water));
        let (radius, speed, linger) = KICK;
        self.push_surge(
            ground,
            time,
            radius,
            speed,
            linger,
            w.impact.max(0.3) * 0.35,
        );
        // Heat bleeding up off the bore's slots, breech to tip.
        for k in 0..14 {
            let along = reach * (0.2 + 0.75 * k as f32 / 13.0);
            let from = at - dir * along + side * self.scatter.signed() * s * 0.4;
            let when = time + 0.08 + self.scatter.unit() * 0.5;
            let roll = self.scatter.unit();
            let sway = side * self.scatter.signed();
            self.push_lit(
                MOTE,
                from,
                Vec3::Z * (2.0 + 2.5 * roll) + sway,
                when,
                0.8 + 0.6 * roll,
                (0.2, 0.07),
                CRIMSON.lerp(HOT, 0.3 * roll) * 3.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: RED * 380.0 * s,
            range: s * 16.0,
            start: time,
            life: 0.45,
            pulse: 0.0,
        });
    }

    /// A surge shell's trail over the stretch `from` to `to` it flies this tick (`size` the
    /// shot's, `(starts, span)` the shares of the tick it flies over): a crimson ember thread
    /// that cools to violet, a short hot sheath round it, beaded wider and brighter where a
    /// pressure pulse passed, crimson embers falling away from it.
    pub(super) fn surge_trail(
        &mut self,
        from: Vec3,
        to: Vec3,
        size: f32,
        (starts, span): (f32, f32),
        time: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        let (step, life) = SURGE_TRAIL;
        // A deliberate cosmetic cap on one shot's stretch: a tick's flight is far under
        // this many pieces.
        let n = ((from.distance(to) / step).ceil() as usize).clamp(1, 16);
        for k in 0..n {
            let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
            let t0 = time + tick * (starts + (span - starts).max(0.0) * f0);
            let when = time + tick * (starts + (span - starts).max(0.0) * f1);
            // A pulse passed between this piece's ends.
            let bead = (when * PULSE_RATE).floor() > (t0 * PULSE_RATE).floor();
            let fx = &mut self.plasma_fx.guns;
            fx.trail(Trail {
                from: a,
                to: b,
                start: when,
                life: if bead { life * 1.3 } else { life },
                width: size * if bead { 0.2 } else { 0.07 },
                kind: TrailKind::Surge,
            });
            fx.trail(Trail {
                from: a,
                to: b,
                start: when,
                life: life * 0.2,
                width: size * 0.2,
                kind: TrailKind::Surge,
            });
            // Embers falling away from it, about one a piece.
            if self.scatter.unit() < 0.7 {
                let at = a.lerp(b, self.scatter.unit());
                let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 4.0;
                let roll = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    at,
                    drift - Vec3::Z * (3.0 + 3.0 * roll),
                    when,
                    0.7 + 0.7 * roll,
                    (0.35, 0.12),
                    CRIMSON.lerp(HOT, 0.5 * roll) * 4.0,
                    0.0,
                );
            }
        }
    }

    /// The shell shunted out along the rails over the last tick of the charge, from `SHUNT`
    /// metres back down the bore to the muzzle (`from` and `to`, where the muzzle is drawn
    /// at that tick's start and end, `dir` down the bore), arriving as it fires: drawn as the
    /// shell in flight is (`plasma_look::SURGE_SHELL`), its plume glowing in the parted gap.
    pub(super) fn surge_shunt(&mut self, w: &Weapon, from: Vec3, to: Vec3, dir: Vec3) {
        // The shot's size as the mirror draws it (`mc_sim::mirror`, a shell's `size`).
        let power = w.damage.to_f32().sqrt();
        let size = (0.3 + power * 0.045 + w.splash.to_f32() * 0.07) * w.tracer;
        let look = (1.0 + w.red + 2.0 * plasma_look::SURGE_SHELL as f32, 0.0);
        self.plasma_fx.guns.shunts.push(ProjectileInstance {
            prev_pos: (from - dir * SHUNT).to_array(),
            color: w.color as u32,
            pos: (to - dir * 0.5).to_array(),
            size,
            wake: 0.0,
            plasma: 0.0,
            _pad: [look.0, look.1],
            aim: [0.0; 4],
            prev_aim: [0.0; 4],
        });
    }

    /// The knot of a surge gun's charge let go at `at`: a crimson flash where it hung.
    pub(super) fn surge_spent(&mut self, at: Vec3, w: &Weapon, life: f32, time: f32) {
        let size = knot(w);
        self.push_lit(
            ORB,
            at,
            Vec3::ZERO,
            time,
            life,
            (size * 1.2, size * 0.3),
            RED * 5.0,
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_surge_shells_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            crate::gpu_consts::plasma_look::SURGE_SHELL,
            mc_sim::mirror::PLASMA_LOOK_SURGE_SHELL
        );
    }
}
