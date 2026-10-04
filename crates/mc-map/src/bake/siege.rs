//! "Halcyon": three against three at the wall of a besieged city.
//!
//! [`Layout::Siege`], 12 km. The north half is the city of Halcyon, an ARC
//! colony's capital on Asteria: a street grid of blocks and avenues, glass
//! towers downtown, an old town on a low hill round Concord Square, parks, a
//! railway and a spaceport. The south half is its outskirts: suburbs burnt out
//! along the front, an industrial estate in the east, farms, lanes and woods.
//! Between them runs the city's wall ([`WALL`]), a 26 m curtain of concrete
//! with towers at its corners and between, pierced by three gates
//! ([`GATES_X`]), and already breached in two places ([`BREACHES_X`]). Its
//! segments are city structures like every building ([`crate::city`]): shells
//! knock them down and open new ways through.
//!
//! The defenders' three bases are inside (the rail yards in the west, the
//! Governor's Park behind the old town, the spaceport in the east), the
//! besiegers' outside (an estate among the farms in the west, the quarry
//! hill on the highway in the south, the works yard in the east), each the
//! same walk from the wall as its opposite. Starts alternate city and
//! outskirts, so skirmish's "Two Sides" (teams by alternate slot) puts the
//! city against the outskirts.
//!
//! A siege is not fair by symmetry. What evens it: every base has the same
//! three fields round it and the same level room; each side has the same
//! number of contested fields at the same walks from its bases (inside the
//! city in its squares and parks, outside in the glacis, the suburbs and the
//! farms); the gates and breaches stand in front of the bases. The city has
//! cover, the wall and its narrow streets; the outskirts have open ground,
//! room to build and roads to the gates. `tests/halcyon.rs` holds the baked
//! map to the walks.
//!
//! The war is old here. Buildings near the front start damaged
//! (`Prop::wear_milli`) and a few are ruins or rubble; the glacis before the
//! wall is shell-cratered earth; craters dot the streets near the front.
//!
//! Everything a unit walks round is laid out at set-up ([`Siege`], in
//! `siege/plan.rs`): the roads, the blocks and their buildings, the wall, the
//! craters. The ground under each building is levelled to it, and the
//! streets layer tells the terrain shader where the roads, pavements, parks
//! and yards are (`tile_streets`).
//!
//! Coordinates are map metres, y north.

use super::{Pad, Terrain};
use crate::format::{Ground, PropKind, Road, StreetSample};
use crate::noise::{bump, smoothstep, unit};
use crate::BUILD_CELL_M;
use grid::{dist, y_at, Buckets, Rect, Seg, P};

mod fields;
mod grid;
mod plan;
mod shape;
#[cfg(test)]
mod tests;
mod wrecks;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 12_288.0;
/// The north-south middle line.
const MID: f64 = SIZE / 2.0;

/// The wall's trace, west to east, from beyond one edge to beyond the other;
/// a tower stands at each corner. Mirror-like about the middle line, so each
/// pair of bases faces the same wall.
pub(super) const WALL: &[P] = &[
    (-200.0, 6_000.0),
    (1_500.0, 6_080.0),
    (3_400.0, 6_240.0),
    (5_000.0, 6_120.0),
    (7_288.0, 6_120.0),
    (8_888.0, 6_240.0),
    (10_788.0, 6_080.0),
    (12_488.0, 6_000.0),
];
/// The gates: where the three roads to the bases go through the wall. Each is
/// on a city avenue (`plan.rs`, 128 m grid from the middle line).
pub(super) const GATES_X: [f64; 3] = [MID - 3_712.0, MID, MID + 3_712.0];
/// The breaches: where the wall is already down, a gap of rubble.
pub(super) const BREACHES_X: [f64; 2] = [4_224.0, 8_064.0];
/// Half a breach's width along the wall, metres.
const BREACH_HALF: f64 = 80.0;

/// The bases, city first then outskirts, west to east. Laid alternately
/// (city, outskirts, ...) as starts.
const CITY_STARTS: [P; 3] = [(2_304.0, 10_500.0), (MID, 11_448.0), (9_984.0, 10_500.0)];
const OUT_STARTS: [P; 3] = [(2_304.0, 1_896.0), (MID, 948.0), (9_984.0, 1_896.0)];
/// How far round a base the ground is kept open and level, metres.
const BASE_CORE: f64 = 340.0;
const BASE_OPEN: f64 = 560.0;

/// Concord Square, at the old town's heart.
const SQUARE: P = (MID, 9_344.0);
/// The old town's hill, under the square.
const OLD_TOWN: (P, f64) = (SQUARE, 760.0);
/// Downtown: an ellipse of towers between the old town and the wall.
const DOWNTOWN: (P, f64, f64) = ((MID, 7_936.0), 1_750.0, 820.0);
/// The parks either side of the old town: centre and half extents.
const PARKS: [(P, f64, f64); 2] = [
    ((4_224.0, 9_600.0), 380.0, 500.0),
    ((8_064.0, 9_600.0), 420.0, 450.0),
];

/// Contested ore inside the city and outside it: centre and radius. Each
/// field inside has its counterpart outside at about the same walk from the
/// bases (`tests/halcyon.rs`).
const CITY_ORE: [(P, f64); 6] = [
    ((2_816.0, 6_784.0), 70.0),
    ((9_472.0, 6_784.0), 70.0),
    ((MID + 160.0, 7_040.0), 75.0),
    ((4_224.0, 9_472.0), 85.0),
    ((8_064.0, 9_600.0), 85.0),
    ((MID, 9_232.0), 70.0),
];
const OUT_ORE: [(P, f64); 6] = [
    ((1_900.0, 5_400.0), 70.0),
    ((10_390.0, 5_400.0), 70.0),
    ((MID - 260.0, 5_250.0), 75.0),
    ((4_100.0, 3_250.0), 85.0),
    ((8_250.0, 3_150.0), 85.0),
    ((MID + 300.0, 3_000.0), 70.0),
];

/// What the siege layout works out at set-up.
#[derive(Default)]
pub(super) struct Siege {
    /// Every road, and an index of them.
    pub(super) roads: Vec<Seg>,
    pub(super) road_index: Option<Buckets>,
    /// Every structure laid, with the level its ground is cut to.
    pub(super) lots: Vec<Lot>,
    pub(super) lot_index: Option<Buckets>,
    /// Ground other than the terrain's own, by area, later ones over earlier.
    pub(super) areas: Vec<Area>,
    pub(super) area_index: Option<Buckets>,
    pub(super) craters: Vec<Crater>,
    pub(super) crater_index: Option<Buckets>,
    /// Trees planted in rows (avenues, pavements, hedges), laid with the
    /// buildings rather than grown by the woods.
    pub(super) planted: Vec<(P, PropKind, u16)>,
}

/// A structure laid on the plan.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lot {
    pub(super) kind: PropKind,
    /// Where the prop stands (its origin) and its heading, radians.
    pub(super) at: P,
    pub(super) heading: f64,
    pub(super) scale: f64,
    /// Damage before the match, 0..=1 of its health.
    pub(super) wear: f64,
    /// The footprint: every solid part inside it.
    pub(super) rect: Rect,
    /// The ground under it, metres; set once the land is shaped.
    pub(super) level: f64,
}

/// An area of made ground.
#[derive(Clone, Copy, Debug)]
pub(super) struct Area {
    pub(super) rect: Rect,
    pub(super) ground: Ground,
}

/// A shell crater: a bowl with a thrown-up rim.
#[derive(Clone, Copy, Debug)]
pub(super) struct Crater {
    pub(super) at: P,
    pub(super) radius: f64,
    pub(super) depth: f64,
}

impl Terrain {
    pub(super) fn setup_siege(&mut self) {
        self.start_outer = 2.0 * BASE_CORE;
        // The land first: the plan levels its lots to it.
        let starts: Vec<P> = CITY_STARTS
            .iter()
            .zip(&OUT_STARTS)
            .flat_map(|(&c, &o)| [c, o])
            .map(|p| self.snap(p))
            .collect();
        self.pads = starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: BASE_CORE,
                outer: BASE_OPEN - 20.0,
                height: self.siege_land(x, y),
            })
            .collect();
        self.starts = starts;

        // Three small fields round each base: one behind, two on the forward
        // flanks.
        let mut sites: Vec<(P, f64)> = Vec::new();
        for &s in &self.starts {
            let city = s.1 > MID;
            let back = if city {
                std::f64::consts::FRAC_PI_2
            } else {
                -std::f64::consts::FRAC_PI_2
            };
            for turn in [
                0.0_f64,
                std::f64::consts::PI - 1.15,
                std::f64::consts::PI + 1.15,
            ] {
                let (sn, c) = (back + turn).sin_cos();
                let d = 0.8 * BASE_CORE;
                sites.push(((s.0 + c * d, s.1 + sn * d), 60.0));
            }
        }
        sites.extend(CITY_ORE.iter().chain(&OUT_ORE).copied());
        let g = BUILD_CELL_M as f64;
        let snapped: Vec<(P, f64)> = sites
            .into_iter()
            .map(|(p, r)| (((p.0 / g).round() * g, (p.1 / g).round() * g), r))
            .collect();

        self.lay_siege_plan(&snapped);
        self.ore = snapped
            .into_iter()
            .map(|(p, r)| self.ore_field(p.0, p.1, r))
            .collect();
        self.forest_edge = 0.0;
        self.fit_forests();
    }

    /// The ground before any lot is levelled or any crater dug: the city's
    /// plain rising gently north to the old town's hill and beyond, the
    /// outskirts' rolling farmland, the bases' pads.
    pub(super) fn siege_land(&self, x: f64, y: f64) -> f64 {
        self.siege_shape(x, y)
    }

    /// Ground with the plan cut in: lots levelled, craters dug.
    pub(super) fn natural_siege(&self, x: f64, y: f64) -> f64 {
        let h = self.siege_land(x, y);
        self.siege_cut(x, y, h)
    }

    /// Whether `p` is inside the city: north of the wall.
    pub(super) fn in_city(p: P) -> bool {
        p.1 > y_at(WALL, p.0)
    }

    /// How thickly trees grow: the parks, the farms' woods and copses; never
    /// on a road, a lot, a yard or the glacis.
    pub(super) fn siege_forest(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        if self.siege_built(p) {
            return 0.0;
        }
        let wall = y_at(WALL, x);
        if y > wall {
            // The parks' groves, thinning toward their lawns' middles.
            return PARKS
                .iter()
                .map(|&(c, rx, ry)| {
                    let e = ((x - c.0) / rx).hypot((y - c.1) / ry);
                    let grove = smoothstep(
                        0.15,
                        0.45,
                        self.forest.fbm(x / 160.0 + 3.1, y / 160.0 - 8.4, 2, 0.5),
                    );
                    (1.0 - smoothstep(0.85, 1.0, e)) * (0.25 + 0.75 * grove)
                })
                .fold(0.0, f64::max);
        }
        // Hedgerows along some of the fields' borders.
        let hedge = if plan::farmland(p) {
            let (field, edge, other) = fields::field_at(p);
            if fields::hedged(field, other) {
                0.95 * (1.0 - smoothstep(1.5, 3.5, edge))
            } else {
                0.0
            }
        } else {
            0.0
        };
        // The glacis is bare; the woods come back with distance from it.
        let open = smoothstep(wall - 900.0, wall - 1_700.0, y);
        let woods = smoothstep(
            0.18,
            0.42,
            self.forest
                .fbm(x / 1_300.0 + 7.7, y / 1_300.0 - 2.9, 3, 0.5),
        );
        let copse = smoothstep(
            0.48,
            0.62,
            self.forest.fbm(x / 230.0 - 40.1, y / 230.0 + 12.6, 2, 0.5),
        );
        (woods.max(0.7 * copse) * open).max(hedge)
    }

    /// Trees by kind: broadleaves, some pines on the outskirts' hills.
    pub(super) fn siege_tree(&self, x: f64, y: f64, hash: u64) -> PropKind {
        let pines = !Self::in_city((x, y))
            && self.forest_kind.fbm(x / 900.0 + 3.0, y / 900.0, 2, 0.5) > 0.2;
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.04 => PropKind::TreeDead,
            (_, pick) if pines && pick < 0.7 => PropKind::TreePine,
            _ => PropKind::TreeBroadleaf,
        }
    }

    /// Whether a point is taken by the plan: a road with its pavement, a
    /// lot, made ground, the wall's glacis. Nothing grows or lies there.
    pub(super) fn siege_built(&self, p: P) -> bool {
        if self.siege_road(p).is_some_and(|(d, s)| d < s.reach() + 2.0) {
            return true;
        }
        if self.siege_lot_near(p, 6.0) {
            return true;
        }
        if !matches!(
            self.siege_area(p),
            None | Some(Ground::Lawn) | Some(Ground::Field)
        ) {
            return true;
        }
        let wall = y_at(WALL, p.0);
        p.1 > wall - 380.0 && p.1 < wall + 60.0
    }

    /// The nearest road to `p` within 64 m, and its distance.
    pub(super) fn siege_road(&self, p: P) -> Option<(f64, Seg)> {
        let index = self.siege.road_index.as_ref()?;
        index
            .near(p, 64.0)
            .map(|i| self.siege.roads[i as usize])
            .map(|s| (s.distance(p), s))
            .filter(|(d, s)| *d < s.reach() + 40.0)
            .min_by(|a, b| (a.0 - a.1.half).total_cmp(&(b.0 - b.1.half)))
    }

    /// Whether a lot's footprint comes within `margin` of `p`.
    pub(super) fn siege_lot_near(&self, p: P, margin: f64) -> bool {
        let Some(index) = self.siege.lot_index.as_ref() else {
            return false;
        };
        index
            .near(p, margin + 8.0)
            .any(|i| self.siege.lots[i as usize].rect.outside(p) < margin)
    }

    /// The made ground at `p`, the last area laid over it winning.
    pub(super) fn siege_area(&self, p: P) -> Option<Ground> {
        let index = self.siege.area_index.as_ref()?;
        index
            .near(p, 0.0)
            .filter(|&i| self.siege.areas[i as usize].rect.outside(p) <= 0.0)
            .max()
            .map(|i| self.siege.areas[i as usize].ground)
    }

    /// The streets layer over one tile, one sample per height sample.
    pub(super) fn tile_streets(&self, x0: f64, y0: f64) -> Vec<u8> {
        let n = crate::TILE_SAMPLES as usize;
        let cell = crate::CELL_SIZE_M as f64;
        let mut out = Vec::with_capacity(n * n * 4);
        for j in 0..n {
            for i in 0..n {
                let p = (x0 + i as f64 * cell, y0 + j as f64 * cell);
                out.extend(self.street_sample(p).bytes());
            }
        }
        out
    }

    fn street_sample(&self, p: P) -> StreetSample {
        let quarters = |m: f64| (m * 4.0).round().clamp(0.0, 255.0) as u8;
        let road = self.siege_road(p);
        let mut ground = self.siege_area(p).unwrap_or(Ground::Natural);
        if let Some((d, s)) = road {
            // City streets run between pavements.
            if s.walk > 0.0 && d < s.half + s.walk {
                ground = Ground::Paving;
            }
        }
        // The glacis, the breaches and the strip behind the wall: shelled,
        // trodden bare earth.
        let wall = y_at(WALL, p.0);
        if ground == Ground::Natural && p.1 < wall + 110.0 && p.1 > wall - 380.0 {
            ground = Ground::Earth;
        }
        // The farms' fields, ploughed or sown, between pasture; not where
        // the woods grow.
        if ground == Ground::Natural && plan::farmland(p) && self.siege_forest(p.0, p.1) < 0.3 {
            let (field, edge, _) = fields::field_at(p);
            if edge > 2.5 && fields::tilled(field) {
                ground = Ground::Field;
            }
        }
        let battered = self.battered(p);
        match road {
            Some((d, s)) => StreetSample {
                // Signed: positive left of the way the road was laid.
                offset: (128.0 + d.copysign(s.side(p)) * 4.0)
                    .round()
                    .clamp(0.0, 255.0) as u8,
                half: quarters(s.half),
                road: s.road,
                junction: self.at_junction(p, &s),
                ground,
                battered,
            },
            None => StreetSample {
                ground,
                battered,
                ..StreetSample::NATURAL
            },
        }
    }

    /// Whether `p`, on or by road `s`, is where it meets or crosses another:
    /// within a few metres of both carriageways.
    fn at_junction(&self, p: P, s: &Seg) -> bool {
        let Some(index) = self.siege.road_index.as_ref() else {
            return false;
        };
        let along = |r: &Seg| {
            let (dx, dy) = (r.b.0 - r.a.0, r.b.1 - r.a.1);
            let len = dx.hypot(dy).max(1e-6);
            (dx / len, dy / len)
        };
        let (ux, uy) = along(s);
        s.distance(p) < s.half + 5.0
            && index.near(p, 64.0).any(|i| {
                let r = &self.siege.roads[i as usize];
                let (vx, vy) = along(r);
                (ux * vy - uy * vx).abs() > 0.3 && r.distance(p) < r.half + 5.0
            })
    }

    /// How shelled the ground at `p` is, 0 to 15: worst at the wall, along
    /// the front either side of it, and round every crater.
    fn battered(&self, p: P) -> u8 {
        let d = p.1 - y_at(WALL, p.0);
        let front = if d > 0.0 {
            1.0 - smoothstep(100.0, 1_400.0, d)
        } else {
            1.0 - smoothstep(200.0, 2_000.0, -d)
        };
        let craters = self.siege.crater_index.as_ref().map_or(0.0, |index| {
            index
                .near(p, 30.0)
                .map(|i| self.siege.craters[i as usize])
                .map(|c| 1.0 - smoothstep(c.radius, 2.6 * c.radius, dist(p, c.at)))
                .fold(0.0, f64::max)
        });
        let grain = 0.5 + 0.5 * self.detail.fbm(p.0 / 90.0 + 3.3, p.1 / 90.0 - 7.1, 2, 0.5);
        ((front * (0.45 + 0.55 * grain)).max(craters) * 15.0).round() as u8
    }

    /// Whether the plan keeps a prop (a tree or a rock grown by the tile)
    /// off this point.
    pub(super) fn siege_clear(&self, x: f64, y: f64) -> bool {
        !self.siege_built((x, y))
    }

    /// The plan's structures and planted trees, as props.
    pub(super) fn siege_props(&self, out: &mut Vec<crate::format::Prop>) {
        use mc_core::{Angle, Fx, FxVec2};
        let fx = |v: f64| Fx((v * 65536.0).round() as i64);
        let angle =
            |a: f64| Angle((a / std::f64::consts::TAU * 65536.0).rem_euclid(65536.0) as u16);
        for lot in &self.siege.lots {
            out.push(crate::format::Prop {
                kind: lot.kind,
                pos: FxVec2::new(fx(lot.at.0), fx(lot.at.1)),
                heading: angle(lot.heading),
                scale_milli: (lot.scale * 1000.0).round() as u16,
                wear_milli: (lot.wear.clamp(0.0, 1.0) * 1000.0).round() as u16,
            });
        }
        for &(p, kind, scale) in &self.siege.planted {
            let h = crate::noise::hash2(self.seed ^ 0x7472_6565, p.0 as i64, p.1 as i64);
            out.push(crate::format::Prop {
                kind,
                pos: FxVec2::new(fx(p.0), fx(p.1)),
                heading: Angle((h >> 8) as u16),
                scale_milli: scale,
                wear_milli: 0,
            });
        }
    }
}

/// Distance from `p` to the nearest base, and whether it is a city base.
fn nearest_base(p: P) -> (f64, bool) {
    CITY_STARTS
        .iter()
        .map(|&s| (dist(p, s), true))
        .chain(OUT_STARTS.iter().map(|&s| (dist(p, s), false)))
        .fold((f64::MAX, false), |a, b| if b.0 < a.0 { b } else { a })
}

/// How far into downtown `p` is: 0 outside its ellipse, 1 at its heart.
fn downtown(p: P) -> f64 {
    let ((cx, cy), rx, ry) = DOWNTOWN;
    let e = ((p.0 - cx) / rx).hypot((p.1 - cy) / ry);
    1.0 - smoothstep(0.0, 1.0, e)
}

/// How far into the old town `p` is: 0 outside, 1 at the square.
fn old_town(p: P) -> f64 {
    bump(dist(p, OLD_TOWN.0) / OLD_TOWN.1)
}

/// Inside a park's ellipse.
fn in_park(p: P, margin: f64) -> bool {
    PARKS
        .iter()
        .any(|&(c, rx, ry)| ((p.0 - c.0) / (rx + margin)).hypot((p.1 - c.1) / (ry + margin)) < 1.0)
}

/// The kind of road a road is, for the plan: `Road` with the city's widths.
fn seg(a: P, b: P, road: Road, half: f64, walk: f64) -> Seg {
    Seg {
        a,
        b,
        half,
        road,
        walk,
    }
}
