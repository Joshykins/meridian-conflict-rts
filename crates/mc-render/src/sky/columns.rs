//! The clouds' column map (cloud_shape.wgsl): what the air holds over each spot
//! (the weather, the day's base, how tall and firm the cloud grows, the wakes and
//! swirls bending its billows), baked once a frame so the march reads it back
//! instead of working it out at every step of every ray.

use super::*;
use crate::gpu_consts::cloud_columns::LAYERS;

/// Texels a side of the map over the map and a margin round it, and of the one over
/// the air far out round that, which only the horizon sees (cloud_shape.wgsl
/// `columns_rect`).
const NEAR_RES: u32 = 1024;
const FAR_RES: u32 = 512;
/// A frame bakes one row in this many: the air drifts a metre or so in a few frames.
const STAGGER: u32 = 4;

pub(super) struct Columns {
    /// `LAYERS` each; written by `cs_columns`, read in GENERAL.
    near: Image,
    far: Image,
    pipeline: vk::Pipeline,
    /// The whole map wants baking on the next frame (made, or the sky started over).
    stale: std::cell::Cell<bool>,
}

/// A `res`-square map of `LAYERS` layers, in GENERAL.
fn map_image(gpu: &Gpu, res: u32) -> Result<Image, GpuError> {
    let image = gpu.image(&ImageDesc {
        width: res,
        height: res,
        format: vk::Format::R16G16B16A16_SFLOAT,
        usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED,
        layers: LAYERS,
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
                layer_count: LAYERS,
            },
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
    })?;
    Ok(image)
}

impl Columns {
    /// `module` is clouds.wgsl's, `layout` the clouds' drawing layout (`cs_columns`
    /// binds the scene set and the drawing set).
    pub(super) fn new(
        gpu: &Gpu,
        module: vk::ShaderModule,
        layout: vk::PipelineLayout,
    ) -> Result<Columns, GpuError> {
        let near = map_image(gpu, NEAR_RES)?;
        let far = map_image(gpu, FAR_RES)?;
        let pipeline = pipelines::compute_pipeline(gpu, module, c"cs_columns", layout)?;
        Ok(Columns {
            near,
            far,
            pipeline,
            stale: std::cell::Cell::new(true),
        })
    }

    /// The near map and the far one, for the drawing set's bindings 7 and 8 (written
    /// and read) and 11 and 12, all in GENERAL.
    pub(super) fn views(&self) -> [vk::ImageView; 2] {
        [self.near.view, self.far.view]
    }

    /// Bake the whole map on the next frame.
    pub(super) fn invalidate(&self) {
        self.stale.set(true);
    }

    /// Bakes the map, or a share of its rows: after the weather step, before anything
    /// draws a cloud.
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
            // `cs_columns` takes its stride from the dispatch's height.
            let rows = if self.stale.replace(false) {
                NEAR_RES
            } else {
                NEAR_RES / STAGGER
            };
            dev.cmd_dispatch(cmd, NEAR_RES.div_ceil(8), rows.div_ceil(8), 2);
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
        gpu.destroy_image_ref(&self.near);
        gpu.destroy_image_ref(&self.far);
    }
}
