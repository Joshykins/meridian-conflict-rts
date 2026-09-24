//! The alpine layouts: mountain country on a coast, with glaciers.
//!
//! * [`Layout::Alpine`], "Serac Divide", 1v1 on 8 km: player 0 in the south,
//!   player 1 in the north, the sea along the east half. Two ways north: an
//!   inland valley, and a broad coast lowland open to the sea most of its
//!   length, with a harbour cove at the middle. Each base has a harbour
//!   beach; glaciers calve into the sea behind the bases; islands offshore.
//! * [`Layout::AlpineTeams`], "Serac Sound", 4v4 on 12 km: each side's four
//!   bases together in one home basin with a harbour, the sea over the east
//!   half, and one way between the sides: a broad pass along the shore, open
//!   to the sea, over a saddle with a ruined town. An icefield fills the rest
//!   of the middle.
//!
//! Fairness: everything a unit can reach (lanes, open ground, rises, the
//! coast, inlets and islands, ore, pads) is laid out once for the south side
//! and stands again mirrored across the middle of the map for the north.
//! Noise that shapes it is sampled in the feature's own frame, so the mirror
//! is exact. What nothing can reach is not mirrored: the massifs' height and
//! crags, horn peaks, ice caps, glaciers and the woods, so the two halves play
//! alike and look nothing alike. Glaciers never reach a lane (a test holds
//! them to it); the ground near the lanes is mirrored, and a band of cliff
//! round every lane keeps units off the massifs.
//!
//! Coordinates are metres at the design's own size; another size scales it.

use super::{Layout, Pad, Terrain, Town};
use crate::noise::{bump, smoothstep};
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

/// A lane: points along its floor, with the floor's half width there.
type Lane = &'static [(f64, f64, f64)];

/// One alpine map, laid out for its south side.
pub(super) struct Design {
    /// Map edge the coordinates are written for.
    size: f64,
    /// Starts on the south side; each is followed on the map by its mirror.
    starts: &'static [(f64, f64)],
    lanes: &'static [Lane],
    /// Open ground that is not a lane: centre and radius.
    blobs: &'static [(f64, f64, f64)],
    /// Rises in the floor: centre, radius, height.
    rises: &'static [(f64, f64, f64, f64)],
    /// The south half's coast as (y, x): the sea lies east of it. Runs to the
    /// middle line.
    coast: &'static [(f64, f64)],
    /// Sea reaching into the land: fjords, coves. Lines with half widths.
    inlets: &'static [Lane],
    /// Islands: centre and radius. Open ones are also in `blobs`.
    islands: &'static [(f64, f64, f64)],
    /// Ore away from the bases: centre and radius.
    ore: &'static [(f64, f64, f64)],
    /// Ruined towns: centre and radius (with ore in the square).
    towns: &'static [(f64, f64, f64)],
    // -- not mirrored: positions are the map's own --
    /// Ice caps: centre, radius, height of the dome.
    ice_caps: &'static [(f64, f64, f64, f64)],
    /// Horn peaks: centre, height, radius of the pyramid's foot.
    horns: &'static [(f64, f64, f64, f64)],
    glaciers: &'static [Glacier],
}

/// A glacier. A land glacier is traced down the eroded ground from its head
/// (`line` gives the head and the way it sets off, and its length); one
/// that calves into the sea is drawn along `line`. Half widths run from the
/// basin that feeds it to the tongue; `head` and `snout` are the ice surface
/// of a drawn one.
struct Glacier {
    line: &'static [(f64, f64, f64)],
    head: f64,
    snout: f64,
    calving: bool,
}

const fn land(line: &'static [(f64, f64, f64)]) -> Glacier {
    Glacier { line, head: 0.0, snout: 0.0, calving: false }
}

const fn tidewater(line: &'static [(f64, f64, f64)], head: f64) -> Glacier {
    Glacier { line, head, snout: 55.0, calving: true }
}

/// "Serac Divide", 1v1.
const DUEL: Design = Design {
    size: 8_192.0,
    starts: &[(2_600.0, 1_700.0)],
    lanes: &[
        // The inland valley.
        &[(2_000.0, 2_100.0, 360.0), (1_400.0, 2_700.0, 360.0), (1_200.0, 3_400.0, 350.0), (1_150.0, 4_096.0, 330.0)],
        // The coast: a broad lowland along the shore, beaches most of the way.
        &[(3_300.0, 2_000.0, 500.0), (3_700.0, 2_800.0, 550.0), (3_650.0, 3_500.0, 520.0), (3_550.0, 4_096.0, 480.0)],
    ],
    blobs: &[
        (2_600.0, 1_700.0, 850.0),
        // The harbour beach beside the base.
        (3_650.0, 1_450.0, 550.0),
        // The meadows at the middle.
        (1_150.0, 4_096.0, 420.0),
        (3_550.0, 4_096.0, 600.0),
        // Open islands.
        (5_600.0, 1_600.0, 330.0),
        (5_900.0, 4_096.0, 400.0),
    ],
    rises: &[(1_150.0, 4_096.0, 1_300.0, 30.0), (2_300.0, 4_096.0, 1_800.0, 40.0)],
    // A bay for the harbour, a rocky headland out into the sea, and a bay
    // by the cove at the middle.
    coast: &[
        (-400.0, 5_000.0),
        (300.0, 4_750.0),
        (800.0, 4_200.0),
        (1_400.0, 3_980.0),
        (2_000.0, 4_400.0),
        (2_350.0, 4_850.0),
        (2_600.0, 5_200.0),
        (2_800.0, 5_300.0),
        (2_980.0, 5_100.0),
        (3_150.0, 4_600.0),
        (3_600.0, 4_150.0),
        (4_096.0, 4_300.0),
    ],
    // A harbour cove at the middle, off the coast meadow.
    inlets: &[&[(3_950.0, 4_096.0, 190.0), (4_500.0, 4_096.0, 300.0)]],
    islands: &[
        (5_600.0, 1_600.0, 460.0),
        (6_800.0, 2_900.0, 280.0),
        (5_900.0, 4_096.0, 520.0),
        (7_400.0, 700.0, 220.0),
        (5_100.0, 3_600.0, 170.0),
        (7_700.0, 3_900.0, 160.0),
        (6_350.0, 2_450.0, 140.0),
        (6_600.0, 2_600.0, 120.0),
        (5_700.0, 2_550.0, 130.0),
    ],
    ore: &[
        (1_350.0, 2_800.0, 70.0),
        (3_700.0, 2_600.0, 75.0),
        (3_750.0, 1_350.0, 70.0),
        (1_150.0, 4_096.0, 85.0),
        (3_350.0, 4_096.0, 90.0),
        (5_600.0, 1_600.0, 80.0),
        (5_900.0, 4_096.0, 90.0),
    ],
    towns: &[],
    ice_caps: &[(2_350.0, 4_450.0, 420.0, 520.0)],
    horns: &[
        (300.0, 2_600.0, 560.0, 600.0),
        (300.0, 5_800.0, 580.0, 600.0),
        (2_350.0, 3_150.0, 600.0, 420.0),
        (2_300.0, 5_150.0, 620.0, 420.0),
        (1_200.0, 400.0, 540.0, 500.0),
        (1_600.0, 7_850.0, 560.0, 500.0),
    ],
    glaciers: &[
        land(&[(2_350.0, 4_200.0, 190.0), (2_380.0, 3_550.0, 140.0)]),
        land(&[(2_300.0, 4_750.0, 190.0), (2_250.0, 5_350.0, 140.0)]),
        land(&[(250.0, 1_700.0, 220.0), (600.0, 1_900.0, 160.0)]),
        land(&[(250.0, 6_400.0, 220.0), (600.0, 6_150.0, 160.0)]),
        // Tidewater: behind each base, out into the sea.
        tidewater(&[(700.0, 750.0, 250.0), (1_700.0, 430.0, 200.0), (2_900.0, 330.0, 180.0), (4_100.0, 290.0, 160.0), (4_850.0, 280.0, 150.0)], 430.0),
        tidewater(&[(500.0, 7_300.0, 260.0), (1_500.0, 7_750.0, 210.0), (2_700.0, 7_930.0, 180.0), (3_900.0, 7_950.0, 160.0), (4_700.0, 7_960.0, 150.0)], 450.0),
    ],
};

/// "Serac Sound", 4v4.
const TEAMS: Design = Design {
    size: 12_288.0,
    starts: &[(1_600.0, 1_900.0), (3_300.0, 1_500.0), (5_000.0, 2_000.0), (3_200.0, 3_200.0)],
    lanes: &[
        // The pass: the one way between the sides, broad, along the shore.
        &[(4_300.0, 3_400.0, 450.0), (5_200.0, 4_200.0, 550.0), (5_600.0, 5_100.0, 650.0), (5_650.0, 6_144.0, 700.0)],
        // The shore road: from the harbour along the beach to the pass.
        &[(5_800.0, 1_300.0, 450.0), (5_700.0, 2_200.0, 480.0), (5_550.0, 3_200.0, 450.0), (5_500.0, 4_200.0, 520.0)],
    ],
    blobs: &[
        // The home basin round the four bases, and each base's bowl.
        (3_200.0, 2_350.0, 1_400.0),
        (1_600.0, 1_900.0, 650.0),
        (3_300.0, 1_500.0, 650.0),
        (5_000.0, 2_000.0, 650.0),
        (3_200.0, 3_200.0, 650.0),
        // The home harbour.
        (5_900.0, 1_400.0, 550.0),
        // Open islands.
        (8_200.0, 1_800.0, 420.0),
        (9_800.0, 3_400.0, 300.0),
        (8_000.0, 6_144.0, 480.0),
    ],
    rises: &[(5_650.0, 6_144.0, 1_500.0, 55.0)],
    // Open to the sea: beaches beside the bases, along the shore road and
    // all down the pass's east side.
    coast: &[
        (-400.0, 6_500.0),
        (300.0, 6_250.0),
        (900.0, 6_000.0),
        (1_500.0, 5_750.0),
        (2_100.0, 6_000.0),
        (2_700.0, 6_300.0),
        (3_300.0, 6_050.0),
        (3_900.0, 5_850.0),
        (4_500.0, 6_150.0),
        (5_100.0, 6_350.0),
        (5_700.0, 6_150.0),
        (6_144.0, 6_250.0),
    ],
    inlets: &[],
    islands: &[
        (8_200.0, 1_800.0, 620.0),
        (9_800.0, 3_400.0, 420.0),
        (8_000.0, 6_144.0, 720.0),
        (10_500.0, 6_144.0, 300.0),
        (11_200.0, 1_200.0, 300.0),
        (9_200.0, 4_900.0, 250.0),
        (11_500.0, 4_200.0, 220.0),
        (7_300.0, 4_050.0, 180.0),
        (8_900.0, 2_900.0, 150.0),
        (9_150.0, 3_050.0, 130.0),
        (10_600.0, 2_500.0, 160.0),
    ],
    ore: &[
        (3_000.0, 2_450.0, 85.0),
        (1_200.0, 2_900.0, 70.0),
        (5_900.0, 1_300.0, 75.0),
        (5_300.0, 4_200.0, 80.0),
        (6_150.0, 5_300.0, 75.0),
        (5_650.0, 6_144.0, 95.0),
        (8_200.0, 1_800.0, 85.0),
        (9_800.0, 3_400.0, 80.0),
        (8_000.0, 6_144.0, 100.0),
    ],
    towns: &[(5_350.0, 6_144.0, 200.0)],
    ice_caps: &[(2_300.0, 6_700.0, 850.0, 590.0)],
    horns: &[
        (1_200.0, 5_100.0, 650.0, 600.0),
        (3_900.0, 5_600.0, 700.0, 600.0),
        (3_400.0, 7_500.0, 680.0, 550.0),
        (700.0, 7_900.0, 620.0, 600.0),
        (250.0, 2_300.0, 580.0, 500.0),
        (250.0, 10_200.0, 580.0, 500.0),
    ],
    glaciers: &[
        land(&[(2_300.0, 6_200.0, 300.0), (2_000.0, 5_300.0, 200.0)]),
        land(&[(2_700.0, 7_200.0, 280.0), (3_200.0, 7_900.0, 200.0)]),
        land(&[(3_000.0, 6_500.0, 260.0), (3_700.0, 6_300.0, 190.0)]),
        land(&[(1_200.0, 5_300.0, 220.0), (1_250.0, 4_700.0, 170.0)]),
        // Tidewater: behind each home basin, out into the sea.
        tidewater(&[(1_300.0, 500.0, 260.0), (2_900.0, 300.0, 210.0), (4_600.0, 260.0, 180.0), (6_100.0, 240.0, 160.0), (7_100.0, 230.0, 150.0)], 440.0),
        tidewater(&[(700.0, 11_500.0, 260.0), (2_200.0, 11_900.0, 210.0), (3_900.0, 12_020.0, 180.0), (5_600.0, 12_050.0, 160.0), (7_000.0, 12_060.0, 150.0)], 460.0),
    ],
};

/// How far a lane's walls take to rise, and how much the floor's edge wanders.
const WALL: f64 = 260.0;
const WOBBLE: f64 = 30.0;
/// Mountains are mirrored up to this far from open ground (where their foot
/// decides how far up a unit can climb), then fade into their own shapes.
const MIRRORED_FOOT: (f64, f64) = (60.0, 320.0);
/// Depth of the sea.
const DEEP: f64 = 40.0;
/// The cliff band round the lanes: how far up the wall it stands (metres
/// from the floor's edge, and how much that wanders), and its height.
const CLIFF_BAND: (f64, f64, f64) = (85.0, 60.0, 38.0);
/// How far a valley glacier's ice stands over the floor of its trough.
const ICE_WALL: f64 = 42.0;
/// Softly held under the top of the height encoding (768 m).
const CEILING: f64 = 740.0;

/// Water erosion over the mountains: droplets run downhill over a coarse
/// copy of the terrain, cutting gullies and dropping what they carry where
/// they slow down, which turns noise into ridges, couloirs and fans. Only the
/// change is kept, and it is only laid on ground far from any lane.
#[derive(Default)]
pub(super) struct Erosion {
    /// Samples per edge, and metres between them.
    n: usize,
    step: f64,
    /// Metres the terrain moved at each sample.
    delta: Vec<f32>,
}

/// Metres between erosion samples.
const EROSION_STEP: f64 = 16.0;
/// Droplets per sample, and the most steps one takes.
const EROSION_DROPS: f64 = 0.8;
const EROSION_LIFE: usize = 56;
/// Metres per unit of height while the droplets run: the rates below are
/// tuned for gentler slopes than a mountain's, so the grid is flattened.
const EROSION_VERTICAL: f32 = 48.0;

impl Erosion {
    /// The change at a map position, bilinear.
    fn at(&self, x: f64, y: f64) -> f64 {
        if self.delta.is_empty() {
            return 0.0;
        }
        let n = self.n;
        let (gx, gy) = ((x / self.step).clamp(0.0, (n - 1) as f64), (y / self.step).clamp(0.0, (n - 1) as f64));
        let (i, j) = ((gx as usize).min(n - 2), (gy as usize).min(n - 2));
        let (fx, fy) = (gx - i as f64, gy - j as f64);
        let d = |i: usize, j: usize| self.delta[j * n + i] as f64;
        (d(i, j) * (1.0 - fx) + d(i + 1, j) * fx) * (1.0 - fy) + (d(i, j + 1) * (1.0 - fx) + d(i + 1, j + 1) * fx) * fy
    }
}

/// Runs the droplets over `h` (row-major, `n` per edge). Deterministic.
fn erode(h: &mut [f32], n: usize, seed: u64) {
    const INERTIA: f32 = 0.05;
    const CAPACITY: f32 = 4.0;
    const MIN_CAPACITY: f32 = 0.01;
    const ERODE: f32 = 0.3;
    const DEPOSIT: f32 = 0.3;
    const EVAPORATE: f32 = 0.015;
    const GRAVITY: f32 = 4.0;
    const RADIUS: i32 = 2;
    let mut state = seed ^ 0x6572_6F64_6521;
    let mut next = || {
        // splitmix64
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    };
    let brush: Vec<(i32, i32, f32)> = {
        let mut b = Vec::new();
        for dy in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                let w = RADIUS as f32 - ((dx * dx + dy * dy) as f32).sqrt();
                if w > 0.0 {
                    b.push((dx, dy, w));
                }
            }
        }
        let total: f32 = b.iter().map(|t| t.2).sum();
        b.into_iter().map(|(x, y, w)| (x, y, w / total)).collect()
    };
    let height_grad = |h: &[f32], x: f32, y: f32| {
        let (i, j) = (x as usize, y as usize);
        let (u, v) = (x - i as f32, y - j as f32);
        let at = j * n + i;
        let (h00, h10, h01, h11) = (h[at], h[at + 1], h[at + n], h[at + n + 1]);
        let gx = (h10 - h00) * (1.0 - v) + (h11 - h01) * v;
        let gy = (h01 - h00) * (1.0 - u) + (h11 - h10) * u;
        let z = h00 * (1.0 - u) * (1.0 - v) + h10 * u * (1.0 - v) + h01 * (1.0 - u) * v + h11 * u * v;
        (z, gx, gy)
    };
    let limit = (n - 2) as f32;
    let drops = (EROSION_DROPS * (n * n) as f64) as usize;
    for _ in 0..drops {
        let (mut x, mut y) = (next() * limit, next() * limit);
        let (mut dx, mut dy) = (0.0f32, 0.0f32);
        let (mut speed, mut water, mut sediment) = (1.0f32, 1.0f32, 0.0f32);
        for _ in 0..EROSION_LIFE {
            let (i, j) = (x as usize, y as usize);
            let (u, v) = (x - i as f32, y - j as f32);
            let (z, gx, gy) = height_grad(h, x, y);
            dx = dx * INERTIA - gx * (1.0 - INERTIA);
            dy = dy * INERTIA - gy * (1.0 - INERTIA);
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-6 {
                break;
            }
            dx /= len;
            dy /= len;
            let (nx, ny) = (x + dx, y + dy);
            if !(0.0..limit).contains(&nx) || !(0.0..limit).contains(&ny) {
                break;
            }
            let drop = z - height_grad(h, nx, ny).0;
            let capacity = (drop * speed * water * CAPACITY).max(MIN_CAPACITY);
            if sediment > capacity || drop < 0.0 {
                // Uphill or overloaded: leave some behind, round the old spot.
                let amount = if drop < 0.0 { (-drop).min(sediment) } else { (sediment - capacity) * DEPOSIT };
                sediment -= amount;
                let at = j * n + i;
                h[at] += amount * (1.0 - u) * (1.0 - v);
                h[at + 1] += amount * u * (1.0 - v);
                h[at + n] += amount * (1.0 - u) * v;
                h[at + n + 1] += amount * u * v;
            } else {
                let amount = ((capacity - sediment) * ERODE).min(drop);
                for &(bx, by, w) in &brush {
                    let (ci, cj) = (i as i32 + bx, j as i32 + by);
                    if ci >= 0 && cj >= 0 && (ci as usize) < n && (cj as usize) < n {
                        h[cj as usize * n + ci as usize] -= amount * w;
                    }
                }
                sediment += amount;
            }
            speed = (speed * speed + drop * GRAVITY).max(0.0).sqrt();
            water *= 1.0 - EVAPORATE;
            x = nx;
            y = ny;
        }
    }
}

/// Rock falls off anything steeper than it can stand at, piling up as scree
/// below: this takes the needles and knife-edges out of the eroded ground.
fn slump(h: &mut [f32], n: usize) {
    // Steepest a face may stay, in height units per sample: about 50 degrees.
    let talus = 1.2 * EROSION_STEP as f32 / EROSION_VERTICAL;
    for _ in 0..24 {
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                for nb in [at - 1, at + 1, at - n, at + n] {
                    let drop = h[at] - h[nb];
                    if drop > talus {
                        let moved = (drop - talus) * 0.25;
                        h[at] -= moved;
                        h[nb] += moved;
                    }
                }
            }
        }
    }
}

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

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Distance to a polyline (design metres scaled by `f`), how far along it the
/// nearest point is, its total length, and the value of `w` (one per point)
/// interpolated there.
fn polyline(p: (f64, f64), pts: &[(f64, f64)], w: impl Fn(usize) -> f64, f: f64) -> (f64, f64, f64, f64) {
    let (mut best, mut along, mut run, mut value) = (f64::INFINITY, 0.0, 0.0, w(0));
    for i in 0..pts.len().saturating_sub(1) {
        let (a, b) = ((pts[i].0 * f, pts[i].1 * f), (pts[i + 1].0 * f, pts[i + 1].1 * f));
        let (d, t) = seg_dist(p, a, b);
        let len = dist(a, b);
        if d < best {
            best = d;
            along = run + t * len;
            value = w(i) + (w(i + 1) - w(i)) * t;
        }
        run += len;
    }
    (best, along, run, value)
}

/// `h` squeezed softly under [`CEILING`].
fn under_ceiling(h: f64) -> f64 {
    const KNEE: f64 = CEILING - 60.0;
    if h <= KNEE {
        h
    } else {
        KNEE + 60.0 * ((h - KNEE) / 60.0).tanh()
    }
}

/// A glacier as laid on the map: its flow line and, at every point of it,
/// the half width and the ice surface. Map metres.
pub(super) struct IceFlow {
    pts: Vec<(f64, f64)>,
    half: Vec<f64>,
    surface: Vec<f64>,
    calving: bool,
    /// Box round everything the glacier can touch.
    lo: (f64, f64),
    hi: (f64, f64),
}

#[derive(Default)]
pub(super) struct IceFlows {
    flows: Vec<IceFlow>,
}

impl IceFlow {
    fn new(pts: Vec<(f64, f64)>, half: Vec<f64>, surface: Vec<f64>, calving: bool, margin: f64) -> IceFlow {
        let reach = margin + half.iter().cloned().fold(0.0, f64::max);
        let lo = pts.iter().fold((f64::INFINITY, f64::INFINITY), |a, p| (a.0.min(p.0), a.1.min(p.1)));
        let hi = pts.iter().fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| (a.0.max(p.0), a.1.max(p.1)));
        IceFlow { pts, half, surface, calving, lo: (lo.0 - reach, lo.1 - reach), hi: (hi.0 + reach, hi.1 + reach) }
    }

    /// Smoothed distance to the flow line, how far along it, its length, and
    /// the half width and ice surface there; `None` well away from it.
    fn near(&self, p: (f64, f64), soft: f64) -> Option<(f64, f64, f64, f64, f64)> {
        if p.0 < self.lo.0 || p.1 < self.lo.1 || p.0 > self.hi.0 || p.1 > self.hi.1 {
            return None;
        }
        let mut segs = Vec::with_capacity(self.pts.len());
        let mut run = 0.0;
        for i in 0..self.pts.len() - 1 {
            let (a, b) = (self.pts[i], self.pts[i + 1]);
            let (d, t) = seg_dist(p, a, b);
            let lerp = |v: &[f64]| v[i] + (v[i + 1] - v[i]) * t;
            segs.push((d, run + t * dist(a, b), lerp(&self.half), lerp(&self.surface)));
            run += dist(a, b);
        }
        let best = segs.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
        let (mut total, mut along, mut half, mut surface) = (0.0, 0.0, 0.0, 0.0);
        for &(d, a, h, z) in &segs {
            let k = (-(d - best) / soft).exp();
            total += k;
            along += k * a;
            half += k * h;
            surface += k * z;
        }
        Some((best - 0.5 * soft * total.ln(), along / total, run, half / total, surface / total))
    }
}

impl Terrain {
    fn design(&self) -> &'static Design {
        match self.layout {
            Layout::AlpineTeams => &TEAMS,
            _ => &DUEL,
        }
    }

    fn scale_a(&self) -> f64 {
        self.size / self.design().size
    }

    /// Design position to map position.
    fn ap(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let f = self.scale_a();
        (x * f, y * f)
    }

    fn al(&self, l: f64) -> f64 {
        l * self.scale_a()
    }

    /// A map position mirrored across the middle line (south to north).
    fn mirror(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (x, self.size_y - y)
    }

    /// How far south of the middle line, in metres (negative in the north).
    fn south_of_middle(&self, y: f64) -> f64 {
        self.size_y / 2.0 - y
    }

    /// Nearest build-grid vertex of a design position.
    fn snap_a(&self, p: (f64, f64)) -> (f64, f64) {
        let g = BUILD_CELL_M as f64;
        let p = self.ap(p);
        ((p.0 / g).round() * g, (p.1 / g).round() * g)
    }

    /// Starts, pads, towns and ore for an alpine layout.
    pub(super) fn setup_alpine(&mut self) {
        let design = self.design();
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        // Woods are thick in the valleys: the ground is fair, the trees need not be.
        self.forest_edge = 0.05;
        let on_middle = |t: &Terrain, p: (f64, f64)| (t.mirror(p).1 - p.1).abs() < 1.0;

        let mut pads = Vec::new();
        // Every pad stands twice, at the same height (the south copy's),
        // but once on the middle line.
        let mut pad = |t: &Terrain, at: (f64, f64), core: f64, outer: f64, floor: f64| {
            let at = t.snap_a(at);
            let height = t.natural(at.0, at.1).max(floor);
            pads.push(Pad { x: at.0, y: at.1, core, outer, height });
            if !on_middle(t, at) {
                let m = t.mirror(at);
                pads.push(Pad { x: m.0, y: m.1, core, outer, height });
            }
        };
        for &s in design.starts {
            pad(self, s, start_core, 2.0 * start_core, 10.0);
        }
        for &(x, y, r) in design.towns {
            let r = self.al(r);
            pad(self, (x, y), r, 1.6 * r, 8.0);
        }
        self.pads = pads;
        self.erode_alpine();
        self.lay_glaciers();

        let south: Vec<(f64, f64)> = design.starts.iter().map(|&s| self.snap_a(s)).collect();
        // South and north in turn, each start's twin next to it: skirmish
        // set-up's "Two Sides" (teams by alternate slot) then puts one side
        // on each half.
        self.starts = south.iter().flat_map(|&s| [s, self.mirror(s)]).collect();
        for &(x, y, r) in design.towns {
            let at = self.snap_a((x, y));
            let r = self.al(r);
            self.towns.push(Town { x: at.0, y: at.1, radius: r, heading: 0.3 });
            if !on_middle(self, at) {
                let m = self.mirror(at);
                self.towns.push(Town { x: m.0, y: m.1, radius: r, heading: -0.3 });
            }
        }

        // Three small fields round each base, inside its pad: one behind
        // (away from the middle), two on the forward flanks.
        let mut sites = Vec::new();
        for &a in &south {
            for turn in [0.0, PI - 1.15, PI + 1.15] {
                let (s, c) = (-PI / 2.0 + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((a.0 + c * d, a.1 + s * d), (0.2 * start_core).max(55.0)));
            }
        }
        for &(x, y, r) in design.ore {
            sites.push((self.snap_a((x, y)), self.al(r)));
        }
        for &(x, y, r) in design.towns {
            sites.push((self.snap_a((x, y)), 0.45 * self.al(r)));
        }
        let g = BUILD_CELL_M as f64;
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = ((p.0 / g).round() * g, (p.1 / g).round() * g);
            let field = self.ore_field(p.0, p.1, r);
            // The north copy is the south one reflected, corner for corner.
            if !on_middle(self, p) {
                let mut corners: Vec<(f64, f64)> = field.corners.iter().map(|&c| self.mirror(c)).collect();
                corners.reverse();
                let m = self.mirror(p);
                fields.push(super::OreField { x: m.0, y: m.1, radius: field.radius, corners });
            }
            fields.push(field);
        }
        self.ore = fields;
        self.fit_forests();
    }

    /// Runs water erosion over a coarse copy of the landscape and keeps the change.
    fn erode_alpine(&mut self) {
        let step = EROSION_STEP;
        let n = (self.size_x.max(self.size_y) / step) as usize + 1;
        let mut h = vec![0f32; n * n];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in h.chunks_mut(rows * n).enumerate() {
                let t = &*self;
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(n).enumerate() {
                        let y = ((k * rows + r) as f64 * step).min(t.size_y);
                        for (i, v) in row.iter_mut().enumerate() {
                            let x = (i as f64 * step).min(t.size_x);
                            *v = t.natural(x, y) as f32 / EROSION_VERTICAL;
                        }
                    }
                });
            }
        });
        let before = h.clone();
        erode(&mut h, n, self.seed);
        slump(&mut h, n);
        let scale = EROSION_VERTICAL;
        let raw: Vec<f32> = h
            .iter()
            .zip(&before)
            .map(|(a, b)| ((a - b) * scale).clamp(-90.0, 40.0))
            .collect();
        // One soft pass, so no single sample stands up as a needle.
        let mut delta = raw.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                let around = raw[at - 1] + raw[at + 1] + raw[at - n] + raw[at + n];
                delta[at] = 0.5 * raw[at] + 0.125 * around;
            }
        }
        self.erosion = Erosion { n, step, delta };
    }

    // -- the fair part --------------------------------------------------------
    // Functions of a map position `q` for the south copy of every feature;
    // called at `p` and at `mirror(p)`, they give both copies.

    /// How open the ground is round the south features (1 on a lane floor,
    /// 0 in the mountains), and the distance to the nearest open floor.
    fn alpine_open_near(&self, q: (f64, f64), wobble: f64) -> (f64, f64) {
        let f = self.scale_a();
        // Some walls lean back over a long scree, some stand nearly sheer.
        let lw = self.al(1_100.0);
        let wall = self.al(WALL) * (1.0 + 0.45 * self.mtn_gap.fbm(q.0 / lw + 3.3, q.1 / lw - 8.1, 2, 0.5));
        let (mut open, mut reach): (f64, f64) = (0.0, f64::INFINITY);
        for lane in self.design().lanes {
            let pts: Vec<(f64, f64)> = lane.iter().map(|&(x, y, _)| (x, y)).collect();
            let (d, _, _, half) = polyline(q, &pts, |i| lane[i].2, f);
            let half = self.al(half) + wobble;
            open = open.max(1.0 - smoothstep(half, half + wall, d));
            reach = reach.min(d - half);
        }
        for &(x, y, r) in self.design().blobs {
            // Bays and spurs round the edge: no round bowls.
            let c = self.ap((x, y));
            let (vx, vy) = (q.0 - c.0, q.1 - c.1);
            let len = (vx * vx + vy * vy).sqrt().max(1.0);
            let lobes = 0.22 * self.coast_warp.fbm(vx / len * 1.8 + x / 811.0, vy / len * 1.8 + y / 797.0, 3, 0.55);
            let d = len - self.al(r) * (1.0 + lobes) - wobble;
            open = open.max(1.0 - smoothstep(0.0, wall, d));
            reach = reach.min(d);
        }
        (open, reach)
    }

    /// Open ground and reach over both copies.
    fn alpine_open(&self, x: f64, y: f64) -> (f64, f64) {
        let (l, s) = (self.al(900.0), self.al(210.0));
        // The edge wanders in each copy's own frame, so both wander alike:
        // broad bays and spurs, and a ragged fringe on them.
        let wobble = |q: (f64, f64)| {
            self.al(WOBBLE) * (3.0 * self.ridge.fbm(q.0 / l, q.1 / l, 2, 0.5)
                + 2.2 * self.ridge.get(q.0 / s + 17.0, q.1 / s - 9.0)
                + 1.0 * self.ridge.get(q.0 / (0.5 * s) - 5.0, q.1 / (0.5 * s) + 13.0))
        };
        let pm = self.mirror((x, y));
        let (o1, r1) = self.alpine_open_near((x, y), wobble((x, y)));
        let (o2, r2) = self.alpine_open_near(pm, wobble(pm));
        (o1.max(o2), r1.min(r2))
    }

    /// The floor's own shape round the south features: bases a little proud
    /// of their valleys, and the rises. Max, not sum, so a feature on the
    /// middle line counts once.
    fn alpine_floor_near(&self, q: (f64, f64)) -> f64 {
        let mut floor: f64 = 0.0;
        for &s in self.design().starts {
            floor = floor.max(10.0 * bump(dist(q, self.ap(s)) / self.al(1_600.0)));
        }
        for &(x, y, r, h) in self.design().rises {
            floor = floor.max(h * (1.0 - smoothstep(self.al(0.15 * r), self.al(r), dist(q, self.ap((x, y))))));
        }
        floor
    }

    /// Signed distance to the south half's sea (the coast, the inlets, less
    /// the islands), positive on land; nothing north of the middle line.
    fn alpine_sea_near(&self, q: (f64, f64)) -> f64 {
        if self.south_of_middle(q.1) < 0.0 {
            return f64::INFINITY;
        }
        let d = self.design();
        let f = self.scale_a();
        let y = q.1 / f;
        let i = d.coast.windows(2).position(|w| y <= w[1].0).unwrap_or(d.coast.len() - 2);
        let (a, b) = (d.coast[i], d.coast[i + 1]);
        let t = ((y - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        let coast = self.al(a.1 + (b.1 - a.1) * t);
        let l = self.al(420.0);
        let wob = self.al(80.0) * self.coast.fbm(q.0 / l + 13.0, q.1 / l, 3, 0.5);
        let mut land = coast + wob - q.0;
        for inlet in d.inlets {
            let pts: Vec<(f64, f64)> = inlet.iter().map(|&(x, y, _)| (x, y)).collect();
            let (dd, _, _, half) = polyline(q, &pts, |i| inlet[i].2, f);
            land = land.min(dd - self.al(half));
        }
        for &(x, y, r) in d.islands {
            let c = self.ap((x, y));
            let (vx, vy) = (q.0 - c.0, q.1 - c.1);
            let a = vy.atan2(vx);
            let r = self.al(r) * (1.0 + 0.2 * self.coast.fbm(a.cos() * 1.6 + x / 997.0, a.sin() * 1.6 + y / 991.0, 2, 0.5));
            land = land.max(r - (vx * vx + vy * vy).sqrt());
        }
        land
    }

    // -- what is not mirrored ---------------------------------------------------

    /// Bays and headlands on the coast, where no lane comes near: added to
    /// the distance to the sea (positive pushes the land out).
    fn coast_far(&self, x: f64, y: f64, reach: f64) -> f64 {
        let free = smoothstep(self.al(350.0), self.al(750.0), reach);
        if free <= 0.0 {
            return 0.0;
        }
        let l = self.al(900.0);
        free * (self.al(220.0) * self.coast_warp.fbm(x / l - 4.0, y / l + 6.0, 3, 0.55)
            + self.al(80.0) * self.coast.fbm(x / (0.3 * l), y / (0.3 * l) - 2.0, 2, 0.5))
    }

    /// A glacier's ice surface over `(x, y)` and how much of it is there:
    /// `body` (the trough it lies in, soft), `ice` (the ice, for the
    /// renderer) and `slab` (the ice standing over its trough, falling to 0
    /// across the face of its wall). `None` away from it.
    fn glacier_at(&self, g: &IceFlow, x: f64, y: f64) -> Option<(f64, f64, f64, f64)> {
        let (d, along, len, half, surface) = g.near((x, y), self.al(90.0))?;
        // The crown follows the smooth width: bumps in the ice would bend
        // its crevasses into whorls.
        let crown = 7.0 * (1.0 - (d / half).powi(2)).max(0.0);
        let l = self.al(320.0);
        let half = half
            * (1.0 + 0.2 * self.lake.fbm(x / l + 3.1, y / l - 7.7, 3, 0.55))
            // Narrower toward the snout.
            * (1.0 - 0.2 * smoothstep(0.7, 1.0, along / len.max(1.0)))
            // A ragged edge: the wall bays in and out.
            + self.al(12.0) * self.detail.get(x / self.al(55.0) + 9.0, y / self.al(55.0) - 3.0);
        let surface = surface + crown;
        if g.calving {
            let body = 1.0 - smoothstep(half, half + self.al(60.0), d);
            let ice = 1.0 - smoothstep(half - self.al(40.0), half + self.al(21.0), d);
            return Some((surface, body, ice, 0.0));
        }
        let body = 1.0 - smoothstep(half, half + self.al(150.0), d);
        // The head fades into the snowfield that feeds it.
        let head = smoothstep(0.0, self.al(300.0), along);
        let ice = (1.0 - smoothstep(half - self.al(4.0), half + self.al(20.0), d)) * head;
        let slab = (1.0 - smoothstep(half - self.al(14.0), half, d)) * smoothstep(self.al(120.0), self.al(400.0), along);
        Some((surface, body, ice, slab))
    }

    /// Lays the glaciers: a drawn one as drawn, a land one traced downhill
    /// from its head over the eroded ground (so it fills the valley it
    /// finds), stopping well back from any lane. Its ice is the valley floor
    /// plus a thickness, and never rises downstream.
    fn lay_glaciers(&mut self) {
        let mut flows = Vec::new();
        for g in self.design().glaciers {
            let design: Vec<(f64, f64)> = g.line.iter().map(|&(x, y, _)| self.ap((x, y))).collect();
            let (h0, h1) = (self.al(g.line[0].2), self.al(g.line[g.line.len() - 1].2));
            // Traced ones run as tongues, narrower than the drawn line's widths.
            let (h0, h1) = if g.calving { (h0, h1) } else { (0.72 * h0, 0.72 * h1) };
            if g.calving {
                let mut run = vec![0.0];
                for w in design.windows(2) {
                    run.push(run[run.len() - 1] + dist(w[0], w[1]));
                }
                let len = run[run.len() - 1];
                let surface = run.iter().map(|r| g.head + (g.snout - g.head) * (r / len).powf(0.8)).collect();
                let half = g.line.iter().map(|l| self.al(l.2)).collect();
                flows.push(IceFlow::new(design, half, surface, true, self.al(80.0)));
                continue;
            }
            let design_len: f64 = design.windows(2).map(|w| dist(w[0], w[1])).sum();
            let most = 1.5 * design_len + self.al(300.0);
            let step = self.al(28.0);
            let e = self.al(70.0);
            let z = |q: (f64, f64)| self.natural(q.0, q.1);
            let mut p = design[0];
            let (dx, dy) = (design[1].0 - p.0, design[1].1 - p.1);
            let dl = (dx * dx + dy * dy).sqrt();
            let mut dir = (dx / dl, dy / dl);
            let mut pts = vec![p];
            let mut run = 0.0;
            while run < most {
                let gx = (z((p.0 + e, p.1)) - z((p.0 - e, p.1))) / (2.0 * e);
                let gy = (z((p.0, p.1 + e)) - z((p.0, p.1 - e))) / (2.0 * e);
                let fall = (gx * gx + gy * gy).sqrt();
                if fall > 0.02 {
                    let (ux, uy) = (dir.0 * 0.55 - gx / fall * 0.45, dir.1 * 0.55 - gy / fall * 0.45);
                    let ul = (ux * ux + uy * uy).sqrt().max(1e-6);
                    dir = (ux / ul, uy / ul);
                }
                let next = (p.0 + dir.0 * step, p.1 + dir.1 * step);
                let t = run / most;
                let half_here = h0 + (h1 - h0) * t;
                let (_, reach) = self.alpine_open(next.0, next.1);
                let off = next.0 < -self.al(600.0)
                    || next.1 < -self.al(600.0)
                    || next.0 > self.size_x + self.al(600.0)
                    || next.1 > self.size_y + self.al(600.0);
                if off || reach < self.al(430.0) + 0.5 * half_here || z(next) < 30.0 {
                    break;
                }
                p = next;
                pts.push(p);
                run += step;
            }
            if pts.len() < 6 {
                continue;
            }
            let n = pts.len();
            let bed: Vec<f64> = pts.iter().map(|&q| z(q)).collect();
            let mut surface = Vec::with_capacity(n);
            let mut half = Vec::with_capacity(n);
            for i in 0..n {
                let (a, b) = (i.saturating_sub(10), (i + 10).min(n - 1));
                let floor = bed[a..=b].iter().sum::<f64>() / (b - a + 1) as f64;
                let t = i as f64 / (n - 1) as f64;
                let mut s = floor + 55.0 - 23.0 * t;
                // Always falling a little: ice that flows is never level.
                if let Some(&before) = surface.last() {
                    s = f64::min(s, before - 0.035 * step);
                }
                surface.push(s);
                half.push(h0 + (h1 - h0) * t);
            }
            flows.push(IceFlow::new(pts, half, surface, false, self.al(170.0)));
        }
        self.glaciers = IceFlows { flows };
    }

    /// The ice caps' dome height and cover (the highest cap wins).
    fn ice_cap_at(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let mut best = (0.0, 0.0, 0.0);
        for &(cx, cy, r, top) in self.design().ice_caps {
            let c = self.ap((cx, cy));
            let (vx, vy) = (x - c.0, y - c.1);
            let d = (vx * vx + vy * vy).sqrt();
            let r0 = self.al(r);
            if d > 1.3 * r0 {
                continue;
            }
            let a = vy.atan2(vx);
            let r = r0 * (1.0 + 0.14 * self.lake_shore.fbm(a.cos() * 1.5 + cx / 997.0, a.sin() * 1.5, 2, 0.5));
            let surface = top + 45.0 * (1.0 - (d / r).powi(2)).max(0.0);
            // Rock peaks stand up through the ice here and there.
            let l = self.al(520.0);
            let nunatak = smoothstep(0.62, 0.8, self.mtn.ridged(x / l + 5.5, y / l - 2.5, 3, 0.5))
                * smoothstep(0.3 * r, 0.6 * r, d);
            let body = (1.0 - smoothstep(0.8 * r, 1.15 * r, d)) * (1.0 - nunatak);
            let ice = (1.0 - smoothstep(0.86 * r, r, d)) * (1.0 - smoothstep(0.2, 0.6, nunatak));
            if body > best.1 {
                best = (surface, body, ice);
            }
        }
        best
    }

    /// Glacier ice over `(x, y)`, 0 to 1.
    pub(super) fn alpine_ice(&self, x: f64, y: f64) -> f64 {
        let mut ice = self.ice_cap_at(x, y).2;
        for g in &self.glaciers.flows {
            if let Some((_, _, i, _)) = self.glacier_at(g, x, y) {
                ice = ice.max(i);
            }
        }
        ice
    }

    /// Glacier ice and lying snow at a point of height `h` with slope `(gx, gy)`.
    pub(super) fn alpine_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        let ice = if h > 1.0 { self.alpine_ice(x, y) } else { 0.0 };
        let slope = (gx * gx + gy * gy).sqrt();
        // Slopes that face north (downhill toward +y) keep their snow lower.
        let north = (-gy / slope.max(1e-3)) * smoothstep(0.05, 0.3, slope);
        let l = 1_400.0;
        let line = 400.0 + 60.0 * self.mtn_mask.get(x / l, y / l) - 110.0 * north;
        let snow = smoothstep(line - 50.0, line + 90.0, h) * (1.0 - smoothstep(0.4, 0.75, slope));
        (ice, snow)
    }

    /// Alpine woods: thick in the valleys, conifer almost everywhere, under a
    /// tree line; none on the ice. Not mirrored.
    pub(super) fn alpine_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        // Woods are timber, and timber is income: on and near the floors
        // units walk, the north's woods are the south's copied across; up
        // the mountainsides each side grows its own.
        let (_, reach) = self.alpine_open(x, y);
        let alike = 1.0 - smoothstep(self.al(100.0), self.al(400.0), reach);
        let q = if self.south_of_middle(y) >= 0.0 { (x, y) } else { self.mirror((x, y)) };
        let field = |x: f64, y: f64| {
            (
                self.forest.fbm(x / l, y / l, 3, 0.5),
                self.forest.fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5),
                self.forest.fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5),
            )
        };
        let (own, copy) = (field(x, y), if alike > 0.0 { field(q.0, q.1) } else { (0.0, 0.0, 0.0) });
        let mix = |a: f64, b: f64| a + (b - a) * alike;
        let (broad, copse, clearing) = (mix(own.0, copy.0), mix(own.1, copy.1), mix(own.2, copy.2));
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(0.42, 0.62, copse);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let tree_line = 310.0 + 50.0 * self.forest_kind.get(x / 900.0, y / 900.0);
        // Conifers climb well up the mountainsides, thinning as the ground steepens.
        let mut habitable = smoothstep(1.5, 6.0, height)
            * (1.0 - smoothstep(0.45, 0.95, slope))
            * (1.0 - smoothstep(tree_line - 60.0, tree_line, height));
        if habitable > 0.0 {
            habitable *= 1.0 - smoothstep(0.0, 0.15, self.alpine_ice(x, y));
        }
        // A glade round every ore field, the same on both sides: the woods
        // are not mirrored, but where to build a mine must be.
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            if d < self.al(200.0) {
                habitable *= smoothstep(self.al(70.0), self.al(200.0), d);
            }
        }
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable;
        // Birch and alder only low down by the water, in patches.
        let cold = 0.35
            + smoothstep(20.0, 90.0, height) * 0.8
            + self.forest_kind.fbm(x / 1_500.0, y / 1_500.0, 2, 0.5) * 0.7
            + self.forest_kind.fbm(x / 380.0 + 41.0, y / 380.0 - 13.0, 2, 0.5) * 0.4;
        (density, smoothstep(-0.15, 0.35, cold))
    }

    /// The alpine landscape before pads, metres above the water.
    pub(super) fn natural_alpine(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let pm = self.mirror(p);
        let (open, reach) = self.alpine_open(x, y);
        let m = 1.0 - open;
        // Player 1's side is player 0's copied across, blended only along
        // the middle line: 1 on the south side, 0 on the north.
        let south = smoothstep(-self.al(120.0), self.al(120.0), self.south_of_middle(y));

        let floor = 14.0
            + self.alpine_floor_near(p).max(self.alpine_floor_near(pm))
            + 2.5 * (self.tilt.get(x / 1_800.0, y / 1_800.0) + self.tilt.get(pm.0 / 1_800.0, pm.1 / 1_800.0));

        // Mountains wherever the ground is not open, taller deeper in. Not
        // mirrored, but at their foot: every massif has its own height and crags.
        let detail = self.detail.fbm(x / 350.0, y / 350.0, 3, 0.45);
        let mut h = floor;
        if m > 0.0 {
            // Buttresses, gullies and crags from the wall's foot up.
            let craggy = smoothstep(self.al(30.0), self.al(260.0), reach);
            let rock = |q: (f64, f64)| {
                let crag = self.crag.ridged(q.0 / 380.0, q.1 / 380.0, 4, 0.5);
                230.0
                    + 70.0 * self.mtn_height.get(q.0 / 2_600.0, q.1 / 2_600.0)
                    + 130.0 * (crag - 0.35) * craggy
                    + 9.0 * self.detail.fbm(q.0 / 150.0, q.1 / 150.0, 3, 0.5)
            };
            let own = smoothstep(self.al(MIRRORED_FOOT.0), self.al(MIRRORED_FOOT.1), reach);
            let rock = if own < 1.0 {
                0.5 * (rock(p) + rock(pm)) * (1.0 - own) + rock(p) * own
            } else {
                rock(p)
            };
            let tall = rock
                + 380.0 * smoothstep(0.0, self.al(1_300.0), reach)
                // Arêtes and the hollows between them, deeper in.
                + 210.0
                    * (self.mtn.ridged(x / 1_100.0, y / 1_100.0, 3, 0.5) - 0.45)
                    * smoothstep(self.al(320.0), self.al(800.0), reach);
            h += m * tall;
        }
        // The valley floors: rising toward the walls (a glacier-cut U),
        // rolling, strewn with drumlins and the odd knoll.
        let calm = self.pad_calm_alpine(x, y);
        let u = smoothstep(-self.al(420.0), 0.0, reach).powf(1.6);
        let (rough, gentle) = {
            let (a, b) = (self.floor_relief(p), self.floor_relief(pm));
            (a.0 * south + b.0 * (1.0 - south), a.1 * south + b.1 * (1.0 - south))
        };
        h += open * (24.0 * u + calm * rough + gentle);

        // Out of reach of every lane: the ice caps, horns and glaciers.
        let aloof = smoothstep(self.al(120.0), self.al(380.0), reach);
        if reach > -self.al(10.0) {
            for &(hx, hy, top, r) in self.design().horns {
                let c = self.ap((hx, hy));
                let (vx, vy) = (x - c.0, y - c.1);
                let d = (vx * vx + vy * vy).sqrt();
                let r = self.al(r);
                if d < r {
                    // Three arêtes run off each horn.
                    let a = vy.atan2(vx) + hx * 0.001;
                    let arete = 0.75 + 0.25 * (1.5 * a).cos().abs().powf(4.0);
                    let peak = floor + (top - floor) * (1.0 - d / r).powf(1.4) * arete;
                    h = h.max(h + (peak - h) * aloof);
                }
            }
            // A band of cliff runs round every lane a little way up its wall,
            // wandering alike on both sides: whatever the water leaves below
            // it, nothing walks up past it onto the massif.
            let (lb, sb) = (self.al(520.0), self.al(170.0));
            let sym = |f: &dyn Fn(f64, f64) -> f64| 0.5 * (f(x, y) + f(pm.0, pm.1));
            let wander = sym(&|x, y| {
                self.mtn_gap.fbm(x / lb + 1.7, y / lb, 2, 0.5) + 0.6 * self.mtn_gap.get(x / sb - 4.1, y / sb + 2.2)
            });
            let tall_band = sym(&|x, y| self.mtn_mask.fbm(x / (1.3 * lb) + 7.3, y / (1.3 * lb), 2, 0.5));
            let band_at = self.al(CLIFF_BAND.0 + CLIFF_BAND.1 * wander);
            let band_h = CLIFF_BAND.2 * (1.0 + 0.55 * tall_band);
            let near_band = bump((reach - band_at) / self.al(45.0));
            // Gullies and ridges cut by water. Near the lanes the north side
            // wears exactly as the south did (the change copied across), so
            // the gullies stay crisp; well away each wears its own way.
            let worn = smoothstep(self.al(-10.0), self.al(90.0), reach) * (1.0 - 0.25 * near_band);
            if worn > 0.0 {
                let own = smoothstep(self.al(160.0), self.al(400.0), reach);
                let here = self.erosion.at(x, y);
                let alike = if own < 1.0 {
                    here * south + self.erosion.at(pm.0, pm.1) * (1.0 - south)
                } else {
                    here
                };
                h += (alike + (here - alike) * own) * worn;
            }
            h += band_h * smoothstep(band_at - self.al(14.0), band_at + self.al(14.0), reach);
            // Ice lies over what the water cut.
            let (cap, cap_body, _) = self.ice_cap_at(x, y);
            h += (cap - h) * cap_body * aloof;
            // Every glacier cuts its trough first; then all their ice lies as
            // one body, so where two meet they run together and walls stand
            // only at its outside edge.
            let (mut slabs, mut lying, mut most): (f64, f64, f64) = (0.0, 0.0, 0.0);
            let wall_h = ICE_WALL * (0.55 + 0.9 * (0.5 + 0.5 * self.detail.get(x / 170.0 + 2.0, y / 170.0)));
            for g in self.glaciers.flows.iter().filter(|g| !g.calving) {
                let Some((surface, body, _, slab)) = self.glacier_at(g, x, y) else { continue };
                // Where an outlet leaves a cap its ice is the cap's.
                let surface = surface + (cap - surface) * cap_body;
                let bed = surface - wall_h * (1.0 - cap_body);
                if bed < h {
                    h += (bed - h) * body * aloof * 0.7;
                }
                slabs += slab;
                lying += slab * surface;
                most = most.max(slab);
            }
            if most > 0.0 {
                let surface = lying / slabs;
                if surface > h {
                    h += (surface - h) * most * aloof;
                }
            }
        }
        h = under_ceiling(h);

        // The sea: steep walls where mountains meet it, a beach where a lane does.
        let shore = self.alpine_sea_near(p).min(self.alpine_sea_near(pm)) + self.coast_far(x, y, reach);
        let sea = -2.0 - DEEP * smoothstep(0.0, self.al(300.0), -shore);
        // Where open ground meets the sea it runs down to it over a broad
        // beach; mountains meet it in cliffs.
        let beach = self.al(380.0 + 520.0 * open);
        h -= open * 10.0 * (1.0 - smoothstep(0.0, self.al(900.0), shore));
        h = if shore <= 0.0 {
            sea + 0.6 * detail
        } else {
            sea + (h - sea) * smoothstep(0.0, beach, shore).powf(0.7)
        };

        // Tidewater glaciers end over the water in an ice cliff.
        for g in self.glaciers.flows.iter().filter(|g| g.calving) {
            if let Some((surface, body, _, _)) = self.glacier_at(g, x, y) {
                h += (surface - h) * body * aloof;
            }
        }
        h
    }

    /// 0 near the pads and ore fields (keep them level), 1 elsewhere.
    fn pad_calm_alpine(&self, x: f64, y: f64) -> f64 {
        let mut calm: f64 = 1.0;
        for pad in &self.pads {
            let d = dist((x, y), (pad.x, pad.y));
            calm = calm.min(smoothstep(pad.core, pad.outer + self.al(200.0), d));
        }
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            calm = calm.min(smoothstep(self.al(20.0), self.al(170.0), d));
        }
        calm
    }

    /// Relief on an open floor at `q`, in that copy's own frame: the rough
    /// part (hummocks, knolls, swells; kept off pads and ore) and the gentle
    /// part everywhere.
    fn floor_relief(&self, q: (f64, f64)) -> (f64, f64) {
        let (x, y) = q;
        let swell = 7.0 * self.lake.fbm(x / 620.0, y / 620.0, 3, 0.5);
        // Drumlins: rounded, drawn out along the old ice's flow.
        let (u, v) = (0.8 * x + 0.6 * y, -0.6 * x + 0.8 * y);
        let hummock = 7.0 * self.crag.fbm(u / 380.0 + 11.0, v / 170.0 + 5.0, 2, 0.45);
        // Knolls: round rises here and there, not ridges.
        let knoll = 10.0 * smoothstep(0.3, 0.75, self.ridge.fbm(x / 230.0 - 3.0, y / 230.0 - 9.0, 2, 0.5));
        let gentle = 1.6 * self.detail.fbm(x / 90.0, y / 90.0, 2, 0.5)
            + 3.0 * self.lake.fbm(x / 1_400.0 + 4.0, y / 1_400.0, 2, 0.5);
        (swell + hummock + knoll, gentle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BakeParams;

    fn terrains() -> [Terrain; 2] {
        [
            Terrain::new(&BakeParams::alpine("Serac Divide", 4, 3)),
            Terrain::new(&BakeParams::alpine_teams("Serac Sound", 6, 5)),
        ]
    }

    #[test]
    fn nothing_unmirrored_comes_near_a_lane() {
        let mut problems = Vec::new();
        for t in terrains() {
            let name = format!("{:?}", t.layout);
            let mut check = |what: String, x: f64, y: f64, keep: f64| {
                let (_, reach) = t.alpine_open(x, y);
                if reach < keep {
                    problems.push(format!("{name}: {what} at ({x:.0}, {y:.0}) is {reach:.0} m from open ground"));
                }
            };
            for (i, g) in t.glaciers.flows.iter().enumerate() {
                // Round the glacier's outline, at every point of its line.
                for (k, &(x, y)) in g.pts.iter().enumerate() {
                    for j in 0..8 {
                        let ang = j as f64 * PI / 4.0;
                        let r = g.half[k] * 1.2;
                        let (px, py) = (x + r * ang.cos(), y + r * ang.sin());
                        if px > 0.0 && py > 0.0 && px < t.size_x && py < t.size_y {
                            // Ice down near the water must keep well away;
                            // high ice may come closer, over a rock step.
                            check(format!("glacier {i}"), px, py, if g.calving { 250.0 } else { 60.0 });
                        }
                    }
                }
            }
            for &(cx, cy, r, _) in t.design().ice_caps {
                let c = t.ap((cx, cy));
                for j in 0..24 {
                    let a = j as f64 * PI / 12.0;
                    let r = t.al(r) * 1.05;
                    check("ice cap".into(), c.0 + r * a.cos(), c.1 + r * a.sin(), 0.0);
                }
            }
            for (i, &(x, y, _, _)) in t.design().horns.iter().enumerate() {
                let p = t.ap((x, y));
                check(format!("horn {i}"), p.0, p.1, 450.0);
            }
            // Every design glacier made it onto the map.
            if t.glaciers.flows.len() != t.design().glaciers.len() {
                problems.push(format!("{name}: {} of {} glaciers laid", t.glaciers.flows.len(), t.design().glaciers.len()));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// `ALPINE_RELIEF=x0,y0,metres,out.ppm cargo test --release -p mc-map --lib
    /// alpine_relief -- --ignored`: a hillshade of the landscape at 8 m, with
    /// ice blue and snow white, for looking at the mountains' shapes.
    #[test]
    #[ignore]
    fn alpine_relief() {
        let spec = std::env::var("ALPINE_RELIEF").unwrap_or_else(|_| "0,0,12288,relief.ppm".into());
        let parts: Vec<&str> = spec.split(',').collect();
        let (x0, y0, span): (f64, f64, f64) = (parts[0].parse().unwrap(), parts[1].parse().unwrap(), parts[2].parse().unwrap());
        let t = &terrains()[std::env::var("ALPINE_MAP").map_or(0, |m| (m == "teams") as usize)];
        let px = 1024usize;
        let m = span / px as f64;
        let z: Vec<f64> = (0..(px + 1) * (px + 1))
            .map(|i| t.height(x0 + (i % (px + 1)) as f64 * m, y0 + (i / (px + 1)) as f64 * m))
            .collect();
        let mut out = format!("P6\n{px} {px}\n255\n").into_bytes();
        for row in (0..px).rev() {
            for col in 0..px {
                let at = row * (px + 1) + col;
                let (h, gx, gy) = (z[at], (z[at + 1] - z[at]) / m, (z[at + px + 1] - z[at]) / m);
                let (x, y) = (x0 + col as f64 * m, y0 + row as f64 * m);
                let (ice, snow) = t.alpine_snow(x, y, h, gx, gy);
                let base = if h <= 0.0 { [40.0, 90.0, 140.0] } else if h < 120.0 { [110.0, 140.0, 90.0] } else { [140.0, 130.0, 120.0] };
                let base = [0, 1, 2].map(|c| base[c] + ([240.0, 244.0, 250.0][c] - base[c]) * snow);
                let base = [0, 1, 2].map(|c| base[c] + ([175.0, 215.0, 240.0][c] - base[c]) * ice);
                let shade = (0.75 + 1.4 * (-gx * 0.6 + gy * 0.6)).clamp(0.2, 1.6);
                out.extend(base.map(|c| (c * shade).clamp(0.0, 255.0) as u8));
            }
        }
        std::fs::write(parts[3], out).unwrap();
    }

    #[test]
    fn erosion_cuts_and_fills() {
        for t in terrains() {
            let d = &t.erosion.delta;
            let (lo, hi) = d.iter().fold((0f32, 0f32), |(a, b), &v| (a.min(v), b.max(v)));
            println!("{:?} erosion: {lo:.1} m to {hi:.1} m", t.layout);
            assert!(lo < -10.0 && hi > 3.0);
        }
    }

    #[test]
    fn open_ground_is_the_same_mirrored() {
        for t in terrains() {
            let mut worst: f64 = 0.0;
            for j in 0..60 {
                for i in 0..60 {
                    let (x, y) = ((i as f64 + 0.37) * t.size_x / 60.0, (j as f64 + 0.61) * t.size_y / 60.0);
                    let (o1, r1) = t.alpine_open(x, y);
                    let (o2, r2) = t.alpine_open(x, t.size_y - y);
                    if r1.min(r2) < 0.0 {
                        worst = worst.max((o1 - o2).abs());
                    }
                }
            }
            assert!(worst < 1e-9, "{:?}: open ground differs by {worst} between the halves", t.layout);
        }
    }
}
