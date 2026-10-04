//! The two tines out of the stern: long armoured spears rooted in the hump's flanks and
//! run back past the stern, either side of the drives. Down the inside of each runs the
//! emission dampening channel: a dark recessed strip set with red-lit vents.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;

use super::super::super::kit::{dark_plate, seam, v3};
use super::body::catmull;

/// One point down a tine's spine: where it is (port side), and its half-width across
/// and half-height.
#[derive(Clone, Copy)]
pub(super) struct Knot {
    pub(super) at: Vec3,
    pub(super) half: f32,
    pub(super) tall: f32,
}

pub(super) const fn knot(x: f32, y: f32, z: f32, half: f32, tall: f32) -> Knot {
    Knot {
        at: Vec3::new(x, y, z),
        half,
        tall,
    }
}

/// A tine's shape: its knots root first (the root sunk in the hull), and the stretch of
/// it (fractions along) the channel runs.
pub(super) struct Tine {
    pub(super) knots: &'static [Knot],
    pub(super) channel: (f32, f32),
}

impl Tine {
    /// The tine at `t` (0 the root, 1 the tip), run smoothly through its knots.
    fn at(&self, t: f32) -> Knot {
        let k = self.knots;
        let span = (k.len() - 1) as f32;
        let f = (t.clamp(0.0, 1.0) * span).min(span - 1e-4);
        let i = f as usize;
        let u = f - i as f32;
        let (p, a, c, q) = (
            k[i.saturating_sub(1)],
            k[i],
            k[i + 1],
            k[(i + 2).min(k.len() - 1)],
        );
        let s = |g: fn(&Knot) -> f32| catmull(g(&p), g(&a), g(&c), g(&q), u);
        Knot {
            at: v3(s(|k| k.at.x), s(|k| k.at.y), s(|k| k.at.z)),
            half: s(|k| k.half).max(0.05),
            tall: s(|k| k.tall).max(0.05),
        }
    }

    /// Down the tine at `t`: forward along its length (toward the tip), outboard, up.
    fn frame(&self, t: f32) -> (Vec3, Vec3, Vec3) {
        let ahead = (self.at((t + 0.01).min(1.0)).at - self.at((t - 0.01).max(0.0)).at)
            .normalize_or(-Vec3::X);
        let out = Vec3::Z.cross(ahead).normalize_or(Vec3::Y);
        // Port side: outboard is +y.
        let out = if out.y < 0.0 { -out } else { out };
        let up = out.cross(ahead).normalize_or(Vec3::Z);
        (ahead, out, if up.z < 0.0 { -up } else { up })
    }

    /// The section at `t`: a flat-topped lozenge, its inner face raked in under the
    /// channel.
    fn ring(&self, t: f32) -> Vec<Vec3> {
        let k = self.at(t);
        let (_, out, up) = self.frame(t);
        [
            (0.35, 1.0),
            (0.9, 0.55),
            (1.0, -0.05),
            (0.62, -0.85),
            (-0.4, -1.0),
            (-0.95, -0.3),
            (-0.8, 0.45),
            (-0.3, 1.0),
        ]
        .iter()
        .map(|&(s, u)| k.at + out * (s * k.half) + up * (u * k.tall))
        .collect()
    }

    /// A point on the inner upper face (between its last two section points), `h` off it.
    fn channel_point(&self, t: f32, across: f32, h: f32) -> Vec3 {
        let k = self.at(t);
        let (_, out, up) = self.frame(t);
        let a = k.at + out * (-0.8 * k.half) + up * (0.45 * k.tall);
        let c = k.at + out * (-0.3 * k.half) + up * (1.0 * k.tall);
        let face = (c - a).cross(self.frame(t).0).normalize_or(Vec3::Z);
        let face = if face.dot(up - out) < 0.0 {
            -face
        } else {
            face
        };
        a.lerp(c, across) + face * h
    }
}

/// Both tines (drawn port and mirrored), each with its channel.
pub(super) fn tines(b: &mut MeshBuilder, tine: &Tine) {
    b.mirror_y(|b| one(b, tine));
}

fn one(b: &mut MeshBuilder, tine: &Tine) {
    let along = if b.fine() { 22 } else { 8 };
    let rings: Vec<Vec<Vec3>> = (0..=along)
        .map(|i| tine.ring(i as f32 / along as f32))
        .collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    if b.coarse() {
        return;
    }
    // The channel: a dark strip let into the inner face, and the vents in it.
    let (from, to) = tine.channel;
    let steps = if b.fine() { 16 } else { 5 };
    let strip: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let t = from + (to - from) * i as f32 / steps as f32;
            vec![
                tine.channel_point(t, 0.2, 0.03),
                tine.channel_point(t, 0.8, 0.03),
                tine.channel_point(t, 0.8, 0.12),
                tine.channel_point(t, 0.2, 0.12),
            ]
        })
        .collect();
    seam(b);
    b.loft(&strip, true, true);
    if b.fine() {
        b.paint(GLOW_LASER);
        let vents = 12;
        for i in 0..vents {
            let t = from + (to - from) * (i as f32 + 0.5) / vents as f32;
            let len = (to - from) / vents as f32 * 0.45;
            let (lo, hi) = (t - len * 0.5, t + len * 0.5);
            b.loft(
                &[
                    vec![
                        tine.channel_point(lo, 0.35, 0.13),
                        tine.channel_point(lo, 0.65, 0.13),
                        tine.channel_point(lo, 0.65, 0.17),
                        tine.channel_point(lo, 0.35, 0.17),
                    ],
                    vec![
                        tine.channel_point(hi, 0.35, 0.13),
                        tine.channel_point(hi, 0.65, 0.13),
                        tine.channel_point(hi, 0.65, 0.17),
                        tine.channel_point(hi, 0.35, 0.17),
                    ],
                ],
                true,
                true,
            );
        }
    }
}

/// Far off: the tine in plan as flat triangles from its root out to its widest and back
/// in to its point, each at the height there (port side).
pub(super) fn far(tine: &Tine) -> Vec<([[f32; 2]; 3], f32)> {
    let k = tine.knots;
    let widest = (1..k.len() - 1)
        .max_by(|&a, &b| k[a].at.y.total_cmp(&k[b].at.y))
        .unwrap_or(1);
    let (root, wide, tip) = (k[1], k[widest], k[k.len() - 1]);
    let flat = |p: Knot, off: f32| [p.at.x, p.at.y + off];
    vec![
        (
            [
                flat(root, -root.half),
                flat(wide, wide.half),
                flat(root, root.half),
            ],
            (root.at.z + wide.at.z) * 0.5,
        ),
        (
            [
                flat(wide, -wide.half),
                flat(tip, 0.0),
                flat(wide, wide.half),
            ],
            (wide.at.z + tip.at.z) * 0.5,
        ),
    ]
}
