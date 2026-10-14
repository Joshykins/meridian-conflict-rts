//! The clouds' column map (cloud_shape.wgsl): what the air holds over each spot
//! (the weather, the day's base, how tall and firm the cloud grows, the wakes and
//! swirls bending its billows), baked once a frame so the march reads it back
//! instead of working it out at every step of every ray.

use super::*;
use crate::gpu_consts::cloud_columns::LAYERS;

/// Texels a side of each half of the map: the map with a margin round it, and the
/// air far out round that (cloud_shape.wgsl `columns_rect`).
const RES: u32 = 1024;

pub(super) struct Columns {
    /// `LAYERS` near, then `LAYERS` far; written by `cs_columns`, read in GENERAL.
    image: Image,
    pipeline: vk::Pipeline,
}

impl Columns {
    /// `module` is clouds.wgsl's, `layout` the clouds' drawing layout (`cs_columns`
    /// binds the scene set and the drawing set).
    pub(super) fn new(
        gpu: &Gpu,
        module: vk::ShaderModule,
        layout: vk::PipelineLayout,
    ) -> Result<Columns, GpuError> {
        let image = gpu.image(&ImageDesc {
            width: RES,
            height: RES,
            format: vk::Format::R16G16B16A16_SFLOAT,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
            layers: LAYERS * 2,
            mips: 1,
            array: true,
        })?;
        // SAFETY: `submit_once` hands a recording command buffer of this device; the image
        // was just made, one mip, every layer still in UNDEFINED.
        gpu.submit_once(|cmd| unsafe {
            gpu.transition(
                cmd,
                image.image,
                vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: LAYERS * 2,
                },
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            );
        })?;
        let pipeline = pipelines::compute_pipeline(gpu, module, c"cs_columns", layout)?;
        Ok(Columns { image, pipeline })
    }

    /// For the drawing set's bindings 7 (written) and 8 (read), both in GENERAL.
    pub(super) fn view(&self) -> vk::ImageView {
        self.image.view
    }

    /// Bakes the map: after the weather step, before anything draws a cloud.
    pub(super) fn record(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        sets: [vk::DescriptorSet; 2],
    ) {
        let dev = &gpu.device;
        let readers =
            vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COMPUTE_SHADER;
        // SAFETY: the renderer calls this with its recording `cmd`, outside any render pass;
        // `pipeline` was made with `layout`, whose sets 0 and 1 are `sets`' layouts, and the
        // image is in GENERAL for its storage writes and the samples after.
        unsafe {
            // Last frame's march and shade read it; this dispatch rewrites it.
            let before = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                readers,
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
            let groups = RES.div_ceil(8);
            dev.cmd_dispatch(cmd, groups, groups, 2);
            let after = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                readers,
                vk::DependencyFlags::empty(),
                &after,
                &[],
                &[],
            );
        }
    }

    pub(super) fn destroy(&self, gpu: &Gpu) {
        // SAFETY: made by `new` on this device; called once, from `Sky::destroy` after the
        // device has gone idle.
        unsafe { gpu.device.destroy_pipeline(self.pipeline, None) };
        gpu.destroy_image_ref(&self.image);
    }
}
