//! Thousand Spires' ground: the benches and their escarpments, the valleys
//! and their rivers, the lake, the sea and its islands, the massif and the
//! outliers cut into blocks and columns, and the pillars standing alone.
//!
//! The benches are the country's lie ([`Terrain::sp_lie`]) stepped every
//! [`BENCH`] metres: a cliff where the step is sharp, a long slope where the
//! ground is gentle, so the escarpments wander and break of themselves.
//!
//! Every third is laid from the same plan, but nothing here is turned: the
//! plan's lines are read through a slow warp of their own in each third,
//! and all the noise is the map's own. Only the bases, their pads and the
//! ore are held where the plan puts them.

use super::*;
use crate::bake::bays::segment;
use crate::noise::hash2;

/// How far the warp bends the plan's lines, metres at most, about; and the
/// lengths it bends them over.
const WARP: f64 = 1_500.0;
const WARP_L: (f64, f64) = (5_200.0, 1_700.0);
/// How far the escarpments' rims and the valleys' walls stand off their
/// (warped) lines, metres at most, about.
const FRAY: f64 = 260.0;
/// The benches: the first step's foot over the water, and the rise from
/// one to the next, metres.
const BENCH_0: f64 = 10.0;
const BENCH: f64 = 46.0;
/// The massif's blocks: the cell they are cut from, and the finer one some
/// are split into columns by; metres.
const BLOCK_CELL: f64 = 230.0;
const COLUMN_CELL: f64 = 96.0;
/// The highest a massif top or pillar stands, metres over the water (the
/// map's heights stop at 768).
const PEAK: f64 = 740.0;
/// The pillars standing on their own: the cell each may stand in, metres.
const PILLAR_CELL: f64 = 120.0;

/// One pillar standing on its own, design metres.
#[derive(Clone, Copy)]
struct Pillar {
    at: (f64, f64),
    radius: f64,
    /// How high it stands over the ground at its foot.
    tall: f64,
    /// Its outline's lobes' phases.
    phase: (f64, f64),
}

/// The pillars, filed by [`PILLAR_CELL`] cell over the whole map.
#[derive(Default)]
pub(in crate::bake) struct Pillars {
    /// Cell index of the grid's south-west corner, and its width.
    origin: i64,
    w: usize,
    cells: Vec<Option<Pillar>>,
}

impl Pillars {
    /// Lays the pillars: some cells stand one, more in the lake and the
    /// valleys and off the coast, few on the benches, none where the ways
    /// run, the bases stand or the ore lies.
    pub(in crate::bake) fn lay(t: &Terrain) -> Pillars {
        let lo = (-MID / PILLAR_CELL).floor() as i64;
        let w = (2.0 * MID / PILLAR_CELL).ceil() as usize + 1;
        let rows: Vec<Vec<Option<Pillar>>> = {
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let per = w.div_ceil(threads);
            std::thread::scope(|s| {
                let handles: Vec<_> = (0..threads)
                    .map(|k| {
                        s.spawn(move || {
                            (k * per..((k + 1) * per).min(w))
                                .map(|j| {
                                    (0..w)
                                        .map(|i| Pillars::cell(t, lo + i as i64, lo + j as i64))
                                        .collect()
                                })
                                .collect::<Vec<Vec<Option<Pillar>>>>()
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().unwrap_or_default())
                    .collect()
            })
        };
        Pillars {
            origin: lo,
            w,
            cells: rows.into_iter().flatten().collect(),
        }
    }

    fn cell(t: &Terrain, ci: i64, cj: i64) -> Option<Pillar> {
        let hash = hash2(t.seed ^ 0x7069_6C6C, ci, cj);
        let at = (
            (ci as f64 + 0.2 + 0.6 * unit(hash, 0)) * PILLAR_CELL,
            (cj as f64 + 0.2 + 0.6 * unit(hash, 24)) * PILLAR_CELL,
        );
        let (chance, size, tall) = t.sp_pillar_zone(at);
        if chance <= 0.0 || unit(hash, 40) >= chance {
            return None;
        }
        let mix = hash2(hash, 7, 11);
        Some(Pillar {
            at,
            radius: size * (16.0 + 32.0 * unit(mix, 0)),
            tall: tall.0 + (tall.1 - tall.0) * unit(mix, 24).powf(1.4),
            phase: (6.3 * unit(mix, 40), 6.3 * unit(hash2(mix, 3, 5), 0)),
        })
    }

    /// The pillars that may reach a design point.
    fn near(&self, q: (f64, f64)) -> impl Iterator<Item = &Pillar> {
        let ci = (q.0 / PILLAR_CELL).floor() as i64 - self.origin;
        let cj = (q.1 / PILLAR_CELL).floor() as i64 - self.origin;
        (-1..=1).flat_map(move |dj| {
            (-1..=1).filter_map(move |di| {
                let (i, j) = (ci + di, cj + dj);
                if i < 0 || j < 0 || i as usize >= self.w || j as usize >= self.w {
                    return None;
                }
                self.cells[j as usize * self.w + i as usize].as_ref()
            })
        })
    }
}

impl Terrain {
    /// Metres from a design point to the nearest base.
    fn sp_base_dist(&self, q: (f64, f64)) -> f64 {
        let p = half(q);
        dist(p, INNER).min(dist(p, FLANK))
    }

    /// Metres from a design point to the edge of the nearest ore field the
    /// plan lays away from the bases (the island's included).
    fn sp_ore_dist(&self, q: (f64, f64)) -> f64 {
        let p = half(q);
        ORE.iter()
            .map(|&(x, y, r)| dist(p, (x, y)) - r)
            .fold(q.0.hypot(q.1) - ISLAND_ORE, f64::min)
    }

    /// Where the plan is read for a design point: bent by a slow warp of the
    /// map's own, so no two thirds' escarpments, valleys, coasts or massifs
    /// lie alike. Held still round the bases, the ore and the island.
    pub(super) fn sp_warp(&self, q: (f64, f64)) -> (f64, f64) {
        let k = WARP
            * smoothstep(650.0, 1_900.0, self.sp_base_dist(q))
            * smoothstep(120.0, 700.0, self.sp_ore_dist(q))
            * smoothstep(700.0, 1_800.0, q.0.hypot(q.1));
        if k <= 0.0 {
            return q;
        }
        let (x, y) = on_map(q);
        let (l, m) = WARP_L;
        let dx = self.warp_x.fbm(x / l, y / l, 2, 0.5)
            + 0.3 * self.warp_y.fbm(x / m + 4.0, y / m, 2, 0.5);
        let dy = self.warp_y.fbm(x / l - 9.0, y / l, 2, 0.5)
            + 0.3 * self.warp_x.fbm(x / m, y / m - 3.0, 2, 0.5);
        (q.0 + k * dx, q.1 + k * dy)
    }

    /// Metres off the nearest valley's middle line and out along it, of a
    /// (warped) design point. The line meanders: it is the edge between two
    /// thirds pushed this way and that, the same push seen from either side.
    fn sp_valley(&self, w: (f64, f64)) -> (f64, f64) {
        let n = north(w);
        let (b, t) = edge_frame((n.0.abs(), n.1));
        // Which side of the line: the east half of one third faces the west
        // half of the next.
        let across = b * n.0.signum();
        let (x, y) = on_map(w);
        let swing = 650.0
            * smoothstep(MASSIF_IN, MASSIF_IN + 1_400.0, t)
            * smoothstep(1_300.0, 2_300.0, self.sp_base_dist(w))
            * (self
                .warp_y
                .fbm(x / 3_400.0 + 21.0, y / 3_400.0 - 4.0, 3, 0.5)
                + 0.3 * self.warp_x.fbm(x / 1_100.0, y / 1_100.0 + 13.0, 2, 0.5));
        ((across - swing).abs(), t)
    }

    /// Metres from a (warped) design point to the nearest ford: over the
    /// lake to the island, or across a river.
    pub(super) fn sp_ford(&self, w: (f64, f64)) -> f64 {
        let p = half(w);
        let r = w.0.hypot(w.1);
        let lake = if (250.0..LAKE_R + 250.0).contains(&r) {
            p.0
        } else {
            f64::INFINITY
        };
        let (b, t) = self.sp_valley(w);
        FORDS
            .iter()
            .map(|&f| (t - f).abs().max(b - 420.0).max(0.0))
            .fold(lake, f64::min)
    }

    /// 1 in open country, falling to 0 round the bases, the ore and the
    /// fords: the small relief and the pillars keep off what must be
    /// built on or walked.
    fn sp_keep(&self, q: (f64, f64), w: (f64, f64)) -> f64 {
        smoothstep(1_050.0, 1_400.0, self.sp_base_dist(q))
            * smoothstep(40.0, 260.0, self.sp_ore_dist(q))
            * smoothstep(60.0, 260.0, self.sp_ford(w))
    }

    /// The river's (and the estuary's) half width and depth below the water,
    /// out along the valley, and how much of a ford's approach the point is
    /// on (0..1: the banks lie back there, so units can walk down to it).
    fn sp_channel(&self, t: f64) -> (f64, f64, f64) {
        let river = RIVER_HALF.0 + (RIVER_HALF.1 - RIVER_HALF.0) * smoothstep(2_000.0, 7_000.0, t);
        let open = smoothstep(ESTUARY.0, 10_400.0, t);
        let ford = FORDS
            .iter()
            .map(|&f| smoothstep(FORD_HALF + 30.0, FORD_HALF, (t - f).abs()))
            .fold(0.0, f64::max);
        let deep =
            RIVER_BED + (ESTUARY.2 - RIVER_BED) * smoothstep(ESTUARY.0, ESTUARY.0 + 1_400.0, t);
        let approach = FORDS
            .iter()
            .map(|&f| smoothstep(FORD_HALF + 160.0, FORD_HALF + 20.0, (t - f).abs()))
            .fold(0.0, f64::max);
        // A ford's crest stands a metre out of the water.
        (
            river + (ESTUARY.1 - river) * open,
            deep + (-1.0 - deep) * ford,
            approach,
        )
    }

    /// The coast's distance from the middle at radians `s` off the axis,
    /// before it is frayed.
    fn sp_coast(&self, s: f64) -> f64 {
        COAST.0 + COAST.1 * s - HARBOUR.0 * crate::noise::bump(s / HARBOUR.1)
    }

    /// The country's lie before it is stepped into benches, metres: high
    /// inland under the massif, falling toward the coast, rising and falling
    /// over the kilometres; held at the middle of its bench round each base.
    fn sp_lie(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64)) -> f64 {
        let r = w.0.hypot(w.1);
        let coast = self.sp_coast(off_axis(half(w)));
        let inland = smoothstep(coast - 400.0, coast - 4_000.0, r);
        let mut e = LOWLAND
            + (TABLE + 30.0 - LOWLAND) * inland
            + 70.0
                * self.tilt.fbm(x / 4_600.0 + 40.0, y / 4_600.0, 3, 0.5)
                * smoothstep(0.1, 0.5, inland)
            + 24.0 * self.tilt.fbm(x / 1_300.0 - 7.0, y / 1_300.0 + 2.0, 2, 0.5);
        // Each base on the middle of the bench it falls on, the bench's edge
        // wandering in and out round it. (All four of the third's bases, by
        // their own distances: no seam down the axis.)
        let n = north(q);
        let k = third(q);
        let wander = 1.0 + 0.4 * self.mtn_gap.fbm(x / 900.0 + 3.0, y / 900.0, 2, 0.5);
        for b in [INNER, FLANK, (-INNER.0, INNER.1), (-FLANK.0, FLANK.1)] {
            let d = dist(n, b) * wander;
            if d < 2_000.0 {
                let at = on_map(turn(b, k));
                let (step, _) = bench_of(self.sp_lie_raw(at.0, at.1, half(b)));
                let middle = BENCH_0 + BENCH * (step + 0.5);
                e += (middle - e) * smoothstep(2_000.0, 1_100.0, d);
            }
        }
        e
    }

    /// [`Terrain::sp_lie`] at a base's own point (where nothing holds it).
    fn sp_lie_raw(&self, x: f64, y: f64, b: (f64, f64)) -> f64 {
        let r = b.0.hypot(b.1);
        let coast = self.sp_coast(off_axis(b));
        let inland = smoothstep(coast - 400.0, coast - 4_000.0, r);
        LOWLAND
            + (TABLE + 30.0 - LOWLAND) * inland
            + 70.0
                * self.tilt.fbm(x / 4_600.0 + 40.0, y / 4_600.0, 3, 0.5)
                * smoothstep(0.1, 0.5, inland)
            + 24.0 * self.tilt.fbm(x / 1_300.0 - 7.0, y / 1_300.0 + 2.0, 2, 0.5)
    }

    /// How much a (warped) design point is on one of a base's ways off its bench,
    /// 0..1: toward the gate, down into the valley, out to the coast and
    /// over to its neighbours. Escarpments lie back into slopes along them,
    /// so no base's bench is ringed by cliffs.
    fn sp_way(&self, q: (f64, f64)) -> f64 {
        let p = half(q);
        let to_valley = |b: (f64, f64)| {
            let (_, t) = edge_frame(b);
            let off = valley_half(t) - 250.0;
            (SIN_EDGE * t - COS_EDGE * off, COS_EDGE * t + SIN_EDGE * off)
        };
        let to_coast = |b: (f64, f64)| {
            let r = b.0.hypot(b.1);
            let out = (COAST.0 - 500.0) / r;
            (b.0 * out, b.1 * out)
        };
        let ways = [
            (INNER, (0.0, MASSIF_OUT + 150.0)),
            ((0.0, MASSIF_OUT + 150.0), (0.0, LAKE_R)),
            (INNER, (-INNER.0, INNER.1)),
            (INNER, FLANK),
            (INNER, to_coast(INNER)),
            (FLANK, to_valley(FLANK)),
            (FLANK, to_coast(FLANK)),
        ];
        ways.iter()
            .map(|&(a, b)| smoothstep(260.0, 110.0, segment(p, a, b).0))
            .fold(0.0, f64::max)
    }

    /// The benches, cut down into the valleys: the land before the water,
    /// the massif and the pillars, read at the warped point `w`. The lie is
    /// stepped into benches [`BENCH`] metres apart; where the ground is
    /// gentle a step is a long slope, elsewhere a cliff. Also how much of
    /// the point is valley floor.
    fn sp_level(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64)) -> (f64, f64) {
        let r = w.0.hypot(w.1);
        let e = self.sp_lie(x, y, q, w);
        let gentle = smoothstep(
            -0.2,
            0.2,
            self.ridge
                .fbm(x / 1_500.0 - 11.0, y / 1_500.0 + 6.0, 3, 0.5),
        )
        .max(self.sp_way(w));
        // A sharp step, laid back toward the unstepped lie where the ground
        // is gentle (a blend, so the change from one to the other is a slope).
        let (k, f) = bench_of(e);
        let stepped = BENCH_0 + BENCH * (k + smoothstep(0.975, 1.0, f));
        let mut land = stepped + (e.max(BENCH_0) - stepped) * gentle;
        // Inside the massif's outer edge the ground under it falls as the
        // gate's floor does, down to the lake's shore.
        if r < MASSIF_OUT {
            let gate = SHORE + (land - SHORE) * smoothstep(MASSIF_IN - 150.0, MASSIF_OUT, r);
            land += (gate - land) * smoothstep(MASSIF_OUT, MASSIF_OUT - 400.0, r);
        }
        // The valley, cut down to its floor: a cliff from the benches, or a
        // slope where the ground is gentle and from the low ground by the sea.
        let (b, t) = self.sp_valley(w);
        let drop = land - valley_floor(t);
        let fray = self.ridge.fbm(x / 700.0 + 3.0, y / 700.0 - 8.0, 3, 0.5) * FRAY;
        let run = 26.0 + 280.0 * (1.0 - smoothstep(14.0, 34.0, drop)) + 520.0 * gentle * gentle;
        let wide =
            valley_half(t) * (1.0 + 0.22 * self.lake.fbm(x / 2_600.0, y / 2_600.0 + 5.0, 2, 0.5));
        let valley = smoothstep(-0.5 * run, 0.5 * run, wide + fray - b);
        land += (valley_floor(t) - land) * valley;
        (land, valley)
    }

    /// The small relief, calmed round what must be built on or walked:
    /// rolling hills on the lowland, a swell on the benches and the floors,
    /// and here and there cone karst, steep round hills in clusters.
    fn sp_relief(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64), land: f64) -> f64 {
        let keep = self.sp_keep(q, w);
        let hills = self.tilt.fbm(x / 1_100.0, y / 1_100.0, 4, 0.5);
        let grain = self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5);
        let lowland = 1.0 - smoothstep(BENCH_0 + 6.0, BENCH_0 + BENCH, land);
        let amp = 6.0 + 18.0 * lowland;
        let mut h =
            land + amp * hills * keep.max(0.3) * smoothstep(3.0, 10.0, land) + 3.0 * grain * keep;
        // Cone karst: where its own field rises, away from the ways.
        let field = self
            .mtn_gap
            .fbm(x / 2_800.0 + 7.0, y / 2_800.0 - 2.0, 3, 0.5);
        let cones =
            smoothstep(0.18, 0.4, field) * smoothstep(0.6, 1.0, keep) * smoothstep(4.0, 12.0, land);
        if cones > 0.0 {
            let (f1, _, h1) = worley(self.seed ^ 0x636F_6E65, q, 170.0);
            let d = dist(q, f1);
            let radius = 70.0 + 40.0 * unit(h1, 0);
            let tall = (55.0 + 110.0 * unit(h1, 24)) * cones;
            let cone = smoothstep(radius, 0.0, d + 8.0 * grain);
            h += tall * cone.powf(0.8);
        }
        h
    }

    /// The sea and its islands, the river and its estuary, the lake and its
    /// island and fords, cut into the land.
    fn sp_water(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64), mut h: f64) -> f64 {
        let p = half(w);
        let r = w.0.hypot(w.1);
        let s = off_axis(p);
        let (b, t) = self.sp_valley(w);
        // The sea: headlands and coves, a beach falling to a shelf, then
        // deep; islands where its own field rises out of it. The coast keeps
        // its distance from the ore by it.
        let hold = smoothstep(150.0, 900.0, self.sp_ore_dist(q))
            * smoothstep(1_300.0, 2_200.0, self.sp_base_dist(q));
        let coast = self.sp_coast(s)
            + hold
                * (self.coast.fbm(x / 7_000.0, y / 7_000.0, 3, 0.5) * 2_000.0
                    + self.coast.fbm(x / 2_400.0 - 3.0, y / 2_400.0, 3, 0.5) * 500.0
                    + self.coast.fbm(x / 1_100.0 + 5.0, y / 1_100.0, 3, 0.55) * 320.0
                    + self.coast_warp.fbm(x / 340.0, y / 340.0, 3, 0.5) * 110.0);
        let sea = r - coast;
        if sea > -400.0 {
            let floor = -DEEP * smoothstep(0.0, 700.0, sea).powf(0.8)
                + 4.0 * self.lake.fbm(x / 600.0, y / 600.0, 3, 0.5) * smoothstep(80.0, 500.0, sea);
            let wet = smoothstep(-220.0, 25.0, sea);
            h = h * (1.0 - wet) + floor.min(0.2) * wet;
            // Islands, out past the shallows, clear of the estuaries.
            let isle = self
                .lake
                .fbm(x / 1_500.0 + 31.0, y / 1_500.0 - 17.0, 3, 0.5);
            let rise = smoothstep(0.32, 0.5, isle)
                * smoothstep(500.0, 900.0, sea)
                * smoothstep(ESTUARY.1 + 500.0, ESTUARY.1 + 900.0, b);
            if rise > 0.0 {
                h = h
                    .max(-DEEP + (DEEP + 6.0 + 30.0 * smoothstep(0.5, 0.7, isle)) * rise.powf(0.5));
            }
        }
        // The river down the valley's line, opening into the estuary.
        if t > LAKE_R - 300.0 {
            let (wide, deep, approach) = self.sp_channel(t);
            let wobble = self.lake_shore.fbm(x / 300.0, y / 300.0, 2, 0.5);
            let inside = wide + 0.25 * wide.min(200.0) * wobble - b;
            let bank = 18.0 + 0.3 * wide + 360.0 * approach;
            if inside > -bank {
                let bed = -deep * smoothstep(-4.0, 0.4 * wide + 8.0, inside);
                h += (bed.min(h) - h) * smoothstep(-bank, 0.0, inside);
            }
        }
        // The lake: coves and points, round the island.
        if r < LAKE_R + 900.0 {
            let shore = LAKE_R
                + self
                    .lake_shore
                    .fbm(x / 700.0 + 13.0, y / 700.0 - 7.0, 3, 0.5)
                    * 320.0;
            let lake = shore - r;
            let floor = (-0.6 - (LAKE_DEEP - 0.6) * smoothstep(0.0, 240.0, lake)).max(-LAKE_DEEP);
            let beach = 0.4 + (h - 0.4) * smoothstep(0.0, 160.0, -lake);
            h = h.min(beach.max(floor));
            h += (floor - h) * smoothstep(-6.0, 6.0, lake);
            // The island: low, level on top for building.
            let edge = ISLAND.0 + self.coast.fbm(x / 300.0 - 9.0, y / 300.0 + 2.0, 3, 0.5) * 90.0;
            let island = smoothstep(edge + 40.0, edge - 200.0, q.0.hypot(q.1));
            if island > 0.0 {
                h = h.max(-LAKE_DEEP + (ISLAND.1 + LAKE_DEEP) * island.powf(0.6));
            }
            // The fords out to the island, one from each gate.
            if (200.0..LAKE_R + 500.0).contains(&r) {
                let d = p.0 + 10.0 * self.detail.fbm(x / 90.0, y / 90.0, 2, 0.5);
                if d < LAKE_FORD_HALF + 80.0 {
                    let crest = 1.4
                        - (1.4 + LAKE_DEEP)
                            * smoothstep(LAKE_FORD_HALF - 15.0, LAKE_FORD_HALF + 80.0, d);
                    h = h.max(crest.min(SHORE));
                }
            }
        }
        h
    }

    /// How much of the massif stands at a (warped) design point, 0..1:
    /// inside its ring, off the valleys' walls and out of the gate; and
    /// outliers of it out on the benches, where their own field rises.
    fn sp_massif_mask(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64)) -> f64 {
        let p = half(w);
        let r = w.0.hypot(w.1);
        let fray = self.mtn_mask.fbm(x / 1_500.0, y / 1_500.0, 3, 0.5) * 900.0
            + self.mtn_mask.fbm(x / 4_000.0 + 9.0, y / 4_000.0, 2, 0.5) * 900.0;
        let (b, t) = self.sp_valley(w);
        let off_valley = smoothstep(
            valley_half(t) + 30.0,
            valley_half(t) + 380.0,
            b + 0.5 * fray,
        );
        let ring = smoothstep(MASSIF_IN - 100.0, MASSIF_IN + 300.0, r + 0.4 * fray)
            * smoothstep(MASSIF_OUT + 100.0, MASSIF_OUT - 300.0, r + fray)
            * smoothstep(
                GATE_HALF + 20.0,
                GATE_HALF + 300.0,
                p.0 + 50.0 * self.crag.fbm(x / 500.0 + 4.0, y / 500.0, 2, 0.5),
            );
        // Outliers: on the benches, not in the valleys, off the ways.
        let field = self.mtn.fbm(x / 2_300.0 - 3.0, y / 2_300.0 + 8.0, 3, 0.5);
        let out = smoothstep(0.28, 0.5, field)
            * smoothstep(0.7, 1.0, self.sp_keep(q, w))
            * smoothstep(MASSIF_OUT + 200.0, MASSIF_OUT + 600.0, r)
            * smoothstep(
                self.sp_coast(off_axis(p)) - 600.0,
                self.sp_coast(off_axis(p)) - 1_200.0,
                r,
            );
        (ring.max(0.7 * out)) * off_valley
    }

    /// The massif's height at a design point, or None off its blocks.
    fn sp_blocks(&self, q: (f64, f64), mask: f64) -> Option<f64> {
        let (f1, f2, h1) = worley(self.seed ^ 0x626C_6B73, q, BLOCK_CELL);
        // Walls stand back from the cell's edge by a gap that widens as the
        // massif thins, till only cores are left, then nothing.
        let rough = 5.0 * self.crag.fbm(q.0 / 40.0, q.1 / 40.0, 2, 0.5);
        let gap = 7.0 + 0.55 * BLOCK_CELL * (1.0 - mask).powf(1.4);
        let edge = bisector(q, f1, f2) + rough;
        let wall = smoothstep(gap, gap + 7.0, edge);
        if wall <= 0.0 {
            return None;
        }
        let peaks = self.mtn_height.fbm(f1.0 / 1_300.0, f1.1 / 1_300.0, 3, 0.5);
        let mut top =
            TABLE + (180.0 + 420.0 * mask * (0.6 + 0.4 * peaks)) * (0.55 + 0.45 * unit(h1, 0));
        // Half the blocks split into columns of their own heights.
        if unit(h1, 24) < 0.55 {
            let (c1, c2, ch) = worley(self.seed ^ 0x636F_6C73, q, COLUMN_CELL);
            let split = bisector(q, c1, c2) + 0.6 * rough;
            let slot = smoothstep(4.0, 9.0, split);
            if slot <= 0.0 {
                return None;
            }
            top *= 0.8 + 0.25 * unit(ch, 0);
            top = TABLE + (top - TABLE) * slot.max(0.25);
        }
        let crown = 6.0 * self.detail.fbm(q.0 / 60.0, q.1 / 60.0, 2, 0.5);
        Some((top + crown).min(PEAK) * wall)
    }

    /// The massif over the ground at a point: its blocks and columns, the
    /// ravines between them floored a little above the ground under them.
    fn sp_massif(&self, x: f64, y: f64, q: (f64, f64), w: (f64, f64), ground: f64) -> f64 {
        let mask = self.sp_massif_mask(x, y, q, w);
        if mask <= 0.0 {
            return ground;
        }
        let floor = ground + 70.0 * mask * mask;
        match self.sp_blocks(q, mask) {
            Some(top) => floor.max(top),
            None => floor,
        }
    }

    /// The pillars standing on their own over the ground at a point.
    fn sp_pillars(&self, q: (f64, f64), ground: f64) -> f64 {
        let mut rise: f64 = 0.0;
        for p in self.sp.pillars.near(q) {
            let d = dist(q, p.at);
            if d > p.radius + 60.0 {
                continue;
            }
            let a = (q.1 - p.at.1).atan2(q.0 - p.at.0);
            let lobes = p.radius
                * (0.16 * (3.0 * a + p.phase.0).sin() + 0.09 * (5.0 * a + p.phase.1).sin());
            let rough = 3.0 * self.crag.fbm(q.0 / 17.0, q.1 / 17.0, 2, 0.5);
            let e = d - p.radius - lobes + rough;
            let body = smoothstep(4.0, -6.0, e);
            let top = p.tall * (1.0 - 0.1 * (d / p.radius).min(1.2).powi(2))
                + 4.0 * self.detail.fbm(q.0 / 25.0, q.1 / 25.0, 2, 0.5);
            let skirt = 0.06 * p.tall * smoothstep(p.radius + 35.0, p.radius, d);
            rise = rise.max(top * body).max(skirt);
        }
        (ground + rise).min(PEAK)
    }

    /// The chance a pillar stands in a cell whose point is `at` (design),
    /// how large (a share of the full size) and how tall (least, most) it
    /// is: thick in the lake and on its shore, in drifts down the valleys,
    /// few on the benches, sea stacks off the coast.
    pub(super) fn sp_pillar_zone(&self, at: (f64, f64)) -> (f64, f64, (f64, f64)) {
        const NONE: (f64, f64, (f64, f64)) = (0.0, 0.0, (0.0, 0.0));
        let (x, y) = on_map(at);
        if !(0.0..SIZE).contains(&x) || !(0.0..SIZE).contains(&y) {
            return NONE;
        }
        let w = self.sp_warp(at);
        let r = w.0.hypot(w.1);
        let p = half(w);
        let (b, t) = self.sp_valley(w);
        if self.sp_keep(at, w) < 0.95 {
            return NONE;
        }
        if (MASSIF_IN - 200.0..MASSIF_OUT + 150.0).contains(&r) && b > valley_half(t) - 150.0 {
            return NONE;
        }
        // Drifts: pillars crowd in some places and leave others open.
        let drift = 0.4
            + 1.2
                * smoothstep(
                    -0.3,
                    0.4,
                    self.forest_kind.fbm(x / 1_400.0, y / 1_400.0, 2, 0.5),
                );
        let (land, valley) = self.sp_level(x, y, at, w);
        let channel = self.sp_channel(t).0;
        let coast = self.sp_coast(off_axis(p));
        let (chance, size, tall) = if at.0.hypot(at.1) < LAKE_R - 150.0 {
            // Out of the lake, round the island, clear of the fords.
            if at.0.hypot(at.1) < ISLAND.0 + 220.0 || p.0 < LAKE_FORD_HALF + 130.0 {
                NONE
            } else {
                (0.45, 1.0, (150.0, 320.0))
            }
        } else if r < MASSIF_IN + 100.0 {
            // The lake's shore, round the gate's mouth.
            (
                if p.0 < GATE_HALF + 250.0 { 0.0 } else { 0.3 },
                0.9,
                (120.0, 280.0),
            )
        } else if r > coast + 150.0 {
            // Stacks in the sea, near the coast, clear of the estuary's way.
            if r > coast + 1_400.0 || b < channel + 250.0 {
                NONE
            } else {
                (0.06, 0.8, (40.0, 140.0))
            }
        } else if r > coast - 250.0 {
            NONE
        } else if valley > 0.95 {
            // The valley floor, clear of the river and its banks.
            if b < channel + 160.0 {
                NONE
            } else {
                (0.15, 0.9, (90.0, 260.0))
            }
        } else if valley < 0.05 && land > 4.0 {
            // The benches.
            let forest = smoothstep(
                0.25,
                0.55,
                self.forest_kind.fbm(x / 2_200.0 - 5.0, y / 2_200.0, 3, 0.5),
            );
            (
                0.03 + 0.3 * forest,
                0.6 + 0.3 * forest,
                (40.0, 120.0 + 120.0 * forest),
            )
        } else {
            NONE
        };
        ((chance * drift).min(0.8), size, tall)
    }

    pub(in crate::bake) fn natural_spires(&self, x: f64, y: f64) -> f64 {
        let q = design((x, y));
        let w = self.sp_warp(q);
        let (land, _) = self.sp_level(x, y, q, w);
        let land = self.sp_relief(x, y, q, w, land);
        let h = self.sp_water(x, y, q, w, land);
        let h = self.sp_massif(x, y, q, w, h);
        self.sp_pillars(q, h)
    }
}

/// The nearest and second nearest of a jittered grid's points to `q`, and
/// the nearest's hash.
fn worley(seed: u64, q: (f64, f64), cell: f64) -> ((f64, f64), (f64, f64), u64) {
    let (ci, cj) = ((q.0 / cell).floor() as i64, (q.1 / cell).floor() as i64);
    let mut best = (f64::INFINITY, (0.0, 0.0), 0u64);
    let mut next = (f64::INFINITY, (0.0, 0.0));
    for dj in -1..=1 {
        for di in -1..=1 {
            let (i, j) = (ci + di, cj + dj);
            let hash = hash2(seed, i, j);
            let f = (
                (i as f64 + 0.15 + 0.7 * unit(hash, 0)) * cell,
                (j as f64 + 0.15 + 0.7 * unit(hash, 24)) * cell,
            );
            let d = dist(q, f);
            if d < best.0 {
                next = (best.0, best.1);
                best = (d, f, hash);
            } else if d < next.0 {
                next = (d, f);
            }
        }
    }
    (best.1, next.1, best.2)
}

/// Metres from `q` to the line halfway between `a` (the nearer) and `b`.
fn bisector(q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy).max(1e-6);
    let mid = (0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1));
    ((mid.0 - q.0) * dx + (mid.1 - q.1) * dy) / len
}

/// The bench a lie falls on, and how far up it toward the next, 0..1.
fn bench_of(e: f64) -> (f64, f64) {
    let v = (e - BENCH_0) / BENCH;
    let k = v.floor().max(0.0);
    (k, (v - k).clamp(0.0, 1.0))
}
