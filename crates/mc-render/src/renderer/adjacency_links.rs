//! Adjacency conduits: a provider (a reactor, a material fabricator) and each neighbour
//! whose lot shares an edge with its own and which it makes cheaper to run (mc-sim
//! `adjacency`) are joined by one slim line on the ground from a little inside the
//! provider's lot to a little inside the neighbour's, so it ducks under both buildings'
//! edges and what shows is the stretch between them. How it runs and what it is made of are the provider's faction's
//! (`mc_data::PowerLine`): ARC's square runs of armoured cable clamped down with a
//! junction box at each turn, the Regency's bowed arc under lapped plates between field
//! nodes (path.rs works out the run, links.wgsl builds the pieces). Its core is lit in the
//! resource's colour, materials in the interface's (`gpu_consts::tone`), energy in the
//! faction's (`PowerLine::color`), with slow pulses running from the provider into the
//! neighbour.
//!
//! The sim's links come with each tick's mirror (`RenderFrame::links`). The game adds
//! the units it wants brought out (the selection, the hovered building) and the
//! would-be links of a placement ghost each frame (`Renderer::set_link_focus`); the
//! ghost's are drawn see-through after the scene. links.wgsl lays everything on
//! `terrain_height`, so it follows the ground. A new link's line runs out from the
//! provider over a lot's settling time.

mod path;

use super::Renderer;
use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError};
use crate::gpu_consts::{link, tone};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, PASS_SET};
use ash::vk;
use glam::Vec2;
use mc_data::{Blueprints, LineLook};
use mc_sim::adjacency::Resource;
use mc_sim::mirror::LinkView;
use std::collections::HashMap;

/// Conduits drawn at most, the sim's and the ghost's together. A cosmetic cap: a base
/// has a few dozen; past it the last links stay bare ground.
const MAX_LINKS: usize = 4096;
/// A ghost's conduits stand from the start: their render time is long past.
const PLANNED_START: f32 = -1.0e9;
/// Vertices per conduit (links.wgsl): per stretch of the cable its two halves, then the
/// pieces along it and the junctions at its turns, each a six-sided block.
const LINK_VERTICES: u32 = link::SEGMENTS * 12 + (link::PIECES + link::JUNCTIONS) * 48;

/// One conduit (links.wgsl `LinkInstance`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LinkInstance {
    /// Its path from inside the provider's lot to inside the neighbour's (path.rs), metres:
    /// the first `count` are used.
    pub(crate) points: [[f32; 2]; link::POINTS as usize],
    pub(crate) count: u32,
    /// Metres along the path, end to end.
    pub(crate) length: f32,
    /// Render time the line began to run out.
    pub(crate) start: f32,
    /// `link::HIGHLIGHT` | `PLANNED` | `TURNS`, and the look from `LOOK_SHIFT`.
    pub(crate) flags: u32,
    /// The core's light, sRGB `0xRRGGBB`.
    pub(crate) rgb: u32,
    /// Where its pulses are in their cycle, 0..1, so neighbouring lines do not beat
    /// together.
    pub(crate) phase: f32,
}

/// Linear RGB as sRGB `0xRRGGBB`.
fn srgb_word(c: [f32; 3]) -> u32 {
    c.iter().fold(0, |word, &v| {
        (word << 8) | (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u32
    })
}

impl LinkInstance {
    fn new(l: &LinkView, blueprints: &Blueprints, start: f32, extra: u32) -> LinkInstance {
        let line = blueprints.power_line(l.provider_blueprint);
        let phase = (l.provider.wrapping_mul(7919) ^ l.consumer.wrapping_mul(104_729)) as f32
            / u32::MAX as f32;
        let route = path::route(
            line.path,
            Vec2::from(l.from),
            Vec2::from(l.to),
            l.edge.map(Vec2::from),
            if phase < 0.5 { 1.0 } else { -1.0 },
        );
        let look = match line.look {
            LineLook::Clamped => link::LOOK_CLAMPED,
            LineLook::Plated => link::LOOK_PLATED,
        };
        let mut flags = extra | look << link::LOOK_SHIFT;
        if route.turns {
            flags |= link::TURNS;
        }
        let rgb = match l.resource {
            Resource::Mass => tone::MASS,
            Resource::Energy => srgb_word(line.color),
        };
        LinkInstance {
            points: route.points,
            count: route.count,
            length: route.length,
            start,
            flags,
            rgb,
            phase,
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
        let mut sets = SetPool::new(gpu, &[(PASS_SET, 1)])?;
        let set = sets.alloc(gpu, layouts.pass_set, PASS_SET)?;
        let pool = sets.into_raw();
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
        })
    }

    /// A new tick's links (`RenderFrame::links`). Links there when the renderer first sees
    /// the match stand already; later ones come up from `time`.
    pub(super) fn set_sim(&mut self, links: &[LinkView], time: f32) {
        let start = if self.synced {
            time
        } else {
            time - crate::gpu_consts::settle::SECONDS
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
    pub(super) fn upload(&mut self, blueprints: &Blueprints) {
        let mut packed = std::mem::take(&mut self.packed);
        packed.clear();
        let lit = |id: u32| self.highlight.binary_search(&id).is_ok();
        packed.extend(self.sim.iter().map(|l| {
            let extra = if lit(l.provider) || lit(l.consumer) {
                link::HIGHLIGHT
            } else {
                0
            };
            let start = self.born.get(&key(l)).copied().unwrap_or(0.0);
            LinkInstance::new(l, blueprints, start, extra)
        }));
        // The cosmetic cap (`MAX_LINKS`): the ghost's go first, as they are what the
        // player is looking at.
        let solid = packed
            .len()
            .min(MAX_LINKS - self.planned.len().min(MAX_LINKS));
        packed.truncate(solid);
        packed.extend(self.planned.iter().take(MAX_LINKS - solid).map(|l| {
            LinkInstance::new(
                l,
                blueprints,
                PLANNED_START,
                link::PLANNED | link::HIGHLIGHT,
            )
        }));
        if !packed.is_empty() {
            self.links.write(0, bytemuck::cast_slice(&packed));
        }
        self.solid = solid as u32;
        self.planned_count = (packed.len() - solid) as u32;
        self.packed = packed;
    }

    /// Draws the solid conduits in the scene pass. Leaves set 0 bound; set 1 is its own.
    pub(super) fn record(&self, gpu: &Gpu, cmd: vk::CommandBuffer, layout: vk::PipelineLayout) {
        self.draw_range(gpu, cmd, layout, self.draw, 0, self.solid);
    }

    /// Draws a placement ghost's would-be conduits, see-through, in the scene pass after
    /// everything solid.
    pub(super) fn record_planned(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
    ) {
        let (first, count) = (self.solid, self.planned_count);
        self.draw_range(gpu, cmd, layout, self.ghost, first, count);
    }

    fn draw_range(
        &self,
        gpu: &Gpu,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        pipeline: vk::Pipeline,
        first: u32,
        count: u32,
    ) {
        if count == 0 {
            return;
        }
        let dev = &gpu.device;
        // SAFETY: the renderer calls this inside the scene pass while `cmd` is recording,
        // with set 0 bound for `layout` (the scene layout the pipelines were made with), and
        // the draw reads conduits `first..first + count`, all written by `upload`.
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
            dev.cmd_draw(cmd, LINK_VERTICES, count, 0, first);
        }
    }

    pub(super) fn destroy(&mut self, gpu: &Gpu) {
        // SAFETY: these objects were made by `new` on this device; this runs once, from the
        // renderer's `Drop` after the device has gone idle, pipelines before their module.
        unsafe {
            let dev = &gpu.device;
            for p in [self.draw, self.ghost] {
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

    #[test]
    fn power_colours_pack_as_srgb() {
        assert_eq!(srgb_word([1.0, 0.0, 1.0]), 0xFF00FF);
        assert_eq!(srgb_word([0.0, 1.0, 0.0]), 0x00FF00);
        assert_eq!(srgb_word([0.2140, 0.2140, 0.2140]), 0x7F7F7F);
    }
}
