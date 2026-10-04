//! The destroyer's hull as a surface: stations along x joined smoothly, each a
//! superellipse section, so every plate, fin and drive can be laid exactly on it.
//!
//! A point on the hull is `(x, phi)`: `phi` in degrees round the section, 0 at the port
//! chine (the widest line), 90 on the spine, -90 on the keel, 180 the starboard chine.

use glam::Vec3;

use crate::builder::MeshBuilder;

use super::super::super::kit::{seam, v3};

/// One station of the hull: its half-beam at the chine, and the keel's, the chine's and
/// the spine's heights.
#[derive(Clone, Copy, Debug)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) w: f32,
    pub(super) keel: f32,
    pub(super) chine: f32,
    pub(super) spine: f32,
}

pub(super) const fn st(x: f32, w: f32, keel: f32, chine: f32, spine: f32) -> Station {
    Station {
        x,
        w,
        keel,
        chine,
        spine,
    }
}

/// The hull: stations stern first, the section's fullness above and below the chine
/// (2 an ellipse, more fuller-shouldered), and the stations its far outline keeps.
pub(super) struct Body {
    pub(super) stations: &'static [Station],
    pub(super) upper: f32,
    pub(super) lower: f32,
    pub(super) far: &'static [usize],
}

pub(super) fn catmull(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t2 * t)
}

impl Body {
    /// Stern and bow.
    pub(super) fn span(&self) -> (f32, f32) {
        (self.stations[0].x, self.stations[self.stations.len() - 1].x)
    }

    /// The station at `x`, run smoothly through its neighbours.
    pub(super) fn at(&self, x: f32) -> Station {
        let s = self.stations;
        let (lo, hi) = self.span();
        let x = x.clamp(lo, hi);
        let i = s
            .windows(2)
            .position(|w| x <= w[1].x)
            .unwrap_or(s.len() - 2);
        let (a, c) = (s[i], s[i + 1]);
        let (p, q) = (s[i.saturating_sub(1)], s[(i + 2).min(s.len() - 1)]);
        let t = ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0);
        let f = |g: fn(&Station) -> f32| catmull(g(&p), g(&a), g(&c), g(&q), t);
        let chine = f(|s| s.chine);
        Station {
            x,
            w: f(|s| s.w).max(0.2),
            keel: f(|s| s.keel).min(chine - 0.2),
            chine,
            spine: f(|s| s.spine).max(chine + 0.2),
        }
    }

    /// The hull's surface at `x`, `phi` degrees round.
    pub(super) fn point(&self, x: f32, phi: f32) -> Vec3 {
        let s = self.at(x);
        let (sn, cs) = phi.to_radians().sin_cos();
        let (n, h) = if sn >= 0.0 {
            (self.upper, s.spine - s.chine)
        } else {
            (self.lower, s.chine - s.keel)
        };
        let e = 2.0 / n;
        v3(
            x,
            s.w * cs.signum() * cs.abs().powf(e),
            s.chine + h * sn.signum() * sn.abs().powf(e),
        )
    }

    /// The outward normal at `x`, `phi`.
    pub(super) fn normal(&self, x: f32, phi: f32) -> Vec3 {
        let (lo, hi) = self.span();
        let (xa, xb) = ((x - 0.5).max(lo), (x + 0.5).min(hi));
        let along = self.point(xb, phi) - self.point(xa, phi);
        let round = self.point(x, phi + 0.5) - self.point(x, phi - 0.5);
        let n = round.cross(along).normalize_or(Vec3::Z);
        let p = self.point(x, phi);
        let s = self.at(x);
        if n.dot(v3(0.0, p.y, p.z - s.chine)) < 0.0 {
            -n
        } else {
            n
        }
    }

    /// `h` metres out from the surface at `x`, `phi`.
    pub(super) fn out(&self, x: f32, phi: f32, h: f32) -> Vec3 {
        self.point(x, phi) + self.normal(x, phi) * h
    }

    /// `h` metres out from the surface at `x`, `phi`, staying in the station's plane:
    /// a plate's sections stay flat, so its end caps do too where the hull narrows fast.
    pub(super) fn across(&self, x: f32, phi: f32, h: f32) -> Vec3 {
        let n = self.normal(x, phi);
        self.point(x, phi) + v3(0.0, n.y, n.z).normalize_or(Vec3::Z) * h
    }

    /// The `phi` at which the upper surface stands `y` out from the centreline.
    pub(super) fn phi_up(&self, x: f32, y: f32) -> f32 {
        let q = (y / self.at(x).w).clamp(0.0, 1.0);
        q.powf(self.upper * 0.5).acos().to_degrees()
    }

    /// The hull itself, in the seam colour: what shows between the plates.
    pub(super) fn hull(&self, b: &mut MeshBuilder) {
        let (lo, hi) = self.span();
        let (along, round) = if b.fine() { (40, 28) } else { (16, 12) };
        let rings: Vec<Vec<Vec3>> = (0..=along)
            .map(|i| {
                // Closer together at the ends, where the hull turns fastest.
                let t = 0.5 - 0.5 * (std::f32::consts::PI * i as f32 / along as f32).cos();
                let x = lo + (hi - lo) * t;
                (0..round)
                    .map(|j| self.point(x, -90.0 + 360.0 * j as f32 / round as f32))
                    .collect()
            })
            .collect();
        seam(b);
        b.loft(&rings, true, true);
    }
}

/// A plate laid on the hull from `head` (forward) back to `tail`: at each fraction `t`
/// of the way back, `band(t)` gives its edges round the hull (`phi` low and high) and
/// how far its underside and face stand out from the surface. A band whose edges meet
/// draws the plate into a point there: the swept spike of its trailing edge.
pub(super) fn skin(
    b: &mut MeshBuilder,
    body: &Body,
    head: f32,
    tail: f32,
    band: impl Fn(f32) -> (f32, f32, f32, f32),
) {
    let steps = if b.fine() { (6, 3) } else { (3, 2) };
    skin_in(b, body, (head, tail), steps, band);
}

/// [`skin`] in `along` steps back and `round` across.
pub(super) fn skin_in(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    (along, round): (usize, usize),
    band: impl Fn(f32) -> (f32, f32, f32, f32),
) {
    let rings: Vec<Vec<Vec3>> = (0..=along)
        .map(|i| {
            let t = i as f32 / along as f32;
            let x = head + (tail - head) * t;
            let (lo, hi, under, face) = band(t);
            let arc = |h: f32, rev: bool| -> Vec<Vec3> {
                (0..=round)
                    .map(|k| {
                        let k = if rev { round - k } else { k };
                        body.across(x, lo + (hi - lo) * k as f32 / round as f32, h)
                    })
                    .collect()
            };
            let mut ring = arc(face, false);
            ring.extend(arc(under, true));
            ring
        })
        .collect();
    b.loft(&rings, true, true);
}

/// The usual plate: full width `lo`..`hi` at its head, drawn back into a spike at `tip`
/// over its last `taper` of length, its trailing edge lifting `lift` off the hull so it
/// rides over the head of the next.
pub(super) fn feather(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    (lo, hi, tip): (f32, f32, f32),
    thick: f32,
    lift: f32,
    taper: f32,
) {
    let steps = if b.fine() { (8, 3) } else { (4, 2) };
    skin_in(b, body, (head, tail), steps, |t| {
        let k = ((t - (1.0 - taper)) / taper).clamp(0.0, 1.0);
        let k = k * k * (3.0 - 2.0 * k);
        let under = -0.25 + lift * t * t;
        (
            lo + (tip - lo) * k,
            hi + (tip - hi) * k,
            under,
            under + thick * (1.0 - 0.6 * k),
        )
    });
}

/// A round tube through `points`, one loft.
pub(super) fn tube(b: &mut MeshBuilder, points: &[Vec3], r: f32, sides: usize) {
    let rings: Vec<Vec<Vec3>> = (0..points.len())
        .map(|i| {
            let dir = if i + 1 < points.len() {
                points[i + 1] - points[i]
            } else {
                points[i] - points[i - 1]
            };
            let e1 = dir.normalize_or(Vec3::X).any_orthonormal_vector();
            let e2 = dir.normalize_or(Vec3::X).cross(e1);
            (0..sides)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / sides as f32;
                    points[i] + (e1 * a.cos() + e2 * a.sin()) * r
                })
                .collect()
        })
        .collect();
    b.loft(&rings, true, true);
}
