//! Surge shells' wakes drawn in light (surge_fx.wgsl): the buffer of live surges, its
//! descriptor set and the two pipelines that lay them over the scene after the water, the
//! burnt disc each leaves and the ripple at its front. What each surge is, frame by frame,
//! comes from `surge_fx`. The Edict's records (`surge_fx::EDICT_WAVE` and the rest) ride in
//! the same buffer after the Saker's and are drawn on finer meshes of their own.

use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::surge_fx::{
    EDICT_RINGS, EDICT_SECTION, EDICT_SEGMENTS, MAX_EDICT, MAX_SURGES, RINGS, SECTION, SEGMENTS,
};
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
    /// What it is (`gpu_consts::surge_fx::SAKER`, `EDICT_WAVE`, ...).
    pub(crate) look: u32,
    pub(crate) _pad1: f32,
    pub(crate) _pad2: f32,
}

pub(super) struct SurgePass {
    buffer: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    /// The burnt disc, and the front over it; the Edict's.
    ground: vk::Pipeline,
    front: vk::Pipeline,
    edict_ground: vk::Pipeline,
    edict_front: vk::Pipeline,
    /// Records written this frame: the Saker's, the Edict's waves (which have a burnt
    /// disc), then its other records (fronts only).
    counts: Counts,
}

/// How many records of each kind lie in the buffer, in that order.
#[derive(Clone, Copy, Default)]
pub(super) struct Counts {
    pub(super) saker: u32,
    pub(super) waves: u32,
    pub(super) fronts: u32,
}

impl SurgePass {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        let dev = &gpu.device;
        let buffer = gpu.host_buffer(
            ((MAX_SURGES + MAX_EDICT) as usize * size_of::<GpuSurge>()) as u64,
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
            edict_ground: pipeline(c"vs_edict_ground", c"fs_edict_ground")?,
            edict_front: pipeline(c"vs_edict_front", c"fs_edict_front")?,
            buffer,
            pool,
            set,
            module,
            counts: Counts::default(),
        })
    }

    /// This frame's records, in the order `counts` gives. `surge_fx` keeps no more than
    /// `MAX_SURGES` of the Saker's and `MAX_EDICT` of the Edict's.
    pub(super) fn upload(&mut self, surges: &[GpuSurge], counts: Counts) {
        let total = (counts.saker + counts.waves + counts.fronts) as usize;
        debug_assert_eq!(total, surges.len());
        let surges = &surges[..total.min(surges.len())];
        self.buffer.write(0, bytemuck::cast_slice(surges));
        self.counts = counts;
    }

    /// Inside `scene_over` with the scene's set 0 bound: lays the surges over the scene,
    /// the burnt ground and then the fronts.
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
    ) {
        let Counts {
            saker,
            waves,
            fronts,
        } = self.counts;
        if saker + waves + fronts == 0 {
            return;
        }
        let edict_disc = EDICT_RINGS * EDICT_SEGMENTS * 6;
        let edict_front = EDICT_SECTION * EDICT_SEGMENTS * 6;
        let bind = |pipeline| {
            // SAFETY: the renderer calls `draw` inside `scene_over` while `cmd` is
            // recording; every pipeline here was made for that pass with the scene layout
            // `layout`, and `set` holds the live surge buffer.
            unsafe {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    layout,
                    1,
                    &[self.set],
                    &[],
                );
            }
        };
        // Each draw `(pipeline, vertices, instances, first)`: the burnt discs, then the
        // fronts over them.
        let draws = [
            (self.ground, RINGS * SEGMENTS * 6, saker, 0),
            (self.edict_ground, edict_disc, waves, saker),
            (self.front, SECTION * SEGMENTS * 6, saker, 0),
            (self.edict_front, edict_front, waves + fronts, saker),
        ];
        for (pipeline, vertices, instances, first) in draws {
            if instances == 0 {
                continue;
            }
            bind(pipeline);
            // SAFETY: as `bind`: the pipeline and its set are bound on `cmd` in `scene_over`.
            unsafe { device.cmd_draw(cmd, vertices, instances, 0, first) };
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
        // SAFETY: these were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipelines before their module.
        unsafe {
            gpu.device.destroy_pipeline(self.ground, None);
            gpu.device.destroy_pipeline(self.front, None);
            gpu.device.destroy_pipeline(self.edict_ground, None);
            gpu.device.destroy_pipeline(self.edict_front, None);
            gpu.device.destroy_shader_module(self.module, None);
            gpu.device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
