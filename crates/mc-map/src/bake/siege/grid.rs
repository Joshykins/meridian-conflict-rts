//! Plane geometry for the siege plan: roads as segments, footprints as turned
//! rectangles, and a bucket grid to find what is near a point quickly. The
//! plan holds tens of thousands of each, and the bake asks about every
//! height sample.

use crate::format::Road;

pub(in crate::bake) type P = (f64, f64);

/// Bucket edge, metres.
const BUCKET: f64 = 128.0;

/// Items by the buckets their bounds touch.
#[derive(Clone, Debug, Default)]
pub(in crate::bake) struct Buckets {
    w: usize,
    h: usize,
    cells: Vec<Vec<u32>>,
}

impl Buckets {
    pub(super) fn new(size: f64) -> Buckets {
        let n = (size / BUCKET).ceil() as usize + 1;
        Buckets {
            w: n,
            h: n,
            cells: vec![Vec::new(); n * n],
        }
    }

    fn range(&self, lo: f64, hi: f64, n: usize) -> std::ops::RangeInclusive<usize> {
        let a = (lo / BUCKET).floor().clamp(0.0, (n - 1) as f64) as usize;
        let b = (hi / BUCKET).floor().clamp(0.0, (n - 1) as f64) as usize;
        a..=b
    }

    /// Files item `id` under every bucket its bounds `(lo, hi)` touch.
    pub(super) fn insert(&mut self, id: u32, lo: P, hi: P) {
        for j in self.range(lo.1, hi.1, self.h) {
            for i in self.range(lo.0, hi.0, self.w) {
                self.cells[j * self.w + i].push(id);
            }
        }
    }

    /// Every item filed in a bucket within `reach` of `p` (some more than once).
    pub(super) fn near(&self, p: P, reach: f64) -> impl Iterator<Item = u32> + '_ {
        let xs = self.range(p.0 - reach, p.0 + reach, self.w);
        let ys = self.range(p.1 - reach, p.1 + reach, self.h);
        ys.flat_map(move |j| {
            xs.clone()
                .flat_map(move |i| self.cells[j * self.w + i].iter().copied())
        })
    }
}

/// A stretch of road from `a` to `b`.
#[derive(Clone, Copy, Debug)]
pub(in crate::bake) struct Seg {
    pub(in crate::bake) a: P,
    pub(in crate::bake) b: P,
    /// Centreline to kerb (or to the edge of the running surface), metres.
    pub(in crate::bake) half: f64,
    pub(in crate::bake) road: Road,
    /// Whether a pavement runs along it (city streets); its width, metres.
    pub(in crate::bake) walk: f64,
}

impl Seg {
    /// Distance from `p` to the centreline.
    pub(in crate::bake) fn distance(&self, p: P) -> f64 {
        dist_to_segment(p, self.a, self.b)
    }

    /// Which side of the road `p` is: positive to the left of the way it
    /// runs from `a` to `b`.
    pub(in crate::bake) fn side(&self, p: P) -> f64 {
        (self.b.0 - self.a.0) * (p.1 - self.a.1) - (self.b.1 - self.a.1) * (p.0 - self.a.0)
    }

    /// What the road claims either side of its centreline: carriageway and
    /// pavement.
    pub(in crate::bake) fn reach(&self) -> f64 {
        self.half + self.walk
    }
}

pub(in crate::bake) fn dist(a: P, b: P) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

pub(in crate::bake) fn dist_to_segment(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

/// A rectangle turned `heading` radians about its middle: a footprint.
#[derive(Clone, Copy, Debug)]
pub(in crate::bake) struct Rect {
    pub(in crate::bake) c: P,
    pub(in crate::bake) hx: f64,
    pub(in crate::bake) hy: f64,
    pub(in crate::bake) heading: f64,
}

impl Rect {
    /// `p` in the rectangle's own frame.
    pub(super) fn local(&self, p: P) -> P {
        let (s, c) = self.heading.sin_cos();
        let (dx, dy) = (p.0 - self.c.0, p.1 - self.c.1);
        (dx * c + dy * s, dy * c - dx * s)
    }

    /// How far `p` lies outside the rectangle (negative inside: how deep).
    pub(in crate::bake) fn outside(&self, p: P) -> f64 {
        let (lx, ly) = self.local(p);
        let (qx, qy) = (lx.abs() - self.hx, ly.abs() - self.hy);
        let out = qx.max(0.0).hypot(qy.max(0.0));
        out + qx.max(qy).min(0.0)
    }

    pub(in crate::bake) fn corners(&self) -> [P; 4] {
        let (s, c) = self.heading.sin_cos();
        [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].map(|(u, v)| {
            let (x, y) = (u * self.hx, v * self.hy);
            (self.c.0 + x * c - y * s, self.c.1 + x * s + y * c)
        })
    }

    /// Axis-aligned bounds.
    pub(super) fn bounds(&self) -> (P, P) {
        let cs = self.corners();
        let lo = cs
            .iter()
            .fold((f64::MAX, f64::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
        let hi = cs
            .iter()
            .fold((f64::MIN, f64::MIN), |m, p| (m.0.max(p.0), m.1.max(p.1)));
        (lo, hi)
    }

    /// The shortest distance between the rectangle and a segment (0 when
    /// they cross).
    pub(super) fn distance_to_segment(&self, a: P, b: P) -> f64 {
        let cs = self.corners();
        if self.outside(a) <= 0.0 || self.outside(b) <= 0.0 {
            return 0.0;
        }
        let mut d = f64::MAX;
        for k in 0..4 {
            let (p, q) = (cs[k], cs[(k + 1) % 4]);
            if segments_cross(a, b, p, q) {
                return 0.0;
            }
            d = d
                .min(dist_to_segment(p, a, b))
                .min(dist_to_segment(a, p, q))
                .min(dist_to_segment(b, p, q));
        }
        d
    }

    /// Whether two rectangles come within `gap` of each other.
    pub(super) fn near(&self, other: &Rect, gap: f64) -> bool {
        let reach = self.hx.hypot(self.hy) + other.hx.hypot(other.hy) + gap;
        if dist(self.c, other.c) > reach {
            return false;
        }
        let (a, b) = (self.corners(), other.corners());
        (0..4).any(|k| {
            self.distance_to_segment(b[k], b[(k + 1) % 4]) < gap
                || other.distance_to_segment(a[k], a[(k + 1) % 4]) < gap
        })
    }
}

fn segments_cross(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Where along a polyline `x` falls (its y there): for lines that run west to
/// east with x rising, like the wall.
pub(in crate::bake) fn y_at(line: &[P], x: f64) -> f64 {
    let k = line
        .windows(2)
        .position(|w| x <= w[1].0)
        .unwrap_or(line.len() - 2);
    let (a, b) = (line[k], line[k + 1]);
    let t = ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    a.1 + t * (b.1 - a.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_measure_the_way_they_are_turned() {
        let r = Rect {
            c: (100.0, 100.0),
            hx: 20.0,
            hy: 5.0,
            heading: std::f64::consts::FRAC_PI_2,
        };
        // Turned a quarter: long along y.
        assert!(r.outside((100.0, 118.0)) < 0.0);
        assert!((r.outside((110.0, 100.0)) - 5.0).abs() < 1e-9);
        assert!((r.distance_to_segment((90.0, 0.0), (90.0, 200.0)) - 5.0).abs() < 1e-9);
        assert_eq!(r.distance_to_segment((0.0, 100.0), (200.0, 100.0)), 0.0);
        let other = Rect {
            c: (120.0, 100.0),
            ..r
        };
        assert!(r.near(&other, 11.0) && !r.near(&other, 9.0));
    }

    #[test]
    fn buckets_find_what_is_near() {
        let mut b = Buckets::new(1024.0);
        b.insert(7, (300.0, 300.0), (320.0, 320.0));
        assert!(b.near((310.0, 330.0), 20.0).any(|i| i == 7));
        assert!(!b.near((900.0, 900.0), 20.0).any(|i| i == 7));
    }
}
