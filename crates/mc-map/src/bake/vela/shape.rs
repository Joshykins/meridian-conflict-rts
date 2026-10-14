//! Vela's land: one island in a warm sea, its beaches and the shallows round
//! it, the creek and the bridge over it, the ridge before the bridge, the rise
//! over it, the town on its slope above the harbour, the spaceport's plateau,
//! the quarry; an atoll and a few cays out at sea.

use super::super::siege::grid::{dist, dist_to_segment, P};
use super::super::Terrain;
use super::*;
use crate::noise::{bump, smoothstep};

/// The island's coast, counter-clockwise from the north-west beach.
const COAST: &[P] = &[
    (1_080.0, 6_200.0),
    (1_500.0, 6_250.0),
    (2_500.0, 6_260.0),
    (3_050.0, 6_420.0),
    (3_380.0, 6_560.0),
    (3_700.0, 6_430.0),
    (4_300.0, 6_460.0),
    (4_900.0, 6_620.0),
    (5_450.0, 6_620.0),
    (6_050.0, 6_650.0),
    (6_500.0, 6_820.0),
    (7_100.0, 6_760.0),
    (7_480.0, 6_350.0),
    (7_560.0, 5_600.0),
    (7_520.0, 4_500.0),
    (7_620.0, 3_500.0),
    (7_400.0, 2_500.0),
    (6_900.0, 1_800.0),
    (5_900.0, 1_350.0),
    (4_700.0, 1_200.0),
    (3_600.0, 1_300.0),
    (2_500.0, 1_550.0),
    (1_700.0, 1_950.0),
    (1_150.0, 2_650.0),
    (850.0, 3_650.0),
    (800.0, 4_800.0),
    (880.0, 5_650.0),
    (880.0, 5_920.0),
    (950.0, 6_110.0),
];

/// The coast rounded into a curve through its points: no corner on a beach.
static SHORE: std::sync::LazyLock<Vec<P>> = std::sync::LazyLock::new(|| {
    let mut ring: Vec<P> = COAST.to_vec();
    ring.extend_from_slice(&COAST[..3]);
    let curve = super::super::canyon::smooth_open(&ring, 6);
    // From the first point's piece round to the same point again.
    curve[6..6 + 6 * COAST.len()].to_vec()
});

/// The beach where the Company lands: the stretch of the north shore that
/// shelves gently into the sea.
const BEACH: (f64, f64) = (900.0, 1_650.0);

/// The creek's course, from its lagoon among the hills to its mouth.
pub(super) const RIVER: &[P] = &[
    (3_520.0, 3_150.0),
    (3_430.0, 3_480.0),
    (3_540.0, 3_820.0),
    (3_430.0, 4_160.0),
    (3_500.0, 4_500.0),
    (3_620.0, 4_820.0),
    (3_520.0, 5_150.0),
    (BRIDGE_X, 5_480.0),
    (BRIDGE_X, ROAD_Y + 140.0),
    (3_450.0, 5_980.0),
    (3_350.0, 6_250.0),
    (3_410.0, 6_450.0),
    (3_380.0, 6_640.0),
];
/// The lagoon at its head.
const LAGOON: (P, f64) = ((3_520.0, 3_050.0), 210.0);
/// The creek's bed and its banks, metres from its middle.
const RIVER_BED: f64 = 24.0;
const RIVER_BANK: f64 = 34.0;

/// The ridge the coast road crosses before the bridge: its crest line.
const RIDGE: (P, P) = ((2_930.0, 4_300.0), (3_040.0, 6_100.0));

/// The rise over the bridge where the guns are dug in.
pub(super) const RISE: P = (3_790.0, 5_300.0);

/// The town's ground: a slope from the quay up to the south.
pub(super) const TOWN: (P, f64, f64) = ((5_700.0, 5_960.0), 820.0, 600.0);

/// The harbour's basin, cut square into the quay.
pub(super) const HARBOUR: (P, P) = ((5_480.0, 6_430.0), (5_980.0, 6_760.0));

/// The spaceport's plateau: its level top and how high.
pub(super) const PLATEAU: (P, f64, f64) = ((6_710.0, 4_650.0), 640.0, 480.0);
pub(super) const PLATEAU_Z: f64 = 30.0;

/// The quarry: its middle, its radius at the rim, its floor's height.
pub(super) const QUARRY: (P, f64) = ((7_000.0, 3_250.0), 420.0);
pub(super) const QUARRY_FLOOR: f64 = 4.0;
/// The ramp down into it: from the rim to the floor.
pub(super) const QUARRY_RAMP: (P, P) = ((7_000.0, 3_700.0), (7_000.0, 3_380.0));

/// The island's mountains: each massif's middle, reach and peak. Their
/// outlines are warped and their faces ridged and eroded, and the design's
/// roads keep valleys through them.
const MASSIFS: [(P, f64, f64); 7] = [
    ((2_050.0, 2_750.0), 1_150.0, 290.0),
    ((3_950.0, 1_950.0), 1_300.0, 380.0),
    ((5_450.0, 2_500.0), 1_050.0, 330.0),
    ((6_650.0, 1_800.0), 850.0, 240.0),
    ((1_350.0, 4_300.0), 700.0, 160.0),
    ((4_750.0, 4_150.0), 600.0, 110.0),
    ((7_150.0, 5_700.0), 450.0, 90.0),
];

/// The cays: middle, radius, height.
pub(super) const ISLANDS: [(P, f64, f64); 3] = [
    ((2_500.0, 7_380.0), 380.0, 7.0),
    ((6_750.0, 7_520.0), 230.0, 5.0),
    ((440.0, 3_300.0), 160.0, 4.0),
];

/// The atoll out to the north: its middle, the ring's radius and width.
pub(super) const ATOLL: (P, f64, f64) = ((4_600.0, 7_560.0), 420.0, 70.0);

impl Terrain {
    /// How far inland `p` is from the sea, metres (negative out at sea).
    pub(super) fn vl_inland(&self, p: P) -> f64 {
        let d = signed_distance(p, &SHORE);
        // The shore wanders, a little along the beach, not at all at the quay.
        let calm = (0.6 * beachness(p)).max(quayness(p));
        d + 45.0 * self.coast.fbm(p.0 / 420.0, p.1 / 420.0, 3, 0.5) * (1.0 - calm)
    }

    /// The land before the plan is cut in.
    pub(super) fn vela_land(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let d = self.vl_inland(p);
        if d < 0.0 {
            let h = self.vl_sea(p, -d);
            return self.vl_river(p, h);
        }
        // White sand up from the sea, then low country, the hills inland.
        let beach = beachness(p);
        let run = 150.0 + 150.0 * beach;
        let sand = 3.0 * smoothstep(0.0, run, d).powf(0.8);
        let inland = smoothstep(run * 0.8, run + 420.0, d);
        // The low country above the sand: green from about 12 m, rolling.
        let roll = self.cont.fbm(x / 1_100.0, y / 1_100.0, 4, 0.5);
        let swale = self.mtn_height.fbm(x / 420.0, y / 420.0, 3, 0.5);
        let mut h = sand + inland * (13.0 + 6.0 * roll + 3.5 * swale);
        h += 1.2 * self.detail.fbm(x / 140.0, y / 140.0, 2, 0.5) * inland;
        h += self.vl_mountains(p, d);
        // The town on its slope up from the quay.
        let ((tx, ty), rx, ry) = TOWN;
        let town = 1.0 - smoothstep(0.75, 1.15, ((x - tx) / rx).hypot((y - ty) / ry));
        let town_z = 5.0 + (6_500.0 - y).max(0.0) * 0.016;
        h += (town_z - h) * town;
        // The quay's sheer wall at the harbour.
        h += (town_z.min(4.0) - h) * quayness(p) * (1.0 - smoothstep(0.0, 120.0, d));
        // The ridge, low where the road goes over it.
        let r = dist_to_segment(p, RIDGE.0, RIDGE.1);
        let saddle = bump((y - ROAD_Y).abs() / 260.0);
        h += 24.0
            * bump(r / 300.0)
            * (1.0 - 0.6 * saddle)
            * (0.85 + 0.3 * self.crag.get(x / 400.0, y / 400.0))
            * smoothstep(0.0, 200.0, d);
        // The rise the guns are dug into.
        h += 12.0 * bump(dist(p, RISE) / 260.0);
        // The spaceport's plateau.
        // The spaceport's plateau: a rounded table, its edge wandering.
        let ((px, py), hx, hy) = PLATEAU;
        let e = (((x - px) / hx).powi(4) + ((y - py) / hy).powi(4)).powf(0.25);
        let out = (e - 1.0) * hx.min(hy) + 70.0 * self.crag.fbm(x / 500.0 + 8.0, y / 500.0, 3, 0.5);
        h += (PLATEAU_Z - h) * (1.0 - smoothstep(0.0, 380.0, out));
        let h = self.vl_river(p, h);
        self.vl_quarry(p, h)
    }

    /// How much the mountains raise `p`, `d` metres inland: the massifs'
    /// warped cones, ridged and eroded, kept off the coast, the design's roads,
    /// the plateau, the quarry and the town.
    pub(super) fn vl_mountains(&self, p: P, d: f64) -> f64 {
        let (x, y) = p;
        let (wx, wy) = (
            self.warp_x.get(x / 1_800.0, y / 1_800.0) * 520.0,
            self.warp_y.get(x / 1_800.0, y / 1_800.0) * 520.0,
        );
        let q = (x + wx, y + wy);
        let mut mass = 0.0_f64;
        for (c, r, peak) in MASSIFS {
            let t = (dist(q, c) / r).min(1.0);
            mass = mass.max(peak * (1.0 - t).powf(1.6));
        }
        if mass <= 0.0 {
            return 0.0;
        }
        let ridges = self.mtn.ridged(q.0 / 1_100.0, q.1 / 1_100.0, 5, 0.5);
        let crags = self.crag.fbm(q.0 / 300.0, q.1 / 300.0, 3, 0.5);
        // Foothills everywhere inland, the massifs over them.
        let foot = 28.0
            * self
                .ridge
                .ridged(q.0 / 700.0 + 4.0, q.1 / 700.0 - 9.0, 4, 0.5)
                .powf(1.5);
        let mut m = mass * (0.3 + 0.8 * ridges) + foot + 8.0 * crags * smoothstep(20.0, 80.0, mass);
        m +=
            0.5 * self.erosion.at(x + ORIGIN, y + ORIGIN).max(-40.0) * smoothstep(10.0, 60.0, mass);
        let keep = smoothstep(120.0, 1_000.0, d)
            * smoothstep(130.0, 950.0, super::plan::design_road_distance(p)).powf(0.8)
            * smoothstep(QUARRY.1 + 150.0, QUARRY.1 + 650.0, dist(p, QUARRY.0))
            * smoothstep(250.0, 1_000.0, dist(p, PLATEAU.0))
            * smoothstep(250.0, 650.0, dist(p, TOWN.0) - TOWN.1)
            * smoothstep(
                100.0,
                900.0,
                river_distance(p).min(dist(p, LAGOON.0) - LAGOON.1),
            )
            .powf(0.8);
        m.max(0.0) * keep
    }

    /// The sea floor `d` metres out: white sand shelving off the beach, the
    /// shallow bank out to the reef's edge, then the deep; the cays and the
    /// atoll on it; deep water off every edge of the map.
    fn vl_sea(&self, p: P, d: f64) -> f64 {
        let wide = smoothstep(
            -0.2,
            0.45,
            self.lake
                .fbm(p.0 / 1_300.0 + 3.0, p.1 / 1_300.0 - 8.0, 3, 0.5),
        );
        let bank = 160.0 + 520.0 * wide;
        let ripple = 0.5 * self.detail.fbm(p.0 / 60.0, p.1 / 60.0, 2, 0.5);
        // From the waterline down: no lip at the shore.
        let flat =
            -0.15 - 3.0 * smoothstep(0.0, bank, d).powf(0.6) + ripple * smoothstep(0.0, 40.0, d);
        // Past the reef's edge the floor drops away to the ocean's deep blue.
        let deep = -(70.0 + 30.0 * self.tilt.fbm(p.0 / 2_400.0, p.1 / 2_400.0, 3, 0.5).abs())
            - 0.02 * (d - bank).max(0.0);
        let mut h = (flat + (deep - flat) * smoothstep(bank, bank + 260.0, d)).min(-0.4);
        // The harbour's approach dredged deep.
        let ((hl, hl_y), (hh, _)) = HARBOUR;
        if p.0 > hl - 60.0 && p.0 < hh + 60.0 && p.1 > hl_y {
            h = h.min(-8.0);
        }
        // The cays.
        for &(c, r, top) in &ISLANDS {
            let wob = 1.0 + 0.2 * self.coast.fbm(p.0 / 160.0 + c.0, p.1 / 160.0, 3, 0.5);
            let t = dist(p, c) / (r * wob);
            // The cay, and the sand shoaling off it out to twice its reach.
            let rise = if t < 1.0 {
                0.6 + top * (1.0 - t * t)
            } else if t < 2.2 {
                // down the shoal, then off its edge into the deep
                -0.3 - 1.8 * smoothstep(1.0, 1.6, t) - 140.0 * smoothstep(1.6, 2.2, t)
            } else {
                f64::MIN
            };
            h = h.max(rise);
        }
        // The atoll: a ring of reef with sand cays on it round a shallow lagoon.
        let (c, ring, width) = ATOLL;
        let r = dist(p, c);
        let wob = 40.0 * self.coast.fbm(p.0 / 300.0 - 9.0, p.1 / 300.0 + 4.0, 3, 0.5);
        let off = (r - ring + wob).abs();
        if r < ring + width + 400.0 {
            let lagoon = -4.5 + 1.2 * self.detail.fbm(p.0 / 90.0, p.1 / 90.0, 2, 0.5);
            let reef = -0.9
                - 2.5 * smoothstep(width, width + 200.0, off)
                - 140.0 * smoothstep(width + 200.0, width + 400.0, off);
            let cay = smoothstep(
                0.32,
                0.48,
                self.crag.fbm(p.0 / 200.0 + 17.0, p.1 / 200.0 - 3.0, 3, 0.5),
            ) * (1.0 - smoothstep(0.0, width, off));
            let mut a = reef + (2.4 - reef) * cay;
            if r < ring - width {
                a = a.max(lagoon);
            }
            h = h.max(a);
        }
        // Deep water off every edge.
        let (wx, wy) = (p.0 + ORIGIN, p.1 + ORIGIN);
        let e = wx.min(wy).min(MAP_SIZE - wx).min(MAP_SIZE - wy);
        h + (-110.0 - h) * (1.0 - smoothstep(150.0, 900.0, e))
    }

    /// The creek's channel and its valley cut into the land, its lagoon, and
    /// the bridge's abutments.
    fn vl_river(&self, p: P, h: f64) -> f64 {
        // The lagoon's shore wanders; the creek winds a little within its line.
        let (lc, lr) = LAGOON;
        let ragged = lr
            * (1.0
                + 0.3
                    * self
                        .coast
                        .fbm(p.0 / 160.0 - 21.0, p.1 / 160.0 + 5.0, 3, 0.5));
        let wind = 10.0 * self.detail.fbm(p.0 / 220.0 + 3.0, p.1 / 220.0, 2, 0.5);
        let d = (river_distance(p) + wind).min(dist(p, lc) - ragged + RIVER_BED);
        // Past here the valley's sides stand over any land: nothing to cut.
        if d > 1_200.0 {
            return h;
        }
        let bed = if d < RIVER_BED {
            -3.2
        } else if d < RIVER_BANK {
            -3.2 + 4.2 * smoothstep(RIVER_BED, RIVER_BANK, d)
        } else {
            // Up out of the sand quickly: green banks, not beaches, inland.
            // and the valley's sides steepening away from the water, so the
            // land meets them on a slope, never at a wall.
            1.0 + 13.0 * smoothstep(RIVER_BANK, RIVER_BANK + 110.0, d)
                + (d - RIVER_BANK) * 0.05
                + 0.0016 * (d - 250.0).max(0.0).powi(2)
        };
        let mut h = h.min(bed);
        // The bridge's abutments: the road held level with the deck's ends.
        for end in [BRIDGE_X - BRIDGE_HALF_SPAN, BRIDGE_X + BRIDGE_HALF_SPAN] {
            let k = 1.0 - smoothstep(10.0, 150.0, dist(p, (end, ROAD_Y)));
            let out = if end < BRIDGE_X {
                p.0 < end + 4.0
            } else {
                p.0 > end - 4.0
            };
            if out {
                h += (DECK_TOP - h) * k;
            }
        }
        h
    }

    /// The quarry's benches and floor, and its ramp.
    fn vl_quarry(&self, p: P, h: f64) -> f64 {
        let (c, r) = QUARRY;
        let t = dist(p, c) / r;
        if t > 1.15 {
            return h;
        }
        let rim = h;
        // Four benches between the floor and the rim, each a flat step and a
        // steep face.
        let q = ((t - 0.3) / 0.7).clamp(0.0, 1.0) * 4.0;
        let stepped = (q.floor() + smoothstep(0.8, 1.0, q.fract())) / 4.0;
        let pit = QUARRY_FLOOR + (rim - QUARRY_FLOOR) * stepped;
        let mut h = if t < 1.0 { pit.min(h) } else { h };
        // The ramp: a graded way down from the rim to the floor.
        let (a, b) = QUARRY_RAMP;
        let along = ((p.1 - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
        let across = (p.0 - a.0).abs();
        if p.1 <= a.1 + 20.0 && p.1 >= b.1 - 20.0 {
            let grade = rim + (QUARRY_FLOOR - rim) * along;
            let k = 1.0 - smoothstep(7.0, 22.0, across);
            h += (grade - h) * k;
        }
        h
    }
}

/// How far `p` is along the beach's stretch of shore: 1 on the beach.
pub(super) fn beachness(p: P) -> f64 {
    let (a, b) = BEACH;
    smoothstep(a - 200.0, a, p.0)
        * (1.0 - smoothstep(b, b + 200.0, p.0))
        * (1.0 - smoothstep(5_700.0, 5_400.0, p.1))
}

/// How far `p` is along the town's quay: 1 on it.
fn quayness(p: P) -> f64 {
    smoothstep(5_000.0, 5_200.0, p.0)
        * (1.0 - smoothstep(6_250.0, 6_450.0, p.0))
        * smoothstep(6_000.0, 6_300.0, p.1)
}

/// Distance from `p` to the creek's middle.
pub(super) fn river_distance(p: P) -> f64 {
    RIVER
        .windows(2)
        .map(|w| dist_to_segment(p, w[0], w[1]))
        .fold(f64::MAX, f64::min)
}

/// Whether `p` is in the harbour's basin.
pub(super) fn in_harbour(p: P) -> bool {
    let (lo, hi) = HARBOUR;
    p.0 > lo.0 && p.0 < hi.0 && p.1 > lo.1 && p.1 < hi.1
}

/// Signed distance from `p` to a closed outline: positive inside.
fn signed_distance(p: P, poly: &[P]) -> f64 {
    let n = poly.len();
    let mut d = f64::MAX;
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        d = d.min(dist_to_segment(p, a, b));
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    if inside {
        d
    } else {
        -d
    }
}
