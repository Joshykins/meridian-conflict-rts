//! Render passes, descriptor layouts and pipelines. Pure set-up code: nothing
//! here runs per frame.

use crate::gpu::{Gpu, GpuError};
use crate::shader_reload::spirv;
use crate::warm;
use ash::vk;
use std::ffi::CStr;

pub const HDR_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
/// The cloud march (sky.rs): rgb and alpha as four halves in two words, then its depth.
pub const CLOUD_MARCH_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

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
    /// Takes the colour away from what is there (the light shafts' shadowed haze).
    Subtract,
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
    /// The scene again after the water's copy of it: colour kept, depth kept
    /// and read-only, so the water can sample it while testing against it.
    pub scene_over: vk::RenderPass,
    pub present: vk::RenderPass,
    /// One level of the bloom chain, previous contents discarded.
    pub bloom_down: vk::RenderPass,
    /// The same, keeping what is there: the way back up the chain adds onto it.
    pub bloom_up: vk::RenderPass,
    /// The cloud march: its light and see-through packed with the cloud's depth.
    pub cloud_march: vk::RenderPass,
}

pub struct Layouts {
    /// Set 0 of every scene pipeline; see shaders/bindings.wgsl.
    pub scene_set: vk::DescriptorSetLayout,
    /// Set 1: up to two storage buffers private to a pass.
    pub pass_set: vk::DescriptorSetLayout,
    pub screen_set: vk::DescriptorSetLayout,
    pub cull_set: vk::DescriptorSetLayout,
    pub scene: vk::PipelineLayout,
    /// The scene's two sets, then a screen set holding the water's copy of the scene and its depth.
    pub water: vk::PipelineLayout,
    pub screen: vk::PipelineLayout,
    pub cull: vk::PipelineLayout,
}

pub struct Pipelines {
    pub terrain: vk::Pipeline,
    /// The terrain shaded once per pixel from the pre-pass's depth, into an image
    /// of its own (renderer/terrain_lit.rs), and the scene pass's terrain reading it.
    pub terrain_lit: vk::Pipeline,
    pub terrain_over_lit: vk::Pipeline,
    pub terrain_shadow: vk::Pipeline,
    pub entity: vk::Pipeline,
    /// `entity` over the pre-pass's own depth: tested, not written, so a hidden
    /// fragment is dropped before `fs_main` runs although it can `discard`.
    pub entity_over_prepass: vk::Pipeline,
    pub entity_shadow: vk::Pipeline,
    /// The same four for trees and rocks, with their own vertex stage (entity.wgsl
    /// `vs_prop`): main, over the pre-pass, shadow, pre-pass.
    pub prop: [vk::Pipeline; 4],
    /// The depth pre-pass: the scene's depth before any of it is shaded.
    pub terrain_prepass: vk::Pipeline,
    pub entity_prepass: vk::Pipeline,
    pub hull_shield: vk::Pipeline,
    pub hull_shield_depth: vk::Pipeline,
    pub stain: vk::Pipeline,
    pub deposit: vk::Pipeline,
    /// Ore veins underground, glowing through the ground during the mine survey.
    pub vein: vk::Pipeline,
    pub pad: vk::Pipeline,
    pub water: vk::Pipeline,
    pub icon: vk::Pipeline,
    pub ring: vk::Pipeline,
    pub range: vk::Pipeline,
    pub shockwave: vk::Pipeline,
    /// The climate walls' curtain of light (curtain.wgsl).
    pub curtain: vk::Pipeline,
    pub bar: vk::Pipeline,
    pub projectile: vk::Pipeline,
    pub shot: vk::Pipeline,
    pub missile: vk::Pipeline,
    /// Strategic missiles' bodies, and their motors' plumes.
    pub nuke_missile: vk::Pipeline,
    pub nuke_plume: vk::Pipeline,
    pub effect: vk::Pipeline,
    pub puff: vk::Pipeline,
    pub beam: vk::Pipeline,
    pub shield: vk::Pipeline,
    pub track: vk::Pipeline,
    /// A giant's footprint, on its own ground-following grid.
    pub print: vk::Pipeline,
    pub bloom_down: vk::Pipeline,
    pub bloom_up: vk::Pipeline,
    /// The blurred scene behind overlay glass, drawn with the `bloom_down` pass.
    pub glass_source: vk::Pipeline,
    pub glass_blur: vk::Pipeline,
    /// What the water sees through itself: the opaque scene, copied before the water draws.
    pub refract_copy: vk::Pipeline,
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
            // SAFETY: the device is alive; `info` and the attachment, subpass, reference and
            // dependency arrays it points to live to the end of the call, and the one subpass's
            // colour reference is attachment 0, which every caller passes.
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
            let dependencies = [vk::SubpassDependency::default()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                )
                .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                .src_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies);
            // SAFETY: the device is alive; `info` and the arrays it points to live to the end
            // of the call, and the references index only the attachments passed in.
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
        // The depth comes from the pre-pass (drawn with `shadow`, then read by GTAO);
        // this pass tests against it and writes what the pre-pass left out.
        let scene = {
            let depth_read = vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL;
            let attachments = [
                attachment(
                    HDR_FORMAT,
                    vk::AttachmentLoadOp::CLEAR,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                ),
                attachment(DEPTH_FORMAT, vk::AttachmentLoadOp::LOAD, depth_read)
                    .initial_layout(depth_read),
            ];
            let depth_ref = vk::AttachmentReference {
                attachment: 1,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            };
            let subpasses = [vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_ref)
                .depth_stencil_attachment(&depth_ref)];
            let dependencies = [
                vk::SubpassDependency::default()
                    .src_subpass(vk::SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::COMPUTE_SHADER
                            | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    )
                    .dst_stage_mask(
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    )
                    .src_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                            | vk::AccessFlags::SHADER_WRITE
                            | vk::AccessFlags::SHADER_READ,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
                            | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::SHADER_READ,
                    ),
                vk::SubpassDependency::default()
                    .src_subpass(0)
                    .dst_subpass(vk::SUBPASS_EXTERNAL)
                    .src_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    )
                    .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                    .src_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .dst_access_mask(vk::AccessFlags::SHADER_READ),
            ];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies);
            // SAFETY: the device is alive; `info` and the arrays it points to live to the end
            // of the call, and the references index only `attachments`.
            unsafe { gpu.device.create_render_pass(&info, None) }?
        };
        // Continues the scene. Nothing drawn from here on writes depth.
        let scene_over = {
            let depth_read = vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL;
            let color_read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            let attachments = [
                attachment(HDR_FORMAT, vk::AttachmentLoadOp::LOAD, color_read)
                    .initial_layout(color_read),
                attachment(DEPTH_FORMAT, vk::AttachmentLoadOp::LOAD, depth_read)
                    .initial_layout(depth_read),
            ];
            let depth_ref = vk::AttachmentReference {
                attachment: 1,
                layout: depth_read,
            };
            let subpasses = [vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_ref)
                .depth_stencil_attachment(&depth_ref)];
            let dependencies = [
                vk::SubpassDependency::default()
                    .src_subpass(vk::SUBPASS_EXTERNAL)
                    .dst_subpass(0)
                    .src_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::FRAGMENT_SHADER
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    )
                    .dst_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::FRAGMENT_SHADER
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .src_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .dst_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_READ
                            | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::SHADER_READ,
                    ),
                vk::SubpassDependency::default()
                    .src_subpass(0)
                    .dst_subpass(vk::SUBPASS_EXTERNAL)
                    .src_stage_mask(
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                            | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    )
                    .dst_stage_mask(
                        vk::PipelineStageFlags::FRAGMENT_SHADER
                            | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    )
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(
                        vk::AccessFlags::SHADER_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    ),
            ];
            let info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies);
            // SAFETY: the device is alive; `info` and the arrays it points to live to the end
            // of the call, and the references index only `attachments`.
            unsafe { gpu.device.create_render_pass(&info, None) }?
        };
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
        let cloud_march = make_with(
            &[attachment(
                CLOUD_MARCH_FORMAT,
                vk::AttachmentLoadOp::DONT_CARE,
                read,
            )],
            &sampled_after,
        )?;
        Ok(Passes {
            shadow,
            scene,
            scene_over,
            present,
            bloom_down,
            bloom_up,
            cloud_march,
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        // SAFETY: the passes were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, so no framebuffer or command buffer
        // still in flight uses them.
        unsafe {
            gpu.device.destroy_render_pass(self.shadow, None);
            gpu.device.destroy_render_pass(self.scene, None);
            gpu.device.destroy_render_pass(self.scene_over, None);
            gpu.device.destroy_render_pass(self.present, None);
            gpu.device.destroy_render_pass(self.bloom_down, None);
            gpu.device.destroy_render_pass(self.bloom_up, None);
            gpu.device.destroy_render_pass(self.cloud_march, None);
        }
    }
}

pub(crate) fn set_layout(
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
    // SAFETY: the device is alive and the create info borrows `bindings`, which lives to the
    // end of the call.
    Ok(unsafe {
        gpu.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
            None,
        )
    }?)
}

/// The scene set (group 0): every `//!use bindings` shader declares exactly these
/// (`scene_set_matches_the_shaders`, generated by build.rs, checks it).
pub(crate) const SCENE_SET: &[(u32, vk::DescriptorType)] = &[
    (0, vk::DescriptorType::UNIFORM_BUFFER),
    (1, vk::DescriptorType::STORAGE_BUFFER),
    (2, vk::DescriptorType::STORAGE_BUFFER),
    (3, vk::DescriptorType::STORAGE_BUFFER),
    (4, vk::DescriptorType::STORAGE_BUFFER),
    (5, vk::DescriptorType::SAMPLED_IMAGE),
    (6, vk::DescriptorType::SAMPLED_IMAGE),
    (7, vk::DescriptorType::SAMPLED_IMAGE),
    (8, vk::DescriptorType::SAMPLED_IMAGE),
    (9, vk::DescriptorType::SAMPLED_IMAGE),
    // retired: 10 (the tiled plate map)
    (11, vk::DescriptorType::SAMPLED_IMAGE),
    (12, vk::DescriptorType::SAMPLER),
    (13, vk::DescriptorType::SAMPLER),
    (14, vk::DescriptorType::SAMPLER),
    (15, vk::DescriptorType::STORAGE_BUFFER),
    (16, vk::DescriptorType::SAMPLED_IMAGE),
    (17, vk::DescriptorType::SAMPLED_IMAGE),
    (18, vk::DescriptorType::SAMPLED_IMAGE),
    (19, vk::DescriptorType::STORAGE_BUFFER),
    (20, vk::DescriptorType::SAMPLED_IMAGE),
    // The weather map and the atmosphere (sky.rs).
    (21, vk::DescriptorType::SAMPLED_IMAGE),
    (22, vk::DescriptorType::UNIFORM_BUFFER),
    (23, vk::DescriptorType::SAMPLED_IMAGE),
    (24, vk::DescriptorType::SAMPLED_IMAGE),
    // Local lights and their cluster grid (lights.rs).
    (25, vk::DescriptorType::STORAGE_BUFFER),
    (26, vk::DescriptorType::STORAGE_BUFFER),
    // The clouds' shade (sky.rs).
    (27, vk::DescriptorType::SAMPLED_IMAGE),
    // Gun houses' poses (`mirror::HousePose`).
    (28, vk::DescriptorType::STORAGE_BUFFER),
    // Craters in the ground (renderer/craters.rs).
    (29, vk::DescriptorType::STORAGE_BUFFER),
    // Screen-space ambient occlusion (renderer/gtao.rs).
    (30, vk::DescriptorType::SAMPLED_IMAGE),
    // Each standing tree's push from the blasts, from the cull (renderer/cull_lists.rs).
    (31, vk::DescriptorType::STORAGE_BUFFER),
    // Heat in the ground (renderer/ground_melt.rs).
    (32, vk::DescriptorType::STORAGE_BUFFER),
];

impl Layouts {
    pub fn new(gpu: &Gpu) -> Result<Layouts, GpuError> {
        use vk::DescriptorType as T;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let scene_set = set_layout(
            gpu,
            SCENE_SET,
            // Compute too: the clouds' shade is worked out from the scene set.
            gfx | vk::ShaderStageFlags::COMPUTE,
        )?;
        let pass_set = set_layout(gpu, &[(0, T::STORAGE_BUFFER), (1, T::STORAGE_BUFFER)], gfx)?;
        let screen_set = set_layout(
            gpu,
            &[
                (0, T::SAMPLED_IMAGE),
                (1, T::SAMPLED_IMAGE),
                (2, T::SAMPLER),
                (3, T::SAMPLED_IMAGE),
                (4, T::STORAGE_BUFFER),
                (5, T::UNIFORM_BUFFER),
                (6, T::STORAGE_BUFFER),
                (7, T::SAMPLED_IMAGE),
                (8, T::STORAGE_BUFFER),
                (9, T::STORAGE_BUFFER),
            ],
            gfx,
        )?;
        let cull_bindings: Vec<_> = (0..12)
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
                // 16: the tone map takes the live shockwaves' mask after its two values.
                size: 16,
            }];
            let info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(sets)
                .push_constant_ranges(&push);
            // SAFETY: the device is alive; `info` borrows `sets` and `push`, which live to the
            // end of the call, and the set layouts are this device's.
            Ok(unsafe { gpu.device.create_pipeline_layout(&info, None) }?)
        };
        Ok(Layouts {
            scene: pipeline_layout(&[scene_set, pass_set], gfx)?,
            water: pipeline_layout(&[scene_set, pass_set, screen_set], gfx)?,
            screen: pipeline_layout(&[screen_set], gfx)?,
            cull: pipeline_layout(&[cull_set], vk::ShaderStageFlags::COMPUTE)?,
            scene_set,
            pass_set,
            screen_set,
            cull_set,
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        // SAFETY: the layouts were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle and after the pipelines using them
        // are gone.
        unsafe {
            for l in [self.scene, self.water, self.screen, self.cull] {
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

pub(crate) fn graphics_pipeline(gpu: &Gpu, d: &PipelineDesc) -> Result<vk::Pipeline, GpuError> {
    let listed = warm::listed(|| warm::Wanted::Graphics {
        module: d.module,
        vs: d.vs.to_owned(),
        fs: d.fs.to_owned(),
        layout: d.layout,
        pass: d.pass,
        vertex: d.vertex,
        blend: d.blend,
        depth: d.depth,
        cull: d.cull,
    });
    if listed {
        return Ok(vk::Pipeline::null());
    }
    create_graphics(gpu, warm::cache(), d)
}

/// Compiles a graphics pipeline, finding it in `cache` if it is there.
pub(crate) fn create_graphics(
    gpu: &Gpu,
    cache: vk::PipelineCache,
    d: &PipelineDesc,
) -> Result<vk::Pipeline, GpuError> {
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
            64,
            vec![
                f(0, vk::Format::R32G32B32_SFLOAT, 0),
                f(1, vk::Format::R32G32B32_SFLOAT, 12),
                f(2, vk::Format::R32G32_SFLOAT, 24),
                f(3, vk::Format::R32_UINT, 32),
                f(4, vk::Format::R32_UINT, 36),
                f(5, vk::Format::R32_UINT, 40),
                f(6, vk::Format::R32G32B32A32_SFLOAT, 44),
                f(7, vk::Format::R32_UINT, 60),
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
        Blend::Subtract => vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE)
            .color_blend_op(vk::BlendOp::REVERSE_SUBTRACT)
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
    // SAFETY: the create info borrows state that lives to the end of the call;
    // the cache synchronises itself.
    unsafe { gpu.device.create_graphics_pipelines(cache, &info, None) }
        .map(|p| p[0])
        .map_err(|(_, e)| GpuError::Vk(e))
}

pub(crate) fn compute_pipeline(
    gpu: &Gpu,
    module: vk::ShaderModule,
    entry: &CStr,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline, GpuError> {
    let listed = warm::listed(|| warm::Wanted::Compute {
        module,
        entry: entry.to_owned(),
        layout,
    });
    if listed {
        return Ok(vk::Pipeline::null());
    }
    create_compute(gpu, warm::cache(), module, entry, layout)
}

/// Compiles a compute pipeline, finding it in `cache` if it is there.
pub(crate) fn create_compute(
    gpu: &Gpu,
    cache: vk::PipelineCache,
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
    // SAFETY: as in `create_graphics`.
    unsafe { gpu.device.create_compute_pipelines(cache, &info, None) }
        .map(|p| p[0])
        .map_err(|(_, e)| GpuError::Vk(e))
}

impl Pipelines {
    /// Every scene pipeline, compiled all at once (`warm`): after a shader
    /// change none is in the driver's cache, and one at a time took a minute.
    pub fn new(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Pipelines, GpuError> {
        warm::warmed(
            gpu,
            || Self::build(gpu, layouts, passes),
            |shell| shell.destroy(gpu),
        )
    }

    fn build(gpu: &Gpu, layouts: &Layouts, passes: &Passes) -> Result<Pipelines, GpuError> {
        let terrain = gpu.shader(spirv!("terrain"))?;
        let entity = gpu.shader(spirv!("entity"))?;
        let ground = gpu.shader(spirv!("ground"))?;
        let sea = gpu.shader(spirv!("water"))?;
        let icons = gpu.shader(spirv!("icons"))?;
        let ranges = gpu.shader(spirv!("ranges"))?;
        let shockwaves = gpu.shader(spirv!("shockwaves"))?;
        let curtain = gpu.shader(spirv!("curtain"))?;
        let sprites = gpu.shader(spirv!("sprites"))?;
        let puffs = gpu.shader(spirv!("puffs"))?;
        let beams = gpu.shader(spirv!("beams"))?;
        let shields = gpu.shader(spirv!("shields"))?;
        let screen = gpu.shader(spirv!("screen"))?;
        let cull = gpu.shader(spirv!("cull"))?;
        let nuke = gpu.shader(spirv!("nuke"))?;

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
        // The camera's depth with the scene's own test and culling (`shadow`'s pass is depth-only).
        let prepass = |module, vs, fs, vertex| {
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
                    depth: Depth::TestWrite,
                    cull: back,
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
            // Set 2 (`layouts.water`) holds the lit image and the depth it was lit from.
            terrain_lit: graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: terrain,
                    vs: c"vs_lit",
                    fs: c"fs_lit",
                    layout: layouts.water,
                    pass: passes.bloom_down,
                    vertex: VertexKind::None,
                    blend: Blend::Opaque,
                    depth: Depth::Off,
                    cull: none,
                },
            )?,
            terrain_over_lit: graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: terrain,
                    vs: c"vs_main",
                    fs: c"fs_over_lit",
                    layout: layouts.water,
                    pass: passes.scene,
                    vertex: VertexKind::Vec2,
                    blend: Blend::Opaque,
                    depth: Depth::TestWrite,
                    cull: back,
                },
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
            entity_over_prepass: scene(
                entity,
                c"vs_main",
                c"fs_main",
                VertexKind::Mesh,
                Blend::Opaque,
                Depth::Test,
                back,
            )?,
            entity_shadow: shadow(entity, c"vs_main", c"fs_shadow", VertexKind::Mesh)?,
            prop: [
                scene(
                    entity,
                    c"vs_prop",
                    c"fs_main",
                    VertexKind::Mesh,
                    Blend::Opaque,
                    Depth::TestWrite,
                    back,
                )?,
                scene(
                    entity,
                    c"vs_prop",
                    c"fs_main",
                    VertexKind::Mesh,
                    Blend::Opaque,
                    Depth::Test,
                    back,
                )?,
                shadow(entity, c"vs_prop", c"fs_shadow", VertexKind::Mesh)?,
                prepass(entity, c"vs_prop", c"fs_prepass", VertexKind::Mesh)?,
            ],
            terrain_prepass: prepass(terrain, c"vs_main", c"fs_shadow", VertexKind::Vec2)?,
            entity_prepass: prepass(entity, c"vs_main", c"fs_prepass", VertexKind::Mesh)?,
            // Set 2 carries the fields' own outermost depth (`hull_shield_depth`).
            hull_shield: graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: entity,
                    vs: c"vs_main",
                    fs: c"fs_hull",
                    layout: layouts.water,
                    pass: passes.scene,
                    vertex: VertexKind::Mesh,
                    blend: Blend::Premultiplied,
                    depth: Depth::Test,
                    cull: back,
                },
            )?,
            // Into a depth target of its own, the shape of the shadow pass's: the
            // scene's depth is read-only by the time the fields draw.
            hull_shield_depth: graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: entity,
                    vs: c"vs_main",
                    fs: c"fs_hull_depth",
                    layout: layouts.scene,
                    pass: passes.shadow,
                    vertex: VertexKind::Mesh,
                    blend: Blend::NoColor,
                    depth: Depth::TestWrite,
                    cull: back,
                },
            )?,
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
                c"vs_ore",
                c"fs_ore",
                VertexKind::Vec2,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            vein: scene(
                ground,
                c"vs_vein",
                c"fs_vein",
                VertexKind::Mesh,
                Blend::Additive,
                Depth::Off,
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
            water: graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: sea,
                    vs: c"vs_water",
                    fs: c"fs_water",
                    layout: layouts.water,
                    pass: passes.scene_over,
                    vertex: VertexKind::None,
                    blend: Blend::Alpha,
                    depth: Depth::Test,
                    cull: none,
                },
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
            shockwave: scene(
                shockwaves,
                c"vs_shockwave",
                c"fs_shockwave",
                VertexKind::None,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            curtain: scene(
                curtain,
                c"vs_curtain",
                c"fs_curtain",
                VertexKind::None,
                Blend::Additive,
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
            missile: scene(
                sprites,
                c"vs_missile",
                c"fs_missile",
                VertexKind::None,
                Blend::Opaque,
                Depth::TestWrite,
                none,
            )?,
            nuke_missile: scene(
                nuke,
                c"vs_strategic",
                c"fs_strategic",
                VertexKind::None,
                Blend::Opaque,
                Depth::TestWrite,
                none,
            )?,
            nuke_plume: scene(
                nuke,
                c"vs_plume",
                c"fs_plume",
                VertexKind::None,
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
            shield: scene(
                shields,
                c"vs_shield",
                c"fs_shield",
                VertexKind::None,
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
            print: scene(
                ground,
                c"vs_print",
                c"fs_print",
                VertexKind::None,
                Blend::Alpha,
                Depth::Test,
                none,
            )?,
            bloom_down: bloom(c"fs_bloom_down", passes.bloom_down, Blend::Opaque)?,
            bloom_up: bloom(c"fs_bloom_up", passes.bloom_up, Blend::Additive)?,
            glass_source: bloom(c"fs_glass_source", passes.bloom_down, Blend::Opaque)?,
            glass_blur: bloom(c"fs_glass_blur", passes.bloom_down, Blend::Opaque)?,
            refract_copy: bloom(c"fs_refract_copy", passes.bloom_down, Blend::Opaque)?,
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
                terrain, entity, ground, sea, icons, ranges, shockwaves, curtain, sprites, puffs,
                beams, shields, screen, cull, nuke,
            ],
        })
    }

    pub fn destroy(&self, gpu: &Gpu) {
        // SAFETY: the pipelines and modules were made by `new` on this device; this runs once,
        // from the renderer's `Drop` after the device has gone idle, so no command buffer still
        // uses them.
        unsafe {
            for p in [
                self.terrain,
                self.terrain_lit,
                self.terrain_over_lit,
                self.terrain_shadow,
                self.entity,
                self.entity_over_prepass,
                self.entity_shadow,
                self.prop[0],
                self.prop[1],
                self.prop[2],
                self.prop[3],
                self.terrain_prepass,
                self.entity_prepass,
                self.hull_shield,
                self.hull_shield_depth,
                self.stain,
                self.deposit,
                self.vein,
                self.pad,
                self.water,
                self.icon,
                self.ring,
                self.range,
                self.shockwave,
                self.curtain,
                self.bar,
                self.projectile,
                self.shot,
                self.missile,
                self.nuke_missile,
                self.nuke_plume,
                self.effect,
                self.puff,
                self.beam,
                self.shield,
                self.track,
                self.print,
                self.bloom_down,
                self.bloom_up,
                self.glass_source,
                self.glass_blur,
                self.refract_copy,
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
