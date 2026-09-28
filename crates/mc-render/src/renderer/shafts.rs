//! Light shafts (shafts.wgsl): the view ray's walk through shadowed air at
//! half the scene's size, then its composite, subtracted from the picture in
//! `scene_over`. Set 1 of both: 0 the scene's depth, 1 the walk's target.

use crate::gpu::{Gpu, GpuError, Image, ImageDesc};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, HDR_FORMAT};
use ash::vk;

pub(super) struct Shafts {
    target: Option<(Image, vk::Framebuffer)>,
    pass: vk::RenderPass,
    pool: vk::DescriptorPool,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    march: vk::Pipeline,
    composite: vk::Pipeline,
    pub(super) enabled: bool,
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
    // SAFETY: called only from `resize`, which runs only from the renderer's
    // `create_size_dependent` after the device went idle, so no command buffer in flight uses
    // `set`; `view` is a live view of this device in `layout`, and `write`/`info` live to the
    // end of the call.
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

impl Shafts {
    pub(super) fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Shafts, GpuError> {
        let dev = &gpu.device;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let bindings: Vec<_> = (0..2)
            .map(|b| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(b)
                    .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                    .descriptor_count(1)
                    .stage_flags(gfx)
            })
            .collect();
        let set_layout =
            // SAFETY: the device is alive and the create info borrows `bindings`, which lives
            // to the end of the call.
            unsafe { dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None) }?;
        let sets = [layouts.scene_set, set_layout];
        // SAFETY: the device is alive; both set layouts are this device's and `sets` lives to
        // the end of the call.
        let layout = unsafe {
            dev.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&sets),
                None,
            )
        }?;
        let sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLED_IMAGE,
            descriptor_count: 2,
        }];
        // SAFETY: the device is alive and `sizes` lives to the end of the call.
        let pool = unsafe {
            dev.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let one = [set_layout];
        // SAFETY: the pool was made just above for exactly this one set of two sampled images,
        // and `one` lives to the end of the call.
        let set = unsafe {
            dev.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&one),
            )
        }?[0];
        let module = gpu.shader(crate::shader_reload::spirv!("shafts"))?;
        let graphics = |fs, pass, blend| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs: c"vs_shafts",
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
        let march = graphics(c"fs_shafts_march", passes.bloom_down, Blend::Opaque)?;
        let composite = graphics(c"fs_shafts_composite", passes.scene_over, Blend::Subtract)?;
        // Off unless asked for (MERIDIAN_SHAFTS=1): at the game's usual camera
        // angles the view crosses too little air for shafts to read, and under a
        // deck that varies across the map they mostly darken the far haze.
        let enabled = std::env::var("MERIDIAN_SHAFTS").is_ok_and(|v| v != "0");
        Ok(Shafts {
            target: None,
            pass: passes.bloom_down,
            pool,
            set_layout,
            layout,
            set,
            module,
            march,
            composite,
            enabled,
        })
    }

    /// Half the scene each way; `depth` is the scene's.
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        scene: (u32, u32),
        depth: vk::ImageView,
    ) -> Result<(), GpuError> {
        self.release(gpu);
        let (w, h) = (scene.0.div_ceil(2).max(1), scene.1.div_ceil(2).max(1));
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
        write_image(
            gpu,
            self.set,
            0,
            depth,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        );
        write_image(
            gpu,
            self.set,
            1,
            image.view,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        self.target = Some((image, fb));
        Ok(())
    }

    /// The walk, outside any render pass, once the scene's depth is final.
    pub(super) fn record_march(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        scene_set: vk::DescriptorSet,
    ) {
        let Some((image, fb)) = &self.target else {
            return;
        };
        if !self.enabled {
            return;
        }
        let dev = &gpu.device;
        let (w, h) = (image.width, image.height);
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D {
                width: w,
                height: h,
            },
        };
        // SAFETY: the renderer calls this with its recording `cmd`, outside any render pass;
        // `fb` was made for `self.pass` at the image's size, the render area; the pipeline was
        // made for that pass with `self.layout`, whose set 0 is `scene_set`'s layout; the pass
        // is ended in this block.
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
                &[scene_set, self.set],
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
        if self.target.is_none() || !self.enabled {
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
                &[scene_set, self.set],
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
