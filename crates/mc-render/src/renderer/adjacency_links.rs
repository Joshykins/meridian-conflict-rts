//! Adjacency conduits: where a provider's lot shares an edge with a neighbour it makes
//! cheaper to run (mc-sim `adjacency`), the seam between the two lots carries a steel
//! tray laid on the ground and a row of armoured couplers across it. Each coupler
//! stands tall on the provider's side and tapers into a socket on the neighbour's, and
//! a slot along its top is lit in the resource's interface colour (`gpu_consts::tone`)
//! with slow pulses running from the provider into the neighbour. A bound pair (they
//! go down together) wears hazard banding on the tray and an amber strap round each
//! coupler.
//!
//! The sim's links come with each tick's mirror (`RenderFrame::links`). The game adds
//! the units it wants brought out (the selection, the hovered building) and the
//! would-be links of a placement ghost each frame (`Renderer::set_link_focus`); the
//! ghost's are drawn see-through after the scene. links.wgsl lays everything on
//! `terrain_height`, so it follows the ground. A new link's couplers come up out of the
//! ground as a lot's foundations do.

use super::Renderer;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::{link, pass, settle};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind};
use ash::vk;
use mc_sim::adjacency::Resource;
use mc_sim::mirror::LinkView;
use std::collections::HashMap;

/// Conduits drawn at most, the sim's and the ghost's together. A cosmetic cap: a base
/// has a few dozen; past it the last links stay bare ground.
const MAX_LINKS: usize = 4096;
/// A ghost's conduits stand from the start: their render time is long past.
const PLANNED_START: f32 = -1.0e9;
/// Vertices per conduit (links.wgsl): the tray's segments (top and two sides), and per
/// coupler its body (two segments of top and sides, two ends), the provider's housing,
/// the neighbour's socket and the bound strap (five faces each).
const LINK_VERTICES: u32 = link::MAX_TRAY_SEGMENTS * 18 + link::MAX_COUPLERS * (48 + 3 * 30);

/// One conduit (links.wgsl `LinkInstance`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LinkInstance {
    /// The shared stretch of lot edge, end to end, metres.
    pub(crate) a: [f32; 2],
    pub(crate) b: [f32; 2],
    /// Unit vector across the seam, from the provider's lot into the neighbour's.
    pub(crate) toward: [f32; 2],
    /// Render time the conduit began to come up.
    pub(crate) start: f32,
    /// `link::ENERGY` | `BOUND` | `HIGHLIGHT` | `PLANNED`.
    pub(crate) flags: u32,
}

impl LinkInstance {
    fn new(l: &LinkView, start: f32, extra: u32) -> LinkInstance {
        let (a, b) = (glam::Vec2::from(l.edge[0]), glam::Vec2::from(l.edge[1]));
        let along = (b - a).normalize_or_zero();
        let mut toward = along.perp();
        if toward.dot((a + b) * 0.5 - glam::Vec2::from(l.from)) < 0.0 {
            toward = -toward;
        }
        let mut flags = extra;
        if l.resource == Resource::Energy {
            flags |= link::ENERGY;
        }
        if l.bound {
            flags |= link::BOUND;
        }
        LinkInstance {
            a: a.into(),
            b: b.into(),
            toward: toward.into(),
            start,
            flags,
        }
    }
}

/// A sim link's identity across ticks: provider, neighbour, resource.
type LinkKey = (u32, u32, u8);

fn key(l: &LinkView) -> LinkKey {
    (l.provider, l.consumer, l.resource as u8)
}

pub(super) struct AdjacencyLinks {
    /// The last mirror's links.
    sim: Vec<LinkView>,
    /// When each of `sim` was first seen, for its couplers to come up.
    born: HashMap<LinkKey, f32>,
    synced: bool,
    /// Units whose links are brought out, sorted.
    highlight: Vec<u32>,
    /// A placement ghost's would-be links.
    planned: Vec<LinkView>,
    /// This frame's conduits as packed for the GPU, kept for its allocation.
    packed: Vec<LinkInstance>,
    /// Conduits in `links`: the solid ones, then the planned.
    solid: u32,
    planned_count: u32,
    links: Buffer,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    module: vk::ShaderModule,
    draw: vk::Pipeline,
    ghost: vk::Pipeline,
    shadow: vk::Pipeline,
    prepass: vk::Pipeline,
}

impl AdjacencyLinks {
    pub(super) fn new(
        gpu: &Gpu,
        layouts: &Layouts,
        passes: &Passes,
    ) -> Result<AdjacencyLinks, GpuError> {
        let dev = &gpu.device;
        let links = gpu.host_buffer(
            (MAX_LINKS * size_of::<LinkInstance>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        let sizes = [vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_BUFFER,
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
        let own = [layouts.pass_set];
        // SAFETY: the pool was made just above for exactly this one pass set of two storage
        // buffers, and `own` lives to the end of the call.
        let set = unsafe {
            dev.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&own),
            )
        }?[0];
        // Both of the pass set's bindings name the links; the shader reads the first.
        let info = [links.info()];
        let writes = [0, 1].map(|b| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(b)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&info)
        });
        // SAFETY: `set` is fresh and unused by any command buffer; each write names one of its
        // two storage-buffer bindings and the live `links`, and `writes`/`info` live to the
        // end of the call.
        unsafe { dev.update_descriptor_sets(&writes, &[]) };

        let module = gpu.shader(crate::shader_reload::spirv!("links"))?;
        let pipeline = |fs, pass, depth, blend| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module,
                    vs: c"vs_main",
                    fs,
                    layout: layouts.scene,
                    pass,
                    vertex: VertexKind::None,
                    blend,
                    depth,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        let draw = pipeline(c"fs_main", passes.scene, Depth::TestWrite, Blend::Opaque)?;
        let ghost = pipeline(c"fs_main", passes.scene, Depth::Test, Blend::Premultiplied)?;
        let shadow = pipeline(c"fs_shadow", passes.shadow, Depth::Shadow, Blend::NoColor)?;
        let prepass = pipeline(
            c"fs_shadow",
            passes.shadow,
            Depth::TestWrite,
            Blend::NoColor,
        )?;
        Ok(AdjacencyLinks {
            sim: Vec::new(),
            born: HashMap::new(),
            synced: false,
            highlight: Vec::new(),
            planned: Vec::new(),
            packed: Vec::new(),
            solid: 0,
            planned_count: 0,
            links,
            pool,
            set,
            module,
            draw,
            ghost,
            shadow,
            prepass,
        })
    }

    /// A new tick's links (`RenderFrame::links`). Links there when the renderer first sees
    /// the match stand already; later ones come up from `time`.
    pub(super) fn set_sim(&mut self, links: &[LinkView], time: f32) {
        let start = if self.synced {
            time
        } else {
            time - settle::SECONDS
        };
        self.synced = true;
        let mut born = HashMap::with_capacity(links.len());
        for l in links {
            let k = key(l);
            born.insert(k, self.born.get(&k).copied().unwrap_or(start));
        }
        self.born = born;
        self.sim.clear();
        self.sim.extend_from_slice(links);
    }

    /// Packs this frame's conduits and writes them to the GPU.
    pub(super) fn upload(&mut self) {
        let mut packed = std::mem::take(&mut self.packed);
        packed.clear();
        let lit = |id: u32| self.highlight.binary_search(&id).is_ok();
        packed.extend(self.sim.iter().map(|l| {
            let extra = if lit(l.provider) || lit(l.consumer) {
                link::HIGHLIGHT
            } else {
                0
            };
            LinkInstance::new(l, self.born.get(&key(l)).copied().unwrap_or(0.0), extra)
        }));
        // The cosmetic cap (`MAX_LINKS`): the ghost's go first, as they are what the
        // player is looking at.
        let solid = packed
            .len()
            .min(MAX_LINKS - self.planned.len().min(MAX_LINKS));
        packed.truncate(solid);
        packed.extend(
            self.planned
                .iter()
                .take(MAX_LINKS - solid)
                .map(|l| LinkInstance::new(l, PLANNED_START, link::PLANNED | link::HIGHLIGHT)),
        );
        if !packed.is_empty() {
            self.links.write(0, bytemuck::cast_slice(&packed));
        }
        self.solid = solid as u32;
        self.planned_count = (packed.len() - solid) as u32;
        self.packed = packed;
    }

    /// Draws the solid conduits in the current render pass: the scene, the depth pre-pass
    /// or a shadow cascade, by `pass_kind`. Leaves set 0 bound; set 1 is its own.
    pub(super) fn record(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        pass_kind: u32,
    ) {
        let pipeline = if pass_kind & pass::KIND_MASK == pass::SHADOW {
            self.shadow
        } else if pass_kind == pass::PREPASS {
            self.prepass
        } else {
            self.draw
        };
        self.draw_range(gpu, cmd, layout, pipeline, pass_kind, 0, self.solid);
    }

    /// Draws a placement ghost's would-be conduits, see-through, in the scene pass after
    /// everything solid.
    pub(super) fn record_planned(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
    ) {
        self.draw_range(
            gpu,
            cmd,
            layout,
            self.ghost,
            pass::MAIN,
            self.solid,
            self.planned_count,
        );
    }

    #[expect(clippy::too_many_arguments, reason = "one draw's whole state, private")]
    fn draw_range(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        pipeline: vk::Pipeline,
        pass_kind: u32,
        first: u32,
        count: u32,
    ) {
        if count == 0 {
            return;
        }
        let dev = &gpu.device;
        // SAFETY: the renderer calls this inside a render pass of the matching kind while `cmd`
        // is recording, with set 0 bound for `layout` (the scene layout the pipelines were
        // made with); the 8 bytes pushed fit its 16-byte vertex+fragment range, and the draw
        // reads conduits `first..first + count`, all written by `upload`.
        unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.set],
                &[],
            );
            dev.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&[pass_kind, 0u32]),
            );
            dev.cmd_draw(cmd, LINK_VERTICES, count, 0, first);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their module.
        unsafe {
            let dev = &gpu.device;
            for p in [self.draw, self.ghost, self.shadow, self.prepass] {
                dev.destroy_pipeline(p, None);
            }
            dev.destroy_shader_module(self.module, None);
            dev.destroy_descriptor_pool(self.pool, None);
        }
        gpu.destroy_buffer(std::mem::replace(&mut self.links, Buffer::null()));
    }
}

impl Renderer {
    /// What the game brings out among the adjacency conduits, from this frame on: the
    /// conduits touching any unit in `highlighted` (the selection, the building under the
    /// pointer) are drawn brighter, and `planned` (a placement ghost's would-be links,
    /// worked out with `mc_sim::adjacency`; `provider`/`consumer` ids are not read) are
    /// drawn see-through. Both stay until the next call; pass empty slices to clear.
    pub fn set_link_focus(&mut self, highlighted: &[u32], planned: &[LinkView]) {
        let links = &mut self.adjacency_links;
        links.highlight.clear();
        links.highlight.extend_from_slice(highlighted);
        links.highlight.sort_unstable();
        links.planned.clear();
        links.planned.extend_from_slice(planned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(edge: [[f32; 2]; 2], from: [f32; 2]) -> LinkView {
        LinkView {
            provider: 1,
            consumer: 2,
            provider_blueprint: mc_data::BlueprintId(0),
            consumer_blueprint: mc_data::BlueprintId(0),
            owner: 0,
            resource: Resource::Energy,
            share: 0.2,
            edge,
            from,
            bound: true,
        }
    }

    #[test]
    fn conduits_run_from_the_provider_into_the_neighbour() {
        // A provider west of a seam along x = 48, then one north of a seam along y = 48.
        let east = LinkInstance::new(&view([[48.0, -48.0], [48.0, 48.0]], [0.0, 0.0]), 0.0, 0);
        assert_eq!(east.toward, [1.0, 0.0]);
        let south = LinkInstance::new(&view([[60.0, 48.0], [84.0, 48.0]], [72.0, 60.0]), 0.0, 0);
        assert_eq!(south.toward, [0.0, -1.0]);
        assert_eq!(east.flags, link::ENERGY | link::BOUND);
    }
}
