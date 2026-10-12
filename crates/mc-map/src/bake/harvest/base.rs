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

/// The paving's west and east edges.
const FLOOR_WEST: f64 = 10_500.0;
const FLOOR_EAST: f64 = 14_100.0;

/// The machine's bastions on the heights either side of the floor, facing in
/// over it: where, which way, at what scale.
const BASTIONS: [((f64, f64), f64, f64); 2] = [
    ((7_950.0, 23_250.0), 0.0, 1.2),
    ((16_750.0, 23_150.0), PI, 1.2),
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
        // The paving: 400 m squares wherever the floor lies level under the
        // whole square.
        let level = |x: f64, y: f64| {
            [
                (-1.0, -1.0),
                (1.0, -1.0),
                (-1.0, 1.0),
                (1.0, 1.0),
                (0.0, 0.0),
            ]
            .iter()
            .all(|&(i, j)| {
                let (px, py) = (x + i * 199.0, y + j * 199.0);
                px < SIZE && py < SIZE && (self.natural(px, py) - BASE_H).abs() < 0.05
            })
        };
        // (A precinct round the processional, the halls and the Heart.)
        let mut y = 18_900.0;
        while y < SIZE {
            let mut x = FLOOR_WEST + 200.0;
            while x < FLOOR_EAST {
                if level(x, y) {
                    put(PropKind::PrecursorFloor, (x, y), 0.0, 1.0);
                }
                x += 400.0;
            }
            y += 400.0;
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
            let level = self.natural(at.0, at.1).round();
            let b = m.bastion(at, heading, scale, level);
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
