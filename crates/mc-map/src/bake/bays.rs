//! "Halden's Grip": four against four across a land bridge between two bays,
//! after the classic Seton's Clutch.
//!
//! [`Layout::TwinBays`], 16 km. One bay fills the north-west, its twin the
//! south-east; the land runs corner to corner between them and pinches to a
//! low, open bridge at the middle. Each side's four bases: one above the
//! islet cove on the nearer bay, one at the front on the open plain where the
//! land bridge starts, one on the shore of the farther bay and one in a high
//! valley in the corner mountains. Every bay is shared, so each is a sea war
//! of its own; a hill island stands off the edge in each.
//!
//! What makes the original play the way it does, and is kept here: a
//! mountain massif on the cove base's seaward flank that ships cannot shoot
//! through; a cove fenced by rock islets and a mountain wall; a sealed cove
//! at the map edge where a navy can be built out of reach of the bay; a
//! shallow lake with an island in the back country; and an open plain from
//! the bridge to the far-shore base. Added: a harbour cove in the tip beside
//! the cove base, walled by mountains; the corner base shut in by a ring of
//! mountains with two passes; a raised headland plateau beside the far-shore
//! base that meets the far bay in a cliff. Away from the mountains the land
//! is open meadow, with no rolling hills.
//!
//! Fairness: the map is the same under a half turn about its centre. The
//! designed pieces are listed once, for the south-west side, and evaluated at
//! the point and at the point turned about the centre. Noise is blended
//! between the two across a band on the north-west to south-east diagonal,
//! so it is exact under the turn and has no crease.
//!
//! Coordinates are pixels of a 1024 px reference drawing, y down (north up):
//! world `(x, y)` is `(px / 1024 * size, (1 - py / 1024) * size)`.

use super::{OreField, Pad, Terrain};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

/// Edge of the reference drawing, in its pixels.
const REF: f64 = 1024.0;

/// The north-west bay's coast: from beyond the north edge, down the east
/// shore to the land bridge, west along the south shore (round the islet
/// cove) and out beyond the west edge, closed round the corner off the map.
const BAY: &[(f64, f64)] = &[
    (-40.0, -40.0),
    (528.0, -40.0),
    (523.0, 38.0),
    (513.0, 61.0),
    (520.0, 93.0),
    (526.0, 105.0),
    (541.0, 111.0),
    (562.0, 132.0),
    (619.0, 153.0),
    (630.0, 222.0),
    (640.0, 242.0),
    (638.0, 273.0),
    (621.0, 289.0),
    (594.0, 347.0),
    (593.0, 399.0),
    (587.0, 414.0),
    (540.0, 446.0),
    (508.0, 488.0),
    (481.0, 500.0),
    (450.0, 528.0),
    (377.0, 517.0),
    (344.0, 548.0),
    (331.0, 607.0),
    (326.0, 617.0),
    (312.0, 623.0),
    (305.0, 640.0),
    (241.0, 654.0),
    (206.0, 633.0),
    (200.0, 612.0),
    (207.0, 574.0),
    (201.0, 543.0),
    (164.0, 508.0),
    (121.0, 484.0),
    (74.0, 491.0),
    (54.0, 478.0),
    (33.0, 482.0),
    (3.0, 474.0),
    (-40.0, 474.0),
];

/// The sealed cove in the west edge: deep enough for ships, joined to
/// neither bay, so a navy built there is out of the bay's reach.
const INLET: &[(f64, f64)] = &[
    (-40.0, 570.0),
    (3.0, 570.0),
    (46.0, 583.0),
    (53.0, 590.0),
    (60.0, 610.0),
    (52.0, 625.0),
    (32.0, 645.0),
    (-40.0, 646.0),
];

/// The hill island off the west edge, in the bay.
const ISLAND: &[(f64, f64)] = &[
    (-40.0, 233.0),
    (35.0, 233.0),
    (50.0, 239.0),
    (62.0, 260.0),
    (64.0, 289.0),
    (69.0, 302.0),
    (61.0, 329.0),
    (23.0, 348.0),
    (-40.0, 349.0),
];

/// The back-country lake, deep in the middle.
const LAKE: &[(f64, f64)] = &[
    (122.0, 687.0),
    (120.0, 695.0),
    (122.0, 703.0),
    (112.0, 705.0),
    (110.0, 716.0),
    (118.0, 734.0),
    (136.0, 739.0),
    (129.0, 755.0),
    (150.0, 772.0),
    (155.0, 773.0),
    (162.0, 769.0),
    (168.0, 753.0),
    (196.0, 747.0),
    (204.0, 730.0),
    (192.0, 717.0),
    (159.0, 720.0),
    (134.0, 700.0),
];

/// The lake's islands: centre, radius (px), height (m). A wooded knoll and
/// two rocks.
const LAKE_ISLES: &[(f64, f64, f64, f64)] = &[
    (137.0, 725.0, 8.0, 24.0),
    (178.0, 735.0, 3.0, 12.0),
    (150.0, 750.0, 2.2, 9.0),
];

/// Rock islets fencing the cove below the cove base: centre, radius (px),
/// height (m). They stand close enough to the shore to screen ships moored
/// behind them.
const ISLETS: &[(f64, f64, f64, f64)] = &[
    (215.0, 610.0, 4.5, 30.0),
    (216.0, 625.0, 4.0, 24.0),
    (226.0, 623.0, 2.5, 16.0),
    (246.0, 640.0, 3.5, 22.0),
    (232.0, 634.0, 2.0, 12.0),
    (214.0, 638.0, 2.0, 14.0),
];

/// Starts, south-west side: the cove base, the front, the far bay's shore,
/// the corner valley. Each is followed on the map by its turned twin.
const STARTS: &[(f64, f64)] = &[
    (172.0, 600.0),
    (357.0, 673.0),
    (365.0, 862.0),
    (99.0, 912.0),
];

/// The harbour cove cut into the north-west tip, open to the bay through a
/// narrow mouth between the edge ridge and the massif: a navy moored in it
/// is out of sight of the bay. It reaches out into the bay so its mouth is
/// deep enough for ships.
const COVE: &[(f64, f64)] = &[
    (10.0, 440.0),
    (42.0, 440.0),
    (40.0, 470.0),
    (46.0, 484.0),
    (58.0, 494.0),
    (62.0, 505.0),
    (56.0, 514.0),
    (42.0, 518.0),
    (28.0, 514.0),
    (18.0, 505.0),
    (13.0, 492.0),
    (14.0, 472.0),
];

/// The raised headland beside the far-shore base: its top stands this high
/// above the land round it, a sheer cliff where it meets the bay and a ramp
/// down to the land.
const PLATEAU: &[(f64, f64)] = &[
    (404.0, 906.0),
    (420.0, 886.0),
    (446.0, 878.0),
    (480.0, 884.0),
    (530.0, 900.0),
    (560.0, 1_060.0),
    (340.0, 1_060.0),
    (360.0, 985.0),
    (392.0, 935.0),
];
const PLATEAU_H: f64 = 90.0;

/// Broad rises the land stands on: centre, radius (px), height (m).
const UPLANDS: &[(f64, f64, f64, f64)] = &[
    // The corner base's highland.
    (55.0, 950.0, 175.0, 42.0),
    // The cove base's shelf under the massif.
    (125.0, 595.0, 75.0, 12.0),
];

struct Ridge {
    /// Crest line, px.
    line: &'static [(f64, f64)],
    /// Half width of the crest's foot, px.
    half: f64,
    /// Height of the crest above the land round it, m.
    tall: f64,
    /// 0 for a smooth grassy ridge, 1 for a broken range of bare rock.
    crag: f64,
}

/// The mountains; everything else is open meadow. The massif stands between
/// the bay and the cove base, so shells from the water hit rock. The corner
/// base sits in a ring of mountains (on a circle 100 px round it) broken by
/// two passes: north to the lake, east to the plain.
const RIDGES: &[Ridge] = &[
    // The massif on the cove base's seaward flank, east wall of the harbour cove.
    Ridge {
        line: &[
            (90.0, 502.0),
            (100.0, 520.0),
            (108.0, 538.0),
            (114.0, 552.0),
        ],
        half: 30.0,
        tall: 560.0,
        crag: 1.0,
    },
    // The edge ridge, the harbour cove's west wall.
    Ridge {
        line: &[(5.0, 470.0), (5.0, 505.0), (0.0, 540.0)],
        half: 15.0,
        tall: 300.0,
        crag: 1.0,
    },
    // The range along the islet cove's south shore, between the cove and the plain.
    Ridge {
        line: &[
            (224.0, 664.0),
            (252.0, 674.0),
            (286.0, 672.0),
            (314.0, 656.0),
        ],
        half: 16.0,
        tall: 240.0,
        crag: 0.9,
    },
    // The range along the west edge, behind the corner base.
    Ridge {
        line: &[
            (-10.0, 780.0),
            (5.0, 850.0),
            (12.0, 930.0),
            (30.0, 1000.0),
            (45.0, 1060.0),
        ],
        half: 38.0,
        tall: 620.0,
        crag: 1.0,
    },
    // The corner base's ring, north-west: from the west range to the north pass.
    Ridge {
        line: &[
            (5.0, 878.0),
            (17.0, 855.0),
            (35.0, 835.0),
            (57.0, 821.0),
            (60.0, 823.0),
        ],
        half: 24.0,
        tall: 440.0,
        crag: 1.0,
    },
    // North-east, between the north pass and the east pass: the ring's highest.
    Ridge {
        line: &[
            (140.0, 820.0),
            (149.0, 825.0),
            (170.0, 841.0),
            (186.0, 862.0),
            (190.0, 872.0),
        ],
        half: 28.0,
        tall: 520.0,
        crag: 1.0,
    },
    // From the east pass round the south to the west range.
    Ridge {
        line: &[
            (198.0, 945.0),
            (181.0, 969.0),
            (156.0, 994.0),
            (125.0, 1_009.0),
            (90.0, 1_012.0),
            (57.0, 1_003.0),
            (28.0, 983.0),
            (5.0, 946.0),
        ],
        half: 22.0,
        tall: 440.0,
        crag: 1.0,
    },
    // Along the south edge, from the ring to the headland plateau.
    Ridge {
        line: &[
            (205.0, 1_000.0),
            (250.0, 992.0),
            (310.0, 984.0),
            (365.0, 994.0),
            (405.0, 1_012.0),
        ],
        half: 26.0,
        tall: 300.0,
        crag: 0.9,
    },
    // On the headland plateau's tip, over the far bay.
    Ridge {
        line: &[(470.0, 930.0), (492.0, 948.0), (500.0, 978.0)],
        half: 18.0,
        tall: 320.0,
        crag: 1.0,
    },
];

/// The river draining the lake into the sealed cove, source (inside the
/// lake) to mouth (inside the cove), px. It winds down a vale cut into the
/// meadow; shallower than a ship draws, too wet to walk except at its fords.
const RIVER: &[(f64, f64)] = &[
    (120.0, 706.0),
    (110.0, 700.0),
    (102.0, 692.0),
    (103.0, 682.0),
    (96.0, 673.0),
    (85.0, 671.0),
    (78.0, 663.0),
    (81.0, 652.0),
    (74.0, 642.0),
    (63.0, 639.0),
    (57.0, 630.0),
    (52.0, 620.0),
    (46.0, 610.0),
];

/// Where the river can be forded, as a share of the way down it.
const FORDS: &[f64] = &[0.33, 0.66];

/// The river's course rounded into a curve (Catmull-Rom, px), the distance
/// down it at each point (px), and how hard it turns there (signed, about
/// -1..1: positive to the left).
struct Course {
    pts: Vec<(f64, f64)>,
    run: Vec<f64>,
    turn: Vec<f64>,
}

fn river_course() -> &'static Course {
    static COURSE: std::sync::OnceLock<Course> = std::sync::OnceLock::new();
    COURSE.get_or_init(|| {
        let line = RIVER;
        let at = |i: isize| line[i.clamp(0, line.len() as isize - 1) as usize];
        let mut pts = Vec::new();
        for i in 0..line.len() as isize - 1 {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            for k in 0..8 {
                let t = k as f64 / 8.0;
                let c = |a: f64, b: f64, c: f64, d: f64| {
                    0.5 * (2.0 * b
                        + (c - a) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                        + (3.0 * b - a - 3.0 * c + d) * t * t * t)
                };
                pts.push((c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1)));
            }
        }
        pts.push(*line.last().unwrap());
        let mut run = vec![0.0];
        for w in pts.windows(2) {
            run.push(run.last().unwrap() + dist(w[0], w[1]));
        }
        // The turn at each point, over a few points either side so a bend
        // reads as one bend.
        let n = pts.len();
        let raw: Vec<f64> = (0..n)
            .map(|i| {
                let (a, b, c) = (pts[i.saturating_sub(3)], pts[i], pts[(i + 3).min(n - 1)]);
                let (u, v) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
                let cross = u.0 * v.1 - u.1 * v.0;
                let len = (u.0.hypot(u.1) * v.0.hypot(v.1)).max(1e-9);
                // y runs down in the drawing: flip so positive turns left on the map.
                -cross / len
            })
            .collect();
        let turn = (0..n)
            .map(|i| {
                let lo = i.saturating_sub(4);
                let hi = (i + 4).min(n - 1);
                raw[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64 * 2.5
            })
            .collect();
        Course { pts, run, turn }
    })
}

/// Distance from `p` to the river's course (px), how far down it the nearest
/// point lies (px), which side `p` is on (+1 left on the map, -1 right) and
/// how hard the river turns there.
fn river_at(p: (f64, f64)) -> (f64, f64, f64, f64) {
    let c = river_course();
    let (mut best, mut at) = (f64::INFINITY, (0.0, 1.0, 0.0));
    for (i, w) in c.pts.windows(2).enumerate() {
        let (d, t) = segment(p, w[0], w[1]);
        if d < best {
            let (ux, uy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let cross = ux * (p.1 - w[0].1) - uy * (p.0 - w[0].0);
            let along = c.run[i] + t * (c.run[i + 1] - c.run[i]);
            let turn = c.turn[i] * (1.0 - t) + c.turn[i + 1] * t;
            best = d;
            at = (along, -cross.signum(), turn);
        }
    }
    (best, at.0, at.1, at.2)
}

/// Ore away from the bases: centre (px), radius (m).
const ORE: &[(f64, f64, f64)] = &[
    // On the bridge.
    (495.0, 535.0, 80.0),
    // Forward of the front base, toward the bridge.
    (378.0, 565.0, 75.0),
    // Round the cove base: toward the harbour cove, toward the lake, on the bay shore.
    (72.0, 592.0, 70.0),
    (138.0, 655.0, 70.0),
    (178.0, 532.0, 65.0),
    // The lake's west shore.
    (92.0, 742.0, 65.0),
    // Inside the corner base's ring, under the north pass and toward the east pass.
    (104.0, 852.0, 70.0),
    (132.0, 935.0, 65.0),
    // Out on the plain.
    (212.0, 790.0, 75.0),
    (336.0, 798.0, 75.0),
    // Out of the east pass, and under the south range.
    (262.0, 868.0, 70.0),
    (290.0, 925.0, 70.0),
    // On the headland plateau.
    (445.0, 970.0, 70.0),
    // On the island.
    (38.0, 300.0, 85.0),
];

/// How far the coast is pushed out to sea (metres; negative pulls it in):
/// small headlands and bights on a coast that already has its shape. Calmer
/// round the bases, so the sea keeps off their pads.
fn wobble_of(t: &Terrain, x: f64, y: f64) -> f64 {
    let near = t.both(x, y, |p| {
        STARTS
            .iter()
            .map(|&(sx, sy)| -((p.0 - sx).powi(2) + (p.1 - sy).powi(2)).sqrt())
            .fold(f64::MIN, f64::max)
    });
    // The harbour cove keeps its drawn shape: a wobble as wide as the cove
    // would close it or open it to the bay.
    let cove = t.both(x, y, |p| inside(p, COVE));
    let calm = (0.3 + 0.7 * smoothstep(t.bm(43.0), t.bm(78.0), -near))
        * (0.15 + 0.85 * smoothstep(15.0, 60.0, -cove));
    calm * t.even(x, y, |x, y| {
        t.coast.fbm(x / 1_400.0, y / 1_400.0, 4, 0.55) * 900.0
            + t.coast_warp.fbm(x / 450.0, y / 450.0, 3, 0.5) * 500.0
    })
}

/// Half width (m) of the band over which noise hands over to its turned twin.
const BLEND: f64 = 500.0;
/// Depth of the open bay. Seabed installations need 20 m over their lot,
/// so the bays are that deep a short way off every shore.
const DEEP: f64 = 70.0;

/// Distance from `p` to the segment `a`..`b`, and how far along it (0..1).
pub(super) fn segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (ux, uy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * ux + (p.1 - a.1) * uy) / (ux * ux + uy * uy)).clamp(0.0, 1.0);
    let (dx, dy) = (p.0 - a.0 - t * ux, p.1 - a.1 - t * uy);
    ((dx * dx + dy * dy).sqrt(), t)
}

/// Distance from `p` to a polyline.
pub(super) fn polyline(p: (f64, f64), line: &[(f64, f64)]) -> f64 {
    polyline_at(p, line).0
}

/// Distance from `p` to a polyline, and how far along it the nearest point
/// lies (0 at the first point, 1 at the last).
fn polyline_at(p: (f64, f64), line: &[(f64, f64)]) -> (f64, f64) {
    let (mut best, mut at, mut run) = (f64::INFINITY, 0.0, 0.0);
    for w in line.windows(2) {
        let len = dist(w[1], w[0]);
        let (d, t) = segment(p, w[0], w[1]);
        if d < best {
            (best, at) = (d, run + t * len);
        }
        run += len;
    }
    (best, at / run)
}

/// Distance from `p` to a point.
pub(super) fn dist(p: (f64, f64), (cx, cy): (f64, f64)) -> f64 {
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// How far `p` is inside the closed polygon (negative outside).
pub(super) fn inside(p: (f64, f64), poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    let mut d = f64::INFINITY;
    let mut odd = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        d = d.min(segment(p, a, b).0);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) {
            odd = !odd;
        }
    }
    if odd {
        d
    } else {
        -d
    }
}

impl Terrain {
    /// Reference pixels to map metres.
    fn bw(&self, (px, py): (f64, f64)) -> (f64, f64) {
        let k = self.size / REF;
        (px * k, self.size_y - py * k)
    }

    /// Map metres to reference pixels.
    fn bp(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let k = REF / self.size;
        (x * k, (self.size_y - y) * k)
    }

    /// Pixels to metres, for lengths.
    fn bm(&self, px: f64) -> f64 {
        px * self.size / REF
    }

    /// A point turned half round the centre of the map.
    pub(super) fn turned(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.size_x - x, self.size_y - y)
    }

    /// `f` sampled so the result is the same at a point and at its turned
    /// twin: `f` itself on the south-west side of the diagonal, its turned
    /// sample on the other, blended across the diagonal. `f` is noise about
    /// zero; the blend keeps its spread.
    fn even(&self, x: f64, y: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        let s = (self.size_x - x - y) / std::f64::consts::SQRT_2;
        let w = smoothstep(-BLEND, BLEND, s);
        let (tx, ty) = self.turned((x, y));
        match w {
            w if w >= 1.0 => f(x, y),
            w if w <= 0.0 => f(tx, ty),
            w => (w * f(x, y) + (1.0 - w) * f(tx, ty)) / (w * w + (1.0 - w) * (1.0 - w)).sqrt(),
        }
    }

    /// The larger of a designed field at the point and at its turned twin,
    /// both in reference pixels.
    fn both(&self, x: f64, y: f64, f: impl Fn((f64, f64)) -> f64) -> f64 {
        f(self.bp((x, y))).max(f(self.bp(self.turned((x, y)))))
    }

    /// Metres in from the shore (negative out to sea).
    fn bays_land(&self, x: f64, y: f64) -> f64 {
        let wobble = wobble_of(self, x, y);
        // Out of both bays, their harbour coves and both sealed coves (each
        // seen from the point and from its twin).
        let sea = -self.both(x, y, |p| {
            inside(p, BAY).max(inside(p, COVE)).max(inside(p, INLET))
        });
        let island = self.both(x, y, |p| inside(p, ISLAND));
        // The cove keeps its outline round the islets (the stacks rise from
        // the sea floor in `bays_shape`).
        let islets = self.both(x, y, |p| {
            ISLETS
                .iter()
                .map(|&(ix, iy, r, _)| r - dist(p, (ix, iy)))
                .fold(f64::MIN, f64::max)
        });
        let calm = smoothstep(-40.0, -15.0, -islets.abs());
        self.bm(sea.max(island)) + wobble * (1.0 - calm)
    }

    /// 1 in open country, falling to 0 round the bases and the ore: the
    /// rough shaping (crags, erosion) keeps off what must be built on.
    fn bays_keep(&self, x: f64, y: f64) -> f64 {
        let near = |p: (f64, f64), list: &mut dyn Iterator<Item = (f64, f64, f64)>| {
            list.map(|(cx, cy, reach)| {
                let d = self.bm(dist(p, (cx, cy)));
                smoothstep(reach, reach + 300.0, d)
            })
            .fold(1.0, f64::min)
        };
        let base = 1.1 * self.start_outer;
        let calm = |p: (f64, f64)| {
            near(p, &mut STARTS.iter().map(|&(x, y)| (x, y, base)))
                .min(near(p, &mut ORE.iter().map(|&(x, y, r)| (x, y, r + 40.0))))
        };
        let (a, b) = (calm(self.bp((x, y))), calm(self.bp(self.turned((x, y)))));
        a.min(b)
    }

    /// The land before erosion.
    fn bays_shape(&self, x: f64, y: f64) -> f64 {
        let s = self.bays_land(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        // The headland plateau, its edge frayed: it meets the bay in a cliff
        // (see `bays_shore`) and slopes up from the land in a long ramp.
        let fray = n(&|x, y| self.mtn_mask.fbm(x / 900.0 + 8.0, y / 900.0, 3, 0.5)) * 220.0;
        let plateau = smoothstep(
            -450.0,
            30.0,
            self.bm(self.both(x, y, |p| inside(p, PLATEAU))) + fray,
        );
        let land = self.bays_lowland(x, y, s) + PLATEAU_H * plateau;
        let mut h = self.bays_shore(x, y, s, land, plateau);
        let crag = n(&|x, y| self.crag.ridged(x / 700.0, y / 700.0, 4, 0.5));
        // The mountains stop at the water: they fall into the bays in cliffs
        // rather than filling them.
        h += self.bays_ranges(x, y, crag) * smoothstep(-350.0, 80.0, s);
        // The cove's rock islets.
        let rough = n(&|x, y| self.crag.ridged(x / 140.0 - 3.0, y / 140.0 + 8.0, 3, 0.5));
        h = h.max(self.both(x, y, |p| {
            ISLETS
                .iter()
                .map(|&(ix, iy, r, tall)| {
                    // Sheer sides, a broken crown.
                    let d = self.bm(dist(p, (ix, iy))) + 40.0 * (rough - 0.5);
                    let rise = smoothstep(self.bm(r) + 12.0, 0.55 * self.bm(r), d).powf(0.5);
                    // From the sea floor, not from a floor of their own.
                    -DEEP + (tall + DEEP) * (1.0 + 0.6 * (crag - 0.4)) * rise
                })
                .fold(f64::MIN, f64::max)
        }));
        self.bays_water(x, y, h)
    }

    /// Open meadow, as if there were no sea: low along the shores, a little
    /// higher inland, the designed rises, and swells no taller than a house.
    /// No hills: the mountains stand out of flat country.
    fn bays_lowland(&self, x: f64, y: f64, s: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        let inland = smoothstep(150.0, 3_000.0, s);
        let mut land = 13.0 + 10.0 * inland;
        let fray = n(&|x, y| self.mtn_mask.fbm(x / 1_600.0, y / 1_600.0, 3, 0.5)) * 1_200.0;
        land += self.both(x, y, |p| {
            UPLANDS
                .iter()
                .map(|&(cx, cy, r, tall)| {
                    let r = self.bm(r);
                    tall * smoothstep(r + 400.0, 0.3 * r, self.bm(dist(p, (cx, cy))) + fray)
                })
                .fold(0.0, f64::max)
        });
        land += 2.0 * n(&|x, y| self.tilt.fbm(x / 3_500.0, y / 3_500.0, 2, 0.5)) * inland;
        // Grain: faint swales, so the meadow is not a sheet of glass.
        land += 0.9 * n(&|x, y| self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5));
        land += 0.3 * n(&|x, y| self.detail.fbm(x / 55.0 + 9.0, y / 55.0 - 4.0, 2, 0.5));
        land
    }

    /// The land meeting the sea: long beaches in the bays, cliffs on the
    /// headlands and all round the plateau.
    fn bays_shore(&self, x: f64, y: f64, s: f64, land: f64, plateau: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        let bases = self.both(x, y, |p| {
            STARTS
                .iter()
                .map(|&(sx, sy)| -self.bm(dist(p, (sx, sy))))
                .fold(f64::MIN, f64::max)
        });
        let cliffy = (smoothstep(
            0.02,
            0.2,
            n(&|x, y| self.coast_warp.fbm(x / 2_600.0 + 30.0, y / 2_600.0, 3, 0.5)),
        ) * smoothstep(1_300.0, 2_300.0, -bases))
        .max(plateau);
        let run = 240.0 - 190.0 * cliffy - 40.0 * plateau;
        // The sea floor: a shelf off the beaches, a drop off the cliffs, sand bars.
        let shelf = 700.0 - 450.0 * cliffy;
        let mut sea = -DEEP * smoothstep(0.0, shelf, -s).powf(0.8);
        sea += 5.0
            * n(&|x, y| self.lake.fbm(x / 700.0, y / 700.0, 3, 0.5))
            * smoothstep(-60.0, -500.0, s);
        // The sealed coves are narrow: they fall away faster, so ships fit.
        let cove = self.both(x, y, |p| inside(p, INLET));
        sea = sea.min(-30.0 * smoothstep(0.0, 14.0, cove));
        let w = smoothstep(-25.0, run, s);
        let h = sea.min(0.2) * (1.0 - w) + land * w;
        // Sea stacks off the cliffs.
        let stack = n(&|x, y| self.crag.get(x / 170.0 - 7.0, y / 170.0 + 3.0));
        let band = smoothstep(-520.0, -260.0, s) * (1.0 - smoothstep(-90.0, -30.0, s));
        let rise = smoothstep(0.5, 0.66, stack) * band * cliffy;
        // Only the stacks themselves rise: the sea floor round them keeps its depth.
        h.max(-8.0 + 40.0 * rise - 200.0 * (1.0 - smoothstep(0.0, 0.15, rise)))
    }

    /// The mountains' height above the land they stand on.
    fn bays_ranges(&self, x: f64, y: f64, crag: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        // Peaks and saddles along each crest.
        let peaks = n(&|x, y| self.ridge.fbm(x / 1_100.0 + 17.0, y / 1_100.0, 3, 0.5));
        // Ranges and ridges; their outline frays in buttresses and gullies,
        // the more the rockier.
        let ragged = n(&|x, y| self.mtn_gap.fbm(x / 700.0, y / 700.0, 4, 0.55)) * 180.0;
        // Arêtes with hollows between them, and the spurs and crags on their
        // flanks: they grow from the foot to the crest, so the mountains
        // rise from smooth skirts into broken rock.
        let arete = n(&|x, y| self.mtn.ridged(x / 900.0 + 3.3, y / 900.0 - 1.9, 3, 0.5));
        let spur = n(&|x, y| self.mtn.ridged(x / 330.0 - 6.1, y / 330.0 + 4.4, 3, 0.5));
        // Broken rock on the upper slopes.
        let rock = n(&|x, y| self.crag.ridged(x / 150.0 + 11.0, y / 150.0 - 5.0, 3, 0.5));
        self.both(x, y, |p| {
            RIDGES
                .iter()
                .map(|r| {
                    let d = self.bm(polyline(p, r.line)) + ragged * (0.3 + 0.7 * r.crag);
                    let half = self.bm(r.half);
                    // Highest mid-crest, falling toward the ends into saddles
                    // and passes. By the distance to the nearer end, not the
                    // way along: inside a bend that would jump from arm to arm.
                    let len: f64 = r.line.windows(2).map(|w| dist(w[0], w[1])).sum();
                    let end = dist(p, r.line[0]).min(dist(p, r.line[r.line.len() - 1]));
                    // A long range stands at full height for most of its length.
                    let crest = 0.55 + 0.45 * smoothstep(0.0, (0.45 * len).min(40.0), end);
                    let tall = r.tall
                        * crest
                        * (1.0 + 0.35 * peaks).max(0.8)
                        * (1.0 + 0.6 * r.crag * (crag - 0.6));
                    // Steep from a narrow apron to the crest: a wall no unit
                    // can climb, not foothills it can.
                    let m = smoothstep(1.0 * half + 40.0, -0.3 * half, d);
                    let body = m;
                    let sharp = smoothstep(0.2, 0.8, m) * r.crag;
                    tall.max(0.0)
                        * (body * (1.0 + sharp * (0.55 * arete * arete - 0.2))
                            + 0.35 * sharp * m * (spur - 0.6))
                        + 22.0 * sharp * (rock - 0.45).max(0.0)
                })
                // Where two ranges meet they merge in a rounded saddle, not a crease.
                .fold(0.0, |a: f64, b: f64| {
                    // Only where both stand: away from them it is a plain max.
                    let k = 30.0 * smoothstep(0.0, 60.0, a.min(b)) + 1e-9;
                    let t = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
                    a + (b - a) * t + k * t * (1.0 - t)
                })
        })
    }

    /// Water cut into the land: the harbour cove, the streams and the lake.
    fn bays_water(&self, x: f64, y: f64, mut h: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        // The harbour cove: the mountains come down to it in sheer walls,
        // and it shelves up gently at its head, where the base can reach it
        // to build yards.
        h = h.min(-self.both(x, y, |p| {
            let c = self.bm(inside(p, COVE));
            let wall = 2.6 - 2.4 * smoothstep(34.0, 18.0, dist(p, (40.0, 520.0)));
            -(-2.0 - 30.0 * smoothstep(0.0, 160.0, c.max(0.0)) + wall * (-c).max(0.0))
        }));

        h = h.min(self.bays_river(x, y));

        // The lake, in a hollow the land slopes into. Its shore frays on two
        // scales; it has beaches in some reaches and low bluffs in others, a
        // sandy shelf and bars under the water.
        let fray = n(&|x, y| self.lake_shore.fbm(x / 220.0, y / 220.0, 3, 0.5)) * 110.0
            + n(&|x, y| self.lake_shore.fbm(x / 60.0 + 13.0, y / 60.0 - 7.0, 2, 0.5)) * 30.0;
        let lake = self.bm(self.both(x, y, |p| inside(p, LAKE))) + fray;
        if lake > -900.0 {
            let bluff = smoothstep(
                0.05,
                0.3,
                n(&|x, y| self.lake_shore.fbm(x / 700.0 + 40.0, y / 700.0, 2, 0.5)),
            );
            // Steep enough that the sand is a strip, not a ring.
            let bank = 80.0 - 60.0 * bluff;
            let rim = 11.0 * smoothstep(0.0, bank, -lake);
            h = h.min(0.3 + rim + 0.04 * (-lake - bank).max(0.0));
            // A sandy shelf, then deep water, dark in the middle.
            let bars = 1.5 * n(&|x, y| self.lake.fbm(x / 260.0 + 5.0, y / 260.0, 3, 0.5));
            let floor =
                (-0.8 - 27.0 * smoothstep(0.0, 420.0, lake).powf(0.8) + bars).clamp(-28.0, -0.4);
            h += (floor - h) * smoothstep(-6.0, 6.0, lake);
            // Its islands: a wooded knoll and two rocks.
            h = h.max(self.both(x, y, |p| {
                LAKE_ISLES
                    .iter()
                    .map(|&(cx, cy, r, tall)| {
                        let d = self.bm(dist(p, (cx, cy))) + fray / 2.0;
                        let r = self.bm(r);
                        let rise = smoothstep(r + 25.0, 0.2 * r, d).powf(0.5);
                        // Away from it the lake floor keeps its depth.
                        if rise > 0.0 {
                            -6.0 + (tall + 6.0) * rise
                        } else {
                            f64::MIN
                        }
                    })
                    .fold(f64::MIN, f64::max)
            }));
        }
        h
    }

    /// The ground the river leaves: its channel, banks, terraces and vale
    /// (higher than the land away from it, so the caller takes the lower).
    /// Cut banks on the outside of each bend, sand bars on the inside, and
    /// gravel fords where the channel shallows to dry ground.
    fn bays_river(&self, x: f64, y: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);
        let wiggle = 14.0 * n(&|x, y| self.ridge.fbm(x / 180.0 - 40.0, y / 180.0, 2, 0.5));
        let vary = n(&|x, y| self.ramp.fbm(x / 600.0 + 12.0, y / 600.0, 2, 0.5));
        let grain = n(&|x, y| self.detail.fbm(x / 90.0 - 3.0, y / 90.0 + 6.0, 2, 0.5));
        let total = self.bm(*river_course().run.last().unwrap());
        -self.both(x, y, |p| {
            let (d, along, side, turn) = river_at(p);
            let d = self.bm(d) + wiggle;
            if d > 1_200.0 {
                return -1e6;
            }
            let down = self.bm(along) / total;
            let ford = FORDS
                .iter()
                .map(|&f| smoothstep(55.0, 20.0, (self.bm(along) - f * total).abs()))
                .fold(0.0, f64::max);
            // Wider toward the mouth, and spread thin over a ford.
            let wide = (34.0 + 26.0 * smoothstep(0.75, 1.0, down))
                * (1.0 + 0.25 * vary)
                * (1.0 + 0.4 * ford);
            let inner = smoothstep(0.0, 0.6, side * turn);
            let outer = smoothstep(0.0, 0.6, -side * turn);
            let bank = (28.0 + 50.0 * inner - 10.0 * outer).max(80.0 * ford);
            // High enough to be grass, not the sand at the water's edge.
            let terrace = 10.5 + 1.5 * vary + 0.5 * grain;
            // The vale's width does not follow the bends: it would crease.
            let vale = wide + 220.0 + 90.0 * vary;
            let bed = -0.3 - 5.1 * (1.0 - (d / wide).min(1.0).powi(2));
            let bed = bed + (0.6 - bed) * ford;
            let edge = bed.max(-0.2);
            let ground = if d < wide {
                bed
            } else if d < wide + bank {
                edge + (terrace - edge) * smoothstep(wide, wide + bank, d)
            } else {
                // The vale's floor, then its walls up to the meadow, gentle
                // enough to walk; far off, nothing.
                terrace + 0.22 * (d - vale).max(0.0) + 2_000.0 * smoothstep(600.0, 1_000.0, d)
            };
            -ground
        })
    }

    pub(super) fn natural_bays(&self, x: f64, y: f64) -> f64 {
        let h = self.bays_shape(x, y);
        if self.erosion.delta.is_empty() || h < 0.5 {
            return h;
        }
        // Erosion digs no ponds of its own: it stops above the water line.
        let e = self.erosion.at(x, y) * self.bays_keep(x, y) * smoothstep(0.5, 6.0, h);
        (h + e).max(h.min(2.5))
    }

    /// Water erosion over the whole country, from the shaped land: gullies
    /// down every slope, fans at their feet. Run over the map as it is, then
    /// made exact under the half turn the way the noise is.
    fn erode_bays(&mut self) {
        const STEP: f64 = 16.0;
        // Metres per unit of height while the droplets run: gentle country.
        const VERTICAL: f32 = 20.0;
        let n = (self.size_x / STEP) as usize + 1;
        let mut h = vec![0f32; n * n];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in h.chunks_mut(rows * n).enumerate() {
                let t = &*self;
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(n).enumerate() {
                        let y = (k * rows + r) as f64 * STEP;
                        for (i, v) in row.iter_mut().enumerate() {
                            *v = t.bays_shape(i as f64 * STEP, y).max(-2.0) as f32 / VERTICAL;
                        }
                    }
                });
            }
        });
        let before = h.clone();
        super::alpine::erode(&mut h, n, self.seed);
        super::alpine::erode(&mut h, n, self.seed ^ 0x6261_7973);
        // Nothing left steeper than a unit can climb, gully sides included.
        let talus = 0.42 * STEP as f32 / VERTICAL;
        for _ in 0..12 {
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let at = j * n + i;
                    for nb in [at - 1, at + 1, at - n, at + n] {
                        let drop = h[at] - h[nb];
                        let was = before[at] - before[nb];
                        if drop > talus && drop > was {
                            let moved = (drop - talus.max(was)) * 0.25;
                            h[at] -= moved;
                            h[nb] += moved;
                        }
                    }
                }
            }
        }
        let raw: Vec<f32> = h
            .iter()
            .zip(&before)
            .map(|(a, b)| {
                // The mountains take shallow gullies, too shallow to open a way up
                // them (deep ones and their fans make ramps); the meadows stay flat.
                // High up, well above the walls at their feet, they cut deep.
                let was = (b * VERTICAL) as f64;
                let high = smoothstep(80.0, 250.0, was) as f32;
                let top = smoothstep(220.0, 420.0, was) as f32;
                ((a - b) * VERTICAL).clamp(-3.0 - 22.0 * high - 35.0 * top, 3.0 + 2.0 * high)
            })
            .collect();
        // Softened once everywhere, and three times more on the mountains,
        // where single droplets would otherwise scratch thin straight lines.
        let high: Vec<f32> = before
            .iter()
            .map(|&b| smoothstep(80.0, 250.0, (b * VERTICAL) as f64) as f32)
            .collect();
        let mut soft = raw.clone();
        for pass in 0..4 {
            let from = soft.clone();
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let at = j * n + i;
                    let blur = 0.5 * from[at]
                        + 0.125 * (from[at - 1] + from[at + 1] + from[at - n] + from[at + n]);
                    let w = if pass == 0 { 1.0 } else { high[at] };
                    soft[at] = from[at] + (blur - from[at]) * w;
                }
            }
        }
        // The same both ways round: blended across the diagonal like the noise.
        let mut delta = soft.clone();
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (i as f64 * STEP, j as f64 * STEP);
                let w = smoothstep(
                    -BLEND,
                    BLEND,
                    (self.size_x - x - y) / std::f64::consts::SQRT_2,
                ) as f32;
                delta[j * n + i] =
                    w * soft[j * n + i] + (1.0 - w) * soft[(n - 1 - j) * n + (n - 1 - i)];
            }
        }
        self.erosion = super::alpine::Erosion {
            n,
            step: STEP,
            delta,
        };
    }

    /// How thickly trees grow, and how much of it is conifer. Exact under
    /// the half turn: timber is income.
    pub(super) fn bays_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let broad = self.even(x, y, |x, y| self.forest.fbm(x / l, y / l, 3, 0.5));
        let copse = self.even(x, y, |x, y| {
            self.forest
                .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5)
        });
        let clearing = self.even(x, y, |x, y| {
            self.forest
                .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5)
        });
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(0.42, 0.62, copse);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let mut habitable = smoothstep(3.0, 8.0, height)
            * (1.0 - smoothstep(0.30, 0.55, slope))
            * (1.0 - smoothstep(150.0, 200.0, height));
        // A glade round every ore field.
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable;
        // Species do not change play: pines on the high ground, broadleaf by the water.
        let cold = smoothstep(25.0, 90.0, height) * 0.9
            + self.forest_kind.fbm(x / 1500.0, y / 1500.0, 2, 0.5) * 0.9;
        (density, smoothstep(-0.15, 0.35, cold))
    }

    pub(super) fn setup_bays(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = -0.05;
        self.erode_bays();

        let south: Vec<(f64, f64)> = STARTS.iter().map(|&s| self.snap(self.bw(s))).collect();
        let mut pads = Vec::new();
        for &at in &south {
            let height = self.natural(at.0, at.1).max(12.0);
            for p in [at, self.turned(at)] {
                pads.push(Pad {
                    x: p.0,
                    y: p.1,
                    core: start_core,
                    outer: 2.0 * start_core,
                    height,
                });
            }
        }
        self.pads = pads;
        // Each side's in turn, the twin next to it: skirmish's "Two Sides"
        // (teams by alternate slot) puts one side on each half.
        self.starts = south.iter().flat_map(|&s| [s, self.turned(s)]).collect();

        // Three small fields round each base, inside its pad: one behind
        // (away from the middle), two on the forward flanks.
        let mut sites = Vec::new();
        let middle = (self.size_x / 2.0, self.size_y / 2.0);
        for &a in &south {
            let back = (a.1 - middle.1).atan2(a.0 - middle.0);
            for turn in [0.0, PI - 1.15, PI + 1.15] {
                let (s, c) = (back + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((a.0 + c * d, a.1 + s * d), (0.2 * start_core).max(55.0)));
            }
        }
        for &(px, py, r) in ORE {
            sites.push((self.bw((px, py)), r));
        }
        let g = BUILD_CELL_M as f64;
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = ((p.0 / g).round() * g, (p.1 / g).round() * g);
            let field = self.ore_field(p.0, p.1, r);
            // The twin is the same field turned, corner for corner.
            let t = self.turned(p);
            let corners = field.corners.iter().map(|&c| self.turned(c)).collect();
            fields.push(OreField {
                x: t.0,
                y: t.1,
                radius: field.radius,
                corners,
            });
            fields.push(field);
        }
        self.ore = fields;
        self.fit_forests();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_twin_bays_are_the_same_under_a_half_turn() {
        let t = &*crate::bake::test_maps::TWIN_BAYS;
        let n = 97;
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (
                    (i as f64 + 0.37) * t.size_x / n as f64,
                    (j as f64 + 0.61) * t.size_y / n as f64,
                );
                let (tx, ty) = t.turned((x, y));
                let (a, b) = (t.height(x, y), t.height(tx, ty));
                assert!((a - b).abs() < 1e-6, "{a} vs {b} at {x},{y}");
                let (fa, fb) = (
                    t.bays_forest(x, y, a, 0.1).0,
                    t.bays_forest(tx, ty, b, 0.1).0,
                );
                assert!((fa - fb).abs() < 1e-6, "woods {fa} vs {fb} at {x},{y}");
            }
        }
    }

    #[test]
    fn noise_has_no_crease_on_the_diagonal() {
        let t = &*crate::bake::test_maps::TWIN_BAYS;
        // Across the diagonal on the land bridge, the ground changes no more
        // from one metre to the next than it does anywhere else.
        let (cx, cy) = (t.size_x / 2.0, t.size_y / 2.0);
        for k in -20..=20 {
            let (x, y) = (cx + k as f64 * 20.0, cy + k as f64 * 20.0 - 400.0);
            let d = (t.natural(x + 0.5, y + 0.5) - t.natural(x - 0.5, y - 0.5)).abs();
            assert!(d < 1.0, "step {d} at {x},{y}");
        }
    }
}

/// `BAYS_RELIEF=x0,y0,span,px,out.ppm cargo test --release -p mc-map --lib bays_relief -- --ignored`:
/// a hillshade of the land (sun from the north-west, water tinted), to judge
/// the landforms without the game, and the heights beside it as
/// little-endian f32 rows, north first (`out.ppm.f32`).
#[cfg(test)]
#[test]
#[ignore]
fn bays_relief() {
    let spec = std::env::var("BAYS_RELIEF").unwrap_or_else(|_| "0,0,16384,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = Terrain::new(&crate::bake::BakeParams::twin_bays("t", 8, 7));
    let step = span / px as f64;
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    let mut raw = Vec::with_capacity(px * px * 4);
    for j in 0..px {
        for i in 0..px {
            let (x, y) = (
                x0 + (i as f64 + 0.5) * step,
                y0 + span - (j as f64 + 0.5) * step,
            );
            let e = step.max(4.0);
            let h = t.height(x, y);
            raw.extend((h as f32).to_le_bytes());
            let gx = (t.height(x + e, y) - t.height(x - e, y)) / (2.0 * e);
            let gy = (t.height(x, y + e) - t.height(x, y - e)) / (2.0 * e);
            let (nx, ny, nz) = (-gx, -gy, 1.0);
            let l = (nx * nx + ny * ny + nz * nz).sqrt();
            let shade = ((nx * -0.5 + ny * 0.5 + nz * 0.7) / l / 0.95).clamp(0.0, 1.0);
            let slope = (gx * gx + gy * gy).sqrt();
            let (r, g, b) = if h < 0.0 {
                (40.0, 70.0 + h.max(-45.0), 120.0 + h.max(-45.0))
            } else if slope > 0.5 {
                (150.0, 140.0, 130.0)
            } else {
                let k = (h / 120.0).clamp(0.0, 1.0);
                (110.0 + 90.0 * k, 150.0 + 30.0 * k, 90.0 + 60.0 * k)
            };
            out.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
        }
    }
    std::fs::write(v[4], out).unwrap();
    std::fs::write(format!("{}.f32", v[4]), raw).unwrap();
}
