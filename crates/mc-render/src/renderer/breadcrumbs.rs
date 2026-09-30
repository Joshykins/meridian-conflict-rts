//! Where the GPU was when the device was lost. With `VK_AMD_buffer_marker`, each
//! point of a frame (a timer scope opening or closing, or a `crumb` before one
//! draw) writes its number twice: when the command processor reaches it (top of
//! pipe) and when everything before it has finished (bottom of pipe). After a
//! loss the hung or faulting work lies after the last point that finished, and
//! at or before the last one reached.
//!
//! Crumbs before single draws are off unless `MERIDIAN_GPU_CRUMBS=1`, which also
//! draws models one draw slot at a time so a crumb can name the model.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use ash::vk;

/// Points a frame may record; the marker keeps the rest of the word for the frame.
const MAX_POINTS: u32 = 1 << POINT_BITS;
const POINT_BITS: u32 = 12;

/// Lines of the report either side of the last finished and last reached points.
const CONTEXT: usize = 6;

const REACHED: u64 = 0;
const FINISHED: u64 = 4;

pub(super) struct Breadcrumbs {
    marker: ash::amd::buffer_marker::Device,
    buffer: crate::gpu::Buffer,
    frame: Cell<u32>,
    points: RefCell<Vec<Cow<'static, str>>>,
    /// `MERIDIAN_GPU_CRUMBS=1`: a crumb before each draw that asks for one.
    pub(super) fine: bool,
}

impl Breadcrumbs {
    /// `None` where the driver lacks `VK_AMD_buffer_marker`.
    pub(super) fn new(gpu: &crate::gpu::Gpu) -> Result<Option<Self>, crate::gpu::GpuError> {
        let Some(marker) = &gpu.buffer_marker else {
            return Ok(None);
        };
        let buffer = gpu.host_buffer(8, vk::BufferUsageFlags::TRANSFER_DST)?;
        buffer.write(0, &[0u8; 8]);
        Ok(Some(Breadcrumbs {
            marker: marker.clone(),
            buffer,
            frame: Cell::new(0),
            points: RefCell::new(Vec::new()),
            fine: std::env::var("MERIDIAN_GPU_CRUMBS").is_ok_and(|v| v == "1"),
        }))
    }

    /// Call once per frame as its recording begins.
    pub(super) fn next_frame(&self) {
        self.frame
            .set(self.frame.get().wrapping_add(1) & (u32::MAX >> POINT_BITS));
        self.points.borrow_mut().clear();
    }

    /// Records a point named `label` in `cmd`. Past `MAX_POINTS` a frame's points
    /// are dropped (the report says how many there were).
    pub(super) fn point(&self, cmd: vk::CommandBuffer, label: Cow<'static, str>) {
        let mut points = self.points.borrow_mut();
        let id = points.len() as u32;
        points.push(label);
        if id >= MAX_POINTS {
            return;
        }
        let value = (self.frame.get() << POINT_BITS) | id;
        for (stage, at) in [
            (vk::PipelineStageFlags::TOP_OF_PIPE, REACHED),
            (vk::PipelineStageFlags::BOTTOM_OF_PIPE, FINISHED),
        ] {
            // SAFETY: `cmd` is recording (points are written only while the frame's buffer
            // records); `buffer` is this device's, made with TRANSFER_DST, and `at` + 4 lies
            // inside its 8 bytes on a 4-byte boundary.
            unsafe {
                self.marker
                    .cmd_write_buffer_marker(cmd, stage, self.buffer.buffer, at, value)
            };
        }
    }

    /// Logs the points around where the GPU stopped. Only the one frame is ever in
    /// flight, so the recorded points are that frame's.
    pub(super) fn report(&self) {
        let mut words = [0u8; 8];
        self.buffer.read(0, &mut words);
        let word = |at: usize| {
            u32::from_le_bytes([words[at], words[at + 1], words[at + 2], words[at + 3]])
        };
        let (reached, finished) = (word(0), word(4));
        let points = self.points.borrow();
        let frame = self.frame.get();
        let of_frame =
            |w: u32| (w >> POINT_BITS == frame).then_some((w & (MAX_POINTS - 1)) as usize);
        let (reached, finished) = (of_frame(reached), of_frame(finished));
        log::error!(
            "device lost in frame {frame} ({} points{}): last finished {}, last reached {}",
            points.len(),
            if self.fine { ", fine" } else { "" },
            finished.map_or("none this frame".into(), |i| format!("#{i}")),
            reached.map_or("none this frame".into(), |i| format!("#{i}")),
        );
        let lo = finished.unwrap_or(0).saturating_sub(CONTEXT);
        let hi = reached
            .unwrap_or(points.len())
            .max(finished.unwrap_or(0))
            .saturating_add(CONTEXT)
            .min(points.len().saturating_sub(1));
        let mut lines = Vec::new();
        for (i, label) in points.iter().enumerate().take(hi + 1).skip(lo) {
            let mark = match (Some(i) == finished, Some(i) == reached) {
                (true, true) => "  <- last finished and last reached",
                (true, false) => "  <- last finished: the fault is after this",
                (false, true) => "  <- last reached",
                _ => "",
            };
            lines.push(format!("{i:>5} {label}{mark}"));
        }
        log::error!("points round the loss:\n{}", lines.join("\n"));
    }

    pub(super) fn destroy(self, gpu: &crate::gpu::Gpu) {
        gpu.destroy_buffer(self.buffer);
    }
}
