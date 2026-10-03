//! The terrain shaded once per pixel (terrain.wgsl `fs_lit`): a full-screen pass
//! between GTAO and the scene pass, from the depth pre-pass's depth, into an HDR
//! image the scene pass's terrain then reads (`fs_over_lit`).
//!
//! Drawn forward, the terrain's triangles are a pixel or two across from a high
//! camera, and the GPU shades a 2x2 block of pixels for every triangle that
//! touches it, so its long fragment shader ran several times a pixel. Shaded from
//! the depth, it runs once. What the pre-pass put in front of the terrain is shaded
//! as ground too and thrown away (the scene pass's depth test keeps only terrain).
//!
//! Set 2 of `layouts.water`: 0 the lit image, 1 the scene's depth.

use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::gpu_consts::pass;
use crate::pipelines::HDR_FORMAT;
use ash::vk;

/// What any pass needs to draw the terrain's nodes (terrain.wgsl `vs_main`), while
/// `cmd` records.
pub(super) struct TerrainDraw<'a> {
    pub(super) gpu: &'a Gpu,
    pub(super) cmd: vk::CommandBuffer,
    /// Set 1: the nodes this frame draws.
    pub(super) nodes_set: vk::DescriptorSet,
    pub(super) grid_vb: &'a Buffer,
    pub(super) grid_ib: &'a Buffer,
    pub(super) index_count: u32,
    pub(super) node_count: u32,
    pub(super) build_grid: bool,
}

impl TerrainDraw<'_> {
    /// Its push constants (terrain.wgsl `TerrainPush`).
    fn push(&self, layout: vk::PipelineLayout, pass_kind: u32) {
        // SAFETY: `cmd` is recording; the 8 bytes fit every scene layout's 16-byte
        // vertex+fragment push range.
        unsafe {
            self.gpu.device.cmd_push_constants(
                self.cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&[pass_kind, self.build_grid as u32]),
            )
        };
    }

    /// The nodes with `pipeline` (made with `layout`), inside a render pass with
    /// set 0 bound. `lit` is set 2 (`fs_over_lit`'s), for `layouts.water`.
    pub(super) fn draw(
        &self,
        pipeline: vk::Pipeline,
        layout: vk::PipelineLayout,
        pass_kind: u32,
        lit: Option<vk::DescriptorSet>,
    ) {
        let (dev, cmd) = (&self.gpu.device, self.cmd);
        let sets = [self.nodes_set, lit.unwrap_or_default()];
        let sets = &sets[..1 + lit.is_some() as usize];
        // SAFETY: called while `cmd` records inside a render pass, after set 0 is bound;
        // `layout` has a set 1 (and a set 2 where `lit` is given) of the kinds bound.
        unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                sets,
                &[],
            );
        }
        self.push(layout, pass_kind);
        // SAFETY: inside the same render pass; the grid buffers are live, made with
        // vertex/index usage, and `index_count` indices fit `grid_ib`.
        unsafe {
            dev.cmd_bind_vertex_buffers(cmd, 0, &[self.grid_vb.buffer], &[0]);
            dev.cmd_bind_index_buffer(cmd, self.grid_ib.buffer, 0, vk::IndexType::UINT32);
            dev.cmd_draw_indexed(cmd, self.index_count, self.node_count, 0, 0, 0);
        }
    }
}

impl super::Renderer {
    /// After GTAO, outside any render pass: the terrain lit from the pre-pass's
    /// depth (without a pre-pass there is no depth to light it from).
    pub(super) fn record_terrain_lit(&self, terrain: &TerrainDraw) {
        if !self.prepass {
            return;
        }
        self.timers
            .scope(&self.gpu.device, terrain.cmd, "terrain.lit");
        self.terrain_lit.record(
            terrain,
            self.passes.bloom_down,
            (self.layouts.water, self.pipelines.terrain_lit),
            self.scene_set,
        );
        self.timers.end(&self.gpu.device, terrain.cmd);
    }

    /// The scene pass's terrain: the lit image where there is one, else shaded here.
    pub(super) fn draw_scene_terrain(&self, terrain: &TerrainDraw) {
        if self.prepass {
            let lit = Some(self.terrain_lit.set);
            terrain.draw(
                self.pipelines.terrain_over_lit,
                self.layouts.water,
                pass::MAIN,
                lit,
            );
        } else {
            terrain.draw(self.pipelines.terrain, self.layouts.scene, pass::MAIN, None);
        }
    }
}

pub(super) struct TerrainLit {
    pool: vk::DescriptorPool,
    pub(super) set: vk::DescriptorSet,
    image: Option<Image>,
    framebuffer: vk::Framebuffer,
}

impl TerrainLit {
    pub(super) fn new(gpu: &Gpu, set_layout: vk::DescriptorSetLayout) -> Result<Self, GpuError> {
        let sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLED_IMAGE,
            descriptor_count: 2,
        }];
        // SAFETY: the device is alive and `sizes` lives to the end of the call.
        let pool = unsafe {
            gpu.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [set_layout];
        // SAFETY: the pool was made just above with room for this one set; only bindings 0
        // and 1 of the screen set are ever written (both sampled images), and `layouts`
        // lives to the end of the call.
        let set = unsafe {
            gpu.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }?[0];
        Ok(TerrainLit {
            pool,
            set,
            image: None,
            framebuffer: vk::Framebuffer::null(),
        })
    }

    /// At the scene's size; `depth` is the scene's depth, `pass` the bloom pass
    /// (one HDR attachment, its contents discarded, read by shaders after).
    pub(super) fn resize(
        &mut self,
        gpu: &Gpu,
        scene: (u32, u32),
        depth: vk::ImageView,
        pass: vk::RenderPass,
    ) -> Result<(), GpuError> {
        self.release(gpu);
        let image = gpu.image(&ImageDesc {
            width: scene.0,
            height: scene.1,
            format: HDR_FORMAT,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        let views = [image.view];
        // SAFETY: `image` is a live HDR view at the scene size, matching the bloom pass's one
        // colour attachment; the create info lives to the end of the call.
        let framebuffer = unsafe {
            gpu.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(pass)
                    .attachments(&views)
                    .width(scene.0)
                    .height(scene.1)
                    .layers(1),
                None,
            )
        };
        let framebuffer = match framebuffer {
            Ok(fb) => fb,
            Err(e) => {
                gpu.destroy_image(image);
                return Err(e.into());
            }
        };
        let lit = [vk::DescriptorImageInfo::default()
            .image_view(image.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let depth = [vk::DescriptorImageInfo::default()
            .image_view(depth)
            .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&lit),
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(1)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&depth),
        ];
        // SAFETY: `resize` runs only from the renderer's `create_size_dependent`, after the
        // device went idle, so no command buffer in flight uses the set; both views are live
        // and the infos live to the end of the call.
        unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
        self.image = Some(image);
        self.framebuffer = framebuffer;
        Ok(())
    }

    /// Outside any render pass, after GTAO and before the scene pass. `pipeline` is
    /// `terrain_lit`, made with `layout` (`layouts.water`); `scene_set` is set 0.
    fn record(
        &self,
        terrain: &TerrainDraw,
        render_pass: vk::RenderPass,
        (layout, pipeline): (vk::PipelineLayout, vk::Pipeline),
        scene_set: vk::DescriptorSet,
    ) {
        let (gpu, cmd) = (terrain.gpu, terrain.cmd);
        let Some(image) = &self.image else {
            return;
        };
        let dev = &gpu.device;
        let extent = vk::Extent2D {
            width: image.width,
            height: image.height,
        };
        terrain.push(layout, pass::MAIN);
        // SAFETY: `cmd` is recording and outside a render pass; the framebuffer was made for
        // `render_pass` at the image's size, the render area; `pipeline` was made with `layout`
        // for it, set 0 of `layout` is the scene set, 1 the pass set and 2 the screen set
        // this owner's set was allocated from.
        unsafe {
            dev.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(render_pass)
                    .framebuffer(self.framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent,
                    }),
                vk::SubpassContents::INLINE,
            );
            dev.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: extent.width as f32,
                    height: extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            dev.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent,
                }],
            );
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &[scene_set, terrain.nodes_set, self.set],
                &[],
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
            dev.cmd_end_render_pass(cmd);
        }
    }

    fn release(&mut self, gpu: &Gpu) {
        if self.framebuffer != vk::Framebuffer::null() {
            // SAFETY: called from `resize` (device idle) or `destroy` (renderer's `Drop`, device
            // idle), so no command buffer in flight uses the framebuffer; it is nulled at once.
            unsafe {
                gpu.device
                    .destroy_framebuffer(std::mem::take(&mut self.framebuffer), None)
            };
        }
        if let Some(image) = self.image.take() {
            gpu.destroy_image(image);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        self.release(gpu);
        // SAFETY: the pool was made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle.
        unsafe { gpu.device.destroy_descriptor_pool(self.pool, None) };
    }
}
