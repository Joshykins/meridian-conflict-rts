//! A twin-rail Pinch-fusion gun's charge, firing, flight and strike as they are drawn
//! (`Grade::Railed`: a gun with `Weapon::rails`, the Flamberge's main battery), for
//! `regency_guns_fx`. Each bore holds its charge between two rails (one either side of it,
//! the middle one shared), the lens heads at their tips squeezing it flat. Where the
//! Sunspear burns a star in front of its bore, this gun is a rail gun's electricity and a
//! pinched knot: white-hot, a little rose and lavender, never blue, green or red. The two
//! bores' charges stay two points.
//!
//! - **The charge:** white-violet lightning jumping across between the rails either side of
//!   each bore, walking up them from the breech to the charge as it fills, more often,
//!   brighter and faster; at the charge a hard white-hot knot squeezed flat between the
//!   lens heads (plasma_puffs.wgsl `pinch_knot`), small at first and growing, strobing
//!   faster toward the shot; motes drawn in along the rails into it. Its light stays small.
//! - **The shot** (each bore with its own round, both at once): a hard, short white flash
//!   at the mouth and the knot let go flat; a discharge crack run down both rails, arcs
//!   snapping across them; plasma blown out sideways between the rails, a short jet down
//!   the line of fire; a heavy kick of hot gas back off the house's back.
//! - **In flight** (sprites.wgsl `plasma_look::RAILED_LANCE`): a long, thin white-hot
//!   lance in a narrow prism sheath; behind it a hard white thread flashing through rose
//!   and lavender to violet, gone within a second or so, shedding a few sparks.
//! - **The strike** (`railed_strike`): a pinch first, light and motes drawn flat into the
//!   point; then a hard white flash and a pillar of light driven straight up, a tight jet
//!   of fusion after it, and a burst of white-hot streamers taking the prism thrown out low
//!   over the ground every way (no ball or shell), sparks flung, lightning into the ground, the ground glassed into a
//!   molten pool and heat rising off it in lavender streamers. The two lances of a salvo
//!   land a beat apart. No rings, no mist, no dust.
//!
//! Presentation only; the renderer's own clock. Geometry comes from the weapon's data
//! (`muzzles`, `muzzle`, `pivot`), never from the model.

use std::f32::consts::TAU;

use glam::Vec3;
use mc_data::Weapon;

use super::super::Renderer;
use super::{Glow, Trail, TrailKind, ARC, GLOW, LAVENDER, MOTE, MOTE_LIFE, MOTE_REACH};
use super::{ROSE, STREAK, VIOLET, WHITE, WISP};
use crate::gpu_consts::puff;

const KNOT: f32 = puff::PINCH_KNOT as f32;
/// The lightning's violet: white-violet arcs (the arc shader whitens their core).
const ARC_VIOLET: Vec3 = Vec3::new(0.78, 0.55, 1.0);
/// Share of the gun's length (trunnion to charge) the rails run back from the charge.
const RAIL_BACK: f32 = 0.7;
/// Share of the charge by which the lightning has walked up the rails to the charge.
const WALK: f32 = 0.85;
/// A knot's thickness over its radius: at the start of the charge, and squeezed at its end.
const FLAT: (f32, f32) = (0.45, 0.2);
/// Seconds a strike's pinch draws in before it goes off, and the most the second lance
/// of a salvo lands after the first (a beat apart).
const PINCH: f32 = 0.12;
const BEAT: f32 = 0.14;
/// Metres a piece of a lance's trail runs, and the seconds it glows.
pub(super) const RAILED_TRAIL: (f32, f32) = (10.0, 1.1);
/// Sparks a lance's trail sheds a tick at most, over every lance. A deliberate cosmetic
/// cap: past it the trail is laid without them.
const MAX_SPARKS: usize = 60;

/// The gun's look scale: its `flash`.
fn scale(w: &Weapon) -> f32 {
    w.flash.clamp(0.5, 3.0)
}

/// Metres from a bore's line to the rail either side of it: half the gap between the
/// bores (the middle rail stands between them).
fn rail_gap(w: &Weapon) -> f32 {
    let m: Vec<Vec3> = w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect();
    let gap = m
        .iter()
        .enumerate()
        .flat_map(|(i, a)| m[i + 1..].iter().map(move |b| a.distance(*b)))
        .fold(f32::INFINITY, f32::min);
    if gap.is_finite() {
        (gap * 0.5).clamp(0.4, 4.0)
    } else {
        1.0
    }
}

/// The gun's length from its trunnion to its charge (an aft house's muzzle mirrored, as
/// `GunFrame::mouth` places it).
pub(super) fn gun_reach(w: &Weapon) -> f32 {
    let m = Vec3::from(w.muzzle.to_f32());
    let m = if w.rear {
        Vec3::new(-m.x, -m.y, m.z)
    } else {
        m
    };
    (m - w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32()))).length()
}

/// Across (level) and up off the bore `dir`: the rails stand either side, across.
fn bore_frame(dir: Vec3) -> (Vec3, Vec3) {
    let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
    (side, side.cross(dir))
}

impl Renderer {
    /// `span` seconds of a railed gun's charge `f` of the way done, for one bore: `at` its
    /// charge, between the rail tips, `dir` down the bore.
    pub(super) fn railed_charge_step(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        f: f32,
        when: f32,
        span: f32,
    ) {
        let s = scale(w);
        let g = rail_gap(w);
        let back = gun_reach(w) * RAIL_BACK;
        let life = span * 2.0;
        let (side, up) = bore_frame(dir);
        // The knot, squeezed flat across (between the lens heads), small at first.
        let grow = f.powf(0.7);
        let r = g * 0.62 * (0.18 + 0.82 * grow) * s.sqrt();
        let flat = FLAT.0 + (FLAT.1 - FLAT.0) * f;
        self.push_lit(
            KNOT,
            at,
            side,
            when,
            life,
            (r, flat),
            Vec3::splat(0.9 + 2.2 * f),
            3.0 + 22.0 * f * f,
        );
        // A tight halo round it, kept inside the rails so the two knots stay two.
        if f > 0.25 {
            let halo = g * (0.55 + 0.35 * f);
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (halo, halo),
                WHITE.lerp(ROSE, 0.35) * 0.35 * f,
                0.0,
            );
        }
        // Lightning jumping across between the rails, walking up them from the breech as it
        // fills: thick at its front, a few behind it.
        let front = -back * (1.0 - super::smooth(0.0, WALK, f));
        let rate = 0.5 + 2.8 * f;
        let n = (rate + self.scatter.unit()) as usize;
        for _ in 0..n {
            let behind = self.scatter.unit();
            let u = (front - back * 0.4 * behind * behind).max(-back);
            let flip = if self.scatter.unit() < 0.5 { 1.0 } else { -1.0 };
            let from = at + dir * u - side * g * flip + up * self.scatter.signed() * g * 0.15;
            let kink = dir * self.scatter.signed() * g * 0.4 + up * self.scatter.signed() * g * 0.2;
            let roll = self.scatter.unit();
            let last = 0.05 + 0.04 * self.scatter.unit();
            self.push_lit(
                ARC,
                from,
                side * 2.0 * g * flip + kink,
                when + roll * span,
                last,
                (g * (0.22 + 0.2 * f), g * (0.22 + 0.2 * f)),
                WHITE.lerp(ARC_VIOLET, 0.3 + 0.5 * roll) * (2.5 + 5.0 * f),
                0.0,
            );
        }
        // Near the shot, lightning from the rail tips into the knot.
        if f > 0.55 && self.scatter.unit() < f * 0.8 {
            let flip = if self.scatter.unit() < 0.5 { 1.0 } else { -1.0 };
            let from = at + side * g * flip + dir * self.scatter.signed() * g * 0.3;
            self.push_lit(
                ARC,
                from,
                at - from,
                when,
                0.06,
                (g * 0.3, g * 0.3),
                WHITE.lerp(ARC_VIOLET, 0.3) * (3.0 + 4.0 * f),
                0.0,
            );
        }
        // Motes drawn in along the rails into it.
        let motes = (0.5 + 2.5 * f + self.scatter.unit()) as usize;
        for _ in 0..motes {
            let flip = if self.scatter.unit() < 0.5 { 1.0 } else { -1.0 };
            let u = -back * (0.1 + 0.6 * self.scatter.unit());
            let from = at + dir * u + side * g * flip * 0.9;
            let roll = self.scatter.unit();
            let dot = 0.12 * g * s;
            self.push_lit(
                MOTE,
                from,
                (at - from) / MOTE_REACH,
                when + roll * span,
                MOTE_LIFE,
                (dot, dot * 0.4),
                LAVENDER
                    .lerp(WHITE, 0.3 + 0.5 * roll)
                    .lerp(ROSE, 0.2 * (1.0 - roll))
                    * (3.0 + 3.0 * f),
                0.0,
            );
        }
        // Its light: small, so the hull is lit, not flooded; strobing faster as it fills.
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE.lerp(ROSE, 0.3) * (4.0 + 40.0 * f * f) * s,
            range: g * (2.5 + 4.0 * f) * s,
            start: when,
            life: life * 1.2,
            pulse: 3.0 + 18.0 * f * f,
        });
    }

    /// A railed gun fired one bore's round: `at` its charge as drawn, `dir` the round's way
    /// out, `reach` the gun's length from trunnion to charge.
    pub(super) fn railed_fired(&mut self, w: &Weapon, at: Vec3, dir: Vec3, reach: f32, time: f32) {
        let s = scale(w);
        let g = rail_gap(w);
        let back = reach * RAIL_BACK;
        let (side, up) = bore_frame(dir);
        // A hard, short white flash at the mouth: inside the rails' reach, not a white-out.
        self.push_lit(
            GLOW,
            at + dir * g * 0.3,
            Vec3::ZERO,
            time,
            0.09,
            (g * 1.4 * s, g * 2.0 * s),
            WHITE * 9.0,
            0.0,
        );
        // The knot let go: flattened out further for an instant.
        self.push_lit(
            KNOT,
            at,
            side,
            time,
            0.1,
            (g * 0.9 * s, 0.14),
            Vec3::splat(4.0),
            0.0,
        );
        // The discharge cracked down both rails, breech to tip, and arcs across them.
        for flip in [-1.0, 1.0] {
            let from = at - dir * back + side * g * flip;
            self.push_lit(
                STREAK,
                from,
                dir * (back + g * 0.4),
                time,
                0.16,
                (g * 0.16, g * 0.16),
                WHITE.lerp(ARC_VIOLET, 0.25) * 7.0,
                0.0,
            );
        }
        for k in 0..5 {
            let u = -back * (k as f32 + self.scatter.unit()) / 5.0;
            let flip = if k % 2 == 0 { 1.0 } else { -1.0 };
            let from = at + dir * u - side * g * flip;
            let lift = up * self.scatter.signed() * g * 0.3;
            self.push_lit(
                ARC,
                from,
                side * 2.0 * g * flip + lift,
                time + 0.012 * (5 - k) as f32,
                0.07,
                (g * 0.3, g * 0.3),
                WHITE.lerp(ARC_VIOLET, 0.3) * 6.0,
                0.0,
            );
        }
        // Plasma blown out sideways between the rails, and a short jet down the line of fire.
        for flip in [-1.0, 1.0] {
            for _ in 0..6 {
                let spread =
                    up * self.scatter.signed() * 0.5 + dir * (0.2 + 0.5 * self.scatter.unit());
                let v = (side * flip + spread).normalize_or(side)
                    * g
                    * s
                    * (5.0 + 5.0 * self.scatter.unit());
                let thick = g * s * (0.18 + 0.1 * self.scatter.unit());
                let roll = self.scatter.unit();
                self.push_lit(
                    WISP,
                    at,
                    v * puff::NOVA_WISP_DRAG,
                    time + roll * 0.03,
                    0.35 + 0.25 * roll,
                    (thick, thick * 2.6),
                    Vec3::splat(2.0),
                    0.0,
                );
            }
        }
        for _ in 0..8 {
            let spread = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.signed(),
            ) * 0.12;
            let v = (dir + spread).normalize_or(dir) * g * s * (8.0 + 10.0 * self.scatter.unit());
            let thick = g * s * (0.16 + 0.1 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                at + dir * g * 0.5,
                v * puff::NOVA_WISP_DRAG,
                time + roll * 0.04,
                0.3 + 0.2 * roll,
                (thick, thick * 3.4),
                Vec3::splat(2.4),
                0.0,
            );
        }
        // A heavy kick of hot gas back off the house's back, behind this bore.
        let flat = dir.with_z(0.0).normalize_or(Vec3::X);
        let across = flat.cross(Vec3::Z);
        // Behind the trunnion, in line with this bore.
        let house = at - dir * reach - flat * reach * 0.25 + Vec3::Z * reach * 0.05;
        for k in 0..12 {
            let from = house + across * self.scatter.signed() * g * 1.2;
            let out = (-flat * 1.0
                + Vec3::Z * (0.35 + 0.4 * self.scatter.unit())
                + across * self.scatter.signed() * 0.35)
                .normalize_or(-flat);
            let thick = g * (0.35 + 0.25 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                from,
                out * reach * (0.8 + 0.8 * roll) * puff::NOVA_WISP_DRAG,
                time + 0.02 + k as f32 * 0.02,
                0.55 + 0.35 * roll,
                (thick, thick * 2.8),
                Vec3::splat(1.3 * (1.0 - k as f32 / 16.0)),
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE * 260.0 * s,
            range: g * 9.0 * s,
            start: time,
            life: 0.22,
            pulse: 0.0,
        });
    }

    /// A railed gun's knot let go at `at` (its round left): a last squeezed flash.
    pub(super) fn railed_spent(&mut self, at: Vec3, w: &Weapon, life: f32, time: f32) {
        let g = rail_gap(w);
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            life,
            (g * 0.8, g * 1.1),
            WHITE * 4.0,
            0.0,
        );
    }

    /// A railed lance's trail over the stretch `from` to `to` it flies this tick (`size` the
    /// shot's, `(starts, span)` the shares of the tick it flies over): a hard white thread
    /// and a narrower-lived sheath round it, flashing through rose and lavender to violet
    /// and going out within a second or so, a few sparks falling off it. `sparks` counts
    /// those shed this tick over every lance.
    pub(super) fn railed_trail(
        &mut self,
        from: Vec3,
        to: Vec3,
        size: f32,
        (starts, span): (f32, f32),
        sparks: &mut usize,
        time: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        let (step, life) = RAILED_TRAIL;
        // A deliberate cosmetic cap on one shot's stretch: a tick's flight is far under
        // this many pieces.
        let n = ((from.distance(to) / step).ceil() as usize).clamp(1, 16);
        for k in 0..n {
            let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
            let when = time + tick * (starts + (span - starts).max(0.0) * f1);
            let fx = &mut self.plasma_fx.guns;
            fx.trail(Trail {
                from: a,
                to: b,
                start: when,
                life,
                width: size * 0.1,
                kind: TrailKind::Railed,
            });
            fx.trail(Trail {
                from: a,
                to: b,
                start: when,
                life: life * 0.25,
                width: size * 0.25,
                kind: TrailKind::Railed,
            });
            if *sparks < MAX_SPARKS && self.scatter.unit() < 0.3 {
                *sparks += 1;
                let at = a.lerp(b, self.scatter.unit());
                let drift = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 6.0;
                let roll = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    at,
                    drift - Vec3::Z * 3.0,
                    when,
                    0.4 + 0.4 * roll,
                    (0.35, 0.14),
                    WHITE.lerp(ROSE, roll).lerp(LAVENDER, roll * roll) * 5.0,
                    0.0,
                );
            }
        }
    }

    /// A railed lance struck at `at`. A pinch drawn flat into the point; then a hard white
    /// flash and a pillar of light driven straight up with a tight jet of fusion after it,
    /// a burst of white-hot streamers taking the prism thrown out low every way, sparks flung, lightning into the ground, the
    /// ground glassed into a molten pool and heat rising off it. Each lance of a salvo
    /// goes off on its own beat, so the pair land a moment apart.
    pub(super) fn railed_strike(
        &mut self,
        at: Vec3,
        impact: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let r = splash.max(8.0) * impact.clamp(0.5, 3.0);
        let centre = at + Vec3::Z * r * 0.08;
        let pinch = PINCH + BEAT * self.scatter.unit();
        // The pinch: a flat white knot drawn in over the point, lavender motes drawn to it.
        self.push_lit(
            KNOT,
            centre,
            Vec3::Z,
            start,
            pinch,
            (r * 0.22, 0.25),
            Vec3::splat(2.5),
            30.0,
        );
        for _ in 0..18 {
            let a = self.scatter.unit() * TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.15 * self.scatter.signed());
            let from = centre + out * r * (0.5 + 0.4 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                from,
                (centre - from) / (MOTE_REACH * pinch / MOTE_LIFE),
                start,
                pinch,
                (0.6, 0.25),
                LAVENDER.lerp(WHITE, roll) * 4.0,
                0.0,
            );
        }
        let bang = start + pinch;
        // A hard white flash, short.
        self.push_effect(centre.to_array(), bang, r * 0.7, 0.18, 9.0, 0.0);
        self.push_lit(
            GLOW,
            centre,
            Vec3::ZERO,
            bang,
            0.14,
            (r * 0.5, r * 0.8),
            WHITE * 10.0,
            0.0,
        );
        // A pillar of light driven straight up out of it.
        self.push_lit(
            STREAK,
            centre,
            Vec3::Z * r * 3.2,
            bang,
            0.22,
            (r * 0.02, r * 0.015),
            WHITE * 6.0,
            0.0,
        );
        // A tight jet of fusion driven up the pillar after it.
        for k in 0..10 {
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.06;
            let thick = r * (0.05 + 0.03 * self.scatter.unit());
            let go = r * (1.6 - 0.08 * k as f32);
            self.push_lit(
                WISP,
                centre,
                (Vec3::Z + lean) * go * puff::NOVA_WISP_DRAG,
                bang + 0.015 * k as f32,
                0.7,
                (thick, thick * 3.0),
                Vec3::splat(2.6),
                0.0,
            );
        }
        // The prism burst: white-hot streamers thrown out low over the ground, every way, each its own length.
        for _ in 0..40 {
            let a = self.scatter.unit() * TAU;
            let rise = if ground {
                0.04 + 0.2 * self.scatter.unit()
            } else {
                self.scatter.signed() * 0.6
            };
            let out = Vec3::new(a.cos(), a.sin(), rise).normalize_or(Vec3::X);
            let go = r * (0.7 + 1.1 * self.scatter.unit());
            let thick = r * (0.07 + 0.06 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                centre + out * r * 0.05,
                out * go * puff::NOVA_WISP_DRAG,
                bang + 0.03 * roll,
                0.9 + 0.6 * roll,
                (thick, thick * 3.4),
                Vec3::splat(3.0),
                0.0,
            );
        }
        // Sparks flung out of it, white going rose and lavender.
        for _ in 0..40 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.15 + 0.85 * self.scatter.unit(),
            )
            .normalize_or(Vec3::Z);
            let roll = self.scatter.unit();
            let tint = if roll < 0.5 {
                WHITE.lerp(ROSE, roll * 2.0)
            } else {
                ROSE.lerp(LAVENDER, roll * 2.0 - 1.0)
            };
            let speed = r * (1.5 + 2.5 * self.scatter.unit());
            let last = 0.7 + 0.8 * self.scatter.unit();
            self.push_lit(
                MOTE,
                centre,
                out * speed,
                bang,
                last,
                (0.6, 0.2),
                tint * 5.0,
                0.0,
            );
        }
        // Lightning into the ground round it as it goes off.
        for _ in 0..7 {
            let a = self.scatter.unit() * TAU;
            let d = r * (0.4 + 0.6 * self.scatter.unit());
            let to = at + Vec3::new(a.cos() * d, a.sin() * d, 0.0);
            let to = to.with_z(self.ground_height(to.truncate()) + 0.2);
            let roll = self.scatter.unit();
            self.push_lit(
                ARC,
                centre,
                to - centre,
                bang + roll * 0.2,
                0.12,
                (r * 0.03, r * 0.03),
                WHITE.lerp(ARC_VIOLET, roll * 0.6) * 7.0,
                0.0,
            );
        }
        if ground {
            self.ground_melt
                .melt(at.truncate(), r * 0.75, 0.85, bang, 16.0);
        }
        // Heat rising off the pool: lavender streamers going up slowly, for a couple of seconds.
        for k in 0..12 {
            let f = k as f32 / 12.0;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.35;
            let thick = r * (0.08 + 0.05 * self.scatter.unit());
            let rise = r * (0.5 + 0.4 * self.scatter.unit());
            let last = 1.0 + 0.4 * self.scatter.unit();
            self.push_lit(
                WISP,
                at + off + Vec3::Z * 0.5,
                Vec3::Z * rise * puff::NOVA_WISP_DRAG,
                bang + 0.3 + f * 1.8,
                last,
                (thick, thick * 2.6),
                Vec3::splat(0.7 * (1.0 - 0.5 * f)),
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: centre + Vec3::Z * r * 0.3,
            color: WHITE * 900.0 * impact,
            range: r * 3.0,
            start: bang,
            life: 0.45,
            pulse: 0.0,
        });
        self.plasma_fx.guns.light(Glow {
            pos: centre + Vec3::Z * r * 0.2,
            color: LAVENDER.lerp(VIOLET, 0.4) * 120.0 * impact,
            range: r * 2.0,
            start: bang + 0.2,
            life: 2.0,
            pulse: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_railed_lances_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            crate::gpu_consts::plasma_look::RAILED_LANCE,
            mc_sim::mirror::PLASMA_LOOK_RAILED_LANCE
        );
    }

    #[test]
    fn the_rails_stand_half_the_bores_gap_off_each_bore() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("../../data"),
        )
        .unwrap();
        let ship = bp
            .units
            .iter()
            .find(|u| u.weapons.iter().any(|w| w.rails))
            .expect("a railed gun is in the data");
        for w in ship.weapons.iter().filter(|w| w.rails) {
            let m: Vec<Vec3> = w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect();
            assert!((rail_gap(w) * 2.0 - m[0].distance(m[1])).abs() < 1e-3);
            assert!(gun_reach(w) > 1.0 && gun_reach(w) < 40.0, "{}", w.name);
        }
    }
}
