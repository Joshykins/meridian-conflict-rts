//! Grass: fields of blades round the eye, grown on the GPU every frame wherever
//! the ground shows grass, and pressed by what is on it (grass_gen.wgsl, then
//! grass.wgsl draws them).
//!
//! Nothing about the grass is stored per map. Each frame, after the props are
//! culled: the units on the ground, boulders and ruins, the structures' lots, the scorch marks and
//! the track marks near the eye are gathered into one list; a trample map a
//! metre to the texel, 512 m on a side, is worked out from it (what stands on the
//! ground, what is burnt, which way the grass is being shoved, how flat it still
//! lies from what went over it); then one candidate tuft per half-metre cell
//! out to where a tuft shrinks under a few pixels is tested against the ground's
//! habitat (habitat.wgsl, the same weights the terrain paints with), thinned by
//! distance, and filed into one of three detail bands. The scene pass draws the
//! bands with one indirect call each, between the ground and the entities.
//!
//! Set 1 of every grass pipeline: 0 tufts, 1 draw commands (three
//! DrawIndexedIndirect, then the press count), 2 the trample map, 3 the press
//! list, 4 the renderer's stains (scorches, then lots), 5 its track marks.
//! `MERIDIAN_GRASS=0` turns it off, `MERIDIAN_GRASS_DENSITY` scales it.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::pipelines::{self, Blend, Depth, PipelineDesc, VertexKind};
use ash::vk;
use glam::Vec3;

/// Blades per tuft and segments per blade in each band, near to far. Mirrors
/// habitat.wgsl `GRASS_BAND_BLADES` / `GRASS_BAND_SEGMENTS`.
const BANDS: [(u32, u32); 3] = [(24, 4), (14, 3), (8, 2)];
/// Tufts each band holds. Mirrors `GRASS_BAND_CAP` (and `GRASS_BAND_FIRST`, the
/// running sum).
const BAND_CAP: [u32; 3] = [98304, 262144, 524288];
/// Mirrors habitat.wgsl `Tuft`.
const TUFT_BYTES: u64 = 48;
/// Mirrors grass_gen.wgsl `MAX_PRESS` and its 32-byte `Press`.
const MAX_PRESS: u64 = 4096;
const PRESS_BYTES: u64 = 32;
/// The trample map's side in metres (one texel each). Mirrors `WINDOW`.
const WINDOW: i32 = 512;
/// A candidate tuft per this many metres each way. Mirrors habitat.wgsl
/// `GRASS_CELL_M` (and MIN_PX `GRASS_MIN_PX`).
const CELL_M: f32 = 0.28;
/// A cell this many pixels across on screen still gets every tuft; smaller,
/// they thin with the square of it.
const FULL_PX: f32 = 4.5;
/// Below this many pixels a cell grows no grass at all: the terrain's own
/// meadow scan carries on from there.
const MIN_PX: f32 = 1.4;
/// Never grown further from the eye than this, whatever the resolution.
const MAX_REACH_M: f32 = 300.0;
/// Candidate cells at most, each way (the reach over the cell).
const MAX_CELLS: u32 = 2200;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GrassPush {
    grid: [f32; 4],
    dims: [u32; 4],
    window: [i32; 4],
    extra: [u32; 4],
    tune: [f32; 4],
}

/// What the renderer knows this frame that the grass needs.
pub(super) struct GrassFrame {
    pub eye: Vec3,
    /// Pixels a metre covers one metre from the eye (`Globals::lod.x`).
    pub projection_scale: f32,
    /// Ground height under the eye.
    pub ground: f32,
    pub dynamic_count: u32,
    /// Map props (static entities), for the boulders the grass grows round.
    pub static_count: u32,
    pub scorch_count: u32,
    pub lot_count: u32,
    pub track_count: u32,
    /// Seconds, `FrameInput::time`.
    pub time: f32,
}

pub(super) struct Grass {
    set_layout: vk::DescriptorSetLayout,
    compute_layout: vk::PipelineLayout,
    draw_layout: vk::PipelineLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    gen_module: vk::ShaderModule,
    draw_module: vk::ShaderModule,
    reset: vk::Pipeline,
    gather_units: vk::Pipeline,
    gather_props: vk::Pipeline,
    gather_stains: vk::Pipeline,
    gather_tracks: vk::Pipeline,
    trample_pass: vk::Pipeline,
    tufts_pass: vk::Pipeline,
    finish: vk::Pipeline,
    draw: vk::Pipeline,
    tufts: Buffer,
    args: Buffer,
    trample: Buffer,
    presses: Buffer,
    indices: Buffer,
    /// The trample map's origin last frame; None until it has one.
    window: Option<[i32; 2]>,
    frame: u32,
    last_time: Option<f32>,
    /// Whether any grass was grown this frame (else nothing is drawn).
    grown: bool,
    pub(super) enabled: bool,
    density: f32,
}

/// The index pattern of one band: each blade a strip up its height, left and
/// right at every level and a point at the tip (grass.wgsl `vs_grass`).
fn band_indices(blades: u32, segments: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let per = segments * 2 + 1;
    for blade in 0..blades {
        let v = blade * per;
        for i in 0..segments - 1 {
            let a = v + i * 2;
            out.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
        let a = v + (segments - 1) * 2;
        out.extend_from_slice(&[a, a + 1, v + segments * 2]);
    }
    out
}

impl Grass {
    pub(super) fn new(
        gpu: &Gpu,
        scene_set_layout: vk::DescriptorSetLayout,
        scene_pass: vk::RenderPass,
        stains: &Buffer,
        track_marks: &Buffer,
    ) -> Result<Grass, GpuError> {
        let dev = &gpu.device;
        use vk::DescriptorType as T;
        let stages = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT | vk::ShaderStageFlags::COMPUTE;
        let bindings: Vec<_> = (0..6)
            .map(|b| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b)
                    .descriptor_type(T::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(stages)
            })
            .collect();
        let set_layout = unsafe {
            dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)
        }?;
        let set_layouts = [scene_set_layout, set_layout];
        let compute_push = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            offset: 0,
            size: size_of::<GrassPush>() as u32,
        }];
        let compute_layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&compute_push),
                None,
            )
        }?;
        // The scene layout's push range, so set 0 stays bound across the switch
        // between it and this one.
        let draw_push = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 16,
        }];
        let draw_layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&draw_push),
                None,
            )
        }?;
        let sizes = [vk::DescriptorPoolSize { ty: T::STORAGE_BUFFER, descriptor_count: 6 }];
        let pool = unsafe {
            dev.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None)
        }?;
        let own = [set_layout];
        let set = unsafe {
            dev.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&own))
        }?[0];

        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let tufts = gpu.device_buffer(BAND_CAP.iter().sum::<u32>() as u64 * TUFT_BYTES, storage)?;
        let args = gpu.device_buffer(20 * 4, storage | vk::BufferUsageFlags::INDIRECT_BUFFER)?;
        let trample = gpu.device_buffer((WINDOW * WINDOW) as u64 * 8, storage)?;
        let presses = gpu.device_buffer(MAX_PRESS * PRESS_BYTES, storage)?;
        let mut all = Vec::new();
        for &(blades, segments) in &BANDS {
            all.extend(band_indices(blades, segments));
        }
        let indices = gpu.buffer_with_data(bytemuck::cast_slice(&all), vk::BufferUsageFlags::INDEX_BUFFER)?;
        let infos: Vec<[vk::DescriptorBufferInfo; 1]> = [&tufts, &args, &trample, &presses, stains, track_marks]
            .iter()
            .map(|b| [b.info()])
            .collect();
        let writes: Vec<_> = infos
            .iter()
            .enumerate()
            .map(|(b, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(b as u32)
                    .descriptor_type(T::STORAGE_BUFFER)
                    .buffer_info(info)
            })
            .collect();
        unsafe { dev.update_descriptor_sets(&writes, &[]) };

        let gen_module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/grass_gen.spv")))?;
        let draw_module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/grass.spv")))?;
        let compute = |entry| pipelines::compute_pipeline(gpu, gen_module, entry, compute_layout);
        let draw = pipelines::graphics_pipeline(
            gpu,
            &PipelineDesc {
                module: draw_module,
                vs: c"vs_grass",
                fs: c"fs_grass",
                layout: draw_layout,
                pass: scene_pass,
                vertex: VertexKind::None,
                blend: Blend::Opaque,
                depth: Depth::TestWrite,
                cull: vk::CullModeFlags::NONE,
            },
        )?;
        let enabled = std::env::var("MERIDIAN_GRASS").map_or(true, |v| v != "0");
        let density = std::env::var("MERIDIAN_GRASS_DENSITY").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        Ok(Grass {
            set_layout,
            compute_layout,
            draw_layout,
            pool,
            set,
            gen_module,
            draw_module,
            reset: compute(c"cs_reset")?,
            gather_units: compute(c"cs_gather_units")?,
            gather_props: compute(c"cs_gather_props")?,
            gather_stains: compute(c"cs_gather_stains")?,
            gather_tracks: compute(c"cs_gather_tracks")?,
            trample_pass: compute(c"cs_trample")?,
            tufts_pass: compute(c"cs_tufts")?,
            finish: compute(c"cs_finish")?,
            draw,
            tufts,
            args,
            trample,
            presses,
            indices,
            window: None,
            frame: 0,
            last_time: None,
            grown: false,
            enabled,
            density,
        })
    }

    /// Outside any render pass, after the cull and before the shadow pass, with
    /// this frame's globals, stains and track marks uploaded. Leaves the tufts and
    /// draw commands ready for `draw`.
    pub(super) fn record(&mut self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet, f: &GrassFrame) {
        self.grown = false;
        if !self.enabled {
            return;
        }
        let dev = &gpu.device;
        // Out to where a cell shrinks under MIN_PX; nothing at all once the
        // ground is that far below the eye.
        let reach = (CELL_M * f.projection_scale / MIN_PX).min(MAX_REACH_M);
        if f.eye.z - f.ground > reach * 0.97 {
            self.window = None;
            return;
        }
        self.grown = true;
        let lo = ((f.eye.truncate() - reach) / CELL_M).floor() * CELL_M;
        let cells = ((reach * 2.0 / CELL_M).ceil() as u32 + 1).min(MAX_CELLS);
        let origin = [f.eye.x.floor() as i32 - WINDOW / 2, f.eye.y.floor() as i32 - WINDOW / 2];
        let before = self.window.unwrap_or(origin);
        let forget = self.window.is_none()
            || (origin[0] - before[0]).abs() >= WINDOW
            || (origin[1] - before[1]).abs() >= WINDOW;
        self.window = Some(origin);
        self.frame = self.frame.wrapping_add(1);
        let dt = self.last_time.map_or(0.0, |t| f.time - t);
        self.last_time = Some(f.time);
        let push = GrassPush {
            grid: [lo.x, lo.y, CELL_M, reach],
            dims: [cells, cells, f.scorch_count, f.lot_count],
            window: [origin[0], origin[1], before[0], before[1]],
            extra: [f.track_count, forget as u32, self.frame, 0],
            tune: [dt.clamp(0.0, 0.5), self.density, FULL_PX, MIN_PX],
        };
        let barrier = |dst_access: vk::AccessFlags, dst_stage: vk::PipelineStageFlags| unsafe {
            let b = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(dst_access)];
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::COMPUTE_SHADER, dst_stage, vk::DependencyFlags::empty(), &b, &[], &[]);
        };
        let rw = vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
        let cs = vk::PipelineStageFlags::COMPUTE_SHADER;
        unsafe {
            // Last frame's draw read the tufts and the commands this rewrites.
            let before_draw = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::INDIRECT_COMMAND_READ)
                .dst_access_mask(rw)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::DRAW_INDIRECT,
                cs,
                vk::DependencyFlags::empty(),
                &before_draw,
                &[],
                &[],
            );
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, self.compute_layout, 0, &[scene_set, self.set], &[]);
            dev.cmd_push_constants(cmd, self.compute_layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(&push));
            let run = |pipeline: vk::Pipeline, x: u32, y: u32| {
                if x > 0 && y > 0 {
                    dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
                    dev.cmd_dispatch(cmd, x, y, 1);
                }
            };
            run(self.reset, 1, 1);
            barrier(rw, cs);
            run(self.gather_units, f.dynamic_count.div_ceil(64), 1);
            run(self.gather_props, f.static_count.div_ceil(64), 1);
            run(self.gather_stains, (f.scorch_count + f.lot_count).div_ceil(64), 1);
            run(self.gather_tracks, f.track_count.div_ceil(64), 1);
            barrier(rw, cs);
            run(self.trample_pass, WINDOW as u32 / 16, WINDOW as u32 / 16);
            barrier(rw, cs);
            run(self.tufts_pass, cells.div_ceil(8), cells.div_ceil(8));
            barrier(rw, cs);
            run(self.finish, 1, 1);
        }
        barrier(
            vk::AccessFlags::SHADER_READ | vk::AccessFlags::INDIRECT_COMMAND_READ,
            vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::DRAW_INDIRECT,
        );
    }

    /// Inside the scene pass, after the ground and its decals. Leaves set 0
    /// bound for the scene's layout (the two layouts share it).
    pub(super) fn draw(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if !self.grown {
            return;
        }
        let dev = &gpu.device;
        unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.draw);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.draw_layout, 0, &[scene_set, self.set], &[]);
            dev.cmd_bind_index_buffer(cmd, self.indices.buffer, 0, vk::IndexType::UINT32);
            // Near to far, so the nearest blades hide the most behind them.
            for band in 0..BANDS.len() as u64 {
                dev.cmd_draw_indexed_indirect(cmd, self.args.buffer, band * 20, 1, 20);
            }
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        unsafe {
            let dev = &gpu.device;
            for p in [
                self.reset,
                self.gather_units,
                self.gather_props,
                self.gather_stains,
                self.gather_tracks,
                self.trample_pass,
                self.tufts_pass,
                self.finish,
                self.draw,
            ] {
                dev.destroy_pipeline(p, None);
            }
            dev.destroy_shader_module(self.gen_module, None);
            dev.destroy_shader_module(self.draw_module, None);
            dev.destroy_pipeline_layout(self.compute_layout, None);
            dev.destroy_pipeline_layout(self.draw_layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
        for b in [&mut self.tufts, &mut self.args, &mut self.trample, &mut self.presses, &mut self.indices] {
            gpu.destroy_buffer(std::mem::replace(b, Buffer::null()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_indices_cover_every_blade_once() {
        for &(blades, segments) in &BANDS {
            let idx = band_indices(blades, segments);
            assert_eq!(idx.len() as u32, blades * (segments * 2 - 1) * 3);
            assert_eq!(*idx.iter().max().unwrap(), blades * (segments * 2 + 1) - 1);
        }
    }

    #[test]
    fn band_slots_do_not_overlap() {
        // habitat.wgsl GRASS_BAND_FIRST is the running sum of the caps.
        assert_eq!(BAND_CAP[0], 98304);
        assert_eq!(BAND_CAP[0] + BAND_CAP[1], 360448);
    }
}
