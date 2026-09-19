//! Frame orchestration.
//!
//! Per frame the CPU writes one uniform block, the terrain node list, and
//! whatever the UI drew. Per sim tick it copies the render mirror into GPU
//! buffers wholesale. Everything per-entity (interpolation, culling, LOD,
//! draw generation, animation) runs in shaders.

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::models::{self, MeshVertex, Model};
use crate::overlay::{Overlay, OverlayVertex, MAX_OVERLAY_VERTICES};
use crate::pipelines::{Layouts, Passes, Pipelines, DEPTH_FORMAT, HDR_FORMAT, SHADOW_SIZE};
use crate::terrain::{self, TerrainNode, TerrainUpload, TileCache, MAX_NODES, TILE_LAYERS};
use crate::textures;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{MapFile, PropKind, TILE_SAMPLES};
use mc_sim::mirror::{ProjectileInstance, RenderFrame, SimEvent, StainInstance, UnitInstance, KIND_PROP};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::sync::Arc;

pub const MAX_DYNAMIC: usize = mc_sim::tables::MAX_UNITS + mc_sim::tables::MAX_WRECKS + 512;
pub const MAX_MARKS: usize = 4096;
pub const MAX_EFFECTS: usize = 2048;
const MAX_PROJECTILES: usize = mc_sim::tables::MAX_PROJECTILES;
const MAX_STAINS: usize = mc_sim::tables::MAX_STAINS;

pub enum Target {
    Window { display: RawDisplayHandle, window: RawWindowHandle, width: u32, height: u32, vsync: bool },
    Headless { width: u32, height: u32 },
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
    scene_fb: vk::Framebuffer,
    shadow_fb: vk::Framebuffer,
    present_fbs: Vec<vk::Framebuffer>,

    descriptor_pool: vk::DescriptorPool,
    scene_set: vk::DescriptorSet,
    cull_set: vk::DescriptorSet,
    screen_set: vk::DescriptorSet,
    nodes_set: vk::DescriptorSet,
    marks_set: vk::DescriptorSet,
    sprites_set: vk::DescriptorSet,
    stains_set: vk::DescriptorSet,
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
    projectiles: Buffer,
    effects: Buffer,
    stains: Buffer,
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
    effect_cursor: usize,
    fog_enabled: bool,
    last_time: f32,
    pub stats: FrameStats,
}

/// Mass deposit marker: a low metal hexagon with a glowing core, so deposits
/// read from any zoom level before anything is built on them.
fn deposit_model() -> Model {
    let mut lod = models::MeshLod::default();
    let ring = |r: f32, z: f32| -> Vec<[f32; 3]> { (0..6).map(|i| { let a = i as f32 * std::f32::consts::FRAC_PI_3; [r * a.cos(), r * a.sin(), z] }).collect() };
    let mut fan = |points: &[[f32; 3]], centre: [f32; 3], material: u32| {
        for i in 0..6 {
            let base = lod.vertices.len() as u32;
            for pos in [centre, points[i], points[(i + 1) % 6]] {
                lod.vertices.push(MeshVertex { pos, normal: [0.0, 0.0, 1.0], uv: [pos[0], pos[1]], material, part: models::part::HULL });
            }
            lod.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    };
    fan(&ring(13.0, 0.35), [0.0, 0.0, 0.35], models::material::METAL);
    fan(&ring(9.0, 0.5), [0.0, 0.0, 0.5], models::material::ACCENT);
    fan(&ring(4.5, 0.65), [0.0, 0.0, 0.65], models::material::GLOW);
    Model { key: "deposit".into(), lods: [lod.clone(), lod.clone(), lod], turret_pivot: [0.0; 3], spinner_pivot: [0.0; 3], bounds_radius: 14.0 }
}

fn fallback_model(key: &str, radius: f32, height: f32) -> Model {
    let mut lod = models::MeshLod::default();
    let (r, h) = (radius * 0.7, height);
    let faces: [([f32; 3], [[f32; 3]; 4]); 5] = [
        ([0.0, 0.0, 1.0], [[-r, -r, h], [r, -r, h], [r, r, h], [-r, r, h]]),
        ([1.0, 0.0, 0.0], [[r, -r, 0.0], [r, r, 0.0], [r, r, h], [r, -r, h]]),
        ([-1.0, 0.0, 0.0], [[-r, r, 0.0], [-r, -r, 0.0], [-r, -r, h], [-r, r, h]]),
        ([0.0, 1.0, 0.0], [[r, r, 0.0], [-r, r, 0.0], [-r, r, h], [r, r, h]]),
        ([0.0, -1.0, 0.0], [[-r, -r, 0.0], [r, -r, 0.0], [r, -r, h], [-r, -r, h]]),
    ];
    for (normal, corners) in faces {
        let base = lod.vertices.len() as u32;
        for pos in corners {
            lod.vertices.push(MeshVertex { pos, normal, uv: [pos[0] + pos[1], pos[2]], material: models::material::PLATING, part: models::part::HULL });
        }
        lod.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Model { key: key.to_owned(), lods: [lod.clone(), lod.clone(), lod], turret_pivot: [0.0; 3], spinner_pivot: [0.0; 3], bounds_radius: (radius * radius + height * height).sqrt() }
}

impl Renderer {
    pub fn new(target: Target, scene: SceneDesc) -> Result<Renderer, GpuError> {
        let (gpu, surface, width, height, vsync) = match &target {
            Target::Window { display, window, width, height, vsync } => {
                let extensions = ash_window::enumerate_required_extensions(*display).map_err(GpuError::Vk)?;
                let gpu = Gpu::new(extensions)?;
                let surface = unsafe { ash_window::create_surface(&gpu.entry, &gpu.instance, *display, *window, None) }?;
                (gpu, Some(surface), *width, *height, *vsync)
            }
            Target::Headless { width, height } => (Gpu::new(&[])?, None, *width, *height, false),
        };
        let (width, height) = (width.max(16), height.max(16));

        let present_format = match surface {
            Some(surface) => {
                let formats = unsafe { gpu.surface_fn.get_physical_device_surface_formats(gpu.physical, surface) }?;
                formats
                    .iter()
                    .find(|f| f.format == vk::Format::B8G8R8A8_SRGB)
                    .or_else(|| formats.iter().find(|f| f.format == vk::Format::R8G8B8A8_SRGB))
                    .or(formats.first())
                    .map(|f| f.format)
                    .ok_or_else(|| GpuError::NoDevice("surface reports no formats".into()))?
            }
            None => vk::Format::R8G8B8A8_SRGB,
        };
        let present_layout = if surface.is_some() { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::TRANSFER_SRC_OPTIMAL };
        let passes = Passes::new(&gpu, present_format, present_layout)?;
        let layouts = Layouts::new(&gpu)?;
        let pipelines = Pipelines::new(&gpu, &layouts, &passes)?;

        // Models: one per blueprint (each is fitted to its blueprint's size), then one per prop kind.
        let bps = &scene.blueprints;
        let mut model_list: Vec<(Model, u32)> = Vec::new();
        for bp in &bps.units {
            let (radius, height) = (bp.radius.to_f32(), bp.height.to_f32());
            let model = models::build_model_scaled(&bp.visual.mesh, radius, height, bp.tech).unwrap_or_else(|| {
                log::warn!("no model for mesh key {:?}; using a box", bp.visual.mesh);
                fallback_model(&bp.visual.mesh, radius, height)
            });
            let icon = bp.visual.icon as u32 | (bp.tech as u32) << 8 | (bp.is_mobile() as u32) << 16;
            model_list.push((model, icon));
        }
        let prop_base = model_list.len() as u32;
        for kind in PropKind::ALL {
            let key = models::prop_model_key(kind.raw());
            let model = models::build_model(key).unwrap_or_else(|| fallback_model(key, 4.0, 8.0));
            model_list.push((model, 13));
        }
        let deposit_model_index = model_list.len() as u32;
        model_list.push((deposit_model(), 13));

        let mut vertices: Vec<MeshVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut slots: Vec<DrawSlot> = Vec::new();
        let mut infos: Vec<ModelInfo> = Vec::new();
        for (model, icon) in &model_list {
            let height = model.lods[0].vertices.iter().map(|v| v.pos[2]).fold(0.0f32, f32::max);
            infos.push(ModelInfo {
                slot: slots.len() as u32,
                icon: *icon,
                bounds_radius: model.bounds_radius,
                height,
                turret_pivot: [model.turret_pivot[0], model.turret_pivot[1], model.turret_pivot[2], 0.0],
                spinner_pivot: [model.spinner_pivot[0], model.spinner_pivot[1], model.spinner_pivot[2], 0.0],
            });
            for lod in &model.lods {
                slots.push(DrawSlot { index_count: lod.indices.len() as u32, first_index: indices.len() as u32, vertex_offset: vertices.len() as i32, pad: 0 });
                vertices.extend_from_slice(&lod.vertices);
                indices.extend_from_slice(&lod.indices);
            }
        }
        // The last slot draws strategic icons: a quad from its own vertex buffer.
        slots.push(DrawSlot { index_count: 6, first_index: indices.len() as u32, vertex_offset: 0, pad: 0 });
        indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        let slot_count = slots.len() as u32;

        // Static entities: the map's props.
        let info = scene.map.info().clone();
        let tile_cache = TileCache::new(scene.map.clone());
        let kinds: Vec<u16> = PropKind::ALL.iter().map(|k| k.raw()).collect();
        let mut statics_data: Vec<UnitInstance> = scene
            .map
            .props()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let xy = p.pos.to_f32();
                let pos = [xy[0], xy[1], tile_cache.overview_height(glam::Vec2::from(xy))];
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
                }
            })
            .collect();
        for (i, d) in scene.map.mass_deposits().iter().enumerate() {
            let xy = d.to_f32();
            let pos = [xy[0], xy[1], tile_cache.overview_height(glam::Vec2::from(xy))];
            statics_data.push(UnitInstance {
                prev_pos: pos,
                prev_heading: 0.0,
                pos,
                heading: 0.0,
                blueprint: deposit_model_index,
                owner_flags: KIND_PROP,
                health: 1.0,
                build: 1.0,
                turret_yaw: 0.0,
                radius: 13.0,
                unit_id: (scene.map.props().len() + i) as u32,
                _pad: 0,
            });
        }
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
        let visible = gpu.device_buffer(total_entities * 4, storage)?;
        let props_dead = gpu.host_buffer((static_count as u64).div_ceil(32).max(1) * 4, storage)?;
        props_dead.write(0, &vec![0u8; props_dead.size as usize]);
        let nodes = gpu.host_buffer((MAX_NODES * size_of::<TerrainNode>()) as u64, storage)?;
        let marks = gpu.host_buffer((MAX_MARKS * size_of::<Mark>()) as u64, storage)?;
        let projectiles = gpu.host_buffer((MAX_PROJECTILES * size_of::<ProjectileInstance>()) as u64, storage)?;
        let effects = gpu.host_buffer((MAX_EFFECTS * size_of::<Effect>()) as u64, storage)?;
        effects.write(0, &vec![0u8; effects.size as usize]);
        let stains = gpu.host_buffer((MAX_STAINS * size_of::<StainInstance>()) as u64, storage)?;
        let overlay_vb = gpu.host_buffer((MAX_OVERLAY_VERTICES * size_of::<OverlayVertex>()) as u64, U::VERTEX_BUFFER)?;
        let mesh_vb = gpu.buffer_with_data(bytemuck::cast_slice(&vertices), U::VERTEX_BUFFER)?;
        let mesh_ib = gpu.buffer_with_data(bytemuck::cast_slice(&indices), U::INDEX_BUFFER)?;
        let quad: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let quad_vb = gpu.buffer_with_data(bytemuck::cast_slice(&quad), U::VERTEX_BUFFER)?;
        let quad_ib = gpu.buffer_with_data(bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]), U::INDEX_BUFFER)?;
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
                patch_i.extend_from_slice(&[at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y), at(x + 1, y + 1), at(x, y + 1)]);
            }
        }
        let patch_vb = gpu.buffer_with_data(bytemuck::cast_slice(&patch_v), U::VERTEX_BUFFER)?;
        let patch_ib = gpu.buffer_with_data(bytemuck::cast_slice(&patch_i), U::INDEX_BUFFER)?;

        // Textures.
        let sampled = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        let (ow, oh) = scene.map.overview_dims();
        let overview = gpu.image(&ImageDesc { width: ow, height: oh, format: vk::Format::R16_UNORM, usage: sampled, layers: 1, mips: 1, array: false })?;
        gpu.upload_image(&overview, 0, 0, None, bytemuck::cast_slice(scene.map.overview()), true)?;
        let tiles = gpu.image(&ImageDesc { width: TILE_SAMPLES, height: TILE_SAMPLES, format: vk::Format::R16_UNORM, usage: sampled, layers: TILE_LAYERS, mips: 1, array: true })?;
        gpu.upload_image(&tiles, 0, 0, None, &vec![0u8; (TILE_SAMPLES * TILE_SAMPLES * 2) as usize], true)?;
        let (tw, th) = scene.map.size_tiles();
        let tile_index = gpu.image(&ImageDesc { width: tw, height: th, format: vk::Format::R16_UINT, usage: sampled, layers: 1, mips: 1, array: false })?;
        gpu.upload_image(&tile_index, 0, 0, None, &vec![0u8; (tw * th * 2) as usize], true)?;
        let size = info.size_metres().to_f32();
        let fog_dims = ((size[0] as u32 >> 6).max(1) + 1, (size[1] as u32 >> 6).max(1) + 1);
        let fog = gpu.image(&ImageDesc { width: fog_dims.0, height: fog_dims.1, format: vk::Format::R8G8_UNORM, usage: sampled, layers: 1, mips: 1, array: false })?;
        gpu.upload_image(&fog, 0, 0, None, &vec![255u8; (fog_dims.0 * fog_dims.1 * 2) as usize], true)?;
        let rgba_mips = |data: Vec<u8>| -> Result<Image, GpuError> {
            let n = textures::SIZE as u32;
            let mips = 32 - n.leading_zeros();
            let image = gpu.image(&ImageDesc { width: n, height: n, format: vk::Format::R8G8B8A8_UNORM, usage: sampled, layers: 1, mips, array: false })?;
            gpu.upload_image(&image, 0, 0, None, &data, true)?;
            for (level, (_, pixels)) in textures::mip_chain(&data, textures::SIZE).iter().enumerate() {
                gpu.upload_image(&image, 0, level as u32 + 1, None, pixels, false)?;
            }
            Ok(image)
        };
        let noise = rgba_mips(textures::noise_map())?;
        let panel = rgba_mips(textures::panel_map())?;
        // The overlay's atlas; its contents arrive with the first frame (`upload_overlay_atlas`).
        let font = gpu.image(&ImageDesc { width: textures::FONT_ATLAS_W as u32, height: textures::FONT_ATLAS_H as u32, format: vk::Format::R8G8B8A8_SRGB, usage: sampled, layers: 1, mips: 1, array: false })?;

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
            gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::REPEAT, true, None)?,
            gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, None)?,
            gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, Some(vk::CompareOp::LESS_OR_EQUAL))?,
        ];

        // Descriptor sets.
        let pool_sizes = [
            vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 4 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 32 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLED_IMAGE, descriptor_count: 16 },
            vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 8 },
        ];
        let descriptor_pool = unsafe { gpu.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(8).pool_sizes(&pool_sizes), None) }?;
        let alloc = |layout: vk::DescriptorSetLayout| -> Result<vk::DescriptorSet, GpuError> {
            let layouts = [layout];
            Ok(unsafe { gpu.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(descriptor_pool).set_layouts(&layouts)) }?[0])
        };
        let scene_set = alloc(layouts.scene_set)?;
        let cull_set = alloc(layouts.cull_set)?;
        let screen_set = alloc(layouts.screen_set)?;
        let nodes_set = alloc(layouts.pass_set)?;
        let marks_set = alloc(layouts.pass_set)?;
        let sprites_set = alloc(layouts.pass_set)?;
        let stains_set = alloc(layouts.pass_set)?;

        let write_buffers = |set: vk::DescriptorSet, first: u32, ty: vk::DescriptorType, buffers: &[&Buffer]| {
            for (i, b) in buffers.iter().enumerate() {
                let info = [b.info()];
                let write = [vk::WriteDescriptorSet::default().dst_set(set).dst_binding(first + i as u32).descriptor_type(ty).buffer_info(&info)];
                unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
            }
        };
        let write_image = |set: vk::DescriptorSet, binding: u32, view: vk::ImageView, layout: vk::ImageLayout| {
            let info = [vk::DescriptorImageInfo { sampler: vk::Sampler::null(), image_view: view, image_layout: layout }];
            let write = [vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&info)];
            unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        };
        let write_sampler = |set: vk::DescriptorSet, binding: u32, sampler: vk::Sampler| {
            let info = [vk::DescriptorImageInfo { sampler, image_view: vk::ImageView::null(), image_layout: vk::ImageLayout::UNDEFINED }];
            let write = [vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&info)];
            unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
        };
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        write_buffers(scene_set, 0, vk::DescriptorType::UNIFORM_BUFFER, &[&globals]);
        write_buffers(scene_set, 1, vk::DescriptorType::STORAGE_BUFFER, &[&dynamic, &statics, &model_table, &visible]);
        for (binding, image) in [(5, &overview), (6, &tiles), (7, &tile_index), (8, &fog), (9, &noise), (10, &panel)] {
            write_image(scene_set, binding, image.view, read);
        }
        write_image(scene_set, 11, shadow.view, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
        for (i, s) in samplers.iter().enumerate() {
            write_sampler(scene_set, 12 + i as u32, *s);
        }
        write_buffers(cull_set, 0, vk::DescriptorType::UNIFORM_BUFFER, &[&globals]);
        write_buffers(cull_set, 1, vk::DescriptorType::STORAGE_BUFFER, &[&dynamic, &statics, &model_table, &slot_table, &vis, &counters, &commands, &visible, &props_dead]);
        write_buffers(nodes_set, 0, vk::DescriptorType::STORAGE_BUFFER, &[&nodes, &nodes]);
        write_buffers(marks_set, 0, vk::DescriptorType::STORAGE_BUFFER, &[&marks, &marks]);
        write_buffers(sprites_set, 0, vk::DescriptorType::STORAGE_BUFFER, &[&projectiles, &effects]);
        write_buffers(stains_set, 0, vk::DescriptorType::STORAGE_BUFFER, &[&stains, &stains]);
        write_image(screen_set, 1, font.view, read);
        write_sampler(screen_set, 2, samplers[1]);

        let shadow_views = [shadow.view];
        let shadow_fb = unsafe {
            gpu.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default().render_pass(passes.shadow).attachments(&shadow_views).width(SHADOW_SIZE).height(SHADOW_SIZE).layers(1),
                None,
            )
        }?;

        let cmd = unsafe {
            gpu.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(gpu.command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))
        }?[0];
        let fence = unsafe { gpu.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None) }?;
        let image_available = unsafe { gpu.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }?;
        let render_finished = unsafe { gpu.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }?;
        let query_pool = unsafe {
            gpu.device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(PASS_NAMES.len() as u32 * 2), None)
        }?;
        let timestamp_period = gpu.limits.timestamp_period;

        let faction = &bps.factions[0];
        let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        let mut team_colors = [[1.0; 4]; 8];
        for (i, c) in scene.team_colors.iter().enumerate() {
            team_colors[i] = rgba(*c);
        }

        let placeholder = |gpu: &Gpu| gpu.image(&ImageDesc { width: 16, height: 16, format: HDR_FORMAT, usage: vk::ImageUsageFlags::SAMPLED, layers: 1, mips: 1, array: false });
        let mut renderer = Renderer {
            output: match surface {
                Some(surface) => Output::Window(Swapchain { surface, swapchain: vk::SwapchainKHR::null(), views: Vec::new(), vsync }),
                None => Output::Headless { image: placeholder(&gpu)?, readback: gpu.host_buffer(16, U::TRANSFER_DST)? },
            },
            hdr: placeholder(&gpu)?,
            depth: placeholder(&gpu)?,
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
            sprites_set,
            stains_set,
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
            projectiles,
            effects,
            stains,
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
            palette: [rgba(faction.plating_color), rgba(faction.accent_color), rgba(faction.highlight_color)],
            team_colors,
            slot_count,
            static_count,
            dynamic_count: 0,
            sim_units: 0,
            projectile_count: 0,
            stain_count: 0,
            effect_cursor: 0,
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
        let rows = if self.font_uploaded { dirty } else { Some(0..textures::FONT_ATLAS_H) };
        let Some(rows) = rows else { return Ok(()) };
        let region = vk::Rect2D { offset: vk::Offset2D { x: 0, y: rows.start as i32 }, extent: vk::Extent2D { width: textures::FONT_ATLAS_W as u32, height: rows.len() as u32 } };
        let bytes = &overlay.atlas()[rows.start * textures::FONT_ATLAS_W * 4..rows.end * textures::FONT_ATLAS_W * 4];
        self.gpu.upload_image(&self.font, 0, 0, Some(region), bytes, !self.font_uploaded)?;
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
        }
        let mut present_views: Vec<vk::ImageView> = Vec::new();
        match &mut self.output {
            Output::Window(sc) => {
                let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target has the swapchain extension");
                let caps = unsafe { self.gpu.surface_fn.get_physical_device_surface_capabilities(self.gpu.physical, sc.surface) }?;
                if caps.current_extent.width != u32::MAX {
                    self.width = caps.current_extent.width.max(1);
                    self.height = caps.current_extent.height.max(1);
                }
                let modes = unsafe { self.gpu.surface_fn.get_physical_device_surface_present_modes(self.gpu.physical, sc.surface) }?;
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
                    .image_extent(vk::Extent2D { width: self.width, height: self.height })
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
                    let view_info = vk::ImageViewCreateInfo::default().image(image).view_type(vk::ImageViewType::TYPE_2D).format(self.present_format).subresource_range(
                        vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 },
                    );
                    sc.views.push(unsafe { device.create_image_view(&view_info, None) }?);
                }
                present_views.extend_from_slice(&sc.views);
            }
            Output::Headless { image, readback } => {
                let new_image = self.gpu.image(&ImageDesc {
                    width: self.width,
                    height: self.height,
                    format: self.present_format,
                    usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                    layers: 1,
                    mips: 1,
                    array: false,
                })?;
                let new_readback = self.gpu.host_buffer(self.width as u64 * self.height as u64 * 4, vk::BufferUsageFlags::TRANSFER_DST)?;
                self.gpu.destroy_image(std::mem::replace(image, new_image));
                self.gpu.destroy_buffer(std::mem::replace(readback, new_readback));
                present_views.push(image.view);
            }
        }

        let attachment = |format, usage| ImageDesc { width: self.width, height: self.height, format, usage, layers: 1, mips: 1, array: false };
        let hdr = self.gpu.image(&attachment(HDR_FORMAT, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED))?;
        let depth = self.gpu.image(&attachment(DEPTH_FORMAT, vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT))?;
        self.gpu.destroy_image(std::mem::replace(&mut self.hdr, hdr));
        self.gpu.destroy_image(std::mem::replace(&mut self.depth, depth));

        let device = &self.gpu.device;
        let views = [self.hdr.view, self.depth.view];
        self.scene_fb = unsafe {
            device.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(self.passes.scene).attachments(&views).width(self.width).height(self.height).layers(1), None)
        }?;
        for view in present_views {
            let views = [view];
            let fb = unsafe {
                device.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(self.passes.present).attachments(&views).width(self.width).height(self.height).layers(1), None)
            }?;
            self.present_fbs.push(fb);
        }
        let info = [vk::DescriptorImageInfo { sampler: vk::Sampler::null(), image_view: self.hdr.view, image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL }];
        let write = [vk::WriteDescriptorSet::default().dst_set(self.screen_set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&info)];
        unsafe { device.update_descriptor_sets(&write, &[]) };
        self.queries_valid = false;
        Ok(())
    }

    /// Copies a new tick's mirror into GPU memory. Bulk copies only.
    fn upload_sim(&mut self, frame: &RenderFrame, time: f32) {
        let units = &frame.units[..frame.units.len().min(MAX_DYNAMIC - 512)];
        if units.len() < frame.units.len() {
            log::error!("render mirror has {} entities; the renderer holds {}", frame.units.len(), units.len());
        }
        self.dynamic.write(0, bytemuck::cast_slice(units));
        self.sim_units = units.len() as u32;

        let projectiles = &frame.projectiles[..frame.projectiles.len().min(MAX_PROJECTILES)];
        self.projectiles.write(0, bytemuck::cast_slice(projectiles));
        self.projectile_count = projectiles.len() as u32;

        let stains = &frame.stains[..frame.stains.len().min(MAX_STAINS)];
        self.stains.write(0, bytemuck::cast_slice(stains));
        self.stain_count = stains.len() as u32;

        let dead_bytes: &[u8] = bytemuck::cast_slice(&frame.props_dead);
        self.props_dead.write(0, &dead_bytes[..dead_bytes.len().min(self.props_dead.size as usize)]);

        for event in &frame.events {
            let effect = match event {
                SimEvent::ShotFired { pos, color, .. } => Some((pos.to_f32(), 3.5, 0.12, *color as u32 as f32)),
                SimEvent::Impact { pos, splash, color } => Some((pos.to_f32(), 5.0 + splash.to_f32() * 1.2, 0.45, *color as u32 as f32)),
                SimEvent::UnitDied { pos, blueprint, .. } => {
                    let r = self.scene_radius(blueprint.0 as usize);
                    Some((pos.to_f32(), r * 3.0, 1.1, 2.0))
                }
                _ => None,
            };
            if let Some((pos, radius, life, kind)) = effect {
                let e = Effect { pos, start: time, params: [radius, life, kind, 0.0] };
                self.effects.write((self.effect_cursor * size_of::<Effect>()) as u64, bytemuck::bytes_of(&e));
                self.effect_cursor = (self.effect_cursor + 1) % MAX_EFFECTS;
            }
        }
    }

    fn scene_radius(&self, _blueprint: usize) -> f32 {
        8.0
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
                match unsafe { swapchain_fn.acquire_next_image(sc.swapchain, u64::MAX, self.image_available, vk::Fence::null()) } {
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
            self.upload_sim(frame, input.time);
            self.fog_enabled = !frame.fog.is_empty();
            self.tile_cache.apply_edits(&frame.terrain_edits, &mut self.upload_scratch);
        }
        let ghosts = &input.ghosts[..input.ghosts.len().min(512)];
        self.dynamic.write((self.sim_units as usize * size_of::<UnitInstance>()) as u64, bytemuck::cast_slice(ghosts));
        self.dynamic_count = self.sim_units + ghosts.len() as u32;

        let z_range = (self.map_info.min_z.to_f32(), self.map_info.min_z.to_f32() + self.map_info.z_step.to_f32() * 65535.0);
        let z_used = (z_range.0.max(-300.0), z_range.1.min(900.0));
        if !terrain::select_nodes(camera, z_used, &mut self.node_scratch) {
            log::error!("terrain node budget of {MAX_NODES} exceeded; distant terrain is missing this frame");
        }
        self.nodes.write(0, bytemuck::cast_slice(&self.node_scratch));
        self.tile_cache.update(camera, &self.pool, &mut self.upload_scratch);

        let marks: Vec<Mark> = input
            .marks
            .iter()
            .take(MAX_MARKS)
            .filter(|m| m.unit_index < self.sim_units)
            .map(|m| Mark { unit_index: m.unit_index | 0x8000_0000, kind: m.kind })
            .collect();
        self.marks.write(0, bytemuck::cast_slice(&marks));
        let overlay = &input.overlay.vertices[..input.overlay.vertices.len().min(MAX_OVERLAY_VERTICES)];
        self.overlay_vb.write(0, bytemuck::cast_slice(overlay));
        self.upload_overlay_atlas(input.overlay)?;

        // Sun, and a shadow box around what the camera is looking at.
        let sun = Vec3::new(0.45, -0.35, 0.82).normalize();
        let shadow_strength = 1.0 - ((camera.distance - 3000.0) / 5000.0).clamp(0.0, 1.0);
        let extent = (camera.distance * 1.4).clamp(150.0, 9000.0);
        let light_view = glam::camera::rh::view::look_at_mat4(camera.focus + sun * extent * 2.0, camera.focus, Vec3::Z);
        let shadow_view_proj: Mat4 = glam::camera::rh::proj::directx::orthographic(-extent, extent, -extent, extent, 1.0, extent * 4.0) * light_view;

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
            viewport: [self.width as f32, self.height as f32, 1.0 / self.width as f32, 1.0 / self.height as f32],
            frustum: camera.frustum().map(|p| p.to_array()),
            map: [size[0], size[1], self.map_info.water_level.to_f32(), shadow_strength],
            height: [z_range.0, z_range.1 - z_range.0, tw as f32, th as f32],
            lod: [camera.projection_scale(), icon_px, 90.0, 28.0],
            counts: [self.dynamic_count, self.static_count, self.slot_count, self.fog_enabled as u32],
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
            device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            device.cmd_reset_query_pool(cmd, self.query_pool, 0, PASS_NAMES.len() as u32 * 2);
        }
        self.record_uploads(cmd, input.sim)?;

        let stamp = |q: u32| unsafe { device.cmd_write_timestamp(cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.query_pool, q) };
        let compute_barrier = |dst: vk::AccessFlags, dst_stage: vk::PipelineStageFlags| unsafe {
            let barrier = [vk::MemoryBarrier::default().src_access_mask(vk::AccessFlags::SHADER_WRITE).dst_access_mask(dst)];
            device.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::COMPUTE_SHADER, dst_stage, vk::DependencyFlags::empty(), &barrier, &[], &[]);
        };
        let rw = vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE;

        // GPU culling and draw generation.
        stamp(0);
        unsafe {
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, self.layouts.cull, 0, &[self.cull_set], &[]);
            let dispatch = |pipeline: vk::Pipeline, count: u32, dynamic: u32| {
                if count == 0 {
                    return;
                }
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
                device.cmd_push_constants(cmd, self.layouts.cull, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(&[count, dynamic]));
                device.cmd_dispatch(cmd, count.div_ceil(64), 1, 1);
            };
            dispatch(self.pipelines.cull_clear, self.slot_count, 0);
            compute_barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
            dispatch(self.pipelines.cull_cull, self.static_count, 0);
            dispatch(self.pipelines.cull_cull, self.dynamic_count, 1);
            compute_barrier(rw, vk::PipelineStageFlags::COMPUTE_SHADER);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.pipelines.cull_prefix);
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
            device.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
            device.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } }]);
        };
        let bind_pass_set = |set: vk::DescriptorSet| unsafe {
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layouts.scene, 1, &[set], &[]);
        };
        let push = |a: u32, b: u32| unsafe { device.cmd_push_constants(cmd, self.layouts.scene, gfx, 0, bytemuck::bytes_of(&[a, b])) };
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
            let clear = [vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.shadow)
                .framebuffer(self.shadow_fb)
                .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: SHADOW_SIZE, height: SHADOW_SIZE } })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            if shadow_strength > 0.0 {
                set_viewport(SHADOW_SIZE, SHADOW_SIZE);
                device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layouts.scene, 0, &[self.scene_set], &[]);
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
                vk::ClearValue { color: vk::ClearColorValue { float32: [0.55, 0.66, 0.8, 1.0] } },
                vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 } },
            ];
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.scene)
                .framebuffer(self.scene_fb)
                .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: self.width, height: self.height } })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.width, self.height);
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layouts.scene, 0, &[self.scene_set], &[]);
            draw_terrain(self.pipelines.terrain, 0);

            if self.stain_count > 0 {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.stain);
                bind_pass_set(self.stains_set);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.patch_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.patch_ib.buffer, 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed(cmd, self.patch_index_count, self.stain_count, 0, 0, 0);
            }
            draw_entities(self.pipelines.entity, 0);

            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.water);
            device.cmd_draw(cmd, 6, 1, 0, 0);

            draw_quads(self.pipelines.ring, self.marks_set, marks.len() as u32);
            draw_quads(self.pipelines.projectile, self.sprites_set, self.projectile_count);
            draw_quads(self.pipelines.effect, self.sprites_set, MAX_EFFECTS as u32);

            // Strategic icons: the cull pass's last draw slot, as quads.
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.icon);
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.quad_vb.buffer], &[0]);
            device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed_indirect(cmd, self.commands.buffer, model_slots as u64 * 20, 1, 20);
            draw_quads(self.pipelines.bar, self.marks_set, marks.len() as u32);
            device.cmd_end_render_pass(cmd);
        }
        stamp(5);

        // Tone map to the output, then the UI on top.
        stamp(6);
        unsafe {
            let begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.passes.present)
                .framebuffer(self.present_fbs[image_index])
                .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: self.width, height: self.height } });
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.width, self.height);
            device.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.layouts.screen, 0, &[self.screen_set], &[]);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.tonemap);
            device.cmd_push_constants(cmd, self.layouts.screen, gfx, 0, bytemuck::bytes_of(&[1.0f32, 0.35]));
            device.cmd_draw(cmd, 3, 1, 0, 0);
            if !overlay.is_empty() {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.overlay);
                device.cmd_push_constants(cmd, self.layouts.screen, gfx, 0, bytemuck::bytes_of(&[2.0 / self.width as f32, 2.0 / self.height as f32]));
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.overlay_vb.buffer], &[0]);
                device.cmd_draw(cmd, overlay.len() as u32, 1, 0, 0);
            }
            device.cmd_end_render_pass(cmd);
        }
        stamp(7);

        if let Output::Headless { image, readback } = &self.output {
            unsafe {
                let copy = [vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 })
                    .image_extent(vk::Extent3D { width: self.width, height: self.height, depth: 1 })];
                device.cmd_copy_image_to_buffer(cmd, image.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, readback.buffer, &copy);
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
                submit = submit.wait_semaphores(&wait).wait_dst_stage_mask(&stages).signal_semaphores(&signal);
            }
            device.queue_submit(self.gpu.queue, &[submit], self.fence)?;
        }
        self.queries_valid = true;

        if let Output::Window(sc) = &self.output {
            let swapchain_fn = self.gpu.swapchain_fn.as_ref().expect("window target");
            let wait = [self.render_finished];
            let swapchains = [sc.swapchain];
            let indices = [image_index as u32];
            let present = vk::PresentInfoKHR::default().wait_semaphores(&wait).swapchains(&swapchains).image_indices(&indices);
            match unsafe { swapchain_fn.queue_present(self.gpu.queue, &present) } {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.create_size_dependent()?,
                Err(e) => return Err(e.into()),
            }
        }

        self.stats.terrain_nodes = self.node_scratch.len();
        self.stats.dynamic_entities = self.dynamic_count as usize;
        self.stats.static_entities = self.static_count as usize;
        Ok(true)
    }

    /// Terrain tile/patch uploads and the fog texture, through one staging buffer.
    fn record_uploads(&mut self, cmd: vk::CommandBuffer, sim: Option<&RenderFrame>) -> Result<(), GpuError> {
        let uploads = std::mem::take(&mut self.upload_scratch);
        let fog = sim.map(|f| &f.fog).filter(|f| f.len() == (self.fog_dims.0 * self.fog_dims.1 * 2) as usize);
        let mut bytes: Vec<u8> = Vec::new();
        let mut copies: Vec<(u64, &Image, u32, Option<vk::Rect2D>)> = Vec::new();
        let rect = |x: u32, y: u32, w: u32, h: u32| Some(vk::Rect2D { offset: vk::Offset2D { x: x as i32, y: y as i32 }, extent: vk::Extent2D { width: w, height: h } });
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
                TerrainUpload::TilePatch { layer, x, y, w, h, sample } => {
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
            let staging = self.gpu.host_buffer(bytes.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
            staging.write(0, &bytes);
            for (offset, image, layer, region) in copies {
                self.gpu.record_image_upload(cmd, image, layer, 0, region, staging.buffer, offset, false);
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
        let ok = unsafe { self.gpu.device.get_query_pool_results(self.query_pool, 0, &mut ticks, vk::QueryResultFlags::TYPE_64) };
        if ok.is_ok() {
            self.stats.gpu_passes.clear();
            for (i, name) in PASS_NAMES.iter().enumerate() {
                let ns = ticks[i * 2 + 1].saturating_sub(ticks[i * 2]) as f32 * self.timestamp_period;
                self.stats.gpu_passes.push((name, ns / 1.0e6));
            }
        }
    }

    /// Headless only: the last rendered frame as tightly packed RGBA8.
    pub fn read_pixels(&mut self) -> Option<Vec<u8>> {
        unsafe { self.gpu.device.wait_for_fences(&[self.fence], true, u64::MAX).ok()? };
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
            &mut self.globals, &mut self.dynamic, &mut self.statics, &mut self.model_table, &mut self.slot_table, &mut self.vis,
            &mut self.counters, &mut self.commands, &mut self.visible, &mut self.props_dead, &mut self.nodes, &mut self.marks,
            &mut self.projectiles, &mut self.effects, &mut self.stains, &mut self.overlay_vb, &mut self.mesh_vb, &mut self.mesh_ib,
            &mut self.quad_vb, &mut self.quad_ib, &mut self.grid_vb, &mut self.grid_ib, &mut self.patch_vb, &mut self.patch_ib,
        ] {
            gpu.destroy_buffer(std::mem::replace(b, placeholder()));
        }
        for b in self.garbage.drain(..) {
            gpu.destroy_buffer(b);
        }
        for image in [&self.hdr, &self.depth, &self.shadow, &self.overview, &self.tiles, &self.tile_index, &self.fog, &self.noise, &self.panel, &self.font] {
            gpu.destroy_image_ref(image);
        }
        if let Output::Headless { image, readback } = &mut self.output {
            gpu.destroy_image_ref(image);
            gpu.destroy_buffer(std::mem::replace(readback, placeholder()));
        }
        // `self.gpu` drops last and destroys the device and instance.
    }
}
