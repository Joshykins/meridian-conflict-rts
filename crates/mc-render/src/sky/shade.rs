//! The column map's bake, the light volume and the cloud shadows (a quarter of the
//! texels a frame, in a compact interleaved dispatch).
use super::*;

impl Sky {
    /// The column map (`columns.rs`): after `record_sim`, before the shade and the march.
    pub fn record_columns(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if !self.clouds {
            return;
        }
        self.columns.record(
            gpu,
            cmd,
            self.draw_pipeline_layout,
            [scene_set, self.draw_sets[0]],
        );
    }

    /// The light volume (`light.rs`): after the column map, before the shade and the march.
    pub fn record_light(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if !self.clouds {
            return;
        }
        self.light.record(
            gpu,
            cmd,
            self.draw_pipeline_layout,
            [scene_set, self.draw_sets[0]],
        );
    }

    /// The clouds' shade on the land: after `record_sim`, before anything is lit.
    pub fn record_shade(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if !self.clouds {
            return;
        }
        let dev = &gpu.device;
        // A quarter of the texels a frame (cs_shade), but the whole field on the first
        // frames and at each wrap of atmos.frame.x.
        let stride = if self.frame_index % 1024 > 1 { 2 } else { 1 };
        let groups = SHADE_RES.div_ceil(stride * 8);
        let shaders = vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
        // SAFETY: the renderer calls `record_shade` with its recording `cmd`, outside any
        // render pass; `shade_pipeline` was made with `draw_pipeline_layout`, whose set 0 is
        // `scene_set`'s layout; cs_shade takes its stride from the dispatch's size.
        unsafe {
            // Last frame's lighting read it; this dispatch reads and rewrites it.
            let before = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(
                cmd,
                shaders,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &before,
                &[],
                &[],
            );
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.shade_pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.draw_pipeline_layout,
                0,
                &[scene_set, self.draw_sets[0]],
                &[],
            );
            dev.cmd_dispatch(cmd, groups, groups, 1);
            let after = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                shaders,
                vk::DependencyFlags::empty(),
                &after,
                &[],
                &[],
            );
        }
    }
}
