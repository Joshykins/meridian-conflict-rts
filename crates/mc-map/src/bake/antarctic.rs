//! "Erebus Strait": two against two across a polar sea.
//!
//! [`Layout::Antarctic`](super::Layout::Antarctic), exactly 12 km. Two
//! coasts of Antarctica face each other across an open strait that runs off
//! the map to the north and the south; nothing grows anywhere. Each team holds
//! one coast, its two starts on ice-free coastal lowland, one toward each end.
//!
//! Behind each coast the ice sheet stands over the lowland in an ice wall
//! forty or fifty metres high, broken by one ramp behind each start; on top it
//! rises in long swells inland, nunataks breaking through it, and along the
//! map's edges it runs up into mountains. In the middle of each
//! coast it bulges forward into two massifs of mountains, a snowy valley
//! between them running down to the shore. The allies meet along the coast
//! (a strip of shore runs under the massifs' seaward ends and across the
//! valley's foot) or over the sheet, behind the mountains.
//!
//! In the strait lie two volcanic islands, each nearer one coast: long and
//! lobed, beaches of dark sand and flats either side of a broken ridge, a
//! volcano at one end. Tabular icebergs lie aground here and there.
//!
//! Thick sea ice, flat and a metre over the water, reaches out from each
//! coast, a kilometre or more in places and held back in front of the starts
//! to keep their harbours open; each island has a fringe of its own. Units walk and build on it. Open water always
//! parts the coast's ice from the islands' and runs the length of the strait
//! between the two coasts' ice, so the teams never meet on foot.
//!
//! Fairness: the map is the same under a half turn about its middle. The
//! designed pieces are listed once, for the west coast and the islands' first
//! one, and evaluated at a point and at its turned image
//! ([`Terrain::an_max`]). Noise is blended between the two images across a
//! band on the north-south middle line ([`Terrain::an_even`]), so it is exact
//! under the turn without a crease. Erosion runs over the whole map and is
//! evened the same way.
//!
//! Positions are map metres, x east, y north, from the south-west corner.

use super::alpine::{erode, Erosion};
use super::bays::{dist, inside, polyline, segment, settle_gullies};
use super::canyon::{smooth_closed, smooth_open};
use super::{OreField, Pad, Terrain};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;
use std::f64::consts::PI;
use std::sync::OnceLock;

/// The map's edge, metres; the layout is drawn for exactly this.
pub(super) const SIZE: f64 = 12_288.0;
/// The middle of the map, which everything turns about.
const MID: f64 = SIZE / 2.0;

/// The west team's starts, south first. Each is followed in the start list
/// by its turned image on the east coast, so the teams alternate.
const STARTS: [(f64, f64); 2] = [(1_850.0, 2_700.0), (1_940.0, 9_500.0)];

/// The west coast's land, closed off the map: from beyond the south edge up
/// the coast to beyond the north edge.
const WEST: &[(f64, f64)] = &[
    (-900.0, -900.0),
    (3_430.0, -900.0),
    (3_390.0, 500.0),
    (3_170.0, 1_300.0),
    (2_860.0, 2_000.0),
    (2_900.0, 2_700.0),
    (3_170.0, 3_300.0),
    (3_520.0, 3_900.0),
    (3_740.0, 4_600.0),
    (3_650.0, 5_200.0),
    (3_780.0, 5_700.0),
    (3_850.0, 6_150.0),
    (3_780.0, 6_600.0),
    (3_700.0, 7_100.0),
    (3_830.0, 7_700.0),
    (3_560.0, 8_300.0),
    (3_080.0, 8_800.0),
    (2_950.0, 9_500.0),
    (3_170.0, 10_200.0),
    (3_480.0, 10_800.0),
    (3_300.0, 11_500.0),
    (3_480.0, 12_300.0),
    (3_610.0, 13_200.0),
    (-900.0, 13_200.0),
];

/// The ice sheet's edge over the lowland, closed off the map to the west. It
/// bulges forward between the starts, behind the massifs, and at
/// the north and south ends of the coast it runs out over the sea and ends
/// in ice cliffs.
const ICE: &[(f64, f64)] = &[
    (-900.0, -900.0),
    (3_960.0, -900.0),
    (3_830.0, 150.0),
    (3_080.0, 550.0),
    (2_200.0, 700.0),
    (1_320.0, 950.0),
    (880.0, 1_400.0),
    (750.0, 1_800.0),
    (660.0, 2_900.0),
    (840.0, 3_800.0),
    (1_410.0, 4_500.0),
    (2_020.0, 5_000.0),
    (2_200.0, 5_600.0),
    (2_240.0, 6_150.0),
    (2_200.0, 6_700.0),
    (2_020.0, 7_300.0),
    (1_410.0, 7_800.0),
    (840.0, 8_500.0),
    (660.0, 9_500.0),
    (750.0, 10_500.0),
    (970.0, 11_150.0),
    (1_580.0, 11_500.0),
    (2_380.0, 11_550.0),
    (3_080.0, 11_750.0),
    (3_740.0, 12_150.0),
    (3_920.0, 13_200.0),
    (-900.0, 13_200.0),
];
/// The sheet's top at its edge over the lowland, and how much it rises per
/// metre inland, metres.
const SHEET_EDGE_H: f64 = 72.0;
const SHEET_RISE: f64 = 0.028;
/// Half the run of the ice wall (m), and of a ramp up it.
const WALL: f64 = 15.0;
const RAMP_RUN: f64 = 300.0;

/// A way cut across a slope: a line across it (from `a`, the foot, to `b`) and
/// its half width, metres.
struct Cut {
    a: (f64, f64),
    b: (f64, f64),
    half: f64,
}

/// The ramps up the ice wall, one behind each start.
const RAMPS: &[Cut] = &[
    Cut {
        a: (1_100.0, 3_350.0),
        b: (450.0, 3_250.0),
        half: 110.0,
    },
    Cut {
        a: (1_140.0, 10_050.0),
        b: (500.0, 10_150.0),
        half: 110.0,
    },
];

struct Ridge {
    line: &'static [(f64, f64)],
    /// Half width of the crest's foot, metres.
    half: f64,
    /// Height of the crest over the ground round it, metres.
    tall: f64,
}

/// The massifs either side of the valley, from the sheet toward the coast (a
/// strip of shore runs under their seaward ends), and the ranges along the
/// map's edges.
const RIDGES: &[Ridge] = &[
    Ridge {
        line: &[
            (1_980.0, 4_700.0),
            (2_510.0, 5_000.0),
            (2_860.0, 5_250.0),
            (3_040.0, 5_380.0),
        ],
        half: 330.0,
        tall: 360.0,
    },
    Ridge {
        line: &[
            (1_980.0, 7_600.0),
            (2_510.0, 7_300.0),
            (2_860.0, 7_050.0),
            (3_040.0, 6_920.0),
        ],
        half: 320.0,
        tall: 420.0,
    },
    // Along the map's edge behind the sheet, and round its corner where the
    // sheet runs out over the sea: the edges are mountains, not empty ice.
    Ridge {
        line: &[
            (-140.0, -700.0),
            (-60.0, 2_400.0),
            (-160.0, 6_100.0),
            (-70.0, 9_800.0),
            (-150.0, 13_000.0),
        ],
        half: 280.0,
        tall: 260.0,
    },
    Ridge {
        line: &[(-400.0, -140.0), (1_400.0, -60.0), (3_000.0, -150.0)],
        half: 260.0,
        tall: 220.0,
    },
    Ridge {
        line: &[(-400.0, 12_430.0), (1_300.0, 12_360.0), (2_900.0, 12_440.0)],
        half: 260.0,
        tall: 200.0,
    },
];

/// Nunataks through the sheet and tors on the lowland: centre, radius and
/// height over the ground round them, metres.
const NUNATAKS: &[(f64, f64, f64, f64)] = &[
    (400.0, 5_300.0, 230.0, 170.0),
    (750.0, 7_250.0, 190.0, 130.0),
    (310.0, 1_500.0, 170.0, 110.0),
    (350.0, 11_000.0, 210.0, 150.0),
];
const TORS: &[(f64, f64, f64, f64)] = &[
    (1_190.0, 3_500.0, 150.0, 60.0),
    (3_120.0, 1_250.0, 130.0, 50.0),
    (1_320.0, 8_700.0, 150.0, 60.0),
    (1_280.0, 10_800.0, 170.0, 70.0),
];

/// Where the ground is designed to be the same under the turn however the
/// coast and the mountains wander: the strips of shore under the massifs'
/// seaward ends, from each lowland to the valley's foot. A line and its
/// half width, metres.
const TAME: &[Cut] = &[
    Cut {
        a: (3_260.0, 4_200.0),
        b: (3_780.0, 6_150.0),
        half: 550.0,
    },
    Cut {
        a: (3_780.0, 6_150.0),
        b: (3_260.0, 8_000.0),
        half: 550.0,
    },
];

/// The first island: its middle, and how far its long axis is turned from
/// east (radians). Its own frame (`island_local`) runs along that axis.
const ISLAND: (f64, f64) = (5_850.0, 8_700.0);
const ISLAND_TURN: f64 = 1.2;
/// The island's coast in its own frame, metres: long and lobed, a bay on
/// each side.
const ISLAND_SHAPE: &[(f64, f64)] = &[
    (-1_420.0, 70.0),
    (-1_280.0, 400.0),
    (-950.0, 580.0),
    (-610.0, 530.0),
    (-400.0, 320.0),
    (-160.0, 450.0),
    (200.0, 700.0),
    (610.0, 650.0),
    (950.0, 510.0),
    (1_280.0, 270.0),
    (1_460.0, -30.0),
    (1_320.0, -350.0),
    (1_010.0, -510.0),
    (680.0, -450.0),
    (430.0, -630.0),
    (0.0, -760.0),
    (-400.0, -650.0),
    (-810.0, -570.0),
    (-1_150.0, -350.0),
    (-1_350.0, -160.0),
];
/// How far the island reaches from its middle at most, metres.
const ISLAND_REACH: f64 = 1_500.0;
/// The ridge down its middle in its own frame, its half width and height,
/// and the volcano at its east end: centre, radius and height, metres.
const SPINE: &[(f64, f64)] = &[
    (-950.0, -40.0),
    (-420.0, 150.0),
    (120.0, 60.0),
    (640.0, -80.0),
];
const SPINE_HALF: f64 = 170.0;
const SPINE_TALL: f64 = 115.0;
const CONE: (f64, f64, f64, f64) = (900.0, -60.0, 320.0, 250.0);

/// Tabular icebergs aground: centre, radius and top over the water, metres.
const BERGS: &[(f64, f64, f64, f64)] = &[
    (4_900.0, 1_900.0, 170.0, 22.0),
    (5_250.0, 4_300.0, 140.0, 20.0),
    (5_350.0, 7_400.0, 120.0, 18.0),
    (5_600.0, 10_900.0, 200.0, 24.0),
    (4_700.0, 11_800.0, 130.0, 20.0),
    (5_900.0, 5_000.0, 110.0, 18.0),
];

/// The sea ice: its top over the water (m: dry, flat, walkable), how far it
/// reaches out from the coast at the least and the most (m), how far in front
/// of a start it is held back to keep the start's harbour open (the reach
/// there, m, within the radius, m), how wide a fringe it lays round the
/// islands (m), and the open lead it always leaves down the strait (m either
/// side of the middle line): the teams never meet on the ice.
const SEA_ICE_H: f64 = 1.3;
/// The sea bed under the edges of the sea ice and round the bergs' feet,
/// metres: deep enough for ships (6 m).
const UNDER_EDGE: f64 = -9.0;
const SEA_ICE_REACH: (f64, f64) = (200.0, 780.0);
const HARBOUR: (f64, f64) = (180.0, 1_900.0);
const ISLAND_ICE: f64 = 210.0;
const LEAD: f64 = 380.0;
/// Open water kept between the coast's ice and an island's, metres.
const ISLAND_WATER: f64 = 380.0;

/// Ore away from the starts: centre and radius, metres.
const ORE: &[(f64, f64, f64)] = &[
    // The south start's lowland, and the shore under the south massif.
    (2_680.0, 1_800.0, 65.0),
    (1_320.0, 1_900.0, 65.0),
    (2_550.0, 3_700.0, 70.0),
    (1_940.0, 4_150.0, 70.0),
    (3_340.0, 4_800.0, 60.0),
    // The north start's.
    (2_680.0, 10_600.0, 65.0),
    (1_320.0, 10_300.0, 65.0),
    (2_770.0, 8_800.0, 70.0),
    (1_890.0, 8_250.0, 70.0),
    (3_260.0, 7_450.0, 60.0),
    // Moraine by the nunataks on the sheet.
    (750.0, 5_650.0, 60.0),
    (1_100.0, 6_900.0, 60.0),
    // Down the valley between the massifs.
    (3_080.0, 6_150.0, 70.0),
];
/// The island's fields in its own frame: centre and radius, metres, on the
/// flats either side of the ridge.
const ISLAND_ORE: &[(f64, f64, f64)] = &[
    (-900.0, -360.0, 65.0),
    (-150.0, 450.0, 70.0),
    (380.0, -420.0, 65.0),
];

/// Half width (m) of the band over which noise hands over between images.
const BLEND: f64 = 500.0;
/// Depth of the open strait: seabed installations want 20 m over their lot.
const DEEP: f64 = 80.0;

/// `p` turned half round the middle.
pub(super) fn turn(p: (f64, f64)) -> (f64, f64) {
    (SIZE - p.0, SIZE - p.1)
}

/// How much of the noise at `q` is its own: 1 on the west side, 0 on the
/// east, handing over across the middle line. A point's and its image's sum to 1.
fn own_share(q: (f64, f64)) -> f64 {
    1.0 - smoothstep(MID - BLEND, MID + BLEND, q.0)
}

/// The smoothed outlines, laid once.
struct Outlines {
    west: Vec<(f64, f64)>,
    ice: Vec<(f64, f64)>,
    ridges: Vec<Vec<(f64, f64)>>,
    island: Vec<(f64, f64)>,
    spine: Vec<(f64, f64)>,
}

fn outlines() -> &'static Outlines {
    static OUTLINES: OnceLock<Outlines> = OnceLock::new();
    OUTLINES.get_or_init(|| Outlines {
        west: smooth_closed(WEST, 4),
        ice: smooth_closed(ICE, 4),
        ridges: RIDGES.iter().map(|r| smooth_open(r.line, 4)).collect(),
        island: smooth_closed(ISLAND_SHAPE, 4),
        spine: smooth_open(SPINE, 4),
    })
}

/// A point at `bearing` and `out` metres from `c`.
fn polar(c: (f64, f64), bearing: f64, out: f64) -> (f64, f64) {
    (c.0 + out * bearing.cos(), c.1 + out * bearing.sin())
}

/// A map point in the first island's own frame.
fn island_local(q: (f64, f64)) -> (f64, f64) {
    let (s, c) = ISLAND_TURN.sin_cos();
    let (dx, dy) = (q.0 - ISLAND.0, q.1 - ISLAND.1);
    (dx * c + dy * s, -dx * s + dy * c)
}

/// A point of the first island's own frame on the map.
fn island_map(l: (f64, f64)) -> (f64, f64) {
    let (s, c) = ISLAND_TURN.sin_cos();
    (ISLAND.0 + l.0 * c - l.1 * s, ISLAND.1 + l.0 * s + l.1 * c)
}

/// The ice sheet at a point: its top, how far it covers the point (0..1)
/// and how far the point is inside it (m, negative outside).
struct IceAt {
    sheet_top: f64,
    sheet: f64,
    /// How far the sheet's ice covers the point, its wall down to the foot.
    cover: f64,
    within: f64,
}

impl Terrain {
    /// `f` sampled so the result is the same at a point and at its turned
    /// image: each side takes `f` at its own point, blended across the middle
    /// line. `f` is noise about zero; the blend keeps its spread.
    pub(super) fn an_even(&self, x: f64, y: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        let p = (x, y);
        let (w0, w1) = (own_share(p), own_share(turn(p)));
        let mut sum = 0.0;
        if w0 > 0.0 {
            sum += w0 * f(p.0, p.1);
        }
        if w1 > 0.0 {
            let q = turn(p);
            sum += w1 * f(q.0, q.1);
        }
        sum / (w0 * w0 + w1 * w1).sqrt()
    }

    /// Noise that is [`Terrain::an_even`]'s where the ground is designed to
    /// be the same under the turn ([`Terrain::an_tame`] is 1) and each side's
    /// own elsewhere, so the two coasts do not look like one turned.
    fn an_wild(&self, x: f64, y: f64, tame: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        if tame >= 1.0 {
            return self.an_even(x, y, &f);
        }
        let own = f(x, y);
        if tame <= 0.0 {
            return own;
        }
        let even = self.an_even(x, y, &f);
        (tame * even + (1.0 - tame) * own) / (tame * tame + (1.0 - tame) * (1.0 - tame)).sqrt()
    }

    /// 1 where the ground must be the same under the turn, 0 where each side
    /// may have its own: round the starts and the ore, the ramps, the shore
    /// strips across the valley's foot.
    pub(super) fn an_tame(&self, x: f64, y: f64) -> f64 {
        let near = |q: (f64, f64), at: (f64, f64), reach: f64| {
            smoothstep(reach + 400.0, reach, dist(q, at))
        };
        let island_ore = ISLAND_ORE
            .iter()
            .map(|&(lx, ly, r)| (island_map((lx, ly)), r));
        self.an_max(x, y, |q| {
            let cuts = RAMPS
                .iter()
                .chain(TAME)
                .map(|c| smoothstep(c.half + 300.0, c.half, segment(q, c.a, c.b).0))
                .fold(0.0, f64::max);
            STARTS
                .iter()
                .map(|&s| near(q, s, 1_500.0))
                .chain(ORE.iter().map(|&(ox, oy, r)| near(q, (ox, oy), r + 250.0)))
                .chain(island_ore.clone().map(|(c, r)| near(q, c, r + 250.0)))
                .fold(cuts, f64::max)
        })
    }

    /// The larger of a designed field at a point and at its turned image.
    fn an_max(&self, x: f64, y: f64, f: impl Fn((f64, f64)) -> f64) -> f64 {
        f((x, y)).max(f(turn((x, y))))
    }

    /// Metres in from the coast (negative out to sea), islands aside.
    fn an_land(&self, x: f64, y: f64) -> f64 {
        let o = outlines();
        let tame = self.an_tame(x, y);
        let wobble = self.an_wild(x, y, tame, |x, y| self.an_coast_wobble(x, y));
        // Away from what is played the same, each coast takes its own broad
        // capes and bights.
        let own = (1.0 - tame) * self.coast.fbm(x / 2_600.0 - 40.0, y / 2_600.0, 3, 0.5) * 650.0;
        self.an_max(x, y, |q| inside(q, &o.west)) + wobble + own
    }

    /// The coast's wander in and out, metres: noise about zero.
    fn an_coast_wobble(&self, x: f64, y: f64) -> f64 {
        self.coast.fbm(x / 1_100.0, y / 1_100.0, 4, 0.55) * 380.0
            + self.coast_warp.fbm(x / 300.0, y / 300.0, 3, 0.5) * 160.0
    }

    /// 1 in open country, falling to 0 round the starts, the ore and the
    /// ramps: rough shaping keeps off what must be built on or walked.
    fn an_keep(&self, x: f64, y: f64) -> f64 {
        let base = 1.1 * self.start_outer;
        let near = |q: (f64, f64), at: (f64, f64), reach: f64| {
            smoothstep(reach, reach + 300.0, dist(q, at))
        };
        [(x, y), turn((x, y))]
            .into_iter()
            .map(|q| {
                let starts = STARTS.iter().map(|&s| near(q, s, base)).fold(1.0, f64::min);
                let ramps = RAMPS
                    .iter()
                    .map(|r| smoothstep(r.half, r.half + 250.0, segment(q, r.a, r.b).0))
                    .fold(starts, f64::min);
                ORE.iter()
                    .map(|&(ox, oy, r)| near(q, (ox, oy), r + 40.0))
                    .fold(ramps, f64::min)
            })
            .fold(1.0, f64::min)
    }

    /// The ice sheet over `(x, y)`.
    fn an_ice(&self, x: f64, y: f64) -> IceAt {
        let o = outlines();
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        // Only broad wander: fine noise on a wall this steep stands up as teeth.
        let fray = n(&|x, y| self.mtn_mask.fbm(x / 600.0 + 5.0, y / 600.0, 2, 0.5)) * 130.0
            + (1.0 - tame) * self.mtn_mask.fbm(x / 2_200.0 + 60.0, y / 2_200.0, 3, 0.5) * 450.0;
        let swell = n(&|x, y| self.tilt.fbm(x / 1_300.0 - 4.0, y / 1_300.0 + 9.0, 3, 0.5));
        let ramp = self.an_max(x, y, |q| {
            RAMPS
                .iter()
                .map(|r| smoothstep(r.half + 70.0, r.half, segment(q, r.a, r.b).0))
                .fold(0.0, f64::max)
        });
        let sheet_in = self.an_max(x, y, |q| inside(q, &o.ice)) + fray * (1.0 - ramp);
        let run = WALL + (RAMP_RUN - WALL) * ramp;
        let sheet = smoothstep(-run, run, sheet_in);
        // The top rounds over into the wall instead of ending in a lip, and rolls
        // in long, low swells inland.
        let shoulder = 8.0 * (1.0 - smoothstep(0.0, 90.0, sheet_in));
        let sheet_top =
            SHEET_EDGE_H + SHEET_RISE * sheet_in.clamp(0.0, 4_000.0) + 16.0 * swell - shoulder;

        IceAt {
            sheet_top,
            sheet,
            cover: smoothstep(-(run + 25.0), run, sheet_in),
            within: sheet_in,
        }
    }

    /// The land before erosion.
    fn an_shape(&self, x: f64, y: f64) -> f64 {
        let s = self.an_land(x, y);
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let keep = self.an_keep(x, y);
        let inland = smoothstep(100.0, 1_800.0, s);
        // The lowland: low along the shore, higher toward the sheet, rolling
        // (calmed round the bases, the ore and the ramps), and fine grain.
        let mut land = 14.0 + 16.0 * inland;
        let warp = n(&|x, y| self.warp_x.fbm(x / 1_800.0, y / 1_800.0, 2, 0.5)) * 700.0;
        let hills = n(&|x, y| {
            self.tilt
                .fbm((x + warp) / 800.0, (y - warp) / 800.0, 3, 0.5)
        });
        land += 45.0 * hills * keep.max(0.2) * smoothstep(80.0, 600.0, s);
        land += 10.0 * n(&|x, y| self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5)) * keep;
        // Knolls of ice-smoothed rock, gentle enough to walk over.
        land += 22.0 * self.an_knolls(x, y, tame) * keep.max(0.4) * smoothstep(60.0, 400.0, s);
        // No hollow holds water: the valleys floor out softly above the shore.
        let low = land - 10.0;
        land = 10.0 + 0.5 * (low + (low * low + 16.0).sqrt());
        land += 0.8 * n(&|x, y| self.detail.fbm(x / 60.0 + 9.0, y / 60.0 - 4.0, 2, 0.5));
        let crag = n(&|x, y| self.crag.ridged(x / 650.0, y / 650.0, 4, 0.5) - 0.55) + 0.55;
        let rough = n(&|x, y| self.crag.ridged(x / 140.0 - 3.0, y / 140.0 + 8.0, 3, 0.5) - 0.55);
        land += self.an_max(x, y, |q| crags(TORS, q, crag, rough, 1.0));
        let mut h = self.an_shore(x, y, s, land);

        // The ice sheet over it in its wall, the massifs, the nunataks
        // through the sheet.
        let ice = self.an_ice(x, y);
        h += (ice.sheet_top - h).max(0.0) * ice.sheet;
        h += self.an_ranges(x, y, crag) * smoothstep(-300.0, 60.0, s);
        h += self.an_max(x, y, |q| crags(NUNATAKS, q, crag, rough, 1.6)) * ice.sheet;
        let h = h
            .max(self.an_island(x, y, crag, rough))
            .max(self.an_bergs(x, y));
        if h >= SEA_ICE_H {
            return h;
        }
        // The sea ice: a flat pan a little over the water, a sheer edge.
        let top = SEA_ICE_H + 0.35 * n(&|x, y| self.detail.fbm(x / 90.0 - 5.0, y / 90.0, 2, 0.5));
        // Under its edge the floor stands only a ship's draught and a little
        // deep: the edge shows a couple of metres of ice over the water, not
        // a cliff dropping to the sea bed.
        let si = self.an_sea_ice(x, y, s);
        let floor = h.max(UNDER_EDGE - 1.2 * (-si - 4.0).max(0.0));
        floor + (top - floor) * smoothstep(-3.5, 3.5, si)
    }

    /// Metres inside the sea ice (negative outside): out from each coast to a
    /// reach that wanders along it, held back in front of the starts and away
    /// from the islands' own fringes, never across the lead down the middle;
    /// a fringe round each island; open pools
    /// here and there. `s` is metres in from the coast.
    fn an_sea_ice(&self, x: f64, y: f64, s: f64) -> f64 {
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let broad = n(&|x, y| self.lake.fbm(x / 1_700.0 + 50.0, y / 1_700.0, 3, 0.5));
        let edge = n(&|x, y| self.lake_shore.fbm(x / 260.0 - 30.0, y / 260.0, 3, 0.55)) * 120.0
            + n(&|x, y| self.coast_warp.fbm(x / 70.0 + 2.0, y / 70.0, 2, 0.5)) * 25.0;
        let pools = n(&|x, y| self.lake.fbm(x / 380.0 - 70.0, y / 380.0 + 20.0, 3, 0.5));
        // In the frame of the side it lies on.
        let q = if x < MID { (x, y) } else { turn((x, y)) };
        let harbour = STARTS
            .iter()
            .map(|&a| smoothstep(HARBOUR.1, 0.6 * HARBOUR.1, dist(q, a)))
            .fold(0.0, f64::max);
        let (lo, hi) = SEA_ICE_REACH;
        let reach = lo + (hi - lo) * smoothstep(-0.35, 0.35, broad);
        let reach = reach + (HARBOUR.0 - reach) * harbour;
        // The lead wanders, as does the water kept round the islands.
        let lead = (MID - LEAD) - q.0 + 250.0 * (broad + 0.35).min(0.0) + 0.5 * edge;
        // The ice's reach is laid from the coast as designed, the same on
        // both sides, not from each side's own capes and bights (it fills a
        // bight and stops short beyond a cape): both teams get the same ice.
        let designed = self.an_max(x, y, |q| inside(q, &outlines().west))
            + self.an_even(x, y, |x, y| self.an_coast_wobble(x, y));
        let coast = (reach + designed + edge).min(30.0 - s).min(lead);
        let island = self.an_island_in(x, y);
        let coast = coast.min(-island - (ISLAND_ICE + ISLAND_WATER) + edge);
        let fringe = ISLAND_ICE + 1.5 * edge + island;
        coast.max(fringe).min(380.0 * (0.42 - pools))
    }

    /// The lowland's knolls at `(x, y)`, 0 between them to about 1 on their
    /// crowns.
    fn an_knolls(&self, x: f64, y: f64, tame: f64) -> f64 {
        let k = self.an_wild(x, y, tame, |x, y| {
            self.ramp.fbm(x / 300.0 + 3.0, y / 300.0 - 8.0, 3, 0.5)
        });
        smoothstep(0.05, 0.6, k).powf(1.5)
    }

    /// The land meeting the sea: beaches of dark shingle near the starts,
    /// low rock cliffs elsewhere, varying along the coast.
    fn an_shore(&self, x: f64, y: f64, s: f64, land: f64) -> f64 {
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let home = self.an_max(x, y, |q| {
            STARTS
                .iter()
                .map(|&a| smoothstep(2_600.0, 1_500.0, dist(q, a)))
                .fold(0.0, f64::max)
        });
        let vary = n(&|x, y| self.coast_warp.fbm(x / 1_600.0 + 30.0, y / 1_600.0, 3, 0.5));
        let cliffy = smoothstep(-0.1, 0.25, vary) * (1.0 - home);
        let run = 240.0 - 200.0 * cliffy;
        let reach = 650.0 - 350.0 * cliffy;
        let mut sea = -DEEP * smoothstep(0.0, reach, -s).powf(0.8);
        sea += 5.0
            * n(&|x, y| self.lake.fbm(x / 700.0, y / 700.0, 3, 0.5))
            * smoothstep(-60.0, -500.0, s);
        let w = smoothstep(-25.0, run, s);
        sea.min(0.2) * (1.0 - w) + land * w
    }

    /// The massifs' height over the ground they stand on (as Crosswater's
    /// spine is laid).
    fn an_ranges(&self, x: f64, y: f64, crag: f64) -> f64 {
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let o = outlines();
        let peaks = n(&|x, y| self.ridge.fbm(x / 900.0 + 17.0, y / 900.0, 3, 0.5));
        let ragged = n(&|x, y| self.mtn_gap.fbm(x / 600.0, y / 600.0, 4, 0.55)) * 300.0;
        let arete =
            n(&|x, y| self.mtn.ridged(x / 800.0 + 3.3, y / 800.0 - 1.9, 3, 0.5) - 0.55) + 0.55;
        let spur =
            n(&|x, y| self.mtn.ridged(x / 300.0 - 6.1, y / 300.0 + 4.4, 3, 0.5) - 0.55) + 0.55;
        let rock =
            n(&|x, y| self.crag.ridged(x / 130.0 + 11.0, y / 130.0 - 5.0, 3, 0.5) - 0.55) + 0.55;
        self.an_max(x, y, |q| {
            RIDGES
                .iter()
                .zip(&o.ridges)
                .map(|(r, line)| {
                    let d = polyline(q, line) + ragged.clamp(-0.4 * r.half, 0.4 * r.half);
                    let end = dist(q, r.line[r.line.len() - 1]);
                    let crest = 0.5 + 0.5 * smoothstep(0.0, 600.0, end);
                    let tall =
                        r.tall * crest * (1.0 + 0.35 * peaks).max(0.8) * (1.0 + 0.6 * (crag - 0.6));
                    let m = smoothstep(r.half + 40.0, -0.3 * r.half, d);
                    let sharp = smoothstep(0.2, 0.8, m);
                    tall.max(0.0)
                        * (m * (1.0 + sharp * (0.55 * arete * arete - 0.2))
                            + 0.35 * sharp * m * (spur - 0.6))
                        + 22.0 * sharp * (rock - 0.45).max(0.0)
                })
                .fold(0.0, f64::max)
        })
    }

    /// Metres inside either island's coast as designed (negative outside).
    fn an_island_in(&self, x: f64, y: f64) -> f64 {
        self.an_max(x, y, |q| {
            let l = island_local(q);
            if l.0.hypot(l.1) > ISLAND_REACH + 1_500.0 {
                return -1_500.0;
            }
            inside(l, &outlines().island)
        })
    }

    /// The volcanic islands: beaches of dark sand, flats either side of a
    /// broken ridge, a volcano at one end. `f64::MIN` away from them.
    fn an_island(&self, x: f64, y: f64, crag: f64, rough: f64) -> f64 {
        if self.an_island_in(x, y) < -1_400.0 {
            return f64::MIN;
        }
        let o = outlines();
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let fray = n(&|x, y| self.coast.fbm(x / 420.0 - 9.0, y / 420.0 + 2.0, 3, 0.5)) * 120.0
            + n(&|x, y| self.coast_warp.fbm(x / 110.0, y / 110.0, 2, 0.5)) * 30.0;
        let grain = n(&|x, y| self.detail.fbm(x / 120.0 - 7.0, y / 120.0 + 1.0, 2, 0.5));
        let peaks =
            n(&|x, y| self.mtn.ridged(x / 260.0 + 2.0, y / 260.0 - 6.0, 3, 0.5) - 0.55) + 0.55;
        self.an_max(x, y, |q| {
            let l = island_local(q);
            let si = inside(l, &o.island) + fray;
            // Sea floor up to the shore, beach, flats rising inland.
            let sea = -DEEP * smoothstep(0.0, 900.0, -si).powf(0.7) - 2.0;
            let shore = smoothstep(-20.0, 120.0, si);
            let flats = 8.0 + 12.0 * smoothstep(120.0, 450.0, si);
            let mut h = sea + (flats + 2.0 * grain - sea) * shore;
            // The ridge: steep, broken, too sheer to climb.
            let d = polyline(l, &o.spine) + 40.0 * rough;
            let ridge = smoothstep(SPINE_HALF + 60.0, -0.2 * SPINE_HALF, d);
            h += SPINE_TALL * (0.7 + 0.6 * peaks) * (1.0 + 0.5 * (crag - 0.55)) * ridge.powf(1.3);
            // The volcano: steep flanks, a broken crown, a crater.
            let (cx, cy, cr, ch) = CONE;
            let d = dist(l, (cx, cy)) + 30.0 * rough;
            let body = (1.0 - d / cr).max(0.0).powf(1.25);
            let crater = smoothstep(0.32 * cr, 0.12 * cr, d);
            h + ch * (1.0 + 0.5 * (crag - 0.55)) * body - 0.25 * ch * crater
        })
    }

    /// Tabular icebergs aground: sheer sides, a flat top. `f64::MIN` away from them.
    fn an_bergs(&self, x: f64, y: f64) -> f64 {
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let fray = n(&|x, y| self.coast.fbm(x / 120.0 + 31.0, y / 120.0 - 5.0, 3, 0.5));
        let top = n(&|x, y| self.detail.fbm(x / 70.0 + 13.0, y / 70.0, 2, 0.5));
        self.an_max(x, y, |q| {
            BERGS
                .iter()
                .map(|&(bx, by, r, tall)| {
                    let d = dist(q, (bx, by)) + 0.3 * r * fray + 0.3 * ragged_of(r, q, (bx, by));
                    // The berg's foot spreads under the water, so its sides
                    // stand out of the sea, not up from the bed.
                    let body = smoothstep(r + 8.0, r - 8.0, d);
                    let foot = smoothstep(r + 50.0, r + 8.0, d);
                    if foot > 0.0 {
                        -DEEP + (DEEP + UNDER_EDGE) * foot + (tall + 1.5 * top - UNDER_EDGE) * body
                    } else {
                        f64::MIN
                    }
                })
                .fold(f64::MIN, f64::max)
        })
    }

    /// How much of `(x, y)` is ice, 0..1, and how much of that lies under
    /// snow (the sheet's and the bergs' tops; sea ice in patches).
    fn an_ice_cover(&self, x: f64, y: f64, h: f64) -> (f64, f64) {
        let ice = self.an_ice(x, y);
        // Rock that stands out of the sheet (the nunataks, the ranges) is
        // rock, not ice.
        let sheet = ice.cover * (1.0 - smoothstep(10.0, 35.0, h - ice.sheet_top));
        let berg = smoothstep(-2.0, 2.0, self.an_bergs(x, y));
        // Sea ice lies at the water: none on the islands or the shore above.
        let sea = smoothstep(-2.0, 2.0, self.an_sea_ice(x, y, self.an_land(x, y)))
            * (1.0 - smoothstep(2.5, 4.5, h))
            * (1.0 - sheet.max(berg));
        let cover = sheet.max(berg).max(sea);
        // Sea ice lies under a patchy skin of snow, swept bare in places.
        let swept = 0.35 + 0.5 * smoothstep(-0.2, 0.3, self.tilt.fbm(x / 400.0, y / 400.0, 3, 0.5));
        let snowed = sheet.max(berg) + sea * swept;
        (cover, snowed)
    }

    /// Ice and lying snow at a point of height `h` with slope `(gx, gy)`:
    /// snow over nearly everything, the ice walls and sea ice, rock where it is
    /// steep or the wind has scoured it, dark shingle along the water, bare
    /// ground round the ore, the islands' warm black ground half bare.
    pub(super) fn antarctic_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        if h < 0.4 {
            return (0.0, 0.0);
        }
        let slope = (gx * gx + gy * gy).sqrt();
        let (ice, snowed) = self.an_ice_cover(x, y, h);
        let scour = smoothstep(
            0.12,
            0.45,
            self.tilt.fbm(x / 520.0 + 77.0, y / 520.0 - 31.0, 3, 0.5)
                + 0.35 * self.detail.fbm(x / 140.0 - 12.0, y / 140.0 + 40.0, 2, 0.5),
        );
        let steep = 1.0 - smoothstep(0.55, 1.05, slope);
        let volcanic = smoothstep(-300.0, 200.0, self.an_island_in(x, y));
        // The knolls' crowns are blown bare.
        let crowns = smoothstep(0.35, 0.8, self.an_knolls(x, y, self.an_tame(x, y)));
        let mut ground =
            (0.95 - 0.6 * scour.max(crowns) - 0.35 * volcanic) * steep * smoothstep(1.0, 5.0, h);
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 120.0 {
                ground *= 0.15 + 0.85 * smoothstep(-10.0, 120.0, d);
            }
        }
        // The sheet and the bergs are snow on top, their ice bare only in
        // their walls; sea ice is bare in places.
        let bare = (ice - snowed).max(0.0);
        let walls = smoothstep(0.35, 0.8, slope);
        let on_sheet = snowed * 0.97 * (1.0 - walls);
        let on_bare = bare * 0.25 * (1.0 - smoothstep(0.7, 1.3, slope));
        let ice_shows = (bare + snowed * walls).min(1.0);
        (ice_shows, ground * (1.0 - ice) + on_sheet + on_bare)
    }

    pub(super) fn natural_antarctic(&self, x: f64, y: f64) -> f64 {
        let h = self.an_shape(x, y);
        if self.erosion.delta.is_empty() || h < 0.5 {
            return h;
        }
        let within = self.an_ice(x, y).within;
        let bare = smoothstep(-40.0, -160.0, within);
        let e = self.erosion.at(x, y) * self.an_keep(x, y) * bare * smoothstep(2.0, 6.0, h);
        (h + e).max(h.min(2.5))
    }

    /// Water erosion over the whole country, from the shaped land, then made
    /// exact under the half turn the way the noise is.
    fn erode_antarctic(&mut self) {
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
                            *v = t.an_shape(i as f64 * STEP, y).max(-2.0) as f32 / VERTICAL;
                        }
                    }
                });
            }
        });
        let before = h.clone();
        erode(&mut h, n, self.seed);
        erode(&mut h, n, self.seed ^ 0x6572_6562);
        let soft = settle_gullies(&h, &before, n, STEP, VERTICAL);
        // The grid's middle sample is the map's middle, so a half turn takes
        // samples to samples: where the ground is tame, blend each with its
        // image.
        let mut delta = vec![0f32; n * n];
        for j in 0..n {
            for i in 0..n {
                let (ti, tj) = (n - 1 - i, n - 1 - j);
                let p = (i as f64 * STEP, j as f64 * STEP);
                let (w0, w1) = (own_share(p), own_share(turn(p)));
                let own = soft[j * n + i] as f64;
                let even = (w0 * own + w1 * soft[tj * n + ti] as f64) / (w0 + w1);
                let tame = self.an_tame(p.0, p.1);
                delta[j * n + i] = (tame * even + (1.0 - tame) * own) as f32;
            }
        }
        self.erosion = Erosion {
            n,
            step: STEP,
            delta,
        };
    }

    pub(super) fn setup_antarctic(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.erode_antarctic();

        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let west: Vec<(f64, f64)> = STARTS.iter().map(|&s| snap(s)).collect();
        self.starts = west.iter().flat_map(|&s| [s, turn(s)]).collect();
        self.pads = self
            .starts
            .iter()
            .map(|&(x, y)| {
                let w = if x < MID { (x, y) } else { turn((x, y)) };
                Pad {
                    x,
                    y,
                    core: start_core,
                    outer: 2.0 * start_core,
                    height: self.natural(w.0, w.1).max(14.0),
                }
            })
            .collect();

        // Three small fields round each base, inside its pad: one behind
        // (toward the sheet), two on the forward flanks.
        let mut sites = Vec::new();
        for &s in &west {
            for bearing in [PI, 1.15, -1.15] {
                sites.push((
                    polar(s, bearing, 0.8 * start_core),
                    (0.2 * start_core).max(55.0),
                ));
            }
        }
        sites.extend(ORE.iter().map(|&(x, y, r)| ((x, y), r)));
        sites.extend(
            ISLAND_ORE
                .iter()
                .map(|&(lx, ly, r)| (island_map((lx, ly)), r)),
        );
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = snap(p);
            let field = self.ore_field(p.0, p.1, r);
            fields.push(turned_field(&field));
            fields.push(field);
        }
        self.ore = fields;
    }
}

/// Crags standing out of the ground round them (tors on the lowland,
/// nunataks through the sheet): sheer flanks, a broken crown, a skirt of
/// scree. `steep` sharpens the flanks.
fn crags(list: &[(f64, f64, f64, f64)], q: (f64, f64), crag: f64, rough: f64, steep: f64) -> f64 {
    list.iter()
        .map(|&(tx, ty, r, tall)| {
            let d = dist(q, (tx, ty)) + 60.0 * rough + 0.4 * ragged_of(r, q, (tx, ty));
            let body = smoothstep(r + 30.0, 0.45 * r, d);
            let skirt = smoothstep(r + 160.0, r, d);
            tall * (1.0 + 0.8 * (crag - 0.55)) * body.powf(0.7 / steep) + 6.0 * skirt
        })
        .fold(0.0, f64::max)
}

/// An outline's wobble at `q`, metres: lobes round its centre.
fn ragged_of(r: f64, q: (f64, f64), c: (f64, f64)) -> f64 {
    let a = (q.1 - c.1).atan2(q.0 - c.0);
    r * (0.25 * (3.0 * a + c.0).sin() + 0.15 * (5.0 * a + c.1).sin())
}

/// An ore field turned half round, corner for corner.
fn turned_field(f: &OreField) -> OreField {
    let (x, y) = turn((f.x, f.y));
    OreField {
        x,
        y,
        radius: f.radius,
        corners: f.corners.iter().map(|&c| turn(c)).collect(),
    }
}

#[cfg(test)]
mod tests;
