//! "Erebus Strait": two against two across a polar sea.
//!
//! [`Layout::Antarctic`](super::Layout::Antarctic), exactly 12 km. Two
//! coasts of Antarctica face each other across an open strait that runs off
//! the map to the north and the south; nothing grows anywhere. Each team holds
//! one coast, its two starts on ice-free coastal lowland, one toward each end.
//!
//! Behind each coast the ice sheet stands over the lowland in an ice wall
//! forty or fifty metres high, broken by one ramp behind each start; on top it
//! rises gently inland, nunataks breaking through it. In the middle of each
//! coast it bulges forward into two massifs of mountains, and between them
//! an outlet glacier flows down from the sheet to the sea. The glacier is
//! the land way between the two allies along the coast (a strip of shore runs
//! under the massifs' seaward ends to its mouth), and the sheet is the other,
//! behind the mountains. Where the glacier meets the sea it floats out as an
//! ice shelf, flat and buildable, ending in sheer ice cliffs over the water.
//!
//! In the strait lie two volcanic islands, each nearer one coast: a ring of
//! black ground round a flooded caldera whose one gap lets ships in, a cone
//! standing on the ring. Tabular icebergs lie aground here and there.
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
const STARTS: [(f64, f64); 2] = [(2_100.0, 2_700.0), (2_200.0, 9_500.0)];

/// The west coast's land, closed off the map: from beyond the south edge up
/// the coast to beyond the north edge.
const WEST: &[(f64, f64)] = &[
    (-900.0, -900.0),
    (3_900.0, -900.0),
    (3_850.0, 500.0),
    (3_600.0, 1_300.0),
    (3_250.0, 2_000.0),
    (3_300.0, 2_700.0),
    (3_600.0, 3_300.0),
    (4_000.0, 3_900.0),
    (4_250.0, 4_600.0),
    (4_150.0, 5_200.0),
    (4_300.0, 5_700.0),
    (4_380.0, 6_150.0),
    (4_300.0, 6_600.0),
    (4_200.0, 7_100.0),
    (4_350.0, 7_700.0),
    (4_050.0, 8_300.0),
    (3_500.0, 8_800.0),
    (3_350.0, 9_500.0),
    (3_600.0, 10_200.0),
    (3_950.0, 10_800.0),
    (3_750.0, 11_500.0),
    (3_950.0, 12_300.0),
    (4_100.0, 13_200.0),
    (-900.0, 13_200.0),
];

/// The ice sheet's edge over the lowland, closed off the map to the west. It
/// bulges forward between the starts, where the glacier leaves it, and at
/// the north and south ends of the coast it runs out over the sea and ends
/// in ice cliffs.
const ICE: &[(f64, f64)] = &[
    (-900.0, -900.0),
    (4_500.0, -900.0),
    (4_350.0, 150.0),
    (3_500.0, 550.0),
    (2_500.0, 700.0),
    (1_500.0, 950.0),
    (1_000.0, 1_400.0),
    (850.0, 1_800.0),
    (750.0, 2_900.0),
    (950.0, 3_800.0),
    (1_600.0, 4_500.0),
    (2_400.0, 5_000.0),
    (2_800.0, 5_600.0),
    (2_900.0, 6_150.0),
    (2_800.0, 6_700.0),
    (2_400.0, 7_300.0),
    (1_600.0, 7_800.0),
    (950.0, 8_500.0),
    (750.0, 9_500.0),
    (850.0, 10_500.0),
    (1_100.0, 11_150.0),
    (1_800.0, 11_500.0),
    (2_700.0, 11_550.0),
    (3_500.0, 11_750.0),
    (4_250.0, 12_150.0),
    (4_450.0, 13_200.0),
    (-900.0, 13_200.0),
];
/// The sheet's top at its edge over the lowland, and how much it rises per
/// metre inland, metres.
const SHEET_EDGE_H: f64 = 72.0;
const SHEET_RISE: f64 = 0.028;
/// Half the run of the ice wall (m), and of a ramp up it.
const WALL: f64 = 9.0;
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
        a: (1_250.0, 3_350.0),
        b: (450.0, 3_250.0),
        half: 110.0,
    },
    Cut {
        a: (1_300.0, 10_050.0),
        b: (500.0, 10_150.0),
        half: 110.0,
    },
];

/// The outlet glacier's middle line, from the sheet to the coast.
const GLACIER: &[(f64, f64)] = &[
    (2_500.0, 6_150.0),
    (3_200.0, 6_100.0),
    (3_850.0, 6_190.0),
    (4_450.0, 6_150.0),
];
/// Its half width at the head and at the mouth, and its surface there, metres.
const GLACIER_HALF: (f64, f64) = (470.0, 400.0);
const GLACIER_H: (f64, f64) = (82.0, 26.0);

/// The ice shelf the glacier floats out into, and its top over the water.
const SHELF: &[(f64, f64)] = &[
    (4_250.0, 5_650.0),
    (4_700.0, 5_600.0),
    (5_150.0, 5_700.0),
    (5_350.0, 5_950.0),
    (5_300.0, 6_350.0),
    (5_100.0, 6_650.0),
    (4_650.0, 6_750.0),
    (4_250.0, 6_700.0),
];
const SHELF_H: f64 = 24.0;

struct Ridge {
    line: &'static [(f64, f64)],
    /// Half width of the crest's foot, metres.
    half: f64,
    /// Height of the crest over the ground round it, metres.
    tall: f64,
}

/// The massifs either side of the glacier, from the sheet toward the coast;
/// a strip of shore runs under their seaward ends.
const RIDGES: &[Ridge] = &[
    Ridge {
        line: &[
            (2_250.0, 4_700.0),
            (2_850.0, 5_000.0),
            (3_250.0, 5_250.0),
            (3_450.0, 5_380.0),
        ],
        half: 330.0,
        tall: 360.0,
    },
    Ridge {
        line: &[
            (2_250.0, 7_600.0),
            (2_850.0, 7_300.0),
            (3_250.0, 7_050.0),
            (3_450.0, 6_920.0),
        ],
        half: 320.0,
        tall: 420.0,
    },
];

/// Nunataks through the sheet and tors on the lowland: centre, radius and
/// height over the ground round them, metres.
const NUNATAKS: &[(f64, f64, f64, f64)] = &[
    (450.0, 5_300.0, 230.0, 170.0),
    (850.0, 7_250.0, 190.0, 130.0),
    (350.0, 1_500.0, 170.0, 110.0),
    (400.0, 11_000.0, 210.0, 150.0),
    (2_900.0, 250.0, 220.0, 140.0),
    (3_100.0, 12_050.0, 200.0, 130.0),
];
const TORS: &[(f64, f64, f64, f64)] = &[
    (1_350.0, 3_500.0, 150.0, 60.0),
    (3_550.0, 1_250.0, 130.0, 50.0),
    (1_500.0, 8_700.0, 150.0, 60.0),
    (1_450.0, 10_800.0, 170.0, 70.0),
];

/// Frozen lakes in the lowland: centre, radius and the ice's level, metres.
/// Flat, walkable, buildable on.
const LAKES: &[(f64, f64, f64, f64)] = &[
    (1_250.0, 2_350.0, 160.0, 22.0),
    (2_650.0, 4_050.0, 140.0, 24.0),
    (2_700.0, 10_250.0, 150.0, 22.0),
    (2_750.0, 7_950.0, 130.0, 26.0),
];

/// Where the ground is designed to be the same under the turn however the
/// coast and the mountains wander: the strips of shore under the massifs'
/// seaward ends, from each lowland to the glacier's mouth. A line and its
/// half width, metres.
const TAME: &[Cut] = &[
    Cut {
        a: (3_700.0, 4_200.0),
        b: (4_300.0, 6_150.0),
        half: 550.0,
    },
    Cut {
        a: (4_300.0, 6_150.0),
        b: (3_700.0, 8_000.0),
        half: 550.0,
    },
];

/// The first island: its middle, the radius of its coast, the caldera's
/// shore inside and the ring's crest between them, metres.
const ISLAND: (f64, f64) = (5_750.0, 8_800.0);
const ISLAND_R: f64 = 880.0;
const CALDERA_R: f64 = 360.0;
const CREST_R: f64 = 470.0;
/// The caldera's gap to the sea: its bearing from the middle (radians), its
/// half width and its depth, metres.
const GAP_BEARING: f64 = -50.0 * PI / 180.0;
const GAP_HALF: f64 = 110.0;
const GAP_DEEP: f64 = 14.0;
/// The cone on the ring: bearing from the island's middle, distance out,
/// radius and height, metres.
const CONE: (f64, f64, f64, f64) = (115.0 * PI / 180.0, 620.0, 300.0, 260.0);

/// Tabular icebergs aground: centre, radius and top over the water, metres.
const BERGS: &[(f64, f64, f64, f64)] = &[
    (4_900.0, 1_900.0, 170.0, 22.0),
    (5_250.0, 4_300.0, 140.0, 20.0),
    (5_350.0, 7_400.0, 120.0, 18.0),
    (5_600.0, 10_900.0, 200.0, 24.0),
    (4_700.0, 11_800.0, 130.0, 20.0),
    (5_900.0, 5_000.0, 110.0, 18.0),
];

/// Ore away from the starts: centre and radius, metres.
const ORE: &[(f64, f64, f64)] = &[
    // The south start's lowland, and the shore under the south massif.
    (3_050.0, 1_800.0, 65.0),
    (1_500.0, 1_900.0, 65.0),
    (2_900.0, 3_700.0, 70.0),
    (2_200.0, 4_150.0, 70.0),
    (3_800.0, 4_800.0, 60.0),
    // The north start's.
    (3_050.0, 10_600.0, 65.0),
    (1_500.0, 10_300.0, 65.0),
    (3_150.0, 8_800.0, 70.0),
    (2_150.0, 8_250.0, 70.0),
    (3_700.0, 7_450.0, 60.0),
    // Moraine by the nunataks on the sheet.
    (850.0, 5_650.0, 60.0),
    (1_250.0, 6_900.0, 60.0),
    // Out on the shelf.
    (4_800.0, 6_150.0, 70.0),
];
/// The island's fields: bearing from its middle (radians) and radius; all
/// at [`ISLAND_ORE_OUT`] metres out, on the flats outside the ring's crest.
const ISLAND_ORE: &[(f64, f64)] = &[
    (20.0 * PI / 180.0, 70.0),
    (180.0 * PI / 180.0, 70.0),
    (240.0 * PI / 180.0, 65.0),
];
const ISLAND_ORE_OUT: f64 = 640.0;

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
    shelf: Vec<(f64, f64)>,
    glacier: Vec<(f64, f64)>,
    ridges: Vec<Vec<(f64, f64)>>,
}

fn outlines() -> &'static Outlines {
    static OUTLINES: OnceLock<Outlines> = OnceLock::new();
    OUTLINES.get_or_init(|| Outlines {
        west: smooth_closed(WEST, 4),
        ice: smooth_closed(ICE, 4),
        shelf: smooth_closed(SHELF, 4),
        glacier: smooth_open(GLACIER, 4),
        ridges: RIDGES.iter().map(|r| smooth_open(r.line, 4)).collect(),
    })
}

/// A point at `bearing` and `out` metres from `c`.
fn polar(c: (f64, f64), bearing: f64, out: f64) -> (f64, f64) {
    (c.0 + out * bearing.cos(), c.1 + out * bearing.sin())
}

/// The ice at a point: the sheet's, the glacier's and the shelf's tops and
/// how far each covers it (0..1), and how far the point is inside any of
/// them (m, negative outside).
struct IceAt {
    sheet_top: f64,
    sheet: f64,
    glacier_top: f64,
    glacier: f64,
    /// Metres inside the glacier's edge (negative outside), for the
    /// mountains it carves away.
    glacier_in: f64,
    shelf: f64,
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
    /// may have its own: round the starts and the ore, the ramps, the lakes,
    /// the shore strips to the glacier's mouth.
    pub(super) fn an_tame(&self, x: f64, y: f64) -> f64 {
        let near = |q: (f64, f64), at: (f64, f64), reach: f64| {
            smoothstep(reach + 400.0, reach, dist(q, at))
        };
        let island_ore = ISLAND_ORE
            .iter()
            .map(|&(bearing, r)| (polar(ISLAND, bearing, ISLAND_ORE_OUT), r));
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
                .chain(
                    LAKES
                        .iter()
                        .map(|&(lx, ly, r, _)| near(q, (lx, ly), r + 200.0)),
                )
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
        let wobble = self.an_wild(x, y, tame, |x, y| {
            self.coast.fbm(x / 1_100.0, y / 1_100.0, 4, 0.55) * 380.0
                + self.coast_warp.fbm(x / 300.0, y / 300.0, 3, 0.5) * 160.0
        });
        // Away from what is played the same, each coast takes its own broad
        // capes and bights.
        let own = (1.0 - tame) * self.coast.fbm(x / 2_600.0 - 40.0, y / 2_600.0, 3, 0.5) * 650.0;
        self.an_max(x, y, |q| inside(q, &o.west)) + wobble + own
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
                    .chain(
                        LAKES
                            .iter()
                            .map(|&(lx, ly, r, _)| near(q, (lx, ly), r + 60.0)),
                    )
                    .fold(ramps, f64::min)
            })
            .fold(1.0, f64::min)
    }

    /// The ice sheet, the glacier and the shelf over `(x, y)`.
    fn an_ice(&self, x: f64, y: f64) -> IceAt {
        let o = outlines();
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let fray = n(&|x, y| self.mtn_mask.fbm(x / 600.0 + 5.0, y / 600.0, 3, 0.5)) * 130.0
            + n(&|x, y| self.detail.fbm(x / 90.0 - 2.0, y / 90.0 + 6.0, 2, 0.5)) * 18.0
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
        let sheet_top = SHEET_EDGE_H + SHEET_RISE * sheet_in.clamp(0.0, 4_000.0) + 7.0 * swell;

        // The glacier, in its own frame: whichever image lies nearer its line.
        let wander = n(&|x, y| self.coast_warp.fbm(x / 500.0 + 3.0, y / 500.0, 2, 0.5)) * 45.0;
        let (glacier_in, along) = [(x, y), turn((x, y))]
            .into_iter()
            .map(|q| {
                let d = polyline(q, &o.glacier);
                let t = ((q.0 - GLACIER[0].0) / (GLACIER[GLACIER.len() - 1].0 - GLACIER[0].0))
                    .clamp(0.0, 1.0);
                let half = GLACIER_HALF.0 + (GLACIER_HALF.1 - GLACIER_HALF.0) * t;
                (half + wander - d, t)
            })
            .fold((f64::MIN, 0.0), |a, b| if b.0 > a.0 { b } else { a });
        let glacier = smoothstep(-140.0, 30.0, glacier_in);
        let glacier_top = GLACIER_H.0
            + (GLACIER_H.1 - GLACIER_H.0) * along.powf(0.8)
            + 3.0 * swell
            + 1.5 * n(&|x, y| self.detail.fbm(x / 160.0 + 8.0, y / 160.0, 2, 0.5));

        let shelf_in = self.an_max(x, y, |q| inside(q, &o.shelf))
            + fray * 0.5
            + n(&|x, y| self.coast.fbm(x / 260.0 + 21.0, y / 260.0, 3, 0.5)) * 70.0;
        let shelf = smoothstep(-5.0, 5.0, shelf_in);
        IceAt {
            sheet_top,
            sheet,
            glacier_top,
            glacier,
            glacier_in,
            shelf,
            within: sheet_in.max(glacier_in).max(shelf_in),
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
        // The frozen lakes: a shallow bowl round each, its ice dead level.
        let shore = n(&|x, y| self.lake_shore.fbm(x / 120.0, y / 120.0, 2, 0.5)) * 55.0;
        for q in [(x, y), turn((x, y))] {
            for &(lx, ly, r, level) in LAKES {
                let d = dist(q, (lx, ly)) + shore + 0.6 * ragged_of(r, q, (lx, ly));
                let k = smoothstep(r + 170.0, r, d);
                if k > 0.0 {
                    land += (level + 0.25 * (d - r).max(0.0).min(40.0) - land) * k;
                }
            }
        }
        let crag = n(&|x, y| self.crag.ridged(x / 650.0, y / 650.0, 4, 0.5) - 0.55) + 0.55;
        let rough = n(&|x, y| self.crag.ridged(x / 140.0 - 3.0, y / 140.0 + 8.0, 3, 0.5) - 0.55);
        land += self.an_max(x, y, |q| crags(TORS, q, crag, rough, 1.0));
        let mut h = self.an_shore(x, y, s, land);

        // The ice over it: the sheet in its wall, the glacier filling its
        // valley (and carving the massifs), the shelf out over the sea.
        let ice = self.an_ice(x, y);
        h += (ice.sheet_top - h).max(0.0) * ice.sheet;
        h += (ice.glacier_top - h) * ice.glacier * smoothstep(-400.0, -60.0, s);
        let carve = 1.0 - smoothstep(-260.0, 0.0, ice.glacier_in);
        h += self.an_ranges(x, y, crag) * carve * smoothstep(-300.0, 60.0, s);
        h += self.an_max(x, y, |q| crags(NUNATAKS, q, crag, rough, 1.6)) * ice.sheet;
        let shelf_top = SHELF_H + 1.2 * n(&|x, y| self.detail.fbm(x / 220.0, y / 220.0, 2, 0.5));
        h += (shelf_top - h) * ice.shelf;
        let h = h.max(self.an_island(x, y, crag, rough));
        h.max(self.an_bergs(x, y))
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

    /// The volcanic islands: a ring round a flooded caldera, its gap to the
    /// sea, the cone on the ring. `f64::MIN` away from them.
    fn an_island(&self, x: f64, y: f64, crag: f64, rough: f64) -> f64 {
        if dist((x, y), ISLAND).min(dist(turn((x, y)), ISLAND)) > ISLAND_R + 1_400.0 {
            return f64::MIN;
        }
        let tame = self.an_tame(x, y);
        let n = |f: &dyn Fn(f64, f64) -> f64| self.an_wild(x, y, tame, f);
        let fray = n(&|x, y| self.coast.fbm(x / 420.0 - 9.0, y / 420.0 + 2.0, 3, 0.5)) * 150.0
            + n(&|x, y| self.coast_warp.fbm(x / 110.0, y / 110.0, 2, 0.5)) * 30.0;
        let inner = n(&|x, y| self.lake_shore.fbm(x / 200.0 + 4.0, y / 200.0, 2, 0.5)) * 50.0;
        let grain = n(&|x, y| self.detail.fbm(x / 120.0 - 7.0, y / 120.0 + 1.0, 2, 0.5));
        let gap_a = polar(ISLAND, GAP_BEARING, 0.6 * CALDERA_R);
        let gap_b = polar(ISLAND, GAP_BEARING, ISLAND_R + 500.0);
        let cone = polar(ISLAND, CONE.0, CONE.1);
        self.an_max(x, y, |q| {
            let r = dist(q, ISLAND);
            let out = r - fray;
            // Sea floor up to the shore, beach, flats, crest, the caldera's
            // inner wall down into its water.
            let sea = -DEEP * smoothstep(ISLAND_R, ISLAND_R + 900.0, out).powf(0.7) - 2.0;
            let flats = 8.0 + 12.0 * smoothstep(ISLAND_R - 120.0, CREST_R + 90.0, out);
            let shore = smoothstep(ISLAND_R + 20.0, ISLAND_R - 120.0, out);
            let mut h = sea + (flats + 2.0 * grain - sea) * shore;
            let rin = r - inner;
            h += 30.0 * smoothstep(CREST_R + 90.0, CREST_R, rin) * (1.0 + 0.4 * rough);
            let wall = smoothstep(CREST_R - 10.0, CALDERA_R, rin);
            h += (-28.0 + 3.0 * grain - h) * wall;
            // The gap: a deep channel through the ring.
            let g = segment(q, gap_a, gap_b).0;
            let cut = smoothstep(GAP_HALF + 60.0, GAP_HALF, g);
            h += ((-GAP_DEEP - 6.0 * smoothstep(ISLAND_R, ISLAND_R + 500.0, r)).min(h) - h) * cut;
            // The cone: steep flanks, a broken crown, a crater.
            let (_, cr, ch) = (CONE.1, CONE.2, CONE.3);
            let d = dist(q, cone) + 30.0 * rough;
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
                    let body = smoothstep(r + 8.0, r - 8.0, d);
                    if body > 0.0 {
                        -DEEP + (tall + 1.5 * top + DEEP) * body
                    } else {
                        f64::MIN
                    }
                })
                .fold(f64::MIN, f64::max)
        })
    }

    /// How much of `(x, y)` is ice, 0..1; how much of that is snow-covered
    /// sheet, shelf or berg (1) rather than the glacier's broken blue ice or
    /// a lake's clear ice (0); and the glacier's edge, where its moraine lies
    /// bare (0..1).
    pub(super) fn an_ice_cover(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let ice = self.an_ice(x, y);
        let berg = smoothstep(-2.0, 2.0, self.an_bergs(x, y));
        let glacier = ice.glacier * (1.0 - ice.shelf) * (1.0 - ice.sheet);
        let lake = self.an_max(x, y, |q| {
            LAKES
                .iter()
                .map(|&(lx, ly, r, _)| smoothstep(r + 6.0, r - 6.0, dist(q, (lx, ly))))
                .fold(0.0, f64::max)
        });
        let cover = ice.sheet.max(glacier).max(ice.shelf).max(berg).max(lake);
        let snowed = ice.sheet.max(ice.shelf).max(berg) * (1.0 - glacier) * (1.0 - lake);
        let moraine = smoothstep(-160.0, -90.0, ice.glacier_in)
            * (1.0 - smoothstep(-60.0, 0.0, ice.glacier_in))
            * (1.0 - ice.sheet);
        (cover, snowed, moraine)
    }

    /// Glacier ice and lying snow at a point of height `h` with slope `(gx, gy)`:
    /// snow over nearly everything, the glacier's blue ice, rock where it is
    /// steep or the wind has scoured it, dark shingle along the water, bare
    /// ground round the ore, the islands' warm black ground half bare.
    pub(super) fn antarctic_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        if h < 0.4 {
            return (0.0, 0.0);
        }
        let slope = (gx * gx + gy * gy).sqrt();
        let (ice, snowed, moraine) = self.an_ice_cover(x, y);
        let scour = smoothstep(
            0.12,
            0.45,
            self.tilt.fbm(x / 520.0 + 77.0, y / 520.0 - 31.0, 3, 0.5)
                + 0.35 * self.detail.fbm(x / 140.0 - 12.0, y / 140.0 + 40.0, 2, 0.5),
        );
        let steep = 1.0 - smoothstep(0.55, 1.05, slope);
        let volcanic = smoothstep(
            ISLAND_R + 300.0,
            ISLAND_R - 200.0,
            dist((x, y), ISLAND).min(dist(turn((x, y)), ISLAND)),
        );
        // The knolls' crowns are blown bare.
        let crowns = smoothstep(0.35, 0.8, self.an_knolls(x, y, self.an_tame(x, y)));
        let mut ground = (0.95 - 0.6 * scour.max(crowns) - 0.35 * volcanic - 0.8 * moraine)
            * steep
            * smoothstep(1.0, 5.0, h);
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 120.0 {
                ground *= 0.15 + 0.85 * smoothstep(-10.0, 120.0, d);
            }
        }
        // The sheet, the shelf and the bergs are snow on top, their ice bare
        // only in their walls; the glacier and the lakes are bare ice with a
        // little drift.
        let bare = (ice - snowed).max(0.0);
        let walls = smoothstep(0.35, 0.8, slope);
        let on_sheet = snowed * 0.97 * (1.0 - walls);
        let on_bare = bare * 0.25 * (1.0 - smoothstep(0.7, 1.3, slope));
        let ice_shows = (bare + snowed * walls).min(1.0);
        // Cirque and hanging ice on the massifs' gentler high ground.
        let high = smoothstep(150.0, 210.0, h)
            * (1.0 - smoothstep(0.45, 0.8, slope))
            * (1.0 - volcanic)
            * (1.0 - ice);
        let snow = ground * (1.0 - ice) + on_sheet + on_bare;
        (ice_shows.max(high), snow * (1.0 - 0.4 * high))
    }

    pub(super) fn natural_antarctic(&self, x: f64, y: f64) -> f64 {
        let h = self.an_shape(x, y);
        if self.erosion.delta.is_empty() || h < 0.5 {
            return h;
        }
        let within = self.an_ice(x, y).within;
        let bare = smoothstep(-40.0, -160.0, within);
        let e = self.erosion.at(x, y) * self.an_keep(x, y) * bare * smoothstep(0.5, 6.0, h);
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
                .map(|&(bearing, r)| (polar(ISLAND, bearing, ISLAND_ORE_OUT), r)),
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
