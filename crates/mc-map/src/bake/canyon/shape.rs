//! The canyon's landforms: the walls' beds, the inner gorge and the lake, the
//! dry valley's wash, erosion, the temples and the woods (`canyon.rs` lays
//! out the design they are read from).

use super::*;

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

/// Temples are sought one to a cell of this size.
const TEMPLE_CELL: f64 = 560.0;

/// How far from its ridge line a temple still shapes the ground: to where
/// its apron meets the bench's own fall (`wall_profile`), 1700 m in.
fn temple_reach(t: &Temple) -> f64 {
    t.top * 1.6 + (WALL_FOOT - t.cap) + (1_700.0 - WALL_FOOT) / 5.0 + 20.0
}

impl Terrain {
    /// How far a point lies from the dam's narrows and the ford, where the
    /// sides are one: 0 there, 1 away.
    fn apart(&self, x: f64, y: f64) -> f64 {
        let c = self.size_x / 2.0;
        let dam = ((x - c) / 1.6).hypot(y - TOE_V - 150.0);
        let ford = segment((x - c, y), (0.0, DELTA.0), (0.0, DELTA.1)).0;
        // The narrows' shoulders are the south bases' ground: the whole of
        // them is shared.
        smoothstep(1_500.0, 2_300.0, dam).min(smoothstep(800.0, 1_400.0, ford))
    }

    /// Metres in from the rim's edge (negative out on the plateau), buttes
    /// and temples included: each is a piece of the plateau left standing.
    /// The noise on it stays gentler than 1 m per m everywhere: steeper, and
    /// the distance folds, and the walls' clean bands break into blobs.
    pub(super) fn rim_depth(&self, x: f64, y: f64) -> f64 {
        let apart = self.apart(x, y);
        let wobble = self.coast.fbm(x / 2_600.0, y / 2_600.0, 3, 0.5) * 700.0 * apart
            + self.coast.fbm(x / 1_300.0 + 7.0, y / 1_300.0, 3, 0.5) * 550.0 * apart
            + self.coast_warp.fbm(x / 340.0, y / 340.0, 3, 0.5) * 110.0 * (0.4 + 0.6 * apart);
        let mut d = self.canyon.rim.at(x, y) + wobble + self.side(x) * self.canyon.rim_bias * apart;
        let lobe = self.ridge.fbm(x / 260.0, y / 260.0, 2, 0.5);
        for &(p, top, cap) in &self.canyon.buttes {
            let r = (x - p.0).hypot(y - p.1);
            d = d.min(cap + (r - top * (1.0 + 2.0 * lobe)).max(0.0));
        }
        let t = TEMPLE;
        let r = (x - self.size_x / 2.0).hypot(y - t.v);
        let lobe_t = self.ridge.fbm(x / 230.0 + 5.0, y / 230.0, 3, 0.5) * 1.6;
        d = d.min(t.cap + (r - t.top * (1.0 + 2.0 * lobe_t)).max(0.0));
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
            // leaving a talus apron, and is gone by `reach`.
            let r = segment((x, y), t.a, t.b).0;
            let own = t.cap + (r - t.top * (1.0 + 2.0 * lobe)).max(0.0);
            d = d.min(own + 4.0 * (own - WALL_FOOT).max(0.0));
        }
        // The walls stand back from the bench's way.
        for way in &self.canyon.ways {
            let (cd, _, _) = along((x, y), way);
            d = d.max(WALL_FOOT + 60.0 - (cd - 120.0).max(0.0));
        }
        d
    }

    /// Metres out from the water's edge (negative out in the lake).
    pub(super) fn shore_out(&self, x: f64, y: f64) -> f64 {
        let apart = self.apart(x, y);
        let wobble = self.lake_shore.fbm(x / 2_000.0, y / 2_000.0, 3, 0.5) * 900.0
            + self.lake_shore.fbm(x / 700.0, y / 700.0, 3, 0.5) * 240.0
            + self.lake_shore.fbm(x / 170.0 + 9.0, y / 170.0, 2, 0.5) * 55.0;
        // The river's slot meanders; the lake's wide water wanders more.
        let bend = self.lake.fbm(y / 1_500.0 + 3.3, 7.1, 3, 0.5) * 450.0 * apart;
        let water = self
            .canyon
            .lake
            .at(x, y)
            .max(self.canyon.river.at(x - bend, y));
        // The shore may widen the water freely but narrow it by no more
        // than 60 m, so no arm is cut off from the lake.
        let wide = apart * (1.0 - smoothstep(DELTA.0 + 200.0, DELTA.1, y));
        let mut inside =
            water + (wobble * wide).max(-60.0) + self.side(x) * self.canyon.shore_bias * wide;
        let t = TEMPLE;
        let r = (x - self.size_x / 2.0).hypot(y - t.v);
        let lobe = self.lake_shore.fbm(x / 400.0 - 7.0, y / 400.0, 3, 0.5) * 900.0;
        inside = inside.min(r - t.shore + lobe);
        for &(a, b, r) in &self.canyon.isles {
            let d = segment((x, y), a, b).0;
            inside = inside.min(d - r + wobble * 0.6);
        }
        // The water stands back from the bench's way.
        for way in &self.canyon.ways {
            let (cd, _, _) = along((x, y), way);
            inside = inside.min(cd - 150.0);
        }
        -inside
    }

    /// The walls from the rim down, and the bench below them.
    pub(super) fn wall_profile(&self, x: f64, y: f64, d: f64) -> f64 {
        if d <= 0.0 {
            // The plateau: rising gently back from the rim, rolling.
            let roll = self.tilt.fbm(x / 1_600.0, y / 1_600.0, 3, 0.5);
            let swell = self.mtn_height.fbm(x / 480.0, y / 480.0, 3, 0.5);
            let back = smoothstep(0.0, 2_800.0, -d);
            return RIM
                + 14.0
                + 30.0 * back
                + (38.0 * roll + 14.0 * swell) * smoothstep(0.0, 250.0, -d);
        }
        let mut h = RIM;
        for (k, bed) in BEDS.iter().enumerate() {
            let k = k as f64;
            let wander = self
                .crag
                .fbm(x / 420.0 + 7.3 * k, y / 420.0 - 3.1 * k, 3, 0.5)
                * bed.wander
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
    pub(super) fn gorge_profile(&self, x: f64, y: f64, s: f64) -> f64 {
        let cove = self
            .canyon
            .coves
            .iter()
            .map(|&(p, r)| 1.0 - smoothstep(0.45 * r, r, (x - p.0).hypot(y - p.1)))
            .fold(0.0, f64::max);
        let u = (x - self.size_x / 2.0).abs();
        let (dv0, dv1) = DELTA;
        let delta = (1.0 - smoothstep(650.0, 1_050.0, u))
            * smoothstep(dv0 - 700.0, dv0, y)
            * (1.0 - smoothstep(dv1, dv1 + 700.0, y));
        // The river above the delta runs shallow.
        let slot = smoothstep(dv1, dv1 + 300.0, y);
        if s < 0.0 {
            // The lake's bed: shelving off the shore, deepest down the old
            // river's channel; the heel of the dam stands in deep water.
            let off = -s;
            let deep = 4.0 + 0.16 * off.min(320.0);
            let wander = self.lake.fbm(x / 900.0, y / 900.0, 3, 0.5) * 220.0;
            let channel = 24.0 * (1.0 - smoothstep(180.0, 520.0, u + wander));
            let floor = -(4.0 + 0.4 * off.min(3.0)) * slot
                - (deep + channel * smoothstep(0.0, 200.0, off)) * (1.0 - slot);
            let shelf = -(0.03 * off.min(60.0) + 0.05 * (off - 60.0).max(0.0));
            let gentle = cove.max(delta);
            return floor * (1.0 - gentle) + shelf.max(floor) * gentle;
        }
        // The walls: sheer to the ring's top and a little over, then the bench.
        let run = 36.0 + 14.0 * self.ramp.fbm(x / 300.0, y / 300.0, 2, 0.5) * 5.0;
        let wall = (RING_TOP + 3.0) * cliff(s / run.max(14.0));
        let ledge = (GORGE_RIM - RING_TOP - 1.0) * smoothstep(run, run + 22.0, s);
        // Past the gorge's rim the bench is the walls' (`wall_profile`); this
        // side rises walkably out of its way, then steeply: only walls stand
        // that high this near the water.
        let on = (s - run - 22.0).max(0.0);
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

    /// The dry wash down the valley below the dam: a braided channel a few
    /// metres deep, cobbles and sand, above the old lake's ring.
    fn wash(&self, x: f64, y: f64, h: f64) -> f64 {
        if y > TOE_V + 50.0 {
            return h;
        }
        let c = self.size_x / 2.0;
        let line: Vec<(f64, f64)> = WASH.iter().map(|&(dx, v)| (c + dx, v)).collect();
        let (d, _, _) = along((x, y), &line);
        let meander = self.detail.fbm(x / 300.0, y / 300.0, 2, 0.5) * 120.0;
        let width = 80.0 + 60.0 * self.ramp.fbm(y / 700.0, x / 700.0, 2, 0.5) * 3.0;
        let braid = self.crag.ridged(x / 110.0, y / 260.0, 2, 0.5);
        let cut = 5.5 * (1.0 - smoothstep(0.35 * width, width, d + meander))
            + 2.0 * smoothstep(0.75, 0.95, braid) * (1.0 - smoothstep(width, 2.6 * width, d));
        let floor = RING_TOP + 3.0;
        h - cut.min((h - floor).max(0.0))
    }

    /// The canyon before erosion, trails and the dam.
    pub(super) fn canyon_shape(&self, x: f64, y: f64) -> f64 {
        let walls = self.wall_profile(x, y, self.rim_depth(x, y));
        let s = self.shore_out(x, y);
        let gorge = self.gorge_profile(x, y, s);
        let mut h = if s < 0.0 { gorge } else { walls.min(gorge) };
        if h > 2.0 {
            // Grain: washes and hummocks on the level ground.
            let wash = self.detail.ridged(x / 520.0, y / 520.0, 3, 0.5);
            h -= 5.0 * smoothstep(0.8, 0.97, wash) * smoothstep(2.0, 20.0, h);
            h += 1.6 * self.detail.fbm(x / 90.0 + 9.0, y / 90.0 - 4.0, 2, 0.5);
            h = self.wash(x, y, h);
        }
        h
    }

    /// 1 in open country, 0 on what must be built on or walked cleanly: the
    /// bases, the ore, the trails, the dam, the coves; erosion and the
    /// temples keep off them. Read off the grid `lay_keep` fills.
    pub(super) fn canyon_keep(&self, x: f64, y: f64) -> f64 {
        let n = (self.size_x / KEEP_STEP) as usize + 1;
        if self.canyon.keep.len() != n * n {
            return self.keep_at(x, y);
        }
        bicubic(&self.canyon.keep, n, n, KEEP_STEP, x, y).clamp(0.0, 1.0)
    }

    /// Fills the keep grid.
    pub(super) fn lay_keep(&mut self) {
        let n = (self.size_x / KEEP_STEP) as usize + 1;
        let mut keep = vec![0f32; n * n];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in keep.chunks_mut(rows * n).enumerate() {
                let t = &*self;
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(n).enumerate() {
                        let y = (k * rows + r) as f64 * KEEP_STEP;
                        for (i, v) in row.iter_mut().enumerate() {
                            *v = t.keep_at(i as f64 * KEEP_STEP, y) as f32;
                        }
                    }
                });
            }
        });
        self.canyon.keep = keep;
    }

    fn keep_at(&self, x: f64, y: f64) -> f64 {
        let near =
            |p: (f64, f64), reach: f64| smoothstep(reach, reach + 250.0, (x - p.0).hypot(y - p.1));
        let mut k = 1.0f64;
        for &s in STARTS {
            for p in self.mirror_sides(s) {
                k = k.min(near(p, 1.1 * self.start_outer));
            }
        }
        for &(u, v, r) in ORE {
            for p in self.both_sides((u, v)) {
                k = k.min(near(p, r + 60.0));
            }
        }
        for &(u, v, r) in RIM_ORE {
            for p in self.mirror_sides((u, v)) {
                k = k.min(near(p, r + 60.0));
            }
        }
        for &(p, r) in &self.canyon.coves {
            k = k.min(near(p, r));
        }
        k = k.min(near(
            (self.size_x / 2.0, TOE_V + 75.0),
            GORGE_DAM.length / 2.0 + 200.0,
        ));
        for trail in &self.canyon.trails {
            let (d, _, _) = along((x, y), &trail.line);
            k = k.min(smoothstep(trail.half + 120.0, trail.half + 320.0, d));
        }
        k
    }

    /// The ground as eroded, before the trails and the dam.
    pub(super) fn canyon_eroded(&self, x: f64, y: f64) -> f64 {
        let h = self.canyon_shape(x, y);
        if self.erosion.delta.is_empty() || h < 1.5 {
            return h;
        }
        let (er, n) = (&self.erosion, self.erosion.n);
        let e = bicubic(&er.delta, n, n, er.step, x, y)
            * self.canyon_keep(x, y)
            * smoothstep(1.5, 8.0, h);
        (h + e).max(h.min(1.5))
    }

    /// Water erosion over the whole canyon: gullies down the slopes between
    /// the cliffs, fans at their feet.
    pub(super) fn erode_canyon(&mut self) {
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
        super::super::alpine::erode(&mut h, n, self.seed);
        super::super::alpine::erode(&mut h, n, self.seed ^ 0x6361_6E79);
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
        let mut delta = raw.clone();
        for j in 1..n - 1 {
            for i in 1..n - 1 {
                let at = j * n + i;
                delta[at] =
                    0.5 * raw[at] + 0.125 * (raw[at - 1] + raw[at + 1] + raw[at - n] + raw[at + n]);
            }
        }
        self.erosion = super::super::alpine::Erosion {
            n,
            step: STEP,
            delta,
        };
    }

    /// Each side's shapes are its own: this moves one side's rim (out on the
    /// open plateau) and then its open shore until both sides hold the same
    /// plateau, then the same dry ground below the walls.
    pub(super) fn even_out_sides(&mut self) {
        const STEP: f64 = 48.0;
        let n = (self.size_x / STEP) as usize;
        let skew = |t: &Terrain, band: (f64, f64)| {
            let rows: Vec<(usize, usize)> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..n)
                    .map(|j| {
                        s.spawn(move || {
                            let y = (j as f64 + 0.5) * STEP;
                            let mut side = (0usize, 0usize);
                            for i in 0..n {
                                let x = (i as f64 + 0.5) * STEP;
                                if (x - t.size_x / 2.0).abs() < 400.0 {
                                    continue;
                                }
                                let h = t.canyon_eroded(x, y);
                                if h > band.0 && h < band.1 {
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
        // A positive bias moves the east's rim out: less plateau there.
        let (mut lo, mut hi) = (-200.0, 200.0);
        for _ in 0..10 {
            self.canyon.rim_bias = 0.5 * (lo + hi);
            if skew(self, (RIM - 2.0, f64::INFINITY)) < 0.0 {
                lo = self.canyon.rim_bias;
            } else {
                hi = self.canyon.rim_bias;
            }
        }
        // A positive bias widens the east's water: less bench there.
        let (mut lo, mut hi) = (-120.0, 120.0);
        for _ in 0..10 {
            self.canyon.shore_bias = 0.5 * (lo + hi);
            if skew(self, (1.0, BENCH_TOP + 30.0)) < 0.0 {
                lo = self.canyon.shore_bias;
            } else {
                hi = self.canyon.shore_bias;
            }
        }
    }

    /// Slides each east isle toward or away from its nearest cove until it
    /// lies as far from it as the west's twin from the west's: the coves are
    /// where each side's ships are built, so the sea war over the isles' ore
    /// is alike.
    pub(super) fn even_out_isles(&mut self) {
        let mid = |(a, b, _): IsleLaid| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let c = self.size_x / 2.0;
        let nearest = |p: (f64, f64), east: bool, coves: &[((f64, f64), f64)]| {
            coves
                .iter()
                .filter(|(q, _)| (q.0 > c) == east)
                .map(|&(q, _)| (seg_len(p, q), q))
                .fold((f64::INFINITY, p), |a, b| if b.0 < a.0 { b } else { a })
        };
        for k in (0..self.canyon.isles.len()).step_by(2) {
            let (dw, _) = nearest(mid(self.canyon.isles[k]), false, &self.canyon.coves);
            let (de, q) = nearest(mid(self.canyon.isles[k + 1]), true, &self.canyon.coves);
            let m = mid(self.canyon.isles[k + 1]);
            // Along the line to the cove: positive moves it nearer.
            let step = (de - dw).clamp(-400.0, 400.0) / de.max(1.0);
            let (dx, dy) = ((q.0 - m.0) * step, (q.1 - m.1) * step);
            let isle = &mut self.canyon.isles[k + 1];
            isle.0 = (isle.0 .0 + dx, isle.0 .1 + dy);
            isle.1 = (isle.1 .0 + dx, isle.1 .1 + dy);
        }
    }

    /// Thins the richer side's woods until both sides hold the same timber
    /// (measured as the woods' density, from the ground as baked).
    pub(super) fn even_out_woods(&mut self) {
        const STEP: f64 = 32.0;
        self.canyon.wood = [1.0, 1.0];
        let n = (self.size_x / STEP) as usize;
        let t = &*self;
        let rows: Vec<[f64; 2]> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..n)
                .map(|j| {
                    s.spawn(move || {
                        let y = (j as f64 + 0.5) * STEP;
                        let mut sum = [0.0; 2];
                        for i in 0..n {
                            let x = (i as f64 + 0.5) * STEP;
                            let h = t.height(x, y);
                            if h < 1.5 {
                                continue;
                            }
                            let density = t.canyon_forest(x, y, h, t.slope(x, y)).0;
                            sum[(x > t.size_x / 2.0) as usize] += density;
                        }
                        sum
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let sum = rows
            .iter()
            .fold([0.0; 2], |a, r| [a[0] + r[0], a[1] + r[1]]);
        let poor = sum[0].min(sum[1]).max(1e-9);
        self.canyon.wood = [poor / sum[0].max(1e-9), poor / sum[1].max(1e-9)];
    }

    /// Scatters the temples: in the canyon off the walls, on dry ground,
    /// nowhere near what must be built on or walked.
    pub(super) fn lay_temples(&mut self) {
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
                let into = self.canyon.rim.at(c.0, c.1);
                let dry = -self
                    .canyon
                    .lake
                    .at(c.0, c.1)
                    .max(self.canyon.river.at(c.0, c.1));
                if !(180.0..WALL_FOOT + 700.0).contains(&into)
                    || dry < 260.0
                    || self.apart(c.0, c.1) < 1.0
                {
                    continue;
                }
                // Tall near the rim, lower out toward the lake.
                let reach_in = ((into - 180.0) / (WALL_FOOT + 520.0)).clamp(0.0, 1.0);
                let cap = 20.0 + 420.0 * (0.25 * unit(hash, 24) + 0.75 * reach_in).min(1.0);
                let half = 260.0 * unit(hash, 40).powi(2);
                let (sa, ca) = (unit(hash, 48) * std::f64::consts::TAU).sin_cos();
                let t = Temple {
                    a: (c.0 - ca * half, c.1 - sa * half),
                    b: (c.0 + ca * half, c.1 + sa * half),
                    top: 30.0 + 90.0 * unit(hash, 32),
                    cap: cap.min(WALL_FOOT - 60.0),
                };
                let foot = temple_reach(&t) + half + 40.0;
                // Nothing that must be built on or walked anywhere under it.
                let k = (foot / 50.0).ceil() as i64;
                let clear = (-k..=k).all(|dj| {
                    (-k..=k).all(|di| {
                        let (dx, dy) = (di as f64 * 50.0, dj as f64 * 50.0);
                        dx * dx + dy * dy > foot * foot
                            || self.canyon_keep(c.0 + dx, c.1 + dy) >= 1.0
                    })
                });
                if clear {
                    temples.push(t);
                }
            }
        }
        self.canyon.temples = temples;
    }

    /// Pinyon and juniper woodland on the rim, scattered junipers down the
    /// walls and on the bench, cottonwoods by the water at the delta and the
    /// coves and along the dry wash. Timber is income: the sides' woods are
    /// held to parity by `tests/vermilion_gorge.rs`.
    pub(in crate::bake) fn canyon_forest(
        &self,
        x: f64,
        y: f64,
        height: f64,
        slope: f64,
    ) -> (f64, f64) {
        let stands = self.forest.fbm(x / 1_400.0, y / 1_400.0, 3, 0.5);
        let clumps = self.forest.fbm(x / 160.0 + 71.3, y / 160.0 - 19.1, 2, 0.5);
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
        let side = (x > self.size_x / 2.0) as usize;
        let mut density = (rim + walls + bench + water) * open * self.canyon.wood[side];
        for f in &self.ore {
            let d = (x - f.x).hypot(y - f.y) - f.radius;
            if d < 160.0 {
                density *= smoothstep(50.0, 160.0, d);
            }
        }
        (density, height)
    }

    /// The canyon's species by where they grow (`forest_density` hands the
    /// height over in the conifer slot).
    pub(in crate::bake) fn canyon_tree(&self, height: f64, hash: u64) -> PropKind {
        use crate::noise::unit;
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.05 => PropKind::TreeDead,
            _ if height < 14.0 => PropKind::TreeCottonwood,
            (_, pick) if height > RIM - 10.0 && pick < 0.55 => PropKind::TreePinyon,
            _ => PropKind::TreeJuniper,
        }
    }
}
