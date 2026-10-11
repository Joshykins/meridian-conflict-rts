//! Cloud targets rebuilt when window size or cloud quality changes.

use super::*;

impl Sky {
    /// Size-dependent targets: the march, its two histories, the half of the march
    /// marched each frame (`MARCH_HALF`), and the depth it stops at. The march and the
    /// histories also go into the scene set (bindings 36-38), for the effects drawn past
    /// the clouds (bindings.wgsl `past_clouds`).
    pub fn resize(
        &mut self,
        gpu: &Gpu,
        (width, height): (u32, u32),
        depth: vk::ImageView,
        cloud_divisor: u32,
        scene_set: vk::DescriptorSet,
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
        for k in 0..4 {
            let march = k == 0 || k == MARCH_HALF;
            // Half as wide: each row marches every other texel (clouds.wgsl `march_phase`).
            let tw = if k == MARCH_HALF { w.div_ceil(2) } else { w };
            let image = gpu.image(&ImageDesc {
                width: tw,
                height: h,
                format: if march {
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
            // SAFETY: `image` was just made at `tw`x`h` in the format of the pass it is paired
            // with (`CLOUD_MARCH_FORMAT` for the march and its fill, `HDR_FORMAT` for the
            // resolve); the create info and `views` live to the end of the call.
            let fb = unsafe {
                gpu.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(if march {
                            self.march_pass
                        } else {
                            self.resolve_pass
                        })
                        .attachments(&views)
                        .width(tw)
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
            write_image(gpu, *set, 6, sampled, self.targets[MARCH_HALF].view, read);
        }
        for (binding, target) in [(36, 0), (37, 1), (38, 2)] {
            write_image(
                gpu,
                scene_set,
                binding,
                sampled,
                self.targets[target].view,
                read,
            );
        }
        self.march_size = (w, h);
        self.history_valid = false;
        Ok(())
    }
}
