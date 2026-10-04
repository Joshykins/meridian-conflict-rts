//! Tripoint's ground: the benches and their escarpments, the lakes, each
//! climate's small relief, the mountains and their erosion.

use super::*;
use crate::bake::bays::polyline;
use crate::bake::frostline::shape::{mesa, mesa_run, Ground, WALL_H};

/// Level ground kept round a mountain's foot before its wall, and how far in
/// from the foot the wall takes to stand at [`WALL_H`], the height all three
/// climates share (too steep to climb from about 7 m up it).
const APRON: f64 = 40.0;
const WALL_RUN: f64 = 85.0;
/// How far the designed lines are bent out of true, metres at most, about.
const WARP: f64 = 300.0;
/// Metres either side of a wall over which one climate's relief hands over
/// to the next's, and the same for the mountains above the shared wall (a
/// sharp line: the Precursors' wall cuts a mountain in two).
const BLEND: f64 = 250.0;
const CUT: f64 = 60.0;
/// The mountains round the land: how high a crest stands, and how far in
/// from the wall's top it takes to get there.
const RING_TALL: f64 = 560.0;
const RING_REACH: f64 = 750.0;

/// Metres a design vector lies inside each third (negative outside it): the
/// distance to the nearer of the two walls that bound it, for a point
/// between them.
pub(super) fn depths(v: (f64, f64)) -> [f64; 3] {
    let r = v.0.hypot(v.1);
    let a = v.1.atan2(v.0);
    let mut out = [0.0; 3];
    for (k, d) in out.iter_mut().enumerate() {
        let axis = FRAC_PI_2 + k as f64 * TAU / 3.0;
        let off = (a - axis + PI).rem_euclid(TAU) - PI;
        *d = r * (FRAC_PI_3 - off.abs()).max(-FRAC_PI_2).sin();
    }
    out
}

/// Each climate's share of a point, summing to 1, handing over within
/// `blend` metres of a wall.
fn shares(v: (f64, f64), blend: f64) -> [f64; 3] {
    let w = depths(v).map(|d| smoothstep(-blend, blend, d));
    let sum = w.iter().sum::<f64>().max(1e-9);
    w.map(|s| s / sum)
}

impl Terrain {
    /// The largest of a designed field at a design point turned into each
    /// third: the north third's design laid on all three.
    fn tp_all(&self, q: (f64, f64), f: impl Fn((f64, f64)) -> f64) -> f64 {
        f(q).max(f(turn(q, 1))).max(f(turn(q, 2)))
    }

    /// Metres from a design point to the nearest base.
    fn tp_start_dist(&self, q: (f64, f64)) -> f64 {
        (0..3)
            .map(|k| dist(turn(q, k), START))
            .fold(f64::INFINITY, f64::min)
    }

    /// Metres from a design point to the nearest lake's middle.
    fn tp_lake_dist(&self, q: (f64, f64)) -> f64 {
        let c = vale_point((LAKE.0, LAKE.1));
        -self.tp_all(q, |p| -dist(p, c))
    }

    /// Metres from a design point to the edge of the nearest ore field
    /// away from the bases.
    fn tp_ore_dist(&self, q: (f64, f64)) -> f64 {
        let mut d = f64::INFINITY;
        for k in 0..3 {
            let p = turn(q, k);
            for &(ox, oy, r) in ORE {
                d = d.min(dist(p, (ox, oy)) - r);
            }
        }
        d
    }

    /// Where the designed lines are read for a point of the map: its place in
    /// the design, bent by a slow warp that is not the same turned, so each
    /// third's escarpments and mountains lie its own way. Still round the
    /// bases, the ore, the lakes (it would stretch one) and the plateau.
    fn tp_warp(&self, x: f64, y: f64) -> (f64, f64) {
        let p = design((x, y));
        let k = WARP
            * smoothstep(650.0, 1_600.0, self.tp_start_dist(p))
            * smoothstep(150.0, 600.0, self.tp_ore_dist(p))
            * smoothstep(500.0, 1_100.0, self.tp_lake_dist(p))
            * smoothstep(1_500.0, 2_400.0, p.0.hypot(p.1));
        if k <= 0.0 {
            return p;
        }
        (
            p.0 + k * self.warp_x.fbm(x / 2_400.0, y / 2_400.0, 2, 0.5),
            p.1 + k * self.warp_y.fbm(x / 2_400.0, y / 2_400.0, 2, 0.5),
        )
    }

    /// 1 in open country, falling to 0 round the bases and the ore: the small
    /// relief keeps off what must be built on.
    fn tp_keep(&self, p: (f64, f64)) -> f64 {
        smoothstep(560.0, 900.0, self.tp_start_dist(p))
            * smoothstep(30.0, 220.0, self.tp_ore_dist(p))
    }

    /// A noise field made the same in all three thirds: summed at the point
    /// and at the point turned each way, which keeps its spread and has no
    /// crease where the thirds meet. What decides where units can go (the
    /// feet of the mountains, the escarpments' rims) frays by this, so every
    /// third has the same ground; each climate's own relief does not.
    fn tp_sym(&self, x: f64, y: f64, f: impl Fn(f64, f64) -> f64) -> f64 {
        let p = design((x, y));
        (0..3)
            .map(|k| {
                let (mx, my) = on_map(turn(p, k));
                f(mx, my)
            })
            .sum::<f64>()
            / 3f64.sqrt()
    }

    /// How far an escarpment's rim stands back from its drawn line at a
    /// point, metres: alcoves and promontories, and now and then a gully run
    /// back into the ground behind. The same in every third.
    fn tp_rim(&self, x: f64, y: f64) -> f64 {
        self.tp_sym(x, y, |x, y| {
            let gully = smoothstep(
                0.8,
                0.97,
                self.ridge.ridged(x / 380.0 + 14.0, y / 380.0 - 3.0, 2, 0.5),
            );
            110.0 * self.ridge.fbm(x / 420.0 - 11.0, y / 420.0 + 6.0, 2, 0.5)
                + 40.0 * self.ridge.fbm(x / 150.0 + 3.0, y / 150.0 - 8.0, 2, 0.5)
                - 90.0 * gully
        })
    }

    /// The bench the land stands at, how much of a vale it is (0..1), metres
    /// inside the plateau (negative off it) and metres to the nearest
    /// escarpment that is not a ramp: the upland, cut down to the vales and
    /// raised to the plateau across escarpments that `fray` makes ragged,
    /// each laid back into a slope at its ramps.
    pub(super) fn tp_level(&self, q: (f64, f64), fray: f64) -> (f64, f64, f64, f64) {
        let tp = &self.tp;
        let low = self.tp_all(q, |p| inside(p, &tp.vale));
        let high = inside(q, &tp.plateau);
        let ramp = self.tp_all(q, |p| {
            tp.ramps
                .iter()
                .map(|&(a, b, r)| smoothstep(r + 110.0, r - 10.0, segment(p, a, b).0))
                .fold(0.0, f64::max)
        });
        let run = 28.0 + 340.0 * ramp;
        let fray = fray * (1.0 - ramp);
        let down = smoothstep(-0.5 * run, 0.5 * run, low + fray);
        // (The plateau's rim frays less: the bastions stand on it.)
        let up = smoothstep(-0.5 * run, 0.5 * run, high + 0.4 * fray);
        let edge = (low + fray).abs().min((high + 0.4 * fray).abs()) + 400.0 * ramp;
        let base = UPLAND + (LOW - UPLAND) * down;
        (base + (HIGH - base) * up, down, high, edge)
    }

    /// The low, wet places of Alaska's open country, each 0..1: kettle
    /// hollows and the lines the streams run in.
    fn tp_wet(&self, x: f64, y: f64) -> (f64, f64) {
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

    /// How much of a point is ice in Alaska's open country: its kettles and
    /// streams, frozen.
    pub(in crate::bake) fn tp_frozen(&self, x: f64, y: f64) -> f64 {
        let (kettle, stream) = self.tp_wet(x, y);
        smoothstep(0.55, 0.9, kettle).max(smoothstep(0.5, 0.9, stream))
            * self.tp_keep(design((x, y)))
    }

    /// The land with the lakes cut into it, and how much of the point lies
    /// in or by one (0..1): no small relief there, and no gullies. The
    /// waterline is drawn (frayed the same in every third), whatever the
    /// land round it does; its banks climb out of the water at 1 in 7.
    fn tp_lake(&self, x: f64, y: f64, q: (f64, f64), h: f64) -> (f64, f64) {
        let r = LAKE.2;
        let d = self.tp_lake_dist(q);
        if d > 2.0 * r {
            return (h, 0.0);
        }
        let d = d + self.tp_sym(x, y, |x, y| {
            30.0 * self.lake_shore.fbm(x / 240.0, y / 240.0, 2, 0.5)
        });
        let shore = 0.75 * r;
        let bed = if d < shore {
            LAKE_FLOOR * smoothstep(shore, 0.45 * r, d)
        } else {
            0.14 * (d - shore)
        };
        (h.min(bed), 1.0 - smoothstep(shore, shore + 200.0, d))
    }

    /// The broad lie of the land, each climate's own, metres about zero:
    /// swells and hollows a kilometre or two across, gentler in the vales.
    /// The hollows go no deeper than about 14 m: they are gentler in the vales
    /// than on the uplands, and a deep one would take the escarpment between
    /// down to a slope a unit could walk.
    fn tp_broad(&self, x: f64, y: f64, w: [f64; 3], low: f64) -> f64 {
        let c = &self.cont;
        let mut broad = 0.0;
        if w[ALASKA] > 0.0 {
            broad += w[ALASKA]
                * (38.0 * c.fbm(x / 2_000.0 - 21.0, y / 2_000.0 + 13.0, 3, 0.5)
                    + 28.0 * c.fbm(x / 700.0 + 15.0, y / 700.0 - 9.0, 2, 0.5));
        }
        if w[DESERT] > 0.0 {
            broad += w[DESERT]
                * (44.0 * c.fbm(x / 2_500.0 + 3.0, y / 2_500.0, 3, 0.5)
                    + 22.0 * c.fbm(x / 850.0 - 7.0, y / 850.0 + 5.0, 2, 0.5));
        }
        if w[JUNGLE] > 0.0 {
            broad += w[JUNGLE]
                * (34.0 * c.fbm(x / 1_800.0 + 41.0, y / 1_800.0 - 6.0, 3, 0.5)
                    + 24.0 * c.fbm(x / 600.0 - 33.0, y / 600.0 + 19.0, 2, 0.5));
        }
        if broad < 0.0 {
            broad = -14.0 * (1.0 - (broad / 14.0).exp());
        }
        broad * (1.0 - 0.62 * low)
    }

    /// The small relief, each climate's own, metres about zero: drumlins,
    /// hummocks, kettles, streams and tors in Alaska; dune fields in the
    /// desert's vales, slickrock swells and rock spires on its upland;
    /// rounded hills and steep limestone knolls under the jungle.
    fn tp_relief(&self, x: f64, y: f64, w: [f64; 3], low: f64) -> f64 {
        // A frame turned 35 degrees: the dunes' and drumlins' grain.
        let (u, v) = (x * 0.819 + y * 0.574, y * 0.819 - x * 0.574);
        // Rock standing alone, in scattered groups.
        let group = smoothstep(
            0.02,
            0.2,
            self.mtn_mask
                .fbm(x / 900.0 - 30.0, y / 900.0 + 12.0, 2, 0.5),
        );
        let spire = smoothstep(0.52, 0.64, self.crag.get(x / 85.0 + 2.0, y / 85.0 - 6.0)) * group;
        let mut relief = 0.0;
        if w[ALASKA] > 0.0 {
            let drumlin = self.detail.fbm(u / 180.0, v / 560.0, 2, 0.5);
            let hummock = 5.0 * self.tilt.fbm(x / 170.0, y / 170.0 + 9.0, 2, 0.5);
            let (kettle, stream) = self.tp_wet(x, y);
            relief += w[ALASKA]
                * (30.0 * (drumlin - 0.04).max(0.0) + hummock - 3.0 * kettle - 3.5 * stream
                    + 18.0 * spire);
        }
        if w[DESERT] > 0.0 {
            let field = smoothstep(
                -0.05,
                0.2,
                self.tilt.fbm(x / 1_700.0 + 9.0, y / 1_700.0 - 4.0, 2, 0.5),
            );
            // Dunes in ranks across the wind, their crests bending and forking.
            let bend = 2.4 * self.detail.fbm(u / 420.0, v / 420.0, 2, 0.5);
            let rank = 0.5 + 0.5 * (u / 22.0 + 2.6 * bend).sin();
            let tall =
                (0.55 + 1.5 * self.detail.fbm(u / 300.0 + 8.0, v / 900.0, 2, 0.5)).clamp(0.0, 1.0);
            let dunes = 9.0 * rank.powf(1.7) * tall * field * low;
            let swell = 6.0 * self.tilt.fbm(x / 230.0 + 5.0, y / 230.0, 2, 0.5);
            relief += w[DESERT] * (dunes + swell * (1.0 - 0.6 * low) + 30.0 * spire * (1.0 - low));
        }
        if w[JUNGLE] > 0.0 {
            let hills = 16.0
                * smoothstep(
                    -0.1,
                    0.5,
                    self.tilt.fbm(x / 520.0 + 3.0, y / 520.0 - 8.0, 3, 0.5),
                );
            let mound = 7.0 * self.detail.fbm(x / 190.0 - 2.0, y / 190.0 + 6.0, 2, 0.5);
            let knoll =
                smoothstep(0.45, 0.62, self.crag.get(x / 120.0 - 9.0, y / 120.0 + 4.0)) * group;
            relief += w[JUNGLE] * (hills * (1.0 - 0.5 * low) + mound + 34.0 * knoll * (1.0 - low));
        }
        relief
            + 0.9 * self.detail.fbm(x / 260.0, y / 260.0, 3, 0.5)
            + 0.3 * self.detail.fbm(x / 55.0 + 9.0, y / 55.0 - 4.0, 2, 0.5)
    }

    /// The mountains' height over land that stands at `base`: the ridges
    /// inside the ring and the ring itself beyond the land's outline
    /// (`beyond` metres past it). Each is the wall all three climates share,
    /// and above it the climate's own: peaks and arêtes in Alaska, mesas in
    /// the desert, steep rounded peaks in the jungle.
    fn tp_ranges(&self, x: f64, y: f64, q: (f64, f64), base: f64, beyond: f64) -> f64 {
        let w = shares(design((x, y)), CUT);
        // (The feet the same in every third.)
        let swell = self.tp_sym(x, y, |x, y| {
            self.mtn_mask
                .fbm(x / 1_500.0 + 4.0, y / 1_500.0 - 6.0, 2, 0.5)
        });
        let ragged =
            self.tp_sym(x, y, |x, y| self.mtn_gap.fbm(x / 520.0, y / 520.0, 2, 0.5)) * 170.0;
        let peaks = self.ridge.fbm(x / 1_100.0 + 17.0, y / 1_100.0, 3, 0.5);
        let arete = self.mtn.ridged(x / 800.0 + 3.3, y / 800.0 - 1.9, 4, 0.5);
        let spur = self.mtn.ridged(x / 290.0 - 6.1, y / 290.0 + 4.4, 3, 0.5);
        let rock = self.crag.ridged(x / 150.0 + 11.0, y / 150.0 - 5.0, 3, 0.5);
        let soft = self.mtn.fbm(x / 650.0 - 8.0, y / 650.0 + 3.0, 3, 0.5);
        let vary = self
            .mtn_height
            .fbm(x / 600.0 - 9.0, y / 600.0 + 2.0, 2, 0.5);
        let slick = self.crag.fbm(x / 120.0 + 5.0, y / 120.0 - 8.0, 3, 0.5);
        // A mountain `s` metres in from its foot, its crest `tall` over the
        // land, reached `reach` metres in from the wall's top.
        let mount = |s: f64, tall: f64, reach: f64| {
            if s <= 0.0 {
                return 0.0;
            }
            // Easing out of the apron and still climbing at its top, where
            // the climate's own mountain takes over: no ledge between them.
            let t = (s / WALL_RUN).min(1.0);
            let wall = WALL_H * t * t * (2.0 - t);
            let u = s - WALL_RUN;
            if u <= 0.0 {
                return wall;
            }
            let foot = base + wall;
            let m = (u / reach).min(1.0);
            let sharp = smoothstep(0.05, 0.6, m);
            let high = (tall * (1.0 + 0.9 * peaks).max(0.75) - WALL_H).max(0.0);
            let mut own = 0.0;
            if w[ALASKA] > 0.0 {
                let top = high
                    * (m.powf(0.8) * (0.5 + 0.95 * arete)
                        + 0.34 * sharp * m.sqrt() * (spur - 0.55))
                    + 26.0 * sharp * (rock - 0.45).max(0.0);
                own += w[ALASKA] * top.max(0.0);
            }
            if w[DESERT] > 0.0 {
                let run = mesa_run(foot) + u * (1.0 + 0.5 * vary);
                let top = mesa(run) - foot + 6.0 * slick * smoothstep(240.0, 300.0, run);
                own += w[DESERT] * top.max(0.0);
            }
            if w[JUNGLE] > 0.0 {
                let top = 0.85
                    * high
                    * (m.powf(0.65) * (0.8 + 0.45 * soft)
                        + 0.22 * sharp * m.sqrt() * (spur - 0.55));
                own += w[JUNGLE] * top.max(0.0);
            }
            wall + own
        };
        // Where two mountains meet they merge in a rounded saddle, not a crease.
        let join = |a: f64, b: f64| {
            let k = 30.0 * smoothstep(0.0, 60.0, a.min(b)) + 1e-9;
            let t = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
            a + (b - a) * t + k * t * (1.0 - t)
        };
        let ridge = |p: (f64, f64), r: &Ridge| {
            let from = polyline(p, r.line);
            if from > 1.4 * r.half + APRON + 100.0 {
                return 0.0;
            }
            // Narrowing to its ends, by the distance to the nearer one.
            let end = dist(p, r.line[0]).min(dist(p, r.line[r.line.len() - 1]));
            let half = r.half
                * (1.0 + 1.1 * swell).clamp(0.62, 1.35)
                * (0.55 + 0.45 * smoothstep(0.0, 1.3 * r.half, end));
            let len: f64 = r.line.windows(2).map(|w| dist(w[0], w[1])).sum();
            let crest = 0.6 + 0.4 * smoothstep(0.0, (0.45 * len).min(640.0), end);
            mount(
                half + APRON - from - ragged,
                r.tall * crest,
                r.half + APRON - WALL_RUN,
            )
        };
        let mut m = mount(beyond, RING_TALL, RING_REACH);
        for r in RIDGES {
            m = join(m, self.tp_all(q, |p| ridge(p, r)));
        }
        m
    }

    /// The land before erosion.
    pub(in crate::bake) fn tp_shape(&self, x: f64, y: f64) -> Ground {
        let p = design((x, y));
        let w = shares(p, BLEND);
        let q = self.tp_warp(x, y);
        let rim = self.tp_rim(x, y);
        let fray = 0.6 * rim
            + 190.0
                * self.tp_sym(x, y, |x, y| {
                    self.mtn_gap
                        .fbm(x / 1_300.0 - 4.0, y / 1_300.0 + 7.0, 2, 0.5)
                });
        let (level, low, high, edge) = self.tp_level(q, fray);
        // Off the plateau: it is level, the installation's bench.
        let off = 1.0 - smoothstep(-200.0, 0.0, high);
        let keep = self.tp_keep(p) * off;
        let lie = level + self.tp_broad(x, y, w, low) * off;
        let (cut, flat) = self.tp_lake(x, y, q, lie);
        // (Easing off toward a rim, so no dune or drumlin climbs an escarpment.)
        let land = cut
            + self.tp_relief(x, y, w, low)
                * keep
                * (1.0 - flat)
                * (0.3 + 0.7 * smoothstep(40.0, 140.0, edge));
        // Metres past the land's outline, where the ring of mountains rises.
        let beyond = -(inside(q, &self.tp.arena) + 0.6 * rim);
        let rise = self.tp_ranges(x, y, q, land, beyond);
        let open = keep
            * (1.0 - flat)
            * smoothstep(50.0, 130.0, edge)
            * (1.0 - smoothstep(1.0, 6.0, rise));
        Ground {
            h: land + rise,
            rise,
            open,
        }
    }

    pub(in crate::bake) fn natural_tripoint(&self, x: f64, y: f64) -> f64 {
        let g = self.tp_shape(x, y);
        // Erosion never comes down onto the shared wall, and the gullies keep
        // to the open country.
        let mut h = g.h + self.erosion.at(x, y) * smoothstep(WALL_H + 2.0, WALL_H + 20.0, g.rise);
        if g.open > 0.0 {
            // A gully digs no pond: it stops above the water.
            h = (h + self.tp.gullies.at(x, y) * g.open).max(h.min(2.5));
        }
        h + self.tp.pools.at(x, y)
    }

    /// Water erosion (`erode_benched`): couloirs and fans on the mountains
    /// above the shared wall (less in the desert's harder rock), the drainage
    /// over the open country, and the pools left in the mountains, filled.
    pub(super) fn erode_tripoint(&mut self) {
        let w = self.erode_benched(
            Terrain::tp_shape,
            |x, y| 1.0 - 0.6 * shares(design((x, y)), CUT)[DESERT],
            0x7472_6970,
        );
        self.erosion = w.mountains;
        self.tp.gullies = w.gullies;
        self.tp.pools = w.pools;
    }
}
