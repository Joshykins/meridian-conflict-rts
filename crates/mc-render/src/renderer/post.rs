//! What happens to the picture between the tone map and the swapchain: SMAA
//! 1x, and FSR 1 when the scene is drawn smaller than the output (post.wgsl).
//!
//! With neither, the tone map writes the swapchain straight. Otherwise it writes `ldr`, an sRGB-encoded UNORM image at the
//! size the anti-aliasing works at (the scene's when upscaling, the output's
//! otherwise), and then:
//!
//!   SMAA:           ldr -> edges -> weights -> (blend) -> present
//!   SMAA + FSR:     ldr -> edges -> weights -> blend into `aa` -> EASU -> RCAS -> present
//!   FSR alone:      ldr -> EASU -> RCAS -> present
//!
//! The pass that writes the swapchain (an _SRGB format) decodes to linear; the
//! UI overlay is drawn over it after, so text is never filtered.
//!
//! Set 0 of every post pipeline: 0 what the pass reads, 1 SMAA's area texture,
//! 2 its search texture, 3 SMAA's weights, 4 a linear and 5 a point sampler.

use crate::gpu::{Gpu, GpuError, Image, ImageDesc};
use crate::pipelines::{self, Blend, Depth, PipelineDesc, VertexKind};
use ash::vk;

/// Edge smoothing on the finished picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Antialiasing {
    Off,
    #[default]
    Smaa,
}

const LDR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const EDGES_FORMAT: vk::Format = vk::Format::R8G8_UNORM;
/// FSR's sharpening, in stops down from the strongest (0).
const RCAS_STOPS: f32 = 0.25;

struct Target {
    image: Image,
    fb: vk::Framebuffer,
}

#[derive(Default)]
struct Targets {
    ldr: Option<Target>,
    edges: Option<Target>,
    weights: Option<Target>,
    /// SMAA's result when FSR upscales it after.
    aa: Option<Target>,
    /// EASU's output, at the output's size.
    upscaled: Option<Target>,
}

pub(super) struct Post {
    ldr_pass: vk::RenderPass,
    edges_pass: vk::RenderPass,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pool: vk::DescriptorPool,
    edges_set: vk::DescriptorSet,
    weights_set: vk::DescriptorSet,
    blend_set: vk::DescriptorSet,
    easu_set: vk::DescriptorSet,
    rcas_set: vk::DescriptorSet,
    area: Image,
    search: Image,
    linear: vk::Sampler,
    point: vk::Sampler,
    module: vk::ShaderModule,
    screen_module: vk::ShaderModule,
    tonemap_ldr: vk::Pipeline,
    smaa_edges: vk::Pipeline,
    smaa_weights: vk::Pipeline,
    smaa_blend: vk::Pipeline,
    smaa_present: vk::Pipeline,
    easu: vk::Pipeline,
    rcas_present: vk::Pipeline,
    targets: Targets,
    smaa: bool,
    upscale: bool,
    /// The size SMAA works at, and the output's.
    aa_size: (u32, u32),
    out_size: (u32, u32),
}

fn color_pass(gpu: &Gpu, format: vk::Format, load: vk::AttachmentLoadOp) -> Result<vk::RenderPass, GpuError> {
    let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(load)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(read)];
    let color_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL }];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)];
    // Whatever read this image last frame is done before it is written, and the
    // write is done before the next pass samples it.
    let dependencies = [
        vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::FRAGMENT_SHADER,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_READ,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                | vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
        vk::SubpassDependency {
            src_subpass: 0,
            dst_subpass: vk::SUBPASS_EXTERNAL,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            dependency_flags: vk::DependencyFlags::empty(),
        },
    ];
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    // SAFETY: the device is alive; `info` and the attachment, subpass, reference and dependency
    // arrays it points to live to the end of the call, and the one colour reference is
    // attachment 0.
    Ok(unsafe { gpu.device.create_render_pass(&info, None) }?)
}

fn write_image(gpu: &Gpu, set: vk::DescriptorSet, binding: u32, view: vk::ImageView) {
    let info = [vk::DescriptorImageInfo {
        sampler: vk::Sampler::null(),
        image_view: view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }];
    let write = [vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
        .image_info(&info)];
    // SAFETY: called from `new` (fresh sets) and from `resize`/`release`, which run only from
    // the renderer's `create_size_dependent` after the device went idle or from its `Drop`, so
    // no command buffer in flight uses `set`; `view` is live and in SHADER_READ_ONLY, and
    // `write`/`info` live to the end of the call.
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

fn write_sampler(gpu: &Gpu, set: vk::DescriptorSet, binding: u32, sampler: vk::Sampler) {
    let info = [vk::DescriptorImageInfo {
        sampler,
        image_view: vk::ImageView::null(),
        image_layout: vk::ImageLayout::UNDEFINED,
    }];
    let write = [vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::SAMPLER)
        .image_info(&info)];
    // SAFETY: called only from `new`, on fresh sets no command buffer uses; `sampler` is live
    // and `write`/`info` live to the end of the call.
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

impl Post {
    /// `present` is the renderer's swapchain pass; `screen_layout` the tone
    /// mapper's pipeline layout (its set and push constants are reused as they are).
    pub(super) fn new(
        gpu: &Gpu,
        present: vk::RenderPass,
        screen_layout: vk::PipelineLayout,
    ) -> Result<Post, GpuError> {
        let dev = &gpu.device;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let ldr_pass = color_pass(gpu, LDR_FORMAT, vk::AttachmentLoadOp::DONT_CARE)?;
        // Edge detection discards where there is no edge: those pixels must read 0.
        let edges_pass = color_pass(gpu, EDGES_FORMAT, vk::AttachmentLoadOp::CLEAR)?;

        let bindings: Vec<_> = (0..6u32)
            .map(|b| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b)
                    .descriptor_type(if b < 4 { vk::DescriptorType::SAMPLED_IMAGE } else { vk::DescriptorType::SAMPLER })
                    .descriptor_count(1)
                    .stage_flags(gfx)
            })
            .collect();
        // SAFETY: the device is alive and the create info borrows `bindings`, which lives to
        // the end of the call.
        let set_layout = unsafe {
            dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None)
        }?;
        let push = [vk::PushConstantRange { stage_flags: gfx, offset: 0, size: 16 }];
        let set_layouts = [set_layout];
        // SAFETY: the device is alive; `set_layout` is this device's and `set_layouts`/`push`
        // live to the end of the call.
        let layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push),
                None,
            )
        }?;
        let sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 20 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 10 },
        ];
        // SAFETY: the device is alive and `sizes` lives to the end of the call.
        let pool = unsafe {
            dev.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(5).pool_sizes(&sizes), None)
        }?;
        let five = [set_layout; 5];
        // SAFETY: the pool was made just above with room for exactly these five sets (20
        // images, 10 samplers), and `five` lives to the end of the call.
        let sets = unsafe {
            dev.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&five))
        }?;

        let area = gpu.image(&ImageDesc {
            width: 160,
            height: 560,
            format: vk::Format::R8G8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&area, 0, 0, None, include_bytes!("../../assets/smaa/area.bin"), true)?;
        let search = gpu.image(&ImageDesc {
            width: 64,
            height: 16,
            format: vk::Format::R8_UNORM,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&search, 0, 0, None, include_bytes!("../../assets/smaa/search.bin"), true)?;
        let linear = gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, None)?;
        let point = gpu.sampler(vk::Filter::NEAREST, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, None)?;
        for &set in &sets {
            for b in 0..4 {
                write_image(gpu, set, b, area.view);
            }
            write_image(gpu, set, 1, area.view);
            write_image(gpu, set, 2, search.view);
            write_sampler(gpu, set, 4, linear);
            write_sampler(gpu, set, 5, point);
        }

        let module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/post.spv")))?;
        let screen_module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/screen.spv")))?;
        let pipeline = |module, vs, fs, layout, pass| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    layout,
                    pass,
                    vertex: VertexKind::None,
                    blend: Blend::Opaque,
                    depth: Depth::Off,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        let tonemap_ldr = pipeline(screen_module, c"vs_fullscreen", c"fs_tonemap_ldr", screen_layout, ldr_pass)?;
        let smaa_edges = pipeline(module, c"vs_post", c"fs_smaa_edges", layout, edges_pass)?;
        let smaa_weights = pipeline(module, c"vs_post", c"fs_smaa_weights", layout, ldr_pass)?;
        let smaa_blend = pipeline(module, c"vs_post", c"fs_smaa_blend", layout, ldr_pass)?;
        let smaa_present = pipeline(module, c"vs_post", c"fs_smaa_present", layout, present)?;
        let easu = pipeline(module, c"vs_post", c"fs_easu", layout, ldr_pass)?;
        let rcas_present = pipeline(module, c"vs_post", c"fs_rcas_present", layout, present)?;
        Ok(Post {
            ldr_pass,
            edges_pass,
            set_layout,
            layout,
            pool,
            edges_set: sets[0],
            weights_set: sets[1],
            blend_set: sets[2],
            easu_set: sets[3],
            rcas_set: sets[4],
            area,
            search,
            linear,
            point,
            module,
            screen_module,
            tonemap_ldr,
            smaa_edges,
            smaa_weights,
            smaa_blend,
            smaa_present,
            easu,
            rcas_present,
            targets: Targets::default(),
            smaa: false,
            upscale: false,
            aa_size: (0, 0),
            out_size: (0, 0),
        })
    }

    /// Whether the tone map writes `ldr` rather than the swapchain.
    pub(super) fn active(&self) -> bool {
        self.smaa || self.upscale
    }

    /// (Re)makes the targets. `scene` is the 3D scene's size, `output` the swapchain's.
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        antialiasing: Antialiasing,
        scene: (u32, u32),
        output: (u32, u32),
    ) -> Result<(), GpuError> {
        self.release(gpu);
        self.smaa = antialiasing == Antialiasing::Smaa;
        self.upscale = scene.0 < output.0 || scene.1 < output.1;
        self.out_size = output;
        self.aa_size = if self.upscale { scene } else { output };
        if !self.active() {
            return Ok(());
        }
        let target = |format, pass, (w, h): (u32, u32)| -> Result<Target, GpuError> {
            let image = gpu.image(&ImageDesc {
                width: w,
                height: h,
                format,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                layers: 1,
                mips: 1,
                array: false,
            })?;
            let views = [image.view];
            // SAFETY: `image` was just made at `w`x`h` in the format `pass` was made for
            // (`LDR_FORMAT` or `EDGES_FORMAT`, as each call pairs them); the create info and
            // `views` live to the end of the call.
            let fb = unsafe {
                gpu.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default().render_pass(pass).attachments(&views).width(w).height(h).layers(1),
                    None,
                )
            }?;
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
            Ok(Target { image, fb })
        };
        let aa = self.aa_size;
        let ldr = target(LDR_FORMAT, self.ldr_pass, aa)?;
        write_image(gpu, self.edges_set, 0, ldr.image.view);
        write_image(gpu, self.blend_set, 0, ldr.image.view);
        write_image(gpu, self.easu_set, 0, ldr.image.view);
        if self.smaa {
            let edges = target(EDGES_FORMAT, self.edges_pass, aa)?;
            let weights = target(LDR_FORMAT, self.ldr_pass, aa)?;
            write_image(gpu, self.weights_set, 0, edges.image.view);
            write_image(gpu, self.blend_set, 3, weights.image.view);
            if self.upscale {
                let smoothed = target(LDR_FORMAT, self.ldr_pass, aa)?;
                write_image(gpu, self.easu_set, 0, smoothed.image.view);
                self.targets.aa = Some(smoothed);
            }
            self.targets.edges = Some(edges);
            self.targets.weights = Some(weights);
        }
        if self.upscale {
            let upscaled = target(LDR_FORMAT, self.ldr_pass, output)?;
            write_image(gpu, self.rcas_set, 0, upscaled.image.view);
            self.targets.upscaled = Some(upscaled);
        }
        self.targets.ldr = Some(ldr);
        Ok(())
    }

    fn pass(&self, gpu: &Gpu, cmd: vk::CommandBuffer, render_pass: vk::RenderPass, target: &Target, draw: impl FnOnce()) {
        let (w, h) = (target.image.width, target.image.height);
        let area = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } };
        let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: [0.0; 4] } }];
        // SAFETY: `pass` is called only from `record`, with the renderer's recording `cmd`
        // outside any render pass; `target.fb` was made for `render_pass` at the image's size,
        // the render area, and the pass is ended here after `draw`.
        unsafe {
            let dev = &gpu.device;
            dev.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(render_pass)
                    .framebuffer(target.fb)
                    .render_area(area)
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            dev.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            dev.cmd_set_scissor(cmd, 0, &[area]);
            draw();
            dev.cmd_end_render_pass(cmd);
        }
    }

    fn post_draw(&self, gpu: &Gpu, cmd: vk::CommandBuffer, pipeline: vk::Pipeline, set: vk::DescriptorSet, push: [f32; 4]) {
        // SAFETY: called inside a pass begun by `pass`, or by `draw_present` inside the
        // swapchain pass, while `cmd` is recording; the pipelines were made with `self.layout`
        // for those passes, and the 16-byte push is its range.
        unsafe {
            let dev = &gpu.device;
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[set], &[]);
            dev.cmd_push_constants(
                cmd,
                self.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&push),
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    /// Everything before the swapchain pass, outside any render pass: the tone
    /// map into `ldr` (`screen_set` and `tonemap_push` as the swapchain tone map
    /// would use), then SMAA's first passes and the upscale.
    pub(super) fn record(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        screen_layout: vk::PipelineLayout,
        screen_set: vk::DescriptorSet,
        tonemap_push: [f32; 4],
    ) {
        let Some(ldr) = &self.targets.ldr else {
            return;
        };
        let dev = &gpu.device;
        // SAFETY: this runs inside the pass `pass` begins, while `cmd` is recording;
        // `tonemap_ldr` was made for `ldr_pass` with `screen_layout`, `screen_set` fits it, and
        // the 16-byte push is its range.
        self.pass(gpu, cmd, self.ldr_pass, ldr, || unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.tonemap_ldr);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, screen_layout, 0, &[screen_set], &[]);
            dev.cmd_push_constants(
                cmd,
                screen_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&tonemap_push),
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
        });
        if let (Some(edges), Some(weights)) = (&self.targets.edges, &self.targets.weights) {
            self.pass(gpu, cmd, self.edges_pass, edges, || {
                self.post_draw(gpu, cmd, self.smaa_edges, self.edges_set, [0.0; 4])
            });
            self.pass(gpu, cmd, self.ldr_pass, weights, || {
                self.post_draw(gpu, cmd, self.smaa_weights, self.weights_set, [0.0; 4])
            });
            if let Some(aa) = &self.targets.aa {
                self.pass(gpu, cmd, self.ldr_pass, aa, || {
                    self.post_draw(gpu, cmd, self.smaa_blend, self.blend_set, [0.0; 4])
                });
            }
        }
        if let Some(upscaled) = &self.targets.upscaled {
            let (w, h) = self.out_size;
            self.pass(gpu, cmd, self.ldr_pass, upscaled, || {
                self.post_draw(gpu, cmd, self.easu, self.easu_set, [w as f32, h as f32, 0.0, 0.0])
            });
        }
    }

    /// Inside the swapchain pass, viewport set to the output: the finished
    /// picture. The caller rebinds its own set before drawing the overlay.
    pub(super) fn draw_present(&self, gpu: &Gpu, cmd: vk::CommandBuffer) {
        if self.upscale {
            self.post_draw(gpu, cmd, self.rcas_present, self.rcas_set, [(-RCAS_STOPS).exp2(), 0.0, 0.0, 0.0]);
        } else if self.smaa {
            self.post_draw(gpu, cmd, self.smaa_present, self.blend_set, [0.0; 4]);
        }
    }

    fn release(&mut self, gpu: &Gpu) {
        let targets = std::mem::take(&mut self.targets);
        for t in [targets.ldr, targets.edges, targets.weights, targets.aa, targets.upscaled].into_iter().flatten() {
            // SAFETY: `release` runs from `resize` (after the renderer's device-idle wait) or
            // `destroy`; the targets were taken out of `self`, so each framebuffer is destroyed
            // once.
            unsafe { gpu.device.destroy_framebuffer(t.fb, None) };
            gpu.destroy_image(t.image);
        }
        // Descriptors that pointed at them fall back to a live image.
        for set in [self.edges_set, self.weights_set, self.blend_set, self.easu_set, self.rcas_set] {
            write_image(gpu, set, 0, self.area.view);
            write_image(gpu, set, 3, self.area.view);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.release(gpu);
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their layout and
        // modules.
        unsafe {
            let dev = &gpu.device;
            for p in [
                self.tonemap_ldr,
                self.smaa_edges,
                self.smaa_weights,
                self.smaa_blend,
                self.smaa_present,
                self.easu,
                self.rcas_present,
            ] {
                dev.destroy_pipeline(p, None);
            }
            dev.destroy_shader_module(self.module, None);
            dev.destroy_shader_module(self.screen_module, None);
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
            dev.destroy_render_pass(self.ldr_pass, None);
            dev.destroy_render_pass(self.edges_pass, None);
            dev.destroy_sampler(self.linear, None);
            dev.destroy_sampler(self.point, None);
        }
        gpu.destroy_image_ref(&self.area);
        gpu.destroy_image_ref(&self.search);
    }
}
