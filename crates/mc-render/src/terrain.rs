//! CPU side of the terrain: CDLOD node selection and the streamed tile cache.
//!
//! Work here scales with the number of terrain nodes and tiles on screen (a few
//! hundred), never with the number of entities.

use crate::camera::Camera;
use crate::gpu_consts::settle;
use glam::{Vec2, Vec3, Vec4};
use mc_jobs::{Pool, TaskHandle};
use mc_map::{FlattenRecord, MapFile, OVERVIEW_STRIDE, TILE_CELLS, TILE_SAMPLES};
use std::sync::Arc;

/// Leaf node edge: 64 cells of 8 m, so leaf vertices sit exactly on height samples.
pub const LEAF_SIZE: f32 = 512.0;
/// Terrain quads are kept to about this many pixels on screen. The distance
/// out to which leaves are used follows from it; each coarser level doubles it.
// Selection and morphing can each double the cell span: a 1 px target keeps the silhouette
// within roughly 4 px, rather than the old 10-20 px angular coast segments.
pub const QUAD_PIXELS: f32 = 1.0;
pub const MAX_NODES: usize = 4096;
/// Full-resolution tiles kept on the GPU.
pub const TILE_LAYERS: u32 = 64;
/// Tiles are streamed in around the eye when it is this close to the ground.
const STREAM_RADIUS: f32 = 3200.0;
const MAX_LOADS_IN_FLIGHT: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainNode {
    /// Origin x, y; edge length; level.
    pub rect: [f32; 4],
    /// Morph start and end distance.
    pub morph: [f32; 4],
}

fn aabb_outside(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    planes.iter().take(5).any(|p| {
        let v = Vec3::new(
            if p.x >= 0.0 { max.x } else { min.x },
            if p.y >= 0.0 { max.y } else { min.y },
            if p.z >= 0.0 { max.z } else { min.z },
        );
        p.truncate().dot(v) + p.w < 0.0
    })
}

fn aabb_distance(min: Vec3, max: Vec3, p: Vec3) -> f32 {
    (p.clamp(min, max) - p).length()
}

/// Picks the quadtree nodes to draw. Returns false if `MAX_NODES` was hit.
pub fn select_nodes(camera: &Camera, z_range: (f32, f32), out: &mut Vec<TerrainNode>) -> bool {
    out.clear();
    let map = camera.map_size;
    let mut levels = 0u32;
    while LEAF_SIZE * (1u32 << levels) as f32 <= map.x.max(map.y) - 1.0 {
        levels += 1;
    }
    let root = LEAF_SIZE * (1u32 << levels) as f32;
    let planes = camera.frustum();
    let eye = camera.eye();
    let leaf_range = (LEAF_SIZE / 64.0 * camera.projection_scale() / QUAD_PIXELS).max(600.0);
    let mut complete = true;
    let mut stack = vec![(Vec2::ZERO, root, levels)];
    while let Some((origin, size, level)) = stack.pop() {
        if origin.x >= map.x || origin.y >= map.y {
            continue;
        }
        let min = origin.extend(z_range.0);
        let max = (origin + Vec2::splat(size)).min(map).extend(z_range.1);
        if aabb_outside(&planes, min, max) {
            continue;
        }
        let finer_range = leaf_range * (1u32 << level.saturating_sub(1)) as f32;
        if level == 0 || aabb_distance(min, max, eye) > finer_range {
            if out.len() >= MAX_NODES {
                complete = false;
                break;
            }
            let range = leaf_range * (1u32 << level) as f32;
            out.push(TerrainNode {
                rect: [origin.x, origin.y, size, level as f32],
                morph: [range * 0.7, range * 0.97, 0.0, 0.0],
            });
        } else {
            let half = size * 0.5;
            for (dx, dy) in [(0.0, 0.0), (half, 0.0), (0.0, half), (half, half)] {
                stack.push((origin + Vec2::new(dx, dy), half, level - 1));
            }
        }
    }
    complete
}

/// A 65x65 grid with each cell split along the (x, y)-(x+1, y+1) diagonal,
/// matching `mc_map::Heightfield::height_at`.
pub fn grid_mesh() -> (Vec<[f32; 2]>, Vec<u32>) {
    const N: u32 = 64;
    let mut vertices = Vec::with_capacity(((N + 1) * (N + 1)) as usize);
    for y in 0..=N {
        for x in 0..=N {
            vertices.push([x as f32, y as f32]);
        }
    }
    let mut indices = Vec::with_capacity((N * N * 6) as usize);
    let at = |x: u32, y: u32| y * (N + 1) + x;
    for y in 0..N {
        for x in 0..N {
            indices.extend_from_slice(&[
                at(x, y),
                at(x + 1, y),
                at(x + 1, y + 1),
                at(x, y),
                at(x + 1, y + 1),
                at(x, y + 1),
            ]);
        }
    }
    (vertices, indices)
}

/// A finished tile or patch waiting to be copied to the GPU by the renderer.
pub enum TerrainUpload {
    Tile {
        layer: u32,
        samples: Vec<u16>,
    },
    TilePatch {
        layer: u32,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        sample: u16,
    },
    OverviewPatch {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        sample: u16,
    },
    /// The tile -> layer table changed; upload all of it.
    Index(Vec<u16>),
}

struct Resident {
    tile: (u32, u32),
    last_used: u64,
}

/// A tile being read off disk: its tile coordinates and the job reading its heights.
type TileLoad = ((u32, u32), TaskHandle<Option<Vec<u16>>>);

/// Which full-resolution tiles live in which layer of the GPU tile array.
pub struct TileCache {
    map: Arc<MapFile>,
    tiles_w: u32,
    tiles_h: u32,
    /// `layer + 1` per tile, zero when not resident. This is the GPU's indirection table.
    index: Vec<u16>,
    layers: Vec<Option<Resident>>,
    loading: Vec<TileLoad>,
    /// Every terrain edit so far, in order. The first `committed` are in the
    /// tiles (replayed onto them as they stream in) and the overview; the rest
    /// are still settling, eased in by the shaders from `settling`.
    edits: Vec<FlattenRecord>,
    committed: usize,
    /// When each edit arrived, seconds of render time.
    arrived: Vec<f32>,
    /// Whether the renderer has seen the sim's edit table yet. The edits already
    /// there when it first does are the match as it stands, and do not settle.
    synced: bool,
    /// CPU copy of the overview for cursor picking.
    pub overview: Vec<u16>,
    pub overview_dims: (u32, u32),
    /// Lowest and highest ground on the map in metres, from the overview with a
    /// margin for what lies between its samples: the height of the boxes terrain
    /// patches are culled by.
    pub height_span: (f32, f32),
    frame: u64,
}

impl TileCache {
    pub fn new(map: Arc<MapFile>) -> TileCache {
        let (tiles_w, tiles_h) = map.size_tiles();
        let info = map.info();
        let (lo, hi) = map
            .overview()
            .iter()
            .fold((u16::MAX, 0u16), |(a, b), &s| (a.min(s), b.max(s)));
        let z = |s: u16| info.min_z.to_f32() + s as f32 * info.z_step.to_f32();
        let height_span = (z(lo) - 60.0, z(hi) + 60.0);
        TileCache {
            height_span,
            overview: map.overview().to_vec(),
            overview_dims: map.overview_dims(),
            map,
            tiles_w,
            tiles_h,
            index: vec![0; (tiles_w * tiles_h) as usize],
            layers: (0..TILE_LAYERS).map(|_| None).collect(),
            loading: Vec::new(),
            edits: Vec::new(),
            committed: 0,
            arrived: Vec::new(),
            synced: false,
            frame: 0,
        }
    }

    pub fn tiles(&self) -> (u32, u32) {
        (self.tiles_w, self.tiles_h)
    }

    /// Once per frame: schedules loads near the eye, collects finished ones.
    pub fn update(&mut self, camera: &Camera, pool: &Pool, uploads: &mut Vec<TerrainUpload>) {
        self.frame += 1;
        let eye = camera.eye();
        let tile_size = (TILE_CELLS * mc_map::CELL_SIZE_M as u32) as f32;
        let mut index_dirty = false;

        // Collect finished loads.
        let mut i = 0;
        while i < self.loading.len() {
            if !self.loading[i].1.is_finished() {
                i += 1;
                continue;
            }
            let (tile, handle) = self.loading.swap_remove(i);
            let Some(mut samples) = handle.join() else {
                log::error!("failed to read map tile {tile:?}");
                continue;
            };
            for e in &self.edits[..self.committed] {
                patch_tile(&mut samples, tile, e);
            }
            if let Some(layer) = self.take_layer() {
                if let Some(old) = self.layers[layer].take() {
                    self.index[(old.tile.1 * self.tiles_w + old.tile.0) as usize] = 0;
                }
                self.layers[layer] = Some(Resident {
                    tile,
                    last_used: self.frame,
                });
                self.index[(tile.1 * self.tiles_w + tile.0) as usize] = layer as u16 + 1;
                uploads.push(TerrainUpload::Tile {
                    layer: layer as u32,
                    samples,
                });
                index_dirty = true;
            }
        }

        // Tiles wanted: those within reach of the eye, nearest first.
        if eye.z < STREAM_RADIUS * 1.5 {
            let mut wanted: Vec<(f32, (u32, u32))> = Vec::new();
            let lo = ((eye.truncate() - Vec2::splat(STREAM_RADIUS)) / tile_size)
                .floor()
                .max(Vec2::ZERO);
            let hi = ((eye.truncate() + Vec2::splat(STREAM_RADIUS)) / tile_size).floor();
            for ty in lo.y as u32..=(hi.y.max(0.0) as u32).min(self.tiles_h - 1) {
                for tx in lo.x as u32..=(hi.x.max(0.0) as u32).min(self.tiles_w - 1) {
                    let min = Vec2::new(tx as f32, ty as f32) * tile_size;
                    let d = (eye.truncate().clamp(min, min + tile_size) - eye.truncate()).length();
                    if d <= STREAM_RADIUS {
                        wanted.push((d, (tx, ty)));
                    }
                }
            }
            wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, tile) in wanted.into_iter().take(TILE_LAYERS as usize) {
                let slot = self.index[(tile.1 * self.tiles_w + tile.0) as usize];
                if slot != 0 {
                    if let Some(r) = &mut self.layers[slot as usize - 1] {
                        r.last_used = self.frame;
                    }
                } else if self.loading.len() < MAX_LOADS_IN_FLIGHT
                    && !self.loading.iter().any(|l| l.0 == tile)
                {
                    let map = self.map.clone();
                    let handle = pool.spawn_background("stream-tile", move || {
                        map.read_tile(tile.0, tile.1).ok()
                    });
                    self.loading.push((tile, handle));
                }
            }
        }
        if index_dirty {
            uploads.push(TerrainUpload::Index(self.index.clone()));
        }
    }

    /// A free layer, or the least recently used one.
    fn take_layer(&self) -> Option<usize> {
        if let Some(free) = self.layers.iter().position(Option::is_none) {
            return Some(free);
        }
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.last_used < self.frame))
            .min_by_key(|(_, r)| r.as_ref().map(|r| r.last_used))
            .map(|(i, _)| i)
    }

    /// Takes in terrain edits the renderer has not seen yet. `all` is the sim's
    /// whole edit table; a shorter table than before means a snapshot replaced it.
    /// A new edit settles: the ground eases to its level over `settle::SECONDS`
    /// (`settling`), and only then is it written into the tiles.
    pub fn apply_edits(
        &mut self,
        all: &[FlattenRecord],
        time: f32,
        uploads: &mut Vec<TerrainUpload>,
    ) {
        let replaced = all.len() < self.edits.len() || all[..self.edits.len()] != self.edits[..];
        if replaced {
            log::warn!("terrain edit history was replaced; reloading terrain");
            self.edits.clear();
            self.arrived.clear();
            self.committed = 0;
            self.overview = self.map.overview().to_vec();
            for l in &mut self.layers {
                *l = None;
            }
            self.index.fill(0);
            uploads.push(TerrainUpload::Index(self.index.clone()));
            uploads.push(TerrainUpload::OverviewPatch {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                sample: 0,
            });
        }
        // A match joined or restored part way is shown as it stands.
        let at_once = replaced || !self.synced;
        self.synced = true;
        self.edits.extend_from_slice(&all[self.edits.len()..]);
        self.arrived.resize(self.edits.len(), time);
        while self.committed < self.edits.len() {
            let age = time - self.arrived[self.committed];
            // Past the slots the oldest snap to their level (a cosmetic cap): a
            // line of lots laid at once still settles, a few at a time.
            let crowded = self.edits.len() - self.committed > settle::SLOTS as usize;
            // A clock that went back (a replay seek) must not hold an edit forever.
            if !(at_once || crowded || age >= settle::SECONDS || age < -1.0) {
                break;
            }
            let e = self.edits[self.committed];
            self.commit(&e, uploads);
            self.committed += 1;
        }
    }

    /// Writes one edit into the overview and the resident tiles.
    fn commit(&mut self, e: &FlattenRecord, uploads: &mut Vec<TerrainUpload>) {
        let ((sx0, sy0), (sx1, sy1)) = e.sample_rect();
        // Overview samples are every fourth full-resolution sample.
        let (ox0, oy0) = (sx0.div_ceil(OVERVIEW_STRIDE), sy0.div_ceil(OVERVIEW_STRIDE));
        let (ox1, oy1) = (sx1 / OVERVIEW_STRIDE, sy1 / OVERVIEW_STRIDE);
        if ox0 <= ox1 && oy0 <= oy1 {
            for y in oy0..=oy1.min(self.overview_dims.1 - 1) {
                for x in ox0..=ox1.min(self.overview_dims.0 - 1) {
                    self.overview[(y * self.overview_dims.0 + x) as usize] = e.sample;
                }
            }
            uploads.push(TerrainUpload::OverviewPatch {
                x: ox0,
                y: oy0,
                w: ox1 - ox0 + 1,
                h: oy1 - oy0 + 1,
                sample: e.sample,
            });
        }
        for (layer, r) in self.layers.iter().enumerate() {
            let Some(r) = r else { continue };
            if let Some((x, y, w, h)) = tile_overlap(r.tile, e) {
                uploads.push(TerrainUpload::TilePatch {
                    layer: layer as u32,
                    x,
                    y,
                    w,
                    h,
                    sample: e.sample,
                });
            }
        }
    }

    /// The lots still settling, oldest first, for `Globals::settling`: per lot
    /// the rect it levels in metres (min xy, max xy), then its level in metres
    /// and how far the ground has eased to it (0..1). Returns how many there are.
    pub fn settling(&self, time: f32, out: &mut [[f32; 4]]) -> u32 {
        let info = self.map.info();
        let (min_z, step) = (info.min_z.to_f32(), info.z_step.to_f32());
        let cell = mc_map::CELL_SIZE_M as f32;
        let mut n = 0;
        for (e, &arrived) in self.edits[self.committed..]
            .iter()
            .zip(&self.arrived[self.committed..])
        {
            let Some(pair) = out.get_mut(n * 2..n * 2 + 2) else {
                break;
            };
            let ((sx0, sy0), (sx1, sy1)) = e.sample_rect();
            pair[0] = [
                sx0 as f32 * cell,
                sy0 as f32 * cell,
                sx1 as f32 * cell,
                sy1 as f32 * cell,
            ];
            pair[1] = [
                min_z + e.sample as f32 * step,
                settle_ease((time - arrived) / settle::SECONDS),
                0.0,
                0.0,
            ];
            n += 1;
        }
        n as u32
    }

    /// Terrain height from the overview, metres. Good enough for the cursor.
    pub fn overview_height(&self, xy: Vec2) -> f32 {
        let info = self.map.info();
        let spacing = (OVERVIEW_STRIDE * mc_map::CELL_SIZE_M as u32) as f32;
        let p = (xy / spacing).clamp(
            Vec2::ZERO,
            Vec2::new(
                self.overview_dims.0 as f32 - 1.001,
                self.overview_dims.1 as f32 - 1.001,
            ),
        );
        let (x, y) = (p.x as u32, p.y as u32);
        let f = p - p.floor();
        let at = |x: u32, y: u32| self.overview[(y * self.overview_dims.0 + x) as usize] as f32;
        let h = at(x, y) * (1.0 - f.x) * (1.0 - f.y)
            + at(x + 1, y) * f.x * (1.0 - f.y)
            + at(x, y + 1) * (1.0 - f.x) * f.y
            + at(x + 1, y + 1) * f.x * f.y;
        info.min_z.to_f32() + h * info.z_step.to_f32()
    }

    /// Where a ray meets the ground, by marching the overview.
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let mut t = 0.0;
        let mut prev = origin;
        for _ in 0..4096 {
            let p = origin + dir * t;
            let above = p.z - self.overview_height(p.truncate());
            if above <= 0.0 {
                // Bisect between the last point above ground and this one.
                let (mut a, mut b) = (prev, p);
                for _ in 0..12 {
                    let m = (a + b) * 0.5;
                    if m.z - self.overview_height(m.truncate()) > 0.0 {
                        a = m
                    } else {
                        b = m
                    }
                }
                return Some(b);
            }
            prev = p;
            t += (above * 0.5).clamp(1.0, 2000.0);
            if t > 400_000.0 {
                break;
            }
        }
        None
    }
}

/// How far a settling lot has come at `t` of the way through: slow to start,
/// slower to finish, like earth being moved and then tamped.
pub fn settle_ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Part of `e` that falls inside `tile`, in tile-local sample coordinates.
fn tile_overlap(tile: (u32, u32), e: &FlattenRecord) -> Option<(u32, u32, u32, u32)> {
    let ((sx0, sy0), (sx1, sy1)) = e.sample_rect();
    let (bx, by) = (tile.0 * TILE_CELLS, tile.1 * TILE_CELLS);
    let x0 = sx0.max(bx);
    let y0 = sy0.max(by);
    let x1 = sx1.min(bx + TILE_SAMPLES - 1);
    let y1 = sy1.min(by + TILE_SAMPLES - 1);
    (x0 <= x1 && y0 <= y1).then(|| (x0 - bx, y0 - by, x1 - x0 + 1, y1 - y0 + 1))
}

fn patch_tile(samples: &mut [u16], tile: (u32, u32), e: &FlattenRecord) {
    if let Some((x, y, w, h)) = tile_overlap(tile, e) {
        for row in y..y + h {
            let start = (row * TILE_SAMPLES + x) as usize;
            samples[start..start + w as usize].fill(e.sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_bounded_and_covers_the_view() {
        let mut cam = Camera::new(Vec2::splat(81_920.0), Vec2::new(2560.0, 1440.0));
        let mut nodes = Vec::new();
        for distance in [30.0, 400.0, 5_000.0, 40_000.0, cam.max_distance()] {
            cam.distance = distance;
            assert!(
                select_nodes(&cam, (-256.0, 768.0), &mut nodes),
                "node budget at {distance}"
            );
            assert!(
                !nodes.is_empty() && nodes.len() < 1500,
                "{} nodes at {distance}",
                nodes.len()
            );
            // The focus point is covered by exactly one node.
            let f = cam.focus.truncate();
            let covering = nodes
                .iter()
                .filter(|n| {
                    f.x >= n.rect[0]
                        && f.x < n.rect[0] + n.rect[2]
                        && f.y >= n.rect[1]
                        && f.y < n.rect[1] + n.rect[2]
                })
                .count();
            assert_eq!(covering, 1, "at {distance}");
        }
    }

    #[test]
    fn strategic_mesh_keeps_morphed_cells_below_four_pixels() {
        // Screen-space acceptance bound, including the doubled cell span at
        // the end of a morph. A coarse mesh changed narrow bays and ridges
        // even though the underlying heightmap had plenty of detail.
        for map_size in [10_240.0, 81_920.0] {
            let mut cam = Camera::new(Vec2::splat(map_size), Vec2::new(2560.0, 1440.0));
            let mut nodes = Vec::new();
            for distance in [5_000.0, 12_000.0, 40_000.0, cam.max_distance()] {
                cam.distance = distance.min(cam.max_distance());
                assert!(select_nodes(&cam, (-256.0, 768.0), &mut nodes));
                for node in &nodes {
                    if node.rect[3] == 0.0 {
                        continue; // The native 8 m map samples cannot subdivide further.
                    }
                    let origin = Vec2::new(node.rect[0], node.rect[1]);
                    let min = origin.extend(-256.0);
                    let max = (origin + Vec2::splat(node.rect[2]))
                        .min(cam.map_size)
                        .extend(768.0);
                    let nearest = aabb_distance(min, max, cam.eye()).max(1.0);
                    let morphed_cell_px =
                        node.rect[2] / 64.0 * 2.0 * cam.projection_scale() / nearest;
                    assert!(
                        morphed_cell_px <= 4.01,
                        "{morphed_cell_px:.2} px terrain cell at distance {} on {map_size} m map",
                        cam.distance
                    );
                }
            }
        }
    }
    #[test]
    fn grid_matches_the_sim_triangulation() {
        let (v, i) = grid_mesh();
        assert_eq!(v.len(), 65 * 65);
        assert_eq!(i.len(), 64 * 64 * 6);
        // First cell: both triangles share the (0,0)-(1,1) diagonal.
        let tri = |k: usize| [v[i[k] as usize], v[i[k + 1] as usize], v[i[k + 2] as usize]];
        assert!(tri(0).contains(&[0.0, 0.0]) && tri(0).contains(&[1.0, 1.0]));
        assert!(tri(3).contains(&[0.0, 0.0]) && tri(3).contains(&[1.0, 1.0]));
    }
}
