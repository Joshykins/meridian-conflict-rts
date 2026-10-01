//! Retaining walls: where a structure's lot was levelled into the ground, the
//! slanted strip of terrain between the lot's level and the ground round it is
//! clad in steel. Cosmetic only; the sim walks the same slope.
//!
//! The terrain is a heightfield of 8 m cells, so a levelled lot's edge samples
//! are shared with the cells round it and those cells slope from the lot's level
//! to the ground's. Each such cell (a cell whose corners belong to different
//! edits, or to an edit and the map) steeper than `MIN_SLOPE` gets a steel
//! plate laid on its slope, ribs running down the slope, and a cap rail along
//! its high edge. foundations.wgsl lays them on `terrain_height`, so they follow
//! the ground exactly, settling included.
//!
//! A lot levelled for a faction that builds with nanites (the Regency,
//! `mc_data::Construction::Nanite`) is clad in its own way: courses of dark armour
//! plate lapped down the slope, their edges cut into backswept points, over bronze.
//!
//! A new lot's plating comes up out of the ground as it settles (terrain.rs
//! `TileCache::settling`, `settle::SECONDS`).

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::settle;
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind};
use ash::vk;
use mc_map::{FlattenRecord, MapFile, CELL_SIZE_M, TILE_CELLS, TILE_SAMPLES};
use std::collections::HashMap;
use std::sync::Arc;

/// Clad cells drawn at most. A cosmetic cap: past it the newest slopes stay
/// bare. A lot has a few dozen, so this is thousands of structures.
const MAX_CELLS: usize = 65_536;
/// Vertices per cell, the larger of the two claddings' (foundations.wgsl). Steel:
/// the plate, three ribs of two segments (top and two sides) and the cap rail (top
/// and two sides). Armour: three courses of three plates (top, lip, and a point of
/// three triangles), and the cap rail.
const CELL_VERTICES: u32 = 3 * 3 * (6 + 6 + 9) + 18;
/// A levelled slope gentler than this (rise over run, as the sim's
/// `Heightfield::cell_slope` measures it) is left bare ground: 0.36 is about 20
/// degrees, a little under where land units stop climbing (mc-sim nav.rs
/// `MAX_SLOPE`, 1 in 2).
const MIN_SLOPE: f32 = 0.36;
/// Map tiles whose heights are kept for the next lot (132 KB each).
const GROUND_TILES_KEPT: usize = 24;
/// `FoundationCell::kind`: the slope runs along y (else along x) ...
const ALONG_Y: u32 = 1;
/// ... and rises toward the cell's far side (else its near side) ...
const HIGH_FAR: u32 = 2;
/// ... and is clad in lapped armour plate, not steel: its lot is a nanite-built
/// faction's.
const ARMOUR: u32 = 4;

/// One clad cell (foundations.wgsl `FoundationCell`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FoundationCell {
    /// The cell's low corner, metres.
    pub(crate) origin: [f32; 2],
    /// Render time the plating began to come up.
    pub(crate) start: f32,
    /// Which way the slope runs, and how it is clad: `ALONG_Y` | `HIGH_FAR` | `ARMOUR`.
    pub(crate) kind: u32,
}

/// A clad cell before it is packed.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Wall {
    kind: u32,
    start: f32,
}

pub(super) struct Foundations {
    map: Arc<MapFile>,
    /// The map's own heights, per tile, as read so far.
    ground: HashMap<(u32, u32), Vec<u16>>,
    /// The edits the walls are built for, in the sim's order.
    edits: Vec<FlattenRecord>,
    /// Whether each of `edits` is clad in armour (`ARMOUR`).
    armour: Vec<bool>,
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
            armour: Vec::new(),
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
    /// edit table, as `TileCache::apply_edits` takes it, and `factions` the faction
    /// each was levelled for), then the cells uploaded if any changed.
    pub(super) fn update(
        &mut self,
        all: &[FlattenRecord],
        factions: &[u8],
        blueprints: &mc_data::Blueprints,
        time: f32,
    ) {
        let replaced = all.len() < self.edits.len() || all[..self.edits.len()] != self.edits[..];
        if replaced {
            self.edits.clear();
            self.armour.clear();
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
            let armour = |i: usize| {
                factions
                    .get(i)
                    .and_then(|&f| blueprints.factions.get(f as usize))
                    .is_some_and(|f| f.construction == mc_data::Construction::Nanite)
            };
            self.armour
                .extend((self.edits.len()..all.len()).map(armour));
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
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let key = (cx, cy);
                let Some(mut kind) = grid.slope_kind(cx, cy, |s| min_z + s as f32 * step) else {
                    self.dirty |= self.walls.remove(&key).is_some();
                    continue;
                };
                // Clad as the newest lot it borders is.
                if grid
                    .newest(cx, cy)
                    .is_some_and(|o| self.armour[o as usize - 1])
                {
                    kind |= ARMOUR;
                }
                let wall = Wall { kind, start };
                match self.walls.get(&key) {
                    // Plating that lies as it did stays put: a neighbour's lot does
                    // not make it come up again.
                    Some(old) if old.kind == kind => {}
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
        let mut height = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
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
                height.push(samples[(ly * TILE_SAMPLES + lx) as usize]);
            }
        }
        Some(Window {
            x0,
            y0,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
            owner: vec![0; height.len()],
            height,
        })
    }

    /// Writes every clad cell to the GPU.
    fn upload(&mut self) {
        let cell = CELL_SIZE_M as f32;
        let mut keys: Vec<_> = self.walls.keys().copied().collect();
        // Any order draws the same; this one is stable, so the cap below drops the
        // same cells every time.
        keys.sort_unstable();
        let packed: Vec<FoundationCell> = keys
            .iter()
            // The cosmetic cap (`MAX_CELLS`): past it slopes stay bare.
            .take(MAX_CELLS)
            .map(|&(x, y)| {
                let wall = &self.walls[&(x, y)];
                FoundationCell {
                    origin: [x as f32 * cell, y as f32 * cell],
                    start: wall.start,
                    kind: wall.kind,
                }
            })
            .collect();
        self.cells.write(0, bytemuck::cast_slice(&packed));
        self.count = packed.len() as u32;
    }

    /// The clad cells, for the grass to keep off (grass.rs).
    pub(super) fn cells(&self) -> &Buffer {
        &self.cells
    }

    /// How many clad cells `cells` holds.
    pub(super) fn count(&self) -> u32 {
        self.count
    }

    /// Draws the plating in the current render pass: the scene, the depth pre-pass or
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

    /// The newest edit (`i + 1`) any corner of cell `(cx, cy)` belongs to; `None` if
    /// they are all the map's own ground.
    fn newest(&self, cx: u32, cy: u32) -> Option<u32> {
        [(cx, cy), (cx + 1, cy), (cx, cy + 1), (cx + 1, cy + 1)]
            .map(|(x, y)| self.at(x, y).0)
            .into_iter()
            .max()
            .filter(|&o| o > 0)
    }

    /// Whether cell `(cx, cy)` is a levelled step to clad, and if so which way its
    /// slope runs (`ALONG_Y`, `HIGH_FAR`): its corners belong to different lots (or a
    /// lot and the map) and one of its triangles is steeper than `MIN_SLOPE`.
    /// `metres` converts a sample.
    fn slope_kind(&self, cx: u32, cy: u32, metres: impl Fn(u16) -> f32) -> Option<u32> {
        let corners = [(cx, cy), (cx + 1, cy), (cx, cy + 1), (cx + 1, cy + 1)];
        let owners = corners.map(|(x, y)| self.at(x, y).0);
        if owners.iter().all(|&o| o == owners[0]) {
            return None;
        }
        let h = corners.map(|(x, y)| metres(self.at(x, y).1));
        // The cell's two triangles, split as the terrain's are (`cell_slope`).
        let cell = CELL_SIZE_M as f32;
        let lower = (h[1] - h[0]).hypot(h[3] - h[1]) / cell;
        let upper = (h[3] - h[2]).hypot(h[2] - h[0]) / cell;
        if lower.max(upper) < MIN_SLOPE {
            return None;
        }
        let gx = (h[1] + h[3] - h[0] - h[2]) * 0.5;
        let gy = (h[2] + h[3] - h[0] - h[1]) * 0.5;
        Some(if gx.abs() >= gy.abs() {
            if gx > 0.0 {
                HIGH_FAR
            } else {
                0
            }
        } else if gy > 0.0 {
            ALONG_Y | HIGH_FAR
        } else {
            ALONG_Y
        })
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

    fn metres(s: u16) -> f32 {
        s as f32 * 0.1
    }

    #[test]
    fn a_raised_lot_is_clad_on_the_slopes_round_it() {
        let w = window(140);
        // West of the lot the slope rises east, toward the lot: along x, high far.
        assert_eq!(w.slope_kind(1, 2, metres), Some(HIGH_FAR));
        // East of it the slope falls away east: along x, high near.
        assert_eq!(w.slope_kind(4, 3, metres), Some(0));
        // South of it the slope rises north.
        assert_eq!(w.slope_kind(3, 1, metres), Some(ALONG_Y | HIGH_FAR));
        // The lot itself and the ground past the ring are bare.
        assert_eq!(w.slope_kind(2, 2, metres), None);
        assert_eq!(w.slope_kind(0, 0, metres), None);
    }

    #[test]
    fn a_sunk_lot_is_clad_rising_away_from_it() {
        let w = window(60);
        assert_eq!(w.slope_kind(1, 2, metres), Some(0));
        assert_eq!(w.slope_kind(3, 4, metres), Some(ALONG_Y | HIGH_FAR));
    }

    #[test]
    fn a_cell_belongs_to_the_newest_lot_it_borders() {
        let mut w = window(140);
        // A second lot east of the first, sharing its edge samples.
        let next = FlattenRecord {
            min_x: 4,
            min_y: 2,
            max_x: 4,
            max_y: 3,
            sample: 120,
        };
        w.apply(&next, 2);
        assert_eq!(w.newest(1, 2), Some(1));
        assert_eq!(w.newest(4, 3), Some(2));
        assert_eq!(w.newest(0, 0), None);
    }

    #[test]
    fn only_a_slope_steeper_than_min_slope_is_clad() {
        // Samples are 0.1 m: a 2 m step over the 8 m cell is 0.25, bare ground;
        // 3.2 m is 0.4, clad.
        assert_eq!(window(120).slope_kind(1, 2, metres), None);
        assert_eq!(window(132).slope_kind(1, 2, metres), Some(HIGH_FAR));
    }
}
