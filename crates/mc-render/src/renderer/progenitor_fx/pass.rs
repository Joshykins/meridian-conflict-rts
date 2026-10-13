//! The Progenitor's hard light drawn (progenitor_fx.wgsl): the buffer of this frame's
//! pieces, sorted by kind, its descriptor set and a pipeline for each kind, laid over
//! the scene after the water. What the pieces are comes from `progenitor_fx`.

use std::mem::size_of;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::progenitor_fx::{
    BEAM_STEPS, BODY_SIDES, BODY_STATIONS, DECK_PANES, DECK_STEPS, KINDS, MAX_PIECES, PRISM_BANDS,
    PRISM_SIDES, RING_BANDS, RING_SEGMENTS, SHELL_BANDS, SHELL_SEGMENTS,
};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, PASS_SET};
use crate::shader_reload::spirv;

/// One piece of hard light (`ProgenitorPiece` in progenitor_fx.wgsl, which says what
/// each field is to each kind). Times are the renderer's clock.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuPiece {
    pub(crate) a: [f32; 3],
    pub(crate) start: f32,
    pub(crate) b: [f32; 3],
    pub(crate) life: f32,
    pub(crate) c: [f32; 3],
    pub(crate) power: f32,
    pub(crate) size_from: f32,
    pub(crate) size_to: f32,
    pub(crate) kind: u32,
    pub(crate) flags: u32,
    pub(crate) seed: f32,
    pub(crate) tier: f32,
    pub(crate) width: f32,
    pub(crate) grow: f32,
}

/// Vertices drawn per piece of each kind (`progenitor_fx` kinds, in order).
const VERTICES: [u32; KINDS as usize] = [
    BEAM_STEPS * 6,
    RING_SEGMENTS * RING_BANDS * 6,
    PRISM_SIDES * PRISM_BANDS * 6,
    BODY_SIDES * BODY_STATIONS * 6,
    SHELL_SEGMENTS * SHELL_BANDS * 6,
    DECK_STEPS * DECK_PANES * 6,
];

pub(super) struct ProgenitorPass {
    buffer: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    /// One a kind, in kind order.
    pipelines: [vk::Pipeline; KINDS as usize],
    /// Pieces of each kind written this frame, one run after another.
    runs: [(u32, u32); KINDS as usize],
}

impl ProgenitorPass {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Self, GpuError> {
        let dev = &gpu.device;
        let buffer = gpu.host_buffer(
            (MAX_PIECES as usize * size_of::<GpuPiece>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        let mut sets = SetPool::new(gpu, &[(PASS_SET, 1)])?;
        let set = sets.alloc(gpu, layouts.pass_set, PASS_SET)?;
        let pool = sets.into_raw();
        let infos = [vk::DescriptorBufferInfo {
            buffer: buffer.buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }];
        let writes = [0, 1].map(|binding| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&infos)
        });
        // SAFETY: `set` was just allocated and no command buffer uses it yet; `buffer` is a
        // live storage buffer of this device, and `writes`/`infos` live to the end of the call.
        unsafe { dev.update_descriptor_sets(&writes, &[]) };
        let module = gpu.shader(spirv!("progenitor_fx"))?;
        let pipeline = |vs, fs| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    layout: layouts.scene,
                    pass: passes.scene_over,
                    vertex: VertexKind::None,
                    // Light added onto the scene, tested against its depth (`scene_over`
                    // keeps it read-only).
                    blend: Blend::Premultiplied,
                    depth: Depth::Test,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        Ok(Self {
            pipelines: [
                pipeline(c"vs_beam", c"fs_beam")?,
                pipeline(c"vs_ring", c"fs_ring")?,
                pipeline(c"vs_prism", c"fs_prism")?,
                // Bodies are solid, their far faces left out (`fs_body`): nothing in
                // `scene_over` writes depth. A shard of light adds itself (alpha zero).
                pipeline(c"vs_body", c"fs_body")?,
                pipeline(c"vs_shell", c"fs_shell")?,
                pipeline(c"vs_deck", c"fs_deck")?,
            ],
            buffer,
            pool,
            set,
            module,
            runs: [(0, 0); KINDS as usize],
        })
    }

    /// This frame's pieces, sorted by kind. Past `MAX_PIECES` the rest are not drawn:
    /// `progenitor_fx` hands over no more than that.
    pub(super) fn upload(&mut self, pieces: &[GpuPiece]) {
        let pieces = &pieces[..pieces.len().min(MAX_PIECES as usize)];
        self.buffer.write(0, bytemuck::cast_slice(pieces));
        let mut first = 0;
        for kind in 0..KINDS {
            let count = pieces[first as usize..]
                .iter()
                .take_while(|p| p.kind == kind)
                .count() as u32;
            self.runs[kind as usize] = (first, count);
            first += count;
        }
    }

    /// Inside `scene_over` with the scene's set 0 bound: every kind's pieces, the solid
    /// bodies first so the light is laid over them, then the decks, whose glass dims
    /// what lies under it, so the rest of the light lands over them.
    pub(super) fn draw(
        &self,
        device: &ash::Device,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
    ) {
        if self.runs.iter().all(|r| r.1 == 0) {
            return;
        }
        let order = [3usize, 5, 4, 2, 1, 0];
        // SAFETY: the renderer calls this inside `scene_over` while `cmd` is recording;
        // every pipeline was made for that pass with the scene layout `layout`, and `set`
        // holds the live piece buffer; each run lies inside what `upload` wrote.
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set],
                &[],
            );
            for kind in order {
                let (first, count) = self.runs[kind];
                if count == 0 {
                    continue;
                }
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines[kind],
                );
                device.cmd_draw(cmd, VERTICES[kind], count, 0, first);
            }
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.buffer, Buffer::null()));
        // SAFETY: these were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, the pipelines before their module.
        unsafe {
            for p in self.pipelines {
                gpu.device.destroy_pipeline(p, None);
            }
            gpu.device.destroy_shader_module(self.module, None);
            gpu.device.destroy_descriptor_pool(self.pool, None);
        }
    }
}
