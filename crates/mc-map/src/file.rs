//! Reading `.mcmap` files.
//!
//! Opening a map reads the small sections (header, directory, overview, props,
//! markers) eagerly. Height tiles are read on demand with positional reads, so
//! one `MapFile` can be shared by the streaming thread, the heightfield loader
//! and anyone else without a lock or a seek cursor to fight over.

use crate::format::{
    self, bytes_to_samples, decode_tile, hash_samples, parse_dir_entry, parse_header, parse_prop,
    DirEntry, MapError, MapInfo, OreRegion, Prop, Reader, DIR_ENTRY_LEN, HEADER_LEN, PROP_RECORD_LEN,
};
use crate::TILE_SAMPLE_COUNT;
use mc_core::{Fx, FxVec2};
use std::fs::File;
use std::io;
use std::path::Path;

pub struct MapFile {
    file: File,
    info: MapInfo,
    content_id: u64,
    dir: Vec<DirEntry>,
    overview: Vec<u16>,
    props: Vec<Prop>,
    starts: Vec<FxVec2>,
    ore: Vec<OreRegion>,
    snow: Vec<u8>,
}

impl MapFile {
    pub fn open(path: impl AsRef<Path>) -> Result<MapFile, MapError> {
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let mut header = [0u8; HEADER_LEN];
        read_exact_at(&file, &mut header, 0).map_err(eof_is_corrupt)?;
        let (info, content_id, layout) = parse_header(&header)?;

        // Sizes come from an untrusted header: check them against the file
        // before allocating for them.
        let section = |offset: u64, len: usize| -> Result<Vec<u8>, MapError> {
            let end = offset.checked_add(len as u64).filter(|&e| e <= file_len);
            end.ok_or(MapError::Corrupt("section runs past the end of the file"))?;
            let mut buf = vec![0u8; len];
            read_exact_at(&file, &mut buf, offset)?;
            Ok(buf)
        };

        let bytes = section(layout.dir_offset, info.tile_count() * DIR_ENTRY_LEN)?;
        let mut r = Reader::new(&bytes);
        let mut dir = Vec::with_capacity(info.tile_count());
        for _ in 0..info.tile_count() {
            let e = parse_dir_entry(&mut r)?;
            let tile_ok = e
                .offset
                .checked_add(e.len as u64)
                .is_some_and(|end| end <= file_len);
            let props_ok = e
                .prop_start
                .checked_add(e.prop_count)
                .is_some_and(|end| end <= layout.prop_count);
            if !tile_ok || !props_ok {
                return Err(MapError::Corrupt("tile directory entry out of range"));
            }
            dir.push(e);
        }

        let (ow, oh) = info.overview_dims();
        let bytes = section(layout.overview_offset, (ow * oh) as usize * 2)?;
        let mut overview = vec![0u16; (ow * oh) as usize];
        bytes_to_samples(&bytes, &mut overview);

        let bytes = section(
            layout.props_offset,
            layout.prop_count as usize * PROP_RECORD_LEN,
        )?;
        let mut r = Reader::new(&bytes);
        let props = (0..layout.prop_count)
            .map(|_| parse_prop(&mut r))
            .collect::<Result<Vec<_>, _>>()?;

        let markers = (layout.start_count + layout.ore_corners) as usize;
        let regions = layout.ore_regions as usize;
        let bytes = section(layout.markers_offset, markers * 16 + regions * 4)?;
        let mut r = Reader::new(&bytes);
        let mut points = Vec::with_capacity(markers);
        for _ in 0..markers {
            points.push(FxVec2::new(Fx(r.i64()?), Fx(r.i64()?)));
        }
        let mut corners = points.split_off(layout.start_count as usize).into_iter();
        let mut ore = Vec::with_capacity(regions);
        let mut left = layout.ore_corners as usize;
        for _ in 0..regions {
            let n = r.u32()? as usize;
            if !(3..=format::MAX_ORE_CORNERS).contains(&n) || n > left {
                return Err(MapError::Corrupt("ore region corner count"));
            }
            left -= n;
            ore.push(OreRegion {
                points: corners.by_ref().take(n).collect(),
            });
        }
        if left != 0 {
            return Err(MapError::Corrupt("ore corners left over"));
        }

        let snow = match layout.snow_offset {
            0 => Vec::new(),
            at => {
                let (w, h) = info.snow_dims();
                section(at, (w * h * 2) as usize)?
            }
        };

        Ok(MapFile {
            file,
            info,
            content_id,
            dir,
            overview,
            props,
            starts: points,
            ore,
            snow,
        })
    }

    #[inline]
    pub fn info(&self) -> &MapInfo {
        &self.info
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Identifies the map content. Replays and netplay compare it so everyone
    /// simulates on the same terrain.
    #[inline]
    pub fn content_id(&self) -> u64 {
        self.content_id
    }

    #[inline]
    pub fn size_tiles(&self) -> (u32, u32) {
        (self.info.tiles_w, self.info.tiles_h)
    }

    /// The 257 x 257 samples of one tile, row-major. Safe to call from any
    /// number of threads at once.
    pub fn read_tile(&self, tx: u32, ty: u32) -> Result<Vec<u16>, MapError> {
        let mut out = vec![0u16; TILE_SAMPLE_COUNT];
        self.read_tile_into(tx, ty, &mut out)?;
        Ok(out)
    }

    /// As [`MapFile::read_tile`], into a caller-owned buffer of 257 x 257 samples.
    pub fn read_tile_into(&self, tx: u32, ty: u32, out: &mut [u16]) -> Result<(), MapError> {
        let e = self.entry(tx, ty)?;
        let mut bytes = vec![0u8; e.len as usize];
        read_exact_at(&self.file, &mut bytes, e.offset).map_err(eof_is_corrupt)?;
        decode_tile(&bytes, out)
    }

    /// Lowest and highest sample in a tile, without reading it.
    pub fn tile_sample_range(&self, tx: u32, ty: u32) -> Result<(u16, u16), MapError> {
        self.entry(tx, ty).map(|e| (e.min, e.max))
    }

    /// Whole-map heights at every fourth sample, row-major, [`MapFile::overview_dims`] in size.
    #[inline]
    pub fn overview(&self) -> &[u16] {
        &self.overview
    }

    /// Overview samples per row and per column.
    #[inline]
    pub fn overview_dims(&self) -> (u32, u32) {
        self.info.overview_dims()
    }

    /// All props, grouped by tile in row-major tile order.
    #[inline]
    pub fn props(&self) -> &[Prop] {
        &self.props
    }

    /// The props standing on one tile.
    pub fn props_in_tile(&self, tx: u32, ty: u32) -> Result<&[Prop], MapError> {
        let e = self.entry(tx, ty)?;
        Ok(&self.props[e.prop_start as usize..(e.prop_start + e.prop_count) as usize])
    }

    #[inline]
    pub fn start_positions(&self) -> &[FxVec2] {
        &self.starts
    }

    /// Ore fields. Core mines produce more the more of them lies in their reach.
    #[inline]
    pub fn ore_regions(&self) -> &[OreRegion] {
        &self.ore
    }

    /// Glacier ice and lying snow as `(ice, snow)` byte pairs, row-major,
    /// [`MapInfo::snow_dims`] in size; `None` for a map without them. For
    /// the renderer only.
    #[inline]
    pub fn snow(&self) -> Option<&[u8]> {
        (!self.snow.is_empty()).then_some(&self.snow[..])
    }

    /// Reads every tile and recomputes the content id. Slow (the whole file);
    /// meant for tools and for checking a download, not for every match start.
    pub fn verify(&self) -> Result<(), MapError> {
        let mut samples = vec![0u16; TILE_SAMPLE_COUNT];
        let mut hashes = Vec::with_capacity(self.dir.len());
        for ty in 0..self.info.tiles_h {
            for tx in 0..self.info.tiles_w {
                self.read_tile_into(tx, ty, &mut samples)?;
                hashes.push(hash_samples(&samples));
            }
        }
        let computed = format::content_id(
            &self.info,
            &hashes,
            hash_samples(&self.overview),
            &self.props,
            &self.starts,
            &self.ore,
            &self.snow,
        );
        if computed == self.content_id {
            Ok(())
        } else {
            Err(MapError::ContentIdMismatch {
                header: self.content_id,
                computed,
            })
        }
    }

    fn entry(&self, tx: u32, ty: u32) -> Result<&DirEntry, MapError> {
        if tx >= self.info.tiles_w || ty >= self.info.tiles_h {
            return Err(MapError::TileOutOfRange { tx, ty });
        }
        Ok(&self.dir[(ty * self.info.tiles_w + tx) as usize])
    }
}

fn eof_is_corrupt(e: io::Error) -> MapError {
    if e.kind() == io::ErrorKind::UnexpectedEof {
        MapError::Corrupt("file is truncated")
    } else {
        MapError::Io(e)
    }
}

#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

/// `seek_read` moves the file cursor as a side effect, but the read itself is
/// positional, and nothing here ever reads through the cursor.
#[cfg(windows)]
fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{encode_tile, MapWriter, PropKind};
    use crate::test_util::{baked_4km, temp_path};
    use crate::{TILE_SAMPLES, TILE_SIZE_M};
    use mc_core::Angle;

    fn info(tiles_w: u32, tiles_h: u32) -> MapInfo {
        MapInfo {
            name: "Round Trip".into(),
            tiles_w,
            tiles_h,
            min_z: Fx::from_int(-256),
            z_step: Fx::ratio(1, 64),
            water_level: Fx::from_int(3),
        }
    }

    /// A height that depends only on global sample coordinates, so shared tile edges agree.
    fn synthetic(gx: u32, gy: u32) -> u16 {
        (10_000 + gx * 13 + gy * 7 + (gx * gy) % 97) as u16
    }

    #[test]
    fn snow_layer_round_trips_and_is_optional() {
        let write = |name: &str, snow: Option<Vec<u8>>| {
            let path = temp_path(name);
            let info = info(1, 1);
            let mut w = MapWriter::create(&path, info.clone()).unwrap();
            let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT as u32)
                .map(|i| synthetic(i % TILE_SAMPLES, i / TILE_SAMPLES))
                .collect();
            w.push_tile(&encode_tile(&samples)).unwrap();
            if let Some(snow) = snow {
                w.set_snow(snow).unwrap();
            }
            let id = w.finish(Vec::new(), &[FxVec2::from_ints(512, 512)], &[]).unwrap();
            (MapFile::open(&path).unwrap(), id)
        };
        let (sw, sh) = info(1, 1).snow_dims();
        let layer: Vec<u8> = (0..sw * sh * 2).map(|i| (i * 7 % 251) as u8).collect();
        let (with, id_with) = write("snowy", Some(layer.clone()));
        assert_eq!(with.snow(), Some(&layer[..]));
        assert_eq!(with.content_id(), id_with);
        with.verify().unwrap();
        let (without, id_without) = write("bare", None);
        assert_eq!(without.snow(), None);
        without.verify().unwrap();
        // The layer is part of the content; its absence changes nothing else.
        assert_ne!(id_with, id_without);
        let mut w = MapWriter::create(&temp_path("snow_size"), info(1, 1)).unwrap();
        assert!(w.set_snow(vec![0; 10]).is_err(), "a layer of the wrong size");
    }

    #[test]
    fn hand_written_map_round_trips() {
        let path = temp_path("handwritten");
        let info = info(3, 2);
        let mut w = MapWriter::create(&path, info.clone()).unwrap();
        for ty in 0..2 {
            for tx in 0..3 {
                let samples: Vec<u16> = (0..TILE_SAMPLE_COUNT as u32)
                    .map(|i| synthetic(tx * 256 + i % TILE_SAMPLES, ty * 256 + i / TILE_SAMPLES))
                    .collect();
                w.push_tile(&encode_tile(&samples)).unwrap();
            }
        }
        let prop = |kind, x, y| Prop {
            kind,
            pos: FxVec2::from_ints(x, y),
            heading: Angle(77),
            scale_milli: 1100,
        };
        // Deliberately not in tile order: the writer sorts.
        let props = vec![
            prop(PropKind::BuildingTower, 5000, 3000),
            prop(PropKind::TreePine, 10, 10),
            prop(PropKind::RockLarge, 5100, 3100),
            prop(PropKind::TreeDead, 2100, 100),
        ];
        let starts = [FxVec2::from_ints(512, 512), FxVec2::from_ints(5600, 3584)];
        let ore = [
            OreRegion {
                points: vec![
                    FxVec2::from_ints(400, 400),
                    FxVec2::from_ints(600, 420),
                    FxVec2::from_ints(520, 610),
                ],
            },
            OreRegion {
                points: vec![
                    FxVec2::from_ints(3000, 2000),
                    FxVec2::from_ints(3100, 2000),
                    FxVec2::from_ints(3100, 2100),
                    FxVec2::from_ints(3000, 2100),
                ],
            },
        ];
        let id = w.finish(props, &starts, &ore).unwrap();

        let map = MapFile::open(&path).unwrap();
        assert_eq!(map.info(), &info);
        assert_eq!(map.content_id(), id);
        assert_eq!(map.start_positions(), &starts);
        assert_eq!(map.ore_regions(), &ore);
        assert!(ore[0].contains(FxVec2::from_ints(500, 450)));
        assert!(!ore[0].contains(FxVec2::from_ints(401, 600)));
        assert!(ore[1].contains(FxVec2::from_ints(3050, 2099)));
        assert!(!ore[1].contains(FxVec2::from_ints(3101, 2050)));
        map.verify().unwrap();

        let tile = map.read_tile(2, 1).unwrap();
        assert_eq!(tile[0], synthetic(512, 256));
        assert_eq!(tile[TILE_SAMPLE_COUNT - 1], synthetic(768, 512));
        assert!(matches!(
            map.read_tile(3, 0),
            Err(MapError::TileOutOfRange { .. })
        ));

        let kinds: Vec<_> = map.props().iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            [
                PropKind::TreePine,
                PropKind::TreeDead,
                PropKind::BuildingTower,
                PropKind::RockLarge
            ]
        );
        assert_eq!(map.props_in_tile(2, 1).unwrap().len(), 2);
        assert_eq!(map.props_in_tile(1, 0).unwrap()[0].kind, PropKind::TreeDead);
        assert!(map.props_in_tile(0, 1).unwrap().is_empty());
        assert_eq!(map.props()[0].heading, Angle(77));

        let (ow, oh) = map.overview_dims();
        assert_eq!((ow, oh), (193, 129));
        for (ox, oy) in [(0, 0), (64, 64), (192, 128), (65, 3)] {
            assert_eq!(
                map.overview()[(oy * ow + ox) as usize],
                synthetic(ox * 4, oy * 4)
            );
        }
        let (lo, hi) = map.tile_sample_range(0, 0).unwrap();
        assert_eq!(lo, 10_000);
        assert!(hi >= synthetic(256, 256) - 97);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn writer_rejects_bad_input() {
        let path = temp_path("rejects");
        assert!(MapWriter::create(&path, info(0, 1)).is_err());
        assert!(MapWriter::create(&path, info(41, 1)).is_err());
        let flat = encode_tile(&vec![20_000u16; TILE_SAMPLE_COUNT]);

        let w = MapWriter::create(&path, info(1, 1)).unwrap();
        assert!(w.finish(Vec::new(), &[], &[]).is_err(), "missing tiles");

        let mut w = MapWriter::create(&path, info(1, 1)).unwrap();
        w.push_tile(&flat).unwrap();
        assert!(w.push_tile(&flat).is_err(), "too many tiles");
        assert!(
            w.finish(
                Vec::new(),
                &[],
                &[OreRegion {
                    points: vec![FxVec2::from_ints(100, 96), FxVec2::from_ints(200, 96)]
                }]
            )
            .is_err(),
            "two-corner ore region"
        );

        let mut w = MapWriter::create(&path, info(1, 1)).unwrap();
        w.push_tile(&flat).unwrap();
        assert!(
            w.finish(Vec::new(), &[FxVec2::from_ints(TILE_SIZE_M + 1, 0)], &[])
                .is_err(),
            "outside"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn damaged_files_are_rejected() {
        let good = std::fs::read(baked_4km()).unwrap();
        let path = temp_path("damaged");

        std::fs::write(&path, &good[..good.len() / 2]).unwrap();
        assert!(matches!(MapFile::open(&path), Err(MapError::Corrupt(_))));

        let mut bad = good.clone();
        bad[0] = b'X';
        std::fs::write(&path, &bad).unwrap();
        assert!(matches!(MapFile::open(&path), Err(MapError::BadMagic)));

        let mut bad = good.clone();
        bad[8] = 99;
        std::fs::write(&path, &bad).unwrap();
        assert!(matches!(
            MapFile::open(&path),
            Err(MapError::UnsupportedVersion(99))
        ));

        // A flipped start-position bit leaves the file readable but changes its content.
        let mut bad = good.clone();
        let ore = MapFile::open(baked_4km()).unwrap().ore_regions().to_vec();
        let corners: usize = ore.iter().map(|r| r.points.len()).sum();
        let last = bad.len() - 1 - 16 * corners - 4 * ore.len();
        bad[last - 7] ^= 1;
        std::fs::write(&path, &bad).unwrap();
        assert!(matches!(
            MapFile::open(&path).unwrap().verify(),
            Err(MapError::ContentIdMismatch { .. })
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn tiles_can_be_streamed_from_many_threads() {
        let map = MapFile::open(baked_4km()).unwrap();
        let reference: Vec<Vec<u16>> = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|&(tx, ty)| map.read_tile(tx, ty).unwrap())
            .collect();
        std::thread::scope(|s| {
            for t in 0..8u32 {
                let (map, reference) = (&map, &reference);
                s.spawn(move || {
                    for i in 0..16 {
                        let k = (t + i) % 4;
                        assert_eq!(map.read_tile(k % 2, k / 2).unwrap(), reference[k as usize]);
                    }
                });
            }
        });
    }
}
