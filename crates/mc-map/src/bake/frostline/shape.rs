//! Frostline's ground: the coasts, the benches and their escarpments, the
//! canyon, the small relief of each climate, the mountains and their erosion.

use super::*;
use crate::bake::bays::{inside, polyline, segment};
use crate::bake::canyon::{BENCH_TOP, COCONINO_TOP, HERMIT_TOP, REDWALL_TOP, RIM, SUPAI_TOP};
use crate::landmark::FROSTLINE_STRATA_LIFT as LIFT;

/// Depth of the open ocean. Seabed installations need 20 m over their lot.
pub(super) const DEEP: f64 = 70.0;
/// Level ground kept round a mountain's foot before its wall, and how far in
/// from the foot the wall takes to stand at [`WALL_H`].
const APRON: f64 = 40.0;
const WALL_RUN: f64 = 85.0;
/// Height of the wall both climates share: too steep to climb from about
/// 7 m up it to its top, so nothing walks onto what stands above. Each
/// climate's own mountain begins where it ends.
pub(super) const WALL_H: f64 = 70.0;
/// How far the designed lines are bent out of true, metres at most, about.
const WARP: f64 = 420.0;
/// A canyon wall's run where it is sheer, and the grade it is laid back to
/// at a crossing.
const CLIFF_RUN: f64 = 22.0;
const CROSSING_GRADE: f64 = 0.27;
/// The dry lake's floor: low enough that the desert paints it pale silt.
const PLAYA_FLOOR: f64 = 4.2;
/// Water the coasts' noise cuts off from the oceans in a piece smaller than
/// this many square metres is a pool, and is filled to [`POOL_FLOOR`]: too
/// shallow for a ship, so nothing takes it for a sea to build a yard on.
const POOL_AREA: f64 = 500_000.0;
const POOL_FLOOR: f32 = -3.0;

/// A desert mountain's height at `run` metres in from where the big cliff
/// begins: the canyon's beds stacked as they are in Vermilion Gorge, lowered
/// by the strata lift, so the terrain shader's colours fall on the cliffs
/// and benches cut here. The Redwall cliff, the Supai's stair of ledges, the
/// Hermit slope, the Coconino cliff, the Kaibab cap, then the rim's gentle top.
pub(super) fn mesa(run: f64) -> f64 {
    let bed = |top: f64| top - LIFT;
    let mut knots = vec![(0.0, bed(BENCH_TOP)), (37.5, bed(REDWALL_TOP))];
    let step = (bed(SUPAI_TOP) - bed(REDWALL_TOP)) / 5.0;
    for _ in 0..5 {
        let (r, h) = *knots.last().unwrap();
        knots.push((r + 5.0, h + step * 0.7));
        knots.push((r + 27.0, h + step));
    }
    let (r, _) = *knots.last().unwrap();
    knots.extend([
        (r + 50.0, bed(HERMIT_TOP)),
        (r + 64.0, bed(COCONINO_TOP)),
        (r + 84.0, bed(RIM)),
    ]);
    let (last_r, last_h) = *knots.last().unwrap();
    if run >= last_r {
        return last_h + 0.08 * (run - last_r).min(220.0);
    }
    let i = knots
        .windows(2)
        .position(|w| run < w[1].0)
        .unwrap_or(knots.len() - 2);
    let (a, b) = (knots[i], knots[i + 1]);
    a.1 + (b.1 - a.1) * ((run - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
}

/// The run at which [`mesa`] stands at `height` on the big cliff.
pub(super) fn mesa_run(height: f64) -> f64 {
    let (foot, top) = (BENCH_TOP - LIFT, REDWALL_TOP - LIFT);
    37.5 * ((height - foot) / (top - foot)).clamp(0.0, 1.0)
}

/// The ground at a point, before erosion.
pub(in crate::bake) struct Ground {
    pub h: f64,
    /// How high a mountain stands over the land here (0 off the mountains).
    pub rise: f64,
    /// How much of the open country's erosion the point takes (0..1): none
    /// near a base, an ore field, an escarpment, a canyon or a cliff.
    pub open: f64,
}

/// A canyon's section at a point.
struct Section {
    /// The ground with the canyon cut into it.
    h: f64,
    /// How much of the point lies in or near it (0..1).
    near: f64,
}

impl Terrain {
    /// 0 west of the wall, 1 east of it, easing over across it: how much of
    /// Alaska's own shaping a point takes.
    fn fl_eastness(&self, x: f64, y: f64) -> f64 {
        smoothstep(-250.0, 250.0, east_of(x, y))
    }

    /// The larger of a designed field at a point and at its turned twin: the
    /// west's design laid on both sides.
    fn fl_both(&self, q: (f64, f64), f: impl Fn((f64, f64)) -> f64) -> f64 {
        f(q).max(f(self.turned(q)))
    }

    /// Metres to the nearest start, from a point of the design.
    fn fl_start_dist(&self, p: (f64, f64)) -> f64 {
        let t = self.turned(p);
        STARTS
            .iter()
            .map(|&s| dist(p, s).min(dist(t, s)))
            .fold(f64::INFINITY, f64::min)
    }

    /// Where the designed lines are read for a point of the map: its place
    /// in the design, bent out of true by a slow warp that is not the same
    /// turned, so the two sides' escarpments, canyons and mountains lie
    /// differently. Still round the bases.
    fn fl_warp(&self, x: f64, y: f64) -> (f64, f64) {
        let p = design((x, y));
        let k = WARP * smoothstep(650.0, 1_600.0, self.fl_start_dist(p));
        (
            p.0 + k * self.warp_x.fbm(x / 2_600.0, y / 2_600.0, 2, 0.5),
            p.1 + k * self.warp_y.fbm(x / 2_600.0, y / 2_600.0, 2, 0.5),
        )
    }

    /// 1 in open country, falling to 0 round the bases and the ore: the
    /// small relief keeps off what must be built on.
    fn fl_keep(&self, x: f64, y: f64) -> f64 {
        let p = design((x, y));
        let t = self.turned(p);
        let mut k = smoothstep(560.0, 900.0, self.fl_start_dist(p));
        for &(ox, oy, r) in ORE.iter().chain(ISLE_ORE) {
            let d = dist(p, (ox, oy)).min(dist(t, (ox, oy)));
            k = k.min(smoothstep(r + 30.0, r + 220.0, d));
        }
        k
    }

    /// Metres in from the shore (negative out to sea), no more than 3 km.
    /// Every shore is its own: the two oceans as drawn, pushed in and out by
    /// each climate's noise (broad bights in the desert; in Alaska, narrow
    /// inlets as well).
    fn fl_land(&self, x: f64, y: f64) -> f64 {
        let p = design((x, y));
        let fr = &self.frost;
        // (Further than 3 km from an ocean, it is not looked at.)
        let north = if p.1 > 5_850.0 {
            inside(p, &fr.north)
        } else {
            f64::MIN
        };
        let south = if p.1 < 10_550.0 {
            inside(p, &fr.south)
        } else {
            f64::MIN
        };
        let east = self.fl_eastness(x, y);
        // The coves keep their drawn shape, and the bases their shores.
        let cove = dist(p, COVE_HEAD).min(dist(self.turned(p), COVE_HEAD));
        let calm = (0.3 + 0.7 * smoothstep(1_100.0, 2_200.0, self.fl_start_dist(p)))
            * (0.08 + 0.92 * smoothstep(500.0, 1_100.0, cove));
        let mut wobble = 0.0;
        if east < 1.0 {
            wobble += (1.0 - east)
                * (self.coast.fbm(x / 1_400.0, y / 1_400.0, 4, 0.55) * 900.0
                    + self.coast_warp.fbm(x / 450.0, y / 450.0, 3, 0.5) * 500.0);
        }
        if east > 0.0 {
            wobble += east
                * (self
                    .coast
                    .fbm(x / 1_100.0 + 50.0, y / 1_100.0 - 20.0, 4, 0.55)
                    * 850.0
                    + (0.55 - self.coast_warp.ridged(x / 700.0, y / 700.0, 3, 0.5)) * 420.0);
        }
        let lobes = self.coast.fbm(x / 420.0 + 31.0, y / 420.0 - 17.0, 3, 0.5);
        let (cx, cy, r) = ISLE;
        let isle = r * (1.0 + 0.7 * lobes) - dist(p, (cx, cy)).min(dist(self.turned(p), (cx, cy)));
        (-north.max(south) + calm * wobble).max(isle).min(3_000.0)
    }

    /// How much of a cliff the coast near a point is (0 a beach, 1 a cliff).
    fn fl_cliffy(&self, x: f64, y: f64) -> f64 {
        let p = design((x, y));
        let d = CLIFFS
            .iter()
            .map(|line| polyline(p, line))
            .fold(f64::INFINITY, f64::min);
        // No cliff at a cove's head: the canyon comes out on a beach.
        let cove = dist(p, COVE_HEAD).min(dist(self.turned(p), COVE_HEAD));
        smoothstep(600.0, 250.0, d) * smoothstep(260.0, 620.0, cove)
    }

    /// The bench the land stands at, how much of the low country it is
    /// (0..1), and the metres to the nearest escarpment that is not a ramp:
    /// the upland, cut down to the low basin and the low bridge and raised to
    /// the high plateau across escarpments that `fray` makes ragged, each
    /// laid back into a slope at its ramps.
    fn fl_level(&self, q: (f64, f64), fray: f64) -> (f64, f64, f64) {
        let fr = &self.frost;
        let low = self.fl_both(q, |p| inside(p, &fr.basin).max(inside(p, BRIDGE)));
        let high = self.fl_both(q, |p| inside(p, &fr.plateau));
        let ramp = self.fl_both(q, |p| {
            RAMPS
                .iter()
                .map(|&(a, b, r)| smoothstep(r + 110.0, r - 10.0, segment(p, a, b).0))
                .fold(0.0, f64::max)
        });
        let run = 28.0 + 340.0 * ramp;
        let fray = fray * (1.0 - ramp);
        let down = smoothstep(-0.5 * run, 0.5 * run, low + fray);
        let up = smoothstep(-0.5 * run, 0.5 * run, high + fray);
        let edge = (low + fray).abs().min((high + fray).abs()) + 400.0 * ramp;
        (
            UPLAND + (LOW - UPLAND) * down + (HIGH - UPLAND) * up,
            down,
            edge,
        )
    }

    /// A canyon's plan at `along` metres up it: half the width of its floor
    /// (the main canyon opens out to the cove), half the width between its
    /// outer walls' feet, how far off the course its floor has wandered
    /// across the bench between them (to the left, going up), and how much
    /// of a crossing it is here (0..1). A side canyon is its floor alone.
    fn fl_plan(&self, c: &Course, along: f64) -> (f64, f64, f64, f64) {
        if !c.main {
            return (30.0, 30.0, 0.0, 0.0);
        }
        let cross = CROSSINGS
            .iter()
            .map(|&f| smoothstep(210.0, 120.0, (along - f * c.length()).abs()))
            .fold(0.0, f64::max);
        let inner = 46.0 + 52.0 * smoothstep(1_000.0, 0.0, along);
        let wide = 1.0 + 1.3 * self.mtn_height.fbm(along / 700.0, 3.5, 2, 0.5);
        let outer = (150.0 + 170.0 * smoothstep(2_800.0, 0.0, along)) * wide.clamp(0.7, 1.5);
        let room = (outer - inner - CLIFF_RUN - 14.0).max(0.0);
        let wander = (2.6 * self.mtn_height.fbm(along / 330.0 + 9.0, 7.5, 2, 0.5)).clamp(-1.0, 1.0)
            * room
            * smoothstep(0.0, 500.0, along)
            * (1.0 - cross);
        (inner, outer, wander, cross)
    }

    /// How much of a point is a canyon's floor, 0..1.
    pub(in crate::bake) fn fl_canyon_floor(&self, x: f64, y: f64) -> f64 {
        let q = self.fl_warp(x, y);
        let tq = self.turned(q);
        let mut floor: f64 = 0.0;
        for c in &self.frost.canyons {
            for p in [q, tq] {
                let Some((side, along)) = c.near(p, 200.0) else {
                    continue;
                };
                let (inner, _, wander, _) = self.fl_plan(c, along);
                let d = (side - wander).abs() + (along - c.length()).max(0.0);
                floor = floor.max(
                    (1.0 - smoothstep(inner - 14.0, inner + 2.0, d))
                        * (1.0 - smoothstep(0.8, 0.95, along / c.length())),
                );
            }
        }
        floor
    }

    /// The low, wet places of Alaska's open country, each 0..1: kettle
    /// hollows and the lines the streams run in. Frozen, all of them.
    fn fl_wet(&self, x: f64, y: f64) -> (f64, f64) {
        let kettle = smoothstep(
            0.30,
            0.46,
            self.lake.fbm(x / 300.0 + 3.0, y / 300.0, 2, 0.5),
        );
        let stream = smoothstep(
            0.86,
            0.96,
            self.lake.ridged(x / 720.0 + 20.0, y / 720.0, 2, 0.5),
        );
        (kettle, stream)
    }

    /// How much of a point is ice in Alaska's open country: the gorge's
    /// frozen river, the frozen lake, the kettles and the streams.
    pub(in crate::bake) fn fl_frozen(&self, x: f64, y: f64) -> f64 {
        let q = self.fl_warp(x, y);
        let lake = self.fl_both(q, |p| {
            PLAYAS
                .iter()
                .map(|&(cx, cy, r)| 1.0 - smoothstep(0.42 * r, 0.52 * r, dist(p, (cx, cy))))
                .fold(0.0, f64::max)
        });
        let (kettle, stream) = self.fl_wet(x, y);
        let small =
            smoothstep(0.55, 0.9, kettle).max(smoothstep(0.5, 0.9, stream)) * self.fl_keep(x, y);
        lake.max(self.fl_canyon_floor(x, y)).max(small)
    }

    /// How far a canyon's or an escarpment's rim stands back from its drawn
    /// line at a point, metres (negative: forward of it): alcoves and
    /// promontories a few hundred metres across, smaller bites out of
    /// those, and here and there a gully run well back into the ground behind.
    fn fl_rim(&self, x: f64, y: f64) -> f64 {
        let gully = smoothstep(
            0.78,
            0.97,
            self.ridge.ridged(x / 380.0 + 14.0, y / 380.0 - 3.0, 2, 0.5),
        );
        // (Gentle enough that a wall stays a wall: steeper noise would lay
        // it back, or fold it, where the two run together.)
        110.0 * self.ridge.fbm(x / 420.0 - 11.0, y / 420.0 + 6.0, 2, 0.5)
            + 40.0 * self.ridge.fbm(x / 150.0 + 3.0, y / 150.0 - 8.0, 2, 0.5)
            - 150.0 * gully
    }

    /// One canyon cut into ground that stands at `h`, `side` metres from its
    /// course (negative on its right): the main one in two steps (an outer
    /// cliff down to a bench, an inner gorge winding from side to side down
    /// to the floor), a side canyon in one. Sheer walls, their rims in
    /// alcoves and the width between them coming and going, laid back to a
    /// slope where the main canyon is to be crossed.
    fn fl_section(&self, x: f64, y: f64, c: &Course, side: f64, along: f64, h: f64) -> Section {
        let d = side.abs() + (along - c.length()).max(0.0);
        let floor_z = c.floor + c.grade * along;
        let depth = h - floor_z;
        if depth <= 0.0 {
            return Section { h, near: 0.0 };
        }
        let (inner, outer, wander, cross) = self.fl_plan(c, along);
        let fray = (1.0 - cross) * self.fl_rim(x, y);
        let (cut, rim) = if c.main {
            // Each step half the depth.
            let run = CLIFF_RUN + (0.5 * depth / CROSSING_GRADE - CLIFF_RUN).max(0.0) * cross;
            let outer = outer.max(inner + run + 12.0);
            let w = 0.5 * smoothstep(inner, inner + run, (side - wander).abs() + 0.3 * fray)
                + 0.5 * smoothstep(outer, outer + run, d + fray);
            (floor_z + depth * w, outer + run)
        } else {
            let w = smoothstep(inner, inner + CLIFF_RUN, d + 0.5 * fray);
            (floor_z + depth * w, inner + CLIFF_RUN)
        };
        Section {
            h: cut,
            near: 1.0 - smoothstep(rim + 60.0, rim + 180.0, d + fray),
        }
    }

    /// The land with the canyons and the dry lake cut into it, and how much
    /// of the point lies in or near either (0..1): no small relief there,
    /// and no gullies.
    fn fl_cut(&self, x: f64, y: f64, q: (f64, f64), mut h: f64) -> (f64, f64) {
        let mut flat: f64 = 0.0;
        let tq = self.turned(q);
        for c in &self.frost.canyons {
            for p in [q, tq] {
                let Some((side, along)) = c.near(p, 900.0) else {
                    continue;
                };
                let s = self.fl_section(x, y, c, side, along, h);
                h = s.h;
                flat = flat.max(s.near);
            }
        }
        let fray = 50.0 * self.lake_shore.fbm(x / 240.0, y / 240.0, 2, 0.5);
        let lake = self.fl_both(q, |p| {
            PLAYAS
                .iter()
                .map(|&(cx, cy, r)| smoothstep(r, 0.5 * r, dist(p, (cx, cy)) + fray))
                .fold(0.0, f64::max)
        });
        if lake > 0.0 && h > PLAYA_FLOOR {
            h += (PLAYA_FLOOR - h) * lake;
            flat = flat.max(smoothstep(0.0, 0.3, lake));
        }
        (h, flat)
    }

    /// The broad lie of each landmass, its own, metres about zero: swells
    /// and vales a kilometre or two across, gentler in the low country.
    fn fl_broad(&self, x: f64, y: f64, east: f64, low: f64) -> f64 {
        let mut broad = 0.0;
        if east < 1.0 {
            broad += (1.0 - east)
                * (44.0 * self.cont.fbm(x / 2_500.0 + 3.0, y / 2_500.0, 3, 0.5)
                    + 22.0 * self.cont.fbm(x / 850.0 - 7.0, y / 850.0 + 5.0, 2, 0.5));
        }
        if east > 0.0 {
            broad += east
                * (38.0
                    * self
                        .cont
                        .fbm(x / 2_000.0 - 21.0, y / 2_000.0 + 13.0, 3, 0.5)
                    + 28.0 * self.cont.fbm(x / 700.0 + 15.0, y / 700.0 - 9.0, 2, 0.5));
        }
        broad * (1.0 - 0.62 * low)
    }

    /// The small relief, each climate's own, metres about zero: dune fields
    /// in the desert's low country, slickrock swells and rock spires on its
    /// upland; drumlins, hummocks, kettles, streams and tors in Alaska.
    fn fl_relief(&self, x: f64, y: f64, east: f64, low: f64) -> f64 {
        // A frame turned 35 degrees: the dunes' and drumlins' grain.
        let (u, v) = (x * 0.819 + y * 0.574, y * 0.819 - x * 0.574);
        // Spires and tors: rock standing alone, in scattered groups.
        let group = smoothstep(
            0.02,
            0.2,
            self.mtn_mask
                .fbm(x / 900.0 - 30.0, y / 900.0 + 12.0, 2, 0.5),
        );
        let spire = smoothstep(0.52, 0.64, self.crag.get(x / 85.0 + 2.0, y / 85.0 - 6.0)) * group;
        let mut relief = 0.0;
        if east < 1.0 {
            let field = smoothstep(
                -0.05,
                0.2,
                self.tilt.fbm(x / 1_700.0 + 9.0, y / 1_700.0 - 4.0, 2, 0.5),
            );
            // Dunes in ranks across the wind, their crests bending and
            // forking, taller in some reaches than others.
            let bend = 2.4 * self.detail.fbm(u / 420.0, v / 420.0, 2, 0.5);
            let rank = 0.5 + 0.5 * (u / 22.0 + 2.6 * bend).sin();
            let tall =
                (0.55 + 1.5 * self.detail.fbm(u / 300.0 + 8.0, v / 900.0, 2, 0.5)).clamp(0.0, 1.0);
            let dunes = 9.0 * rank.powf(1.7) * tall * field * low;
            let swell = 6.0 * self.tilt.fbm(x / 230.0 + 5.0, y / 230.0, 2, 0.5);
            relief +=
                (1.0 - east) * (dunes + swell * (1.0 - 0.6 * low) + 30.0 * spire * (1.0 - low));
        }
        if east > 0.0 {
            let drumlin = self.detail.fbm(u / 180.0, v / 560.0, 2, 0.5);
            let hummock = 5.0 * self.tilt.fbm(x / 170.0, y / 170.0 + 9.0, 2, 0.5);
            let (kettle, stream) = self.fl_wet(x, y);
            relief += east
                * (30.0 * (drumlin - 0.04).max(0.0) + hummock - 3.0 * kettle - 3.5 * stream
                    + 18.0 * spire);
        }
        relief
            + 0.9 * self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5)
            + 0.3 * self.detail.fbm(x / 55.0 + 9.0, y / 55.0 - 4.0, 2, 0.5)
    }

    /// The land meeting the sea: long beaches, or cliffs with stacks off
    /// them. `across` is `s` as true metres across the shore where it is a cliff.
    fn fl_shore(&self, x: f64, y: f64, s: f64, across: f64, land: f64, cliffy: f64) -> f64 {
        let run = 240.0 - 195.0 * cliffy;
        // The sea floor: a shelf off the beaches, a drop off the cliffs, sand bars.
        let shelf = 700.0 - 450.0 * cliffy;
        let mut sea = -DEEP * smoothstep(0.0, shelf, -s).powf(0.8);
        sea += 5.0 * self.lake.fbm(x / 700.0, y / 700.0, 3, 0.5) * smoothstep(-60.0, -500.0, s);
        let w = smoothstep(-25.0, run, across);
        let h = sea.min(0.2) * (1.0 - w) + land * w;
        if cliffy <= 0.0 {
            return h;
        }
        // Sea stacks off the cliffs.
        let stack = self.crag.get(x / 170.0 - 7.0, y / 170.0 + 3.0);
        let band = smoothstep(-520.0, -260.0, s) * (1.0 - smoothstep(-90.0, -30.0, s));
        let rise = smoothstep(0.5, 0.66, stack) * band * cliffy;
        // Only the stacks themselves rise: the sea floor round them keeps its depth.
        h.max(-8.0 + 40.0 * rise - 200.0 * (1.0 - smoothstep(0.0, 0.15, rise)))
    }

    /// The mountains' height over land that stands at `base`: the wall both
    /// climates share, and above it each climate's own mountain.
    fn fl_ranges(&self, x: f64, y: f64, q: (f64, f64), base: f64, east: f64) -> f64 {
        // Each mountain swells and pinches along its crest and frays in
        // buttresses and alcoves. (Two broad octaves: finer ones squeeze and
        // stretch the wall's run until it can be walked in places.)
        let swell = self
            .mtn_mask
            .fbm(x / 1_500.0 + 4.0, y / 1_500.0 - 6.0, 2, 0.5);
        let ragged = self.mtn_gap.fbm(x / 520.0, y / 520.0, 2, 0.5) * 170.0;
        // The passes stay open whatever the outlines do, their sides as
        // frayed as any mountain's foot.
        let pass = self.fl_both(q, |p| {
            PASSES
                .iter()
                .map(|&(a, b, r)| r - segment(p, a, b).0)
                .fold(f64::MIN, f64::max)
        }) + 0.5 * ragged;
        let peaks = self.ridge.fbm(x / 1_100.0 + 17.0, y / 1_100.0, 3, 0.5);
        let arete = self.mtn.ridged(x / 800.0 + 3.3, y / 800.0 - 1.9, 4, 0.5);
        let spur = self.mtn.ridged(x / 290.0 - 6.1, y / 290.0 + 4.4, 3, 0.5);
        let rock = self.crag.ridged(x / 150.0 + 11.0, y / 150.0 - 5.0, 3, 0.5);
        let vary = self
            .mtn_height
            .fbm(x / 600.0 - 9.0, y / 600.0 + 2.0, 2, 0.5);
        let slick = self.crag.fbm(x / 120.0 + 5.0, y / 120.0 - 8.0, 3, 0.5);
        let mount = |p: (f64, f64), r: &Ridge, pass: f64| {
            let from = polyline(p, r.line);
            if from > 1.4 * r.half + APRON + 100.0 {
                return 0.0;
            }
            // Narrowing to its ends, by the distance to the nearer one.
            let end = dist(p, r.line[0]).min(dist(p, r.line[r.line.len() - 1]));
            let half = r.half
                * (1.0 + 1.1 * swell).clamp(0.62, 1.35)
                * (0.55 + 0.45 * smoothstep(0.0, 1.3 * r.half, end));
            // Metres in from the foot.
            let s = (half + APRON - from - ragged).min(-pass);
            if s <= 0.0 {
                return 0.0;
            }
            // Easing out of the apron and still climbing at its top, where
            // the mountain takes over: no ledge between them.
            let t = (s / WALL_RUN).min(1.0);
            let wall = WALL_H * t * t * (2.0 - t);
            let u = s - WALL_RUN;
            if u <= 0.0 {
                return wall;
            }
            let foot = base + wall;
            let mut own = 0.0;
            if east < 1.0 {
                // A mesa: the beds, their benches wider here, narrower there.
                let run = mesa_run(foot) + u * (1.0 + 0.5 * vary);
                let top = mesa(run) - foot + 6.0 * slick * smoothstep(240.0, 300.0, run);
                own += (1.0 - east) * top.max(0.0);
            }
            if east > 0.0 {
                // A range: peaks and cols along the crest, arêtes and spurs
                // growing from the wall's top to the crest.
                let len: f64 = r.line.windows(2).map(|w| dist(w[0], w[1])).sum();
                let crest = 0.6 + 0.4 * smoothstep(0.0, (0.45 * len).min(640.0), end);
                let tall = (r.tall * crest * (1.0 + 0.9 * peaks).max(0.75) - WALL_H).max(0.0);
                let m = (u / (r.half + APRON - WALL_RUN)).min(1.0);
                let sharp = smoothstep(0.05, 0.6, m);
                let top = tall
                    * (m.powf(0.8) * (0.5 + 0.95 * arete)
                        + 0.34 * sharp * m.sqrt() * (spur - 0.55))
                    + 26.0 * sharp * (rock - 0.45).max(0.0);
                own += east * top.max(0.0);
            }
            wall + own
        };
        // Where two mountains meet they merge in a rounded saddle, not a crease.
        let join = |a: f64, b: f64| {
            let k = 30.0 * smoothstep(0.0, 60.0, a.min(b)) + 1e-9;
            let t = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
            a + (b - a) * t + k * t * (1.0 - t)
        };
        let tq = self.turned(q);
        let mut m = 0.0;
        for r in RIDGES {
            m = join(m, mount(q, r, pass).max(mount(tq, r, pass)));
        }
        let (p, tp) = ((x, y), self.turned((x, y)));
        for r in EDGE_RANGES {
            m = join(m, mount(p, r, f64::MIN).max(mount(tp, r, f64::MIN)));
        }
        m
    }

    /// The land before erosion.
    pub(in crate::bake) fn fl_shape(&self, x: f64, y: f64) -> Ground {
        let s = self.fl_land(x, y);
        if s < -700.0 {
            // Open sea: nothing but its floor.
            return Ground {
                h: self.fl_shore(x, y, s, s, 0.0, 0.0),
                rise: 0.0,
                open: 0.0,
            };
        }
        let cliffy = self.fl_cliffy(x, y);
        let q = self.fl_warp(x, y);
        let east = self.fl_eastness(x, y);
        let keep = self.fl_keep(x, y);
        let fray = 0.6 * self.fl_rim(x, y)
            + 190.0
                * self
                    .mtn_gap
                    .fbm(x / 1_300.0 - 4.0, y / 1_300.0 + 7.0, 2, 0.5);
        let (level, low, edge) = self.fl_level(q, fray);
        let lie = level
            + 2.5 * smoothstep(150.0, 3_000.0, s)
            + self.fl_broad(x, y, east, low) * smoothstep(40.0, 500.0, s);
        let (cut, flat) = self.fl_cut(x, y, q, lie);
        let land = cut + self.fl_relief(x, y, east, low) * keep * (1.0 - flat);
        // The coast's wobble stretches `s` here and squeezes it there, which
        // would lay a cliff back into a slope units can walk: a cliff is cut
        // across true metres, by how fast `s` changes over the ground.
        let mut across = s;
        if cliffy > 0.0 && s.abs() < 300.0 {
            let e = 6.0;
            let gx = (self.fl_land(x + e, y) - self.fl_land(x - e, y)) / (2.0 * e);
            let gy = (self.fl_land(x, y + e) - self.fl_land(x, y - e)) / (2.0 * e);
            let g = gx.hypot(gy).clamp(0.25, 1.5);
            across = s * (1.0 + cliffy * (1.0 / g - 1.0));
        }
        let h = self.fl_shore(x, y, s, across, land, cliffy);
        // The mountains stop at the water: they fall into it in cliffs.
        let rise = if s > -350.0 {
            self.fl_ranges(x, y, q, land, east) * smoothstep(-350.0, 80.0, s)
        } else {
            0.0
        };
        let open = keep
            * (1.0 - flat)
            * smoothstep(50.0, 130.0, edge)
            * smoothstep(40.0 + 120.0 * cliffy, 160.0 + 120.0 * cliffy, s)
            * (1.0 - smoothstep(1.0, 6.0, rise));
        Ground {
            h: h + rise,
            rise,
            open,
        }
    }

    pub(in crate::bake) fn natural_frostline(&self, x: f64, y: f64) -> f64 {
        let g = self.fl_shape(x, y);
        // Erosion never comes down onto the shared wall, and the gullies
        // keep to the open country (their samples are 16 m apart: the
        // weights are taken here, at the point itself).
        let mut h = g.h + self.erosion.at(x, y) * smoothstep(WALL_H + 2.0, WALL_H + 20.0, g.rise);
        if g.open > 0.0 {
            // A gully digs no pond: it stops above the water.
            h = (h + self.frost.gullies.at(x, y) * g.open).max(h.min(2.5));
        }
        h + self.frost.pools.at(x, y)
    }

    /// Water erosion. Over the mountains above the shared wall: couloirs and
    /// fans on the ranges, gullies down the mesas' slopes. Over the open
    /// country: the drainage, shallow valleys gathering into washes and
    /// streams, no steeper anywhere than a unit can walk. And the pools the
    /// coasts' noise cut off from the oceans, filled.
    pub(super) fn erode_frostline(&mut self) {
        const STEP: f64 = 16.0;
        /// Metres per unit of height while the droplets run: the mountains
        /// flattened to the slopes the droplets are tuned for, the open
        /// country less.
        const VERTICAL: f32 = 48.0;
        const GENTLE: f32 = 5.0;
        let n = (self.size_x / STEP) as usize + 1;
        let mut raw = vec![0f32; n * n];
        let mut rise = vec![0f32; n * n];
        let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, (hs, rs)) in raw
                .chunks_mut(rows * n)
                .zip(rise.chunks_mut(rows * n))
                .enumerate()
            {
                let t = &*self;
                s.spawn(move || {
                    for (r, (row, rrow)) in hs.chunks_mut(n).zip(rs.chunks_mut(n)).enumerate() {
                        let y = (k * rows + r) as f64 * STEP;
                        for (i, (v, rv)) in row.iter_mut().zip(rrow.iter_mut()).enumerate() {
                            let g = t.fl_shape(i as f64 * STEP, y);
                            *v = g.h as f32;
                            *rv = g.rise as f32;
                        }
                    }
                });
            }
        });
        self.frost.pools = crate::bake::alpine::Erosion {
            n,
            step: STEP,
            delta: pool_fill(&raw, n, (POOL_AREA / (STEP * STEP)) as usize),
        };
        // The droplets run over the land: the sea is a level floor to them.
        let h: Vec<f32> = raw.iter().map(|v| v.max(-2.0)).collect();
        let soften = |delta: &mut Vec<f32>, passes: usize| {
            for _ in 0..passes {
                let from = delta.clone();
                for j in 1..n - 1 {
                    for i in 1..n - 1 {
                        let at = j * n + i;
                        delta[at] = 0.5 * from[at]
                            + 0.125 * (from[at - 1] + from[at + 1] + from[at - n] + from[at + n]);
                    }
                }
            }
        };

        // The mountains.
        let mut high: Vec<f32> = h.iter().map(|v| v / VERTICAL).collect();
        crate::bake::alpine::erode(&mut high, n, self.seed);
        crate::bake::alpine::erode(&mut high, n, self.seed ^ 0x6672_6F73);
        let mut delta: Vec<f32> = (0..n * n)
            .map(|at| {
                let (x, y) = ((at % n) as f64 * STEP, (at / n) as f64 * STEP);
                // Only above the wall, and the desert's rock is harder.
                let own = smoothstep(WALL_H + 15.0, WALL_H + 60.0, rise[at] as f64);
                let k = own * (0.4 + 0.6 * self.fl_eastness(x, y));
                (high[at] * VERTICAL - h[at]).clamp(-70.0, 14.0) * k as f32
            })
            .collect();
        // Softened, so single droplets do not scratch thin straight lines.
        soften(&mut delta, 2);
        self.erosion = crate::bake::alpine::Erosion {
            n,
            step: STEP,
            delta,
        };

        // The open country.
        let mut low: Vec<f32> = h.iter().map(|v| v / GENTLE).collect();
        let before = low.clone();
        crate::bake::alpine::erode(&mut low, n, self.seed ^ 0x6775_6C6C);
        crate::bake::alpine::erode(&mut low, n, self.seed ^ 0x7761_7368);
        // Nothing left steeper than a unit can climb, gully sides included.
        let talus = 0.38 * STEP as f32 / GENTLE;
        for _ in 0..12 {
            for j in 1..n - 1 {
                for i in 1..n - 1 {
                    let at = j * n + i;
                    for nb in [at - 1, at + 1, at - n, at + n] {
                        let drop = low[at] - low[nb];
                        let was = before[at] - before[nb];
                        if drop > talus && drop > was {
                            let moved = (drop - talus.max(was)) * 0.25;
                            low[at] -= moved;
                            low[nb] += moved;
                        }
                    }
                }
            }
        }
        let mut gullies: Vec<f32> = low
            .iter()
            .zip(&before)
            .map(|(a, b)| ((a - b) * GENTLE).clamp(-11.0, 5.0))
            .collect();
        soften(&mut gullies, 1);
        let cut = gullies.iter().filter(|d| **d < -1.5).count() as f64 / gullies.len() as f64;
        debug_assert!(cut > 0.01, "water cut no gullies: {cut}");
        self.frost.gullies = crate::bake::alpine::Erosion {
            n,
            step: STEP,
            delta: gullies,
        };
    }
}

/// How far to raise each sample of `raw` (heights, row-major, `n` per edge)
/// to fill the pools: every piece of water under a ship's keel that is
/// joined to fewer than `least` samples of the same, raised to
/// [`POOL_FLOOR`], and its rim with it.
fn pool_fill(raw: &[f32], n: usize, least: usize) -> Vec<f32> {
    // (A ship floats in 6 m; a little shallower counts, so no deep cell is
    // left between the samples of a pool's rim.)
    let deep = |at: usize| raw[at] < -4.5;
    let around = |at: usize| {
        let (i, j) = (at % n, at / n);
        [
            (i > 0).then(|| at - 1),
            (i + 1 < n).then(|| at + 1),
            (j > 0).then(|| at - n),
            (j + 1 < n).then(|| at + n),
        ]
        .into_iter()
        .flatten()
    };
    let mut fill = vec![0f32; n * n];
    let mut seen = vec![false; n * n];
    for from in 0..n * n {
        if seen[from] || !deep(from) {
            continue;
        }
        seen[from] = true;
        let mut piece = vec![from];
        let mut next = 0;
        while next < piece.len() {
            for nb in around(piece[next]) {
                if !seen[nb] && deep(nb) {
                    seen[nb] = true;
                    piece.push(nb);
                }
            }
            next += 1;
        }
        if piece.len() >= least {
            continue;
        }
        for &at in &piece {
            fill[at] = POOL_FLOOR - raw[at];
            for nb in around(at) {
                fill[nb] = (POOL_FLOOR - raw[nb]).max(0.0);
            }
        }
    }
    fill
}
