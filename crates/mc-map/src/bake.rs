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
//! two-player sea map built from the same parts: one long main island with
//! both starts on it, a lake in its middle, a ridge in the passage either side
//! of the lake (so each passage is a lake-shore lane and a coastal lane), and
//! a secondary island with a town across a deep channel on either flank.

use crate::format::{
    encode_tile, EncodedTile, MapError, MapInfo, MapWriter, OreRegion, Prop, PropKind,
    SNOW_STRIDE,
};
use crate::noise::{bump, hash2, smoothstep, unit, Noise};
use crate::{
    BUILD_CELL_M, CELL_SIZE_M, DEFAULT_MIN_Z, DEFAULT_Z_STEP, MAX_START_POSITIONS, TILE_CELLS,
    TILE_SAMPLES, TILE_SAMPLE_COUNT, TILE_SIZE_M,
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

/// The overall shape of the map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// A continent of terraces, ranges and lakes around a central city, for 1..=8 players.
    #[default]
    Basin,
    /// A main island with a central lake and two flanking town islands. Two players only.
    Islands,
    /// "The Crucible": a designed, asymmetric survival map (see `survival.rs`).
    /// Three defender starts in the south-west, the engine's start last, in
    /// the north-east. Four start positions; wants 14 km.
    Survival,
    /// "Serac Divide": a designed two-player mountain map on a coast (see
    /// `alpine.rs`). Fair by a mirror across the middle wherever units can
    /// go; the mountains, glaciers and woods around that are not mirrored.
    /// Wants 8 km.
    Alpine,
    /// "Serac Sound": the same country for four against four, the sea down
    /// the east side. Eight starts, the south team's first. Wants 12 km.
    AlpineTeams,
}

mod alpine;
mod survival;

#[derive(Clone, Debug)]
pub struct BakeParams {
    pub name: String,
    pub tiles_w: u32,
    pub tiles_h: u32,
    pub seed: u64,
    /// Start positions, 1..=8. [`Layout::Islands`] takes exactly 2.
    pub players: u32,
    /// Worker threads; 0 uses every core. Does not affect the result.
    pub threads: usize,
    pub layout: Layout,
}

impl BakeParams {
    /// A square map of `size_tiles` tiles (2.048 km each) per edge, with a
    /// player count that suits the size: 2 up to 8 km, 4 up to 24 km, else 8.
    pub fn square(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        let players = match size_tiles {
            0..=4 => 2,
            5..=12 => 4,
            _ => 8,
        };
        BakeParams {
            name: name.to_owned(),
            tiles_w: size_tiles,
            tiles_h: size_tiles,
            seed,
            players,
            threads: 0,
            layout: Layout::Basin,
        }
    }

    /// A square two-player [`Layout::Islands`] map. The layout is sized by the
    /// map, and wants 3 tiles (6 km) or more per edge to have room for its lanes.
    pub fn islands(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 2,
            layout: Layout::Islands,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square two-player [`Layout::Alpine`] map.
    pub fn alpine(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 2,
            layout: Layout::Alpine,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square eight-player [`Layout::AlpineTeams`] map.
    pub fn alpine_teams(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 8,
            layout: Layout::AlpineTeams,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square [`Layout::Survival`] map: three defender starts and the engine's.
    pub fn survival(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 4,
            layout: Layout::Survival,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakeReport {
    pub content_id: u64,
    pub tiles: usize,
    pub file_bytes: u64,
    pub trees: usize,
    pub rocks: usize,
    pub buildings: usize,
    pub start_positions: usize,
    pub ore_regions: usize,
    /// Share of height samples above the water level.
    pub land_percent: u32,
}

/// Bakes a map to `out`.
pub fn bake(params: &BakeParams, out: &Path) -> Result<BakeReport, MapError> {
    if !(1..=MAX_START_POSITIONS as u32).contains(&params.players) {
        return Err(MapError::Invalid(format!(
            "players must be 1..={MAX_START_POSITIONS}"
        )));
    }
    if params.layout == Layout::AlpineTeams && params.players != 8 {
        return Err(MapError::Invalid("the AlpineTeams layout is for exactly 8 players".into()));
    }
    if matches!(params.layout, Layout::Islands | Layout::Alpine) && params.players != 2 {
        return Err(MapError::Invalid(format!(
            "the {:?} layout is for exactly 2 players",
            params.layout
        )));
    }
    if params.layout == Layout::Survival && params.players != 4 {
        return Err(MapError::Invalid(
            "the survival layout has exactly 4 starts (3 defenders and the engine)".into(),
        ));
    }
    let info = MapInfo {
        name: params.name.clone(),
        tiles_w: params.tiles_w,
        tiles_h: params.tiles_h,
        min_z: DEFAULT_MIN_Z,
        z_step: DEFAULT_Z_STEP,
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
        let mut early = BTreeMap::new();
        for (index, mut tile) in receive {
            if !tile.snow.is_empty() {
                // The tile's snow samples, shared edges included, into the map's.
                let n = SNOW_PER_TILE;
                let (tx, ty) = (index as u32 % params.tiles_w, index as u32 / params.tiles_w);
                for j in 0..n {
                    let row = (ty * (n - 1) + j) * snow_w + tx * (n - 1);
                    let (at, from) = (row as usize * 2, (j * n) as usize * 2);
                    snow[at..at + n as usize * 2]
                        .copy_from_slice(&tile.snow[from..from + n as usize * 2]);
                }
                tile.snow = Vec::new();
            }
            early.insert(index, tile);
            while let Some(tile) = early.remove(&writer.tiles_written()) {
                writer.push_tile(&tile.encoded)?;
                props.extend(tile.props);
                land_samples += tile.land_samples;
            }
        }
        Ok(())
    })?;

    terrain.city_props(&mut props);
    let count = |family: fn(PropKind) -> bool| props.iter().filter(|p| family(p.kind)).count();
    let (trees, rocks, buildings) = (
        count(PropKind::is_tree),
        count(PropKind::is_rock),
        count(PropKind::is_building),
    );
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
    let content_id = writer.finish(props, &starts, &ore)?;

    Ok(BakeReport {
        content_id,
        tiles: tile_count,
        file_bytes: std::fs::metadata(out)?.len(),
        trees,
        rocks,
        buildings,
        start_positions: starts.len(),
        ore_regions: ore.len(),
        // Interior samples only, so shared edges are not counted twice.
        land_percent: (land_samples * 100 / (tile_count as u64 * (TILE_CELLS * TILE_CELLS) as u64))
            as u32,
    })
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
            start_r: if islands { 0.30 } else { 0.36 } * size,
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
        };

        if islands {
            // A few stacks per quadrant, strung along the main island's
            // outline a little way out to sea, clear of the town channels.
            let mut rng = mc_core::Rng::new(seed ^ 0x6973_6C65);
            for at in [0.20, 0.58, 0.95] {
                let angle: f64 = at + 0.24 * (rng.unit().to_f64() - 0.5);
                let out = 1.24 + 0.10 * rng.unit().to_f64();
                let radius = (0.008 + 0.014 * rng.unit().to_f64().powi(2)) * size;
                t.islets.push((
                    0.40 * size * out * angle.cos(),
                    0.24 * size * out * angle.sin(),
                    radius,
                ));
            }
        }

        if params.layout == Layout::Survival {
            t.setup_survival();
            return t;
        }
        if t.is_alpine() {
            t.setup_alpine();
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
        // The survival map is not symmetric: nothing folds.
        if self.layout == Layout::Survival {
            return (vx, vy, r);
        }
        // The alpine map is fair by a half turn: fold the far half onto the near.
        // The alpine maps are fair by a mirror across the middle: fold the
        // north half onto the south.
        if self.is_alpine() {
            return if vy > 0.0 { (vx, -vy, r) } else { (vx, vy, r) };
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
        match self.layout {
            Layout::Basin => self.natural_basin(x, y),
            Layout::Islands => self.natural_islands(x, y),
            Layout::Survival => self.natural_survival(x, y),
            Layout::Alpine | Layout::AlpineTeams => self.natural_alpine(x, y),
        }
    }

    /// Whether the map carries a snow layer.
    fn has_snow(&self) -> bool {
        self.is_alpine()
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

    /// Folded position for sampling noise in the islands layout. A field
    /// sampled at the plain folded position is creased along the fold axes,
    /// which shows wherever a coast or a ridge crosses one; this fold is
    /// rounded (zero gradient on the axis), so fields stay smooth across it.
    fn soft_fold(&self, px: f64, py: f64) -> (f64, f64) {
        let e = 0.02 * self.size;
        ((px * px + e * e).sqrt(), (py * py + e * e).sqrt())
    }

    /// How far inside the central lake's shoreline a folded position is, in
    /// metres (roughly; negative on land). The lake is longer along the start
    /// axis, which keeps the passages either side of it open, and its shore is
    /// pushed about by noise on the scale of the lake itself.
    fn lake_inside(&self, px: f64, py: f64) -> f64 {
        let s = self.size;
        let (sx, sy) = self.soft_fold(px, py);
        let e = ((px / (0.080 * s)).powi(2) + (py / (0.055 * s)).powi(2)).sqrt();
        0.065
            * s
            * (1.0 - e
                + 0.42
                    * self
                        .lake_shore
                        .fbm(sx / (0.05 * s), sy / (0.05 * s), 3, 0.55))
    }

    /// The islands layout. Everything designed is an even function of the
    /// folded coordinates, so it is smooth across the fold axes too.
    fn natural_islands(&self, x: f64, y: f64) -> f64 {
        let s = self.size;
        // Lengths given in metres (beaches, shore shelves) are for the 10 km
        // map and follow the size only so far: a beach can be neither a cliff
        // nor a plain.
        let k = (s / 10_240.0).clamp(0.8, 1.25);
        let (px, py, _) = self.fold(x, y);
        let (sx, sy) = self.soft_fold(px, py);
        let d_start = ((px - self.start_r).powi(2) + py * py).sqrt();
        let d_town = ((px - self.town.0).powi(2) + (py - self.town.1).powi(2)).sqrt();
        let so = self.start_outer;

        // Distance inland from the main island's coast: an ellipse along the
        // start axis, pushed in and out by a slow warp (so the outline is not
        // an ellipse) and a faster wobble (bays and headlands). The coast keeps
        // still around the starts, and keeps clear of the town islands so the
        // channel between them stays wide and deep.
        let (a, b) = (0.40 * s, 0.24 * s);
        let e = ((px / a).powi(2) + (py / b).powi(2)).sqrt();
        let g = ((px / (a * a)).powi(2) + (py / (b * b)).powi(2)).sqrt();
        let wobble = 0.06 * s * self.coast.fbm(sx / (0.11 * s), sy / (0.11 * s), 4, 0.55)
            + 0.06 * s * self.coast_warp.get(sx / (0.28 * s), sy / (0.28 * s));
        let d_main = (1.0 - e) * if g > 1e-12 { e / g } else { b }
            + wobble * (0.4 + 0.6 * smoothstep(so, 4.0 * so, d_start));
        // (A smooth minimum with the distance to a circle around the town island.)
        let d_clear = d_town - 0.20 * s;
        let d_main =
            0.5 * (d_main + d_clear - ((d_main - d_clear).powi(2) + (100.0 * k).powi(2)).sqrt());
        // The town islands: rough ovals lying along the channel.
        let e_sec = ((px / (0.125 * s)).powi(2) + ((py - self.town.1) / (0.10 * s)).powi(2)).sqrt();
        let d_sec = 0.11
            * s
            * (1.0 - e_sec
                + 0.40
                    * self
                        .coast
                        .fbm(sx / (0.08 * s) + 40.0, sy / (0.08 * s), 3, 0.55)
                + 0.2 * self.coast_warp.get(sx / (0.15 * s) + 40.0, sy / (0.15 * s)));
        let inland_m = d_main.max(d_sec);

        // Interior relief is the basin's terrace field, but only in the two
        // lobes around the starts: the lake shore, the passages either side of
        // it and the town islands are lowland, and so is a coastal strip all
        // the way round (the field is capped by the distance inland), which is
        // what guarantees a ground route from each start to every lane. The
        // pulls are wide and soft so the terrace edges they leave behind
        // are still the noise's, not arcs around the start and the lake.
        let lake_in = self.lake_inside(px, py);
        let wf = 1.0 / (0.6 * self.l_cont);
        let wx = self.warp_x.get(sx * wf, sy * wf) * 0.15 * self.l_cont;
        let wy = self.warp_y.get(sx * wf, sy * wf) * 0.15 * self.l_cont;
        let mut c = 0.42
            + 1.0
                * self
                    .cont
                    .fbm((sx + wx) / self.l_cont, (sy + wy) / self.l_cont, 4, 0.5);
        let rough = 1.0 + 0.3 * self.detail.get(sx / (1.5 * so), sy / (1.5 * so));
        c += (0.15 - c) * bump(d_start * rough / (3.0 * so));
        c += (0.15 - c) * bump(-lake_in.min(0.0) / (0.10 * s));
        c += (0.15 - c) * (1.0 - smoothstep(0.09 * s, 0.17 * s, px));
        let u = (inland_m / (1500.0 * k)).min(c.max(0.12));
        // 0 around the starts, the lake and in the passages; 1 out in the lobes.
        let lobes = smoothstep(so, 2.5 * so, d_start)
            * smoothstep(0.12 * s, 0.17 * s, px)
            * smoothstep(0.03 * s, 0.07 * s, -lake_in);

        // Same shore and terrace profile as the basin, on a wider beach.
        let ramp = self
            .ramp
            .get(sx / (0.35 * self.l_cont), sy / (0.35 * self.l_cont));
        let half = 0.006 + 0.10 * smoothstep(0.15, 0.55, ramp);
        let mut h = -8.0 + (TERRACES[0] + 8.0) * smoothstep(-0.08, 0.08, u);
        h -= 40.0 * smoothstep(0.08, 0.45, -u);
        h += (TERRACES[1] - TERRACES[0]) * smoothstep(0.40 - half, 0.40 + half, u);
        h += (TERRACES[2] - TERRACES[1]) * smoothstep(0.80 - half, 0.80 + half, u);
        let land = smoothstep(0.0, 0.1, u);
        h += 2.5 * land * self.tilt.get(sx / 1800.0, sy / 1800.0);

        // Noise-driven ranges and ponds as in the basin, in the lobes only.
        let mut m = 0.0;
        let inland = smoothstep(300.0 * k, 520.0 * k, inland_m) * lobes;
        if inland > 0.0 {
            let mask = smoothstep(
                -0.25,
                0.05,
                self.mtn_mask
                    .get(sx / (2.4 * self.l_mtn), sy / (2.4 * self.l_mtn)),
            );
            let (mx, my) = ((sx + 0.5 * wx) / self.l_mtn, (sy + 0.5 * wy) / self.l_mtn);
            let line = smoothstep(0.88, 0.985, 1.0 - self.mtn.get(mx, my).abs());
            let gap = smoothstep(
                -0.35,
                -0.05,
                self.mtn_gap
                    .get(sx / (0.7 * self.l_mtn), sy / (0.7 * self.l_mtn)),
            );
            m = line * mask * gap * inland;
        }
        let crag = self.crag.ridged(sx / 700.0, sy / 700.0, 4, 0.5);
        if m > 0.0 {
            let tall = 250.0
                + 70.0
                    * self
                        .mtn_height
                        .get(sx / (1.3 * self.l_mtn), sy / (1.3 * self.l_mtn));
            h += self.mtn_scale * m * (tall + 120.0 * (crag - 0.4));
        }
        let lowland = land
            * (1.0 - smoothstep(0.30, 0.38, u))
            * lobes
            * smoothstep(300.0 * k, 500.0 * k, inland_m);
        if lowland > 0.0 {
            let pond = smoothstep(0.45, 0.7, self.lake.get(sx / self.l_lake, sy / self.l_lake));
            h += (-7.0 - h) * pond * lowland;
        }

        // The designed ridge in each passage. It is laid out in the passage's
        // own coordinate (0 at the lake shore, 1 at the coast) so that wherever
        // the shores wander, the lanes either side of it share the room. The
        // crest wanders and its height comes and goes in peaks and
        // saddles, none low enough to cross.
        let (d_lake, d_coast) = (-lake_in, d_main);
        let mut ridge = 0.0;
        if d_lake > 0.0 && d_coast > 0.0 && px < 0.13 * s {
            let wander = 0.10 * self.ridge.fbm(sx / (0.06 * s), 0.5, 2, 0.6);
            let across = d_lake / (d_lake + d_coast) - 0.55 - wander;
            let width = 0.13 + 0.035 * self.ridge.get(sx / (0.03 * s), 4.5);
            let end = 0.085 * s + 0.02 * s * self.ridge.get(sy / (0.02 * s), 14.5);
            ridge = bump(across.abs() / width)
                * (1.0 - smoothstep(end - 0.035 * s, end + 0.015 * s, px));
            let tall = 150.0 + 30.0 * self.ridge.get(sx / (0.035 * s), 9.5);
            h += ridge * (tall + 110.0 * (crag - 0.45));
        }

        // A rocky knoll on the seaward side of each town island.
        let d_knoll = (px * px + (py - self.town.1 - 0.05 * s).powi(2)).sqrt();
        h += (22.0 + 40.0 * crag) * bump(d_knoll / (0.04 * s)) * smoothstep(0.0, 300.0 * k, d_sec);

        // The lake: deep enough in the middle that land units go around.
        let lake = smoothstep(-200.0 * k, 200.0 * k, lake_in);
        h += (-9.0 - 9.0 * smoothstep(0.0, 0.05 * s, lake_in) - h) * lake;

        h += (1.0 + 5.0 * m + 3.0 * ridge) * 1.4 * self.detail.fbm(sx / 350.0, sy / 350.0, 3, 0.45);

        // Sea stacks off the main island's coast.
        for &(ix, iy, radius) in &self.islets {
            let d = ((px - ix).powi(2) + (py - iy).powi(2)).sqrt();
            if d < radius {
                h += (24.0 + 36.0 * crag - h) * smoothstep(0.1, 0.7, bump(d / radius));
            }
        }
        // Open sea all the way round, whatever the noise did.
        let edge = x.min(y).min(self.size_x - x).min(self.size_y - y);
        h + (-48.0 - h) * (1.0 - smoothstep(0.015 * s, 0.04 * s, edge))
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
                add((0.2 * start_core).max(55.0), self.snap((sx + c * d, sy + s * d)));
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
        let seed = hash2(self.seed ^ 0x6F72_6531, px.round() as i64, py.round() as i64);
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
                    * (1.0 + wave.iter().map(|&(f, amp, ph)| amp * (f * a + ph).sin()).sum::<f64>());
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
        let (min_z, per_metre) = (DEFAULT_MIN_Z.to_f64(), 1.0 / DEFAULT_Z_STEP.to_f64());
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
        BakedTile {
            encoded: encode_tile(&samples),
            props,
            land_samples,
            snow,
        }
    }

    /// The snow layer over one tile, read off the tile's own samples.
    fn tile_snow(&self, x0: f64, y0: f64, samples: &[u16]) -> Vec<u8> {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let z = |i: usize, j: usize| {
            DEFAULT_MIN_Z.to_f64()
                + samples[j.min(n - 1) * n + i.min(n - 1)] as f64 * DEFAULT_Z_STEP.to_f64()
        };
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
                let (ice, snow) = self.alpine_snow(x, y, z(i, j), gx, gy);
                out.push((ice.clamp(0.0, 1.0) * 255.0).round() as u8);
                out.push((snow.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        out
    }

    /// How thickly trees grow at a point, 0..=1, and how much of the stand is
    /// conifer. Sampled at the folded position, so every player gets the same
    /// woods. Big forests come from one broad field with clearings cut into
    /// it; small groves and copses dot the open ground between them.
    fn forest_density(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        if self.is_alpine() {
            return self.alpine_forest(x, y, height, slope);
        }
        let (px, py, _) = self.fold(x, y);
        let l = self.l_forest;
        let broad = self.forest.fbm(px / l, py / l, 3, 0.5);
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(0.42, 0.62, self.forest.fbm(px / (0.16 * l) + 71.3, py / (0.16 * l) - 19.1, 2, 0.5));
        let clearing = smoothstep(0.30, 0.55, self.forest.fbm(px / (0.09 * l) - 33.7, py / (0.09 * l) + 57.2, 2, 0.5));
        let tree_floor = if self.layout == Layout::Islands { 6.0 } else { 1.5 };
        let habitable = smoothstep(tree_floor, tree_floor + 6.0, height)
            * (1.0 - smoothstep(0.28, 0.5, slope))
            * (1.0 - smoothstep(230.0, 290.0, height));
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable;
        // Conifers take the high and the cold ground, in stands, not a salt-and-pepper mix.
        // Species do not change play, so an unfolded term may break up the
        // straight line a mirror axis would otherwise draw between stands.
        let cold = smoothstep(60.0, 170.0, height) * 0.9
            + self.forest_kind.fbm(px / 1500.0, py / 1500.0, 2, 0.5) * 0.9
            + self.forest_kind.fbm(x / 380.0 + 41.0, y / 380.0 - 13.0, 2, 0.5) * 0.45;
        (density, smoothstep(-0.15, 0.35, cold))
    }

    /// Trees expected on the map for a given forest edge, from a fixed sample
    /// of the landscape: bakes stay reproducible.
    fn expected_trees(&self, samples: &[(f64, f64, f64, f64)]) -> f64 {
        let per_sample = self.size_x * self.size_y / (FOREST_GRID_M * FOREST_GRID_M) / samples.len() as f64;
        samples
            .iter()
            .map(|&(x, y, h, s)| self.forest_density(x, y, h, s).0 * FOREST_ACCEPT)
            .sum::<f64>()
            * per_sample
    }

    /// Raises the forest edge until the map's trees fit [`MAX_TREES`]. Small
    /// maps keep the full woods; the largest lose whole forests rather than
    /// thinning every one of them into an orchard.
    fn fit_forests(&mut self) {
        let mut samples = Vec::new();
        let step = (self.size_x * self.size_y / 30_000.0).sqrt();
        let (nx, ny) = ((self.size_x / step) as i64, (self.size_y / step) as i64);
        for j in 0..ny {
            for i in 0..nx {
                let hash = hash2(self.seed ^ 0x6669_7466, i, j);
                let x = (i as f64 + unit(hash, 0)) * step;
                let y = (j as f64 + unit(hash, 24)) * step;
                let h = self.height(x, y);
                if h > 1.5 {
                    samples.push((x, y, h, self.slope(x, y)));
                }
            }
        }
        if samples.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (self.forest_edge, 1.0);
        if self.expected_trees(&samples) <= MAX_TREES as f64 {
            return;
        }
        for _ in 0..16 {
            self.forest_edge = 0.5 * (lo + hi);
            if self.expected_trees(&samples) > MAX_TREES as f64 {
                lo = self.forest_edge;
            } else {
                hi = self.forest_edge;
            }
        }
        self.forest_edge = hi;
    }

    fn tree_kind(&self, x: f64, y: f64, conifer: f64, hash: u64) -> PropKind {
        // The alpine map's woods are not mirrored.
        let (px, py) = match self.layout {
            Layout::Alpine | Layout::AlpineTeams => (x, y),
            _ => {
                let (px, py, _) = self.fold(x, y);
                (px, py)
            }
        };
        // Pines stand in their own patches among the firs.
        let pines = self.forest_kind.get(px / 420.0 + 11.0, py / 420.0 - 5.0) > 0.1;
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.02 => PropKind::TreeDead,
            (_, pick) if pick < conifer && pines => PropKind::TreePine,
            (_, pick) if pick < conifer => PropKind::TreeConifer,
            _ => PropKind::TreeBroadleaf,
        }
    }

    /// Trees and rocks for one tile, read off the tile's own samples so props
    /// react to the terrain the player will actually see.
    fn tile_props(
        &self,
        tx: u32,
        ty: u32,
        samples: &[u16],
        pads: &[Pad],
    ) -> Vec<Prop> {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let (x0, y0) = (
            (tx as i32 * TILE_SIZE_M) as f64,
            (ty as i32 * TILE_SIZE_M) as f64,
        );
        let z = |s: u16| DEFAULT_MIN_Z.to_f64() + s as f64 * DEFAULT_Z_STEP.to_f64();
        // Height, slope and lowest corner of the sample cell under a point.
        let ground = |x: f64, y: f64| {
            let (lx, ly) = ((x - x0) / cell, (y - y0) / cell);
            let (cx, cy) = ((lx as usize).min(n - 2), (ly as usize).min(n - 2));
            let at = cy * n + cx;
            let (z00, z10, z01, z11) = (
                z(samples[at]),
                z(samples[at + 1]),
                z(samples[at + n]),
                z(samples[at + n + 1]),
            );
            let gx = (z10 - z00 + z11 - z01) / (2.0 * cell);
            let gy = (z01 - z00 + z11 - z10) / (2.0 * cell);
            (
                (z00 + z10 + z01 + z11) / 4.0,
                (gx * gx + gy * gy).sqrt(),
                z00.min(z10).min(z01).min(z11),
            )
        };
        let blocked = |x: f64, y: f64| {
            pads.iter()
                .any(|p| (x - p.x).powi(2) + (y - p.y).powi(2) < p.outer * p.outer)
        };
        let mut props = Vec::new();
        // The alpine map is rockier, and its woods climb steeper ground.
        let alpine = self.is_alpine();
        let (rock_slope, rock_roll) = if alpine { (0.18, 0.8) } else { (0.3, 0.93) };
        let forest_slope = if alpine { 0.95 } else { 0.5 };

        // Rocks on the slopes, and lone trees out in the open.
        let per_tile = (TILE_SIZE_M as f64 / PROP_GRID_M) as i64;
        for j in 0..per_tile {
            for i in 0..per_tile {
                let (gi, gj) = (tx as i64 * per_tile + i, ty as i64 * per_tile + j);
                let hash = hash2(self.seed ^ 0x7072_6F70, gi, gj);
                let x = (gi as f64 + unit(hash, 0)) * PROP_GRID_M;
                let y = (gj as f64 + unit(hash, 24)) * PROP_GRID_M;
                let roll = unit(hash2(self.seed ^ 0x726F_6C6C, gi, gj), 0);
                let (height, slope, lowest) = ground(x, y);
                if lowest < 1.5 || blocked(x, y) {
                    continue;
                }
                let (density, conifer) = self.forest_density(x, y, height, slope);
                let (kind, scale) = if density > 0.0 && slope < 0.3 && roll < 0.05 * (1.0 - density) {
                    (self.tree_kind(x, y, conifer, hash), 800 + ((hash >> 32) % 600) as u16)
                } else if (rock_slope..1.5).contains(&slope)
                    && roll > rock_roll
                    && !(alpine && self.alpine_ice(x, y) > 0.05)
                {
                    let kind = if roll > 0.98 { PropKind::RockLarge } else { PropKind::RockSmall };
                    (kind, 700 + ((hash >> 32) % 700) as u16)
                } else {
                    continue;
                };
                props.push(Prop {
                    kind,
                    pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                    heading: Angle((hash >> 8) as u16),
                    scale_milli: scale,
                });
            }
        }

        // Forests: a fine jittered grid, kept where the woods are dense.
        let per_tile = (TILE_SIZE_M as f64 / FOREST_GRID_M) as i64;
        for j in 0..per_tile {
            for i in 0..per_tile {
                let (gi, gj) = (tx as i64 * per_tile + i, ty as i64 * per_tile + j);
                let hash = hash2(self.seed ^ 0x776F_6F64, gi, gj);
                let roll = unit(hash2(self.seed ^ 0x7374_616E, gi, gj), 0);
                if roll >= FOREST_ACCEPT {
                    continue;
                }
                // Jitter within the middle of the cell keeps trunks from touching.
                let x = (gi as f64 + 0.15 + 0.7 * unit(hash, 0)) * FOREST_GRID_M;
                let y = (gj as f64 + 0.15 + 0.7 * unit(hash, 24)) * FOREST_GRID_M;
                let (height, slope, lowest) = ground(x, y);
                if lowest < 1.5 || slope >= forest_slope {
                    continue;
                }
                let (density, conifer) = self.forest_density(x, y, height, slope);
                if roll >= density * FOREST_ACCEPT || blocked(x, y) {
                    continue;
                }
                // Tall in the heart of a wood, shorter and bushier at its edge.
                let scale = 650.0 + density * 400.0 + unit(hash, 32) * 450.0;
                props.push(Prop {
                    kind: self.tree_kind(x, y, conifer, hash),
                    pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                    heading: Angle((hash >> 8) as u16),
                    scale_milli: (scale as u16).min(1500),
                });
            }
        }
        props
    }

    /// Street grids of buildings on the city and town pads: blocks of three
    /// lots between streets, towers toward the middle, vacant lots here and
    /// there, the square around a town's ore field left open.
    fn city_props(&self, out: &mut Vec<Prop>) {
        for (index, town) in self.towns.iter().enumerate() {
            let reach = (town.radius / CITY_LOT_M) as i64;
            let (sin, cos) = town.heading.sin_cos();
            let heading = (town.heading / TAU * 65536.0).rem_euclid(65536.0) as u16;
            for j in -reach..=reach {
                for i in -reach..=reach {
                    let (lx, ly) = (i as f64 * CITY_LOT_M, j as f64 * CITY_LOT_M);
                    let d = (lx * lx + ly * ly).sqrt();
                    let hash = hash2(self.seed ^ (0x6369_7479 + index as u64), i, j);
                    let street = i.rem_euclid(4) == 0 || j.rem_euclid(4) == 0;
                    if street || d > town.radius - CITY_LOT_M / 2.0 || unit(hash, 0) < 0.12 {
                        continue;
                    }
                    let (x, y) = (town.x + lx * cos - ly * sin, town.y + lx * sin + ly * cos);
                    if self.ore.iter().any(|f| f.covers(x, y, CITY_LOT_M / 2.0)) {
                        continue;
                    }
                    let pick = unit(hash, 24);
                    let kind = match d / town.radius {
                        f if f < 0.3 => [PropKind::BuildingTower, PropKind::BuildingLarge]
                            [(pick < 0.4) as usize],
                        f if f < 0.65 => [PropKind::BuildingLarge, PropKind::BuildingMedium]
                            [(pick < 0.6) as usize],
                        _ => [PropKind::BuildingMedium, PropKind::BuildingSmall]
                            [(pick < 0.65) as usize],
                    };
                    out.push(Prop {
                        kind,
                        pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                        // Square to the street grid, facing any of the four ways.
                        heading: Angle(heading.wrapping_add(((hash >> 50) as u16 & 3) << 14)),
                        scale_milli: 850 + ((hash >> 32) % 400) as u16,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::MapFile;
    use crate::heightfield::Heightfield;
    use crate::test_util::{baked_4km, baked_islands, temp_path};
    use crate::MAX_MAP_TILES;

    /// Mean corner of every ore field.
    fn ore_centres(map: &MapFile) -> Vec<FxVec2> {
        map.ore_regions()
            .iter()
            .map(|r| {
                let n = r.points.len() as i32;
                let sum = r.points.iter().fold(FxVec2::ZERO, |a, p| a + *p);
                FxVec2::new(sum.x / n, sum.y / n)
            })
            .collect()
    }

    #[test]
    fn ore_fields_are_organic_and_mostly_dry() {
        let map = MapFile::open(baked_4km()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        for region in map.ore_regions() {
            assert_eq!(region.points.len(), ORE_CORNERS);
            let (lo, hi) = region.bounds();
            assert!((hi - lo).length() > Fx::from_int(80), "{:?} is tiny", (lo, hi));
            // Sample the inside: dry, and no tree or rock on it.
            let (mut inside, mut dry) = (0, 0);
            let mut y = lo.y;
            while y < hi.y {
                let mut x = lo.x;
                while x < hi.x {
                    let p = FxVec2::new(x, y);
                    if region.contains(p) {
                        inside += 1;
                        dry += (hf.height_at(p) > hf.water_level()) as i32;
                    }
                    x += Fx::from_int(8);
                }
                y += Fx::from_int(8);
            }
            assert!(inside > 50 && dry * 10 >= inside * 9, "{dry} of {inside} dry");
            // Towns keep their buildings off a field; woods may grow over it.
            assert!(!map
                .props()
                .iter()
                .any(|p| p.kind.is_building() && region.contains(p.pos)));
        }
    }

    #[test]
    fn same_seed_same_map_any_thread_count() {
        let (a, b, c) = (temp_path("det-a"), temp_path("det-b"), temp_path("det-c"));
        let params = BakeParams::square("Determinism", 2, 99);
        let first = bake(
            &BakeParams {
                threads: 1,
                ..params.clone()
            },
            &a,
        )
        .unwrap();
        let second = bake(
            &BakeParams {
                threads: 5,
                ..params.clone()
            },
            &b,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let other = bake(
            &BakeParams {
                seed: 100,
                ..params
            },
            &c,
        )
        .unwrap();
        assert_ne!(first.content_id, other.content_id);
        for p in [a, b, c] {
            std::fs::remove_file(p).ok();
        }
    }

    #[test]
    fn baked_map_round_trips_and_verifies() {
        let map = MapFile::open(baked_4km()).unwrap();
        assert_eq!(map.name(), "Test Basin");
        assert_eq!(map.size_tiles(), (2, 2));
        assert_eq!(map.info().min_z, DEFAULT_MIN_Z);
        assert_eq!(map.info().z_step, DEFAULT_Z_STEP);
        assert_eq!(map.overview_dims(), (129, 129));
        map.verify().unwrap();

        // The overview is every fourth sample of the full-resolution data.
        let hf = Heightfield::load(&map).unwrap();
        for (ox, oy) in [(0, 0), (128, 128), (64, 17), (63, 64), (5, 99)] {
            assert_eq!(
                map.overview()[(oy * 129 + ox) as usize],
                hf.sample(ox * 4, oy * 4)
            );
        }
        // Props come back grouped by tile, and on their tile.
        let mut seen = 0;
        for ty in 0..2 {
            for tx in 0..2 {
                let props = map.props_in_tile(tx, ty).unwrap();
                assert!(props.iter().all(|p| map.info().tile_of(p.pos) == (tx, ty)));
                seen += props.len();
            }
        }
        assert_eq!(seen, map.props().len());
    }

    #[test]
    fn starts_are_level_dry_and_symmetric() {
        let map = MapFile::open(baked_4km()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        let starts = map.start_positions();
        assert_eq!(starts.len(), 2);
        let centre = FxVec2::from_ints(2048, 2048);
        assert!((starts[0] + starts[1] - centre - centre).length() <= Fx::from_int(BUILD_CELL_M));
        for &s in starts {
            let z = hf.height_at(s);
            assert!(z > hf.water_level() + Fx::from_int(5));
            for (dx, dy) in [
                (-150, 0),
                (150, 0),
                (0, -150),
                (0, 150),
                (100, 100),
                (-100, 100),
            ] {
                let p = s + FxVec2::from_ints(dx, dy);
                assert!(
                    (hf.height_at(p) - z).abs() < Fx::ratio(1, 4),
                    "start pad is not level at {p:?}"
                );
                assert!(hf.slope_at(p) < Fx::ratio(1, 50));
            }
        }
        // Mirrored terrain: opposite points are equally high (to within detail
        // lost to the sample grid).
        for (x, y) in [(300, 700), (1500, 1900), (2222, 3333), (4000, 100)] {
            let (a, b) = (
                FxVec2::from_ints(x, y),
                FxVec2::from_ints(4096 - x, 4096 - y),
            );
            assert!((hf.height_at(a) - hf.height_at(b)).abs() < Fx::ratio(1, 10));
        }
    }

    #[test]
    fn ore_fields_and_props_respect_the_rules() {
        let map = MapFile::open(baked_4km()).unwrap();
        let hf = Heightfield::load(&map).unwrap();

        let deposits = &ore_centres(&map);
        assert!(deposits.len() >= 2 * 3 + 4);
        for d in deposits {
            assert!(hf.in_bounds(*d));
            assert!(hf.height_at(*d) > hf.water_level());
            assert!(hf.slope_at(*d) < Fx::ratio(1, 5));
        }
        for s in map.start_positions() {
            let close: Vec<FxVec2> = deposits
                .iter()
                .copied()
                .filter(|d| d.distance(*s) < Fx::from_int(260))
                .collect();
            assert_eq!(close.len(), 3);
            for (i, a) in close.iter().enumerate() {
                for b in &close[i + 1..] {
                    assert!(
                        a.distance(*b) > Fx::from_int(200),
                        "start deposits {a:?} and {b:?} crowd each other"
                    );
                }
            }
        }

        let props = map.props();
        let trees = props.iter().filter(|p| p.kind.is_tree()).count();
        let buildings = props.iter().filter(|p| p.kind.is_building()).count();
        assert!(trees > 100, "{trees} trees");
        assert!(buildings > 10, "{buildings} buildings");
        for p in props {
            assert!(hf.in_bounds(p.pos));
            assert!((500..=1500).contains(&p.scale_milli));
            if p.kind.is_tree() {
                assert!(hf.height_at(p.pos) > hf.water_level());
                assert!(
                    hf.slope_at(p.pos) < Fx::ratio(3, 5),
                    "tree on a cliff at {:?}",
                    p.pos
                );
                assert!(map
                    .start_positions()
                    .iter()
                    .all(|s| s.distance(p.pos) > Fx::from_int(300)));
            }
            if p.kind.is_building() {
                assert!(
                    hf.slope_at(p.pos) < Fx::ratio(1, 10),
                    "building on a slope at {:?}",
                    p.pos
                );
            }
        }
    }

    #[test]
    fn islands_bake_is_deterministic_and_leaves_the_basin_alone() {
        let (a, b, c) = (temp_path("isl-a"), temp_path("isl-b"), temp_path("isl-c"));
        let params = BakeParams::islands("Determinism", 3, 99);
        assert_eq!((params.players, params.layout), (2, Layout::Islands));
        let first = bake(
            &BakeParams {
                threads: 1,
                ..params.clone()
            },
            &a,
        )
        .unwrap();
        let second = bake(
            &BakeParams {
                threads: 5,
                ..params.clone()
            },
            &b,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let basin = bake(
            &BakeParams {
                layout: Layout::Basin,
                ..params
            },
            &c,
        )
        .unwrap();
        assert_ne!(first.content_id, basin.content_id);
        // `square` is the basin, and the layout is part of neither the file nor its defaults.
        assert_eq!(BakeParams::square("x", 3, 99).layout, Layout::default());
        assert_eq!(
            basin,
            bake(&BakeParams::square("Determinism", 3, 99), &c).unwrap()
        );
        for p in [a, b, c] {
            std::fs::remove_file(p).ok();
        }
    }

    #[test]
    fn islands_have_level_starts_a_lake_towns_and_open_sea() {
        let map = MapFile::open(baked_islands()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        let (water, size) = (hf.water_level(), 4 * TILE_SIZE_M);
        let centre = FxVec2::from_ints(size / 2, size / 2);

        // Both starts on the main island: level, dry, mirrored, about 0.6 of the map apart.
        let starts = map.start_positions();
        assert_eq!(starts.len(), 2);
        assert!((starts[0] + starts[1] - centre - centre).length() <= Fx::from_int(BUILD_CELL_M));
        let apart = starts[0].distance(starts[1]).to_f64() / size as f64;
        assert!(
            (0.55..0.65).contains(&apart),
            "starts are {apart} of the map apart"
        );
        for &s in starts {
            let z = hf.height_at(s);
            assert!(z > water + Fx::from_int(5));
            for (dx, dy) in [
                (-150, 0),
                (150, 0),
                (0, -150),
                (0, 150),
                (100, 100),
                (-100, 100),
            ] {
                let p = s + FxVec2::from_ints(dx, dy);
                assert!(
                    (hf.height_at(p) - z).abs() < Fx::ratio(1, 4),
                    "start pad is not level at {p:?}"
                );
                assert!(hf.slope_at(p) < Fx::ratio(1, 50));
            }
        }
        // Mirrored through the centre and across the start axis (the NE-SW diagonal).
        for (x, y) in [
            (2300, 2700),
            (3500, 3900),
            (4222, 5333),
            (6000, 5100),
            (1800, 6300),
        ] {
            let z = hf.height_at(FxVec2::from_ints(x, y));
            assert!(
                (z - hf.height_at(FxVec2::from_ints(size - x, size - y))).abs() < Fx::ratio(1, 10)
            );
            assert!((z - hf.height_at(FxVec2::from_ints(y, x))).abs() < Fx::ratio(1, 10));
        }

        // A lake too deep to ford in the middle, open sea along every edge,
        // and dry, level town islands out on the other diagonal.
        assert!(hf.height_at(centre) < water - Fx::from_int(6));
        let (w, h) = hf.size_cells();
        for i in 0..=w {
            for (cx, cy) in [(i, 0), (i, h), (0, i), (w, i)] {
                assert!(
                    hf.sample_height(cx, cy) < water - Fx::from_int(20),
                    "land at the map edge ({cx}, {cy})"
                );
            }
        }
        let out = (0.45 * size as f64 / 2f64.sqrt()) as i32;
        for town in [
            centre + FxVec2::from_ints(-out, out),
            centre + FxVec2::from_ints(out, -out),
        ] {
            assert!(hf.height_at(town) > water + Fx::from_int(5));
            assert!(hf.slope_at(town) < Fx::ratio(1, 50));
            let ore = ore_centres(&map);
            assert!(ore
                .iter()
                .any(|d| d.distance(town) < Fx::from_int(4 * BUILD_CELL_M)));
            let near = |p: FxVec2| p.distance(town) < Fx::from_int(size / 8);
            assert_eq!(ore.iter().filter(|d| near(**d)).count(), 3);
            assert!(map
                .props()
                .iter()
                .any(|p| p.kind.is_building() && near(p.pos)));
        }
    }

    #[test]
    fn islands_ore_and_props_stay_on_dry_land() {
        let map = MapFile::open(baked_islands()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        let centre = FxVec2::from_ints(2 * TILE_SIZE_M, 2 * TILE_SIZE_M);

        // Three at each start, four round the lake, three on each town island, and the scatter.
        let deposits = &ore_centres(&map);
        assert!(
            deposits.len() >= 2 * 3 + 4 + 2 * 3 + 4,
            "{} ore fields",
            deposits.len()
        );
        for d in deposits {
            assert!(hf.in_bounds(*d));
            assert!(
                hf.height_at(*d) > hf.water_level() + Fx::from_int(5),
                "{d:?} is in or by the water"
            );
            assert!(hf.slope_at(*d) < Fx::ratio(1, 5));
            let image = centre + centre - *d;
            assert!(
                deposits
                    .iter()
                    .any(|o| o.distance(image) <= Fx::from_int(3 * BUILD_CELL_M)),
                "{d:?} has no mirror image"
            );
        }
        for s in map.start_positions() {
            let close: Vec<FxVec2> = deposits
                .iter()
                .copied()
                .filter(|d| d.distance(*s) < Fx::from_int(260))
                .collect();
            assert_eq!(close.len(), 3);
            for (i, a) in close.iter().enumerate() {
                for b in &close[i + 1..] {
                    assert!(
                        a.distance(*b) > Fx::from_int(200),
                        "start deposits {a:?} and {b:?} crowd each other"
                    );
                }
            }
        }
        assert_eq!(
            deposits
                .iter()
                .filter(|d| d.distance(centre) < Fx::from_int(800))
                .count(),
            4
        );

        let props = map.props();
        assert!(props.iter().filter(|p| p.kind.is_tree()).count() > 100);
        for p in props
            .iter()
            .filter(|p| p.kind.is_tree() || p.kind.is_building())
        {
            assert!(
                hf.height_at(p.pos) > hf.water_level() + Fx::ONE,
                "{:?} in the water at {:?}",
                p.kind,
                p.pos
            );
        }
    }

    /// Path cells a land unit at `from` can reach: dry, no steeper than 0.5,
    /// and not in `blocked`. Returns whether `to` is among them.
    fn land_route(
        hf: &Heightfield,
        from: FxVec2,
        to: FxVec2,
        blocked: impl Fn(f64, f64) -> bool,
    ) -> bool {
        let (w, h) = hf.size_cells();
        let cell = CELL_SIZE_M as f64;
        let open = |cx: u32, cy: u32| {
            let dry = [(0, 0), (1, 0), (0, 1), (1, 1)]
                .iter()
                .all(|&(dx, dy)| hf.sample_height(cx + dx, cy + dy) > hf.water_level());
            dry && hf.cell_slope(cx, cy) <= Fx::ratio(1, 2)
                && !blocked((cx as f64 + 0.5) * cell, (cy as f64 + 0.5) * cell)
        };
        let mut seen = vec![false; (w * h) as usize];
        let mut queue = std::collections::VecDeque::from([hf.cell_at(from)]);
        while let Some((cx, cy)) = queue.pop_front() {
            if cx >= w || cy >= h || seen[(cy * w + cx) as usize] || !open(cx, cy) {
                continue;
            }
            seen[(cy * w + cx) as usize] = true;
            // Four-connected, so a diagonal wall three cells thick holds. (Wrapping below 0 fails the bounds check.)
            queue.extend([
                (cx + 1, cy),
                (cx.wrapping_sub(1), cy),
                (cx, cy + 1),
                (cx, cy.wrapping_sub(1)),
            ]);
        }
        let (tx, ty) = hf.cell_at(to);
        seen[(ty * w + tx) as usize]
    }

    /// The starts are joined by land both ways round the lake, and only so.
    fn assert_two_land_routes(map: &MapFile) {
        let hf = Heightfield::load(map).unwrap();
        let starts = map.start_positions();
        let size = hf.size_metres().x.to_f64();
        // A wall across one passage: from the lake's middle out to the map's
        // corner along the diagonal the town islands are on.
        let wall = |x: f64, y: f64, north_west: bool| {
            let (along, across) = ((x + y - size) / 2f64.sqrt(), (y - x) / 2f64.sqrt());
            along.abs() < 20.0 && (across > 0.0) == north_west
        };
        assert!(
            land_route(&hf, starts[0], starts[1], |_, _| false),
            "no land route between the starts"
        );
        assert!(
            land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, true)),
            "no route through the south-east passage"
        );
        assert!(
            land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, false)),
            "no route through the north-west passage"
        );
        assert!(
            !land_route(&hf, starts[0], starts[1], |x, y| wall(x, y, true)
                || wall(x, y, false)),
            "the lake is fordable"
        );
        // No causeways: the town islands are real islands.
        for d in ore_centres(&map)
            .iter()
            .filter(|d| d.distance(starts[0]).min(d.distance(starts[1])).to_f64() > 0.4 * size)
        {
            assert!(
                !land_route(&hf, starts[0], *d, |_, _| false),
                "a land bridge to the town island at {d:?}"
            );
        }
    }

    #[test]
    fn islands_starts_are_joined_by_land_on_both_sides_of_the_lake() {
        assert_two_land_routes(&MapFile::open(baked_islands()).unwrap());
        // The smallest size the layout is meant for.
        let path = temp_path("isl-6km");
        bake(&BakeParams::islands("Small Shoals", 3, 7), &path).unwrap();
        assert_two_land_routes(&MapFile::open(&path).unwrap());
        std::fs::remove_file(path).ok();
    }

    /// `MC_CHECK_ISLANDS=$PWD/maps/twin_shoals.mcmap cargo test --release -p mc-map -- --ignored checks_a_baked`
    /// runs the route check on a map baked with `mc-bake --layout islands`.
    #[test]
    #[ignore = "needs MC_CHECK_ISLANDS=<file.mcmap>"]
    fn checks_a_baked_islands_map() {
        let path = std::env::var("MC_CHECK_ISLANDS").expect("MC_CHECK_ISLANDS is not set");
        assert_two_land_routes(&MapFile::open(std::path::Path::new(&path)).unwrap());
    }

    #[test]
    fn bad_parameters_are_errors() {
        let path = temp_path("bad-params");
        assert!(bake(
            &BakeParams {
                players: 4,
                ..BakeParams::islands("x", 3, 1)
            },
            &path
        )
        .is_err());
        assert!(bake(
            &BakeParams {
                players: 9,
                ..BakeParams::square("x", 2, 1)
            },
            &path
        )
        .is_err());
        assert!(bake(
            &BakeParams {
                players: 0,
                ..BakeParams::square("x", 2, 1)
            },
            &path
        )
        .is_err());
        assert!(bake(&BakeParams::square("x", MAX_MAP_TILES + 1, 1), &path).is_err());
        assert!(bake(&BakeParams::square(&"n".repeat(65), 2, 1), &path).is_err());
    }
}
