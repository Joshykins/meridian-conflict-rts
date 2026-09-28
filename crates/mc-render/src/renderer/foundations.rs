//! Foundation walls: where a structure's lot was levelled into the ground, the
//! drawn terrain steps down (or up) at a wall instead of a ramp. Cosmetic only;
//! the sim still walks the ramp.
//!
//! The terrain is a heightfield of 8 m cells, so a levelled lot's edge samples
//! are shared with the cells round it and those cells slope from the lot's level
//! to the ground's. Each such cell (a cell whose corners belong to different
//! edits, or to an edit and the map) gets a block drawn over it: its top is flat
//! across the step at the higher side's height, so the ramp lies inside it, and
//! its sides run down into the ground. Where the lot stands proud, the wall faces
//! out; where it is cut into a slope, the wall holds the slope back and faces
//! in. The top is kept per sample (`wall_top`), so neighbouring blocks meet
//! without a seam and a lot's corners close.
//!
//! A new lot's walls rise out of the ground as it settles (terrain.rs
//! `TileCache::settling`, `settle::SECONDS`); foundations.wgsl draws them.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::settle;
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind};
use ash::vk;
use mc_map::{FlattenRecord, MapFile, CELL_SIZE_M, TILE_CELLS, TILE_SAMPLES};
use std::collections::HashMap;
use std::sync::Arc;

/// Wall cells drawn at most. A cosmetic cap: past it the newest steps show
/// the plain ramp. A lot has a few dozen, so this is thousands of structures.
const MAX_CELLS: usize = 65_536;
/// Vertices per cell: the top's two triangles, then four sides of two each.
const CELL_VERTICES: u32 = 30;
/// A step lower than this is left as the ramp: a wall would only z-fight the ground.
const MIN_STEP_M: f32 = 0.5;
/// The wall's coping stands this proud of the ground it meets.
const LIP_M: f32 = 0.15;
/// Map tiles whose heights are kept for the next lot (132 KB each).
const GROUND_TILES_KEPT: usize = 24;
/// How far the sides reach below the lowest ground they meet.
const FOOTING_M: f32 = 1.5;

/// One wall block (foundations.wgsl `FoundationCell`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FoundationCell {
    /// The cell's low corner, metres.
    pub(crate) origin: [f32; 2],
    /// Where the sides end, metres.
    pub(crate) bottom: f32,
    /// Render time the wall began to rise.
    pub(crate) start: f32,
    /// The top at the corners (x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1), metres.
    pub(crate) top: [f32; 4],
    /// Sides drawn: bit 0 -x, 1 +x, 2 -y, 3 +y. A side against another wall cell is not.
    pub(crate) faces: u32,
    _pad: [u32; 3],
}

/// A wall block before it is packed: what the heights call for, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wall {
    top: [f32; 4],
    bottom: f32,
    start: f32,
}

pub(super) struct Foundations {
    map: Arc<MapFile>,
    /// The map's own heights, per tile, as read so far.
    ground: HashMap<(u32, u32), Vec<u16>>,
    /// The edits the walls are built for, in the sim's order.
    edits: Vec<FlattenRecord>,
    synced: bool,
    walls: HashMap<(u32, u32), Wall>,
    dirty: bool,
    count: u32,
    cells: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    draw: vk::Pipeline,
    shadow: vk::Pipeline,
    prepass: vk::Pipeline,
}

impl Foundations {
    pub(super) fn new(
        gpu: &Gpu,
        layouts: &Layouts,
        passes: &Passes,
        map: Arc<MapFile>,
    ) -> Result<Foundations, GpuError> {
        let dev = &gpu.device;
        let cells = gpu.host_buffer(
            (MAX_CELLS * size_of::<FoundationCell>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        let sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
            descriptor_count: 2,
        }];
        // SAFETY: the device is alive and `sizes` lives to the end of the call.
        let pool = unsafe {
            dev.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let own = [layouts.pass_set];
        // SAFETY: the pool was made just above for exactly this one pass set of two storage
        // buffers, and `own` lives to the end of the call.
        let set = unsafe {
            dev.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&own),
            )
        }?[0];
        // Both of the pass set's bindings name the cells; the shader reads the first.
        let info = [cells.info()];
        let writes = [0, 1].map(|b| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(b)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&info)
        });
        // SAFETY: `set` is fresh and unused by any command buffer; each write names one of its
        // two storage-buffer bindings and the live `cells`, and `writes`/`info` live to the end
        // of the call.
        unsafe { dev.update_descriptor_sets(&writes, &[]) };

        let module = gpu.shader(crate::shader_reload::spirv!("foundations"))?;
        let pipeline = |fs, pass, depth, blend| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs: c"vs_main",
                    fs,
                    layout: layouts.scene,
                    pass,
                    vertex: VertexKind::None,
                    blend,
                    depth,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        let draw = pipeline(c"fs_main", passes.scene, Depth::TestWrite, Blend::Opaque)?;
        let shadow = pipeline(c"fs_shadow", passes.shadow, Depth::Shadow, Blend::NoColor)?;
        let prepass = pipeline(
            c"fs_shadow",
            passes.shadow,
            Depth::TestWrite,
            Blend::NoColor,
        )?;
        Ok(Foundations {
            map,
            ground: HashMap::new(),
            edits: Vec::new(),
            synced: false,
            walls: HashMap::new(),
            dirty: false,
            count: 0,
            cells,
            pool,
            set,
            module,
            draw,
            shadow,
            prepass,
        })
    }

    /// Walls for the edits the renderer has not seen yet (`all` is the sim's whole
    /// edit table, as `TileCache::apply_edits` takes it), then the cells uploaded
    /// if any changed.
    pub(super) fn update(&mut self, all: &[FlattenRecord], time: f32) {
        let replaced = all.len() < self.edits.len() || all[..self.edits.len()] != self.edits[..];
        if replaced {
            self.edits.clear();
            self.walls.clear();
            self.dirty = true;
        }
        // Walls already there when the renderer first sees the match stand already.
        let start = if replaced || !self.synced {
            time - settle::SECONDS
        } else {
            time
        };
        self.synced = true;
        if all.len() > self.edits.len() {
            let fresh = all.len() - self.edits.len();
            self.edits.extend_from_slice(&all[self.edits.len()..]);
            // A snapshot's whole history is rebuilt a lot at a time, merged where lots touch.
            for i in self.edits.len() - fresh..self.edits.len() {
                let e = self.edits[i];
                self.rebuild(
                    (e.min_x as u32).saturating_sub(1),
                    (e.min_y as u32).saturating_sub(1),
                    e.max_x as u32 + 1,
                    e.max_y as u32 + 1,
                    start,
                );
            }
        }
        if self.dirty {
            self.dirty = false;
            self.upload();
        }
    }

    /// Works out the wall cells `x0..=x1`, `y0..=y1` again from the map and the edits.
    fn rebuild(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, start: f32) {
        let (w, h) = self.map.info().size_cells();
        let (x1, y1) = (x1.min(w - 1), y1.min(h - 1));
        // Every sample a cell's corners and their neighbours touch.
        let (sx0, sy0) = (x0.saturating_sub(1), y0.saturating_sub(1));
        let (sx1, sy1) = ((x1 + 2).min(w), (y1 + 2).min(h));
        let Some(mut grid) = self.window(sx0, sy0, sx1, sy1) else {
            return;
        };
        for (i, e) in self.edits.iter().enumerate() {
            grid.apply(e, i as u32 + 1);
        }
        let info = self.map.info();
        let (min_z, step) = (info.min_z.to_f32(), info.z_step.to_f32());
        let metres = |s: u16| min_z + s as f32 * step;
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let corners = [(cx, cy), (cx + 1, cy), (cx, cy + 1), (cx + 1, cy + 1)];
                let owners = corners.map(|(x, y)| grid.at(x, y).0);
                let key = (cx, cy);
                if owners.iter().all(|&o| o == owners[0]) {
                    self.dirty |= self.walls.remove(&key).is_some();
                    continue;
                }
                let tops = corners.map(|(x, y)| metres(grid.wall_top(x, y)));
                let ground = corners.map(|(x, y)| metres(grid.at(x, y).1));
                let rise = (0..4).map(|k| tops[k] - ground[k]).fold(0.0, f32::max);
                if rise < MIN_STEP_M {
                    self.dirty |= self.walls.remove(&key).is_some();
                    continue;
                }
                let lowest = corners
                    .map(|(x, y)| metres(grid.at(x, y).1.min(grid.original(x, y))))
                    .into_iter()
                    .fold(f32::MAX, f32::min);
                let wall = Wall {
                    top: tops.map(|t| t + LIP_M),
                    bottom: lowest - FOOTING_M,
                    start,
                };
                match self.walls.get(&key) {
                    // A wall that stands as it did keeps standing: a neighbour's lot
                    // does not make it rise again.
                    Some(old) if old.top == wall.top && old.bottom == wall.bottom => {}
                    _ => {
                        self.walls.insert(key, wall);
                        self.dirty = true;
                    }
                }
            }
        }
    }

    /// The map's heights over the samples `x0..=x1`, `y0..=y1`, before any edit.
    fn window(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) -> Option<Window> {
        let (tiles_w, tiles_h) = self.map.size_tiles();
        let mut original = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
        for y in y0..=y1 {
            for x in x0..=x1 {
                // A tile's last row and column are its neighbour's first.
                let tile = (
                    (x / TILE_CELLS).min(tiles_w - 1),
                    (y / TILE_CELLS).min(tiles_h - 1),
                );
                if !self.ground.contains_key(&tile) {
                    // Kept to a few megabytes: tiles are read again when needed.
                    if self.ground.len() >= GROUND_TILES_KEPT {
                        self.ground.clear();
                    }
                    match self.map.read_tile(tile.0, tile.1) {
                        Ok(samples) => {
                            self.ground.insert(tile, samples);
                        }
                        Err(e) => {
                            log::error!("foundation walls: map tile {tile:?}: {e}");
                            return None;
                        }
                    }
                }
                let samples = &self.ground[&tile];
                let (lx, ly) = (x - tile.0 * TILE_CELLS, y - tile.1 * TILE_CELLS);
                original.push(samples[(ly * TILE_SAMPLES + lx) as usize]);
            }
        }
        Some(Window {
            x0,
            y0,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
            owner: vec![0; original.len()],
            height: original.clone(),
            original,
        })
    }

    /// Writes every wall to the GPU, with the sides against other walls left out.
    fn upload(&mut self) {
        let cell = CELL_SIZE_M as f32;
        let mut keys: Vec<_> = self.walls.keys().copied().collect();
        // Oldest ground first would be nicer, but any order draws the same; this
        // one is stable, so the cap below drops the same cells every time.
        keys.sort_unstable();
        let packed: Vec<FoundationCell> = keys
            .iter()
            // The cosmetic cap (`MAX_CELLS`): the ramp shows past it.
            .take(MAX_CELLS)
            .map(|&(x, y)| {
                let wall = &self.walls[&(x, y)];
                let near = [
                    x.checked_sub(1).map(|x| (x, y)),
                    Some((x + 1, y)),
                    y.checked_sub(1).map(|y| (x, y)),
                    Some((x, y + 1)),
                ];
                let faces = near.iter().enumerate().fold(0, |m, (bit, n)| {
                    if n.is_some_and(|n| self.walls.contains_key(&n)) {
                        m
                    } else {
                        m | 1 << bit
                    }
                });
                FoundationCell {
                    origin: [x as f32 * cell, y as f32 * cell],
                    bottom: wall.bottom,
                    start: wall.start,
                    top: wall.top,
                    faces,
                    _pad: [0; 3],
                }
            })
            .collect();
        self.cells.write(0, bytemuck::cast_slice(&packed));
        self.count = packed.len() as u32;
    }

    /// Draws the walls in the current render pass: the scene, the depth pre-pass or
    /// a shadow cascade, by `pass_kind`. Leaves set 0 bound; set 1 is its own.
    pub(super) fn record(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        pass_kind: u32,
    ) {
        if self.count == 0 {
            return;
        }
        let pipeline =
            if pass_kind & crate::gpu_consts::pass::KIND_MASK == crate::gpu_consts::pass::SHADOW {
                self.shadow
            } else if pass_kind == crate::gpu_consts::pass::PREPASS {
                self.prepass
            } else {
                self.draw
            };
        let dev = &gpu.device;
        // SAFETY: the renderer calls this inside a render pass of the matching kind while `cmd`
        // is recording, with set 0 bound for `layout` (the scene layout the pipelines were
        // made with); the 8 bytes pushed fit its 16-byte vertex+fragment range, and the draw
        // reads `count` cells, all written by `upload`.
        unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set],
                &[],
            );
            dev.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&[pass_kind, 0u32]),
            );
            dev.cmd_draw(cmd, CELL_VERTICES, self.count, 0, 0);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their module.
        unsafe {
            let dev = &gpu.device;
            for p in [self.draw, self.shadow, self.prepass] {
                dev.destroy_pipeline(p, None);
            }
            dev.destroy_shader_module(self.module, None);
            dev.destroy_descriptor_pool(self.pool, None);
        }
        gpu.destroy_buffer(std::mem::replace(&mut self.cells, Buffer::null()));
    }
}

/// Heights and owners over a window of samples: owner 0 is the map's own ground,
/// `i + 1` the edit `i` that last levelled the sample.
struct Window {
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
    owner: Vec<u32>,
    height: Vec<u16>,
    original: Vec<u16>,
}

impl Window {
    fn index(&self, x: u32, y: u32) -> Option<usize> {
        let (lx, ly) = (x.checked_sub(self.x0)?, y.checked_sub(self.y0)?);
        (lx < self.w && ly < self.h).then_some((ly * self.w + lx) as usize)
    }

    /// Owner and height of a sample inside the window.
    fn at(&self, x: u32, y: u32) -> (u32, u16) {
        let i = self.index(x, y).expect("sample inside the window");
        (self.owner[i], self.height[i])
    }

    fn original(&self, x: u32, y: u32) -> u16 {
        self.original[self.index(x, y).expect("sample inside the window")]
    }

    fn apply(&mut self, e: &FlattenRecord, owner: u32) {
        let ((ex0, ey0), (ex1, ey1)) = e.sample_rect();
        let (x0, y0) = (ex0.max(self.x0), ey0.max(self.y0));
        let (x1, y1) = (ex1.min(self.x0 + self.w - 1), ey1.min(self.y0 + self.h - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = ((y - self.y0) * self.w + (x - self.x0)) as usize;
                self.owner[i] = owner;
                self.height[i] = e.sample;
            }
        }
    }

    /// The wall's top at a sample: its own height, or the highest of its eight
    /// neighbours that belong to another lot (or to the map), so the top is flat
    /// across a step and covers the ramp under it.
    fn wall_top(&self, x: u32, y: u32) -> u16 {
        let (own, mut top) = self.at(x, y);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                if let Some(i) = self.index(nx as u32, ny as u32) {
                    if self.owner[i] != own {
                        top = top.max(self.height[i]);
                    }
                }
            }
        }
        top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 6 x 6 sample window of flat ground at 100, with a lot levelled at `level`
    /// over the cells (2, 2)..=(3, 3), so samples 2..=4 each way.
    fn window(level: u16) -> Window {
        let n = 36;
        let mut w = Window {
            x0: 0,
            y0: 0,
            w: 6,
            h: 6,
            owner: vec![0; n],
            height: vec![100; n],
            original: vec![100; n],
        };
        let lot = FlattenRecord {
            min_x: 2,
            min_y: 2,
            max_x: 3,
            max_y: 3,
            sample: level,
        };
        w.apply(&lot, 1);
        w
    }

    #[test]
    fn a_raised_lot_is_walled_at_its_level_out_to_the_ground() {
        let w = window(140);
        // The ring cell (1, 2): its lot corners and its outer corners all top out
        // at the lot's level, so the ramp from 140 down to 100 is inside the block.
        for (x, y) in [(1, 2), (2, 2), (1, 3), (2, 3)] {
            assert_eq!(w.wall_top(x, y), 140, "{x},{y}");
        }
        // Past the ring the ground is its own.
        assert_eq!(w.wall_top(0, 3), 100);
    }

    #[test]
    fn a_sunk_lot_is_walled_at_the_ground_down_to_its_level() {
        let w = window(60);
        // The lot's edge sample carries the ground's height, so the wall faces in.
        assert_eq!(w.wall_top(2, 3), 100);
        assert_eq!(w.wall_top(1, 3), 100);
        // Inside the lot, past its edge, the floor is the lot's.
        assert_eq!(w.wall_top(3, 3), 60);
    }
}
