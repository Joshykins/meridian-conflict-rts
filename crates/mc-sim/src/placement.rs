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
use crate::world::{path_cells_of, place_cells_of, prop_cells};
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
    /// A seabed installation without `SEABED_DEPTH` of water over the whole lot.
    Shallow,
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
            Unfit::Shallow => "Needs deeper water",
            Unfit::City => "City in the way",
            Unfit::Taken => "Lot taken",
        }
    }
}

/// Bit on a cell with a city block on it.
pub(crate) const CITY: u8 = 1 << 7;
/// Bit on a cell with at least `SEABED_DEPTH` of water over all of it.
pub(crate) const ABYSS: u8 = 1 << 6;

/// Shallowest water a seabed installation (`UnitBlueprint::seabed`) stands in: its
/// body wholly under the surface, with room over it for a keel to pass.
pub const SEABED_DEPTH: Fx = Fx::from_int(20);

/// Whether heightfield cell (`cx`, `cy`) has `SEABED_DEPTH` of water over its highest corner.
pub(crate) fn abyss(ground: &Heightfield, water: Fx, cx: u32, cy: u32) -> bool {
    let high = ground
        .sample_height(cx, cy)
        .max(ground.sample_height(cx + 1, cy))
        .max(ground.sample_height(cx, cy + 1))
        .max(ground.sample_height(cx + 1, cy + 1));
    water - high >= SEABED_DEPTH
}

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
        #[expect(
            clippy::disallowed_methods,
            reason = "the band count only splits work: each cell is a pure function of the terrain and its own index"
        )]
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let band = (h as usize).div_ceil(threads).max(1) * w as usize;
        std::thread::scope(|s| {
            for (i, rows) in cells.chunks_mut(band).enumerate() {
                s.spawn(move || {
                    for (j, class) in rows.iter_mut().enumerate() {
                        let at = i * band + j;
                        let (x, y) = ((at % w as usize) as u32, (at / w as usize) as u32);
                        *class = cell_class(ground, water, x, y)
                            | if abyss(ground, water, x, y) { ABYSS } else { 0 };
                    }
                });
            }
        });
        debug_assert!(cells.iter().all(|c| c & CITY == 0));
        for p in props {
            for (min, max) in prop_cells(p, size) {
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
    pub fn for_map(map: &MapFile) -> Result<SiteMap, mc_map::MapError> {
        Ok(SiteMap::new(&Heightfield::load(map)?, map.props()))
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
        check_cells(bp, pos, self.size, |x, y| self.cell(x, y))
    }
}

/// [`SiteMap::check`] over any source of cell classes: `class` gives an 8 m cell's
/// terrain class with the [`CITY`] and [`ABYSS`] bits.
pub(crate) fn check_cells(
    bp: &UnitBlueprint,
    pos: FxVec2,
    size: FxVec2,
    class: impl Fn(u32, u32) -> u8,
) -> Result<(), Unfit> {
    let half = FxVec2::from_ints(
        bp.footprint.0 as i32 * mc_map::BUILD_CELL_M / 2,
        bp.footprint.1 as i32 * mc_map::BUILD_CELL_M / 2,
    );
    if pos.x - half.x < Fx::ZERO
        || pos.y - half.y < Fx::ZERO
        || pos.x + half.x > size.x
        || pos.y + half.y > size.y
    {
        return Err(Unfit::OffMap);
    }
    let (min, max) = path_cells_of(bp.footprint, pos);
    for y in min.1..=max.1 {
        for x in min.0..=max.0 {
            let class = class(x, y);
            let steep = class & terrain::STEEP != 0;
            if bp.seabed {
                if class & ABYSS == 0 {
                    return Err(Unfit::Shallow);
                }
            } else if bp.water_only() {
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
            if class(x, y) & CITY != 0 {
                return Err(Unfit::City);
            }
        }
    }
    Ok(())
}
