//! Cloud shadows: compact interleaved dispatch on lower quality presets.
use super::*;

impl Sky {
    /// The clouds' shade on the land: after `record_sim`, before anything is lit.
    pub fn record_shade(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        scene_set: vk::DescriptorSet,
        simple: bool,
    ) {
        if !self.clouds {
            return;
        }
        let dev = &gpu.device;
        // Match atmos.frame.x in cs_shade: the first frame and wrap refresh the
        // whole field. Reuse the existing uniforms to keep draw layouts compatible.
        let stride = if simple && self.frame_index % 1024 > 1 {
            2
        } else {
            1
        };
        let groups = SHADE_RES.div_ceil(stride * 8);
        let shaders = vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER;
        // SAFETY: the renderer calls `record_shade` with its recording `cmd`, outside any
        // render pass; `shade_pipeline` was made with `draw_pipeline_layout`, whose set 0 is
        // `scene_set`'s layout. Dispatch stride matches cs_shade's existing uniforms.
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
