//! "Halden's Grip": four against four across a land bridge between two bays,
//! after the classic Seton's Clutch.
//!
//! [`Layout::TwinBays`], 16 km. One bay fills the north-west, its twin the
//! south-east; the land runs corner to corner between them and pinches to a
//! low, open bridge at the middle. Each side's four bases: one on its beach of the nearer bay,
//! one at the front where the land bridge starts, one on the shore of the
//! farther bay and one back in the corner under the mountains. Every bay is
//! shared, so each is a sea war of its own; a rocky island stands off the
//! edge in each.
//!
//! Fairness: the map is the same under a half turn about its centre. The
//! designed pieces (the bay's coast, rises, rocks, ponds, ore) are listed
//! once, for the south-west side, and evaluated at the point and at the point
//! turned about the centre. Noise is blended between the two across a band
//! on the north-west to south-east diagonal, so it is exact under the turn
//! and has no crease.
//!
//! Coordinates are pixels of the 288 px reference drawing, y down (north up):
//! world `(x, y)` is `(px / 288 * size, (1 - py / 288) * size)`.

use super::{OreField, Pad, Terrain};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

/// Edge of the reference drawing, in its pixels.
const REF: f64 = 288.0;

/// The north-west bay's coast, land on the right going round: from beyond
/// the north edge, down the east shore, along the south shore and out
/// beyond the west edge, closed round the corner off the map.
const BAY: &[(f64, f64)] = &[
    (-40.0, -40.0),
    (140.0, -40.0),
    (144.0, 0.0),
    (150.0, 16.0),
    (164.0, 30.0),
    (177.0, 42.0),
    (179.0, 60.0),
    (173.0, 76.0),
    (168.0, 95.0),
    (163.0, 113.0),
    (152.0, 125.0),
    (137.0, 135.0),
    (119.0, 145.0),
    (101.0, 150.0),
    (89.0, 159.0),
    (80.0, 171.0),
    (70.0, 180.0),
    (60.0, 179.0),
    (57.0, 167.0),
    (49.0, 153.0),
    (38.0, 146.0),
    (22.0, 141.0),
    (8.0, 139.0),
    (0.0, 138.0),
    (-40.0, 136.0),
];

/// Starts, south-west side: the near bay's beach, the front, the far bay's
/// shore, the corner. Each is followed on the map by its turned twin.
const STARTS: &[(f64, f64)] = &[(45.0, 173.0), (100.0, 188.0), (96.0, 241.0), (27.0, 255.0)];

/// Islands: centre, radius (px).
const ISLANDS: &[(f64, f64, f64)] = &[(6.0, 79.0, 17.0)];

type Rock = ((f64, f64), (f64, f64), f64, f64);
/// Rock: capsules from `a` to `b`, half width (px), height (m). Nothing
/// climbs them; they wall the coast and the corners.
const ROCKS: &[Rock] = &[
    // The cliffs along the near bay's south shore, west of the beach base.
    ((6.0, 142.0), (40.0, 149.0), 4.5, 70.0),
    // The mountains in the corner behind the corner base.
    ((-6.0, 222.0), (6.0, 292.0), 11.0, 210.0),
    ((6.0, 294.0), (52.0, 294.0), 9.0, 170.0),
    // The island's crags, along its seaward side.
    ((0.0, 66.0), (2.0, 92.0), 7.0, 60.0),
];

type Stream = (&'static [(f64, f64)], f64, f64);
/// Streams, source to mouth (px), the valley floor's half width (px) and
/// the bed's height at the mouth (below zero: an estuary).
const STREAMS: &[Stream] = &[
    // From the ponds down to the beach base's bay.
    (
        &[
            (36.0, 196.0),
            (46.0, 191.0),
            (57.0, 186.0),
            (66.0, 181.0),
            (72.0, 177.0),
        ],
        1.2,
        -3.0,
    ),
    // From the ponds across the back country to the far bay.
    (
        &[
            (48.0, 213.0),
            (60.0, 222.0),
            (76.0, 226.0),
            (92.0, 223.0),
            (106.0, 218.0),
            (121.0, 212.0),
        ],
        1.4,
        -3.0,
    ),
    // Off the corner uplands, round the corner base to the south coast.
    (
        &[
            (8.0, 226.0),
            (20.0, 236.0),
            (46.0, 238.0),
            (52.0, 256.0),
            (66.0, 266.0),
            (90.0, 265.0),
            (108.0, 276.0),
            (126.0, 286.0),
        ],
        1.1,
        2.0,
    ),
];

/// The streams' courses rounded into curves (Catmull-Rom, px).
fn stream_courses() -> &'static [Vec<(f64, f64)>] {
    static COURSES: std::sync::OnceLock<Vec<Vec<(f64, f64)>>> = std::sync::OnceLock::new();
    COURSES.get_or_init(|| {
        STREAMS
            .iter()
            .map(|&(line, _, _)| {
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

/// Ponds: centre, radius (px).
const PONDS: &[(f64, f64, f64)] = &[(37.0, 201.0, 7.0), (45.0, 212.0, 4.5), (30.0, 213.0, 3.5)];

/// Ore away from the bases: centre (px), radius (m).
const ORE: &[(f64, f64, f64)] = &[
    // On the bridge, either side of the middle.
    (138.0, 151.0, 90.0),
    // Forward of the front base, toward the bridge.
    (116.0, 168.0, 80.0),
    // Between the bases.
    (70.0, 213.0, 75.0),
    (60.0, 248.0, 70.0),
    // The west beach and the south coast.
    (14.0, 186.0, 70.0),
    (126.0, 268.0, 75.0),
    (78.0, 276.0, 65.0),
    // On the island.
    (9.0, 79.0, 85.0),
];

/// How far the coast is pushed out to sea (metres; negative pulls it in):
/// headlands and bays on the scale of kilometres, coves on hundreds of
/// metres. Calmer round the bases, so the sea keeps off their pads.
fn wobble_of(t: &Terrain, x: f64, y: f64) -> f64 {
    let near = t.both(x, y, |p| {
        STARTS
            .iter()
            .map(|&(sx, sy)| -((p.0 - sx).powi(2) + (p.1 - sy).powi(2)).sqrt())
            .fold(f64::MIN, f64::max)
    });
    let calm = 0.3 + 0.7 * smoothstep(t.bm(12.0), t.bm(22.0), -near);
    calm * t.even(x, y, |x, y| {
        t.coast.fbm(x / 2_400.0, y / 2_400.0, 4, 0.55) * 2_800.0
            + t.coast_warp.fbm(x / 650.0, y / 650.0, 3, 0.5) * 900.0
    })
}

/// Half width (m) of the band over which noise hands over to its turned twin.
const BLEND: f64 = 500.0;
/// Depth of the open bay.
const DEEP: f64 = 45.0;

/// Distance from `p` to the segment `a`..`b`, and how far along it (0..1).
fn segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (ux, uy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * ux + (p.1 - a.1) * uy) / (ux * ux + uy * uy)).clamp(0.0, 1.0);
    let (dx, dy) = (p.0 - a.0 - t * ux, p.1 - a.1 - t * uy);
    ((dx * dx + dy * dy).sqrt(), t)
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
        // Out of both bays (each seen from the point and from its twin).
        let bay = -self.both(x, y, |p| inside(p, BAY));
        let island = self.both(x, y, |p| {
            ISLANDS
                .iter()
                .map(|&(ix, iy, r)| r - ((p.0 - ix).powi(2) + (p.1 - iy).powi(2)).sqrt())
                .fold(f64::MIN, f64::max)
        });
        self.bm(bay.max(island)) + wobble
    }

    /// 1 in open country, falling to 0 round the bases and the ore: the
    /// rough shaping (terrace cliffs, crags, erosion) keeps off what must be
    /// built on.
    fn bays_keep(&self, x: f64, y: f64) -> f64 {
        let near = |p: (f64, f64), list: &mut dyn Iterator<Item = (f64, f64, f64)>| {
            list.map(|(cx, cy, reach)| {
                let d = self.bm(((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt());
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
        // Uplands toward the back country, lowland along the bridge and the shores.
        let inland = smoothstep(150.0, 3_200.0, s);
        let upland = smoothstep(
            -0.25,
            0.3,
            n(&|x, y| self.mtn_mask.fbm(x / 4_200.0, y / 4_200.0, 3, 0.5)),
        );
        let rise = inland * (14.0 + 80.0 * upland);
        let mut land = 13.0 + rise;
        // Rolling ground, low hill ranges, and outcrops of bare rock.
        // The bridge stays low and open: the hills die away toward the middle.
        let from_middle =
            ((x - self.size_x / 2.0).powi(2) + (y - self.size_y / 2.0).powi(2)).sqrt();
        let open = smoothstep(2_200.0, 4_800.0, from_middle);
        let roll = n(&|x, y| self.tilt.fbm(x / 1_500.0, y / 1_500.0, 3, 0.5));
        land += 9.0 * roll * (0.4 + 0.6 * inland);
        // Hills: warped noise, its highs rounded into domes and spurs (a
        // ridged field's highs would be thin worms). Knolls: small round rises.
        let hills = n(&|x, y| {
            let (wx, wy) = (
                self.warp_x.get(x / 2_400.0, y / 2_400.0) * 900.0,
                self.warp_y.get(x / 2_400.0, y / 2_400.0) * 900.0,
            );
            self.mtn.fbm((x + wx) / 1_900.0, (y + wy) / 1_900.0, 4, 0.5)
        });
        land += open
            * 75.0
            * smoothstep(-0.05, 0.45, hills).powf(1.4)
            * (0.25 + 0.75 * inland)
            * smoothstep(80.0, 600.0, s)
            * (0.55 + 0.45 * keep);
        // Smaller hills and hollows everywhere, which is what the eye reads at play zoom.
        let lumps = n(&|x, y| self.mtn_height.fbm(x / 650.0, y / 650.0, 3, 0.5));
        land += (0.35 + 0.65 * open)
            * 70.0
            * (lumps + 0.05)
            * smoothstep(100.0, 700.0, s)
            * (0.5 + 0.5 * keep)
            * (0.45 + 0.55 * inland);
        // Hollows bottom out as dry meadow: only the designed ponds hold water.
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
                .map(|&(sx, sy)| -self.bm(((p.0 - sx).powi(2) + (p.1 - sy).powi(2)).sqrt()))
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
        let w = smoothstep(-25.0, run, s);
        let mut h = sea.min(0.2) * (1.0 - w) + land * w;
        // Sea stacks off the cliffs.
        let stack = n(&|x, y| self.crag.get(x / 170.0 - 7.0, y / 170.0 + 3.0));
        let band = smoothstep(-520.0, -260.0, s) * (1.0 - smoothstep(-90.0, -30.0, s));
        h = h.max(-8.0 + 40.0 * smoothstep(0.42, 0.62, stack) * band * cliffy);

        // -- designed pieces --
        let crag = n(&|x, y| self.crag.ridged(x / 700.0, y / 700.0, 4, 0.5));
        // Rock: the coast cliffs, the corner mountains, the island's crags.
        // Their outline frays in buttresses and gullies.
        let ragged = n(&|x, y| self.mtn_gap.fbm(x / 700.0, y / 700.0, 4, 0.55)) * 380.0;
        let rock = self.both(x, y, |p| {
            ROCKS
                .iter()
                .map(|&(a, b, half, tall)| {
                    let d = self.bm(segment(p, a, b).0) + ragged;
                    let half = self.bm(half);
                    tall * smoothstep(half + 120.0, 0.2 * half, d).powf(0.8)
                })
                .fold(0.0, f64::max)
        });
        h += rock * (0.7 + 0.8 * (crag - 0.4));

        // Streams: each cuts its valley down to a bed that falls to the sea,
        // sides no steeper than a unit can climb.
        let meander = 150.0 * n(&|x, y| self.ridge.fbm(x / 800.0 - 40.0, y / 800.0, 3, 0.5));
        // Some reaches run in a steep-sided cut, some in a broad open vale.
        let grade = 0.14 + 0.1 * n(&|x, y| self.ramp.fbm(x / 900.0 + 12.0, y / 900.0, 2, 0.5));
        for (&(_, half, mouth), line) in STREAMS.iter().zip(stream_courses()) {
            let cut = -self.both(x, y, |p| -{
                let (mut best, mut at, mut run, mut total) = (f64::INFINITY, 0.0, 0.0, 0.0);
                for w in line.windows(2) {
                    let len =
                        self.bm(((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt());
                    let (d, t) = segment(p, w[0], w[1]);
                    if self.bm(d) < best {
                        (best, at) = (self.bm(d), run + t * len);
                    }
                    run += len;
                    total = run;
                }
                let t = at / total;
                let bed = 6.0 - (6.0 - mouth) * smoothstep(0.55, 1.0, t);
                // Negative: this stream wants the ground this much lower.
                bed + grade * (best + meander - self.bm(half)).max(0.0)
            });
            h = h.min(cut);
        }

        // Ponds in the back country, shallower than a ship draws.
        // Each pond is lobed: its radius swings with the bearing, and the
        // shore frays on a finer scale. It lies in a hollow the land slopes into.
        let fray = n(&|x, y| self.lake_shore.fbm(x / 160.0, y / 160.0, 3, 0.5)) * 70.0;
        let pond = self.bm(self.both(x, y, |p| {
            PONDS
                .iter()
                .enumerate()
                .map(|(k, &(cx, cy, r))| {
                    let a = (p.1 - cy).atan2(p.0 - cx);
                    let k = k as f64;
                    let lobes = 1.0
                        + 0.28 * (2.0 * a + 1.3 * k).sin()
                        + 0.16 * (3.0 * a + 2.1 + k).sin()
                        + 0.1 * (5.0 * a + 0.4 * k).sin();
                    r * lobes - ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
                })
                .fold(f64::MIN, f64::max)
        })) + fray;
        if pond > -700.0 {
            let hollow = smoothstep(-700.0, 0.0, pond);
            let floor = -1.5 - 3.0 * smoothstep(0.0, 220.0, pond);
            let shore = 5.0 + 0.02 * (-pond).max(0.0);
            h = h.min(h * (1.0 - hollow) + shore * hollow);
            h += (floor - h) * smoothstep(-12.0, 12.0, pond);
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
            .map(|(a, b)| ((a - b) * VERTICAL).clamp(-30.0, 10.0))
            .collect();
        let mut soft = raw.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                soft[at] =
                    0.5 * raw[at] + 0.125 * (raw[at - 1] + raw[at + 1] + raw[at - n] + raw[at + n]);
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
    use crate::bake::{BakeParams, Terrain};

    #[test]
    fn the_twin_bays_are_the_same_under_a_half_turn() {
        let t = Terrain::new(&BakeParams::twin_bays("t", 8, 1));
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
        let t = Terrain::new(&BakeParams::twin_bays("t", 8, 1));
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
/// the landforms without the game.
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
    for j in 0..px {
        for i in 0..px {
            let (x, y) = (
                x0 + (i as f64 + 0.5) * step,
                y0 + span - (j as f64 + 0.5) * step,
            );
            let e = step.max(4.0);
            let h = t.height(x, y);
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
}
