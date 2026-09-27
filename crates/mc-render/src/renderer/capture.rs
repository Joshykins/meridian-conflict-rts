//! A copy of what the window showed, HUD and all, for the game's Mark Issue.
//! Asked for before a frame; the swapchain image is copied into a host buffer
//! after the frame is drawn, and read once the frame's fence has passed.

use crate::gpu::{Buffer, Gpu, GpuError};
use ash::vk;

/// One captured frame, tightly packed RGBA8, rows top to bottom.
pub struct Shot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub(crate) struct Capture {
    wanted: bool,
    /// Copied into this frame, read after its fence: the buffer, size and whether it is BGRA.
    pending: Option<(Buffer, u32, u32, bool)>,
    done: Option<Shot>,
    refused: bool,
}

impl Capture {
    pub(crate) fn request(&mut self) {
        self.wanted = true;
    }

    pub(crate) fn wanted(&self) -> bool {
        self.wanted && self.pending.is_none()
    }

    /// The window cannot be copied from: the request is dropped, and `take` says so.
    pub(crate) fn refuse(&mut self) {
        self.wanted = false;
        self.refused = true;
    }

    /// Whether the last request was dropped because the window cannot be copied.
    pub(crate) fn refused(&mut self) -> bool {
        std::mem::take(&mut self.refused)
    }

    pub(crate) fn take(&mut self) -> Option<Shot> {
        self.done.take()
    }

    /// Records the copy of `image`, which the present pass left in
    /// PRESENT_SRC_KHR and which this puts back there. Outside a render pass.
    pub(crate) fn record(
        &mut self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        image: vk::Image,
        (width, height): (u32, u32),
        format: vk::Format,
    ) -> Result<(), GpuError> {
        if !self.wanted || self.pending.is_some() {
            return Ok(());
        }
        self.wanted = false;
        let buffer = gpu.host_buffer(
            u64::from(width) * u64::from(height) * 4,
            vk::BufferUsageFlags::TRANSFER_DST,
        )?;
        let range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let turn = |from, to, src, dst| {
            vk::ImageMemoryBarrier::default()
                .old_layout(from)
                .new_layout(to)
                .src_access_mask(src)
                .dst_access_mask(dst)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range)
        };
        let copy = [vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })];
        let device = &gpu.device;
        // SAFETY: `cmd` is recording outside a render pass; `image` is this frame's
        // swapchain image, made with TRANSFER_SRC usage, last written as a colour
        // attachment and left in PRESENT_SRC_KHR by the present pass; it is turned to
        // TRANSFER_SRC_OPTIMAL for the copy and back before the submit that presents it.
        // `buffer` holds `width * height * 4` bytes and lives until `collect`.
        unsafe {
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[turn(
                    vk::ImageLayout::PRESENT_SRC_KHR,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    vk::AccessFlags::TRANSFER_READ,
                )],
            );
            device.cmd_copy_image_to_buffer(
                cmd,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buffer.buffer,
                &copy,
            );
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[turn(
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::ImageLayout::PRESENT_SRC_KHR,
                    vk::AccessFlags::TRANSFER_READ,
                    vk::AccessFlags::empty(),
                )],
            );
        }
        let bgra = matches!(
            format,
            vk::Format::B8G8R8A8_SRGB | vk::Format::B8G8R8A8_UNORM
        );
        self.pending = Some((buffer, width, height, bgra));
        Ok(())
    }

    /// Call once the fence of the frame that recorded the copy has passed.
    pub(crate) fn collect(&mut self, gpu: &Gpu) {
        let Some((buffer, width, height, bgra)) = self.pending.take() else {
            return;
        };
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        buffer.read(0, &mut rgba);
        gpu.destroy_buffer(buffer);
        for px in rgba.as_chunks_mut::<4>().0 {
            if bgra {
                px.swap(0, 2);
            }
            // The window is opaque whatever alpha the frame left.
            px[3] = 255;
        }
        self.done = Some(Shot {
            width,
            height,
            rgba,
        });
    }

    /// The device must be idle.
    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        if let Some((buffer, ..)) = self.pending.take() {
            gpu.destroy_buffer(buffer);
        }
    }
}
