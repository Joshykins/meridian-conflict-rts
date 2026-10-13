//! The Progenitor's base on a survival map: one installation laid on a level
//! floor, nothing of it walled off. The Gate stands over the road out, the
//! processional runs in from it past the two print halls (the Forges) to the
//! Heart, the tower ring round the Heart with an aerie at each tower's outer
//! foot, the Great Forge behind it all; viaducts tie the pieces together,
//! paving lies under them, monoliths and needles stand in pairs. On the
//! heights either side, the machine's bastions reach out over the floor, their
//! spans running into the ring.
//!
//! The layout is drawn once, facing south (The Last Harvest's, map metres
//! from its Heart), and laid on a map turned by a quarter turn as the map asks
//! ([`Facing`]): exactly, so a south-facing base is where the numbers say.
//! The base's Shapers' rooms are kept clear of all of it. The forge, the
//! tower and the gate are `precursor_forge.rs`, `precursor_tower.rs` and
//! `precursor_gate.rs` at the scales below.

use super::bays::dist;
use super::machine::{Machine, PrecursorSite};
use super::sites::Site;
use super::Terrain;
use crate::format::PropKind;
use crate::noise::{smoothstep, unit};
use std::f64::consts::{FRAC_PI_2, PI};

/// The way a base faces: down its processional, toward the defenders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Facing {
    South,
    West,
}

/// A base on a map: its Heart, the way it faces, its floor's level and the
/// map's Shaper sites (their rooms are kept clear).
pub(super) struct Facility {
    pub(super) heart: (f64, f64),
    pub(super) facing: Facing,
    pub(super) floor: f64,
    pub(super) sites: &'static [Site],
}

/// The way the layout faces as drawn: south, down the processional.
const SOUTH: f64 = -FRAC_PI_2;

/// The Gate over the road at the floor's front edge, its legs either side of
/// the road (heading: across the road).
const GATE: (f64, f64) = (0.0, -3_694.0);
const GATE_SCALE: f64 = 1.4;
/// The print halls either side of the processional, facing out, at half as
/// big again as the model; the Great Forge at the floor's back at three times.
const FORGES: [(f64, f64); 2] = [(-1_200.0, -1_394.0), (1_200.0, -1_394.0)];
const FORGE_SCALE: f64 = 1.5;
const GREAT_FORGE: (f64, f64) = (0.0, 1_306.0);
const GREAT_SCALE: f64 = 3.0;
/// A hall's bays across it (model y), the printed unit's stand and the
/// projector over it (model x, height), a port in its back for a viaduct,
/// and the Great Forge's piers' ports (`precursor_forge.rs`).
#[cfg(test)]
const FORGE_BAYS: [f64; 4] = [-255.0, -85.0, 85.0, 255.0];
#[cfg(test)]
const FORGE_STAND: f64 = -40.0;
#[cfg(test)]
const FORGE_EMITTER: (f64, f64) = (-115.0, 110.0);
const FORGE_BACK: f64 = -244.0;
const GREAT_PORT: (f64, f64) = (40.0, 170.0);
/// The biggest unit each bay takes (collision radius): the halls', the
/// Great Forge's, the aeries'.
#[cfg(test)]
const FORGE_FITS: f64 = 45.0;
#[cfg(test)]
const GREAT_FITS: f64 = 180.0;
#[cfg(test)]
const AERIE_FITS: f64 = 40.0;

/// The tower ring round the Heart; each has an aerie at its foot on its
/// outer side, where aircraft are printed by a projector 220 m up the tower.
const RING: [(f64, f64); 4] = [
    (-750.0, -694.0),
    (750.0, -694.0),
    (750.0, 706.0),
    (-750.0, 706.0),
];
const AERIE_OUT: f64 = 230.0;
const AERIE_EMITTER: (f64, f64) = (60.0, 220.0);

/// Pairs that float and stand: monoliths over the processional's flanks,
/// needles at the floor's corners.
const MONOLITHS: [((f64, f64), f64); 2] = [((-1_300.0, -2_894.0), 0.0), ((1_300.0, -2_894.0), PI)];
const NEEDLES: [((f64, f64), f64); 4] = [
    ((-2_800.0, -2_744.0), 1.0),
    ((2_800.0, -2_744.0), 1.0),
    ((-2_700.0, 1_856.0), 1.2),
    ((2_700.0, 1_856.0), 1.2),
];

/// The paving's precinct: courts square to the floor's grid (west, east,
/// south, north as drawn), each where the architecture asks for it.
const PRECINCT: [(f64, f64, f64, f64); 6] = [
    // The Gate's threshold.
    (-800.0, 800.0, -3_944.0, -3_344.0),
    // The processional, from the Gate to the Heart's court.
    (-400.0, 400.0, -3_544.0, -744.0),
    // The halls' aprons.
    (-2_000.0, -400.0, -2_144.0, -944.0),
    (400.0, 2_000.0, -2_144.0, -944.0),
    // The Heart's court, inside the tower ring.
    (-1_000.0, 1_000.0, -944.0, 956.0),
    // The Great Forge's apron.
    (-1_200.0, 1_200.0, 856.0, 2_056.0),
];
/// The paving's grid: its first 400 m slab's middle, and how many across and deep.
const GRID: (f64, f64) = (-3_400.0, -4_444.0);
const GRID_CELLS: (usize, usize) = (17, 19);
/// The Heart's dais: half its side, and its tiles' side.
const DAIS: f64 = 520.0;
const DAIS_TILE: f64 = 130.0;
/// The smallest paving tile's side, at the precinct's frayed edge.
const TILE: f64 = 100.0;

/// The machine's bastions at the floor's back corners, facing in over it:
/// where, which way, at what scale.
const BASTIONS: [((f64, f64), f64, f64); 2] = [
    ((-2_900.0, 1_706.0), 0.0, 1.2),
    ((2_850.0, 1_706.0), PI, 1.2),
];

/// The floor's outline as drawn: the base's floor, level at its height, runs
/// off the map behind.
const FLOOR: [(f64, f64); 9] = [
    (-3_100.0, -3_244.0),
    (-1_500.0, -3_594.0),
    (0.0, -3_744.0),
    (1_600.0, -3_594.0),
    (3_000.0, -3_194.0),
    (3_100.0, -1_044.0),
    (3_200.0, 3_156.0),
    (-3_300.0, 3_156.0),
    (-3_250.0, -1_044.0),
];

/// A point in a piece's own frame (x along its heading), on the map.
pub(super) fn world(o: (f64, f64), heading: f64, (lx, ly): (f64, f64)) -> (f64, f64) {
    let (s, c) = heading.sin_cos();
    (o.0 + lx * c - ly * s, o.1 + lx * s + ly * c)
}

/// An aerie's pad, and its projector head (x, y, height).
type Aerie = ((f64, f64), (f64, f64, f64));

impl Facility {
    /// A point of the layout as drawn (metres from the Heart, facing south), on the map.
    pub(super) fn at(&self, (lx, ly): (f64, f64)) -> (f64, f64) {
        let (h0, h1) = self.heart;
        match self.facing {
            Facing::South => (h0 + lx, h1 + ly),
            // (A quarter turn clockwise: the drawn south is the map's west.)
            Facing::West => (h0 + ly, h1 - lx),
        }
    }

    /// A heading as drawn, on the map.
    fn turn(&self, heading: f64) -> f64 {
        match self.facing {
            Facing::South => heading,
            Facing::West => heading - FRAC_PI_2,
        }
    }

    /// The way the printed units face, degrees (0 east, counter-clockwise).
    #[cfg(test)]
    fn facing_deg(&self) -> f64 {
        match self.facing {
            Facing::South => 270.0,
            Facing::West => 180.0,
        }
    }

    /// The floor's outline on the map.
    pub(super) fn floor_outline(&self) -> Vec<(f64, f64)> {
        FLOOR.iter().map(|&p| self.at(p)).collect()
    }

    /// The machine's bastions on the map: where, which way, at what scale.
    pub(super) fn bastions(&self) -> [((f64, f64), f64, f64); 2] {
        BASTIONS.map(|(at, heading, scale)| (self.at(at), self.turn(heading), scale))
    }

    /// The Gate on the map, and its scale.
    pub(super) fn gate(&self) -> ((f64, f64), f64) {
        (self.at(GATE), GATE_SCALE)
    }

    /// The aeries: each pad, and its projector head.
    fn aeries(&self) -> Vec<Aerie> {
        RING.iter()
            .map(|&(x, y)| {
                let out = if x < 0.0 { -1.0 } else { 1.0 };
                let pad = self.at((x + out * AERIE_OUT, y));
                let head = self.at((x + out * AERIE_EMITTER.0, y));
                (pad, (head.0, head.1, AERIE_EMITTER.1))
            })
            .collect()
    }

    /// Metres inside the paving's precinct (negative outside), its courts
    /// ([`PRECINCT`]) taken together.
    fn precinct(&self, p: (f64, f64)) -> f64 {
        PRECINCT
            .iter()
            .map(|&(w, e, s, n)| {
                let (a, b) = (self.at((w, s)), self.at((e, n)));
                let (w, e, s, n) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
                let (dx, dy) = ((w - p.0).max(p.0 - e), (s - p.1).max(p.1 - n));
                if dx > 0.0 || dy > 0.0 {
                    -dx.max(0.0).hypot(dy.max(0.0))
                } else {
                    -dx.max(dy)
                }
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// The viaducts tying the installation together, port to port: the Heart's
    /// spokes to the ring, the halls' backs to the ring, the ring's back
    /// towers to the Great Forge's piers, the Gate's legs to the halls' fronts.
    fn links(&self) -> Vec<((f64, f64), (f64, f64))> {
        let south = self.turn(SOUTH);
        let forge = |k: usize| self.at(FORGES[k]);
        let ring = |k: usize| self.at(RING[k]);
        let back = |o: (f64, f64)| world(o, south, (FORGE_BACK * FORGE_SCALE, 0.0));
        let great = |side: f64| {
            world(
                self.at(GREAT_FORGE),
                south,
                (
                    GREAT_PORT.0 * GREAT_SCALE,
                    side * GREAT_PORT.1 * GREAT_SCALE,
                ),
            )
        };
        let leg = |side: f64| {
            world(
                self.at(GATE),
                self.turn(FRAC_PI_2),
                (0.0, side * 300.0 * GATE_SCALE),
            )
        };
        let front = |o: (f64, f64), side: f64| world(o, south, (0.0, side * 340.0 * FORGE_SCALE));
        let mut out: Vec<_> = (0..4).map(|k| (self.heart, ring(k))).collect();
        out.push((back(forge(0)), ring(0)));
        out.push((back(forge(1)), ring(1)));
        out.push((ring(3), great(-1.0)));
        out.push((ring(2), great(1.0)));
        out.push((leg(1.0), front(forge(0), 1.0)));
        out.push((leg(-1.0), front(forge(1), -1.0)));
        out
    }
}

impl Terrain {
    /// Every piece of the installation on the floor; the machine on the
    /// heights is [`Terrain::facility_machine`].
    pub(super) fn facility_pieces(&self, f: &Facility) -> Vec<PrecursorSite> {
        let mut out = Vec::new();
        let mut put = |kind: PropKind, at: (f64, f64), heading: f64, scale: f64| {
            out.push(PrecursorSite {
                kind,
                x: at.0,
                y: at.1,
                heading,
                scale,
            });
        };
        let in_room =
            |p: (f64, f64), margin: f64| f.sites.iter().any(|s| dist(p, s.at) < s.room() + margin);
        let south = f.turn(SOUTH);
        put(
            PropKind::PrecursorGate,
            f.at(GATE),
            f.turn(FRAC_PI_2),
            GATE_SCALE,
        );
        for &at in &FORGES {
            put(PropKind::PrecursorForge, f.at(at), south, FORGE_SCALE);
        }
        put(
            PropKind::PrecursorForge,
            f.at(GREAT_FORGE),
            south,
            GREAT_SCALE,
        );
        put(PropKind::PrecursorHeart, f.heart, f.turn(0.25 * PI), 1.0);
        put(PropKind::PrecursorHalo, f.heart, f.turn(0.0), 1.25);
        put(PropKind::PrecursorHalo, f.heart, f.turn(FRAC_PI_2), 0.95);
        for &(at, heading) in &MONOLITHS {
            put(PropKind::PrecursorMonolith, f.at(at), f.turn(heading), 1.8);
        }
        for &(at, scale) in &NEEDLES {
            put(
                PropKind::PrecursorNeedle,
                f.at(at),
                f.turn(0.25 * PI),
                scale,
            );
        }
        // The aeries: a lit pad marked by beacons either side, under the tower.
        for &(x, y) in &RING {
            let out = if x < 0.0 { -1.0 } else { 1.0 };
            let pad = (x + out * AERIE_OUT, y);
            for side in [-1.0, 1.0] {
                put(
                    PropKind::PrecursorBeacon,
                    f.at((pad.0, pad.1 + side * 75.0)),
                    f.turn(0.0),
                    1.3,
                );
            }
            put(PropKind::PrecursorConduit, f.at(pad), f.turn(0.0), 1.4);
            put(
                PropKind::PrecursorConduit,
                f.at(pad),
                f.turn(FRAC_PI_2),
                1.4,
            );
        }
        // The processional: conduits from the Gate to the Heart, pylons
        // flanking it, round the Forecourt's room.
        let mut y = GATE.1 + 260.0;
        while y < -320.0 {
            for side in [-1.0, 1.0] {
                let p = f.at((side * 160.0, y));
                if !in_room(p, 60.0) {
                    put(PropKind::PrecursorConduit, p, f.turn(FRAC_PI_2), 2.0);
                }
                let q = f.at((side * 480.0, y));
                if (y - GATE.1) as i64 % 480 < 160 && !in_room(q, 60.0) && y < FORGES[0].1 - 80.0 {
                    put(PropKind::PrecursorPylon, q, f.turn(0.0), 2.4);
                }
            }
            y += 160.0;
        }
        // The viaducts, and a pier under every joint that stands clear.
        let keep_clear = self.facility_keep_clear(f);
        for (a, b) in f.links() {
            let len = dist(a, b);
            let n = (len / 200.0).round().max(1.0);
            let scale = len / (n * 200.0);
            let heading = (b.1 - a.1).atan2(b.0 - a.0);
            let (s, c) = heading.sin_cos();
            for k in 0..n as usize {
                let d = k as f64 * 200.0 * scale;
                let at = (a.0 + c * d, a.1 + s * d);
                put(PropKind::PrecursorViaduct, at, heading, scale);
                if k > 0 && keep_clear.iter().all(|&(p, r)| dist(p, at) > r) {
                    put(PropKind::PrecursorPier, at, heading, scale);
                }
            }
        }
        out.extend(self.facility_paving(f));
        out
    }

    /// The paving, laid at grade in layers: the Heart's dais, a square of
    /// tiles turned to the Heart's diagonal, inside the Heart's court; the
    /// courts round it on the floor's own grid, great 400 m slabs where they
    /// are wide, broken into 200 m slabs and 100 m tiles toward their edges,
    /// a tile missing here and there; and past their edges loose tiles,
    /// fewer further out into the ground, so the precinct wears away into the
    /// country rather than ending on a line. Only where the floor lies level
    /// under the whole piece.
    fn facility_paving(&self, f: &Facility) -> Vec<PrecursorSite> {
        let level = |c: (f64, f64), half: f64, heading: f64| {
            [
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (1.0, 1.0),
                (0.0, 0.0),
            ]
            .iter()
            .all(|&(i, j)| {
                let p = world(c, heading, (i * (half - 1.0), j * (half - 1.0)));
                p.0 > 0.0
                    && p.1 > 0.0
                    && p.0 < self.size_x
                    && p.1 < self.size_y
                    && (self.natural(p.0, p.1) - f.floor).abs() < 0.05
            })
        };
        let mut out = Vec::new();
        let mut slab = |at: (f64, f64), half: f64, heading: f64| {
            out.push(PrecursorSite {
                kind: PropKind::PrecursorFloor,
                x: at.0,
                y: at.1,
                heading,
                scale: half / 200.0,
            })
        };
        // The dais, on its own turned grid.
        let turn = f.turn(0.25 * PI);
        let n = (2.0 * DAIS / DAIS_TILE).round() as usize;
        for j in 0..n {
            for i in 0..n {
                let local = (
                    -DAIS + DAIS_TILE * (i as f64 + 0.5),
                    -DAIS + DAIS_TILE * (j as f64 + 0.5),
                );
                let at = world(f.heart, turn, local);
                if level(at, 0.5 * DAIS_TILE, turn) {
                    slab(at, 0.5 * DAIS_TILE, turn);
                }
            }
        }
        // Whether a square on the floor's grid stays off the dais.
        let off_dais = |c: (f64, f64), half: f64| {
            let (s, co) = turn.sin_cos();
            [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .all(|&(i, j)| {
                    let (dx, dy) = (c.0 + i * half - f.heart.0, c.1 + j * half - f.heart.1);
                    let (u, v) = (dx * co + dy * s, dy * co - dx * s);
                    u.abs().max(v.abs()) > DAIS + 30.0
                })
        };
        let mut stack: Vec<((f64, f64), f64)> = (0..GRID_CELLS.1)
            .flat_map(|j| (0..GRID_CELLS.0).map(move |i| (i, j)))
            .map(|(i, j)| {
                (
                    f.at((GRID.0 + 400.0 * i as f64, GRID.1 + 400.0 * j as f64)),
                    200.0,
                )
            })
            .collect();
        while let Some((c, half)) = stack.pop() {
            let deep = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|&(i, j)| f.precinct((c.0 + i * half, c.1 + j * half)))
                .fold(f64::INFINITY, f64::min);
            let edge = f.precinct(c);
            let tile = half <= 0.5 * TILE + 1.0;
            let dice = unit(
                crate::noise::hash2(self.seed ^ 0x736c_6162, c.0 as i64, c.1 as i64),
                0,
            );
            let fits = || off_dais(c, half) && level(c, half, 0.0);
            if deep >= -1.0 {
                // (A tile gone here and there toward a court's edge.)
                let gap = tile && edge < 300.0 && dice < 0.08;
                if !gap && fits() {
                    slab(c, half, 0.0);
                    continue;
                }
                if tile {
                    continue;
                }
            } else if tile {
                // Out past the edge: a loose slab, smaller, skewed and shifted off
                // the grid, as if the ground had moved under it.
                if dice < 0.4 * smoothstep(-320.0, 0.0, edge).powf(1.5) {
                    let h = crate::noise::hash2(self.seed ^ 0x6c6f_6f73, c.0 as i64, c.1 as i64);
                    let at = (
                        c.0 + 40.0 * (unit(h, 0) - 0.5),
                        c.1 + 40.0 * (unit(h, 24) - 0.5),
                    );
                    let small = half * (0.45 + 0.4 * unit(h, 8));
                    let skew = 0.24 * (unit(h, 32) - 0.5);
                    if off_dais(at, small) && level(at, small, skew) {
                        slab(at, small, skew);
                    }
                }
                continue;
            } else if edge < -(2.0 * half + 340.0) {
                continue;
            }
            let q = 0.5 * half;
            for (i, j) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                stack.push(((c.0 + i * q, c.1 + j * q), q));
            }
        }
        out
    }

    /// Points a viaduct's pier keeps its distance from, and how far: the
    /// buildings, the aeries, the Gate's road, every room.
    fn facility_keep_clear(&self, f: &Facility) -> Vec<((f64, f64), f64)> {
        let south = f.turn(SOUTH);
        let mut out = Vec::new();
        for &o in &FORGES {
            out.push((world(f.at(o), south, (-140.0 * FORGE_SCALE, 0.0)), 560.0));
        }
        out.push((
            world(f.at(GREAT_FORGE), south, (-140.0 * GREAT_SCALE, 0.0)),
            1_150.0,
        ));
        out.push((f.heart, 330.0));
        for &r in &RING {
            out.push((f.at(r), 140.0));
        }
        for (pad, _) in f.aeries() {
            out.push((pad, 110.0));
        }
        out.push((f.at(GATE), 420.0 * GATE_SCALE));
        for s in f.sites {
            out.push((s.at, s.room() + 30.0));
        }
        out
    }

    /// The machine: the tower ring round the Heart with spans between its
    /// towers; the bastions on the heights either side of the floor, booms
    /// reaching in over it, a span from each into the ring.
    pub(super) fn facility_machine(&self, f: &Facility) -> Machine<'_> {
        let mut m = Machine::new(self, 50.0);
        let ring: Vec<_> = RING
            .iter()
            .map(|&at| m.tower(f.at(at), f.turn(0.25 * PI), 1.0, f.floor))
            .collect();
        for i in 0..4 {
            m.link(ring[i], ring[(i + 1) % 4]);
        }
        for (k, &(at, heading, scale)) in f.bastions().iter().enumerate() {
            let b = m.bastion(at, heading, scale, f.floor);
            m.booms(b, heading, 1.5 * scale, scale);
            m.link(b, ring[[3, 2][k]]);
        }
        m
    }

    /// The sidecar's facility block for the survival rules: the Heart and the
    /// print bays.
    #[cfg(test)]
    pub(super) fn facility_bays(&self, f: &Facility) -> String {
        let south = f.turn(SOUTH);
        let facing = f.facing_deg();
        let mut out = String::new();
        out += &format!("engine: ({:.0}, {:.0}),\n", f.heart.0, f.heart.1);
        out += "bays: [\n";
        let hall = |o: (f64, f64), k: f64, fits: f64, out: &mut String| {
            for &by in &FORGE_BAYS {
                let stand = world(o, south, (FORGE_STAND * k, by * k));
                let head = world(o, south, (FORGE_EMITTER.0 * k, by * k));
                *out += &format!(
                    "    (at: ({:.0}, {:.0}), facing: {facing:.0}, emitter: ({:.0}, {:.0}, {:.0}), max_radius: Some({fits:.0})),\n",
                    stand.0, stand.1, head.0, head.1, FORGE_EMITTER.1 * k
                );
            }
        };
        for &o in &FORGES {
            hall(f.at(o), FORGE_SCALE, FORGE_FITS, &mut out);
        }
        hall(f.at(GREAT_FORGE), GREAT_SCALE, GREAT_FITS, &mut out);
        for (pad, head) in f.aeries() {
            out += &format!(
                "    (at: ({:.0}, {:.0}), facing: {facing:.0}, emitter: ({:.0}, {:.0}, {:.0}), domain: Air, max_radius: Some({AERIE_FITS:.0})),\n",
                pad.0, pad.1, head.0, head.1, head.2
            );
        }
        out
    }
}
