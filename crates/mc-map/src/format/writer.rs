//! Writing a map: [`MapWriter`] streams tiles to disk, then the rest of the
//! sections and the header (the layout is in `format.rs`).

use super::{
    content_id, hash_samples, samples_to_bytes, write_header, write_wreck, DirEntry, EncodedTile,
    Layout, MapError, MapInfo, MapWreck, OreRegion, Prop, DIR_ENTRY_LEN, HEADER_LEN,
    MAX_MAP_WRECKS, MAX_ORE_CORNERS, MAX_ORE_REGIONS, MAX_WRECK_KEY, TILE_OVERVIEW_SAMPLES,
    WRECK_RECORD_LEN,
};
use crate::{MapFile, MAX_PROPS, MAX_START_POSITIONS};
use mc_core::{Fx, FxVec2};
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

/// Streams a map to disk: tiles first, in row-major order, then everything
/// else in [`MapWriter::finish`]. Only the overview is held in memory.
pub struct MapWriter {
    out: BufWriter<File>,
    info: MapInfo,
    dir: Vec<DirEntry>,
    tile_hashes: Vec<u64>,
    overview: Vec<u16>,
    snow: Vec<u8>,
    wrecks: Vec<MapWreck>,
    ways: Vec<u8>,
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
            snow: Vec::new(),
            wrecks: Vec::new(),
            ways: Vec::new(),
            pos: reserved as u64,
            info,
        })
    }

    pub fn info(&self) -> &MapInfo {
        &self.info
    }

    /// Gives the map a snow layer: `(ice, snow)` byte pairs, row-major,
    /// [`MapInfo::snow_dims`] in size.
    pub fn set_snow(&mut self, layer: Vec<u8>) -> Result<(), MapError> {
        let (w, h) = self.info.snow_dims();
        if layer.len() != (w * h * 2) as usize {
            return Err(MapError::Invalid(format!(
                "a snow layer of {} bytes; this map's is {}",
                layer.len(),
                w * h * 2
            )));
        }
        self.snow = layer;
        Ok(())
    }

    /// Gives the map a ways layer: `(wear, cos 2a, sin 2a)` byte triples on
    /// the snow layer's grid ([`MapInfo::snow_dims`]), row-major.
    pub fn set_ways(&mut self, layer: Vec<u8>) -> Result<(), MapError> {
        let (w, h) = self.info.snow_dims();
        if layer.len() != (w * h * 3) as usize {
            return Err(MapError::Invalid(format!(
                "a ways layer of {} bytes; this map's is {}",
                layer.len(),
                w * h * 3
            )));
        }
        self.ways = layer;
        Ok(())
    }

    /// Carries over a map's renderer layers (snow, ways) when rewriting it.
    pub fn keep_layers(&mut self, file: &MapFile) -> Result<(), MapError> {
        if let Some(snow) = file.snow() {
            self.set_snow(snow.to_vec())?;
        }
        if let Some(ways) = file.ways() {
            self.set_ways(ways.to_vec())?;
        }
        Ok(())
    }

    /// Gives the map wreckage to start with. Each must lie inside the map and
    /// name a key of 1 to [`MAX_WRECK_KEY`] bytes.
    pub fn set_wrecks(&mut self, wrecks: Vec<MapWreck>) -> Result<(), MapError> {
        if wrecks.len() > MAX_MAP_WRECKS {
            return Err(MapError::Invalid(format!(
                "{} wrecks; the limit is {MAX_MAP_WRECKS}",
                wrecks.len()
            )));
        }
        let size = self.info.size_metres();
        for w in &wrecks {
            let inside = w.pos.x >= Fx::ZERO
                && w.pos.y >= Fx::ZERO
                && w.pos.x <= size.x
                && w.pos.y <= size.y;
            let key = w.blueprint.len();
            if !inside || key == 0 || key > MAX_WRECK_KEY || w.blueprint.contains('\0') {
                return Err(MapError::Invalid(format!(
                    "wreck {} at {:?}",
                    w.blueprint, w.pos
                )));
            }
        }
        self.wrecks = wrecks;
        Ok(())
    }

    /// Tiles pushed so far; the next one is `(n % tiles_w, n / tiles_w)`.
    pub fn tiles_written(&self) -> usize {
        self.dir.len()
    }

    pub fn push_tile(&mut self, tile: &EncodedTile) -> Result<(), MapError> {
        let index = self.dir.len();
        if index >= self.info.tile_count() {
            return Err(MapError::Invalid(
                "more tiles pushed than the map holds".into(),
            ));
        }
        let (tx, ty) = (
            index % self.info.tiles_w as usize,
            index / self.info.tiles_w as usize,
        );
        let ow = self.info.overview_dims().0 as usize;
        let per_tile = TILE_OVERVIEW_SAMPLES - 1;
        for (row, src) in tile
            .overview
            .as_chunks::<TILE_OVERVIEW_SAMPLES>()
            .0
            .iter()
            .enumerate()
        {
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
    /// Props are reordered by tile (stably). Start positions and ore corners
    /// must lie inside the map; each ore region needs 3 to [`MAX_ORE_CORNERS`] corners.
    pub fn finish(
        mut self,
        mut props: Vec<Prop>,
        starts: &[FxVec2],
        ore: &[OreRegion],
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
            return Err(MapError::Invalid(format!(
                "more than {MAX_START_POSITIONS} start positions"
            )));
        }
        if props.len() > MAX_PROPS {
            return Err(MapError::Invalid(format!(
                "{} props; the limit is {MAX_PROPS}",
                props.len()
            )));
        }
        let size = info.size_metres();
        let inside =
            |p: &FxVec2| p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y;
        let corners = || ore.iter().flat_map(|r| r.points.iter());
        if !starts.iter().chain(corners()).all(inside) || !props.iter().all(|p| inside(&p.pos)) {
            return Err(MapError::Invalid(
                "a prop or marker lies outside the map".into(),
            ));
        }
        if ore.len() > MAX_ORE_REGIONS
            || ore
                .iter()
                .any(|r| r.points.len() < 3 || r.points.len() > MAX_ORE_CORNERS)
        {
            return Err(MapError::Invalid(format!(
                "at most {MAX_ORE_REGIONS} ore regions of 3 to {MAX_ORE_CORNERS} corners"
            )));
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
            ore_corners: corners().count() as u32,
            ore_regions: ore.len() as u32,
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
            buf.extend_from_slice(&p.wear_milli.to_le_bytes());
            buf.extend_from_slice(&p.pos.x.0.to_le_bytes());
            buf.extend_from_slice(&p.pos.y.0.to_le_bytes());
        }
        layout.markers_offset = layout.props_offset + buf.len() as u64;
        for v in starts.iter().chain(corners()) {
            buf.extend_from_slice(&v.x.0.to_le_bytes());
            buf.extend_from_slice(&v.y.0.to_le_bytes());
        }
        for r in ore {
            buf.extend_from_slice(&(r.points.len() as u32).to_le_bytes());
        }
        self.out.write_all(&buf)?;
        // `buf` holds the props and the markers.
        let mut at = layout.props_offset + buf.len() as u64;
        if !self.snow.is_empty() {
            layout.snow_offset = at;
            self.out.write_all(&self.snow)?;
            at += self.snow.len() as u64;
        }
        if !self.wrecks.is_empty() {
            let mut bytes = Vec::with_capacity(self.wrecks.len() * WRECK_RECORD_LEN);
            for w in &self.wrecks {
                write_wreck(w, &mut bytes);
            }
            layout.wrecks_offset = at;
            layout.wreck_count = self.wrecks.len() as u32;
            self.out.write_all(&bytes)?;
            at += bytes.len() as u64;
        }
        if !self.ways.is_empty() {
            layout.ways_offset = at;
            self.out.write_all(&self.ways)?;
        }

        let id = content_id(
            &info,
            &self.tile_hashes,
            hash_samples(&self.overview),
            &props,
            starts,
            ore,
            &self.snow,
            &self.wrecks,
            &self.ways,
        );
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
