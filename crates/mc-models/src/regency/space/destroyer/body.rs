//! A Scourge hull as a faceted surface: stations along x, each a hard-chined section of
//! flat facets, joined station to station by flat faces too, so the hull reads as cut
//! armour, never as a moulded shell. Every plate, house and fin is laid on it exactly.
//!
//! A point on the hull is `(x, u)`: `u` runs round the port half of the section from
//! the keel's middle (0) out along the keel flat, up the bilge to the knife chine, in
//! over the shoulder to the deck edge and across the deck to the spine (`SPINE`). The
//! whole numbers are the section's corners. Starboard is drawn by mirroring.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;

use super::super::super::kit::{seam, v3};

/// The corners round the port half, keel to spine.
pub(super) const KEEL_EDGE: f32 = 1.0;
pub(super) const BILGE: f32 = 2.0;
pub(super) const CHINE: f32 = 3.0;
pub(super) const DECK_EDGE: f32 = 5.0;
pub(super) const SPINE: f32 = 6.0;
const CORNERS: usize = 7;

/// One station: the half-beam at the chine, the keel flat's half-width, the deck's
/// half-width, and the keel's, the chine's, the deck edge's and the spine's heights;
/// and how far the chine alone is pressed in there (a gun's pocket), the corners above
/// and below it standing.
#[derive(Clone, Copy, Debug)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) w: f32,
    pub(super) keel_w: f32,
    pub(super) keel: f32,
    pub(super) chine: f32,
    pub(super) deck_w: f32,
    pub(super) deck: f32,
    pub(super) spine: f32,
    pub(super) notch: f32,
}

#[expect(clippy::too_many_arguments, reason = "one station of a hull table")]
pub(super) const fn st(
    x: f32,
    w: f32,
    keel_w: f32,
    keel: f32,
    chine: f32,
    deck_w: f32,
    deck: f32,
    spine: f32,
) -> Station {
    Station {
        x,
        w,
        keel_w,
        keel,
        chine,
        deck_w,
        deck,
        spine,
        notch: 0.0,
    }
}

impl Station {
    fn lerp(a: &Station, c: &Station, t: f32) -> Station {
        let f = |p: f32, q: f32| p + (q - p) * t;
        Station {
            x: f(a.x, c.x),
            w: f(a.w, c.w),
            keel_w: f(a.keel_w, c.keel_w),
            keel: f(a.keel, c.keel),
            chine: f(a.chine, c.chine),
            deck_w: f(a.deck_w, c.deck_w),
            deck: f(a.deck, c.deck),
            spine: f(a.spine, c.spine),
            notch: f(a.notch, c.notch),
        }
    }

    /// The port half's corners, (y, z), keel to spine.
    fn corners(&self) -> [Vec2; CORNERS] {
        let lower = self.chine - self.keel;
        let upper = self.deck - self.chine;
        [
            Vec2::new(0.0, self.keel),
            Vec2::new(self.keel_w, self.keel),
            Vec2::new(
                self.keel_w + (self.w - self.keel_w) * 0.8,
                self.chine - lower * 0.4,
            ),
            Vec2::new(self.w - self.notch, self.chine),
            Vec2::new(
                self.deck_w + (self.w - self.deck_w) * 0.68,
                self.chine + upper * 0.45,
            ),
            Vec2::new(self.deck_w, self.deck),
            Vec2::new(0.0, self.spine),
        ]
    }
}

/// A hull: its stations stern first.
pub(super) struct Body {
    pub(super) stations: Vec<Station>,
}

impl Body {
    /// Stern and bow.
    pub(super) fn span(&self) -> (f32, f32) {
        (self.stations[0].x, self.stations[self.stations.len() - 1].x)
    }

    /// The section at `x`, straight between its stations.
    pub(super) fn at(&self, x: f32) -> Station {
        let s = &self.stations;
        let (lo, hi) = self.span();
        let x = x.clamp(lo, hi);
        let i = s
            .windows(2)
            .position(|w| x <= w[1].x)
            .unwrap_or(s.len() - 2);
        let (a, c) = (&s[i], &s[i + 1]);
        Station::lerp(a, c, ((x - a.x) / (c.x - a.x).max(1e-4)).clamp(0.0, 1.0))
    }

    /// The surface at `x`, `u` round the port half.
    pub(super) fn point(&self, x: f32, u: f32) -> Vec3 {
        let c = self.at(x).corners();
        let u = u.clamp(0.0, SPINE);
        let i = (u as usize).min(CORNERS - 2);
        let p = c[i].lerp(c[i + 1], u - i as f32);
        v3(x, p.x, p.y)
    }

    /// The outward normal in the section's plane at `x`, `u`: the facet's, or between
    /// the two facets at a corner.
    pub(super) fn normal(&self, x: f32, u: f32) -> Vec3 {
        let c = self.at(x).corners();
        let facet = |i: usize| {
            let d = c[i + 1] - c[i];
            Vec2::new(d.y, -d.x).normalize_or(Vec2::Y)
        };
        let u = u.clamp(0.0, SPINE);
        let i = (u as usize).min(CORNERS - 2);
        let f = u - u.floor();
        let n = if f < 1e-3 && i > 0 {
            (facet(i - 1) + facet(i)).normalize_or(Vec2::Y)
        } else if u >= SPINE - 1e-3 {
            Vec2::Y
        } else if u < 1e-3 {
            -Vec2::Y
        } else {
            facet(i)
        };
        v3(0.0, n.x, n.y)
    }

    /// `h` metres out from the surface at `x`, `u`.
    pub(super) fn out(&self, x: f32, u: f32, h: f32) -> Vec3 {
        self.point(x, u) + self.normal(x, u) * h
    }

    /// The `u` on the shoulder or deck at which the upper surface stands `y` out from
    /// the centreline (`y` from the body's own centreline).
    pub(super) fn u_up(&self, x: f32, y: f32) -> f32 {
        let c = self.at(x).corners();
        for i in (CHINE as usize..CORNERS - 1).rev() {
            let (a, d) = (c[i + 1], c[i]);
            if y >= a.x && y <= d.x + 1e-4 {
                return i as f32 + (d.x - y) / (d.x - a.x).max(1e-4);
            }
        }
        if y <= 0.0 {
            SPINE
        } else {
            CHINE
        }
    }

    /// The `u` on the keel or bilge at which the lower surface stands `y` out.
    pub(super) fn u_down(&self, x: f32, y: f32) -> f32 {
        let c = self.at(x).corners();
        for i in 0..CHINE as usize {
            let (a, d) = (c[i], c[i + 1]);
            if y >= a.x && y <= d.x + 1e-4 {
                return i as f32 + (y - a.x) / (d.x - a.x).max(1e-4);
            }
        }
        if y <= 0.0 {
            0.0
        } else {
            CHINE
        }
    }

    /// The full section at `x`, port keel round to starboard, mirrored.
    fn ring(&self, x: f32) -> Vec<Vec3> {
        let mut ring: Vec<Vec3> = (0..CORNERS).map(|i| self.point(x, i as f32)).collect();
        ring.extend((1..CORNERS - 1).rev().map(|i| {
            let p = self.point(x, i as f32);
            v3(p.x, -p.y, p.z)
        }));
        ring
    }

    /// The hull itself, in the seam colour: what shows between the plates.
    pub(super) fn hull(&self, b: &mut MeshBuilder) {
        let rings: Vec<Vec<Vec3>> = self.stations.iter().map(|s| self.ring(s.x)).collect();
        seam(b);
        b.with_facets(|b| b.loft(&rings, true, true));
    }

    /// The stations between `a` and `b` (either order), and the ends: where a plate laid
    /// along the hull must bend with it.
    fn stops(&self, a: f32, c: f32, steps: usize) -> Vec<f32> {
        let mut xs: Vec<f32> = (0..=steps)
            .map(|i| a + (c - a) * i as f32 / steps as f32)
            .collect();
        let (lo, hi) = (a.min(c), a.max(c));
        xs.extend(
            self.stations
                .iter()
                .map(|s| s.x)
                .filter(|&x| x > lo + 0.5 && x < hi - 0.5),
        );
        let ascending = c > a;
        xs.sort_by(|p, q| {
            if ascending {
                p.total_cmp(q)
            } else {
                q.total_cmp(p)
            }
        });
        xs.dedup_by(|p, q| (*p - *q).abs() < 0.3);
        xs
    }
}

/// `n` values of `u` evenly from `lo` to `hi`. Every ring of a plate is sampled alike,
/// so its faces never twist; across a corner the plate's chord sits a little inside the
/// hull's, under its thickness.
fn arc(lo: f32, hi: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| lo + (hi - lo) * i as f32 / (n - 1) as f32)
        .collect()
}

/// A plate laid on the hull from `head` (forward) back to `tail`: at each fraction `t`
/// of the way back, `band(t)` gives its edges round the hull (`u` low and high) and how
/// far its underside and face stand off the surface. Its long edges are chamfered, so a
/// plate reads as a cut slab.
pub(super) fn skin(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    band: impl Fn(f32) -> (f32, f32, f32, f32),
) {
    let steps = if b.fine() { 4 } else { 2 };
    let n = if b.fine() { 5 } else { 3 };
    let rings: Vec<Vec<Vec3>> = body
        .stops(head, tail, steps)
        .into_iter()
        .map(|x| {
            let t = ((x - head) / (tail - head)).clamp(0.0, 1.0);
            let (lo, hi, under, face) = band(t);
            let chamfer = ((hi - lo).abs() * 0.12).min(0.12) * (hi - lo).signum();
            let mut ring: Vec<Vec3> = arc(lo + chamfer, hi - chamfer, n)
                .into_iter()
                .map(|u| body.out(x, u, face))
                .collect();
            ring.extend(
                arc(lo, hi, n)
                    .into_iter()
                    .rev()
                    .map(|u| body.out(x, u, under)),
            );
            ring
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// The Regency plate: full width `lo`..`hi` at its head, drawn back into a spike at
/// `tip` over its last `taper` of length, its trailing edge lifting `lift` off the hull
/// so it rides over the head of the next.
pub(super) fn feather(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    (lo, hi, tip): (f32, f32, f32),
    thick: f32,
    lift: f32,
    taper: f32,
) {
    skin(b, body, (head, tail), |t| {
        let k = ((t - (1.0 - taper)) / taper).clamp(0.0, 1.0);
        let under = -0.6 + lift * t * t * t;
        (
            lo + (tip - lo) * k,
            hi + (tip - hi) * k,
            under,
            under + thick * (1.0 - 0.6 * k),
        )
    });
}
