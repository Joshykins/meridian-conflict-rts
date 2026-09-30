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
    /// Pipeline statistics queries (triangles, shader invocations) are enabled.
    pub pipeline_stats: bool,
    /// Device memory allocations not yet freed. Every buffer and image goes through
    /// `allocate` and the `destroy_*` functions, so a resource its owner forgot shows
    /// up here when the device goes (CLAUDE.md section 6: GPU resources are owned).
    live_allocations: std::sync::atomic::AtomicUsize,
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

/// The driver version as the vendor numbers it: NVIDIA packs 10.8.8.6 bits, Intel on
/// Windows 18.14 bits; everyone else uses the Vulkan major.minor.patch packing.
fn driver_version(vendor: u32, v: u32) -> String {
    match vendor {
        0x10de => format!(
            "{}.{}.{}.{}",
            v >> 22,
            (v >> 14) & 0xff,
            (v >> 6) & 0xff,
            v & 0x3f
        ),
        0x8086 if cfg!(windows) => format!("{}.{}", v >> 14, v & 0x3fff),
        _ => format!(
            "{}.{}.{}",
            vk::api_version_major(v),
            vk::api_version_minor(v),
            vk::api_version_patch(v)
        ),
    }
}

impl Gpu {
    /// `surface_extensions` are the instance extensions the window system
    /// needs; empty for headless rendering.
    pub fn new(surface_extensions: &[*const c_char]) -> Result<Gpu, GpuError> {
        // The splash and the first renderer open their devices at the same time,
        // on two threads. Some drivers' layers (seen on an AMD laptop) answer a
        // count query with VK_INCOMPLETE while another instance is enumerating:
        // instances are opened one at a time, and a count query is retried.
        static OPENING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let opening = OPENING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Keep the loader alive for as long as anything made through it lives.
        let entry = load_entry()?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"Meridian Conflict")
            .engine_name(c"meridian")
            .api_version(vk::API_VERSION_1_2);

        let mut layers: Vec<*const c_char> = Vec::new();
        if std::env::var_os("MC_VALIDATION").is_some() {
            let wanted = c"VK_LAYER_KHRONOS_validation";
            // SAFETY: `entry` holds a loaded Vulkan loader; this query has no other
            // requirement.
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
        // MoltenVK devices are hidden by the loader unless portability enumeration
        // is requested. Query support so the same path also works on native Vulkan.
        // SAFETY: `entry` is a live loader; no layer name is supplied.
        let available =
            retry_incomplete(|| unsafe { entry.enumerate_instance_extension_properties(None) })?;
        let portability = available
            .iter()
            .any(|e| e.extension_name_as_c_str() == Ok(ash::khr::portability_enumeration::NAME));
        let mut instance_extensions = surface_extensions.to_vec();
        let mut flags = vk::InstanceCreateFlags::empty();
        if portability {
            instance_extensions.push(ash::khr::portability_enumeration::NAME.as_ptr());
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .flags(flags)
            .enabled_extension_names(&instance_extensions)
            .enabled_layer_names(&layers);
        // SAFETY: `info` and the `app`, layer and extension name arrays it points to live to
        // the end of the call, and every name is a NUL-terminated string (`c"..."` literals or
        // the window system's static list).
        let instance = unsafe { entry.create_instance(&info, None) }?;
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical, queue_family, device_name) = Self::pick_device(&instance)?;
        drop(opening);
        let headless = surface_extensions.is_empty();

        let priorities = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        // SAFETY: `physical` was enumerated from this live instance.
        let supported = unsafe { instance.get_physical_device_features(physical) };
        // MERIDIAN_GPU_STATS=0 leaves pipeline statistics queries off on the device.
        let pipeline_stats = supported.pipeline_statistics_query == vk::TRUE
            && std::env::var("MERIDIAN_GPU_STATS").map_or(true, |v| v != "0");
        let features = vk::PhysicalDeviceFeatures::default()
            .multi_draw_indirect(true)
            .draw_indirect_first_instance(true)
            .sampler_anisotropy(supported.sampler_anisotropy == vk::TRUE)
            .depth_clamp(supported.depth_clamp == vk::TRUE)
            .pipeline_statistics_query(pipeline_stats);
        let mut extensions: Vec<*const c_char> = Vec::new();
        if !headless {
            extensions.push(ash::khr::swapchain::NAME.as_ptr());
        }
        // A device advertising the portability subset requires it to be enabled.
        // SAFETY: `physical` was enumerated from this live instance.
        let available = unsafe { instance.enumerate_device_extension_properties(physical) }?;
        if available
            .iter()
            .any(|e| e.extension_name_as_c_str() == Ok(ash::khr::portability_subset::NAME))
        {
            extensions.push(ash::khr::portability_subset::NAME.as_ptr());
        }
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_features(&features)
            .enabled_extension_names(&extensions);
        // SAFETY: `physical` belongs to `instance`; `device_info` and the queue, priority,
        // feature and extension arrays it borrows live to the end of the call; multi-draw
        // indirect and first-instance were checked in `pick_device`, and the optional features
        // are enabled only when `supported` reports them.
        let device = unsafe { instance.create_device(physical, &device_info, None) }?;
        // SAFETY: `device_info` asked for one queue of `queue_family`, so queue 0 of it exists.
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_fn =
            (!headless).then(|| ash::khr::swapchain::Device::new(&instance, &device));

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: the device is alive and `pool_info` names the queue family it was created
        // with.
        let command_pool = unsafe { device.create_command_pool(&pool_info, None) }?;

        // SAFETY: `physical` was enumerated from this live instance.
        let props = unsafe { instance.get_physical_device_properties(physical) };
        // SAFETY: `physical` was enumerated from this live instance.
        let memory = unsafe { instance.get_physical_device_memory_properties(physical) };
        let vram_mib: u64 = memory.memory_heaps[..memory.memory_heap_count as usize]
            .iter()
            .filter(|h| h.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|h| h.size >> 20)
            .sum();
        log::info!(
            "Vulkan device: {device_name} ({:?}, vendor {:04x}, device {:04x}), driver {}, \
             Vulkan {}.{}.{}, {vram_mib} MiB device memory",
            props.device_type,
            props.vendor_id,
            props.device_id,
            driver_version(props.vendor_id, props.driver_version),
            vk::api_version_major(props.api_version),
            vk::api_version_minor(props.api_version),
            vk::api_version_patch(props.api_version),
        );
        Ok(Gpu {
            memory,
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
            pipeline_stats,
            live_allocations: Default::default(),
        })
    }

    /// Discrete GPUs first, then integrated, then anything. `MC_GPU=<substring>` overrides.
    fn pick_device(
        instance: &ash::Instance,
    ) -> Result<(vk::PhysicalDevice, u32, String), GpuError> {
        let wanted = std::env::var("MC_GPU").ok().map(|s| s.to_lowercase());
        let mut best: Option<(i32, vk::PhysicalDevice, u32, String)> = None;
        // SAFETY: `instance` is alive for the call.
        for physical in retry_incomplete(|| unsafe { instance.enumerate_physical_devices() })? {
            // SAFETY: `physical` was just enumerated from this live instance.
            let props = unsafe { instance.get_physical_device_properties(physical) };
            let name = props
                .device_name_as_c_str()
                .unwrap_or(c"?")
                .to_string_lossy()
                .into_owned();
            // SAFETY: `physical` was just enumerated from this live instance.
            let features = unsafe { instance.get_physical_device_features(physical) };
            if features.multi_draw_indirect == vk::FALSE
                || features.draw_indirect_first_instance == vk::FALSE
            {
                continue;
            }
            let families =
                // SAFETY: `physical` was just enumerated from this live instance.
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
        // SAFETY: the device is alive and `index` is a memory type the device reports and `req`
        // allows.
        let memory = unsafe { self.device.allocate_memory(&info, None) }?;
        self.live_allocations
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(memory)
    }

    /// Counts a freed allocation (see `live_allocations`).
    fn freed(&self, memory: vk::DeviceMemory) {
        if memory != vk::DeviceMemory::null() {
            self.live_allocations
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
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
        // SAFETY: the device is alive and `info` lives to the end of the call; the new buffer
        // is bound once, at offset 0, to fresh memory of a type its requirements allow; the
        // memory is mapped at most once, and only when `map` is set, which only `host_buffer`
        // does and it asks for HOST_VISIBLE memory.
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
            // SAFETY: `cmd` is recording inside `submit_once`; both buffers are this device's,
            // `staging` has TRANSFER_SRC and `dst` TRANSFER_DST (added by `device_buffer`), and
            // each holds at least `data.len()` bytes; `submit_once` waits for the copy before
            // `staging` is destroyed.
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
        // SAFETY: `b` is taken by value, so its handles are destroyed once (null handles from
        // `Buffer::null` are a no-op), and freeing mapped memory unmaps it. By convention
        // callers destroy a buffer only once the GPU has finished with it (after a fence wait,
        // `submit_once`'s queue wait or a device-idle wait); the type does not enforce this.
        unsafe {
            self.device.destroy_buffer(b.buffer, None);
            self.device.free_memory(b.memory, None);
        }
        self.freed(b.memory);
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
        // SAFETY: the device is alive and `info` lives to the end of the call; the image is
        // bound once, at offset 0, to fresh device-local memory its requirements allow; the
        // view covers exactly the image's mips and layers with its format and aspect.
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
        // SAFETY: the handles were made together by `image` on this device; callers use this
        // from teardown only, once per image, after the device has gone idle, and never use the
        // image afterwards.
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
            self.device.free_memory(i.memory, None);
        }
        self.freed(i.memory);
    }

    pub fn destroy_image(&self, i: Image) {
        // SAFETY: `i` is taken by value, so its handles are destroyed once, view before image.
        // By convention callers destroy an image only once the GPU has finished with it (after
        // a fence wait or a device-idle wait); the type does not enforce this.
        unsafe {
            self.device.destroy_image_view(i.view, None);
            self.device.destroy_image(i.image, None);
            self.device.free_memory(i.memory, None);
        }
        self.freed(i.memory);
    }

    /// Records, submits and waits. For set-up work only, never per frame.
    pub fn submit_once(&self, record: impl FnOnce(vk::CommandBuffer)) -> Result<(), GpuError> {
        // SAFETY: the pool is this device's and allows resetting; the buffer is allocated,
        // begun, recorded, ended and submitted in order, and `queue_wait_idle` finishes it
        // before it is freed. The pool and queue are used only from the thread that owns this
        // `Gpu` (each renderer or splash owns its own and does not share it between threads).
        // `record` gets a buffer in the recording state.
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
        // SAFETY: the caller passes `cmd` recording on this device and `src` with TRANSFER_SRC
        // holding the pixels at `src_offset`; `image` is this device's with TRANSFER_DST and,
        // as documented, in UNDEFINED on first use or SHADER_READ_ONLY otherwise, and the copy
        // stays inside the chosen mip and layer.
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
    ///
    /// # Safety
    /// `cmd` is a command buffer of this device in the recording state, and
    /// `image` is an image of this device currently in layout `old`.
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
        // SAFETY: the caller guarantees `cmd` is recording and `image` is in `old`.
        unsafe {
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
    }

    pub fn sampler(
        &self,
        filter: vk::Filter,
        address: vk::SamplerAddressMode,
        anisotropy: bool,
        compare: Option<vk::CompareOp>,
    ) -> Result<vk::Sampler, GpuError> {
        // `new` enables sampler anisotropy only where the device has it.
        // SAFETY: `physical` belongs to this live instance.
        let features = unsafe { self.instance.get_physical_device_features(self.physical) };
        let anisotropy = anisotropy && features.sampler_anisotropy == vk::TRUE;
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
            .max_anisotropy(if anisotropy {
                self.limits.max_sampler_anisotropy.min(8.0)
            } else {
                1.0
            })
            .compare_enable(compare.is_some())
            .compare_op(compare.unwrap_or(vk::CompareOp::ALWAYS));
        // SAFETY: the device is alive and `info` lives to the end of the call; anisotropy is on
        // only when the device enabled the feature, and at most its limit.
        Ok(unsafe { self.device.create_sampler(&info, None) }?)
    }

    pub fn shader(&self, spirv: &[u8]) -> Result<vk::ShaderModule, GpuError> {
        let words: Vec<u32> = spirv
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // SAFETY: the device is alive and the create info borrows `words`, which outlives the
        // call; the SPIR-V comes from the build script's compiled shaders.
        Ok(unsafe {
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
        }?)
    }

    pub fn wait_idle(&self) {
        // SAFETY: the device is alive; waiting for it to go idle has no other requirement.
        unsafe { self.device.device_wait_idle().ok() };
    }
}

/// Rustup's macOS shell wrappers can strip DYLD_* variables. Try the normal
/// loader first, then explicit SDK/Homebrew paths so Cargo runs work as well.
/// Runs an enumeration again while it fails with VK_INCOMPLETE: ash retries
/// that answer to the fill call, not to the count query, and some layers give
/// it there too.
fn retry_incomplete<T>(
    mut query: impl FnMut() -> ash::prelude::VkResult<T>,
) -> ash::prelude::VkResult<T> {
    for _ in 0..8 {
        match query() {
            Err(vk::Result::INCOMPLETE) => std::thread::sleep(std::time::Duration::from_millis(5)),
            done => return done,
        }
    }
    query()
}

fn load_entry() -> Result<ash::Entry, GpuError> {
    // SAFETY: loading the Vulkan loader runs its initialisers; the caller retains
    // the entry for the lifetime of every Vulkan object created through it.
    let loaded = unsafe { ash::Entry::load() };
    #[cfg(target_os = "macos")]
    if loaded.is_err() {
        let sdk = std::env::var_os("VULKAN_SDK")
            .map(|root| std::path::PathBuf::from(root).join("lib/libvulkan.dylib"));
        for path in sdk.into_iter().chain([
            std::path::PathBuf::from("/opt/homebrew/lib/libvulkan.dylib"),
            std::path::PathBuf::from("/usr/local/lib/libvulkan.dylib"),
        ]) {
            // SAFETY: these are Vulkan loader paths, with the same lifetime
            // requirement as Entry::load above.
            if let Ok(entry) = unsafe { ash::Entry::load_from(path) } {
                return Ok(entry);
            }
        }
    }
    loaded.map_err(|e| {
        let hint = if cfg!(target_os = "macos") {
            "; install the runtime with: brew install vulkan-loader molten-vk"
        } else {
            ""
        };
        GpuError::Load(format!("{e}{hint}"))
    })
}

impl Drop for Gpu {
    fn drop(&mut self) {
        let leaked = *self.live_allocations.get_mut();
        if leaked != 0 {
            log::error!("{leaked} GPU memory allocations were never freed: a buffer or image lost its owner");
            debug_assert!(
                std::thread::panicking(),
                "{leaked} GPU memory allocations leaked"
            );
        }
        // SAFETY: `Gpu` is dropped last by its owner, after every resource made from it has
        // been destroyed; the device is idled first, then the pool, device and instance go
        // once, child before parent.
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

// SAFETY: the handles are plain Vulkan handles, usable from any thread, and the
// mapping stays valid for every thread until the memory is freed. `Buffer` stays
// `!Sync` (the raw pointer), so only the one thread that owns it can `write`/`read`.
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
            offset
                .checked_add(data.len() as u64)
                .is_some_and(|end| end <= self.size),
            "buffer overflow: {} + {} > {}",
            offset,
            data.len(),
            self.size
        );
        // SAFETY: `mapped` is non-null (host-visible memory, mapped whole) and the assert keeps
        // `offset..offset + data.len()` inside `size`, which is at most the mapped allocation;
        // `data` is ordinary host memory, so the ranges do not overlap. `size` is only set at
        // creation.
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.mapped.add(offset as usize),
                data.len(),
            )
        };
    }

    pub fn read(&self, offset: u64, out: &mut [u8]) {
        assert!(
            !self.mapped.is_null()
                && offset
                    .checked_add(out.len() as u64)
                    .is_some_and(|end| end <= self.size)
        );
        // SAFETY: `mapped` is non-null and the assert keeps `offset..offset + out.len()` inside
        // `size`, which is at most the mapped allocation; `out` is ordinary host memory, so the
        // ranges do not overlap.
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
