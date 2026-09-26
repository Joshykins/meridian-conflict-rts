//! The first picture of a run. Building a renderer takes seconds; this is a
//! bare presenter that is on the window within a moment of it opening and
//! draws the loading screen's overlay while the renderer is built on another
//! thread. Its own device, the overlay pipeline and nothing else: no scene,
//! no glass (glass panels show black). `release_window` hands the window on.

use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::overlay::{Overlay, MAX_OVERLAY_VERTICES};
use crate::pipelines::{self, Blend, Depth, PipelineDesc, VertexKind};
use crate::renderer::Target;
use crate::{swapchain, textures};
use ash::vk;

pub struct Splash {
    gpu: Gpu,
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    views: Vec<vk::ImageView>,
    framebuffers: Vec<vk::Framebuffer>,
    format: vk::Format,
    vsync: bool,
    width: u32,
    height: u32,
    pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    descriptor_pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    pipeline: vk::Pipeline,
    atlas: Image,
    atlas_uploaded: bool,
    /// Stands in for the blurred scene glass would show.
    blank: Image,
    sampler: vk::Sampler,
    vertices: Buffer,
    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
}

impl Splash {
    /// Opens a device on `target` (a window) and takes the window.
    pub fn new(target: Target) -> Result<Splash, GpuError> {
        let Target::Window { display, window, width, height, vsync } = target else {
            return Err(GpuError::NoDevice("the splash draws to a window".into()));
        };
        let extensions = ash_window::enumerate_required_extensions(display).map_err(GpuError::Vk)?;
        let gpu = Gpu::new(extensions)?;
        // SAFETY: the handles come from a live window that outlives this presenter
        // (the app drops the splash before the window).
        let surface = unsafe { ash_window::create_surface(&gpu.entry, &gpu.instance, display, window, None) }?;
        let format = swapchain::surface_format(&gpu, surface)?;
        let device = &gpu.device;

        let pass = {
            let attachments = [vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
            let color = [vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }];
            let subpasses = [vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color)];
            // The image is written only once the acquire has signalled.
            let dependencies = [vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies);
            // SAFETY: the create info borrows arrays that live to the end of the call.
            unsafe { device.create_render_pass(&info, None) }?
        };

        // The overlay's bindings in screen.wgsl: the scene (read by glass), the atlas, the sampler.
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let set_layout = pipelines::set_layout(
            &gpu,
            &[
                (0, vk::DescriptorType::SAMPLED_IMAGE),
                (1, vk::DescriptorType::SAMPLED_IMAGE),
                (2, vk::DescriptorType::SAMPLER),
            ],
            gfx,
        )?;
        let layout = {
            // The same push block as the renderer's screen layout.
            let push = [vk::PushConstantRange { stage_flags: gfx, offset: 0, size: 16 }];
            let sets = [set_layout];
            let info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&sets)
                .push_constant_ranges(&push);
            // SAFETY: as above.
            unsafe { device.create_pipeline_layout(&info, None) }?
        };
        let module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/screen.spv")))?;
        let pipeline = pipelines::graphics_pipeline(
            &gpu,
            &PipelineDesc {
                module,
                vs: c"vs_overlay",
                fs: c"fs_overlay",
                layout,
                pass,
                vertex: VertexKind::Overlay,
                blend: Blend::Alpha,
                depth: Depth::Off,
                cull: vk::CullModeFlags::NONE,
            },
        )?;

        let sampled = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        let atlas = gpu.image(&ImageDesc {
            width: textures::FONT_ATLAS_W as u32,
            height: textures::FONT_ATLAS_H as u32,
            format: vk::Format::R8G8B8A8_SRGB,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        let blank = gpu.image(&ImageDesc {
            width: 1,
            height: 1,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&blank, 0, 0, None, &[0, 0, 0, 255], true)?;
        let sampler = gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, None)?;

        let descriptor_pool = {
            let sizes = [
                vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 2 },
                vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 1 },
            ];
            let info = vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes);
            // SAFETY: as above.
            unsafe { device.create_descriptor_pool(&info, None) }?
        };
        let layouts = [set_layout];
        // SAFETY: the pool was sized for exactly this one set.
        let set = unsafe {
            device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )
        }?[0];
        let image_info = |view| {
            [vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view: view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }]
        };
        let (scene_info, atlas_info) = (image_info(blank.view), image_info(atlas.view));
        let sampler_info = [vk::DescriptorImageInfo { sampler, ..Default::default() }];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&scene_info),
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&atlas_info),
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(2)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&sampler_info),
        ];
        // SAFETY: every write names a live set, view and sampler of this device.
        unsafe { device.update_descriptor_sets(&writes, &[]) };

        let vertices = gpu.host_buffer(
            (MAX_OVERLAY_VERTICES * size_of::<crate::overlay::OverlayVertex>()) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;
        // SAFETY: plain object creation on a live device and command pool.
        let (cmd, fence, image_available, render_finished) = unsafe {
            let cmd = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(gpu.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            let fence = device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )?;
            let semaphore = || device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None);
            (cmd, fence, semaphore()?, semaphore()?)
        };

        let mut splash = Splash {
            gpu,
            surface,
            swapchain: vk::SwapchainKHR::null(),
            views: Vec::new(),
            framebuffers: Vec::new(),
            format,
            vsync,
            width: width.max(1),
            height: height.max(1),
            pass,
            set_layout,
            layout,
            descriptor_pool,
            set,
            module,
            pipeline,
            atlas,
            atlas_uploaded: false,
            blank,
            sampler,
            vertices,
            cmd,
            fence,
            image_available,
            render_finished,
        };
        splash.remake_chain()?;
        Ok(splash)
    }

    /// Follows the window to a new size.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        if (width, height) == (self.width, self.height) || width == 0 || height == 0 {
            return Ok(());
        }
        (self.width, self.height) = (width, height);
        self.remake_chain()
    }

    /// (Re)makes the swapchain and its framebuffers at the current size.
    fn remake_chain(&mut self) -> Result<(), GpuError> {
        self.gpu.wait_idle();
        self.destroy_framebuffers();
        (self.width, self.height) = swapchain::rebuild(
            &self.gpu,
            self.surface,
            self.format,
            self.vsync,
            (self.width, self.height),
            &mut self.swapchain,
            &mut self.views,
        )?;
        for &view in &self.views {
            let attachments = [view];
            let info = vk::FramebufferCreateInfo::default()
                .render_pass(self.pass)
                .attachments(&attachments)
                .width(self.width)
                .height(self.height)
                .layers(1);
            // SAFETY: the view and pass are this device's and match in format.
            self.framebuffers.push(unsafe { self.gpu.device.create_framebuffer(&info, None) }?);
        }
        Ok(())
    }

    fn destroy_framebuffers(&mut self) {
        for fb in self.framebuffers.drain(..) {
            // SAFETY: the callers waited for the device to go idle first.
            unsafe { self.gpu.device.destroy_framebuffer(fb, None) };
        }
    }

    /// Draws `overlay` over black and presents it.
    pub fn render(&mut self, overlay: &Overlay) -> Result<(), GpuError> {
        let device = self.gpu.device.clone();
        // SAFETY: the fence is this device's; waiting on it has no other requirement.
        unsafe { device.wait_for_fences(&[self.fence], true, u64::MAX) }?;
        let swapchain_fn = self.gpu.swapchain_fn.clone().expect("window target");
        // SAFETY: the chain and semaphore are live; the semaphore is unsignalled
        // because the last frame's submit waited on it.
        let image = match unsafe {
            swapchain_fn.acquire_next_image(self.swapchain, u64::MAX, self.image_available, vk::Fence::null())
        } {
            Ok((index, _)) => index as usize,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return self.remake_chain(),
            Err(e) => return Err(e.into()),
        };
        // SAFETY: the fence is not in use by a pending submit: it was just waited on.
        unsafe { device.reset_fences(&[self.fence]) }?;

        // The atlas: all of it the first time, afterwards the rows that changed.
        let rows = match overlay.take_dirty_rows() {
            _ if !self.atlas_uploaded => Some(0..textures::FONT_ATLAS_H),
            dirty => dirty,
        };
        if let Some(rows) = rows {
            let w = textures::FONT_ATLAS_W;
            let region = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: rows.start as i32 },
                extent: vk::Extent2D { width: w as u32, height: rows.len() as u32 },
            };
            let bytes = &overlay.atlas()[rows.start * w * 4..rows.end * w * 4];
            self.gpu.upload_image(&self.atlas, 0, 0, Some(region), bytes, !self.atlas_uploaded)?;
            self.atlas_uploaded = true;
        }
        let drawn = &overlay.vertices[..overlay.vertices.len().min(MAX_OVERLAY_VERTICES)];
        self.vertices.write(0, bytemuck::cast_slice(drawn));

        let cmd = self.cmd;
        // SAFETY: the command buffer is idle (its fence was waited on), and every
        // object it records is alive until the device is next waited on.
        unsafe {
            device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] } }];
            let extent = vk::Extent2D { width: self.width, height: self.height };
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.pass)
                .framebuffer(self.framebuffers[image])
                .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            if !drawn.is_empty() {
                device.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: self.width as f32,
                        height: self.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                );
                device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent }]);
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[self.set], &[]);
                let scale = [2.0 / self.width as f32, 2.0 / self.height as f32, 0.0, 0.0];
                device.cmd_push_constants(
                    cmd,
                    self.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&scale),
                );
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.vertices.buffer], &[0]);
                device.cmd_draw(cmd, drawn.len() as u32, 1, 0, 0);
            }
            device.cmd_end_render_pass(cmd);
            device.end_command_buffer(cmd)?;

            let cmds = [cmd];
            let wait = [self.image_available];
            let signal = [self.render_finished];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit = vk::SubmitInfo::default()
                .command_buffers(&cmds)
                .wait_semaphores(&wait)
                .wait_dst_stage_mask(&stages)
                .signal_semaphores(&signal);
            device.queue_submit(self.gpu.queue, &[submit], self.fence)?;

            let swapchains = [self.swapchain];
            let indices = [image as u32];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal)
                .swapchains(&swapchains)
                .image_indices(&indices);
            match swapchain_fn.queue_present(self.gpu.queue, &present) {
                Ok(false) => Ok(()),
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.remake_chain(),
                Err(e) => Err(e.into()),
            }
        }
    }

    /// Gives the window up so a renderer can `attach` to it. Draws nothing after.
    pub fn release_window(&mut self) {
        self.gpu.wait_idle();
        self.destroy_framebuffers();
        for view in self.views.drain(..) {
            // SAFETY: the device is idle, so nothing uses the view.
            unsafe { self.gpu.device.destroy_image_view(view, None) };
        }
        if let Some(f) = &self.gpu.swapchain_fn {
            // SAFETY: as above; the chain is not presented from again.
            unsafe { f.destroy_swapchain(self.swapchain, None) };
        }
        self.swapchain = vk::SwapchainKHR::null();
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        self.release_window();
        let gpu = &self.gpu;
        // SAFETY: `release_window` waited for the device to go idle; each object
        // is this device's and destroyed once. `gpu` itself goes after, with the
        // device and instance.
        unsafe {
            let device = &gpu.device;
            device.destroy_semaphore(self.image_available, None);
            device.destroy_semaphore(self.render_finished, None);
            device.destroy_fence(self.fence, None);
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_shader_module(self.module, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.set_layout, None);
            device.destroy_render_pass(self.pass, None);
            device.destroy_sampler(self.sampler, None);
            gpu.surface_fn.destroy_surface(self.surface, None);
        }
        gpu.destroy_image_ref(&self.atlas);
        gpu.destroy_image_ref(&self.blank);
        gpu.destroy_buffer(std::mem::replace(&mut self.vertices, Buffer::null()));
    }
}
