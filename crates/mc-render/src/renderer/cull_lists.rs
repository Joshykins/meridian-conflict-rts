//! The GPU cull's buffers and the draw lists it builds (cull.wgsl): one set of
//! per-slot indirect commands for each pass that draws models (`cull_list`), so
//! the depth pre-pass and each shadow cascade draw only what they have a use for.

use ash::vk;

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::cull_list;
use crate::pipelines::Pipelines;

/// Bytes of one `DrawCommand` (cull.wgsl), a `VkDrawIndexedIndirectCommand`.
const COMMAND_BYTES: u64 = 20;

pub(super) struct CullLists {
    pub(super) draws: super::active_draws::ActiveDraws,
    /// Per entity: its draw slot and the lists it is in.
    vis: Buffer,
    /// Per list and slot: how many instances, then where the next one goes.
    counters: Buffer,
    /// Per list and slot: the indirect draw.
    pub(super) commands: Buffer,
    /// Every list's instances, list after list (set 0 binding 4 of the scene).
    pub(super) visible: Buffer,
    /// Per entity: the blasts' push on a standing tree (cull.wgsl `tree_blast`; set 0
    /// binding 31 of the scene).
    pub(super) sway: Buffer,
    slot_count: u32,
}

impl CullLists {
    /// `entities` is every static and dynamic entity there can be, and `units` the
    /// dynamic ones, which the colour list holds a second time for their icons.
    pub(super) fn new(
        gpu: &Gpu,
        slot_count: u32,
        entities: u64,
        units: u64,
        draws: super::active_draws::ActiveDraws,
    ) -> Result<Self, GpuError> {
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let lists = cull_list::COUNT as u64;
        Ok(CullLists {
            draws,
            vis: gpu.device_buffer(entities * 4, storage)?,
            counters: gpu.device_buffer(slot_count as u64 * lists * 4, storage)?,
            commands: gpu.device_buffer(
                slot_count as u64 * lists * COMMAND_BYTES,
                storage | vk::BufferUsageFlags::INDIRECT_BUFFER,
            )?,
            visible: gpu.device_buffer((entities + units + (lists - 1) * entities) * 4, storage)?,
            sway: gpu.device_buffer(entities * 16, storage)?,
            slot_count,
        })
    }

    /// The cull set's bindings 5 to 8, in order (11 is `sway`).
    pub(super) fn bindings(&self) -> [&Buffer; 4] {
        [&self.vis, &self.counters, &self.commands, &self.visible]
    }

    /// Where in `commands` the draws of `list` (a `cull_list` number) start.
    pub(super) fn first_command(&self, list: u32) -> u64 {
        (list * self.slot_count) as u64 * COMMAND_BYTES
    }

    /// Records the cull: clear, classify, prefix sums, scatter, then a barrier so the
    /// vertex stage and indirect draws see the lists. `set` is the cull set.
    ///
    /// # Safety
    /// `cmd` is recording and outside a render pass, and `set` is a live set made for
    /// `layout`, which is the cull layout the cull pipelines were made with.
    pub(super) unsafe fn record(
        &self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        pipelines: &Pipelines,
        layout: vk::PipelineLayout,
        set: vk::DescriptorSet,
        static_count: u32,
        dynamic_count: u32,
    ) {
        let rw = vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;
        let barrier = |dst: vk::AccessFlags, dst_stage: vk::PipelineStageFlags| {
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(dst)];
            // SAFETY: `cmd` is recording outside a render pass (the caller's contract), and
            // the barrier array lives to the end of the call.
            unsafe {
                device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    dst_stage,
                    vk::DependencyFlags::empty(),
                    &barrier,
                    &[],
                    &[],
                )
            };
        };
        let dispatch = |pipeline: vk::Pipeline, count: u32, dynamic: u32| {
            if count == 0 {
                return;
            }
            // SAFETY: as above; the pipeline was made for `layout`, and the 8-byte push fits
            // its 16-byte range.
            unsafe {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
                device.cmd_push_constants(
                    cmd,
                    layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    bytemuck::bytes_of(&[count, dynamic]),
                );
                device.cmd_dispatch(cmd, count.div_ceil(64), 1, 1);
            }
        };
        // SAFETY: as above: `set` is made for set 0 of `layout`.
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                layout,
                0,
                &[set],
                &[],
            )
        };
        dispatch(pipelines.cull_clear, self.slot_count * cull_list::COUNT, 0);
        barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
        dispatch(pipelines.cull_cull, static_count, 0);
        dispatch(pipelines.cull_cull, dynamic_count, 1);
        barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
        // One workgroup: a thread per list.
        // SAFETY: as above.
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipelines.cull_prefix);
            device.cmd_dispatch(cmd, 1, 1, 1);
        }
        barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
        dispatch(pipelines.cull_scatter, static_count, 0);
        dispatch(pipelines.cull_scatter, dynamic_count, 1);
        barrier(
            vk::AccessFlags::SHADER_READ | vk::AccessFlags::INDIRECT_COMMAND_READ,
            vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::DRAW_INDIRECT,
        );
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        for b in [
            &mut self.vis,
            &mut self.counters,
            &mut self.commands,
            &mut self.visible,
            &mut self.sway,
        ] {
            gpu.destroy_buffer(std::mem::replace(b, Buffer::null()));
        }
    }
}

impl super::Renderer {
    /// A draw slot's model, for a breadcrumb: the blueprint's key, or a prop kind.
    fn slot_label(&self, slot: u32) -> String {
        match self.cull.draws.owner(slot) {
            Some(b) => match self.blueprints.units.get(b) {
                Some(bp) => format!("slot {slot}: {}", bp.key),
                None => format!("slot {slot}: prop kind {}", b - self.blueprints.units.len()),
            },
            None => format!("slot {slot}: strategic icons"),
        }
    }

    /// The models of cull list `list`, inside a render pass with set 0 bound:
    /// `pipelines` are units and the rest, then trees and rocks (entity.wgsl `vs_prop`).
    pub(super) fn draw_entities(
        &self,
        cmd: vk::CommandBuffer,
        pipelines: [vk::Pipeline; 2],
        pass_kind: u32,
        list: u32,
    ) {
        let device = &self.gpu.device;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let first = self.cull.first_command(list);
        let draws = &self.cull.draws;
        // SAFETY: called only inside a render pass of `render` while `cmd` is recording, after
        // set 0 is bound; `shields_set` is a pass set, the 8 pushed bytes fit the layout's
        // 16-byte push range; `commands` has INDIRECT_BUFFER usage and holds `slot_count`
        // 20-byte commands for each of the `cull_list::COUNT` lists, `list` is below it and
        // each occupied range is bounded by the model slots, so all command reads lie inside it.
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                1,
                &[self.shields_set],
                &[],
            );
            let pushed = [pass_kind, self.shield_count];
            device.cmd_push_constants(cmd, self.layouts.scene, gfx, 0, bytemuck::bytes_of(&pushed));
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.mesh_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
            for (pipeline, ranges) in pipelines
                .into_iter()
                .zip([&draws.ranges, &draws.prop_ranges])
            {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                for range in ranges {
                    if self.timers.fine() {
                        // One slot at a time, each named, to find a draw that hangs the GPU.
                        for slot in range.clone() {
                            self.timers.crumb(cmd, || self.slot_label(slot));
                            device.cmd_draw_indexed_indirect(
                                cmd,
                                self.cull.commands.buffer,
                                first + slot as u64 * COMMAND_BYTES,
                                1,
                                COMMAND_BYTES as u32,
                            );
                        }
                        continue;
                    }
                    device.cmd_draw_indexed_indirect(
                        cmd,
                        self.cull.commands.buffer,
                        first + range.start as u64 * COMMAND_BYTES,
                        range.end - range.start,
                        COMMAND_BYTES as u32,
                    );
                }
            }
        }
    }
}
