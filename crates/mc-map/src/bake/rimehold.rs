//! "Rimehold": a survival map for four defenders, all of it arctic, 20 km.
//!
//! [`Layout::Rimehold`](super::Layout::Rimehold). The defenders hold the
//! hold: an upland of snowbound tundra in the middle-west of the map, their
//! four starts close together on it, one toward each corner. Open water runs
//! all the way round the hold, a kilometre and a half wide, and four necks of
//! land cross it, one from each side, narrow enough to wall:
//!
//! * east, the Causeway, the widest, from the Progenitor's base: the Heart
//!   stands at grade on a level floor on the mainland, its Gate over the
//!   road, facing west down it;
//! * north, a gravel spit from the highlands: ice-capped mountains, the
//!   printing Shapers' rooms in basins among them;
//! * south, a rocky neck from the fells: rolling tundra under nunataks;
//! * west, a long tombolo from Wester Isle out in the ocean.
//!
//! The mainland wraps the hold's sea round on three sides; the ocean is open
//! to the west. The necks part the sea ring into three: the west (the ocean,
//! and the north and south straits as far as their necks), the north lagoon
//! and the south lagoon either side of the Causeway. Each has its own
//! Shaper slips, and the base has a sea gate on each lagoon, so ships come at
//! every start; aircraft come from the base and every site.
//!
//! Behind the printing sites, hidden, the auxiliary Shapers: in a cirque in
//! the highlands, a hollow in the fells, a basin over the base, on an atoll in
//! the ocean. Reachable, but only by a narrow way.
//!
//! Map metres throughout, x east, y north, from the south-west corner; the
//! sidecar `maps/rimehold.ron` carries the same numbers (`rimehold_block`
//! prints its survival block). Nothing grows; snow lies over nearly all of it.

use super::bays::dist;
use super::facility::{Facility, Facing};
use super::sites::{Ground, Kind, Site, Way};
use super::{Pad, Terrain};
use crate::noise::smoothstep;
use crate::BUILD_CELL_M;

mod base;
mod shape;
#[cfg(test)]
mod tests;
mod wrecks;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 20_480.0;

/// The base's floor, metres over the water.
const BASE_H: f64 = 40.0;

/// The Heart: the Progenitor's start, where its ray leaves.
const HEART: (f64, f64) = (17_700.0, 10_236.0);

/// The base, facing west down the Causeway.
const FACILITY: Facility = Facility {
    heart: HEART,
    facing: Facing::West,
    floor: BASE_H,
    sites: SITES,
};

/// The necks' lines, hold end first: every way over a neck runs along its
/// line (`shape.rs` raises the neck round it).
const NORTH_NECK: [(f64, f64); 4] = [
    (7_850.0, 12_750.0),
    (7_750.0, 13_300.0),
    (7_600.0, 13_900.0),
    (7_650.0, 14_550.0),
];
const SOUTH_NECK: [(f64, f64); 4] = [
    (8_400.0, 7_550.0),
    (8_500.0, 6_900.0),
    (8_400.0, 6_300.0),
    (8_300.0, 5_750.0),
];
const WEST_NECK: [(f64, f64); 3] = [
    (5_300.0, 10_150.0),
    (4_500.0, 10_200.0),
    (3_700.0, 10_250.0),
];
const CAUSEWAY: [(f64, f64); 3] = [
    (10_800.0, 10_230.0),
    (11_600.0, 10_180.0),
    (12_450.0, 10_240.0),
];

/// Every site: the base's three, then the highlands', the fells', the
/// west's, and the auxiliary site over the base.
const SITES: &[Site] = &[
    // The base: the forecourt inside the Gate, the courts north and south of the Heart.
    Site {
        name: "The Forecourt",
        at: (15_406.0, 10_240.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (14_700.0, 10_240.0),
            (14_000.0, 10_240.0),
            (13_200.0, 10_250.0),
            (12_450.0, 10_240.0),
            (11_600.0, 10_180.0),
            (10_800.0, 10_230.0),
            (10_250.0, 10_260.0),
        ],
        cut: 2.0,
    },
    Site {
        name: "North Court",
        at: (16_956.0, 12_590.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (15_556.0, 12_190.0),
            (14_600.0, 11_900.0),
            (13_900.0, 11_550.0),
            (13_200.0, 10_800.0),
            (12_450.0, 10_260.0),
            (11_600.0, 10_200.0),
            (10_800.0, 10_250.0),
            (10_300.0, 10_550.0),
        ],
        cut: 2.0,
    },
    Site {
        name: "South Court",
        at: (17_106.0, 7_840.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: BASE_H,
        way: &[
            (15_556.0, 8_240.0),
            (14_600.0, 8_550.0),
            (13_900.0, 8_900.0),
            (13_200.0, 9_650.0),
            (12_450.0, 10_220.0),
            (11_600.0, 10_160.0),
            (10_800.0, 10_210.0),
            (10_300.0, 9_950.0),
        ],
        cut: 2.0,
    },
    // The highlands: two basins, their valleys down to the north neck; slips
    // in a bay east of the neck, on the north lagoon; the auxiliary site in a
    // cirque over the western basin.
    Site {
        name: "Hoarfrost",
        at: (5_000.0, 17_300.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 150.0,
        way: &[
            (5_500.0, 16_650.0),
            (6_300.0, 15_950.0),
            (7_100.0, 15_300.0),
            (7_650.0, 14_550.0),
            (7_600.0, 13_900.0),
            (7_750.0, 13_300.0),
            (7_850.0, 12_750.0),
            (7_950.0, 12_200.0),
        ],
        cut: 1.2,
    },
    Site {
        name: "Whitecap",
        at: (10_500.0, 18_000.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 260.0,
        way: &[
            (10_000.0, 17_300.0),
            (9_300.0, 16_600.0),
            (8_500.0, 15_900.0),
            (7_900.0, 15_200.0),
            (7_650.0, 14_550.0),
            (7_600.0, 13_900.0),
            (7_750.0, 13_300.0),
            (7_850.0, 12_750.0),
            (8_150.0, 12_200.0),
        ],
        cut: 1.2,
    },
    Site {
        name: "Ice Harbour",
        at: (11_400.0, 15_050.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 6.0,
        way: &[
            (11_100.0, 14_300.0),
            (10_500.0, 13_750.0),
            (9_800.0, 13_500.0),
            (9_300.0, 13_450.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Rime Cirque",
        at: (3_900.0, 19_500.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 290.0,
        way: &[
            (4_350.0, 18_850.0),
            (4_700.0, 18_250.0),
            (4_950.0, 17_780.0),
        ],
        cut: 0.8,
    },
    // The fells: two rooms among the nunataks, their ways to the south neck;
    // slips on the south lagoon east of the neck; the auxiliary site in a
    // hollow behind the western room.
    Site {
        name: "Scarfell",
        at: (5_400.0, 2_900.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 100.0,
        way: &[
            (6_100.0, 3_500.0),
            (7_000.0, 4_300.0),
            (7_800.0, 5_050.0),
            (8_300.0, 5_750.0),
            (8_400.0, 6_300.0),
            (8_500.0, 6_900.0),
            (8_400.0, 7_550.0),
            (8_300.0, 8_100.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Blackwater",
        at: (11_300.0, 2_900.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 140.0,
        way: &[
            (10_600.0, 3_500.0),
            (9_700.0, 4_200.0),
            (8_900.0, 4_950.0),
            (8_300.0, 5_750.0),
            (8_400.0, 6_300.0),
            (8_500.0, 6_900.0),
            (8_400.0, 7_550.0),
            (8_550.0, 8_150.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Floe Harbour",
        at: (11_500.0, 5_500.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 6.0,
        way: &[
            (11_250.0, 6_250.0),
            (10_700.0, 6_700.0),
            (9_900.0, 6_900.0),
            (9_300.0, 6_950.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Frostvault",
        at: (2_900.0, 1_500.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 160.0,
        way: &[(3_600.0, 2_000.0), (4_300.0, 2_500.0), (4_950.0, 2_800.0)],
        cut: 0.8,
    },
    // The west: a room in Wester Isle's valley, its way over the tombolo;
    // slips on a skerry off each strait's mouth; the Gatewright's atoll out
    // in the ocean, its ring of hills opening west.
    Site {
        name: "Wester Isle",
        at: (2_300.0, 10_500.0),
        kind: Kind::Printer,
        ground: Ground::Land,
        level: 40.0,
        way: &[
            (3_000.0, 10_350.0),
            (3_700.0, 10_250.0),
            (4_500.0, 10_200.0),
            (5_300.0, 10_150.0),
            (5_900.0, 10_200.0),
            (6_500.0, 10_250.0),
        ],
        cut: 1.2,
    },
    Site {
        name: "Gull Skerry",
        at: (1_500.0, 13_700.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 5.0,
        way: &[
            (2_300.0, 13_650.0),
            (3_300.0, 13_600.0),
            (4_400.0, 13_600.0),
            (5_400.0, 13_550.0),
            (6_200.0, 13_500.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Seal Skerry",
        at: (1_500.0, 6_900.0),
        kind: Kind::Printer,
        ground: Ground::Naval,
        level: 5.0,
        way: &[
            (2_300.0, 6_950.0),
            (3_300.0, 7_000.0),
            (4_300.0, 7_000.0),
            (5_300.0, 6_950.0),
            (6_300.0, 7_000.0),
        ],
        cut: 1.0,
    },
    Site {
        name: "Far Skerry",
        at: (1_100.0, 2_900.0),
        kind: Kind::Fleet,
        ground: Ground::Land,
        level: 18.0,
        way: &[(500.0, 2_950.0), (200.0, 2_980.0)],
        cut: 0.6,
    },
    // Over the base: a basin in the mountains north of its floor.
    Site {
        name: "Echo Vault",
        at: (17_500.0, 17_600.0),
        kind: Kind::Auxiliary,
        ground: Ground::Land,
        level: 330.0,
        way: &[
            (17_200.0, 16_850.0),
            (16_900.0, 16_050.0),
            (16_650.0, 15_150.0),
            (16_450.0, 14_300.0),
        ],
        cut: 0.8,
    },
];

/// The defenders' starts, and what holding each is like.
const SPAWNS: [((f64, f64), &str, &str); 4] = [
    (
        (6_600.0, 11_550.0),
        "Cairn Rise",
        "North-west, on the open tundra: the highlands' columns over the north neck and Gull Skerry's ships up the north strait come at it first.",
    ),
    (
        (9_400.0, 11_450.0),
        "Lantern Bluff",
        "North-east, nearest the base: the Causeway's columns, and the north lagoon's ships under its bluff.",
    ),
    (
        (6_700.0, 8_950.0),
        "Sealstone",
        "South-west: Wester Isle's columns over the tombolo and Seal Skerry's ships up the south strait.",
    ),
    (
        (9_350.0, 9_050.0),
        "Kittiwake Head",
        "South-east: the Causeway and the south neck meet over it, the south lagoon's ships behind.",
    ),
];

/// The middle of the hold, which the starts stand round.
const MIDDLE: (f64, f64) = (8_000.0, 10_240.0);

/// Ore away from the starts: centre and radius. Across the hold, at the
/// necks' feet, and on the ways out of each region.
const ORE: &[(f64, f64, f64)] = &[
    // The hold: its middle, between the starts, toward each neck.
    (8_000.0, 10_250.0, 140.0),
    (8_050.0, 12_050.0, 110.0),
    (8_000.0, 8_450.0, 110.0),
    (5_900.0, 10_250.0, 110.0),
    (10_150.0, 10_250.0, 120.0),
    (5_800.0, 12_250.0, 100.0),
    (10_100.0, 12_100.0, 100.0),
    (5_900.0, 8_300.0, 100.0),
    (10_050.0, 8_350.0, 100.0),
    // Past the necks.
    (7_250.0, 15_600.0, 110.0),
    (8_000.0, 4_800.0, 110.0),
    (3_050.0, 10_000.0, 100.0),
    (13_300.0, 10_700.0, 120.0),
    (13_300.0, 9_750.0, 120.0),
    // Deep in each region, on its ways.
    (5_900.0, 16_500.0, 100.0),
    (9_300.0, 16_900.0, 100.0),
    (6_300.0, 3_700.0, 100.0),
    (10_300.0, 3_800.0, 100.0),
    (2_400.0, 11_400.0, 90.0),
];

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Rimehold {
    /// The ground's plan (`shape.rs`).
    plan: shape::Plan,
}

impl Terrain {
    pub(super) fn setup_rimehold(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        // The outlines first; the erosion is run over them before the ways,
        // rooms and the base's floor are cut, which it never touches.
        self.rh.plan = shape::Plan::outlines();
        self.erode_alpine();
        self.rh_profiles();

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

        // Three fields at each start, behind it (away from the hold's middle)
        // and on its flanks; then the rest.
        let home = (0.27 * start_core).max(70.0);
        let mut sites: Vec<((f64, f64), f64)> = Vec::new();
        for &(sx, sy) in &self.starts[..SPAWNS.len()] {
            let away = (sy - MIDDLE.1).atan2(sx - MIDDLE.0);
            for turn in [-1.1_f64, 0.0, 1.1] {
                let (s, c) = (away + turn).sin_cos();
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
        self.precursor = self.facility_pieces(&FACILITY);
        self.precursor.extend(self.rh_sea_gates());
        self.lay_machine();
    }

    /// Ice and lying snow at a point of height `h` with slope `(gx, gy)`:
    /// snow over nearly everything; rock where it is steep or the wind has
    /// scoured it, dark shingle along the water, bare ground round the ore;
    /// the sea ice and the bergs ice under a patchy skin of snow, their walls bare.
    pub(super) fn rimehold_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        if h < 0.4 {
            return (0.0, 0.0);
        }
        let p = (x, y);
        let slope = (gx * gx + gy * gy).sqrt();
        let scour = smoothstep(
            0.12,
            0.45,
            self.tilt.fbm(x / 520.0 + 77.0, y / 520.0 - 31.0, 3, 0.5)
                + 0.35 * self.detail.fbm(x / 140.0 - 12.0, y / 140.0 + 40.0, 2, 0.5),
        );
        let steep = 1.0 - smoothstep(0.55, 1.05, slope);
        // The knolls' crowns and the heights' ridges are blown bare.
        let crowns = smoothstep(0.35, 0.8, self.rh_knolls(p)) * smoothstep(6.0, 20.0, h);
        let shingle = 1.0 - smoothstep(1.8, 4.5, h);
        let mut ground = (0.95 - 0.6 * scour.max(crowns)) * steep * (1.0 - 0.85 * shingle);
        for f in &self.ore {
            let d = dist(p, (f.x, f.y)) - f.radius;
            if d < 120.0 {
                ground *= 0.15 + 0.85 * smoothstep(-10.0, 120.0, d);
            }
        }
        let ice = self.rh_ice(p, h);
        if ice <= 0.0 {
            return (0.0, ground);
        }
        // Sea ice and bergs: snow on top, swept bare in places; bare in their walls.
        let swept = 0.35 + 0.5 * smoothstep(-0.2, 0.3, self.tilt.fbm(x / 400.0, y / 400.0, 3, 0.5));
        let walls = smoothstep(0.35, 0.8, slope);
        let snowed = ice * swept * (1.0 - walls);
        ((ice - snowed).max(0.0), ground * (1.0 - ice) + snowed)
    }

    /// Whether rocks may stand here: not on the base's floor, in a site's
    /// room or on a naval site's slips. (Nothing grows.)
    pub(super) fn rimehold_clear(&self, x: f64, y: f64) -> bool {
        let p = (x, y);
        if self.rh.plan.on_floor(p) {
            return false;
        }
        !SITES.iter().any(|s| {
            let r = match s.ground {
                Ground::Land => s.room() + 40.0,
                Ground::Naval => 280.0,
            };
            dist(p, s.at) < r
        })
    }

    /// Works out every way's floor, once the ground is laid.
    fn rh_profiles(&mut self) {
        let profiles: Vec<Vec<f64>> = self
            .rh
            .plan
            .ways
            .iter()
            .map(|w| {
                if w.sea {
                    Vec::new()
                } else {
                    Self::way_profile(w, SITES, |q| {
                        let r = self.rh_raw(q);
                        r.h + self.erosion.at(q.0, q.1) * r.erodes
                    })
                }
            })
            .collect();
        for (w, p) in self.rh.plan.ways.iter_mut().zip(profiles) {
            w.profile = p;
        }
    }
}

/// The ways, in [`SITES`]' order.
fn ways() -> Vec<Way> {
    SITES.iter().map(Way::new).collect()
}
