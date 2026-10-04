//! Halcyon's plan: the wall, the roads, the blocks and what stands on them,
//! the craters. Laid once at set-up; the bake reads it for every sample.
//!
//! The city is a 128 m grid from the middle line, avenues on some lines,
//! streets on the rest, two boulevards from the side gates to Concord Square;
//! each block between them is filled from a few templates by district
//! (downtown towers, old town tenements and courtyards, midtown offices,
//! residential flats, works along the wall). The outskirts have the three
//! highways to the gates, a country road across, lanes among the fields,
//! suburbs behind the glacis and an industrial estate in the east.

use super::shape::LOT_FLAT;
use super::*;
use crate::city::{self, GATE_PASSAGE_M, WALL_SEGMENT_M, WALL_THICK_M};
use crate::noise::hash2;
use std::f64::consts::{FRAC_PI_2, PI};

pub(super) mod furniture;

/// The city grid's pitch, metres; lines run from the middle line both ways.
const PITCH: f64 = 128.0;
/// The first east-west grid line north of the wall.
const GRID_Y0: f64 = 6_528.0;
/// Grid lines (counted from the middle line, and from `GRID_Y0`) that are avenues.
const AVENUE_COLUMNS: [i64; 7] = [-42, -29, -12, 0, 12, 29, 42];
const AVENUE_ROWS: [i64; 4] = [5, 15, 24, 33];
/// The grid row the railway runs along instead of a street.
const RAIL_ROW: i64 = 28;
/// The central station: the middle of the merged pair of cells it fills,
/// north of the railway east of the middle avenue.
const STATION_CELL: (i64, i64) = (2, RAIL_ROW);
const STATION: P = (
    MID + (STATION_CELL.0 + 1) as f64 * PITCH,
    GRID_Y0 + (RAIL_ROW as f64 + 0.5) * PITCH,
);
/// The ring road behind the wall: how far north of the wall's line.
const RING_OFFSET: f64 = 120.0;
/// Concord Square's half size.
const SQUARE_HALF: f64 = 150.0;

/// Road sizes: half width to the kerb, pavement width.
const AVENUE: (f64, f64) = (22.0, 6.0);
const STREET: (f64, f64) = (9.0, 5.0);
const RING: (f64, f64) = (14.0, 4.0);
const BOULEVARD: (f64, f64) = (20.0, 6.0);
const HIGHWAY: f64 = 13.0;
const COUNTRY: f64 = 7.5;
const LANE: f64 = 4.0;
const SUBURB: (f64, f64) = (7.0, 3.0);
const WORKS: (f64, f64) = (9.0, 3.0);
const RAIL_HALF: f64 = 7.0;

/// Gap kept between two buildings, metres.
const GAP: f64 = 3.0;
/// The suburbs' belt behind the glacis, metres south of the wall: from the
/// glacis to as deep as they ever reach ([`suburb_depth`]).
const SUBURBS: (f64, f64) = (420.0, 1_650.0);
/// The villages on the country road.
const VILLAGES: [P; 3] = [
    (1_300.0, 3_420.0),
    (MID - 720.0, 3_400.0),
    (10_900.0, 3_270.0),
];
/// The industrial estate in the east of the outskirts.
const ESTATE: (P, P) = ((8_350.0, 2_500.0), (11_900.0, 4_450.0));

/// What a block of the city holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum District {
    /// Works, depots and barracks behind the wall.
    Belt,
    Downtown,
    OldTown,
    Midtown,
    Residential,
    /// Warehouses round the rail yards and the spaceport.
    Logistics,
}

/// The planner: the plan as it is laid, and what it keeps clear.
struct Planner {
    seed: u64,
    roads: Vec<Seg>,
    roads_index: Buckets,
    lots: Vec<Lot>,
    lots_index: Buckets,
    areas: Vec<Area>,
    craters: Vec<Crater>,
    planted: Vec<(P, PropKind, u16)>,
    /// Ore fields: nothing built within reach of them.
    ore: Vec<(P, f64)>,
    /// The plazas' middles, for their monuments.
    plazas: Vec<P>,
    dressing: Vec<furniture::Dressing>,
}

impl Terrain {
    pub(super) fn lay_siege_plan(&mut self, ore: &[(P, f64)]) {
        let mut plan = Planner {
            seed: self.seed,
            roads: Vec::new(),
            roads_index: Buckets::new(SIZE),
            lots: Vec::new(),
            lots_index: Buckets::new(SIZE),
            areas: Vec::new(),
            craters: Vec::new(),
            planted: Vec::new(),
            ore: ore.to_vec(),
            plazas: Vec::new(),
            dressing: Vec::new(),
        };
        plan.bases();
        plan.city_roads();
        plan.outskirts_roads();
        plan.wall();
        plan.square();
        plan.city_blocks();
        plan.suburbs();
        plan.estate();
        plan.farms();
        plan.villages();
        plan.street_trees();
        plan.furniture();
        plan.craters();

        // Level each lot to the ground under it: the mean of its corners and
        // middle, so a lot on a slope is cut at one end and built up at the other.
        let mut lots = plan.lots;
        for lot in &mut lots {
            let mut sum = self.siege_land(lot.rect.c.0, lot.rect.c.1);
            for c in lot.rect.corners() {
                sum += self.siege_land(c.0, c.1);
            }
            lot.level = sum / 5.0;
        }
        let mut crater_index = Buckets::new(SIZE);
        for (i, c) in plan.craters.iter().enumerate() {
            let r = c.radius * 1.5;
            crater_index.insert(i as u32, (c.at.0 - r, c.at.1 - r), (c.at.0 + r, c.at.1 + r));
        }
        let mut area_index = Buckets::new(SIZE);
        for (i, a) in plan.areas.iter().enumerate() {
            let (lo, hi) = a.rect.bounds();
            area_index.insert(i as u32, lo, hi);
        }
        let mut lot_index = Buckets::new(SIZE);
        for (i, l) in lots.iter().enumerate() {
            let (lo, hi) = l.rect.bounds();
            let r = super::shape::LOT_EASE;
            lot_index.insert(i as u32, (lo.0 - r, lo.1 - r), (hi.0 + r, hi.1 + r));
        }
        self.siege = Siege {
            roads: plan.roads,
            road_index: Some(plan.roads_index),
            lots,
            lot_index: Some(lot_index),
            areas: plan.areas,
            area_index: Some(area_index),
            craters: plan.craters,
            crater_index: Some(crater_index),
            planted: plan.planted,
            dressing: plan.dressing,
        };
    }
}

/// The footprint a kind covers at `at`, turned `heading`, at `scale`: the
/// box round all its solid parts.
fn footprint(kind: PropKind, at: P, heading: f64, scale: f64) -> Rect {
    let plan = city::structure(kind).map_or(&[][..], |s| s.plan);
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for &(cx, cy, hx, hy) in plan {
        lo = (lo.0.min((cx - hx) as f64), lo.1.min((cy - hy) as f64));
        hi = (hi.0.max((cx + hx) as f64), hi.1.max((cy + hy) as f64));
    }
    if plan.is_empty() {
        // Rubble: a heap about this big.
        (lo, hi) = ((-9.0, -7.0), (9.0, 7.0));
    }
    let (mx, my) = ((lo.0 + hi.0) / 2.0 * scale, (lo.1 + hi.1) / 2.0 * scale);
    let (s, c) = heading.sin_cos();
    Rect {
        c: (at.0 + mx * c - my * s, at.1 + mx * s + my * c),
        hx: (hi.0 - lo.0) / 2.0 * scale,
        hy: (hi.1 - lo.1) / 2.0 * scale,
        heading,
    }
}

/// The unscaled half extents of a kind's footprint.
fn size_of(kind: PropKind) -> (f64, f64) {
    let r = footprint(kind, (0.0, 0.0), 0.0, 1.0);
    (r.hx, r.hy)
}

/// Where a prop's origin goes so its footprint's middle lands on `c`.
fn origin_for(kind: PropKind, c: P, heading: f64, scale: f64) -> P {
    let r = footprint(kind, (0.0, 0.0), heading, scale);
    (c.0 - r.c.0, c.1 - r.c.1)
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

    /// A polyline of road, in pieces of at most `piece` metres.
    fn road_line(&mut self, line: &[P], road: Road, (half, walk): (f64, f64), piece: f64) {
        for w in line.windows(2) {
            let n = (dist(w[0], w[1]) / piece).ceil().max(1.0) as usize;
            for k in 0..n {
                let t0 = k as f64 / n as f64;
                let t1 = (k + 1) as f64 / n as f64;
                let at = |t: f64| {
                    (
                        w[0].0 + (w[1].0 - w[0].0) * t,
                        w[0].1 + (w[1].1 - w[0].1) * t,
                    )
                };
                self.road(seg(at(t0), at(t1), road, half, walk));
            }
        }
    }

    fn area(&mut self, rect: Rect, ground: Ground) {
        self.areas.push(Area { rect, ground });
    }

    /// Lays a structure without asking (the wall, set pieces).
    fn place(&mut self, kind: PropKind, at: P, heading: f64, scale: f64, wear: f64) {
        let rect = footprint(kind, at, heading, scale);
        let (lo, hi) = rect.bounds();
        self.lots_index.insert(self.lots.len() as u32, lo, hi);
        self.lots.push(Lot {
            kind,
            at,
            heading,
            scale,
            wear,
            rect,
            level: 0.0,
        });
    }

    /// Lays a structure whose footprint's middle is `c`, if it keeps off the
    /// roads and their pavements, the other lots, the ore, the bases, the
    /// wall's glacis and the map's edge.
    fn try_place(&mut self, kind: PropKind, c: P, heading: f64, scale: f64, wear: f64) -> bool {
        let at = origin_for(kind, c, heading, scale);
        let rect = footprint(kind, at, heading, scale);
        if !self.fits(&rect, kind == PropKind::CityRubble) {
            return false;
        }
        self.place(kind, at, heading, scale, wear);
        true
    }

    fn fits(&self, rect: &Rect, rubble: bool) -> bool {
        let (lo, hi) = rect.bounds();
        if lo.0 < 24.0 || lo.1 < 24.0 || hi.0 > SIZE - 24.0 || hi.1 > SIZE - 24.0 {
            return false;
        }
        let radius = rect.hx.hypot(rect.hy);
        let wall = y_at(WALL, rect.c.0);
        let glacis = if Terrain::in_city(rect.c) {
            60.0
        } else {
            380.0
        };
        if (rect.c.1 - wall).abs() < glacis + radius && !rubble {
            return false;
        }
        if nearest_base(rect.c).0 < BASE_OPEN + radius {
            return false;
        }
        if self
            .ore
            .iter()
            .any(|&(p, r)| rect.outside(p) < 1.5 * r + 16.0)
        {
            return false;
        }
        if self
            .roads_index
            .near(rect.c, radius + 40.0)
            .map(|i| &self.roads[i as usize])
            .any(|s| rect.distance_to_segment(s.a, s.b) < s.reach() + 1.5)
        {
            return false;
        }
        !self
            .lots_index
            .near(rect.c, radius + 40.0)
            .any(|i| self.lots[i as usize].rect.near(rect, GAP))
    }

    /// The bases' ground: rail yards, the Governor's Park, the spaceport's
    /// apron; outside, the works yard and the quarry floor.
    fn bases(&mut self) {
        let square = |c: P, half: f64| Rect {
            c,
            hx: half,
            hy: half,
            heading: 0.0,
        };
        let r = BASE_OPEN - 40.0;
        self.area(square(CITY_STARTS[0], r), Ground::Ballast);
        self.area(square(CITY_STARTS[1], r), Ground::Lawn);
        self.area(square(CITY_STARTS[2], r), Ground::Apron);
        self.area(square(OUT_STARTS[1], 0.8 * r), Ground::Earth);
        self.area(square(OUT_STARTS[2], r), Ground::Yard);
    }

    // -- roads --------------------------------------------------------------

    fn city_roads(&mut self) {
        let ring_y = |x: f64| y_at(WALL, x) + RING_OFFSET;
        // The ring road behind the wall, in 128 m pieces.
        let ring: Vec<P> = (0..=(SIZE / PITCH) as i64)
            .map(|i| {
                let x = (i as f64 * PITCH).clamp(8.0, SIZE - 8.0);
                (x, ring_y(x))
            })
            .collect();
        self.road_line(&ring, Road::Street, RING, PITCH);

        // Where the grid is not: the bases, the parks, the square.
        let open = |p: P, avenue: bool| {
            nearest_base(p).0 < BASE_OPEN - 60.0
                || (!avenue && in_park(p, -30.0))
                || ((p.0 - SQUARE.0).abs() < SQUARE_HALF && (p.1 - SQUARE.1).abs() < SQUARE_HALF)
        };
        let top = SIZE - 16.0;
        // North-south lines, from the ring road to the north edge.
        for k in -47..=47_i64 {
            let x = MID + k as f64 * PITCH;
            if !(16.0..=SIZE - 16.0).contains(&x) {
                continue;
            }
            let avenue = AVENUE_COLUMNS.contains(&k);
            let size = if avenue { AVENUE } else { STREET };
            let kind = if avenue { Road::Avenue } else { Road::Street };
            let mut y = ring_y(x);
            let mut next = GRID_Y0;
            let mut row = -1;
            while y < top {
                let to = next.min(top);
                let mid = (x, (y + to) / 2.0);
                let merged = merged_east(self.seed, k - 1, row);
                if !open(mid, avenue) && !merged && to - y > 1.0 {
                    self.road(seg((x, y), (x, to), kind, size.0, size.1));
                }
                y = to;
                next += PITCH;
                row += 1;
            }
        }
        // East-west lines, across the whole city.
        for j in 0..=((top - GRID_Y0) / PITCH) as i64 {
            let y = GRID_Y0 + j as f64 * PITCH;
            if j == RAIL_ROW {
                continue;
            }
            let avenue = AVENUE_ROWS.contains(&j);
            let size = if avenue { AVENUE } else { STREET };
            let kind = if avenue { Road::Avenue } else { Road::Street };
            for k in -48..48_i64 {
                let (a, b) = (MID + k as f64 * PITCH, MID + (k + 1) as f64 * PITCH);
                let (a, b) = (a.max(16.0), b.min(SIZE - 16.0));
                if b <= a || open(((a + b) / 2.0, y), avenue) || merged_north(self.seed, k, j - 1) {
                    continue;
                }
                self.road(seg((a, y), (b, y), kind, size.0, size.1));
            }
        }
        // The boulevards from the side gates to the square.
        for &gx in &[GATES_X[0], GATES_X[2]] {
            let from = (gx, ring_y(gx));
            let to = (
                SQUARE.0 + (gx - SQUARE.0).signum() * SQUARE_HALF,
                SQUARE.1 - SQUARE_HALF,
            );
            self.road_line(&[from, to], Road::Avenue, BOULEVARD, 64.0);
        }
        // The railway, along its grid row.
        let rail_y = GRID_Y0 + RAIL_ROW as f64 * PITCH;
        self.road_line(
            &[(8.0, rail_y), (SIZE - 8.0, rail_y)],
            Road::Rail,
            (RAIL_HALF, 0.0),
            PITCH,
        );
    }

    fn outskirts_roads(&mut self) {
        let below = |x: f64| y_at(WALL, x) + RING_OFFSET;
        let w = [
            (GATES_X[0], below(GATES_X[0])),
            (GATES_X[0], 5_500.0),
            (2_700.0, 4_400.0),
            (2_480.0, 3_300.0),
            (2_330.0, 2_520.0),
            (OUT_STARTS[0].0, OUT_STARTS[0].1 + 250.0),
        ];
        let c = [
            (GATES_X[1], below(GATES_X[1])),
            (GATES_X[1], 5_300.0),
            (6_020.0, 4_100.0),
            (6_200.0, 2_700.0),
            (OUT_STARTS[1].0, OUT_STARTS[1].1 + 300.0),
        ];
        let e = [
            (GATES_X[2], below(GATES_X[2])),
            (GATES_X[2], 5_500.0),
            (9_620.0, 4_350.0),
            (9_830.0, 3_250.0),
            (9_960.0, 2_520.0),
            (OUT_STARTS[2].0, OUT_STARTS[2].1 + 250.0),
        ];
        for line in [&w[..], &c[..], &e[..]] {
            // Straight through the gate, then easing into curves.
            self.road_line(&line[..2], Road::Highway, (HIGHWAY, 0.0), 64.0);
            let smooth = super::super::canyon::smooth_open(&line[1..], 10);
            self.road_line(&smooth, Road::Highway, (HIGHWAY, 0.0), 64.0);
        }
        // The country road across the outskirts.
        let across = [
            (-60.0, 3_520.0),
            (1_500.0, 3_380.0),
            (3_000.0, 3_250.0),
            (4_600.0, 3_420.0),
            (MID, 3_340.0),
            (7_700.0, 3_160.0),
            (9_200.0, 3_380.0),
            (10_800.0, 3_260.0),
            (SIZE + 60.0, 3_400.0),
        ];
        let across = super::super::canyon::smooth_open(&across, 12);
        self.road_line(&across, Road::Highway, (COUNTRY, 0.0), 64.0);
        // Lanes among the farms: north-south every kilometre or so, wandering,
        // and two east-west ones; they stop short of the suburbs and the estate.
        for (k, x) in [
            700.0, 1_550.0, 3_450.0, 4_350.0, 5_250.0, 7_050.0, 7_950.0, 8_850.0, 11_000.0,
        ]
        .into_iter()
        .enumerate()
        {
            let h = hash2(self.seed ^ 0x6C61_6E65, k as i64, 0);
            let top = y_at(WALL, x) - suburb_depth(x) - 40.0;
            let mut line = Vec::new();
            let mut y = 120.0;
            while y < top {
                let wobble = 90.0 * ((y / 700.0) + unit(h, 0) * 6.0).sin()
                    + 40.0 * ((y / 230.0) + unit(h, 24) * 6.0).sin();
                line.push((x + wobble, y));
                y += 150.0;
            }
            let line: Vec<P> = line.into_iter().filter(|&p| !in_estate(p, 60.0)).collect();
            if line.len() > 1 {
                self.road_line(&line, Road::Lane, (LANE, 0.0), 64.0);
            }
        }
        for (k, y) in [1_250.0, 4_650.0].into_iter().enumerate() {
            let h = hash2(self.seed ^ 0x6C61_6E66, k as i64, 0);
            let line: Vec<P> = (0..=82)
                .map(|i| {
                    let x = i as f64 * 150.0;
                    (x, y + 70.0 * ((x / 900.0) + unit(h, 0) * 6.0).sin())
                })
                .filter(|&p| !in_estate(p, 60.0) && nearest_base(p).0 > BASE_OPEN)
                .collect();
            // Only runs of the line that stayed whole.
            for run in line.chunk_by(|a, b| (b.0 - a.0) < 151.0) {
                if run.len() > 1 {
                    self.road_line(run, Road::Lane, (LANE, 0.0), 64.0);
                }
            }
        }
    }

    // -- the wall -------------------------------------------------------------

    /// The wall: towers at its corners and every few hundred metres, curtain
    /// segments between, a gate on each gate road, rubble in the breaches.
    fn wall(&mut self) {
        let tower = size_of(PropKind::CityWallTower).0;
        let gate_half = size_of(PropKind::CityGate).1;
        for (run, w) in WALL.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let len = dist(a, b);
            let heading = (b.1 - a.1).atan2(b.0 - a.0);
            let along = |s: f64| (a.0 + (b.0 - a.0) * s / len, a.1 + (b.1 - a.1) * s / len);
            // Where x falls along this run.
            let s_of = |x: f64| (x - a.0) / (b.0 - a.0) * len;
            // What is not curtain: (from, to, piece).
            let mut taken: Vec<(f64, f64, Option<PropKind>)> = Vec::new();
            if run > 0 {
                taken.push((-tower, tower, Some(PropKind::CityWallTower)));
            }
            for &gx in &GATES_X {
                let s = s_of(gx);
                if s > 0.0 && s < len {
                    taken.push((s - gate_half, s + gate_half, Some(PropKind::CityGate)));
                }
            }
            for &bx in &BREACHES_X {
                let s = s_of(bx);
                if s > 0.0 && s < len {
                    taken.push((s - BREACH_HALF, s + BREACH_HALF, None));
                }
            }
            // The run's own end at the map's edge.
            let (start, end) = (s_of(0.0).max(0.0), s_of(SIZE).min(len));
            if run + 2 < WALL.len() {
                taken.push((len - tower, len + tower, None));
            }
            taken.sort_by(|p, q| p.0.total_cmp(&q.0));
            // The corner towers, gates and breaches.
            for &(from, to, piece) in &taken {
                let mid = along((from + to) / 2.0);
                match piece {
                    Some(PropKind::CityGate) => {
                        let wear = 0.25 + 0.2 * unit(self.hash(0x6761_7465, mid), 0);
                        self.place(PropKind::CityGate, mid, heading + FRAC_PI_2, 1.0, wear);
                    }
                    Some(kind) => {
                        let wear = 0.1 + 0.4 * unit(self.hash(0x746F_7772, mid), 0);
                        self.place(kind, mid, heading, 1.0, wear);
                    }
                    None if to <= len => self.breach(along(from), along(to), heading),
                    None => {}
                }
            }
            // The curtain between, a tower every five segments or so.
            let mut s = start;
            for &(from, to, _) in taken.iter().chain([&(end, end, None)]) {
                let gap_end = from.min(end);
                if gap_end > s + 1.0 {
                    self.curtain(&along, s, gap_end, heading);
                }
                s = s.max(to);
            }
        }
    }

    /// Curtain from `s0` to `s1` along a run: segments scaled to fit, with
    /// towers standing in it no more than about 330 m apart.
    fn curtain(&mut self, along: &dyn Fn(f64) -> P, s0: f64, s1: f64, heading: f64) {
        let tower = size_of(PropKind::CityWallTower).0;
        let seg_len = WALL_SEGMENT_M as f64;
        let stretch = s1 - s0;
        let bays = (stretch / 330.0).round().max(1.0) as usize;
        let bay = stretch / bays as f64;
        for k in 0..bays {
            let (mut from, mut to) = (s0 + k as f64 * bay, s0 + (k + 1) as f64 * bay);
            if k > 0 {
                let at = along(from);
                let wear = 0.1 + 0.45 * unit(self.hash(0x746F_7772, at), 0);
                self.place(PropKind::CityWallTower, at, heading, 1.0, wear);
                from += tower;
            }
            if k + 1 < bays {
                to -= tower;
            }
            let n = ((to - from) / seg_len).round().max(1.0);
            let piece = (to - from) / n;
            for i in 0..n as usize {
                let at = along(from + (i as f64 + 0.5) * piece);
                let h = self.hash(0x7761_6C6C, at);
                let wear = 0.08 + 0.5 * unit(h, 0).powi(2);
                self.place(PropKind::CityWall, at, heading, piece / seg_len, wear);
            }
        }
    }

    /// A breach: broken curtain at either end, rubble fanned out across the
    /// gap and down both faces, craters.
    fn breach(&mut self, from: P, to: P, heading: f64) {
        let mid = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        let (s, c) = heading.sin_cos();
        let thick = WALL_THICK_M as f64;
        self.area(
            Rect {
                c: mid,
                hx: BREACH_HALF + 20.0,
                hy: 110.0,
                heading,
            },
            Ground::Rubble,
        );
        for i in 0..26 {
            let h = hash2(self.seed ^ 0x6272_6563, mid.0 as i64, i);
            let u = (unit(h, 0) - 0.5) * 2.0 * (BREACH_HALF - 6.0);
            let v = (unit(h, 24) - 0.5) * 2.0 * (thick + 50.0 * unit(h, 48));
            let at = (mid.0 + u * c - v * s, mid.1 + u * s + v * c);
            let turn = unit(h, 12) * PI;
            self.place(PropKind::CityRubble, at, turn, 0.8 + 0.6 * unit(h, 36), 0.0);
        }
        for i in 0..14 {
            let h = hash2(self.seed ^ 0x6372_6174, mid.0 as i64, i);
            let u = (unit(h, 0) - 0.5) * 2.0 * (BREACH_HALF + 40.0);
            let v = (unit(h, 24) - 0.5) * 220.0;
            self.craters.push(Crater {
                at: (mid.0 + u * c - v * s, mid.1 + u * s + v * c),
                radius: 6.0 + 8.0 * unit(h, 48),
                depth: 1.5 + 2.0 * unit(h, 36),
            });
        }
    }

    // -- the city -------------------------------------------------------------

    /// Concord Square: paving, the civic hall on its north side facing it,
    /// the church on its west.
    fn square(&mut self) {
        let half = SQUARE_HALF;
        self.area(
            Rect {
                c: SQUARE,
                hx: half,
                hy: half,
                heading: 0.0,
            },
            Ground::Paving,
        );
        // The hall's portico (+y) faces the square, to the south.
        let hall = (SQUARE.0, SQUARE.1 + 70.0);
        self.place(PropKind::CityCivic, hall, PI, 1.0, 0.05);
        let church = (SQUARE.0 + half - 34.0, SQUARE.1 + 30.0);
        self.place(PropKind::CityChurch, church, PI, 1.0, 0.1);
        for i in 0..16 {
            let t = i as f64 / 16.0 * std::f64::consts::TAU;
            let p = (
                SQUARE.0 + 0.82 * half * t.cos(),
                SQUARE.1 - 30.0 + 0.6 * half * t.sin(),
            );
            self.planted.push((p, PropKind::TreeBroadleaf, 800));
        }
    }

    fn district(&self, c: P) -> District {
        let wall = y_at(WALL, c.0);
        // The rail yards' and the spaceport's warehouses (not the park's).
        let base = [CITY_STARTS[0], CITY_STARTS[2]]
            .iter()
            .map(|&s| dist(c, s))
            .fold(f64::MAX, f64::min);
        let jitter = unit(self.hash(0x6469_7374, c), 0);
        if c.1 - wall < 420.0 + 160.0 * jitter {
            District::Belt
        } else if base < 1_150.0 + 250.0 * jitter && c.1 < 11_900.0 {
            District::Logistics
        } else if downtown(c) > 0.05 {
            District::Downtown
        } else if old_town(c) > 0.15 {
            District::OldTown
        } else if c.1 < 8_900.0 + 500.0 * jitter {
            District::Midtown
        } else {
            District::Residential
        }
    }

    /// How worn a building at `c` starts: badly near the front, now and then
    /// a struck block deeper in, little in the north.
    fn wear(&self, c: P, salt: u64) -> f64 {
        let h = self.hash(0x7765_6172 ^ salt, c);
        let (u, v, w) = (unit(h, 0), unit(h, 24), unit(h, 48));
        let wall = y_at(WALL, c.0);
        let front = if Terrain::in_city(c) {
            1.0 - smoothstep(200.0, 1_500.0, c.1 - wall)
        } else {
            1.0 - smoothstep(SUBURBS.0, SUBURBS.1 + 300.0, wall - c.1)
        };
        let struck = if v < 0.07 { 0.35 + 0.45 * w } else { 0.0 };
        (front * (0.2 + 0.65 * u) + 0.06 * u).max(struck).min(0.92)
    }

    /// Every block of the grid, filled by its district.
    fn city_blocks(&mut self) {
        let top = SIZE - 16.0;
        let ring_y = |x: f64| y_at(WALL, x) + RING_OFFSET;
        let column_half = |k: i64| {
            if AVENUE_COLUMNS.contains(&k) {
                AVENUE.0 + AVENUE.1
            } else {
                STREET.0 + STREET.1
            }
        };
        let row_half = |j: i64| {
            if j == RAIL_ROW {
                RAIL_HALF + 6.0
            } else if AVENUE_ROWS.contains(&j) {
                AVENUE.0 + AVENUE.1
            } else {
                STREET.0 + STREET.1
            }
        };
        for k in -48..48_i64 {
            let (x0, x1) = (MID + k as f64 * PITCH, MID + (k + 1) as f64 * PITCH);
            if x0 < 0.0 || x1 > SIZE {
                continue;
            }
            let (bx0, bx1) = (x0 + column_half(k), x1 - column_half(k + 1));
            // The first row runs down to the ring road.
            let ring = ring_y(x0).max(ring_y(x1)) + RING.0 + RING.1;
            for j in -1..=((top - GRID_Y0) / PITCH) as i64 {
                // A merged pair is built once, from its south-west cell.
                if merged_east(self.seed, k - 1, j) || merged_north(self.seed, k, j - 1) {
                    continue;
                }
                let (east, north) = (merged_east(self.seed, k, j), merged_north(self.seed, k, j));
                let bx1 = if east {
                    x1 + PITCH - column_half(k + 2)
                } else {
                    bx1
                };
                let rows = if north { 2.0 } else { 1.0 };
                let (y0, y1) = if j < 0 {
                    (ring, GRID_Y0 - row_half(0))
                } else {
                    (
                        GRID_Y0 + j as f64 * PITCH + row_half(j),
                        (GRID_Y0 + (j as f64 + rows) * PITCH).min(top + 16.0)
                            - row_half(j + rows as i64),
                    )
                };
                if y1 - y0 < 30.0 || bx1 - bx0 < 30.0 {
                    continue;
                }
                let block = Rect {
                    c: ((bx0 + bx1) / 2.0, (y0 + y1) / 2.0),
                    hx: (bx1 - bx0) / 2.0,
                    hy: (y1 - y0) / 2.0,
                    heading: 0.0,
                };
                self.fill_block(block);
            }
        }
    }

    fn fill_block(&mut self, b: Rect) {
        let c = b.c;
        if nearest_base(c).0 < BASE_OPEN {
            return;
        }
        if in_park(c, 0.0) {
            self.area(b, Ground::Lawn);
            return;
        }
        let h = self.hash(0x626C_6F6B, c);
        let (pick, more) = (unit(h, 0), unit(h, 24));
        // A field's block is a square (or a park in the north).
        if self.ore.iter().any(|&(p, r)| b.outside(p) < r) {
            let ground = if c.1 > 9_000.0 {
                Ground::Lawn
            } else {
                Ground::Paving
            };
            self.area(b, ground);
            return;
        }
        let district = self.district(c);
        let ground = match district {
            District::Belt | District::Logistics => Ground::Yard,
            _ => Ground::Paving,
        };
        self.area(b, ground);
        // Buildings stand a step back from the pavement.
        let b = Rect {
            hx: b.hx - 2.5,
            hy: b.hy - 2.5,
            ..b
        };
        // Now and then a block flattened outright.
        let wall = y_at(WALL, c.0);
        let flatten = 0.22 * (1.0 - smoothstep(300.0, 1_400.0, c.1 - wall)) + 0.015;
        if more < flatten {
            self.ruined_block(b);
            return;
        }
        if (b.c.0 - STATION.0).abs() < 1.0 && (b.c.1 - STATION.1).abs() < 70.0 {
            self.try_place(PropKind::CityStation, b.c, 0.0, 1.0, 0.1);
            return;
        }
        // A merged block is built as two, with a yard or lane between.
        if b.hx > 80.0 || b.hy > 80.0 {
            let along_x = b.hx >= b.hy;
            for side in [-1.0, 1.0] {
                let half = if along_x {
                    Rect {
                        c: (b.c.0 + side * b.hx / 2.0, b.c.1),
                        hx: b.hx / 2.0 - GAP,
                        ..b
                    }
                } else {
                    Rect {
                        c: (b.c.0, b.c.1 + side * b.hy / 2.0),
                        hy: b.hy / 2.0 - GAP,
                        ..b
                    }
                };
                let g = self.hash(0x6861_6C66, half.c);
                self.build(half, district, unit(g, 0), unit(g, 24));
            }
            return;
        }
        self.build(b, district, pick, unit(h, 48));
    }

    /// Fills a block from its district's templates; `pick` and `tall` are
    /// the block's own rolls.
    fn build(&mut self, b: Rect, district: District, pick: f64, tall: f64) {
        let c = b.c;
        use PropKind::*;
        match district {
            District::Belt => match pick {
                p if p < 0.35 => self.pair(b, CityWarehouse, CityGarage),
                p if p < 0.55 => self.one(b, CityFactory),
                p if p < 0.7 => self.rows(b, CityTenement, 2),
                p if p < 0.82 => self.one(b, CityTankFarm),
                _ => self.ruined_block(b),
            },
            District::Logistics => match pick {
                p if p < 0.6 => self.pair(b, CityWarehouse, CityWarehouse),
                p if p < 0.8 => self.one(b, CityFactory),
                _ => self.pair(b, CityGarage, CityWarehouse),
            },
            District::Downtown => {
                let core = downtown(c);
                match pick {
                    p if p < 0.12 => self.plaza(b),
                    // Towers thickest at the heart, offices and squares between them.
                    p if p < 0.16 + 0.44 * core => {
                        let kind = match tall {
                            t if t < 0.03 + 0.25 * core * core => CitySpire,
                            t if t < 0.4 => CitySkyscraper,
                            t if t < 0.75 => CityHighrise,
                            _ => CitySlab,
                        };
                        self.tower(b, kind)
                    }
                    p if p < 0.88 => self.pair(b, CityOffice, CityOffice),
                    _ => self.one(b, CityMall),
                }
            }
            District::OldTown => match pick {
                p if p < 0.45 => self.rows(b, CityTenement, 4),
                p if p < 0.8 => self.one(b, CityCourtyard),
                p if p < 0.94 => self.rows(b, CityShops, 4),
                _ => self.plaza(b),
            },
            District::Midtown => match pick {
                p if p < 0.3 => self.one(b, CityCourtyard),
                p if p < 0.5 => self.pair(b, CityOffice, CityGarage),
                p if p < 0.65 => self.rows(b, CityApartments, 3),
                p if p < 0.72 => self.one(b, CityMall),
                p if p < 0.95 => self.rows(b, CityTenement, 4),
                _ => self.plaza(b),
            },
            District::Residential => match pick {
                p if p < 0.26 => self.one(b, CityCourtyard),
                p if p < 0.52 => self.rows(b, CityTenement, 4),
                p if p < 0.72 => self.rows(b, CityApartments, 3),
                p if p < 0.93 => self.rows(b, CityRowhouses, 5),
                _ => self.green(b),
            },
        }
    }

    /// One building filling the block's middle, scaled up a little to fill a
    /// big block, never past what fits.
    fn one(&mut self, b: Rect, kind: PropKind) {
        let (hx, hy) = size_of(kind);
        let turn = hx > hy && b.hy > b.hx;
        let (fx, fy) = if turn { (hy, hx) } else { (hx, hy) };
        let fit = (b.hx / fx).min(b.hy / fy);
        if fit < 0.85 {
            return self.rows(b, PropKind::CityTenement, 1);
        }
        let scale = fit.min(1.25) * 0.97;
        let h = self.hash(0x6F6E_6531, b.c);
        // Front to the south or north at random, east or west if turned.
        let flip = if unit(h, 0) < 0.5 { 0.0 } else { PI };
        let heading = if turn { FRAC_PI_2 + flip } else { flip };
        let wear = self.wear(b.c, 1);
        if !self.try_place(kind, b.c, heading, scale, wear) {
            self.try_place(kind, b.c, heading, scale * 0.85, wear);
        }
    }

    /// A tower in the middle of the block, its podium filling it.
    fn tower(&mut self, b: Rect, kind: PropKind) {
        let (hx, hy) = size_of(kind);
        let scale = (b.hx / hx).min(b.hy / hy).clamp(0.85, 1.15) * 0.97;
        let h = self.hash(0x746F_7765, b.c);
        let heading = (unit(h, 0) * 4.0).floor() * FRAC_PI_2;
        let wear = 0.6 * self.wear(b.c, 2);
        if !self.try_place(kind, b.c, heading, scale, wear) {
            self.pair(b, PropKind::CityOffice, PropKind::CityOffice);
        }
    }

    /// Two buildings side by side along the block's longer way.
    fn pair(&mut self, b: Rect, first: PropKind, second: PropKind) {
        let along_x = b.hx >= b.hy;
        for (i, kind) in [first, second].into_iter().enumerate() {
            let side = if i == 0 { -0.5 } else { 0.5 };
            let c = if along_x {
                (b.c.0 + side * b.hx, b.c.1)
            } else {
                (b.c.0, b.c.1 + side * b.hy)
            };
            let half = Rect {
                c,
                hx: if along_x { b.hx / 2.0 - GAP } else { b.hx },
                hy: if along_x { b.hy } else { b.hy / 2.0 - GAP },
                heading: 0.0,
            };
            self.one(half, kind);
        }
    }

    /// Buildings in rows along the block's north and south edges (and the
    /// east and west ones where there is room), fronts to the street.
    fn rows(&mut self, b: Rect, kind: PropKind, most: usize) {
        let (hx, hy) = size_of(kind);
        // As many as fit near their own size, scaled to fill the edge.
        let fit = |span: f64, most: usize| {
            let n = ((span + GAP) / (2.0 * hx * 0.9 + GAP))
                .floor()
                .clamp(0.0, most as f64);
            let scale = (span - (n - 1.0) * GAP) / (n * 2.0 * hx);
            (n as usize, scale.clamp(0.8, 1.15))
        };
        let (n, scale) = fit(2.0 * b.hx, most);
        if n == 0 {
            return;
        }
        let dy = hy * scale;
        let step = 2.0 * b.hx / n as f64;
        for (edge, heading) in [(1.0, 0.0), (-1.0, PI)] {
            let y = b.c.1 + edge * (b.hy - dy);
            for i in 0..n {
                let x = b.c.0 - b.hx + step * (i as f64 + 0.5);
                let wear = self.wear((x, y), i as u64 + 3);
                self.try_place(kind, (x, y), heading, scale, wear);
            }
        }
        // The side edges, between the rows, turned to face their streets.
        let inner = 2.0 * (b.hy - 2.0 * dy - GAP);
        let (m, side_scale) = fit(inner, most);
        if m == 0 || (side_scale - scale).abs() > 0.3 {
            return;
        }
        let side_step = inner / m as f64;
        for (edge, heading) in [(1.0, -FRAC_PI_2), (-1.0, FRAC_PI_2)] {
            let x = b.c.0 + edge * (b.hx - hy * side_scale);
            for i in 0..m {
                let y = b.c.1 - inner / 2.0 + side_step * (i as f64 + 0.5);
                let wear = self.wear((x, y), i as u64 + 9);
                self.try_place(kind, (x, y), heading, side_scale, wear);
            }
        }
    }

    /// A paved square with trees round it.
    fn plaza(&mut self, b: Rect) {
        self.plazas.push(b.c);
        for i in 0..4 {
            let (sx, sy) = ([-1.0, 1.0, 1.0, -1.0][i], [-1.0, -1.0, 1.0, 1.0][i]);
            let p = (b.c.0 + sx * (b.hx - 10.0), b.c.1 + sy * (b.hy - 10.0));
            self.planted.push((p, PropKind::TreeBroadleaf, 900));
        }
    }

    /// A pocket park: lawn and a few trees.
    fn green(&mut self, b: Rect) {
        self.area(b, Ground::Lawn);
        for i in 0..7 {
            let h = hash2(self.seed ^ 0x6772_6E73, b.c.0 as i64, i);
            let p = (
                b.c.0 + (unit(h, 0) - 0.5) * 1.6 * b.hx,
                b.c.1 + (unit(h, 24) - 0.5) * 1.6 * b.hy,
            );
            self.planted
                .push((p, PropKind::TreeBroadleaf, 800 + (h % 500) as u16));
        }
    }

    /// A block knocked flat: rubble ground, a ruin or two still standing,
    /// heaps, craters.
    fn ruined_block(&mut self, b: Rect) {
        self.area(b, Ground::Rubble);
        let h = self.hash(0x7275_696E, b.c);
        if unit(h, 0) < 0.7 {
            let at = (b.c.0 - 0.4 * b.hx, b.c.1 + 0.4 * b.hy);
            self.try_place(PropKind::CityRuin, at, 0.0, 1.0, 0.75 + 0.15 * unit(h, 8));
        }
        if unit(h, 16) < 0.5 {
            let at = (b.c.0 + 0.4 * b.hx, b.c.1 - 0.4 * b.hy);
            self.try_place(PropKind::CityRuin, at, PI, 1.0, 0.7 + 0.2 * unit(h, 30));
        }
        for i in 0..6 {
            let g = hash2(self.seed ^ 0x6865_6170, b.c.0 as i64 + i, b.c.1 as i64);
            let p = (
                b.c.0 + (unit(g, 0) - 0.5) * 1.5 * b.hx,
                b.c.1 + (unit(g, 24) - 0.5) * 1.5 * b.hy,
            );
            self.try_place(
                PropKind::CityRubble,
                p,
                unit(g, 48) * PI,
                0.8 + 0.5 * unit(g, 40),
                0.0,
            );
        }
        for i in 0..3 {
            let g = hash2(self.seed ^ 0x6372_7472, b.c.0 as i64, b.c.1 as i64 + i);
            self.craters.push(Crater {
                at: (
                    b.c.0 + (unit(g, 0) - 0.5) * 1.4 * b.hx,
                    b.c.1 + (unit(g, 24) - 0.5) * 1.4 * b.hy,
                ),
                radius: 5.0 + 7.0 * unit(g, 48),
                depth: 1.2 + 1.8 * unit(g, 36),
            });
        }
    }

    // -- the outskirts ----------------------------------------------------------

    /// Suburbs behind the glacis: a grid of small streets, houses facing them
    /// across front gardens, shops along the highways.
    fn suburbs(&mut self) {
        const DX: f64 = 128.0;
        const DY: f64 = 104.0;
        let (sx0, sx1) = (700.0, SIZE - 700.0);
        let near_highway = |plan: &Planner, p: P, reach: f64| {
            plan.roads_index
                .near(p, reach)
                .map(|i| plan.roads[i as usize])
                .any(|s| s.road == Road::Highway && s.distance(p) < reach)
        };
        // Streets: east-west lines at the belt's depths, north-south ones
        // every 128 m, each piece only where it is in the belt.
        // A street reaches half a block past the last houses.
        let in_belt = |p: P, past: f64| {
            let d = y_at(WALL, p.0) - p.1;
            (SUBURBS.0..suburb_depth(p.0) + past).contains(&d) && !in_estate(p, 80.0)
        };
        let rows = ((SUBURBS.1 - SUBURBS.0) / DY) as i64;
        for j in 0..=rows {
            let off = SUBURBS.0 + j as f64 * DY;
            let mut x = sx0;
            while x < sx1 {
                let (a, b) = ((x, y_at(WALL, x) - off), (x + DX, y_at(WALL, x + DX) - off));
                let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                if in_belt(mid, DY / 2.0) {
                    self.road(seg(a, b, Road::Street, SUBURB.0, SUBURB.1));
                }
                x += DX;
            }
        }
        let mut x = sx0;
        while x <= sx1 {
            for j in 0..rows {
                let a = (x, y_at(WALL, x) - SUBURBS.0 - j as f64 * DY);
                let b = (x, a.1 - DY);
                if in_belt(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), 0.0) {
                    self.road(seg(a, b, Road::Street, SUBURB.0, SUBURB.1));
                }
            }
            x += DX;
        }
        // Houses, two rows per block, back to back; shops where a highway runs.
        let mut x = sx0;
        while x < sx1 {
            for j in 0..rows {
                let top = y_at(WALL, x + DX / 2.0) - SUBURBS.0 - j as f64 * DY;
                let c = (x + DX / 2.0, top - DY / 2.0);
                if !in_belt(c, -DY / 2.0) {
                    continue;
                }
                let block = Rect {
                    c,
                    hx: DX / 2.0 - SUBURB.0 - SUBURB.1,
                    hy: DY / 2.0 - SUBURB.0 - SUBURB.1,
                    heading: 0.0,
                };
                if self.ore.iter().any(|&(p, r)| block.outside(p) < 1.5 * r) {
                    continue;
                }
                self.area(block, Ground::Lawn);
                if near_highway(self, c, 90.0) {
                    self.rows(block, PropKind::CityShops, 4);
                    continue;
                }
                let h = self.hash(0x7375_6273, c);
                let kind = if unit(h, 0) < 0.25 {
                    PropKind::CityRowhouses
                } else {
                    PropKind::CityHouse
                };
                let (hx, _) = size_of(kind);
                for (edge, heading) in [(1.0, 0.0), (-1.0, PI)] {
                    let y = c.1 + edge * (block.hy - 14.0);
                    let n = ((2.0 * block.hx) / (2.0 * hx + 8.0)).floor().max(1.0) as usize;
                    for i in 0..n {
                        let px = c.0 - block.hx + (i as f64 + 0.5) * 2.0 * block.hx / n as f64;
                        let g = self.hash(0x686F_7573 + i as u64, (px, y));
                        let scale = 0.9 + 0.2 * unit(g, 0);
                        let wear = self.wear((px, y), i as u64);
                        // A few lots stand empty, a few hold only rubble.
                        if unit(g, 24) < 0.08 {
                            continue;
                        }
                        if wear > 0.8 && unit(g, 48) < 0.5 {
                            self.try_place(
                                PropKind::CityRubble,
                                (px, y),
                                unit(g, 40) * PI,
                                1.0,
                                0.0,
                            );
                            continue;
                        }
                        self.try_place(kind, (px, y), heading, scale, wear);
                    }
                }
                // Garden trees behind.
                for i in 0..3 {
                    let g = hash2(self.seed ^ 0x6761_7264, c.0 as i64 + i, c.1 as i64);
                    if unit(g, 0) < 0.6 {
                        let p = (
                            c.0 + (unit(g, 8) - 0.5) * 1.6 * block.hx,
                            c.1 + (unit(g, 32) - 0.5) * 8.0,
                        );
                        self.planted
                            .push((p, PropKind::TreeBroadleaf, 700 + (g % 500) as u16));
                    }
                }
            }
            x += DX;
        }
    }

    /// The industrial estate: yards between works roads, warehouses, works
    /// halls and tank farms.
    fn estate(&mut self) {
        let ((x0, y0), (x1, y1)) = ESTATE;
        const DX: f64 = 192.0;
        const DY: f64 = 176.0;
        let (nx, ny) = (((x1 - x0) / DX) as i64, ((y1 - y0) / DY) as i64);
        let open = |p: P| nearest_base(p).0 < BASE_OPEN - 40.0;
        for j in 0..=ny {
            let y = y0 + j as f64 * DY;
            for i in 0..nx {
                let (a, b) = ((x0 + i as f64 * DX, y), (x0 + (i + 1) as f64 * DX, y));
                if !open(((a.0 + b.0) / 2.0, y)) {
                    self.road(seg(a, b, Road::Street, WORKS.0, WORKS.1));
                }
            }
        }
        for i in 0..=nx {
            let x = x0 + i as f64 * DX;
            for j in 0..ny {
                let (a, b) = ((x, y0 + j as f64 * DY), (x, y0 + (j + 1) as f64 * DY));
                if !open((x, (a.1 + b.1) / 2.0)) {
                    self.road(seg(a, b, Road::Street, WORKS.0, WORKS.1));
                }
            }
        }
        let half = WORKS.0 + WORKS.1;
        for j in 0..ny {
            for i in 0..nx {
                let c = (x0 + (i as f64 + 0.5) * DX, y0 + (j as f64 + 0.5) * DY);
                // Its edge is ragged: some outer plots stand empty.
                let edge = i == 0 || j == 0 || i == nx - 1 || j == ny - 1;
                if open(c) || (edge && unit(self.hash(0x6564_6765, c), 0) < 0.45) {
                    continue;
                }
                let b = Rect {
                    c,
                    hx: DX / 2.0 - half,
                    hy: DY / 2.0 - half,
                    heading: 0.0,
                };
                self.area(b, Ground::Yard);
                let h = self.hash(0x6573_7461, c);
                use PropKind::*;
                match unit(h, 0) {
                    p if p < 0.35 => self.pair(b, CityWarehouse, CityWarehouse),
                    p if p < 0.6 => self.one(b, CityFactory),
                    p if p < 0.78 => self.pair(b, CityTankFarm, CityWarehouse),
                    p if p < 0.9 => self.one(b, CityWarehouse),
                    _ => {}
                }
            }
        }
    }

    /// Farmsteads by the lanes and the country road, each with a house.
    fn farms(&mut self) {
        for j in 0..9_i64 {
            for i in 0..18_i64 {
                let h = hash2(self.seed ^ 0x6661_726D, i, j);
                let p = (
                    (i as f64 + 0.2 + 0.6 * unit(h, 0)) * 680.0,
                    (j as f64 + 0.2 + 0.6 * unit(h, 24)) * 560.0,
                );
                if y_at(WALL, p.0) - p.1 < suburb_depth(p.0) + 150.0 || in_estate(p, 200.0) {
                    continue;
                }
                // The nearest lane or road, to face.
                let Some(s) = self
                    .roads_index
                    .near(p, 140.0)
                    .map(|k| self.roads[k as usize])
                    .filter(|s| s.distance(p) < 140.0)
                    .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
                else {
                    continue;
                };
                // Set back from the road, facing it.
                let foot = closest_on(p, s.a, s.b);
                let away = ((p.0 - foot.0), (p.1 - foot.1));
                let len = away.0.hypot(away.1).max(1.0);
                let n = (away.0 / len, away.1 / len);
                let c = (
                    foot.0 + n.0 * (s.reach() + 40.0),
                    foot.1 + n.1 * (s.reach() + 40.0),
                );
                // +y faces the road.
                let heading = (-n.1).atan2(-n.0) - FRAC_PI_2;
                let wear = self.wear(c, 5);
                if self.try_place(PropKind::CityFarmstead, c, heading, 1.0, wear) {
                    let side = (c.0 + n.1 * 48.0, c.1 - n.0 * 48.0);
                    self.try_place(PropKind::CityHouse, side, heading, 1.0, wear);
                }
            }
        }
    }

    /// Villages strung along the country road: houses and shops facing it
    /// either side, a church in the middle, farmsteads at the ends.
    fn villages(&mut self) {
        for (v, &centre) in VILLAGES.iter().enumerate() {
            let roads: Vec<Seg> = self
                .roads_index
                .near(centre, 420.0)
                .map(|i| self.roads[i as usize])
                .filter(|s| s.road == Road::Highway && s.half == COUNTRY)
                .filter(|s| s.distance(centre) < 380.0)
                .collect();
            let mut church = false;
            for s in roads {
                let len = dist(s.a, s.b);
                let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
                let normal = (-dir.1, dir.0);
                let mut t = 6.0;
                while t < len {
                    let on = (s.a.0 + dir.0 * t, s.a.1 + dir.1 * t);
                    let from_middle = dist(on, centre);
                    if from_middle > 360.0 {
                        t += 26.0;
                        continue;
                    }
                    for side in [1.0, -1.0] {
                        let h = self.hash(0x7669_6C6C ^ v as u64, (on.0 + side, on.1));
                        let kind = match unit(h, 0) {
                            _ if !church && from_middle < 60.0 => PropKind::CityChurch,
                            _ if from_middle > 300.0 => PropKind::CityFarmstead,
                            u if u < 0.3 && from_middle < 160.0 => PropKind::CityShops,
                            u if u < 0.5 => PropKind::CityRowhouses,
                            u if u < 0.92 => PropKind::CityHouse,
                            _ => continue,
                        };
                        let (_, hy) = size_of(kind);
                        let back = s.reach() + hy + 5.0 + 4.0 * unit(h, 24);
                        let c = (on.0 + side * normal.0 * back, on.1 + side * normal.1 * back);
                        // +y faces the road.
                        let heading = dir.1.atan2(dir.0) + if side > 0.0 { PI } else { 0.0 };
                        let wear = self.wear(c, 11);
                        if self.try_place(kind, c, heading, 0.95 + 0.1 * unit(h, 48), wear)
                            && kind == PropKind::CityChurch
                        {
                            church = true;
                        }
                    }
                    t += 26.0;
                }
            }
        }
    }

    /// Trees down the avenues' medians and along residential pavements.
    fn street_trees(&mut self) {
        let mut trees = Vec::new();
        for s in &self.roads {
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            let normal = (-dir.1, dir.0);
            let (spacing, offsets): (f64, &[f64]) = match s.road {
                Road::Avenue => (15.0, &[0.0]),
                Road::Street if s.walk >= STREET.1 => (19.0, &[1.0, -1.0]),
                _ => continue,
            };
            let mid = ((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0);
            let leafy = Terrain::in_city(mid)
                && downtown(mid) < 0.3
                && (s.road == Road::Avenue || mid.1 > 8_700.0);
            if !leafy {
                continue;
            }
            let n = (len / spacing).floor() as usize;
            for i in 1..n {
                let t = i as f64 * spacing;
                for &side in offsets {
                    let off = if side == 0.0 {
                        0.0
                    } else {
                        side * (s.half + 0.5 * s.walk)
                    };
                    let p = (
                        s.a.0 + dir.0 * t + normal.0 * off,
                        s.a.1 + dir.1 * t + normal.1 * off,
                    );
                    trees.push((p, s));
                }
            }
        }
        for (p, s) in trees {
            // Clear of every other road's carriageway (crossings) and of lots.
            let crossing = self
                .roads_index
                .near(p, 40.0)
                .map(|i| &self.roads[i as usize])
                .any(|o| (o.a, o.b) != (s.a, s.b) && o.distance(p) < o.half + 6.0);
            let on_lot = self
                .lots_index
                .near(p, 30.0)
                .any(|i| self.lots[i as usize].rect.outside(p) < LOT_FLAT + 1.0);
            if crossing || on_lot || nearest_base(p).0 < BASE_OPEN {
                continue;
            }
            let h = self.hash(0x7374_7472, p);
            self.planted
                .push((p, PropKind::TreeBroadleaf, 650 + (h % 300) as u16));
        }
    }

    /// Shell craters: thick on the glacis, scattered along the front on both
    /// sides, a few deep in the city and out in the fields.
    fn craters(&mut self) {
        let step = 40.0;
        let (n, m) = ((SIZE / step) as i64, (SIZE / step) as i64);
        for j in 0..m {
            for i in 0..n {
                let h = hash2(self.seed ^ 0x6372_6174, i, j);
                let p = (
                    (i as f64 + unit(h, 0)) * step,
                    (j as f64 + unit(h, 24)) * step,
                );
                let d = p.1 - y_at(WALL, p.0);
                let chance = if (-380.0..-30.0).contains(&d) {
                    0.32
                } else if d < 0.0 {
                    0.07 * (1.0 - smoothstep(400.0, 2_200.0, -d)) + 0.004
                } else {
                    0.03 * (1.0 - smoothstep(150.0, 1_800.0, d)) + 0.002
                };
                if unit(h, 48) >= chance
                    || nearest_base(p).0 < BASE_OPEN + 60.0
                    || self.ore.iter().any(|&(o, r)| dist(o, p) < 1.6 * r + 20.0)
                    || (-30.0..60.0).contains(&d)
                {
                    continue;
                }
                let g = hash2(self.seed ^ 0x6465_6570, i, j);
                let big = unit(g, 0).powi(3);
                self.craters.push(Crater {
                    at: p,
                    radius: 3.5 + 13.0 * big,
                    depth: (0.8 + 4.0 * big) * (0.7 + 0.3 * unit(g, 24)),
                });
            }
        }
    }
}

/// Whether grid cell `(k, j)` is built as one block with the cell east of
/// it, the street between left out. Cells pair up from even columns, never
/// across an avenue or in the ring road's row.
fn merged_east(seed: u64, k: i64, j: i64) -> bool {
    if (k, j) == STATION_CELL {
        return true;
    }
    j >= 0
        && k.rem_euclid(2) == 0
        && !AVENUE_COLUMNS.contains(&(k + 1))
        && unit(hash2(seed ^ 0x6D65_7267, k, j), 0) < 0.16
}

/// Whether grid cell `(k, j)` is built as one block with the cell north of
/// it. Rows pair up from even rows; a cell merged east or west is not.
fn merged_north(seed: u64, k: i64, j: i64) -> bool {
    j >= 0
        && j.rem_euclid(2) == 0
        && !AVENUE_ROWS.contains(&(j + 1))
        && ![(k, j), (k, j + 1)]
            .iter()
            .any(|&(k, j)| merged_east(seed, k, j) || merged_east(seed, k - 1, j))
        && unit(hash2(seed ^ 0x6D65_7268, k, j), 0) < 0.12
}

/// How deep the suburbs reach south of the wall at `x`: their edge wanders
/// between about 1 and 1.65 km.
pub(super) fn suburb_depth(x: f64) -> f64 {
    SUBURBS.1 - 220.0 * (1.0 + (x / 830.0 + 0.7).sin()) - 110.0 * (1.0 + (x / 310.0 + 2.1).sin())
}

/// Whether `p` is out among the farms: south of the suburbs, off the estate
/// and the bases.
pub(super) fn farmland(p: P) -> bool {
    y_at(WALL, p.0) - p.1 > suburb_depth(p.0) + 40.0
        && !in_estate(p, 30.0)
        && nearest_base(p).0 > BASE_OPEN - 40.0
}

pub(super) fn in_estate(p: P, margin: f64) -> bool {
    let ((x0, y0), (x1, y1)) = ESTATE;
    p.0 > x0 - margin && p.0 < x1 + margin && p.1 > y0 - margin && p.1 < y1 + margin
}

fn closest_on(p: P, a: P, b: P) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    (a.0 + t * dx, a.1 + t * dy)
}

/// The gate's road passage is wider than any road through it.
const _: () = assert!(GATE_PASSAGE_M as f64 > 2.0 * HIGHWAY + 8.0);
