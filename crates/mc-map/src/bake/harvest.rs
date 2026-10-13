//! "The Last Harvest": the survival map for four defenders, 24 km.
//!
//! [`Layout::Harvest`](super::Layout::Harvest). The defenders hold the south
//! of an autumn heartland in the middle of the map, their four starts in a
//! row above the beaches of the south sea, a river across the heartland north
//! of them that the ways ford. A broad valley runs north from the
//! heartland to the Progenitor's base, which stands at grade at its head:
//! no rampart, no plateau, no ramps, the facility's floor laid level across
//! the valley's end so what it prints walks straight out. Round the
//! heartland, each in its own climate (the sidecar's `regions`, parted by the
//! walls in [`HARVEST_WALLS`](crate::landmark::HARVEST_WALLS)):
//!
//! * north-west, the desert: sunken flats among mesas and buttes, ringed on
//!   the heartland's side by a rim of mesa walls that only canyon passes cut;
//! * north-east, alpine country: mountains with high basins, snow on the
//!   heights, a valley down from each basin;
//! * south-west, the isles: a tropical archipelago, a great island joined to
//!   the heartland's shore by a sandbar;
//! * south-east, the fjords: polar plateau and mountains cut by deep fjords
//!   that open into the south sea, the sound between them and the heartland.
//!
//! The south sea runs along the whole bottom edge, so ships come at the
//! defenders from behind as well as down both flanks.
//!
//! Shapers rise at the sites ([`SITES`]): printers in every outer region and
//! three in the base, each on a level, clear room with a way down to the
//! heartland the bake cuts for it (a road, a canyon pass, a valley, a sandbar;
//! for a naval site a sea lane); and behind them, hidden, the auxiliary
//! Shapers (artillery and nukes): in a mesa-ringed hollow, a cirque, a basin
//! up a pass, on an atoll. Reachable, but only by a narrow way.
//!
//! The map is natural, not symmetric: the defenders are allies, and their
//! starts are held level and alike only within tolerances
//! (`tests/last_harvest.rs`). Map metres throughout, x east, y north, from
//! the south-west corner; the sidecar `maps/last_harvest.ron` carries the same
//! numbers (`harvest_block` prints its survival block).

use super::bays::{dist, inside};
use super::canyon::{smooth_closed, smooth_open};
use super::{Pad, Terrain};
use crate::format::PropKind;
use crate::landmark::{
    harvest_depth, harvest_look, harvest_region, HARVEST_ALPINE, HARVEST_DESERT, HARVEST_FJORDS,
    HARVEST_ISLES,
};
use crate::noise::{smoothstep, unit};
use crate::BUILD_CELL_M;

mod base;
mod shape;
#[cfg(test)]
mod tests;
mod wrecks;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 24_576.0;

/// The base's floor, metres over the water: the valley rises to it.
const BASE_H: f64 = 62.0;

/// What a site raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Prints units onto platforms in front of it.
    Printer,
    /// Raises an auxiliary Shaper (whichever kind the sim picks): hidden behind
    /// the printers.
    Auxiliary,
    /// Kept for the Gatewright, whose fleets come by air: always raises one once its
    /// tier comes.
    Fleet,
}

/// Where a site's Shaper stands: on dry ground, or on an islet or shore pad
/// with its slips in deep water in front of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ground {
    Land,
    Naval,
}

/// A Shaper site.
struct Site {
    name: &'static str,
    at: (f64, f64),
    kind: Kind,
    ground: Ground,
    /// The room's level (land), or the Shaper's pad's (naval), metres over the water.
    level: f64,
    /// A printer's way toward the defenders, after `at`; an auxiliary site's
    /// narrow way in, from `at` out to ground the rest of the map reaches.
    way: &'static [(f64, f64)],
    /// How the way's walls stand where it cuts into higher ground: metres out
    /// per metre up (sheer canyon walls in the desert, valley sides in the
    /// mountains).
    cut: f64,
}

/// The room a printing site's base grows into, and an auxiliary one's, metres.
const PRINTER_ROOM: f64 = 450.0;
const AUX_ROOM: f64 = 350.0;
/// Half a printing site's way's width, and an auxiliary site's narrow way's;
/// half a sea lane's.
const WAY_HALF: f64 = 130.0;
const NARROW_HALF: f64 = 36.0;
const LANE_HALF: f64 = 160.0;
/// How deep a sea lane and a naval site's slips are kept, metres.
const LANE_DEEP: f64 = 18.0;
/// The steepest a way climbs, rise over run.
const GRADE: f64 = 0.09;

impl Site {
    fn room(&self) -> f64 {
        match self.kind {
            Kind::Printer => PRINTER_ROOM,
            _ => AUX_ROOM,
        }
    }

    fn half(&self) -> f64 {
        match (self.kind, self.ground) {
            (Kind::Printer, Ground::Land) => WAY_HALF,
            (Kind::Printer, Ground::Naval) => LANE_HALF,
            _ => NARROW_HALF,
        }
    }
}

/// Every site: the base's three, then each region's printers and its
/// auxiliary Shaper behind them.
const SITES: &[Site] = &[
    // The base: the forecourt inside the Gate, the courts either side of the Heart.
    Site {
        name: "The Forecourt",
        at: (12_300.0, 19_750.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (12_300.0, 18_900.0),
            (12_400.0, 17_000.0),
            (12_150.0, 15_000.0),
            (12_400.0, 13_000.0),
            (12_250.0, 11_000.0),
            (12_350.0, 9_000.0),
            (12_300.0, 7_400.0),
            (12_250.0, 6_500.0),
        ],
        cut: 2.0,
    },
    Site {
        name: "West Court",
        at: (9_950.0, 21_300.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (10_350.0, 19_900.0),
            (10_900.0, 18_300.0),
            (11_400.0, 16_400.0),
            (11_650.0, 14_000.0),
            (11_500.0, 11_500.0),
            (11_000.0, 9_000.0),
            (10_500.0, 7_200.0),
        ],
        cut: 2.0,
    },
    Site {
        name: "East Court",
        at: (14_700.0, 21_450.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (14_300.0, 19_900.0),
            (13_800.0, 18_200.0),
            (13_350.0, 16_300.0),
            (13_050.0, 14_000.0),
            (13_150.0, 11_500.0),
            (13_550.0, 9_000.0),
            (14_000.0, 7_300.0),
        ],
        cut: 2.0,
    },
    // The desert: two on its flats, their ways through passes in the rim;
    // the artillery in a hollow ringed by mesas, up a slot canyon.
    Site {
        name: "Red Anvil",
        at: (5_200.0, 15_300.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 86.0,
        way: &[
            (5_950.0, 14_700.0),
            (6_300.0, 14_000.0),
            (7_000.0, 13_650.0),
            (7_400.0, 12_900.0),
            (7_950.0, 12_350.0),
            (8_400.0, 11_500.0),
            (9_100.0, 10_200.0),
            (9_800.0, 8_600.0),
            (10_300.0, 7_000.0),
        ],
        cut: 0.4,
    },
    Site {
        name: "The Glasspan",
        at: (4_300.0, 20_100.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 94.0,
        way: &[
            (5_300.0, 19_700.0),
            (6_100.0, 19_000.0),
            (6_900.0, 18_900.0),
            (7_500.0, 18_150.0),
            (8_200.0, 17_700.0),
            (8_800.0, 17_000.0),
            (9_800.0, 16_300.0),
            (10_800.0, 15_100.0),
            (11_250.0, 13_000.0),
            (11_350.0, 11_000.0),
            (11_150.0, 9_000.0),
            (11_000.0, 7_300.0),
        ],
        cut: 0.4,
    },
    Site {
        name: "The Hollow",
        at: (1_800.0, 22_800.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 118.0,
        way: &[
            (2_550.0, 21_900.0),
            (3_250.0, 21_050.0),
            (4_050.0, 20_250.0),
        ],
        cut: 0.3,
    },
    // The alpine country: two high basins, a valley down from each; the
    // auxiliary site in a cirque above the higher, over a pass.
    Site {
        name: "Glass Tarn",
        at: (18_900.0, 15_300.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 300.0,
        way: &[
            (18_200.0, 14_650.0),
            (17_900.0, 13_900.0),
            (17_300.0, 13_450.0),
            (17_050.0, 12_700.0),
            (16_500.0, 12_100.0),
            (16_100.0, 11_050.0),
            (15_300.0, 9_700.0),
            (14_700.0, 8_200.0),
            (14_300.0, 7_000.0),
        ],
        cut: 1.2,
    },
    Site {
        name: "Hornfold",
        at: (21_200.0, 19_700.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 410.0,
        way: &[
            (20_300.0, 19_300.0),
            (19_600.0, 18_650.0),
            (18_800.0, 18_450.0),
            (18_100.0, 17_800.0),
            (17_300.0, 17_650.0),
            (16_600.0, 17_050.0),
            (15_600.0, 16_600.0),
            (14_500.0, 15_500.0),
            (13_750.0, 13_600.0),
            (13_500.0, 11_200.0),
            (13_750.0, 9_200.0),
            (14_150.0, 7_600.0),
        ],
        cut: 1.2,
    },
    Site {
        name: "Crown Cirque",
        at: (22_800.0, 22_850.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 600.0,
        way: &[
            (22_500.0, 22_200.0),
            (22_350.0, 21_650.0),
            (21_850.0, 21_250.0),
            (21_750.0, 20_650.0),
            (21_350.0, 20_000.0),
        ],
        cut: 0.8,
    },
    // The isles: one on the great island, its way over the sandbar; one
    // on an islet in the sound south of it, its slips toward the south sea;
    // the artillery on an atoll out by the map's edge, its ring of hills
    // opening by one notch to the west.
    Site {
        name: "Ember Isle",
        at: (3_500.0, 8_350.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 24.0,
        way: &[
            (4_450.0, 8_500.0),
            (5_350.0, 8_600.0),
            (6_250.0, 8_650.0),
            (7_300.0, 8_300.0),
            (8_500.0, 7_500.0),
            (9_300.0, 6_500.0),
        ],
        cut: 1.4,
    },
    Site {
        name: "Coral Sound",
        at: (2_600.0, 4_400.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 6.0,
        way: &[
            (3_300.0, 3_700.0),
            (4_300.0, 2_600.0),
            (5_600.0, 1_600.0),
            (7_400.0, 1_100.0),
            (9_200.0, 1_100.0),
            (10_600.0, 1_300.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Far Atoll",
        at: (1_250.0, 10_000.0),
        kind: Kind::Fleet,
        ground: Ground::Land,
        level: 18.0,
        way: &[(650.0, 10_050.0), (330.0, 10_080.0)],
        cut: 0.6,
    },
    // The fjords: one at the head of the long fjord, its slips down it; one
    // on the plateau past the sound's head; the nukes in a basin ringed by
    // mountains, over a pass from the plateau.
    Site {
        name: "Fjordhead",
        at: (21_300.0, 7_350.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 8.0,
        way: &[
            (21_300.0, 6_600.0),
            (21_450.0, 5_200.0),
            (21_300.0, 3_600.0),
            (20_900.0, 2_100.0),
            (19_800.0, 1_100.0),
            (17_500.0, 900.0),
            (15_200.0, 1_100.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Rime Shelf",
        at: (21_600.0, 9_750.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 92.0,
        way: &[
            (20_800.0, 10_050.0),
            (20_100.0, 9_700.0),
            (19_450.0, 9_900.0),
            (18_700.0, 9_350.0),
            (17_700.0, 8_700.0),
            (16_700.0, 7_700.0),
            (15_700.0, 6_600.0),
            (15_200.0, 5_900.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Frostvault",
        at: (23_500.0, 7_600.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 170.0,
        way: &[
            (23_000.0, 8_300.0),
            (22_500.0, 9_000.0),
            (21_800.0, 9_650.0),
        ],
        cut: 0.8,
    },
];

/// The defenders' starts, west to east, and what holding each is like.
const SPAWNS: [((f64, f64), &str, &str); 4] = [
    (
        (9_800.0, 4_550.0),
        "Orchard Hill",
        "Westmost, nearest the isles: the sandbar's columns and the Coral Sound's ships come at it first.",
    ),
    (
        (11_450.0, 3_850.0),
        "The Granary",
        "On the shore in the middle: the valley's columns reach it straight, the south sea's ships behind it.",
    ),
    (
        (13_200.0, 4_300.0),
        "Millrace",
        "Middle east, under the valley's mouth: the alpine valleys and the base's columns meet over it.",
    ),
    (
        (14_950.0, 4_700.0),
        "Stubble Point",
        "Eastmost, by the sound: the fjords' columns and ships come at it first.",
    ),
];
/// The Heart: the Progenitor's start, where its ray leaves.
const HEART: (f64, f64) = (12_300.0, 22_044.0);

/// Ore away from the starts: centre and radius. Across the heartland and the
/// valley, and on the ways out of each region.
const ORE: &[(f64, f64, f64)] = &[
    // The heartland's south, round and between the starts.
    (7_700.0, 3_700.0, 110.0),
    (12_300.0, 2_950.0, 100.0),
    (16_700.0, 3_500.0, 110.0),
    (8_100.0, 6_000.0, 130.0),
    (10_600.0, 6_050.0, 130.0),
    (12_350.0, 6_100.0, 140.0),
    (13_950.0, 6_150.0, 130.0),
    (15_950.0, 5_700.0, 120.0),
    // The heartland's middle.
    (9_000.0, 8_300.0, 120.0),
    (16_800.0, 8_200.0, 120.0),
    (10_300.0, 10_300.0, 130.0),
    (14_000.0, 10_750.0, 130.0),
    (7_500.0, 10_250.0, 110.0),
    (16_900.0, 10_300.0, 110.0),
    // Up the valley.
    (12_300.0, 12_100.0, 140.0),
    (10_950.0, 14_550.0, 120.0),
    (13_750.0, 14_650.0, 120.0),
    (12_300.0, 16_750.0, 130.0),
    // The regions, on their ways out.
    (6_300.0, 16_600.0, 100.0),
    (17_350.0, 13_300.0, 90.0),
    (19_750.0, 9_600.0, 100.0),
    (4_700.0, 8_250.0, 90.0),
];

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Harvest {
    /// The ground's plan (`shape.rs`).
    plan: shape::Plan,
}

impl Terrain {
    /// The region whose look a point takes: by the walls (`landmark::HARVEST_*`),
    /// read where the renderer reads them for the ground's colour (`harvest_look`).
    pub(super) fn hv_region(&self, p: (f64, f64)) -> usize {
        harvest_region(harvest_look(p))
    }

    pub(super) fn setup_harvest(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = -0.1;
        // The ground's outlines first; the erosion is run over them before
        // the ways, rooms and the base's floor are cut, which it never touches.
        self.hv.plan = shape::Plan::outlines();
        self.hv_lay_hills();
        self.erode_alpine();
        self.hv_profiles();

        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        self.starts = SPAWNS.iter().map(|s| snap(s.0)).collect();
        self.pads = self
            .starts
            .iter()
            .map(|&(x, y)| Pad {
                x,
                y,
                core: start_core,
                outer: 2.0 * start_core,
                height: self.natural(x, y).max(12.0),
            })
            .collect();
        self.starts.push(snap(HEART));

        // Three fields at each start, behind it toward the sea and on its
        // flanks; then the rest.
        let home = (0.27 * start_core).max(70.0);
        let mut sites: Vec<((f64, f64), f64)> = Vec::new();
        for &(sx, sy) in &self.starts[..SPAWNS.len()] {
            for bearing in [205.0_f64, 270.0, 335.0] {
                let (s, c) = bearing.to_radians().sin_cos();
                let d = 0.85 * start_core;
                sites.push(((sx + c * d, sy + s * d), home));
            }
        }
        sites.extend(ORE.iter().map(|&(x, y, r)| ((x, y), r)));
        self.ore = sites
            .into_iter()
            .map(|(p, r)| {
                let p = snap(p);
                self.ore_field(p.0, p.1, r)
            })
            .collect();
        self.precursor = self.lay_facility_harvest();
        self.lay_machine();
        self.fit_forests();
    }

    /// Lying snow: on the alpine heights, lower on the faces that look
    /// north; over nearly all the fjords' land but its cliffs. None elsewhere.
    pub(super) fn harvest_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        let slope = (gx * gx + gy * gy).sqrt();
        match self.hv_region((x, y)) {
            HARVEST_ALPINE => {
                let north = (-gy / slope.max(1e-3)) * smoothstep(0.05, 0.3, slope);
                let line =
                    620.0 + 80.0 * self.mtn_mask.get(x / 1_400.0, y / 1_400.0) - 120.0 * north;
                let snow =
                    smoothstep(line - 50.0, line + 110.0, h) * (1.0 - smoothstep(0.9, 1.6, slope));
                (0.0, snow)
            }
            _ => {
                // Toward the fjords: frost in the hollows a little way past the
                // wall, tundra with snow lying in drifts, then snow all over, and
                // ice capping the high plateau toward the fjords.
                let cold = harvest_depth(harvest_look((x, y)), HARVEST_FJORDS)
                    + 350.0 * self.tilt.fbm(x / 1_600.0 - 5.0, y / 1_600.0, 2, 0.5);
                let frost = smoothstep(-900.0, 2_600.0, cold);
                if frost <= 0.0 {
                    return (0.0, 0.0);
                }
                // (Drifts in the lee, on the faces that look north, and in hollows;
                // small and many at first, run together further in.)
                let north = (-gy / slope.max(1e-3)) * smoothstep(0.02, 0.15, slope);
                let drift = 0.7 * self.tilt.fbm(x / 420.0 + 17.0, y / 420.0 - 3.0, 3, 0.5)
                    + 0.45 * self.detail.fbm(x / 110.0 - 12.0, y / 110.0 + 40.0, 3, 0.55)
                    + 0.25 * north;
                let lying = smoothstep(0.55 - 1.1 * frost, 0.75 - 1.1 * frost, drift)
                    * smoothstep(1.5, 6.0, h)
                    * (1.0 - smoothstep(0.9, 1.6, slope));
                let ice = smoothstep(2_800.0, 5_000.0, cold)
                    * smoothstep(170.0, 300.0, h)
                    * (1.0 - smoothstep(0.2, 0.45, slope));
                (ice, lying)
            }
        }
    }

    /// How thickly trees grow: autumn broadleaf over the heartland, pinyon
    /// and juniper thin on the desert's flats, spruce up the alpine valleys to
    /// the tree line, jungle on the isles; nothing on the fjords. None on the
    /// base's floor or in a site's room (`harvest_clear`).
    pub(super) fn harvest_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5);
        let copse = self
            .forest
            .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5);
        let clearing = smoothstep(
            0.30,
            0.55,
            self.forest
                .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5),
        );
        let mut habitable = smoothstep(2.0, 5.0, height);
        let density = match self.hv_region((x, y)) {
            HARVEST_DESERT => {
                habitable *= (1.0 - smoothstep(0.25, 0.45, slope))
                    * (1.0 - smoothstep(150.0, 200.0, height));
                let forest = smoothstep(self.forest_edge + 0.1, self.forest_edge + 0.3, broad);
                0.35 * (forest * (1.0 - 0.85 * clearing)).max(smoothstep(0.5, 0.7, copse) * 0.6)
            }
            HARVEST_ALPINE => {
                let line = 430.0 + 60.0 * self.forest_kind.get(x / 900.0, y / 900.0);
                habitable *= (1.0 - smoothstep(0.5, 0.85, slope))
                    * (1.0 - smoothstep(line - 70.0, line, height));
                let forest = smoothstep(self.forest_edge - 0.1, self.forest_edge + 0.15, broad);
                0.35 * (forest * (1.0 - 0.8 * clearing)).max(smoothstep(0.42, 0.62, copse) * 0.75)
            }
            HARVEST_ISLES => {
                habitable *= (1.0 - smoothstep(0.7, 0.95, slope)) * smoothstep(1.5, 3.5, height);
                let forest = smoothstep(self.forest_edge - 0.4, self.forest_edge - 0.12, broad);
                0.4 * (forest * (1.0 - 0.6 * clearing)).max(smoothstep(0.3, 0.5, copse) * 0.85)
            }
            HARVEST_FJORDS => 0.0,
            _ => {
                // Broad stands of broadleaf with ragged edges and glades in them,
                // a few copses out in the open; thinning to scrub toward the
                // desert and to bare tundra toward the fjords.
                let look = harvest_look((x, y));
                habitable *= (1.0 - smoothstep(0.6, 0.9, slope))
                    * (1.0 - smoothstep(-1_600.0, -300.0, harvest_depth(look, HARVEST_DESERT)))
                    * (1.0 - smoothstep(-1_400.0, -200.0, harvest_depth(look, HARVEST_FJORDS)));
                let wide = 1.8 * l;
                let field = self.forest.fbm(x / wide + 5.3, y / wide - 2.9, 4, 0.55)
                    + 0.3
                        * self
                            .forest
                            .fbm(x / (0.4 * l) - 7.1, y / (0.4 * l) + 3.3, 2, 0.5)
                    // (Ragged at the edges: thickets run out, bays run in.)
                    + 0.1 * self.detail.fbm(x / 130.0 + 2.0, y / 130.0 - 8.0, 2, 0.5);
                // (Held to their own edge, not the budget's `forest_edge`: the
                // other regions give way first when the trees run over.)
                let stands = smoothstep(-0.12, 0.1, field);
                // Clearings in the woods, some a few trees across, some meadows.
                let meadows = smoothstep(
                    0.3,
                    0.46,
                    self.forest
                        .fbm(x / (0.3 * l) + 19.0, y / (0.3 * l) - 4.0, 2, 0.5),
                );
                let glades = smoothstep(0.34, 0.5, clearing).max(meadows);
                // Half the canopy's trees are enough for a closed wood; a few
                // copses and lone trees out in the open.
                0.55 * stands * (1.0 - 0.9 * glades) + 0.08 * smoothstep(0.56, 0.7, copse) + 0.003
            }
        };
        if habitable > 0.0 {
            habitable *= self.hv_open_woods((x, y));
        }
        // A glade round every ore field.
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        // (`conifer` carries the height.)
        (density * habitable, height)
    }

    /// Each region's trees: broadleaf in the heartland (red and gold under
    /// the autumn climate) with pines on the hills' tops; juniper and pinyon
    /// in the desert; spruce and pine with birch low in the alpine valleys;
    /// jungle and palms on the isles.
    pub(super) fn harvest_tree(&self, x: f64, y: f64, height: f64, hash: u64) -> PropKind {
        // (Bits of their own: `unit(hash, 48)` would have only 16 left.)
        let (dead, pick) = (unit(hash, 40), unit(crate::noise::hash2(hash, 5, 3), 0));
        match self.hv_region((x, y)) {
            HARVEST_DESERT => match () {
                _ if dead < 0.05 => PropKind::TreeDead,
                _ if pick < smoothstep(80.0, 160.0, height) * 0.6 => PropKind::TreePinyon,
                _ => PropKind::TreeJuniper,
            },
            HARVEST_ALPINE => {
                let pines = self.forest_kind.get(x / 420.0 + 11.0, y / 420.0 - 5.0) > 0.1;
                match () {
                    _ if dead < 0.03 => PropKind::TreeDead,
                    _ if pick < 0.3 * (1.0 - smoothstep(120.0, 220.0, height)) => {
                        PropKind::TreeBroadleaf
                    }
                    _ if pines => PropKind::TreePine,
                    _ => PropKind::TreeConifer,
                }
            }
            HARVEST_ISLES => {
                let palms = 0.08 + 0.8 * (1.0 - smoothstep(4.0, 12.0, height));
                match () {
                    _ if dead < 0.006 => PropKind::TreeDead,
                    _ if pick < palms => PropKind::TreePalm,
                    _ if pick > 0.86 => PropKind::TreeBroadleaf,
                    _ => PropKind::TreeJungle,
                }
            }
            _ => {
                let stand = self.forest_kind.get(x / 520.0 + 11.0, y / 520.0 - 5.0);
                let pines = 0.04
                    + 0.3 * smoothstep(BASE_H + 60.0, BASE_H + 160.0, height)
                    + 0.12 * smoothstep(0.35, 0.6, stand);
                match () {
                    _ if dead < 0.012 => PropKind::TreeDead,
                    _ if pick < pines => PropKind::TreePine,
                    _ => PropKind::TreeBroadleaf,
                }
            }
        }
    }

    /// Whether trees and rocks may stand here: not on the base's floor, in a
    /// site's room or on a naval site's slips.
    pub(super) fn harvest_clear(&self, x: f64, y: f64) -> bool {
        let p = (x, y);
        if self.hv.plan.on_floor(p) {
            return false;
        }
        // (A naval site's islet is wooded round its pad.)
        !SITES.iter().any(|s| {
            let r = match s.ground {
                Ground::Land => s.room() + 40.0,
                Ground::Naval => 280.0,
            };
            dist(p, s.at) < r
        })
    }

    /// The woods thin over a way's floor: roads through the trees.
    fn hv_open_woods(&self, p: (f64, f64)) -> f64 {
        self.hv
            .plan
            .ways
            .iter()
            .fold(1.0, |k: f64, w| match w.near(p) {
                Some((d, _)) => k.min(0.25 + 0.75 * smoothstep(0.4 * w.half, w.half + 60.0, d)),
                None => k,
            })
    }
}

/// A site's sidecar line: its room, its kind and its way, map metres.
#[cfg(test)]
fn site_line(s: &Site, way: &[(f64, f64)]) -> String {
    let kind = match s.kind {
        Kind::Printer => "Printer",
        Kind::Auxiliary => "Auxiliary",
        Kind::Fleet => "Fleet",
    };
    let domain = match s.ground {
        Ground::Land => "Land",
        Ground::Naval => "Naval",
    };
    let path = way
        .iter()
        .map(|p| format!("({:.0}, {:.0})", p.0, p.1))
        .collect::<Vec<_>>()
        .join(", ");
    // An auxiliary site faces the defenders.
    let facing = if s.kind == Kind::Printer {
        "None".to_string()
    } else {
        let (sx, sy) = SPAWNS
            .iter()
            .fold((0.0, 0.0), |a, s| (a.0 + s.0 .0 / 4.0, a.1 + s.0 .1 / 4.0));
        let deg = (sy - s.at.1)
            .atan2(sx - s.at.0)
            .to_degrees()
            .rem_euclid(360.0);
        format!("Some({deg:.0})")
    };
    format!(
        "    (name: \"{}\", at: ({:.0}, {:.0}), domain: {domain}, facing: {facing}, kind: {kind}, room: {:.0}, path: [{path}]),\n",
        s.name,
        s.at.0,
        s.at.1,
        s.room()
    )
}

/// A design point's distance round a closed outline, for a smooth one.
fn closed(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    smooth_closed(points, 6)
}

/// An open line rounded through its points.
fn rounded(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    smooth_open(points, 8)
}
