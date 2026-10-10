//! The gravitic lenses drawn (lens_fx.wgsl): the buffer of lenses at work, its descriptor
//! set and the four pipelines that lay them over the scene after the water. Each reads
//! the scene the water sees through (the copy taken before the water, `Renderer::refract`,
//! scene set 2) to bend it: no light of its own beyond a faint rim. What each lens is,
//! frame by frame, comes from `lens_fx`.

use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::lens_fx::{
    BALLS, DOME_RINGS, MAX_LENSES, RIBBONS, RIBBON_QUADS, RINGS, SEGMENTS, SHARDS,
};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, PASS_SET};
use crate::shader_reload::spirv;

/// One lens at work (`LensWork` in lens_fx.wgsl). Times are the renderer's clock, below
/// zero for what has not happened.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuLens {
    /// The emitter, and when its charge began.
    pub(crate) emitter: [f32; 3],
    pub(crate) charge: f32,
    /// Where the front is (struck: the middle of the target), and when it was let go.
    pub(crate) front: [f32; 3],
    pub(crate) launch: f32,
    /// The ground under the target, the well's foot, and when the front struck.
    pub(crate) ground: [f32; 3],
    pub(crate) strike: f32,
    /// The well's radius, metres; seconds the charge takes; when the well let go.
    pub(crate) radius: f32,
    pub(crate) charge_time: f32,
    pub(crate) release: f32,
    /// Sets each lens's noise and grit apart.
    pub(crate) seed: f32,
    /// 0 gone, 1 full (one lost from sight fades out).
    pub(crate) fade: f32,
    /// Metres the front has run so far.
    pub(crate) run: f32,
    pub(crate) _pad: [f32; 2],
}

pub(super) struct LensPass {
    buffer: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    /// The well on the ground and its dome, the grit in it, the rippled air, and the
    /// charge, front and strike, in the order drawn.
    well: vk::Pipeline,
    shards: vk::Pipeline,
    ribbons: vk::Pipeline,
    balls: vk::Pipeline,
    /// Lenses written this frame.
    count: u32,
}

impl LensPass {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        let dev = &gpu.device;
        let buffer = gpu.host_buffer(
            (MAX_LENSES as usize * size_of::<GpuLens>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        let mut sets = SetPool::new(gpu, &[(PASS_SET, 1)])?;
        let set = sets.alloc(gpu, layouts.pass_set, PASS_SET)?;
        let pool = sets.into_raw();
        let infos = [vk::DescriptorBufferInfo {
            buffer: buffer.buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }];
        let writes = [0, 1].map(|binding| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&infos)
        });
        // SAFETY: `set` was just allocated and no command buffer uses it yet; `buffer` is a
        // live storage buffer of this device, and `writes`/`infos` live to the end of the call.
        unsafe { dev.update_descriptor_sets(&writes, &[]) };
        let module = gpu.shader(spirv!("lens_fx"))?;
        let pipeline = |vs, fs| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    // Set 2 holds the scene the water sees through, which the lens bends.
                    layout: layouts.water,
                    pass: passes.scene_over,
                    vertex: VertexKind::None,
                    blend: Blend::Premultiplied,
                    depth: Depth::Test,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        Ok(Self {
            well: pipeline(c"vs_lens_well", c"fs_lens_well")?,
            shards: pipeline(c"vs_lens_shard", c"fs_lens_shard")?,
            ribbons: pipeline(c"vs_lens_ribbon", c"fs_lens_ribbon")?,
            balls: pipeline(c"vs_lens_ball", c"fs_lens_ball")?,
            buffer,
            pool,
            set,
            module,
            count: 0,
        })
    }

    /// This frame's lenses. Past `MAX_LENSES` the rest are not drawn: `lens_fx` keeps no
    /// more than that.
    pub(super) fn upload(&mut self, lenses: &[GpuLens]) {
        let lenses = &lenses[..lenses.len().min(MAX_LENSES as usize)];
        self.buffer.write(0, bytemuck::cast_slice(lenses));
        self.count = lenses.len() as u32;
    }

    /// Inside `scene_over` with the scene's set 0 bound: lays the lenses over the scene.
    /// `layout` is the water's pipeline layout, `seen` the water's set (the scene copied
    /// before the water, and its depth).
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        seen: vk::DescriptorSet,
    ) {
        if self.count == 0 {
            return;
        }
        let ground = (RINGS + DOME_RINGS) * SEGMENTS * 6;
        // SAFETY: the renderer calls this inside `scene_over` while `cmd` is recording;
        // every pipeline was made for that pass with the water's layout `layout`, `set`
        // holds the live lens buffer and `seen` is the live water set.
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set, seen],
                &[],
            );
            for (pipeline, vertices) in [
                (self.well, ground),
                (self.shards, SHARDS * 24),
                (self.ribbons, RIBBONS * RIBBON_QUADS * 6),
                (self.balls, BALLS * 6),
            ] {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                device.cmd_draw(cmd, vertices, self.count, 0, 0);
            }
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
        // SAFETY: these were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipelines before their module.
        unsafe {
            for p in [self.well, self.shards, self.ribbons, self.balls] {
                gpu.device.destroy_pipeline(p, None);
            }
            gpu.device.destroy_shader_module(self.module, None);
            gpu.device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
