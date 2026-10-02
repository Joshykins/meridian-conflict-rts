//! "Frostline": four against four across a land bridge between two oceans,
//! the Precursors' climate wall down the middle of it all.
//!
//! [`Layout::Frostline`], 16 km. Two landmasses, one in the west and one in
//! the east; a land bridge joins them across the middle and parts the sea
//! into a north ocean and a south one. The wall ([`FROSTLINE_WALL`]) runs
//! south to north through both oceans and over the bridge, straight but for
//! one diagonal slash in each ocean: west of it the country is red-rock
//! desert, east of it Alaska. Where it runs through open sea it is a line of
//! towers a kilometre apart, each standing on the sea floor; over the bridge
//! and the islands it is only light in the ground. Armies and fleets pass
//! through it almost anywhere.
//!
//! The country is designed with the landmasses due west and east of each
//! other and laid on the map turned [`FROSTLINE_TURN_DEG`] clockwise about
//! its middle ([`design`], [`on_map`]): the bridge runs from the west-north-
//! west to the east-south-east, the north ocean opens into the map's
//! north-east corner and the south ocean into its south-west, and each
//! landmass runs on into the corner behind it. Everything below is in the
//! design's frame unless it says otherwise.
//!
//! Each side's four bases, after Seton's Clutch: the rear guard at the far
//! edge inside a ring of mountains with two passes; the front on the high
//! neck of the bridge, cliffs to either ocean (its war is the bridge); a
//! beach base in a low basin on a long strand of one ocean; and a cliff base
//! on the upland over the other ocean, whose only way down to the water is
//! the canyon that comes out at a cove well along the coast. So in each
//! ocean one side's beach base faces the other's cliff base. One island
//! stands in the middle of each ocean, the wall across it: half red rock,
//! half snow. The bridge has beaches on both oceans.
//!
//! The land is benches: the upland, a low basin behind the beach base's
//! strand and the low bridge cut into it by an escarpment, and a high
//! plateau in the back country; ramps lead from one to the next. A canyon
//! runs from the back country down to the cliff base's cove, to be crossed
//! at two places; a dry lake lies in the basin.
//!
//! Fairness is in the play, not the look. What decides where a unit can go
//! is designed once, for the west, and laid on the east turned half round
//! the centre: the bases, the benches and their ramps, the canyon and its
//! crossings, the rear's ring and its passes, the mountains' feet and the
//! first 70 m of their walls, the ore. Everything else is each side's own:
//! all four shores are drawn separately, the designed lines are bent by a
//! warp that is not the same turned, every edge frays to its own noise, and
//! the small relief is the climate's (dunes, swells and washes in the
//! desert; drumlins, hummocks and streams in Alaska). What stands above the
//! mountains' walls is the climate's too: mesas cut in the canyon's beds in
//! the west, peaks, arêtes and ice in the east. The canyon is dry sand in the west; in
//! the east it is a gorge with a frozen river, the dry lake a frozen one.
//! `tests/frostline.rs` holds the baked map to the same walks, the same
//! ground, shore and timber for both sides, within a few percent.
//!
//! Coordinates are map metres, y north.

use super::bays::dist;
use super::canyon::{smooth_closed, smooth_open};
use super::machine::Machine;
use super::{Pad, Terrain};
use crate::format::PropKind;
use crate::landmark::{frostline_on_map as on_map, FROSTLINE_TURN_DEG, FROSTLINE_WALL as WALL};
use crate::noise::{smoothstep, unit};
use crate::BUILD_CELL_M;
use std::f64::consts::PI;

mod shape;
#[cfg(test)]
mod tests;

/// The map's edge, metres: the design is for exactly this.
pub(super) const SIZE: f64 = 16_384.0;

/// Where a point of the map lies in the design's frame ([`on_map`] undone).
/// The map's corners lie outside the design's own square, by 2.3 km at most:
/// the coasts, the benches and the mountains are drawn on past its edges.
fn design((x, y): (f64, f64)) -> (f64, f64) {
    let mid = SIZE / 2.0;
    let (s, c) = FROSTLINE_TURN_DEG.to_radians().sin_cos();
    let (dx, dy) = (x - mid, y - mid);
    (mid + dx * c - dy * s, mid + dx * s + dy * c)
}

/// The north ocean's coast: the desert's shore from beyond the map's north
/// edge south to the bridge (a cape, then the beach base's bight, then the
/// headland the front's cliffs stand on), east along the bridge, and
/// Alaska's shore north again (the cliff base's coast and its fjord) to
/// beyond the map's corner.
const NORTH: &[(f64, f64)] = &[
    (6_650.0, 19_800.0),
    (6_550.0, 17_600.0),
    (6_300.0, 16_750.0),
    (6_420.0, 16_000.0),
    (6_300.0, 15_300.0),
    (6_250.0, 14_500.0),
    (5_800.0, 14_050.0),
    (5_450.0, 13_500.0),
    (5_300.0, 12_900.0),
    (5_350.0, 12_200.0),
    (5_600.0, 11_600.0),
    (6_200.0, 11_300.0),
    (6_500.0, 10_800.0),
    (6_200.0, 10_300.0),
    (5_750.0, 10_000.0),
    (5_700.0, 9_600.0),
    (5_950.0, 9_300.0),
    (6_500.0, 9_150.0),
    (7_000.0, 9_250.0),
    (7_450.0, 9_000.0),
    (7_900.0, 8_850.0),
    (8_200.0, 8_900.0),
    (8_600.0, 9_050.0),
    (9_200.0, 8_950.0),
    (9_800.0, 9_200.0),
    (10_450.0, 9_350.0),
    (10_750.0, 9_800.0),
    (10_550.0, 10_400.0),
    (10_900.0, 10_900.0),
    (11_350.0, 11_300.0),
    (11_200.0, 11_900.0),
    (11_500.0, 12_500.0),
    (11_350.0, 13_200.0),
    (11_650.0, 13_800.0),
    (11_500.0, 14_350.0),
    (11_700.0, 14_650.0),
    (12_150.0, 14_800.0),
    (12_600.0, 14_950.0),
    (12_200.0, 15_180.0),
    (11_750.0, 15_180.0),
    (11_900.0, 15_700.0),
    (12_500.0, 16_100.0),
    (13_000.0, 16_384.0),
    (13_050.0, 17_600.0),
    (13_250.0, 18_900.0),
    (13_200.0, 19_800.0),
];

/// The south ocean's coast: Alaska's shore from beyond the map's south edge
/// north to the bridge (a cape, the beach base's bight, the front's
/// headland), west along the bridge, and the desert's shore south again
/// (the cliff base's coast and the cove its canyon comes out at) to beyond
/// the map's corner.
const SOUTH: &[(f64, f64)] = &[
    (9_750.0, -3_400.0),
    (9_900.0, -1_200.0),
    (10_050.0, -380.0),
    (9_930.0, 400.0),
    (10_100.0, 1_100.0),
    (10_150.0, 1_850.0),
    (10_550.0, 2_350.0),
    (10_950.0, 2_850.0),
    (11_100.0, 3_500.0),
    (11_000.0, 4_200.0),
    (10_700.0, 4_800.0),
    (10_150.0, 5_100.0),
    (9_850.0, 5_600.0),
    (10_200.0, 6_100.0),
    (10_650.0, 6_400.0),
    (10_700.0, 6_800.0),
    (10_450.0, 7_080.0),
    (9_900.0, 7_250.0),
    (9_400.0, 7_150.0),
    (8_950.0, 7_400.0),
    (8_500.0, 7_550.0),
    (8_200.0, 7_480.0),
    (7_800.0, 7_330.0),
    (7_200.0, 7_430.0),
    (6_600.0, 7_180.0),
    (5_950.0, 7_050.0),
    (5_650.0, 6_600.0),
    (5_850.0, 6_000.0),
    (5_500.0, 5_500.0),
    (5_050.0, 5_100.0),
    (5_200.0, 4_500.0),
    (4_900.0, 3_900.0),
    (5_050.0, 3_200.0),
    (4_750.0, 2_600.0),
    (4_900.0, 2_050.0),
    (4_700.0, 1_750.0),
    (4_250.0, 1_620.0),
    (3_784.0, 1_434.0),
    (4_200.0, 1_230.0),
    (4_650.0, 1_230.0),
    (4_500.0, 700.0),
    (3_900.0, 300.0),
    (3_400.0, 0.0),
    (3_350.0, -1_200.0),
    (3_150.0, -2_500.0),
    (3_200.0, -3_400.0),
];

/// Coasts that meet the sea in a cliff: the cliff bases' shores, and the
/// headlands either side of each neck of the bridge.
const CLIFFS: &[&[(f64, f64)]] = &[
    // The desert's, on the south ocean, from the neck to the map's corner.
    &[
        (6_300.0, 7_100.0),
        (5_950.0, 7_050.0),
        (5_650.0, 6_600.0),
        (5_850.0, 6_000.0),
        (5_500.0, 5_500.0),
        (5_050.0, 5_100.0),
        (5_200.0, 4_500.0),
        (4_900.0, 3_900.0),
        (5_050.0, 3_200.0),
        (4_750.0, 2_600.0),
        (4_900.0, 2_050.0),
        (4_700.0, 1_750.0),
        (4_650.0, 1_230.0),
        (4_500.0, 700.0),
        (3_900.0, 300.0),
        (3_400.0, 0.0),
        (3_350.0, -1_200.0),
        (3_150.0, -2_500.0),
    ],
    // The front's north headland.
    &[
        (6_200.0, 10_300.0),
        (5_750.0, 10_000.0),
        (5_700.0, 9_600.0),
        (5_950.0, 9_300.0),
        (6_300.0, 9_200.0),
    ],
    // Alaska's, on the north ocean, from the neck to the map's corner.
    &[
        (10_100.0, 9_280.0),
        (10_450.0, 9_350.0),
        (10_750.0, 9_800.0),
        (10_550.0, 10_400.0),
        (10_900.0, 10_900.0),
        (11_350.0, 11_300.0),
        (11_200.0, 11_900.0),
        (11_500.0, 12_500.0),
        (11_350.0, 13_200.0),
        (11_650.0, 13_800.0),
        (11_500.0, 14_350.0),
        (11_700.0, 14_650.0),
        (11_750.0, 15_180.0),
        (11_900.0, 15_700.0),
        (12_500.0, 16_100.0),
        (13_000.0, 16_384.0),
        (13_050.0, 17_600.0),
        (13_250.0, 18_900.0),
    ],
    // Its front's south headland.
    &[
        (10_200.0, 6_100.0),
        (10_650.0, 6_400.0),
        (10_700.0, 6_800.0),
        (10_450.0, 7_080.0),
        (10_100.0, 7_200.0),
    ],
];

/// The head of the cliff base's cove, where its canyon comes out on a beach.
/// The east's fjord ends at this turned.
const COVE_HEAD: (f64, f64) = (3_784.0, 1_434.0);

/// The north ocean's island, in the middle of it on the wall's line: centre,
/// radius. The south ocean's is this turned, its shape its own.
const ISLE: (f64, f64, f64) = (8_400.0, 12_900.0, 620.0);

/// Starts, west side: the rear guard, the front, the beach base, the cliff
/// base. Each is followed on the map by its turned twin.
const STARTS: &[(f64, f64)] = &[
    (1_500.0, 8_192.0),
    (5_150.0, 8_192.0),
    (4_300.0, 12_400.0),
    (3_800.0, 3_900.0),
];

/// A line from one point to another and everything within a distance of it.
type Capsule = ((f64, f64), (f64, f64), f64);

/// The benches, metres over the sea: the low country (the beach base's basin
/// and the bridge), the upland most of the land stands at, the high plateau.
const LOW: f64 = 14.0;
const UPLAND: f64 = 60.0;
const HIGH: f64 = 104.0;

/// The beach base's basin: low ground behind its strand, shut in by the
/// upland's escarpment from the map's north edge round to the front's
/// headland. West side; closed out at sea.
const BASIN: &[(f64, f64)] = &[
    (3_650.0, 18_000.0),
    (3_350.0, 15_300.0),
    (2_950.0, 14_200.0),
    (3_050.0, 13_100.0),
    (3_400.0, 12_000.0),
    (3_600.0, 11_000.0),
    (4_200.0, 10_350.0),
    (5_000.0, 10_150.0),
    (5_800.0, 10_250.0),
    (7_600.0, 10_400.0),
    (7_600.0, 18_000.0),
];

/// The bridge's low ground, from the west neck to the middle.
const BRIDGE: &[(f64, f64)] = &[
    (6_250.0, 10_300.0),
    (6_250.0, 6_100.0),
    (8_400.0, 6_100.0),
    (8_400.0, 10_300.0),
];

/// The high plateau in the north-west back country, out to the map's corner.
const PLATEAU: &[(f64, f64)] = &[
    (-3_000.0, 11_700.0),
    (-1_700.0, 11_450.0),
    (-1_050.0, 11_000.0),
    (-350.0, 11_420.0),
    (350.0, 11_250.0),
    (1_300.0, 11_180.0),
    (2_400.0, 11_050.0),
    (2_950.0, 11_800.0),
    (2_780.0, 13_300.0),
    (1_950.0, 14_450.0),
    (450.0, 14_700.0),
    (-3_000.0, 14_900.0),
];

/// Ramps: where an escarpment is laid back into a slope, as the line it is
/// laid back along and how far either side of it. The basin's three (west to
/// the back country, south toward the front, north-west onto the plateau's
/// shoulder), the neck's whole width down onto the bridge, and the plateau's
/// two.
const RAMPS: &[Capsule] = &[
    ((3_020.0, 13_250.0), (3_090.0, 12_950.0), 170.0),
    ((4_100.0, 10_400.0), (4_400.0, 10_300.0), 170.0),
    ((3_030.0, 14_300.0), (3_110.0, 14_580.0), 160.0),
    ((6_250.0, 7_150.0), (6_250.0, 9_250.0), 260.0),
    ((2_890.0, 12_400.0), (2_850.0, 12_700.0), 160.0),
    ((1_250.0, 11_165.0), (1_550.0, 11_135.0), 160.0),
];

/// The canyon's course, from the cove's head up into the back country.
const CANYON: &[(f64, f64)] = &[
    (3_784.0, 1_434.0),
    (3_450.0, 2_100.0),
    (3_100.0, 2_700.0),
    (2_700.0, 3_300.0),
    (2_800.0, 4_000.0),
    (2_500.0, 4_700.0),
    (1_900.0, 5_100.0),
    (1_300.0, 5_600.0),
];
/// Where its walls are laid back so it can be crossed, and entered, as
/// shares of the way up it.
const CROSSINGS: &[f64] = &[0.2, 0.56];
/// Its side canyons, each from a point of the course up to its head.
const SIDE_CANYONS: &[&[(f64, f64)]] = &[
    &[(3_100.0, 2_700.0), (2_650.0, 2_480.0), (2_250.0, 2_620.0)],
    &[(2_700.0, 3_300.0), (2_250.0, 3_550.0), (1_850.0, 3_480.0)],
    &[(2_500.0, 4_700.0), (2_850.0, 4_880.0), (2_980.0, 5_150.0)],
    &[(2_800.0, 4_000.0), (2_350.0, 4_150.0), (2_000.0, 3_950.0)],
    &[(1_900.0, 5_100.0), (1_700.0, 4_650.0), (1_420.0, 4_560.0)],
];
/// The main canyon's floor at the cove, metres over the sea, and its rise
/// per metre up the course; a side canyon's rise from where it joins.
const CANYON_MOUTH: f64 = 2.5;
const CANYON_GRADE: f64 = 0.0105;
const SIDE_GRADE: f64 = 0.024;

/// The dry lake in the basin: centre, radius.
const PLAYAS: &[(f64, f64, f64)] = &[(4_350.0, 14_300.0, 340.0)];

struct Ridge {
    /// Crest line.
    line: &'static [(f64, f64)],
    /// Half width of the mountain's foot.
    half: f64,
    /// Height of an alpine crest above the land round it. A desert one is as
    /// high as its width lets the beds stack (`shape::mesa`).
    tall: f64,
}

/// The mountains that shape the play: laid on the west as listed and on the
/// east turned.
const RIDGES: &[Ridge] = &[
    // The rear guard's ring, about 1.5 km round its base. East: the great
    // mesa between it and the front.
    Ridge {
        line: &[
            (2_880.0, 7_180.0),
            (3_200.0, 7_700.0),
            (3_300.0, 8_192.0),
            (3_200.0, 8_680.0),
            (2_880.0, 9_200.0),
        ],
        half: 500.0,
        tall: 600.0,
    },
    // North, from the map's edge round to the north-east pass.
    Ridge {
        line: &[
            (-1_500.0, 9_330.0),
            (-300.0, 9_500.0),
            (450.0, 9_650.0),
            (1_000.0, 9_800.0),
            (1_500.0, 9_830.0),
            (1_950.0, 9_720.0),
            (2_250.0, 9_560.0),
        ],
        half: 290.0,
        tall: 440.0,
    },
    // Behind it; the map's edge runs off north-westward beyond.
    Ridge {
        line: &[(60.0, 10_100.0), (-60.0, 8_192.0), (60.0, 6_300.0)],
        half: 320.0,
        tall: 500.0,
    },
    // South, from the edge round to the south-east pass.
    Ridge {
        line: &[
            (-300.0, 6_880.0),
            (450.0, 6_730.0),
            (1_000.0, 6_580.0),
            (1_500.0, 6_550.0),
            (1_950.0, 6_660.0),
            (2_250.0, 6_820.0),
        ],
        half: 290.0,
        tall: 440.0,
    },
    // The headland between the front and the basin: sea cliffs on the north
    // ocean, so the front has no shore of its own.
    Ridge {
        line: &[(5_300.0, 9_800.0), (5_650.0, 10_250.0)],
        half: 210.0,
        tall: 380.0,
    },
    // Its match on the south ocean, between the front and the cliff base.
    Ridge {
        line: &[(5_300.0, 6_650.0), (5_350.0, 6_050.0)],
        half: 210.0,
        tall: 360.0,
    },
    // A mesa in the open upland between the rear's south-east pass and the
    // cliff base.
    Ridge {
        line: &[(3_250.0, 5_750.0), (3_950.0, 6_150.0)],
        half: 360.0,
        tall: 460.0,
    },
    // Buttes and knolls standing alone in the open country.
    Ridge {
        line: &[(5_000.0, 11_500.0), (5_110.0, 11_580.0)],
        half: 150.0,
        tall: 190.0,
    },
    Ridge {
        line: &[(3_900.0, 13_500.0), (3_960.0, 13_620.0)],
        half: 130.0,
        tall: 170.0,
    },
    Ridge {
        line: &[(2_150.0, 6_050.0), (2_260.0, 6_000.0)],
        half: 140.0,
        tall: 180.0,
    },
    Ridge {
        line: &[(4_400.0, 2_750.0), (4_450.0, 2_870.0)],
        half: 140.0,
        tall: 180.0,
    },
    Ridge {
        line: &[(3_650.0, 9_350.0), (3_720.0, 9_250.0)],
        half: 150.0,
        tall: 190.0,
    },
    Ridge {
        line: &[(4_300.0, 7_250.0), (4_420.0, 7_300.0)],
        half: 140.0,
        tall: 180.0,
    },
    // On the plateau in the corner behind the rear guard: a long mesa and
    // two buttes.
    Ridge {
        line: &[(-350.0, 11_950.0), (250.0, 12_180.0), (700.0, 12_700.0)],
        half: 230.0,
        tall: 420.0,
    },
    Ridge {
        line: &[(900.0, 13_500.0), (1_040.0, 13_560.0)],
        half: 170.0,
        tall: 220.0,
    },
    Ridge {
        line: &[(-250.0, 13_150.0), (-130.0, 13_230.0)],
        half: 150.0,
        tall: 190.0,
    },
];

/// The ranges along the map's edges, so the country ends in mountains and
/// not on a ruled line. Map metres, not the design's: the desert's along the
/// west edge and the north edge as far as its coast, and Alaska's the same
/// turned. Where the coast comes to the map's corner they fall into the sea.
const EDGE_RANGES: &[Ridge] = &[
    // The west edge: behind the cliff base's country from the corner's sea
    // cliffs up, then behind the rear guard's ring and the plateau.
    Ridge {
        line: &[
            (140.0, 700.0),
            (260.0, 1_900.0),
            (120.0, 3_300.0),
            (240.0, 4_700.0),
            (90.0, 6_100.0),
            (200.0, 7_500.0),
            (60.0, 8_900.0),
            (180.0, 10_300.0),
            (90.0, 11_700.0),
            (230.0, 13_100.0),
            (110.0, 14_600.0),
            (190.0, 16_250.0),
        ],
        half: 330.0,
        tall: 640.0,
    },
    // The north edge: over the plateau, then behind the beach base's basin
    // to the coast.
    Ridge {
        line: &[
            (190.0, 16_250.0),
            (1_700.0, 16_120.0),
            (3_300.0, 16_260.0),
            (4_900.0, 16_150.0),
            (6_500.0, 16_280.0),
            (8_000.0, 16_200.0),
            (9_300.0, 16_330.0),
        ],
        half: 310.0,
        tall: 540.0,
    },
];

/// The passes: ground no mountain may stand on, as the line through each
/// and its half width. The rear guard's two ways out of its ring,
/// north-east and south-east, each drawn well past the mountains either
/// side of it: the warp moves those, and a pass must cut clean through.
const PASSES: &[Capsule] = &[
    ((1_980.0, 8_860.0), (3_200.0, 9_950.0), 200.0),
    ((1_980.0, 7_520.0), (3_200.0, 6_430.0), 200.0),
];

/// Ore away from the bases: centre, radius. The turn gives each its twin.
const ORE: &[(f64, f64, f64)] = &[
    // On the bridge.
    (6_700.0, 8_400.0, 80.0),
    (7_550.0, 7_900.0, 75.0),
    // Forward of the front base, on the neck.
    (5_800.0, 8_700.0, 70.0),
    // The beach base's basin.
    (4_900.0, 13_350.0, 70.0),
    (3_700.0, 13_900.0, 70.0),
    (4_450.0, 10_950.0, 70.0),
    // The cliff base's upland: toward the front, by the coast, across the canyon.
    (4_300.0, 5_000.0, 70.0),
    (4_350.0, 3_250.0, 65.0),
    (1_700.0, 5_650.0, 70.0),
    // Inside the rear guard's ring.
    (1_000.0, 8_900.0, 65.0),
    (1_900.0, 7_450.0, 65.0),
    // The back country: on the plateau, under it, toward the canyon's head,
    // south of the canyon, and the upland either side of the front.
    (1_500.0, 12_400.0, 80.0),
    (2_150.0, 13_500.0, 75.0),
    (1_750.0, 10_520.0, 70.0),
    (2_600.0, 6_000.0, 70.0),
    (2_700.0, 1_750.0, 75.0),
    (4_150.0, 9_450.0, 70.0),
    (4_150.0, 6_900.0, 70.0),
    // The corner behind the rear guard: on the upland, and out on the plateau.
    (-350.0, 10_450.0, 70.0),
    (-700.0, 12_500.0, 75.0),
];

/// Ore on the north ocean's island, one field either side of the wall; the
/// south's has the same turned.
const ISLE_ORE: &[(f64, f64, f64)] = &[(8_610.0, 12_800.0, 70.0), (8_190.0, 13_000.0, 70.0)];

/// Towers stand this far apart along the wall, about.
const TOWER_PITCH: f64 = 1_100.0;
/// A tower stands only in sea at least this deep, some 400 m off a beach, so
/// the land bridge and the islands keep the wall as light in the ground alone.
const TOWER_DEPTH: f64 = 45.0;

/// A canyon's course as laid: rounded, the metres along it to each point,
/// its floor's height at its first point and rise per metre from there, and
/// the box it lies in.
struct Course {
    pts: Vec<(f64, f64)>,
    run: Vec<f64>,
    floor: f64,
    grade: f64,
    main: bool,
    bounds: ((f64, f64), (f64, f64)),
}

impl Course {
    fn lay(line: &[(f64, f64)], floor: f64, grade: f64, main: bool) -> Course {
        let pts = smooth_open(line, 8);
        let mut run = vec![0.0];
        for w in pts.windows(2) {
            run.push(run.last().unwrap() + dist(w[0], w[1]));
        }
        let fold = |f: fn(f64, f64) -> f64, start: f64| {
            (
                pts.iter().map(|p| p.0).fold(start, f),
                pts.iter().map(|p| p.1).fold(start, f),
            )
        };
        let bounds = (
            fold(f64::min, f64::INFINITY),
            fold(f64::max, f64::NEG_INFINITY),
        );
        Course {
            pts,
            run,
            floor,
            grade,
            main,
            bounds,
        }
    }

    fn length(&self) -> f64 {
        *self.run.last().unwrap()
    }

    /// Metres from the course to `p` (negative on its right, going up it),
    /// and how far along it the nearest point lies; nothing if `p` is more
    /// than `reach` outside its box.
    fn near(&self, p: (f64, f64), reach: f64) -> Option<(f64, f64)> {
        let ((x0, y0), (x1, y1)) = self.bounds;
        if p.0 < x0 - reach || p.0 > x1 + reach || p.1 < y0 - reach || p.1 > y1 + reach {
            return None;
        }
        let (mut best, mut at) = (f64::INFINITY, 0.0);
        for (i, w) in self.pts.windows(2).enumerate() {
            let (d, t) = super::bays::segment(p, w[0], w[1]);
            if d < best.abs() {
                let cross = (w[1].0 - w[0].0) * (p.1 - w[0].1) - (w[1].1 - w[0].1) * (p.0 - w[0].0);
                best = if cross < 0.0 { -d } else { d };
                at = self.run[i] + t * (self.run[i + 1] - self.run[i]);
            }
        }
        Some((best, at))
    }
}

/// What the layout works out once, at set-up.
#[derive(Default)]
pub(super) struct Frostline {
    /// The two oceans' coasts, rounded.
    north: Vec<(f64, f64)>,
    south: Vec<(f64, f64)>,
    /// The benches' outlines, rounded: the basin and the plateau.
    basin: Vec<(f64, f64)>,
    plateau: Vec<(f64, f64)>,
    /// The canyon, then its side canyons.
    canyons: Vec<Course>,
    /// What water did to the open country: gullies and fans (`erode_frostline`).
    gullies: super::alpine::Erosion,
    /// Pools cut off from the oceans, filled (`shape::pool_fill`).
    pools: super::alpine::Erosion,
    /// How far each side's woods are drawn in (west, east), as a rise of the
    /// field value they begin at, so both have the same timber.
    thin: [f64; 2],
}

/// The wall's x at `y`.
fn wall_x(y: f64) -> f64 {
    let i = WALL
        .windows(2)
        .position(|w| y <= w[1].1)
        .unwrap_or(WALL.len() - 2);
    let (a, b) = (WALL[i], WALL[i + 1]);
    a.0 + (b.0 - a.0) * ((y - a.1) / (b.1 - a.1)).clamp(0.0, 1.0)
}

/// Metres east of the wall (negative: west, in the desert).
pub(super) fn east_of(x: f64, y: f64) -> f64 {
    x - wall_x(y)
}

impl Terrain {
    /// Glacier ice and lying snow, east of the wall only: snow on everything
    /// above the low country and in drifts across it, ice on the high
    /// shoulders, the frozen river in the gorge and the frozen lake.
    pub(super) fn frostline_snow(&self, x: f64, y: f64, h: f64, gx: f64, gy: f64) -> (f64, f64) {
        // Up to the wall itself: the renderer cuts the climates on the line.
        let east = smoothstep(-48.0, -16.0, east_of(x, y));
        if east <= 0.0 || h < 1.0 {
            return (0.0, 0.0);
        }
        let slope = (gx * gx + gy * gy).sqrt();
        // Slopes that face north (downhill toward +y) keep their snow lower.
        let north = (-gy / slope.max(1e-3)) * smoothstep(0.05, 0.3, slope);
        let line =
            UPLAND + 24.0 + 12.0 * self.mtn_mask.get(x / 1_400.0, y / 1_400.0) - 20.0 * north;
        let high = smoothstep(line - 24.0, line + 36.0, h);
        // Drifts lying over the lower ground in broad patches, more of them
        // on the upland than down by the sea.
        let drift = self.tilt.fbm(x / 620.0 + 77.0, y / 620.0 - 31.0, 3, 0.5)
            + 0.4 * self.detail.fbm(x / 160.0 - 12.0, y / 160.0 + 40.0, 2, 0.5);
        let reach = 0.16 - 0.2 * smoothstep(LOW + 4.0, UPLAND - 4.0, h);
        let low = 0.62 * smoothstep(reach, reach + 0.2, drift) * smoothstep(5.0, 11.0, h);
        let snow = high.max(low) * (1.0 - smoothstep(0.75, 1.5, slope));
        // An ice cap on the highest gentle ground. (Not on the faces: glacier
        // ice painted on a steep, broken face reads as blue tiles.)
        let cap = smoothstep(
            300.0,
            400.0,
            h + 70.0 * self.mtn_mask.fbm(x / 700.0 + 5.0, y / 700.0, 2, 0.5),
        ) * (1.0 - smoothstep(0.25, 0.5, slope));
        // The frozen river, lake, ponds and streams: on the level only.
        let floor = self.fl_frozen(x, y) * (1.0 - smoothstep(0.12, 0.3, slope));
        (east * cap.max(floor), east * snow * (1.0 - floor))
    }

    /// How thickly trees grow: each side's own woods, the richer side's
    /// drawn in until both have the same timber (it is income). Nothing
    /// grows on the mountains.
    pub(super) fn frostline_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let l = self.l_forest;
        let east = east_of(x, y) > 0.0;
        let thin = self.frost.thin[east as usize];
        let broad = self.forest.fbm(x / l, y / l, 3, 0.5) - thin;
        let copse = self
            .forest
            .fbm(x / (0.16 * l) + 71.3, y / (0.16 * l) - 19.1, 2, 0.5)
            - thin;
        let clearing = self
            .forest
            .fbm(x / (0.09 * l) - 33.7, y / (0.09 * l) + 57.2, 2, 0.5);
        let clearing = smoothstep(0.30, 0.55, clearing);
        let mut habitable = smoothstep(5.0, 10.0, height) * (1.0 - smoothstep(0.30, 0.5, slope));
        let density = if east {
            // Spruce in the low country and over the upland, thinning out
            // toward the plateau's height; none on the ice.
            let forest = smoothstep(self.forest_edge - 0.06, self.forest_edge + 0.16, broad);
            habitable *= 1.0 - smoothstep(UPLAND + 14.0, UPLAND + 44.0, height);
            if habitable > 0.0 {
                habitable *= 1.0 - self.fl_frozen(x, y);
            }
            (forest * (1.0 - 0.85 * clearing)).max(smoothstep(0.42, 0.62, copse) * 0.75)
        } else {
            // Pinyon and juniper in open stands on the upland and the
            // plateau, little in the basin; cottonwoods down the canyon.
            let forest = smoothstep(self.forest_edge + 0.1, self.forest_edge + 0.32, broad);
            habitable *= 1.0 - smoothstep(150.0, 185.0, height);
            let stand = (forest * (1.0 - 0.85 * clearing)).max(smoothstep(0.5, 0.7, copse) * 0.7)
                * (0.35 + 0.65 * smoothstep(LOW + 6.0, UPLAND - 6.0, height));
            let gallery = self.fl_canyon_floor(x, y) * smoothstep(0.2, 0.5, clearing + copse);
            stand.max(0.8 * gallery)
        };
        // A glade round every ore field.
        for f in &self.ore {
            let d = dist((x, y), (f.x, f.y)) - f.radius;
            if d < 200.0 {
                habitable *= smoothstep(70.0, 200.0, d);
            }
        }
        (density * habitable, height)
    }

    /// Each climate's trees (`forest_density` hands the height over in the
    /// conifer slot): juniper and pinyon west of the wall with cottonwood
    /// down in the canyon, spruce and pine east of it with birch by the water.
    pub(super) fn frostline_tree(&self, x: f64, y: f64, height: f64, hash: u64) -> PropKind {
        let (dead, pick) = (unit(hash, 40), unit(hash, 48));
        if east_of(x, y) < 0.0 {
            return match () {
                _ if dead < 0.05 => PropKind::TreeDead,
                _ if self.fl_canyon_floor(x, y) > 0.3 => PropKind::TreeCottonwood,
                _ if pick < smoothstep(40.0, 100.0, height) * 0.6 => PropKind::TreePinyon,
                _ => PropKind::TreeJuniper,
            };
        }
        let pines = self.forest_kind.get(x / 420.0 + 11.0, y / 420.0 - 5.0) > 0.1;
        match () {
            _ if dead < 0.03 => PropKind::TreeDead,
            _ if pick < 0.25 * (1.0 - smoothstep(16.0, 26.0, height)) => PropKind::TreeBroadleaf,
            _ if pines => PropKind::TreePine,
            _ => PropKind::TreeConifer,
        }
    }

    /// Draws the richer side's woods in until both sides hold the same
    /// timber: fewer and smaller woods, their trees as tall as before.
    fn fl_even_woods(&mut self) {
        let n = 400;
        let mut samples: [Vec<(f64, f64, f64, f64)>; 2] = [Vec::new(), Vec::new()];
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (
                    (i as f64 + 0.5) * self.size_x / n as f64,
                    (j as f64 + 0.5) * self.size_y / n as f64,
                );
                let h = self.height(x, y);
                if h > 1.5 {
                    samples[(east_of(x, y) > 0.0) as usize].push((x, y, h, self.slope(x, y)));
                }
            }
        }
        // A wood's timber by how thickly it grows: a tree's bulk goes with
        // the cube of its scale, and trees stand taller in a wood's heart
        // (`tile_props`).
        let timber = |t: &Terrain, side: usize| {
            samples[side]
                .iter()
                .map(|&(x, y, h, s)| {
                    let d = t.forest_density(x, y, h, s).0;
                    d * (0.875 + 0.4 * d).powi(3)
                })
                .sum::<f64>()
        };
        self.frost.thin = [0.0, 0.0];
        let have = [timber(self, 0), timber(self, 1)];
        let rich = (have[1] > have[0]) as usize;
        let (mut lo, mut hi) = (0.0, 0.6);
        for _ in 0..14 {
            self.frost.thin[rich] = 0.5 * (lo + hi);
            if timber(self, rich) > have[1 - rich] {
                lo = self.frost.thin[rich];
            } else {
                hi = self.frost.thin[rich];
            }
        }
    }

    /// The wall as built: a tower at every corner of its line and others
    /// between them about [`TOWER_PITCH`] apart, wherever the line runs
    /// through open sea, each standing on the sea floor as it is (no ground
    /// is made for it: its foot goes down into the water). Over the bridge and
    /// the islands nothing is built: the wall there is the line of light the
    /// terrain shader draws on it (`wall_seam`). Laid from the middle outward,
    /// the south the north turned. Map metres.
    pub(super) fn machine_wall(&self) -> Machine<'_> {
        let mut m = Machine::new(self, 44.0);
        let mid = (self.size_x / 2.0, self.size_y / 2.0);
        let deep = |p: (f64, f64)| self.natural(p.0, p.1) < -TOWER_DEPTH;
        // The line north of the middle, corner to corner.
        let first = WALL.iter().position(|p| p.1 > mid.1).unwrap();
        let mut from = mid;
        for (k, &to) in WALL[first..].iter().enumerate() {
            let last = first + k == WALL.len() - 1;
            let len = dist(from, to);
            let heading = (to.1 - from.1).atan2(to.0 - from.0);
            let count = (len / TOWER_PITCH).round().max(1.0) as usize;
            // The last stretch runs off the map: no tower on its edge.
            let parts = if last { count + 1 } else { count };
            for i in 1..=count {
                let t = i as f64 / parts as f64;
                let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                // Both of a pair or neither.
                if deep(at) && deep(self.turned(at)) {
                    for p in [at, self.turned(at)] {
                        m.put(PropKind::PrecursorTower, p, heading, 1.0);
                    }
                }
            }
            from = to;
        }
        m
    }

    pub(super) fn setup_frostline(&mut self) {
        let start_core = (0.02 * self.size).clamp(200.0, 700.0);
        self.start_outer = 2.0 * start_core;
        self.forest_edge = 0.0;
        let main = Course::lay(CANYON, CANYON_MOUTH, CANYON_GRADE, true);
        let mut canyons = Vec::new();
        for line in SIDE_CANYONS {
            // From the main floor where it joins.
            let (_, along) = main
                .near(line[0], 0.0)
                .expect("a side canyon joins the canyon");
            let floor = CANYON_MOUTH + CANYON_GRADE * along;
            canyons.push(Course::lay(line, floor, SIDE_GRADE, false));
        }
        canyons.insert(0, main);
        self.frost = Frostline {
            north: smooth_closed(NORTH, 4),
            south: smooth_closed(SOUTH, 4),
            basin: smooth_closed(BASIN, 4),
            plateau: smooth_closed(PLATEAU, 5),
            canyons,
            gullies: super::alpine::Erosion::default(),
            pools: super::alpine::Erosion::default(),
            thin: [0.0, 0.0],
        };
        self.erode_frostline();

        let west: Vec<(f64, f64)> = STARTS.iter().map(|&s| self.snap(on_map(s))).collect();
        let mut pads = Vec::new();
        for &at in &west {
            // Both of a pair at one height: the west's.
            let height = self.natural(at.0, at.1).max(12.0);
            for p in [at, self.turned(at)] {
                pads.push(Pad {
                    x: p.0,
                    y: p.1,
                    core: start_core,
                    outer: 2.0 * start_core,
                    height,
                });
            }
        }
        self.pads = pads;
        // Each side's in turn, the twin next to it: skirmish's "Two Sides"
        // (teams by alternate slot) puts one side on each landmass.
        self.starts = west.iter().flat_map(|&s| [s, self.turned(s)]).collect();

        // Three small fields round each base, inside its pad: one behind
        // (away from the middle), two on the forward flanks.
        let mut sites = Vec::new();
        let middle = (self.size_x / 2.0, self.size_y / 2.0);
        for &a in &west {
            let back = if a.1 == middle.1 {
                PI
            } else {
                (a.1 - middle.1).atan2(a.0 - middle.0)
            };
            for turn in [0.0, PI - 1.15, PI + 1.15] {
                let (s, c) = (back + turn).sin_cos();
                let d = 0.8 * start_core;
                sites.push(((a.0 + c * d, a.1 + s * d), (0.2 * start_core).max(55.0)));
            }
        }
        sites.extend(
            ORE.iter()
                .chain(ISLE_ORE)
                .map(|&(x, y, r)| (on_map((x, y)), r)),
        );
        let g = BUILD_CELL_M as f64;
        let snap = |p: (f64, f64)| ((p.0 / g).round() * g, (p.1 / g).round() * g);
        let mut fields = Vec::new();
        for (p, r) in sites {
            let p = snap(p);
            for q in [self.turned(p), p] {
                fields.push(self.ore_field(q.0, q.1, r));
            }
        }
        self.ore = fields;
        self.lay_machine();
        self.fit_forests();
        self.fl_even_woods();
    }
}
