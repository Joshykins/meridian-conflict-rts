//! Where the nuclear blasts' volumes are drawn (nuke.wgsl, nuke_fx.rs).
//!
//! The march runs at half the output's size into a target of its own, over the
//! whole view in one full-screen pass (each ray finds the blasts' boxes itself),
//! stopped by the scene's depth. The composite then lays it over the picture in
//! `scene_over` through a small tent, which also smooths the march's fixed
//! dither away. At half size the march can take enough steps to be smooth and
//! still costs a quarter of what a full-size one would, however much of the
//! view a blast fills.
//!
//! Set 1 of both passes: 0 the scene's depth, 1 the clouds' 3D billow noise
//! (sky.rs), 2 the march's target, 3 the clouds' march (how far to the cloud) and 4
//! the clouds' resolved picture (how much gets through), so what lies past the deck is
//! seen through it. One set per cloud history, as the clouds alternate theirs.

use crate::descriptors::{Binding, SetPool};
use crate::gpu::{Gpu, GpuError, Image, ImageDesc};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, HDR_FORMAT};
use ash::vk;

/// The volume's own set: five sampled images (binding 1 the sky's noise).
const BINDINGS: &[Binding] = &[
    (0, vk::DescriptorType::SAMPLED_IMAGE),
    (1, vk::DescriptorType::SAMPLED_IMAGE),
    (2, vk::DescriptorType::SAMPLED_IMAGE),
    (3, vk::DescriptorType::SAMPLED_IMAGE),
    (4, vk::DescriptorType::SAMPLED_IMAGE),
];

pub(super) struct NukeVolume {
    target: Option<(Image, vk::Framebuffer)>,
    size: (u32, u32),
    pass: vk::RenderPass,
    pool: vk::DescriptorPool,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    sets: [vk::DescriptorSet; 2],
    module: vk::ShaderModule,
    march: vk::Pipeline,
    composite: vk::Pipeline,
}

fn write_image(
    gpu: &Gpu,
    set: vk::DescriptorSet,
    binding: u32,
    view: vk::ImageView,
    layout: vk::ImageLayout,
) {
    let info = [vk::DescriptorImageInfo {
        sampler: vk::Sampler::null(),
        image_view: view,
        image_layout: layout,
    }];
    let write = [vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
        .image_info(&info)];
    // SAFETY: called only from `new` (fresh sets) and `resize` (which runs only from the
    // renderer's `create_size_dependent`, after the device went idle), so no command buffer in
    // flight uses `set`; `view` is a live view of this device in `layout`, and `write`/`info`
    // live to the end of the call.
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

impl NukeVolume {
    pub(super) fn new(
        gpu: &Gpu,
        layouts: &Layouts,
        passes: &Passes,
        noise: vk::ImageView,
    ) -> Result<NukeVolume, GpuError> {
        let dev = &gpu.device;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let set_layout = pipelines::set_layout(gpu, BINDINGS, gfx)?;
        let sets = [layouts.scene_set, set_layout];
        // SAFETY: the device is alive; both set layouts are this device's and `sets` lives to
        // the end of the call.
        let layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&sets),
                None,
            )
        }?;
        let mut pool = SetPool::new(gpu, &[(BINDINGS, 2)])?;
        let sets = [
            pool.alloc(gpu, set_layout, BINDINGS)?,
            pool.alloc(gpu, set_layout, BINDINGS)?,
        ];
        let pool = pool.into_raw();
        for set in sets {
            write_image(gpu, set, 1, noise, vk::ImageLayout::GENERAL);
        }
        let module = gpu.shader(crate::shader_reload::spirv!("nuke"))?;
        let graphics = |fs, pass, blend| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs: c"vs_nuke_full",
                    fs,
                    layout,
                    pass,
                    vertex: VertexKind::None,
                    blend,
                    depth: Depth::Off,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        let march = graphics(c"fs_nuke_march", passes.bloom_down, Blend::Opaque)?;
        let composite = graphics(
            c"fs_nuke_composite",
            passes.scene_over,
            Blend::Premultiplied,
        )?;
        Ok(NukeVolume {
            target: None,
            size: (0, 0),
            pass: passes.bloom_down,
            pool,
            set_layout,
            layout,
            sets,
            module,
            march,
            composite,
        })
    }

    /// Half the output each way; `depth` is the scene's, `clouds` the clouds' march
    /// target and two histories (`Sky::cloud_targets`).
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        width: u32,
        height: u32,
        depth: vk::ImageView,
        clouds: (vk::ImageView, [vk::ImageView; 2]),
    ) -> Result<(), GpuError> {
        self.release(gpu);
        let (w, h) = (width.div_ceil(2).max(1), height.div_ceil(2).max(1));
        let image = gpu.image(&ImageDesc {
            width: w,
            height: h,
            format: HDR_FORMAT,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        let views = [image.view];
        // SAFETY: `image` was just made at `w`x`h` in `HDR_FORMAT`, the format of `self.pass`'s
        // one colour attachment; the create info and `views` live to the end of the call.
        let fb = unsafe {
            gpu.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.pass)
                    .attachments(&views)
                    .width(w)
                    .height(h)
                    .layers(1),
                None,
            )
        }?;
        // SAFETY: `submit_once` hands a recording command buffer of this device, and `image`
        // was just made, one mip and layer, still in UNDEFINED.
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
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        for (k, set) in self.sets.iter().enumerate() {
            write_image(
                gpu,
                *set,
                0,
                depth,
                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            );
            write_image(gpu, *set, 2, image.view, read);
            write_image(gpu, *set, 3, clouds.0, read);
            write_image(gpu, *set, 4, clouds.1[k], read);
        }
        self.target = Some((image, fb));
        self.size = (w, h);
        Ok(())
    }

    /// The march, outside any render pass, after the scene's depth is final and the
    /// clouds have resolved into history `history` (`Sky::history_index`).
    pub(super) fn record_march(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        scene_set: vk::DescriptorSet,
        history: usize,
    ) {
        let Some((_, fb)) = &self.target else {
            return;
        };
        let (w, h) = self.size;
        let dev = &gpu.device;
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D {
                width: w,
                height: h,
            },
        };
        // SAFETY: the renderer calls this with its recording `cmd`, outside any render pass;
        // `fb` was made for `self.pass` at `self.size`, the render area; the pipeline was made
        // for that pass with `self.layout`, whose set 0 is `scene_set`'s layout; the pass is
        // ended in this block.
        unsafe {
            dev.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.pass)
                    .framebuffer(*fb)
                    .render_area(area),
                vk::SubpassContents::INLINE,
            );
            dev.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: w as f32,
                    height: h as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            dev.cmd_set_scissor(cmd, 0, &[area]);
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.march);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layout,
                0,
                &[scene_set, self.sets[history & 1]],
                &[],
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
            dev.cmd_end_render_pass(cmd);
        }
    }

    /// Inside `scene_over`, full-size viewport set. Leaves set 1 bound to this layout.
    pub(super) fn draw_composite(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        scene_set: vk::DescriptorSet,
    ) {
        if self.target.is_none() {
            return;
        }
        // SAFETY: the renderer calls this inside `scene_over` while `cmd` is recording, with
        // the viewport set; `composite` was made for that pass with `self.layout`.
        unsafe {
            let dev = &gpu.device;
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.composite);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layout,
                0,
                &[scene_set, self.sets[0]],
                &[],
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    fn release(&mut self, gpu: &Gpu) {
        if let Some((image, fb)) = self.target.take() {
            // SAFETY: `release` runs from `resize` (after the renderer's device-idle wait) or
            // `destroy`; `fb` was taken out of `target`, so it is destroyed once.
            unsafe { gpu.device.destroy_framebuffer(fb, None) };
            gpu.destroy_image(image);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.release(gpu);
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their layout and
        // module.
        unsafe {
            let dev = &gpu.device;
            dev.destroy_pipeline(self.march, None);
            dev.destroy_pipeline(self.composite, None);
            dev.destroy_shader_module(self.module, None);
            dev.destroy_pipeline_layout(self.layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}
