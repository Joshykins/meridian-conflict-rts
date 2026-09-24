//! Vulkan device, memory and resource helpers. Everything here is thin: one
//! queue, dedicated allocations (the renderer owns a few dozen resources, far
//! below any allocation-count limit), explicit lifetimes.

use ash::vk;
use std::ffi::{c_char, CStr};

pub struct Gpu {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub memory: vk::PhysicalDeviceMemoryProperties,
    pub limits: vk::PhysicalDeviceLimits,
    pub device_name: String,
    pub surface_fn: ash::khr::surface::Instance,
    pub swapchain_fn: Option<ash::khr::swapchain::Device>,
    pub command_pool: vk::CommandPool,
}

#[derive(Debug)]
pub enum GpuError {
    Load(String),
    NoDevice(String),
    Vk(vk::Result),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::Load(e) => write!(f, "could not load Vulkan: {e}"),
            GpuError::NoDevice(e) => write!(f, "no usable Vulkan device: {e}"),
            GpuError::Vk(e) => write!(f, "Vulkan error: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<vk::Result> for GpuError {
    fn from(e: vk::Result) -> Self {
        GpuError::Vk(e)
    }
}

impl Gpu {
    /// `surface_extensions` are the instance extensions the window system
    /// needs; empty for headless rendering.
    pub fn new(surface_extensions: &[*const c_char]) -> Result<Gpu, GpuError> {
        let entry = unsafe { ash::Entry::load() }.map_err(|e| GpuError::Load(e.to_string()))?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"Meridian Conflict")
            .engine_name(c"meridian")
            .api_version(vk::API_VERSION_1_2);

        let mut layers: Vec<*const c_char> = Vec::new();
        if std::env::var_os("MC_VALIDATION").is_some() {
            let wanted = c"VK_LAYER_KHRONOS_validation";
            let available = unsafe { entry.enumerate_instance_layer_properties() }?;
            if available
                .iter()
                .any(|l| l.layer_name_as_c_str() == Ok(wanted))
            {
                layers.push(wanted.as_ptr());
            } else {
                log::warn!(
                    "MC_VALIDATION is set but the Khronos validation layer is not installed"
                );
            }
        }
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(surface_extensions)
            .enabled_layer_names(&layers);
        let instance = unsafe { entry.create_instance(&info, None) }?;
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical, queue_family, device_name) = Self::pick_device(&instance)?;
        let headless = surface_extensions.is_empty();

        let priorities = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let supported = unsafe { instance.get_physical_device_features(physical) };
        let features = vk::PhysicalDeviceFeatures::default()
            .multi_draw_indirect(true)
            .draw_indirect_first_instance(true)
            .sampler_anisotropy(supported.sampler_anisotropy == vk::TRUE)
            .depth_clamp(supported.depth_clamp == vk::TRUE);
        let mut extensions: Vec<*const c_char> = Vec::new();
        if !headless {
            extensions.push(ash::khr::swapchain::NAME.as_ptr());
        }
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_features(&features)
            .enabled_extension_names(&extensions);
        let device = unsafe { instance.create_device(physical, &device_info, None) }?;
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_fn =
            (!headless).then(|| ash::khr::swapchain::Device::new(&instance, &device));

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&pool_info, None) }?;

        let props = unsafe { instance.get_physical_device_properties(physical) };
        log::info!("Vulkan device: {device_name}");
        Ok(Gpu {
            memory: unsafe { instance.get_physical_device_memory_properties(physical) },
            limits: props.limits,
            entry,
            instance,
            physical,
            device,
            queue,
            queue_family,
            device_name,
            surface_fn,
            swapchain_fn,
            command_pool,
        })
    }

    /// Discrete GPUs first, then integrated, then anything. `MC_GPU=<substring>` overrides.
    fn pick_device(
        instance: &ash::Instance,
    ) -> Result<(vk::PhysicalDevice, u32, String), GpuError> {
        let wanted = std::env::var("MC_GPU").ok().map(|s| s.to_lowercase());
        let mut best: Option<(i32, vk::PhysicalDevice, u32, String)> = None;
        for physical in unsafe { instance.enumerate_physical_devices() }? {
            let props = unsafe { instance.get_physical_device_properties(physical) };
            let name = props
                .device_name_as_c_str()
                .unwrap_or(c"?")
                .to_string_lossy()
                .into_owned();
            let features = unsafe { instance.get_physical_device_features(physical) };
            if features.multi_draw_indirect == vk::FALSE
                || features.draw_indirect_first_instance == vk::FALSE
            {
                continue;
            }
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(physical) };
            let Some(family) = families.iter().position(|f| {
                f.queue_flags
                    .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
            }) else {
                continue;
            };
            let mut score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                _ => 0,
            };
            if wanted
                .as_ref()
                .is_some_and(|w| name.to_lowercase().contains(w))
            {
                score += 100;
            }
            if best.as_ref().is_none_or(|b| score > b.0) {
                best = Some((score, physical, family as u32, name));
            }
        }
        best.map(|(_, p, f, n)| (p, f, n)).ok_or_else(|| {
            GpuError::NoDevice("need a device with graphics+compute and multi-draw indirect".into())
        })
    }

    pub fn memory_type(&self, type_bits: u32, flags: vk::MemoryPropertyFlags) -> Option<u32> {
        (0..self.memory.memory_type_count).find(|&i| {
            type_bits & (1 << i) != 0
                && self.memory.memory_types[i as usize]
                    .property_flags
                    .contains(flags)
        })
    }

    pub(crate) fn allocate(
        &self,
        req: vk::MemoryRequirements,
        flags: vk::MemoryPropertyFlags,
    ) -> Result<vk::DeviceMemory, GpuError> {
        let index = self
            .memory_type(req.memory_type_bits, flags)
            .ok_or_else(|| GpuError::NoDevice(format!("no memory type for {flags:?}")))?;
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(index);
        Ok(unsafe { self.device.allocate_memory(&info, None) }?)
    }

    /// A buffer the CPU writes every frame or tick; stays mapped.
    pub fn host_buffer(&self, size: u64, usage: vk::BufferUsageFlags) -> Result<Buffer, GpuError> {
        self.buffer(
            size,
            usage,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            true,
        )
    }

    /// A buffer only the GPU touches after an optional initial upload.
    pub fn device_buffer(
        &self,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<Buffer, GpuError> {
        self.buffer(
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            false,
        )
    }

    fn buffer(
        &self,
        size: u64,
        usage: vk::BufferUsageFlags,
        flags: vk::MemoryPropertyFlags,
        map: bool,
    ) -> Result<Buffer, GpuError> {
        let size = size.max(16);
        let info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        unsafe {
            let buffer = self.device.create_buffer(&info, None)?;
            let memory =
                self.allocate(self.device.get_buffer_memory_requirements(buffer), flags)?;
            self.device.bind_buffer_memory(buffer, memory, 0)?;
            let mapped = if map {
                self.device
                    .map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())?
                    as *mut u8
            } else {
                std::ptr::null_mut()
            };
            Ok(Buffer {
                buffer,
                memory,
                size,
                mapped,
            })
        }
    }

    /// Creates a device-local buffer holding `data`.
    pub fn buffer_with_data(
        &self,
        data: &[u8],
        usage: vk::BufferUsageFlags,
    ) -> Result<Buffer, GpuError> {
        let dst = self.device_buffer(data.len() as u64, usage)?;
        if !data.is_empty() {
            let staging =
                self.host_buffer(data.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
            staging.write(0, data);
            self.submit_once(|cmd| unsafe {
                let region = [vk::BufferCopy::default().size(data.len() as u64)];
                self.device
                    .cmd_copy_buffer(cmd, staging.buffer, dst.buffer, &region);
            })?;
            self.destroy_buffer(staging);
        }
        Ok(dst)
    }

    pub fn destroy_buffer(&self, b: Buffer) {
        unsafe {
            self.device.destroy_buffer(b.buffer, None);
            self.device.free_memory(b.memory, None);
        }
    }

    pub fn image(&self, desc: &ImageDesc) -> Result<Image, GpuError> {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(desc.format)
            .extent(vk::Extent3D {
                width: desc.width,
                height: desc.height,
                depth: 1,
            })
            .mip_levels(desc.mips.max(1))
            .array_layers(desc.layers.max(1))
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(desc.usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        unsafe {
            let image = self.device.create_image(&info, None)?;
            let memory = self.allocate(
                self.device.get_image_memory_requirements(image),
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            self.device.bind_image_memory(image, memory, 0)?;
            let aspect = if desc.format == vk::Format::D32_SFLOAT {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            };
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(if desc.array {
                    vk::ImageViewType::TYPE_2D_ARRAY
                } else {
                    vk::ImageViewType::TYPE_2D
                })
                .format(desc.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: aspect,
                    base_mip_level: 0,
                    level_count: desc.mips.max(1),
                    base_array_layer: 0,
                    layer_count: desc.layers.max(1),
                });
            let view = self.device.create_image_view(&view_info, None)?;
            Ok(Image {
                image,
                view,
                memory,
                format: desc.format,
                width: desc.width,
                height: desc.height,
                layers: desc.layers.max(1),
                mips: desc.mips.max(1),
                aspect,
            })
        }
    }

    /// For teardown, where the owner is being dropped and cannot give the image away.
    pub fn destroy_image_ref(&self, i: &Image) {
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
            self.device.free_memory(i.memory, None);
        }
    }

    pub fn destroy_image(&self, i: Image) {
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
            self.device.free_memory(i.memory, None);
        }
    }

    /// Records, submits and waits. For set-up work only, never per frame.
    pub fn submit_once(&self, record: impl FnOnce(vk::CommandBuffer)) -> Result<(), GpuError> {
        unsafe {
            let info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let cmd = self.device.allocate_command_buffers(&info)?[0];
            self.device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            record(cmd);
            self.device.end_command_buffer(cmd)?;
            let cmds = [cmd];
            let submit = [vk::SubmitInfo::default().command_buffers(&cmds)];
            self.device
                .queue_submit(self.queue, &submit, vk::Fence::null())?;
            self.device.queue_wait_idle(self.queue)?;
            self.device.free_command_buffers(self.command_pool, &cmds);
        }
        Ok(())
    }

    /// Uploads pixel data into one layer/mip of `image` and leaves it shader-readable.
    /// `first_use` transitions from UNDEFINED; otherwise from SHADER_READ_ONLY.
    pub fn upload_image(
        &self,
        image: &Image,
        layer: u32,
        mip: u32,
        region: Option<vk::Rect2D>,
        data: &[u8],
        first_use: bool,
    ) -> Result<(), GpuError> {
        let staging = self.host_buffer(data.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
        staging.write(0, data);
        self.submit_once(|cmd| {
            self.record_image_upload(cmd, image, layer, mip, region, staging.buffer, 0, first_use)
        })?;
        self.destroy_buffer(staging);
        Ok(())
    }

    /// Records a copy from `src` into `image`. With `first_use` the whole image
    /// is transitioned from UNDEFINED, so do that only on the very first upload.
    #[allow(clippy::too_many_arguments)]
    pub fn record_image_upload(
        &self,
        cmd: vk::CommandBuffer,
        image: &Image,
        layer: u32,
        mip: u32,
        region: Option<vk::Rect2D>,
        src: vk::Buffer,
        src_offset: u64,
        first_use: bool,
    ) {
        let whole = vk::ImageSubresourceRange {
            aspect_mask: image.aspect,
            base_mip_level: 0,
            level_count: image.mips,
            base_array_layer: 0,
            layer_count: image.layers,
        };
        let one = vk::ImageSubresourceRange {
            aspect_mask: image.aspect,
            base_mip_level: mip,
            level_count: 1,
            base_array_layer: layer,
            layer_count: 1,
        };
        let (range, old) = if first_use {
            (whole, vk::ImageLayout::UNDEFINED)
        } else {
            (one, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        };
        let rect = region.unwrap_or(vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D {
                width: (image.width >> mip).max(1),
                height: (image.height >> mip).max(1),
            },
        });
        unsafe {
            self.transition(
                cmd,
                image.image,
                range,
                old,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            let copy = [vk::BufferImageCopy::default()
                .buffer_offset(src_offset)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: image.aspect,
                    mip_level: mip,
                    base_array_layer: layer,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D {
                    x: rect.offset.x,
                    y: rect.offset.y,
                    z: 0,
                })
                .image_extent(vk::Extent3D {
                    width: rect.extent.width,
                    height: rect.extent.height,
                    depth: 1,
                })];
            self.device.cmd_copy_buffer_to_image(
                cmd,
                src,
                image.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copy,
            );
            self.transition(
                cmd,
                image.image,
                range,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        }
    }

    /// Layout transition with conservative stage masks. Fine for set-up and the
    /// handful of per-frame transitions; hot paths use render-pass layouts.
    pub unsafe fn transition(
        &self,
        cmd: vk::CommandBuffer,
        image: vk::Image,
        range: vk::ImageSubresourceRange,
        old: vk::ImageLayout,
        new: vk::ImageLayout,
    ) {
        let access = |l: vk::ImageLayout| match l {
            vk::ImageLayout::TRANSFER_DST_OPTIMAL => vk::AccessFlags::TRANSFER_WRITE,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL => vk::AccessFlags::TRANSFER_READ,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => vk::AccessFlags::SHADER_READ,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            _ => vk::AccessFlags::empty(),
        };
        let barrier = [vk::ImageMemoryBarrier::default()
            .old_layout(old)
            .new_layout(new)
            .src_access_mask(access(old))
            .dst_access_mask(access(new))
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(range)];
        self.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &barrier,
        );
    }

    pub fn sampler(
        &self,
        filter: vk::Filter,
        address: vk::SamplerAddressMode,
        anisotropy: bool,
        compare: Option<vk::CompareOp>,
    ) -> Result<vk::Sampler, GpuError> {
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(filter)
            .min_filter(filter)
            .mipmap_mode(if filter == vk::Filter::LINEAR {
                vk::SamplerMipmapMode::LINEAR
            } else {
                vk::SamplerMipmapMode::NEAREST
            })
            .address_mode_u(address)
            .address_mode_v(address)
            .address_mode_w(address)
            .max_lod(vk::LOD_CLAMP_NONE)
            .anisotropy_enable(anisotropy)
            .max_anisotropy(if anisotropy { 8.0 } else { 1.0 })
            .compare_enable(compare.is_some())
            .compare_op(compare.unwrap_or(vk::CompareOp::ALWAYS));
        Ok(unsafe { self.device.create_sampler(&info, None) }?)
    }

    pub fn shader(&self, spirv: &[u8]) -> Result<vk::ShaderModule, GpuError> {
        let words: Vec<u32> = spirv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(unsafe {
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
        }?)
    }

    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().ok() };
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

pub struct Buffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: u64,
    mapped: *mut u8,
}

// The mapped pointer is only written through `write`, which takes `&self` but
// is called from the render thread alone.
unsafe impl Send for Buffer {}

impl Buffer {
    /// A handle to nothing; destroying it is a no-op in Vulkan.
    pub fn null() -> Buffer {
        Buffer {
            buffer: vk::Buffer::null(),
            memory: vk::DeviceMemory::null(),
            size: 0,
            mapped: std::ptr::null_mut(),
        }
    }

    pub fn write(&self, offset: u64, data: &[u8]) {
        assert!(!self.mapped.is_null(), "buffer is not host visible");
        assert!(
            offset + data.len() as u64 <= self.size,
            "buffer overflow: {} + {} > {}",
            offset,
            data.len(),
            self.size
        );
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.mapped.add(offset as usize),
                data.len(),
            )
        };
    }

    pub fn read(&self, offset: u64, out: &mut [u8]) {
        assert!(!self.mapped.is_null() && offset + out.len() as u64 <= self.size);
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.mapped.add(offset as usize),
                out.as_mut_ptr(),
                out.len(),
            )
        };
    }

    pub fn info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo {
            buffer: self.buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }
    }
}

pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    pub format: vk::Format,
    pub usage: vk::ImageUsageFlags,
    pub layers: u32,
    pub mips: u32,
    /// View as a 2D array even with one layer.
    pub array: bool,
}

pub struct Image {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub memory: vk::DeviceMemory,
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mips: u32,
    pub aspect: vk::ImageAspectFlags,
}

/// Name of an entry point as a C string; shaders use fixed names.
pub fn entry(name: &'static CStr) -> &'static CStr {
    name
}
