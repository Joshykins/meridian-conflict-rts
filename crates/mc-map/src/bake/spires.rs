//! "Thousand Spires": one against one in sandstone karst country in autumn,
//! after Wulingyuan: pillars of rock standing out of misty valleys, woods on
//! their tops, the trees red and gold.
//!
//! [`Layout::Spires`](super::Layout::Spires), exactly 12 km. The map is two
//! halves, one player in each, the south half the north half turned about
//! the middle: the north half's axis points north. From the middle out, a
//! half holds:
//!
//! * The Mirror, a lake in the middle shared by both, too shallow for ships,
//!   pillars standing out of it, an island in the middle with the richest
//!   ore on the map. A ford runs out to the island from each player's gate.
//! * The Spires, a massif of sandstone cut by ravines into blocks and
//!   columns 300-600 m tall, ringing the lake. Nothing climbs it. One gorge
//!   on the axis, the Gate, runs down through it from the player's tableland
//!   to the lake.
//! * The tableland behind the massif with the base on it; the terraces below
//!   it across escarpments; the coastal lowland below those, a harbour bay
//!   on the axis behind the base; the sea.
//! * East and west, between the two halves, a valley runs from the lake to
//!   the sea: a broad floor walled by the massif's cliffs and the terraces'
//!   escarpments, a river down its middle crossed at three fords, pillars
//!   standing about on its floor, and an estuary ships can sail up from the
//!   sea. The valleys are where the players meet.
//!
//! Fairness is in the play, not the look, as on Vermilion Gorge: both halves
//! are laid from one plan (drawn for the north half, mirrored about its
//! axis, turned into the south), so each player has the same benches, ways,
//! gate, fords and ore in the same order. But the plan's lines are read
//! through a slow warp of the map's own ([`Terrain::sp_warp`], held still
//! round the bases, the ore and the island), and the noise, the cone karst,
//! the massif's outliers, the islands and the pillars are each half's own,
//! so the two halves do not look alike. `tests/thousand_spires.rs` holds the
//! baked map's walks to within a few percent between the players.
//!
//! "Design" points are metres from the map's middle; map points are metres
//! from its south-west corner, y north. The half frame `(x, y)` of a design
//! point is its place in the north half mirrored onto the axis's east side:
//! `x` metres off the axis, `y` along it.

use super::bays::dist;
use super::{OreField, Pad, Terrain};
use crate::format::PropKind;
use crate::noise::{smoothstep, unit};
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

mod shape;
#[cfg(test)]
mod tests;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 12_288.0;
const MID: f64 = SIZE / 2.0;

/// The lowland's foot by the sea, the tableland's height (the lie rises
/// a little past it under the massif) and the lake's shore, metres over the
/// water.
const LOWLAND: f64 = 9.0;
const TABLE: f64 = 108.0;
const SHORE: f64 = 11.0;

/// The coast, metres from the middle: on the axis, and how much farther per
/// radian off it; and the harbour bay on the axis, how far in it cuts and
/// its half width, radians.
const COAST: (f64, f64) = (4_900.0, 260.0);
const HARBOUR: (f64, f64) = (650.0, 0.16);
/// The sea's depth off the coast: deep enough for anything that floats.
const DEEP: f64 = 60.0;

/// The massif: its inner edge over the lake's shore and its outer edge
/// over the tableland, metres from the middle; the gorge through it on the
/// axis, half its floor's width.
const MASSIF_IN: f64 = 1_250.0;
const MASSIF_OUT: f64 = 2_500.0;
const GATE_HALF: f64 = 140.0;

/// The valleys: half their width where they leave the lake and where they
/// meet the sea, metres; their floor where they leave the lake, at their
/// highest and where they meet the sea.
const VALLEY_HALF: (f64, f64) = (520.0, 760.0);
const VALLEY_FLOOR: (f64, f64, f64) = (SHORE, 21.0, 8.0);
/// The river: half its width, metres; how far below the water its bed lies
/// (too shallow for ships); the fords across it, metres from the middle
/// along the valley, and half their width.
const RIVER_HALF: (f64, f64) = (24.0, 40.0);
const RIVER_BED: f64 = 2.6;
const FORDS: &[f64] = &[1_200.0, 2_150.0, 3_250.0];
const FORD_HALF: f64 = 55.0;
/// The estuary: where along the valley it opens out of the river, and its
/// half width and depth where it meets the sea.
const ESTUARY: (f64, f64, f64) = (3_850.0, 420.0, 18.0);

/// The lake: its radius, its island's radius and top, its depth (shallower
/// than a ship draws), the fords' half width.
const LAKE_R: f64 = 850.0;
const ISLAND: (f64, f64) = (260.0, 18.0);
const LAKE_DEEP: f64 = 4.6;
const LAKE_FORD_HALF: f64 = 70.0;

/// The base, half frame: on the axis, on the tableland behind the gate.
const BASE: (f64, f64) = (0.0, 3_150.0);

/// Ore away from the base, half frame: centre, radius. Each is mirrored
/// across the axis (those on it are laid once) and turned into the south.
const ORE: &[(f64, f64, f64)] = &[
    // The tableland, either side of the base.
    (1_300.0, 2_900.0, 65.0),
    // The terrace toward the harbour.
    (1_100.0, 4_150.0, 65.0),
    // The lowland out toward the estuary.
    (2_900.0, 3_300.0, 65.0),
    // The valley floor on this side of the river: under the massif, and
    // toward the sea.
    (1_700.0, 300.0, 70.0),
    (2_800.0, 380.0, 70.0),
    // The lake's shore at the gate's mouth.
    (0.0, 1_050.0, 75.0),
];
/// The island's field, in the middle.
const ISLAND_ORE: f64 = 130.0;

/// A design vector turned `k` half turns about the middle.
fn turn((x, y): (f64, f64), k: usize) -> (f64, f64) {
    if k % 2 == 0 {
        (x, y)
    } else {
        (-x, -y)
    }
}

/// Which half a design vector lies in: 0 the north, 1 the south.
fn side((_, y): (f64, f64)) -> usize {
    usize::from(y < 0.0)
}

/// A design vector's place in the north half.
fn north(q: (f64, f64)) -> (f64, f64) {
    turn(q, side(q))
}

/// A design vector's half frame.
fn half(q: (f64, f64)) -> (f64, f64) {
    let (x, y) = north(q);
    (x.abs(), y)
}

/// Metres off the nearest valley's line (the edge between the halves) and
/// out along it, of a half-frame point.
fn edge_frame((x, y): (f64, f64)) -> (f64, f64) {
    (y, x)
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
    VALLEY_HALF.0 + (VALLEY_HALF.1 - VALLEY_HALF.0) * smoothstep(1_500.0, 4_800.0, t)
}

/// The valley's floor, out along it.
fn valley_floor(t: f64) -> f64 {
    let (lake, high, sea) = VALLEY_FLOOR;
    lake + (high - lake) * smoothstep(MASSIF_IN - 150.0, 2_200.0, t)
        + (sea - high) * smoothstep(3_100.0, 4_900.0, t)
}

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Spires {
    /// The pillars standing on their own (not the massif's blocks), filed
    /// by cell.
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
        let mut habitable = smoothstep(2.0, 5.0, height) * (1.0 - smoothstep(0.7, 0.95, slope));
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
        if r < ISLAND.0 + 150.0 {
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
        self.erode_spires();
        self.sp = Spires {
            pillars: shape::Pillars::lay(self),
        };

        // The bases: the north one laid on the build grid, the south one
        // turned from it.
        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let north = snap(on_map(BASE));
        self.starts = (0..2).map(|k| on_map(turn(design(north), k))).collect();
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

        // Three small fields round the base, inside its pad: one behind
        // (away from the middle), two on the forward flanks. Then the rest.
        let mut sites: Vec<((f64, f64), f64)> = Vec::new();
        let back = design(north).1.atan2(design(north).0);
        for turn_by in [0.0, PI - 1.15, PI + 1.15] {
            let (s, c) = (back + turn_by).sin_cos();
            let d = 0.8 * start_core;
            sites.push((
                (north.0 + c * d, north.1 + s * d),
                (0.2 * start_core).max(55.0),
            ));
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
            let turned = turned_field(&f, 1);
            fields.push(f);
            fields.push(turned);
        }
        // The island's field: half its outline, turned round.
        let whole = self.ore_field(MID, MID, ISLAND_ORE);
        let share = whole.corners.len() / 2;
        let corners = (0..2)
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

/// An ore field turned `k` half turns, corner for corner.
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
