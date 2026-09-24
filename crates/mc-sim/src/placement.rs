//! Where a structure may stand as far as the map goes: its ground, its water
//! and the city blocks on it. The same rules as [`World::can_place`], from
//! the map alone, so the placement preview can say no before an engineer
//! walks all the way there to find out.
//!
//! What it leaves out is other structures: those are the player's to see
//! (fog), and the preview checks them against what it is shown.
//!
//! [`World::can_place`]: crate::world::World::can_place

use crate::nav::cell_class;
use crate::world::{building_cells, path_cells_of, place_cells_of};
use mc_core::{Fx, FxVec2};
use mc_data::UnitBlueprint;
use mc_map::{Heightfield, MapFile, Prop};
use mc_path::terrain;

/// Why a site will not take a structure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unfit {
    /// Part of the lot is off the map.
    OffMap,
    /// Ground too steep to build on.
    Steep,
    /// A land structure over water.
    Water,
    /// A naval structure without deep water under the whole lot.
    Shore,
    /// A city block stands there.
    City,
    /// Another structure, standing or planned, has the lot.
    Taken,
}

impl Unfit {
    /// A few words for the cursor.
    pub fn label(self) -> &'static str {
        match self {
            Unfit::OffMap => "Off the map",
            Unfit::Steep => "Ground too steep",
            Unfit::Water => "Needs dry ground",
            Unfit::Shore => "Needs deep water",
            Unfit::City => "City in the way",
            Unfit::Taken => "Lot taken",
        }
    }
}

/// Bit on a cell with a city block on it.
const CITY: u8 = 1 << 7;

/// Every 8 m cell's terrain class, and whether a city block stands on it.
pub struct SiteMap {
    w: u32,
    h: u32,
    size: FxVec2,
    cells: Vec<u8>,
}

impl SiteMap {
    pub fn new(ground: &Heightfield, props: &[Prop]) -> SiteMap {
        let (w, h) = ground.size_cells();
        let water = ground.water_level();
        let size = ground.size_metres();
        let mut cells = vec![0u8; (w * h) as usize];
        // Bands of rows, one per core: a big map has millions of cells.
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let band = (h as usize).div_ceil(threads).max(1) * w as usize;
        std::thread::scope(|s| {
            for (i, rows) in cells.chunks_mut(band).enumerate() {
                s.spawn(move || {
                    for (j, class) in rows.iter_mut().enumerate() {
                        let at = i * band + j;
                        *class = cell_class(
                            ground,
                            water,
                            (at % w as usize) as u32,
                            (at / w as usize) as u32,
                        );
                    }
                });
            }
        });
        debug_assert!(cells.iter().all(|c| c & CITY == 0));
        for p in props {
            if let Some((min, max)) = building_cells(p, size) {
                for y in min.1..=max.1.min(h - 1) {
                    for x in min.0..=max.0.min(w - 1) {
                        cells[(y * w + x) as usize] |= CITY;
                    }
                }
            }
        }
        SiteMap { w, h, size, cells }
    }

    /// The sites of a map file, from its unedited ground.
    pub fn for_map(map: &MapFile) -> Option<SiteMap> {
        Some(SiteMap::new(&Heightfield::load(map).ok()?, map.props()))
    }

    fn cell(&self, x: u32, y: u32) -> u8 {
        if x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize]
        } else {
            0
        }
    }

    /// Whether `bp` could stand at `pos` (already snapped to the build grid),
    /// leaving other structures out of it.
    pub fn check(&self, bp: &UnitBlueprint, pos: FxVec2) -> Result<(), Unfit> {
        let half = FxVec2::from_ints(
            bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2,
            bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2,
        );
        if pos.x - half.x < Fx::ZERO
            || pos.y - half.y < Fx::ZERO
            || pos.x + half.x > self.size.x
            || pos.y + half.y > self.size.y
        {
            return Err(Unfit::OffMap);
        }
        let (min, max) = path_cells_of(bp.footprint, pos);
        for y in min.1..=max.1 {
            for x in min.0..=max.0 {
                let class = self.cell(x, y);
                let steep = class & terrain::STEEP != 0;
                if bp.water_only() {
                    if class & terrain::DEEP == 0 {
                        return Err(Unfit::Shore);
                    }
                } else if bp.water_build {
                    if steep && class & (terrain::SHALLOW | terrain::DEEP) == 0 {
                        return Err(Unfit::Steep);
                    }
                } else if class & terrain::LAND == 0 {
                    return Err(Unfit::Water);
                } else if steep {
                    return Err(Unfit::Steep);
                }
            }
        }
        let (min, max) = place_cells_of(bp.footprint, pos);
        for y in min.1..=max.1 {
            for x in min.0..=max.0 {
                if self.cell(x, y) & CITY != 0 {
                    return Err(Unfit::City);
                }
            }
        }
        Ok(())
    }
}
