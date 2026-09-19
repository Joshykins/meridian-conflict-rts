//! Baked maps: the `.mcmap` file format, tile streaming, the sim-side
//! heightfield and the procedural baker.
//!
//! The terrain is one regular grid of `u16` height samples, 8 m apart. The
//! renderer streams it tile by tile ([`MapFile::read_tile`]); the simulation
//! loads all of it into a [`Heightfield`]. Both interpret the samples the same
//! way (same height encoding, same two-triangle split per cell), which is what
//! makes units and projectiles collide with exactly the surface on screen.
//!
//! Everything the simulation calls ([`Heightfield`], [`FlattenRecord`]) is
//! integer-only. Floats appear only in the baker ([`mod@bake`]), whose output is
//! distributed as a file and identified by its content id.
//!
//! The file layout is documented in [`mod@format`].

pub mod bake;
pub mod file;
pub mod format;
pub mod heightfield;
mod noise;
#[cfg(test)]
mod test_util;

pub use bake::{bake, BakeParams, BakeReport, Layout};
pub use file::MapFile;
pub use format::{encode_tile, EncodedTile, MapError, MapInfo, MapWriter, Prop, PropKind};
pub use heightfield::{FlattenRecord, Heightfield, RAYCAST_MAX_LENGTH_M, RAYCAST_MAX_STEPS};

use mc_core::Fx;

/// Height/path cell edge in metres.
pub const CELL_SIZE_M: i32 = 8;
/// Cells along one edge of a map tile.
pub const TILE_CELLS: u32 = 256;
/// Samples along one edge of a tile. One more than the cells: the last row and
/// column repeat the neighbour's first, so a tile can be meshed on its own.
pub const TILE_SAMPLES: u32 = TILE_CELLS + 1;
/// Samples in one tile.
pub const TILE_SAMPLE_COUNT: usize = (TILE_SAMPLES * TILE_SAMPLES) as usize;
/// Tile edge in metres.
pub const TILE_SIZE_M: i32 = TILE_CELLS as i32 * CELL_SIZE_M;
/// Build cell edge in metres. Structures and mass deposits snap to it.
pub const BUILD_CELL_M: i32 = 16;
/// The overview keeps every fourth sample (32 m).
pub const OVERVIEW_STRIDE: u32 = 4;
/// Largest map edge in tiles (80 km).
pub const MAX_MAP_TILES: u32 = 40;
/// Most start positions a map may carry.
pub const MAX_START_POSITIONS: usize = mc_core::MAX_PLAYERS;
/// Most props a map may carry. Exceeding it is a bake/load error.
pub const MAX_PROPS: usize = 4_000_000;
/// Height encoding the baker uses: samples span -256 m to +768 m in steps of
/// 1/64 m. Both are exact in `Fx`, so decoding a sample never rounds.
pub const DEFAULT_MIN_Z: Fx = Fx::from_int(-256);
pub const DEFAULT_Z_STEP: Fx = Fx::ratio(1, 64);
