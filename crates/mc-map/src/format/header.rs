//! The header (`format.rs` gives its layout) and the content id.

use super::*;
use crate::deck::{Deck, MAX_DECKS};

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
    decks: &[Deck],
    tints: &[[u8; 3]],
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
                w.heading.0 as u64
                    | (w.bank as u16 as u64) << 16
                    | (w.materials_milli as u64) << 32,
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
    // And the decks.
    if !decks.is_empty() {
        h.write_u64(decks.len() as u64);
        for d in decks {
            d.hash(&mut h);
        }
    }
    if !tints.is_empty() {
        h.write_u64(tints.len() as u64);
        for rgb in tints {
            h.write_u8s(rgb);
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
    put(112, &layout.ore_corners.to_le_bytes());
    put(116, &(info.name.len() as u32).to_le_bytes());
    put(120, info.name.as_bytes());
    put(184, &layout.ore_regions.to_le_bytes());
    put(188, &layout.snow_offset.to_le_bytes());
    put(196, &layout.wrecks_offset.to_le_bytes());
    put(204, &layout.wreck_count.to_le_bytes());
    put(208, &layout.ways_offset.to_le_bytes());
    put(216, &layout.streets_offset.to_le_bytes());
    put(224, &layout.decks_offset.to_le_bytes());
    put(232, &layout.deck_count.to_le_bytes());
    put(236, &layout.tints_offset.to_le_bytes());
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
    let decks_offset = r.u64()?;
    let deck_count = r.u32()?;
    let tints_offset = r.u64()?;
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
    if deck_count as usize > MAX_DECKS {
        return Err(MapError::Corrupt("deck count"));
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
        decks_offset,
        deck_count,
        tints_offset,
    };
    Ok((info, content_id, layout))
}
