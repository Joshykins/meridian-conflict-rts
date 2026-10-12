//! "Thousand Spires": three teams of four in sandstone karst country in
//! autumn, after Wulingyuan: pillars of rock standing out of misty valleys,
//! woods on their tops, the trees red and gold.
//!
//! [`Layout::Spires`](super::Layout::Spires), exactly 24 km. The map is three
//! thirds of a turn about its middle, one team in each: the north third's
//! axis points north, the others are it turned. From the middle out, a third
//! holds:
//!
//! * The Mirror, a lake in the middle shared by all three, shallow, too
//!   shallow for ships, pillars standing out of it, an island in the middle
//!   with the richest ore on the map. Fords run out to the island from each
//!   team's gate.
//! * The Spires, a massif of sandstone cut by ravines into blocks and
//!   columns 400-700 m tall, ringing the lake. Nothing climbs it. One gorge
//!   on the team's axis, the Gate, runs down through it from the team's
//!   tableland to the lake.
//! * The tableland behind the massif, the two inner bases on it; the middle
//!   terrace below it across an escarpment, the two flank bases on it; the
//!   coastal lowland below that, a harbour bay on the axis; the sea.
//! * Between every two teams a valley runs from the lake to the sea along
//!   the line between their thirds: a broad floor walled by the massif's
//!   cliffs and the terraces' escarpments, a river down its middle crossed
//!   at three fords, pillars standing about on its floor, and an estuary
//!   ships can sail up from the sea. The valleys are where teams meet.
//!
//! Fairness is in the play, not the look, as on Vermilion Gorge: every
//! third is laid from one plan (drawn for the north third, mirrored about
//! its axis, turned into the others), so each team has the same benches,
//! bases, ramps, gate, fords and ore in the same order. But the plan's lines
//! are read through a slow warp of the map's own ([`Terrain::sp_warp`],
//! held still round the bases, the ore and the island), and the noise, the
//! cone karst, the massif's outliers, the islands and the pillars are each
//! third's own, so no two thirds look alike. `tests/thousand_spires.rs`
//! holds the baked map's walks to within a few percent between the teams.
//!
//! "Design" points are metres from the map's middle; map points are metres
//! from its south-west corner, y north. The half frame `(x, y)` of a design
//! point is its place in the north third mirrored onto the axis's east side:
//! `x` metres off the axis, `y` along it.

use super::bays::dist;
use super::tripoint::{third, turn};
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::noise::{smoothstep, unit};
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

mod shape;
#[cfg(test)]
mod tests;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 24_576.0;
const MID: f64 = SIZE / 2.0;
/// The sine and cosine of the angle between a third's axis and its edge.
const SIN_EDGE: f64 = 0.866_025_403_784_438_6;
const COS_EDGE: f64 = 0.5;

/// The lowland's foot by the sea, the tableland's height (the lie rises
/// a little past it under the massif) and the lake's shore, metres over the
/// water.
const LOWLAND: f64 = 9.0;
const TABLE: f64 = 108.0;
const SHORE: f64 = 11.0;

/// The coast, metres from the middle: on the axis, and how much farther per
/// radian off it; and the harbour bay on the axis, how far in it cuts and
/// its half width, radians.
const COAST: (f64, f64) = (10_150.0, 320.0);
const HARBOUR: (f64, f64) = (1_350.0, 0.13);
/// The sea's depth off the coast: deep enough for anything that floats.
const DEEP: f64 = 60.0;

/// The massif: its inner edge over the lake's shore and its outer edge
/// over the tableland, metres from the middle; the gorge through it on the
/// axis, half its floor's width.
const MASSIF_IN: f64 = 2_450.0;
const MASSIF_OUT: f64 = 5_150.0;
const GATE_HALF: f64 = 150.0;

/// The valleys: half their width where they leave the lake and where they
/// meet the sea, metres; their floor where they leave the lake, at their
/// highest and where they meet the sea.
const VALLEY_HALF: (f64, f64) = (950.0, 1_400.0);
const VALLEY_FLOOR: (f64, f64, f64) = (SHORE, 21.0, 8.0);
/// The river: half its width, metres; how far below the water its bed lies
/// (too shallow for ships); the fords across it, metres from the middle
/// along the valley, and half their width.
const RIVER_HALF: (f64, f64) = (26.0, 50.0);
const RIVER_BED: f64 = 2.6;
const FORDS: &[f64] = &[2_150.0, 4_050.0, 6_250.0];
const FORD_HALF: f64 = 55.0;
/// The estuary: where along the valley it opens out of the river, and its
/// half width and depth where it meets the sea.
const ESTUARY: (f64, f64, f64) = (7_350.0, 700.0, 18.0);

/// The lake: its radius, its island's radius and top, its depth (shallower
/// than a ship draws), the fords' half width.
const LAKE_R: f64 = 1_650.0;
const ISLAND: (f64, f64) = (430.0, 18.0);
const LAKE_DEEP: f64 = 4.6;
const LAKE_FORD_HALF: f64 = 70.0;

/// The bases, half frame: the inner one up toward the massif, the flank
/// one out toward the valley. Each has its mirror across the axis.
const INNER: (f64, f64) = (1_385.0, 5_990.0);
const FLANK: (f64, f64) = (4_330.0, 6_660.0);

/// Ore away from the bases, half frame: centre, radius. Each is mirrored
/// across the axis (those on it are laid once) and turned into each third.
const ORE: &[(f64, f64, f64)] = &[
    // The tableland, between the inner and flank bases.
    (2_850.0, 5_700.0, 70.0),
    // The terrace, in front of the harbour and behind the flank base.
    (900.0, 7_850.0, 70.0),
    (3_700.0, 7_800.0, 70.0),
    // The lowland: by the harbour and out by the estuary.
    (1_900.0, 9_450.0, 75.0),
    (5_900.0, 7_700.0, 70.0),
    // The valley floor on this side of the river: under the massif, and
    // toward the sea.
    (3_330.0, 2_750.0, 75.0),
    (5_330.0, 3_890.0, 75.0),
    // The lake's shore at the gate's mouth, and toward the valley.
    (0.0, 2_150.0, 80.0),
    (1_010.0, 1_840.0, 70.0),
];
/// The island's field, in the middle.
const ISLAND_ORE: f64 = 150.0;

/// A design vector's place in the north third.
fn north((x, y): (f64, f64)) -> (f64, f64) {
    turn((x, y), (3 - third((x, y))) % 3)
}

/// A design vector's half frame.
fn half(q: (f64, f64)) -> (f64, f64) {
    let (x, y) = north(q);
    (x.abs(), y)
}

/// Metres off the nearest valley's line (the edge between two thirds) and
/// out along it, of a half-frame point.
fn edge_frame((x, y): (f64, f64)) -> (f64, f64) {
    (SIN_EDGE * y - COS_EDGE * x, SIN_EDGE * x + COS_EDGE * y)
}

/// The radians off the axis of a half-frame point.
fn off_axis((x, y): (f64, f64)) -> f64 {
    x.atan2(y)
}

/// A design point on the map, and a map point in the design.
fn on_map((x, y): (f64, f64)) -> (f64, f64) {
    (MID + x, MID + y)
}
fn design((x, y): (f64, f64)) -> (f64, f64) {
    (x - MID, y - MID)
}

/// Half the valley's width, out along it.
fn valley_half(t: f64) -> f64 {
    VALLEY_HALF.0 + (VALLEY_HALF.1 - VALLEY_HALF.0) * smoothstep(3_000.0, 9_500.0, t)
}

/// The valley's floor, out along it.
fn valley_floor(t: f64) -> f64 {
    let (lake, high, sea) = VALLEY_FLOOR;
    lake + (high - lake) * smoothstep(MASSIF_IN - 200.0, 4_200.0, t)
        + (sea - high) * smoothstep(6_000.0, 9_600.0, t)
}

/// Every base, design: the north team's four (its west inner and flank,
/// then its east ones), then the same turned for the next team and the next.
fn bases() -> Vec<(f64, f64)> {
    let north = [
        (-FLANK.0, FLANK.1),
        (-INNER.0, INNER.1),
        (INNER.0, INNER.1),
        (FLANK.0, FLANK.1),
    ];
    (0..3)
        .flat_map(|k| north.iter().map(move |&p| turn(p, k)))
        .collect()
}

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Spires {
    /// The pillars standing on their own (not the massif's blocks), laid in
    /// the north third and filed by cell.
    pillars: shape::Pillars,
}

impl Terrain {
    /// How thickly trees grow: broadleaf woods over nearly everything,
    /// thinning on the lowland by the sea, on the pillars' and the massif's
    /// tops as on the ground. None on the lake's island or the fords.
    pub(super) fn spires_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5);
        let copse = self
            .forest
            .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5);
        let clearing = self
            .forest
            .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let mut habitable = smoothstep(2.0, 5.0, height) * (1.0 - smoothstep(0.45, 0.8, slope));
        let q = design((x, y));
        let r = q.0.hypot(q.1);
        // Open broadleaf woods over most of the land, thinner on the lowland by
        // the sea; thick on the tops of the pillars and the massif, the woods
        // the mist lies under. (Half density in the open woods: the budget of
        // trees covers twice the ground, and half is canopy enough.)
        let lush = smoothstep(14.0, 40.0, height);
        let forest = smoothstep(
            self.forest_edge - 0.3 * lush,
            self.forest_edge + 0.25 - 0.3 * lush,
            broad,
        );
        let open =
            0.5 * (forest * (1.0 - 0.75 * clearing)).max(smoothstep(0.42, 0.62, copse) * 0.7);
        let tops = 0.85 * smoothstep(TABLE + 50.0, TABLE + 120.0, height);
        let density = open.max(tops);
        // The island and the fords stay open.
        if r < ISLAND.0 + 200.0 {
            habitable = 0.0;
        }
        if habitable > 0.0 {
            let ford = self.sp_ford(self.sp_warp(q));
            habitable *= smoothstep(60.0, 120.0, ford);
        }
        // A glade round every ore field.
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        // (`conifer` carries the height: pines take the high tops.)
        (density * habitable, height)
    }

    /// Mostly broadleaf, which the autumn map colours red and gold, with
    /// pines on the high tops (the pillars' and the massif's) and a few
    /// stands of them lower down, birch-pale dead trees here and there.
    pub(super) fn spires_tree(&self, x: f64, y: f64, height: f64, hash: u64) -> PropKind {
        // (Bits of their own: `unit(hash, 48)` would have only 16 left.)
        let (dead, pick) = (unit(hash, 40), unit(crate::noise::hash2(hash, 3, 1), 0));
        let stand = self.forest_kind.get(x / 520.0 + 11.0, y / 520.0 - 5.0);
        let pines = 0.05
            + 0.3 * smoothstep(TABLE + 60.0, TABLE + 220.0, height)
            + 0.15 * smoothstep(0.35, 0.6, stand);
        match () {
            _ if dead < 0.012 => PropKind::TreeDead,
            _ if pick < pines => PropKind::TreePine,
            _ => PropKind::TreeBroadleaf,
        }
    }

    pub(super) fn setup_spires(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = -0.1;
        self.sp = Spires {
            pillars: shape::Pillars::lay(self),
        };

        // The bases: the north team's laid on the build grid, the rest
        // turned from them (off the grid; Tripoint's are the same).
        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let all = bases();
        let north: Vec<(f64, f64)> = all[..4].iter().map(|&p| snap(on_map(p))).collect();
        self.starts = (0..3)
            .flat_map(|k| north.iter().map(move |&p| on_map(turn(design(p), k))))
            .collect();
        self.pads = self
            .starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: start_core,
                outer: 2.0 * start_core,
                height: self.natural(x, y),
            })
            .collect();

        // Three small fields round each base, inside its pad: one behind
        // (away from the middle), two on the forward flanks. Then the rest.
        let mut sites: Vec<((f64, f64), f64)> = Vec::new();
        for &b in &north {
            let back = design(b).1.atan2(design(b).0);
            for turn_by in [0.0, PI - 1.15, PI + 1.15] {
                let (s, c) = (back + turn_by).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((b.0 + c * d, b.1 + s * d), (0.2 * start_core).max(55.0)));
            }
        }
        for &(x, y, r) in ORE {
            sites.push((on_map((x, y)), r));
            if x != 0.0 {
                sites.push((on_map((-x, y)), r));
            }
        }
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = snap(p);
            let f = self.ore_field(p.0, p.1, r);
            let turned: Vec<OreField> = (1..3).map(|k| turned_field(&f, k)).collect();
            fields.push(f);
            fields.extend(turned);
        }
        // The island's field: one third of its outline, turned round.
        let whole = self.ore_field(MID, MID, ISLAND_ORE);
        let share = whole.corners.len() / 3;
        let corners = (0..3)
            .flat_map(|k| {
                whole.corners[..share]
                    .iter()
                    .map(move |&c| on_map(turn(design(c), k)))
            })
            .collect();
        fields.push(OreField { corners, ..whole });
        self.ore = fields;
        self.fit_forests();
    }
}

/// An ore field turned `k` thirds, corner for corner.
fn turned_field(f: &OreField, k: usize) -> OreField {
    let moved = |q: (f64, f64)| on_map(turn(design(q), k));
    let (x, y) = moved((f.x, f.y));
    OreField {
        x,
        y,
        radius: f.radius,
        corners: f.corners.iter().map(|&c| moved(c)).collect(),
    }
}
