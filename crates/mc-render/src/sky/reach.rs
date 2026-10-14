//! The nearest and farthest any march texel round each one went (clouds.wgsl
//! `cs_reach`): the composite takes a pixel's clouds through a cubic only from texels
//! that marched as far as the pixel is, and with this one load tells, for nearly
//! every pixel, that all sixteen of its texels did.

use super::*;

pub(super) struct ReachRange {
    /// x the nearest, y the farthest, at the march's size; `None` before `resize`.
    image: Option<Image>,
    pipeline: vk::Pipeline,
}

impl ReachRange {
    /// `module` is clouds.wgsl's, `layout` the clouds' drawing layout.
    pub(super) fn new(
        gpu: &Gpu,
        module: vk::ShaderModule,
        layout: vk::PipelineLayout,
    ) -> Result<ReachRange, GpuError> {
        let pipeline = pipelines::compute_pipeline(gpu, module, c"cs_reach", layout)?;
        Ok(ReachRange {
            image: None,
            pipeline,
        })
    }

    /// A new image at the march's size (the old one dropped), in GENERAL; its view for
    /// the drawing set's bindings 13 (written) and 14 (read).
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        (width, height): (u32, u32),
    ) -> Result<vk::ImageView, GpuError> {
        if let Some(old) = self.image.take() {
            gpu.destroy_image(old);
        }
        let image = gpu.image(&ImageDesc {
            width,
            height,
            format: vk::Format::R32G32_SFLOAT,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        // SAFETY: `submit_once` hands a recording command buffer of this device; the image
        // was just made, one mip and layer, still in UNDEFINED.
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
        let view = image.view;
        self.image = Some(image);
        Ok(view)
    }

    /// After the march is laid out at full size (`fs_fill`), before the composite.
    pub(super) fn record(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        sets: [vk::DescriptorSet; 2],
    ) {
        let Some(image) = &self.image else {
            return;
        };
        let dev = &gpu.device;
        // SAFETY: the renderer calls this with its recording `cmd`, outside any render pass;
        // `pipeline` was made with `layout`, whose sets 0 and 1 are `sets`' layouts; the
        // fill pass that wrote the march has ended, and the image is in GENERAL.
        unsafe {
            let before = [vk::MemoryBarrier::default()
                .src_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_READ,
                )
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &before,
                &[],
                &[],
            );
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                layout,
                0,
                &sets,
                &[],
            );
            dev.cmd_dispatch(cmd, image.width.div_ceil(8), image.height.div_ceil(8), 1);
            let after = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &after,
                &[],
                &[],
            );
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: made by `new` on this device; called once, from `Sky::destroy` after the
        // device has gone idle.
        unsafe { gpu.device.destroy_pipeline(self.pipeline, None) };
        if let Some(image) = self.image.take() {
            gpu.destroy_image(image);
        }
    }
}
