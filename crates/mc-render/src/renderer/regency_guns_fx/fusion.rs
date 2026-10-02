//! Where a Pinch-fusion shot strikes (`fusion_nova`): a small star going supernova, drawn
//! with the power generators' own supernova pieces (`supernova_fx.rs`, plasma_puffs.wgsl
//! `supernova` and `nova_wisp`), so a fusion gun's strike and a dying generator read as the
//! same light: a blinding white flash, a hollow shell of plasma tearing outward from it,
//! white, then the prism's pinks, cooling to lavender and violet, streamers thrown round
//! its waist, up from it and every way, lightning into the ground, the ground melted into
//! a wide pool, and a small star left burning over the pool for seconds, letting white
//! lightning go into the ground round it while lavender sparkles drift off it. No reds or
//! oranges: it is a star's light, not fire. Sized by the strike: the Sunspear's shell is
//! about a supernova's at tech 1, a bomber's held to [`REACH_MAX`].

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::{Glow, ARC, GLOW, KNOT, LAVENDER, MOTE, ROSE, STAR, VIOLET, WHITE, WISP};
use crate::gpu_consts::puff;
use crate::renderer::Renderer;

/// The farthest a fusion strike's shell gets (metres), a bomb's: a little under a dying
/// tech 3 generator's 75, so a gun's strike never outdoes the plant's death.
const REACH_MAX: f32 = 60.0;
/// The shell's reach a strike of this reach is drawn at full count (more pieces past it
/// would only overlap): the Sunspear's.
const REACH_FULL: f32 = 30.0;

impl Renderer {
    /// A Pinch-fusion shot of ball `size` striking at `at`: the strike's size from its
    /// splash and `impact`; `ground` when it lands on or near the ground (the pool).
    pub(super) fn fusion_nova(
        &mut self,
        at: Vec3,
        impact: f32,
        size: f32,
        splash: f32,
        ground: bool,
        start: f32,
    ) {
        let pool = (size * 1.6).max(splash * 1.3) * impact;
        let reach = (pool * 0.6).clamp(5.0, REACH_MAX);
        // How full the strike is drawn, 0..1, by its reach.
        let k = (reach / REACH_FULL).min(1.0);
        // The ball the shell tears out of, and its middle: stood up off the ground by its
        // face, so the shell opens as a dome with the ground cutting it.
        let face = (reach * 0.1).max(1.0);
        let centre = at + Vec3::Z * face * 0.6;
        let seed = self.scatter.unit() * 97.0;
        // The flash: white beyond the screen, then a broader, slower one.
        self.push_effect(centre.to_array(), start, face * 5.0, 0.3, 9.0, 0.0);
        self.push_effect(centre.to_array(), start + 0.04, face * 3.5, 0.6, 9.0, 0.0);
        self.push_lit(
            GLOW,
            centre,
            Vec3::ZERO,
            start,
            0.22,
            (face * 3.0, face * 4.5),
            WHITE * 14.0,
            0.0,
        );
        // The shell and a slower one inside it; a big strike leaves a faint one lingering.
        let life = 1.4 + 2.2 * k;
        self.nova_shell(centre, start, life, (face, reach), 1.7, seed);
        self.nova_shell(
            centre,
            start + 0.08,
            life * 1.25,
            (face * 0.8, reach * 0.62),
            1.0,
            seed + 3.7,
        );
        if k > 0.5 {
            self.nova_shell(
                centre,
                start + 0.25,
                life * 1.8,
                (face, reach * 0.85),
                0.5,
                seed + 6.1,
            );
        }
        self.fusion_streamers(centre, face, reach, k, start);
        self.fusion_spray(at, centre, face, reach, k, start);
        if ground {
            self.ground_melt
                .melt(at.truncate(), pool * 0.65, start, 18.0);
        }
        // Its light on everything round: white, then lavender as the shell spreads.
        self.plasma_fx.guns.light(Glow {
            pos: centre + Vec3::Z * face * 2.0,
            color: WHITE * 4000.0 * impact,
            range: reach * 3.0,
            start,
            life: 0.9,
            pulse: 0.0,
        });
        self.plasma_fx.guns.light(Glow {
            pos: centre + Vec3::Z * reach * 0.3,
            color: LAVENDER * 500.0 * impact,
            range: reach * 2.2,
            start: start + 0.25,
            life,
            pulse: 0.0,
        });
        self.fusion_knot(at, pool * 0.7, impact, start);
    }

    /// The streamers a fusion strike throws (`nova_wisp`): a loose band round its waist,
    /// tilted its own way, a jet up from it, and thinner ones flung every way.
    fn fusion_streamers(&mut self, centre: Vec3, face: f32, reach: f32, k: f32, start: f32) {
        let tilt = 0.35 * self.scatter.signed();
        let (ts, tc) = tilt.sin_cos();
        let yaw = self.scatter.unit() * TAU;
        let around = (10.0 + 26.0 * k) as usize;
        for i in 0..around {
            let a = (i as f32 + 0.5 * self.scatter.unit()) * TAU / around as f32;
            let flat = Vec2::from_angle(a + yaw);
            let off = 0.3 * self.scatter.signed();
            let dir = Vec3::new(flat.x, flat.y * tc, (flat.y * ts + off).max(-0.05)).normalize();
            let go = reach * (0.55 + 0.3 * self.scatter.unit());
            let size = face * (0.5 + 0.4 * self.scatter.unit());
            let (roll0, roll1) = (self.scatter.unit(), self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face,
                dir * go * puff::NOVA_WISP_DRAG,
                start + 0.03 * roll0,
                1.4 + 1.4 * k + 0.4 * roll1,
                (size, size * 3.2),
                Vec3::splat(1.4),
                0.0,
            );
        }
        // A jet up out of it, laid over a quarter of a second, the first highest.
        let jet = (8.0 + 14.0 * k) as usize;
        for i in 0..jet {
            let f = i as f32 / jet as f32;
            let lean = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * 0.08;
            let dir = (Vec3::Z + lean).normalize();
            let go = reach * (1.1 - 0.4 * f) * (0.9 + 0.2 * self.scatter.unit());
            let size = face * (0.45 + 0.3 * self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face,
                dir * go * puff::NOVA_WISP_DRAG,
                start + 0.25 * f,
                1.3 + 1.2 * k,
                (size, size * 2.6),
                Vec3::splat(2.4),
                0.0,
            );
        }
        for _ in 0..(12.0 + 28.0 * k) as usize {
            let dir = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit() * 0.9 + 0.1,
            )
            .normalize_or(Vec3::Z);
            let go = reach * (0.4 + 0.6 * self.scatter.unit());
            let size = face * (0.22 + 0.16 * self.scatter.unit());
            self.push_lit(
                WISP,
                centre + dir * face * 0.8,
                dir * go * puff::NOVA_WISP_DRAG,
                start,
                0.9 + 0.9 * k,
                (size, size * 2.4),
                Vec3::splat(2.8),
                0.0,
            );
        }
    }

    /// What a fusion strike throws out of itself besides its streamers: white lightning
    /// into the ground round it as it opens, and sparks, white going rose and lavender.
    fn fusion_spray(&mut self, at: Vec3, centre: Vec3, face: f32, reach: f32, k: f32, start: f32) {
        for _ in 0..(6.0 + 8.0 * k) as usize {
            let a = self.scatter.unit() * TAU;
            let r = reach * (0.3 + 0.5 * self.scatter.unit());
            let to = at + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            let to = to.with_z(self.ground_height(to.truncate()) + 0.2);
            let when = start + self.scatter.unit() * 0.25;
            let tint = WHITE.lerp(LAVENDER, self.scatter.unit() * 0.6);
            self.push_lit(
                ARC,
                centre,
                to - centre,
                when,
                0.16,
                (face * 0.4, face * 0.4),
                tint * 8.0,
                0.0,
            );
        }
        for _ in 0..(30.0 + 50.0 * k) as usize {
            let out = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.2 + self.scatter.unit() * 0.8,
            )
            .normalize_or(Vec3::Z);
            let speed = reach * (1.2 + 2.4 * self.scatter.unit());
            let life = 0.9 + self.scatter.unit() * 1.2;
            let roll = self.scatter.unit();
            let tint = if roll < 0.5 {
                WHITE.lerp(ROSE, roll * 2.0)
            } else {
                ROSE.lerp(LAVENDER, roll * 2.0 - 1.0)
            };
            self.push_lit(
                MOTE,
                centre,
                out * speed,
                start,
                life,
                (0.6, 0.2),
                tint * 5.0,
                0.0,
            );
        }
    }

    /// The small star a fusion strike leaves burning over its pool (`s` the strike's size)
    /// for `KNOT` seconds (plasma_puffs.wgsl `star_core`, as a power generator's):
    /// shrinking as it slowly lets white lightning go into the ground round it, lavender
    /// sparkles drifting up off its edges.
    fn fusion_knot(&mut self, at: Vec3, s: f32, impact: f32, start: f32) {
        let knot = at + Vec3::Z * (1.0 + s * 0.12);
        let seed = self.scatter.unit() * 61.0;
        // Laid every half second, each lasting a second, so their tents add up to a
        // steady star that shrinks and dims.
        let pieces = (KNOT / 0.5) as usize;
        for i in 0..pieces {
            let f = i as f32 / pieces as f32;
            let face = s * 0.22 * (1.0 - 0.65 * f);
            let across = face / 0.42;
            self.push_lit(
                STAR,
                knot,
                Vec3::ZERO,
                start + 0.2 + i as f32 * 0.5,
                1.0,
                (across, across),
                Vec3::splat(2.2 * (1.0 - 0.6 * f)),
                seed,
            );
        }
        let strokes = 16;
        for i in 0..strokes {
            let when = start + 0.4 + KNOT * (i as f32 + self.scatter.unit()) / strokes as f32;
            let a = self.scatter.unit() * TAU;
            let r = s * (0.5 + 0.8 * self.scatter.unit());
            let to = at + Vec3::new(a.cos() * r, a.sin() * r, 0.0);
            let to = to.with_z(self.ground_height(to.truncate()) + 0.2);
            let life = 0.14 + 0.1 * self.scatter.unit();
            let tint = WHITE.lerp(LAVENDER, self.scatter.unit());
            self.push_lit(
                ARC,
                knot,
                to - knot,
                when,
                life,
                (s * 0.12, s * 0.12),
                tint * 7.0,
                0.0,
            );
        }
        for _ in 0..45 {
            let a = self.scatter.unit() * TAU;
            let r = s * (0.3 + 0.5 * self.scatter.unit());
            let from = at + Vec3::new(a.cos() * r, a.sin() * r, 0.4 + self.scatter.unit() * 2.0);
            let drift = Vec3::new(a.cos(), a.sin(), 1.5 + self.scatter.unit())
                * (1.0 + self.scatter.unit() * 2.0);
            let when = start + 0.3 + self.scatter.unit() * KNOT;
            let dot = 0.45 + 0.45 * self.scatter.unit();
            let life = 1.2 + self.scatter.unit();
            let tint = LAVENDER.lerp(VIOLET, self.scatter.unit());
            self.push_lit(
                MOTE,
                from,
                drift,
                when,
                life,
                (dot, dot * 0.5),
                tint * 5.0,
                0.0,
            );
        }
        self.plasma_fx.guns.light(Glow {
            pos: knot,
            color: ROSE.lerp(WHITE, 0.4) * 150.0 * impact,
            range: s * 3.0,
            start: start + 0.3,
            life: KNOT,
            pulse: 7.0,
        });
    }
}
