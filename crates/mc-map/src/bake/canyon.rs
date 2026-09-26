//! "Vermilion Gorge": three against three across a desert canyon, a drowned
//! reservoir filling its middle and a colossal gravity dam across a narrows
//! at its south end, an homage to the Three Gorges.
//!
//! [`Layout::Canyon`](super::Layout::Canyon), 12 km. The canyon runs north to
//! south down the middle of the map; each side's three bases stand on its
//! rim plateau. The rim drops to a broad bench over a Grand Canyon profile
//! of cliffs and slopes, the bench to the lake over the inner gorge, whose
//! walls carry the white ring of the lake's old, higher shore: the lake has
//! fallen to dead pool, below the dam's outlets. Drowned side canyons reach
//! into the bench, the rim is cut back in amphitheatres, buttes and temples
//! stand off the walls and a temple of rock stands out of the lake.
//!
//! At the south the rims pinch into a narrows and the dam closes it, 230 m
//! over the lake; below it the river's bed lies dry, a broad valley at the
//! bench's height with a wash down its middle. Land crosses the canyon
//! there, below the dam, and at the north over the river's delta flats.
//! Nothing crosses the dam. Trails lead from the rim down through breaks in
//! the walls.
//!
//! Heights above the water level, shared with the Desert palette in
//! `mc-render/shaders/desert.wgsl` (`CANYON_*`), which colours the beds by
//! height: the old full-pool line [`RING_TOP`]; the bench's low edge at the
//! inner gorge's rim, [`GORGE_RIM`]; the foot of the big cliff (the
//! "Redwall"), [`BENCH_TOP`]; and above it [`REDWALL_TOP`], [`SUPAI_TOP`],
//! [`HERMIT_TOP`], [`COCONINO_TOP`] and the rim, [`RIM`].
//!
//! Fairness: effectively the same both sides, not a reflection. The west
//! side is designed in `(u, v)`: `u` metres out from the middle line, `v`
//! metres north. The east side is the same design displaced along the canyon
//! ([`east_of`]: its features stand up to some 180 m north or south of the
//! west's and are shaped differently), except at the narrows and the ford,
//! which both sides share. All the noise is each side's own. What plays is
//! then evened out by measure: each side's plateau and bench are brought to
//! the same area ([`Terrain::even_out_sides`]), and `tests/vermilion_gorge.rs`
//! holds the walks, the level ground, the sea and the timber to parity.

use super::bays::segment;
use super::machine::PrecursorSite;
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::landmark::GORGE_DAM;
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;

mod shape;

/// Top of the white ring: the lake's old full-pool line.
pub(super) const RING_TOP: f64 = GORGE_DAM.ring_top;
/// The inner gorge's rim, where the bench begins; the dry riverbed below the dam.
pub(super) const GORGE_RIM: f64 = GORGE_DAM.floor_z;
/// Foot of the big cliff, the top of the bench.
pub(super) const BENCH_TOP: f64 = 100.0;
pub(super) const REDWALL_TOP: f64 = 190.0;
pub(super) const SUPAI_TOP: f64 = 275.0;
pub(super) const HERMIT_TOP: f64 = 300.0;
pub(super) const COCONINO_TOP: f64 = 340.0;
/// The rim's edge; the plateau rises gently behind it.
pub(super) const RIM: f64 = 370.0;

/// The rim's edge, west side, south to north (u, v): the canyon lies toward
/// the middle line. The south's broad dry valley, the narrows the dam
/// closes, amphitheatres cut back into the plateau and points between them.
const RIM_EDGE: &[(f64, f64)] = &[
    (2950.0, -400.0),
    (3050.0, 500.0),
    (2950.0, 1150.0),
    (2700.0, 1600.0),
    (2150.0, 1950.0),
    // The narrows.
    (1450.0, 2150.0),
    (1050.0, 2300.0),
    (1000.0, 2450.0),
    (1150.0, 2600.0),
    (1600.0, 2700.0),
    (2300.0, 2780.0),
    (3000.0, 2800.0),
    // The south amphitheatre.
    (3700.0, 2950.0),
    (4150.0, 3200.0),
    (4150.0, 3500.0),
    (3600.0, 3650.0),
    (3150.0, 3800.0),
    // The south point.
    (2850.0, 4200.0),
    (2700.0, 4500.0),
    (2950.0, 4800.0),
    (3400.0, 5100.0),
    (3850.0, 5450.0),
    (3900.0, 5700.0),
    (3450.0, 5950.0),
    (3300.0, 6250.0),
    (3450.0, 6650.0),
    // The north point.
    (2950.0, 7000.0),
    (2700.0, 7300.0),
    (2850.0, 7650.0),
    (3300.0, 8000.0),
    // The north amphitheatre.
    (3900.0, 8250.0),
    (4300.0, 8500.0),
    (4150.0, 8850.0),
    (3550.0, 9000.0),
    (3150.0, 9400.0),
    (3000.0, 10000.0),
    (3050.0, 10700.0),
    (2900.0, 11400.0),
    (2950.0, 12700.0),
];

/// The lake's shore at the water level, west side, south to north: from
/// the dam's heel, up the narrows' gorge, round the basin and its three
/// drowned side canyons ("arms"), to the tail at the delta.
const LAKE: &[(f64, f64)] = &[
    (0.0, 2460.0),
    (230.0, 2470.0),
    (300.0, 2650.0),
    (520.0, 3000.0),
    (820.0, 3350.0),
    // The south arm.
    (1050.0, 3600.0),
    (1500.0, 3680.0),
    (1950.0, 3740.0),
    (2250.0, 3830.0),
    (2200.0, 3960.0),
    (1800.0, 3960.0),
    (1300.0, 4020.0),
    (1250.0, 4400.0),
    (1400.0, 4900.0),
    (1550.0, 5400.0),
    // The middle arm.
    (1650.0, 5880.0),
    (2100.0, 5930.0),
    (2420.0, 6040.0),
    (2380.0, 6240.0),
    (2000.0, 6300.0),
    (1650.0, 6420.0),
    (1600.0, 6900.0),
    (1450.0, 7400.0),
    // The north arm.
    (1400.0, 7780.0),
    (1850.0, 7840.0),
    (2180.0, 7950.0),
    (2130.0, 8130.0),
    (1700.0, 8200.0),
    (1300.0, 8300.0),
    (1000.0, 8800.0),
    (700.0, 9300.0),
    (450.0, 9700.0),
    (250.0, 9950.0),
    (0.0, 9960.0),
];

/// The river above the delta, west bank, from the pool it ends in to the
/// north edge.
const RIVER: &[(f64, f64)] = &[
    (0.0, 10440.0),
    (150.0, 10470.0),
    (90.0, 10800.0),
    (75.0, 11500.0),
    (85.0, 12700.0),
];

/// Where the delta's flats lie, along the middle line (v): the ford.
const DELTA: (f64, f64) = (9500.0, 10650.0);

/// The dry wash down the valley below the dam: metres east of the middle
/// line and north, from the dam's toe to the south edge. One line for both
/// sides.
const WASH: &[(f64, f64)] = &[
    (0.0, 2300.0),
    (60.0, 2050.0),
    (170.0, 1750.0),
    (40.0, 1400.0),
    (-150.0, 1050.0),
    (-60.0, 650.0),
    (120.0, 250.0),
    (60.0, -400.0),
];

/// A butte: a remnant of the plateau standing in the canyon, its top `top`
/// across (m), capped at `cap` metres in from the rim's profile (0: the rim
/// itself; more: a lower top). `shore` > 0 makes it an island in the lake
/// whose shore stands that far out.
struct Butte {
    u: f64,
    v: f64,
    top: f64,
    cap: f64,
    shore: f64,
}

/// The temple standing out of the lake on the middle line, shared.
#[rustfmt::skip]
const TEMPLE: Butte = Butte { u: 0.0, v: 6200.0, top: 110.0, cap: 90.0, shore: 520.0 };

/// Buttes on the west bench; the east's are displaced like the rest.
#[rustfmt::skip]
const BUTTES: &[Butte] = &[
    Butte { u: 2350.0, v: 5150.0, top: 70.0, cap: 360.0, shore: 0.0 },
    Butte { u: 2300.0, v: 9150.0, top: 90.0, cap: 300.0, shore: 0.0 },
];

/// A mesa island in the lake, bench-high: a ridge `half` either side of its
/// centre at `angle` radians (from +u toward +v), its shore `r` out.
struct Isle {
    u: f64,
    v: f64,
    r: f64,
    half: f64,
    angle: f64,
}

#[rustfmt::skip]
const ISLES: &[Isle] = &[
    Isle { u: 640.0, v: 4700.0, r: 130.0, half: 180.0, angle: 1.25 },
    Isle { u: 700.0, v: 7650.0, r: 130.0, half: 200.0, angle: 2.0 },
];

/// Coves: where a beach runs gently from the bench into the lake instead of
/// the gorge's wall (u, v, reach). A shipyard's way to the water.
const COVES: &[(f64, f64, f64)] = &[
    (2250.0, 3900.0, 380.0),
    (2400.0, 6140.0, 400.0),
    (2150.0, 8040.0, 380.0),
];

/// Trails from the rim down through a break in the walls: rim end first
/// (u, v). The south base's down into the dry valley and onto the lake's
/// south bench, the middle's to its cove, the north's toward the ford.
const TRAILS: &[&[(f64, f64)]] = &[
    &[(3550.0, 1350.0), (2950.0, 1250.0), (2150.0, 1050.0)],
    &[(2300.0, 2480.0), (1850.0, 2850.0), (1450.0, 3300.0)],
    &[(3700.0, 6700.0), (3150.0, 6600.0), (2450.0, 6480.0)],
    &[(3400.0, 10350.0), (2900.0, 10100.0), (2150.0, 9800.0)],
];

/// The bench's way along each side, from the lake's south bench to the
/// delta (u, v): the walls and the water are held back from it, so however
/// each side's shapes fall the bench runs unbroken.
const BENCH_WAY: &[(f64, f64)] = &[
    (1450.0, 3300.0),
    (2000.0, 3450.0),
    (2400.0, 3850.0),
    (1700.0, 4400.0),
    (1800.0, 4900.0),
    (2100.0, 5400.0),
    (2580.0, 6100.0),
    (2200.0, 6700.0),
    (1850.0, 7200.0),
    (1850.0, 7550.0),
    (2480.0, 8050.0),
    (2200.0, 8700.0),
    (1800.0, 9200.0),
    (1150.0, 9750.0),
];

/// The starts, west side (u, v). Each is followed by its mirror image: the
/// bases' plateau is mirrored, the canyon below it is not.
const STARTS: &[(f64, f64)] = &[(4700.0, 1650.0), (4850.0, 6150.0), (4750.0, 10250.0)];

/// Ore on the rim plateau, west side: between the bases, and out on the
/// narrows' shoulder. Mirrored, like the bases.
const RIM_ORE: &[(f64, f64, f64)] = &[
    (5100.0, 4150.0, 80.0),
    (5100.0, 8150.0, 80.0),
    (2300.0, 2350.0, 80.0),
];

/// Ore down in the canyon, west side: (u, v, radius). The east's stand
/// where its design is displaced to, settled onto level ground there. A
/// field on the middle line is shared.
const ORE: &[(f64, f64, f64)] = &[
    // The dry valley.
    (850.0, 1150.0, 90.0),
    // The lake's bench.
    // On the bench's way, where each side is sure to have bench.
    (1450.0, 3300.0, 85.0),
    (1750.0, 4650.0, 80.0),
    (1850.0, 7550.0, 80.0),
    (1450.0, 9480.0, 85.0),
    // The mesa islands.
    (640.0, 4700.0, 80.0),
    (700.0, 7650.0, 80.0),
    // The valley's middle, and the ford.
    (0.0, 1500.0, 110.0),
    (0.0, 10250.0, 110.0),
];

/// The dam's toe, on the middle line (v, metres north): the model's origin.
/// A multiple of the 32 m overview step, so the renderer stands the model on
/// exactly the riverbed's height.
const TOE_V: f64 = 2304.0;

/// The works at the dam's toe (`models/dam.rs`: the stilling basin, the
/// powerhouses, the ship lift) stand on level riverbed this far downstream
/// of the toe and this far either side of the middle; the dam's solid plan
/// (`PropKind::solid_plan`) covers them.
pub(super) const TOE_WORKS: (f64, f64) = (185.0, 705.0);

/// How far out past a sample the ground under the dam looks: one 8 m sample
/// diagonal and a little.
const REACH_OUT: f64 = 12.0;

/// The floor of the slot along the dam's upstream foot (`dam_ground`).
const TRENCH: f64 = -170.0;

/// Metres between the samples of the keep grid.
const KEEP_STEP: f64 = 16.0;

/// Metres between the samples of the designed distance fields.
const FIELD_STEP: f64 = 8.0;

/// The east side's design: the west's point displaced along the canyon (and
/// a little in and out), smoothly, by up to some 180 m, and not at all at
/// the narrows or the ford, which the sides share. Monotonic in `v`, so an
/// outline keeps its order.
fn east_of((u, v): (f64, f64)) -> (f64, f64) {
    let w = smoothstep(2_900.0, 3_600.0, v) * (1.0 - smoothstep(9_100.0, 9_700.0, v));
    let dv = 120.0 * (v / 830.0 + 1.1).sin() + 60.0 * (v / 310.0 + 0.3).sin();
    let du = 0.05 * (v / 650.0 + 2.0).sin();
    (u * (1.0 + w * du), v + w * dv)
}

/// A signed distance to a designed outline, on a grid over the map: positive
/// inside.
#[derive(Default)]
pub(super) struct Outline {
    nx: usize,
    ny: usize,
    d: Vec<f32>,
}

impl Outline {
    fn build(poly: &[(f64, f64)], width: f64, height: f64) -> Outline {
        let smooth = smooth_closed(poly, 8);
        // Exact on a coarse grid, then exact on the fine one only near the
        // outline: far from it the coarse field, read bicubic, is as good.
        let coarse = Self::sample(&smooth, width, height, 4.0 * FIELD_STEP, None);
        Self::sample(&smooth, width, height, FIELD_STEP, Some(&coarse)).0
    }

    fn sample(
        poly: &[(f64, f64)],
        width: f64,
        height: f64,
        step: f64,
        coarse: Option<&(Outline, f64)>,
    ) -> (Outline, f64) {
        let (nx, ny) = ((width / step) as usize + 2, (height / step) as usize + 2);
        let mut d = vec![0f32; nx * ny];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = ny.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in d.chunks_mut(rows * nx).enumerate() {
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(nx).enumerate() {
                        let y = (k * rows + r) as f64 * step;
                        for (i, out) in row.iter_mut().enumerate() {
                            let x = i as f64 * step;
                            let far = coarse.map(|(c, cs)| bicubic(&c.d, c.nx, c.ny, *cs, x, y));
                            *out = match far {
                                Some(f) if f.abs() > 400.0 => f,
                                _ => signed((x, y), poly),
                            } as f32;
                        }
                    }
                });
            }
        });
        (Outline { nx, ny, d }, step)
    }

    /// Bicubic, so cliffs laid off it have no creases at the samples.
    fn at(&self, x: f64, y: f64) -> f64 {
        bicubic(&self.d, self.nx, self.ny, FIELD_STEP, x, y)
    }
}

/// A trail as laid: its centreline (world), its surface's height at every
/// `TRAIL_STEP` along it, and its width.
struct Trail {
    line: Vec<(f64, f64)>,
    profile: Vec<f64>,
    /// Half width at the middle; it swells and narrows along the way.
    half: f64,
}

/// Metres between a trail's profile samples.
const TRAIL_STEP: f64 = 8.0;
/// The steepest a trail runs, rise over run: the sim's limit is 1/2.
const TRAIL_GRADE: f64 = 0.36;

/// The canyon as laid out at set-up: both sides' outlines, trails, buttes,
/// isles and coves in world metres, and how each side is evened out.
#[derive(Default)]
pub(super) struct Canyon {
    rim: Outline,
    lake: Outline,
    river: Outline,
    trails: Vec<Trail>,
    /// Buttes, isles (as ridge ends and shore radius) and coves, both sides.
    buttes: Vec<((f64, f64), f64, f64)>,
    isles: Vec<IsleLaid>,
    coves: Vec<((f64, f64), f64)>,
    /// Temples: buttes and spurs standing off the walls out in open country.
    temples: Vec<Temple>,
    /// `canyon_keep` on a grid (`KEEP_STEP`), laid once the trails are.
    keep: Vec<f32>,
    /// Metres the east's rim is moved out (and the west's in), and its open
    /// shore, so both sides hold the same plateau and bench (`even_out_sides`).
    rim_bias: f64,
    shore_bias: f64,
    /// The bench's way along each side (`BENCH_WAY`).
    ways: Vec<Vec<(f64, f64)>>,
    /// Each side's woods thinned so both hold the same timber (west, east;
    /// `even_out_woods`).
    wood: [f64; 2],
}

/// An isle as laid: its ridge's two ends and its shore's reach (world).
type IsleLaid = ((f64, f64), (f64, f64), f64);

/// A temple: a ridge of rock from `a` to `b` (world metres), its top `top`
/// either side of that line, capped `cap` metres in from the rim's profile.
struct Temple {
    a: (f64, f64),
    b: (f64, f64),
    top: f64,
    cap: f64,
}

/// A grid of samples `step` apart, `nx` by `ny`, read at `(x, y)` through a
/// Catmull-Rom cubic each way; clamped at the edges.
fn bicubic(d: &[f32], nx: usize, ny: usize, step: f64, x: f64, y: f64) -> f64 {
    let (gx, gy) = (
        (x / step).clamp(0.0, (nx - 1) as f64),
        (y / step).clamp(0.0, (ny - 1) as f64),
    );
    let (i, j) = ((gx as usize).min(nx - 2), (gy as usize).min(ny - 2));
    let (fx, fy) = (gx - i as f64, gy - j as f64);
    let at = |di: isize, dj: isize| {
        let ii = (i as isize + di).clamp(0, nx as isize - 1) as usize;
        let jj = (j as isize + dj).clamp(0, ny as isize - 1) as usize;
        d[jj * nx + ii] as f64
    };
    let cubic = |a: f64, b: f64, c: f64, e: f64, t: f64| {
        b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - e + t * (3.0 * (b - c) + e - a)))
    };
    let row = |dj: isize| cubic(at(-1, dj), at(0, dj), at(1, dj), at(2, dj), fx);
    cubic(row(-1), row(0), row(1), row(2), fy)
}

/// A closed outline rounded into a curve (Catmull-Rom, `per` points a span).
fn smooth_closed(poly: &[(f64, f64)], per: usize) -> Vec<(f64, f64)> {
    let n = poly.len() as isize;
    let at = |i: isize| poly[i.rem_euclid(n) as usize];
    let mut out = Vec::new();
    for i in 0..n {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..per {
            out.push(catmull(p0, p1, p2, p3, k as f64 / per as f64));
        }
    }
    out
}

/// An open line rounded into a curve through its points.
fn smooth_open(line: &[(f64, f64)], per: usize) -> Vec<(f64, f64)> {
    let n = line.len() as isize;
    let at = |i: isize| line[i.clamp(0, n - 1) as usize];
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..per {
            out.push(catmull(p0, p1, p2, p3, k as f64 / per as f64));
        }
    }
    out.push(line[line.len() - 1]);
    out
}

fn catmull(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), t: f64) -> (f64, f64) {
    let c = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * (2.0 * b
            + (c - a) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
            + (3.0 * b - a - 3.0 * c + d) * t * t * t)
    };
    (c(p0.0, p1.0, p2.0, p3.0), c(p0.1, p1.1, p2.1, p3.1))
}

/// Signed distance to a closed outline: positive inside.
fn signed(p: (f64, f64), poly: &[(f64, f64)]) -> f64 {
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

/// Distance to an open line, how far along it (metres), and its length.
fn along(p: (f64, f64), line: &[(f64, f64)]) -> (f64, f64, f64) {
    let (mut best, mut at, mut run) = (f64::INFINITY, 0.0, 0.0);
    for w in line.windows(2) {
        let len = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        let (d, t) = segment(p, w[0], w[1]);
        if d < best {
            (best, at) = (d, run + t * len);
        }
        run += len;
    }
    (best, at, run)
}

/// A cliff's face: steep at the top, easing into a talus foot.
fn cliff(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powf(2.2)
}

impl Terrain {
    /// Whether nothing may grow or lie here: the dam and the works at its toe.
    pub(in crate::bake) fn canyon_clear(&self, x: f64, y: f64) -> bool {
        let (mx, my) = (y - TOE_V, (x - self.size_x / 2.0).abs());
        !((-TOE_WORKS.0 - 30.0..=GORGE_DAM.base + 20.0).contains(&mx)
            && my <= GORGE_DAM.length / 2.0 + GORGE_DAM.key)
    }

    /// A west-side designed point in the world.
    fn west(&self, (u, v): (f64, f64)) -> (f64, f64) {
        (self.size_x / 2.0 - u, v)
    }

    /// The east side's twin of a west-side designed point, in the world.
    fn east(&self, p: (f64, f64)) -> (f64, f64) {
        let (u, v) = east_of(p);
        (self.size_x / 2.0 + u, v)
    }

    /// A designed point on both sides: west, then east.
    fn both_sides(&self, p: (f64, f64)) -> [(f64, f64); 2] {
        [self.west(p), self.east(p)]
    }

    /// A designed point on the plateau on both sides, mirrored: west, then east.
    fn mirror_sides(&self, (u, v): (f64, f64)) -> [(f64, f64); 2] {
        [self.west((u, v)), (self.size_x / 2.0 + u, v)]
    }

    /// A point near `p` on level ground at about `want` metres: where a field
    /// is laid.
    fn settle(&self, p: (f64, f64), want: f64) -> (f64, f64) {
        let mut best = (f64::INFINITY, p);
        for j in -15i32..=15 {
            for i in -15i32..=15 {
                let q = (p.0 + i as f64 * 24.0, p.1 + j as f64 * 24.0);
                let r = (i as f64).hypot(j as f64) * 24.0;
                if r > 360.0 {
                    continue;
                }
                let steep = [
                    (0.0, 0.0),
                    (60.0, 0.0),
                    (-60.0, 0.0),
                    (0.0, 60.0),
                    (0.0, -60.0),
                ]
                .iter()
                .map(|&(dx, dy)| self.slope(q.0 + dx, q.1 + dy))
                .fold(0.0, f64::max);
                let score = (self.height(q.0, q.1) - want).abs() + 600.0 * steep + 0.02 * r;
                if steep < 0.1 && score < best.0 {
                    best = (score, q);
                }
            }
        }
        best.1
    }

    /// -1 on the west side, 1 on the east, blended across the middle.
    fn side(&self, x: f64) -> f64 {
        2.0 * smoothstep(-400.0, 400.0, x - self.size_x / 2.0) - 1.0
    }

    /// An outline of the whole canyon from a west-side design: the west as
    /// drawn south to north, then the east's twin north to south, closed
    /// round the ends beyond `v0` and `v1` when given.
    fn both_outline(&self, west: &[(f64, f64)], ends: Option<(f64, f64)>) -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = west.iter().map(|&p| self.west(p)).collect();
        if let Some((_, v1)) = ends {
            out.push((0.0, v1));
            out.push((self.size_x, v1));
        }
        out.extend(west.iter().rev().map(|&p| self.east(p)));
        if let Some((v0, _)) = ends {
            out.push((self.size_x, v0));
            out.push((0.0, v0));
        }
        out
    }

    pub(super) fn natural_canyon(&self, x: f64, y: f64) -> f64 {
        let mut h = self.canyon_eroded(x, y);
        for trail in &self.canyon.trails {
            h = self.trail_ground(trail, x, y, h);
        }
        self.dam_ground(x, y, h)
    }

    /// A trail's surface: its profile along the line, a shallow trough
    /// across it, ragged edges blending into the ground either side.
    fn trail_ground(&self, trail: &Trail, x: f64, y: f64, h: f64) -> f64 {
        let reach = trail.half * 1.5 + 420.0;
        let (d, at, total) = along((x, y), &trail.line);
        if d > reach {
            return h;
        }
        let i = (at / TRAIL_STEP).min((trail.profile.len() - 1) as f64);
        let (i0, f) = (i.floor() as usize, i.fract());
        let z = trail.profile[i0] * (1.0 - f)
            + trail.profile[(i0 + 1).min(trail.profile.len() - 1)] * f;
        // Wider on the shelves, narrower through the cliffs, never even.
        let swell = self.ramp.fbm(at / 260.0 + 3.3, x / 900.0, 2, 0.5) * 2.2;
        let half = trail.half * (1.0 + swell).clamp(0.6, 1.5);
        // Ragged at its edges only: the walked middle is always the trail.
        let ragged =
            self.crag.fbm(x / 70.0, y / 70.0, 2, 0.5) * 90.0 * smoothstep(0.5 * half, half, d);
        let blend =
            70.0 + 50.0 * (self.detail.fbm(x / 400.0, y / 400.0, 2, 0.5) * 3.0).clamp(-1.0, 1.0);
        // Ends meet the ground they leave and reach.
        let ends = smoothstep(0.0, 40.0, at) * smoothstep(0.0, 40.0, total - at);
        let floor = z + 5.0 * (d / half).min(1.0).powi(2) * ends;
        // The deeper the trail cuts, the wider its sides lean out: a ravine
        // through the cliffs, not a road cut.
        let blend = blend
            + 1.3
                * (h - floor).clamp(0.0, 260.0)
                * smoothstep(BENCH_TOP + 10.0, BENCH_TOP + 60.0, h);
        let w = 1.0 - smoothstep(half, half + blend, d + ragged);
        h + (floor - h) * w
    }

    /// The dam: the ground flat at the riverbed under its footprint (inside
    /// the concrete), dropped into a trench under the water along its
    /// upstream heel, and the dry basin at its toe.
    fn dam_ground(&self, x: f64, y: f64, h: f64) -> f64 {
        let d = GORGE_DAM;
        let (cx, reach) = (self.size_x / 2.0, d.length / 2.0 + d.key);
        if (x - cx).abs() > reach + 200.0 || (y - TOE_V).abs() > 450.0 {
            return h;
        }
        // Heading north: the model's x is north.
        let (mx, my) = (y - TOE_V, (x - cx).abs());
        let inner = d.length / 2.0 - 20.0;
        // Level under the dam, and below its toe under the stilling basin,
        // the powerhouses and the ship lift (`models/dam.rs`).
        if (my <= inner && (0.0..=d.base).contains(&mx))
            || (my <= TOE_WORKS.1 && (-TOE_WORKS.0..=0.0).contains(&mx))
        {
            return d.floor_z;
        }
        let mut h = h;
        // A triangle from the heel's last sample down to the lake must fall
        // fast to stay behind the plumb upstream face.
        if my <= inner && mx > d.base && mx <= d.base + REACH_OUT && h < d.floor_z {
            h = TRENCH;
        }
        // Easing out from the works into the valley.
        if my <= TOE_WORKS.1 + 150.0 && mx < 0.0 {
            let w = (1.0 - smoothstep(TOE_WORKS.0, TOE_WORKS.0 + 180.0, -mx))
                * (1.0 - smoothstep(TOE_WORKS.1, TOE_WORKS.1 + 150.0, my));
            h += (d.floor_z - h) * w;
        }
        h
    }

    pub(super) fn setup_canyon(&mut self) {
        self.canyon.wood = [1.0, 1.0];
        let (w, hgt) = (self.size_x, self.size_y);
        let rim = self.both_outline(RIM_EDGE, Some((-400.0, 12_700.0)));
        let lake = self.both_outline(LAKE, None);
        let river = self.both_outline(RIVER, None);
        self.canyon.rim = Outline::build(&rim, w, hgt);
        self.canyon.lake = Outline::build(&lake, w, hgt);
        self.canyon.river = Outline::build(&river, w, hgt);
        for b in BUTTES {
            for p in self.both_sides((b.u, b.v)) {
                self.canyon.buttes.push((p, b.top, b.cap));
            }
        }
        for i in ISLES {
            let (s, c) = i.angle.sin_cos();
            let ends = [
                (i.u - c * i.half, i.v - s * i.half),
                (i.u + c * i.half, i.v + s * i.half),
            ];
            let [wa, ea] = self.both_sides(ends[0]);
            let [wb, eb] = self.both_sides(ends[1]);
            // The east's ridge lies another way (turned 40 degrees about its
            // middle): the isle is its own shape.
            let turn = |a: (f64, f64), b: (f64, f64)| {
                let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let (sn, cs) = 0.7_f64.sin_cos();
                let r = |p: (f64, f64)| {
                    let (dx, dy) = (p.0 - m.0, p.1 - m.1);
                    (m.0 + dx * cs - dy * sn, m.1 + dx * sn + dy * cs)
                };
                (r(a), r(b))
            };
            self.canyon.isles.push((wa, wb, i.r));
            let (ta, tb) = turn(ea, eb);
            self.canyon.isles.push((ta, tb, i.r));
        }
        for &(u, v, r) in COVES {
            for p in self.both_sides((u, v)) {
                self.canyon.coves.push((p, r));
            }
        }
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        // The trails' lines first: erosion and the temples keep off them.
        for t in TRAILS {
            for side in 0..2 {
                // The rim end mirrored, like the bases; the rest down in the
                // canyon each side's own.
                let pts: Vec<(f64, f64)> = t
                    .iter()
                    .enumerate()
                    .map(|(k, &p)| {
                        if k == 0 {
                            self.mirror_sides(p)[side]
                        } else {
                            self.both_sides(p)[side]
                        }
                    })
                    .collect();
                self.canyon.trails.push(Trail {
                    line: smooth_open(&pts, 12),
                    profile: Vec::new(),
                    half: 48.0,
                });
            }
        }
        for side in 0..2 {
            let pts: Vec<(f64, f64)> = BENCH_WAY
                .iter()
                .map(|&p| self.both_sides(p)[side])
                .collect();
            self.canyon.ways.push(smooth_open(&pts, 10));
        }
        self.lay_keep();
        self.lay_temples();
        self.erode_canyon();
        self.even_out_sides();
        self.even_out_isles();
        self.lay_trails();

        let starts: Vec<[(f64, f64); 2]> = STARTS
            .iter()
            .map(|&s| self.mirror_sides(s).map(|p| self.snap(p)))
            .collect();
        let mut pads = Vec::new();
        for pair in &starts {
            for p in pair {
                pads.push(Pad {
                    x: p.0,
                    y: p.1,
                    core: start_core,
                    outer: 2.0 * start_core,
                    height: self.natural(p.0, p.1),
                });
            }
        }
        self.pads = pads;
        // West, east, in turn: skirmish's "Two Sides" (teams by alternate
        // slot) puts one side on each rim.
        self.starts = starts.iter().flatten().copied().collect();

        // Three small fields round each base, inside its pad: one behind
        // (away from the canyon), two on the forward flanks.
        let mut sites = Vec::new();
        for (k, &s) in self.starts.iter().enumerate() {
            let back = if k % 2 == 0 {
                std::f64::consts::PI
            } else {
                0.0
            };
            for turn in [
                0.0_f64,
                std::f64::consts::PI - 1.15,
                std::f64::consts::PI + 1.15,
            ] {
                let (sn, c) = (back + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((s.0 + c * d, s.1 + sn * d), (0.2 * start_core).max(55.0)));
            }
        }
        for &(u, v, r) in RIM_ORE {
            sites.extend(self.mirror_sides((u, v)).map(|p| (p, r)));
        }
        for &(u, v, r) in ORE {
            if u == 0.0 {
                sites.push((self.west((u, v)), r));
                continue;
            }
            let [w, e] = self.both_sides((u, v));
            // The isles' fields sit on the isles' ridges as laid.
            if let Some(k) = ISLES.iter().position(|i| (i.u, i.v) == (u, v)) {
                let mid = |(a, b, _): IsleLaid| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let isles = &self.canyon.isles;
                sites.extend([(mid(isles[2 * k]), r), (mid(isles[2 * k + 1]), r)]);
                continue;
            }
            let w = self.settle(w, self.height(w.0, w.1));
            let want = self.height(w.0, w.1);
            sites.extend([(w, r), (self.settle(e, want), r)]);
        }
        let g = BUILD_CELL_M as f64;
        self.ore = sites
            .into_iter()
            .map(|(p, r)| {
                let p = ((p.0 / g).round() * g, (p.1 / g).round() * g);
                self.ore_field(p.0, p.1, r)
            })
            .collect::<Vec<OreField>>();
        self.precursor.push(PrecursorSite {
            kind: PropKind::Dam,
            x: self.size_x / 2.0,
            y: TOE_V,
            heading: std::f64::consts::FRAC_PI_2,
            scale: 1.0,
        });
        self.forest_edge = 0.0;
        self.fit_forests();
        self.even_out_woods();
    }

    /// Each trail's surface along its line: the ground's own profile, with
    /// every step steeper than [`TRAIL_GRADE`] cut above and filled below
    /// until it is not, so the trail eases down the shelves and ramps
    /// through the cliffs instead of running at one grade.
    fn lay_trails(&mut self) {
        let profiles: Vec<Vec<f64>> = self
            .canyon
            .trails
            .iter()
            .map(|t| {
                let (_, _, total) = along(t.line[0], &t.line);
                let n = (total / TRAIL_STEP) as usize + 1;
                let mut k = 0;
                let mut p = Vec::with_capacity(n);
                // Walk the line in even steps.
                let mut run = 0.0;
                for i in 0..n {
                    let want = i as f64 * TRAIL_STEP;
                    while k + 1 < t.line.len() - 1 && run + seg_len(t.line[k], t.line[k + 1]) < want
                    {
                        run += seg_len(t.line[k], t.line[k + 1]);
                        k += 1;
                    }
                    let len = seg_len(t.line[k], t.line[k + 1]).max(1e-6);
                    let f = ((want - run) / len).clamp(0.0, 1.0);
                    let (a, b) = (t.line[k], t.line[k + 1]);
                    let q = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                    p.push(self.canyon_eroded(q.0, q.1));
                }
                relax(&mut p, TRAIL_GRADE * TRAIL_STEP);
                p
            })
            .collect();
        for (t, p) in self.canyon.trails.iter_mut().zip(profiles) {
            t.profile = p;
        }
    }
}

fn seg_len(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Cuts and fills a profile, its ends held, until no step between
/// neighbours exceeds `max`; then smooths it a little.
fn relax(p: &mut [f64], max: f64) {
    let n = p.len();
    if n < 3 {
        return;
    }
    for _ in 0..2_000 {
        let mut worst = 0.0f64;
        for i in 1..n {
            let step = p[i] - p[i - 1];
            let excess = step.abs() - max;
            if excess > 0.0 {
                worst = worst.max(excess);
                let fix = excess / 2.0 * step.signum();
                if i - 1 > 0 {
                    p[i - 1] += fix;
                } else {
                    p[i] -= fix;
                }
                if i < n - 1 {
                    p[i] -= fix;
                } else {
                    p[i - 1] += fix;
                }
            }
        }
        if worst < 0.01 {
            break;
        }
    }
    for _ in 0..3 {
        let q = p.to_vec();
        for i in 1..n - 1 {
            p[i] = 0.25 * q[i - 1] + 0.5 * q[i] + 0.25 * q[i + 1];
        }
    }
}

#[cfg(test)]
mod tests;
