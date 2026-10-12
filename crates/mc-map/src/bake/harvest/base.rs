//! The Progenitor's base at the valley's head: one installation laid on the
//! level floor at [`BASE_H`], nothing of it walled off. The Gate stands over
//! the valley's road, the processional runs north from it past the two print
//! halls (the Forges) to the Heart, the tower ring round the Heart with an
//! aerie at each tower's outer foot, the Great Forge behind it all; viaducts
//! tie the pieces together, paving lies under them, monoliths and needles
//! stand in pairs. On the heights either side, the machine's bastions reach
//! out over the floor, their spans running into the ring.
//!
//! The three base Shapers' rooms ([`SITES`]) are kept clear of all of it.
//! The forge, the tower and the gate are `precursor_forge.rs`,
//! `precursor_tower.rs` and `precursor_gate.rs` at the scales below.

use super::*;
use crate::bake::machine::{Machine, PrecursorSite};
use std::f64::consts::{FRAC_PI_2, PI};

/// The way the base faces: south, down the valley.
const SOUTH: f64 = -FRAC_PI_2;

/// The Gate over the valley's road at the floor's south edge, its legs
/// either side of the road (heading: across the road).
const GATE: (f64, f64) = (12_300.0, 18_350.0);
const GATE_SCALE: f64 = 1.4;
/// The print halls either side of the processional, facing south, at
/// half as big again as the model; the Great Forge at the floor's north end
/// at three times.
const FORGES: [(f64, f64); 2] = [(11_100.0, 20_650.0), (13_500.0, 20_650.0)];
const FORGE_SCALE: f64 = 1.5;
const GREAT_FORGE: (f64, f64) = (12_300.0, 23_350.0);
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
    (11_550.0, 21_350.0),
    (13_050.0, 21_350.0),
    (13_050.0, 22_750.0),
    (11_550.0, 22_750.0),
];
const AERIE_OUT: f64 = 230.0;
const AERIE_EMITTER: (f64, f64) = (60.0, 220.0);

/// Pairs that float and stand: monoliths over the processional's flanks,
/// needles at the floor's corners.
const MONOLITHS: [((f64, f64), f64); 2] = [((11_000.0, 19_150.0), 0.0), ((13_600.0, 19_150.0), PI)];
const NEEDLES: [((f64, f64), f64); 4] = [
    ((9_500.0, 19_300.0), 1.0),
    ((15_100.0, 19_300.0), 1.0),
    ((9_600.0, 23_900.0), 1.2),
    ((15_000.0, 23_900.0), 1.2),
];

/// The paving's precinct: courts square to the floor's grid (west, east,
/// south, north), each where the architecture asks for it.
const PRECINCT: [(f64, f64, f64, f64); 6] = [
    // The Gate's threshold.
    (11_500.0, 13_100.0, 18_100.0, 18_700.0),
    // The processional, from the Gate to the Heart's court.
    (11_900.0, 12_700.0, 18_500.0, 21_300.0),
    // The halls' aprons.
    (10_300.0, 11_900.0, 19_900.0, 21_100.0),
    (12_700.0, 14_300.0, 19_900.0, 21_100.0),
    // The Heart's court, inside the tower ring.
    (11_300.0, 13_300.0, 21_100.0, 23_000.0),
    // The Great Forge's apron.
    (11_100.0, 13_500.0, 22_900.0, 24_100.0),
];
/// The Heart's dais: half its side, and its tiles' side.
const DAIS: f64 = 520.0;
const DAIS_TILE: f64 = 130.0;
/// The smallest paving tile's side, at the precinct's frayed edge.
const TILE: f64 = 100.0;

/// The machine's bastions at the floor's north corners, facing in over it:
/// where, which way, at what scale.
pub(super) const BASTIONS: [((f64, f64), f64, f64); 2] = [
    ((9_400.0, 23_750.0), 0.0, 1.2),
    ((15_150.0, 23_750.0), PI, 1.2),
];

/// A point in a piece's own frame (x along its heading), on the map.
fn world(o: (f64, f64), heading: f64, (lx, ly): (f64, f64)) -> (f64, f64) {
    let (s, c) = heading.sin_cos();
    (o.0 + lx * c - ly * s, o.1 + lx * s + ly * c)
}

/// An aerie's pad, and its projector head (x, y, height).
type Aerie = ((f64, f64), (f64, f64, f64));

/// The aeries: each pad, and its projector head.
fn aeries() -> Vec<Aerie> {
    RING.iter()
        .map(|&(x, y)| {
            let out = if x < HEART.0 { -1.0 } else { 1.0 };
            (
                (x + out * AERIE_OUT, y),
                (x + out * AERIE_EMITTER.0, y, AERIE_EMITTER.1),
            )
        })
        .collect()
}

impl Terrain {
    /// Every piece of the installation on the floor; the machine on the
    /// heights is `machine_harvest`.
    pub(super) fn lay_facility_harvest(&self) -> Vec<PrecursorSite> {
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
            |p: (f64, f64), margin: f64| SITES.iter().any(|s| dist(p, s.at) < s.room() + margin);
        put(PropKind::PrecursorGate, GATE, FRAC_PI_2, GATE_SCALE);
        for &at in &FORGES {
            put(PropKind::PrecursorForge, at, SOUTH, FORGE_SCALE);
        }
        put(PropKind::PrecursorForge, GREAT_FORGE, SOUTH, GREAT_SCALE);
        put(PropKind::PrecursorHeart, HEART, 0.25 * PI, 1.0);
        put(PropKind::PrecursorHalo, HEART, 0.0, 1.25);
        put(PropKind::PrecursorHalo, HEART, FRAC_PI_2, 0.95);
        for &(at, heading) in &MONOLITHS {
            put(PropKind::PrecursorMonolith, at, heading, 1.8);
        }
        for &(at, scale) in &NEEDLES {
            put(PropKind::PrecursorNeedle, at, 0.25 * PI, scale);
        }
        // The aeries: a lit pad marked by beacons either side, under the tower.
        for (pad, _) in aeries() {
            for side in [-1.0, 1.0] {
                put(
                    PropKind::PrecursorBeacon,
                    (pad.0, pad.1 + side * 75.0),
                    0.0,
                    1.3,
                );
            }
            put(PropKind::PrecursorConduit, pad, 0.0, 1.4);
            put(PropKind::PrecursorConduit, pad, FRAC_PI_2, 1.4);
        }
        // The processional: conduits from the Gate to the Heart, pylons
        // flanking it, round the Forecourt's room.
        let mut y = GATE.1 + 260.0;
        while y < HEART.1 - 320.0 {
            for side in [-1.0, 1.0] {
                let p = (HEART.0 + side * 160.0, y);
                if !in_room(p, 60.0) {
                    put(PropKind::PrecursorConduit, p, FRAC_PI_2, 2.0);
                }
                let q = (HEART.0 + side * 480.0, y);
                if (y - GATE.1) as i64 % 480 < 160 && !in_room(q, 60.0) && y < FORGES[0].1 - 80.0 {
                    put(PropKind::PrecursorPylon, q, 0.0, 2.4);
                }
            }
            y += 160.0;
        }
        // The viaducts, and a pier under every joint that stands clear.
        let keep_clear = self.hv_keep_clear();
        for (a, b) in links() {
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
        out.extend(self.hv_paving());
        out
    }

    /// Metres inside the paving's precinct (negative outside), its courts
    /// ([`PRECINCT`]) taken together.
    fn hv_precinct(p: (f64, f64)) -> f64 {
        PRECINCT
            .iter()
            .map(|&(w, e, s, n)| {
                let (dx, dy) = ((w - p.0).max(p.0 - e), (s - p.1).max(p.1 - n));
                if dx > 0.0 || dy > 0.0 {
                    -dx.max(0.0).hypot(dy.max(0.0))
                } else {
                    -dx.max(dy)
                }
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// The paving, laid at grade in layers: the Heart's dais, a square of
    /// tiles turned to the Heart's diagonal, inside the Heart's court; the
    /// courts round it on the floor's own grid, great 400 m slabs where they
    /// are wide, broken into 200 m slabs and 100 m tiles toward their edges,
    /// a tile missing here and there; and past their edges loose tiles,
    /// fewer further out into the grass, so the precinct wears away into the
    /// country rather than ending on a line. Only where the floor lies level
    /// under the whole piece.
    fn hv_paving(&self) -> Vec<PrecursorSite> {
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
                    && p.0 < SIZE
                    && p.1 < SIZE
                    && (self.natural(p.0, p.1) - BASE_H).abs() < 0.05
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
        let turn = 0.25 * PI;
        let n = (2.0 * DAIS / DAIS_TILE).round() as usize;
        for j in 0..n {
            for i in 0..n {
                let local = (
                    -DAIS + DAIS_TILE * (i as f64 + 0.5),
                    -DAIS + DAIS_TILE * (j as f64 + 0.5),
                );
                let at = world(HEART, turn, local);
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
                    let (dx, dy) = (c.0 + i * half - HEART.0, c.1 + j * half - HEART.1);
                    let (u, v) = (dx * co + dy * s, dy * co - dx * s);
                    u.abs().max(v.abs()) > DAIS + 30.0
                })
        };
        let (x0, y0) = (HEART.0 - 200.0 - 400.0 * 8.0, 17_600.0);
        let mut stack: Vec<((f64, f64), f64)> = (0..19)
            .flat_map(|j| (0..17).map(move |i| (i, j)))
            .map(|(i, j)| ((x0 + 400.0 * i as f64, y0 + 400.0 * j as f64), 200.0))
            .collect();
        while let Some((c, half)) = stack.pop() {
            let deep = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|&(i, j)| Self::hv_precinct((c.0 + i * half, c.1 + j * half)))
                .fold(f64::INFINITY, f64::min);
            let edge = Self::hv_precinct(c);
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
    fn hv_keep_clear(&self) -> Vec<((f64, f64), f64)> {
        let mut out = Vec::new();
        for &o in &FORGES {
            out.push((world(o, SOUTH, (-140.0 * FORGE_SCALE, 0.0)), 560.0));
        }
        out.push((
            world(GREAT_FORGE, SOUTH, (-140.0 * GREAT_SCALE, 0.0)),
            1_150.0,
        ));
        out.push((HEART, 330.0));
        for &r in &RING {
            out.push((r, 140.0));
        }
        for (pad, _) in aeries() {
            out.push((pad, 110.0));
        }
        out.push((GATE, 420.0 * GATE_SCALE));
        for s in SITES {
            out.push((s.at, s.room() + 30.0));
        }
        out
    }

    /// The machine: the tower ring round the Heart with spans between its
    /// towers; the bastions on the heights either side of the floor, booms
    /// reaching in over it, a span from each into the ring.
    pub(in crate::bake) fn machine_harvest(&self) -> Machine<'_> {
        let mut m = Machine::new(self, 50.0);
        let ring: Vec<_> = RING
            .iter()
            .map(|&at| m.tower(at, 0.25 * PI, 1.0, BASE_H))
            .collect();
        for i in 0..4 {
            m.link(ring[i], ring[(i + 1) % 4]);
        }
        for (k, &(at, heading, scale)) in BASTIONS.iter().enumerate() {
            let b = m.bastion(at, heading, scale, BASE_H);
            m.booms(b, heading, 1.5 * scale, scale);
            m.link(b, ring[[3, 2][k]]);
        }
        m
    }

    /// The sidecar's facility block for the survival rules: the Heart, the
    /// print bays, the sites.
    #[cfg(test)]
    pub(super) fn harvest_block(&self) -> String {
        let mut out = String::new();
        out += &format!("engine: ({:.0}, {:.0}),\n", HEART.0, HEART.1);
        out += "bays: [\n";
        let hall = |o: (f64, f64), k: f64, fits: f64, out: &mut String| {
            for &by in &FORGE_BAYS {
                let stand = world(o, SOUTH, (FORGE_STAND * k, by * k));
                let head = world(o, SOUTH, (FORGE_EMITTER.0 * k, by * k));
                *out += &format!(
                    "    (at: ({:.0}, {:.0}), facing: 270, emitter: ({:.0}, {:.0}, {:.0}), max_radius: Some({fits:.0})),\n",
                    stand.0, stand.1, head.0, head.1, FORGE_EMITTER.1 * k
                );
            }
        };
        for &o in &FORGES {
            hall(o, FORGE_SCALE, FORGE_FITS, &mut out);
        }
        hall(GREAT_FORGE, GREAT_SCALE, GREAT_FITS, &mut out);
        for (pad, head) in aeries() {
            out += &format!(
                "    (at: ({:.0}, {:.0}), facing: 270, emitter: ({:.0}, {:.0}, {:.0}), domain: Air, max_radius: Some({AERIE_FITS:.0})),\n",
                pad.0, pad.1, head.0, head.1, head.2
            );
        }
        out += "],\nnode_sites: [\n";
        for (s, way) in SITES.iter().zip(&self.hv.plan.ways) {
            let path = if s.kind == Kind::Printer {
                // From the room's edge on, every 400 m or so.
                way.points_from(s.room() + 10.0, 400.0)
            } else {
                Vec::new()
            };
            out += &site_line(s, &path);
        }
        out += "],\n";
        out
    }
}

/// The viaducts tying the installation together, port to port: the Heart's
/// spokes to the ring, the halls' backs to the ring, the ring's north
/// towers to the Great Forge's piers, the Gate's legs to the halls' fronts.
fn links() -> Vec<((f64, f64), (f64, f64))> {
    let back = |o: (f64, f64)| world(o, SOUTH, (FORGE_BACK * FORGE_SCALE, 0.0));
    let great = |side: f64| {
        world(
            GREAT_FORGE,
            SOUTH,
            (
                GREAT_PORT.0 * GREAT_SCALE,
                side * GREAT_PORT.1 * GREAT_SCALE,
            ),
        )
    };
    let leg = |side: f64| world(GATE, FRAC_PI_2, (0.0, side * 300.0 * GATE_SCALE));
    let front = |o: (f64, f64), side: f64| world(o, SOUTH, (0.0, side * 340.0 * FORGE_SCALE));
    let mut out: Vec<_> = RING.iter().map(|&r| (HEART, r)).collect();
    out.push((back(FORGES[0]), RING[0]));
    out.push((back(FORGES[1]), RING[1]));
    out.push((RING[3], great(-1.0)));
    out.push((RING[2], great(1.0)));
    out.push((leg(1.0), front(FORGES[0], 1.0)));
    out.push((leg(-1.0), front(FORGES[1], -1.0)));
    out
}
