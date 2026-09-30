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
//! through; a cove fenced by rock islets and a wall ridge; a sealed cove at
//! the map edge where a navy can be built out of reach of the bay; a shallow
//! lake with an island in the back country; the corner base's valley
//! between two ranges; low bluffs along the far bay's shore; and an open
//! plain from the bridge to the far-shore base.
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

/// The back-country lake, shallower than a ship draws.
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

/// The lake's island: centre, radius (px).
const LAKE_ISLE: (f64, f64, f64) = (137.0, 725.0, 7.0);

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

/// Broad rises the land stands on: centre, radius (px), height (m).
const UPLANDS: &[(f64, f64, f64, f64)] = &[
    // The corner base's highland.
    (55.0, 950.0, 175.0, 42.0),
    // The cove base's shelf under the massif.
    (125.0, 595.0, 75.0, 12.0),
    // The neck of land between the bay and the sealed cove.
    (20.0, 525.0, 60.0, 16.0),
    // The headland on the far bay.
    (470.0, 955.0, 55.0, 12.0),
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

/// Ranges and ridges. The massif and the bluffs stand between the bays and
/// the bases: shells from the water hit rock, not the base. The corner base
/// sits in a ring of mountains broken by three passes: north to the lake,
/// east to the plain, south-east to the south edge.
const RIDGES: &[Ridge] = &[
    // The massif on the cove base's seaward flank, between the bay and the base.
    Ridge {
        line: &[
            (38.0, 496.0),
            (62.0, 506.0),
            (80.0, 520.0),
            (96.0, 542.0),
            (112.0, 558.0),
        ],
        half: 33.0,
        tall: 560.0,
        crag: 1.0,
    },
    // Its spur west to the edge, over the neck.
    Ridge {
        line: &[(-20.0, 520.0), (38.0, 496.0)],
        half: 16.0,
        tall: 120.0,
        crag: 0.8,
    },
    // The wall along the cove's south shore, between the cove and the plain.
    Ridge {
        line: &[
            (212.0, 657.0),
            (250.0, 666.0),
            (290.0, 661.0),
            (318.0, 646.0),
        ],
        half: 11.0,
        tall: 90.0,
        crag: 0.6,
    },
    // Bluffs along the far bay's shore, in front of the front base.
    Ridge {
        line: &[
            (452.0, 585.0),
            (425.0, 640.0),
            (403.0, 705.0),
            (386.0, 760.0),
            (383.0, 800.0),
        ],
        half: 12.0,
        tall: 60.0,
        crag: 0.4,
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
        half: 42.0,
        tall: 480.0,
        crag: 1.0,
    },
    // The corner base's ring, north-west: from the west range to the north pass.
    Ridge {
        line: &[
            (2.4, 886.1),
            (9.1, 868.2),
            (19.1, 851.8),
            (32.1, 837.7),
            (47.5, 826.3),
            (64.8, 818.0),
        ],
        half: 18.0,
        tall: 330.0,
        crag: 1.0,
    },
    // North-east, between the north pass and the east pass.
    Ridge {
        line: &[
            (132.5, 819.9),
            (149.8, 828.2),
            (165.2, 839.7),
            (178.0, 854.1),
            (187.8, 870.6),
        ],
        half: 18.0,
        tall: 320.0,
        crag: 1.0,
    },
    // East, a lone peak between the east pass and the south-east pass.
    Ridge {
        line: &[(198.6, 920.7), (195.4, 938.7), (188.9, 955.8)],
        half: 16.0,
        tall: 280.0,
        crag: 1.0,
    },
    // South, from the south-east pass round to the west range.
    Ridge {
        line: &[
            (142.8, 1001.9),
            (123.2, 1009.0),
            (102.5, 1011.9),
            (81.6, 1010.5),
            (61.5, 1004.7),
            (43.1, 994.9),
            (27.1, 981.5),
            (14.2, 965.0),
            (5.0, 946.2),
        ],
        half: 19.0,
        tall: 340.0,
        crag: 1.0,
    },
    // Along the south edge.
    Ridge {
        line: &[
            (190.0, 1040.0),
            (240.0, 1000.0),
            (310.0, 992.0),
            (380.0, 1005.0),
            (430.0, 1040.0),
        ],
        half: 24.0,
        tall: 200.0,
        crag: 0.9,
    },
    // The low hills north-west of the far-shore base.
    Ridge {
        line: &[(232.0, 905.0), (270.0, 885.0), (318.0, 855.0)],
        half: 10.0,
        tall: 60.0,
        crag: 0.5,
    },
    // The headland's bluff over the far bay.
    Ridge {
        line: &[(430.0, 915.0), (470.0, 935.0), (500.0, 975.0)],
        half: 13.0,
        tall: 60.0,
        crag: 0.4,
    },
    // Rock knobs out on the plain.
    Ridge {
        line: &[(229.0, 733.0), (236.0, 737.0)],
        half: 5.0,
        tall: 16.0,
        crag: 1.0,
    },
    Ridge {
        line: &[(306.0, 760.0), (314.0, 763.0)],
        half: 5.0,
        tall: 14.0,
        crag: 1.0,
    },
    Ridge {
        line: &[(348.0, 781.0), (355.0, 783.0)],
        half: 4.5,
        tall: 12.0,
        crag: 1.0,
    },
];

/// The open plain from the land bridge to the far-shore base: its spine
/// (px) and half width (px). The rolling hills die away on it.
const PLAIN: (&[(f64, f64)], f64) = (
    &[
        (560.0, 470.0),
        (470.0, 560.0),
        (300.0, 720.0),
        (300.0, 830.0),
    ],
    90.0,
);

/// Streams, source to mouth (px), the valley floor's half width (px) and the
/// bed's height at the source and at the mouth (below zero: an estuary).
type Stream = (&'static [(f64, f64)], f64, f64, f64);
const STREAMS: &[Stream] = &[
    // The lake's outlet, down the vale to the sealed cove.
    (
        &[
            (114.0, 700.0),
            (101.0, 694.0),
            (93.0, 675.0),
            (79.0, 664.0),
            (73.0, 644.0),
            (60.0, 631.0),
            (50.0, 615.0),
        ],
        4.5,
        1.0,
        -3.0,
    ),
];

/// The streams' courses rounded into curves (Catmull-Rom, px).
fn stream_courses() -> &'static [Vec<(f64, f64)>] {
    static COURSES: std::sync::OnceLock<Vec<Vec<(f64, f64)>>> = std::sync::OnceLock::new();
    COURSES.get_or_init(|| {
        STREAMS
            .iter()
            .map(|&(line, ..)| {
                let at = |i: isize| line[i.clamp(0, line.len() as isize - 1) as usize];
                let mut out = Vec::new();
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
                        out.push((c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1)));
                    }
                }
                out.push(*line.last().unwrap());
                out
            })
            .collect()
    })
}

/// Ore away from the bases: centre (px), radius (m).
const ORE: &[(f64, f64, f64)] = &[
    // On the bridge.
    (495.0, 535.0, 80.0),
    // Forward of the front base, toward the bridge.
    (378.0, 565.0, 75.0),
    // Round the cove base: under the massif, toward the lake, on the bay shore.
    (72.0, 592.0, 70.0),
    (138.0, 655.0, 70.0),
    (178.0, 532.0, 65.0),
    // The lake's west shore.
    (92.0, 742.0, 65.0),
    // The corner valley's mouth, and out past its ridge.
    (100.0, 848.0, 70.0),
    (138.0, 948.0, 65.0),
    // Out on the plain.
    (200.0, 796.0, 75.0),
    (336.0, 798.0, 75.0),
    // Among the hills, and on the south edge.
    (232.0, 872.0, 70.0),
    (295.0, 942.0, 70.0),
    // The headland.
    (472.0, 992.0, 70.0),
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
    let calm = 0.3 + 0.7 * smoothstep(t.bm(43.0), t.bm(78.0), -near);
    calm * t.even(x, y, |x, y| {
        t.coast.fbm(x / 1_400.0, y / 1_400.0, 4, 0.55) * 900.0
            + t.coast_warp.fbm(x / 450.0, y / 450.0, 3, 0.5) * 500.0
    })
}

/// Half width (m) of the band over which noise hands over to its turned twin.
const BLEND: f64 = 500.0;
/// Depth of the open bay.
const DEEP: f64 = 45.0;

/// Distance from `p` to the segment `a`..`b`, and how far along it (0..1).
pub(super) fn segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (ux, uy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * ux + (p.1 - a.1) * uy) / (ux * ux + uy * uy)).clamp(0.0, 1.0);
    let (dx, dy) = (p.0 - a.0 - t * ux, p.1 - a.1 - t * uy);
    ((dx * dx + dy * dy).sqrt(), t)
}

/// Distance from `p` to a polyline.
fn polyline(p: (f64, f64), line: &[(f64, f64)]) -> f64 {
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
fn dist(p: (f64, f64), (cx, cy): (f64, f64)) -> f64 {
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// How far `p` is inside the closed polygon (negative outside).
fn inside(p: (f64, f64), poly: &[(f64, f64)]) -> f64 {
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
    fn turned(&self, (x, y): (f64, f64)) -> (f64, f64) {
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
        // Out of both bays and both sealed coves (each seen from the point
        // and from its twin).
        let sea = -self.both(x, y, |p| inside(p, BAY).max(inside(p, INLET)));
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
        let keep = self.bays_keep(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.even(x, y, f);

        // -- the land, as if there were no sea --
        // Lowland along the shores and the bridge, rising a little inland,
        // and the designed rises, their outlines frayed.
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
        // The plain from the bridge to the far-shore base stays open.
        let plain = self.both(x, y, |p| {
            smoothstep(PLAIN.1 + 40.0, PLAIN.1 - 20.0, polyline(p, PLAIN.0))
        });
        // The corner base's valley floor, inside its ring of mountains.
        let corner = self.both(x, y, |p| smoothstep(95.0, 55.0, dist(p, STARTS[3])));
        let open = (1.0 - 0.8 * plain) * (1.0 - corner);
        // Rolling ground, low hill ranges.
        let roll = n(&|x, y| self.tilt.fbm(x / 1_500.0, y / 1_500.0, 3, 0.5));
        land += 9.0 * roll * (0.4 + 0.6 * inland);
        // Hills: warped noise, its highs rounded into domes and spurs (a
        // ridged field's highs would be thin worms).
        let hills = n(&|x, y| {
            let (wx, wy) = (
                self.warp_x.get(x / 2_400.0, y / 2_400.0) * 900.0,
                self.warp_y.get(x / 2_400.0, y / 2_400.0) * 900.0,
            );
            self.mtn.fbm((x + wx) / 1_900.0, (y + wy) / 1_900.0, 4, 0.5)
        });
        land += open
            * 40.0
            * smoothstep(-0.05, 0.45, hills).powf(1.4)
            * (0.25 + 0.75 * inland)
            * smoothstep(80.0, 600.0, s)
            * (0.55 + 0.45 * keep);
        // Smaller hills and hollows everywhere, which is what the eye reads at play zoom.
        let lumps = n(&|x, y| self.mtn_height.fbm(x / 650.0, y / 650.0, 3, 0.5));
        land += (0.35 + 0.65 * open)
            * (1.0 - 0.7 * corner)
            * 55.0
            * (lumps + 0.05)
            * smoothstep(100.0, 700.0, s)
            * (0.5 + 0.5 * keep)
            * (0.45 + 0.55 * inland);
        // Hollows bottom out as dry meadow: only the designed lake holds water.
        land = if land < 8.0 {
            4.0 + 4.0 * smoothstep(-6.0, 8.0, land)
        } else {
            land
        };
        // Knolls, crowned with a low bluff of bare rock.
        let knoll = n(&|x, y| self.crag.fbm(x / 520.0 + 5.1, y / 520.0 - 2.7, 2, 0.45));
        let inshore = smoothstep(300.0, 1_000.0, s) * keep * open;
        let rough = n(&|x, y| self.crag.ridged(x / 140.0 - 3.0, y / 140.0 + 8.0, 3, 0.5));
        land += (12.0 * smoothstep(0.15, 0.4, knoll) + 5.0 * smoothstep(0.3, 0.5, knoll) * rough)
            * inshore;
        // Grain: swales and hummocks.
        land += 2.2 * n(&|x, y| self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5));
        land += 0.7 * n(&|x, y| self.detail.fbm(x / 55.0 + 9.0, y / 55.0 - 4.0, 2, 0.5));

        // -- the shore: long beaches in the bays, cliffs on the headlands --
        let bases = self.both(x, y, |p| {
            STARTS
                .iter()
                .map(|&(sx, sy)| -self.bm(dist(p, (sx, sy))))
                .fold(f64::MIN, f64::max)
        });
        let cliffy = smoothstep(
            0.02,
            0.2,
            n(&|x, y| self.coast_warp.fbm(x / 2_600.0 + 30.0, y / 2_600.0, 3, 0.5)),
        ) * smoothstep(1_300.0, 2_300.0, -bases);
        let run = 240.0 - 190.0 * cliffy;
        // The sea floor: a shelf off the beaches, a drop off the cliffs, sand bars.
        let shelf = 1_300.0 - 900.0 * cliffy;
        let mut sea = -DEEP * smoothstep(0.0, shelf, -s).powf(0.8);
        sea += 5.0
            * n(&|x, y| self.lake.fbm(x / 700.0, y / 700.0, 3, 0.5))
            * smoothstep(-60.0, -500.0, s);
        // The sealed coves are narrow: they fall away faster, so ships fit.
        let cove = self.both(x, y, |p| inside(p, INLET));
        sea = sea.min(-18.0 * smoothstep(0.0, 14.0, cove));
        let w = smoothstep(-25.0, run, s);
        let mut h = sea.min(0.2) * (1.0 - w) + land * w;
        // Sea stacks off the cliffs.
        let stack = n(&|x, y| self.crag.get(x / 170.0 - 7.0, y / 170.0 + 3.0));
        let band = smoothstep(-520.0, -260.0, s) * (1.0 - smoothstep(-90.0, -30.0, s));
        h = h.max(-8.0 + 40.0 * smoothstep(0.5, 0.66, stack) * band * cliffy);

        // -- designed pieces --
        let crag = n(&|x, y| self.crag.ridged(x / 700.0, y / 700.0, 4, 0.5));
        // Peaks and saddles along each crest.
        let peaks = n(&|x, y| self.ridge.fbm(x / 1_100.0 + 17.0, y / 1_100.0, 3, 0.5));
        // Ranges and ridges; their outline frays in buttresses and gullies,
        // the more the rockier.
        let ragged = n(&|x, y| self.mtn_gap.fbm(x / 700.0, y / 700.0, 4, 0.55)) * 500.0;
        // Arêtes with hollows between them, and the spurs and crags on their
        // flanks: they grow from the foot to the crest, so the mountains
        // rise from smooth skirts into broken rock.
        let arete = n(&|x, y| self.mtn.ridged(x / 900.0 + 3.3, y / 900.0 - 1.9, 3, 0.5));
        let spur = n(&|x, y| self.mtn.ridged(x / 330.0 - 6.1, y / 330.0 + 4.4, 3, 0.5));
        h += self.both(x, y, |p| {
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
                    let crest = 0.3 + 0.7 * smoothstep(0.0, 0.45 * len, end);
                    let tall =
                        r.tall * crest * (1.0 + 0.5 * peaks) * (1.0 + 0.6 * r.crag * (crag - 0.6));
                    let m = smoothstep(half + 200.0 + 0.3 * half, -0.4 * half, d);
                    let body = m.powf(1.3);
                    let sharp = smoothstep(0.2, 0.8, m) * r.crag;
                    tall.max(0.0)
                        * (body * (1.0 + 0.7 * sharp * (arete - 0.6))
                            + 0.3 * sharp * m * (spur - 0.6))
                })
                // Where two ranges meet they merge in a rounded saddle, not a crease.
                .fold(0.0, |a: f64, b: f64| {
                    // Only where both stand: away from them it is a plain max.
                    let k = 30.0 * smoothstep(0.0, 60.0, a.min(b)) + 1e-9;
                    let t = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
                    a + (b - a) * t + k * t * (1.0 - t)
                })
        });
        // The cove's rock islets.
        h = h.max(self.both(x, y, |p| {
            ISLETS
                .iter()
                .map(|&(ix, iy, r, tall)| {
                    // Sheer sides, a broken crown.
                    let d = self.bm(dist(p, (ix, iy))) + 40.0 * (rough - 0.5);
                    let rise = smoothstep(self.bm(r) + 12.0, 0.55 * self.bm(r), d).powf(0.5);
                    -20.0 + (tall + 20.0) * (1.0 + 0.6 * (crag - 0.4)) * rise
                })
                .fold(-DEEP, f64::max)
        }));

        // Streams: each cuts its valley down to a bed that falls to the sea,
        // sides no steeper than a unit can climb.
        let meander = 150.0 * n(&|x, y| self.ridge.fbm(x / 800.0 - 40.0, y / 800.0, 3, 0.5));
        // Some reaches run in a steep-sided cut, some in a broad open vale.
        let grade = 0.14 + 0.1 * n(&|x, y| self.ramp.fbm(x / 900.0 + 12.0, y / 900.0, 2, 0.5));
        for (&(_, half, source, mouth), line) in STREAMS.iter().zip(stream_courses()) {
            let cut = -self.both(x, y, |p| -{
                let (best, t) = polyline_at(p, line);
                let best = self.bm(best);
                let bed = source - (source - mouth) * smoothstep(0.55, 1.0, t);
                // Negative: this stream wants the ground this much lower.
                // Past the valley the stream no longer cuts: its walls would
                // otherwise shave the tops off mountains kilometres away.
                bed + grade * (best + meander - self.bm(half)).max(0.0)
                    + 2_000.0 * smoothstep(500.0, 900.0, best)
            });
            h = h.min(cut);
        }

        // The lake, shallower than a ship draws. It lies in a hollow the
        // land slopes into; the shore frays on a finer scale.
        let fray = n(&|x, y| self.lake_shore.fbm(x / 160.0, y / 160.0, 3, 0.5)) * 70.0;
        let lake = self.bm(self.both(x, y, |p| inside(p, LAKE))) + fray;
        if lake > -700.0 {
            let hollow = smoothstep(-700.0, 0.0, lake);
            let floor = -1.5 - 3.0 * smoothstep(0.0, 220.0, lake);
            let shore = 5.0 + 0.02 * (-lake).max(0.0);
            h = h.min(h * (1.0 - hollow) + shore * hollow);
            h += (floor - h) * smoothstep(-12.0, 12.0, lake);
            // Its island: a low wooded rise.
            let isle = self.both(x, y, |p| LAKE_ISLE.2 - dist(p, (LAKE_ISLE.0, LAKE_ISLE.1)));
            h = h.max(-3.0 + 12.0 * smoothstep(-2.0, 5.0, isle + fray / 30.0));
        }
        h
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
                // The mountains take deep gullies and big fans; the lowland only a little.
                let high = smoothstep(80.0, 250.0, (b * VERTICAL) as f64) as f32;
                ((a - b) * VERTICAL).clamp(-30.0 - 45.0 * high, 10.0 + 20.0 * high)
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
