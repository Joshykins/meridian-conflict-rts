//! A shard: one of the engine's great blades. A faceted core of dark graphite works, keeled
//! along its back and drawn to a sharp edge either side, its point leaning off to one side
//! like a broken blade. Dark plates are laid up its back in overlapping courses, each
//! pitched so its upper end stands proud, its point a spike; a graphite rib shows under
//! each plate's lower edge. Its inner faces, turned to the well, carry glazed plates the
//! held energy shows through (`kit::nanite_plate`). It stands in a plated boot, a hinge
//! drum across its back.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::kit::{dark_plate, metal, nanite_plate};
use super::super::machine::{armour, collar, swept, Frame};
use super::jitter;

/// How a shard narrows up its length: `(t, width, sideways)`, `t` from foot (0) to point
/// (1); the width as a share of its foot's, and how far its middle has moved toward the
/// side its point leans to, as a share of its foot's half width.
const PROFILE: [(f32, f32, f32); 5] = [
    (0.0, 1.0, 0.0),
    (0.24, 0.97, 0.02),
    (0.52, 0.8, 0.12),
    (0.8, 0.44, 0.26),
    (1.0, 0.0, 0.0),
];

/// The corners of a shard's section: keel, back shoulders, edges, inner ridge. Indices into
/// [`Shard::ring`].
const KEEL: usize = 0;
const SHOULDER: [usize; 2] = [1, 5];
const EDGE: [usize; 2] = [2, 4];
const INNER: usize = 3;

#[derive(Clone, Copy, Debug)]
pub(super) struct Shard {
    /// The middle of its foot, and its point.
    pub(super) root: Vec3,
    pub(super) tip: Vec3,
    /// The way its back faces (out from the well).
    pub(super) back: Vec3,
    /// Half its width and half its depth at the foot.
    pub(super) half: f32,
    pub(super) depth: f32,
    /// Which side its point leans to, -1 to 1.
    pub(super) skew: f32,
    /// Courses of plate up its back.
    pub(super) plates: usize,
    pub(super) seed: u32,
}

impl Shard {
    /// Along it, across it, and out of its back.
    fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let u = (self.tip - self.root).normalize();
        let n = (self.back - u * self.back.dot(u)).normalize();
        (u, n.cross(u), n)
    }

    /// Its width and sideways shift `t` of the way up (from [`PROFILE`]).
    fn profile(t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        for w in PROFILE.windows(2) {
            let ((t0, w0, s0), (t1, w1, s1)) = (w[0], w[1]);
            if t <= t1 {
                let k = (t - t0) / (t1 - t0);
                return (w0 + (w1 - w0) * k, s0 + (s1 - s0) * k);
            }
        }
        (0.0, 0.0)
    }

    /// The middle of its section `t` of the way up.
    pub(super) fn centre(&self, t: f32) -> Vec3 {
        let (_, v, _) = self.axes();
        let (_, side) = Self::profile(t);
        self.root.lerp(self.tip, t) + v * (self.skew * self.half * side)
    }

    /// Its section `t` of the way up: keel, shoulder, edge, inner ridge, edge, shoulder.
    pub(super) fn ring(&self, t: f32) -> Vec<Vec3> {
        let (_, v, n) = self.axes();
        let (wk, _) = Self::profile(t);
        let c = self.centre(t);
        let w = self.half * wk;
        let d = self.depth * if wk > 0.0 { 0.3 + 0.7 * wk } else { 0.0 };
        let ring = [
            c + n * d,
            c + v * (w * 0.6) + n * (d * 0.5),
            c + v * w - n * (d * 0.12),
            c - n * (d * 0.62),
            c - v * w - n * (d * 0.12),
            c - v * (w * 0.6) + n * (d * 0.5),
        ];
        // The foot is cut level, where it stands.
        let level = if t <= 0.0 { Some(self.root.z) } else { None };
        ring.iter()
            .map(|p| level.map_or(*p, |z| p.with_z(z)))
            .collect()
    }

    /// The point of its edge on `side` (0 or 1) at height `z`.
    pub(super) fn edge_at(&self, z: f32, side: usize) -> Vec3 {
        let t = (z - self.root.z) / (self.tip.z - self.root.z);
        self.ring(t)[EDGE[side]]
    }

    /// The middle of its back (the keel) `t` of the way up.
    pub(super) fn keel(&self, t: f32) -> Vec3 {
        self.ring(t)[KEEL]
    }

    /// The edge nearer `to` at height `z`.
    pub(super) fn edge_toward(&self, z: f32, to: Vec3) -> Vec3 {
        let a = self.edge_at(z, 0);
        let c = self.edge_at(z, 1);
        if a.distance(to) < c.distance(to) {
            a
        } else {
            c
        }
    }

    /// The whole shard at the builder's level of detail.
    pub(super) fn build(&self, b: &mut MeshBuilder) {
        b.with_facets(|b| {
            self.core(b);
            if b.coarse() {
                return;
            }
            self.boot(b);
            self.back_plates(b);
            self.flank_plates(b);
            self.inner_plates(b);
        });
    }

    /// Far off: three faces from its foot to its point.
    pub(super) fn coarse(&self, b: &mut MeshBuilder) {
        let r = self.ring(0.0);
        dark_plate(b);
        b.loft(
            &[vec![r[KEEL], r[EDGE[0]], r[EDGE[1]]], vec![self.tip; 3]],
            false,
            false,
        );
    }

    /// The core: graphite works, every plate laid over it.
    fn core(&self, b: &mut MeshBuilder) {
        if b.coarse() {
            return self.coarse(b);
        }
        metal(b);
        let rings: Vec<Vec<Vec3>> = PROFILE.iter().map(|&(t, _, _)| self.ring(t)).collect();
        b.loft(&rings, true, false);
    }

    /// The plated boot it stands in, and the hinge drum across its back.
    fn boot(&self, b: &mut MeshBuilder) {
        let (u, v, n) = self.axes();
        let foot = self.ring(0.0);
        let lip = self.ring(0.07);
        let out = |p: &Vec3, c: Vec3, k: f32| c + (*p - c) * k;
        let c0 = self.centre(0.0);
        let c1 = self.centre(0.07);
        dark_plate(b);
        b.loft(
            &[
                foot.iter().map(|p| out(p, c0, 1.22)).collect(),
                lip.iter().map(|p| out(p, c1, 1.12)).collect(),
            ],
            false,
            true,
        );
        // A red optic looking out of the boot's back.
        if b.fine() {
            let k = self.keel(0.035) + n * (self.depth * 0.2);
            b.paint(PLATING_DARK).pattern(pattern::EMBER_SLOT);
            let f = Frame::new(k, u, n);
            armour(
                b,
                &f,
                &[
                    [-self.depth * 0.2, -self.half * 0.08],
                    [-self.depth * 0.2, self.half * 0.08],
                    [self.depth * 0.25, self.half * 0.06],
                    [self.depth * 0.25, -self.half * 0.06],
                ],
                0.8,
            );
        }
        // The hinge it would open on.
        let hinge = self.keel(0.1) + n * (self.depth * 0.25);
        if !b.coarse() {
            collar(b, hinge, v, self.depth * 0.5, self.half * 1.3);
            dark_plate(b);
            for s in [-1.0, 1.0] {
                let cheek = hinge + v * (s * self.half * 0.72);
                let reach = (self.depth * 1.6).min((cheek.z - self.depth * 0.8) / u.z.max(0.1));
                b.beam(
                    cheek - u * reach + n * (self.depth * 0.1),
                    cheek + u * (self.depth * 0.9),
                    glam::Vec2::new(self.depth * 1.2, self.half * 0.16),
                    glam::Vec2::new(self.depth * 0.8, self.half * 0.12),
                );
            }
        }
    }

    /// A plate on the face between corners `i` and `j` of the section, from `t0` to `t1`,
    /// standing `lift` off it and pitched up by `pitch`, `thick` deep.
    #[expect(
        clippy::too_many_arguments,
        reason = "a plate's place on the blade and its cut, all used"
    )]
    fn plate(
        &self,
        b: &mut MeshBuilder,
        (i, j): (usize, usize),
        (t0, t1): (f32, f32),
        lift: f32,
        pitch: f32,
        thick: f32,
        tip: f32,
    ) -> Option<(Vec3, Vec3)> {
        let r0 = self.ring(t0);
        let r1 = self.ring(t1);
        let (a, c) = (r0[i], r0[j]);
        let mid0 = (a + c) * 0.5;
        let mid1 = (r1[i] + r1[j]) * 0.5;
        let along = mid1 - mid0;
        let width = (a - c).length().min((r1[i] - r1[j]).length() * 1.6);
        if width < 1.0 || along.length() < 1.0 {
            return None;
        }
        let u = along.normalize();
        let mut nf = (c - a).cross(u).normalize();
        if nf.dot(mid0 - self.centre(t0)) < 0.0 {
            nf = -nf;
        }
        let f = Frame::new(mid0 + nf * lift, u + nf * pitch, nf);
        armour(
            b,
            &f,
            &swept(along.length() * 1.18, width * 0.47, tip, 0.6),
            thick,
        );
        Some((a + nf * (lift * 0.5), c + nf * (lift * 0.5)))
    }

    /// Courses of plate up its back, either side of the keel, each over a graphite rib.
    fn back_plates(&self, b: &mut MeshBuilder) {
        let count = if b.fine() {
            self.plates
        } else {
            self.plates.div_ceil(2)
        };
        let thick = (self.depth * 0.16).clamp(0.6, 1.6);
        for k in 0..count {
            let t0 = 0.1 + 0.78 * k as f32 / count as f32;
            let t1 = 0.1 + 0.78 * (k + 1) as f32 / count as f32;
            for (s, &sh) in SHOULDER.iter().enumerate() {
                let rim = k + 1 == count && (s as u32 + self.seed).is_multiple_of(2);
                if rim {
                    b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
                } else {
                    dark_plate(b);
                }
                let tip = 0.5 * jitter(self.seed * 31 + k as u32 * 7 + s as u32);
                let lean = 0.035 + 0.02 * jitter(self.seed * 17 + k as u32);
                let rib = self.plate(b, (KEEL, sh), (t0, t1), 0.25, lean, thick, tip);
                if let (Some((a, c)), true) = (rib, b.fine()) {
                    metal(b);
                    let sides = b.sides(6);
                    b.cylinder_between(a, c, thick * 0.55, thick * 0.55, sides);
                }
            }
        }
    }

    /// Plates down its flanks, between the back and the edge, staggered against the back's.
    fn flank_plates(&self, b: &mut MeshBuilder) {
        let count = if b.fine() { self.plates + 1 } else { 1 };
        let thick = (self.depth * 0.12).clamp(0.5, 1.2);
        dark_plate(b);
        for k in 0..count {
            let t0 = 0.05 + 0.7 * (k as f32 + 0.5) / count as f32;
            let t1 = t0 + 0.72 / count as f32;
            for &sh in &SHOULDER {
                let e = if sh == SHOULDER[0] { EDGE[0] } else { EDGE[1] };
                self.plate(b, (sh, e), (t0, t1), 0.2, 0.05, thick, 0.7);
            }
        }
    }

    /// Glazed plates on its inner faces, the held energy showing through them.
    fn inner_plates(&self, b: &mut MeshBuilder) {
        let thick = (self.depth * 0.1).clamp(0.5, 1.0);
        nanite_plate(b);
        let spans: &[(f32, f32)] = if b.fine() {
            &[(0.08, 0.4), (0.44, 0.74)]
        } else {
            &[(0.08, 0.6)]
        };
        for &span in spans {
            for e in EDGE {
                self.plate(b, (e, INNER), span, 0.15, 0.0, thick, 0.0);
            }
        }
    }
}
