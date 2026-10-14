//! The clouds' light volume (cloud_shape.wgsl `light_tau`): over the map, how much
//! cloud lies between each point of the layer and the sun. The march reads it once
//! a sample instead of looking the cloud up again at each of the light's taps,
//! which was three quarters of what it cost.

use super::*;

/// Texels across the map and its margin each way (cloud_shape.wgsl `columns_rect`).
const RES: u32 = 256;
/// Slices up the layer, closer together low down where fair-weather cloud is.
const SLICES: u32 = 48;
/// A frame refreshes one slice in this many: clouds drift and the sun moves slowly,
/// so the light a few frames old is the light now.
const STAGGER: u32 = 16;

pub(super) struct LightVolume {
    /// r the optical depth toward the sun; written by `cs_light`, read in GENERAL.
    image: Image,
    pipeline: vk::Pipeline,
    /// The whole volume wants filling on the next frame (made, or the sky started over).
    stale: std::cell::Cell<bool>,
}

impl LightVolume {
    /// `module` is clouds.wgsl's, `layout` the clouds' drawing layout.
    pub(super) fn new(
        gpu: &Gpu,
        module: vk::ShaderModule,
        layout: vk::PipelineLayout,
    ) -> Result<LightVolume, GpuError> {
        let image = volume::volume_image(gpu, [RES, RES, SLICES], vk::Format::R16G16B16A16_SFLOAT)?;
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
        let pipeline = pipelines::compute_pipeline(gpu, module, c"cs_light", layout)?;
        Ok(LightVolume {
            image,
            pipeline,
            stale: std::cell::Cell::new(true),
        })
    }

    /// For the drawing set's bindings 9 (written) and 10 (read), both in GENERAL.
    pub(super) fn view(&self) -> vk::ImageView {
        self.image.view
    }

    /// Fill the whole volume on the next frame.
    pub(super) fn invalidate(&self) {
        self.stale.set(true);
    }

    /// Refreshes a share of the volume: after the column map, before the march.
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
        // `cs_light` takes its slices from the dispatch's depth: all of them, or every
        // `STAGGER`th from the frame number's.
        let depth = if self.stale.replace(false) {
            SLICES
        } else {
            SLICES / STAGGER
        };
        // SAFETY: the renderer calls this with its recording `cmd`, outside any render pass;
        // `pipeline` was made with `layout`, whose sets 0 and 1 are `sets`' layouts, and the
        // image is in GENERAL for its storage writes and the samples after.
        unsafe {
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
            dev.cmd_dispatch(cmd, groups, groups, depth);
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
