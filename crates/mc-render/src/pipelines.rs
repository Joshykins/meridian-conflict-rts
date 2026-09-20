//! Render passes, descriptor layouts and pipelines. Pure set-up code: nothing
//! here runs per frame.

use crate::gpu::{Gpu, GpuError};
use ash::vk;
use std::ffi::CStr;

pub const HDR_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
pub const SHADOW_SIZE: u32 = 2048;

macro_rules! spirv {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".spv"))
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VertexKind {
    None,
    Vec2,
    Mesh,
    Overlay,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    /// Depth-only pass.
    NoColor,
    Opaque,
    Alpha,
    /// Colour already multiplied by alpha: one pipeline covers (smoke) and adds light (sparks, alpha 0).
    Premultiplied,
    Additive,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Off,
    /// Reversed-Z scene depth.
    Test,
    TestWrite,
    /// Conventional depth with slope bias, for the shadow map.
    Shadow,
}

pub struct PipelineDesc<'a> {
    pub module: vk::ShaderModule,
    pub vs: &'a CStr,
    pub fs: &'a CStr,
    pub layout: vk::PipelineLayout,
    pub pass: vk::RenderPass,
    pub vertex: VertexKind,
    pub blend: Blend,
    pub depth: Depth,
    pub cull: vk::CullModeFlags,
}

pub struct Passes {
    pub shadow: vk::RenderPass,
    pub scene: vk::RenderPass,
    pub present: vk::RenderPass,
    /// One level of the bloom chain, previous contents discarded.
    pub bloom_down: vk::RenderPass,
    /// The same, keeping what is there: the way back up the chain adds onto it.
    pub bloom_up: vk::RenderPass,
}

pub struct Layouts {
    /// Set 0 of every scene pipeline; see shaders/bindings.wgsl.
    pub scene_set: vk::DescriptorSetLayout,
    /// Set 1: up to two storage buffers private to a pass.
    pub pass_set: vk::DescriptorSetLayout,
    pub screen_set: vk::DescriptorSetLayout,
    pub cull_set: vk::DescriptorSetLayout,
    pub scene: vk::PipelineLayout,
    pub screen: vk::PipelineLayout,
    pub cull: vk::PipelineLayout,
}

pub struct Pipelines {
    pub terrain: vk::Pipeline,
    pub terrain_shadow: vk::Pipeline,
    pub entity: vk::Pipeline,
    pub entity_shadow: vk::Pipeline,
    pub stain: vk::Pipeline,
    pub deposit: vk::Pipeline,
    pub pad: vk::Pipeline,
    pub water: vk::Pipeline,
    pub icon: vk::Pipeline,
    pub ring: vk::Pipeline,
    pub range: vk::Pipeline,
    pub bar: vk::Pipeline,
    pub projectile: vk::Pipeline,
    pub shot: vk::Pipeline,
    pub effect: vk::Pipeline,
    pub puff: vk::Pipeline,
    pub beam: vk::Pipeline,
    pub track: vk::Pipeline,
    pub bloom_down: vk::Pipeline,
    pub bloom_up: vk::Pipeline,
    pub tonemap: vk::Pipeline,
    pub overlay: vk::Pipeline,
    pub cull_clear: vk::Pipeline,
    pub cull_cull: vk::Pipeline,
    pub cull_prefix: vk::Pipeline,
    pub cull_scatter: vk::Pipeline,
    modules: Vec<vk::ShaderModule>,
}

fn attachment(
    format: vk::Format,
    load: vk::AttachmentLoadOp,
    final_layout: vk::ImageLayout,
) -> vk::AttachmentDescription {
    vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(load)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_layout)
}

impl Passes {
    pub fn new(
        gpu: &Gpu,
        present_format: vk::Format,
        present_layout: vk::ImageLayout,
    ) -> Result<Passes, GpuError> {
        let color_ref = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        // A colour target that a later pass samples: the write has to be finished
        // before that read, and whatever wrote or sampled it before has to be
        // finished before this pass touches it. The implicit dependencies promise neither.
        let sampled_after = [
            vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                dst_subpass: 0,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::FRAGMENT_SHADER,
                src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::SHADER_READ,
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
        let make_with = |attachments: &[vk::AttachmentDescription],
                         dependencies: &[vk::SubpassDependency]|
         -> Result<vk::RenderPass, GpuError> {
            let subpasses = [vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_ref)];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(attachments)
                .subpasses(&subpasses)
                .dependencies(dependencies);
            Ok(unsafe { gpu.device.create_render_pass(&info, None) }?)
        };
        let make = |attachments: &[vk::AttachmentDescription],
                    color: bool,
                    depth: Option<u32>|
         -> Result<vk::RenderPass, GpuError> {
            let depth_ref = vk::AttachmentReference {
                attachment: depth.unwrap_or(0),
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            };
            let mut subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);
            if color {
                subpass = subpass.color_attachments(&color_ref);
            }
            if depth.is_some() {
                subpass = subpass.depth_stencil_attachment(&depth_ref);
            }
            let subpasses = [subpass];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(attachments)
                .subpasses(&subpasses);
            Ok(unsafe { gpu.device.create_render_pass(&info, None) }?)
        };
        let shadow = make(
            &[attachment(
                DEPTH_FORMAT,
                vk::AttachmentLoadOp::CLEAR,
                vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            )],
            false,
            Some(0),
        )?;
        let scene = make(
            &[
                attachment(
                    HDR_FORMAT,
                    vk::AttachmentLoadOp::CLEAR,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ),
                attachment(
                    DEPTH_FORMAT,
                    vk::AttachmentLoadOp::CLEAR,
                    vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ),
            ],
            true,
            Some(1),
        )?;
        let present = make(
            &[attachment(
                present_format,
                vk::AttachmentLoadOp::DONT_CARE,
                present_layout,
            )],
            true,
            None,
        )?;
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let bloom_down = make_with(
            &[attachment(
                HDR_FORMAT,
                vk::AttachmentLoadOp::DONT_CARE,
                read,
            )],
            &sampled_after,
        )?;
        let bloom_up = make_with(
            &[attachment(HDR_FORMAT, vk::AttachmentLoadOp::LOAD, read).initial_layout(read)],
            &sampled_after,
        )?;
        Ok(Passes {
            shadow,
            scene,
            present,
            bloom_down,
            bloom_up,
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        unsafe {
            gpu.device.destroy_render_pass(self.shadow, None);
            gpu.device.destroy_render_pass(self.scene, None);
            gpu.device.destroy_render_pass(self.present, None);
            gpu.device.destroy_render_pass(self.bloom_down, None);
            gpu.device.destroy_render_pass(self.bloom_up, None);
        }
    }
}

fn set_layout(
    gpu: &Gpu,
    bindings: &[(u32, vk::DescriptorType)],
    stages: vk::ShaderStageFlags,
) -> Result<vk::DescriptorSetLayout, GpuError> {
    let bindings: Vec<_> = bindings
        .iter()
        .map(|&(binding, ty)| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(ty)
                .descriptor_count(1)
                .stage_flags(stages)
        })
        .collect();
    Ok(unsafe {
        gpu.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
            None,
        )
    }?)
}

impl Layouts {
    pub fn new(gpu: &Gpu) -> Result<Layouts, GpuError> {
        use vk::DescriptorType as T;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let scene_set = set_layout(
            gpu,
            &[
                (0, T::UNIFORM_BUFFER),
                (1, T::STORAGE_BUFFER),
                (2, T::STORAGE_BUFFER),
                (3, T::STORAGE_BUFFER),
                (4, T::STORAGE_BUFFER),
                (5, T::SAMPLED_IMAGE),
                (6, T::SAMPLED_IMAGE),
                (7, T::SAMPLED_IMAGE),
                (8, T::SAMPLED_IMAGE),
                (9, T::SAMPLED_IMAGE),
                (10, T::SAMPLED_IMAGE),
                (11, T::SAMPLED_IMAGE),
                (12, T::SAMPLER),
                (13, T::SAMPLER),
                (14, T::SAMPLER),
            ],
            gfx,
        )?;
        let pass_set = set_layout(gpu, &[(0, T::STORAGE_BUFFER), (1, T::STORAGE_BUFFER)], gfx)?;
        let screen_set = set_layout(
            gpu,
            &[
                (0, T::SAMPLED_IMAGE),
                (1, T::SAMPLED_IMAGE),
                (2, T::SAMPLER),
                (3, T::SAMPLED_IMAGE),
            ],
            gfx,
        )?;
        let cull_bindings: Vec<_> = (0..10)
            .map(|i| {
                (
                    i,
                    if i == 0 {
                        T::UNIFORM_BUFFER
                    } else {
                        T::STORAGE_BUFFER
                    },
                )
            })
            .collect();
        let cull_set = set_layout(gpu, &cull_bindings, vk::ShaderStageFlags::COMPUTE)?;

        let pipeline_layout = |sets: &[vk::DescriptorSetLayout],
                               stages: vk::ShaderStageFlags|
         -> Result<vk::PipelineLayout, GpuError> {
            let push = [vk::PushConstantRange {
                stage_flags: stages,
                offset: 0,
                size: 8,
            }];
            let info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(sets)
                .push_constant_ranges(&push);
            Ok(unsafe { gpu.device.create_pipeline_layout(&info, None) }?)
        };
        Ok(Layouts {
            scene: pipeline_layout(&[scene_set, pass_set], gfx)?,
            screen: pipeline_layout(&[screen_set], gfx)?,
            cull: pipeline_layout(&[cull_set], vk::ShaderStageFlags::COMPUTE)?,
            scene_set,
            pass_set,
            screen_set,
            cull_set,
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        unsafe {
            for l in [self.scene, self.screen, self.cull] {
                gpu.device.destroy_pipeline_layout(l, None);
            }
            for s in [
                self.scene_set,
                self.pass_set,
                self.screen_set,
                self.cull_set,
            ] {
                gpu.device.destroy_descriptor_set_layout(s, None);
            }
        }
    }
}

fn graphics_pipeline(gpu: &Gpu, d: &PipelineDesc) -> Result<vk::Pipeline, GpuError> {
    let mut stages = vec![vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::VERTEX)
        .module(d.module)
        .name(d.vs)];
    stages.push(
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(d.module)
            .name(d.fs),
    );

    let f = |location, format, offset| vk::VertexInputAttributeDescription {
        location,
        binding: 0,
        format,
        offset,
    };
    let (stride, attributes): (u32, Vec<vk::VertexInputAttributeDescription>) = match d.vertex {
        VertexKind::None => (0, vec![]),
        VertexKind::Vec2 => (8, vec![f(0, vk::Format::R32G32_SFLOAT, 0)]),
        VertexKind::Mesh => (
            44,
            vec![
                f(0, vk::Format::R32G32B32_SFLOAT, 0),
                f(1, vk::Format::R32G32B32_SFLOAT, 12),
                f(2, vk::Format::R32G32_SFLOAT, 24),
                f(3, vk::Format::R32_UINT, 32),
                f(4, vk::Format::R32_UINT, 36),
                f(5, vk::Format::R32_UINT, 40),
            ],
        ),
        VertexKind::Overlay => (
            32,
            vec![
                f(0, vk::Format::R32G32_SFLOAT, 0),
                f(1, vk::Format::R32G32_SFLOAT, 8),
                f(2, vk::Format::R32G32B32A32_SFLOAT, 16),
            ],
        ),
    };
    let bindings = [vk::VertexInputBindingDescription {
        binding: 0,
        stride,
        input_rate: vk::VertexInputRate::VERTEX,
    }];
    let mut vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    if d.vertex != VertexKind::None {
        vertex_input = vertex_input
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attributes);
    }
    let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(d.cull)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0)
        .depth_bias_enable(d.depth == Depth::Shadow)
        .depth_bias_constant_factor(1.5)
        .depth_bias_slope_factor(2.5);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(d.depth != Depth::Off)
        .depth_write_enable(matches!(d.depth, Depth::TestWrite | Depth::Shadow))
        .depth_compare_op(if d.depth == Depth::Shadow {
            vk::CompareOp::LESS_OR_EQUAL
        } else {
            vk::CompareOp::GREATER_OR_EQUAL
        });
    let blend_attachment = match d.blend {
        Blend::Opaque | Blend::NoColor => vk::PipelineColorBlendAttachmentState::default(),
        Blend::Alpha => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA),
        Blend::Premultiplied => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA),
        Blend::Additive => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .src_alpha_blend_factor(vk::BlendFactor::ZERO)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE),
    }
    .color_write_mask(vk::ColorComponentFlags::RGBA);
    let blend_attachments = [blend_attachment];
    let mut blend = vk::PipelineColorBlendStateCreateInfo::default();
    if d.blend != Blend::NoColor {
        blend = blend.attachments(&blend_attachments);
    }
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&assembly)
        .viewport_state(&viewport)
        .rasterization_state(&raster)
        .multisample_state(&multisample)
        .depth_stencil_state(&depth)
        .color_blend_state(&blend)
        .dynamic_state(&dynamic)
        .layout(d.layout)
        .render_pass(d.pass)];
    unsafe {
        gpu.device
            .create_graphics_pipelines(vk::PipelineCache::null(), &info, None)
    }
    .map(|p| p[0])
    .map_err(|(_, e)| GpuError::Vk(e))
}

fn compute_pipeline(
    gpu: &Gpu,
    module: vk::ShaderModule,
    entry: &CStr,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline, GpuError> {
    let stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(module)
        .name(entry);
    let info = [vk::ComputePipelineCreateInfo::default()
        .stage(stage)
        .layout(layout)];
    unsafe {
        gpu.device
            .create_compute_pipelines(vk::PipelineCache::null(), &info, None)
    }
    .map(|p| p[0])
    .map_err(|(_, e)| GpuError::Vk(e))
}

impl Pipelines {
    pub fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Pipelines, GpuError> {
        let terrain = gpu.shader(spirv!("terrain"))?;
        let entity = gpu.shader(spirv!("entity"))?;
        let ground = gpu.shader(spirv!("ground"))?;
        let icons = gpu.shader(spirv!("icons"))?;
        let ranges = gpu.shader(spirv!("ranges"))?;
        let sprites = gpu.shader(spirv!("sprites"))?;
        let puffs = gpu.shader(spirv!("puffs"))?;
        let beams = gpu.shader(spirv!("beams"))?;
        let screen = gpu.shader(spirv!("screen"))?;
        let cull = gpu.shader(spirv!("cull"))?;

        let none = vk::CullModeFlags::NONE;
        let back = vk::CullModeFlags::BACK;
        let scene = |module, vs, fs, vertex, blend, depth, cull| {
            graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    layout: layouts.scene,
                    pass: passes.scene,
                    vertex,
                    blend,
                    depth,
                    cull,
                },
            )
        };
        let shadow = |module, vs, fs, vertex| {
            graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs,
                    fs,
                    layout: layouts.scene,
                    pass: passes.shadow,
                    vertex,
                    blend: Blend::NoColor,
                    depth: Depth::Shadow,
                    cull: none,
                },
            )
        };
        let present = |vs, fs, vertex, blend| {
            graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: screen,
                    vs,
                    fs,
                    layout: layouts.screen,
                    pass: passes.present,
                    vertex,
                    blend,
                    depth: Depth::Off,
                    cull: none,
                },
            )
        };

        let bloom = |fs, pass, blend| {
            graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: screen,
                    vs: c"vs_fullscreen",
                    fs,
                    layout: layouts.screen,
                    pass,
                    vertex: VertexKind::None,
                    blend,
                    depth: Depth::Off,
                    cull: none,
                },
            )
        };

        Ok(Pipelines {
            terrain: scene(
                terrain,
                c"vs_main",
                c"fs_main",
                VertexKind::Vec2,
                Blend::Opaque,
                Depth::TestWrite,
                back,
            )?,
            terrain_shadow: shadow(terrain, c"vs_main", c"fs_shadow", VertexKind::Vec2)?,
            entity: scene(
                entity,
                c"vs_main",
                c"fs_main",
                VertexKind::Mesh,
                Blend::Opaque,
                Depth::TestWrite,
                back,
            )?,
            entity_shadow: shadow(entity, c"vs_main", c"fs_shadow", VertexKind::Mesh)?,
            stain: scene(
                ground,
                c"vs_stain",
                c"fs_stain",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            deposit: scene(
                ground,
                c"vs_deposit",
                c"fs_deposit",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            pad: scene(
                ground,
                c"vs_pad",
                c"fs_pad",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            water: scene(
                ground,
                c"vs_water",
                c"fs_water",
                VertexKind::None,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            icon: scene(
                icons,
                c"vs_icon",
                c"fs_icon",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Off,
                none,
            )?,
            ring: scene(
                icons,
                c"vs_ring",
                c"fs_ring",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            range: scene(
                ranges,
                c"vs_range",
                c"fs_range",
                VertexKind::None,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            bar: scene(
                icons,
                c"vs_bar",
                c"fs_bar",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Off,
                none,
            )?,
            projectile: scene(
                sprites,
                c"vs_projectile",
                c"fs_sprite",
                VertexKind::Vec2,
                Blend::Additive,
                Depth::Test,
                none,
            )?,
            shot: scene(
                sprites,
                c"vs_shot",
                c"fs_shot",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            effect: scene(
                sprites,
                c"vs_effect",
                c"fs_sprite",
                VertexKind::Vec2,
                Blend::Additive,
                Depth::Test,
                none,
            )?,
            puff: scene(
                puffs,
                c"vs_puff",
                c"fs_puff",
                VertexKind::Vec2,
                Blend::Premultiplied,
                Depth::Test,
                none,
            )?,
            beam: scene(
                beams,
                c"vs_beam",
                c"fs_beam",
                VertexKind::Vec2,
                Blend::Premultiplied,
                Depth::Test,
                none,
            )?,
            track: scene(
                ground,
                c"vs_track",
                c"fs_track",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            bloom_down: bloom(c"fs_bloom_down", passes.bloom_down, Blend::Opaque)?,
            bloom_up: bloom(c"fs_bloom_up", passes.bloom_up, Blend::Additive)?,
            tonemap: present(
                c"vs_fullscreen",
                c"fs_tonemap",
                VertexKind::None,
                Blend::Opaque,
            )?,
            overlay: present(
                c"vs_overlay",
                c"fs_overlay",
                VertexKind::Overlay,
                Blend::Alpha,
            )?,
            cull_clear: compute_pipeline(gpu, cull, c"cs_clear", layouts.cull)?,
            cull_cull: compute_pipeline(gpu, cull, c"cs_cull", layouts.cull)?,
            cull_prefix: compute_pipeline(gpu, cull, c"cs_prefix", layouts.cull)?,
            cull_scatter: compute_pipeline(gpu, cull, c"cs_scatter", layouts.cull)?,
            modules: vec![
                terrain, entity, ground, icons, ranges, sprites, puffs, beams, screen, cull,
            ],
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        unsafe {
            for p in [
                self.terrain,
                self.terrain_shadow,
                self.entity,
                self.entity_shadow,
                self.stain,
                self.deposit,
                self.pad,
                self.water,
                self.icon,
                self.ring,
                self.range,
                self.bar,
                self.projectile,
                self.shot,
                self.effect,
                self.puff,
                self.beam,
                self.track,
                self.bloom_down,
                self.bloom_up,
                self.tonemap,
                self.overlay,
                self.cull_clear,
                self.cull_cull,
                self.cull_prefix,
                self.cull_scatter,
            ] {
                gpu.device.destroy_pipeline(p, None);
            }
            for m in &self.modules {
                gpu.device.destroy_shader_module(*m, None);
            }
        }
    }
}
