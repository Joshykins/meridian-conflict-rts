//! A knotted Pinch-fusion gun's charge, firing, flight and strike as they are drawn
//! (`Grade::Knotted`: a gun with `Weapon::knots`, the Springald's Triune Howitzer), for
//! `regency_guns_fx`. Where the Sunspear burns one rose-white star in front of its bore,
//! this gun holds a knot of fusion at each of its projectors and pinches them into one:
//! cold white light and bent indigo air. What it lets go is red: a red-hot shell, a red
//! trail going out into dark smoke, a red strike.
//!
//! - **The charge:** a small white knot kindles at each lens in a cold lilac halo and grows,
//!   lilac motes running up each rail from the breech into its knot. Over the last third the knots are drawn in to the bore's line,
//!   bent indigo air pulled in tight round them: the pinch. They meet as one hard white
//!   point, its light strobing faster to the shot.
//! - **The shot:** a short hard white flash (no long white-out), fusion thrown out in three
//!   jets between the rails as well as down the line of fire; heat rising off the three
//!   cells on the turret's back. No lightning anywhere.
//! - **In flight** (sprites.wgsl `plasma_look::KNOTTED_SHELL`): a long solid graphite shell,
//!   three red seams down it, its base burning; a short red plume round a white-hot core;
//!   behind it a red-hot thread that goes out within a second or two, and dark smoke laid
//!   down the whole arc that greys and hangs (`Weapon::wake` seconds).
//! - **The strike** (`knotted_strike`): red light drawn in to the point for a quarter of
//!   a second, then three red bursts round it one after another, swelling into one bloom; a
//!   fireball climbing, plasma rolling out low, globs flung out, a melted pool, and dark
//!   smoke climbing over it. No supernova, no lightning.
//!
//! Presentation only; the renderer's own clock. Hard-edged and short-lived (no mist).

use std::f32::consts::TAU;

use glam::Vec3;
use mc_data::Weapon;

use super::super::{Renderer, PUFF_TRAIL, PUFF_TREE_SMOKE};
use super::{Glow, Trail, TrailKind, BURST, GLOB, GLOW, MOTE, MOTE_LIFE, MOTE_REACH};
use super::{HOT, KNOTTED_TRAIL, RED, STAR, WAKE, WHITE, WISP};
use crate::gpu_consts::puff;

/// Cold lilac: the knots' rims, the motes, the lens.
const LILAC: Vec3 = Vec3::new(0.72, 0.6, 1.0);
/// The bent indigo air round the pinch, and what the trail cools to.
const INDIGO: Vec3 = Vec3::new(0.3, 0.16, 1.0);
/// A knot's face across at full charge, and the one point they meet as, metres, at the
/// gun's `flash` of 1.
const KNOT_FACE: f32 = 1.3;
const POINT_FACE: f32 = 2.2;
/// Metres out from the bore's line the knots kindle at (between the lenses and the charge),
/// and metres back down the bore.
const KNOT_OUT: f32 = 3.0;
const KNOT_BACK: f32 = 1.2;
/// Share of the charge after which the knots are drawn in, and by which they have met.
const DRAW: (f32, f32) = (0.6, 0.9);
/// Metres between the pieces of a shell's smoke, the seconds after the shell passes that
/// it is laid (as its red goes out), and the pieces laid a tick at most over every shell.
const SMOKE_STEP: f32 = 25.0;
const SMOKE_LATE: f32 = 0.5;
const MAX_SMOKE: usize = 48;
/// Seconds the strike's pinch closes before it goes off.
const PINCH: f32 = 0.24;

/// The gun's look scale: its `flash`.
fn scale(w: &Weapon) -> f32 {
    w.flash.clamp(0.5, 3.0)
}

/// Across and up off the bore `dir`: the bore's frame for the knots' bearings (the first
/// straight up, as the rails stand).
fn bore_frame(dir: Vec3) -> (Vec3, Vec3) {
    let side = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
    (side, side.cross(dir))
}

/// The knots' bearings round the bore: `n` of them a whole turn apart, the first up.
fn bearing(k: usize, n: usize) -> f32 {
    TAU * 0.25 + TAU * k as f32 / n as f32
}

impl Renderer {
    /// `span` seconds of a knotted gun's charge `f` of the way done: `at` the middle of the
    /// charge, `dir` down the bore.
    pub(super) fn knotted_charge_step(
        &mut self,
        at: Vec3,
        dir: Vec3,
        w: &Weapon,
        f: f32,
        when: f32,
        span: f32,
    ) {
        let s = scale(w);
        let n = w.knots.max(2) as usize;
        let life = span * 2.0;
        let (side, up) = bore_frame(dir);
        let draw = super::smooth(DRAW.0, DRAW.1, f);
        let seed = (w.damage.to_f32() * 0.013).fract() * 40.0;
        let knots: Vec<Vec3> = (0..n)
            .map(|k| {
                let a = bearing(k, n);
                let out = -side * a.cos() + up * a.sin();
                at + (out * KNOT_OUT - dir * KNOT_BACK) * (1.0 - draw)
            })
            .collect();
        // The knots, until they meet; then the one point.
        // Each a small fusing star in a cold lilac halo, so the three read apart.
        if draw < 0.98 {
            let face = KNOT_FACE * s * (0.25 + 0.75 * f.sqrt()) * (1.0 - 0.35 * draw);
            for (k, &p) in knots.iter().enumerate() {
                self.push_lit(
                    STAR,
                    p,
                    Vec3::ZERO,
                    when,
                    life,
                    (face / 0.42, face / 0.42),
                    Vec3::splat(0.8 + 1.4 * f),
                    seed + k as f32 * 7.3,
                );
                let halo = face * 2.4;
                self.push_lit(
                    GLOW,
                    p,
                    Vec3::ZERO,
                    when,
                    life,
                    (halo, halo),
                    LILAC * (0.3 + 0.7 * f),
                    0.0,
                );
            }
        }
        if draw > 0.5 {
            let k = super::smooth(0.5, 1.0, draw);
            let face = POINT_FACE * s * k;
            self.push_lit(
                STAR,
                at,
                Vec3::ZERO,
                when,
                life,
                (face / 0.42, face / 0.42),
                Vec3::splat(1.2 + 1.6 * draw),
                seed + 31.0,
            );
            let halo = face * 2.0;
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (halo, halo),
                WHITE.lerp(LILAC, 0.6) * (0.6 + 1.0 * k),
                0.0,
            );
        }
        // Lilac motes running up each rail from the breech into its knot.
        let reach = (Vec3::from(w.muzzle.to_f32())
            - w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32())))
        .length();
        let motes = (f * 2.0 + self.scatter.unit()) as usize;
        for &p in &knots {
            for _ in 0..motes {
                let back = reach * (0.15 + 0.25 * self.scatter.unit());
                let from = p - dir * back;
                let roll = self.scatter.unit();
                let dot = 0.18 * s;
                self.push_lit(
                    MOTE,
                    from,
                    (p - from) / MOTE_REACH,
                    when + roll * span,
                    MOTE_LIFE,
                    (dot, dot * 0.4),
                    LILAC.lerp(WHITE, roll * 0.5) * (3.0 + 3.0 * f),
                    0.0,
                );
            }
        }
        // The pinch: bent indigo air pulled in tight round the knots as they are drawn in.
        if f > DRAW.0 - 0.1 {
            let g = (KNOT_OUT * 3.6 - KNOT_OUT * 2.0 * draw) * s;
            self.push_lit(
                GLOW,
                at,
                Vec3::ZERO,
                when,
                life,
                (g, g),
                INDIGO * (0.08 + 0.2 * draw),
                0.0,
            );
        }
        // Its light, strobing faster as it fills.
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE.lerp(LILAC, 0.5) * (10.0 + 160.0 * f * f) * s,
            range: (8.0 + 18.0 * f) * s,
            start: when,
            life: life * 1.2,
            pulse: 2.0 + 16.0 * f * f,
        });
    }

    /// A knotted gun fired: `at` the middle of its charge as drawn, `dir` the shell's way
    /// out, `reach` the gun's length from trunnion to charge.
    pub(super) fn knotted_fired(&mut self, w: &Weapon, at: Vec3, dir: Vec3, reach: f32, time: f32) {
        let s = scale(w);
        let n = w.knots.max(2) as usize;
        let (side, up) = bore_frame(dir);
        // A short hard white flash.
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            0.14,
            (4.0 * s, 6.0 * s),
            WHITE * 12.0,
            0.0,
        );
        // Fusion thrown out in a jet between each pair of rails, and down the line of fire.
        for k in 0..n {
            let a = bearing(k, n) + TAU / (2 * n) as f32;
            let out = -side * a.cos() + up * a.sin();
            for _ in 0..5 {
                let spread = Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.15;
                let v = (out + dir * 0.35 + spread).normalize_or(out)
                    * s
                    * (9.0 + 9.0 * self.scatter.unit());
                let thick = s * (0.35 + 0.2 * self.scatter.unit());
                let roll = self.scatter.unit();
                self.push_lit(
                    WISP,
                    at,
                    v * puff::NOVA_WISP_DRAG,
                    time + roll * 0.04,
                    0.5 + 0.3 * roll,
                    (thick, thick * 3.2),
                    Vec3::splat(2.2),
                    0.0,
                );
            }
        }
        self.plasma_jet(at, dir, 3.0 * s, true, time);
        // Heat rising off the three cells on the turret's back, where their caps lift.
        let flat = dir.with_z(0.0).normalize_or(Vec3::X);
        let across = flat.cross(Vec3::Z);
        let trunnion = at - dir * reach;
        let cells = trunnion - flat * reach * 0.27 + Vec3::Z * reach * 0.08;
        for k in 0..18 {
            let lane = (k % 3) as f32 - 1.0;
            let from = cells + across * lane * reach * 0.08;
            let when = time + 0.12 + (k / 3) as f32 * 0.18 + self.scatter.unit() * 0.08;
            let thick = reach * (0.03 + 0.015 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                WISP,
                from,
                Vec3::Z * (2.5 + 2.0 * roll) * puff::NOVA_WISP_DRAG,
                when,
                1.0,
                (thick, thick * 2.6),
                Vec3::splat(0.8 * (1.0 - k as f32 / 22.0)),
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: at,
            color: WHITE * 1600.0 * s,
            range: 40.0 * s,
            start: time,
            life: 0.35,
            pulse: 0.0,
        });
    }

    /// The point a knotted gun's charge met at, let go at `at`: a white flash where it hung.
    pub(super) fn knotted_spent(&mut self, at: Vec3, w: &Weapon, life: f32, time: f32) {
        let face = POINT_FACE * scale(w);
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            time,
            life,
            (face, face * 1.4),
            WHITE * 6.0,
            0.0,
        );
    }

    /// A knotted shell's trail over the stretch `from` to `to` it flies this tick (`size`
    /// the shot's, `(starts, span)` the shares of the tick it flies over, `hang` the seconds
    /// its smoke hangs): a red-hot thread that cools and goes out within a second or two,
    /// red sparks falling away from it, and behind it dark smoke laid down the stretch a
    /// moment after the shell passed, greying as it hangs and lifts. `smoke` counts the
    /// smoke laid this tick over every shell.
    pub(super) fn knotted_trail(
        &mut self,
        from: Vec3,
        to: Vec3,
        size: f32,
        (starts, span): (f32, f32),
        hang: f32,
        smoke: &mut usize,
        time: f32,
    ) {
        let tick = self.tick_seconds.max(0.02);
        let (step, life) = KNOTTED_TRAIL;
        // A deliberate cosmetic cap on one shot's stretch: a tick's flight is far under
        // this many pieces.
        let n = ((from.distance(to) / step).ceil() as usize).clamp(1, 16);
        let dir = (to - from).normalize_or_zero();
        for k in 0..n {
            let (f0, f1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
            let (a, b) = (from.lerp(to, f0), from.lerp(to, f1));
            let when = time + tick * (starts + (span - starts).max(0.0) * f1);
            self.plasma_fx.guns.trail(Trail {
                from: a,
                to: b,
                start: when,
                life,
                width: size * 0.22,
                kind: TrailKind::Knotted,
            });
            if self.scatter.unit() < 0.35 {
                let at = a.lerp(b, self.scatter.unit());
                let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 5.0;
                let roll = self.scatter.unit();
                self.push_lit(
                    MOTE,
                    at,
                    drift - Vec3::Z * (2.0 + 3.0 * roll),
                    when,
                    0.7 + 0.7 * roll,
                    (0.4, 0.14),
                    RED.lerp(HOT, roll * 0.5) * 4.0,
                    0.0,
                );
            }
        }
        // The smoke, a piece every `SMOKE_STEP` metres, laid as the red goes out. A
        // deliberate cosmetic cap: past `MAX_SMOKE` pieces a tick the trail is laid
        // without it.
        let pieces = ((from.distance(to) / SMOKE_STEP).ceil() as usize).clamp(1, 8);
        for k in 0..pieces {
            if *smoke >= MAX_SMOKE {
                break;
            }
            *smoke += 1;
            let f = (k as f32 + 0.5) / pieces as f32;
            let at = from.lerp(to, f);
            let when = time + tick * (starts + (span - starts).max(0.0) * f) + SMOKE_LATE;
            let width = (size * 0.5).clamp(2.0, 6.0);
            let last = hang * (0.85 + 0.3 * self.scatter.unit());
            self.push_puff(
                PUFF_TRAIL,
                at,
                dir * (from.distance(to) / pieces as f32),
                when,
                last,
                (width, width * 3.5),
            );
        }
    }

    /// A knotted shell struck at `at`: the three knots it was gathered from let go again.
    /// Red light and motes drawn in to the point for a moment, then three red bursts round it one
    /// after another that swell into one great red bloom; a fireball climbing out of it,
    /// red plasma thrown up and rolling out low over the ground, globs flung out, the ground
    /// melted into a pool, and dark smoke climbing over it for seconds after. No supernova,
    /// no lightning: it is the Springald's alone.
    pub(super) fn knotted_strike(
        &mut self,
        at: Vec3,
        w: &Weapon,
        impact: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let n = w.knots.max(2) as usize;
        let r = splash.max(10.0) * 0.6 * impact.clamp(0.5, 2.0);
        let centre = at + Vec3::Z * r * 0.12;
        // The pinch: red light drawn in to the point.
        self.push_lit(
            GLOW,
            centre,
            Vec3::ZERO,
            start,
            PINCH,
            (r * 0.9, r * 0.08),
            RED.lerp(HOT, 0.3) * 1.4,
            0.0,
        );
        // Red motes drawn in to it from round it.
        for _ in 0..24 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.6,
            )
            .normalize_or(Vec3::Z);
            let from = centre + out * r * (0.6 + 0.4 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                MOTE,
                from,
                (centre - from) / (PINCH * 1.1),
                start + roll * 0.05,
                PINCH,
                (0.7, 0.3),
                RED.lerp(HOT, 0.4 * roll) * 4.0,
                0.0,
            );
        }
        let bang = start + PINCH;
        // A hard red-white flash.
        self.push_lit(
            GLOW,
            centre,
            Vec3::ZERO,
            bang,
            0.18,
            (r * 0.9, r * 1.6),
            HOT * 4.0,
            0.0,
        );
        // Three bursts round the point, one after another, then the bloom they swell into.
        let turn = self.scatter.unit() * TAU;
        for k in 0..n {
            let a = turn + TAU * k as f32 / n as f32;
            let off = Vec3::new(a.cos(), a.sin(), 0.2) * r * 0.5;
            self.push_lingering(
                BURST,
                centre + off,
                Vec3::ZERO,
                bang + 0.07 * k as f32,
                0.6,
                (r * 0.35, r * 1.2),
                RED.lerp(HOT, 0.35) * 3.4,
                0.0,
                2,
            );
        }
        self.push_lingering(
            BURST,
            centre + Vec3::Z * r * 0.2,
            Vec3::ZERO,
            bang + 0.2,
            1.0,
            (r * 0.8, r * 2.4),
            RED * 2.8,
            0.0,
            3,
        );
        // A fireball climbing out of it, cooling as it goes.
        for k in 0..14 {
            let f = k as f32 / 14.0;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.15;
            let puff = r * (0.35 + 0.15 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lingering(
                WAKE,
                centre,
                (Vec3::Z + lean) * r * (2.0 + 2.4 * f),
                bang + 0.15 + f * 0.3,
                1.0 + 0.5 * roll,
                (puff, puff * 1.6),
                RED.lerp(HOT, 0.5 - f * 0.45) * 3.0,
                0.0,
                2,
            );
        }
        // Red plasma rolling out low over the ground, slowing to a stop.
        for _ in 0..14 {
            let a = self.scatter.unit() * TAU;
            let rise = if ground {
                0.05
            } else {
                self.scatter.signed() * 0.7
            };
            let out = Vec3::new(a.cos(), a.sin(), rise);
            let puff = r * (0.22 + 0.1 * self.scatter.unit());
            let roll = self.scatter.unit();
            let when = bang + 0.12 + 0.1 * self.scatter.unit();
            self.push_lingering(
                WAKE,
                centre + out * r * 0.3,
                out * r * (1.6 + 1.6 * roll),
                when,
                1.0 + 0.5 * roll,
                (puff, puff * 1.4),
                RED.lerp(HOT, 0.25 * roll) * 2.8,
                0.0,
                2,
            );
        }
        // Globs of plasma flung out of it.
        for _ in 0..16 {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.3 + self.scatter.unit() * 0.8,
            )
            .normalize_or(Vec3::Z);
            let blob = r * (0.06 + 0.04 * self.scatter.unit());
            let roll = self.scatter.unit();
            self.push_lit(
                GLOB,
                centre,
                out * r * (1.6 + 2.2 * roll),
                bang + 0.05 * roll,
                1.1 + 0.6 * roll,
                (blob, blob * 0.5),
                RED.lerp(HOT, 0.4) * 3.0,
                0.0,
            );
        }
        if ground {
            self.ground_melt
                .melt(at.truncate(), r * 0.9, 0.7, bang, 14.0);
        }
        // Dark smoke climbing over it, drifting, for seconds after.
        for k in 0..14 {
            let f = k as f32 / 14.0;
            let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.04;
            let puff = r * (0.5 + 0.3 * self.scatter.unit());
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.3;
            let last = 7.0 + 3.0 * self.scatter.unit();
            self.push_puff(
                PUFF_TREE_SMOKE,
                centre + off,
                drift + Vec3::Z * r * (0.6 + 0.9 * f),
                bang + 0.3 + f * 1.6,
                last,
                (puff, puff * 2.6),
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: centre,
            color: RED * 200.0 * impact,
            range: r * 2.0,
            start,
            life: PINCH,
            pulse: 0.0,
        });
        self.plasma_fx.guns.light(Glow {
            pos: centre + Vec3::Z * r * 0.4,
            color: RED.lerp(HOT, 0.3) * 3000.0 * impact,
            range: r * 4.0,
            start: bang,
            life: 1.6,
            pulse: 0.0,
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_knotted_shells_look_is_the_one_the_mirror_writes() {
        assert_eq!(
            crate::gpu_consts::plasma_look::KNOTTED_SHELL,
            mc_sim::mirror::PLASMA_LOOK_KNOTTED_SHELL
        );
    }
}
