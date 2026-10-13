//! Vela's plan: its roads, Port Calder's streets and blocks, the spaceport,
//! the quarry, the miners' camp, the guns over the bridge, the farms and the
//! islands' hamlets.

use std::f64::consts::{FRAC_PI_2, PI};

use super::super::canyon::smooth_open;
use super::super::siege::grid::{dist, Buckets, Rect, Seg, P};
use super::super::siege::plan::{footprint, origin_for};
use super::super::Terrain;
use super::shape::{river_distance, ISLANDS, QUARRY_RAMP, RISE};
use super::*;
use crate::format::{Ground, PropKind, Road};
use crate::noise::{hash2, unit};

/// Road widths: centreline to kerb, and the pavement beyond.
const COUNTRY: (f64, f64) = (6.0, 0.0);
const STREET: (f64, f64) = (4.5, 3.0);
const LANE: (f64, f64) = (3.0, 0.0);
/// Lots keep this far apart.
const GAP: f64 = 3.0;

/// The coast road, from the beach to the bridge's west end, and from its east
/// end into the town.
const COAST_WEST: &[P] = &[
    BEACH_START,
    (1_520.0, 5_880.0),
    (1_900.0, 5_760.0),
    (2_250.0, 5_690.0),
    (2_650.0, 5_650.0),
    (2_980.0, 5_615.0),
    (BRIDGE_X - BRIDGE_HALF_SPAN, ROAD_Y),
];
const COAST_EAST: &[P] = &[
    (BRIDGE_X + BRIDGE_HALF_SPAN, ROAD_Y),
    (3_700.0, 5_570.0),
    (4_050.0, 5_560.0),
    (4_420.0, 5_520.0),
    (4_800.0, 5_640.0),
    (4_900.0, 5_700.0),
];
/// Where the PDF meets the Company, and the road south from it.
const JUNCTION: P = (2_250.0, 5_690.0);
const SOUTH_WEST: &[P] = &[
    JUNCTION,
    (2_330.0, 4_900.0),
    (2_280.0, 4_000.0),
    (2_450.0, 3_000.0),
    (2_700.0, 2_000.0),
    (2_850.0, 1_000.0),
    (2_900.0, 40.0),
];
/// The town's road south, through the eastern farms.
const SOUTH_EAST: &[P] = &[
    (5_550.0, 5_420.0),
    (5_450.0, 4_800.0),
    (5_200.0, 4_000.0),
    (4_900.0, 3_000.0),
    (4_700.0, 2_000.0),
    (4_650.0, 1_000.0),
    (4_700.0, 40.0),
];
/// The harbour road: out of the town's south-east corner and up to the apron.
const HARBOUR_ROAD: &[P] = &[
    (5_760.0, 5_900.0),
    (5_930.0, 5_820.0),
    (6_175.0, 5_650.0),
    (6_380.0, 5_330.0),
    (6_600.0, 5_150.0),
    (6_680.0, 5_090.0),
];
/// The way from the apron down to the quarry's rim.
const QUARRY_ROAD: &[P] = &[
    (7_320.0, 4_200.0),
    (7_320.0, 3_960.0),
    QUARRY_RAMP.0,
    QUARRY_RAMP.1,
];

/// Port Calder: its square, where its streets meet.
const SQUARE_C: P = (5_690.0, 5_960.0);
/// Where the town's streets run: through these points, each line rounded
/// into a curve and nudged off true. The coast road's end, the high street
/// down to the harbour, the quay, the shore street, the east street, the
/// north side street the PDF's column comes down.
const STREETS: &[&[P]] = &[
    &[
        (4_900.0, 5_700.0),
        (5_100.0, 5_790.0),
        (5_320.0, 5_850.0),
        (5_500.0, 5_905.0),
        (5_610.0, 5_945.0),
    ],
    &[
        (5_700.0, 6_030.0),
        (5_725.0, 6_160.0),
        (5_700.0, 6_300.0),
        (5_735.0, 6_405.0),
    ],
    &[
        (5_060.0, 6_340.0),
        (5_280.0, 6_410.0),
        (5_480.0, 6_400.0),
        (5_735.0, 6_405.0),
        (5_990.0, 6_405.0),
        (6_200.0, 6_455.0),
        (6_400.0, 6_560.0),
    ],
    &[
        (5_180.0, 5_815.0),
        (5_130.0, 6_000.0),
        (5_110.0, 6_200.0),
        (5_150.0, 6_370.0),
    ],
    &[
        (5_775.0, 5_970.0),
        (5_950.0, 6_010.0),
        (6_150.0, 6_070.0),
        (6_340.0, 6_160.0),
        (6_420.0, 6_320.0),
    ],
    &[
        (6_160.0, 6_440.0),
        (6_175.0, 6_250.0),
        (6_150.0, 6_070.0),
        (6_140.0, 5_850.0),
        (6_175.0, 5_650.0),
    ],
    &[
        (5_680.0, 5_895.0),
        (5_645.0, 5_760.0),
        (5_595.0, 5_600.0),
        (5_550.0, 5_420.0),
    ],
];
/// The back lanes between them.
const LANES: &[&[P]] = &[
    &[
        (5_320.0, 5_850.0),
        (5_300.0, 5_700.0),
        (5_380.0, 5_560.0),
        (5_550.0, 5_500.0),
    ],
    &[
        (5_645.0, 5_760.0),
        (5_800.0, 5_705.0),
        (5_950.0, 5_735.0),
        (6_080.0, 5_660.0),
    ],
    &[
        (5_420.0, 5_905.0),
        (5_430.0, 6_080.0),
        (5_510.0, 6_200.0),
        (5_700.0, 6_300.0),
    ],
    &[
        (5_950.0, 6_010.0),
        (5_930.0, 6_180.0),
        (5_990.0, 6_300.0),
        (6_165.0, 6_330.0),
    ],
    &[(5_100.0, 5_790.0), (5_150.0, 5_650.0), (5_290.0, 5_560.0)],
    &[(6_340.0, 6_160.0), (6_330.0, 5_980.0), (6_250.0, 5_820.0)],
];
/// An alley: a narrow way between buildings.
const ALLEY: (f64, f64) = (3.5, 1.5);

/// The miners' camp: its yard's middle and half size.
pub(super) const CAMP: (P, f64, f64) = ((4_420.0, 5_330.0), 72.0, 48.0);

/// The rebels' checkpoints on the coast road.
const CHECKPOINTS: [P; 4] = [
    (1_950.0, 5_750.0),
    (2_700.0, 5_645.0),
    (4_150.0, 5_552.0),
    (4_820.0, 5_650.0),
];

/// Farmsteads: by the roads through the farms, the side of the road they stand.
const FARMSTEADS: [(P, f64); 10] = [
    ((2_330.0, 4_600.0), 1.0),
    ((2_290.0, 3_700.0), -1.0),
    ((2_420.0, 3_100.0), 1.0),
    ((2_600.0, 2_350.0), -1.0),
    ((2_820.0, 1_400.0), 1.0),
    ((5_420.0, 4_600.0), -1.0),
    ((5_300.0, 4_200.0), 1.0),
    ((5_030.0, 3_400.0), -1.0),
    ((4_780.0, 2_500.0), 1.0),
    ((4_660.0, 1_500.0), -1.0),
];

/// Wind turbines on the southern hills.
const TURBINES: [P; 6] = [
    (1_500.0, 2_300.0),
    (1_760.0, 2_050.0),
    (2_000.0, 1_800.0),
    (6_100.0, 1_900.0),
    (6_420.0, 2_150.0),
    (6_760.0, 1_750.0),
];

struct Planner {
    seed: u64,
    roads: Vec<Seg>,
    roads_index: Buckets,
    lots: Vec<Lot>,
    lots_index: Buckets,
    areas: Vec<Area>,
    dressing: Vec<Dressing>,
}

impl Terrain {
    pub(super) fn lay_vela_plan(&mut self) {
        let mut plan = Planner {
            seed: self.seed,
            roads: Vec::new(),
            roads_index: Buckets::new(SIZE),
            lots: Vec::new(),
            lots_index: Buckets::new(SIZE),
            areas: Vec::new(),
            dressing: Vec::new(),
        };
        plan.roads();
        plan.town();
        plan.spaceport();
        plan.quarry();
        plan.camp();
        plan.guns();
        plan.farms(self);
        plan.islands();
        plan.country();

        // Each lot cut level to the mean of the ground at its corners and middle.
        let mut lots = plan.lots;
        for lot in &mut lots {
            let mut sum = self.vela_land(lot.rect.c.0, lot.rect.c.1);
            for c in lot.rect.corners() {
                sum += self.vela_land(c.0, c.1);
            }
            lot.level = sum / 5.0;
        }
        let mut lot_index = Buckets::new(SIZE);
        for (i, l) in lots.iter().enumerate() {
            let (lo, hi) = l.rect.bounds();
            let r = LOT_EASE;
            lot_index.insert(i as u32, (lo.0 - r, lo.1 - r), (hi.0 + r, hi.1 + r));
        }
        let mut area_index = Buckets::new(SIZE);
        for (i, a) in plan.areas.iter().enumerate() {
            let (lo, hi) = a.rect.bounds();
            area_index.insert(i as u32, lo, hi);
        }
        self.vela = Vela {
            roads: plan.roads,
            road_index: Some(plan.roads_index),
            lots,
            lot_index: Some(lot_index),
            areas: plan.areas,
            area_index: Some(area_index),
            dressing: plan.dressing,
        };
    }

    /// Whether `p` is farmland: the country south of the coast either side of
    /// the river, off the hills' woods, the river's banks, the town, the
    /// spaceport and the quarry.
    pub(super) fn vl_farmed(&self, p: P) -> bool {
        let west = p.0 > 1_150.0 && p.0 < 3_150.0 && p.1 > 1_100.0 && p.1 < 5_300.0;
        let east = p.0 > 3_850.0 && p.0 < 6_300.0 && p.1 > 800.0 && p.1 < 4_950.0;
        if !(west || east) || river_distance(p) < 180.0 {
            return false;
        }
        let ((px, py), hx, hy) = PLATEAU;
        if (p.0 - px).abs() < hx + 300.0 && (p.1 - py).abs() < hy + 300.0 {
            return false;
        }
        if dist(p, QUARRY.0) < QUARRY.1 + 200.0 {
            return false;
        }
        let ((cx, cy), chx, chy) = CAMP;
        if (p.0 - cx).abs() < chx + 60.0 && (p.1 - cy).abs() < chy + 60.0 {
            return false;
        }
        // Woods break the farms up.
        self.forest.fbm(p.0 / 900.0, p.1 / 900.0, 4, 0.5) < 0.12
    }
}

/// An upright rectangle round `c`.
fn square(c: P, hx: f64, hy: f64) -> Rect {
    Rect {
        c,
        hx,
        hy,
        heading: 0.0,
    }
}

impl Planner {
    fn hash(&self, salt: u64, p: P) -> u64 {
        hash2(self.seed ^ salt, p.0.round() as i64, p.1.round() as i64)
    }

    fn road(&mut self, s: Seg) {
        let r = s.reach();
        let lo = (s.a.0.min(s.b.0) - r, s.a.1.min(s.b.1) - r);
        let hi = (s.a.0.max(s.b.0) + r, s.a.1.max(s.b.1) + r);
        self.roads_index.insert(self.roads.len() as u32, lo, hi);
        self.roads.push(s);
    }

    /// A polyline of road, in pieces of at most 64 m.
    fn road_line(&mut self, line: &[P], road: Road, (half, walk): (f64, f64)) {
        for w in line.windows(2) {
            let n = (dist(w[0], w[1]) / 64.0).ceil().max(1.0) as usize;
            for k in 0..n {
                let at = |t: f64| {
                    (
                        w[0].0 + (w[1].0 - w[0].0) * t,
                        w[0].1 + (w[1].1 - w[0].1) * t,
                    )
                };
                self.road(Seg {
                    a: at(k as f64 / n as f64),
                    b: at((k + 1) as f64 / n as f64),
                    half,
                    road,
                    walk,
                });
            }
        }
    }

    fn area(&mut self, rect: Rect, ground: Ground) {
        self.areas.push(Area { rect, ground });
    }

    fn dress(&mut self, kind: PropKind, at: P, heading: f64, scale: f64) {
        self.dressing.push(Dressing {
            kind,
            at,
            heading,
            scale,
        });
    }

    /// Lays a structure without asking.
    fn place(&mut self, kind: PropKind, at: P, heading: f64) {
        let rect = footprint(kind, at, heading, 1.0);
        let (lo, hi) = rect.bounds();
        self.lots_index.insert(self.lots.len() as u32, lo, hi);
        self.lots.push(Lot {
            kind,
            at,
            heading,
            rect,
            level: 0.0,
        });
    }

    /// Lays a structure whose footprint's middle is `c`, if it keeps off the
    /// roads and the other lots.
    fn try_place(&mut self, kind: PropKind, c: P, heading: f64) -> bool {
        let at = origin_for(kind, c, heading, 1.0);
        let rect = footprint(kind, at, heading, 1.0);
        let (lo, hi) = rect.bounds();
        if lo.0 < 24.0 || lo.1 < 24.0 || hi.0 > SIZE - 24.0 || hi.1 > SIZE - 24.0 {
            return false;
        }
        let radius = rect.hx.hypot(rect.hy);
        if self
            .roads_index
            .near(rect.c, radius + 40.0)
            .map(|i| &self.roads[i as usize])
            .any(|s| rect.distance_to_segment(s.a, s.b) < s.reach() + 1.5)
        {
            return false;
        }
        if self
            .lots_index
            .near(rect.c, radius + 40.0)
            .any(|i| self.lots[i as usize].rect.near(&rect, GAP))
        {
            return false;
        }
        self.place(kind, at, heading);
        true
    }

    // -- roads ----------------------------------------------------------------

    fn roads(&mut self) {
        self.road_line(COAST_WEST, Road::Highway, COUNTRY);
        self.road_line(COAST_EAST, Road::Highway, COUNTRY);
        self.road_line(SOUTH_WEST, Road::Highway, COUNTRY);
        self.road_line(SOUTH_EAST, Road::Highway, COUNTRY);
        self.road_line(HARBOUR_ROAD, Road::Highway, COUNTRY);
        self.road_line(QUARRY_ROAD, Road::Highway, (5.0, 0.0));
        // The bridge: its road carried on the map's deck.
        self.dress(
            PropKind::ColonyBridge,
            (BRIDGE_X - BRIDGE_HALF_SPAN, ROAD_Y),
            0.0,
            1.0,
        );
    }

    // -- the town ---------------------------------------------------------------

    /// A town street through `points`: rounded, nudged off true, laid.
    fn street(&mut self, points: &[P], road: Road, width: (f64, f64), salt: u64) -> Vec<P> {
        let nudged: Vec<P> = points
            .iter()
            .enumerate()
            .map(|(k, &p)| {
                // The ends stay where they join other ways.
                if k == 0 || k + 1 == points.len() {
                    return p;
                }
                let h = self.hash(salt, p);
                (
                    p.0 + 24.0 * (unit(h, 0) - 0.5),
                    p.1 + 24.0 * (unit(h, 16) - 0.5),
                )
            })
            .collect();
        let line = smooth_open(&nudged, 5);
        self.road_line(&line, road, width);
        line
    }

    fn town(&mut self) {
        let mut lines = Vec::new();
        for (k, points) in STREETS.iter().enumerate() {
            lines.push((
                self.street(points, Road::Street, STREET, 0x7374 + k as u64),
                STREET,
            ));
        }
        for (k, points) in LANES.iter().enumerate() {
            lines.push((
                self.street(points, Road::Lane, ALLEY, 0x6c61 + k as u64),
                ALLEY,
            ));
        }
        // The square: paved shapes laid askew over each other where the
        // streets meet, a rough octagon.
        for (hx, hy, turn) in [
            (92.0, 66.0, 0.18),
            (78.0, 78.0, 0.18 + std::f64::consts::FRAC_PI_4),
        ] {
            self.area(
                Rect {
                    c: SQUARE_C,
                    hx,
                    hy,
                    heading: turn,
                },
                Ground::Paving,
            );
        }
        // The quay's hard standing.
        self.area(
            Rect {
                c: (5_735.0, 6_395.0),
                hx: 560.0,
                hy: 34.0,
                heading: 0.02,
            },
            Ground::Yard,
        );
        // The ops tower over the square, facing it: the first spot round the
        // square's north side clear of the streets.
        'tower: for r in [120.0, 140.0, 165.0, 190.0] {
            for a in [0.0_f64, 0.35, -0.35, 0.7, -0.7] {
                let (s, c) = (FRAC_PI_2 + a).sin_cos();
                let at = (SQUARE_C.0 + c * r, SQUARE_C.1 + s * r);
                // Its front (local +y) to the square.
                if self.try_place(PropKind::ColonyTower, at, PI + a) {
                    break 'tower;
                }
            }
        }
        for (k, (line, width)) in lines.iter().enumerate() {
            self.frontage(line, *width, 0x6672 + k as u64);
        }
        self.infill();
        self.street_lights();
        self.town_barricades(&lines);
    }

    /// What the town builds at `c`: warehouses on the quay, flats round the
    /// square and up the slope behind it, habs beyond, a depot here and there.
    fn town_kind(&self, c: P, roll: f64) -> PropKind {
        let d = dist(c, SQUARE_C);
        if c.1 > 6_330.0 {
            if roll < 0.75 {
                PropKind::ColonyDepot
            } else {
                PropKind::ColonyHabStack
            }
        } else if d < 280.0 {
            if roll < 0.7 {
                PropKind::ColonyHabStack
            } else {
                PropKind::ColonyHab
            }
        } else if d < 560.0 {
            match roll {
                r if r < 0.55 => PropKind::ColonyHab,
                r if r < 0.85 => PropKind::ColonyHabStack,
                _ => PropKind::ColonyDepot,
            }
        } else if roll < 0.88 {
            PropKind::ColonyHab
        } else {
            PropKind::ColonyDepot
        }
    }

    /// Buildings along both sides of a street, each facing it, set back by a
    /// little more or less, turned a little off square, with gaps between.
    fn frontage(&mut self, line: &[P], (half, walk): (f64, f64), salt: u64) {
        let lengths: Vec<f64> = line.windows(2).map(|w| dist(w[0], w[1])).collect();
        let total: f64 = lengths.iter().sum();
        let at = |s: f64| -> (P, P) {
            let mut left = s;
            for (k, w) in line.windows(2).enumerate() {
                if left <= lengths[k] || k + 2 == line.len() {
                    let t = (left / lengths[k].max(1e-6)).clamp(0.0, 1.0);
                    let d = (
                        (w[1].0 - w[0].0) / lengths[k].max(1e-6),
                        (w[1].1 - w[0].1) / lengths[k].max(1e-6),
                    );
                    return (
                        (
                            w[0].0 + (w[1].0 - w[0].0) * t,
                            w[0].1 + (w[1].1 - w[0].1) * t,
                        ),
                        d,
                    );
                }
                left -= lengths[k];
            }
            (line[line.len() - 1], (1.0, 0.0))
        };
        for side in [1.0_f64, -1.0] {
            let mut s = 8.0 + 20.0 * unit(self.hash(salt ^ side.to_bits(), line[0]), 0);
            while s < total - 8.0 {
                let (p, d) = at(s);
                let h = self.hash(salt ^ side.to_bits(), (p.0 * 3.1, p.1 * 1.7));
                if unit(h, 0) < 0.14 {
                    s += 18.0 + 20.0 * unit(h, 8);
                    continue;
                }
                let n = (-d.1 * side, d.0 * side);
                let kind = self.town_kind((p.0 + n.0 * 30.0, p.1 + n.1 * 30.0), unit(h, 16));
                let probe = footprint(kind, (0.0, 0.0), 0.0, 1.0);
                let setback = 2.0 + 6.0 * unit(h, 24);
                let off = half + walk + setback + probe.hy;
                let c = (p.0 + n.0 * off, p.1 + n.1 * off);
                // Its front (local +y) to the street, a little askew.
                let heading = n.0.atan2(-n.1) + 0.08 * (unit(h, 32) - 0.5);
                if self.try_place(kind, c, heading) {
                    // A row behind it, here and there: back plots off the street.
                    if unit(h, 48) < 0.65 {
                        let back = footprint(PropKind::ColonyHab, (0.0, 0.0), 0.0, 1.0);
                        let off = off + probe.hy + 8.0 + 10.0 * unit(h, 52) + back.hy;
                        let c2 = (p.0 + n.0 * off, p.1 + n.1 * off);
                        self.try_place(
                            PropKind::ColonyHab,
                            c2,
                            heading + 0.1 * (unit(h, 56) - 0.5),
                        );
                    }
                    if kind == PropKind::ColonyDepot {
                        let r = footprint(kind, origin_for(kind, c, heading, 1.0), heading, 1.0);
                        self.area(
                            Rect {
                                hx: r.hx + 6.0,
                                hy: r.hy + 6.0,
                                ..r
                            },
                            Ground::Yard,
                        );
                    }
                    s += 2.0 * probe.hx + 5.0 + 16.0 * unit(h, 40);
                } else {
                    s += 10.0;
                }
            }
        }
    }

    /// Habs in the gaps between the streets' plots, each turned to its nearest
    /// way: the town grown in, not laid out.
    fn infill(&mut self) {
        let ((tx, ty), rx, ry) = super::shape::TOWN;
        let step = 34.0;
        let (nx, ny) = ((2.0 * rx / step) as i32, (2.0 * ry / step) as i32);
        for j in 0..ny {
            for i in 0..nx {
                let p0 = (tx - rx + i as f64 * step, ty - ry + j as f64 * step);
                let h = self.hash(0x696e_6669, p0);
                let p = (
                    p0.0 + 20.0 * (unit(h, 0) - 0.5),
                    p0.1 + 20.0 * (unit(h, 8) - 0.5),
                );
                if ((p.0 - tx) / (0.85 * rx)).hypot((p.1 - ty) / (0.85 * ry)) > 1.0
                    || unit(h, 16) < 0.35
                {
                    continue;
                }
                // Face the nearest way.
                let Some(s) = self
                    .roads_index
                    .near(p, 120.0)
                    .map(|k| self.roads[k as usize])
                    .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
                else {
                    continue;
                };
                let (dx, dy) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
                let len = dx.hypot(dy).max(1e-6);
                let side = s.side(p).signum();
                let to_road = (dy / len * side, -dx / len * side);
                let heading = (-to_road.0).atan2(to_road.1) + 0.2 * (unit(h, 24) - 0.5);
                let kind = if unit(h, 32) < 0.15 {
                    PropKind::ColonyHabStack
                } else {
                    PropKind::ColonyHab
                };
                self.try_place(kind, p, heading);
            }
        }
    }

    /// Lights along the town's streets, every few dozen metres, their arms
    /// over the road.
    fn street_lights(&mut self) {
        let mut lights = Vec::new();
        for (k, s) in self.roads.iter().enumerate() {
            if s.road != Road::Street || k % 2 == 1 {
                continue;
            }
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            let side = if k % 4 == 0 { 1.0 } else { -1.0 };
            let normal = (-dir.1 * side, dir.0 * side);
            let off = s.half + 0.7;
            let p = (s.a.0 + normal.0 * off, s.a.1 + normal.1 * off);
            lights.push((p, (-normal.1).atan2(-normal.0)));
        }
        for (p, heading) in lights {
            self.dress(PropKind::CityStreetLight, p, heading, 1.0);
        }
    }

    /// The rebels' barricades across each street a little way out of the
    /// square, a lane left open.
    fn town_barricades(&mut self, lines: &[(Vec<P>, (f64, f64))]) {
        for (line, _) in lines {
            let Some(w) = line.windows(2).find(|w| {
                let (a, b) = (dist(w[0], SQUARE_C), dist(w[1], SQUARE_C));
                (a - 120.0) * (b - 120.0) <= 0.0
            }) else {
                continue;
            };
            let mid = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
            let along = (w[1].0 - w[0].0, w[1].1 - w[0].1);
            let across = (-along.1, along.0);
            let heading = across.1.atan2(across.0);
            let len = across.0.hypot(across.1).max(1e-6);
            for t in [-3.0, 9.0] {
                let p = (mid.0 + across.0 / len * t, mid.1 + across.1 / len * t);
                self.dress(PropKind::CityBarricade, p, heading, 0.6);
            }
        }
    }

    // -- the spaceport ------------------------------------------------------------

    fn spaceport(&mut self) {
        // The apron, the Bastion's berth its east half; the terminal north of
        // it facing the town, hangars along its west side, stores south.
        let (ax, ay, hx, hy) = (7_060.0, 4_620.0, 430.0, 300.0);
        self.area(square((ax, ay), hx, hy), Ground::Apron);
        let put = |plan: &mut Planner, kind, c: P, heading: f64| {
            plan.place(kind, origin_for(kind, c, heading, 1.0), heading);
        };
        put(
            self,
            PropKind::ColonyPortControl,
            (ax - 230.0, ay + hy + 70.0),
            0.0,
        );
        for y in [ay + 120.0, ay - 130.0] {
            put(
                self,
                PropKind::ColonyHangar,
                (ax - hx - 60.0, y),
                -FRAC_PI_2,
            );
        }
        for x in [ax - 160.0, ax + 200.0] {
            put(self, PropKind::ColonyDepot, (x, ay - hy - 55.0), PI);
        }
        // Shuttle pads in the apron's west half.
        for c in [
            (ax - 330.0, ay + 180.0),
            (ax - 330.0, ay - 10.0),
            (ax - 330.0, ay - 200.0),
        ] {
            self.dress(PropKind::ColonyPad, c, 0.0, 1.0);
        }
        // Gun pits round the edge, facing out.
        let e = 20.0;
        for (c, heading) in [
            ((ax + 60.0, ay + hy + e), 0.0),
            ((ax + 330.0, ay + hy + e), 0.0),
            ((ax + hx + e, ay + 160.0), -FRAC_PI_2),
            ((ax + hx + e, ay - 160.0), -FRAC_PI_2),
            ((ax + 400.0, ay - hy - e), PI),
            ((ax - hx - 140.0, ay + hy + 40.0), FRAC_PI_2),
            ((ax - hx - 140.0, ay - hy - 30.0), FRAC_PI_2),
        ] {
            put(self, PropKind::ColonyGunPit, c, heading);
        }
        self.dress(
            PropKind::CityMast,
            (ax + hx + 70.0, ay + hy + 60.0),
            0.0,
            0.8,
        );
        self.dress(
            PropKind::CityMast,
            (ax - hx - 120.0, ay - hy - 120.0),
            0.0,
            0.6,
        );
    }

    fn quarry(&mut self) {
        let (c, _) = QUARRY;
        self.place(
            PropKind::ColonyDig,
            origin_for(PropKind::ColonyDig, (c.0, c.1 - 20.0), 0.0, 1.0),
            0.0,
        );
        self.area(square(c, 110.0, 110.0), Ground::Earth);
    }

    // -- the road ------------------------------------------------------------------

    /// The miners' camp: a fenced yard off the road, its gate to the road, the
    /// miners' huts inside.
    fn camp(&mut self) {
        let ((cx, cy), hx, hy) = CAMP;
        self.area(square((cx, cy), hx, hy), Ground::Earth);
        let run = crate::city::FENCE_SEGMENT_M as f64;
        let n_x = (2.0 * hx / run).round() as usize;
        let n_y = (2.0 * hy / run).round() as usize;
        for k in 0..n_x {
            let x = cx - hx + (k as f64 + 0.5) * run;
            // The gate: the middle two runs of the north side left open.
            if k + 1 != n_x / 2 && k != n_x / 2 {
                self.place(PropKind::ColonyFence, (x, cy + hy), 0.0);
            }
            self.place(PropKind::ColonyFence, (x, cy - hy), 0.0);
        }
        for k in 0..n_y {
            let y = cy - hy + (k as f64 + 0.5) * run;
            self.place(PropKind::ColonyFence, (cx - hx, y), FRAC_PI_2);
            self.place(PropKind::ColonyFence, (cx + hx, y), FRAC_PI_2);
        }
        for x in [-36.0, 0.0, 36.0] {
            self.place(
                PropKind::ColonyHab,
                origin_for(PropKind::ColonyHab, (cx + x, cy - hy + 18.0), PI, 1.0),
                PI,
            );
        }
        self.dress(
            PropKind::CityBarricade,
            (cx - 18.0, cy + hy + 6.0),
            0.0,
            0.5,
        );
    }

    /// The guns dug into the rise over the bridge, facing it; sandbags along
    /// the crest.
    fn guns(&mut self) {
        for (dx, dy) in [(-30.0, 30.0), (30.0, -30.0)] {
            let c = (RISE.0 + dx, RISE.1 + dy);
            self.place(
                PropKind::ColonyGunPit,
                origin_for(PropKind::ColonyGunPit, c, FRAC_PI_2, 1.0),
                FRAC_PI_2,
            );
        }
        for dy in [-80.0, 70.0] {
            self.dress(
                PropKind::CityBarricade,
                (RISE.0 - 40.0, RISE.1 + dy),
                FRAC_PI_2,
                0.8,
            );
        }
        // The rebels' checkpoints on the road: sandbags either side.
        for p in CHECKPOINTS {
            for side in [-1.0, 1.0] {
                self.dress(PropKind::CityBarricade, (p.0, p.1 + side * 11.0), 0.0, 0.6);
            }
        }
        // The PDF's post at the junction.
        self.try_place(
            PropKind::ColonyHab,
            (JUNCTION.0 - 50.0, JUNCTION.1 - 40.0),
            0.0,
        );
    }

    // -- the country ---------------------------------------------------------------

    /// The farmsteads: a house, a barn and its solar panels, a lane to the road.
    fn farms(&mut self, terrain: &Terrain) {
        for (at, side) in FARMSTEADS {
            let yard = (at.0 + side * 90.0, at.1);
            self.road_line(
                &[(at.0 + side * 6.0, at.1), (yard.0 - side * 30.0, yard.1)],
                Road::Lane,
                LANE,
            );
            self.area(square(yard, 45.0, 40.0), Ground::Yard);
            let heading = if side > 0.0 { -FRAC_PI_2 } else { FRAC_PI_2 };
            self.try_place(PropKind::ColonyHab, (yard.0, yard.1 + 22.0), heading);
            self.try_place(
                PropKind::ColonyDepot,
                (yard.0 + side * 10.0, yard.1 - 30.0),
                0.0,
            );
            if terrain.vl_inland(yard) > 0.0 {
                self.dress(
                    PropKind::CitySolarArray,
                    (yard.0 + side * 60.0, yard.1 + 30.0),
                    0.0,
                    1.0,
                );
            }
        }
    }

    /// A fishing hamlet on the big island, a mast on the far one.
    fn islands(&mut self) {
        let (c, r, _) = ISLANDS[1];
        self.dress(PropKind::ColonyPad, (c.0 + 60.0, c.1 - 40.0), 0.0, 1.0);
        for k in 0..6 {
            let a = k as f64 / 6.0 * std::f64::consts::TAU + 0.4;
            let at = (c.0 + 0.55 * r * a.cos(), c.1 + 0.55 * r * a.sin());
            self.try_place(PropKind::ColonyHab, at, a - FRAC_PI_2);
        }
        let (c, _, _) = ISLANDS[3];
        self.dress(PropKind::CityMast, c, 0.0, 0.6);
        let (c, _, _) = ISLANDS[0];
        self.dress(PropKind::CityWindTurbine, c, 0.0, 1.0);
    }

    /// Turbines on the hills.
    fn country(&mut self) {
        for p in TURBINES {
            self.dress(PropKind::CityWindTurbine, p, 0.0, 1.0);
        }
    }
}
