//! Vela's land: the north coast and its beach, the harbour, the river and its
//! valley, the ridge before the bridge, the rise over it, the spaceport's
//! plateau, the quarry, the islands out to sea.

use super::super::siege::grid::{dist, dist_to_segment, y_at, P};
use super::super::Terrain;
use super::*;
use crate::noise::{bump, smoothstep};

/// The north shore: its y at each x, the sea north of it.
const NORTH: &[P] = &[
    (0.0, 6_420.0),
    (700.0, 6_330.0),
    (1_150.0, 6_230.0),
    (1_500.0, 6_250.0),
    (1_900.0, 6_310.0),
    (2_500.0, 6_260.0),
    (3_050.0, 6_420.0),
    (3_380.0, 6_560.0),
    (3_700.0, 6_430.0),
    (4_300.0, 6_460.0),
    (4_900.0, 6_620.0),
    (5_450.0, 6_620.0),
    (6_050.0, 6_650.0),
    (6_500.0, 6_880.0),
    (7_300.0, 6_870.0),
    (8_192.0, 7_060.0),
];

/// The west shore: its x at each y (pairs are (y, x)), the sea west of it. The
/// south-west runs out to the map's edge.
const WEST: &[P] = &[
    (0.0, -400.0),
    (1_400.0, -300.0),
    (2_000.0, 420.0),
    (3_500.0, 640.0),
    (4_800.0, 760.0),
    (5_600.0, 920.0),
    (6_100.0, 1_050.0),
    (8_192.0, 1_400.0),
];

/// The beach where the Company lands: the stretch of the north shore that
/// shelves gently into the sea.
const BEACH: (f64, f64) = (900.0, 1_650.0);

/// The river's course, from the hills in the south to its mouth.
pub(super) const RIVER: &[P] = &[
    (3_650.0, -50.0),
    (3_560.0, 1_500.0),
    (3_700.0, 2_700.0),
    (3_460.0, 4_000.0),
    (3_560.0, 5_000.0),
    (BRIDGE_X, 5_480.0),
    (BRIDGE_X, ROAD_Y + 140.0),
    (3_400.0, 6_200.0),
    (3_380.0, 6_640.0),
];
/// The river's bed and its banks, metres from its middle.
const RIVER_BED: f64 = 24.0;
const RIVER_BANK: f64 = 34.0;

/// The ridge the coast road crosses before the bridge: its crest line.
const RIDGE: (P, P) = ((2_930.0, 4_300.0), (3_040.0, 6_150.0));

/// The rise over the bridge where the guns are dug in.
pub(super) const RISE: P = (3_790.0, 5_300.0);

/// The town's ground: a slope from the quay up to the south.
pub(super) const TOWN: (P, f64, f64) = ((5_700.0, 5_960.0), 820.0, 600.0);

/// The harbour's basin, cut square into the quay.
pub(super) const HARBOUR: (P, P) = ((5_480.0, 6_430.0), (5_980.0, 6_760.0));

/// The spaceport's plateau: its level top and how high.
pub(super) const PLATEAU: (P, f64, f64) = ((6_960.0, 4_650.0), 640.0, 480.0);
pub(super) const PLATEAU_Z: f64 = 46.0;

/// The quarry: its middle, its radius at the rim, its floor's height.
pub(super) const QUARRY: (P, f64) = ((7_320.0, 3_330.0), 440.0);
pub(super) const QUARRY_FLOOR: f64 = 6.0;
/// The ramp down into it: from the rim to the floor.
pub(super) const QUARRY_RAMP: (P, P) = ((7_320.0, 3_800.0), (7_320.0, 3_440.0));

/// The islands: middle, radius, height.
pub(super) const ISLANDS: [(P, f64, f64); 5] = [
    ((620.0, 7_500.0), 300.0, 12.0),
    ((2_700.0, 7_380.0), 430.0, 16.0),
    ((4_650.0, 7_620.0), 220.0, 22.0),
    ((7_650.0, 7_720.0), 360.0, 14.0),
    ((350.0, 2_600.0), 180.0, 9.0),
];

impl Terrain {
    /// How far inland `p` is from the sea, metres (negative out at sea).
    pub(super) fn vl_inland(&self, p: P) -> f64 {
        let north = y_at(NORTH, p.0) - p.1;
        let west = p.0 - y_at(WEST, p.1);
        let d = north.min(west);
        // The shore wanders, but not along the beach or the quay.
        let calm = beachness(p).max(quayness(p));
        d + 35.0 * self.coast.fbm(p.0 / 380.0, p.1 / 380.0, 3, 0.5) * (1.0 - calm)
    }

    /// The land before the plan is cut in.
    pub(super) fn vela_land(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let d = self.vl_inland(p);
        // The country: low and rolling by the coast, rising into hills in the
        // south.
        let roll = self.cont.fbm(x / 1_100.0, y / 1_100.0, 4, 0.5);
        let hills = smoothstep(5_400.0, 2_200.0, y)
            * (24.0 + 26.0 * self.ridge.fbm(x / 1_600.0, y / 1_600.0, 3, 0.5).abs());
        let mut h = 12.0 + 7.0 * roll + hills;
        h += 1.6 * self.detail.fbm(x / 140.0, y / 140.0, 2, 0.5);
        // The town on its slope up from the quay.
        let ((tx, ty), rx, ry) = TOWN;
        let town = 1.0 - smoothstep(0.75, 1.15, ((x - tx) / rx).hypot((y - ty) / ry));
        let town_z = 8.5 + (6_500.0 - y).max(0.0) * 0.006;
        h += (town_z - h) * town;
        // The ridge, low where the road goes over it.
        let r = dist_to_segment(p, RIDGE.0, RIDGE.1);
        let saddle = bump((y - ROAD_Y).abs() / 260.0);
        h += 30.0
            * bump(r / 300.0)
            * (1.0 - 0.6 * saddle)
            * (0.85 + 0.3 * self.crag.get(x / 400.0, y / 400.0));
        // The rise the guns are dug into.
        h += 14.0 * bump(dist(p, RISE) / 260.0);
        // The spaceport's plateau.
        let ((px, py), hx, hy) = PLATEAU;
        let out = ((x - px).abs() - hx)
            .max(0.0)
            .hypot(((y - py).abs() - hy).max(0.0));
        h += (PLATEAU_Z - h) * (1.0 - smoothstep(0.0, 460.0, out));
        // The coast: a low cliff, or the beach shelving into the sea, or the
        // quay's sheer wall.
        let beach = beachness(p);
        let quay = quayness(p);
        let cliff = 2.0 + 10.0 * smoothstep(0.0, 40.0, d);
        let sand = 0.6 + 0.022 * d;
        let edge = cliff + (sand - cliff) * beach;
        let edge = edge + (town_z.min(9.5) - edge) * quay;
        let reach = 160.0 + 260.0 * beach;
        h = edge + (h - edge) * smoothstep(0.0, reach, d);
        if d < 0.0 {
            // The sea: shallow off the beach, deeper off the cliffs.
            let shelf = 0.02 + 0.03 * (1.0 - beach);
            h = (-1.5 - shelf * -d).max(-48.0);
            h = h.max(self.vl_islands(p));
        }
        h = self.vl_river(p, h);
        self.vl_quarry(p, h)
    }

    /// The islands' ground out at sea, or the sea floor.
    fn vl_islands(&self, p: P) -> f64 {
        let mut h = f64::MIN;
        for &(c, r, top) in &ISLANDS {
            let wob = 1.0 + 0.18 * self.coast.fbm(p.0 / 160.0 + c.0, p.1 / 160.0, 3, 0.5);
            let t = dist(p, c) / (r * wob);
            let rise = top * (1.0 - t * t).max(-1.0) + 1.2;
            h = h.max(rise);
        }
        h
    }

    /// The river's channel and valley cut into the land.
    fn vl_river(&self, p: P, h: f64) -> f64 {
        let d = river_distance(p);
        if d > 600.0 {
            return h;
        }
        let bed = if d < RIVER_BED {
            -3.2
        } else if d < RIVER_BANK {
            -3.2 + 4.7 * smoothstep(RIVER_BED, RIVER_BANK, d)
        } else {
            1.5 + (d - RIVER_BANK) * 0.09
        };
        let mut h = h.min(bed);
        // The bridge's abutments: the road held level with the deck's ends.
        for end in [BRIDGE_X - BRIDGE_HALF_SPAN, BRIDGE_X + BRIDGE_HALF_SPAN] {
            let k = 1.0 - smoothstep(10.0, 70.0, dist(p, (end, ROAD_Y)));
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
        // Five benches between the floor and the rim, each a flat step and a
        // steep face.
        let q = ((t - 0.28) / 0.72).clamp(0.0, 1.0) * 5.0;
        let stepped = (q.floor() + smoothstep(0.82, 1.0, q.fract())) / 5.0;
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

/// Distance from `p` to the river's middle.
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
