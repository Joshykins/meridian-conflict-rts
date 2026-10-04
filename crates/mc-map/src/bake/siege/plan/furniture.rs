//! What a city of a spacefaring people has besides its blocks: the maglev
//! over its avenues and its stations, street lights, billboards, cars parked
//! and burnt out, the barricades of the siege, comms masts, the squares'
//! monuments; out in the country, wind turbines on the hills and solar farms.
//!
//! Laid last, after every block, so it fills what the blocks left. Most of it
//! is dressing ([`Dressing`]): nothing walks round it and its ground is not
//! levelled. The guideway's pylons stand in the avenues' medians, between
//! the carriageways, and come down like any structure.

use super::*;
use crate::city::TRANSIT_SEGMENT_M;

/// The avenue rows the maglev runs along, west to east.
const TRANSIT_ROWS: [i64; 2] = [15, 33];
/// A station every this many blocks along a line.
const STATION_EVERY: i64 = 11;
/// Street lights' spacing along a street, metres.
const LIGHT_SPACING: f64 = 36.0;

/// A prop the plan lays that neither blocks the plan nor is levelled for.
#[derive(Clone, Copy, Debug)]
pub(in crate::bake) struct Dressing {
    pub(in crate::bake) kind: PropKind,
    pub(in crate::bake) at: P,
    pub(in crate::bake) heading: f64,
    pub(in crate::bake) scale: f64,
    pub(in crate::bake) wear: f64,
}

impl Planner {
    pub(super) fn furniture(&mut self) {
        self.maglev();
        self.street_lights();
        self.cars();
        self.billboards();
        self.barricades();
        self.monuments();
        self.masts();
        self.wind_farms();
        self.solar_farms();
    }

    fn dress(&mut self, kind: PropKind, at: P, heading: f64, scale: f64, wear: f64) {
        self.dressing.push(Dressing {
            kind,
            at,
            heading,
            scale,
            wear,
        });
    }

    /// Whether the road at `p` is a carriageway of kind `road` (its middle within `slack`).
    fn on_road(&self, p: P, road: Road, slack: f64) -> bool {
        self.roads_index
            .near(p, 40.0)
            .map(|i| &self.roads[i as usize])
            .any(|s| s.road == road && s.distance(p) < slack)
    }

    /// Whether `p` is in a crossing: within reach of a road other than one
    /// running along `along`.
    fn near_crossing(&self, p: P, along: P, margin: f64) -> bool {
        self.roads_index
            .near(p, 50.0)
            .map(|i| &self.roads[i as usize])
            .any(|s| {
                let (dx, dy) = (s.b.0 - s.a.0, s.b.1 - s.a.1);
                let len = dx.hypot(dy).max(1e-6);
                let cross = (dx * along.1 - dy * along.0).abs() / len;
                cross > 0.3 && s.distance(p) < s.reach() + margin
            })
    }

    /// Two maglev lines along avenue medians, west to east, a station every
    /// few blocks; never through a base, the square or a crossing road.
    fn maglev(&mut self) {
        let seg = TRANSIT_SEGMENT_M as f64;
        for &row in &TRANSIT_ROWS {
            let y = GRID_Y0 + row as f64 * PITCH;
            for k in -48..48_i64 {
                let x0 = MID + k as f64 * PITCH;
                let block_mid = (x0 + PITCH / 2.0, y);
                if x0 < 40.0 || x0 + PITCH > SIZE - 40.0 {
                    continue;
                }
                let open = |p: P| {
                    nearest_base(p).0 < BASE_OPEN + 40.0
                        || ((p.0 - SQUARE.0).abs() < SQUARE_HALF + 40.0
                            && (p.1 - SQUARE.1).abs() < SQUARE_HALF + 40.0)
                };
                if open(block_mid) || !self.on_road(block_mid, Road::Avenue, 1.0) {
                    continue;
                }
                let station = k.rem_euclid(STATION_EVERY) == 5;
                if station && !self.near_crossing(block_mid, (1.0, 0.0), 6.0) {
                    let wear = 0.5 * self.wear(block_mid, 31);
                    self.place(PropKind::CityTransitStation, block_mid, 0.0, 1.0, wear);
                    continue;
                }
                for half in [0.25, 0.75] {
                    let at = (x0 + PITCH * half, y);
                    // The pylon keeps clear of the crossing streets and boulevards.
                    if self.near_crossing(at, (1.0, 0.0), 4.0) {
                        continue;
                    }
                    let wear = 0.5 * self.wear(at, 37);
                    self.place(PropKind::CityTransit, at, 0.0, PITCH / 2.0 / seg, wear);
                }
            }
        }
    }

    /// Street lights along the city's streets and avenues, both sides, at the
    /// kerb; along the outskirts' highways on one side.
    fn street_lights(&mut self) {
        let mut lights = Vec::new();
        for s in &self.roads {
            let city = Terrain::in_city(s.a);
            let sides: &[f64] = match s.road {
                Road::Street | Road::Avenue if city && s.walk > 0.0 => &[1.0, -1.0],
                Road::Highway if s.half > 10.0 => &[1.0],
                _ => continue,
            };
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            let normal = (-dir.1, dir.0);
            let spacing = if city { LIGHT_SPACING } else { 50.0 };
            let n = (len / spacing).floor() as usize;
            for i in 0..n {
                let t = (i as f64 + 0.5) * len / n.max(1) as f64;
                for &side in sides {
                    let off = side * (s.half + if city { 0.7 } else { 2.5 });
                    let p = (
                        s.a.0 + dir.0 * t + normal.0 * off,
                        s.a.1 + dir.1 * t + normal.1 * off,
                    );
                    // The arms reach over the road.
                    let heading = (-side * normal.1).atan2(-side * normal.0);
                    lights.push((p, heading, dir));
                }
            }
        }
        for (p, heading, along) in lights {
            if self.near_crossing(p, along, 3.0) || nearest_base(p).0 < BASE_OPEN {
                continue;
            }
            let wear = self.wear(p, 41);
            self.dress(PropKind::CityStreetLight, p, heading, 1.0, wear);
        }
    }

    /// Cars parked at the kerb of the city's quieter streets; burnt-out ones
    /// left across the roads near the front; some abandoned on the highways.
    fn cars(&mut self) {
        let mut cars = Vec::new();
        for s in &self.roads {
            if !matches!(s.road, Road::Street | Road::Highway) {
                continue;
            }
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            let normal = (-dir.1, dir.0);
            let heading = dir.1.atan2(dir.0);
            let n = (len / 7.0).floor() as usize;
            for i in 1..n {
                let t = i as f64 * 7.0;
                for side in [1.0, -1.0] {
                    let lane = if s.road == Road::Highway {
                        s.half * 0.5
                    } else {
                        s.half - 1.3
                    };
                    let p = (
                        s.a.0 + dir.0 * t + normal.0 * side * lane,
                        s.a.1 + dir.1 * t + normal.1 * side * lane,
                    );
                    cars.push((p, heading + if side < 0.0 { PI } else { 0.0 }, dir));
                }
            }
        }
        for (p, heading, along) in cars {
            let h = self.hash(0x6361_7273, p);
            let wear = self.wear(p, 43);
            let city = Terrain::in_city(p);
            // Parked where the street is quiet, more of them wrecked the worse it is.
            let chance = if city { 0.05 } else { 0.012 } + 0.06 * wear;
            if unit(h, 0) >= chance
                || self.near_crossing(p, along, 2.0)
                || nearest_base(p).0 < BASE_OPEN
                || self
                    .lots_index
                    .near(p, 20.0)
                    .any(|i| self.lots[i as usize].rect.outside(p) < 2.0)
            {
                continue;
            }
            // A wreck sits askew, shoved by whatever hit it.
            let burnt = wear > 0.45 && unit(h, 24) < wear;
            let turn = if burnt {
                (unit(h, 48) - 0.5) * 1.6
            } else {
                (unit(h, 48) - 0.5) * 0.06
            };
            let wear = if burnt {
                0.7 + 0.3 * unit(h, 32)
            } else {
                0.1 * unit(h, 32)
            };
            self.dress(
                PropKind::CityCar,
                p,
                heading + turn,
                0.92 + 0.16 * unit(h, 40),
                wear,
            );
        }
    }

    /// LED billboards on corners downtown and in midtown, and by the highways.
    fn billboards(&mut self) {
        let mut spots = Vec::new();
        for s in &self.roads {
            let mid = ((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0);
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            let normal = (-dir.1, dir.0);
            let busy = Terrain::in_city(mid) && (downtown(mid) > 0.0 || mid.1 < 9_000.0);
            let chance = match s.road {
                Road::Avenue | Road::Street if busy && s.walk > 0.0 => 0.12,
                Road::Highway => 0.05,
                _ => continue,
            };
            if unit(self.hash(0x6269_6C6C, mid), 0) >= chance {
                continue;
            }
            let side = if unit(self.hash(0x7369_6465, mid), 0) < 0.5 {
                1.0
            } else {
                -1.0
            };
            let off = side * (s.half + s.walk.max(4.0) * 0.6);
            let p = (mid.0 + normal.0 * off, mid.1 + normal.1 * off);
            // Its face (local +x) toward the road.
            let heading = (-side * normal.1).atan2(-side * normal.0);
            spots.push((p, heading, dir));
        }
        for (p, heading, along) in spots {
            if self.near_crossing(p, along, 4.0)
                || self
                    .lots_index
                    .near(p, 20.0)
                    .any(|i| self.lots[i as usize].rect.outside(p) < 1.0)
            {
                continue;
            }
            let wear = self.wear(p, 47);
            self.dress(PropKind::CityBillboard, p, heading, 1.0, wear);
        }
    }

    /// Lines of hedgehogs and sandbags across the streets behind the wall,
    /// and in the suburbs before it.
    fn barricades(&mut self) {
        let mut rows = Vec::new();
        for s in &self.roads {
            if !matches!(s.road, Road::Street | Road::Avenue) {
                continue;
            }
            let mid = ((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0);
            let d = mid.1 - y_at(WALL, mid.0);
            let front = (60.0..700.0).contains(&d) || (-SUBURBS.0 - 500.0..-SUBURBS.0).contains(&d);
            if !front || unit(self.hash(0x6261_7272, mid), 0) >= 0.18 {
                continue;
            }
            let len = dist(s.a, s.b);
            let dir = ((s.b.0 - s.a.0) / len, (s.b.1 - s.a.1) / len);
            rows.push((mid, dir, s.half));
        }
        for (mid, dir, half) in rows {
            // Across the road, leaving a lane open at one end.
            let across = (-dir.1, dir.0);
            let heading = across.1.atan2(across.0);
            let n = ((2.0 * half - 6.0) / 16.0).ceil().max(1.0) as usize;
            for i in 0..n {
                let t = -half + 8.0 + i as f64 * 16.0;
                let p = (mid.0 + across.0 * t, mid.1 + across.1 * t);
                self.dress(PropKind::CityBarricade, p, heading, 1.0, 0.3);
            }
        }
    }

    /// A monument in Concord Square and one in every plaza.
    fn monuments(&mut self) {
        self.try_place(
            PropKind::CityMonument,
            (SQUARE.0 - 80.0, SQUARE.1 - 20.0),
            0.0,
            1.4,
            0.1,
        );
        let plazas: Vec<P> = self.plazas.clone();
        for c in plazas {
            let wear = self.wear(c, 53);
            self.try_place(PropKind::CityMonument, c, 0.0, 1.0, wear);
        }
    }

    /// Comms masts: on the old town's hill, by the rail yards and the
    /// spaceport, and on two hills outside.
    fn masts(&mut self) {
        let spots = [
            (SQUARE.0 + 420.0, SQUARE.1 + 520.0),
            (CITY_STARTS[0].0 + 640.0, CITY_STARTS[0].1 - 600.0),
            (CITY_STARTS[2].0 - 640.0, CITY_STARTS[2].1 - 600.0),
            (OUT_STARTS[1].0 - 900.0, OUT_STARTS[1].1 + 500.0),
            (OUT_STARTS[1].0 + 900.0, OUT_STARTS[1].1 + 500.0),
        ];
        for &at in &spots {
            // The first clear spot spiralling out from the one designed.
            for r in 0..40 {
                let a = r as f64 * 2.4;
                let p = (
                    at.0 + a.cos() * r as f64 * 12.0,
                    at.1 + a.sin() * r as f64 * 12.0,
                );
                if self.try_place(PropKind::CityMast, p, 0.0, 1.0, 0.1) {
                    break;
                }
            }
        }
    }

    /// Rows of wind turbines along the outskirts' hills, clear of the lanes.
    fn wind_farms(&mut self) {
        let rows: [(P, P); 3] = [
            ((500.0, 4_300.0), (1_700.0, 2_900.0)),
            ((3_700.0, 500.0), (5_200.0, 700.0)),
            ((7_200.0, 600.0), (8_600.0, 400.0)),
        ];
        for (a, b) in rows {
            let n = (dist(a, b) / 260.0).floor() as usize;
            for i in 0..=n {
                let t = i as f64 / n.max(1) as f64;
                let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                if farmland(p) {
                    self.try_place(PropKind::CityWindTurbine, p, 0.6, 1.0, 0.0);
                }
            }
        }
    }

    /// Solar farms: blocks of arrays in rows beside the estate and the
    /// western villages.
    fn solar_farms(&mut self) {
        let farms: [(P, i32, i32); 3] = [
            ((8_000.0, 1_700.0), 5, 4),
            ((11_300.0, 2_200.0), 3, 6),
            ((900.0, 2_700.0), 4, 3),
        ];
        for (corner, nx, ny) in farms {
            for j in 0..ny {
                for i in 0..nx {
                    let p = (corner.0 + i as f64 * 46.0, corner.1 + j as f64 * 30.0);
                    let clear = farmland(p)
                        && !self.roads_index.near(p, 40.0).any(|k| {
                            let s = &self.roads[k as usize];
                            s.distance(p) < s.reach() + 24.0
                        })
                        && !self
                            .lots_index
                            .near(p, 40.0)
                            .any(|k| self.lots[k as usize].rect.outside(p) < 24.0)
                        && !self.ore.iter().any(|&(o, r)| dist(o, p) < 1.5 * r + 30.0);
                    if clear {
                        self.dress(PropKind::CitySolarArray, p, 0.0, 1.0, 0.0);
                    }
                }
            }
        }
    }
}
