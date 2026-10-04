//! The `.mcmap` container, version 2. Everything is little-endian.
//!
//! ```text
//! header      256 bytes, fixed
//! directory   tiles_w * tiles_h entries of 24 bytes, row-major (ty * tiles_w + tx)
//! tiles       one blob per tile, row-major
//! overview    (w_cells/4 + 1) * (h_cells/4 + 1) raw u16 samples
//! props       prop_count records of 24 bytes, grouped by tile
//! markers     start positions, then every ore region's corners, each (x i64, y i64)
//!             raw Fx; then one u32 corner count per ore region
//! snow        optional: (w_cells/2 + 1) * (h_cells/2 + 1) pairs of bytes,
//!             (glacier ice, lying snow) from 0 to 255, row-major
//! wrecks      optional: wreck records
//! ways        optional: three bytes per sample on the snow layer's grid,
//!             row-major: how trodden the ground is (a trail's floor 255),
//!             then the trail's heading as cos 2a and sin 2a (128 = 0), which
//!             blend between samples without a seam where a heading wraps
//! streets     optional: four bytes per height sample (`streets_dims`, 8 m
//!             apart), row-major: [`StreetSample`]s
//! ```
//!
//! Header:
//!
//! ```text
//!   0  [u8; 8]  magic "MCMAP\0\0\0"
//!   8  u32      version (2)
//!  12  u32      header length (256)
//!  16  u32      tiles_w            20  u32  tiles_h
//!  24  u32      cells per tile edge (256)
//!  28  u32      cell size in metres (8)
//!  32  u32      overview stride in cells (4)
//!  36  u32      reserved
//!  40  i64      min_z      raw Fx   | height in metres =
//!  48  i64      z_step     raw Fx   |   min_z + sample * z_step
//!  56  i64      water level, raw Fx
//!  64  u64      content id
//!  72  u64      directory offset
//!  80  u64      overview offset
//!  88  u64      props offset       96  u32  prop count      100  u32  start count
//! 104  u64      markers offset    112  u32  ore corners     116  u32  name length
//! 120  [u8; 64] map name, UTF-8, zero padded
//! 184  u32      ore region count
//! 188  u64      snow offset, 0 when the map has no snow layer
//! 196  u64      wrecks offset, 0 when the map starts with none
//! 204  u32      wreck count
//! 208  u64      ways offset, 0 when the map has no ways layer
//! 216  u64      streets offset, 0 when the map has no streets layer
//! 224           reserved, zero
//! ```
//!
//! The snow layer came after version 2 was fixed, in bytes that were reserved
//! and zero, so every older file reads as a map without one. It is only for
//! the renderer; the simulation never reads it. The wreckage came the same way,
//! and the ways layer (also for the renderer only) after it, and the streets
//! layer (renderer only too) after that.
//!
//! Wreck record (`WRECK_RECORD_LEN`, 72 bytes): `blueprint key [u8; 48]`
//! (UTF-8, zero padded), `x i64, y i64, heading u16 (Angle), bank i16, mass
//! u16 (thousandths of the unit's new wreck), pad u16`. Laid by
//! `wreckage::stamp`; the sim puts them down when the match starts.
//!
//! Directory entry: `offset u64, len u32, min u16, max u16, prop_start u32,
//! prop_count u32`. `min`/`max` bound the tile's samples (culling, bounding
//! boxes); the prop range indexes the props table, which is sorted by tile so
//! the streamer can pick up a tile's props with its heights.
//!
//! Tile blob: one encoding byte, then 257 x 257 samples, row-major, +Y last.
//! Row and column 256 repeat the neighbouring tiles' first row and column.
//!
//! * encoding 0: raw `u16`.
//! * encoding 1: each sample is predicted from its left, upper and upper-left
//!   neighbours (`left + up - upleft`, wrapping), the residual is zigzag coded,
//!   and residuals are bit-packed LSB-first in blocks of 64 as `width u8` plus
//!   `ceil(n * width / 8)` bytes. Lossless; roughly halves natural terrain.
//!
//! Prop record: `kind u16, scale u16 (thousandths), heading u16 (Angle), wear
//! u16 (a city structure's damage before the match, thousandths; zero in older
//! files, where it was padding), x i64, y i64`.
//!
//! The content id hashes the decoded content (grid parameters, every tile's
//! samples, overview, props, markers), not the file bytes, so it survives a
//! re-encode. The map name is not part of it.

use crate::{
    CELL_SIZE_M, MAX_MAP_TILES, MAX_PROPS, MAX_START_POSITIONS, OVERVIEW_STRIDE, TILE_CELLS,
    TILE_SAMPLES, TILE_SAMPLE_COUNT, TILE_SIZE_M,
};
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use std::fmt;
use std::io;

mod writer;
pub use writer::MapWriter;

pub const MAGIC: [u8; 8] = *b"MCMAP\0\0\0";
pub const VERSION: u32 = 2;
/// Most ore regions a map may carry, and most corners one region may have.
pub const MAX_ORE_REGIONS: usize = 4096;
pub const MAX_ORE_CORNERS: usize = 256;
pub const HEADER_LEN: usize = 256;
pub const DIR_ENTRY_LEN: usize = 24;
pub const PROP_RECORD_LEN: usize = 24;
pub const MAX_NAME_LEN: usize = 64;
/// Most wrecks a map may start with.
pub const MAX_MAP_WRECKS: usize = 4096;
/// Longest blueprint key a map wreck may name.
pub const MAX_WRECK_KEY: usize = 48;
pub const WRECK_RECORD_LEN: usize = MAX_WRECK_KEY + 24;
/// The snow layer keeps one sample every this many cells (16 m).
pub const SNOW_STRIDE: u32 = 2;

/// Overview samples along one tile edge, shared edge included.
const TILE_OVERVIEW_SAMPLES: usize = (TILE_CELLS / OVERVIEW_STRIDE) as usize + 1;
const PACK_BLOCK: usize = 64;

#[derive(Debug)]
pub enum MapError {
    Io(io::Error),
    BadMagic,
    UnsupportedVersion(u32),
    /// The file contradicts itself or is truncated.
    Corrupt(&'static str),
    /// The caller asked for something the format cannot hold.
    Invalid(String),
    TileOutOfRange {
        tx: u32,
        ty: u32,
    },
    /// `MapFile::verify` recomputed a different content id than the header's.
    ContentIdMismatch {
        header: u64,
        computed: u64,
    },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::Io(e) => write!(f, "map i/o: {e}"),
            MapError::BadMagic => write!(f, "not a .mcmap file"),
            MapError::UnsupportedVersion(v) => write!(f, "unsupported .mcmap version {v}"),
            MapError::Corrupt(what) => write!(f, "corrupt map: {what}"),
            MapError::Invalid(what) => write!(f, "invalid map: {what}"),
            MapError::TileOutOfRange { tx, ty } => {
                write!(f, "tile ({tx}, {ty}) is outside the map")
            }
            MapError::ContentIdMismatch { header, computed } => {
                write!(
                    f,
                    "content id mismatch: header {header:016x}, computed {computed:016x}"
                )
            }
        }
    }
}

impl std::error::Error for MapError {}

impl From<io::Error> for MapError {
    fn from(e: io::Error) -> Self {
        MapError::Io(e)
    }
}

/// What a prop is. The numeric value is the on-disk `kind`; gaps leave room
/// for more variants per family.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u16)]
pub enum PropKind {
    TreeBroadleaf = 0,
    TreeConifer = 1,
    TreePine = 2,
    TreeDead = 3,
    /// A coconut palm: a slender leaning ringed trunk under a crown of drooping
    /// fronds; ~15 m at scale 1. Tropical maps.
    TreePalm = 4,
    /// A tropical rainforest hardwood: a pale buttressed trunk under a broad,
    /// tiered umbrella canopy; ~22 m at scale 1. Tropical maps.
    TreeJungle = 5,
    /// A Utah juniper: a squat, many-stemmed evergreen, twisted grey trunk,
    /// ragged blue-grey crown; ~6 m at scale 1. Desert maps.
    TreeJuniper = 6,
    /// A pinyon pine: a small rounded dark-green pine on a short trunk; ~9 m at
    /// scale 1. Desert maps.
    TreePinyon = 7,
    /// A Fremont cottonwood: a broad, bright green crown on a pale furrowed
    /// trunk, by water; ~17 m at scale 1. Desert maps.
    TreeCottonwood = 8,
    RockSmall = 16,
    RockLarge = 17,
    /// A block of bedded sandstone fallen from a cliff: flat-topped, layered,
    /// red; ~8 m across at scale 1. Desert maps.
    RockSlab = 18,
    BuildingSmall = 32,
    BuildingMedium = 33,
    BuildingLarge = 34,
    BuildingTower = 35,
    /// Precursor artifacts: the Foundry's own structures, standing where its shell
    /// breaks the surface. Indestructible scenery, far bigger than anything built in a
    /// match; their solid parts block the ground (`solid_plan`). Survival maps.
    /// A faceted obelisk with a capstone hovering over it; ~140 m.
    PrecursorSpire = 48,
    /// Two leaning blades either side of a light core; ~70 m.
    PrecursorPylon = 49,
    /// A gateway over a road running along +x: two legs, the span between them open.
    PrecursorArch = 50,
    /// A colossal ring standing upright across +x, half sunk in the ground.
    PrecursorRing = 51,
    /// A fallen fragment of something vast, lying tilted and half buried.
    PrecursorShard = 52,
    /// A revetment along y, its face at x = 0 looking +x, its body running back
    /// 16 m into the higher ground behind: clads a terrace step or a cut.
    PrecursorWall = 53,
    /// A plinth under a hovering segmented crown and its light column.
    PrecursorBeacon = 54,
    /// A channel of light set flush in the ground along x. Walkable.
    PrecursorConduit = 55,
    /// A small cluster of blocks hanging over a broken stub.
    PrecursorFragment = 56,
    /// The megastructure: one machine per map, laid out on one axis
    /// (`mc-map/src/bake/machine.rs`). A footing 530 m by 360 m: a battered plinth,
    /// a raked body and an open frame on its deck, ~420 m; stands on a bench cut for it.
    PrecursorBastion = 57,
    /// A cantilever: twin girders leaning out along +x from a shoulder block at the
    /// origin, 340 m up at 670 m out. Solid under the shoulder only.
    PrecursorBoom = 58,
    /// A tower into the clouds, 930 m, on a plinth.
    PrecursorTower = 59,
    /// A bridge girder from the origin 1000 m along +x, its deck 84 m over the
    /// bench it leaves from. Overhead only: nothing solid on the ground.
    PrecursorSpan = 60,
    /// The Threshold's connective tissue: 200 m of elevated deck along +x from its
    /// origin, 48-64 m up at scale 1. Overhead only: nothing solid on the ground.
    PrecursorViaduct = 61,
    /// A viaduct's pier: a column to its deck with a hub on top. Solid at its foot.
    PrecursorPier = 62,
    /// The polar map's great vault: an arch from its origin 6000 m along +x to a
    /// second foot at the same level, 1600 m high at the crown, its ribs 58 m either
    /// side of the axis. Solid under its two feet only (`bake/polar.rs`).
    PrecursorVault = 64,
    /// The Axis: a needle 4600 m high at scale 1, its ring floating 1070 m up
    /// (The Axis lays it on dry ground at 0.35). Solid at its 150 m foot.
    PrecursorAxis = 65,
    /// 200 m of stepped face between two of the polar pit's terraces: six 30 m
    /// courses, each 22 m behind the one below, facing +x, the first 50 m behind
    /// the origin (which stands on the terrace below). Solid along its foot.
    PrecursorTerrace = 66,
    /// 200 m of casing down a sheer wall to the sea, hanging from a coping 60 m out
    /// along +x from its origin on the rim, 560 m deep at scale 1. Nothing solid.
    PrecursorLining = 67,
    /// The Threshold's facility kit (`mc-map/src/bake/threshold.rs`, models in
    /// `mc-models/src/precursor_{forge,sky,gate}.rs`). A print hall facing
    /// +x: four bays open on its face at y = -255, -85, 85, 255, recessed 150 m;
    /// survival prints the rounds in them.
    PrecursorForge = 68,
    /// A berth where a row of three Shapers forms, facing +x, at y = -72, 0, 72.
    PrecursorCradle = 69,
    /// The central spire, 2800 m, its Lens hovering 720 m up (the ray's source).
    PrecursorHeart = 70,
    /// A ring 1200 m across floating 1500 m up. Nothing solid.
    PrecursorHalo = 71,
    /// A slab island hovering 760-1000 m up. Nothing solid.
    PrecursorMonolith = 72,
    /// A harbour printing ships: a channel along +x between two moles, three slips.
    PrecursorSeaGate = 73,
    /// A sea platform on four legs; ships pass under its deck.
    PrecursorPlatform = 74,
    /// The great land gate: the road runs along +x between its legs.
    PrecursorGate = 75,
    /// A mast 1800 m high on a plinth.
    PrecursorNeedle = 76,
    /// 240 m of a 90 m plateau face along y, facing +x, its origin at the foot.
    PrecursorRampart = 77,
    /// A 400 m square of the facility's paving, flush on level ground. Walked over.
    PrecursorFloor = 78,
    /// A 200 m length of causeway lying on the seabed along x, 60 m wide and 14 m
    /// high at scale 1: The Axis lays them island to island, scaled to the water
    /// over them. Sailed and dived over.
    PrecursorSeaway = 63,
    /// The Axis's citadel: two blades leaning together to 1600 m off a stepped
    /// plinth 660 by 560 m at scale 1. Solid at its plinth.
    PrecursorCitadel = 79,
    /// Landmarks: great works of the colonists, scenery that belongs to the map.
    /// A colossal straight gravity dam across a gorge, laid to
    /// `crate::landmark::GORGE_DAM`: its origin is the middle of its
    /// downstream toe, the lake toward +x. Solid over its whole footprint.
    Dam = 80,
    /// The dam's switchyard (`crate::landmark::GORGE_YARD`): a fenced yard of
    /// breakers, bus gantries and transformers round its origin, its line
    /// leaving along +x from a dead-end gantry, the first span included.
    /// Solid over the yard.
    DamSwitchyard = 81,
    /// A power line's lattice tower (`crate::landmark::GORGE_LINE`), its foot
    /// at the origin. Solid at its foot.
    DamPylon = 82,
    /// The dam's operations town (`crate::landmark::GORGE_TOWN`): offices,
    /// workers' flats, sheds and a water tower round a square, on a level
    /// lot round its origin. Solid under each building.
    DamTown = 83,
    /// A power line's span, standing with a tower: its wires from the tower's
    /// clamps on along +x to the next tower's, pitched to meet that tower's
    /// ground ([`PropKind::span`]). Overhead only.
    DamSpan = 84,
    /// The city kit (`crate::city`): structures that block the ground, take
    /// hits and come down (`city::structure` holds their numbers).
    /// A detached house on a garden plot, pitched roof; 16 by 12 m.
    CityHouse = 96,
    /// A terrace of four houses; 40 by 12 m.
    CityRowhouses = 97,
    /// A two-storey row of shops under flats; 44 by 18 m.
    CityShops = 98,
    /// A barn and its silo.
    CityFarmstead = 99,
    /// A sawtooth-roofed warehouse; 64 by 36 m.
    CityWarehouse = 100,
    /// A works hall and its stack.
    CityFactory = 101,
    /// Four fuel tanks in their bund.
    CityTankFarm = 102,
    /// A six-storey block on a street front; 48 by 16 m.
    CityTenement = 103,
    /// A perimeter block round a courtyard; 72 m square.
    CityCourtyard = 104,
    /// A fourteen-storey slab of flats.
    CityApartments = 105,
    /// A twelve-storey office block, glass bands.
    CityOffice = 106,
    /// A glass tower on a podium; 132 m.
    CityHighrise = 107,
    /// A setback glass skyscraper on a podium; 212 m.
    CitySkyscraper = 108,
    /// A supertall with a crown; 320 m. A handful per city.
    CitySpire = 109,
    /// A glass slab tower; 110 m.
    CitySlab = 110,
    /// A stone hall of government under a dome.
    CityCivic = 111,
    /// A railway station under an arched glass roof.
    CityStation = 112,
    /// A multi-storey car park.
    CityGarage = 113,
    /// A mall under glass roofs.
    CityMall = 114,
    /// A church, its tower at the west end (-x).
    CityChurch = 115,
    /// A gutted shell of a block, burnt out before the match.
    CityRuin = 116,
    /// A 64 m run of the city's wall along x (`city::WALL_SEGMENT_M`).
    CityWall = 117,
    /// A tower on the wall.
    CityWallTower = 118,
    /// A gatehouse over a road along x: two towers and a bridge between.
    CityGate = 119,
    /// A heap of rubble: walked over, never hit.
    CityRubble = 120,
    /// A street light: a slim mast with two LED arms, ~10 m.
    CityStreetLight = 121,
    /// 64 m of an elevated maglev guideway along x, its deck 14 m up on a pylon at the origin (`city::TRANSIT_SEGMENT_M`).
    CityTransit = 122,
    /// An elevated maglev station along x over an avenue: platforms and canopy on pylons in its median.
    CityTransitStation = 123,
    /// A free-standing LED billboard on a pole.
    CityBillboard = 124,
    /// A civilian car, heading along x; burnt out once worn.
    CityCar = 125,
    /// A 16 m row of anti-tank hedgehogs and sandbags along x. Walked through.
    CityBarricade = 126,
    /// A wind turbine: a tower to its hub ~95 m up, three blades.
    CityWindTurbine = 127,
    /// A field of solar panel rows, 40 by 24 m. Walked through.
    CitySolarArray = 128,
    /// A lattice communications mast, 110 m.
    CityMast = 129,
    /// A square's monument over a fountain basin.
    CityMonument = 130,
}

impl PropKind {
    pub const ALL: [PropKind; 88] = [
        PropKind::TreeBroadleaf,
        PropKind::TreeConifer,
        PropKind::TreePine,
        PropKind::TreeDead,
        PropKind::TreePalm,
        PropKind::TreeJungle,
        PropKind::TreeJuniper,
        PropKind::TreePinyon,
        PropKind::TreeCottonwood,
        PropKind::RockSmall,
        PropKind::RockLarge,
        PropKind::RockSlab,
        PropKind::BuildingSmall,
        PropKind::BuildingMedium,
        PropKind::BuildingLarge,
        PropKind::BuildingTower,
        PropKind::PrecursorSpire,
        PropKind::PrecursorPylon,
        PropKind::PrecursorArch,
        PropKind::PrecursorRing,
        PropKind::PrecursorShard,
        PropKind::PrecursorWall,
        PropKind::PrecursorBeacon,
        PropKind::PrecursorConduit,
        PropKind::PrecursorFragment,
        PropKind::PrecursorBastion,
        PropKind::PrecursorBoom,
        PropKind::PrecursorTower,
        PropKind::PrecursorSpan,
        PropKind::PrecursorVault,
        PropKind::PrecursorAxis,
        PropKind::PrecursorTerrace,
        PropKind::PrecursorLining,
        PropKind::PrecursorForge,
        PropKind::PrecursorCradle,
        PropKind::PrecursorHeart,
        PropKind::PrecursorHalo,
        PropKind::PrecursorMonolith,
        PropKind::PrecursorSeaGate,
        PropKind::PrecursorPlatform,
        PropKind::PrecursorGate,
        PropKind::PrecursorNeedle,
        PropKind::PrecursorRampart,
        PropKind::PrecursorFloor,
        PropKind::PrecursorViaduct,
        PropKind::PrecursorPier,
        PropKind::PrecursorSeaway,
        PropKind::PrecursorCitadel,
        PropKind::Dam,
        PropKind::DamSwitchyard,
        PropKind::DamPylon,
        PropKind::DamTown,
        PropKind::DamSpan,
        PropKind::CityHouse,
        PropKind::CityRowhouses,
        PropKind::CityShops,
        PropKind::CityFarmstead,
        PropKind::CityWarehouse,
        PropKind::CityFactory,
        PropKind::CityTankFarm,
        PropKind::CityTenement,
        PropKind::CityCourtyard,
        PropKind::CityApartments,
        PropKind::CityOffice,
        PropKind::CityHighrise,
        PropKind::CitySkyscraper,
        PropKind::CitySpire,
        PropKind::CitySlab,
        PropKind::CityCivic,
        PropKind::CityStation,
        PropKind::CityGarage,
        PropKind::CityMall,
        PropKind::CityChurch,
        PropKind::CityRuin,
        PropKind::CityWall,
        PropKind::CityWallTower,
        PropKind::CityGate,
        PropKind::CityRubble,
        PropKind::CityStreetLight,
        PropKind::CityTransit,
        PropKind::CityTransitStation,
        PropKind::CityBillboard,
        PropKind::CityCar,
        PropKind::CityBarricade,
        PropKind::CityWindTurbine,
        PropKind::CitySolarArray,
        PropKind::CityMast,
        PropKind::CityMonument,
    ];

    pub fn from_raw(raw: u16) -> Option<PropKind> {
        Self::ALL.iter().copied().find(|k| *k as u16 == raw)
    }

    #[inline]
    pub fn raw(self) -> u16 {
        self as u16
    }

    #[inline]
    pub fn is_tree(self) -> bool {
        (self as u16) < 16
    }

    #[inline]
    pub fn is_rock(self) -> bool {
        (16..32).contains(&(self as u16))
    }

    #[inline]
    pub fn is_building(self) -> bool {
        (32..48).contains(&(self as u16))
    }

    #[inline]
    pub fn is_precursor(self) -> bool {
        (48..80).contains(&(self as u16))
    }

    #[inline]
    pub fn is_landmark(self) -> bool {
        (80..96).contains(&(self as u16))
    }

    /// The city kit: blocks, towers and the wall, which take hits and come
    /// down, and the street furniture round them (`crate::city`).
    #[inline]
    pub fn is_city(self) -> bool {
        (96..160).contains(&(self as u16))
    }

    /// A span of wires strung along +x from its origin to the next tower,
    /// `(length, pivot)`: the renderer pitches it about `pivot` metres over
    /// the ground at its origin until its far end meets the ground `length`
    /// on. `None` for anything else.
    pub fn span(self) -> Option<(f64, f64)> {
        match self {
            PropKind::DamSpan => Some((
                crate::landmark::GORGE_LINE.span,
                crate::landmark::GORGE_LINE.pivot,
            )),
            _ => None,
        }
    }

    /// The solid parts of a precursor artifact's or a landmark's plan at its authored size, as
    /// rectangles in its own frame (x along its heading, y to its left), in
    /// metres: `(centre x, centre y, half x, half y)`. The models in
    /// `mc-models/src/precursor.rs` are built to these. Empty for
    /// anything else, and for a conduit, which lies flush and is walked over.
    pub fn solid_plan(self) -> &'static [(i32, i32, i32, i32)] {
        match self {
            PropKind::PrecursorSpire => &[(0, 0, 14, 14)],
            PropKind::PrecursorPylon => &[(0, 0, 10, 18)],
            PropKind::PrecursorArch => &[(0, 46, 12, 10), (0, -46, 12, 10)],
            PropKind::PrecursorRing => &[(0, 98, 10, 20), (0, -98, 10, 20)],
            PropKind::PrecursorShard => &[(0, 0, 45, 14)],
            PropKind::PrecursorWall => &[(-8, 0, 8, 40)],
            PropKind::PrecursorBeacon => &[(0, 0, 16, 16)],
            PropKind::PrecursorFragment => &[(0, 0, 6, 6)],
            PropKind::PrecursorBastion => &[(0, 0, 266, 181)],
            PropKind::PrecursorBoom => &[(-10, 0, 98, 86)],
            PropKind::PrecursorTower => &[(0, 0, 78, 78)],
            PropKind::PrecursorVault => &[(0, 0, 70, 92), (6_000, 0, 70, 92)],
            PropKind::PrecursorAxis => &[(0, 0, 150, 150)],
            PropKind::PrecursorCitadel => &[(0, 0, 330, 280)],
            PropKind::PrecursorTerrace => &[(-40, 0, 8, 100)],
            PropKind::PrecursorForge => &[
                (-215, 0, 65, 360),
                (-75, -340, 75, 20),
                (-75, -170, 75, 20),
                (-75, 0, 75, 20),
                (-75, 170, 75, 20),
                (-75, 340, 75, 20),
            ],
            PropKind::PrecursorCradle => &[(-80, 0, 16, 170), (0, 150, 36, 26), (0, -150, 36, 26)],
            PropKind::PrecursorHeart => &[(0, 0, 210, 210)],
            PropKind::PrecursorSeaGate => &[
                (-150, 230, 250, 50),
                (-150, -230, 250, 50),
                (-412, 0, 8, 190),
            ],

            PropKind::PrecursorPlatform => &[
                (160, 90, 28, 28),
                (160, -90, 28, 28),
                (-160, 90, 28, 28),
                (-160, -90, 28, 28),
            ],
            PropKind::PrecursorGate => &[(0, 300, 70, 60), (0, -300, 70, 60)],
            PropKind::PrecursorNeedle => &[(0, 0, 44, 44)],
            PropKind::PrecursorPier => &[(0, 0, 16, 16)],
            PropKind::PrecursorRampart => &[(-45, 0, 45, 120)],
            // The gravity dam's footprint, toe to heel and end to end, keys
            // included (`landmark::GORGE_DAM`), and the works at its toe
            // (stilling basin, powerhouses, ship lift) out to 185 m below
            // it: nothing walks through or over them.
            PropKind::Dam => &[(-17, 0, 167, 760)],
            // The yard inside its fence (`landmark::GORGE_YARD`), and a tower's
            // four feet.
            PropKind::DamSwitchyard => &[(0, 0, 120, 75)],
            PropKind::DamPylon => &[(0, 0, 7, 7)],
            // The town's buildings, one lot each (mc-render's `dam_works.rs`
            // builds to these): the office on the square, four blocks of flats,
            // workshop, shed, warehouse, water tower and the monument. Streets
            // at least 24 m wide run between every two.
            PropKind::DamTown => &[
                (0, 70, 30, 26),
                (-115, 38, 45, 9),
                (-115, 92, 45, 9),
                (115, 38, 45, 9),
                (115, 92, 45, 9),
                (-115, -42, 40, 14),
                (-115, -98, 40, 12),
                (110, -42, 40, 14),
                (110, -98, 8, 8),
                (0, -65, 4, 4),
            ],

            _ => match crate::city::structure(self) {
                Some(structure) => structure.plan,
                None => &[],
            },
        }
    }
}

impl Prop {
    /// The path cells (`CELL_SIZE_M`) a precursor artifact's solid parts cover,
    /// as runs `(y, first x, last x)` inside a map `cells` wide and high. A cell
    /// counts when its centre is inside a part. Fixed point throughout, so every
    /// peer blocks the same cells.
    pub fn solid_runs(&self, cells: (u32, u32)) -> Vec<(u32, u32, u32)> {
        let mut runs = Vec::new();
        let plan = self.kind.solid_plan();
        if plan.is_empty() {
            return runs;
        }
        let cell = Fx::from_int(crate::CELL_SIZE_M);
        let scale = Fx::from_int(self.scale_milli as i32) / Fx::from_int(1000);
        let (sin, cos) = (self.heading.sin(), self.heading.cos());
        let mut row: Vec<(u32, u32)> = Vec::new();
        for &(cx, cy, hx, hy) in plan {
            let (cx, cy) = (Fx::from_int(cx) * scale, Fx::from_int(cy) * scale);
            let (hx, hy) = (Fx::from_int(hx) * scale, Fx::from_int(hy) * scale);
            // The part's middle in the world, and a circle round it to search.
            let mid = FxVec2::new(
                self.pos.x + cx * cos - cy * sin,
                self.pos.y + cx * sin + cy * cos,
            );
            let reach = hx + hy;
            let lo = |v: Fx, limit: u32| {
                ((v - reach) / cell).floor_int().clamp(0, limit as i32 - 1) as u32
            };
            let hi = |v: Fx, limit: u32| {
                ((v + reach) / cell).floor_int().clamp(0, limit as i32 - 1) as u32
            };
            for y in lo(mid.y, cells.1)..=hi(mid.y, cells.1) {
                row.clear();
                let mut open: Option<u32> = None;
                for x in lo(mid.x, cells.0)..=hi(mid.x, cells.0) {
                    let dx = Fx::from_int(x as i32) * cell + cell / 2 - mid.x;
                    let dy = Fx::from_int(y as i32) * cell + cell / 2 - mid.y;
                    let (lx, ly) = (dx * cos + dy * sin, dy * cos - dx * sin);
                    let inside = lx.abs() <= hx && ly.abs() <= hy;
                    match (inside, open) {
                        (true, None) => open = Some(x),
                        (false, Some(first)) => {
                            row.push((first, x - 1));
                            open = None;
                        }
                        _ => {}
                    }
                }
                if let Some(first) = open {
                    row.push((first, hi(mid.x, cells.0)));
                }
                runs.extend(row.iter().map(|&(a, b)| (y, a, b)));
            }
        }
        runs
    }
}

/// What a road is, in the streets layer. The numbers are in map files and
/// the terrain shader: never renumber one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Road {
    None = 0,
    /// Asphalt between kerbs, a lane each way.
    Street = 1,
    /// A boulevard: two carriageways either side of a planted median.
    Avenue = 2,
    /// A highway: two carriageways, a crash barrier between, no kerbs.
    Highway = 3,
    /// An unpaved country lane: wheel ruts and a grass crown.
    Lane = 4,
    /// A railway: ballast, sleepers and rails along the line.
    Rail = 5,
}

/// What the ground off the road is, in the streets layer. The numbers are in
/// map files and the terrain shader: never renumber one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Ground {
    /// The terrain's own (grass, earth, rock): outside the city.
    Natural = 0,
    /// Paving slabs: pavements, squares, building lots.
    Paving = 1,
    /// A park's mown lawn and paths.
    Lawn = 2,
    /// Stained concrete hardstanding: works yards, depots.
    Yard = 3,
    /// A rail yard's ballast.
    Ballast = 4,
    /// A ploughed or planted field.
    Field = 5,
    /// Broken ground, brick and concrete rubble: where a block was flattened.
    Rubble = 6,
    /// Bare trodden earth: a vacant lot, a building site.
    Earth = 7,
    /// A spaceport's apron: large poured slabs with markings.
    Apron = 8,
}

/// The edge of the cells [`Ground::Field`] is surveyed into, metres: one
/// field per cell (`bake/siege/fields.rs`), the terrain shader drawing the
/// same cells to lay each field's furrows.
pub const FIELD_CELL_M: f64 = 280.0;

/// One sample of the streets layer: what the ground is at a height sample,
/// for the terrain shader. The offset is signed, so it blends between samples
/// even across a road's centreline, and kerbs and lane lines are drawn from it
/// far finer than the 8 m grid.
///
/// Bytes: offset, half, then the road's kind in the low nibble with the
/// junction flag in the top bit, then the ground's kind in the low nibble
/// with how battered it is in the high nibble.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreetSample {
    /// Offset from the nearest road's centreline, quarter metres, positive to
    /// the left of the way the road was laid, stored plus 128: 128 on the
    /// centreline, 0 and 255 at 32 m or more either side.
    pub offset: u8,
    /// That road's half width from its centreline to the kerb, quarter metres.
    pub half: u8,
    pub road: Road,
    /// Where roads cross: no lane lines are painted through it.
    pub junction: bool,
    pub ground: Ground,
    /// How shelled and worn the ground is, 0 to 15: cracks, debris, scorch.
    pub battered: u8,
}

impl StreetSample {
    /// Off every road, on the terrain's own ground.
    pub const NATURAL: StreetSample = StreetSample {
        offset: 255,
        half: 0,
        road: Road::None,
        junction: false,
        ground: Ground::Natural,
        battered: 0,
    };

    pub fn bytes(self) -> [u8; 4] {
        [
            self.offset,
            self.half,
            self.road as u8 | (self.junction as u8) << 7,
            self.ground as u8 | self.battered.min(15) << 4,
        ]
    }
}

/// A static map object. It has no height: it stands on the terrain at `pos`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prop {
    pub kind: PropKind,
    pub pos: FxVec2,
    pub heading: Angle,
    /// Uniform scale in thousandths; 1000 is the model's authored size.
    pub scale_milli: u16,
    /// A city structure's damage before the match, thousandths of its
    /// health (`crate::city`); 0 for everything else.
    pub wear_milli: u16,
}

/// Wreckage a map starts with: salvage from fighting before the match. It is
/// named by blueprint key so the map does not depend on blueprint ids; the sim
/// resolves it when the match starts and leaves out a key it does not know.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapWreck {
    pub blueprint: String,
    pub pos: FxVec2,
    pub heading: Angle,
    /// Roll it lies at, binary angle steps: a hull on the seabed lists.
    pub bank: i16,
    /// Mass left, in thousandths of what the unit's wreck would hold new.
    pub mass_milli: u16,
}

/// An ore field: a simple polygon of corners in metres, either winding. Ore lies
/// under all of it; a core mine near it draws on the part inside its reach.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct OreRegion {
    pub points: Vec<FxVec2>,
}

impl OreRegion {
    /// Exact even-odd test on the raw fixed-point corners, so every peer agrees.
    pub fn contains(&self, p: FxVec2) -> bool {
        let pts = &self.points;
        let mut inside = false;
        let mut j = pts.len().wrapping_sub(1);
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[j]);
            if (a.y > p.y) != (b.y > p.y) {
                // x of the edge at p.y, compared without dividing:
                // p.x < a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y)
                let dy = b.y.0 as i128 - a.y.0 as i128;
                let lhs = (p.x.0 as i128 - a.x.0 as i128) * dy;
                let rhs = (b.x.0 as i128 - a.x.0 as i128) * (p.y.0 as i128 - a.y.0 as i128);
                if (dy > 0 && lhs < rhs) || (dy < 0 && lhs > rhs) {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }

    /// How deep under the ground the ore lies, metres: 140 to 400, read off
    /// the outline so every peer and the interface agree without storing it.
    pub fn depth(&self) -> Fx {
        let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
        for p in &self.points {
            h = (h ^ p.x.0 as u64).wrapping_mul(0x1000_0000_01B3);
            h = (h ^ p.y.0 as u64).wrapping_mul(0x1000_0000_01B3);
        }
        h ^= h >> 29;
        Fx::from_int(140 + (h % 261) as i32)
    }

    /// The middle of the field: the mean of its corners.
    pub fn centre(&self) -> FxVec2 {
        let n = self.points.len().max(1) as i32;
        let sum = self.points.iter().fold(FxVec2::ZERO, |a, p| a + *p);
        FxVec2::new(sum.x / n, sum.y / n)
    }

    /// Bounding box: (min, max).
    pub fn bounds(&self) -> (FxVec2, FxVec2) {
        let mut lo = self.points.first().copied().unwrap_or(FxVec2::ZERO);
        let mut hi = lo;
        for p in &self.points {
            lo = FxVec2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = FxVec2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        (lo, hi)
    }
}

/// Grid parameters and naming shared by the writer, the reader and the heightfield.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapInfo {
    pub name: String,
    pub tiles_w: u32,
    pub tiles_h: u32,
    /// Height of sample value 0.
    pub min_z: Fx,
    /// Height per sample step.
    pub z_step: Fx,
    pub water_level: Fx,
}

impl MapInfo {
    #[inline]
    pub fn tile_count(&self) -> usize {
        (self.tiles_w * self.tiles_h) as usize
    }

    #[inline]
    pub fn size_cells(&self) -> (u32, u32) {
        (self.tiles_w * TILE_CELLS, self.tiles_h * TILE_CELLS)
    }

    #[inline]
    pub fn size_metres(&self) -> FxVec2 {
        FxVec2::from_ints(
            self.tiles_w as i32 * TILE_SIZE_M,
            self.tiles_h as i32 * TILE_SIZE_M,
        )
    }

    /// Overview samples per row and per column.
    #[inline]
    pub fn overview_dims(&self) -> (u32, u32) {
        let (w, h) = self.size_cells();
        (w / OVERVIEW_STRIDE + 1, h / OVERVIEW_STRIDE + 1)
    }

    /// Streets layer samples per row and per column: one per height sample.
    #[inline]
    pub fn streets_dims(&self) -> (u32, u32) {
        let (w, h) = self.size_cells();
        (w + 1, h + 1)
    }

    /// Snow layer samples per row and per column.
    #[inline]
    pub fn snow_dims(&self) -> (u32, u32) {
        let (w, h) = self.size_cells();
        (w / SNOW_STRIDE + 1, h / SNOW_STRIDE + 1)
    }

    #[inline]
    pub fn sample_to_height(&self, sample: u16) -> Fx {
        Fx(self.min_z.0 + self.z_step.0 * sample as i64)
    }

    /// Nearest sample value, clamped to what a `u16` can hold.
    pub fn height_to_sample(&self, z: Fx) -> u16 {
        let steps = (z.0 - self.min_z.0 + self.z_step.0 / 2).div_euclid(self.z_step.0);
        steps.clamp(0, u16::MAX as i64) as u16
    }

    /// Tile holding `pos`; positions outside the map go to the nearest tile.
    pub fn tile_of(&self, pos: FxVec2) -> (u32, u32) {
        let tx = pos
            .x
            .floor_int()
            .div_euclid(TILE_SIZE_M)
            .clamp(0, self.tiles_w as i32 - 1);
        let ty = pos
            .y
            .floor_int()
            .div_euclid(TILE_SIZE_M)
            .clamp(0, self.tiles_h as i32 - 1);
        (tx as u32, ty as u32)
    }

    pub(crate) fn validate(&self) -> Result<(), MapError> {
        let dims = 1..=MAX_MAP_TILES;
        if !dims.contains(&self.tiles_w) || !dims.contains(&self.tiles_h) {
            return Err(MapError::Invalid(format!(
                "map is {} x {} tiles; each edge must be 1..={MAX_MAP_TILES}",
                self.tiles_w, self.tiles_h
            )));
        }
        if self.name.len() > MAX_NAME_LEN {
            return Err(MapError::Invalid(format!(
                "map name is longer than {MAX_NAME_LEN} bytes"
            )));
        }
        // The upper bound keeps `z_step * sample` products comfortably inside an i64.
        if self.z_step.0 <= 0 || self.z_step.0 > Fx::from_int(16).0 {
            return Err(MapError::Invalid("z_step must be in (0, 16] metres".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct DirEntry {
    pub offset: u64,
    pub len: u32,
    pub min: u16,
    pub max: u16,
    pub prop_start: u32,
    pub prop_count: u32,
}

/// Where the sections sit in the file.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Layout {
    pub dir_offset: u64,
    pub overview_offset: u64,
    pub props_offset: u64,
    pub prop_count: u32,
    pub markers_offset: u64,
    pub start_count: u32,
    pub ore_corners: u32,
    pub ore_regions: u32,
    /// 0: no snow layer.
    pub snow_offset: u64,
    /// 0: no wrecks.
    pub wrecks_offset: u64,
    pub wreck_count: u32,
    /// 0: no ways layer.
    pub ways_offset: u64,
    /// 0: no streets layer.
    pub streets_offset: u64,
}

// ---------------------------------------------------------------------------
// Byte helpers

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], MapError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or(MapError::Corrupt("section is truncated"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, MapError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, MapError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, MapError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub(crate) fn i64(&mut self) -> Result<i64, MapError> {
        Ok(self.u64()? as i64)
    }
}

pub(crate) fn samples_to_bytes(samples: &[u16], out: &mut Vec<u8>) {
    out.reserve(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
}

pub(crate) fn bytes_to_samples(bytes: &[u8], out: &mut [u16]) {
    for (o, &b) in out.iter_mut().zip(bytes.as_chunks::<2>().0) {
        *o = u16::from_le_bytes(b);
    }
}

/// Four samples per hasher word; one word per sample would quadruple the cost
/// of hashing a 100-million-sample map.
pub(crate) fn hash_samples(samples: &[u16]) -> u64 {
    let mut h = StateHasher::new();
    h.write_u64(samples.len() as u64);
    let (chunks, rest) = samples.as_chunks::<4>();
    for c in chunks {
        h.write_u64(c[0] as u64 | (c[1] as u64) << 16 | (c[2] as u64) << 32 | (c[3] as u64) << 48);
    }
    for &s in rest {
        h.write_u64(s as u64);
    }
    h.finish()
}

/// The content id. Reader and writer must feed it identically.
pub(crate) fn content_id(
    info: &MapInfo,
    tile_hashes: &[u64],
    overview_hash: u64,
    props: &[Prop],
    starts: &[FxVec2],
    ore: &[OreRegion],
    snow: &[u8],
    wrecks: &[MapWreck],
    ways: &[u8],
    streets: &[u8],
) -> u64 {
    let mut h = StateHasher::new();
    h.write_u32(VERSION);
    h.write_u32(info.tiles_w);
    h.write_u32(info.tiles_h);
    h.write_u32(TILE_CELLS);
    h.write_u32(CELL_SIZE_M as u32);
    h.write_u32(OVERVIEW_STRIDE);
    h.write_i64(info.min_z.0);
    h.write_i64(info.z_step.0);
    h.write_i64(info.water_level.0);
    h.write_u64s(tile_hashes);
    h.write_u64(overview_hash);
    h.write_u64(props.len() as u64);
    for p in props {
        h.write_u64(
            p.kind.raw() as u64
                | (p.heading.0 as u64) << 16
                | (p.scale_milli as u64) << 32
                | (p.wear_milli as u64) << 48,
        );
        h.write_i64(p.pos.x.0);
        h.write_i64(p.pos.y.0);
    }
    h.write_u64(starts.len() as u64);
    for v in starts {
        h.write_i64(v.x.0);
        h.write_i64(v.y.0);
    }
    h.write_u64(ore.len() as u64);
    for r in ore {
        h.write_u64(r.points.len() as u64);
        for v in &r.points {
            h.write_i64(v.x.0);
            h.write_i64(v.y.0);
        }
    }
    // Only when present, so maps from before the layer keep their ids.
    if !snow.is_empty() {
        h.write_u64(snow.len() as u64);
        h.write_u8s(snow);
    }
    // Likewise the wreckage.
    if !wrecks.is_empty() {
        h.write_u64(wrecks.len() as u64);
        for w in wrecks {
            h.write_u8s(w.blueprint.as_bytes());
            h.write_u64(
                w.heading.0 as u64 | (w.bank as u16 as u64) << 16 | (w.mass_milli as u64) << 32,
            );
            h.write_i64(w.pos.x.0);
            h.write_i64(w.pos.y.0);
        }
    }
    // And the ways layer.
    if !ways.is_empty() {
        h.write_u64(ways.len() as u64);
        h.write_u8s(ways);
    }
    // And the streets layer.
    if !streets.is_empty() {
        h.write_u64(streets.len() as u64);
        h.write_u8s(streets);
    }
    h.finish()
}

// ---------------------------------------------------------------------------
// Header

pub(crate) fn write_header(info: &MapInfo, content_id: u64, layout: &Layout) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    let mut put = |at: usize, bytes: &[u8]| h[at..at + bytes.len()].copy_from_slice(bytes);
    put(0, &MAGIC);
    put(8, &VERSION.to_le_bytes());
    put(12, &(HEADER_LEN as u32).to_le_bytes());
    put(16, &info.tiles_w.to_le_bytes());
    put(20, &info.tiles_h.to_le_bytes());
    put(24, &TILE_CELLS.to_le_bytes());
    put(28, &(CELL_SIZE_M as u32).to_le_bytes());
    put(32, &OVERVIEW_STRIDE.to_le_bytes());
    put(40, &info.min_z.0.to_le_bytes());
    put(48, &info.z_step.0.to_le_bytes());
    put(56, &info.water_level.0.to_le_bytes());
    put(64, &content_id.to_le_bytes());
    put(72, &layout.dir_offset.to_le_bytes());
    put(80, &layout.overview_offset.to_le_bytes());
    put(88, &layout.props_offset.to_le_bytes());
    put(96, &layout.prop_count.to_le_bytes());
    put(100, &layout.start_count.to_le_bytes());
    put(104, &layout.markers_offset.to_le_bytes());
    put(112, &layout.ore_corners.to_le_bytes());
    put(116, &(info.name.len() as u32).to_le_bytes());
    put(120, info.name.as_bytes());
    put(184, &layout.ore_regions.to_le_bytes());
    put(188, &layout.snow_offset.to_le_bytes());
    put(196, &layout.wrecks_offset.to_le_bytes());
    put(204, &layout.wreck_count.to_le_bytes());
    put(208, &layout.ways_offset.to_le_bytes());
    put(216, &layout.streets_offset.to_le_bytes());
    h
}

pub(crate) fn parse_header(bytes: &[u8; HEADER_LEN]) -> Result<(MapInfo, u64, Layout), MapError> {
    let mut r = Reader::new(bytes);
    if r.take(8)? != MAGIC {
        return Err(MapError::BadMagic);
    }
    let version = r.u32()?;
    if version != VERSION {
        return Err(MapError::UnsupportedVersion(version));
    }
    if r.u32()? as usize != HEADER_LEN {
        return Err(MapError::Corrupt("unexpected header length"));
    }
    let (tiles_w, tiles_h) = (r.u32()?, r.u32()?);
    // This build's heightfield math assumes these; a file that disagrees is not ours.
    if r.u32()? != TILE_CELLS || r.u32()? != CELL_SIZE_M as u32 || r.u32()? != OVERVIEW_STRIDE {
        return Err(MapError::Corrupt("unsupported grid constants"));
    }
    r.u32()?;
    let (min_z, z_step, water_level) = (Fx(r.i64()?), Fx(r.i64()?), Fx(r.i64()?));
    let content_id = r.u64()?;
    let dir_offset = r.u64()?;
    let overview_offset = r.u64()?;
    let props_offset = r.u64()?;
    let prop_count = r.u32()?;
    let start_count = r.u32()?;
    let markers_offset = r.u64()?;
    let ore_corners = r.u32()?;
    let name_len = r.u32()? as usize;
    if name_len > MAX_NAME_LEN {
        return Err(MapError::Corrupt("name length"));
    }
    let name = std::str::from_utf8(&r.take(MAX_NAME_LEN)?[..name_len])
        .map_err(|_| MapError::Corrupt("name is not UTF-8"))?
        .to_owned();
    let ore_regions = r.u32()?;
    let snow_offset = r.u64()?;
    let wrecks_offset = r.u64()?;
    let wreck_count = r.u32()?;
    let ways_offset = r.u64()?;
    let streets_offset = r.u64()?;
    let info = MapInfo {
        name,
        tiles_w,
        tiles_h,
        min_z,
        z_step,
        water_level,
    };
    info.validate()
        .map_err(|_| MapError::Corrupt("header grid parameters"))?;
    if prop_count as usize > MAX_PROPS || start_count as usize > MAX_START_POSITIONS {
        return Err(MapError::Corrupt("marker counts"));
    }
    if ore_regions as usize > MAX_ORE_REGIONS
        || ore_corners as usize > MAX_ORE_REGIONS * MAX_ORE_CORNERS
    {
        return Err(MapError::Corrupt("ore counts"));
    }
    if wreck_count as usize > MAX_MAP_WRECKS {
        return Err(MapError::Corrupt("wreck count"));
    }
    let layout = Layout {
        dir_offset,
        overview_offset,
        props_offset,
        prop_count,
        markers_offset,
        start_count,
        ore_corners,
        ore_regions,
        snow_offset,
        wrecks_offset,
        wreck_count,
        ways_offset,
        streets_offset,
    };
    Ok((info, content_id, layout))
}

pub(crate) fn parse_dir_entry(r: &mut Reader<'_>) -> Result<DirEntry, MapError> {
    Ok(DirEntry {
        offset: r.u64()?,
        len: r.u32()?,
        min: r.u16()?,
        max: r.u16()?,
        prop_start: r.u32()?,
        prop_count: r.u32()?,
    })
}

pub(crate) fn parse_prop(r: &mut Reader<'_>) -> Result<Prop, MapError> {
    let kind = PropKind::from_raw(r.u16()?).ok_or(MapError::Corrupt("unknown prop kind"))?;
    let scale_milli = r.u16()?;
    let heading = Angle(r.u16()?);
    let wear_milli = r.u16()?;
    if wear_milli > 1000 {
        return Err(MapError::Corrupt("prop wear over 1000"));
    }
    let pos = FxVec2::new(Fx(r.i64()?), Fx(r.i64()?));
    Ok(Prop {
        kind,
        pos,
        heading,
        scale_milli,
        wear_milli,
    })
}

pub(crate) fn write_wreck(w: &MapWreck, out: &mut Vec<u8>) {
    let mut key = [0u8; MAX_WRECK_KEY];
    key[..w.blueprint.len()].copy_from_slice(w.blueprint.as_bytes());
    out.extend_from_slice(&key);
    out.extend_from_slice(&w.pos.x.0.to_le_bytes());
    out.extend_from_slice(&w.pos.y.0.to_le_bytes());
    out.extend_from_slice(&w.heading.0.to_le_bytes());
    out.extend_from_slice(&w.bank.to_le_bytes());
    out.extend_from_slice(&w.mass_milli.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
}

pub(crate) fn parse_wreck(r: &mut Reader<'_>) -> Result<MapWreck, MapError> {
    let key = r.take(MAX_WRECK_KEY)?;
    let len = key.iter().position(|&b| b == 0).unwrap_or(MAX_WRECK_KEY);
    let blueprint = std::str::from_utf8(&key[..len])
        .map_err(|_| MapError::Corrupt("wreck key is not UTF-8"))?
        .to_owned();
    let pos = FxVec2::new(Fx(r.i64()?), Fx(r.i64()?));
    let heading = Angle(r.u16()?);
    let bank = r.u16()? as i16;
    let mass_milli = r.u16()?;
    r.u16()?;
    Ok(MapWreck {
        blueprint,
        pos,
        heading,
        bank,
        mass_milli,
    })
}

// ---------------------------------------------------------------------------
// Tile codec

/// A tile ready for [`MapWriter::push_tile`]. Encoding is the expensive part of
/// writing a map, so it is a free function that bake workers run in parallel.
#[derive(Clone, Debug)]
pub struct EncodedTile {
    bytes: Vec<u8>,
    min: u16,
    max: u16,
    hash: u64,
    /// Every fourth sample, 65 x 65.
    overview: Vec<u16>,
}

impl EncodedTile {
    /// Size of the blob as stored.
    pub fn encoded_len(&self) -> usize {
        self.bytes.len()
    }
}

/// `samples` is 257 x 257, row-major.
pub fn encode_tile(samples: &[u16]) -> EncodedTile {
    assert_eq!(
        samples.len(),
        TILE_SAMPLE_COUNT,
        "a tile is 257 x 257 samples"
    );
    let n = TILE_SAMPLES as usize;
    let (mut min, mut max) = (u16::MAX, 0u16);
    for &s in samples {
        min = min.min(s);
        max = max.max(s);
    }
    let mut overview = Vec::with_capacity(TILE_OVERVIEW_SAMPLES * TILE_OVERVIEW_SAMPLES);
    for y in (0..n).step_by(OVERVIEW_STRIDE as usize) {
        for x in (0..n).step_by(OVERVIEW_STRIDE as usize) {
            overview.push(samples[y * n + x]);
        }
    }

    let mut bytes = Vec::with_capacity(TILE_SAMPLE_COUNT);
    bytes.push(1u8);
    let mut block = [0u16; PACK_BLOCK];
    let mut filled = 0;
    for i in 0..samples.len() {
        let residual = samples[i].wrapping_sub(predict(samples, i, n)) as i16;
        block[filled] = ((residual << 1) ^ (residual >> 15)) as u16;
        filled += 1;
        if filled == PACK_BLOCK || i + 1 == samples.len() {
            pack_block(&block[..filled], &mut bytes);
            filled = 0;
        }
    }
    if bytes.len() > 1 + TILE_SAMPLE_COUNT * 2 {
        bytes.clear();
        bytes.push(0u8);
        samples_to_bytes(samples, &mut bytes);
    }
    EncodedTile {
        bytes,
        min,
        max,
        hash: hash_samples(samples),
        overview,
    }
}

/// Decodes a tile blob into `out` (257 x 257).
pub(crate) fn decode_tile(bytes: &[u8], out: &mut [u16]) -> Result<(), MapError> {
    assert_eq!(out.len(), TILE_SAMPLE_COUNT, "a tile is 257 x 257 samples");
    let n = TILE_SAMPLES as usize;
    let (&encoding, payload) = bytes.split_first().ok_or(MapError::Corrupt("empty tile"))?;
    match encoding {
        0 => {
            if payload.len() != TILE_SAMPLE_COUNT * 2 {
                return Err(MapError::Corrupt("raw tile size"));
            }
            bytes_to_samples(payload, out);
            Ok(())
        }
        1 => {
            let mut r = Reader::new(payload);
            let mut i = 0;
            while i < out.len() {
                let count = PACK_BLOCK.min(out.len() - i);
                let width = r.take(1)?[0] as u32;
                if width > 16 {
                    return Err(MapError::Corrupt("tile bit width"));
                }
                let packed = r.take((count * width as usize).div_ceil(8))?;
                let (mut acc, mut bits, mut at) = (0u64, 0u32, 0usize);
                for _ in 0..count {
                    while bits < width {
                        acc |= (packed[at] as u64) << bits;
                        at += 1;
                        bits += 8;
                    }
                    let zz = (acc & ((1u64 << width) - 1)) as u16;
                    acc >>= width;
                    bits -= width;
                    let residual = (zz >> 1) ^ (zz & 1).wrapping_neg();
                    out[i] = predict(out, i, n).wrapping_add(residual);
                    i += 1;
                }
            }
            Ok(())
        }
        _ => Err(MapError::Corrupt("unknown tile encoding")),
    }
}

/// Plane predictor over already-known neighbours. Terrain is locally planar,
/// so the residual is a second difference and stays small even on slopes.
#[inline]
fn predict(samples: &[u16], i: usize, n: usize) -> u16 {
    let (x, y) = (i % n, i / n);
    match (x > 0, y > 0) {
        (false, false) => 0,
        (true, false) => samples[i - 1],
        (false, true) => samples[i - n],
        (true, true) => samples[i - 1]
            .wrapping_add(samples[i - n])
            .wrapping_sub(samples[i - n - 1]),
    }
}

fn pack_block(values: &[u16], out: &mut Vec<u8>) {
    let width = 16 - values.iter().fold(0u16, |a, &v| a | v).leading_zeros();
    out.push(width as u8);
    let (mut acc, mut bits) = (0u64, 0u32);
    for &v in values {
        acc |= (v as u64) << bits;
        bits += width;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    }
    if bits > 0 {
        out.push(acc as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::Rng;

    fn round_trip(samples: &[u16]) -> EncodedTile {
        let tile = encode_tile(samples);
        let mut back = vec![0u16; TILE_SAMPLE_COUNT];
        decode_tile(&tile.bytes, &mut back).unwrap();
        assert_eq!(samples, &back[..]);
        tile
    }

    #[test]
    fn smooth_tiles_compress_and_round_trip() {
        let n = TILE_SAMPLES as usize;
        let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT)
            .map(|i| {
                let (x, y) = ((i % n) as i64, (i / n) as i64);
                (20_000 + 40 * x - 25 * y + (x * x + y * y) / 50) as u16
            })
            .collect();
        let tile = round_trip(&samples);
        assert_eq!(tile.bytes[0], 1);
        assert!(tile.encoded_len() < TILE_SAMPLE_COUNT);
        assert_eq!(tile.overview.len(), 65 * 65);
        assert_eq!(tile.overview[64], samples[256]);
        assert_eq!(tile.overview[65 * 64], samples[256 * n]);
    }

    #[test]
    fn noisy_tiles_fall_back_to_raw() {
        let mut rng = Rng::new(3);
        let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT)
            .map(|_| rng.next_u32() as u16)
            .collect();
        let tile = round_trip(&samples);
        assert_eq!(tile.bytes[0], 0);
        assert_eq!(
            (tile.min, tile.max),
            (
                *samples.iter().min().unwrap(),
                *samples.iter().max().unwrap()
            )
        );
    }

    #[test]
    fn extremes_round_trip() {
        let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT)
            .map(|i| if i % 3 == 0 { u16::MAX } else { 0 })
            .collect();
        round_trip(&samples);
        round_trip(&vec![0u16; TILE_SAMPLE_COUNT]);
        assert_eq!(
            encode_tile(&vec![7u16; TILE_SAMPLE_COUNT]).encoded_len(),
            1 + 1033 + 32
        );
    }

    #[test]
    fn truncated_tile_is_an_error() {
        let tile = encode_tile(&vec![1234u16; TILE_SAMPLE_COUNT]);
        let mut out = vec![0u16; TILE_SAMPLE_COUNT];
        assert!(decode_tile(&tile.bytes[..tile.bytes.len() - 1], &mut out).is_err());
        assert!(decode_tile(&[9], &mut out).is_err());
    }

    #[test]
    fn height_encoding_round_trips() {
        let info = MapInfo {
            name: "t".into(),
            tiles_w: 1,
            tiles_h: 1,
            min_z: Fx::from_int(-256),
            z_step: Fx::ratio(1, 64),
            water_level: Fx::ZERO,
        };
        for s in [0u16, 1, 16_384, 40_000, u16::MAX] {
            assert_eq!(info.height_to_sample(info.sample_to_height(s)), s);
        }
        assert_eq!(info.sample_to_height(16_384), Fx::ZERO);
        assert_eq!(info.height_to_sample(Fx::from_int(-1000)), 0);
        assert_eq!(info.height_to_sample(Fx::from_int(5000)), u16::MAX);
        assert_eq!(info.tile_of(FxVec2::from_ints(-5, 99_999)), (0, 0));
    }

    #[test]
    fn prop_kinds_round_trip() {
        for k in PropKind::ALL {
            assert_eq!(PropKind::from_raw(k.raw()), Some(k));
            assert_eq!(
                k.is_tree() as u8
                    + k.is_rock() as u8
                    + k.is_building() as u8
                    + k.is_precursor() as u8
                    + k.is_landmark() as u8
                    + k.is_city() as u8,
                1
            );
        }
        assert_eq!(PropKind::from_raw(999), None);
    }
}
