//! Cloud targets rebuilt when window size or cloud quality changes.

use super::*;

impl Sky {
    /// Size-dependent targets: the march, its two histories, and the depth it stops at.
    pub fn resize(
        &mut self,
        gpu: &Gpu,
        width: u32,
        height: u32,
        depth: vk::ImageView,
        cloud_divisor: u32,
    ) -> Result<(), GpuError> {
        // SAFETY: `resize` runs only from the renderer's `create_size_dependent`, after the
        // device went idle, so no command buffer in flight uses these framebuffers; they are
        // drained, so each is destroyed once.
        unsafe {
            for fb in self.target_fbs.drain(..) {
                gpu.device.destroy_framebuffer(fb, None);
            }
        }
        for old in self.targets.drain(..) {
            gpu.destroy_image(old);
        }
        let div = cloud_divisor.clamp(1, 4);
        let (w, h) = (width.div_ceil(div).max(1), height.div_ceil(div).max(1));
        for k in 0..3 {
            let image = gpu.image(&ImageDesc {
                width: w,
                height: h,
                format: if k == 0 {
                    CLOUD_MARCH_FORMAT
                } else {
                    HDR_FORMAT
                },
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                layers: 1,
                mips: 1,
                array: false,
            })?;
            let views = [image.view];
            // SAFETY: `image` was just made at `w`x`h` in the format of the pass it is paired
            // with (`CLOUD_MARCH_FORMAT` for the march, `HDR_FORMAT` for the resolve); the
            // create info and `views` live to the end of the call.
            let fb = unsafe {
                gpu.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(if k == 0 {
                            self.march_pass
                        } else {
                            self.resolve_pass
                        })
                        .attachments(&views)
                        .width(w)
                        .height(h)
                        .layers(1),
                    None,
                )
            }?;
            // Give the target its sampled layout before anything reads it.
            // SAFETY: `submit_once` hands a recording command buffer of this device, and
            // `image` was just made, one mip and layer, still in UNDEFINED.
            gpu.submit_once(|cmd| unsafe {
                gpu.transition(
                    cmd,
                    image.image,
                    vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            })?;
            self.targets.push(image);
            self.target_fbs.push(fb);
        }
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let sampled = vk::DescriptorType::SAMPLED_IMAGE;
        for (k, set) in self.draw_sets.iter().enumerate() {
            write_image(
                gpu,
                *set,
                0,
                sampled,
                depth,
                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            );
            write_image(gpu, *set, 2, sampled, self.targets[0].view, read);
            write_image(gpu, *set, 3, sampled, self.targets[1 + (1 - k)].view, read);
            write_image(gpu, *set, 4, sampled, self.targets[1 + k].view, read);
        }
        self.march_size = (w, h);
        self.history_valid = false;
        Ok(())
    }
}
