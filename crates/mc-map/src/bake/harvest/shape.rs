//! The Last Harvest's ground: the heartland's lowland and the seas round it,
//! the four regions raised over it each in its own way (the desert's rim and
//! mesas, the alpine mountains and their basins, the fjords' plateau, the
//! isles), the hills and outcrops, and then what the plan cuts into all that: the
//! fjords, every site's way and room, and the base's floor.
//!
//! The regions' outlines are read through a slow warp and frayed by noise,
//! so no edge runs true; the ways and rooms are cut exactly, after the
//! erosion, so they stay level and drivable whatever the noise did.

use super::*;
use crate::bake::bays::segment;
use crate::bake::sites::{
    off_ways, Way, FILL, FILL_MOST, LANE_DEEP, LOW_CUT, PRINTER_ROOM, SHOULDER,
};
use crate::landmark::{harvest_depth, harvest_look, HARVEST_HEARTLAND};
use crate::noise::{bump, hash2};

/// The south sea's coast: y at x. Under the isles the sea runs on north (the
/// isles' own coast is [`WEST_COAST`]).
const SOUTH_COAST: [(f64, f64); 12] = [
    (-600.0, 2_600.0),
    (3_000.0, 2_500.0),
    (6_200.0, 2_800.0),
    (7_600.0, 2_450.0),
    (9_000.0, 2_200.0),
    (10_400.0, 2_550.0),
    (12_000.0, 2_150.0),
    (13_700.0, 2_450.0),
    (15_400.0, 2_200.0),
    (17_000.0, 2_650.0),
    (20_500.0, 2_350.0),
    (25_200.0, 2_600.0),
];
/// The heartland's west coast on the isles' sea: x at y.
const WEST_COAST: [(f64, f64); 11] = [
    (-600.0, 6_400.0),
    (2_500.0, 6_900.0),
    (3_600.0, 6_050.0),
    (4_600.0, 5_650.0),
    (5_800.0, 6_500.0),
    (7_000.0, 6_150.0),
    (8_100.0, 5_950.0),
    (9_500.0, 6_850.0),
    (10_500.0, 6_350.0),
    (11_400.0, 6_650.0),
    (11_800.0, 6_700.0),
];
/// The isles' sea's north shore, under the desert's sea cliffs: y at x.
const ISLES_NORTH: [(f64, f64); 5] = [
    (-600.0, 11_300.0),
    (1_800.0, 10_800.0),
    (3_900.0, 11_200.0),
    (6_000.0, 11_050.0),
    (7_500.0, 11_450.0),
];

/// How far round the base's floor the ground takes to come back.
const BASE_BLEND: f64 = 520.0;

/// A fjord: its line from the sea in to its head, half its width and its bed.
type Fjord = (&'static [(f64, f64)], f64, f64);

/// The fjords.
const FJORDS: [Fjord; 5] = [
    // The sound, between the heartland and the fjords' plateau.
    (
        &[
            (18_400.0, -600.0),
            (18_900.0, 1_400.0),
            (18_050.0, 3_000.0),
            (18_800.0, 4_600.0),
            (19_100.0, 6_000.0),
            (18_350.0, 7_300.0),
            (18_350.0, 8_150.0),
        ],
        470.0,
        -38.0,
    ),
    // The long fjord, Fjordhead at its head.
    (
        &[
            (21_000.0, -600.0),
            (20_900.0, 1_600.0),
            (21_250.0, 2_800.0),
            (21_450.0, 4_000.0),
            (21_250.0, 5_600.0),
            (21_300.0, 7_050.0),
        ],
        640.0,
        -48.0,
    ),
    // An arm off it to the east.
    (
        &[
            (21_400.0, 4_300.0),
            (22_050.0, 4_750.0),
            (22_700.0, 4_550.0),
            (23_050.0, 4_900.0),
        ],
        250.0,
        -30.0,
    ),
    // A narrower one further east, bending.
    (
        &[
            (23_700.0, -600.0),
            (23_450.0, 1_500.0),
            (23_900.0, 2_900.0),
            (24_200.0, 3_900.0),
            (23_800.0, 5_200.0),
        ],
        330.0,
        -40.0,
    ),
    // A short one into the plateau west of the long fjord.
    (
        &[
            (19_900.0, 1_500.0),
            (19_700.0, 3_000.0),
            (20_000.0, 4_100.0),
        ],
        280.0,
        -34.0,
    ),
];

/// The river: from the alpine mountains' foot west across the heartland,
/// north of the starts, to the isles' sea. Too shallow for ships, too deep to
/// wade: the ways ford it.
const RIVER: [(f64, f64); 11] = [
    (17_300.0, 12_100.0),
    (16_600.0, 11_250.0),
    (15_800.0, 10_500.0),
    (14_700.0, 10_100.0),
    (13_600.0, 9_300.0),
    (12_500.0, 9_450.0),
    (11_300.0, 8_900.0),
    (10_100.0, 9_300.0),
    (8_900.0, 8_800.0),
    (7_800.0, 9_300.0),
    (6_500.0, 9_450.0),
];

/// The dales: each from its head on a rise down to its mouth on the river.
const DALES: [&[(f64, f64)]; 8] = [
    // From the north, off the valley's flanks.
    &[(8_300.0, 12_300.0), (8_700.0, 10_700.0), (8_850.0, 9_150.0)],
    &[
        (10_000.0, 13_900.0),
        (10_350.0, 11_800.0),
        (10_150.0, 9_500.0),
    ],
    &[
        (14_700.0, 13_400.0),
        (14_250.0, 11_600.0),
        (13_750.0, 9_700.0),
    ],
    &[
        (16_600.0, 9_800.0),
        (16_000.0, 10_300.0),
        (15_250.0, 10_400.0),
    ],
    // From the south, off the rises behind the starts.
    &[(7_500.0, 7_100.0), (7_950.0, 8_200.0), (7_800.0, 9_200.0)],
    &[(9_500.0, 6_900.0), (9_850.0, 8_000.0), (10_100.0, 9_150.0)],
    &[
        (12_950.0, 6_900.0),
        (12_750.0, 8_100.0),
        (12_550.0, 9_300.0),
    ],
    &[
        (15_800.0, 7_000.0),
        (15_350.0, 8_700.0),
        (14_750.0, 9_950.0),
    ],
];

/// An island of the isles: centre, half lengths, the angle its long axis
/// lies at, how high it stands; an atoll's hills stand in a ring.
struct Isle {
    at: (f64, f64),
    half: (f64, f64),
    angle: f64,
    peak: f64,
    ring: bool,
}

const ISLES: [Isle; 8] = [
    // The great island, Ember Isle's room in its middle; a low neck out
    // toward the heartland's shore, the sandbar across from its end.
    Isle {
        at: (3_750.0, 8_150.0),
        half: (1_500.0, 1_200.0),
        angle: 0.25,
        peak: 240.0,
        ring: false,
    },
    Isle {
        at: (5_150.0, 8_550.0),
        half: (750.0, 300.0),
        angle: 0.05,
        peak: 10.0,
        ring: false,
    },
    // The atoll.
    Isle {
        at: (1_250.0, 10_000.0),
        half: (860.0, 820.0),
        angle: 0.0,
        peak: 170.0,
        ring: true,
    },
    // Others about the sound.
    Isle {
        at: (1_700.0, 3_450.0),
        half: (650.0, 450.0),
        angle: -0.4,
        peak: 80.0,
        ring: false,
    },
    Isle {
        at: (1_000.0, 5_800.0),
        half: (800.0, 620.0),
        angle: 0.6,
        peak: 270.0,
        ring: false,
    },
    Isle {
        at: (4_700.0, 5_450.0),
        half: (520.0, 380.0),
        angle: 1.1,
        peak: 95.0,
        ring: false,
    },
    Isle {
        at: (2_750.0, 6_150.0),
        half: (360.0, 300.0),
        angle: 0.0,
        peak: 45.0,
        ring: false,
    },
    Isle {
        at: (5_250.0, 3_700.0),
        half: (340.0, 240.0),
        angle: 0.4,
        peak: 40.0,
        ring: false,
    },
];

/// Basins cut into the mountains round the sites there: (site, radius);
/// a cirque's ring of peaks round it.
const BASINS: [(&str, f64, bool); 5] = [
    ("Glass Tarn", 1_400.0, false),
    ("Hornfold", 1_300.0, false),
    ("Crown Cirque", 850.0, true),
    ("Rime Shelf", 1_200.0, false),
    ("Frostvault", 800.0, true),
];

/// The ground's plan, worked out once.
#[derive(Default)]
pub(super) struct Plan {
    base: Vec<(f64, f64)>,
    fjords: Vec<Vec<(f64, f64)>>,
    /// The river, rounded, and metres along it at each of its points.
    river: Vec<(f64, f64)>,
    river_run: Vec<f64>,
    /// The dales down to the river, rounded.
    dales: Vec<Vec<(f64, f64)>>,
    /// The hills in the heartland, one or none per cell of their grid
    /// (`hv_lay_hills`).
    hills: Vec<Option<Hill>>,
    /// Every site's way, in [`SITES`]' order (their floors worked out once
    /// the ground is: `hv_profiles`).
    pub(super) ways: Vec<Way>,
}

/// A hill: where, how wide, how high over the ground; or a sandstone
/// outcrop, a tilted bed of rock (`r` half its length) whose edge stands out
/// as a low scarp, facing `heading`'s left.
#[derive(Clone, Copy)]
struct Hill {
    at: (f64, f64),
    r: f64,
    top: f64,
    outcrop: Option<f64>,
}

/// The hills' grid.
const HILL_CELL: f64 = 1_000.0;

/// How many cells of `cell` metres cover the map's edge.
fn cells_of(cell: f64) -> usize {
    (SIZE / cell).ceil() as usize
}

impl Plan {
    pub(super) fn outlines() -> Plan {
        Plan {
            base: closed(&super::base::FACILITY.floor_outline()),
            fjords: FJORDS.iter().map(|f| rounded(f.0)).collect(),
            river: rounded(&RIVER),
            river_run: Vec::new(),
            dales: DALES.iter().map(|d| rounded(d)).collect(),
            hills: Vec::new(),
            ways: SITES.iter().map(Way::new).collect(),
        }
        .with_river_run()
    }

    fn with_river_run(mut self) -> Plan {
        let mut run = vec![0.0];
        for w in self.river.windows(2) {
            run.push(run.last().unwrap() + dist(w[0], w[1]));
        }
        self.river_run = run;
        self
    }

    /// Metres from the river's line, and how far down it (0 at its source,
    /// 1 at its mouth).
    fn river(&self, p: (f64, f64)) -> (f64, f64) {
        let (mut best, mut at) = (f64::INFINITY, 0.0);
        for (k, w) in self.river.windows(2).enumerate() {
            let (d, t) = segment(p, w[0], w[1]);
            if d < best {
                best = d;
                at = self.river_run[k] + t * (self.river_run[k + 1] - self.river_run[k]);
            }
        }
        (best, at / self.river_run.last().copied().unwrap_or(1.0))
    }

    /// Whether a point is on (or at the edge of) the base's floor.
    pub(super) fn on_floor(&self, p: (f64, f64)) -> bool {
        !self.base.is_empty() && inside(p, &self.base) > 200.0
    }

    /// Metres from the nearest way's line, less its half width.
    fn off_ways(&self, p: (f64, f64)) -> f64 {
        off_ways(&self.ways, p)
    }
}

/// A y-at-x (or x-at-y) table, eased between its corners.
fn table(t: &[(f64, f64)], v: f64) -> f64 {
    let i = t
        .windows(2)
        .position(|w| v <= w[1].0)
        .unwrap_or(t.len() - 2);
    let (a, b) = (t[i], t[i + 1]);
    let s = ((v - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    a.1 + (b.1 - a.1) * s * s * (3.0 - 2.0 * s)
}

/// A cliff's rise across its band, 0 at its foot to 1 at its rim: a talus
/// slope, then the face.
fn cliff(t: f64) -> f64 {
    0.3 * smoothstep(0.0, 0.6, t) + 0.7 * smoothstep(0.45, 1.0, t)
}

/// The raw ground at a point, and how much of it is the desert's.
pub(super) struct Raw {
    pub(super) h: f64,
    /// How much of the ground is the erosion's to furrow: the mountains, not
    /// the desert's mesas.
    erodes: f64,
}

impl Terrain {
    /// The landscape before pads, metres above the water.
    pub(in crate::bake) fn natural_harvest(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let raw = self.hv_raw(p);
        let h = raw.h + self.erosion.at(x, y) * raw.erodes;
        self.hv_cut(p, h)
    }

    /// The ground before the erosion and the plan's cuts.
    pub(super) fn hv_raw(&self, p: (f64, f64)) -> Raw {
        let plan = &self.hv.plan;
        // Each region rises from its climate walls (`HARVEST_WALLS`) read where the
        // renderer reads them for its colour (`harvest_look`, which wanders them by
        // kilometres), frayed a little more: its colour and its shape come in together.
        let q = harvest_look(p);
        let fray = |k: f64| {
            180.0
                * self
                    .ridge
                    .fbm(p.0 / 1_500.0 + k, p.1 / 1_500.0 - k, 4, 0.55)
                + 160.0
                    * self
                        .mtn_gap
                        .fbm(p.0 / 4_200.0 - k, p.1 / 4_200.0 + k, 2, 0.5)
        };
        let shore = self.hv_shore(p);
        let mut h = self.hv_lowland(p, shore);
        let lowland = h;
        let dp = harvest_depth(q, HARVEST_FJORDS) + fray(3.1);
        if dp > -500.0 {
            h = self.hv_polar(p, dp, h, shore);
        }
        let da = harvest_depth(q, HARVEST_ALPINE) + fray(7.3);
        if da > -1_000.0 {
            h = self.hv_alpine(p, da, h);
        }
        let dd = harvest_depth(q, HARVEST_DESERT) + fray(-4.7);
        let mut desert = 0.0;
        if dd > -1_100.0 {
            desert = smoothstep(-1_100.0, 0.0, dd);
            h = self.hv_desert(p, dd, h);
        }
        // Hills stand about the heartland's open country.
        if h - lowland < 1.0 && shore > 300.0 && dd < -1_200.0 && da < -1_100.0 && dp < -600.0 {
            h += self.hv_hills(p);
        }
        // The fjords wander off their lines and swell and narrow along them.
        let f = (
            p.0 + 200.0
                * self
                    .coast_warp
                    .fbm(p.0 / 1_100.0 + 4.0, p.1 / 1_100.0, 3, 0.5),
            p.1 + 200.0
                * self
                    .coast_warp
                    .fbm(p.0 / 1_100.0, p.1 / 1_100.0 - 4.0, 3, 0.5),
        );
        for (k, line) in plan.fjords.iter().enumerate() {
            let (_, half, bed) = FJORDS[k];
            let half = half * (0.85 + 0.3 * self.lake.get(p.0 / 1_600.0 + k as f64, p.1 / 1_600.0));
            // (The sound's shores wander the more, coves and points along them.)
            let f = if k == 0 {
                (
                    f.0 + 380.0 * self.coast.fbm(p.0 / 1_300.0 - 6.0, p.1 / 1_300.0, 3, 0.55),
                    f.1,
                )
            } else {
                f
            };
            let d = crate::bake::bays::polyline(f, line);
            if d < half + 900.0 {
                // Steep walls, wider the higher the ground they cut; the sound's
                // shores low and shelving, out on the tundra.
                let wall = if k == 0 {
                    120.0 + 1.6 * (h - bed).max(0.0)
                } else {
                    40.0 + 0.5 * (h - bed).max(0.0)
                };
                let w = 1.0 - smoothstep(half - 80.0, half + wall, d);
                let floor = bed + 4.0 * self.detail.fbm(p.0 / 300.0, p.1 / 300.0, 2, 0.5);
                h += (floor - h) * w;
            }
        }
        // The mountains furrowed deep, the heartland's hills and ridges into
        // small valleys; not the desert's mesas.
        let low = 0.35
            * smoothstep(200.0, 600.0, shore)
            * (1.0 - smoothstep(-900.0, -300.0, da))
            * (1.0 - smoothstep(-700.0, -200.0, dp));
        let erodes = smoothstep(140.0, 360.0, h).max(low) * (1.0 - desert);
        Raw { h, erodes }
    }

    /// Metres inland from the nearest sea (negative out at sea), roughly: the
    /// south sea, the isles' sea north to the desert's cliffs, the islands
    /// standing out of it.
    fn hv_shore(&self, p: (f64, f64)) -> f64 {
        let (x, y) = p;
        let wobble = 1_300.0 * self.coast.fbm(x / 2_600.0, y / 2_600.0, 3, 0.55)
            + 220.0 * self.coast.fbm(x / 600.0 + 9.0, y / 600.0, 2, 0.5)
            + 50.0 * self.coast_warp.get(x / 220.0, y / 220.0);
        let south = y - table(&SOUTH_COAST, x);
        let isles = (x - table(&WEST_COAST, y)).max(y - table(&ISLES_NORTH, x));
        let mut shore = south.min(isles) + wobble;
        for isle in &ISLES {
            shore = shore.max(self.hv_isle(isle, p).0);
        }
        shore
    }

    /// Metres inside an island's coast, and how far toward its middle (0 on
    /// the coast, 1 at the middle).
    fn hv_isle(&self, isle: &Isle, p: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (p.0 - isle.at.0, p.1 - isle.at.1);
        let (s, c) = isle.angle.sin_cos();
        let (u, v) = (dx * c + dy * s, dy * c - dx * s);
        let e = ((u / isle.half.0).powi(2) + (v / isle.half.1).powi(2)).sqrt();
        let r = isle.half.0.min(isle.half.1);
        if e > 2.5 {
            return (-(e - 1.0) * r, 0.0);
        }
        let l = 0.6 * r;
        let wobble = 0.3 * r * self.coast.fbm(p.0 / l + 13.0, p.1 / l - 7.0, 3, 0.55)
            + 0.1 * r * self.coast_warp.get(p.0 / (0.2 * r), p.1 / (0.2 * r));
        let sd = (1.0 - e) * r + wobble;
        (sd, (sd / r).clamp(0.0, 1.0))
    }

    /// The heartland's rolling lowland, rising north up the valley to the
    /// base; the sea's floor off its coasts; the islands.
    fn hv_lowland(&self, p: (f64, f64), shore: f64) -> f64 {
        let (x, y) = p;
        let sea = -3.0 - 44.0 * smoothstep(0.0, 1_100.0, -shore)
            + 5.0 * self.detail.fbm(x / 700.0, y / 700.0, 2, 0.5);
        if shore <= 0.0 {
            return sea;
        }
        let inland = smoothstep(0.0, 2_400.0, shore);
        let north = smoothstep(9_000.0, 18_800.0, y);
        // Rolling country: long low swells, broad rises between them, and low
        // ridges running across the heartland; dales down to the river.
        let roll = self.lake.fbm(x / 2_300.0, y / 2_300.0, 4, 0.5);
        let rises = self.ramp.fbm(x / 3_600.0 + 3.7, y / 3_600.0 - 1.3, 3, 0.5);
        let calm = SPAWNS.iter().fold(1.0_f64, |k, s| {
            k.min(smoothstep(500.0, 1_500.0, dist(p, s.0)))
        });
        let mut h = 3.0
            + 24.0 * inland
            + (BASE_H - 30.0) * north
            + 16.0 * roll * inland
            + (34.0 * smoothstep(-0.12, 0.55, rises) + 40.0 * self.hv_ridges(p)) * inland * calm
            - 18.0 * self.hv_dales(p) * inland;
        // The river's broad shallow valley (its channel is cut after the
        // erosion, `hv_cut`, which would silt it up).
        if let Some((d, half, plain)) = self.hv_river(p) {
            if h > plain {
                h = plain + (h - plain) * smoothstep(half + 50.0, half + 1_000.0, d).powf(0.7);
            }
        }
        // The isles' hills: jungle peaks, or an atoll's ring.
        for isle in &ISLES {
            let (sd, toward) = self.hv_isle(isle, p);
            if sd <= 0.0 {
                continue;
            }
            let crags = 0.65 + 0.35 * self.crag.ridged(x / 420.0 + 5.1, y / 420.0 - 2.3, 3, 0.5);
            let rise = if isle.ring {
                bump(((toward - 0.24) / 0.2).abs()) * crags
            } else {
                smoothstep(0.05, 0.85, toward).powf(1.4) * crags
            };
            h = h.max(4.0 + 6.0 * smoothstep(0.0, 0.3, toward) + isle.peak * rise);
        }
        // Down to the water over a beach.
        sea + (h - sea) * smoothstep(0.0, 140.0, shore)
    }

    /// Near the river: metres from its (wandering) line, half its channel's
    /// width there (narrow where it springs from the mountains' foot), and
    /// the height its valley's floor eases down to.
    fn hv_river(&self, (x, y): (f64, f64)) -> Option<(f64, f64, f64)> {
        let wander = (
            x + 180.0 * self.lake_shore.fbm(x / 900.0, y / 900.0, 2, 0.5),
            y + 180.0 * self.lake_shore.fbm(x / 900.0 + 5.0, y / 900.0, 2, 0.5),
        );
        let (d, down) = self.hv.plan.river(wander);
        (d < 1_200.0).then(|| {
            let half = (26.0 + 34.0 * down) * (0.2 + 0.8 * smoothstep(0.0, 0.12, down));
            (d, half, 9.0 - 5.0 * down)
        })
    }

    /// Metres inside the base's floor (negative outside), its edge frayed so
    /// the floor gives way to the country in lobes, not along a ruled line.
    fn hv_on_base(&self, p: (f64, f64)) -> f64 {
        let frayed = inside(p, &self.hv.plan.base)
            + 260.0 * self.mtn_gap.fbm(p.0 / 1_100.0 + 9.0, p.1 / 1_100.0, 3, 0.5);
        // (Always well round the machine's bastions, whose benches are cut at
        // the floor's level: no notch in the hills behind.)
        super::base::FACILITY
            .bastions()
            .iter()
            .fold(frayed, |b, &(at, _, _)| b.max(700.0 - dist(p, at)))
    }

    /// Low ridges across the heartland, long and wandering, 0 to 1: broad
    /// shoulders under a rounded crest, nowhere steep.
    fn hv_ridges(&self, (x, y): (f64, f64)) -> f64 {
        let l = 3_200.0;
        let (wx, wy) = (
            900.0 * self.warp_x.fbm(x / 4_500.0 + 2.0, y / 4_500.0, 2, 0.5),
            900.0 * self.warp_y.fbm(x / 4_500.0, y / 4_500.0 + 2.0, 2, 0.5),
        );
        // (Long east to west, as the river runs.)
        let crest = self
            .crag
            .ridged((x + wx) / (1.8 * l) + 7.7, (y + wy) / l - 2.2, 2, 0.45);
        let t = smoothstep(0.45, 0.97, crest);
        t * t * (0.75 + 0.25 * self.detail.fbm(x / 900.0, y / 900.0, 2, 0.5))
    }

    /// The heartland's dales, 0 to 1: shallow valleys wandering down to the
    /// river from the rises either side of it, deepest at their mouths.
    fn hv_dales(&self, p: (f64, f64)) -> f64 {
        let w = (
            p.0 + 220.0
                * self
                    .lake_shore
                    .fbm(p.0 / 1_100.0 - 3.0, p.1 / 1_100.0, 3, 0.5),
            p.1 + 220.0
                * self
                    .lake_shore
                    .fbm(p.0 / 1_100.0, p.1 / 1_100.0 + 6.0, 3, 0.5),
        );
        let mut deep: f64 = 0.0;
        for line in &self.hv.plan.dales {
            let (d, down) = crate::bake::bays::polyline_at(w, line);
            let wide = 160.0 + 260.0 * down;
            deep = deep.max(smoothstep(0.0, 0.5, down) * (1.0 - smoothstep(0.2 * wide, wide, d)));
        }
        deep
    }

    /// The desert at `dd` metres inside its outline, over the ground `low`
    /// outside it: mesas where a slow field comes up, run together into a rim
    /// of mesa walls along its edge, their tops at the canyon's beds; the
    /// sunken flats between them; the hollow's ring. Kept off the rooms; the
    /// ways cut their own canyons through.
    fn hv_desert(&self, p: (f64, f64), dd: f64, low: f64) -> f64 {
        let (x, y) = p;
        let flats = 84.0
            + 9.0 * self.tilt.fbm(x / 2_200.0, y / 2_200.0, 2, 0.5)
            + 3.0 * self.detail.ridged(x / 160.0 + 2.0, y / 90.0 - 1.0, 2, 0.5);
        let l = 1_500.0;
        let (wx, wy) = (
            420.0 * self.warp_x.get(x / 2_100.0 + 3.0, y / 2_100.0),
            420.0 * self.warp_y.get(x / 2_100.0, y / 2_100.0 + 3.0),
        );
        // A rim of mesa walls a kilometre or so in from the edge, broken; outside
        // it the mesas thin to lone buttes standing out over the heartland,
        // none on a way; the flats rise gently out of the lowland.
        let rim_w = 1_100.0 + 450.0 * self.mtn_gap.fbm(x / 1_900.0, y / 1_900.0, 2, 0.5);
        let rim = 0.42
            * smoothstep(800.0, 1_400.0, dd)
            * (1.0 - smoothstep(1.4 * rim_w, 1.9 * rim_w, dd));
        let mut mesa = self.mtn_gap.fbm((x + wx) / l, (y + wy) / l, 4, 0.55) + 0.02 + rim
            - 0.16 * (1.0 - smoothstep(-300.0, 900.0, dd));
        for s in SITES.iter().filter(|s| s.cut < 0.5) {
            let d = dist(p, s.at);
            mesa -= 1.2 * (1.0 - smoothstep(s.room() + 150.0, s.room() + 900.0, d));
        }
        if dd < 400.0 {
            for (way, s) in self.hv.plan.ways.iter().zip(SITES) {
                if let (Some((d, _)), true) = (way.near(p), s.cut < 0.5) {
                    mesa -= 0.6 * (1.0 - smoothstep(way.half + 150.0, way.half + 500.0, d));
                }
            }
        }
        let ground = low + (flats - low) * smoothstep(-100.0, 700.0, dd);
        let tier = self.mtn_height.fbm(x / 2_600.0 + 1.0, y / 2_600.0, 3, 0.5);
        let tops = 205.0
            + 66.0 * cliff((tier + 0.12) / 0.05)
            + 70.0 * cliff((tier - 0.18) / 0.05)
            + 4.0 * self.detail.get(x / 500.0, y / 500.0);
        let mut h = ground + (tops - ground) * cliff(mesa / 0.07) * smoothstep(-500.0, -150.0, dd);
        // The hollow: a ring of mesa round the artillery's room.
        let hollow = SITES.iter().find(|s| s.name == "The Hollow").unwrap();
        let d = dist(p, hollow.at)
            + 150.0 * self.ridge.fbm(x / 400.0, y / 400.0, 2, 0.5)
            + 280.0 * self.ridge.fbm(x / 1_300.0 - 4.0, y / 1_300.0, 2, 0.5);
        if d < 1_600.0 {
            let ring = cliff((d - 520.0) / 140.0) * (1.0 - cliff((d - 1_250.0) / 200.0));
            h = h.max(flats + (300.0 + 40.0 * tier - flats) * ring);
        }
        h
    }

    /// The alpine country at `da` metres inside its outline, over `low`: its
    /// front, its peaks, the basins round the sites, the cirque's ring.
    fn hv_alpine(&self, p: (f64, f64), da: f64, low: f64) -> f64 {
        let (x, y) = p;
        let mass = smoothstep(-300.0, 3_000.0, da);
        let l = 2_600.0;
        let (wx, wy) = (
            600.0 * self.warp_x.get(x / l + 5.0, y / l),
            600.0 * self.warp_y.get(x / l, y / l + 5.0),
        );
        let big = self
            .crag
            .ridged((x + wx) / 2_600.0, (y + wy) / 2_600.0, 5, 0.5)
            .max(0.0);
        let small = self.crag.ridged(x / 700.0 + 3.1, y / 700.0 - 7.7, 3, 0.5);
        let peaks = 170.0
            + 360.0 * mass
            + (300.0 + 900.0 * mass) * big.powf(1.8)
            + 150.0 * small * big
            + 50.0 * self.mtn_height.get(x / 2_900.0, y / 2_900.0);
        // Foothills rise out of the lowland before the wall.
        let h = low + (peaks - low) * smoothstep(-900.0, 900.0, da);
        let h = self.site_valleys(&self.hv.plan.ways, p, h, low);
        self.hv_basins(p, h, HARVEST_ALPINE)
    }

    /// The fjords' plateau at `dp` metres inside its outline, over `low`:
    /// rising from its coasts, mountains on it in ranges, the basins round
    /// the sites there. (The fjords are cut after.)
    fn hv_polar(&self, p: (f64, f64), dp: f64, low: f64, shore: f64) -> f64 {
        let (x, y) = p;
        // Tundra rising slowly off the heartland past the sound, then the
        // plateau, its ranges further in toward the fjords.
        let plateau = 22.0
            + 128.0 * smoothstep(0.0, 4_200.0, dp)
            + 22.0 * self.tilt.fbm(x / 1_500.0 + 9.0, y / 1_500.0, 3, 0.5);
        let ranges = smoothstep(
            -0.3,
            0.25,
            self.mtn_mask.fbm(x / 3_000.0, y / 3_000.0, 2, 0.5),
        ) * smoothstep(1_400.0, 3_400.0, dp);
        let big = self
            .crag
            .ridged(x / 2_200.0 + 11.0, y / 2_200.0 - 4.0, 4, 0.5)
            .max(0.0);
        let hills = self.crag.ridged(x / 900.0 - 3.0, y / 900.0 + 6.0, 3, 0.5);
        let ground = plateau
            + 70.0 * hills * smoothstep(500.0, 2_400.0, dp)
            + ranges * (160.0 + 560.0 * big.powf(1.5));
        let w = smoothstep(-400.0, 1_600.0, dp) * smoothstep(0.0, 450.0, shore);
        let h = low + (ground - low) * w;
        let h = self.site_valleys(&self.hv.plan.ways, p, h, low);
        self.hv_basins(p, h, HARVEST_FJORDS)
    }

    /// The basins round a region's sites: the ground eased down to a bowl
    /// round the room's level; a cirque's ring of peaks kept round it.
    fn hv_basins(&self, p: (f64, f64), mut h: f64, region: usize) -> f64 {
        for &(name, r, ring) in &BASINS {
            let site = SITES.iter().find(|s| s.name == name).unwrap();
            let in_region = match region {
                HARVEST_ALPINE => site.at.1 > 11_500.0,
                _ => site.at.1 <= 11_500.0,
            };
            if !in_region {
                continue;
            }
            let wobble = 0.18 * r * self.ridge.fbm(p.0 / (0.5 * r), p.1 / (0.5 * r), 2, 0.5);
            let d = dist(p, site.at) + wobble;
            if d > r + 900.0 {
                continue;
            }
            let t = d / r;
            let bowl = site.level + 90.0 * t * t;
            h += (bowl - h) * (1.0 - smoothstep(0.55, 1.0, t));
            if ring {
                let wide = 1.0 + 0.35 * self.mtn_gap.fbm(p.0 / 500.0 + 2.0, p.1 / 500.0, 2, 0.5);
                let crest = bump(((d - r - 320.0) / (420.0 * wide)).abs());
                let tall = 300.0
                    + 160.0
                        * self
                            .crag
                            .ridged(p.0 / 380.0 + 7.0, p.1 / 380.0 - 1.0, 3, 0.5);
                h = h.max(site.level + tall * crest);
            }
        }
        h
    }

    /// Wooded hills about the heartland, and here and there a rocky outcrop:
    /// one may stand in each cell of a jittered grid (`hv_lay_hills`).
    fn hv_hills(&self, p: (f64, f64)) -> f64 {
        let n = cells_of(HILL_CELL);
        let (ci, cj) = ((p.0 / HILL_CELL) as i64, (p.1 / HILL_CELL) as i64);
        let mut add: f64 = 0.0;
        for dj in -1..=1 {
            for di in -1..=1 {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i >= n as i64 || j >= n as i64 {
                    continue;
                }
                let Some(k) = self.hv.plan.hills[j as usize * n + i as usize] else {
                    continue;
                };
                let shape = match k.outcrop {
                    Some(heading) => self.hv_outcrop(p, &k, heading),
                    None => {
                        let l = 0.7 * k.r;
                        let d = dist(p, k.at)
                            + 0.3 * k.r * self.ridge.fbm(p.0 / l, p.1 / l + 9.0, 3, 0.5);
                        let t = (d / k.r).min(1.0);
                        (1.0 - t * t).powi(2)
                    }
                };
                add = add.max(k.top * shape);
            }
        }
        add
    }

    /// An outcrop's rise, 0 to 1: its bed climbing gently across it to a
    /// crest near one edge, then the scarp, weathered, falling away; tapering
    /// at its ends, its outline and crest wandering.
    fn hv_outcrop(&self, p: (f64, f64), k: &Hill, heading: f64) -> f64 {
        let (s, c) = heading.sin_cos();
        let (dx, dy) = (p.0 - k.at.0, p.1 - k.at.1);
        let wob = |f: f64, z: f64| self.ridge.fbm(p.0 / f + z, p.1 / f - z, 3, 0.5);
        let u = (dx * c + dy * s) / k.r + 0.25 * wob(0.6 * k.r, 3.0);
        let v = (dy * c - dx * s) / (0.45 * k.r) + 0.3 * wob(0.35 * k.r, -5.0);
        if u.abs() >= 1.0 || v.abs() >= 1.0 {
            return 0.0;
        }
        let ends = (1.0 - u * u).powf(1.4);
        let crest = 0.5 + 0.12 * wob(0.25 * k.r, 11.0);
        let back = smoothstep(-1.0, crest, v);
        let scarp = 1.0 - smoothstep(crest, crest + 0.3, v);
        // (Ledges where the beds break along the face.)
        let ledges = 0.92 + 0.08 * self.crag.ridged(p.0 / 45.0, p.1 / 45.0, 2, 0.5);
        ends * back * scarp * ledges
    }

    /// Lays the hills, cell by cell, in clusters where a slow field comes up;
    /// none on a way, near a start, an ore field, the river or the base.
    pub(super) fn hv_lay_hills(&mut self) {
        let plan = &self.hv.plan;
        let n = cells_of(HILL_CELL);
        let hills = (0..n * n)
            .map(|k| {
                let (i, j) = ((k % n) as i64, (k / n) as i64);
                let hash = hash2(self.seed ^ 0x6b6e_6f6c, i, j);
                let c = (
                    (i as f64 + 0.15 + 0.7 * unit(hash, 8)) * HILL_CELL,
                    (j as f64 + 0.15 + 0.7 * unit(hash, 16)) * HILL_CELL,
                );
                let cluster = self
                    .mtn_mask
                    .fbm(c.0 / 4_000.0 + 7.0, c.1 / 4_000.0 - 3.0, 2, 0.5);
                let rocky = unit(hash, 40) < 0.3;
                let r = if rocky {
                    220.0 + 180.0 * unit(hash, 24)
                } else {
                    300.0 + 350.0 * unit(hash, 24)
                };
                let picked = unit(hash, 0) < 0.45 * smoothstep(-0.2, 0.2, cluster)
                    && plan.off_ways(c) > r + 160.0
                    && SPAWNS.iter().all(|s| dist(c, s.0) > 1_400.0)
                    && ORE.iter().all(|o| dist(c, (o.0, o.1)) > o.2 + r + 200.0)
                    && inside(c, &plan.base) < -700.0
                    && plan.river(c).0 > r + 250.0;
                picked.then(|| Hill {
                    at: c,
                    r,
                    top: if rocky {
                        16.0 + 20.0 * unit(hash, 32)
                    } else {
                        25.0 + 40.0 * unit(hash, 32)
                    },
                    // (The beds dip about the same way across the heartland.)
                    outcrop: rocky.then(|| 0.5 + 0.9 * (unit(hash2(hash, 7, 1), 0) - 0.5)),
                })
            })
            .collect();
        self.hv.plan.hills = hills;
    }

    /// What the plan cuts into the ground: every way, every room, the base's floor.
    fn hv_cut(&self, p: (f64, f64), mut h: f64) -> f64 {
        let plan = &self.hv.plan;
        // The rooms first: a way leaving or entering one is held at its level
        // through it, and cuts on through the room's walls.
        for s in SITES {
            h = self.site_room(s, p, h);
        }
        if let Some((d, half, _)) = self.hv_river(p) {
            h += (-2.6 - h) * (1.0 - smoothstep(half - 8.0, half + 28.0, d));
        }
        let mut heartland = None;
        for way in &plan.ways {
            let Some((d, along)) = way.near(p) else {
                continue;
            };
            if way.sea {
                // A sea lane: deep enough all along, from past the site's slips.
                if along < PRINTER_ROOM + 100.0 {
                    continue;
                }
                let target = -LANE_DEEP - 2.0;
                if h > target {
                    h += (target - h) * (1.0 - smoothstep(way.half, way.half + 220.0, d));
                }
                continue;
            }
            if way.profile.is_empty() {
                continue;
            }
            let floor = way.floor(along);
            // Across the heartland a way is a narrow strip, its banks long and
            // low: the country round it is its road.
            let lowland = *heartland.get_or_insert_with(|| {
                smoothstep(
                    -200.0,
                    400.0,
                    harvest_depth(harvest_look(p), HARVEST_HEARTLAND),
                )
            });
            let (run, most) = if h > floor {
                (way.cut + (LOW_CUT - way.cut) * lowland, SHOULDER)
            } else {
                (FILL, FILL_MOST)
            };
            let half = way.half + (40.0 - way.half) * lowland;
            let edge = half + (run * (h - floor).abs() + 8.0).min(most);
            h += (floor - h) * (1.0 - smoothstep(half, edge, d));
        }
        // The rooms' floors once more, exactly, over anything a way's walls
        // spread into them.
        for s in SITES.iter().filter(|s| s.ground == Ground::Land) {
            let d = dist(p, s.at);
            let core = s.room() + 40.0;
            if d < core + 40.0 {
                h += (s.level - h) * (1.0 - smoothstep(core, core + 40.0, d));
            }
        }
        if !plan.base.is_empty() {
            let b = self.hv_on_base(p);
            if b > -BASE_BLEND {
                h += (BASE_H - h) * smoothstep(-BASE_BLEND, 0.0, b);
            }
        }
        h
    }

    /// Works out every way's floor, once the ground is laid.
    pub(super) fn hv_profiles(&mut self) {
        let profiles: Vec<Vec<f64>> = self
            .hv
            .plan
            .ways
            .iter()
            .map(|w| {
                if w.sea {
                    Vec::new()
                } else {
                    Self::way_profile(w, SITES, |q| {
                        let r = self.hv_raw(q);
                        r.h + self.erosion.at(q.0, q.1) * r.erodes
                    })
                }
            })
            .collect();
        for (w, p) in self.hv.plan.ways.iter_mut().zip(profiles) {
            w.profile = p;
        }
    }
}
