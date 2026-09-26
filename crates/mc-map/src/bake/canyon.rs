//! "Vermilion Gorge": three against three across a desert canyon, a drowned
//! reservoir filling its middle and an arch dam across its slot at the south.
//!
//! [`Layout::Canyon`](super::Layout::Canyon), 12 km. The canyon runs north to
//! south down the middle of the map, its lake some 7 km long; each side's
//! three bases stand on the rim plateau. The rim drops to a broad bench over
//! a Grand Canyon profile of cliffs and slopes (below), the bench to the lake
//! over the inner gorge, whose walls carry the white ring of the lake's old,
//! higher shore. Drowned side canyons reach into the bench from the lake, the
//! rim is cut back in amphitheatres, buttes stand on the bench and a temple
//! of rock stands out of the lake. Land crosses the canyon twice: over the
//! dam's crest at the south, and over the river's delta flats at the north.
//! Trails (ramps) lead from the rim down to the bench.
//!
//! Heights above the water level, shared with the Desert palette in
//! `mc-render/shaders/desert.wgsl` (`CANYON_*`), which colours the beds by height:
//! the old full-pool line [`RING_TOP`]; the bench's low edge at the inner
//! gorge's rim, [`GORGE_RIM`] (the dam's crest, `landmark::DAM.crest_z`);
//! the foot of the big cliff (the "Redwall"), [`BENCH_TOP`]; and above it
//! [`REDWALL_TOP`], [`SUPAI_TOP`], [`HERMIT_TOP`], [`COCONINO_TOP`] and the
//! rim, [`RIM`].
//!
//! Fairness: a mirror across the north-south middle line, in what plays. The
//! canyon is designed once for the west side in `(u, v)`: `u` metres west of
//! the middle line, `v` metres north of the south edge; designed fields are
//! read at the folded point. Round the bases, the ore, the trails, the dam,
//! the coves and the ford the noise is blended with its mirror image across
//! the middle line ([`Terrain::side_even`]), so the ground there is exact;
//! out in open country it is each side's own ([`Terrain::loose`]), so the
//! outlines of the rim, the shore, the buttes and the walls do not mirror,
//! and the two sides look like two sides of one canyon, not a reflection.

use super::bays::segment;
use super::machine::PrecursorSite;
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::landmark::DAM;
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;

/// Top of the white ring: the lake's old full-pool line.
pub(super) const RING_TOP: f64 = DAM.ring_top;
/// The inner gorge's rim, where the bench begins; the dam's crest.
pub(super) const GORGE_RIM: f64 = DAM.crest_z;
/// Foot of the big cliff, the top of the bench.
pub(super) const BENCH_TOP: f64 = 100.0;
pub(super) const REDWALL_TOP: f64 = 190.0;
pub(super) const SUPAI_TOP: f64 = 275.0;
pub(super) const HERMIT_TOP: f64 = 300.0;
pub(super) const COCONINO_TOP: f64 = 340.0;
/// The rim's edge; the plateau rises gently behind it.
pub(super) const RIM: f64 = 370.0;

/// The canyon's walls, from the rim down: where each bed's face begins
/// (metres in from the rim's edge), its run, its drop, whether it is a cliff
/// (sharp at the top, a talus foot) or a slope, and how far its line wanders.
struct Bed {
    at: f64,
    run: f64,
    drop: f64,
    cliff: bool,
    wander: f64,
}

#[rustfmt::skip]
const BEDS: &[Bed] = &[
    // Kaibab and Toroweap: the pale cap.
    Bed { at: 0.0, run: 22.0, drop: RIM - COCONINO_TOP, cliff: true, wander: 10.0 },
    // Coconino sandstone.
    Bed { at: 30.0, run: 26.0, drop: COCONINO_TOP - HERMIT_TOP, cliff: true, wander: 14.0 },
    // Hermit shale: a red slope, a shelf round the canyon.
    Bed { at: 58.0, run: 180.0, drop: HERMIT_TOP - SUPAI_TOP, cliff: false, wander: 40.0 },
    // The Supai's stair of ledges and slopes.
    Bed { at: 240.0, run: 14.0, drop: 22.0, cliff: true, wander: 30.0 },
    Bed { at: 256.0, run: 110.0, drop: 13.0, cliff: false, wander: 30.0 },
    Bed { at: 368.0, run: 12.0, drop: 18.0, cliff: true, wander: 30.0 },
    Bed { at: 382.0, run: 100.0, drop: 10.0, cliff: false, wander: 30.0 },
    Bed { at: 484.0, run: 12.0, drop: 16.0, cliff: true, wander: 30.0 },
    Bed { at: 498.0, run: 80.0, drop: 6.0, cliff: false, wander: 24.0 },
    // The Redwall: the big cliff down to the bench.
    Bed { at: 580.0, run: 34.0, drop: REDWALL_TOP - BENCH_TOP, cliff: true, wander: 40.0 },
];

/// Metres in from the rim's edge to the foot of the walls.
const WALL_FOOT: f64 = 650.0;

/// The rim's edge, west side, south to north (u, v): the canyon lies east
/// of it. Amphitheatres cut back into the plateau, points jut out between.
const RIM_EDGE: &[(f64, f64)] = &[
    (3050.0, -400.0),
    (3100.0, 800.0),
    (3150.0, 1500.0),
    (3000.0, 2150.0),
    (3100.0, 2650.0),
    // The south amphitheatre.
    (3650.0, 2950.0),
    (4150.0, 3150.0),
    (4200.0, 3450.0),
    (3650.0, 3650.0),
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
    // Closed round beyond the middle line.
    (-3000.0, 12700.0),
    (-3000.0, -400.0),
];

/// The lake's shore at the water level, west side, south to north: the
/// tailwater's slot, the dam's, the forebay, the basin with its three drowned
/// side canyons ("arms"), the tail at the delta.
const LAKE: &[(f64, f64)] = &[
    (120.0, -400.0),
    (110.0, 800.0),
    (130.0, 1600.0),
    (140.0, 2150.0),
    (150.0, 2400.0),
    (260.0, 2700.0),
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
    (-250.0, 9950.0),
    (-250.0, 5000.0),
    (-250.0, -400.0),
    (0.0, -400.0),
];

/// The river above the delta, from the pool it ends in to the north edge.
const RIVER: &[(f64, f64)] = &[
    (150.0, 10470.0),
    (90.0, 10800.0),
    (75.0, 11500.0),
    (85.0, 12700.0),
    (0.0, 12700.0),
    (-85.0, 12700.0),
    (-75.0, 11500.0),
    (-90.0, 10800.0),
    (-150.0, 10470.0),
    (0.0, 10440.0),
];

/// Where the delta's flats lie, along the middle line (v): the ford.
const DELTA: (f64, f64) = (9500.0, 10650.0);

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

#[rustfmt::skip]
const BUTTES: &[Butte] = &[
    // The temple standing out of the lake on the middle line.
    Butte { u: 0.0, v: 6200.0, top: 110.0, cap: 90.0, shore: 520.0 },
    // On the benches.
    Butte { u: 2150.0, v: 2750.0, top: 80.0, cap: 330.0, shore: 0.0 },
    Butte { u: 2350.0, v: 5150.0, top: 70.0, cap: 360.0, shore: 0.0 },
    Butte { u: 2300.0, v: 9150.0, top: 90.0, cap: 300.0, shore: 0.0 },
];

/// A mesa island in the lake, bench-high: a ridge `half` either side of its
/// centre, its shore `r` out from the ridge. The ridge lies at `west`
/// radians (from +u, toward +v) on the west side and `east` on the east:
/// each side's island is its own shape, of the same size and place.
struct Isle {
    u: f64,
    v: f64,
    r: f64,
    half: f64,
    west: f64,
    east: f64,
}

#[rustfmt::skip]
const ISLES: &[Isle] = &[
    Isle { u: 640.0, v: 4700.0, r: 130.0, half: 180.0, west: 1.25, east: 1.95 },
    Isle { u: 700.0, v: 7650.0, r: 130.0, half: 200.0, west: 2.0, east: 1.15 },
];

/// Coves: where a beach runs gently from the bench into the lake instead of
/// the gorge's wall (u, v, reach). A shipyard's way to the water.
const COVES: &[(f64, f64, f64)] = &[
    (2250.0, 3900.0, 380.0),
    (2400.0, 6140.0, 400.0),
    (2150.0, 8040.0, 380.0),
    (700.0, 2950.0, 300.0),
];

/// Trails from the rim down to the bench: rim end first (u, v).
const TRAILS: &[&[(f64, f64)]] = &[
    &[(3450.0, 1600.0), (2900.0, 1980.0), (2250.0, 2350.0)],
    // Down to the middle arm's cove.
    &[(3700.0, 6700.0), (3150.0, 6600.0), (2450.0, 6480.0)],
    &[(3400.0, 10350.0), (2950.0, 10150.0), (2450.0, 9850.0)],
];
/// A trail's level width either side, and the blend into the walls beyond.
const TRAIL_HALF: f64 = 34.0;
const TRAIL_BLEND: f64 = 60.0;

/// The starts, west side (u, v). Each is followed by its mirror image.
const STARTS: &[(f64, f64)] = &[(4750.0, 2050.0), (4850.0, 6150.0), (4750.0, 10250.0)];

/// Ore away from the bases: (u, v, radius). A field on the middle line is its own twin.
const ORE: &[(f64, f64, f64)] = &[
    // On the bench.
    (1400.0, 2300.0, 90.0),
    (1750.0, 4600.0, 80.0),
    (2750.0, 7550.0, 80.0),
    (1500.0, 9550.0, 85.0),
    // On the rim, between the bases.
    (5100.0, 4150.0, 80.0),
    (5100.0, 8150.0, 80.0),
    // The mesa islands.
    (640.0, 4700.0, 80.0),
    (700.0, 7650.0, 80.0),
    // The ford.
    (0.0, 10250.0, 110.0),
];

/// The dam: its crest's apex on the middle line (v, metres north). A
/// multiple of the 32 m overview step, so the renderer stands the model on
/// exactly the crest's height.
const DAM_V: f64 = 2304.0;

/// Half width (m) of the band over which noise hands over to its mirror image.
const BLEND: f64 = 400.0;
/// Metres between the samples of the designed distance fields.
const FIELD_STEP: f64 = 8.0;

/// A signed distance to a designed outline over the west half, on a grid in
/// `(u, v)`: positive inside.
#[derive(Default)]
pub(super) struct Outline {
    nu: usize,
    nv: usize,
    d: Vec<f32>,
}

impl Outline {
    fn build(poly: &[(f64, f64)], width: f64, height: f64) -> Outline {
        let smooth = smooth_closed(poly, 8);
        let (nu, nv) = (
            (width / FIELD_STEP) as usize + 2,
            (height / FIELD_STEP) as usize + 2,
        );
        let mut d = vec![0f32; nu * nv];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = nv.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in d.chunks_mut(rows * nu).enumerate() {
                let smooth = &smooth;
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(nu).enumerate() {
                        let v = (k * rows + r) as f64 * FIELD_STEP;
                        for (i, out) in row.iter_mut().enumerate() {
                            *out = signed(((i as f64) * FIELD_STEP, v), smooth) as f32;
                        }
                    }
                });
            }
        });
        Outline { nu, nv, d }
    }

    /// Bicubic, so cliffs laid off it have no creases at the samples.
    fn at(&self, u: f64, v: f64) -> f64 {
        bicubic(&self.d, self.nu, self.nv, FIELD_STEP, u, v)
    }
}

/// The canyon as designed: its outlines and trails, laid at set-up.
#[derive(Default)]
pub(super) struct Canyon {
    rim: Outline,
    lake: Outline,
    river: Outline,
    /// Trails, smoothed, with the heights they run between.
    trails: Vec<Trail>,
    /// Temples: buttes and spurs standing off the walls out in open
    /// country, each side its own (world positions, not mirrored).
    temples: Vec<Temple>,
    /// Metres the west side's open shore is moved inland (and the east's
    /// out), so both sides keep the same dry bench (`even_out_shores`).
    shore_bias: f64,
}

/// A trail's line (u, v) and the heights at its rim and bench ends.
type Trail = (Vec<(f64, f64)>, f64, f64);

/// A temple: a ridge of rock from `a` to `b` (world metres), its top `top`
/// either side of that line, capped `cap` metres in from the rim's profile.
struct Temple {
    a: (f64, f64),
    b: (f64, f64),
    top: f64,
    cap: f64,
}

/// How far from its ridge line a temple still shapes the ground: to where
/// its apron meets the bench's own fall (`wall_profile`), 1700 m in.
fn temple_reach(t: &Temple) -> f64 {
    t.top * 1.6 + (WALL_FOOT - t.cap) + (1_700.0 - WALL_FOOT) / 5.0 + 20.0
}

/// Temples are sought one to a cell of this size.
const TEMPLE_CELL: f64 = 560.0;

/// A grid of samples `step` apart, `nu` by `nv`, read at `(u, v)` through a
/// Catmull-Rom cubic each way; clamped at the edges.
fn bicubic(d: &[f32], nu: usize, nv: usize, step: f64, u: f64, v: f64) -> f64 {
    let (gu, gv) = (
        (u / step).clamp(0.0, (nu - 1) as f64),
        (v / step).clamp(0.0, (nv - 1) as f64),
    );
    let (i, j) = ((gu as usize).min(nu - 2), (gv as usize).min(nv - 2));
    let (fu, fv) = (gu - i as f64, gv - j as f64);
    let at = |di: isize, dj: isize| {
        let ii = (i as isize + di).clamp(0, nu as isize - 1) as usize;
        let jj = (j as isize + dj).clamp(0, nv as isize - 1) as usize;
        d[jj * nu + ii] as f64
    };
    let cubic = |a: f64, b: f64, c: f64, e: f64, t: f64| {
        b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - e + t * (3.0 * (b - c) + e - a)))
    };
    let row = |dj: isize| cubic(at(-1, dj), at(0, dj), at(1, dj), at(2, dj), fu);
    cubic(row(-1), row(0), row(1), row(2), fv)
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

/// Distance to an open line, and how far along it (metres).
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
    /// A world point in the designed frame: metres west of the middle line
    /// (either side folds onto the west), metres north.
    fn uv(&self, x: f64, y: f64) -> (f64, f64) {
        ((x - self.size_x / 2.0).abs(), y)
    }

    /// A west-side designed point in the world, on the west side.
    fn west(&self, (u, v): (f64, f64)) -> (f64, f64) {
        (self.size_x / 2.0 - u, v)
    }

    /// The mirror image of a world point across the middle line.
    fn mirrored(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.size_x - x, y)
    }

    /// `f` sampled so the result is the same at a point and its mirror image:
    /// `f` itself on the east side, its mirrored sample on the west, blended
    /// across the middle line with the spread kept.
    fn side_even(&self, x: f64, y: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        // Worked from the pair's own two points, so a point and its image
        // get bit-identical results.
        let west = x.min(self.size_x - x);
        let east = self.size_x - west;
        let w = smoothstep(-BLEND, BLEND, east - self.size_x / 2.0);
        match w {
            w if w >= 1.0 => f(east, y),
            w => (w * f(east, y) + (1.0 - w) * f(west, y)) / (w * w + (1.0 - w) * (1.0 - w)).sqrt(),
        }
    }

    /// `f` as [`Terrain::side_even`] has it where `free` is 0, its own
    /// unmirrored self where `free` is 1: the look of the land is free to
    /// differ side to side away from what must play the same.
    fn loose(&self, x: f64, y: f64, free: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        if free <= 0.0 {
            return self.side_even(x, y, f);
        }
        let own = f(x, y);
        if free >= 1.0 {
            return own;
        }
        self.side_even(x, y, &f) * (1.0 - free) + own * free
    }

    /// How free the land's look is here to differ from its mirror image: 0 at
    /// what must play the same (the bases, ore, trails, the dam, the coves,
    /// the ford), 1 out in open country. Itself the same both sides.
    fn canyon_free(&self, x: f64, y: f64) -> f64 {
        let (u, v) = self.uv(x, y);
        let mut k = self.canyon_keep(x, y);
        for &(cu, cv, r) in COVES {
            k = k.min(smoothstep(
                r,
                r + 400.0,
                ((u - cu).powi(2) + (v - cv).powi(2)).sqrt(),
            ));
        }
        let (dv0, dv1) = DELTA;
        let ford = segment((u, v), (0.0, dv0), (0.0, dv1)).0;
        k.min(smoothstep(900.0, 1_500.0, ford))
    }

    /// Metres in from the rim's edge (negative out on the plateau), buttes
    /// included: each is a piece of the plateau left standing. The noise on
    /// it stays gentler than 1 m per m everywhere: steeper, and the distance
    /// folds, and the walls' clean bands break into closed blobs.
    fn rim_depth(&self, x: f64, y: f64) -> f64 {
        let (u, v) = self.uv(x, y);
        // The rim is mirrored exactly: it bounds the plateau, the bases' ground.
        let e = |f: &dyn Fn(f64, f64) -> f64| self.side_even(x, y, f);
        let wobble = e(&|x, y| self.coast.fbm(x / 1_300.0, y / 1_300.0, 3, 0.5)) * 550.0
            + e(&|x, y| self.coast_warp.fbm(x / 340.0, y / 340.0, 3, 0.5)) * 110.0;
        let mut d = self.canyon.rim.at(u, v) + wobble;
        let lobe = e(&|x, y| self.ridge.fbm(x / 260.0, y / 260.0, 2, 0.5));
        for b in BUTTES {
            let r = ((u - b.u).powi(2) + (v - b.v).powi(2)).sqrt();
            // The temple in the lake, out of everyone's way, is its own shape.
            let lobe = if b.shore > 0.0 {
                self.ridge.fbm(x / 230.0 + 5.0, y / 230.0, 3, 0.5) * 1.6
            } else {
                lobe
            };
            d = d.min(b.cap + (r - b.top * (1.0 + 2.0 * lobe)).max(0.0));
        }
        for t in &self.canyon.temples {
            let reach = temple_reach(t);
            let (lo, hi) = (
                (t.a.0.min(t.b.0), t.a.1.min(t.b.1)),
                (t.a.0.max(t.b.0), t.a.1.max(t.b.1)),
            );
            if x < lo.0 - reach || x > hi.0 + reach || y < lo.1 - reach || y > hi.1 + reach {
                continue;
            }
            // Past its foot a temple's hold on the ground falls off fast,
            // leaving a talus apron on the bench, and is gone by `reach`.
            let r = segment((x, y), t.a, t.b).0;
            let own = t.cap + (r - t.top * (1.0 + 2.0 * lobe)).max(0.0);
            d = d.min(own + 4.0 * (own - WALL_FOOT).max(0.0));
        }
        d
    }

    /// Metres out from the water's edge (negative out in the lake).
    fn shore_out(&self, x: f64, y: f64, free: f64) -> f64 {
        let (u, v) = self.uv(x, y);
        let e = |f: &dyn Fn(f64, f64) -> f64| self.loose(x, y, free, f);
        let wobble = e(&|x, y| self.lake_shore.fbm(x / 2_000.0, y / 2_000.0, 3, 0.5)) * 700.0
            + e(&|x, y| self.lake_shore.fbm(x / 700.0, y / 700.0, 3, 0.5)) * 240.0
            + e(&|x, y| self.lake_shore.fbm(x / 170.0 + 9.0, y / 170.0, 2, 0.5)) * 55.0;
        // The wide water's shore wanders; the slots (the dam's, the
        // tailwater's, the river's) keep their width but meander, each side
        // its own way, away from the dam and the ford.
        let wide = smoothstep(DAM_V + 150.0, DAM_V + 700.0, v)
            * (1.0 - smoothstep(DELTA.0 + 200.0, DELTA.1, v));
        let bend = (1.0 - wide) * free * 450.0 * self.lake.fbm(y / 1_500.0 + 3.3, 7.1, 3, 0.5);
        let us = (x - self.size_x / 2.0 - bend).abs();
        let water = self.canyon.lake.at(us, v).max(self.canyon.river.at(us, v));
        // It may widen the water freely but narrow it by no more than
        // 60 m, so no arm is cut off from the lake.
        let side = if x < self.size_x / 2.0 { 1.0 } else { -1.0 };
        let mut inside =
            water + (wobble * wide).max(-60.0) + side * self.canyon.shore_bias * free * wide;
        for b in BUTTES.iter().filter(|b| b.shore > 0.0) {
            let r = ((u - b.u).powi(2) + (v - b.v).powi(2)).sqrt();
            let lobe = self.lake_shore.fbm(x / 400.0 - 7.0, y / 400.0, 3, 0.5) * 900.0;
            inside = inside.min(r - b.shore + lobe);
        }
        for isle in ISLES {
            let a = if x < self.size_x / 2.0 {
                isle.west
            } else {
                isle.east
            };
            let (sa, ca) = a.sin_cos();
            let ends = (isle.u - ca * isle.half, isle.v - sa * isle.half);
            let r = segment(
                (u, v),
                ends,
                (isle.u + ca * isle.half, isle.v + sa * isle.half),
            )
            .0;
            inside = inside.min(r - isle.r + wobble * 0.6);
        }
        -inside
    }

    /// The walls from the rim down, and the bench below them.
    fn wall_profile(&self, x: f64, y: f64, d: f64) -> f64 {
        if d <= 0.0 {
            // The plateau: rising gently back from the rim, rolling.
            // Mirrored: the bases' ground.
            let e = |f: &dyn Fn(f64, f64) -> f64| self.side_even(x, y, f);
            let roll = e(&|x, y| self.tilt.fbm(x / 1_600.0, y / 1_600.0, 3, 0.5));
            let swell = e(&|x, y| self.mtn_height.fbm(x / 480.0, y / 480.0, 3, 0.5));
            let back = smoothstep(0.0, 2_800.0, -d);
            return RIM
                + 14.0
                + 30.0 * back
                + (38.0 * roll + 14.0 * swell) * smoothstep(0.0, 250.0, -d);
        }
        let mut h = RIM;
        for (k, bed) in BEDS.iter().enumerate() {
            let k = k as f64;
            let wander = self.side_even(x, y, |x, y| {
                self.crag
                    .fbm(x / 420.0 + 7.3 * k, y / 420.0 - 3.1 * k, 3, 0.5)
            }) * bed.wander
                * 2.5;
            let t = (d + wander - bed.at) / bed.run;
            h -= bed.drop
                * if bed.cliff {
                    cliff(t)
                } else {
                    smoothstep(0.0, 1.0, t)
                };
        }
        // The bench falls gently toward the gorge.
        h - (BENCH_TOP - GORGE_RIM - 4.0) * smoothstep(WALL_FOOT, 1_700.0, d)
    }

    /// The inner gorge and the lake: `s` metres out from the water's edge.
    fn gorge_profile(&self, x: f64, y: f64, s: f64, free: f64) -> f64 {
        let (u, v) = self.uv(x, y);
        let e = |f: &dyn Fn(f64, f64) -> f64| self.loose(x, y, free, f);
        let cove = COVES
            .iter()
            .map(|&(cu, cv, r)| {
                1.0 - smoothstep(0.45 * r, r, ((u - cu).powi(2) + (v - cv).powi(2)).sqrt())
            })
            .fold(0.0, f64::max);
        let (dv0, dv1) = DELTA;
        let delta = (1.0 - smoothstep(650.0, 1_050.0, u))
            * smoothstep(dv0 - 700.0, dv0, v)
            * (1.0 - smoothstep(dv1, dv1 + 700.0, v));
        // The tailwater below the dam and the river above the delta run shallow.
        let slot =
            (1.0 - smoothstep(DAM_V - 260.0, DAM_V - 40.0, v)).max(smoothstep(dv1, dv1 + 300.0, v));
        if s < 0.0 {
            // The lake's bed: shelving off the shore, deepest down the old river's channel.
            let off = -s;
            let deep = 4.0 + 0.16 * off.min(320.0);
            let channel = 24.0
                * (1.0
                    - smoothstep(
                        180.0,
                        520.0,
                        u + 220.0 * e(&|x, y| self.lake.fbm(x / 900.0, y / 900.0, 3, 0.5)),
                    ));
            let floor = -(4.0 + 0.4 * off.min(3.0)) * slot
                - (deep + channel * smoothstep(0.0, 200.0, off)) * (1.0 - slot);
            let shelf = -(0.03 * off.min(60.0) + 0.05 * (off - 60.0).max(0.0));
            let gentle = cove.max(delta);
            return floor * (1.0 - gentle) + shelf.max(floor) * gentle;
        }
        // The walls: sheer to the ring's top and a little over, then the bench.
        let run = 36.0 + 14.0 * e(&|x, y| self.ramp.fbm(x / 300.0, y / 300.0, 2, 0.5)) * 5.0;
        let wall = (RING_TOP + 3.0) * cliff(s / run.max(14.0));
        let ledge = 8.0 * smoothstep(run, run + 22.0, s);
        // Past the gorge's rim the bench is the walls' (`wall_profile`); this
        // side only rises out of its way.
        let on = (s - run - 22.0).max(0.0);
        // Walkably up to above the bench's top, then steeply: only walls
        // (a butte's, the Redwall's) stand that high this near the water.
        let bench = 0.03 * on.min(150.0)
            + 0.3 * (on - 150.0).clamp(0.0, 120.0)
            + 1.5 * (on - 270.0).max(0.0);
        let steep = wall + ledge + bench;
        // A cove's beach runs gently up to the bench instead.
        let beach =
            (GORGE_RIM + 2.0) * (s / 650.0).clamp(0.0, 1.0).powf(1.25) + 0.4 * (s - 650.0).max(0.0);
        // The delta's flats: level near the water, then up to the bench.
        let flats = 0.025 * s.min(120.0)
            + 0.012 * (s - 120.0).clamp(0.0, 200.0)
            + 0.1 * (s - 320.0).max(0.0);
        let h = steep * (1.0 - cove) + beach.min(steep.max(beach)) * cove;
        h * (1.0 - delta) + flats.min(h.max(flats)) * delta
    }

    /// The canyon before erosion, trails and the dam.
    fn canyon_shape(&self, x: f64, y: f64) -> f64 {
        let free = self.canyon_free(x, y);
        let walls = self.wall_profile(x, y, self.rim_depth(x, y));
        let s = self.shore_out(x, y, free);
        let gorge = self.gorge_profile(x, y, s, free);
        let mut h = if s < 0.0 { gorge } else { walls.min(gorge) };
        // Grain: washes and hummocks on the level ground.
        if h > 2.0 {
            let e = |f: &dyn Fn(f64, f64) -> f64| self.side_even(x, y, f);
            let wash = e(&|x, y| self.detail.ridged(x / 520.0, y / 520.0, 3, 0.5));
            h -= 5.0 * smoothstep(0.8, 0.97, wash) * smoothstep(2.0, 20.0, h);
            h += 1.6 * e(&|x, y| self.detail.fbm(x / 90.0 + 9.0, y / 90.0 - 4.0, 2, 0.5));
        }
        h
    }

    /// 1 in open country, 0 on what must be built on or walked cleanly: the
    /// bases, the ore, the trails, the dam; erosion keeps off them.
    fn canyon_keep(&self, x: f64, y: f64) -> f64 {
        let (u, v) = self.uv(x, y);
        let near = |cu: f64, cv: f64, reach: f64| {
            smoothstep(
                reach,
                reach + 250.0,
                ((u - cu).powi(2) + (v - cv).powi(2)).sqrt(),
            )
        };
        let mut k = STARTS
            .iter()
            .map(|&(su, sv)| near(su, sv, 1.1 * self.start_outer))
            .fold(1.0, f64::min);
        // A mesa isle is each side's own shape: its field lies on it either way.
        for &(ou, ov, r) in ORE
            .iter()
            .filter(|&&(ou, ov, _)| !ISLES.iter().any(|i| (i.u, i.v) == (ou, ov)))
        {
            k = k.min(near(ou, ov, r + 60.0));
        }
        k = k.min(near(0.0, DAM_V, 420.0));
        for (line, _, _) in &self.canyon.trails {
            let (d, _, _) = along((u, v), line);
            k = k.min(smoothstep(
                TRAIL_HALF + TRAIL_BLEND,
                TRAIL_HALF + TRAIL_BLEND + 150.0,
                d,
            ));
        }
        k
    }

    /// The ground as eroded, before the trails and the dam.
    fn canyon_eroded(&self, x: f64, y: f64) -> f64 {
        let h = self.canyon_shape(x, y);
        if self.erosion.delta.is_empty() || h < 1.5 {
            return h;
        }
        let (er, n) = (&self.erosion, self.erosion.n);
        let west = x.min(self.size_x - x);
        let e = bicubic(&er.delta, n, n, er.step, west, y)
            * self.canyon_free(x, y)
            * smoothstep(1.5, 8.0, h);
        (h + e).max(h.min(1.5))
    }

    pub(super) fn natural_canyon(&self, x: f64, y: f64) -> f64 {
        let mut h = self.canyon_eroded(x, y);
        let (u, v) = self.uv(x, y);
        for (line, z0, z1) in &self.canyon.trails {
            let (d, at, total) = along((u, v), line);
            if d > TRAIL_HALF + TRAIL_BLEND {
                continue;
            }
            // Past its ends the trail's height is the ground's own, so the
            // round blend there changes little.
            let t = (at / total).clamp(0.0, 1.0);
            let z = z0 + (z1 - z0) * t;
            h += (z - h) * (1.0 - smoothstep(TRAIL_HALF, TRAIL_HALF + TRAIL_BLEND, d));
        }
        self.dam_ground(x, y, h)
    }

    /// The dam's crest and the level ground off each end of it.
    fn dam_ground(&self, x: f64, y: f64, h: f64) -> f64 {
        let (cx, cy) = (self.size_x / 2.0, DAM_V);
        if (x - cx).abs() > 500.0 || (y - cy).abs() > 400.0 {
            return h;
        }
        // Heading north: the model's x is north, its y is west.
        let (mx, my) = (y - cy, cx - x);
        let (a, off) = DAM.arch_coords(mx, my);
        let crest = DAM.crest_z;
        let mut h = h;
        // Between the abutments: the walked crest, and off it the ground cut
        // away under the dam's faces, the lake to its bed upstream and a
        // plunge pool downstream as wide as the arch, so the water meets
        // the dam across the whole gorge. The rock stands sheer at the
        // abutments.
        if a.abs() <= DAM.half_angle && (-DAM.road_down..=DAM.road_up).contains(&off) {
            return crest;
        }
        let span = 1.0 - smoothstep(DAM.half_angle - 0.02, DAM.half_angle + 0.05, a.abs());
        if span > 0.0 {
            // Just off the band the ground tucks a metre under the dam's
            // face (the 8 m samples' triangles then stay in the concrete);
            // past the footing, the floors.
            let cut = if off > 0.0 {
                // Upstream: under the face, then the lake's bed past the toe,
                // then the lake's own bed.
                let toe = DAM.upstream_face(crest + DAM.bed);
                let floor =
                    -DAM.bed + (h + DAM.bed).max(0.0) * smoothstep(toe + 10.0, toe + 50.0, off);
                DAM.upstream_depth(off)
                    .map_or(floor, |d| crest - d - 1.0)
                    .min(h)
            } else {
                // Downstream: under the face down to the tailwater's shallow
                // floor, which runs 140 m past the footing and then gives
                // way to the ground's own.
                let foot = -DAM.downstream_face(crest + DAM.bed);
                let floor = -5.0 + (h + 5.0) * smoothstep(foot + 140.0, foot + 260.0, -off);
                DAM.downstream_depth(off)
                    .map_or(floor, |d| (crest - d - 1.0).max(floor))
            };
            h += (cut - h) * span;
        }
        // Off each abutment, level ground along the arc's tangent: exactly
        // the crest's height beside the road for its whole length (the
        // thrust blocks carry the road onto it), blending out beyond, and
        // back over the gorge only as far as the rock is left standing.
        for side in [-1.0, 1.0] {
            let a = side * DAM.half_angle;
            let (ax, ay) = DAM.crest_at(a);
            let (tx, ty) = (-a.sin() * side, a.cos() * side);
            let (px, py) = (mx - ax, my - ay);
            let along = px * tx + py * ty;
            let beside = (px * ty - py * tx).abs();
            if (0.0..=DAM.approach).contains(&along) && beside <= 26.0 {
                return crest;
            }
            let b = (ax + tx * DAM.approach, ay + ty * DAM.approach);
            let (d, _) = segment((mx, my), (ax, ay), b);
            let mut w = 1.0 - smoothstep(26.0, 90.0, d);
            if along < 0.0 {
                w *= 1.0 - span;
            }
            h += (crest - h) * w;
        }
        h
    }

    /// Scatters the temples: in the canyon off the walls, on dry ground,
    /// nowhere near what must play the same.
    fn lay_temples(&mut self) {
        use crate::noise::{hash2, unit};
        let cells = (self.size_x / TEMPLE_CELL).ceil() as i64;
        let mut temples = Vec::new();
        for j in 0..cells {
            for i in 0..cells {
                let hash = hash2(self.seed ^ 0x7465_6D70, i, j);
                if unit(hash, 0) > 0.6 {
                    continue;
                }
                let c = (
                    (i as f64 + 0.15 + 0.7 * unit(hash, 8)) * TEMPLE_CELL,
                    (j as f64 + 0.15 + 0.7 * unit(hash, 16)) * TEMPLE_CELL,
                );
                let (u, v) = self.uv(c.0, c.1);
                let into = self.canyon.rim.at(u, v);
                let dry = -self.canyon.lake.at(u, v).max(self.canyon.river.at(u, v));
                if !(180.0..WALL_FOOT + 700.0).contains(&into) || dry < 260.0 {
                    continue;
                }
                // Tall near the rim, lower out toward the lake.
                let reach_in = ((into - 180.0) / (WALL_FOOT + 520.0)).clamp(0.0, 1.0);
                let cap = 20.0 + 420.0 * (0.25 * unit(hash, 24) + 0.75 * reach_in).min(1.0);
                let top = 30.0 + 90.0 * unit(hash, 32);
                let half = 260.0 * unit(hash, 40).powi(2);
                let angle = unit(hash, 48) * std::f64::consts::TAU;
                let (sa, ca) = angle.sin_cos();
                let t = Temple {
                    a: (c.0 - ca * half, c.1 - sa * half),
                    b: (c.0 + ca * half, c.1 + sa * half),
                    top,
                    cap: cap.min(WALL_FOOT - 60.0),
                };
                let foot = temple_reach(&t) + half + 40.0;
                // Nothing that must play the same anywhere under it.
                let k = (foot / 50.0).ceil() as i64;
                let clear = (-k..=k).all(|dj| {
                    (-k..=k).all(|di| {
                        let (dx, dy) = (di as f64 * 50.0, dj as f64 * 50.0);
                        dx * dx + dy * dy > foot * foot
                            || self.canyon_free(c.0 + dx, c.1 + dy) >= 1.0
                    })
                });
                if clear {
                    temples.push(t);
                }
            }
        }
        self.canyon.temples = temples;
    }

    /// The shore's noise is each side's own; this moves one side's open
    /// shore in and the other's out until both hold the same dry ground
    /// below the walls (the bench, what the lake can take).
    fn even_out_shores(&mut self) {
        const STEP: f64 = 24.0;
        let n = (self.size_x / STEP) as usize;
        let skew = |t: &Terrain| {
            let rows: Vec<(usize, usize)> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..n)
                    .map(|j| {
                        s.spawn(move || {
                            let y = (j as f64 + 0.5) * STEP;
                            let mut side = (0usize, 0usize);
                            for i in 0..n {
                                let x = (i as f64 + 0.5) * STEP;
                                let h = t.canyon_eroded(x, y);
                                if h > 1.0 && h < BENCH_TOP + 30.0 {
                                    if x < t.size_x / 2.0 {
                                        side.0 += 1;
                                    } else {
                                        side.1 += 1;
                                    }
                                }
                            }
                            side
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let (w, e) = rows.iter().fold((0, 0), |a, r| (a.0 + r.0, a.1 + r.1));
            w as f64 - e as f64
        };
        let (mut lo, mut hi) = (-80.0, 80.0);
        for _ in 0..14 {
            self.canyon.shore_bias = 0.5 * (lo + hi);
            if skew(self) > 0.0 {
                lo = self.canyon.shore_bias;
            } else {
                hi = self.canyon.shore_bias;
            }
        }
    }

    /// Water erosion over the whole canyon: gullies down every wall, fans at
    /// their feet. Run over the map as it is, then made exact under the
    /// mirror the way the noise is.
    fn erode_canyon(&mut self) {
        const STEP: f64 = 16.0;
        const VERTICAL: f32 = 24.0;
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
                            *v = t.canyon_shape(i as f64 * STEP, y).max(-2.0) as f32 / VERTICAL;
                        }
                    }
                });
            }
        });
        let before = h.clone();
        super::alpine::erode(&mut h, n, self.seed);
        super::alpine::erode(&mut h, n, self.seed ^ 0x6361_6E79);
        // Gullies down the walls; the level ground keeps its shape (droplets
        // running over a plateau or a bench fret it into fern patterns).
        let mut raw = vec![0f32; n * n];
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                let gx = before[at + 1] - before[at - 1];
                let gy = before[at + n] - before[at - n];
                let slope = (gx * gx + gy * gy).sqrt() as f64 * VERTICAL as f64 / (2.0 * STEP);
                // Most on the slopes between the cliffs; the cliffs keep their faces.
                let weight = 0.08
                    + 0.92
                        * smoothstep(0.2, 0.6, slope)
                        * (1.0 - 0.75 * smoothstep(0.9, 1.8, slope));
                raw[at] = ((h[at] - before[at]) * VERTICAL).clamp(-24.0, 8.0) * weight as f32;
            }
        }
        let mut soft = raw.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                soft[at] =
                    0.5 * raw[at] + 0.125 * (raw[at - 1] + raw[at + 1] + raw[at - n] + raw[at + n]);
            }
        }
        // The same both sides (the walls' shelves are walked on): blended
        // across the middle line like the noise, and read on the west.
        let mut delta = soft.clone();
        for j in 0..n {
            for i in 0..n {
                let x = i as f64 * STEP;
                let w = smoothstep(-BLEND, BLEND, x - self.size_x / 2.0) as f32;
                delta[j * n + i] = w * soft[j * n + i] + (1.0 - w) * soft[j * n + (n - 1 - i)];
            }
        }
        self.erosion = super::alpine::Erosion {
            n,
            step: STEP,
            delta,
        };
    }

    /// Pinyon and juniper woodland on the rim, scattered junipers down the
    /// walls and on the bench, cottonwoods by the water at the delta and the
    /// coves. Exact under the mirror: timber is income.
    pub(super) fn canyon_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let e = |f: &dyn Fn(f64, f64) -> f64| self.side_even(x, y, f);
        let stands = e(&|x, y| self.forest.fbm(x / 1_400.0, y / 1_400.0, 3, 0.5));
        let clumps = e(&|x, y| self.forest.fbm(x / 160.0 + 71.3, y / 160.0 - 19.1, 2, 0.5));
        let wood = smoothstep(-0.08, 0.25, stands) * 0.42 + 0.06;
        let clump = smoothstep(0.0, 0.3, clumps);
        let open = 1.0 - smoothstep(0.35, 0.6, slope);
        let rim = smoothstep(RIM - 8.0, RIM + 6.0, height) * wood * (0.35 + 0.65 * clump);
        let walls = smoothstep(BENCH_TOP + 10.0, BENCH_TOP + 40.0, height)
            * (1.0 - smoothstep(RIM - 8.0, RIM + 6.0, height))
            * 0.1
            * clump;
        let bench = smoothstep(GORGE_RIM, GORGE_RIM + 8.0, height)
            * (1.0 - smoothstep(BENCH_TOP + 10.0, BENCH_TOP + 40.0, height))
            * 0.07
            * clump;
        let water = (1.0 - smoothstep(4.0, 12.0, height))
            * smoothstep(0.8, 1.6, height)
            * (0.2 + 0.4 * clump);
        let mut density = (rim + walls + bench + water) * open;
        for f in &self.ore {
            let d = ((x - f.x).powi(2) + (y - f.y).powi(2)).sqrt() - f.radius;
            if d < 160.0 {
                density *= smoothstep(50.0, 160.0, d);
            }
        }
        (density, height)
    }

    /// The canyon's species by where they grow (`forest_density` hands the
    /// height over in the conifer slot).
    pub(super) fn canyon_tree(&self, height: f64, hash: u64) -> PropKind {
        use crate::noise::unit;
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.05 => PropKind::TreeDead,
            _ if height < 14.0 => PropKind::TreeCottonwood,
            (_, pick) if height > RIM - 10.0 && pick < 0.55 => PropKind::TreePinyon,
            _ => PropKind::TreeJuniper,
        }
    }

    pub(super) fn setup_canyon(&mut self) {
        let (half, size_y) = (self.size_x / 2.0, self.size_y);
        self.canyon.rim = Outline::build(RIM_EDGE, half, size_y);
        self.canyon.lake = Outline::build(LAKE, half, size_y);
        self.canyon.river = Outline::build(RIVER, half, size_y);
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        // The trails' ends, on the ground as shaped (before they are cut in).
        let trails: Vec<Vec<(f64, f64)>> = TRAILS.iter().map(|t| smooth_open(t, 8)).collect();
        self.canyon.trails = trails.into_iter().map(|line| (line, 0.0, 0.0)).collect();
        // With the lines laid, the ground round them is the mirrored ground.
        let ends: Vec<(f64, f64)> = self
            .canyon
            .trails
            .iter()
            .map(|(line, _, _)| {
                let (a, b) = (self.west(line[0]), self.west(line[line.len() - 1]));
                (self.canyon_shape(a.0, a.1), self.canyon_shape(b.0, b.1))
            })
            .collect();
        for (trail, (z0, z1)) in self.canyon.trails.iter_mut().zip(ends) {
            (trail.1, trail.2) = (z0, z1);
        }
        self.lay_temples();
        self.erode_canyon();
        self.even_out_shores();

        let west: Vec<(f64, f64)> = STARTS.iter().map(|&s| self.snap(self.west(s))).collect();
        let mut pads = Vec::new();
        for &at in &west {
            let height = self.natural(at.0, at.1);
            for p in [at, self.mirrored(at)] {
                pads.push(Pad {
                    x: p.0,
                    y: p.1,
                    core: start_core,
                    outer: 2.0 * start_core,
                    height,
                });
            }
        }
        self.pads = pads;
        // Each side's in turn, its mirror next: skirmish's "Two Sides" (teams
        // by alternate slot) puts one side on each rim.
        self.starts = west.iter().flat_map(|&s| [s, self.mirrored(s)]).collect();

        // Three small fields round each base, inside its pad: one behind
        // (away from the canyon), two on the forward flanks.
        let mut sites = Vec::new();
        for &a in &west {
            for turn in [
                0.0_f64,
                std::f64::consts::PI - 1.15,
                std::f64::consts::PI + 1.15,
            ] {
                let (s, c) = (std::f64::consts::PI + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((a.0 + c * d, a.1 + s * d), (0.2 * start_core).max(55.0)));
            }
        }
        for &(u, v, r) in ORE {
            sites.push((self.west((u, v)), r));
        }
        let g = BUILD_CELL_M as f64;
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = ((p.0 / g).round() * g, (p.1 / g).round() * g);
            let field = self.ore_field(p.0, p.1, r);
            if (p.0 - self.size_x / 2.0).abs() > 1.0 {
                let m = self.mirrored(p);
                let corners = field.corners.iter().map(|&c| self.mirrored(c)).collect();
                fields.push(OreField {
                    x: m.0,
                    y: m.1,
                    radius: field.radius,
                    corners,
                });
            }
            fields.push(field);
        }
        self.ore = fields;
        self.precursor.push(PrecursorSite {
            kind: PropKind::Dam,
            x: self.size_x / 2.0,
            y: DAM_V,
            heading: std::f64::consts::FRAC_PI_2,
            scale: 1.0,
        });
        self.forest_edge = 0.0;
        self.fit_forests();
    }
}

#[cfg(test)]
mod tests;
