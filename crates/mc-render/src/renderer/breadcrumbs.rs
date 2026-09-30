//! Where the GPU was when the device was lost. With `VK_AMD_buffer_marker`, each
//! point of a frame (a timer scope opening or closing, or a `crumb` before one
//! draw) has two words of its own in a host-visible buffer: the frame number is
//! written into the first when the command processor reaches the point (top of
//! pipe) and into the second when everything before it has finished (bottom of
//! pipe). After a loss, every point of the last two frames is known reached or not
//! and finished or not, so the work that hung is the stretch between the last
//! finished point and the first unfinished one.
//!
//! On a GPU with the extension (AMD) crumbs before single draws are on, models
//! drawn one draw slot at a time so a crumb can name the model; `MERIDIAN_GPU_CRUMBS=0`
//! turns that off (scope edges stay marked).

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use ash::vk;

/// Points a frame may record; past that they are counted but not marked.
const MAX_POINTS: usize = 4096;

/// Finished points shown before the first unfinished one, and unreached after it.
const BEFORE: usize = 8;
const AFTER: usize = 6;
/// Reached-but-unfinished points listed at most.
const RUNNING: usize = 60;

pub(super) struct Breadcrumbs {
    marker: ash::amd::buffer_marker::Device,
    /// Per point: the frame that reached it, then the frame that finished it (u32 each).
    buffer: crate::gpu::Buffer,
    frame: Cell<u32>,
    points: RefCell<Vec<Cow<'static, str>>>,
    /// The frame before's points, in case the GPU never got to this one.
    previous: RefCell<Vec<Cow<'static, str>>>,
    /// Crumbs before single draws (on unless `MERIDIAN_GPU_CRUMBS=0`).
    pub(super) fine: bool,
}

impl Breadcrumbs {
    /// `None` where the driver lacks `VK_AMD_buffer_marker`.
    pub(super) fn new(gpu: &crate::gpu::Gpu) -> Result<Option<Self>, crate::gpu::GpuError> {
        let Some(marker) = &gpu.buffer_marker else {
            return Ok(None);
        };
        let size = (MAX_POINTS * 8) as u64;
        let buffer = gpu.host_buffer(size, vk::BufferUsageFlags::TRANSFER_DST)?;
        buffer.write(0, &vec![0u8; size as usize]);
        let fine = std::env::var("MERIDIAN_GPU_CRUMBS").map_or(true, |v| v.trim() != "0");
        log::info!(
            "GPU breadcrumbs on ({})",
            if fine {
                "every draw"
            } else {
                "scopes only: MERIDIAN_GPU_CRUMBS=0"
            }
        );
        Ok(Some(Breadcrumbs {
            marker: marker.clone(),
            buffer,
            frame: Cell::new(0),
            points: RefCell::new(Vec::new()),
            previous: RefCell::new(Vec::new()),
            fine,
        }))
    }

    /// Call once per frame as its recording begins.
    pub(super) fn next_frame(&self) {
        // Never 0: a word still 0 was never written.
        self.frame.set(self.frame.get().wrapping_add(1).max(1));
        let mut points = self.points.borrow_mut();
        let mut previous = self.previous.borrow_mut();
        std::mem::swap(&mut *points, &mut *previous);
        points.clear();
    }

    /// Records a point named `label` in `cmd`. Past `MAX_POINTS` a frame's points
    /// are counted but not marked.
    pub(super) fn point(&self, cmd: vk::CommandBuffer, label: Cow<'static, str>) {
        let mut points = self.points.borrow_mut();
        let id = points.len();
        points.push(label);
        if id >= MAX_POINTS {
            return;
        }
        let at = id as u64 * 8;
        for (stage, at) in [
            (vk::PipelineStageFlags::TOP_OF_PIPE, at),
            (vk::PipelineStageFlags::BOTTOM_OF_PIPE, at + 4),
        ] {
            // SAFETY: `cmd` is recording (points are written only while the frame's buffer
            // records); `buffer` is this device's, made with TRANSFER_DST, and `at` + 4 lies
            // inside its `MAX_POINTS * 8` bytes on a 4-byte boundary (`id` < `MAX_POINTS`).
            unsafe {
                self.marker.cmd_write_buffer_marker(
                    cmd,
                    stage,
                    self.buffer.buffer,
                    at,
                    self.frame.get(),
                )
            };
        }
    }

    /// Logs, for the last two frames, which points the GPU reached and finished.
    /// Only one frame is ever in flight, so these are the frames it was on.
    pub(super) fn report(&self) {
        let mut bytes = vec![0u8; MAX_POINTS * 8];
        self.buffer.read(0, &mut bytes);
        let word = |i: usize| {
            u32::from_le_bytes([
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ])
        };
        let frame = self.frame.get();
        let previous_frame = frame.wrapping_sub(1).max(1);
        log::error!("device lost while frame {frame} was the last recorded");
        for (f, points) in [
            (previous_frame, &*self.previous.borrow()),
            (frame, &*self.points.borrow()),
        ] {
            let marked = points.len().min(MAX_POINTS);
            let state: Vec<(bool, bool)> = (0..marked)
                .map(|i| (word(i * 2) == f, word(i * 2 + 1) == f))
                .collect();
            let reached = state.iter().filter(|s| s.0).count();
            let finished = state.iter().filter(|s| s.1).count();
            let first_open = state.iter().position(|s| !s.1);
            log::error!(
                "frame {f}: {} points ({marked} marked), {reached} reached, {finished} finished{}",
                points.len(),
                match first_open {
                    None if marked > 0 => ": all finished".to_owned(),
                    None => String::new(),
                    Some(i) => format!(": first unfinished #{i} {}", points[i].trim()),
                }
            );
            let Some(open) = first_open else {
                continue;
            };
            let mut lines = Vec::new();
            let mut running = 0;
            for (i, &(r, fin)) in state.iter().enumerate() {
                let near_open = i + BEFORE >= open && i <= open + AFTER;
                let is_running = r && !fin;
                if !(near_open || (is_running && running < RUNNING)) {
                    continue;
                }
                running += usize::from(is_running);
                let tag = match (r, fin) {
                    (_, true) => "finished",
                    (true, false) => "RUNNING (reached, not finished)",
                    (false, false) => "not reached",
                };
                lines.push(format!("{i:>5} {:<44} {tag}", points[i]));
            }
            log::error!("frame {f} round the stop:\n{}", lines.join("\n"));
        }
    }

    pub(super) fn destroy(self, gpu: &crate::gpu::Gpu) {
        gpu.destroy_buffer(self.buffer);
    }
}
