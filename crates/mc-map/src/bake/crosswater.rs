//! "Crosswater": four players, every one for themselves, round a lake.
//!
//! [`Layout::Crosswater`](super::Layout::Crosswater), exactly 16 km. The land
//! is one rounded square, and a sea runs round it all, a kilometre and a half
//! out to every map edge. Each player starts toward a corner, the coast a
//! kilometre or so behind and beside them, on a raised shelf: a scarp round
//! its landward side that nothing climbs, broken by two ramps, one down to
//! the beach lowland on the bay to its east, one toward the lake; on its
//! seaward side it drops to the sea in cliffs.
//!
//! Between every two neighbours a bay runs in from the outer sea, so ships
//! can sail round the map from bay to bay and come at anyone's coast. The
//! bays lean: each runs in toward the player on its west (turning with the
//! map), whose shore is a long beach close to home; the other side's shore is
//! cliffs and sea stacks. So every player has a beach on one bay and cliffs
//! over the other. A rock islet stands in each bay's mouth.
//!
//! From each bay's head a spine of mountains runs on inland toward the lake.
//! A narrow pass cuts through it halfway; between its end and the lake an open
//! road runs round the shore. Those are the land ways between neighbours.
//!
//! In the middle lies the lake, shallow, too shallow for ships, and in it an
//! island with the richest ore on the map. Four fords lead to the island, one
//! from each player's front. Whoever holds the island is open to everyone.
//!
//! Fairness: the map is the same under a quarter turn about its middle (a
//! pinwheel, not a mirror: the bays lean the same way round). The designed
//! pieces are listed once, for the south-west player, and evaluated at the
//! point and at its three turned images ([`Terrain::cw_max`]). Noise is
//! blended between the four images across bands on the north-south and
//! east-west middle lines ([`Terrain::cw_even`]), so it is exact under the
//! turn and has no crease. Erosion is run over the whole map and evened the
//! same way.
//!
//! Positions are map metres, x east, y north, from the south-west corner.
//! Everything listed is the south-west player's, or the south bay's.

use super::bays::{dist, inside, polyline, segment, settle_gullies};
use super::canyon::{smooth_closed, smooth_open};
use super::{OreField, Pad, Terrain};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;
use std::f64::consts::{FRAC_PI_4, PI};
use std::sync::OnceLock;

/// The map's edge, metres; the layout is drawn for exactly this.
pub(super) const SIZE: f64 = 16_384.0;
/// The middle of the map, which everything turns about.
const MID: f64 = SIZE / 2.0;

/// The south-west player's start. The others are it turned a quarter at a
/// time counter-clockwise: south-east, north-east, north-west.
const START: (f64, f64) = (2_700.0, 2_700.0);

/// The south bay's coast, from beyond the south edge up the west shore (the
/// south-west player's beach) round the head and down the east shore (the
/// south-east player's cliffs), closed off the map.
const BAY: &[(f64, f64)] = &[
    (6_500.0, -700.0),
    (6_620.0, 300.0),
    (6_850.0, 1_150.0),
    (7_050.0, 2_050.0),
    (7_250.0, 2_850.0),
    (7_480.0, 3_450.0),
    (7_760.0, 3_870.0),
    (8_080.0, 4_020.0),
    (8_420.0, 3_900.0),
    (8_700.0, 3_560.0),
    (8_980.0, 3_020.0),
    (9_330.0, 2_460.0),
    (9_820.0, 1_920.0),
    (10_300.0, 1_420.0),
    (10_640.0, 820.0),
    (10_820.0, 200.0),
    (10_900.0, -700.0),
];

/// The bay's middle line, head to mouth: which shore a point is nearer.
const BAY_AXIS: [(f64, f64); 2] = [(8_000.0, 3_800.0), (8_700.0, -700.0)];

/// The outer sea round the whole map: the land is a square inset this far
/// from every edge, its corners rounded by this radius, metres. The coast
/// runs a kilometre or so behind and beside each start.
const COAST_INSET: f64 = 1_500.0;
const COAST_CORNER: f64 = 1_500.0;

/// How far `p` is out in the outer sea, metres (negative on the land).
fn outer_sea(p: (f64, f64)) -> f64 {
    let inner = MID - COAST_INSET - COAST_CORNER;
    let (qx, qy) = ((p.0 - MID).abs() - inner, (p.1 - MID).abs() - inner);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - COAST_CORNER
}

/// The rock islet in the bay's mouth: centre, radius and height, metres.
const ISLET: (f64, f64, f64, f64) = (8_750.0, 1_750.0, 190.0, 46.0);

/// The south-west player's shelf: its top, running off the map into the
/// corner. It stands [`SHELF_H`] over the lowland in a scarp.
const SHELF: &[(f64, f64)] = &[
    (700.0, 900.0),
    (1_700.0, 300.0),
    (2_900.0, 400.0),
    (3_900.0, 900.0),
    (4_400.0, 1_900.0),
    (4_200.0, 2_900.0),
    (3_700.0, 3_700.0),
    (2_800.0, 4_200.0),
    (1_800.0, 4_100.0),
    (1_000.0, 3_500.0),
    (500.0, 2_500.0),
    (400.0, 1_600.0),
];
const SHELF_H: f64 = 34.0;
/// Half the run of the shelf's scarp (m), and of a ramp down it.
const SCARP: f64 = 24.0;
const RAMP_RUN: f64 = 260.0;

/// A way cut across a slope or a range: a line across it (from `a` to `b`)
/// and its half width, metres.
struct Cut {
    a: (f64, f64),
    b: (f64, f64),
    half: f64,
}

/// The ramps down the scarp, each a line across it from top to foot: east
/// to the beach lowland, north-east toward the lake.
const RAMPS: &[Cut] = &[
    Cut {
        a: (4_000.0, 1_850.0),
        b: (4_800.0, 1_960.0),
        half: 120.0,
    },
    Cut {
        a: (3_550.0, 3_250.0),
        b: (4_150.0, 3_850.0),
        half: 120.0,
    },
];

struct Ridge {
    line: &'static [(f64, f64)],
    /// Half width of the crest's foot, metres.
    half: f64,
    /// Height of the crest over the land round it, metres.
    tall: f64,
}

/// The mountains: the spine from the south bay's head toward the lake.
const RIDGES: &[Ridge] = &[Ridge {
    line: &[
        (7_980.0, 3_960.0),
        (8_150.0, 4_450.0),
        (8_380.0, 5_180.0),
        (8_560.0, 5_800.0),
        (8_640.0, 6_150.0),
    ],
    half: 180.0,
    tall: 290.0,
}];

/// The pass through the spine: a line across it and its half width, metres.
/// It runs well past the mountains' feet either side.
const PASS: Cut = Cut {
    a: (7_760.0, 5_380.0),
    b: (9_000.0, 4_980.0),
    half: 70.0,
};

/// Broad rises the lowland stands on: centre, radius and height, metres.
/// Gentle enough to walk up anywhere.
const UPLANDS: &[(f64, f64, f64, f64)] = &[
    // A hill over the front lowland, between the shelf and the lake.
    (5_850.0, 5_950.0, 650.0, 26.0),
    // Under the cliffs of the west bay.
    (2_600.0, 6_700.0, 600.0, 16.0),
];

/// Tors: crags standing out of the lowland, too steep to climb, cover and a
/// break in the lines of sight. Centre, radius and height, metres.
const TORS: &[(f64, f64, f64, f64)] = &[
    (5_650.0, 4_250.0, 190.0, 85.0),
    (3_300.0, 6_250.0, 230.0, 110.0),
    (7_250.0, 6_000.0, 160.0, 70.0),
];

/// The lake round the middle: its radius, the island's, and the island's top.
const LAKE_R: f64 = 1_500.0;
const ISLAND_R: f64 = 470.0;
const ISLAND_H: f64 = 16.0;
/// The lake's deepest, metres: shallower than a ship draws.
const LAKE_DEEP: f64 = 5.0;
/// The fords to the island: half width, and how far out from the middle they
/// run (into the island and well onto the shore), metres. Each runs straight
/// out toward its player.
const FORD_HALF: f64 = 75.0;
const FORD_REACH: (f64, f64) = (300.0, 1_850.0);
/// The ford's crown over the water, metres: dry enough to walk.
const FORD_H: f64 = 1.6;

/// Ore away from the start: centre and radius, metres.
const ORE: &[(f64, f64, f64)] = &[
    // On the shelf, behind and toward the east ramp.
    (2_000.0, 3_350.0, 65.0),
    (3_350.0, 2_000.0, 65.0),
    (3_850.0, 2_550.0, 65.0),
    // The beach lowland.
    (6_250.0, 2_300.0, 75.0),
    (6_550.0, 3_550.0, 70.0),
    // Under the cliffs of the west bay, out of the north-east ramp.
    (2_100.0, 5_700.0, 75.0),
    // The front lowland.
    (5_300.0, 5_150.0, 75.0),
    (4_250.0, 6_350.0, 70.0),
    (6_600.0, 4_650.0, 70.0),
    // At the west mouth of the pass.
    (7_620.0, 5_250.0, 70.0),
    // Where the ford comes ashore.
    (6_740.0, 6_740.0, 70.0),
];
/// The island's field, in the middle, metres across its nominal radius.
const ISLAND_ORE: f64 = 135.0;

/// Half width (m) of the band over which noise hands over between images.
const BLEND: f64 = 500.0;
/// Depth of the open bays: seabed installations want 20 m over their lot.
const DEEP: f64 = 70.0;

/// `p` turned `k` quarters counter-clockwise about the middle.
pub(super) fn turn(p: (f64, f64), k: u32) -> (f64, f64) {
    let (mut x, mut y) = (p.0 - MID, p.1 - MID);
    for _ in 0..k % 4 {
        (x, y) = (-y, x);
    }
    (MID + x, MID + y)
}

/// The four images of `p`, itself first.
fn images(p: (f64, f64)) -> [(f64, f64); 4] {
    [turn(p, 0), turn(p, 1), turn(p, 2), turn(p, 3)]
}

/// How much of the noise at `q` is its own: 1 in the south-west quarter (the
/// one the design is drawn in), 0 in the other three, handing over across
/// the middle lines. The weights of a point's four images sum to 1.
fn own_share(q: (f64, f64)) -> f64 {
    let (dx, dy) = (q.0 - MID, q.1 - MID);
    let r = dx.hypot(dy);
    let off = (dy.atan2(dx) - 1.25 * PI).rem_euclid(2.0 * PI);
    let off = off.min(2.0 * PI - off);
    let band = (BLEND / r.max(1.0)).min(FRAC_PI_4);
    1.0 - smoothstep(FRAC_PI_4 - band, FRAC_PI_4 + band, off)
}

/// The smoothed outlines, laid once.
struct Outlines {
    bay: Vec<(f64, f64)>,
    shelf: Vec<(f64, f64)>,
    ridges: Vec<Vec<(f64, f64)>>,
}

fn outlines() -> &'static Outlines {
    static OUTLINES: OnceLock<Outlines> = OnceLock::new();
    OUTLINES.get_or_init(|| {
        // The bay is closed off the map: round its open edge only.
        let mut bay = smooth_open(BAY, 4);
        bay.push(BAY[0]);
        Outlines {
            bay,
            shelf: smooth_closed(SHELF, 4),
            ridges: RIDGES.iter().map(|r| smooth_open(r.line, 4)).collect(),
        }
    })
}

/// Signed distance from `p` to the line through `a` and `b`: positive to the
/// right going from `a` to `b`.
fn side(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (ux, uy) = (b.0 - a.0, b.1 - a.1);
    ((p.0 - a.0) * uy - (p.1 - a.1) * ux) / ux.hypot(uy)
}

impl Terrain {
    /// `f` sampled so the result is the same at a point and at its turned
    /// images: each image's own quarter takes `f` there, blended across the
    /// middle lines. `f` is noise about zero; the blend keeps its spread.
    pub(super) fn cw_even(&self, x: f64, y: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        let (mut sum, mut weights) = (0.0, 0.0);
        for q in images((x, y)) {
            let w = own_share(q);
            if w > 0.0 {
                sum += w * f(q.0, q.1);
                weights += w * w;
            }
        }
        if weights > 0.0 {
            sum / weights.sqrt()
        } else {
            f(x, y)
        }
    }

    /// The largest of a designed field at a point and its turned images.
    fn cw_max(&self, x: f64, y: f64, f: impl Fn((f64, f64)) -> f64) -> f64 {
        images((x, y)).into_iter().map(f).fold(f64::MIN, f64::max)
    }

    /// Metres in from the bays' shores (negative out to sea).
    fn cw_land(&self, x: f64, y: f64) -> f64 {
        let o = outlines();
        let sea = self
            .cw_max(x, y, |q| inside(q, &o.bay))
            .max(outer_sea((x, y)));
        let wobble = self.cw_even(x, y, |x, y| {
            self.coast.fbm(x / 1_300.0, y / 1_300.0, 4, 0.55) * 700.0
                + self.coast_warp.fbm(x / 420.0, y / 420.0, 3, 0.5) * 380.0
        });
        -sea + wobble
    }

    /// 1 in open country, falling to 0 round the starts, the ore and the
    /// fords: rough shaping keeps off what must be built on or walked.
    fn cw_keep(&self, x: f64, y: f64) -> f64 {
        let base = 1.1 * self.start_outer;
        let near = |q: (f64, f64), at: (f64, f64), reach: f64| {
            smoothstep(reach, reach + 300.0, dist(q, at))
        };
        let ford = self.cw_ford(x, y);
        let calm = images((x, y))
            .into_iter()
            .map(|q| {
                ORE.iter()
                    .map(|&(ox, oy, r)| near(q, (ox, oy), r + 40.0))
                    .fold(near(q, START, base), f64::min)
            })
            .fold(1.0, f64::min);
        calm.min(smoothstep(FORD_HALF, FORD_HALF + 250.0, ford))
    }

    /// Distance to the nearest ford's middle line, metres. Each ford wanders
    /// either side of the straight line out from the middle, the same way in
    /// each image.
    fn cw_ford(&self, x: f64, y: f64) -> f64 {
        let towards = (START.0 - MID, START.1 - MID);
        let len = towards.0.hypot(towards.1);
        let (ux, uy) = (towards.0 / len, towards.1 / len);
        let a = (MID + ux * FORD_REACH.0, MID + uy * FORD_REACH.0);
        let b = (MID + ux * FORD_REACH.1, MID + uy * FORD_REACH.1);
        let run = FORD_REACH.1 - FORD_REACH.0;
        images((x, y))
            .into_iter()
            .map(|q| {
                let (d, t) = segment(q, a, b);
                if t <= 0.0 || t >= 1.0 {
                    return d;
                }
                let along = t * run;
                let wander = (PI * t).sin()
                    * (110.0 * (along / 300.0 + 1.3).sin() + 45.0 * (along / 120.0).sin());
                (side(q, a, b) - wander).abs()
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// The shelf's rise, 0 on the lowland to 1 on its top.
    fn cw_shelf(&self, x: f64, y: f64) -> f64 {
        let o = outlines();
        let fray = self.cw_even(x, y, |x, y| {
            self.mtn_mask.fbm(x / 900.0 + 5.0, y / 900.0, 2, 0.5) * 900.0
        });
        self.cw_max(x, y, |q| {
            let ramp = RAMPS
                .iter()
                .map(|r| smoothstep(r.half + 70.0, r.half, segment(q, r.a, r.b).0))
                .fold(0.0, f64::max);
            let run = SCARP + (RAMP_RUN - SCARP) * ramp;
            smoothstep(-run, run, inside(q, &o.shelf) + fray * (1.0 - ramp))
        })
    }

    /// The land before erosion.
    fn cw_shape(&self, x: f64, y: f64) -> f64 {
        let s = self.cw_land(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.cw_even(x, y, f);
        let keep = self.cw_keep(x, y);
        let inland = smoothstep(150.0, 2_800.0, s);
        // The lowland: low along the shores, higher inland, rolling hills
        // (calmed round the bases, the ore and the fords) and faint grain.
        let mut land = 16.0 + 12.0 * inland;
        let warp = n(&|x, y| self.warp_x.fbm(x / 2_000.0, y / 2_000.0, 2, 0.5)) * 900.0;
        let hills = n(&|x, y| {
            self.tilt
                .fbm((x + warp) / 950.0, (y - warp) / 950.0, 3, 0.5)
        });
        land += 70.0 * hills * keep.max(0.25) * smoothstep(80.0, 700.0, s);
        land += 14.0 * n(&|x, y| self.detail.fbm(x / 330.0, y / 330.0, 3, 0.5)) * keep;
        // No hollow holds water: the valleys floor out softly above the sand.
        let low = land - 11.0;
        land = 11.0 + 0.5 * (low + (low * low + 16.0).sqrt());
        land += 0.9 * n(&|x, y| self.detail.fbm(x / 70.0 + 9.0, y / 70.0 - 4.0, 2, 0.5));
        let fray = n(&|x, y| self.mtn_mask.fbm(x / 1_600.0, y / 1_600.0, 3, 0.5)) * 1_200.0;
        land += self.cw_max(x, y, |q| {
            UPLANDS
                .iter()
                .map(|&(cx, cy, r, tall)| {
                    tall * smoothstep(r + 400.0, 0.3 * r, dist(q, (cx, cy)) + fray)
                })
                .fold(0.0, f64::max)
        });
        let shelf = self.cw_shelf(x, y);
        land += SHELF_H * shelf;
        let mut h = self.cw_shore(x, y, s, land, shelf);
        let crag = n(&|x, y| self.crag.ridged(x / 700.0, y / 700.0, 4, 0.5) - 0.55) + 0.55;
        // The mountains stop at the water in cliffs rather than filling it.
        h += self.cw_ranges(x, y, crag) * smoothstep(-350.0, 80.0, s);
        // The bays' islets: sheer sides, a broken crown, from the sea floor.
        let rough = n(&|x, y| self.crag.ridged(x / 140.0 - 3.0, y / 140.0 + 8.0, 3, 0.5) - 0.55);
        h = h.max(self.cw_max(x, y, |q| {
            let (ix, iy, r, tall) = ISLET;
            let d = dist(q, (ix, iy)) + 40.0 * rough;
            let rise = smoothstep(r + 14.0, 0.5 * r, d).powf(0.5);
            if rise > 0.0 {
                -DEEP + (tall + DEEP) * (1.0 + 0.6 * (crag - 0.4)) * rise
            } else {
                f64::MIN
            }
        }));
        // The tors: sheer flanks, a broken crown, a scree skirt.
        h += self.cw_max(x, y, |q| {
            TORS.iter()
                .map(|&(tx, ty, r, tall)| {
                    let d = dist(q, (tx, ty)) + 60.0 * rough + 0.4 * ragged_of(r, q, (tx, ty));
                    let body = smoothstep(r + 30.0, 0.45 * r, d);
                    let skirt = smoothstep(r + 160.0, r, d);
                    tall * (1.0 + 0.8 * (crag - 0.55)) * body.powf(0.7) + 6.0 * skirt
                })
                .fold(0.0, f64::max)
        });
        self.cw_lake(x, y, h)
    }

    /// The land meeting the bays: a long beach on each bay's west shore,
    /// cliffs and sea stacks down its east shore toward the mouth.
    fn cw_shore(&self, x: f64, y: f64, s: f64, land: f64, shelf: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.cw_even(x, y, f);
        // In the frame of the nearest bay: its own image is the one inside
        // (or least outside) its outline.
        let o = outlines();
        let (bay, q) = images((x, y))
            .into_iter()
            .map(|q| (inside(q, &o.bay), q))
            .fold((f64::MIN, (x, y)), |a, b| if b.0 > a.0 { b } else { a });
        // The bay's cliffs only where the bay is the nearer water; on the outer
        // coast the shelves drop to the sea in cliffs and the lowland in beaches.
        let in_bay = smoothstep(-200.0, 200.0, bay - outer_sea((x, y)));
        let east = side(q, BAY_AXIS[0], BAY_AXIS[1]);
        let vary = n(&|x, y| self.coast_warp.fbm(x / 2_400.0 + 30.0, y / 2_400.0, 3, 0.5));
        let cliffy = (smoothstep(-200.0, 300.0, -east)
            * smoothstep(3_300.0, 2_400.0, q.1 + 1_500.0 * vary)
            * in_bay)
            .max(smoothstep(0.3, 0.8, shelf));
        let run = 260.0 - 210.0 * cliffy;
        // The sea floor: a shelf off the beaches, a drop off the cliffs, bars.
        let shelf = 750.0 - 480.0 * cliffy;
        let mut sea = -DEEP * smoothstep(0.0, shelf, -s).powf(0.8);
        sea += 5.0
            * n(&|x, y| self.lake.fbm(x / 700.0, y / 700.0, 3, 0.5))
            * smoothstep(-60.0, -500.0, s);
        let w = smoothstep(-25.0, run, s);
        let h = sea.min(0.2) * (1.0 - w) + land * w;
        // Sea stacks off the cliffs; the floor round them keeps its depth.
        let stack = n(&|x, y| self.crag.get(x / 170.0 - 7.0, y / 170.0 + 3.0));
        let band = smoothstep(-520.0, -260.0, s) * (1.0 - smoothstep(-90.0, -30.0, s));
        let rise = smoothstep(0.5, 0.66, stack) * band * cliffy;
        h.max(-8.0 + 40.0 * rise - 200.0 * (1.0 - smoothstep(0.0, 0.15, rise)))
    }

    /// The mountains' height over the land they stand on (as in
    /// `bays_ranges`), with the spine's pass cut through.
    fn cw_ranges(&self, x: f64, y: f64, crag: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.cw_even(x, y, f);
        let o = outlines();
        let peaks = n(&|x, y| self.ridge.fbm(x / 1_100.0 + 17.0, y / 1_100.0, 3, 0.5));
        let ragged = n(&|x, y| self.mtn_gap.fbm(x / 700.0, y / 700.0, 4, 0.55)) * 450.0;
        let arete =
            n(&|x, y| self.mtn.ridged(x / 900.0 + 3.3, y / 900.0 - 1.9, 3, 0.5) - 0.55) + 0.55;
        let spur =
            n(&|x, y| self.mtn.ridged(x / 330.0 - 6.1, y / 330.0 + 4.4, 3, 0.5) - 0.55) + 0.55;
        let rock =
            n(&|x, y| self.crag.ridged(x / 150.0 + 11.0, y / 150.0 - 5.0, 3, 0.5) - 0.55) + 0.55;
        self.cw_max(x, y, |q| {
            let pass = smoothstep(PASS.half, PASS.half + 160.0, segment(q, PASS.a, PASS.b).0);
            RIDGES
                .iter()
                .zip(&o.ridges)
                .map(|(r, line)| {
                    let d = polyline(q, line) + ragged.clamp(-0.6 * r.half, 0.6 * r.half);
                    let len: f64 = r.line.windows(2).map(|w| dist(w[0], w[1])).sum();
                    let end = dist(q, r.line[0]).min(dist(q, r.line[r.line.len() - 1]));
                    let crest = 0.55 + 0.45 * smoothstep(0.0, (0.45 * len).min(600.0), end);
                    let tall =
                        r.tall * crest * (1.0 + 0.35 * peaks).max(0.8) * (1.0 + 0.6 * (crag - 0.6));
                    let m = smoothstep(r.half + 40.0, -0.3 * r.half, d);
                    let sharp = smoothstep(0.2, 0.8, m);
                    tall.max(0.0)
                        * (m * (1.0 + sharp * (0.55 * arete * arete - 0.2))
                            + 0.35 * sharp * m * (spur - 0.6))
                        + 22.0 * sharp * (rock - 0.45).max(0.0)
                })
                .fold(0.0, |a: f64, b: f64| {
                    let k = 30.0 * smoothstep(0.0, 60.0, a.min(b)) + 1e-9;
                    let t = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
                    a + (b - a) * t + k * t * (1.0 - t)
                })
                * pass
        })
    }

    /// The lake in the middle, its island and the fords out to it.
    fn cw_lake(&self, x: f64, y: f64, mut h: f64) -> f64 {
        let n = |f: &dyn Fn(f64, f64) -> f64| self.cw_even(x, y, f);
        let r = (x - MID).hypot(y - MID);
        if r > LAKE_R + 2_500.0 {
            return h;
        }
        // Coves and points round the shore: only harmonics that repeat every
        // quarter turn, and none as slow as the fourth, which squares a circle.
        let a = (y - MID).atan2(x - MID);
        let fray = LAKE_R
            * (0.045 * (8.0 * a + 0.7).sin()
                + 0.04 * (12.0 * a + 2.1).sin()
                + 0.025 * (20.0 * a + 4.0).sin())
            + n(&|x, y| {
                self.lake_shore
                    .fbm(x / 240.0 + 13.0, y / 240.0 - 7.0, 3, 0.5)
            }) * 320.0
            + n(&|x, y| self.lake_shore.fbm(x / 70.0 - 21.0, y / 70.0 + 3.0, 2, 0.5)) * 50.0;
        let lake = LAKE_R + fray - r;
        // The land slopes down into the lake's hollow; low bluffs here and there.
        let bluff = smoothstep(
            0.02,
            0.2,
            n(&|x, y| self.lake_shore.fbm(x / 700.0 + 40.0, y / 700.0, 2, 0.5)),
        );
        let bank = 110.0 - 70.0 * bluff;
        let rim = 10.5 * smoothstep(0.0, bank, -lake);
        // Within a kilometre or so of the shore: beyond, the country is its own.
        let back = (-lake - bank).max(0.0);
        h = h.min(0.4 + rim + 0.03 * back + 1_000.0 * smoothstep(700.0, 1_300.0, back));
        let bars = 1.0 * n(&|x, y| self.lake.fbm(x / 260.0 + 5.0, y / 260.0, 3, 0.5));
        let floor = (-0.6 - (LAKE_DEEP - 0.6) * smoothstep(0.0, 260.0, lake) + bars)
            .clamp(-LAKE_DEEP, -0.3);
        h += (floor - h) * smoothstep(-6.0, 6.0, lake);
        // The island: a broad low mound, level on top for building.
        let edge =
            ISLAND_R + n(&|x, y| self.coast.fbm(x / 300.0 - 9.0, y / 300.0 + 2.0, 3, 0.5)) * 160.0;
        let island = smoothstep(edge + 40.0, edge - 220.0, r);
        if island > 0.0 {
            h = h.max(-LAKE_DEEP + (ISLAND_H + LAKE_DEEP) * island.powf(0.6));
        }
        // The fords: a low causeway of gravel, crowned, falling into the water.
        let d = self.cw_ford(x, y) + 12.0 * n(&|x, y| self.detail.fbm(x / 90.0, y / 90.0, 2, 0.5));
        if d < FORD_HALF + 80.0 {
            h = h.max(
                FORD_H - (FORD_H + LAKE_DEEP) * smoothstep(FORD_HALF - 15.0, FORD_HALF + 80.0, d),
            );
        }
        h
    }

    pub(super) fn natural_crosswater(&self, x: f64, y: f64) -> f64 {
        let h = self.cw_shape(x, y);
        if self.erosion.delta.is_empty() || h < 0.5 {
            return h;
        }
        let e = self.erosion.at(x, y) * self.cw_keep(x, y) * smoothstep(0.5, 6.0, h);
        (h + e).max(h.min(2.5))
    }

    /// Water erosion over the whole country, from the shaped land, then made
    /// exact under the quarter turn the way the noise is.
    fn erode_crosswater(&mut self) {
        const STEP: f64 = 16.0;
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
                            *v = t.cw_shape(i as f64 * STEP, y).max(-2.0) as f32 / VERTICAL;
                        }
                    }
                });
            }
        });
        let before = h.clone();
        super::alpine::erode(&mut h, n, self.seed);
        super::alpine::erode(&mut h, n, self.seed ^ 0x6372_6F73);
        let soft = settle_gullies(&h, &before, n, STEP, VERTICAL);
        // The grid's middle sample is the map's middle, so a quarter turn
        // takes samples to samples: blend each with its images.
        let mut delta = vec![0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                let mut at = (i, j);
                let (mut sum, mut weights) = (0.0, 0.0);
                for _ in 0..4 {
                    let w = own_share((at.0 as f64 * STEP, at.1 as f64 * STEP));
                    sum += w * soft[at.1 * n + at.0] as f64;
                    weights += w;
                    at = (n - 1 - at.1, at.0);
                }
                delta[j * n + i] = if weights > 0.0 {
                    (sum / weights) as f32
                } else {
                    soft[j * n + i]
                };
            }
        }
        self.erosion = super::alpine::Erosion {
            n,
            step: STEP,
            delta,
        };
    }

    /// How thickly trees grow, and how much of it is conifer. Exact under the
    /// quarter turn: timber is income.
    pub(super) fn cw_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let broad = self.cw_even(x, y, |x, y| self.forest.fbm(x / l, y / l, 3, 0.5));
        let copse = self.cw_even(x, y, |x, y| {
            self.forest
                .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5)
        });
        let clearing = self.cw_even(x, y, |x, y| {
            self.forest
                .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5)
        });
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(0.42, 0.62, copse);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let mut habitable = smoothstep(3.0, 8.0, height)
            * (1.0 - smoothstep(0.30, 0.55, slope))
            * (1.0 - smoothstep(220.0, 280.0, height));
        // A glade round every ore field, and the fords' landings kept open.
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        habitable *= smoothstep(FORD_HALF + 40.0, FORD_HALF + 260.0, self.cw_ford(x, y));
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable;
        // Species do not change play: pines up high, broadleaf by the water.
        let cold = smoothstep(30.0, 110.0, height) * 0.9
            + self.forest_kind.fbm(x / 1500.0, y / 1500.0, 2, 0.5) * 0.9;
        (density, smoothstep(-0.15, 0.35, cold))
    }

    pub(super) fn setup_crosswater(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = -0.06;
        self.erode_crosswater();

        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let first = snap(START);
        self.starts = images(first).to_vec();
        let height = self.natural(first.0, first.1).max(12.0);
        self.pads = self
            .starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: start_core,
                outer: 2.0 * start_core,
                height,
            })
            .collect();

        // Three small fields round each base, inside its pad: one behind
        // (toward the corner), two on the forward flanks.
        let mut sites = Vec::new();
        let back = (first.1 - MID).atan2(first.0 - MID);
        for turn_by in [0.0, PI - 1.15, PI + 1.15] {
            let (s, c) = (back + turn_by).sin_cos();
            let d = 0.8 * start_core;
            sites.push((
                (first.0 + c * d, first.1 + s * d),
                (0.2 * start_core).max(55.0),
            ));
        }
        sites.extend(ORE.iter().map(|&(x, y, r)| ((x, y), r)));
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = snap(p);
            let field = self.ore_field(p.0, p.1, r);
            for k in 1..4 {
                fields.push(turned_field(&field, k));
            }
            fields.push(field);
        }
        // The island's field: one quarter of its outline, turned round.
        let whole = self.ore_field(MID, MID, ISLAND_ORE);
        let quarter = whole.corners.len() / 4;
        let corners = (0..4)
            .flat_map(|k| whole.corners[..quarter].iter().map(move |&c| turn(c, k)))
            .collect();
        fields.push(OreField { corners, ..whole });
        self.ore = fields;
        self.fit_forests();
    }
}

/// A tor's outline wobble at `q`, metres: lobes round its centre.
fn ragged_of(r: f64, q: (f64, f64), c: (f64, f64)) -> f64 {
    let a = (q.1 - c.1).atan2(q.0 - c.0);
    r * (0.25 * (3.0 * a + c.0).sin() + 0.15 * (5.0 * a + c.1).sin())
}

/// An ore field turned `k` quarters, corner for corner.
fn turned_field(f: &OreField, k: u32) -> OreField {
    let (x, y) = turn((f.x, f.y), k);
    OreField {
        x,
        y,
        radius: f.radius,
        corners: f.corners.iter().map(|&c| turn(c, k)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{images, own_share, turn, MID};

    #[test]
    fn the_shares_of_a_point_and_its_images_sum_to_one() {
        for &(x, y) in &[
            (100.0, 200.0),
            (8_000.0, 8_300.0),
            (8_192.0, 3_000.0),
            (15_000.0, 9_000.0),
        ] {
            let sum: f64 = images((x, y)).into_iter().map(own_share).sum();
            assert!((sum - 1.0).abs() < 1e-9, "{sum} at {x},{y}");
        }
        assert_eq!(turn(turn((1.0, 2.0), 3), 1), (1.0, 2.0));
        assert_eq!(turn((MID, MID), 1), (MID, MID));
    }

    #[test]
    fn crosswater_is_the_same_under_a_quarter_turn() {
        let t = &*crate::bake::test_maps::CROSSWATER;
        let n = 61;
        for j in 0..n {
            for i in 0..n {
                let p = (
                    (i as f64 + 0.37) * t.size_x / n as f64,
                    (j as f64 + 0.61) * t.size_y / n as f64,
                );
                let a = t.height(p.0, p.1);
                let fa = t.cw_forest(p.0, p.1, a, 0.1).0;
                for k in 1..4 {
                    let q = turn(p, k);
                    let b = t.height(q.0, q.1);
                    assert!((a - b).abs() < 1e-6, "{a} vs {b} at {p:?} turned {k}");
                    let fb = t.cw_forest(q.0, q.1, b, 0.1).0;
                    assert!((fa - fb).abs() < 1e-6, "woods {fa} vs {fb} at {p:?}");
                }
            }
        }
    }

    #[test]
    fn noise_has_no_crease_on_the_middle_lines() {
        let t = &*crate::bake::test_maps::CROSSWATER;
        // Across the line x = middle, north of the lake: the ground steps from
        // one metre to the next across the line as it does a metre beside it.
        for k in 0..40 {
            let y = MID + 1_600.0 + k as f64 * 150.0;
            let at = |x: f64| t.natural(x, y);
            let across = at(MID + 0.5) - at(MID - 0.5);
            let beside = at(MID + 1.5) - at(MID + 0.5);
            assert!(
                (across - beside).abs() < 0.5,
                "crease {across} vs {beside} at {MID},{y}"
            );
        }
    }
}

/// `CROSSWATER_RELIEF=x0,y0,span,px,out.ppm cargo test --profile gate -p mc-map --lib crosswater_relief -- --ignored`:
/// a hillshade of the land (sun from the north-west, water tinted, ground a
/// unit cannot climb grey), to judge the landforms without the game, and the
/// heights beside it as little-endian f32 rows, north first (`out.ppm.f32`).
#[cfg(test)]
#[test]
#[ignore]
fn crosswater_relief() {
    let spec =
        std::env::var("CROSSWATER_RELIEF").unwrap_or_else(|_| "0,0,16384,1024,relief.ppm".into());
    let v: Vec<&str> = spec.split(',').collect();
    let (x0, y0, span, px): (f64, f64, f64, usize) = (
        v[0].parse().unwrap(),
        v[1].parse().unwrap(),
        v[2].parse().unwrap(),
        v[3].parse().unwrap(),
    );
    let t = &*crate::bake::test_maps::CROSSWATER;
    let step = span / px as f64;
    let row = |j: usize| {
        let (mut rgb, mut raw) = (Vec::new(), Vec::new());
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
            let l = (gx * gx + gy * gy + 1.0).sqrt();
            let shade = ((gx * 0.5 - gy * 0.5 + 0.7) / l / 0.95).clamp(0.0, 1.0);
            let slope = (gx * gx + gy * gy).sqrt();
            let (r, g, b) = if h < 0.0 {
                (40.0, 70.0 + h.max(-45.0), 120.0 + h.max(-45.0))
            } else if slope > 0.5 {
                (150.0, 140.0, 130.0)
            } else {
                let k = (h / 120.0).clamp(0.0, 1.0);
                (110.0 + 90.0 * k, 150.0 + 30.0 * k, 90.0 + 60.0 * k)
            };
            rgb.extend([(r * shade) as u8, (g * shade) as u8, (b * shade) as u8]);
        }
        (rgb, raw)
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut rows: Vec<(Vec<u8>, Vec<u8>)> = vec![Default::default(); px];
    std::thread::scope(|s| {
        for (k, chunk) in rows.chunks_mut(px.div_ceil(threads)).enumerate() {
            let row = &row;
            s.spawn(move || {
                for (r, out) in chunk.iter_mut().enumerate() {
                    *out = row(k * px.div_ceil(threads) + r);
                }
            });
        }
    });
    let mut out = format!("P6 {px} {px} 255\n").into_bytes();
    let mut raw = Vec::new();
    for (rgb, r) in rows {
        out.extend(rgb);
        raw.extend(r);
    }
    std::fs::write(v[4], out).unwrap();
    std::fs::write(format!("{}.f32", v[4]), raw).unwrap();
}
