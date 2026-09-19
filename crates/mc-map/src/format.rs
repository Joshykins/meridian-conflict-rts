//! The `.mcmap` container, version 1. Everything is little-endian.
//!
//! ```text
//! header      256 bytes, fixed
//! directory   tiles_w * tiles_h entries of 24 bytes, row-major (ty * tiles_w + tx)
//! tiles       one blob per tile, row-major
//! overview    (w_cells/4 + 1) * (h_cells/4 + 1) raw u16 samples
//! props       prop_count records of 24 bytes, grouped by tile
//! markers     start positions then mass deposits, each (x i64, y i64) raw Fx
//! ```
//!
//! Header:
//!
//! ```text
//!   0  [u8; 8]  magic "MCMAP\0\0\0"
//!   8  u32      version (1)
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
//! 104  u64      markers offset    112  u32  mass count      116  u32  name length
//! 120  [u8; 64] map name, UTF-8, zero padded
//! 184           reserved, zero
//! ```
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
//! Prop record: `kind u16, scale u16 (thousandths), heading u16 (Angle), pad
//! u16, x i64, y i64`.
//!
//! The content id hashes the decoded content (grid parameters, every tile's
//! samples, overview, props, markers), not the file bytes, so it survives a
//! re-encode. The map name is not part of it.

use crate::{
    BUILD_CELL_M, CELL_SIZE_M, MAX_MAP_TILES, MAX_PROPS, MAX_START_POSITIONS, OVERVIEW_STRIDE,
    TILE_CELLS, TILE_SAMPLES, TILE_SAMPLE_COUNT, TILE_SIZE_M,
};
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

pub const MAGIC: [u8; 8] = *b"MCMAP\0\0\0";
pub const VERSION: u32 = 1;
pub const HEADER_LEN: usize = 256;
pub const DIR_ENTRY_LEN: usize = 24;
pub const PROP_RECORD_LEN: usize = 24;
pub const MAX_NAME_LEN: usize = 64;

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
    TileOutOfRange { tx: u32, ty: u32 },
    /// `MapFile::verify` recomputed a different content id than the header's.
    ContentIdMismatch { header: u64, computed: u64 },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::Io(e) => write!(f, "map i/o: {e}"),
            MapError::BadMagic => write!(f, "not a .mcmap file"),
            MapError::UnsupportedVersion(v) => write!(f, "unsupported .mcmap version {v}"),
            MapError::Corrupt(what) => write!(f, "corrupt map: {what}"),
            MapError::Invalid(what) => write!(f, "invalid map: {what}"),
            MapError::TileOutOfRange { tx, ty } => write!(f, "tile ({tx}, {ty}) is outside the map"),
            MapError::ContentIdMismatch { header, computed } => {
                write!(f, "content id mismatch: header {header:016x}, computed {computed:016x}")
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
    RockSmall = 16,
    RockLarge = 17,
    BuildingSmall = 32,
    BuildingMedium = 33,
    BuildingLarge = 34,
    BuildingTower = 35,
}

impl PropKind {
    pub const ALL: [PropKind; 10] = [
        PropKind::TreeBroadleaf,
        PropKind::TreeConifer,
        PropKind::TreePine,
        PropKind::TreeDead,
        PropKind::RockSmall,
        PropKind::RockLarge,
        PropKind::BuildingSmall,
        PropKind::BuildingMedium,
        PropKind::BuildingLarge,
        PropKind::BuildingTower,
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
}

/// A static map object. It has no height: it stands on the terrain at `pos`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prop {
    pub kind: PropKind,
    pub pos: FxVec2,
    pub heading: Angle,
    /// Uniform scale in thousandths; 1000 is the model's authored size.
    pub scale_milli: u16,
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
        FxVec2::from_ints(self.tiles_w as i32 * TILE_SIZE_M, self.tiles_h as i32 * TILE_SIZE_M)
    }

    /// Overview samples per row and per column.
    #[inline]
    pub fn overview_dims(&self) -> (u32, u32) {
        let (w, h) = self.size_cells();
        (w / OVERVIEW_STRIDE + 1, h / OVERVIEW_STRIDE + 1)
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
        let tx = pos.x.floor_int().div_euclid(TILE_SIZE_M).clamp(0, self.tiles_w as i32 - 1);
        let ty = pos.y.floor_int().div_euclid(TILE_SIZE_M).clamp(0, self.tiles_h as i32 - 1);
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
            return Err(MapError::Invalid(format!("map name is longer than {MAX_NAME_LEN} bytes")));
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
    pub mass_count: u32,
}

// ---------------------------------------------------------------------------
// Byte helpers

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], MapError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or(MapError::Corrupt("section is truncated"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    pub fn u16(&mut self) -> Result<u16, MapError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32, MapError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn u64(&mut self) -> Result<u64, MapError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn i64(&mut self) -> Result<i64, MapError> {
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
    for (o, b) in out.iter_mut().zip(bytes.chunks_exact(2)) {
        *o = u16::from_le_bytes([b[0], b[1]]);
    }
}

/// Four samples per hasher word; one word per sample would quadruple the cost
/// of hashing a 100-million-sample map.
pub(crate) fn hash_samples(samples: &[u16]) -> u64 {
    let mut h = StateHasher::new();
    h.write_u64(samples.len() as u64);
    let mut chunks = samples.chunks_exact(4);
    for c in &mut chunks {
        h.write_u64(c[0] as u64 | (c[1] as u64) << 16 | (c[2] as u64) << 32 | (c[3] as u64) << 48);
    }
    for &s in chunks.remainder() {
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
    mass: &[FxVec2],
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
        h.write_u64(p.kind.raw() as u64 | (p.heading.0 as u64) << 16 | (p.scale_milli as u64) << 32);
        h.write_i64(p.pos.x.0);
        h.write_i64(p.pos.y.0);
    }
    for list in [starts, mass] {
        h.write_u64(list.len() as u64);
        for v in list {
            h.write_i64(v.x.0);
            h.write_i64(v.y.0);
        }
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
    put(112, &layout.mass_count.to_le_bytes());
    put(116, &(info.name.len() as u32).to_le_bytes());
    put(120, info.name.as_bytes());
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
    let mass_count = r.u32()?;
    let name_len = r.u32()? as usize;
    if name_len > MAX_NAME_LEN {
        return Err(MapError::Corrupt("name length"));
    }
    let name = std::str::from_utf8(&r.take(MAX_NAME_LEN)?[..name_len])
        .map_err(|_| MapError::Corrupt("name is not UTF-8"))?
        .to_owned();
    let info = MapInfo { name, tiles_w, tiles_h, min_z, z_step, water_level };
    info.validate().map_err(|_| MapError::Corrupt("header grid parameters"))?;
    if prop_count as usize > MAX_PROPS || start_count as usize > MAX_START_POSITIONS {
        return Err(MapError::Corrupt("marker counts"));
    }
    let layout = Layout {
        dir_offset,
        overview_offset,
        props_offset,
        prop_count,
        markers_offset,
        start_count,
        mass_count,
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
    r.u16()?;
    let pos = FxVec2::new(Fx(r.i64()?), Fx(r.i64()?));
    Ok(Prop { kind, pos, heading, scale_milli })
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
    assert_eq!(samples.len(), TILE_SAMPLE_COUNT, "a tile is 257 x 257 samples");
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
    EncodedTile { bytes, min, max, hash: hash_samples(samples), overview }
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
        (true, true) => samples[i - 1].wrapping_add(samples[i - n]).wrapping_sub(samples[i - n - 1]),
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

// ---------------------------------------------------------------------------
// Writer

/// Streams a map to disk: tiles first, in row-major order, then everything
/// else in [`MapWriter::finish`]. Only the overview is held in memory.
pub struct MapWriter {
    out: BufWriter<File>,
    info: MapInfo,
    dir: Vec<DirEntry>,
    tile_hashes: Vec<u64>,
    overview: Vec<u16>,
    pos: u64,
}

impl MapWriter {
    pub fn create(path: &Path, info: MapInfo) -> Result<MapWriter, MapError> {
        info.validate()?;
        let mut out = BufWriter::with_capacity(1 << 20, File::create(path)?);
        // Header and directory are filled in by `finish`, once offsets are known.
        let reserved = HEADER_LEN + info.tile_count() * DIR_ENTRY_LEN;
        out.write_all(&vec![0u8; reserved])?;
        let (ow, oh) = info.overview_dims();
        Ok(MapWriter {
            out,
            dir: Vec::with_capacity(info.tile_count()),
            tile_hashes: Vec::with_capacity(info.tile_count()),
            overview: vec![0; (ow * oh) as usize],
            pos: reserved as u64,
            info,
        })
    }

    pub fn info(&self) -> &MapInfo {
        &self.info
    }

    /// Tiles pushed so far; the next one is `(n % tiles_w, n / tiles_w)`.
    pub fn tiles_written(&self) -> usize {
        self.dir.len()
    }

    pub fn push_tile(&mut self, tile: &EncodedTile) -> Result<(), MapError> {
        let index = self.dir.len();
        if index >= self.info.tile_count() {
            return Err(MapError::Invalid("more tiles pushed than the map holds".into()));
        }
        let (tx, ty) = (index % self.info.tiles_w as usize, index / self.info.tiles_w as usize);
        let ow = self.info.overview_dims().0 as usize;
        let per_tile = TILE_OVERVIEW_SAMPLES - 1;
        for (row, src) in tile.overview.chunks_exact(TILE_OVERVIEW_SAMPLES).enumerate() {
            let at = (ty * per_tile + row) * ow + tx * per_tile;
            self.overview[at..at + TILE_OVERVIEW_SAMPLES].copy_from_slice(src);
        }
        self.out.write_all(&tile.bytes)?;
        self.dir.push(DirEntry {
            offset: self.pos,
            len: tile.bytes.len() as u32,
            min: tile.min,
            max: tile.max,
            prop_start: 0,
            prop_count: 0,
        });
        self.tile_hashes.push(tile.hash);
        self.pos += tile.bytes.len() as u64;
        Ok(())
    }

    /// Writes the remaining sections and the header. Returns the content id.
    ///
    /// Props are reordered by tile (stably). Start positions and mass deposits
    /// must lie inside the map; deposits must sit on build-grid vertices.
    pub fn finish(
        mut self,
        mut props: Vec<Prop>,
        starts: &[FxVec2],
        mass: &[FxVec2],
    ) -> Result<u64, MapError> {
        let info = self.info.clone();
        if self.dir.len() != info.tile_count() {
            return Err(MapError::Invalid(format!(
                "{} of {} tiles written",
                self.dir.len(),
                info.tile_count()
            )));
        }
        if starts.len() > MAX_START_POSITIONS {
            return Err(MapError::Invalid(format!("more than {MAX_START_POSITIONS} start positions")));
        }
        if props.len() > MAX_PROPS {
            return Err(MapError::Invalid(format!("{} props; the limit is {MAX_PROPS}", props.len())));
        }
        let size = info.size_metres();
        let inside = |p: &FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y;
        if !starts.iter().chain(mass).all(inside) || !props.iter().all(|p| inside(&p.pos)) {
            return Err(MapError::Invalid("a prop or marker lies outside the map".into()));
        }
        let grid = Fx::from_int(BUILD_CELL_M).0;
        if mass.iter().any(|p| p.x.0 % grid != 0 || p.y.0 % grid != 0) {
            return Err(MapError::Invalid("mass deposits must sit on build-grid vertices".into()));
        }

        let tile_index = |p: &Prop| {
            let (tx, ty) = info.tile_of(p.pos);
            ty * info.tiles_w + tx
        };
        props.sort_by_key(tile_index);
        for (i, p) in props.iter().enumerate() {
            let entry = &mut self.dir[tile_index(p) as usize];
            if entry.prop_count == 0 {
                entry.prop_start = i as u32;
            }
            entry.prop_count += 1;
        }

        let mut layout = Layout {
            dir_offset: HEADER_LEN as u64,
            overview_offset: self.pos,
            prop_count: props.len() as u32,
            start_count: starts.len() as u32,
            mass_count: mass.len() as u32,
            ..Layout::default()
        };
        let mut buf = Vec::new();
        samples_to_bytes(&self.overview, &mut buf);
        layout.props_offset = layout.overview_offset + buf.len() as u64;
        self.out.write_all(&buf)?;

        buf.clear();
        for p in &props {
            buf.extend_from_slice(&p.kind.raw().to_le_bytes());
            buf.extend_from_slice(&p.scale_milli.to_le_bytes());
            buf.extend_from_slice(&p.heading.0.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&p.pos.x.0.to_le_bytes());
            buf.extend_from_slice(&p.pos.y.0.to_le_bytes());
        }
        layout.markers_offset = layout.props_offset + buf.len() as u64;
        for v in starts.iter().chain(mass) {
            buf.extend_from_slice(&v.x.0.to_le_bytes());
            buf.extend_from_slice(&v.y.0.to_le_bytes());
        }
        self.out.write_all(&buf)?;

        let id = content_id(&info, &self.tile_hashes, hash_samples(&self.overview), &props, starts, mass);
        self.out.seek(SeekFrom::Start(0))?;
        self.out.write_all(&write_header(&info, id, &layout))?;
        buf.clear();
        for e in &self.dir {
            buf.extend_from_slice(&e.offset.to_le_bytes());
            buf.extend_from_slice(&e.len.to_le_bytes());
            buf.extend_from_slice(&e.min.to_le_bytes());
            buf.extend_from_slice(&e.max.to_le_bytes());
            buf.extend_from_slice(&e.prop_start.to_le_bytes());
            buf.extend_from_slice(&e.prop_count.to_le_bytes());
        }
        self.out.write_all(&buf)?;
        self.out.flush()?;
        Ok(id)
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
        let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT).map(|_| rng.next_u32() as u16).collect();
        let tile = round_trip(&samples);
        assert_eq!(tile.bytes[0], 0);
        assert_eq!((tile.min, tile.max), (*samples.iter().min().unwrap(), *samples.iter().max().unwrap()));
    }

    #[test]
    fn extremes_round_trip() {
        let samples: Vec<u16> =
            (0..TILE_SAMPLE_COUNT).map(|i| if i % 3 == 0 { u16::MAX } else { 0 }).collect();
        round_trip(&samples);
        round_trip(&vec![0u16; TILE_SAMPLE_COUNT]);
        assert_eq!(encode_tile(&vec![7u16; TILE_SAMPLE_COUNT]).encoded_len(), 1 + 1033 + 32);
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
            assert_eq!(k.is_tree() as u8 + k.is_rock() as u8 + k.is_building() as u8, 1);
        }
        assert_eq!(PropKind::from_raw(999), None);
    }
}
