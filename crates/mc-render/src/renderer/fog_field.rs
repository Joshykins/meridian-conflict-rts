//! Fog of war as drawn (fog_field.wgsl). The sim hands over its coarse on/off
//! grid (`RenderFrame::fog`, one texel per `fog::CELL_M` cell, visible and
//! explored); a compute pass each frame turns it into a field `fog::FIELD_SCALE`
//! times finer, blurred round and eased over time, which the scene set reads at
//! binding 8 (`fog_at`).
//!
//! Set 0 of the pass: 0 the grid (sampled), 1 the field (read and written).
//! The field stays in GENERAL.

use super::gtao::storage_image;
use crate::gpu::{Gpu, GpuError, Image, ImageDesc};
use crate::gpu_consts::fog;
use crate::pipelines;
use ash::vk;

/// Seconds for the visible area to close most of the way (1/e) on a new edge:
/// a disc stepping a cell on a sim tick glides there over about a tick.
const VISIBLE_EASE_S: f32 = 0.12;
/// The same for ground first explored, fading up out of the dark.
const EXPLORED_EASE_S: f32 = 0.3;
/// A frame longer than this eases no further (a stall, a seek).
const LONGEST_STEP_S: f32 = 0.25;

pub(super) struct FogField {
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    pipeline: vk::Pipeline,
    grid: Image,
    field: Image,
    /// When the field last eased, for the next frame's step.
    last_time: Option<f32>,
    /// The next pass jumps straight to the grid: fog just turned on.
    snap: bool,
}

impl FogField {
    /// For a map `size` metres across.
    pub(super) fn new(gpu: &Gpu, size: [f32; 2]) -> Result<FogField, GpuError> {
        let cell = fog::CELL_M as u32;
        // As `mc_sim::Fog::new` sizes its grid.
        let (w, h) = (
            (size[0] as u32 / cell).max(1) + 1,
            (size[1] as u32 / cell).max(1) + 1,
        );
        let grid = gpu.image(&ImageDesc {
            width: w,
            height: h,
            format: vk::Format::R8G8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&grid, 0, 0, None, &vec![255u8; (w * h * 2) as usize], true)?;
        let field = storage_image(
            gpu,
            w * fog::FIELD_SCALE,
            h * fog::FIELD_SCALE,
            vk::Format::R16G16B16A16_SFLOAT,
        )?;

        let dev = &gpu.device;
        use vk::DescriptorType as T;
        let bindings: Vec<_> = [T::SAMPLED_IMAGE, T::STORAGE_IMAGE]
            .iter()
            .enumerate()
            .map(|(b, &ty)| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b as u32)
                    .descriptor_type(ty)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
            })
            .collect();
        // SAFETY: the device is alive and the create info borrows `bindings`, which lives to
        // the end of the call.
        let set_layout = unsafe {
            dev.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }?;
        let push = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            offset: 0,
            size: 16,
        }];
        let set_layouts = [set_layout];
        // SAFETY: the device is alive; `set_layout` is this device's and `set_layouts`/`push`
        // live to the end of the call.
        let layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&push),
                None,
            )
        }?;
        let sizes = [T::SAMPLED_IMAGE, T::STORAGE_IMAGE].map(|ty| vk::DescriptorPoolSize {
            ty,
            descriptor_count: 1,
        });
        // SAFETY: the device is alive and `sizes` lives to the end of the call.
        let pool = unsafe {
            dev.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        // SAFETY: the pool was made just above with room for exactly this one set, and
        // `set_layouts` lives to the end of the call.
        let set = unsafe {
            dev.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&set_layouts),
            )
        }?[0];
        for (binding, view, ty, image_layout) in [
            (
                0,
                grid.view,
                T::SAMPLED_IMAGE,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            (1, field.view, T::STORAGE_IMAGE, vk::ImageLayout::GENERAL),
        ] {
            let info = [vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view: view,
                image_layout,
            }];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(ty)
                .image_info(&info)];
            // SAFETY: `set` is fresh and unused by any command buffer; the views are live and
            // in the layouts named (the upload leaves the grid shader-readable,
            // `storage_image` the field in GENERAL); `write`/`info` live to the end of the call.
            unsafe { dev.update_descriptor_sets(&write, &[]) };
        }

        let module = gpu.shader(crate::shader_reload::spirv!("fog_field"))?;
        let pipeline = pipelines::compute_pipeline(gpu, module, c"cs_fog_field", layout)?;
        Ok(FogField {
            set_layout,
            layout,
            pool,
            set,
            module,
            pipeline,
            grid,
            field,
            last_time: None,
            snap: true,
        })
    }

    /// The sim's grid, which `record_uploads` refreshes from each new frame.
    pub(super) fn grid(&self) -> &Image {
        &self.grid
    }

    /// Bytes a frame's fog must hold to fit the grid: two per cell.
    pub(super) fn grid_len(&self) -> usize {
        (self.grid.width * self.grid.height * 2) as usize
    }

    /// The smoothed field, for binding 8 of the scene set, in GENERAL.
    pub(super) fn view(&self) -> vk::ImageView {
        self.field.view
    }

    /// Outside any render pass, after this frame's grid upload and before anything
    /// reads `fog_at`. With fog off nothing reads the field; it is skipped, and
    /// the next pass with fog on starts from the grid as it stands.
    pub(super) fn record(&mut self, gpu: &Gpu, cmd: vk::CommandBuffer, enabled: bool, time: f32) {
        let step = self
            .last_time
            .map_or(0.0, |t| (time - t).clamp(0.0, LONGEST_STEP_S));
        self.last_time = Some(time);
        if !enabled {
            self.snap = true;
            return;
        }
        let ease = |seconds: f32| {
            if self.snap {
                1.0
            } else {
                1.0 - (-step / seconds).exp()
            }
        };
        let push = [ease(VISIBLE_EASE_S), ease(EXPLORED_EASE_S), 0.0, 0.0];
        self.snap = false;
        let dev = &gpu.device;
        // SAFETY: the renderer calls `record` with its recording `cmd`, outside any render pass;
        // the set and pipeline were made with `layout`, and the 16-byte push is its range.
        unsafe {
            // The grid's upload, and last frame's reads of the field, before this pass.
            let ready = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE | vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::ALL_GRAPHICS,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &ready,
                &[],
                &[],
            );
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.layout,
                0,
                &[self.set],
                &[],
            );
            dev.cmd_push_constants(
                cmd,
                self.layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                bytemuck::bytes_of(&push),
            );
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            dev.cmd_dispatch(
                cmd,
                self.field.width.div_ceil(8),
                self.field.height.div_ceil(8),
                1,
            );
            // The field before every shader that reads `fog_at` (the cull and the
            // grass among them).
            let done = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::COMPUTE_SHADER
                    | vk::PipelineStageFlags::VERTEX_SHADER
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &done,
                &[],
                &[],
            );
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipeline before its layout and
        // module.
        unsafe {
            let dev = &gpu.device;
            dev.destroy_pipeline(self.pipeline, None);
            dev.destroy_shader_module(self.module, None);
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
        gpu.destroy_image_ref(&self.grid);
        gpu.destroy_image_ref(&self.field);
    }
}
