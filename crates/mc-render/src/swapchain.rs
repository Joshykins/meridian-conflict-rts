//! The window's swapchain, shared by the renderer and the splash: which pixel
//! format the window takes, and making (or remaking) the chain at a size.

use crate::gpu::{Gpu, GpuError};
use ash::vk;

/// The window's pixel format: 8-bit sRGB, BGRA first, else whatever it offers.
pub(crate) fn surface_format(gpu: &Gpu, surface: vk::SurfaceKHR) -> Result<vk::Format, GpuError> {
    // SAFETY: `surface` was made on this instance and is alive.
    let formats = unsafe {
        gpu.surface_fn
            .get_physical_device_surface_formats(gpu.physical, surface)
    }?;
    formats
        .iter()
        .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
        .or_else(|| {
            formats
                .iter()
                .find(|f| f.format == vk::Format::R8G8B8A8_SRGB)
        })
        .or(formats.first())
        .map(|f| f.format)
        .ok_or_else(|| GpuError::NoDevice("surface reports no formats".into()))
}

/// Makes the chain on `surface`, retiring the one in `swapchain` if any, and
/// replaces `views` with views of its images. The surface's own extent wins
/// over `size` when it has one; returns the size it was made at.
pub(crate) fn rebuild(
    gpu: &Gpu,
    surface: vk::SurfaceKHR,
    format: vk::Format,
    vsync: bool,
    size: (u32, u32),
    swapchain: &mut vk::SwapchainKHR,
    views: &mut Vec<vk::ImageView>,
) -> Result<(u32, u32), GpuError> {
    let swapchain_fn = gpu
        .swapchain_fn
        .as_ref()
        .expect("window target has the swapchain extension");
    // SAFETY: `surface` belongs to this instance and physical device.
    let caps = unsafe {
        gpu.surface_fn
            .get_physical_device_surface_capabilities(gpu.physical, surface)
    }?;
    let (width, height) = if caps.current_extent.width != u32::MAX {
        (
            caps.current_extent.width.max(1),
            caps.current_extent.height.max(1),
        )
    } else {
        size
    };
    // SAFETY: as above.
    let modes = unsafe {
        gpu.surface_fn
            .get_physical_device_surface_present_modes(gpu.physical, surface)
    }?;
    let mode = if !vsync && modes.contains(&vk::PresentModeKHR::MAILBOX) {
        vk::PresentModeKHR::MAILBOX
    } else if !vsync && modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
        vk::PresentModeKHR::IMMEDIATE
    } else {
        vk::PresentModeKHR::FIFO
    };
    let mut count = caps.min_image_count + 1;
    if caps.max_image_count > 0 {
        count = count.min(caps.max_image_count);
    }
    let old = *swapchain;
    let info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(count)
        .image_format(format)
        .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
        .image_extent(vk::Extent2D { width, height })
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(mode)
        .clipped(true)
        .old_swapchain(old);
    // SAFETY: the caller waited for the device to go idle, so nothing still
    // uses the old chain or its views when they go.
    unsafe {
        *swapchain = swapchain_fn.create_swapchain(&info, None)?;
        for v in views.drain(..) {
            gpu.device.destroy_image_view(v, None);
        }
        if old != vk::SwapchainKHR::null() {
            swapchain_fn.destroy_swapchain(old, None);
        }
    }
    // SAFETY: the chain was just made on this device.
    for image in unsafe { swapchain_fn.get_swapchain_images(*swapchain) }? {
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        // SAFETY: `image` belongs to the chain just made on this device.
        views.push(unsafe { gpu.device.create_image_view(&view_info, None) }?);
    }
    Ok((width, height))
}
