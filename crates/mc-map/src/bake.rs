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

use crate::format::{encode_tile, EncodedTile, MapError, MapInfo, MapWriter, Prop, PropKind};
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
const CITY_LOT_M: f64 = 48.0;
/// Trees and rocks keep this far from mass deposits.
const DEPOSIT_CLEARING_M: f64 = 40.0;

#[derive(Clone, Debug)]
pub struct BakeParams {
    pub name: String,
    pub tiles_w: u32,
    pub tiles_h: u32,
    pub seed: u64,
    /// Start positions, 1..=8.
    pub players: u32,
    /// Worker threads; 0 uses every core. Does not affect the result.
    pub threads: usize,
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
        BakeParams { name: name.to_owned(), tiles_w: size_tiles, tiles_h: size_tiles, seed, players, threads: 0 }
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
    pub mass_deposits: usize,
    /// Share of height samples above the water level.
    pub land_percent: u32,
}

/// Bakes a map to `out`.
pub fn bake(params: &BakeParams, out: &Path) -> Result<BakeReport, MapError> {
    if !(1..=MAX_START_POSITIONS as u32).contains(&params.players) {
        return Err(MapError::Invalid(format!("players must be 1..={MAX_START_POSITIONS}")));
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
                let tile = terrain.bake_tile(index as u32 % params.tiles_w, index as u32 / params.tiles_w);
                if send.send((index, tile)).is_err() {
                    break;
                }
            });
        }
        drop(send);
        // Tiles finish out of order; hold the early ones until their turn.
        let mut early = BTreeMap::new();
        for (index, tile) in receive {
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
    let (trees, rocks, buildings) =
        (count(PropKind::is_tree), count(PropKind::is_rock), count(PropKind::is_building));
    let starts: Vec<FxVec2> = terrain.starts.iter().map(|&p| to_fx(p)).collect();
    let mass: Vec<FxVec2> = terrain.deposits.iter().map(|&p| to_fx(p)).collect();
    let content_id = writer.finish(props, &starts, &mass)?;

    Ok(BakeReport {
        content_id,
        tiles: tile_count,
        file_bytes: std::fs::metadata(out)?.len(),
        trees,
        rocks,
        buildings,
        start_positions: starts.len(),
        mass_deposits: mass.len(),
        // Interior samples only, so shared edges are not counted twice.
        land_percent: (land_samples * 100 / (tile_count as u64 * (TILE_CELLS * TILE_CELLS) as u64)) as u32,
    })
}

/// Exact for the grid-snapped values this is used on.
fn to_fx(p: (f64, f64)) -> FxVec2 {
    FxVec2::new(Fx((p.0 * 65536.0).round() as i64), Fx((p.1 * 65536.0).round() as i64))
}

struct BakedTile {
    encoded: EncodedTile,
    props: Vec<Prop>,
    land_samples: u64,
}

/// A level pad blended into the terrain: fully level within `core`, untouched beyond `outer`.
#[derive(Clone, Copy)]
struct Pad {
    x: f64,
    y: f64,
    core: f64,
    outer: f64,
    height: f64,
}

struct Town {
    x: f64,
    y: f64,
    radius: f64,
    heading: f64,
}

struct Terrain {
    seed: u64,
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
    pads: Vec<Pad>,
    towns: Vec<Town>,
    starts: Vec<(f64, f64)>,
    deposits: Vec<(f64, f64)>,
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
        let town_r = 0.30 * size;
        let l_mtn = (0.09 * size).clamp(1500.0, 7000.0);
        let noise = |channel| Noise::new(seed, channel);
        let mut t = Terrain {
            seed,
            size_x,
            size_y,
            size,
            wedge,
            base,
            start_r: 0.36 * size,
            start_outer: 2.0 * start_core,
            town: (town_r * (wedge / 2.0).cos(), town_r * (wedge / 2.0).sin()),
            town_outer: 1.6 * town_core,
            city_outer: 1.6 * city_core,
            l_cont: (0.18 * size).clamp(2500.0, 15_000.0),
            l_mtn,
            l_lake: (0.05 * size).clamp(1200.0, 4000.0),
            l_forest: (0.06 * size).clamp(600.0, 2600.0),
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
            pads: Vec::new(),
            towns: Vec::new(),
            starts: Vec::new(),
            deposits: Vec::new(),
        };

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
        pads.push(Pad { outer: t.city_outer, ..pad(&t, (cx, cy), city_core, 8.0) });
        t.towns.push(Town { x: cx, y: cy, radius: city_core, heading: base });
        // With one player the "between players" axis is the far side of the
        // same player; still a fine place for a town.
        for k in 0..players {
            let at = t.snap(t.unfold(town_r, wedge / 2.0, k, false));
            pads.push(Pad { outer: t.town_outer, ..pad(&t, at, town_core, 8.0) });
            t.towns.push(Town { x: at.0, y: at.1, radius: town_core, heading: base + (k as f64 + 0.5) * wedge });
        }
        t.pads = pads;
        t.deposits = t.place_deposits();
        t
    }

    // -- symmetry -----------------------------------------------------------

    /// World position to the canonical wedge: `(x, y)` with the start axis
    /// along +X, plus the distance from the map centre.
    fn fold(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let (vx, vy) = (x - self.size_x / 2.0, y - self.size_y / 2.0);
        let r = (vx * vx + vy * vy).sqrt();
        let a = (vy.atan2(vx) - self.base).rem_euclid(self.wedge);
        let a = if a > self.wedge / 2.0 { self.wedge - a } else { a };
        (r * a.cos(), r * a.sin(), r)
    }

    /// Image `k` of the canonical point at radius `r`, angle `a` off the start
    /// axis; `mirrored` picks the clockwise half of the wedge pair.
    fn unfold(&self, r: f64, a: f64, k: u32, mirrored: bool) -> (f64, f64) {
        let angle = self.base + k as f64 * self.wedge + if mirrored { -a } else { a };
        (self.size_x / 2.0 + r * angle.cos(), self.size_y / 2.0 + r * angle.sin())
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
        let mut c = self.cont.fbm((px + wx) / self.l_cont, (py + wy) / self.l_cont, 4, 0.5);
        c -= 0.9 * smoothstep(0.50 * self.size, 0.68 * self.size, r);
        c += (-0.02 - c) * bump(d_start / (4.0 * so));
        c += (0.15 - c) * bump(d_town / (4.0 * to));
        c += (0.15 - c) * bump(r / (4.0 * co));
        let u = (c - SEA_THRESHOLD) / 0.5;

        // Shore profile crosses the water level at a slope, then terraces.
        // `ramp` decides where a cliff relaxes into a slope units can climb.
        let ramp = self.ramp.get(px / (0.35 * self.l_cont), py / (0.35 * self.l_cont));
        let half = 0.006 + 0.10 * smoothstep(0.15, 0.55, ramp);
        let mut h = -8.0 + (TERRACES[0] + 8.0) * smoothstep(-0.08, 0.08, u);
        h -= 50.0 * smoothstep(0.08, 0.6, -u);
        h += (TERRACES[1] - TERRACES[0]) * smoothstep(0.40 - half, 0.40 + half, u);
        h += (TERRACES[2] - TERRACES[1]) * smoothstep(0.80 - half, 0.80 + half, u);
        let land = smoothstep(0.0, 0.1, u);
        h += 2.5 * land * self.tilt.get(px / 1800.0, py / 1800.0);

        // 0 around starts, towns and the city; 1 in open country.
        let open = smoothstep(so, 2.5 * so, d_start) * smoothstep(to, 2.5 * to, d_town) * smoothstep(co, 2.2 * co, r);

        // Mountain ranges follow the zero crossings of one noise field, which
        // are long connected curves; a mask keeps whole regions free of them
        // and `gap` cuts the passes.
        let mut m = 0.0;
        let inland = smoothstep(0.1, 0.3, u) * open;
        if inland > 0.0 {
            let mask = smoothstep(-0.25, 0.05, self.mtn_mask.get(px / (2.4 * self.l_mtn), py / (2.4 * self.l_mtn)));
            if mask > 0.0 {
                let (mx, my) = ((px + 0.5 * wx) / self.l_mtn, (py + 0.5 * wy) / self.l_mtn);
                let line = smoothstep(0.88, 0.985, 1.0 - self.mtn.get(mx, my).abs());
                let gap = smoothstep(-0.35, -0.05, self.mtn_gap.get(px / (0.7 * self.l_mtn), py / (0.7 * self.l_mtn)));
                m = line * mask * gap * inland;
            }
        }
        if m > 0.0 {
            let tall = 250.0 + 70.0 * self.mtn_height.get(px / (1.3 * self.l_mtn), py / (1.3 * self.l_mtn));
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

    fn place_deposits(&self) -> Vec<(f64, f64)> {
        let g = BUILD_CELL_M as f64;
        let mut out: Vec<(f64, f64)> = Vec::new();
        let mut add = |p: (f64, f64)| {
            if !out.contains(&p) {
                out.push(p);
            }
        };
        // Four at each start, in a pinwheel, well inside the level pad.
        for &(sx, sy) in &self.starts {
            for (dx, dy) in [(-5.0, -2.0), (2.0, -5.0), (5.0, 2.0), (-2.0, 5.0)] {
                add((sx + dx * g, sy + dy * g));
            }
        }
        // Contested: four on the edge of the central city, one in every town square.
        let players = (TAU / self.wedge).round() as u32;
        for k in 0..4 {
            let a = self.base + PI / 4.0 + k as f64 * PI / 2.0;
            let r = 1.25 * self.towns[0].radius;
            add(self.snap((self.size_x / 2.0 + r * a.cos(), self.size_y / 2.0 + r * a.sin())));
        }
        for town in &self.towns[1..] {
            add((town.x, town.y));
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
            let clear = ((px - self.start_r).powi(2) + py * py).sqrt() > 2.5 * self.start_outer
                && ((px - self.town.0).powi(2) + (py - self.town.1).powi(2)).sqrt() > 2.0 * self.town_outer
                && r > 2.0 * self.city_outer
                && picked.iter().all(|&(qx, qy)| ((px - qx).powi(2) + (py - qy).powi(2)).sqrt() > spacing);
            if clear && self.height(x, y) > 3.0 && self.slope(x, y) < 0.10 {
                picked.push((px, py));
                for k in 0..players {
                    for mirrored in [false, true] {
                        add(self.snap(self.unfold(r, a, k, mirrored)));
                    }
                }
            }
        }
        out
    }

    // -- tiles --------------------------------------------------------------

    fn bake_tile(&self, tx: u32, ty: u32) -> BakedTile {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let (x0, y0) = ((tx as i32 * TILE_SIZE_M) as f64, (ty as i32 * TILE_SIZE_M) as f64);
        let tile_m = TILE_SIZE_M as f64;
        // Exact prefilter: a pad changes a sample only within `outer` of its centre.
        let near = |x: f64, y: f64, reach: f64| {
            x > x0 - reach && x < x0 + tile_m + reach && y > y0 - reach && y < y0 + tile_m + reach
        };
        let pads: Vec<Pad> = self.pads.iter().copied().filter(|p| near(p.x, p.y, p.outer)).collect();

        let mut samples = vec![0u16; TILE_SAMPLE_COUNT];
        let mut land_samples = 0u64;
        let (min_z, per_metre) = (DEFAULT_MIN_Z.to_f64(), 1.0 / DEFAULT_Z_STEP.to_f64());
        for (i, s) in samples.iter_mut().enumerate() {
            let (gx, gy) = (i % n, i / n);
            let h = self.height_with(&pads, x0 + gx as f64 * cell, y0 + gy as f64 * cell);
            *s = ((h - min_z) * per_metre).round().clamp(0.0, u16::MAX as f64) as u16;
            land_samples += (h > 0.0 && gx < n - 1 && gy < n - 1) as u64;
        }

        let deposits: Vec<(f64, f64)> =
            self.deposits.iter().copied().filter(|&(x, y)| near(x, y, DEPOSIT_CLEARING_M)).collect();
        let props = self.tile_props(tx, ty, &samples, &pads, &deposits);
        BakedTile { encoded: encode_tile(&samples), props, land_samples }
    }

    /// Trees and rocks for one tile, read off the tile's own samples so props
    /// react to the terrain the player will actually see.
    fn tile_props(&self, tx: u32, ty: u32, samples: &[u16], pads: &[Pad], deposits: &[(f64, f64)]) -> Vec<Prop> {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let per_tile = (TILE_SIZE_M as f64 / PROP_GRID_M) as i64;
        let (x0, y0) = ((tx as i32 * TILE_SIZE_M) as f64, (ty as i32 * TILE_SIZE_M) as f64);
        let z = |s: u16| DEFAULT_MIN_Z.to_f64() + s as f64 * DEFAULT_Z_STEP.to_f64();
        let mut props = Vec::new();
        for j in 0..per_tile {
            for i in 0..per_tile {
                let (gi, gj) = (tx as i64 * per_tile + i, ty as i64 * per_tile + j);
                let hash = hash2(self.seed ^ 0x7072_6F70, gi, gj);
                let x = (gi as f64 + unit(hash, 0)) * PROP_GRID_M;
                let y = (gj as f64 + unit(hash, 24)) * PROP_GRID_M;
                let roll = unit(hash2(self.seed ^ 0x726F_6C6C, gi, gj), 0);

                let (lx, ly) = ((x - x0) / cell, (y - y0) / cell);
                let (cx, cy) = ((lx as usize).min(n - 2), (ly as usize).min(n - 2));
                let at = cy * n + cx;
                let (z00, z10, z01, z11) = (z(samples[at]), z(samples[at + 1]), z(samples[at + n]), z(samples[at + n + 1]));
                let height = (z00 + z10 + z01 + z11) / 4.0;
                let gx = (z10 - z00 + z11 - z01) / (2.0 * cell);
                let gy = (z01 - z00 + z11 - z10) / (2.0 * cell);
                let slope = (gx * gx + gy * gy).sqrt();
                if z00.min(z10).min(z01).min(z11) < 1.5 {
                    continue;
                }
                let blocked = pads.iter().any(|p| (x - p.x).powi(2) + (y - p.y).powi(2) < p.outer * p.outer)
                    || deposits.iter().any(|d| (x - d.0).powi(2) + (y - d.1).powi(2) < DEPOSIT_CLEARING_M.powi(2));
                if blocked {
                    continue;
                }

                // Forests are blobs of one noise field, denser toward their
                // middle, over a thin scatter of lone trees.
                let (px, py, _) = self.fold(x, y);
                let forest = smoothstep(0.22, 0.55, self.forest.fbm(px / self.l_forest, py / self.l_forest, 2, 0.5));
                let kind = if slope < 0.35 && height < 260.0 && roll < 0.012 + 0.9 * forest {
                    let pick = unit(hash, 40);
                    let cold = height > 90.0 || self.forest_kind.get(px / 1500.0, py / 1500.0) > 0.2;
                    match (pick < 0.04, cold, pick < 0.5) {
                        (true, _, _) => PropKind::TreeDead,
                        (_, true, true) => PropKind::TreeConifer,
                        (_, true, false) => PropKind::TreePine,
                        _ => PropKind::TreeBroadleaf,
                    }
                } else if (0.3..1.5).contains(&slope) && roll > 0.93 {
                    if roll > 0.98 {
                        PropKind::RockLarge
                    } else {
                        PropKind::RockSmall
                    }
                } else {
                    continue;
                };
                props.push(Prop {
                    kind,
                    pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                    heading: Angle((hash >> 8) as u16),
                    scale_milli: 700 + ((hash >> 32) % 700) as u16,
                });
            }
        }
        props
    }

    /// Street grids of buildings on the city and town pads: blocks of three
    /// lots between streets, towers toward the middle, vacant lots here and
    /// there, the square around a town's mass deposit left open.
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
                    if self.deposits.iter().any(|p| (x - p.0).powi(2) + (y - p.1).powi(2) < 50.0 * 50.0) {
                        continue;
                    }
                    let pick = unit(hash, 24);
                    let kind = match d / town.radius {
                        f if f < 0.3 => [PropKind::BuildingTower, PropKind::BuildingLarge][(pick < 0.4) as usize],
                        f if f < 0.65 => [PropKind::BuildingLarge, PropKind::BuildingMedium][(pick < 0.6) as usize],
                        _ => [PropKind::BuildingMedium, PropKind::BuildingSmall][(pick < 0.65) as usize],
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
    use crate::test_util::{baked_4km, temp_path};
    use crate::MAX_MAP_TILES;

    #[test]
    fn same_seed_same_map_any_thread_count() {
        let (a, b, c) = (temp_path("det-a"), temp_path("det-b"), temp_path("det-c"));
        let params = BakeParams::square("Determinism", 2, 99);
        let first = bake(&BakeParams { threads: 1, ..params.clone() }, &a).unwrap();
        let second = bake(&BakeParams { threads: 5, ..params.clone() }, &b).unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let other = bake(&BakeParams { seed: 100, ..params }, &c).unwrap();
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
            assert_eq!(map.overview()[(oy * 129 + ox) as usize], hf.sample(ox * 4, oy * 4));
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
            for (dx, dy) in [(-150, 0), (150, 0), (0, -150), (0, 150), (100, 100), (-100, 100)] {
                let p = s + FxVec2::from_ints(dx, dy);
                assert!((hf.height_at(p) - z).abs() < Fx::ratio(1, 4), "start pad is not level at {p:?}");
                assert!(hf.slope_at(p) < Fx::ratio(1, 50));
            }
        }
        // Mirrored terrain: opposite points are equally high (to within detail
        // lost to the sample grid).
        for (x, y) in [(300, 700), (1500, 1900), (2222, 3333), (4000, 100)] {
            let (a, b) = (FxVec2::from_ints(x, y), FxVec2::from_ints(4096 - x, 4096 - y));
            assert!((hf.height_at(a) - hf.height_at(b)).abs() < Fx::ratio(1, 10));
        }
    }

    #[test]
    fn deposits_and_props_respect_the_rules() {
        let map = MapFile::open(baked_4km()).unwrap();
        let hf = Heightfield::load(&map).unwrap();
        let grid = Fx::from_int(BUILD_CELL_M).0;

        let deposits = map.mass_deposits();
        assert!(deposits.len() >= 2 * 4 + 4);
        for d in deposits {
            assert!(d.x.0 % grid == 0 && d.y.0 % grid == 0, "{d:?} is off the build grid");
            assert!(hf.in_bounds(*d));
            assert!(hf.height_at(*d) > hf.water_level());
            assert!(hf.slope_at(*d) < Fx::ratio(1, 5));
        }
        for s in map.start_positions() {
            let close = deposits.iter().filter(|d| d.distance(*s) < Fx::from_int(120)).count();
            assert_eq!(close, 4);
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
                assert!(hf.slope_at(p.pos) < Fx::ratio(3, 5), "tree on a cliff at {:?}", p.pos);
                assert!(map.start_positions().iter().all(|s| s.distance(p.pos) > Fx::from_int(300)));
            }
            if p.kind.is_building() {
                assert!(hf.slope_at(p.pos) < Fx::ratio(1, 10), "building on a slope at {:?}", p.pos);
            }
        }
    }

    #[test]
    fn bad_parameters_are_errors() {
        let path = temp_path("bad-params");
        assert!(bake(&BakeParams { players: 9, ..BakeParams::square("x", 2, 1) }, &path).is_err());
        assert!(bake(&BakeParams { players: 0, ..BakeParams::square("x", 2, 1) }, &path).is_err());
        assert!(bake(&BakeParams::square("x", MAX_MAP_TILES + 1, 1), &path).is_err());
        assert!(bake(&BakeParams::square(&"n".repeat(65), 2, 1), &path).is_err());
    }
}
