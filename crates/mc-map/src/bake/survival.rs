//! [`Layout::Survival`]: "The Crucible", a designed, deliberately asymmetric
//! map for survival mode. The Replication Engine stands on a raised plateau
//! in the north-east with a harbour cut into the cliff beside it; the
//! defenders hold a lowland pocket in the south-west. Between them:
//!
//! * three land corridors, walled apart by mountains: the Coast Road along the
//!   shore, the Kiln Road straight down the middle, and the Northern Reach, the
//!   long way round by the north and west;
//! * a sea along the whole south-east, with a chain of islands (the Teeth)
//!   offshore, so ships come either by the inner channel under the coast or
//!   by the open sea outside the islands;
//! * three defender starts: a coastal one on the open south shore, a mesa the
//!   ships barely reach, and a river valley behind a river crossed at two fords.
//!
//! Everything is laid out in metres for the 14.336 km map (7 x 7 tiles),
//! sized so land rounds arrive in a few minutes: land fronts run 8-10 km from
//! their first point to a spawn, the sea lanes 12-15 km. Another map size
//! scales the design (mostly for tests). The sidecar `maps/crucible.ron`
//! carries the same coordinates for the survival rules.

use super::{Layout, Pad, Terrain, Town};
use crate::noise::{bump, smoothstep};
use crate::BUILD_CELL_M;
use std::f64::consts::{PI, SQRT_2};

/// The size the design coordinates below are written for.
const DESIGN_M: f64 = 14_336.0;

/// Middle of the Replication Engine; the last start position.
pub(crate) const ENGINE: (f64, f64) = (10_704.0, 10_704.0);
/// Defender starts: coastal, highland (the mesa), river valley.
pub(crate) const SPAWNS: [(f64, f64); 3] =
    [(2_808.0, 1_896.0), (2_100.0, 4_500.0), (4_140.0, 3_852.0)];
/// Middle of the harbour basin beside the engine, 1.15 km off.
pub(crate) const HARBOR: (f64, f64) = (11_584.0, 9_963.0);
/// Where the harbour's channel opens into the sea (south-east, straight
/// down the coast).
const HARBOR_MOUTH: (f64, f64) = (12_600.0, 9_100.0);
const HARBOR_R: f64 = 380.0;

/// The mainland coast, sea to the right of the direction of travel (south
/// and east). The ends run off the map; `SEA_CLOSE` closes the polygon well outside it.
const COAST: [(f64, f64); 11] = [
    (12_180.0, 15_750.0),
    (12_110.0, 11_550.0),
    (12_320.0, 9_940.0),
    (11_620.0, 8_400.0),
    (10_220.0, 6_720.0),
    (8_610.0, 5_180.0),
    (7_000.0, 3_710.0),
    (5_460.0, 2_310.0),
    (3_920.0, 1_330.0),
    (2_100.0, 945.0),
    (-1_750.0, 945.0),
];
const SEA_CLOSE: [(f64, f64); 3] = [
    (-1_750.0, -4_200.0),
    (18_900.0, -4_200.0),
    (18_900.0, 15_750.0),
];

/// The Teeth: centre and radius of each island.
const ISLANDS: [(f64, f64, f64); 6] = [
    (12_705.0, 7_525.0, 336.0),
    (11_515.0, 6_125.0, 434.0),
    (10_220.0, 4_830.0, 294.0),
    (8_820.0, 3_500.0, 406.0),
    (7_665.0, 2_520.0, 252.0),
    (13_510.0, 3_780.0, 490.0),
];

/// Land corridors, engine end first, running on into the defenders' pocket.
const CORRIDORS: [&[(f64, f64)]; 3] = [
    // The Coast Road.
    &[
        (10_500.0, 9_950.0),
        (10_850.0, 8_820.0),
        (9_590.0, 7_280.0),
        (7_980.0, 5_740.0),
        (6_440.0, 4_340.0),
        (5_320.0, 3_220.0),
        (4_200.0, 2_660.0),
    ],
    // The Kiln Road.
    &[
        (10_150.0, 10_150.0),
        (8_820.0, 8_750.0),
        (7_140.0, 7_000.0),
        (5_740.0, 5_460.0),
        (5_040.0, 4_830.0),
        (4_410.0, 4_270.0),
    ],
    // The Northern Reach.
    &[
        (9_800.0, 11_100.0),
        (7_600.0, 10_600.0),
        (5_500.0, 9_300.0),
        (4_000.0, 7_700.0),
        (3_200.0, 6_300.0),
        (2_700.0, 5_300.0),
    ],
];
/// Half the width of a corridor's floor (it narrows and widens by up to
/// `CORRIDOR_REACH`, and wobbles by `CORRIDOR_WOBBLE`, never below 300 m),
/// and how far its walls take to rise.
const CORRIDOR_HALF: f64 = 410.0;
const CORRIDOR_REACH: f64 = 60.0;
const CORRIDOR_WOBBLE: f64 = 45.0;
const CORRIDOR_WALL: f64 = 230.0;

/// Land node sites; each gets a small level pad and a clearing around it.
pub(crate) const LAND_SITES: [(f64, f64); 9] = [
    (9_590.0, 7_280.0),
    (8_785.0, 6_510.0),
    (6_902.0, 4_760.0),
    (8_820.0, 8_750.0),
    (6_580.0, 6_384.0),
    (8_700.0, 10_850.0),
    (6_550.0, 9_950.0),
    (4_750.0, 8_500.0),
    (3_600.0, 7_000.0),
];
/// The ruined town beside the Kiln Road, off to one side of the road.
const TOWN: (f64, f64, f64) = (7_605.0, 8_235.0, 230.0);

/// The river in front of the valley start: source (at the foot of the ridge
/// between the Coast and Kiln roads) to the sea, and its two fords.
const RIVER: [(f64, f64); 5] = [
    (6_090.0, 4_900.0),
    (5_250.0, 3_850.0),
    (4_935.0, 3_010.0),
    (4_830.0, 2_170.0),
    (4_655.0, 1_330.0),
];
const FORDS: [(f64, f64); 2] = [(4_935.0, 3_024.0), (4_816.0, 2_100.0)];

/// The highland start stands on a mesa, cliffs all round but for three ramps.
const MESA_R: f64 = 850.0;
const MESA_RISE: f64 = 42.0;
/// Ramp directions off the mesa: east (the valley), south (the coast),
/// north-east (down to the Northern Reach).
const MESA_RAMPS: [f64; 3] = [-0.31, -1.31, 1.02];

/// Ore: small fields around each start, a few contested ones out on the roads.
const CONTESTED_ORE: [(f64, f64); 4] = [
    (6_160.0, 6_090.0),
    (5_500.0, 9_300.0),
    (5_950.0, 3_850.0),
    (TOWN.0, TOWN.1),
];

/// Plateau height at the engine.
const PLATEAU_M: f64 = 72.0;

fn seg_dist((px, py): (f64, f64), (ax, ay): (f64, f64), (bx, by): (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (qx, qy) = (ax + t * dx - px, ay + t * dy - py);
    ((qx * qx + qy * qy).sqrt(), t)
}

/// Distance to a polyline given in design metres and scaled by `f`, and the
/// distance along it to the nearest point.
fn polyline_dist(p: (f64, f64), pts: &[(f64, f64)], f: f64) -> (f64, f64) {
    let (mut best, mut along, mut run) = (f64::INFINITY, 0.0, 0.0);
    for w in pts.windows(2) {
        let (a, b) = ((w[0].0 * f, w[0].1 * f), (w[1].0 * f, w[1].1 * f));
        let (d, t) = seg_dist(p, a, b);
        let len = dist(a, b);
        if d < best {
            best = d;
            along = run + t * len;
        }
        run += len;
    }
    (best, along)
}

/// 0 near the starts, the engine and the node sites (keep them level), 1 elsewhere.
fn pad_calm(t: &Terrain, x: f64, y: f64) -> f64 {
    let mut calm: f64 = 1.0;
    for &s in SPAWNS
        .iter()
        .chain([ENGINE].iter())
        .chain(LAND_SITES.iter())
    {
        let d = dist((x, y), t.sv(s));
        calm = calm.min(smoothstep(t.sv_len(220.0), t.sv_len(700.0), d));
    }
    calm
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Terrain {
    /// Design position to map position.
    fn sv(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let f = self.size / DESIGN_M;
        (x * f, y * f)
    }

    fn sv_len(&self, l: f64) -> f64 {
        l * self.size / DESIGN_M
    }

    /// Starts, pads, the town, ore and woods for the survival layout.
    pub(super) fn setup_survival(&mut self) {
        debug_assert_eq!(self.layout, Layout::Survival);
        // Fewer, thicker woods than the basin: the corridors want cover, the
        // mountains between them want to read as bare rock from above.
        self.forest_edge = -0.02;
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        let snap = |t: &Terrain, p: (f64, f64)| {
            let g = BUILD_CELL_M as f64;
            let p = t.sv(p);
            ((p.0 / g).round() * g, (p.1 / g).round() * g)
        };
        let mut pads = Vec::new();
        for &s in &SPAWNS {
            let at = snap(self, s);
            pads.push(Pad {
                x: at.0,
                y: at.1,
                core: start_core,
                outer: 2.0 * start_core,
                height: self.natural(at.0, at.1).max(10.0),
            });
            self.starts.push(at);
        }
        let engine = snap(self, ENGINE);
        let engine_core = self.sv_len(540.0).max(300.0);
        pads.push(Pad {
            x: engine.0,
            y: engine.1,
            core: engine_core,
            outer: engine_core * 1.35,
            height: PLATEAU_M,
        });
        self.starts.push(engine);
        for &site in &LAND_SITES {
            let at = snap(self, site);
            let core = self.sv_len(110.0).max(60.0);
            pads.push(Pad {
                x: at.0,
                y: at.1,
                core,
                outer: 2.1 * core,
                height: self.natural(at.0, at.1).max(8.0),
            });
        }
        let town = snap(self, (TOWN.0, TOWN.1));
        let town_r = self.sv_len(TOWN.2);
        pads.push(Pad {
            x: town.0,
            y: town.1,
            core: town_r,
            outer: 1.6 * town_r,
            height: self.natural(town.0, town.1).max(8.0),
        });
        self.towns.push(Town {
            x: town.0,
            y: town.1,
            radius: town_r,
            heading: 0.8,
        });
        self.pads = pads;

        // Two small fields at each start, behind it on either side (away
        // from the engine), inside its pad; the contested ones are no bigger.
        let small = (0.18 * start_core).max(55.0);
        let mut sites = Vec::new();
        for &(sx, sy) in &self.starts[..SPAWNS.len()] {
            let axis = (engine.1 - sy).atan2(engine.0 - sx);
            for turn in [PI - 0.95, PI + 0.95] {
                let (s, c) = (axis + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push((sx + c * d, sy + s * d, small));
            }
        }
        for &p in &CONTESTED_ORE {
            let at = snap(self, p);
            sites.push((at.0, at.1, small * 1.15));
        }
        self.ore = sites
            .into_iter()
            .map(|(x, y, r)| {
                let g = BUILD_CELL_M as f64;
                self.ore_field((x / g).round() * g, (y / g).round() * g, r)
            })
            .collect();
        self.fit_forests();
    }

    /// Signed distance to the coast in metres: positive on land.
    fn survival_shore(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        // The sea polygon: the coast then the closing corners off the map.
        const N: usize = COAST.len() + SEA_CLOSE.len();
        let corner = |i: usize| {
            self.sv(if i < COAST.len() {
                COAST[i]
            } else {
                SEA_CLOSE[i - COAST.len()]
            })
        };
        let mut inside = false;
        let mut d_coast = f64::INFINITY;
        for i in 0..N {
            let (a, b) = (corner(i), corner((i + 1) % N));
            if (a.1 > y) != (b.1 > y) && x < a.0 + (b.0 - a.0) * (y - a.1) / (b.1 - a.1) {
                inside = !inside;
            }
            if i + 1 < COAST.len() {
                d_coast = d_coast.min(seg_dist(p, a, b).0);
            }
        }
        // Edges past the coast are all well off the map.
        let mut sd = if inside { -d_coast } else { d_coast };
        // Bays and headlands, calmer by the harbour and the coastal start.
        let calm = smoothstep(
            self.sv_len(600.0),
            self.sv_len(1500.0),
            dist(p, self.sv(HARBOR)),
        ) * smoothstep(
            self.sv_len(700.0),
            self.sv_len(1500.0),
            dist(p, self.sv(SPAWNS[0])),
        );
        let l = self.sv_len(1000.0);
        sd += calm
            * (self.sv_len(140.0) * self.coast.fbm(x / l, y / l, 3, 0.55)
                + self.sv_len(110.0) * self.coast_warp.get(x / (3.0 * l), y / (3.0 * l)));
        // The harbour and its channel out to the sea.
        let (hd, _) = seg_dist(p, self.sv(HARBOR), self.sv(HARBOR_MOUTH));
        sd = sd.min(hd - self.sv_len(HARBOR_R));
        // The Teeth.
        for &(ix, iy, r) in &ISLANDS {
            let c = self.sv((ix, iy));
            let (vx, vy) = (x - c.0, y - c.1);
            let a = vy.atan2(vx);
            let wob = 1.0
                + 0.30
                    * self
                        .coast
                        .get(a.cos() * 1.7 + ix / 997.0, a.sin() * 1.7 + iy / 991.0);
            let d = (vx * vx + vy * vy).sqrt();
            sd = sd.max(self.sv_len(r) * wob - d);
        }
        sd
    }

    /// How open the ground is (1 on a corridor floor, in the defenders'
    /// pocket, on the plateau and by the shore; 0 in the mountains), and a
    /// rough distance to the nearest open ground (for massif height).
    fn survival_open(&self, x: f64, y: f64, shore: f64) -> (f64, f64) {
        let p = (x, y);
        let mut open: f64 = 0.0;
        let mut reach = f64::INFINITY;
        let wall = self.sv_len(CORRIDOR_WALL);
        let l = self.sv_len(700.0);
        let wobble = self.sv_len(CORRIDOR_WOBBLE) * self.ridge.get(x / l, y / l);
        for (i, c) in CORRIDORS.iter().enumerate() {
            let (d, along) = polyline_dist(p, c, self.size / DESIGN_M);
            // Narrows and wide reaches along the way.
            let reaches = self
                .ridge
                .get(along / self.sv_len(1_300.0), 7.5 + 3.0 * i as f64);
            let half = self.sv_len(CORRIDOR_HALF + CORRIDOR_REACH * reaches) + wobble;
            open = open.max(1.0 - smoothstep(half, half + wall, d));
            reach = reach.min(d - half);
        }
        let mut blob = |c: (f64, f64), r: f64, wall: f64| {
            let d = dist(p, self.sv(c)) - self.sv_len(r) - wobble;
            open = open.max(1.0 - smoothstep(0.0, wall, d));
            reach = reach.min(d);
        };
        for &site in &LAND_SITES {
            blob(site, 450.0, wall);
        }
        blob((TOWN.0, TOWN.1), 580.0, wall);
        // The defenders' pocket.
        blob(SPAWNS[0], 1_200.0, wall);
        blob(SPAWNS[1], 1_350.0, wall);
        blob(SPAWNS[2], 1_200.0, wall);
        blob((3_016.0, 3_416.0), 1_350.0, wall);
        // The plateau.
        blob(ENGINE, 1_100.0, 1.5 * wall);
        // A shore strip, so the mountains stand back from the sea.
        open = open.max(1.0 - smoothstep(self.sv_len(250.0), self.sv_len(520.0), shore));
        reach = reach.min(shore - self.sv_len(250.0));
        (open, reach)
    }

    /// The survival layout's landscape before pads, metres above the water.
    pub(super) fn natural_survival(&self, x: f64, y: f64) -> f64 {
        let s = self.size;
        let shore = self.survival_shore(x, y);
        let crag = self.crag.ridged(x / 700.0, y / 700.0, 4, 0.5);
        let detail = self.detail.fbm(x / 350.0, y / 350.0, 3, 0.45);
        // The sea: a short shelf, then deep water.
        let sea = -3.0 - 50.0 * smoothstep(0.0, self.sv_len(480.0), -shore);
        if shore <= 0.0 {
            return sea + 0.8 * detail;
        }

        // Lowland floor, climbing gently toward the engine and up onto its plateau.
        let u = (x + y) / SQRT_2 / s * DESIGN_M;
        let mut floor = 12.0 + 22.0 * smoothstep(8_000.0, 26_000.0, u);
        let d_engine = dist((x, y), self.sv(ENGINE));
        floor += (PLATEAU_M - floor)
            * (1.0 - smoothstep(self.sv_len(950.0), self.sv_len(1_900.0), d_engine));
        floor += 3.0 * self.tilt.get(x / 1800.0, y / 1800.0);

        // The mesa under the highland start.
        let mesa_c = self.sv(SPAWNS[1]);
        let (mx, my) = (x - mesa_c.0, y - mesa_c.1);
        let d_mesa = (mx * mx + my * my).sqrt();
        if d_mesa < self.sv_len(MESA_R + 700.0) {
            let a = my.atan2(mx);
            let ramp = MESA_RAMPS
                .iter()
                .map(|&r| {
                    let off = (a - r + PI).rem_euclid(2.0 * PI) - PI;
                    bump(off.abs() / 0.30)
                })
                .fold(0.0, f64::max);
            let r = self.sv_len(MESA_R)
                * (1.0 + 0.10 * self.lake_shore.get(a.cos() * 1.3, a.sin() * 1.3));
            let edge = self.sv_len(60.0 + 420.0 * ramp);
            floor += MESA_RISE * (1.0 - smoothstep(r - 0.5 * edge, r + 0.5 * edge, d_mesa));
        }

        // A shallow valley along the river.
        let (d_river, along) = polyline_dist((x, y), &RIVER, s / DESIGN_M);
        floor -= 5.0 * bump(d_river / self.sv_len(700.0));

        // Mountains wherever the ground is not open, taller deeper in.
        let (open, reach) = self.survival_open(x, y, shore);
        let m = 1.0 - open;
        let mut h = floor;
        if m > 0.0 {
            let tall = 165.0
                + 35.0 * self.mtn_height.get(x / 2600.0, y / 2600.0)
                + 90.0 * (crag - 0.3)
                + 110.0 * smoothstep(self.sv_len(650.0), self.sv_len(2_800.0), reach);
            h += m * tall;
        }
        // Low rolling ground and the odd rocky rise on the open floor.
        let roll = self.lake.fbm(x / 1100.0, y / 1100.0, 3, 0.5);
        let rise = self.crag.ridged(x / 900.0 + 5.3, y / 900.0 - 2.1, 2, 0.5);
        let calm = pad_calm(self, x, y);
        h += open
            * calm
            * (7.0 * roll + 8.0 * smoothstep(0.62, 0.9, rise) * smoothstep(0.1, 0.5, roll));
        h += (1.0 + 5.0 * m) * 1.4 * detail;

        // The river: too deep to wade, too shallow to sail, but for the fords.
        // It comes out of the foot of the ridge, so it starts in the open.
        let bed_w = self.sv_len(38.0);
        let bank = self.sv_len(110.0);
        let cut = (1.0 - smoothstep(bed_w, bank, d_river))
            * smoothstep(0.0, self.sv_len(300.0), along)
            * (1.0 - smoothstep(0.25, 0.6, m));
        if cut > 0.0 {
            let ford = FORDS
                .iter()
                .map(|&f| bump(dist((x, y), self.sv(f)) / self.sv_len(140.0)))
                .fold(0.0, f64::max);
            let bed = -3.2 + 3.9 * smoothstep(0.2, 0.7, ford);
            h += (bed - h) * cut;
        }

        // A rocky knoll on each of the Teeth.
        for &(ix, iy, r) in &ISLANDS {
            let d = dist((x, y), self.sv((ix, iy)));
            let r = self.sv_len(r);
            if d < r {
                h += (18.0 + 45.0 * crag) * bump(d / (0.75 * r));
            }
        }

        // Down to the water on a beach.
        let beach = smoothstep(0.0, self.sv_len(240.0), shore);
        sea + (h - sea) * beach
    }
}
