//! Procedural map baker. Offline tool code: floats are fine here, because the
//! result ships as a file and is identified by its content id.
//!
//! The terrain is a pure function of position. Tiles are therefore baked
//! independently on every core, agree exactly on their shared edges, and go to
//! disk as they finish (in order, so a bake is byte-for-byte reproducible).
//!
//! Fairness comes from symmetry. The map is folded into `2 * players` mirrored
//! wedges around its centre and every noise field is sampled at the folded
//! position, so each player gets the same land, the same distances and the
//! same chokepoints. Start positions sit on the fold axes; the axes between
//! them carry the contested towns.
//!
//! What makes it a battlefield rather than noise:
//! * a terraced continent field: broad, nearly level lowlands and plateaus
//!   (buildable), separated by cliffs that open into ramps only here and there;
//! * narrow ridged mountain ranges, cut by passes, that wall off regions;
//! * sea where the continent field is low, and lakes in the lowlands;
//! * a guaranteed level pad at every start, under the central city and under
//!   every town, with mountains and lakes kept away from them.
//!
//! That is the default [`Layout::Basin`]. [`Layout::Islands`] is a designed
//! two-player sea map built from the same parts (`islands.rs`): one main island
//! with both starts on it, narrowing to a waist round a lake in its middle, so
//! the land either side of the lake is a causeway, and a secondary island with
//! a town across a deep channel on either flank.

use crate::format::{
    encode_tile, EncodedTile, MapError, MapInfo, MapWriter, OreRegion, Prop, PropKind, SNOW_STRIDE,
};
use crate::noise::{bump, hash2, smoothstep, unit, Noise};
use crate::{
    BUILD_CELL_M, CELL_SIZE_M, DEFAULT_MIN_Z, DEFAULT_Z_STEP, TILE_CELLS, TILE_SAMPLES,
    TILE_SAMPLE_COUNT, TILE_SIZE_M,
};
use mc_core::{Angle, Fx, FxVec2};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

/// Continent field value at the shoreline.
const SEA_THRESHOLD: f64 = -0.12;
/// Lowland, mid plateau and high plateau elevations in metres.
const TERRACES: [f64; 3] = [14.0, 55.0, 110.0];
/// Prop candidates sit on a jittered grid of this pitch. It divides the tile
/// size, so every candidate belongs to exactly one tile.
const PROP_GRID_M: f64 = 32.0;
/// Forest trees sit on a finer jittered grid, also dividing the tile size.
const FOREST_GRID_M: f64 = 8.0;
/// Share of forest grid cells that hold a tree in the heart of a wood.
const FOREST_ACCEPT: f64 = 0.72;
/// Trees a map may carry: each is a static entity the renderer culls every frame.
const MAX_TREES: usize = 450_000;
const CITY_LOT_M: f64 = 48.0;
/// Corners on one ore field's outline.
const ORE_CORNERS: usize = 28;

mod alpine;
mod antarctic;
mod archipelago;
mod bays;
mod canyon;
mod crosswater;
mod facility;
mod frostline;
mod harvest;
mod islands;
mod machine;
mod params;
mod planned_wrecks;
mod props;
mod rimehold;
mod siege;
mod sites;
mod spires;
mod threshold;
mod tripoint;
mod vela;

/// Every wreck key the layouts that plan their own wreckage can lay.
pub(crate) fn planned_wreck_keys() -> impl Iterator<Item = &'static str> {
    siege::wrecks::ASSAULT
        .iter()
        .chain(&planned_wrecks::KINDS)
        .map(|k| k.0)
}

pub use params::{BakeParams, Layout};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakeReport {
    pub content_id: u64,
    pub tiles: usize,
    pub file_bytes: u64,
    pub trees: usize,
    pub rocks: usize,
    pub buildings: usize,
    /// Precursor artifacts.
    pub precursor: usize,
    pub start_positions: usize,
    pub ore_regions: usize,
    /// Share of height samples above the water level.
    pub land_percent: u32,
}

/// Bakes a map to `out`.
pub fn bake(params: &BakeParams, out: &Path) -> Result<BakeReport, MapError> {
    params.check()?;
    let info = MapInfo {
        name: params.name.clone(),
        tiles_w: params.tiles_w,
        tiles_h: params.tiles_h,
        min_z: z_range(params.layout).0,
        z_step: z_range(params.layout).1,
        water_level: Fx::ZERO,
    };
    info.validate()?;
    let terrain = Terrain::new(params);
    let mut writer = MapWriter::create(out, info.clone())?;

    let tile_count = info.tile_count();
    let threads = match params.threads {
        0 => std::thread::available_parallelism().map_or(4, |n| n.get()),
        n => n,
    }
    .min(tile_count);
    let next = AtomicUsize::new(0);
    let mut props = Vec::new();
    let mut land_samples = 0u64;
    let (snow_w, snow_h) = info.snow_dims();
    let mut snow = if terrain.has_snow() {
        vec![0u8; (snow_w * snow_h * 2) as usize]
    } else {
        Vec::new()
    };
    let mut ways = if terrain.has_ways() {
        vec![0u8; (snow_w * snow_h * 3) as usize]
    } else {
        Vec::new()
    };
    let (streets_w, streets_h) = info.streets_dims();
    let mut streets = if terrain.has_streets() {
        vec![0u8; (streets_w * streets_h * 4) as usize]
    } else {
        Vec::new()
    };

    std::thread::scope(|s| -> Result<(), MapError> {
        // Bounded, so workers stall rather than pile up tiles if the disk is slow.
        let (send, receive) = mpsc::sync_channel::<(usize, BakedTile)>(threads * 2);
        for _ in 0..threads {
            let send = send.clone();
            let (terrain, next) = (&terrain, &next);
            s.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= tile_count {
                    break;
                }
                let tile =
                    terrain.bake_tile(index as u32 % params.tiles_w, index as u32 / params.tiles_w);
                if send.send((index, tile)).is_err() {
                    break;
                }
            });
        }
        drop(send);
        // Tiles finish out of order; hold the early ones until their turn.
        // (In turn for the snow and ways layers too: two tiles' samples on
        // the edge they share can differ, each taking its slopes from its own
        // samples, and the later tile's must stand whichever thread finished
        // first, or the bake would not be reproducible.)
        let tiles_w = params.tiles_w;
        let lay = |layer: &mut [u8], samples: &[u8], index: usize, bytes: usize| {
            let n = SNOW_PER_TILE;
            let (tx, ty) = (index as u32 % tiles_w, index as u32 / tiles_w);
            for j in 0..n {
                let row = (ty * (n - 1) + j) * snow_w + tx * (n - 1);
                let (at, from) = (row as usize * bytes, (j * n) as usize * bytes);
                layer[at..at + n as usize * bytes]
                    .copy_from_slice(&samples[from..from + n as usize * bytes]);
            }
        };
        let mut early = BTreeMap::new();
        for (index, tile) in receive {
            early.insert(index, tile);
            while let Some(tile) = early.remove(&writer.tiles_written()) {
                let index = writer.tiles_written();
                if !tile.snow.is_empty() {
                    lay(&mut snow, &tile.snow, index, 2);
                }
                if !tile.ways.is_empty() {
                    lay(&mut ways, &tile.ways, index, 3);
                }
                if !tile.streets.is_empty() {
                    // The streets layer, four bytes per height sample.
                    let n = TILE_SAMPLES;
                    let (tx, ty) = (index as u32 % tiles_w, index as u32 / tiles_w);
                    for j in 0..n {
                        let row = (ty * TILE_CELLS + j) * streets_w + tx * TILE_CELLS;
                        let (at, from) = (row as usize * 4, (j * n) as usize * 4);
                        streets[at..at + n as usize * 4]
                            .copy_from_slice(&tile.streets[from..from + n as usize * 4]);
                    }
                }
                writer.push_tile(&tile.encoded)?;
                props.extend(tile.props);
                land_samples += tile.land_samples;
            }
        }
        Ok(())
    })?;

    terrain.city_props(&mut props);
    terrain.precursor_props(&mut props);
    if terrain.layout == Layout::Siege {
        terrain.siege_props(&mut props);
    }
    if terrain.layout == Layout::Vela {
        terrain.vela_props(&mut props);
    }
    // The siege and the survival map lay their own wreckage, with their plans
    // (`siege/wrecks.rs`, `threshold/wrecks.rs`); the trees and small rocks
    // under a wreck are gone.
    let wrecks = match terrain.layout {
        Layout::Siege => terrain.siege_wrecks(),
        Layout::Threshold => terrain.threshold_wrecks(),
        Layout::Harvest => terrain.harvest_wrecks(),
        Layout::Rimehold => terrain.rimehold_wrecks(),
        _ => Vec::new(),
    };
    if !wrecks.is_empty() {
        let reach = mc_core::Fx::from_int(20);
        props.retain(|p| {
            !(p.kind.is_tree() || p.kind == PropKind::RockSmall)
                || wrecks.iter().all(|w| {
                    let d = w.pos - p.pos;
                    d.x.abs() > reach || d.y.abs() > reach || d.length() > reach
                })
        });
    }
    let count = |family: fn(PropKind) -> bool| props.iter().filter(|p| family(p.kind)).count();
    let (trees, rocks, buildings) = (
        count(PropKind::is_tree),
        count(PropKind::is_rock),
        count(|k| k.is_building() || k.is_city()),
    );
    let precursor = count(PropKind::is_precursor);
    let starts: Vec<FxVec2> = terrain.starts.iter().map(|&p| to_fx(p)).collect();
    let ore: Vec<OreRegion> = terrain
        .ore
        .iter()
        .map(|f| OreRegion {
            points: f.corners.iter().map(|&p| to_fx(p)).collect(),
        })
        .collect();
    if !snow.is_empty() {
        writer.set_snow(snow)?;
    }
    if !ways.is_empty() {
        writer.set_ways(ways)?;
    }
    if !streets.is_empty() {
        writer.set_streets(streets)?;
    }
    if !wrecks.is_empty() {
        writer.set_wrecks(wrecks)?;
    }
    // Rimehold's bridges carry decks the simulation reads (`crate::deck`).
    if terrain.layout == Layout::Rimehold {
        writer.set_decks(terrain.rimehold_decks())?;
    }
    // Vela's bridge carries the coast road over the river.
    if terrain.layout == Layout::Vela {
        writer.set_decks(terrain.vela_decks())?;
    }
    let content_id = writer.finish(props, &starts, &ore)?;

    Ok(BakeReport {
        content_id,
        tiles: tile_count,
        file_bytes: std::fs::metadata(out)?.len(),
        trees,
        rocks,
        buildings,
        precursor,
        start_positions: starts.len(),
        ore_regions: ore.len(),
        // Interior samples only, so shared edges are not counted twice.
        land_percent: (land_samples * 100 / (tile_count as u64 * (TILE_CELLS * TILE_CELLS) as u64))
            as u32,
    })
}

/// Height of sample 0 and height per sample step for a layout. Most maps keep
/// -256 m to +768 m in 1/64 m steps.
fn z_range(layout: Layout) -> (Fx, Fx) {
    match layout {
        // Peaks well over the clouds: -256 m to +1791 m in 1/32 m steps.
        Layout::Threshold | Layout::Harvest | Layout::Rimehold => {
            (Fx::from_int(-256), Fx::ratio(1, 32))
        }
        _ => (DEFAULT_MIN_Z, DEFAULT_Z_STEP),
    }
}

/// Exact for the grid-snapped values this is used on; ore corners round to the raw step.
fn to_fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(
        Fx((p.0 * 65536.0).round() as i64),
        Fx((p.1 * 65536.0).round() as i64),
    )
}

struct BakedTile {
    encoded: EncodedTile,
    props: Vec<Prop>,
    land_samples: u64,
    /// `(ice, snow)` pairs, [`SNOW_PER_TILE`] squared; empty on maps without snow.
    snow: Vec<u8>,
    /// `(wear, cos 2a, sin 2a)` triples, [`SNOW_PER_TILE`] squared; empty on
    /// maps without trails.
    ways: Vec<u8>,
    /// Street samples, four bytes per height sample; empty on maps without a city.
    streets: Vec<u8>,
}

/// Snow layer samples along one tile edge, shared edge included.
const SNOW_PER_TILE: u32 = TILE_CELLS / SNOW_STRIDE + 1;

/// A level pad blended into the terrain: fully level within `core`, untouched beyond `outer`.
#[derive(Clone, Copy)]
struct Pad {
    x: f64,
    y: f64,
    core: f64,
    outer: f64,
    height: f64,
}

/// An ore field: centre and nominal radius, and the organic outline around them.
struct OreField {
    x: f64,
    y: f64,
    radius: f64,
    corners: Vec<(f64, f64)>,
}

impl OreField {
    /// Inside the outline pushed out by `margin` metres (roughly: the corners
    /// move away from the centre).
    fn covers(&self, x: f64, y: f64, margin: f64) -> bool {
        let (dx, dy) = (x - self.x, y - self.y);
        let reach = self.radius * 1.5 + margin;
        if dx * dx + dy * dy > reach * reach {
            return false;
        }
        let grow = |(cx, cy): (f64, f64)| {
            let (vx, vy) = (cx - self.x, cy - self.y);
            let len = (vx * vx + vy * vy).sqrt().max(1.0);
            (cx + vx / len * margin, cy + vy / len * margin)
        };
        let n = self.corners.len();
        let mut inside = false;
        for i in 0..n {
            let (ax, ay) = grow(self.corners[i]);
            let (bx, by) = grow(self.corners[(i + n - 1) % n]);
            if (ay > y) != (by > y) && x < ax + (bx - ax) * (y - ay) / (by - ay) {
                inside = !inside;
            }
        }
        inside
    }
}

struct Town {
    x: f64,
    y: f64,
    radius: f64,
    heading: f64,
}

struct Terrain {
    seed: u64,
    layout: Layout,
    size_x: f64,
    size_y: f64,
    /// Shorter edge; the symmetric layout is sized by it.
    size: f64,
    /// Angle of one wedge pair, and the direction of player 0's axis.
    wedge: f64,
    base: f64,
    start_r: f64,
    start_outer: f64,
    /// Town position in folded coordinates.
    town: (f64, f64),
    town_outer: f64,
    city_outer: f64,
    l_cont: f64,
    l_mtn: f64,
    l_lake: f64,
    l_forest: f64,
    /// Broad forest field value where the woods begin; raised on big maps to fit [`MAX_TREES`].
    forest_edge: f64,
    mtn_scale: f64,
    cont: Noise,
    warp_x: Noise,
    warp_y: Noise,
    ramp: Noise,
    tilt: Noise,
    mtn: Noise,
    mtn_mask: Noise,
    mtn_gap: Noise,
    mtn_height: Noise,
    crag: Noise,
    lake: Noise,
    detail: Noise,
    forest: Noise,
    forest_kind: Noise,
    /// Islands layout only: coastline wobble (bays and headlands), the slow
    /// warp of the main island's outline, the lake shore and the ridge line.
    coast: Noise,
    coast_warp: Noise,
    lake_shore: Noise,
    ridge: Noise,
    /// Islands layout only: sea stacks as folded position and radius.
    islets: Vec<(f64, f64, f64)>,
    pads: Vec<Pad>,
    towns: Vec<Town>,
    starts: Vec<(f64, f64)>,
    ore: Vec<OreField>,
    /// Alpine layout only: how much water erosion lowered or raised the
    /// mountains, on a coarse grid (`alpine.rs`). Empty until it has run.
    erosion: alpine::Erosion,
    /// Alpine layout only: glaciers as laid on the map (traced down the
    /// eroded ground). Empty until traced.
    glaciers: alpine::IceFlows,
    /// The precursor artifacts (the arctic layout's, and every map's machine,
    /// `machine.rs`), laid at set-up so the ground round them can be shaped and kept clear.
    precursor: Vec<machine::PrecursorSite>,
    /// Archipelago layout only: its islands (`archipelago.rs`). Empty until laid.
    arch: archipelago::Archipelago,
    /// Canyon layout only: its designed outlines and trails (`canyon.rs`).
    canyon: canyon::Canyon,
    /// Frostline layout only: what it works out at set-up (`frostline.rs`).
    frost: frostline::Frostline,
    /// Tripoint layout only: what it works out at set-up (`tripoint.rs`).
    tp: tripoint::Tripoint,
    /// Spires layout only: what it works out at set-up (`spires.rs`).
    sp: spires::Spires,
    /// Harvest layout only: what it works out at set-up (`harvest.rs`).
    hv: harvest::Harvest,
    /// Rimehold layout only: what it works out at set-up (`rimehold.rs`).
    rh: rimehold::Rimehold,
    /// Siege layout only: the city's plan (`siege.rs`).
    siege: siege::Siege,
    /// Vela layout only: its plan (`vela.rs`).
    vela: vela::Vela,
    /// The machine's benches: ground cut level for its nodes (`machine.rs`).
    /// Empty until the machine is laid, so what is designed before it sees the landscape.
    benches: Vec<machine::Bench>,
}

impl Terrain {
    fn new(params: &BakeParams) -> Terrain {
        let seed = params.seed;
        let size_x = (params.tiles_w as i32 * TILE_SIZE_M) as f64;
        let size_y = (params.tiles_h as i32 * TILE_SIZE_M) as f64;
        let size = size_x.min(size_y);
        let players = params.players;
        let wedge = TAU / players as f64;
        // Up to four players start toward the corners, where there is most room.
        let base = if players <= 4 { PI / 4.0 } else { PI / 8.0 };

        // Feature sizes follow the map so a 4 km dev map is still a whole
        // landscape, not a corner of one.
        let start_core = (0.02 * size).clamp(200.0, 700.0);
        let city_core = (0.011 * size).clamp(260.0, 900.0);
        let town_core = 0.55 * city_core;
        // Islands: the towns sit on their own islands out on the flanks, and
        // the starts move in to share the main island.
        let islands = params.layout == Layout::Islands;
        let town_r = if islands { 0.45 } else { 0.30 } * size;
        let l_mtn = (0.09 * size).clamp(1500.0, 7000.0);
        let noise = |channel| Noise::new(seed, channel);
        let mut t = Terrain {
            seed,
            layout: params.layout,
            size_x,
            size_y,
            size,
            wedge,
            base,
            start_r: if islands { islands::START_R } else { 0.36 } * size,
            start_outer: 2.0 * start_core,
            town: (town_r * (wedge / 2.0).cos(), town_r * (wedge / 2.0).sin()),
            town_outer: 1.6 * town_core,
            city_outer: 1.6 * city_core,
            // An island lobe is only a few kilometres across and still wants several terraces.
            l_cont: if islands {
                0.14 * size
            } else {
                (0.18 * size).clamp(2500.0, 15_000.0)
            },
            l_mtn,
            l_lake: (0.05 * size).clamp(1200.0, 4000.0),
            l_forest: (0.06 * size).clamp(600.0, 2600.0),
            // The islands are small and low; the same edge would wood them over.
            forest_edge: if islands { 0.02 } else { -0.12 },
            mtn_scale: (l_mtn / 7000.0).clamp(0.3, 1.0),
            cont: noise(1),
            warp_x: noise(2),
            warp_y: noise(3),
            ramp: noise(4),
            tilt: noise(5),
            mtn: noise(6),
            mtn_mask: noise(7),
            mtn_gap: noise(8),
            mtn_height: noise(9),
            crag: noise(10),
            lake: noise(11),
            detail: noise(12),
            forest: noise(13),
            forest_kind: noise(14),
            coast: noise(15),
            coast_warp: noise(16),
            lake_shore: noise(17),
            ridge: noise(18),
            islets: Vec::new(),
            pads: Vec::new(),
            towns: Vec::new(),
            starts: Vec::new(),
            ore: Vec::new(),
            erosion: alpine::Erosion::default(),
            glaciers: alpine::IceFlows::default(),
            precursor: Vec::new(),
            arch: archipelago::Archipelago::default(),
            canyon: canyon::Canyon::default(),
            frost: frostline::Frostline::default(),
            siege: siege::Siege::default(),
            vela: vela::Vela::default(),
            tp: tripoint::Tripoint::default(),
            sp: spires::Spires::default(),
            hv: harvest::Harvest::default(),
            rh: rimehold::Rimehold::default(),
            benches: Vec::new(),
        };

        if islands {
            t.lay_islets();
        }

        if params.layout == Layout::Archipelago {
            t.setup_archipelago();
            return t;
        }
        if params.layout == Layout::Threshold {
            t.setup_threshold();
            return t;
        }
        if params.layout == Layout::Harvest {
            t.setup_harvest();
            return t;
        }
        if params.layout == Layout::Rimehold {
            t.setup_rimehold();
            return t;
        }
        if t.is_alpine() {
            t.setup_alpine();
            return t;
        }
        if params.layout == Layout::TwinBays {
            t.setup_bays();
            return t;
        }
        if params.layout == Layout::Canyon {
            t.setup_canyon();
            return t;
        }
        if params.layout == Layout::Frostline {
            t.setup_frostline();
            return t;
        }
        if params.layout == Layout::Tripoint {
            t.setup_tripoint();
            return t;
        }
        if params.layout == Layout::Crosswater {
            t.setup_crosswater();
            return t;
        }
        if params.layout == Layout::Siege {
            t.setup_siege();
            return t;
        }
        if params.layout == Layout::Vela {
            t.setup_vela();
            return t;
        }
        if params.layout == Layout::Antarctic {
            t.setup_antarctic();
            return t;
        }
        if params.layout == Layout::Spires {
            t.setup_spires();
            return t;
        }

        let (cx, cy) = (size_x / 2.0, size_y / 2.0);
        let pad = |t: &Terrain, (x, y): (f64, f64), core: f64, floor: f64| Pad {
            x,
            y,
            core,
            outer: 2.0 * core,
            height: t.natural(x, y).max(floor),
        };
        let mut pads = Vec::new();
        for k in 0..players {
            let start = t.snap(t.unfold(t.start_r, 0.0, k, false));
            pads.push(pad(&t, start, start_core, 10.0));
            t.starts.push(start);
        }
        // The islands layout has a lake where the basin has its city.
        if !islands {
            pads.push(Pad {
                outer: t.city_outer,
                ..pad(&t, (cx, cy), city_core, 8.0)
            });
            t.towns.push(Town {
                x: cx,
                y: cy,
                radius: city_core,
                heading: base,
            });
        }
        // With one player the "between players" axis is the far side of the
        // same player; still a fine place for a town.
        for k in 0..players {
            let at = t.snap(t.unfold(town_r, wedge / 2.0, k, false));
            pads.push(Pad {
                outer: t.town_outer,
                ..pad(&t, at, town_core, 8.0)
            });
            t.towns.push(Town {
                x: at.0,
                y: at.1,
                radius: town_core,
                heading: base + (k as f64 + 0.5) * wedge,
            });
        }
        t.pads = pads;
        t.ore = t
            .place_ore_sites()
            .into_iter()
            .map(|(x, y, r)| t.ore_field(x, y, r))
            .collect();
        t.fit_forests();
        t
    }

    // -- symmetry -----------------------------------------------------------

    /// World position to the canonical wedge: `(x, y)` with the start axis
    /// along +X, plus the distance from the map centre.
    fn fold(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let (vx, vy) = (x - self.size_x / 2.0, y - self.size_y / 2.0);
        let r = (vx * vx + vy * vy).sqrt();
        // The survival map and the siege are not symmetric: nothing folds.
        if matches!(
            self.layout,
            Layout::Threshold | Layout::Siege | Layout::Harvest | Layout::Rimehold | Layout::Vela
        ) {
            return (vx, vy, r);
        }
        // The alpine map is fair by a half turn: fold the far half onto the near.
        // The alpine maps are fair by a mirror across the middle: fold the
        // north half onto the south.
        if self.is_alpine() {
            return if vy > 0.0 { (vx, -vy, r) } else { (vx, vy, r) };
        }
        // The canyon is fair by a mirror across the north-south middle line:
        // fold the east half onto the west.
        if self.layout == Layout::Canyon {
            return (-vx.abs(), vy, r);
        }
        // Tripoint is fair by a third of a turn: turn every point into the
        // north third.
        if self.layout == Layout::Tripoint {
            let k = tripoint::third((vx, vy));
            let (qx, qy) = tripoint::turn((vx, vy), 3 - k);
            return (qx, qy, r);
        }
        // Halden's Grip, The Axis, Frostline, Erebus Strait and Thousand
        // Spires are fair by a half turn: fold the north-east half onto the
        // south-west.
        if matches!(
            self.layout,
            Layout::TwinBays
                | Layout::Archipelago
                | Layout::Frostline
                | Layout::Antarctic
                | Layout::Spires
        ) {
            return if vx + vy > 0.0 {
                (-vx, -vy, r)
            } else {
                (vx, vy, r)
            };
        }
        let a = (vy.atan2(vx) - self.base).rem_euclid(self.wedge);
        let a = if a > self.wedge / 2.0 {
            self.wedge - a
        } else {
            a
        };
        (r * a.cos(), r * a.sin(), r)
    }

    /// Image `k` of the canonical point at radius `r`, angle `a` off the start
    /// axis; `mirrored` picks the clockwise half of the wedge pair.
    fn unfold(&self, r: f64, a: f64, k: u32, mirrored: bool) -> (f64, f64) {
        let angle = self.base + k as f64 * self.wedge + if mirrored { -a } else { a };
        (
            self.size_x / 2.0 + r * angle.cos(),
            self.size_y / 2.0 + r * angle.sin(),
        )
    }

    /// Nearest build-grid vertex, kept a few cells inside the map.
    fn snap(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let g = BUILD_CELL_M as f64;
        let snap = |v: f64, size: f64| ((v / g).round() * g).clamp(4.0 * g, size - 4.0 * g);
        (snap(x, self.size_x), snap(y, self.size_y))
    }

    // -- height -------------------------------------------------------------

    /// The landscape before pads, in metres above the water level.
    fn natural(&self, x: f64, y: f64) -> f64 {
        let h = match self.layout {
            Layout::Basin => self.natural_basin(x, y),
            Layout::Islands => self.natural_islands(x, y),
            Layout::Archipelago => self.natural_archipelago(x, y),
            Layout::Alpine | Layout::AlpineTeams => self.natural_alpine(x, y),
            Layout::TwinBays => self.natural_bays(x, y),
            Layout::Threshold => self.natural_threshold(x, y),
            Layout::Canyon => self.natural_canyon(x, y),
            Layout::Frostline => self.natural_frostline(x, y),
            Layout::Tripoint => self.natural_tripoint(x, y),
            Layout::Crosswater => self.natural_crosswater(x, y),
            Layout::Siege => self.natural_siege(x, y),
            Layout::Antarctic => self.natural_antarctic(x, y),
            Layout::Spires => self.natural_spires(x, y),
            Layout::Harvest => self.natural_harvest(x, y),
            Layout::Rimehold => self.natural_rimehold(x, y),
            Layout::Vela => self.natural_vela(x, y),
        };
        if self.benches.is_empty() {
            h
        } else {
            self.machine_ground(x, y, h)
        }
    }

    /// Whether the map carries a snow layer.
    fn has_snow(&self) -> bool {
        self.is_alpine()
            || matches!(
                self.layout,
                Layout::Threshold
                    | Layout::Frostline
                    | Layout::Tripoint
                    | Layout::Antarctic
                    | Layout::Harvest
                    | Layout::Rimehold
            )
    }

    /// Whether the map carries a streets layer: the siege's city.
    fn has_streets(&self) -> bool {
        matches!(self.layout, Layout::Siege | Layout::Vela)
    }

    /// Whether the map carries a ways layer: the canyon's trails.
    fn has_ways(&self) -> bool {
        self.layout == Layout::Canyon
    }

    fn is_alpine(&self) -> bool {
        matches!(self.layout, Layout::Alpine | Layout::AlpineTeams)
    }

    fn natural_basin(&self, x: f64, y: f64) -> f64 {
        let (px, py, r) = self.fold(x, y);
        let d_start = ((px - self.start_r).powi(2) + py * py).sqrt();
        let d_town = ((px - self.town.0).powi(2) + (py - self.town.1).powi(2)).sqrt();
        let (so, to, co) = (self.start_outer, self.town_outer, self.city_outer);

        // Continent field, warped so coasts and cliffs meander. Near the places
        // that must be buildable it is pulled to a chosen terrace instead of
        // being left to chance: lowland at starts, the mid plateau for towns.
        let wf = 1.0 / (0.6 * self.l_cont);
        let wx = self.warp_x.get(px * wf, py * wf) * 0.15 * self.l_cont;
        let wy = self.warp_y.get(px * wf, py * wf) * 0.15 * self.l_cont;
        let mut c = self
            .cont
            .fbm((px + wx) / self.l_cont, (py + wy) / self.l_cont, 4, 0.5);
        c -= 0.9 * smoothstep(0.50 * self.size, 0.68 * self.size, r);
        c += (-0.02 - c) * bump(d_start / (4.0 * so));
        c += (0.15 - c) * bump(d_town / (4.0 * to));
        c += (0.15 - c) * bump(r / (4.0 * co));
        let u = (c - SEA_THRESHOLD) / 0.5;

        // Shore profile crosses the water level at a slope, then terraces.
        // `ramp` decides where a cliff relaxes into a slope units can climb.
        let ramp = self
            .ramp
            .get(px / (0.35 * self.l_cont), py / (0.35 * self.l_cont));
        let half = 0.006 + 0.10 * smoothstep(0.15, 0.55, ramp);
        let mut h = -8.0 + (TERRACES[0] + 8.0) * smoothstep(-0.08, 0.08, u);
        h -= 50.0 * smoothstep(0.08, 0.6, -u);
        h += (TERRACES[1] - TERRACES[0]) * smoothstep(0.40 - half, 0.40 + half, u);
        h += (TERRACES[2] - TERRACES[1]) * smoothstep(0.80 - half, 0.80 + half, u);
        let land = smoothstep(0.0, 0.1, u);
        h += 2.5 * land * self.tilt.get(px / 1800.0, py / 1800.0);

        // 0 around starts, towns and the city; 1 in open country.
        let open = smoothstep(so, 2.5 * so, d_start)
            * smoothstep(to, 2.5 * to, d_town)
            * smoothstep(co, 2.2 * co, r);

        // Mountain ranges follow the zero crossings of one noise field, which
        // are long connected curves; a mask keeps whole regions free of them
        // and `gap` cuts the passes.
        let mut m = 0.0;
        let inland = smoothstep(0.1, 0.3, u) * open;
        if inland > 0.0 {
            let mask = smoothstep(
                -0.25,
                0.05,
                self.mtn_mask
                    .get(px / (2.4 * self.l_mtn), py / (2.4 * self.l_mtn)),
            );
            if mask > 0.0 {
                let (mx, my) = ((px + 0.5 * wx) / self.l_mtn, (py + 0.5 * wy) / self.l_mtn);
                let line = smoothstep(0.88, 0.985, 1.0 - self.mtn.get(mx, my).abs());
                let gap = smoothstep(
                    -0.35,
                    -0.05,
                    self.mtn_gap
                        .get(px / (0.7 * self.l_mtn), py / (0.7 * self.l_mtn)),
                );
                m = line * mask * gap * inland;
            }
        }
        if m > 0.0 {
            let tall = 250.0
                + 70.0
                    * self
                        .mtn_height
                        .get(px / (1.3 * self.l_mtn), py / (1.3 * self.l_mtn));
            let crag = self.crag.ridged(px / 700.0, py / 700.0, 4, 0.5);
            h += self.mtn_scale * m * (tall + 120.0 * (crag - 0.4));
        }

        // Lakes, lowland only.
        let lowland = land * (1.0 - smoothstep(0.30, 0.38, u)) * open;
        if lowland > 0.0 {
            let lake = smoothstep(0.45, 0.7, self.lake.get(px / self.l_lake, py / self.l_lake));
            h += (-7.0 - h) * lake * lowland;
        }

        h + (1.0 + 5.0 * m) * 1.4 * self.detail.fbm(px / 350.0, py / 350.0, 3, 0.45)
    }

    fn height_with(&self, pads: &[Pad], x: f64, y: f64) -> f64 {
        let mut h = self.natural(x, y);
        for p in pads {
            let d = ((x - p.x).powi(2) + (y - p.y).powi(2)).sqrt();
            h += (p.height - h) * (1.0 - smoothstep(p.core, p.outer, d));
        }
        h
    }

    fn height(&self, x: f64, y: f64) -> f64 {
        self.height_with(&self.pads, x, y)
    }

    /// Rise over run across a build cell.
    fn slope(&self, x: f64, y: f64) -> f64 {
        let e = BUILD_CELL_M as f64 / 2.0;
        let gx = (self.height(x + e, y) - self.height(x - e, y)) / (2.0 * e);
        let gy = (self.height(x, y + e) - self.height(x, y - e)) / (2.0 * e);
        (gx * gx + gy * gy).sqrt()
    }

    // -- markers ------------------------------------------------------------

    /// Centres and nominal radii of the ore fields, symmetric between players.
    fn place_ore_sites(&self) -> Vec<(f64, f64, f64)> {
        let g = BUILD_CELL_M as f64;
        let start_core = self.start_outer / 2.0;
        let mut out: Vec<(f64, f64, f64)> = Vec::new();
        let mut add = |size: f64, p: (f64, f64)| {
            if !out.iter().any(|q| (q.0, q.1) == p) {
                out.push((p.0, p.1, size));
            }
        };
        let players = (TAU / self.wedge).round() as u32;
        // Three small fields around each start, inside its level pad: one
        // behind, two on the forward flanks, far enough apart that they want
        // two or three mines between them.
        for k in 0..players {
            let (sx, sy) = self.starts[k as usize];
            let axis = self.base + k as f64 * self.wedge;
            for turn in [0.0, PI - 1.15, PI + 1.15] {
                let (s, c) = (axis + turn).sin_cos();
                let d = 0.8 * start_core;
                add(
                    (0.2 * start_core).max(55.0),
                    self.snap((sx + c * d, sy + s * d)),
                );
            }
        }
        let islands = self.layout == Layout::Islands;
        // Contested fields are the big ones; the scatter sits in between.
        let contested = (0.36 * start_core).max(90.0);
        let field = (0.26 * start_core).max(70.0);
        // Folded positions the scatter below must keep its distance from.
        let mut designed: Vec<(f64, f64)> = Vec::new();
        if islands {
            // Contested: four around the lake, on the first level ground back
            // from the shore, on the diagonals so neither passage owns them.
            let mut r = 0.05 * self.size;
            let level = |r: f64| {
                let (x, y) = self.snap(self.unfold(r, PI / 4.0, 0, false));
                self.height(x, y) > 10.0 && self.slope(x, y) < 0.03
            };
            while r < 0.2 * self.size && !(level(r) && level(r + 2.0 * g)) {
                r += g;
            }
            r += 2.0 * g;
            // One in every town square, and two more on every town island
            // either side of the fold axis, on the side facing the main island.
            let (tx, ty) = (0.05 * self.size, self.town.1 - 0.035 * self.size);
            let (tr, ta) = ((tx * tx + ty * ty).sqrt(), ty.atan2(tx));
            for k in 0..players {
                for mirrored in [false, true] {
                    add(contested, self.snap(self.unfold(r, PI / 4.0, k, mirrored)));
                    add(field, self.snap(self.unfold(tr, ta, k, mirrored)));
                }
            }
            for town in &self.towns {
                add(0.45 * town.radius, (town.x, town.y));
            }
            designed.extend([(r * (PI / 4.0).cos(), r * (PI / 4.0).sin()), (tx, ty)]);
        } else {
            // Contested: four on the edge of the central city, one in every town square.
            for k in 0..4 {
                let a = self.base + PI / 4.0 + k as f64 * PI / 2.0;
                let r = 1.25 * self.towns[0].radius;
                add(
                    contested,
                    self.snap((
                        self.size_x / 2.0 + r * a.cos(),
                        self.size_y / 2.0 + r * a.sin(),
                    )),
                );
            }
            for town in &self.towns[1..] {
                add(0.45 * town.radius, (town.x, town.y));
            }
        }

        // Scattered: pick points in the canonical half wedge, then give every
        // player the same ones through the symmetry.
        let area_km2 = self.size_x * self.size_y / 1.0e6;
        let total = 6.0 * players as f64 + area_km2 / 40.0;
        let per_wedge = ((total / (2.0 * players as f64)).round() as usize).clamp(1, 32);
        let spacing = (0.02 * self.size).clamp(250.0, 1500.0);
        let mut rng = mc_core::Rng::new(self.seed ^ 0x6D61_7373);
        let mut picked: Vec<(f64, f64)> = Vec::new();
        for _ in 0..4000 {
            if picked.len() == per_wedge {
                break;
            }
            // Uniform over the wedge's area, inside the inscribed circle so every image is on the map.
            let r = (0.08 + 0.40 * rng.unit().to_f64().sqrt()) * self.size;
            let mut a = rng.unit().to_f64() * self.wedge / 2.0;
            // Close to a fold the mirror image would land next to the
            // original; put the point on the fold instead.
            if r * a.sin() < spacing / 2.0 {
                a = 0.0;
            } else if r * (self.wedge / 2.0 - a).sin() < spacing / 2.0 {
                a = self.wedge / 2.0;
            }
            let (px, py) = (r * a.cos(), r * a.sin());
            let (x, y) = self.unfold(r, a, 0, false);
            // The middle is the city, or the lake and its shore. A town island
            // has its three deposits already.
            let (centre, town) = if islands {
                (0.0, 0.2 * self.size)
            } else {
                (2.0 * self.city_outer, 2.0 * self.town_outer)
            };
            let clear = ((px - self.start_r).powi(2) + py * py).sqrt() > 2.5 * self.start_outer
                && ((px - self.town.0).powi(2) + (py - self.town.1).powi(2)).sqrt() > town
                && r > centre
                && !(islands && self.lake_inside(px, py) > -2.0 * spacing)
                && picked
                    .iter()
                    .chain(&designed)
                    .all(|&(qx, qy)| ((px - qx).powi(2) + (py - qy).powi(2)).sqrt() > spacing);
            // Islands: lowland only, off the beach (level, but it belongs to the
            // sea) and off plateaus an engineer may have no way up to.
            let (floor, ceiling) = if islands {
                (8.0, 22.0)
            } else {
                (3.0, f64::INFINITY)
            };
            let height = self.height(x, y);
            if clear && height > floor && height < ceiling && self.slope(x, y) < 0.10 {
                picked.push((px, py));
                let grow = 0.8 + 0.5 * rng.unit().to_f64();
                for k in 0..players {
                    for mirrored in [false, true] {
                        add(field * grow, self.snap(self.unfold(r, a, k, mirrored)));
                    }
                }
            }
        }
        out
    }

    /// An organic outline around `(x, y)`: a few slow harmonics on the radius,
    /// the same for every player's image of the site (seeded from the folded
    /// position, turned with the site), pulled in off water and cliffs.
    fn ore_field(&self, x: f64, y: f64, radius: f64) -> OreField {
        let (px, py, _) = self.fold(x, y);
        let seed = hash2(
            self.seed ^ 0x6F72_6531,
            px.round() as i64,
            py.round() as i64,
        );
        let wave: Vec<(f64, f64, f64)> = (0..4)
            .map(|i| {
                let amp = [0.22, 0.14, 0.08, 0.05][i] * (0.6 + 0.8 * unit(seed, 8 * i as u32));
                let phase = unit(seed, 8 * i as u32 + 4) * TAU;
                ((i + 2) as f64, amp, phase)
            })
            .collect();
        let facing = (y - self.size_y / 2.0).atan2(x - self.size_x / 2.0);
        // Elongated a little along a random axis, so fields are not all round.
        let stretch = 1.0 + 0.35 * unit(seed, 40);
        let axis = unit(seed, 44) * PI;
        let ok = |x: f64, y: f64| self.height(x, y) > 2.0 && self.slope(x, y) < 0.22;
        let corners = (0..ORE_CORNERS)
            .map(|i| {
                let a = i as f64 / ORE_CORNERS as f64 * TAU;
                let mut r = radius
                    * (1.0
                        + wave
                            .iter()
                            .map(|&(f, amp, ph)| amp * (f * a + ph).sin())
                            .sum::<f64>());
                let along = (a - axis).cos();
                r *= 1.0 + (stretch - 1.0) * along * along - (stretch - 1.0) * 0.5;
                let (s, c) = (a + facing).sin_cos();
                let floor = 0.3 * radius;
                while r > floor && !ok(x + c * r, y + s * r) {
                    r -= 0.08 * radius;
                }
                let r = r.max(floor);
                let clamp = |v: f64, size: f64| v.clamp(1.0, size - 1.0);
                (clamp(x + c * r, self.size_x), clamp(y + s * r, self.size_y))
            })
            .collect();
        OreField {
            x,
            y,
            radius,
            corners,
        }
    }

    // -- tiles --------------------------------------------------------------

    fn bake_tile(&self, tx: u32, ty: u32) -> BakedTile {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let (x0, y0) = (
            (tx as i32 * TILE_SIZE_M) as f64,
            (ty as i32 * TILE_SIZE_M) as f64,
        );
        let tile_m = TILE_SIZE_M as f64;
        // Exact prefilter: a pad changes a sample only within `outer` of its centre.
        let near = |x: f64, y: f64, reach: f64| {
            x > x0 - reach && x < x0 + tile_m + reach && y > y0 - reach && y < y0 + tile_m + reach
        };
        let pads: Vec<Pad> = self
            .pads
            .iter()
            .copied()
            .filter(|p| near(p.x, p.y, p.outer))
            .collect();

        let mut samples = vec![0u16; TILE_SAMPLE_COUNT];
        let mut land_samples = 0u64;
        let (min_z, step) = z_range(self.layout);
        let (min_z, per_metre) = (min_z.to_f64(), 1.0 / step.to_f64());
        for (i, s) in samples.iter_mut().enumerate() {
            let (gx, gy) = (i % n, i / n);
            let h = self.height_with(&pads, x0 + gx as f64 * cell, y0 + gy as f64 * cell);
            *s = ((h - min_z) * per_metre)
                .round()
                .clamp(0.0, u16::MAX as f64) as u16;
            land_samples += (h > 0.0 && gx < n - 1 && gy < n - 1) as u64;
        }

        // Ore lies deep: woods grow over a field like anywhere else.
        let props = self.tile_props(tx, ty, &samples, &pads);
        let snow = if self.has_snow() {
            self.tile_snow(x0, y0, &samples)
        } else {
            Vec::new()
        };
        let ways = if self.has_ways() {
            self.tile_ways(x0, y0)
        } else {
            Vec::new()
        };
        let streets = if self.layout == Layout::Vela {
            self.vela_streets(x0, y0)
        } else if self.has_streets() {
            self.tile_streets(x0, y0)
        } else {
            Vec::new()
        };
        BakedTile {
            encoded: encode_tile(&samples),
            props,
            land_samples,
            snow,
            ways,
            streets,
        }
    }

    /// The snow layer over one tile, read off the tile's own samples.
    fn tile_snow(&self, x0: f64, y0: f64, samples: &[u16]) -> Vec<u8> {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let (min_z, step) = z_range(self.layout);
        let (min_z, step) = (min_z.to_f64(), step.to_f64());
        let z = |i: usize, j: usize| min_z + samples[j.min(n - 1) * n + i.min(n - 1)] as f64 * step;
        let per = SNOW_PER_TILE as usize;
        let stride = SNOW_STRIDE as usize;
        let mut out = Vec::with_capacity(per * per * 2);
        for sj in 0..per {
            for si in 0..per {
                let (i, j) = (si * stride, sj * stride);
                // Across two cells each way (one where the tile ends).
                let (il, ir) = (i.saturating_sub(1), (i + 1).min(n - 1));
                let (jd, ju) = (j.saturating_sub(1), (j + 1).min(n - 1));
                let gx = (z(ir, j) - z(il, j)) / ((ir - il) as f64 * cell);
                let gy = (z(i, ju) - z(i, jd)) / ((ju - jd) as f64 * cell);
                let (x, y) = (x0 + i as f64 * cell, y0 + j as f64 * cell);
                let (ice, snow) = match self.layout {
                    Layout::Threshold => self.threshold_snow(x, y, z(i, j), gx, gy),
                    Layout::Frostline => self.frostline_snow(x, y, z(i, j), gx, gy),
                    Layout::Tripoint => self.tripoint_snow(x, y, z(i, j), gx, gy),
                    Layout::Antarctic => self.antarctic_snow(x, y, z(i, j), gx, gy),
                    Layout::Harvest => self.harvest_snow(x, y, z(i, j), gx, gy),
                    Layout::Rimehold => self.rimehold_snow(x, y, z(i, j), gx, gy),
                    _ => self.alpine_snow(x, y, z(i, j), gx, gy),
                };
                // No glacier ice on the machine's benches or their cut faces.
                let ice = ice * self.machine_ice(x, y);
                out.push((ice.clamp(0.0, 1.0) * 255.0).round() as u8);
                out.push((snow.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        out
    }

    /// The ways layer over one tile, on the snow layer's grid.
    fn tile_ways(&self, x0: f64, y0: f64) -> Vec<u8> {
        let per = SNOW_PER_TILE as usize;
        let pitch = SNOW_STRIDE as f64 * CELL_SIZE_M as f64;
        let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let mut out = Vec::with_capacity(per * per * 3);
        for sj in 0..per {
            for si in 0..per {
                let (x, y) = (x0 + si as f64 * pitch, y0 + sj as f64 * pitch);
                let (wear, (dx, dy)) = self.trail_wear(x, y);
                // The heading doubled: a trail runs both ways.
                let (c, s) = (dx * dx - dy * dy, 2.0 * dx * dy);
                out.extend([byte(wear), byte(0.5 + 0.5 * c), byte(0.5 + 0.5 * s)]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;

/// The designed maps the tests read, each laid out once per test binary and
/// shared: laying one out takes seconds, and the tests only look at it.
#[cfg(test)]
mod test_maps {
    use super::{BakeParams, Terrain};
    use std::sync::LazyLock;

    pub(super) static SERAC_DIVIDE: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::alpine("Serac Divide", 4, 3)));
    pub(super) static SERAC_SOUND: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::alpine_teams("Serac Sound", 6, 5)));
    pub(super) static THE_AXIS: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::archipelago("The Axis", 10, 23)));
    pub(super) static THRESHOLD: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::threshold("The Threshold", 8, 31)));
    pub(super) static TWIN_BAYS: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::twin_bays("t", 8, 1)));
    pub(super) static CANYON: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::canyon("t", 6, 11)));
    pub(super) static FROSTLINE: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::frostline("t", 8, 9)));
    pub(super) static ANTARCTIC: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::antarctic("Erebus Strait", 6, 37)));
    pub(super) static CROSSWATER: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::crosswater("Crosswater", 8, 7)));
    pub(super) static TRIPOINT: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::tripoint("t", 6, 13)));
    pub(super) static HALCYON: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::siege("t", 3, 3)));
    pub(super) static SPIRES: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::spires("t", 6, 17)));
    pub(super) static HARVEST: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::harvest("t", 12, 41)));
    pub(super) static RIMEHOLD: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::rimehold("t", 10, 43)));
    pub(super) static VELA: LazyLock<Terrain> =
        LazyLock::new(|| Terrain::new(&BakeParams::vela("t", 6, 5)));
}
