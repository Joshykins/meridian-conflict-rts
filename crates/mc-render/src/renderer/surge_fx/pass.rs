//! Surge shells' wakes drawn in light (surge_fx.wgsl): the buffer of live surges, its
//! descriptor set and the two pipelines that lay them over the scene after the water, the
//! burnt disc each leaves and the ripple at its front. What each surge is, frame by frame,
//! comes from `surge_fx`.

use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::surge_fx::{MAX_SURGES, RINGS, SECTION, SEGMENTS};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, PASS_SET};
use crate::shader_reload::spirv;

/// One surge's wake (`SurgeWake` in surge_fx.wgsl). Times are the renderer's clock.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuSurge {
    /// The crater, and when the shell landed.
    pub(crate) centre: [f32; 3],
    pub(crate) start: f32,
    /// Metres out it rolls, and metres a second.
    pub(crate) radius: f32,
    pub(crate) speed: f32,
    /// Seconds the ground burns once the front has rolled out.
    pub(crate) linger: f32,
    /// Sets each surge's noise apart.
    pub(crate) seed: f32,
    /// How hard it lands (`Weapon::impact`): the front's height and light.
    pub(crate) power: f32,
    pub(crate) _pad0: f32,
    pub(crate) _pad1: f32,
    pub(crate) _pad2: f32,
}

pub(super) struct SurgePass {
    buffer: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    /// The burnt disc, and the front over it.
    ground: vk::Pipeline,
    front: vk::Pipeline,
    /// Surges written this frame.
    count: u32,
}

impl SurgePass {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        let dev = &gpu.device;
        let buffer = gpu.host_buffer(
            (MAX_SURGES as usize * size_of::<GpuSurge>()) as u64,
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
        let module = gpu.shader(spirv!("surge_fx"))?;
        let pipeline = |vs, fs| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    layout: layouts.scene,
                    pass: passes.scene_over,
                    vertex: VertexKind::None,
                    blend: Blend::Premultiplied,
                    depth: Depth::Test,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        Ok(Self {
            ground: pipeline(c"vs_surge_ground", c"fs_surge_ground")?,
            front: pipeline(c"vs_surge_front", c"fs_surge_front")?,
            buffer,
            pool,
            set,
            module,
            count: 0,
        })
    }

    /// This frame's surges. Past `MAX_SURGES` the rest are not drawn: `surge_fx` keeps no
    /// more than that.
    pub(super) fn upload(&mut self, surges: &[GpuSurge]) {
        let surges = &surges[..surges.len().min(MAX_SURGES as usize)];
        self.buffer.write(0, bytemuck::cast_slice(surges));
        self.count = surges.len() as u32;
    }

    /// Inside `scene_over` with the scene's set 0 bound: lays the surges over the scene,
    /// the burnt ground and then the fronts.
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
    ) {
        if self.count == 0 {
            return;
        }
        // SAFETY: the renderer calls this inside `scene_over` while `cmd` is recording;
        // both pipelines were made for that pass with the scene layout `layout`, and `set`
        // holds the live surge buffer.
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.ground);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set],
                &[],
            );
            device.cmd_draw(cmd, RINGS * SEGMENTS * 6, self.count, 0, 0);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.front);
            device.cmd_draw(cmd, SECTION * SEGMENTS * 6, self.count, 0, 0);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
        // SAFETY: these were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipelines before their module.
        unsafe {
            gpu.device.destroy_pipeline(self.ground, None);
            gpu.device.destroy_pipeline(self.front, None);
            gpu.device.destroy_shader_module(self.module, None);
            gpu.device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
