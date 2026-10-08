//! Every model's surface marks (`mc_models::Mark`, docs/STANDARD.md) in one list, scene
//! set binding 35: boxes in a model's own space saying what its surface carries there
//! (a zone, hazard stripes, a stencilled number). `ModelInfo::marks` finds a model's run.

use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::surface_mark;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use mc_models::Mark;

/// One mark as the shaders read it (common.wgsl `SurfaceMark`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuMark {
    pub(crate) min: [f32; 3],
    /// `gpu_consts::surface_mark` kind.
    pub(crate) kind: u32,
    pub(crate) max: [f32; 3],
    pub(crate) arg: f32,
}

/// The marks gathered model by model while the renderer builds its model table.
#[derive(Default)]
pub(crate) struct MarkList {
    marks: Vec<GpuMark>,
}

impl MarkList {
    /// Adds a model's marks; returns its `ModelInfo::marks` word (first | count << 24).
    /// A model holds at most `surface_mark::MAX` (an mc-models test holds every catalogue
    /// model to it).
    pub(crate) fn add(&mut self, marks: &[Mark]) -> u32 {
        let first = self.marks.len() as u32;
        let count = marks.len().min(surface_mark::MAX as usize);
        self.marks.extend(marks[..count].iter().map(|m| GpuMark {
            min: m.min,
            kind: m.kind,
            max: m.max,
            arg: m.arg,
        }));
        first | (count as u32) << 24
    }

    pub(crate) fn upload(&self, gpu: &Gpu) -> Result<Buffer, GpuError> {
        // An empty list still binds one record.
        let table: Vec<GpuMark> = if self.marks.is_empty() {
            vec![GpuMark::zeroed()]
        } else {
            self.marks.clone()
        };
        let bytes: &[u8] = bytemuck::cast_slice(&table);
        let buffer = gpu.host_buffer(bytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        buffer.write(0, bytes);
        Ok(buffer)
    }
}
