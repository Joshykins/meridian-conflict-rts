//! Cone weapons' wakes drawn in light (wake_shell.wgsl): the buffer of live wakes, its
//! descriptor set and the two pipelines that lay them over the scene after the water, the
//! trail each leaves and the wall of plasma at its front. What each wake is, frame by frame,
//! comes from `wake_fx`.

use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::wake_shell::{ALONG, ARCH, AROUND, MAX_SHELLS, TUBE};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind};
use crate::shader_reload::spirv;

/// One wake's shell (`WakeShell` in wake_shell.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuWakeShell {
    /// The muzzle, and the renderer time the wake left it.
    pub(crate) apex: [f32; 3],
    pub(crate) start: f32,
    /// Which way it rolls (unit length), and the fan's half-angle, radians.
    pub(crate) ahead: [f32; 2],
    pub(crate) half: f32,
    /// Metres out the front stands now, and metres a second it rolls.
    pub(crate) front: f32,
    pub(crate) speed: f32,
    /// The wall's height over the middle of the fan, metres.
    pub(crate) height: f32,
    /// 0 gone, 1 full.
    pub(crate) fade: f32,
    /// Seconds since the front ran out; below 0 while it rolls.
    pub(crate) spent: f32,
}

pub(super) struct WakeShells {
    buffer: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    /// The trail, and the front over it.
    trail: vk::Pipeline,
    front: vk::Pipeline,
    /// Wakes written this frame.
    count: u32,
}

impl WakeShells {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        let dev = &gpu.device;
        let buffer = gpu.host_buffer(
            (MAX_SHELLS as usize * size_of::<GpuWakeShell>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
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
        let set_layouts = [layouts.pass_set];
        // SAFETY: the pool was made just above for exactly this one set of two storage
        // buffers, and `set_layouts` lives to the end of the call.
        let set = unsafe {
            dev.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&set_layouts),
            )
        }?[0];
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
        let module = gpu.shader(spirv!("wake_shell"))?;
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
        let trail = pipeline(c"vs_wake_trail", c"fs_wake_trail")?;
        let front = pipeline(c"vs_wake_front", c"fs_wake_front")?;
        Ok(Self {
            buffer,
            pool,
            set,
            module,
            trail,
            front,
            count: 0,
        })
    }

    /// This frame's shells. Past `MAX_SHELLS` the rest are not drawn: `wake_fx` keeps no
    /// more wakes than that.
    pub(super) fn upload(&mut self, shells: &[GpuWakeShell]) {
        let shells = &shells[..shells.len().min(MAX_SHELLS as usize)];
        self.buffer.write(0, bytemuck::cast_slice(shells));
        self.count = shells.len() as u32;
    }

    /// Inside `scene_over` with the scene's set 0 bound: lays the wakes over the scene,
    /// their trails and then their fronts.
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
        // holds the live shell buffer.
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.trail);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set],
                &[],
            );
            device.cmd_draw(cmd, ALONG * AROUND * 6, self.count, 0, 0);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.front);
            device.cmd_draw(cmd, ARCH * TUBE * 6, self.count, 0, 0);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
        // SAFETY: these were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipelines before their module.
        unsafe {
            gpu.device.destroy_pipeline(self.trail, None);
            gpu.device.destroy_pipeline(self.front, None);
            gpu.device.destroy_shader_module(self.module, None);
            gpu.device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
