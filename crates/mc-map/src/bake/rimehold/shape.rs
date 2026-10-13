//! Rimehold's ground: the hold and the sea round it, the necks across, the
//! mainland wrapped round on three sides (the highlands north, the base's
//! country east, the fells south), Wester Isle and the skerries in the ocean,
//! bergs aground and sea ice along the mainland's shores; then what the plan
//! cuts into all that: every site's way and room, the base's sea gates and
//! its floor.
//!
//! The coasts are read off a grid of distances worked out once from the
//! outlines and frayed by noise, so no shore runs true; the ways and rooms are
//! cut exactly, after the erosion, so they stay level and drivable whatever
//! the noise did.

use super::*;
use crate::bake::bays::{inside, polyline_at};
use crate::bake::canyon::{smooth_closed, smooth_open};
use crate::bake::sites::{FILL, FILL_MOST, LANE_DEEP, LOW_CUT, PRINTER_ROOM, SHOULDER};
use crate::noise::bump;
use std::f64::consts::PI;

/// The hold's coast: coves and headlands round it, the necks' landings.
const HOLD: [(f64, f64); 32] = [
    (5_250.0, 10_150.0),
    (5_500.0, 10_700.0),
    (5_150.0, 11_250.0),
    (5_450.0, 11_650.0),
    (5_050.0, 12_150.0),
    (5_700.0, 12_700.0),
    (6_250.0, 12_500.0),
    (6_700.0, 13_050.0),
    (7_300.0, 12_650.0),
    (7_850.0, 12_850.0),
    (8_400.0, 13_100.0),
    (9_000.0, 12_650.0),
    (9_600.0, 12_900.0),
    (10_250.0, 12_550.0),
    (10_350.0, 12_150.0),
    (10_900.0, 11_850.0),
    (10_700.0, 11_100.0),
    (10_850.0, 10_240.0),
    (11_050.0, 9_500.0),
    (10_650.0, 9_000.0),
    (10_850.0, 8_400.0),
    (10_250.0, 7_950.0),
    (9_700.0, 8_050.0),
    (9_200.0, 7_500.0),
    (8_400.0, 7_550.0),
    (7_700.0, 7_300.0),
    (7_100.0, 7_750.0),
    (6_400.0, 7_500.0),
    (5_700.0, 7_900.0),
    (5_500.0, 8_500.0),
    (5_050.0, 8_900.0),
    (5_400.0, 9_500.0),
];

/// The mainland's coast, round the hold's sea from the fells' western cape to
/// the highlands', and out round the map's edges.
const MAINLAND: [(f64, f64); 37] = [
    (2_400.0, -600.0),
    (21_100.0, -600.0),
    (21_100.0, 21_100.0),
    (3_000.0, 21_100.0),
    (2_900.0, 18_500.0),
    (2_500.0, 16_800.0),
    (3_100.0, 15_300.0),
    (3_400.0, 14_700.0),
    (4_300.0, 14_450.0),
    (5_300.0, 14_650.0),
    (6_400.0, 14_400.0),
    (7_250.0, 14_600.0),
    (8_100.0, 14_450.0),
    (9_200.0, 14_700.0),
    (10_300.0, 14_450.0),
    (11_200.0, 14_600.0),
    (12_100.0, 14_250.0),
    (12_500.0, 13_400.0),
    (12_250.0, 12_400.0),
    (12_350.0, 11_300.0),
    (12_450.0, 10_240.0),
    (12_300.0, 9_200.0),
    (12_450.0, 8_100.0),
    (12_200.0, 7_000.0),
    (11_800.0, 6_300.0),
    (11_000.0, 6_000.0),
    (10_000.0, 5_850.0),
    (9_000.0, 6_050.0),
    (8_300.0, 5_850.0),
    (7_400.0, 6_050.0),
    (6_400.0, 5_800.0),
    (5_300.0, 6_000.0),
    (4_300.0, 5_750.0),
    (3_300.0, 5_900.0),
    (2_700.0, 5_300.0),
    (2_300.0, 3_800.0),
    (2_600.0, 2_200.0),
];

/// Wester Isle's coast.
const WESTER: [(f64, f64); 12] = [
    (1_000.0, 9_000.0),
    (1_600.0, 8_400.0),
    (2_400.0, 8_300.0),
    (3_200.0, 8_700.0),
    (3_750.0, 9_500.0),
    (3_800.0, 10_250.0),
    (3_600.0, 11_000.0),
    (3_200.0, 11_700.0),
    (2_400.0, 12_150.0),
    (1_500.0, 12_000.0),
    (900.0, 11_200.0),
    (800.0, 10_100.0),
];

/// A neck: its line (from the hold), half its width, its crown over the water.
type Neck = (&'static [(f64, f64)], f64, f64);

const NECKS: [Neck; 4] = [
    (&NORTH_NECK, 340.0, 7.0),
    (&SOUTH_NECK, 380.0, 8.0),
    (&WEST_NECK, 300.0, 6.0),
    (&CAUSEWAY, 620.0, 9.0),
];

/// A skerry or the atoll: centre, half lengths, the angle its long axis lies
/// at, how high it stands; an atoll's hills stand in a ring.
struct Isle {
    at: (f64, f64),
    half: (f64, f64),
    angle: f64,
    peak: f64,
    ring: bool,
}

const ISLES: [Isle; 9] = [
    // Far Skerry, the Gatewright's atoll.
    Isle {
        at: (1_100.0, 2_900.0),
        half: (860.0, 820.0),
        angle: 0.0,
        peak: 150.0,
        ring: true,
    },
    Isle {
        at: (4_450.0, 12_050.0),
        half: (260.0, 180.0),
        angle: 0.3,
        peak: 45.0,
        ring: false,
    },
    Isle {
        at: (4_350.0, 8_350.0),
        half: (220.0, 160.0),
        angle: -0.5,
        peak: 35.0,
        ring: false,
    },
    Isle {
        at: (700.0, 15_600.0),
        half: (500.0, 350.0),
        angle: 0.8,
        peak: 120.0,
        ring: false,
    },
    Isle {
        at: (1_400.0, 18_400.0),
        half: (650.0, 420.0),
        angle: -0.3,
        peak: 180.0,
        ring: false,
    },
    Isle {
        at: (500.0, 12_700.0),
        half: (300.0, 220.0),
        angle: 0.2,
        peak: 60.0,
        ring: false,
    },
    Isle {
        at: (600.0, 5_200.0),
        half: (380.0, 260.0),
        angle: 0.6,
        peak: 70.0,
        ring: false,
    },
    // The skerries behind Gull Skerry's and Seal Skerry's slips.
    Isle {
        at: (1_150.0, 13_800.0),
        half: (520.0, 380.0),
        angle: 1.2,
        peak: 70.0,
        ring: false,
    },
    Isle {
        at: (900.0, 6_880.0),
        half: (520.0, 380.0),
        angle: 1.2,
        peak: 60.0,
        ring: false,
    },
];

/// A fjord: its line from the sea in to its head, and half its width.
type Fjord = (&'static [(f64, f64)], f64);

/// Fjords into the highlands and the fells off the straits' western reaches.
const FJORDS: [Fjord; 2] = [
    (
        &[
            (3_500.0, 14_300.0),
            (3_350.0, 15_300.0),
            (3_550.0, 16_300.0),
            (3_350.0, 17_100.0),
        ],
        300.0,
    ),
    (
        &[
            (4_500.0, 6_100.0),
            (4_250.0, 5_100.0),
            (4_450.0, 4_400.0),
            (4_200.0, 3_800.0),
        ],
        260.0,
    ),
];

/// The naval sites on the mainland's shore: each harbour is an inlet cut
/// steep and deep from its quay out to open water, along its lane's first
/// leg: how far out it starts and runs, and half its width.
const HARBOURS: [&str; 2] = ["Ice Harbour", "Floe Harbour"];
const HARBOUR: (f64, f64, f64) = (150.0, 1_100.0, 300.0);

/// Tabular bergs aground: centre, radius, height over the water.
const BERGS: [(f64, f64, f64, f64); 7] = [
    (1_900.0, 15_400.0, 160.0, 22.0),
    (400.0, 9_000.0, 140.0, 20.0),
    (2_000.0, 4_600.0, 150.0, 20.0),
    (6_000.0, 14_100.0, 120.0, 18.0),
    (11_350.0, 13_450.0, 130.0, 20.0),
    (10_000.0, 6_250.0, 120.0, 18.0),
    (4_800.0, 6_400.0, 150.0, 22.0),
];

/// The base's sea gates, one on each lagoon, facing west: ships leave
/// westward out of a basin cut into the coast (`precursor_forge.rs`).
pub(super) const SEA_GATES: [(f64, f64); 2] = [(12_350.0, 12_800.0), (12_350.0, 7_700.0)];
pub(super) const SEA_GATE_HEADING: f64 = PI;
/// A sea gate's basin in its own frame: from its back wall to past its
/// mouth, and half its width (the moles stand inside it, in water).
const BASIN: (f64, f64, f64) = (-450.0, 420.0, 320.0);

/// Basins eased into the heights round the sites there: (site, radius,
/// whether a ring of peaks stands round it).
const BASINS: [(&str, f64, bool); 7] = [
    ("Hoarfrost", 1_300.0, false),
    ("Whitecap", 1_300.0, false),
    ("Rime Cirque", 800.0, true),
    ("Scarfell", 1_100.0, false),
    ("Blackwater", 1_200.0, false),
    ("Frostvault", 750.0, true),
    ("Echo Vault", 800.0, true),
];

/// How deep the straits and lagoons run, metres; the ocean more.
const DEEP: f64 = 48.0;
const OCEAN: f64 = 28.0;
/// The sea ice: its top over the water (dry, flat, walkable), the most it
/// reaches out from the mainland's and the isles' shores (never from the
/// hold's, whose harbours stay open), and the bed under its edges, deep
/// enough for ships.
const SEA_ICE_H: f64 = 1.3;
const SEA_ICE_REACH: f64 = 260.0;
const UNDER_EDGE: f64 = -9.0;
/// How far round the base's floor the ground takes to come back.
const FLOOR_BLEND: f64 = 520.0;

/// Distances to the outlines, worked out once on a grid this fine.
const GRID_STEP: f64 = 32.0;

/// A signed distance on a grid, read between its samples.
#[derive(Default)]
struct Field {
    n: usize,
    values: Vec<f32>,
}

impl Field {
    fn of(poly: &[(f64, f64)]) -> Field {
        let n = (SIZE / GRID_STEP) as usize + 2;
        let mut values = vec![0f32; n * n];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in values.chunks_mut(rows * n).enumerate() {
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(n).enumerate() {
                        let y = (k * rows + r) as f64 * GRID_STEP;
                        for (i, v) in row.iter_mut().enumerate() {
                            *v = inside((i as f64 * GRID_STEP, y), poly) as f32;
                        }
                    }
                });
            }
        });
        Field { n, values }
    }

    fn at(&self, (x, y): (f64, f64)) -> f64 {
        let f = |v: f64| (v / GRID_STEP).clamp(0.0, (self.n - 1) as f64 - 1e-6);
        let (fx, fy) = (f(x), f(y));
        let (i, j) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let v = |i: usize, j: usize| self.values[j * self.n + i] as f64;
        let a = v(i, j) + (v(i + 1, j) - v(i, j)) * tx;
        let b = v(i, j + 1) + (v(i + 1, j + 1) - v(i, j + 1)) * tx;
        a + (b - a) * ty
    }
}

/// The ground's plan, worked out once.
#[derive(Default)]
pub(super) struct Plan {
    hold: Field,
    main: Field,
    wester: Field,
    necks: Vec<Vec<(f64, f64)>>,
    fjords: Vec<Vec<(f64, f64)>>,
    floor: Vec<(f64, f64)>,
    /// Every site's way, in [`SITES`]' order (their floors worked out once
    /// the ground is: `rh_profiles`).
    pub(super) ways: Vec<Way>,
}

impl Plan {
    pub(super) fn outlines() -> Plan {
        Plan {
            hold: Field::of(&smooth_closed(&HOLD, 6)),
            main: Field::of(&smooth_closed(&MAINLAND, 6)),
            wester: Field::of(&smooth_closed(&WESTER, 6)),
            necks: NECKS.iter().map(|n| smooth_open(n.0, 8)).collect(),
            fjords: FJORDS.iter().map(|f| smooth_open(f.0, 8)).collect(),
            floor: smooth_closed(&FACILITY.floor_outline(), 6),
            ways: ways(),
        }
    }

    /// Whether a point is on (or at the edge of) the base's floor.
    pub(super) fn on_floor(&self, p: (f64, f64)) -> bool {
        !self.floor.is_empty() && inside(p, &self.floor) > 200.0
    }
}

/// Metres inland of each coast at a point (negative out at sea).
#[derive(Clone, Copy)]
struct Coasts {
    hold: f64,
    main: f64,
    isle: f64,
    /// The nearest neck's: metres in from its edge, and its crown there.
    neck: f64,
    neck_top: f64,
    /// Metres from the nearest fjord's water (negative in it).
    fjord: f64,
}

impl Coasts {
    fn land(&self) -> f64 {
        self.hold
            .max(self.main)
            .max(self.isle)
            .max(self.neck)
            .min(self.fjord)
    }
}

/// The raw ground at a point, and how much of it is the erosion's to furrow.
pub(super) struct Raw {
    pub(super) h: f64,
    pub(super) erodes: f64,
}

impl Terrain {
    /// The landscape before pads, metres above the water.
    pub(in crate::bake) fn natural_rimehold(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let raw = self.rh_raw(p);
        let h = raw.h + self.erosion.at(x, y) * raw.erodes;
        self.rh_cut(p, h)
    }

    /// Every coast at a point, frayed.
    fn rh_coasts(&self, p: (f64, f64)) -> Coasts {
        let plan = &self.rh.plan;
        let (x, y) = p;
        let wobble = 240.0 * self.coast.fbm(x / 1_900.0, y / 1_900.0, 3, 0.55)
            + 80.0 * self.coast.fbm(x / 520.0 + 9.0, y / 520.0, 2, 0.5)
            + 22.0 * self.coast_warp.get(x / 160.0, y / 160.0);
        let mut isle = plan.wester.at(p) + wobble;
        for s in &ISLES {
            isle = isle.max(self.rh_isle(s, p).0);
        }
        let (mut neck, mut neck_top) = (f64::NEG_INFINITY, 0.0);
        for (k, line) in plan.necks.iter().enumerate() {
            let (_, half, top) = NECKS[k];
            let (d, t) = polyline_at(p, line);
            if d > 2.0 * half + 1_200.0 {
                continue;
            }
            // Narrowest in the middle, shores wandering a little.
            let half = half * (0.8 + 0.4 * (2.0 * t - 1.0).powi(2))
                + 30.0 * self.coast.fbm(x / 300.0 - 4.0, y / 300.0 + 7.0, 2, 0.5);
            let s = half - d;
            if s > neck {
                neck = s;
                neck_top = top + 1.5 * self.detail.fbm(x / 150.0, y / 150.0, 2, 0.5);
            }
        }
        // The fjords wander off their lines and swell and narrow along them,
        // narrowing to their heads.
        let f = (
            x + 160.0 * self.coast_warp.fbm(x / 1_100.0 + 4.0, y / 1_100.0, 3, 0.5),
            y + 160.0 * self.coast_warp.fbm(x / 1_100.0, y / 1_100.0 - 4.0, 3, 0.5),
        );
        let mut fjord = f64::INFINITY;
        for (k, line) in plan.fjords.iter().enumerate() {
            let (_, half) = FJORDS[k];
            let (d, t) = polyline_at(f, line);
            let half = half
                * (0.85 + 0.3 * self.lake.get(x / 1_600.0 + k as f64, y / 1_600.0))
                * (1.0 - 0.5 * smoothstep(0.6, 1.0, t));
            fjord = fjord.min(d - half);
        }
        for name in HARBOURS {
            let site = SITES.iter().find(|s| s.name == name).unwrap();
            let first = site.way[0];
            let l = dist(first, site.at);
            let u = ((first.0 - site.at.0) / l, (first.1 - site.at.1) / l);
            let (a, b) = (HARBOUR.0, HARBOUR.1);
            let d = crate::bake::bays::segment(
                p,
                (site.at.0 + a * u.0, site.at.1 + a * u.1),
                (site.at.0 + b * u.0, site.at.1 + b * u.1),
            )
            .0;
            fjord = fjord.min(d - HARBOUR.2);
        }
        Coasts {
            hold: plan.hold.at(p) + wobble,
            main: plan.main.at(p) + wobble,
            isle,
            neck,
            neck_top,
            fjord,
        }
    }

    /// Metres inside a skerry's coast, and how far toward its middle (0 on
    /// the coast, 1 at the middle).
    fn rh_isle(&self, isle: &Isle, p: (f64, f64)) -> (f64, f64) {
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

    /// The ground before the erosion and the plan's cuts.
    pub(super) fn rh_raw(&self, p: (f64, f64)) -> Raw {
        let (x, y) = p;
        let c = self.rh_coasts(p);
        let s = c.land();
        // The sea's floor, shelving off every shore, deeper out in the ocean;
        // a fjord's or a harbour's deep right under its walls.
        let walled = 1.0 - smoothstep(0.0, 500.0, c.fjord);
        let open = smoothstep(0.0, 1_000.0 - 750.0 * walled, -s);
        let ocean = smoothstep(4_200.0, 1_800.0, x);
        let sea = -3.0 - (DEEP + OCEAN * ocean) * open.powf(0.8)
            + 4.0 * open * self.detail.fbm(x / 700.0, y / 700.0, 2, 0.5);
        // Each shore's land, the nearest ones blended where they meet.
        let mut lands: [(f64, f64); 4] = [(f64::NEG_INFINITY, 0.0); 4];
        if c.hold > s - 300.0 {
            lands[0] = (c.hold, self.rh_hold(p, c.hold));
        }
        if c.main > s - 300.0 {
            lands[1] = (c.main, self.rh_mainland(p, c.main));
        }
        if c.isle > s - 300.0 {
            lands[2] = (c.isle, self.rh_isles(p, c.isle));
        }
        if c.neck > s - 300.0 {
            lands[3] = (c.neck, c.neck_top);
        }
        let (mut sum, mut weight) = (0.0, 0.0);
        for &(si, h) in &lands {
            if si > f64::NEG_INFINITY {
                let w = smoothstep(s - 300.0, s, si).powi(2) + 1e-9;
                sum += w * h;
                weight += w;
            }
        }
        let land = sum / weight;
        // Down to the water: a shingle beach, or a low cliff of rock and
        // old ice where the coast turns steep.
        let cliffy = smoothstep(
            0.0,
            0.35,
            self.coast_warp.fbm(x / 1_500.0 + 30.0, y / 1_500.0, 3, 0.5),
        )
        .max(walled)
            * (1.0 - smoothstep(-200.0, 0.0, c.neck));
        let run = 170.0 - 135.0 * cliffy;
        let w = smoothstep(-20.0, run, s);
        let mut h = sea.min(0.2) * (1.0 - w) + land * w;
        h = h.max(self.rh_bergs(p));
        if h < SEA_ICE_H {
            h = self.rh_on_sea_ice(p, &c, h);
        }
        // The heights furrowed deep, the hold's tundra lightly; not the sea, ice or necks.
        let erodes = smoothstep(140.0, 380.0, h)
            .max(0.25 * smoothstep(150.0, 600.0, c.hold) * (1.0 - smoothstep(-300.0, 0.0, c.neck)))
            * smoothstep(3.0, 8.0, h);
        Raw { h, erodes }
    }

    /// The hold: tundra rising gently off its coast, rolling, knolls of
    /// ice-smoothed rock about it, calm round the starts.
    fn rh_hold(&self, p: (f64, f64), s: f64) -> f64 {
        let (x, y) = p;
        let inland = smoothstep(0.0, 700.0, s);
        let calm = SPAWNS.iter().fold(1.0_f64, |k, sp| {
            k.min(smoothstep(450.0, 1_300.0, dist(p, sp.0)))
        });
        let roll = self.lake.fbm(x / 2_000.0, y / 2_000.0, 3, 0.5);
        let knolls = 18.0 * self.rh_knolls(p) * calm.max(0.3) * smoothstep(80.0, 500.0, s);
        let tors =
            12.0 * smoothstep(
                0.6,
                0.85,
                self.crag.ridged(x / 650.0 + 2.0, y / 650.0 - 5.0, 3, 0.5),
            ) * calm
                * smoothstep(250.0, 700.0, s);
        10.0 + 18.0 * inland
            + 10.0 * roll * inland
            + knolls
            + tors
            + 0.8 * self.detail.fbm(x / 60.0 + 9.0, y / 60.0 - 4.0, 2, 0.5)
    }

    /// The hold's knolls at a point, 0 between them to about 1 on their crowns.
    pub(super) fn rh_knolls(&self, (x, y): (f64, f64)) -> f64 {
        let k = self.ramp.fbm(x / 320.0 + 3.0, y / 320.0 - 8.0, 3, 0.5);
        smoothstep(0.05, 0.6, k).powf(1.5)
    }

    /// The mainland: a plateau rising off its coast, ranges standing on it,
    /// highest along the map's edges and in the highlands, lower in the fells,
    /// none on and round the base's floor; valleys down from the printers'
    /// rooms, basins round them, rings round the hidden ones.
    fn rh_mainland(&self, p: (f64, f64), s: f64) -> f64 {
        let (x, y) = p;
        let inland = smoothstep(0.0, 2_600.0, s);
        let hills = self.crag.ridged(x / 900.0 - 3.0, y / 900.0 + 6.0, 3, 0.5);
        let plateau = 14.0
            + 110.0 * inland
            + 25.0 * inland * self.tilt.fbm(x / 1_500.0 + 9.0, y / 1_500.0, 3, 0.5)
            + 45.0 * hills * smoothstep(400.0, 2_200.0, s);
        let edge = x.min(y).min(SIZE - x).min(SIZE - y);
        let mask = smoothstep(
            -0.3,
            0.25,
            self.mtn_mask.fbm(x / 3_000.0, y / 3_000.0, 2, 0.5),
        )
        .max(smoothstep(2_600.0, 700.0, edge));
        // The fells are lower than the highlands; the base's country low by its floor.
        let tall = 0.55 + 0.45 * smoothstep(5_000.0, 9_000.0, y);
        let off_floor = smoothstep(400.0, 2_400.0, -inside(p, &self.rh.plan.floor));
        let big = self
            .crag
            .ridged(x / 2_300.0 + 11.0, y / 2_300.0 - 4.0, 4, 0.5)
            .max(0.0);
        let small = self.crag.ridged(x / 650.0 + 3.1, y / 650.0 - 7.7, 3, 0.5);
        let ranges = mask
            * smoothstep(900.0, 3_000.0, s)
            * tall
            * off_floor
            * (180.0 + 950.0 * big.powf(1.5) + 120.0 * small * big);
        let h = plateau + ranges;
        let h = self.site_valleys(&self.rh.plan.ways, p, h, plateau.min(40.0));
        self.rh_basins(p, h)
    }

    /// The basins round the sites in the heights: the ground eased down to a
    /// bowl round the room's level; a ring of peaks kept round a hidden one.
    fn rh_basins(&self, p: (f64, f64), mut h: f64) -> f64 {
        for &(name, r, ring) in &BASINS {
            let site = SITES.iter().find(|s| s.name == name).unwrap();
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

    /// Wester Isle and the skerries: rugged peaks over a strand, a valley
    /// down the great isle's middle to its tombolo, the atoll's ring.
    fn rh_isles(&self, p: (f64, f64), s: f64) -> f64 {
        let (x, y) = p;
        let mut h = f64::NEG_INFINITY;
        let w = self.rh.plan.wester.at(p);
        if w > -400.0 {
            let crags = self.crag.ridged(x / 700.0 + 3.0, y / 700.0 - 2.0, 3, 0.5);
            let peaks = (120.0 + 280.0 * crags.max(0.0).powf(1.4)) * smoothstep(150.0, 900.0, s);
            h = 6.0 + 14.0 * smoothstep(0.0, 900.0, s) + peaks;
            h = self.site_valleys(&self.rh.plan.ways, p, h, 20.0);
        }
        for isle in &ISLES {
            let (sd, toward) = self.rh_isle(isle, p);
            if sd <= -300.0 {
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
        if h == f64::NEG_INFINITY {
            8.0
        } else {
            h
        }
    }

    /// Tabular bergs aground: sheer sides, a flat top. `f64::MIN` away from them.
    fn rh_bergs(&self, (x, y): (f64, f64)) -> f64 {
        let fray = self.coast.fbm(x / 120.0 + 31.0, y / 120.0 - 5.0, 3, 0.5);
        let top = self.detail.fbm(x / 70.0 + 13.0, y / 70.0, 2, 0.5);
        BERGS
            .iter()
            .map(|&(bx, by, r, tall)| {
                let d = dist((x, y), (bx, by)) + 0.3 * r * fray;
                let body = smoothstep(r + 8.0, r - 8.0, d);
                let foot = smoothstep(r + 50.0, r + 8.0, d);
                if foot > 0.0 {
                    -DEEP + (DEEP + UNDER_EDGE) * foot + (tall + 1.5 * top - UNDER_EDGE) * body
                } else {
                    f64::MIN
                }
            })
            .fold(f64::MIN, f64::max)
    }

    /// Metres inside the sea ice (negative outside): out from the mainland's
    /// and the isles' shores to a reach that wanders along them, none at all
    /// along many of them, none off the hold and none in the slips.
    fn rh_sea_ice(&self, p: (f64, f64), c: &Coasts) -> f64 {
        let (x, y) = p;
        let broad = self.lake.fbm(x / 1_700.0 + 50.0, y / 1_700.0, 3, 0.5);
        let edge = 70.0 * self.lake_shore.fbm(x / 260.0 - 30.0, y / 260.0, 3, 0.55)
            + 20.0 * self.coast_warp.fbm(x / 70.0 + 2.0, y / 70.0, 2, 0.5);
        let reach = SEA_ICE_REACH * smoothstep(-0.1, 0.35, broad);
        let shore = c.main.max(c.isle).min(c.fjord);
        // (None off the hold or a neck, nor in a naval site's slips.)
        let held = SITES
            .iter()
            .filter(|s| s.ground == Ground::Naval)
            .fold(c.hold.max(c.neck) + 500.0, |k, s| {
                k.max(900.0 - dist(p, s.at))
            });
        (reach + shore + edge).min(-held)
    }

    /// The sea ice laid over the water at a point of the sea `h` deep.
    fn rh_on_sea_ice(&self, p: (f64, f64), c: &Coasts, h: f64) -> f64 {
        let si = self.rh_sea_ice(p, c);
        if si < -6.0 {
            return h;
        }
        let top = SEA_ICE_H + 0.35 * self.detail.fbm(p.0 / 90.0 - 5.0, p.1 / 90.0, 2, 0.5);
        // Under its edge the floor stands only a ship's draught and a little
        // deep: the edge shows a metre or two of ice over the water.
        let floor = h.max(UNDER_EDGE - 1.2 * (-si - 4.0).max(0.0));
        floor + (top - floor) * smoothstep(-3.5, 3.5, si)
    }

    /// How much of a point of height `h` is ice (sea ice, a berg), 0..1.
    pub(super) fn rh_ice(&self, p: (f64, f64), h: f64) -> f64 {
        if h > 40.0 {
            return 0.0;
        }
        let berg = smoothstep(-2.0, 2.0, self.rh_bergs(p));
        let c = self.rh_coasts(p);
        let sea = smoothstep(-2.0, 2.0, self.rh_sea_ice(p, &c))
            * (1.0 - smoothstep(2.5, 4.5, h))
            * (1.0 - smoothstep(-40.0, 10.0, c.land()));
        berg.max(sea)
    }

    /// What the plan cuts into the ground: every way, every room, the sea
    /// gates' basins, the base's floor.
    fn rh_cut(&self, p: (f64, f64), mut h: f64) -> f64 {
        let plan = &self.rh.plan;
        // The rooms first: a way leaving or entering one is held at its level
        // through it, and cuts on through the room's walls.
        for s in SITES {
            h = self.site_room(s, p, h);
        }
        let mut lowland = None;
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
            // On the hold and the necks a way is a narrow strip, its banks
            // long and low: the open ground round it is its road.
            let lowland = *lowland.get_or_insert_with(|| {
                let c = self.rh_coasts(p);
                smoothstep(-200.0, 300.0, c.hold.max(c.neck + 400.0))
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
        // The sea gates' basins: square cut, a level floor well under a keel.
        for &g in &SEA_GATES {
            let (lx, ly) = gate_frame(g, p);
            let (b0, b1, bh) = BASIN;
            let out = (b0 - lx).max(lx - b1).max(ly.abs() - bh);
            if out < 60.0 {
                let floor = -24.0 + 0.3 * self.detail.fbm(p.0 / 300.0, p.1 / 300.0, 2, 0.5);
                h += (floor - h) * (1.0 - smoothstep(0.0, 60.0, out));
            }
        }
        let b = self.rh_on_floor(p);
        if b > -FLOOR_BLEND {
            h += (BASE_H - h) * smoothstep(-FLOOR_BLEND, 0.0, b);
        }
        h
    }

    /// Metres inside the base's floor (negative outside), its edge frayed so
    /// the floor gives way to the country in lobes, not along a ruled line.
    fn rh_on_floor(&self, p: (f64, f64)) -> f64 {
        let plan = &self.rh.plan;
        if plan.floor.is_empty() {
            return f64::NEG_INFINITY;
        }
        let frayed = inside(p, &plan.floor)
            + 260.0 * self.mtn_gap.fbm(p.0 / 1_100.0 + 9.0, p.1 / 1_100.0, 3, 0.5);
        // (Always well round the machine's bastions, whose benches are cut at
        // the floor's level: no notch in the hills behind.)
        FACILITY
            .bastions()
            .iter()
            .fold(frayed, |b, &(at, _, _)| b.max(700.0 - dist(p, at)))
    }
}

/// A point in a sea gate's frame: x out to sea along its heading, y to its left.
pub(super) fn gate_frame(g: (f64, f64), p: (f64, f64)) -> (f64, f64) {
    let (s, c) = SEA_GATE_HEADING.sin_cos();
    let (dx, dy) = (p.0 - g.0, p.1 - g.1);
    (dx * c + dy * s, dy * c - dx * s)
}
