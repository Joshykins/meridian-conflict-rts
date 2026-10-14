//! A storage buffer the CPU fills and the GPU reads from its own memory.
//!
//! A mapped host buffer is system RAM on most cards: every pass that reads it pulls
//! it over the bus, and a vertex shader that loads its instance record per vertex
//! pulls the record once per vertex. With a hundred thousand units on screen that
//! was most of the frame. Here the CPU writes a mapped copy, the frame begins by
//! copying the stretch it wrote into a buffer in the card's memory, and the passes
//! read that one.

use crate::gpu::{Buffer, Gpu, GpuError};
use ash::vk;

pub(crate) struct StagedBuffer {
    /// What the CPU writes (mapped, TRANSFER_SRC).
    host: Buffer,
    /// What the passes read.
    pub(crate) device: Buffer,
    /// Bytes written since the last copy, as a range.
    dirty: Option<(u64, u64)>,
}

impl StagedBuffer {
    #[track_caller]
    pub(crate) fn new(
        gpu: &Gpu,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<StagedBuffer, GpuError> {
        Ok(StagedBuffer {
            host: gpu.host_buffer(size, vk::BufferUsageFlags::TRANSFER_SRC)?,
            device: gpu.device_buffer(size, usage)?,
            dirty: None,
        })
    }

    /// Writes `data` at `offset` of the host copy; the next `record` carries it over.
    pub(crate) fn write(&mut self, offset: u64, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.host.write(offset, data);
        let end = offset + data.len() as u64;
        self.dirty = Some(
            self.dirty
                .map_or((offset, end), |(a, b)| (a.min(offset), b.max(end))),
        );
    }

    /// Copies what was written since the last call into the card's buffer, ahead of
    /// every shader stage that reads it.
    pub(crate) fn record(&mut self, device: &ash::Device, cmd: vk::CommandBuffer) {
        let Some((start, end)) = self.dirty.take() else {
            return;
        };
        // SAFETY: `cmd` is recording outside a render pass (the frame's uploads). Both
        // buffers are this device's and `size` long; `write` kept the range within the host
        // copy, which is TRANSFER_SRC, and `device_buffer` made the other TRANSFER_DST. The
        // previous frame's reads of the device copy are over: the frame waited on its fence
        // before any write.
        unsafe {
            device.cmd_copy_buffer(
                cmd,
                self.host.buffer,
                self.device.buffer,
                &[vk::BufferCopy {
                    src_offset: start,
                    dst_offset: start,
                    size: end - start,
                }],
            );
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COMPUTE_SHADER
                    | vk::PipelineStageFlags::VERTEX_SHADER
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &barrier,
                &[],
                &[],
            );
        }
    }

    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        gpu.destroy_buffer(std::mem::replace(&mut self.host, Buffer::null()));
        gpu.destroy_buffer(std::mem::replace(&mut self.device, Buffer::null()));
    }
}
