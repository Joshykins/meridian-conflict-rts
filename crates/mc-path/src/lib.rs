//! Hierarchical flow fields over a sparse nav grid.
//!
//! Three levels: cells (8 m) carry per-layer clearance, sectors (32 x 32 cells)
//! carry portals and an abstract graph, and a flow field is a corridor of
//! per-sector tiles shared by every unit heading for the same goal cell.
//!
//! Everything the sim can observe is a pure function of the call sequence:
//! background builds run against a copy-on-write snapshot of the grid taken
//! when they are scheduled, and their results are adopted on a tick fixed at
//! that same moment (see `nav`).

mod field;
mod graph;
mod grid;
mod nav;
mod wire;

pub use field::BuildStats;
pub use grid::{NavGrid, Touched};
pub use nav::{FieldId, InlineSpawner, Nav, NavConfig, NavStats, Sample, Spawner, ThreadSpawner};

use mc_core::{Fx, FxVec2};

/// Path cell edge in metres.
pub const CELL_SIZE: i32 = 8;
/// Structure rects align to this many path cells. The 12 m build grid is not
/// an integer number of 8 m path cells; blocking uses the lot interior.
pub const BUILD_CELLS: i32 = 1;
/// Sector edge in path cells.
pub const SECTOR_CELLS: i32 = 32;
/// Largest map edge in path cells (80 km).
pub const MAX_MAP_CELLS: i32 = 10_240;
/// Number of size classes. Class `s` needs a clear square of `s + 1` cells.
/// Class 5 (a 48 m square) is for the capital ships.
pub const SIZE_CLASSES: u8 = 6;

/// Terrain class bits, one byte per cell, supplied by the map.
pub mod terrain {
    /// Dry ground.
    pub const LAND: u8 = 1;
    /// Water too shallow for ships.
    pub const SHALLOW: u8 = 2;
    /// Water deep enough for ships.
    pub const DEEP: u8 = 4;
    /// Slope too steep to drive on (also applies to the seabed for amphibious units).
    pub const STEEP: u8 = 8;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum MoveLayer {
    Land = 0,
    /// Drives on land and on the seabed.
    Amphibious = 1,
    /// Deep water only.
    Naval = 2,
    /// Floats over water, so underwater slopes do not matter.
    Hover = 3,
}

impl MoveLayer {
    pub const ALL: [MoveLayer; 4] = [
        MoveLayer::Land,
        MoveLayer::Amphibious,
        MoveLayer::Naval,
        MoveLayer::Hover,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether terrain class `class` can be crossed on this layer.
    #[inline]
    pub fn passable(self, class: u8) -> bool {
        use terrain::*;
        let steep = class & STEEP != 0;
        match self {
            MoveLayer::Land => class & LAND != 0 && !steep,
            MoveLayer::Amphibious => class & (LAND | SHALLOW | DEEP) != 0 && !steep,
            MoveLayer::Naval => class & DEEP != 0,
            MoveLayer::Hover => class & (SHALLOW | DEEP) != 0 || (class & LAND != 0 && !steep),
        }
    }
}

/// Unit footprint class, `0..SIZE_CLASSES`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct SizeClass(u8);

impl SizeClass {
    pub const SMALL: SizeClass = SizeClass(0);
    pub const MEDIUM: SizeClass = SizeClass(1);
    pub const LARGE: SizeClass = SizeClass(2);
    pub const HUGE: SizeClass = SizeClass(3);

    pub fn new(class: u8) -> Result<SizeClass, PathError> {
        if class < SIZE_CLASSES {
            Ok(SizeClass(class))
        } else {
            Err(PathError::BadSizeClass)
        }
    }

    #[inline]
    pub fn index(self) -> u8 {
        self.0
    }

    /// Edge of the clear square this class needs, in cells.
    #[inline]
    pub fn cells(self) -> u8 {
        self.0 + 1
    }
}

/// Path cell coordinate. May lie outside the map; queries treat that as impassable.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
}

impl Cell {
    #[inline]
    pub const fn new(x: i32, y: i32) -> Cell {
        Cell { x, y }
    }

    /// Cell containing a world position.
    #[inline]
    pub fn from_pos(pos: FxVec2) -> Cell {
        // 8 m cells: shifting the raw value keeps floor semantics for negatives.
        Cell {
            x: (pos.x.0 >> (Fx::FRAC_BITS + 3)) as i32,
            y: (pos.y.0 >> (Fx::FRAC_BITS + 3)) as i32,
        }
    }

    /// World position of the cell centre.
    #[inline]
    pub fn center(self) -> FxVec2 {
        FxVec2::from_ints(
            self.x * CELL_SIZE + CELL_SIZE / 2,
            self.y * CELL_SIZE + CELL_SIZE / 2,
        )
    }
}

/// Half-open cell rectangle `[min, max)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CellRect {
    pub min: Cell,
    pub max: Cell,
}

impl CellRect {
    #[inline]
    pub const fn new(min: Cell, max: Cell) -> CellRect {
        CellRect { min, max }
    }
}

/// Every limit and misuse surfaces as one of these; nothing is dropped silently.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathError {
    /// Map edge is zero, not a multiple of the sector size, or above `MAX_MAP_CELLS`.
    BadMapSize,
    BadSizeClass,
    /// Rect is empty, outside the map or not aligned to the build grid.
    BadRect,
    OutOfMap,
    GoalImpassable,
    /// `extend` from a position the unit cannot stand on.
    Impassable,
    /// Stale or never-issued `FieldId`.
    InvalidField,
    /// `NavConfig::max_fields` live fields and none is evictable.
    TooManyFields,
    /// `NavConfig::max_anchors` corridor anchors on one field.
    TooManyAnchors,
    /// Corridor needs more than `NavConfig::max_tiles_per_field` sectors.
    CorridorTooLarge,
    /// All fields together exceed `NavConfig::max_total_tiles` and nothing is evictable.
    TileBudget,
    /// Abstract search expanded more than `NavConfig::max_search_nodes` nodes.
    SearchLimit,
    /// `extend` from a cell whose anchor was routed but never integrated
    /// (the fix-up pass cap was hit). Treat the position as unreachable.
    Unresolved,
    /// A background build died. Always a bug.
    BuildPanicked,
    /// `Nav::import_state`: truncated, corrupt, or taken with a different map or config.
    BadSnapshot,
}

impl PathError {
    const ALL: [PathError; 15] = [
        PathError::BadMapSize,
        PathError::BadSizeClass,
        PathError::BadRect,
        PathError::OutOfMap,
        PathError::GoalImpassable,
        PathError::Impassable,
        PathError::InvalidField,
        PathError::TooManyFields,
        PathError::TooManyAnchors,
        PathError::CorridorTooLarge,
        PathError::TileBudget,
        PathError::SearchLimit,
        PathError::Unresolved,
        PathError::BuildPanicked,
        PathError::BadSnapshot,
    ];

    /// Stable wire code (position in `ALL`), independent of declaration order.
    pub(crate) fn code(self) -> u8 {
        Self::ALL
            .iter()
            .position(|&e| e == self)
            .expect("every variant is listed") as u8
    }

    pub(crate) fn from_code(code: u8) -> Option<PathError> {
        Self::ALL.get(code as usize).copied()
    }
}

impl core::fmt::Display for PathError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PathError::Unresolved => write!(
                f,
                "Unresolved: a routed cell was never integrated (walled-off pocket or fix-up cap)"
            ),
            other => core::fmt::Debug::fmt(other, f),
        }
    }
}

impl std::error::Error for PathError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_conversion_floors() {
        assert_eq!(Cell::from_pos(FxVec2::from_ints(7, 8)), Cell::new(0, 1));
        assert_eq!(
            Cell::from_pos(FxVec2::from_ints(-1, 81_919)),
            Cell::new(-1, 10_239)
        );
        assert_eq!(Cell::new(2, 0).center(), FxVec2::from_ints(20, 4));
        assert_eq!(
            Cell::from_pos(Cell::new(123, 4567).center()),
            Cell::new(123, 4567)
        );
    }

    #[test]
    fn layers_read_terrain_bits() {
        use terrain::*;
        assert!(MoveLayer::Land.passable(LAND));
        assert!(!MoveLayer::Land.passable(LAND | STEEP));
        assert!(!MoveLayer::Land.passable(SHALLOW));
        assert!(MoveLayer::Amphibious.passable(DEEP));
        assert!(!MoveLayer::Amphibious.passable(DEEP | STEEP));
        assert!(MoveLayer::Hover.passable(DEEP | STEEP));
        assert!(MoveLayer::Naval.passable(DEEP) && !MoveLayer::Naval.passable(SHALLOW));
        assert!(!MoveLayer::Hover.passable(0));
    }
}
