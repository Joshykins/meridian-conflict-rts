//! Authoring copy of a baked map. Shared tile edges have one canonical sample,
//! so a brush can cross a tile boundary without opening a crack.
use crate::{
    encode_tile, Deck, MapError, MapFile, MapInfo, MapWreck, MapWriter, OreRegion, Prop,
    TILE_CELLS, TILE_SAMPLES, TILE_SAMPLE_COUNT,
};
use mc_core::FxVec2;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub struct EditMap {
    pub info: MapInfo,
    samples: Vec<u16>,
    pub props: Vec<Prop>,
    pub tints: Vec<[u8; 3]>,
    pub starts: Vec<FxVec2>,
    pub ore: Vec<OreRegion>,
    pub snow: Vec<u8>,
    pub ways: Vec<u8>,
    pub streets: Vec<u8>,
    pub wrecks: Vec<MapWreck>,
    pub decks: Vec<Deck>,
}

impl EditMap {
    pub fn load(file: &MapFile) -> Result<Self, MapError> {
        let info = file.info().clone();
        let (w, h) = info.size_cells();
        let stride = (w + 1) as usize;
        let mut samples = vec![0; stride * (h + 1) as usize];
        for ty in 0..info.tiles_h {
            for tx in 0..info.tiles_w {
                let tile = file.read_tile(tx, ty)?;
                for y in 0..TILE_SAMPLES {
                    for x in 0..TILE_SAMPLES {
                        let dst = ((ty * TILE_CELLS + y) * (w + 1) + tx * TILE_CELLS + x) as usize;
                        let value = tile[(y * TILE_SAMPLES + x) as usize];
                        // A corrupt seam must not silently change on the first save.
                        if ((tx > 0 && x == 0) || (ty > 0 && y == 0)) && samples[dst] != value {
                            return Err(MapError::Corrupt("neighbouring height tiles disagree"));
                        }
                        samples[dst] = value;
                    }
                }
            }
        }
        Ok(Self {
            info,
            samples,
            props: file.props().to_vec(),
            tints: file.prop_tints().to_vec(),
            starts: file.start_positions().to_vec(),
            ore: file.ore_regions().to_vec(),
            snow: file.snow().unwrap_or_default().to_vec(),
            ways: file.ways().unwrap_or_default().to_vec(),
            streets: file.streets().unwrap_or_default().to_vec(),
            wrecks: file.wrecks().to_vec(),
            decks: file.decks().to_vec(),
        })
    }

    pub fn samples(&self) -> &[u16] {
        &self.samples
    }

    /// Replace one canonical sample, returning its previous value for undo.
    pub fn set_sample(&mut self, x: u32, y: u32, value: u16) -> Result<u16, MapError> {
        let (w, h) = self.info.size_cells();
        if x > w || y > h {
            return Err(MapError::Invalid("height sample outside map".into()));
        }
        Ok(std::mem::replace(
            &mut self.samples[(y * (w + 1) + x) as usize],
            value,
        ))
    }

    pub fn tile(&self, tx: u32, ty: u32) -> Result<Vec<u16>, MapError> {
        if tx >= self.info.tiles_w || ty >= self.info.tiles_h {
            return Err(MapError::TileOutOfRange { tx, ty });
        }
        let stride = self.info.size_cells().0 as usize + 1;
        let mut tile = Vec::with_capacity(TILE_SAMPLE_COUNT);
        for y in 0..TILE_SAMPLES as usize {
            let at = (ty as usize * TILE_CELLS as usize + y) * stride
                + tx as usize * TILE_CELLS as usize;
            tile.extend_from_slice(&self.samples[at..at + TILE_SAMPLES as usize]);
        }
        Ok(tile)
    }

    /// Write and validate a complete replacement before publishing it. A failed
    /// write leaves the previous map intact. The temporary directory is private
    /// to this save, even when several editors share the destination directory.
    pub fn save(&self, path: &Path) -> Result<u64, MapError> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        self.info.validate()?;
        let (w, h) = self.info.size_cells();
        if self.samples.len() != ((w + 1) * (h + 1)) as usize {
            return Err(MapError::Invalid(
                "resize requires resampling the document".into(),
            ));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let dir = parent.join(format!(
            ".mc-save-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir)?;
        let tmp = dir.join("terrain.mcmap");
        let result = self.write(&tmp).and_then(|id| {
            MapFile::open(&tmp)?.verify()?;
            std::fs::rename(&tmp, path)?;
            Ok(id)
        });
        if tmp.exists() {
            let _ = std::fs::remove_file(&tmp);
        }
        let _ = std::fs::remove_dir(&dir);
        result
    }

    fn write(&self, path: &Path) -> Result<u64, MapError> {
        let mut writer = MapWriter::create(path, self.info.clone())?;
        if !self.snow.is_empty() {
            writer.set_snow(self.snow.clone())?;
        }
        if !self.ways.is_empty() {
            writer.set_ways(self.ways.clone())?;
        }
        if !self.streets.is_empty() {
            writer.set_streets(self.streets.clone())?;
        }
        writer.set_wrecks(self.wrecks.clone())?;
        writer.set_decks(self.decks.clone())?;
        writer.set_prop_tints(self.tints.clone())?;
        for ty in 0..self.info.tiles_h {
            for tx in 0..self.info.tiles_w {
                writer.push_tile(&encode_tile(&self.tile(tx, ty)?))?;
            }
        }
        writer.finish(self.props.clone(), &self.starts, &self.ore)
    }
}

#[cfg(test)]
mod tests;
