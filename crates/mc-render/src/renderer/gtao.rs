//! Screen-space ambient occlusion (gtao.wgsl): two compute passes at half the
//! scene's size between the depth pre-pass and the scene pass, into an image
//! the scene's shaders read at binding 30 of set 0 (`screen_ao`).
//!
//! Set 0 of both passes: 0 the globals, 1 the scene's depth, 2 and 3 the raw
//! occlusion (written, then read), 4 the blurred result. Both images stay in
//! the GENERAL layout.

use crate::descriptors::{Binding, SetPool};
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::pipelines;
use ash::vk;

/// Metres the search reaches out from each point: under a tank, between
/// buildings, at the foot of a tree.
const RADIUS_M: f32 = 4.5;
/// Exponent on the visibility: over 1 deepens the occlusion.
const STRENGTH: f32 = 2.0;
/// The widest search on screen, in full-size pixels, so a unit filling the view stays cheap.
const MAX_SCREEN_RADIUS: f32 = 110.0;

pub(super) struct Gtao {
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    main: vk::Pipeline,
    blur: vk::Pipeline,
    raw: Image,
    ao: Image,
    pub(super) enabled: bool,
}

/// A one-mip image compute writes and shaders sample, left in GENERAL.
pub(super) fn storage_image(
    gpu: &Gpu,
    width: u32,
    height: u32,
    format: vk::Format,
) -> Result<Image, GpuError> {
    let image = gpu.image(&ImageDesc {
        width,
        height,
        format,
        usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        layers: 1,
        mips: 1,
        array: false,
    })?;
    // SAFETY: `submit_once` hands a recording command buffer of this device, and `image` was
    // just made, one mip and layer, still in UNDEFINED.
    gpu.submit_once(|cmd| unsafe {
        gpu.transition(
            cmd,
            image.image,
            vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
    })?;
    Ok(image)
}

/// The set: the frame's uniforms, the depth, the raw AO (written, then read by the blur)
/// and the blurred AO.
const BINDINGS: &[Binding] = &[
    (0, vk::DescriptorType::UNIFORM_BUFFER),
    (1, vk::DescriptorType::SAMPLED_IMAGE),
    (2, vk::DescriptorType::STORAGE_IMAGE),
    (3, vk::DescriptorType::SAMPLED_IMAGE),
    (4, vk::DescriptorType::STORAGE_IMAGE),
];

impl Gtao {
    pub(super) fn new(gpu: &Gpu, globals: &Buffer) -> Result<Gtao, GpuError> {
        let dev = &gpu.device;
        use vk::DescriptorType as T;
        let set_layout = pipelines::set_layout(gpu, BINDINGS, vk::ShaderStageFlags::COMPUTE)?;
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
        let mut sets = SetPool::new(gpu, &[(BINDINGS, 1)])?;
        let set = sets.alloc(gpu, set_layout, BINDINGS)?;
        let pool = sets.into_raw();
        let info = [globals.info()];
        let write = [vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(0)
            .descriptor_type(T::UNIFORM_BUFFER)
            .buffer_info(&info)];
        // SAFETY: `set` is fresh and unused by any command buffer; binding 0 is its uniform
        // buffer and `globals` is live; `write`/`info` live to the end of the call.
        unsafe { dev.update_descriptor_sets(&write, &[]) };

        let module = gpu.shader(crate::shader_reload::spirv!("gtao"))?;
        let main = pipelines::compute_pipeline(gpu, module, c"cs_gtao", layout)?;
        let blur = pipelines::compute_pipeline(gpu, module, c"cs_gtao_blur", layout)?;
        let raw = storage_image(gpu, 1, 1, vk::Format::R16G16B16A16_SFLOAT)?;
        let ao = storage_image(gpu, 1, 1, vk::Format::R8G8B8A8_UNORM)?;
        let gtao = Gtao {
            set_layout,
            layout,
            pool,
            set,
            module,
            main,
            blur,
            raw,
            ao,
            enabled: true,
        };
        gtao.write_images(gpu);
        Ok(gtao)
    }

    fn write_images(&self, gpu: &Gpu) {
        for (binding, view, ty) in [
            (2, self.raw.view, vk::DescriptorType::STORAGE_IMAGE),
            (3, self.raw.view, vk::DescriptorType::SAMPLED_IMAGE),
            (4, self.ao.view, vk::DescriptorType::STORAGE_IMAGE),
        ] {
            let info = [vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view: view,
                image_layout: vk::ImageLayout::GENERAL,
            }];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(binding)
                .descriptor_type(ty)
                .image_info(&info)];
            // SAFETY: called from `new` (fresh set) or `resize` (which runs only from the
            // renderer's `create_size_dependent`, after the device went idle), so no command
            // buffer in flight uses the set; the views are live and in GENERAL, as
            // `storage_image` left them.
            unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        }
    }

    /// Half of `scene` each way; `depth` is the scene's depth. The caller points
    /// the scene set's binding 30 at `ao_view` afterwards.
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        scene: (u32, u32),
        depth: vk::ImageView,
    ) -> Result<(), GpuError> {
        let (w, h) = (scene.0.div_ceil(2).max(1), scene.1.div_ceil(2).max(1));
        let raw = storage_image(gpu, w, h, vk::Format::R16G16B16A16_SFLOAT)?;
        let ao = storage_image(gpu, w, h, vk::Format::R8G8B8A8_UNORM)?;
        gpu.destroy_image(std::mem::replace(&mut self.raw, raw));
        gpu.destroy_image(std::mem::replace(&mut self.ao, ao));
        self.write_images(gpu);
        let info = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: depth,
            image_layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        }];
        let write = [vk::WriteDescriptorSet::default()
            .dst_set(self.set)
            .dst_binding(1)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(&info)];
        // SAFETY: `resize` runs only from the renderer's `create_size_dependent`, after the
        // device went idle, so no command buffer in flight uses the set; `depth` is the scene's
        // live depth view and `write`/`info` live to the end of the call.
        unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        Ok(())
    }

    pub(super) fn ao_view(&self) -> vk::ImageView {
        self.ao.view
    }

    /// Outside any render pass, after the depth pre-pass. Leaves the result
    /// ready for the scene's fragment shaders, and the depth free to be written again.
    pub(super) fn record(&self, gpu: &Gpu, cmd: vk::CommandBuffer) {
        let dev = &gpu.device;
        let groups = |image: &Image| (image.width.div_ceil(8), image.height.div_ceil(8));
        let push = [
            RADIUS_M,
            STRENGTH,
            MAX_SCREEN_RADIUS,
            self.enabled as u32 as f32,
        ];
        // SAFETY: the renderer calls `record` with its recording `cmd`, outside any render pass
        // (after the depth pre-pass); the set and pipelines were made with `layout`, and the
        // 16-byte push is its range.
        unsafe {
            // The pre-pass's depth writes, before the search reads them.
            let depth_ready = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &depth_ready,
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
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.main);
            let (x, y) = groups(&self.raw);
            dev.cmd_dispatch(cmd, x, y, 1);
            let raw_ready = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &raw_ready,
                &[],
                &[],
            );
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.blur);
            let (x, y) = groups(&self.ao);
            dev.cmd_dispatch(cmd, x, y, 1);
            // The result before the scene's fragments sample it, and the search's
            // reads of the depth before the scene pass writes it.
            let ao_ready = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::FRAGMENT_SHADER
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::DependencyFlags::empty(),
                &ao_ready,
                &[],
                &[],
            );
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their layout and
        // module.
        unsafe {
            let dev = &gpu.device;
            dev.destroy_pipeline(self.main, None);
            dev.destroy_pipeline(self.blur, None);
            dev.destroy_shader_module(self.module, None);
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
        gpu.destroy_image_ref(&self.raw);
        gpu.destroy_image_ref(&self.ao);
    }
}
