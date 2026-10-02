//! Fog of war as drawn (fog_field.wgsl). Each frame three compute passes build a
//! field `fog::FIELD_SCALE` times finer than the sim's `fog::CELL_M` grid, which
//! the scene set reads at binding 8 (`fog_at`): what is visible is drawn from the
//! sim's vision discs (`RenderFrame::vision`), each round and where its unit is
//! drawn this frame, and eased a little so discs that come and go fade; explored
//! ground is all the field has shown, started from the sim's grid
//! (`RenderFrame::fog`) whenever the eyes or the time it shows change under it.
//!
//! Set 0 of the passes: 0 the grid (sampled), 1 the field (read and written, in
//! GENERAL), 2 the discs, 3 the cover the discs are laid into.

use super::gtao::storage_image;
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::gpu_consts::fog;
use crate::pipelines;
use ash::vk;
use mc_sim::mirror::VisionDisc;
use mc_sim::RenderFrame;

/// Seconds for a disc that appears or goes (a unit built, a unit lost) to fade
/// most of the way (1/e). Moving discs need none: they are drawn where they are.
const VISIBLE_EASE_S: f32 = 0.08;
/// A frame longer than this eases no further (a stall).
const LONGEST_STEP_S: f32 = 0.25;
/// Disc workgroups per dispatch row (fog_field.wgsl `FOG_DISC_ROW`).
const DISC_ROW: u32 = 65535;

/// fog_field.wgsl's push constants.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FogPush {
    pub(crate) ease: f32,
    pub(crate) alpha: f32,
    pub(crate) restart: f32,
    _pad0: f32,
    pub(crate) disc_count: u32,
    pub(crate) field_width: u32,
    _pad1: u32,
    _pad2: u32,
}

pub(super) struct FogField {
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    clear: vk::Pipeline,
    lay: vk::Pipeline,
    blend: vk::Pipeline,
    grid: Image,
    field: Image,
    /// The latest frame's discs; grown as a side's units need.
    discs: Buffer,
    disc_count: u32,
    cover: Buffer,
    /// When the field last eased, for the next frame's step.
    last_time: Option<f32>,
    /// The eyes and tick of the latest frame, to tell when explored must start over.
    eyes: u32,
    tick: u32,
    /// The next pass starts explored over from the grid and jumps visible to its cover.
    restart: bool,
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
        let cover = gpu.device_buffer(
            u64::from(field.width * field.height) * 4,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        let discs = disc_buffer(gpu, 256)?;

        let dev = &gpu.device;
        use vk::DescriptorType as T;
        let types = [
            T::SAMPLED_IMAGE,
            T::STORAGE_IMAGE,
            T::STORAGE_BUFFER,
            T::STORAGE_BUFFER,
        ];
        let bindings: Vec<_> = types
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
            size: size_of::<FogPush>() as u32,
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
        let sizes = [
            vk::DescriptorPoolSize {
                ty: T::SAMPLED_IMAGE,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: T::STORAGE_IMAGE,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: T::STORAGE_BUFFER,
                descriptor_count: 2,
            },
        ];
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
        write_buffer(gpu, set, 3, &cover);

        let module = gpu.shader(crate::shader_reload::spirv!("fog_field"))?;
        let clear = pipelines::compute_pipeline(gpu, module, c"cs_fog_clear", layout)?;
        let lay = pipelines::compute_pipeline(gpu, module, c"cs_fog_discs", layout)?;
        let blend = pipelines::compute_pipeline(gpu, module, c"cs_fog_field", layout)?;
        let field_out = FogField {
            set_layout,
            layout,
            pool,
            set,
            module,
            clear,
            lay,
            blend,
            grid,
            field,
            discs,
            disc_count: 0,
            cover,
            last_time: None,
            eyes: 0,
            tick: 0,
            restart: true,
        };
        write_buffer(gpu, set, 2, &field_out.discs);
        Ok(field_out)
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

    /// Takes a new sim frame's discs. Called after the frame fence, with nothing in
    /// flight that reads the disc buffer.
    pub(super) fn set_frame(&mut self, gpu: &Gpu, frame: &RenderFrame) -> Result<(), GpuError> {
        // Other eyes, or a seek back: what was explored is the grid's, not the field's.
        if frame.fog_mask != self.eyes || frame.tick < self.tick {
            self.restart = true;
        }
        self.eyes = frame.fog_mask;
        self.tick = frame.tick;
        let bytes: &[u8] = bytemuck::cast_slice(&frame.vision);
        if bytes.len() as u64 > self.discs.size {
            let grown = disc_buffer(gpu, frame.vision.len().next_power_of_two())?;
            gpu.destroy_buffer(std::mem::replace(&mut self.discs, grown));
            write_buffer(gpu, self.set, 2, &self.discs);
        }
        self.discs.write(0, bytes);
        self.disc_count = frame.vision.len() as u32;
        Ok(())
    }

    /// Outside any render pass, after this frame's grid upload and before anything
    /// reads `fog_at`. `alpha` is how far through the tick units are drawn. With fog
    /// off nothing reads the field; it is skipped, and starts over when fog is back.
    pub(super) fn record(
        &mut self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        enabled: bool,
        time: f32,
        alpha: f32,
    ) {
        let step = self
            .last_time
            .map_or(0.0, |t| (time - t).clamp(0.0, LONGEST_STEP_S));
        self.last_time = Some(time);
        if !enabled {
            self.restart = true;
            return;
        }
        let push = FogPush {
            ease: if self.restart {
                1.0
            } else {
                1.0 - (-step / VISIBLE_EASE_S).exp()
            },
            alpha: alpha.clamp(0.0, 1.0),
            restart: self.restart as u32 as f32,
            _pad0: 0.0,
            disc_count: self.disc_count,
            field_width: self.field.width,
            _pad1: 0,
            _pad2: 0,
        };
        self.restart = false;
        let dev = &gpu.device;
        let texels = (self.field.width.div_ceil(8), self.field.height.div_ceil(8));
        let compute_to_compute = |access: vk::AccessFlags| {
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(access)];
            // SAFETY: `cmd` is recording, outside any render pass (as `record` requires).
            unsafe {
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &barrier,
                    &[],
                    &[],
                )
            };
        };
        // SAFETY: the renderer calls `record` with its recording `cmd`, outside any render pass;
        // the set and pipelines were made with `layout`, and the push is its range.
        unsafe {
            // The grid's upload and the discs' host writes, and last frame's reads of
            // the field, before these passes.
            let ready = [vk::MemoryBarrier::default()
                .src_access_mask(
                    vk::AccessFlags::TRANSFER_WRITE
                        | vk::AccessFlags::HOST_WRITE
                        | vk::AccessFlags::SHADER_READ,
                )
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER
                    | vk::PipelineStageFlags::HOST
                    | vk::PipelineStageFlags::ALL_GRAPHICS,
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
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.clear);
            dev.cmd_dispatch(cmd, texels.0, texels.1, 1);
            compute_to_compute(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE);
            if self.disc_count > 0 {
                dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.lay);
                dev.cmd_dispatch(
                    cmd,
                    self.disc_count.min(DISC_ROW),
                    self.disc_count.div_ceil(DISC_ROW),
                    1,
                );
                compute_to_compute(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE);
            }
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.blend);
            dev.cmd_dispatch(cmd, texels.0, texels.1, 1);
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
        // renderer's `Drop` after the device has gone idle, the pipelines before their layout
        // and module.
        unsafe {
            let dev = &gpu.device;
            for pipeline in [self.clear, self.lay, self.blend] {
                dev.destroy_pipeline(pipeline, None);
            }
            dev.destroy_shader_module(self.module, None);
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
        gpu.destroy_image_ref(&self.grid);
        gpu.destroy_image_ref(&self.field);
        gpu.destroy_buffer(std::mem::replace(&mut self.discs, Buffer::null()));
        gpu.destroy_buffer(std::mem::replace(&mut self.cover, Buffer::null()));
    }
}

/// Room for `count` discs.
fn disc_buffer(gpu: &Gpu, count: usize) -> Result<Buffer, GpuError> {
    gpu.host_buffer(
        (count.max(1) * size_of::<VisionDisc>()) as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER,
    )
}

/// Points `binding` of `set` at `buffer`. Only while no command buffer in flight uses
/// the set: from `new`, or after the frame fence.
fn write_buffer(gpu: &Gpu, set: vk::DescriptorSet, binding: u32, buffer: &Buffer) {
    let info = [buffer.info()];
    let write = [vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        .buffer_info(&info)];
    // SAFETY: callers keep to the rule above, so nothing in flight reads the set;
    // `buffer` is live and `write`/`info` live to the end of the call.
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}
