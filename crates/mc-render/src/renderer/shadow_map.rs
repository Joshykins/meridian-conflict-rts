//! The sun's shadow map: a depth array with one layer per cascade, a view and
//! framebuffer per layer for the shadow pass. Its size follows the graphics quality
//! (`SceneQuality::shadow_size`), so it can be remade while the game runs.

use super::shadow_cascades::CASCADES;
use crate::gpu::{Gpu, GpuError, Image, ImageDesc};
use crate::pipelines::DEPTH_FORMAT;
use ash::vk;

/// The scene set binding the shaders read the map through (`shadow_map`, bindings.wgsl).
const BINDING: u32 = 11;

pub(super) struct ShadowMap {
    image: Image,
    layer_views: [vk::ImageView; CASCADES],
    fbs: [vk::Framebuffer; CASCADES],
}

impl ShadowMap {
    /// A map `size` texels square for `pass` (the depth-only shadow pass), written into
    /// `scene_set`.
    pub(super) fn new(
        gpu: &Gpu,
        pass: vk::RenderPass,
        scene_set: vk::DescriptorSet,
        size: u32,
    ) -> Result<ShadowMap, GpuError> {
        let image = gpu.image(&ImageDesc {
            width: size,
            height: size,
            format: DEPTH_FORMAT,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            layers: CASCADES as u32,
            mips: 1,
            array: true,
        })?;
        let mut map = ShadowMap {
            image,
            layer_views: [vk::ImageView::null(); CASCADES],
            fbs: [vk::Framebuffer::null(); CASCADES],
        };
        for (layer, (view, fb)) in map.layer_views.iter_mut().zip(&mut map.fbs).enumerate() {
            // SAFETY: `image` is a live depth array image of this device with `CASCADES`
            // layers, and the view names one of them in its own format; the create info lives
            // to the end of the call.
            *view = unsafe {
                gpu.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(map.image.image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(DEPTH_FORMAT)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::DEPTH,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: layer as u32,
                            layer_count: 1,
                        }),
                    None,
                )
            }?;
            // SAFETY: the shadow pass has one depth attachment of `DEPTH_FORMAT`, which is
            // `view`'s format, and the size matches the image; the create info and the one-view
            // slice live to the end of the call.
            *fb = unsafe {
                gpu.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(pass)
                        .attachments(std::slice::from_ref(view))
                        .width(size)
                        .height(size)
                        .layers(1),
                    None,
                )
            }?;
        }
        let info = [vk::DescriptorImageInfo::default()
            .image_view(map.image.view)
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
        let write = [vk::WriteDescriptorSet::default()
            .dst_set(scene_set)
            .dst_binding(BINDING)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .image_info(&info)];
        // SAFETY: the renderer makes the map before any frame or after waiting for the device
        // to go idle, so no command buffer uses `scene_set`; binding 11 is its sampled shadow
        // image, the view is live, and `write`/`info` live to the end of the call.
        unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        Ok(map)
    }

    /// Texels along each side.
    pub(super) fn size(&self) -> u32 {
        self.image.width
    }

    /// The framebuffer of each cascade, near to far.
    pub(super) fn framebuffers(&self) -> &[vk::Framebuffer; CASCADES] {
        &self.fbs
    }

    /// Only once the device is idle, or no frame used the map.
    pub(super) fn destroy(&self, gpu: &Gpu) {
        // SAFETY: the caller guarantees no command buffer still uses these; each was made
        // from this device and is destroyed once.
        unsafe {
            for (fb, view) in self.fbs.iter().zip(&self.layer_views) {
                gpu.device.destroy_framebuffer(*fb, None);
                gpu.device.destroy_image_view(*view, None);
            }
        }
        gpu.destroy_image_ref(&self.image);
    }
}
