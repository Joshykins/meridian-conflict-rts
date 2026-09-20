//! Frame orchestration.
//!
//! Per frame the CPU writes one uniform block, the terrain node list, and
//! whatever the UI drew. Per sim tick it copies the render mirror into GPU
//! buffers wholesale. Everything per-entity (interpolation, culling, LOD,
//! draw generation, animation) runs in shaders.

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::models::{self, Legs, MeshVertex, Model, Treads};
use crate::overlay::{Overlay, OverlayVertex, MAX_OVERLAY_VERTICES};
use crate::pipelines::{Layouts, Passes, Pipelines, DEPTH_FORMAT, HDR_FORMAT, SHADOW_SIZE};
use crate::terrain::{self, TerrainNode, TerrainUpload, TileCache, MAX_NODES, TILE_LAYERS};
use crate::textures;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::{MapFile, PropKind, BUILD_CELL_M, TILE_SAMPLES};
use mc_sim::mirror::{
    ProjectileInstance, RenderFrame, SimEvent, StainInstance, UnitInstance, KIND_GHOST, KIND_PROP,
    KIND_WRECK, STATE_RADAR,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::sync::Arc;

pub const MAX_DYNAMIC: usize = mc_sim::tables::MAX_UNITS + mc_sim::tables::MAX_WRECKS + 512;
pub const MAX_MARKS: usize = 4096;
pub const MAX_EFFECTS: usize = 2048;
/// Rings of short-lived particles and of track marks; the oldest are overwritten.
pub const MAX_PUFFS: usize = 8192;
/// Reclaim beams drawn at once, each `BEAM_QUADS` quads: the ribbon, two glows and the bits going up it.
pub const MAX_BEAMS: usize = 1024;
const BEAM_QUADS: u32 = 32;
/// A beam that has shut off is kept this long, so what was already on its way up it arrives.
const BEAM_LINGER: f32 = 3.0;
/// A beam whose far end jumps farther than this (plus the target's size) between ticks is on something new.
const BEAM_JUMP: f32 = 6.0;

/// Mirrors `Beam` in shaders/beams.wgsl: the sim's record, and when the beam came on and went off.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuBeam {
    beam: mc_sim::reclaim::BeamInstance,
    start: f32,
    /// Negative while the beam is on.
    end: f32,
    pad: [f32; 2],
}
pub const MAX_TRACK_MARKS: usize = 32768;
/// Seconds a track mark stays on the ground.
const TRACK_MARK_LIFE: f32 = 50.0;
/// Levels of the bloom chain, each half the size of the one before.
const BLOOM_LEVELS: usize = 5;
pub const MAX_RANGES: usize = 512;
/// Steps around a range ring: rings reaching `RANGE_LONG` metres or more, and shorter ones.
const RANGE_SEGMENTS: [u32; 2] = [256, 96];
const RANGE_LONG: f32 = 450.0;
const MAX_PROJECTILES: usize = mc_sim::tables::MAX_PROJECTILES;
const MAX_STAINS: usize = mc_sim::tables::MAX_STAINS;

pub enum Target {
    Window {
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
        vsync: bool,
    },
    Headless {
        width: u32,
        height: u32,
    },
}

pub struct SceneDesc {
    pub map: Arc<MapFile>,
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    /// Colour per player slot, linear RGB.
    pub team_colors: [[f32; 3]; 8],
}

/// Selection / hover marker on an entity of the current render frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Mark {
    /// Index into `RenderFrame::units`.
    pub unit_index: u32,
    /// 0 selected, 1 hovered.
    pub kind: u32,
    /// Construction fill, zero to one. Negative: the unit is not building, so
    /// the bar under health stays off.
    pub work: f32,
    pub _pad: u32,
}

/// What something reaches, drawn as a circle on the ground: the edge of
/// `outer`, and dashed the edge of `inner` when there is a dead zone. Rings
/// of one `group` merge into the outline of what they cover together.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
pub struct RangeRing {
    /// World position, already interpolated to this frame.
    pub center: [f32; 2],
    pub inner: f32,
    pub outer: f32,
    /// Linear RGB.
    pub color: [f32; 3],
    pub group: u32,
}

pub struct FrameInput<'a> {
    pub camera: &'a Camera,
    /// Seconds since start, for shader animation.
    pub time: f32,
    /// Position between the last two sim ticks, zero to one.
    pub alpha: f32,
    /// A new tick's mirror, when one arrived since the last frame.
    pub sim: Option<&'a RenderFrame>,
    /// Placement previews, appended after the sim's units.
    pub ghosts: &'a [UnitInstance],
    pub marks: &'a [Mark],
    pub ranges: &'a [RangeRing],
    /// How many of `ranges`, from the front, are drawn. The rest only mask the
    /// others: rings the game knows to lie wholly inside their group's reach.
    pub ranges_drawn: usize,
    pub overlay: &'a Overlay,
    pub build_grid: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// GPU time per pass in milliseconds, from timestamp queries of the previous frame.
    pub gpu_passes: Vec<(&'static str, f32)>,
    pub terrain_nodes: usize,
    pub dynamic_entities: usize,
    pub static_entities: usize,
    pub tiles_resident: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    shadow_view_proj: [[f32; 4]; 4],
    camera: [f32; 4],
    sun: [f32; 4],
    viewport: [f32; 4],
    frustum: [[f32; 4]; 6],
    map: [f32; 4],
    height: [f32; 4],
    lod: [f32; 4],
    counts: [u32; 4],
    plating: [f32; 4],
    accent: [f32; 4],
    glow: [f32; 4],
    team_colors: [[f32; 4]; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ModelInfo {
    slot: u32,
    icon: u32,
    bounds_radius: f32,
    height: f32,
    turret_pivot: [f32; 4],
    spinner_pivot: [f32; 4],
    /// `Legs`: hip and the stride, knee and the lift, ankle and the stance. All zero for a model without legs.
    leg_hip: [f32; 4],
    leg_knee: [f32; 4],
    leg_ankle: [f32; 4],
    /// The left elbow forearms pitch about; w is one when the model has one.
    arm_pivot: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawSlot {
    index_count: u32,
    first_index: u32,
    vertex_offset: i32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Effect {
    pos: [f32; 3],
    start: f32,
    params: [f32; 4],
}

/// Mirrors `Puff` in shaders/puffs.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Puff {
    pos: [f32; 3],
    start: f32,
    vel: [f32; 3],
    life: f32,
    /// Size at birth, size at the end, kind, seed.
    params: [f32; 4],
}

const PUFF_DUST: f32 = 0.0;
const PUFF_SMOKE: f32 = 1.0;
const PUFF_CLOD: f32 = 2.0;
const PUFF_SPARK: f32 = 3.0;
const PUFF_FIRE: f32 = 4.0;
const PUFF_FIREBALL: f32 = 5.0;

/// Mirrors `TrackMark` in shaders/ground.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TrackMark {
    from: [f32; 2],
    to: [f32; 2],
    half_gauge: f32,
    width: f32,
    start: f32,
    life: f32,
}

/// Small deterministic generator for effect scatter. Presentation only.
struct Scatter(u32);

impl Scatter {
    /// Zero to one.
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    /// Minus one to one.
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// A direction on the upper hemisphere, at least `up` of the way toward straight up.
    fn upward(&mut self, up: f32) -> Vec3 {
        let a = self.unit() * std::f32::consts::TAU;
        let z = up + (1.0 - up) * self.unit();
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(a.cos() * r, a.sin() * r, z)
    }
}

struct Swapchain {
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    views: Vec<vk::ImageView>,
    vsync: bool,
}

enum Output {
    Window(Swapchain),
    Headless { image: Image, readback: Buffer },
}

const PASS_NAMES: [&str; 4] = ["cull", "shadow", "scene", "present"];

pub struct Renderer {
    gpu: Gpu,
    output: Output,
    present_format: vk::Format,
    width: u32,
    height: u32,
    passes: Passes,
    layouts: Layouts,
    pipelines: Pipelines,
    hdr: Image,
    depth: Image,
    shadow: Image,
    /// Bloom chain, largest (half size) first.
    bloom: Vec<Image>,
    bloom_fbs: Vec<vk::Framebuffer>,
    /// One per bloom level, for reading it; `hdr_set` reads the scene the same way.
    bloom_sets: Vec<vk::DescriptorSet>,
    hdr_set: vk::DescriptorSet,
    scene_fb: vk::Framebuffer,
    shadow_fb: vk::Framebuffer,
    present_fbs: Vec<vk::Framebuffer>,

    descriptor_pool: vk::DescriptorPool,
    scene_set: vk::DescriptorSet,
    cull_set: vk::DescriptorSet,
    screen_set: vk::DescriptorSet,
    nodes_set: vk::DescriptorSet,
    marks_set: vk::DescriptorSet,
    ranges_set: vk::DescriptorSet,
    sprites_set: vk::DescriptorSet,
    stains_set: vk::DescriptorSet,
    puffs_set: vk::DescriptorSet,
    samplers: [vk::Sampler; 3],

    globals: Buffer,
    dynamic: Buffer,
    statics: Buffer,
    model_table: Buffer,
    slot_table: Buffer,
    vis: Buffer,
    counters: Buffer,
    commands: Buffer,
    visible: Buffer,
    props_dead: Buffer,
    nodes: Buffer,
    marks: Buffer,
    ranges: Buffer,
    range_ib: Buffer,
    projectiles: Buffer,
    effects: Buffer,
    stains: Buffer,
    puffs: Buffer,
    beams: Buffer,
    track_marks: Buffer,
    overlay_vb: Buffer,
    mesh_vb: Buffer,
    mesh_ib: Buffer,
    quad_vb: Buffer,
    quad_ib: Buffer,
    grid_vb: Buffer,
    grid_ib: Buffer,
    grid_index_count: u32,
    patch_vb: Buffer,
    patch_ib: Buffer,
    patch_index_count: u32,

    overview: Image,
    tiles: Image,
    tile_index: Image,
    fog: Image,
    fog_dims: (u32, u32),
    noise: Image,
    panel: Image,
    font: Image,
    font_uploaded: bool,

    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    query_pool: vk::QueryPool,
    timestamp_period: f32,
    queries_valid: bool,
    garbage: Vec<Buffer>,

    pool: Arc<Pool>,
    tile_cache: TileCache,
    node_scratch: Vec<TerrainNode>,
    upload_scratch: Vec<TerrainUpload>,
    map_info: mc_map::MapInfo,
    palette: [[f32; 4]; 3],
    team_colors: [[f32; 4]; 8],
    slot_count: u32,
    static_count: u32,
    dynamic_count: u32,
    sim_units: u32,
    projectile_count: u32,
    stain_count: u32,
    pad_count: u32,
    /// Mass deposits as ground splats, written after stains and pads.
    deposit_splats: Vec<StainInstance>,
    deposit_count: u32,
    deposit_first: u32,
    effect_cursor: usize,
    puff_cursor: usize,
    beam_count: u32,
    /// Beams that are on, by the unit they come from.
    beams_live: std::collections::HashMap<u32, GpuBeam>,
    /// Beams that have shut off and are emptying out.
    beams_ended: Vec<GpuBeam>,
    track_cursor: usize,
    /// Track marks written so far, capped at the ring's size: how many to draw.
    track_count: u32,
    scatter: Scatter,
    blueprints: Arc<Blueprints>,
    /// Per blueprint: where its tracks touch the ground, if it has any.
    treads: Vec<Option<Treads>>,
    /// Per blueprint: the legs of a walker, for the dust and prints its feet leave.
    legs: Vec<Option<Legs>>,
    /// Render-clock seconds between the last two sim ticks.
    tick_seconds: f32,
    last_tick_time: f32,
    fog_enabled: bool,
    last_time: f32,
    pub stats: FrameStats,
}

fn fallback_model(key: &str, radius: f32, height: f32) -> Model {
    let mut lod = models::MeshLod::default();
    let (r, h) = (radius * 0.7, height);
    let faces: [([f32; 3], [[f32; 3]; 4]); 5] = [
        (
            [0.0, 0.0, 1.0],
            [[-r, -r, h], [r, -r, h], [r, r, h], [-r, r, h]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[r, -r, 0.0], [r, r, 0.0], [r, r, h], [r, -r, h]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-r, r, 0.0], [-r, -r, 0.0], [-r, -r, h], [-r, r, h]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[r, r, 0.0], [-r, r, 0.0], [-r, r, h], [r, r, h]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-r, -r, 0.0], [r, -r, 0.0], [r, -r, h], [-r, -r, h]],
        ),
    ];
    for (normal, corners) in faces {
        let base = lod.vertices.len() as u32;
        for pos in corners {
            lod.vertices.push(MeshVertex {
                pos,
                normal,
                uv: [pos[0] + pos[1], pos[2]],
                material: models::material::PLATING,
                part: models::part::HULL,
                rig: 0,
            });
        }
        lod.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Model {
        key: key.to_owned(),
        lods: [lod.clone(), lod.clone(), lod],
        turret_pivot: [0.0; 3],
        spinner_pivot: [0.0; 3],
        bounds_radius: (radius * radius + height * height).sqrt(),
        treads: None,
        legs: None,
        arm_pivot: None,
    }
}

impl Renderer {
    pub fn new(target: Target, scene: SceneDesc) -> Result<Renderer, GpuError> {
        let (gpu, surface, width, height, vsync) = match &target {
            Target::Window {
                display,
                window,
                width,
                height,
                vsync,
            } => {
                let extensions =
                    ash_window::enumerate_required_extensions(*display).map_err(GpuError::Vk)?;
                let gpu = Gpu::new(extensions)?;
                let surface = unsafe {
                    ash_window::create_surface(&gpu.entry, &gpu.instance, *display, *window, None)
                }?;
                (gpu, Some(surface), *width, *height, *vsync)
            }
            Target::Headless { width, height } => (Gpu::new(&[])?, None, *width, *height, false),
        };
        let (width, height) = (width.max(16), height.max(16));

        let present_format = match surface {
            Some(surface) => {
                let formats = unsafe {
                    gpu.surface_fn
                        .get_physical_device_surface_formats(gpu.physical, surface)
                }?;
                formats
                    .iter()
                    .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
                    .or_else(|| {
                        formats
                            .iter()
                            .find(|f| f.format == vk::Format::R8G8B8A8_SRGB)
                    })
                    .or(formats.first())
                    .map(|f| f.format)
                    .ok_or_else(|| GpuError::NoDevice("surface reports no formats".into()))?
            }
            None => vk::Format::R8G8B8A8_SRGB,
        };
        let present_layout = if surface.is_some() {
            vk::ImageLayout::PRESENT_SRC_KHR
        } else {
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL
        };
        let passes = Passes::new(&gpu, present_format, present_layout)?;
        let layouts = Layouts::new(&gpu)?;
        let pipelines = Pipelines::new(&gpu, &layouts, &passes)?;

        // Models: one per blueprint (each is fitted to its blueprint's size), then one per prop kind.
        let bps = &scene.blueprints;
        let mut model_list: Vec<(Model, u32)> = Vec::new();
        let mut treads: Vec<Option<Treads>> = Vec::new();
        let mut legs: Vec<Option<Legs>> = Vec::new();
        for bp in &bps.units {
            let (radius, height) = (bp.radius.to_f32(), bp.height.to_f32());
            let model = models::build_model_scaled(&bp.visual.mesh, radius, height, bp.tech)
                .unwrap_or_else(|| {
                    log::warn!("no model for mesh key {:?}; using a box", bp.visual.mesh);
                    fallback_model(&bp.visual.mesh, radius, height)
                });
            let icon =
                bp.visual.icon as u32 | (bp.tech as u32) << 8 | (bp.is_mobile() as u32) << 16;
            treads.push(model.treads);
            legs.push(model.legs);
            model_list.push((model, icon));
        }
        let prop_base = model_list.len() as u32;
        for kind in PropKind::ALL {
            let key = models::prop_model_key(kind.raw());
            let model = models::build_model(key).unwrap_or_else(|| fallback_model(key, 4.0, 8.0));
            model_list.push((model, 13));
        }
        let mut vertices: Vec<MeshVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut slots: Vec<DrawSlot> = Vec::new();
        let mut infos: Vec<ModelInfo> = Vec::new();
        for (model, icon) in &model_list {
            let height = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(0.0f32, f32::max);
            infos.push(ModelInfo {
                slot: slots.len() as u32,
                icon: *icon,
                bounds_radius: model.bounds_radius,
                height,
                turret_pivot: [
                    model.turret_pivot[0],
                    model.turret_pivot[1],
                    model.turret_pivot[2],
                    0.0,
                ],
                spinner_pivot: [
                    model.spinner_pivot[0],
                    model.spinner_pivot[1],
                    model.spinner_pivot[2],
                    0.0,
                ],
                leg_hip: model
                    .legs
                    .map_or([0.0; 4], |l| [l.hip[0], l.hip[1], l.hip[2], l.stride]),
                leg_knee: model
                    .legs
                    .map_or([0.0; 4], |l| [l.knee[0], l.knee[1], l.knee[2], l.lift]),
                leg_ankle: model
                    .legs
                    .map_or([0.0; 4], |l| [l.ankle[0], l.ankle[1], l.ankle[2], l.stance]),
                arm_pivot: model
                    .arm_pivot
                    .map_or([0.0; 4], |p| [p[0], p[1], p[2], 1.0]),
            });
            for lod in &model.lods {
                slots.push(DrawSlot {
                    index_count: lod.indices.len() as u32,
                    first_index: indices.len() as u32,
                    vertex_offset: vertices.len() as i32,
                    pad: 0,
                });
                vertices.extend_from_slice(&lod.vertices);
                indices.extend_from_slice(&lod.indices);
            }
        }
        // The last slot draws strategic icons: a quad from its own vertex buffer.
        slots.push(DrawSlot {
            index_count: 6,
            first_index: indices.len() as u32,
            vertex_offset: 0,
            pad: 0,
        });
        indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        let slot_count = slots.len() as u32;

        // Static entities: the map's props.
        let info = scene.map.info().clone();
        let tile_cache = TileCache::new(scene.map.clone());
        let kinds: Vec<u16> = PropKind::ALL.iter().map(|k| k.raw()).collect();
        let statics_data: Vec<UnitInstance> = scene
            .map
            .props()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let xy = p.pos.to_f32();
                let pos = [
                    xy[0],
                    xy[1],
                    tile_cache.overview_height(glam::Vec2::from(xy)),
                ];
                let heading = p.heading.to_radians_f32();
                let kind = kinds.iter().position(|k| *k == p.kind.raw()).unwrap_or(0) as u32;
                UnitInstance {
                    prev_pos: pos,
                    prev_heading: heading,
                    pos,
                    heading,
                    blueprint: prop_base + kind,
                    owner_flags: KIND_PROP,
                    health: 1.0,
                    build: 1.0,
                    turret_yaw: 0.0,
                    radius: 4.0,
                    unit_id: i as u32,
                    _pad: p.scale_milli as u32,
                    gait: [0.0; 3],
                    upgrade: 0.0,
                    arm_pitch: [0.0; 4],
                    prev_turret_yaw: 0.0,
                    weld: [0.0; 3],
                }
            })
            .collect();
        let deposit_splats: Vec<StainInstance> = scene
            .map
            .mass_deposits()
            .iter()
            .map(|d| {
                let xy = d.to_f32();
                let seed = ((xy[0] * 7.13 + xy[1] * 13.27).abs() as u32) & 0xFFFFFF;
                StainInstance {
                    pos: xy,
                    // Half-extent of the 2x2 extractor lot (one build cell).
                    radius: BUILD_CELL_M as f32,
                    strength_seed: 255 | seed << 8,
                }
            })
            .collect();
        let static_count = statics_data.len() as u32;

        use vk::BufferUsageFlags as U;
        let storage = U::STORAGE_BUFFER;
        let total_entities = static_count as u64 + MAX_DYNAMIC as u64;
        let globals = gpu.host_buffer(size_of::<Globals>() as u64, U::UNIFORM_BUFFER)?;
        let dynamic = gpu.host_buffer((MAX_DYNAMIC * size_of::<UnitInstance>()) as u64, storage)?;
        let statics = gpu.buffer_with_data(bytemuck::cast_slice(&statics_data), storage)?;
        let model_table = gpu.buffer_with_data(bytemuck::cast_slice(&infos), storage)?;
        let slot_table = gpu.buffer_with_data(bytemuck::cast_slice(&slots), storage)?;
        let vis = gpu.device_buffer(total_entities * 4, storage)?;
        let counters = gpu.device_buffer(slot_count as u64 * 4, storage)?;
        let commands = gpu.device_buffer(slot_count as u64 * 20, storage | U::INDIRECT_BUFFER)?;
        // Room for every unit twice: its model and its strategic icon.
        let visible = gpu.device_buffer((total_entities + MAX_DYNAMIC as u64) * 4, storage)?;
        let props_dead = gpu.host_buffer((static_count as u64).div_ceil(32).max(1) * 4, storage)?;
        props_dead.write(0, &vec![0u8; props_dead.size as usize]);
        let nodes = gpu.host_buffer((MAX_NODES * size_of::<TerrainNode>()) as u64, storage)?;
        let marks = gpu.host_buffer((MAX_MARKS * size_of::<Mark>()) as u64, storage)?;
        let ranges = gpu.host_buffer((MAX_RANGES * size_of::<RangeRing>()) as u64, storage)?;
        let projectiles = gpu.host_buffer(
            (MAX_PROJECTILES * size_of::<ProjectileInstance>()) as u64,
            storage,
        )?;
        let effects = gpu.host_buffer((MAX_EFFECTS * size_of::<Effect>()) as u64, storage)?;
        effects.write(0, &vec![0u8; effects.size as usize]);
        let stains = gpu.host_buffer((MAX_STAINS * size_of::<StainInstance>()) as u64, storage)?;
        let puffs = gpu.host_buffer((MAX_PUFFS * size_of::<Puff>()) as u64, storage)?;
        puffs.write(0, &vec![0u8; puffs.size as usize]);
        let beams = gpu.host_buffer((MAX_BEAMS * size_of::<GpuBeam>()) as u64, storage)?;
        let track_marks =
            gpu.host_buffer((MAX_TRACK_MARKS * size_of::<TrackMark>()) as u64, storage)?;
        track_marks.write(0, &vec![0u8; track_marks.size as usize]);
        let overlay_vb = gpu.host_buffer(
            (MAX_OVERLAY_VERTICES * size_of::<OverlayVertex>()) as u64,
            U::VERTEX_BUFFER,
        )?;
        let mesh_vb = gpu.buffer_with_data(bytemuck::cast_slice(&vertices), U::VERTEX_BUFFER)?;
        let mesh_ib = gpu.buffer_with_data(bytemuck::cast_slice(&indices), U::INDEX_BUFFER)?;
        let quad: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let quad_vb = gpu.buffer_with_data(bytemuck::cast_slice(&quad), U::VERTEX_BUFFER)?;
        let quad_ib = gpu.buffer_with_data(
            bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]),
            U::INDEX_BUFFER,
        )?;
        // A range ring is a strip of two vertices per step, made in the vertex shader.
        let range_i: Vec<u32> = (0..RANGE_SEGMENTS[0])
            .flat_map(|s| [2 * s, 2 * s + 1, 2 * s + 2, 2 * s + 2, 2 * s + 1, 2 * s + 3])
            .collect();
        let range_ib = gpu.buffer_with_data(bytemuck::cast_slice(&range_i), U::INDEX_BUFFER)?;
        let (grid_v, grid_i) = terrain::grid_mesh();
        let grid_vb = gpu.buffer_with_data(bytemuck::cast_slice(&grid_v), U::VERTEX_BUFFER)?;
        let grid_ib = gpu.buffer_with_data(bytemuck::cast_slice(&grid_i), U::INDEX_BUFFER)?;
        // Stain decal patch: a 6x6 grid over [-1, 1] so it can drape over the ground.
        let mut patch_v: Vec<[f32; 2]> = Vec::new();
        let mut patch_i: Vec<u32> = Vec::new();
        for y in 0..=6 {
            for x in 0..=6 {
                patch_v.push([x as f32 / 3.0 - 1.0, y as f32 / 3.0 - 1.0]);
            }
        }
        for y in 0..6u32 {
            for x in 0..6u32 {
                let at = |x: u32, y: u32| y * 7 + x;
                patch_i.extend_from_slice(&[
                    at(x, y),
                    at(x + 1, y),
                    at(x + 1, y + 1),
                    at(x, y),
                    at(x + 1, y + 1),
                    at(x, y + 1),
                ]);
            }
        }
        let patch_vb = gpu.buffer_with_data(bytemuck::cast_slice(&patch_v), U::VERTEX_BUFFER)?;
        let patch_ib = gpu.buffer_with_data(bytemuck::cast_slice(&patch_i), U::INDEX_BUFFER)?;

        // Textures.
        let sampled = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        let (ow, oh) = scene.map.overview_dims();
        let overview = gpu.image(&ImageDesc {
            width: ow,
            height: oh,
            format: vk::Format::R16_UNORM,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(
            &overview,
            0,
            0,
            None,
            bytemuck::cast_slice(scene.map.overview()),
            true,
        )?;
        let tiles = gpu.image(&ImageDesc {
            width: TILE_SAMPLES,
            height: TILE_SAMPLES,
            format: vk::Format::R16_UNORM,
            usage: sampled,
            layers: TILE_LAYERS,
            mips: 1,
            array: true,
        })?;
        gpu.upload_image(
            &tiles,
            0,
            0,
            None,
            &vec![0u8; (TILE_SAMPLES * TILE_SAMPLES * 2) as usize],
            true,
        )?;
        let (tw, th) = scene.map.size_tiles();
        let tile_index = gpu.image(&ImageDesc {
            width: tw,
            height: th,
            format: vk::Format::R16_UINT,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(
            &tile_index,
            0,
            0,
            None,
            &vec![0u8; (tw * th * 2) as usize],
            true,
        )?;
        let size = info.size_metres().to_f32();
        let fog_dims = (
            (size[0] as u32 >> 6).max(1) + 1,
            (size[1] as u32 >> 6).max(1) + 1,
        );
        let fog = gpu.image(&ImageDesc {
            width: fog_dims.0,
            height: fog_dims.1,
            format: vk::Format::R8G8_UNORM,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(
            &fog,
            0,
            0,
            None,
            &vec![255u8; (fog_dims.0 * fog_dims.1 * 2) as usize],
            true,
        )?;
        let rgba_mips = |data: Vec<u8>, flatten: bool| -> Result<Image, GpuError> {
            let n = textures::SIZE as u32;
            let mips = 32 - n.leading_zeros();
            let image = gpu.image(&ImageDesc {
                width: n,
                height: n,
                format: vk::Format::R8G8B8A8_UNORM,
                usage: sampled,
                layers: 1,
                mips,
                array: false,
            })?;
            gpu.upload_image(&image, 0, 0, None, &data, true)?;
            let mut chain = textures::mip_chain(&data, textures::SIZE);
            if flatten {
                textures::flatten_noise_mips(&mut chain);
            }
            for (level, (_, pixels)) in chain.iter().enumerate() {
                gpu.upload_image(&image, 0, level as u32 + 1, None, pixels, false)?;
            }
            Ok(image)
        };
        let noise = rgba_mips(textures::noise_map(), true)?;
        let panel = rgba_mips(textures::panel_map(), false)?;
        // The overlay's atlas; its contents arrive with the first frame (`upload_overlay_atlas`).
        let font = gpu.image(&ImageDesc {
            width: textures::FONT_ATLAS_W as u32,
            height: textures::FONT_ATLAS_H as u32,
            format: vk::Format::R8G8B8A8_SRGB,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;

        let shadow = gpu.image(&ImageDesc {
            width: SHADOW_SIZE,
            height: SHADOW_SIZE,
            format: DEPTH_FORMAT,
            usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        let samplers = [
            gpu.sampler(
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                true,
                None,
            )?,
            gpu.sampler(
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                false,
                None,
            )?,
            gpu.sampler(
                vk::Filter::LINEAR,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                false,
                Some(vk::CompareOp::LESS_OR_EQUAL),
            )?,
        ];

        // Descriptor sets.
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 4,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 48,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: 16,
            },
        ];
        let descriptor_pool = unsafe {
            gpu.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(17)
                    .pool_sizes(&pool_sizes),
                None,
            )
        }?;
        let alloc = |layout: vk::DescriptorSetLayout| -> Result<vk::DescriptorSet, GpuError> {
            let layouts = [layout];
            Ok(unsafe {
                gpu.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(descriptor_pool)
                        .set_layouts(&layouts),
                )
            }?[0])
        };
        let scene_set = alloc(layouts.scene_set)?;
        let cull_set = alloc(layouts.cull_set)?;
        let screen_set = alloc(layouts.screen_set)?;
        let nodes_set = alloc(layouts.pass_set)?;
        let marks_set = alloc(layouts.pass_set)?;
        let ranges_set = alloc(layouts.pass_set)?;
        let sprites_set = alloc(layouts.pass_set)?;
        let stains_set = alloc(layouts.pass_set)?;
        let puffs_set = alloc(layouts.pass_set)?;
        let hdr_set = alloc(layouts.screen_set)?;
        let bloom_sets = (0..BLOOM_LEVELS)
            .map(|_| alloc(layouts.screen_set))
            .collect::<Result<Vec<_>, _>>()?;

        let write_buffers =
            |set: vk::DescriptorSet, first: u32, ty: vk::DescriptorType, buffers: &[&Buffer]| {
                for (i, b) in buffers.iter().enumerate() {
                    let info = [b.info()];
                    let write = [vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(first + i as u32)
                        .descriptor_type(ty)
                        .buffer_info(&info)];
                    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
                }
            };
        let write_image =
            |set: vk::DescriptorSet, binding: u32, view: vk::ImageView, layout: vk::ImageLayout| {
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
                unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
            };
        let write_sampler = |set: vk::DescriptorSet, binding: u32, sampler: vk::Sampler| {
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
            unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        };
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        write_buffers(
            scene_set,
            0,
            vk::DescriptorType::UNIFORM_BUFFER,
            &[&globals],
        );
        write_buffers(
            scene_set,
            1,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&dynamic, &statics, &model_table, &visible],
        );
        for (binding, image) in [
            (5, &overview),
            (6, &tiles),
            (7, &tile_index),
            (8, &fog),
            (9, &noise),
            (10, &panel),
        ] {
            write_image(scene_set, binding, image.view, read);
        }
        write_image(
            scene_set,
            11,
            shadow.view,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        );
        for (i, s) in samplers.iter().enumerate() {
            write_sampler(scene_set, 12 + i as u32, *s);
        }
        write_buffers(cull_set, 0, vk::DescriptorType::UNIFORM_BUFFER, &[&globals]);
        write_buffers(
            cull_set,
            1,
            vk::DescriptorType::STORAGE_BUFFER,
            &[
                &dynamic,
                &statics,
                &model_table,
                &slot_table,
                &vis,
                &counters,
                &commands,
                &visible,
                &props_dead,
            ],
        );
        write_buffers(
            nodes_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&nodes, &nodes],
        );
        write_buffers(
            marks_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&marks, &marks],
        );
        write_buffers(
            ranges_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&ranges, &ranges],
        );
        write_buffers(
            sprites_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&projectiles, &effects],
        );
        write_buffers(
            stains_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&stains, &track_marks],
        );
        // The beams ride in the puffs' set: its second binding was spare.
        write_buffers(
            puffs_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&puffs, &beams],
        );
        for set in std::iter::once(&screen_set)
            .chain([&hdr_set])
            .chain(&bloom_sets)
        {
            write_image(*set, 1, font.view, read);
            write_sampler(*set, 2, samplers[1]);
        }

        let shadow_views = [shadow.view];
        let shadow_fb = unsafe {
            gpu.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(passes.shadow)
                    .attachments(&shadow_views)
                    .width(SHADOW_SIZE)
                    .height(SHADOW_SIZE)
                    .layers(1),
                None,
            )
        }?;

        let cmd = unsafe {
            gpu.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(gpu.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }?[0];
        let fence = unsafe {
            gpu.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }?;
        let image_available = unsafe {
            gpu.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }?;
        let render_finished = unsafe {
            gpu.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }?;
        let query_pool = unsafe {
            gpu.device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(PASS_NAMES.len() as u32 * 2),
                None,
            )
        }?;
        let timestamp_period = gpu.limits.timestamp_period;

        let faction = &bps.factions[0];
        let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        let mut team_colors = [[1.0; 4]; 8];
        for (i, c) in scene.team_colors.iter().enumerate() {
            team_colors[i] = rgba(*c);
        }

        let placeholder = |gpu: &Gpu| {
            gpu.image(&ImageDesc {
                width: 16,
                height: 16,
                format: HDR_FORMAT,
                usage: vk::ImageUsageFlags::SAMPLED,
                layers: 1,
                mips: 1,
                array: false,
            })
        };
        let mut renderer = Renderer {
            output: match surface {
                Some(surface) => Output::Window(Swapchain {
                    surface,
                    swapchain: vk::SwapchainKHR::null(),
                    views: Vec::new(),
                    vsync,
                }),
                None => Output::Headless {
                    image: placeholder(&gpu)?,
                    readback: gpu.host_buffer(16, U::TRANSFER_DST)?,
                },
            },
            hdr: placeholder(&gpu)?,
            depth: placeholder(&gpu)?,
            bloom: Vec::new(),
            bloom_fbs: Vec::new(),
            bloom_sets,
            hdr_set,
            present_format,
            width,
            height,
            passes,
            layouts,
            pipelines,
            shadow,
            scene_fb: vk::Framebuffer::null(),
            shadow_fb,
            present_fbs: Vec::new(),
            descriptor_pool,
            scene_set,
            cull_set,
            screen_set,
            nodes_set,
            marks_set,
            ranges_set,
            sprites_set,
            stains_set,
            puffs_set,
            samplers,
            globals,
            dynamic,
            statics,
            model_table,
            slot_table,
            vis,
            counters,
            commands,
            visible,
            props_dead,
            nodes,
            marks,
            ranges,
            range_ib,
            projectiles,
            effects,
            stains,
            puffs,
            beams,
            track_marks,
            overlay_vb,
            mesh_vb,
            mesh_ib,
            quad_vb,
            quad_ib,
            grid_vb,
            grid_ib,
            grid_index_count: grid_i.len() as u32,
            patch_vb,
            patch_ib,
            patch_index_count: patch_i.len() as u32,
            overview,
            tiles,
            tile_index,
            fog,
            fog_dims,
            noise,
            panel,
            font,
            font_uploaded: false,
            cmd,
            fence,
            image_available,
            render_finished,
            query_pool,
            timestamp_period,
            queries_valid: false,
            garbage: Vec::new(),
            pool: scene.pool.clone(),
            tile_cache,
            node_scratch: Vec::new(),
            upload_scratch: Vec::new(),
            map_info: info,
            palette: [
                rgba(faction.plating_color),
                rgba(faction.accent_color),
                rgba(faction.highlight_color),
            ],
            team_colors,
            slot_count,
            static_count,
            dynamic_count: 0,
            sim_units: 0,
            projectile_count: 0,
            stain_count: 0,
            pad_count: 0,
            deposit_splats,
            deposit_count: 0,
            deposit_first: 0,
            effect_cursor: 0,
            puff_cursor: 0,
            beam_count: 0,
            beams_live: Default::default(),
            beams_ended: Vec::new(),
            track_cursor: 0,
            track_count: 0,
            scatter: Scatter(0x9E37_79B9),
            blueprints: scene.blueprints.clone(),
            treads,
            legs,
            tick_seconds: 0.1,
            last_tick_time: 0.0,
            fog_enabled: false,
            last_time: 0.0,
            stats: FrameStats::default(),
            gpu,
        };
        renderer.create_size_dependent()?;
        log::info!(
            "renderer ready on {}: {} models, {} draw slots, {} mesh vertices, {} props",
            renderer.gpu.device_name,
            model_list.len(),
            slot_count,
            vertices.len(),
            static_count
        );
        Ok(renderer)
    }

    /// Brings the GPU copy of the overlay's atlas up to date: all of it the first
    /// time this renderer sees it, afterwards only the rows with new glyphs or images.
    fn upload_overlay_atlas(&mut self, overlay: &Overlay) -> Result<(), GpuError> {
        let dirty = overlay.take_dirty_rows();
        let rows = if self.font_uploaded {
            dirty
        } else {
            Some(0..textures::FONT_ATLAS_H)
        };
        let Some(rows) = rows else { return Ok(()) };
        let region = vk::Rect2D {
            offset: vk::Offset2D {
                x: 0,
                y: rows.start as i32,
            },
            extent: vk::Extent2D {
                width: textures::FONT_ATLAS_W as u32,
                height: rows.len() as u32,
            },
        };
        let bytes = &overlay.atlas()
            [rows.start * textures::FONT_ATLAS_W * 4..rows.end * textures::FONT_ATLAS_W * 4];
        self.gpu
            .upload_image(&self.font, 0, 0, Some(region), bytes, !self.font_uploaded)?;
        self.font_uploaded = true;
        Ok(())
    }

    pub fn device_name(&self) -> &str {
        &self.gpu.device_name
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Cursor ray against the terrain (overview resolution).
    pub fn pick_ground(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        self.tile_cache.pick(origin, dir)
    }

    pub fn ground_height(&self, xy: glam::Vec2) -> f32 {
        self.tile_cache.overview_height(xy)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), GpuError> {
        if (width, height) == (self.width, self.height) || width == 0 || height == 0 {
            return Ok(());
        }
        self.width = width;
        self.height = height;
        self.create_size_dependent()
    }

    /// (Re)creates everything that depends on the output size.
    fn create_size_dependent(&mut self) -> Result<(), GpuError> {
        self.gpu.wait_idle();
        let device = &self.gpu.device;
        unsafe {
            for fb in self.present_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            if self.scene_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(self.scene_fb, None);
            }
            for fb in self.bloom_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
        }
        for image in self.bloom.drain(..) {
            self.gpu.destroy_image(image);
        }
        let mut present_views: Vec<vk::ImageView> = Vec::new();
        match &mut self.output {
            Output::Window(sc) => {
                let swapchain_fn = self
                    .gpu
                    .swapchain_fn
                    .as_ref()
                    .expect("window target has the swapchain extension");
                let caps = unsafe {
                    self.gpu
                        .surface_fn
                        .get_physical_device_surface_capabilities(self.gpu.physical, sc.surface)
                }?;
                if caps.current_extent.width != u32::MAX {
                    self.width = caps.current_extent.width.max(1);
                    self.height = caps.current_extent.height.max(1);
                }
                let modes = unsafe {
                    self.gpu
                        .surface_fn
                        .get_physical_device_surface_present_modes(self.gpu.physical, sc.surface)
                }?;
                let mode = if !sc.vsync && modes.contains(&vk::PresentModeKHR::MAILBOX) {
                    vk::PresentModeKHR::MAILBOX
                } else if !sc.vsync && modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
                    vk::PresentModeKHR::IMMEDIATE
                } else {
                    vk::PresentModeKHR::FIFO
                };
                let mut count = caps.min_image_count + 1;
                if caps.max_image_count > 0 {
                    count = count.min(caps.max_image_count);
                }
                let old = sc.swapchain;
                let info = vk::SwapchainCreateInfoKHR::default()
                    .surface(sc.surface)
                    .min_image_count(count)
                    .image_format(self.present_format)
                    .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                    .image_extent(vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    })
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(caps.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(mode)
                    .clipped(true)
                    .old_swapchain(old);
                sc.swapchain = unsafe { swapchain_fn.create_swapchain(&info, None) }?;
                unsafe {
                    for v in sc.views.drain(..) {
                        device.destroy_image_view(v, None);
                    }
                    if old != vk::SwapchainKHR::null() {
                        swapchain_fn.destroy_swapchain(old, None);
                    }
                }
                for image in unsafe { swapchain_fn.get_swapchain_images(sc.swapchain) }? {
                    let view_info = vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.present_format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    sc.views
                        .push(unsafe { device.create_image_view(&view_info, None) }?);
                }
                present_views.extend_from_slice(&sc.views);
            }
            Output::Headless { image, readback } => {
                let new_image = self.gpu.image(&ImageDesc {
                    width: self.width,
                    height: self.height,
                    format: self.present_format,
                    usage: vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_SRC,
                    layers: 1,
                    mips: 1,
                    array: false,
                })?;
                let new_readback = self.gpu.host_buffer(
                    self.width as u64 * self.height as u64 * 4,
                    vk::BufferUsageFlags::TRANSFER_DST,
                )?;
                self.gpu.destroy_image(std::mem::replace(image, new_image));
                self.gpu
                    .destroy_buffer(std::mem::replace(readback, new_readback));
                present_views.push(image.view);
            }
        }

        let attachment = |format, usage| ImageDesc {
            width: self.width,
            height: self.height,
            format,
            usage,
            layers: 1,
            mips: 1,
            array: false,
        };
        let hdr = self.gpu.image(&attachment(
            HDR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ))?;
        let depth = self.gpu.image(&attachment(
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        ))?;
        self.gpu
            .destroy_image(std::mem::replace(&mut self.hdr, hdr));
        self.gpu
            .destroy_image(std::mem::replace(&mut self.depth, depth));

        let device = &self.gpu.device;
        let views = [self.hdr.view, self.depth.view];
        self.scene_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.passes.scene)
                    .attachments(&views)
                    .width(self.width)
                    .height(self.height)
                    .layers(1),
                None,
            )
        }?;
        for view in present_views {
            let views = [view];
            let fb = unsafe {
                device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.passes.present)
                        .attachments(&views)
                        .width(self.width)
                        .height(self.height)
                        .layers(1),
                    None,
                )
            }?;
            self.present_fbs.push(fb);
        }
        for level in 0..BLOOM_LEVELS {
            let (w, h) = (
                (self.width >> (level + 1)).max(1),
                (self.height >> (level + 1)).max(1),
            );
            let image = self.gpu.image(&ImageDesc {
                width: w,
                height: h,
                format: HDR_FORMAT,
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                layers: 1,
                mips: 1,
                array: false,
            })?;
            let views = [image.view];
            let fb = unsafe {
                device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.passes.bloom_down)
                        .attachments(&views)
                        .width(w)
                        .height(h)
                        .layers(1),
                    None,
                )
            }?;
            self.bloom.push(image);
            self.bloom_fbs.push(fb);
        }
        // Binding 0 is what a screen pass reads, binding 3 the finished bloom for the tone mapper.
        let write_view = |set: vk::DescriptorSet, binding: u32, view: vk::ImageView| {
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
            unsafe { device.update_descriptor_sets(&write, &[]) };
        };
        write_view(self.screen_set, 0, self.hdr.view);
        write_view(self.screen_set, 3, self.bloom[0].view);
        write_view(self.hdr_set, 0, self.hdr.view);
        write_view(self.hdr_set, 3, self.hdr.view);
        for (set, image) in self.bloom_sets.iter().zip(&self.bloom) {
            write_view(*set, 0, image.view);
            write_view(*set, 3, self.hdr.view);
        }
        self.queries_valid = false;
        Ok(())
    }

    /// Copies a new tick's mirror into GPU memory. Bulk copies only.
    fn upload_sim(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let units = &frame.units[..frame.units.len().min(MAX_DYNAMIC - 512)];
        if units.len() < frame.units.len() {
            log::error!(
                "render mirror has {} entities; the renderer holds {}",
                frame.units.len(),
                units.len()
            );
        }
        self.dynamic.write(0, bytemuck::cast_slice(units));
        self.sim_units = units.len() as u32;

        let projectiles = &frame.projectiles[..frame.projectiles.len().min(MAX_PROJECTILES)];
        self.projectiles.write(0, bytemuck::cast_slice(projectiles));
        self.projectile_count = projectiles.len() as u32;

        self.upload_beams(frame, time);

        let stains = &frame.stains[..frame.stains.len().min(MAX_STAINS)];
        self.stains.write(0, bytemuck::cast_slice(stains));
        self.stain_count = stains.len() as u32;
        let pads = self.structure_pads(units, &frame.pads, MAX_STAINS.saturating_sub(stains.len()));
        if !pads.is_empty() {
            self.stains.write(
                (stains.len() * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&pads),
            );
        }
        self.pad_count = pads.len() as u32;
        let used = stains.len() + pads.len();
        let n = self
            .deposit_splats
            .len()
            .min(MAX_STAINS.saturating_sub(used));
        if n > 0 {
            self.stains.write(
                (used * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&self.deposit_splats[..n]),
            );
        }
        self.deposit_first = used as u32;
        self.deposit_count = n as u32;

        let dead_bytes: &[u8] = bytemuck::cast_slice(&frame.props_dead);
        self.props_dead.write(
            0,
            &dead_bytes[..dead_bytes.len().min(self.props_dead.size as usize)],
        );

        // Units glide from the last tick's place to this one's over the coming
        // tick, so what a tick reports is timed against that stretch.
        let since = time - self.last_tick_time;
        if since > 0.0 && since < 1.0 {
            self.tick_seconds = self.tick_seconds * 0.7 + since * 0.3;
        }
        self.last_tick_time = time;

        self.ground_contact(units, time, camera);
        self.construction(frame, time, camera);
        for event in &frame.events {
            self.effects_of(event, time);
        }
    }

    /// Reclaim beams have a beginning and an end the sim does not tell of: it only lists
    /// who is at work each tick. A beam new to the list starts empty and fills from the
    /// target; one that has left it stops tearing bits loose and lets those in flight arrive.
    fn upload_beams(&mut self, frame: &RenderFrame, time: f32) {
        let mut was = std::mem::take(&mut self.beams_live);
        let shut_off = |mut old: GpuBeam, ended: &mut Vec<GpuBeam>| {
            (old.end, old.beam.to_prev) = (time, old.beam.to);
            ended.push(old);
        };
        for (source, beam) in frame.beam_sources.iter().zip(&frame.beams) {
            let mut start = time;
            if let Some(old) = was.remove(source) {
                let jump = Vec3::from(old.beam.to).distance(Vec3::from(beam.to_prev));
                if jump <= BEAM_JUMP + beam.radius {
                    start = old.start;
                } else {
                    shut_off(old, &mut self.beams_ended);
                }
            }
            self.beams_live.insert(
                *source,
                GpuBeam {
                    beam: *beam,
                    start,
                    end: -1.0,
                    pad: [0.0; 2],
                },
            );
        }
        for (_, old) in was {
            shut_off(old, &mut self.beams_ended);
        }
        self.beams_ended.retain(|b| time - b.end < BEAM_LINGER);
        let all: Vec<GpuBeam> = self
            .beams_live
            .values()
            .chain(&self.beams_ended)
            .copied()
            .take(MAX_BEAMS)
            .collect();
        self.beams.write(0, bytemuck::cast_slice(&all));
        self.beam_count = all.len() as u32;
    }

    fn push_effect(
        &mut self,
        pos: [f32; 3],
        start: f32,
        radius: f32,
        life: f32,
        kind: f32,
        ring: f32,
    ) {
        let e = Effect {
            pos,
            start,
            params: [radius, life, kind, ring],
        };
        self.effects.write(
            (self.effect_cursor * size_of::<Effect>()) as u64,
            bytemuck::bytes_of(&e),
        );
        self.effect_cursor = (self.effect_cursor + 1) % MAX_EFFECTS;
    }

    fn push_puff(
        &mut self,
        kind: f32,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: (f32, f32),
    ) {
        let seed = self.scatter.unit();
        let p = Puff {
            pos: pos.to_array(),
            start,
            vel: vel.to_array(),
            life,
            params: [size.0, size.1, kind, seed],
        };
        self.puffs.write(
            (self.puff_cursor * size_of::<Puff>()) as u64,
            bytemuck::bytes_of(&p),
        );
        self.puff_cursor = (self.puff_cursor + 1) % MAX_PUFFS;
    }

    fn push_mark(&mut self, mark: TrackMark) {
        self.track_marks.write(
            (self.track_cursor * size_of::<TrackMark>()) as u64,
            bytemuck::bytes_of(&mark),
        );
        self.track_cursor = (self.track_cursor + 1) % MAX_TRACK_MARKS;
        self.track_count = (self.track_count + 1).min(MAX_TRACK_MARKS as u32);
    }

    /// Ground lots for structures: persisted pours first, then any live site,
    /// ghost or wreck that is not already on that centre. Death does not
    /// remove a lot; the scorch stain is drawn on top of it.
    fn structure_pads(
        &self,
        units: &[UnitInstance],
        persisted: &[StainInstance],
        room: usize,
    ) -> Vec<StainInstance> {
        let mut pads = persisted.iter().copied().take(room).collect::<Vec<_>>();
        for u in units {
            if pads.len() >= room || u.owner_flags & (KIND_PROP | STATE_RADAR) != 0 {
                continue;
            }
            let Some(bp) = self.blueprints.units.get(u.blueprint as usize) else {
                continue;
            };
            if !bp.is_structure() || bp.has(cat::WALL) {
                continue;
            }
            let pos = [u.pos[0], u.pos[1]];
            if pads
                .iter()
                .any(|p| (p.pos[0] - pos[0]).abs() < 0.5 && (p.pos[1] - pos[1]).abs() < 0.5)
            {
                continue;
            }
            let half = bp.footprint.0.max(bp.footprint.1) as f32 * (BUILD_CELL_M as f32 * 0.5);
            // Same seed as World::remember_structure_pad, so the plan does not
            // jump when the pour is recorded.
            let seed = (u.pos[0].round() as i32 as u32).wrapping_mul(7)
                ^ (u.pos[1].round() as i32 as u32).wrapping_mul(13);
            let ghost = u.owner_flags & KIND_GHOST != 0;
            let build = if u.owner_flags & KIND_WRECK != 0 {
                255
            } else {
                (u.build.clamp(0.0, 1.0) * 255.0) as u32
            };
            pads.push(StainInstance {
                pos,
                radius: half,
                strength_seed: mc_sim::pack_structure_pad(
                    (u.owner_flags & 7) as u8,
                    build as u8,
                    seed,
                    ghost,
                    bp.needs_deposit,
                ),
            });
        }
        pads
    }

    /// Track marks and dust for the tracked vehicles that moved this tick,
    /// and the prints a walker leaves as each foot comes down, near enough
    /// to the camera for either to be seen.
    fn ground_contact(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        const FLAG_MOVING: u32 = (mc_sim::tables::flag::MOVING as u32) << 8;
        if camera.distance > 2500.0 {
            return;
        }
        let reach = camera.distance * 2.5 + 300.0;
        let dust = camera.distance < 900.0;
        let focus = camera.focus.truncate();
        let water = self.map_info.water_level.to_f32();
        for u in units {
            if u.owner_flags & FLAG_MOVING == 0 || u.owner_flags & (KIND_WRECK | STATE_RADAR) != 0 {
                continue;
            }
            if let Some(legs) = self.legs.get(u.blueprint as usize).copied().flatten() {
                let close = Vec3::from(u.pos).truncate().distance(focus) <= reach;
                self.footfall(u, &legs, time, close, dust && close);
                continue;
            }
            let Some(treads) = self.treads.get(u.blueprint as usize).copied().flatten() else {
                continue;
            };
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            let moved = to.truncate().distance(from.truncate());
            if !(0.05..=40.0).contains(&moved)
                || to.truncate().distance(focus) > reach
                || to.z < water
            {
                continue;
            }
            // Laid as the hull passes over it: partway through this tick's glide.
            self.push_mark(TrackMark {
                from: [from.x, from.y],
                to: [to.x, to.y],
                half_gauge: treads.half_gauge,
                width: treads.width,
                start: time + self.tick_seconds * 0.5,
                life: TRACK_MARK_LIFE,
            });

            if !dust {
                continue;
            }
            let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
            let left = Vec3::new(-forward.y, forward.x, 0.0);
            let speed = moved / self.tick_seconds.max(0.02);
            for side in [-1.0f32, 1.0] {
                // Born somewhere along this tick's glide, behind the track that threw it.
                let k = self.scatter.unit();
                let at = from.lerp(to, k)
                    + forward * (treads.rear + 0.4)
                    + left
                        * (side * treads.half_gauge + self.scatter.signed() * treads.width * 0.3)
                    + Vec3::Z * 0.35;
                let vel = forward * (-speed * 0.12)
                    + left * (side * 0.8 + self.scatter.signed() * 0.6)
                    + Vec3::Z * (1.2 + self.scatter.unit() * 1.4);
                let grow = 1.8 + self.scatter.unit() * 1.6 + speed * 0.03;
                let life = 1.1 + self.scatter.unit() * 0.9;
                self.push_puff(
                    PUFF_DUST,
                    at,
                    vel,
                    time + k * self.tick_seconds,
                    life,
                    (0.7, grow),
                );
            }
        }
    }

    /// A walker's foot coming down: a print the size of the sole, and dust from
    /// under it. The shader plants the left foot as the stride's cycle wraps
    /// and the right half a cycle later; this keeps the same time.
    fn footfall(&mut self, u: &UnitInstance, legs: &Legs, time: f32, mark: bool, dust: bool) {
        let cycle = |ground: f32| (ground / legs.stride * 2.0).floor();
        let (before, now) = (cycle(u.gait[0] - u.gait[1]), cycle(u.gait[0]));
        if u.gait[1] <= 0.0 || before == now || u.pos[2] < self.map_info.water_level.to_f32() {
            return;
        }
        let side = if now.rem_euclid(2.0) < 1.0 { 1.0 } else { -1.0 };
        let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        // Set down at the front of its reach.
        let plant = Vec3::from(u.pos)
            + forward * (legs.ankle[0] + legs.stride * legs.stance * 0.5)
            + left * (side * legs.ankle[1]);
        if mark && legs.foot[2] > 0.0 {
            let heel = [
                plant.x + forward.x * legs.foot[0],
                plant.y + forward.y * legs.foot[0],
            ];
            let toe = [
                plant.x + forward.x * legs.foot[1],
                plant.y + forward.y * legs.foot[1],
            ];
            self.push_mark(TrackMark {
                from: heel,
                to: toe,
                half_gauge: 0.0,
                width: legs.foot[2],
                start: time + self.tick_seconds * 0.5,
                life: TRACK_MARK_LIFE,
            });
        }
        if !dust {
            return;
        }
        let at = plant + Vec3::Z * 0.3;
        let weight = legs.hip[2];
        for _ in 0..4 {
            let out =
                Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.15).normalize_or_zero();
            let life = 0.9 + self.scatter.unit() * 0.7;
            self.push_puff(
                PUFF_DUST,
                at + out * weight * 0.12,
                out * (2.0 + weight * 0.5) + Vec3::Z * 0.8,
                time + self.tick_seconds * 0.5,
                life,
                (weight * 0.12, weight * 0.42),
            );
        }
    }

    /// Construction seen at work: a glow at the emitter, a hotter one at the
    /// weld, sparks off the print, and the welding that goes on all over a unit being refitted.
    fn construction(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        if camera.distance > 1800.0 {
            return;
        }
        let reach = camera.distance * 2.5 + 300.0;
        let focus = camera.focus.truncate();
        for p in frame
            .projectiles
            .iter()
            .filter(|p| p.color & mc_sim::mirror::PROJECTILE_BEAM != 0)
        {
            let (from, to) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
            if to.truncate().distance(focus) > reach {
                continue;
            }
            let along = (to - from).normalize_or_zero();
            let side = along.cross(Vec3::Z).normalize_or_zero();
            let up = along.cross(side).normalize_or_zero();
            self.push_effect(
                from.to_array(),
                time,
                1.8,
                self.tick_seconds * 1.6,
                3.0,
                0.0,
            );
            let flare = 2.8 + self.scatter.unit() * 0.8;
            self.push_effect(
                to.to_array(),
                time,
                flare,
                self.tick_seconds * 1.8,
                3.0,
                0.15,
            );
            for _ in 0..5 {
                let spray = (side * self.scatter.signed()
                    + up * self.scatter.signed()
                    + along * (self.scatter.unit() * 0.35))
                    .normalize_or_zero();
                let spot = to + spray * 0.35;
                let vel = spray * (5.0 + self.scatter.unit() * 11.0)
                    + Vec3::Z * (1.5 + self.scatter.unit() * 4.0);
                let (start, life) = (
                    time + self.scatter.unit() * self.tick_seconds,
                    0.28 + self.scatter.unit() * 0.5,
                );
                self.push_puff(PUFF_SPARK, spot, vel, start, life, (0.2, 0.05));
            }
        }
        for u in frame
            .units
            .iter()
            .filter(|u| u.upgrade > 0.0 && u.owner_flags & KIND_WRECK == 0)
        {
            let at = Vec3::from(u.pos);
            if at.truncate().distance(focus) > reach {
                continue;
            }
            let bp = self
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16));
            let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
            // Most of the work is on the build arm, which is what a new engineering suite changes.
            let facing = u.heading + u.turret_yaw;
            let arm = bp.builder.as_ref().and_then(|b| b.arm).map(|a| {
                let e = a.emitter.to_f32();
                at + Vec3::new(
                    e[0] * facing.cos() - e[1] * facing.sin(),
                    e[0] * facing.sin() + e[1] * facing.cos(),
                    e[2],
                )
            });
            for i in 0..5 {
                let on_arm = arm.filter(|_| i < 3);
                let spot = match on_arm {
                    Some(arm) => {
                        arm + Vec3::new(
                            self.scatter.signed() - 1.0,
                            self.scatter.signed(),
                            self.scatter.signed(),
                        ) * 0.9
                    }
                    None => {
                        let a = self.scatter.unit() * std::f32::consts::TAU;
                        at + Vec3::new(
                            a.cos() * r * 0.5,
                            a.sin() * r * 0.5,
                            h * (0.35 + 0.6 * self.scatter.unit()),
                        )
                    }
                };
                let start = time + self.scatter.unit() * self.tick_seconds;
                if i % 2 == 0 {
                    let (size, life) = (
                        0.8 + self.scatter.unit() * 1.2,
                        0.12 + self.scatter.unit() * 0.1,
                    );
                    self.push_effect(spot.to_array(), start, size, life, 3.0, 0.0);
                }
                for _ in 0..2 {
                    let vel = self.scatter.upward(0.0) * (3.0 + self.scatter.unit() * 8.0);
                    let life = 0.3 + self.scatter.unit() * 0.5;
                    self.push_puff(PUFF_SPARK, spot, vel, start, life, (0.14, 0.04));
                }
            }
        }
    }

    /// A commander's reactor going up: a flash that whites the screen out, a fireball
    /// that climbs into a cloud on a stalk, a shock front across the ground out to the
    /// edge of the blast, and a column of smoke that stands for a long time.
    fn reactor_death(&mut self, at: Vec3, r: f32, h: f32, time: f32) {
        let core = at + Vec3::Z * h * 0.5;
        const BLAST: f32 = 140.0;
        // The flash: twice, the second broader and slower, so it blinds and then lingers.
        self.push_effect(core.to_array(), time, 420.0, 0.55, 4.0, 0.0);
        self.push_effect(core.to_array(), time + 0.05, 260.0, 1.9, 4.0, 1.0);
        // The shock front along the ground, and a second behind it.
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.05,
            BLAST * 1.25,
            1.1,
            2.0,
            1.0,
        );
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.35,
            BLAST * 0.9,
            1.4,
            2.0,
            1.0,
        );
        // The fireball, rolling upward.
        for i in 0..7 {
            let rise = i as f32 * 7.0;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 1.2
                + Vec3::Z * rise;
            self.push_effect(
                (core + off).to_array(),
                time + 0.12 + 0.16 * i as f32,
                r * (7.0 - i as f32 * 0.5),
                1.6 + 0.2 * i as f32,
                2.0,
                0.0,
            );
        }
        for i in 0..26 {
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            ) * r
                * 2.2;
            let vel = self.scatter.upward(0.3) * (14.0 + self.scatter.unit() * 22.0);
            let life = 2.2 + self.scatter.unit() * 1.6;
            self.push_puff(
                PUFF_FIREBALL,
                core + off,
                vel,
                time + 0.04 * i as f32,
                life,
                (r * 1.6, r * 4.5),
            );
        }
        // The stalk, growing at forty metres a second, and the cloud it carries up.
        for i in 0..22 {
            let z = 4.0 + i as f32 * 3.2;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.7
                + Vec3::Z * z;
            let kind = if i % 3 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 5.0 + self.scatter.unit() * 3.0;
            let climb = Vec3::Z * (6.0 + self.scatter.unit() * 5.0);
            self.push_puff(
                kind,
                at + off,
                climb,
                time + 0.3 + z / 40.0,
                life,
                (r * 1.4, r * 3.2),
            );
        }
        for i in 0..28 {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 28.0;
            let ring = Vec3::new(a.cos(), a.sin(), 0.0) * r * (2.0 + self.scatter.unit() * 2.4);
            let top = at + ring + Vec3::Z * (72.0 + self.scatter.signed() * 7.0);
            let roll = ring.normalize_or_zero() * (5.0 + self.scatter.unit() * 5.0)
                + Vec3::Z * (2.0 + self.scatter.unit() * 3.0);
            let kind = if i % 4 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 6.0 + self.scatter.unit() * 3.5;
            let start = time + 2.0 + self.scatter.unit() * 0.5;
            self.push_puff(kind, top, roll, start, life, (r * 2.4, r * 6.0));
        }
        // Dust driven out flat ahead of the shock, all the way to the edge of the blast.
        for ring in 0..3 {
            for i in 0..30 {
                let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 30.0;
                let out = Vec3::new(a.cos(), a.sin(), 0.03);
                let from = BLAST * (0.12 + 0.3 * ring as f32);
                let life = 2.2 + self.scatter.unit() * 1.6;
                let push = out * (60.0 + self.scatter.unit() * 30.0);
                self.push_puff(
                    PUFF_DUST,
                    at + out * from + Vec3::Z * 0.6,
                    push,
                    time + 0.05 + from / 170.0,
                    life,
                    (r * 1.2, r * 4.2),
                );
            }
        }
        // Burning fragments thrown a long way, and earth with them.
        for _ in 0..110 {
            let vel = self.scatter.upward(0.1) * (25.0 + self.scatter.unit() * 70.0);
            let life = 1.0 + self.scatter.unit() * 2.2;
            let (start, size) = (
                time + self.scatter.unit() * 0.1,
                0.5 + self.scatter.unit() * 0.5,
            );
            self.push_puff(PUFF_SPARK, core, vel, start, life, (size, 0.1));
        }
        for _ in 0..40 {
            let vel = self.scatter.upward(0.3) * (18.0 + self.scatter.unit() * 35.0);
            let life = 1.6 + self.scatter.unit() * 1.4;
            let size = 0.5 + self.scatter.unit() * 0.6;
            self.push_puff(PUFF_CLOD, core, vel, time + 0.05, life, (size, 0.3));
        }
        // Fire in the crater and smoke over it, for a long while after.
        for i in 0..30 {
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 2.5
                + Vec3::Z * 1.5;
            let vel = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                3.0 + self.scatter.unit() * 2.5,
            );
            let start = time + 1.2 + i as f32 * 0.35 + self.scatter.unit() * 0.2;
            let kind = if i % 2 == 0 { PUFF_FIRE } else { PUFF_SMOKE };
            let life = 2.5 + self.scatter.unit() * 2.0;
            self.push_puff(kind, at + off, vel, start, life, (r * 0.8, r * 2.6));
        }
    }

    /// The flashes, smoke and debris of one sim event.
    fn effects_of(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::ShotFired {
                pos,
                vel,
                color,
                blueprint,
                weapon,
                ..
            } => {
                let weapon = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let flash = weapon.flash;
                let at = Vec3::from(pos.to_f32());
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                let shell = *color == mc_data::WeaponColor::Orange;
                self.push_effect(
                    at.to_array(),
                    time,
                    (1.2 + power * 0.42) * flash,
                    if shell { 0.09 } else { 0.12 },
                    *color as u32 as f32,
                    0.0,
                );
                if !shell {
                    return;
                }
                // A gun: smoke blown out along the bore and a few sparks ahead of it.
                for i in 0..3 {
                    let push = dir * (5.0 + 4.0 * i as f32 + power)
                        + Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.unit() * 1.5,
                        );
                    self.push_puff(
                        PUFF_SMOKE,
                        at + dir * 0.4,
                        push,
                        time + 0.02 * i as f32,
                        0.7 + 0.25 * i as f32,
                        (0.5 + power * 0.06, 1.6 + power * 0.28),
                    );
                }
                for _ in 0..4 {
                    let spray = (dir * 1.4
                        + Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.signed(),
                        ) * 0.5)
                        .normalize_or_zero();
                    let (speed, life) = (
                        14.0 + self.scatter.unit() * 16.0,
                        0.12 + self.scatter.unit() * 0.12,
                    );
                    self.push_puff(PUFF_SPARK, at, spray * speed, time, life, (0.16, 0.05));
                }
            }
            SimEvent::Impact {
                pos,
                splash,
                color,
                after,
                on_unit,
                blueprint,
                weapon,
            } => {
                let weapon = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let flash = weapon.flash;
                let at = Vec3::from(pos.to_f32());
                let start = time + after.to_f32() * self.tick_seconds;
                let splash = splash.to_f32();
                let shell = *color == mc_data::WeaponColor::Orange;
                let radius = (1.4 + power * 0.4 + splash * 1.2) * flash;
                let (life, ring) = if splash > 0.0 {
                    (0.45, 1.0)
                } else {
                    (0.22, 0.0)
                };
                self.push_effect(
                    at.to_array(),
                    start,
                    radius,
                    life,
                    *color as u32 as f32,
                    ring,
                );
                if !shell {
                    return;
                }
                // Sparks off armour or a burst of earth off the ground, and the smoke that hangs after.
                let (sparks, clods) = if *on_unit { (12, 2) } else { (5, 8) };
                let reach = 1.0 + splash * 0.12;
                for _ in 0..sparks {
                    let vel =
                        self.scatter.upward(0.15) * (8.0 + self.scatter.unit() * 14.0) * reach;
                    let life = 0.25 + self.scatter.unit() * 0.35;
                    self.push_puff(
                        PUFF_SPARK,
                        at + Vec3::Z * 0.2,
                        vel,
                        start,
                        life,
                        (0.2, 0.05),
                    );
                }
                for _ in 0..clods {
                    let vel = self.scatter.upward(0.45) * (6.0 + self.scatter.unit() * 9.0) * reach;
                    let life = 0.7 + self.scatter.unit() * 0.6;
                    self.push_puff(
                        PUFF_CLOD,
                        at + Vec3::Z * 0.2,
                        vel,
                        start,
                        life,
                        (0.12 + power * 0.025, 0.08),
                    );
                }
                for i in 0..2 + (splash > 0.0) as usize * 3 {
                    let vel =
                        self.scatter.upward(0.5) * (1.5 + self.scatter.unit() * 2.5 + splash * 0.2);
                    let kind = if *on_unit || i % 2 == 0 {
                        PUFF_SMOKE
                    } else {
                        PUFF_DUST
                    };
                    let life = 0.9 + self.scatter.unit() * 0.8;
                    self.push_puff(
                        kind,
                        at + Vec3::Z * 0.4,
                        vel,
                        start + 0.03,
                        life,
                        (0.6 + power * 0.08, 1.8 + power * 0.3 + splash * 0.35),
                    );
                }
            }
            SimEvent::UnitDied { pos, blueprint, .. } => {
                // A unit going up is an event, not a big impact: the detonation, secondary blasts
                // walking across the hull, burning fragments thrown wide, a ring of dust along the
                // ground, and fire that turns into a column of black smoke over the wreck.
                let at = Vec3::from(pos.to_f32());
                let bp = self.blueprints.unit(*blueprint);
                let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
                if bp.has(mc_data::cat::COMMANDER) {
                    self.reactor_death(at, r, h, time);
                    return;
                }
                let core = at + Vec3::Z * h * 0.45;
                self.push_effect(core.to_array(), time, r * 1.6, 0.16, 1.0, 0.0);
                self.push_effect(core.to_array(), time, r * 3.4, 1.0, 2.0, 1.0);
                for i in 0..3 {
                    let off = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit() * 0.6,
                    ) * r
                        * 0.7;
                    let delay = 0.08 + 0.11 * i as f32 + self.scatter.unit() * 0.06;
                    let size = r * (1.2 + self.scatter.unit() * 0.9);
                    self.push_effect((core + off).to_array(), time + delay, size, 0.5, 2.0, 0.0);
                }
                for _ in 0..26 {
                    let vel = self.scatter.upward(0.12)
                        * (9.0 + self.scatter.unit() * 22.0)
                        * (0.7 + r * 0.06);
                    let life = 0.5 + self.scatter.unit() * 1.1;
                    self.push_puff(PUFF_SPARK, core, vel, time, life, (0.16 + r * 0.03, 0.05));
                }
                for _ in 0..10 {
                    let vel = self.scatter.upward(0.35) * (7.0 + self.scatter.unit() * 11.0);
                    let life = 0.9 + self.scatter.unit() * 0.8;
                    self.push_puff(PUFF_CLOD, core, vel, time, life, (0.18 + r * 0.04, 0.12));
                }
                // The dust ring: pushed out flat from the foot of the blast.
                for i in 0..12 {
                    let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 12.0;
                    let out = Vec3::new(a.cos(), a.sin(), 0.06);
                    let life = 1.2 + self.scatter.unit() * 0.8;
                    self.push_puff(
                        PUFF_DUST,
                        at + out * r * 0.6 + Vec3::Z * 0.4,
                        out * (9.0 + r * 1.2),
                        time + 0.03,
                        life,
                        (r * 0.3, r * 0.95),
                    );
                }
                // The fireball, then fire licking out of the wreck for a few seconds, smoke above it.
                for i in 0..8 {
                    let off = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit(),
                    ) * r
                        * 0.45;
                    let vel = self.scatter.upward(0.5) * (3.0 + self.scatter.unit() * 5.0);
                    let life = 0.9 + self.scatter.unit() * 0.7;
                    self.push_puff(
                        PUFF_FIRE,
                        core + off,
                        vel,
                        time + 0.02 * i as f32,
                        life,
                        (r * 0.45, r * 1.3),
                    );
                }
                for i in 0..14 {
                    let off =
                        Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.4
                            + Vec3::Z * h * 0.3;
                    let vel = Vec3::new(
                        self.scatter.signed() * 0.6,
                        self.scatter.signed() * 0.6,
                        2.2 + self.scatter.unit() * 1.6,
                    );
                    let start = time + 0.5 + i as f32 * 0.28 + self.scatter.unit() * 0.15;
                    let life = 1.3 + self.scatter.unit() * 0.6;
                    self.push_puff(PUFF_FIRE, at + off, vel, start, life, (r * 0.22, r * 0.75));
                }
                for i in 0..12 {
                    let off =
                        Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.4
                            + Vec3::Z * h * 0.5;
                    let vel = Vec3::new(
                        self.scatter.signed() * 0.8 + 0.9,
                        self.scatter.signed() * 0.8,
                        3.0 + self.scatter.unit() * 2.0,
                    );
                    let start = time + 0.15 + i as f32 * 0.35;
                    let life = 3.0 + self.scatter.unit() * 1.5;
                    self.push_puff(PUFF_SMOKE, at + off, vel, start, life, (r * 0.4, r * 1.7));
                }
            }
            SimEvent::Reclaimed {
                pos,
                blueprint,
                wreck,
            } => {
                // The last of it goes up the beam: a soft flare and a few embers, no blast, no smoke.
                let bp = self.blueprints.unit(*blueprint);
                let (r, h) = (
                    bp.radius.to_f32(),
                    bp.height.to_f32() * if *wreck { 0.25 } else { 0.5 },
                );
                let core = Vec3::from(pos.to_f32()) + Vec3::Z * h;
                self.push_effect(core.to_array(), time, r * 1.3, 0.3, 1.0, 0.0);
                for _ in 0..10 {
                    let vel = self.scatter.upward(0.6) * (2.0 + self.scatter.unit() * 4.0);
                    let off =
                        Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.6;
                    let life = 0.4 + self.scatter.unit() * 0.5;
                    self.push_puff(
                        PUFF_SPARK,
                        core + off,
                        vel,
                        time,
                        life,
                        (0.14 + r * 0.02, 0.04),
                    );
                }
            }
            _ => {}
        }
    }

    /// Renders one frame. Returns `false` if the swapchain had to be rebuilt and the frame was skipped.
    pub fn render(&mut self, input: &FrameInput) -> Result<bool, GpuError> {
        let device = self.gpu.device.clone();
        unsafe {
            device.wait_for_fences(&[self.fence], true, u64::MAX)?;
        }
        for b in self.garbage.drain(..) {
            self.gpu.destroy_buffer(b);
        }
        self.read_timestamps();

        let image_index = match &self.output {
            Output::Window(sc) => {
                let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
                match unsafe {
                    swapchain_fn.acquire_next_image(
                        sc.swapchain,
                        u64::MAX,
                        self.image_available,
                        vk::Fence::null(),
                    )
                } {
                    Ok((index, _suboptimal)) => index as usize,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.create_size_dependent()?;
                        return Ok(false);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            Output::Headless { .. } => 0,
        };
        unsafe { device.reset_fences(&[self.fence])? };

        // ---- CPU-side updates -------------------------------------------------
        let camera = input.camera;
        if let Some(frame) = input.sim {
            self.upload_sim(frame, input.time, camera);
            self.fog_enabled = !frame.fog.is_empty();
            self.tile_cache
                .apply_edits(&frame.terrain_edits, &mut self.upload_scratch);
        }
        let ghosts = &input.ghosts[..input.ghosts.len().min(512)];
        self.dynamic.write(
            (self.sim_units as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(ghosts),
        );
        self.dynamic_count = self.sim_units + ghosts.len() as u32;

        let z_range = (
            self.map_info.min_z.to_f32(),
            self.map_info.min_z.to_f32() + self.map_info.z_step.to_f32() * 65535.0,
        );
        let z_used = (z_range.0.max(-300.0), z_range.1.min(900.0));
        if !terrain::select_nodes(camera, z_used, &mut self.node_scratch) {
            log::error!("terrain node budget of {MAX_NODES} exceeded; distant terrain is missing this frame");
        }
        self.nodes
            .write(0, bytemuck::cast_slice(&self.node_scratch));
        self.tile_cache
            .update(camera, &self.pool, &mut self.upload_scratch);

        let marks: Vec<Mark> = input
            .marks
            .iter()
            .take(MAX_MARKS)
            .filter(|m| m.unit_index < self.sim_units)
            .map(|m| Mark {
                unit_index: m.unit_index | 0x8000_0000,
                kind: m.kind,
                work: m.work,
                _pad: 0,
            })
            .collect();
        self.marks.write(0, bytemuck::cast_slice(&marks));
        // Long rings first, then short ones, then those that only mask: each run is one draw.
        let all = &input.ranges[..input.ranges.len().min(MAX_RANGES)];
        let (shown, masks) = all.split_at(input.ranges_drawn.min(all.len()));
        let mut ranges: Vec<RangeRing> = shown
            .iter()
            .filter(|r| r.outer >= RANGE_LONG)
            .copied()
            .collect();
        let ranges_long = ranges.len() as u32;
        ranges.extend(shown.iter().filter(|r| r.outer < RANGE_LONG));
        let ranges_short = ranges.len() as u32 - ranges_long;
        ranges.extend_from_slice(masks);
        self.ranges.write(0, bytemuck::cast_slice(&ranges));
        let overlay =
            &input.overlay.vertices[..input.overlay.vertices.len().min(MAX_OVERLAY_VERTICES)];
        self.overlay_vb.write(0, bytemuck::cast_slice(overlay));
        self.upload_overlay_atlas(input.overlay)?;

        // Sun, and a shadow box around what the camera is looking at.
        let sun = Vec3::new(0.45, -0.35, 0.82).normalize();
        let shadow_strength = 1.0 - ((camera.distance - 3000.0) / 5000.0).clamp(0.0, 1.0);
        let extent = (camera.distance * 1.4).clamp(150.0, 9000.0);
        let light_view = glam::camera::rh::view::look_at_mat4(
            camera.focus + sun * extent * 2.0,
            camera.focus,
            Vec3::Z,
        );
        let shadow_view_proj: Mat4 = glam::camera::rh::proj::directx::orthographic(
            -extent,
            extent,
            -extent,
            extent,
            1.0,
            extent * 4.0,
        ) * light_view;

        let view_proj = camera.view_proj();
        let eye = camera.eye();
        let size = self.map_info.size_metres().to_f32();
        let (tw, th) = self.tile_cache.tiles();
        // Below this projected radius a unit is drawn as its strategic icon.
        let icon_px = self.height as f32 * 0.0055;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            shadow_view_proj: shadow_view_proj.to_cols_array_2d(),
            camera: [eye.x, eye.y, eye.z, input.time],
            sun: [sun.x, sun.y, sun.z, input.alpha.clamp(0.0, 1.0)],
            viewport: [
                self.width as f32,
                self.height as f32,
                1.0 / self.width as f32,
                1.0 / self.height as f32,
            ],
            frustum: camera.frustum().map(|p| p.to_array()),
            map: [
                size[0],
                size[1],
                self.map_info.water_level.to_f32(),
                shadow_strength,
            ],
            height: [z_range.0, z_range.1 - z_range.0, tw as f32, th as f32],
            lod: [camera.projection_scale(), icon_px, 90.0, 28.0],
            counts: [
                self.dynamic_count,
                self.static_count,
                self.slot_count,
                self.fog_enabled as u32,
            ],
            plating: self.palette[0],
            accent: self.palette[1],
            glow: self.palette[2],
            team_colors: self.team_colors,
        };
        self.globals.write(0, bytemuck::bytes_of(&globals));
        self.last_time = input.time;

        // ---- Record -----------------------------------------------------------
        let cmd = self.cmd;
        unsafe {
            device.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            device.cmd_reset_query_pool(cmd, self.query_pool, 0, PASS_NAMES.len() as u32 * 2);
        }
        self.record_uploads(cmd, input.sim)?;

        let stamp = |q: u32| unsafe {
            device.cmd_write_timestamp(
                cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.query_pool,
                q,
            )
        };
        let compute_barrier = |dst: vk::AccessFlags, dst_stage: vk::PipelineStageFlags| unsafe {
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(dst)];
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                dst_stage,
                vk::DependencyFlags::empty(),
                &barrier,
                &[],
                &[],
            );
        };
        let rw = vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;

        // GPU culling and draw generation.
        stamp(0);
        unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.layouts.cull,
                0,
                &[self.cull_set],
                &[],
            );
            let dispatch = |pipeline: vk::Pipeline, count: u32, dynamic: u32| {
                if count == 0 {
                    return;
                }
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
                device.cmd_push_constants(
                    cmd,
                    self.layouts.cull,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    bytemuck::bytes_of(&[count, dynamic]),
                );
                device.cmd_dispatch(cmd, count.div_ceil(64), 1, 1);
            };
            dispatch(self.pipelines.cull_clear, self.slot_count, 0);
            compute_barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
            dispatch(self.pipelines.cull_cull, self.static_count, 0);
            dispatch(self.pipelines.cull_cull, self.dynamic_count, 1);
            compute_barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.pipelines.cull_prefix,
            );
            device.cmd_dispatch(cmd, 1, 1, 1);
            compute_barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
            dispatch(self.pipelines.cull_scatter, self.static_count, 0);
            dispatch(self.pipelines.cull_scatter, self.dynamic_count, 1);
            compute_barrier(
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::INDIRECT_COMMAND_READ,
                vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::DRAW_INDIRECT,
            );
        }
        stamp(1);

        let node_count = self.node_scratch.len() as u32;
        let model_slots = self.slot_count - 1;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let set_viewport = |w: u32, h: u32| unsafe {
            device.cmd_set_viewport(
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
            device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: w,
                        height: h,
                    },
                }],
            );
        };
        let bind_pass_set = |set: vk::DescriptorSet| unsafe {
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                1,
                &[set],
                &[],
            );
        };
        let push = |a: u32, b: u32| unsafe {
            device.cmd_push_constants(cmd, self.layouts.scene, gfx, 0, bytemuck::bytes_of(&[a, b]))
        };
        let draw_terrain = |pipeline: vk::Pipeline, pass_kind: u32| unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            bind_pass_set(self.nodes_set);
            push(pass_kind, input.build_grid as u32);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.grid_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.grid_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed(cmd, self.grid_index_count, node_count, 0, 0, 0);
        };
        let draw_entities = |pipeline: vk::Pipeline, pass_kind: u32| unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            push(pass_kind, 0);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.mesh_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed_indirect(cmd, self.commands.buffer, 0, model_slots, 20);
        };
        let draw_quads = |pipeline: vk::Pipeline, set: vk::DescriptorSet, instances: u32| unsafe {
            if instances == 0 {
                return;
            }
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            bind_pass_set(set);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.quad_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed(cmd, 6, instances, 0, 0, 0);
        };

        // Shadow pass. Always begun so the map ends up in its sampled layout.
        stamp(2);
        unsafe {
            let clear = [vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            }];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.shadow)
                .framebuffer(self.shadow_fb)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: SHADOW_SIZE,
                        height: SHADOW_SIZE,
                    },
                })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            if shadow_strength > 0.0 {
                set_viewport(SHADOW_SIZE, SHADOW_SIZE);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.scene,
                    0,
                    &[self.scene_set],
                    &[],
                );
                draw_terrain(self.pipelines.terrain_shadow, 1);
                draw_entities(self.pipelines.entity_shadow, 1);
            }
            device.cmd_end_render_pass(cmd);
        }
        stamp(3);

        // Scene pass.
        stamp(4);
        unsafe {
            let clear = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0.55, 0.66, 0.8, 1.0],
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 0.0,
                        stencil: 0,
                    },
                },
            ];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.scene)
                .framebuffer(self.scene_fb)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    },
                })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.width, self.height);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.scene,
                0,
                &[self.scene_set],
                &[],
            );
            draw_terrain(self.pipelines.terrain, 0);

            if self.stain_count + self.pad_count + self.deposit_count > 0 {
                bind_pass_set(self.stains_set);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.patch_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.patch_ib.buffer, 0, vk::IndexType::UINT32);
                if self.pad_count > 0 {
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.pad,
                    );
                    device.cmd_draw_indexed(
                        cmd,
                        self.patch_index_count,
                        self.pad_count,
                        0,
                        0,
                        self.stain_count,
                    );
                }
                if self.stain_count > 0 {
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.stain,
                    );
                    device.cmd_draw_indexed(cmd, self.patch_index_count, self.stain_count, 0, 0, 0);
                }
                if self.deposit_count > 0 {
                    device.cmd_bind_pipeline(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.pipelines.deposit,
                    );
                    device.cmd_draw_indexed(
                        cmd,
                        self.patch_index_count,
                        self.deposit_count,
                        0,
                        0,
                        self.deposit_first,
                    );
                }
            }
            if self.track_count > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.track,
                );
                bind_pass_set(self.stains_set);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.quad_ib.buffer, 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed(cmd, 6, self.track_count, 0, 0, 0);
            }
            draw_entities(self.pipelines.entity, 0);

            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.water);
            device.cmd_draw(cmd, 6, 1, 0, 0);

            if ranges_long + ranges_short > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.range,
                );
                bind_pass_set(self.ranges_set);
                device.cmd_bind_index_buffer(cmd, self.range_ib.buffer, 0, vk::IndexType::UINT32);
                // Two instances a ring: its reach and its dead zone.
                for (first, count, segments) in [
                    (0, ranges_long, RANGE_SEGMENTS[0]),
                    (ranges_long, ranges_short, RANGE_SEGMENTS[1]),
                ] {
                    if count > 0 {
                        push(ranges.len() as u32, segments);
                        device.cmd_draw_indexed(cmd, segments * 6, count * 2, 0, 0, first * 2);
                    }
                }
            }
            draw_quads(self.pipelines.ring, self.marks_set, marks.len() as u32);
            draw_quads(self.pipelines.puff, self.puffs_set, MAX_PUFFS as u32);
            draw_quads(
                self.pipelines.beam,
                self.puffs_set,
                self.beam_count * BEAM_QUADS,
            );
            draw_quads(
                self.pipelines.projectile,
                self.sprites_set,
                self.projectile_count,
            );
            draw_quads(self.pipelines.effect, self.sprites_set, MAX_EFFECTS as u32);
            // After the glow, so a shot's dot is not washed out by its own tracer.
            draw_quads(self.pipelines.shot, self.sprites_set, self.projectile_count);

            // Strategic icons: the cull pass's last draw slot, as quads.
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.icon);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed_indirect(
                cmd,
                self.commands.buffer,
                model_slots as u64 * 20,
                1,
                20,
            );
            draw_quads(self.pipelines.bar, self.marks_set, marks.len() as u32);
            device.cmd_end_render_pass(cmd);
        }
        stamp(5);

        // Bloom: down the chain from the scene, then back up it, each level added onto the next larger one.
        stamp(6);
        unsafe {
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.bloom_down,
            );
            let level_pass = |pass: vk::RenderPass,
                              level: usize,
                              source: vk::DescriptorSet,
                              a: [f32; 2]| {
                let image = &self.bloom[level];
                let begin = vk::RenderPassBeginInfo::default()
                    .render_pass(pass)
                    .framebuffer(self.bloom_fbs[level])
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent: vk::Extent2D {
                            width: image.width,
                            height: image.height,
                        },
                    });
                device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
                set_viewport(image.width, image.height);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.screen,
                    0,
                    &[source],
                    &[],
                );
                device.cmd_push_constants(cmd, self.layouts.screen, gfx, 0, bytemuck::bytes_of(&a));
                device.cmd_draw(cmd, 3, 1, 0, 0);
                device.cmd_end_render_pass(cmd);
            };
            for level in 0..BLOOM_LEVELS {
                let source = if level == 0 {
                    self.hdr_set
                } else {
                    self.bloom_sets[level - 1]
                };
                level_pass(
                    self.passes.bloom_down,
                    level,
                    source,
                    [(level == 0) as u32 as f32, 0.0],
                );
            }
            device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipelines.bloom_up,
            );
            for level in (1..BLOOM_LEVELS).rev() {
                level_pass(
                    self.passes.bloom_up,
                    level - 1,
                    self.bloom_sets[level],
                    [1.0, 0.85],
                );
            }
        }

        // Tone map to the output, then the UI on top.
        unsafe {
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.present)
                .framebuffer(self.present_fbs[image_index])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent: vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    },
                });
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.width, self.height);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.screen,
                0,
                &[self.screen_set],
                &[],
            );
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.tonemap);
            device.cmd_push_constants(
                cmd,
                self.layouts.screen,
                gfx,
                0,
                bytemuck::bytes_of(&[1.0f32, 0.35]),
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
            if !overlay.is_empty() {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.overlay,
                );
                device.cmd_push_constants(
                    cmd,
                    self.layouts.screen,
                    gfx,
                    0,
                    bytemuck::bytes_of(&[2.0 / self.width as f32, 2.0 / self.height as f32]),
                );
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.overlay_vb.buffer], &[0]);
                device.cmd_draw(cmd, overlay.len() as u32, 1, 0, 0);
            }
            device.cmd_end_render_pass(cmd);
        }
        stamp(7);

        if let Output::Headless { image, readback } = &self.output {
            unsafe {
                let copy = [vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image_extent(vk::Extent3D {
                        width: self.width,
                        height: self.height,
                        depth: 1,
                    })];
                device.cmd_copy_image_to_buffer(
                    cmd,
                    image.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    readback.buffer,
                    &copy,
                );
            }
        }

        // ---- Submit -----------------------------------------------------------
        unsafe {
            device.end_command_buffer(cmd)?;
            let cmds = [cmd];
            let wait = [self.image_available];
            let signal = [self.render_finished];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let mut submit = vk::SubmitInfo::default().command_buffers(&cmds);
            if matches!(self.output, Output::Window(_)) {
                submit = submit
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&stages)
                    .signal_semaphores(&signal);
            }
            device.queue_submit(self.gpu.queue, &[submit], self.fence)?;
        }
        self.queries_valid = true;

        if let Output::Window(sc) = &self.output {
            let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
            let wait = [self.render_finished];
            let swapchains = [sc.swapchain];
            let indices = [image_index as u32];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait)
                .swapchains(&swapchains)
                .image_indices(&indices);
            match unsafe { swapchain_fn.queue_present(self.gpu.queue, &present) } {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.create_size_dependent()?
                }
                Err(e) => return Err(e.into()),
            }
        }

        self.stats.terrain_nodes = self.node_scratch.len();
        self.stats.dynamic_entities = self.dynamic_count as usize;
        self.stats.static_entities = self.static_count as usize;
        Ok(true)
    }

    /// Terrain tile/patch uploads and the fog texture, through one staging buffer.
    fn record_uploads(
        &mut self,
        cmd: vk::CommandBuffer,
        sim: Option<&RenderFrame>,
    ) -> Result<(), GpuError> {
        let uploads = std::mem::take(&mut self.upload_scratch);
        let fog = sim
            .map(|f| &f.fog)
            .filter(|f| f.len() == (self.fog_dims.0 * self.fog_dims.1 * 2) as usize);
        let mut bytes: Vec<u8> = Vec::new();
        let mut copies: Vec<(u64, &Image, u32, Option<vk::Rect2D>)> = Vec::new();
        let rect = |x: u32, y: u32, w: u32, h: u32| {
            Some(vk::Rect2D {
                offset: vk::Offset2D {
                    x: x as i32,
                    y: y as i32,
                },
                extent: vk::Extent2D {
                    width: w,
                    height: h,
                },
            })
        };
        for u in &uploads {
            // Buffer-to-image copies need offsets aligned to the texel size.
            while bytes.len() % 4 != 0 {
                bytes.push(0);
            }
            let offset = bytes.len() as u64;
            match u {
                TerrainUpload::Tile { layer, samples } => {
                    bytes.extend_from_slice(bytemuck::cast_slice(samples));
                    copies.push((offset, &self.tiles, *layer, None));
                }
                TerrainUpload::TilePatch {
                    layer,
                    x,
                    y,
                    w,
                    h,
                    sample,
                } => {
                    bytes.extend((0..w * h).flat_map(|_| sample.to_le_bytes()));
                    copies.push((offset, &self.tiles, *layer, rect(*x, *y, *w, *h)));
                }
                TerrainUpload::OverviewPatch { x, y, w, h, sample } => {
                    if *w == 0 {
                        bytes.extend_from_slice(bytemuck::cast_slice(&self.tile_cache.overview));
                        copies.push((offset, &self.overview, 0, None));
                    } else {
                        bytes.extend((0..w * h).flat_map(|_| sample.to_le_bytes()));
                        copies.push((offset, &self.overview, 0, rect(*x, *y, *w, *h)));
                    }
                }
                TerrainUpload::Index(index) => {
                    bytes.extend_from_slice(bytemuck::cast_slice(index));
                    copies.push((offset, &self.tile_index, 0, None));
                }
            }
        }
        if let Some(fog) = fog {
            while bytes.len() % 4 != 0 {
                bytes.push(0);
            }
            copies.push((bytes.len() as u64, &self.fog, 0, None));
            bytes.extend_from_slice(fog);
        }
        if !copies.is_empty() {
            let staging = self
                .gpu
                .host_buffer(bytes.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
            staging.write(0, &bytes);
            for (offset, image, layer, region) in copies {
                self.gpu.record_image_upload(
                    cmd,
                    image,
                    layer,
                    0,
                    region,
                    staging.buffer,
                    offset,
                    false,
                );
            }
            self.garbage.push(staging);
        }
        self.stats.tiles_resident = 0;
        Ok(())
    }

    fn read_timestamps(&mut self) {
        if !self.queries_valid || self.timestamp_period <= 0.0 {
            return;
        }
        let mut ticks = [0u64; PASS_NAMES.len() * 2];
        let ok = unsafe {
            self.gpu.device.get_query_pool_results(
                self.query_pool,
                0,
                &mut ticks,
                vk::QueryResultFlags::TYPE_64,
            )
        };
        if ok.is_ok() {
            self.stats.gpu_passes.clear();
            for (i, name) in PASS_NAMES.iter().enumerate() {
                let ns =
                    ticks[i * 2 + 1].saturating_sub(ticks[i * 2]) as f32 * self.timestamp_period;
                self.stats.gpu_passes.push((name, ns / 1.0e6));
            }
        }
    }

    /// Headless only: the last rendered frame as tightly packed RGBA8.
    pub fn read_pixels(&mut self) -> Option<Vec<u8>> {
        unsafe {
            self.gpu
                .device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .ok()?
        };
        match &self.output {
            Output::Headless { readback, .. } => {
                let mut out = vec![0u8; (self.width * self.height * 4) as usize];
                readback.read(0, &mut out);
                Some(out)
            }
            Output::Window(_) => None,
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.gpu.wait_idle();
        let device = &self.gpu.device;
        unsafe {
            for fb in self.present_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            device.destroy_framebuffer(self.scene_fb, None);
            device.destroy_framebuffer(self.shadow_fb, None);
            for fb in self.bloom_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            device.destroy_query_pool(self.query_pool, None);
            device.destroy_fence(self.fence, None);
            device.destroy_semaphore(self.image_available, None);
            device.destroy_semaphore(self.render_finished, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            for s in self.samplers {
                device.destroy_sampler(s, None);
            }
            if let Output::Window(sc) = &mut self.output {
                for v in sc.views.drain(..) {
                    device.destroy_image_view(v, None);
                }
                if let Some(f) = &self.gpu.swapchain_fn {
                    f.destroy_swapchain(sc.swapchain, None);
                }
                self.gpu.surface_fn.destroy_surface(sc.surface, None);
            }
        }
        self.pipelines.destroy(&self.gpu);
        self.layouts.destroy(&self.gpu);
        self.passes.destroy(&self.gpu);

        let gpu = &self.gpu;
        let placeholder = || Buffer::null();
        for b in [
            &mut self.globals,
            &mut self.dynamic,
            &mut self.statics,
            &mut self.model_table,
            &mut self.slot_table,
            &mut self.vis,
            &mut self.counters,
            &mut self.commands,
            &mut self.visible,
            &mut self.props_dead,
            &mut self.nodes,
            &mut self.marks,
            &mut self.ranges,
            &mut self.range_ib,
            &mut self.projectiles,
            &mut self.effects,
            &mut self.stains,
            &mut self.puffs,
            &mut self.beams,
            &mut self.track_marks,
            &mut self.overlay_vb,
            &mut self.mesh_vb,
            &mut self.mesh_ib,
            &mut self.quad_vb,
            &mut self.quad_ib,
            &mut self.grid_vb,
            &mut self.grid_ib,
            &mut self.patch_vb,
            &mut self.patch_ib,
        ] {
            gpu.destroy_buffer(std::mem::replace(b, placeholder()));
        }
        for b in self.garbage.drain(..) {
            gpu.destroy_buffer(b);
        }
        for image in [
            &self.hdr,
            &self.depth,
            &self.shadow,
            &self.overview,
            &self.tiles,
            &self.tile_index,
            &self.fog,
            &self.noise,
            &self.panel,
            &self.font,
        ]
        .into_iter()
        .chain(&self.bloom)
        {
            gpu.destroy_image_ref(image);
        }
        if let Output::Headless { image, readback } = &mut self.output {
            gpu.destroy_image_ref(image);
            gpu.destroy_buffer(std::mem::replace(readback, placeholder()));
        }
        // `self.gpu` drops last and destroys the device and instance.
    }
}
