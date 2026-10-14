//! 3D images for the sky: the tiling cloud noise and the light volume (`light.rs`).

use super::*;

/// A `width`x`height`x`depth` 3D image in `format`, one mip, for storage writes and
/// filtered reads; its view is 3D. Left in UNDEFINED.
pub(super) fn volume_image(
    gpu: &Gpu,
    [width, height, depth]: [u32; 3],
    format: vk::Format,
) -> Result<Image, GpuError> {
    let dev = &gpu.device;
    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_3D)
        .format(format)
        .extent(vk::Extent3D {
            width,
            height,
            depth,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    // SAFETY: the device is alive and `info` lives to the end of the call; the image is bound
    // once, at offset 0, to fresh device-local memory its requirements allow; the 3D view
    // covers its one mip and layer in its own format.
    unsafe {
        let image = dev.create_image(&info, None)?;
        let memory = gpu.allocate(
            dev.get_image_memory_requirements(image),
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        dev.bind_image_memory(image, memory, 0)?;
        let aspect = vk::ImageAspectFlags::COLOR;
        let view = dev.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_3D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }),
            None,
        )?;
        Ok(Image {
            image,
            view,
            memory,
            format,
            width,
            height,
            layers: 1,
            mips: 1,
            aspect,
        })
    }
}
