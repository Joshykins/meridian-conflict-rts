//! Face frames: how each face's vertices get their [`MeshVertex::face`] numbers, the
//! frame the shaders lay fitted detail out by (surface.wgsl, regency.wgsl).
//!
//! [`MeshVertex::face`]: crate::MeshVertex::face

use std::f32::consts::{PI, TAU};

use glam::{Vec2, Vec3};

use super::newell_normal;

/// How a face's vertices get their [`MeshVertex::face`] frame.
#[derive(Clone, Copy)]
pub(super) enum Framing {
    /// From the face's own outline.
    Flat,
    /// From the tube the face is a facet of.
    Tube(Tube),
    /// None: the shader draws no fitted detail on it.
    Bare,
}

/// A face's own coordinate system: the smallest rectangle round its outline.
pub(super) struct FaceFrame {
    s: Vec3,
    t: Vec3,
    /// Middle of the rectangle, in (s, t).
    centre: Vec2,
    half: Vec2,
    /// A tube: s is the way round, and `centre.x` the face's own angle.
    tube: Option<Tube>,
}

impl FaceFrame {
    /// The frame of a planar polygon, or none for one that fills too little of
    /// its rectangle for an outline along the rectangle to mean anything.
    pub(super) fn flat(points: &[Vec3], normal: Vec3) -> Option<FaceFrame> {
        let extent = |u: Vec3, v: Vec3| {
            points.iter().fold(
                (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
                |(lo, hi), p| {
                    let q = Vec2::new(p.dot(u), p.dot(v));
                    (lo.min(q), hi.max(q))
                },
            )
        };
        // Level first: on a wall the frame that runs level, on a deck the one square to
        // the model. Lines drawn "horizontal" then are, even on a gable whose longest
        // edge slopes. A tilted frame has to be a much better fit to win (a raked beam).
        let level = if normal.z.abs() < 0.9 {
            Vec3::Z.cross(normal).normalize()
        } else {
            normal.cross(Vec3::X.cross(normal)).normalize()
        };
        let (lo, hi) = extent(level, normal.cross(level));
        let level_area = (hi.x - lo.x) * (hi.y - lo.y);
        let mut best: Option<(f32, Vec3, Vec3)> =
            Some((level_area * 0.7, level, normal.cross(level)));
        // The smallest bounding rectangle of a convex outline lies along one of its edges.
        for (i, &p) in points.iter().enumerate() {
            let Some(u) = (points[(i + 1) % points.len()] - p).try_normalize() else {
                continue;
            };
            let v = normal.cross(u);
            let (lo, hi) = extent(u, v);
            let area = (hi.x - lo.x) * (hi.y - lo.y);
            // Strictly smaller by a margin, so a rectangle keeps its first edge and mirrored halves agree.
            if best.is_none_or(|(a, ..)| area < a * 0.999) {
                best = Some((area, u, v));
            }
        }
        let (_, a, b) = best?;
        let (lo, hi) = extent(a, b);
        let area = (hi.x - lo.x) * (hi.y - lo.y);
        let covered = newell_normal(points).length() * 0.5;
        if area <= 1e-8 || covered < area * 0.62 {
            return None;
        }
        // On a wall t runs up it, so "horizontal" means the same thing on every
        // face; on a deck s runs the way the model faces.
        let (s, t) = if normal.z.abs() < 0.9 {
            let t = if a.z.abs() >= b.z.abs() { a } else { b };
            let t = if t.z < 0.0 { -t } else { t };
            (t.cross(normal), t)
        } else {
            let s = if a.x.abs() >= b.x.abs() { a } else { b };
            let s = if s.x < 0.0 { -s } else { s };
            (s, normal.cross(s))
        };
        let (lo, hi) = extent(s, t);
        Some(FaceFrame {
            s,
            t,
            centre: (lo + hi) * 0.5,
            half: (hi - lo) * 0.5,
            tube: None,
        })
    }

    pub(super) fn at(&self, p: Vec3) -> [f32; 4] {
        if let Some(tube) = self.tube {
            let around = tube.angle(p) - self.centre.x;
            let around = self.centre.x + (around + PI).rem_euclid(TAU) - PI;
            return [
                around * tube.radius,
                (p - tube.origin).dot(tube.axis) - tube.length * 0.5,
                -self.half.x,
                self.half.y,
            ];
        }
        [
            p.dot(self.s) - self.centre.x,
            p.dot(self.t) - self.centre.y,
            self.half.x,
            self.half.y,
        ]
    }

    /// A byte of randomness that a face keeps across levels of detail and
    /// shares with its mirror image.
    pub(super) fn seed(&self) -> u32 {
        let q = |v: f32| (v * 8.0).round() as i32 as u32;
        let middle = match self.tube {
            Some(tube) => tube.origin,
            None => self.s * self.centre.x + self.t * self.centre.y,
        };
        let mut h = 0x9E37_79B9u32;
        for v in [
            q(middle.x),
            q(middle.y.abs()),
            q(self.half.x),
            q(self.half.y),
        ] {
            h = (h ^ v).wrapping_mul(0x85EB_CA6B);
            h ^= h >> 13;
        }
        (h >> 8) & 0xFF
    }
}

/// A convex face's own edges, for the edge form of [`MeshVertex::face`]: distance to
/// a straight edge is linear over the face, so the GPU interpolates it exactly.
pub(super) struct FaceEdges {
    /// Each edge as a point on it and the way into the face; the longest four.
    lines: Vec<(Vec3, Vec3)>,
    pub(super) seed: u32,
}

impl FaceEdges {
    /// None for a face that is not convex: there the nearest edge's line is not the
    /// nearest edge.
    pub(super) fn of(points: &[Vec3], normal: Vec3) -> Option<FaceEdges> {
        let n = points.len();
        let centre = points.iter().copied().sum::<Vec3>() / n as f32;
        let mut lines: Vec<(f32, Vec3, Vec3)> = Vec::with_capacity(n);
        for (i, &a) in points.iter().enumerate() {
            let b = points[(i + 1) % n];
            let Some(along) = (b - a).try_normalize() else {
                continue;
            };
            let mut inward = normal.cross(along);
            if (centre - a).dot(inward) < 0.0 {
                inward = -inward;
            }
            if points.iter().any(|&p| (p - a).dot(inward) < -1e-3) {
                return None;
            }
            lines.push((a.distance(b), a, inward));
        }
        if lines.len() < 3 {
            return None;
        }
        // The longest first, stably, so mirrored faces keep the same four.
        lines.sort_by(|x, y| y.0.total_cmp(&x.0));
        lines.truncate(4);
        // Mirror twins share a seed: from the middle (y folded) and the size.
        let q = |v: f32| (v * 8.0).round() as i32 as u32;
        let area = newell_normal(points).length() * 0.5;
        let mut h = 0x2545_F491u32;
        for v in [q(centre.x), q(centre.y.abs()), q(centre.z), q(area)] {
            h = (h ^ v).wrapping_mul(0x85EB_CA6B);
            h ^= h >> 13;
        }
        Some(FaceEdges {
            lines: lines.into_iter().map(|(_, a, i)| (a, i)).collect(),
            seed: (h >> 8) & 0xFF,
        })
    }

    pub(super) fn at(&self, p: Vec3) -> [f32; 4] {
        // A triangle repeats its first edge as its fourth.
        let d = |k: usize| {
            let (a, inward) = self.lines[k % self.lines.len()];
            (p - a).dot(inward).max(0.0)
        };
        [
            d(0),
            d(1),
            d(2),
            -(d(3) + crate::gpu_consts::face_edges::BIAS),
        ]
    }
}

/// The shared frame of a tube's side facets.
#[derive(Clone, Copy)]
pub(super) struct Tube {
    origin: Vec3,
    axis: Vec3,
    /// Where the angle round the axis is zero.
    zero: Vec3,
    radius: f32,
    length: f32,
}

impl Tube {
    /// From a loft's rings (already in final space), or none for one with no length.
    pub(super) fn around(rings: &[Vec<Vec3>]) -> Option<Tube> {
        let middle = |ring: &Vec<Vec3>| ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
        let (origin, end) = (middle(&rings[0]), middle(&rings[rings.len() - 1]));
        let length = origin.distance(end);
        let axis = (end - origin).try_normalize()?;
        let mut radius = 0.0;
        for ring in rings {
            let c = middle(ring);
            radius += ring.iter().map(|p| p.distance(c)).sum::<f32>() / ring.len() as f32;
        }
        let radius = radius / rings.len() as f32;
        let reference = if axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
        let zero = (reference - axis * reference.dot(axis)).try_normalize()?;
        (radius > 1e-4).then_some(Tube {
            origin,
            axis,
            zero,
            radius,
            length,
        })
    }

    pub(super) fn angle(&self, p: Vec3) -> f32 {
        let r = p - self.origin;
        r.dot(self.axis.cross(self.zero)).atan2(r.dot(self.zero))
    }

    pub(super) fn frame(self, points: &[Vec3]) -> FaceFrame {
        let middle = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        FaceFrame {
            s: Vec3::ZERO,
            t: self.axis,
            centre: Vec2::new(self.angle(middle), 0.0),
            half: Vec2::new(PI * self.radius, self.length * 0.5),
            tube: Some(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Affine3A, Vec3};

    use crate::builder::MeshBuilder;
    use crate::gpu_consts::face_edges::BIAS;
    use crate::{material, pattern, MeshVertex};

    fn triangle(look: u32, y: f32) -> Vec<MeshVertex> {
        let mut b = MeshBuilder::new(0, Affine3A::IDENTITY);
        b.paint(material::PLATING_DARK).pattern(look);
        // Wound so the face looks up +z on either side of the model's middle.
        let (a, c) = (
            Vec3::new(0.0, y, 0.0),
            Vec3::new(0.0, y + 3.0 * y.signum(), 0.0),
        );
        let tip = Vec3::new(4.0, y + 1.0 * y.signum(), 0.0);
        if y > 0.0 {
            b.face(&[a, tip, c]);
        } else {
            b.face(&[a, c, tip]);
        }
        b.finish().vertices
    }

    /// A Regency triangle carries the distances to its own three edges, and its
    /// mirror image the same seed; anyone else's triangle carries no frame at all.
    #[test]
    fn a_regency_triangle_knows_its_own_edges() {
        let left = triangle(pattern::EMBER, 2.0);
        assert_eq!(left.len(), 3);
        for v in &left {
            let [d0, d1, d2, w] = v.face;
            assert!(w < 0.0, "{v:?}");
            let d = [d0, d1, d2, -w - BIAS];
            // Each corner lies on two of the edges and the third's altitude away.
            assert_eq!(
                d[..3].iter().filter(|&&x| x.abs() < 1e-4).count(),
                2,
                "{v:?}"
            );
            assert!(d[..3].iter().all(|&x| x >= 0.0), "{v:?}");
            // A triangle repeats its first edge as its fourth.
            assert!((d[3] - d[0]).abs() < 1e-4, "{v:?}");
        }
        let right = triangle(pattern::EMBER, -2.0);
        assert_eq!(left[0].surface >> 8, right[0].surface >> 8);
        for v in triangle(pattern::GENERIC, 2.0) {
            assert_eq!(v.face, [0.0; 4]);
        }
    }
}
