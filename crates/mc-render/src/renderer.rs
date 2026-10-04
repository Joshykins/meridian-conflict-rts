//! Frame orchestration.
//!
//! Per frame the CPU writes one uniform block, the terrain node list, and
//! whatever the UI drew. Per sim tick it copies the render mirror into GPU
//! buffers wholesale. Everything per-entity (interpolation, culling, LOD,
//! draw generation, animation) runs in shaders.

use crate::camera::Camera;
use crate::descriptors::SetPool;
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::gpu_consts::{cull_list, fade_beam, lod, pass, settle, sprite_layer};
use crate::ground_cover;
use crate::models::{self, Legs, MeshVertex, Model, Treads};
use crate::overlay::{Overlay, OverlayVertex, MAX_OVERLAY_VERTICES};
use crate::pipelines::{
    Layouts, Passes, Pipelines, CULL_SET, DEPTH_FORMAT, HDR_FORMAT, PASS_SET, SCENE_SET, SCREEN_SET,
};
use crate::swapchain;
use crate::terrain::{self, TerrainNode, TerrainUpload, TileCache, MAX_NODES, TILE_LAYERS};
use crate::textures;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{MapFile, Prop, PropKind, BUILD_CELL_M, TILE_SAMPLES};
use mc_sim::mirror::{
    FireInstance, ProjectileInstance, RenderFrame, SimEvent, StainInstance, UnitInstance,
    KIND_GHOST, KIND_PROP, KIND_WRECK, MAX_CONSTRUCTION_WELDS, PROJECTILE_APOGEE, PROJECTILE_BEAM,
    PROJECTILE_COLD, PROJECTILE_ENDS_SHIFT, PROJECTILE_FADE_BEAM, PROJECTILE_FRESH,
    PROJECTILE_MISSILE, PROJECTILE_SKIM, PROJECTILE_SMOKE, PROJECTILE_TRAIL, STATE_RADAR,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::collections::HashMap;
use std::sync::Arc;

mod active_draws;
pub(crate) mod adjacency_links;
mod aircraft_trails;
mod arc_howitzer_fx;
mod blast_fx;
mod bolt_rifle_fx;
mod bore_fx;
mod capital_crash_fx;
mod capital_fx;
mod capture;
mod casing_fx;
mod damper_fx;
mod dive_fx;
mod drive_swing;
pub use capture::Shot;
mod city_fx;
mod clearing;
mod cliff_rocks;
mod cluster_fx;
mod craters;
mod crush_fx;
mod cull_lists;
mod effect_barriers;
mod fallen_trees;
mod flak_fx;
pub(crate) mod fog_field;
mod footfalls;
pub(crate) mod foundations;
mod frame;
pub(crate) mod grass;
mod gravitic_fx;
mod great_gun_fx;
mod ground_contact;
mod ground_melt;
mod gtao;
pub(crate) mod heat_haze;
mod heavy_rail_fx;
mod hull_frame;
mod impact_craters;
mod impact_fx;
mod lance_core_fx;
mod laser_fx;
mod launch_fx;
pub(crate) mod lens_flare;
mod lift_fx;
mod map_look;
mod mine_fx;
mod nuke_fx;
mod nuke_volume;
mod ore_fields;
mod plasma_drive_fx;
mod plasma_fx;
mod post;
mod quality;
mod rail_fx;
mod reactor_blast;
mod reactor_fx;
mod regency_guns_fx;
mod regency_mine_fx;
mod shafts;
mod shield_upload;
mod stake_fx;
mod star_core_fx;
mod structure_pads;
mod stun_fx;
mod supernova_fx;
mod survival_fx;
mod terrain_lit;
mod trail_fx;
mod tree_wind;
mod wake_fx;
pub(crate) mod wake_shell;
mod warp_fx;
mod water_fx;
mod work_beams;
mod wreck_finish;
mod wreck_fx;
pub(crate) use effect_barriers::EffectBarrier;
pub use ore_fields::OreClaim;
pub use post::Antialiasing;
pub use quality::SceneQuality;
mod breadcrumbs;
mod gpu_timers;
mod shadow_cascades;
mod shadow_map;
mod titan_charge;
mod titan_fx;
pub use gpu_timers::{to_perf as gpu_scopes_to_perf, DrawStats, GpuScope};

/// Everything the mirror can hand over: every unit and wreck the sim's tables hold,
/// and the extra sections broken wrecks are drawn in.
const MAX_SIM_ENTITIES: usize =
    mc_sim::tables::MAX_UNITS + mc_sim::tables::MAX_WRECKS + mc_sim::mirror::WRECK_EXTRA_INSTANCES;
/// Placement ghosts drawn at once: a drag line longer than this shows its first ones (a
/// cosmetic cap; the orders are not cut).
const MAX_GHOSTS: usize = 512;
/// The dynamic buffer: the mirror's entities, then ghosts, burning trees, fallen ones and
/// reclaimed hulls going, each with room for its own most, so raising a sim table raises
/// this with it.
pub const MAX_DYNAMIC: usize = MAX_SIM_ENTITIES
    + MAX_GHOSTS
    + MAX_BURNING_TREES
    + fallen_trees::MOST_SHOWN
    + wreck_finish::MOST_SHOWN
    + city_fx::MOST_FALLING;
/// Gun-house poses (`mirror::HousePose`): at most one per unit, so every unit can have one.
pub const MAX_HOUSES: usize = mc_sim::tables::MAX_UNITS;
/// Selection rings and status bars: at most one per unit or wreck drawn.
pub const MAX_MARKS: usize = mc_sim::tables::MAX_UNITS + mc_sim::tables::MAX_WRECKS;
pub const MAX_EFFECTS: usize = 2048;
/// Expanding 3D pressure spheres. The oldest are overwritten.
pub const MAX_SHOCKWAVES: usize = 64;
/// Must match the missile mesh dimensions in sprites.wgsl (`missile_half_length`): a
/// rocket sized to its tube (`caliber` across) or, with none, from its `size`.
fn missile_half_length(size: f32, caliber: f32) -> f32 {
    if caliber > 0.0 {
        caliber / 0.28
    } else {
        (size * 1.4).clamp(1.4, 4.8)
    }
}

/// UV-sphere tessellation for a shockwave shell. Must match `shockwaves.wgsl`.
const SHOCKWAVE_LAT: u32 = 32;
const SHOCKWAVE_LON: u32 = 64;
/// Rings of short-lived particles and of track marks; the oldest are overwritten.
// Three times what it was: a giant's salvo of heavy rockets lays a dozen trail puffs each a
// tick, and at 8192 the ring came round in two seconds and ate every longer-lived puff
// (rocket trails, the storm's thunderhead) long before its time.
pub const MAX_PUFFS: usize = 24576 + nuke_fx::NUKE_PUFF_SLOTS;
/// Tail of the puff buffer. Smoke over a fire is rewritten here every frame,
/// so a carpet cannot wrap the ring and erase a column halfway through.
const GROUND_FIRE_SLOTS: usize = 512;
/// Trees burning at once, at most (a nuclear blast lights a forest).
const MAX_BURNING_TREES: usize = 1536;
/// The ring every puff goes round; then the ground fires' slots, then the missile trails'
/// (`nuke_fx::NUKE_PUFF_SLOTS`).
const PUFF_RING: usize = MAX_PUFFS - GROUND_FIRE_SLOTS - nuke_fx::NUKE_PUFF_SLOTS;
const MAX_GROUND_FIRES: usize = 256;
/// Tail of the effect buffer, rewritten every frame so a fire is not pushed out of the ring.
const EFFECT_RING: usize = MAX_EFFECTS - MAX_GROUND_FIRES;
pub const MAX_SHIELDS: usize = 256;
pub const MAX_SHIELD_HITS: usize = 64;
/// Hulls a dome may light a contact line on. The fragment shader samples
/// each hull's baked mesh plan; this list is only "who is near the shell".
const SHIELD_CONTACTS: usize = 16;
/// Matches `PAD` in `shields.wgsl`.
const SHIELD_PAD: f32 = 2.0;
pub const MAX_TRACK_MARKS: usize = 32768;
/// Giants' footprints, in a ring of their own after the track marks (`titan_fx::Prints`).
const MAX_PRINTS: usize = 256;
/// Seconds a track mark stays on the ground.
const TRACK_MARK_LIFE: f32 = 50.0;
/// Levels of the bloom chain, each half the size of the one before.
const BLOOM_LEVELS: usize = 5;
/// Tone mapping: exposure, vignette strength. Overlay glass copies the picture with them.
const TONEMAP: [f32; 2] = [0.86, 0.25];
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
    pub team_colors: [[f32; 3]; mc_core::MAX_PLAYERS],
}

/// Selection / hover marker on an entity of the current render frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Mark {
    /// Index into `RenderFrame::units`.
    pub unit_index: u32,
    /// `gpu_consts::mark` bits: hovered (else selected), enemy, reclaim target, or
    /// [`Mark::BARS_ONLY`]: no selection ring, only the status bars (work under way,
    /// seen unselected).
    pub kind: u32,
    /// Construction fill, zero to one. Negative: the unit is not building, so
    /// the bar under health stays off.
    pub work: f32,
    /// Shield fill, zero to one. Negative: the unit has no bubble, so the
    /// line above health stays off.
    pub shield: f32,
}

impl Mark {
    /// [`Mark::kind`] bit: the unit is not selected or hovered, it only shows its bars.
    pub const BARS_ONLY: u32 = crate::gpu_consts::mark::BARS_ONLY;
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
    /// World angle (radians, as unit headings) the ring's arc is centred on.
    pub facing: f32,
    /// Radians either side of `facing` the ring reaches; pi or more is all the way round.
    /// A part ring is drawn as its arcs and the two edges from the centre out.
    pub half_arc: f32,
    pub _pad: [f32; 2],
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
    /// Strategic icons drawn. Off (the free camera), every unit keeps its model
    /// at any distance and radar blips are not shown.
    pub icons: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// GPU time per pass in milliseconds, from timestamp queries of the previous frame.
    pub gpu_passes: Vec<(&'static str, f32)>,
    /// Every named GPU scope of the previous frame, nested, with triangle and
    /// fragment counts for draw scopes while statistics are on.
    pub gpu_scopes: Vec<GpuScope>,
    pub terrain_nodes: usize,
    pub dynamic_entities: usize,
    pub static_entities: usize,
    pub tiles_resident: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Globals {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) inv_view_proj: [[f32; 4]; 4],
    pub(crate) shadow_view_proj: [[f32; 4]; 4],
    pub(crate) camera: [f32; 4],
    pub(crate) sun: [f32; 4],
    pub(crate) viewport: [f32; 4],
    pub(crate) frustum: [[f32; 4]; 6],
    pub(crate) map: [f32; 4],
    pub(crate) height: [f32; 4],
    pub(crate) lod: [f32; 4],
    pub(crate) counts: [u32; 4],
    pub(crate) plating: [f32; 4],
    pub(crate) accent: [f32; 4],
    pub(crate) glow: [f32; 4],
    pub(crate) team_colors: [[f32; 4]; crate::gpu_consts::owner::COLORS as usize],
    pub(crate) build_cursor: [f32; 4],
    pub(crate) build_blocked: [[f32; 4]; BUILD_BLOCKED_MAX],
    /// The 3D scene's size in pixels, the render scale, and how far the selection's
    /// see-through is in (`Sky::clear_strength`, the Precursor cutaway in entity.wgsl).
    /// `viewport` stays the output's size: pixel widths are output pixels.
    pub(crate) scene: [f32; 4],
    /// x how many of `tree_blasts` are in use (tree_wind.rs); yz the camera's focus
    /// (the Precursor cutaway, entity.wgsl); w how awake a survival map's Precursor
    /// facility is (0 on any other map: its light as authored, no cutaway).
    pub(crate) tree_wind: [f32; 4],

    pub(crate) tree_blasts: [[f32; 4]; tree_wind::TREE_BLASTS * 2],
    /// The sun's shadow cascades (shadow_cascades.rs), near to far.
    pub(crate) shadow_cascades: [[[f32; 4]; 4]; shadow_cascades::CASCADES],
    /// Per cascade: metres per texel, metres of depth.
    pub(crate) shadow_info: [[f32; 4]; shadow_cascades::CASCADES],
    /// The faction's shield colour (shields.wgsl).
    pub(crate) shield: [f32; 4],
    /// Nuclear blasts drawn as volumes (nuke_fx.rs, nuke.wgsl), three vec4 each.
    pub(crate) nukes: [[f32; 4]; nuke_fx::NUKE_SLOTS * 4],
    /// x the flash whiting the view out, y the scene dimmed after it, z blasts in `nukes`,
    /// w missiles in `strategic`.
    pub(crate) nuke_view: [f32; 4],
    /// Strategic missiles in flight: nose and kind, then axis and heat (nuke_fx.rs).
    pub(crate) strategic: [[f32; 4]; nuke_fx::MISSILE_SLOTS * 2],
    /// x the map's climate, region 0's on a map with regions: 0 temperate, 1 tropical,
    /// 2 desert (map_look.rs; `climate_at` in bindings.wgsl);
    /// y 1 while grass is grown (grass.rs), so the ground under it is shaded for it;
    /// z how far from the eye it grows (`grass::reach`);
    /// w how many sim ticks this frame covers (the treads' motion blur, entity.wgsl).
    pub(crate) climate: [f32; 4],
    /// Prop detail: common.wgsl `Globals::detail`.
    pub(crate) detail: [f32; 4],
    /// Lots settling into the ground (terrain.rs `TileCache::settling`): rect, then
    /// level and progress.
    pub(crate) settling: [[f32; 4]; settle::SLOTS as usize * 2],
    /// x how many of `settling` are in use.
    pub(crate) settle: [f32; 4],
    /// Each region's climate (map_look.rs): x 1 where it is tropical, y 1 where desert.
    pub(crate) region_climate: [[f32; 4]; crate::gpu_consts::regions::MAX as usize],
    /// x the map's `strata_lift` in metres.
    pub(crate) map_look: [f32; 4],
    /// The mines' reaches the ore dims under (`ore_fields.rs`): x the first's
    /// index in the stains, y how many.
    pub(crate) ore_claims: [u32; 4],
}

/// Lots the build grid shows as taken, at most.
pub const BUILD_BLOCKED_MAX: usize = 48;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ModelInfo {
    pub(crate) slot: u32,
    pub(crate) icon: u32,
    pub(crate) bounds_radius: f32,
    pub(crate) height: f32,
    /// Metres of the hull-plan atlas: [-1, 1] in plan UV is this square.
    pub(crate) plan_half: f32,
    /// The refit modules on show (`Blueprints::look`), one bit each.
    pub(crate) modules: u32,
    /// A pit dug into the ground (`Model::pit`): its opening's height and radius. Zero for none.
    pub(crate) pit: [f32; 2],
    pub(crate) turret_pivot: [f32; 4],
    /// w: how far a `part::RAM` pile driver is hauled up (`models::Pit::stroke`).
    pub(crate) spinner_pivot: [f32; 4],
    /// `Legs`: hip and the stride, knee and the lift, ankle and the stance. All zero for a model without legs.
    pub(crate) leg_hip: [f32; 4],
    pub(crate) leg_knee: [f32; 4],
    pub(crate) leg_ankle: [f32; 4],
    /// The left elbow forearms pitch about; w is one when the model has one.
    pub(crate) arm_pivot: [f32; 4],
    /// Rest-space barrel axis and recoil travel; zero if the tube does not slide.
    pub(crate) recoil: [f32; 4],
    /// Hinge of the folding gear and its stowed angle; zero if there is none.
    pub(crate) fold: [f32; 4],
    /// Trunnion of a mounted turret and its tube's kick-back; zero if there is none.
    pub(crate) mount: [f32; 4],
    /// The rotary barrels' axis for this loadout (a point on it; it runs along x), w 1 when there is one.
    pub(crate) spin: [f32; 4],
    /// Wrist of the head on the folding gear and its stowed angle; zero if there is none.
    pub(crate) fold_wrist: [f32; 4],
    /// A pit's pipe feed (`models::Pit`): where the next section waits (xy) and the
    /// section's length; w is unused. Zero for none.
    pub(crate) pit_feed: [f32; 4],
    /// How the surface shader sizes the model (`Model::surface_reach`), the height
    /// its field dust reaches (`Model::dust_line`), how far a walker's hips sink in
    /// stride (`Legs::crouch`) and its neck's height (`Model::neck`, zero for none).
    pub(crate) surface: [f32; 4],
    /// Gun houses of their own (`rig::HOUSE_FIRST + i`): pivot and kick-back travel.
    pub(crate) houses: [[f32; 4]; 4],
    /// Which weapon each house is bound to, plus one; zero for no house in that slot.
    pub(crate) house_weapon: [f32; 4],
    /// A spacecraft's rig for `entity.wgsl` (`models::capital_rig`): gear legs, bay doors,
    /// drives, lift jets, ramp. All zero for any other model.
    pub(crate) capital: [[f32; 4]; 7],
    /// Houses 4..8 (`rig::HOUSE_HIGH`), as `houses` and `house_weapon`.
    pub(crate) houses_high: [[f32; 4]; 4],
    pub(crate) house_weapon_high: [f32; 4],
    /// Where a personal (hull) shield is thrown from (`Model::shield_emitter`), w 1 when
    /// the model says; zero for the default, the top of the hull over the middle.
    pub(crate) shield_emitter: [f32; 4],
    /// A many-legged walker (`models::Crawl::gpu`): pair count and tail heights, then each
    /// pair's hip (w: phase), knee and foot. All zero for any other model.
    pub(crate) crawl: [[f32; 4]; models::CRAWL_SLOTS],
    /// A reverse-kneed walker's hock (`Legs::hock`) and how much of the swing the tarsus
    /// follows (w). All zero for any other model.
    pub(crate) leg_hock: [f32; 4],
    /// A gun's breech door (`Model::breech`): hinge and open angle. Zero for none.
    pub(crate) breech: [f32; 4],
    /// The box round the hull plan (`models::hull_plan_box`): centre xy and half-extents
    /// zw, metres in the model's frame. The selection mark is fitted to it.
    pub(crate) plan_box: [f32; 4],
    /// A VTOL's pods (`models::Vtol::gpu`): front pivot and kind, rear pivot and the
    /// nozzle's distance behind its pivot. All zero for any other model.
    pub(crate) vtol: [[f32; 4]; 2],
    /// Hatched missile cells (`models::CellBlock::gpu`): per block its centre, deck, pitch
    /// and hatch half width. All zero for none.
    pub(crate) cells: [[f32; 4]; 4],
    /// Per block its grid word, then per block its missile order word.
    pub(crate) cell_grid: [u32; 4],
    /// A charge gun's working gear (`Model::charge_gear`): hub and travel scale. Zero for none.
    pub(crate) charge_gear: [f32; 4],
    /// How a walker's hull rides its stride (`Legs::sway`): roll, nod and settle; w 0.
    pub(crate) leg_sway: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<ModelInfo>() == 1056);

// A prop's far level is the draw slot after its own levels.
const _: () = assert!(models::LOD_COUNT as u32 == lod::FAR);

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
pub(crate) struct Effect {
    pub(crate) origin: [f32; 4],
    pub(crate) pos: [f32; 3],
    pub(crate) start: f32,
    pub(crate) params: [f32; 4],
}

/// GPU shockwave. `axis` is the barrel direction for a muzzle blast; zero
/// for an isotropic sphere (impact, death, a dome going).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuShockwave {
    pub(crate) pos: [f32; 3],
    pub(crate) start: f32,
    pub(crate) params: [f32; 4],
    pub(crate) axis: [f32; 3],
    pub(crate) _pad: f32,
    pub(crate) tint: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<GpuShockwave>() == 64);

/// Mirrors `Puff` in shaders/puffs.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Puff {
    pub(crate) origin: [f32; 3],
    pub(crate) opacity: f32,
    pub(crate) pos: [f32; 3],
    pub(crate) start: f32,
    pub(crate) vel: [f32; 3],
    pub(crate) life: f32,
    /// Size at birth, size at the end, kind, seed.
    pub(crate) params: [f32; 4],
    /// Custom dust RGB (negative means natural color), brightness.
    pub(crate) appearance: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<Puff>() == 80);

/// One missile or energy slug's flown path, so a trail skipped while the camera
/// was far can be lit in full when the view comes in.
struct TrailPath {
    /// Head position after each tick, and the render-clock time of that tick.
    points: Vec<(Vec3, f32)>,
    /// How many points already have puffs; 0 means none, including the history.
    spawned: usize,
}

/// A fading line along a shot's path (a rail slug), lingering after the tick.
struct FadeBeam {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
    /// How it is drawn: its colour under `PROJECTILE_FADE_BEAM` (sprites.wgsl), one of
    /// the `FADE_*` looks or `fade_beam::GRAVITY_TETHER`.
    kind: u32,
}

/// A plain fading beam: a pale blue line.
const FADE_PLAIN: u32 = 0;
/// A red intercept laser shot: struck at full brightness, fading fast (sprites.wgsl beam
/// colour 1).
const FADE_LASER: u32 = 1;
/// A rail slug's path: white-hot, cooling to orange (sprites.wgsl beam colour 5).
const FADE_RAIL: u32 = 5;

/// A hitscan shot waiting for its impact so the beam can run muzzle to hit.
struct PendingRail {
    muzzle: Vec3,
    dir: Vec3,
    range: f32,
    width: f32,
    /// A rail slug (an orange hitscan gun): a hot streak and a vapour trail, not a blue beam.
    hot: bool,
}

/// How well a hit at `at` lines up with a shot from `muzzle` along `dir`: lower is
/// better, `None` behind the muzzle.
fn beam_score(muzzle: Vec3, dir: Vec3, range: f32, at: Vec3) -> Option<f32> {
    let delta = at - muzzle;
    let along = delta.dot(dir);
    if along < 1.0 {
        return None;
    }
    let lateral = (delta - dir * along).length();
    Some(lateral + (along - range).max(0.0) * 0.45)
}

fn trail_key(p: Vec3) -> [u32; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// GPU shield record. Wider than the sim's `ShieldInstance`: overlap and a
/// short hull list are packed here so the fragment shader never walks every
/// unit, or every other dome, unless those actually meet this one.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuShield {
    pub(crate) pos: [f32; 3],
    pub(crate) radius: f32,
    pub(crate) prev_open: f32,
    pub(crate) open: f32,
    pub(crate) health: f32,
    pub(crate) packed: u32,
    pub(crate) unit_id: u32,
    pub(crate) projector: f32,
    pub(crate) height: f32,
    /// 1 when another same-team dome overlaps this one.
    pub(crate) overlap: u32,
    pub(crate) contact_n: u32,
    /// `radius` last tick, eased in the shader while an upgraded dome swells.
    pub(crate) prev_radius: f32,
    /// The shield's unit in the entity buffer (`shield::NO_ENTITY`: none drawn), so the
    /// shader moves the dome and its projector beam with the unit between ticks.
    pub(crate) entity: u32,
    pub(crate) _pad: u32,
    pub(crate) contacts: [u32; SHIELD_CONTACTS],
}

const _: () = assert!(std::mem::size_of::<GpuShield>() == 128);

/// Mirrors `ShieldHit` in shaders/shields.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ShieldHit {
    pub(crate) pos: [f32; 3],
    pub(crate) start: f32,
    pub(crate) strength: f32,
    pub(crate) _pad: [f32; 3],
}

const _: () = assert!(std::mem::size_of::<ShieldHit>() == 32);

/// When a pressure sphere reaches a dry ground sample, and its directional
/// strength there. Inverts the shader's radius = reach * (1 - (1-age)^2).
fn shockwave_ground_arrival(
    center: Vec3,
    ground: Vec3,
    radius: f32,
    axis: Vec3,
    water: f32,
) -> Option<(f32, f32)> {
    if radius <= 0.2 || ground.z < water + 0.2 {
        return None;
    }
    let delta = ground - center;
    let fraction = delta.length() / radius;
    // By the last part of its reach the front is too weak to lift fresh dust.
    if fraction >= 0.86 {
        return None;
    }
    let directional = if axis.length_squared() > 0.25 {
        let t = ((delta.normalize_or_zero().dot(axis.normalize()) + 0.45) / 1.1).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    };
    if directional < 0.08 {
        return None;
    }
    Some((
        1.0 - (1.0 - fraction).sqrt(),
        directional * (1.0 - fraction * 0.65),
    ))
}

const PUFF_DUST: f32 = 0.0;
const PUFF_SMOKE: f32 = 1.0;
const PUFF_CLOD: f32 = 2.0;
const PUFF_SPARK: f32 = 3.0;
const PUFF_FIRE: f32 = 4.0;
const PUFF_FIREBALL: f32 = 5.0;
const PUFF_TRAIL: f32 = 6.0;
/// A blue energy slug's wake.
const PUFF_ARC: f32 = 7.0;
/// A glowing hex fragment thrown off a shattered dome.
const PUFF_SHARD: f32 = 8.0;
/// A blue-white spark thrown by an energy discharge or an arc impact.
const PUFF_BOLT: f32 = 9.0;
/// Soft blue disc around an energy slug: a glow, not a sausage ribbon.
const PUFF_PLASMA: f32 = 10.0;
const PUFF_CONTRAIL: f32 = 11.0;
const PUFF_ION: f32 = 28.0;
const PUFF_CLOUD_WISP: f32 = 29.0;
// retired: 13 (splinter)
/// Torn blue-white explosion lobes at an airburst and fragment strikes.
const PUFF_SHATTER_BLAST: f32 = 15.0;
const PUFF_TREE_SMOKE: f32 = 16.0;
/// A puff origin height that means no shield clips the puff (puffs.wgsl `fs_puff`).
const PUFF_UNCLIPPED_Z: f32 = 1.0e9;
const PUFF_TREE_FIRE: f32 = 17.0;
/// Broad overlapping billows lifted by a ground pressure front.
const PUFF_SHOCK_DUST: f32 = 18.0;
const PUFF_SHOCK_SMOKE: f32 = 19.0;
/// A thin, continuous, unlit smoke ribbon behind a falling bomb.
const PUFF_BOMB_TRAIL: f32 = 12.0;
/// A spent brass casing thrown out of a gun's breech (`Weapon::casings`).
const PUFF_CASING: f32 = 21.0;
/// A casing's fall and air drag: puffs.wgsl `CASING_FALL` and `CASING_DRAG`.
const CASING_FALL: f32 = 10.0;
const CASING_DRAG: f32 = 1.3;

/// `TrackMark` in shaders/common.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct TrackMark {
    pub(crate) start_xy: [f32; 2],
    pub(crate) end_xy: [f32; 2],
    pub(crate) half_gauge: f32,
    pub(crate) width: f32,
    pub(crate) start: f32,
    pub(crate) life: f32,
}

/// Small deterministic generator for effect scatter. Presentation only.
/// What `damage_fires` needs to know about a blueprint's model.
#[derive(Clone)]
struct BurnSite {
    grid: models::burns::BurnGrid,
    reach: f32,
    /// The model's bounds radius: what a wreck section's stretch is in shares of.
    bounds: f32,
    height: f32,
    turret_pivot: Vec3,
}

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

/// Mirrors `Weld` in shaders/entity.wgsl: local print origin and how alive the wave is.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuWeld {
    local: [f32; 3],
    fade: f32,
}

/// GPU scope names of the shadow cascades, by cascade.
const SHADOW_SCOPES: [&str; 4] = ["shadow.c0", "shadow.c1", "shadow.c2", "shadow.c3"];

#[derive(Clone, Copy)]
struct BurningTree {
    instance: UnitInstance,
    start: f32,
    height: f32,
}

/// 0..1, stable for a patch and a tongue. Not a golden-angle spiral.
fn ground_hash(seed: f32, i: u32, salt: u32) -> f32 {
    let n = seed
        .to_bits()
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(i.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(salt.wrapping_mul(0xC2B2_AE35));
    let n = n ^ (n >> 16);
    (n & 0x00FF_FFFF) as f32 / 16_777_216.0
}

pub struct Renderer {
    effect_barriers: Buffer,
    live_effect_barriers: Vec<EffectBarrier>,
    /// Every weapon's pose for units with gun houses of their own (scene set 28, `mirror::HousePose`).
    houses: Buffer,
    /// Local lights (lights.rs) and their GPU list and cluster grid (scene set 25, 26).
    lights: crate::lights::Lights,
    /// Device-local: every lit pixel reads them. Filled from `light_stage`.
    light_list: Buffer,
    light_grid: Buffer,
    /// The CPU's copy of this frame's list, then grid; and how many bytes of each to copy.
    light_stage: Buffer,
    light_copy: (u64, u64),
    effect_origin: Option<Vec3>,
    /// Set while a gun's muzzle effects are pushed: they leave through the firer's own
    /// shield, so no shield clips them (`rail_fx`).
    effect_outbound: bool,
    effect_settings: mc_data::EffectSettings,
    gpu: Gpu,
    output: Output,
    present_format: vk::Format,
    width: u32,
    height: u32,
    /// What the 3D scene renders at: the output times `render_scale`. The tone
    /// map resamples it to the output, and the UI draws over that at output size.
    scene_width: u32,
    scene_height: u32,
    render_scale: f32,
    antialiasing: Antialiasing,
    /// The tone map's way to the swapchain: SMAA and FSR (post.rs).
    post: post::Post,
    passes: Passes,
    layouts: Layouts,
    pipelines: Pipelines,
    hdr: Image,
    depth: Image,
    /// The sun's shadow cascades, sized by the graphics quality (shadow_map.rs).
    shadow: shadow_map::ShadowMap,
    /// Bloom chain, largest (half size) first.
    bloom: Vec<Image>,
    bloom_fbs: Vec<vk::Framebuffer>,
    /// One per bloom level, for reading it; `hdr_set` reads the scene the same way.
    bloom_sets: Vec<vk::DescriptorSet>,
    /// Overlay glass: the picture at quarter size, then blurred across into the
    /// second image and back down into the first, which the overlay reads.
    glass: Vec<Image>,
    glass_fbs: Vec<vk::Framebuffer>,
    /// One per glass image, for reading it; the first also carries the overlay's atlas.
    glass_sets: [vk::DescriptorSet; 2],
    hdr_set: vk::DescriptorSet,
    /// The opaque scene copied before the water draws, so the sea can bend
    /// and tint what lies under it and mirror what stands on it.
    refract: Image,
    refract_fb: vk::Framebuffer,
    /// Set 2 of the water: `refract` at binding 0, the scene depth at 7.
    water_set: vk::DescriptorSet,
    /// The outermost hull-field skin's depth, so a field is drawn as one layer
    /// where a hull's pieces overlap; `hull_set` has it at binding 7.
    hull_depth: Image,
    hull_depth_fb: vk::Framebuffer,
    /// The depth pre-pass writes `depth` through this (the shadow pass's shape).
    prepass_fb: vk::Framebuffer,
    /// Whether the pre-pass draws (MERIDIAN_PREPASS=0 for A/B timings).
    prepass: bool,
    /// Light shafts through shadowed air, at half size (shafts.rs).
    shafts: shafts::Shafts,
    /// Live scenery detail and cloud target resolution.
    quality: SceneQuality,
    /// Ambient occlusion from the pre-pass's depth, for the scene's shaders (gtao.rs).
    gtao: gtao::Gtao,
    /// The terrain shaded once per pixel before the scene pass (terrain_lit.rs).
    terrain_lit: terrain_lit::TerrainLit,
    /// Fields of grass round the eye (grass.rs).
    grass: grass::Grass,
    /// Walls where a structure's lot was levelled into the ground.
    foundations: foundations::Foundations,
    /// Conduits across the seams of buildings that save each other upkeep (adjacency_links.rs).
    adjacency_links: adjacency_links::AdjacencyLinks,
    hull_set: vk::DescriptorSet,
    scene_fb: vk::Framebuffer,
    present_fbs: Vec<vk::Framebuffer>,

    descriptor_pool: vk::DescriptorPool,
    scene_set: vk::DescriptorSet,
    cull_set: vk::DescriptorSet,
    screen_set: vk::DescriptorSet,
    nodes_set: vk::DescriptorSet,
    marks_set: vk::DescriptorSet,
    ranges_set: vk::DescriptorSet,
    shockwaves_set: vk::DescriptorSet,
    sprites_set: vk::DescriptorSet,
    stains_set: vk::DescriptorSet,
    puffs_set: vk::DescriptorSet,
    shields_set: vk::DescriptorSet,
    samplers: [vk::Sampler; 3],

    globals: Buffer,
    dynamic: Buffer,
    statics: Buffer,
    model_table: Buffer,
    slot_table: Buffer,
    /// The GPU cull's buffers and draw lists (cull_lists.rs).
    cull: cull_lists::CullLists,
    props_dead: Buffer,
    prop_instances: Vec<UnitInstance>,
    cliff_rocks: cliff_rocks::CliffRocks,
    tree_model_base: u32,
    previous_dead: Vec<u32>,
    burning_trees: Vec<BurningTree>,
    fallen_trees: fallen_trees::FallenTrees,
    tree_blasts: tree_wind::TreeBlasts,
    /// Rings, flashes and wakes on the sea, and the set the water draws with.
    water_fx: water_fx::WaterFx,
    /// The smoke off wrecks (renderer/wreck_fx.rs).
    wreck_fx: wreck_fx::WreckFx,
    /// The last of reclaimed wrecks burning away (renderer/wreck_finish.rs).
    wreck_finish: wreck_finish::WreckFinish,
    /// Capital hulls falling, for how they break up when they hit (capital_crash_fx.rs).
    hull_crash_fx: capital_crash_fx::HullCrashFx,
    /// Craters where blasts struck the ground (renderer/impact_craters.rs).
    impact_craters: impact_craters::ImpactCraters,
    /// Electric bore lightning and the molten ground it leaves (renderer/bore_fx.rs).
    bore_fx: bore_fx::BoreFx,
    /// The Regency's held beams and plasma charges (renderer/plasma_fx.rs).
    plasma_fx: plasma_fx::PlasmaFx,
    /// The Regency mines' excavation beams (renderer/regency_mine_fx.rs).
    regency_mine_fx: regency_mine_fx::RegencyMineFx,
    /// The Regency power generators' stars (renderer/star_core_fx.rs).
    star_core_fx: star_core_fx::StarCoreFx,
    giant_fx: titan_fx::GiantFx,
    heavy_rail: heavy_rail_fx::HeavyRailFx,
    /// EMP stuns and warp dampeners in the world (stun_fx.rs, damper_fx.rs).
    emp_fx: stun_fx::EmpFx,
    /// The great guns' shots, trails and hits (renderer/great_gun_fx.rs).
    great_gun: great_gun_fx::GreatGunFx,
    nuke_fx: nuke_fx::NukeFx,
    /// Craters big blasts leave in the ground (renderer/craters.rs, scene set 29).
    craters: craters::Craters,
    /// Heat in the ground, molten then glass (renderer/ground_melt.rs, scene set 32).
    ground_melt: ground_melt::GroundMelt,
    /// A city coming apart (renderer/city_fx.rs, scene set 33).
    city_fx: city_fx::CityFx,
    /// Hot air shimmering over running engines' exhausts (renderer/heat_haze.rs, screen set 8).
    heat_haze: heat_haze::HeatHaze,
    /// Lens flares on bright points (renderer/lens_flare.rs, screen set 9).
    lens_flares: lens_flare::LensFlares,
    /// Red plasma under the bells of craft on gravity lift (renderer/lift_fx.rs).
    lift_fx: lift_fx::LiftFx,
    /// Arcs on running reactors, and reactors going up (renderer/reactor_fx.rs).
    reactor_fx: reactor_fx::ReactorFx,
    /// Capital ships' drives, lift jets and lamps (renderer/capital_fx.rs).
    capital_fx: capital_fx::CapitalFx,
    /// Capital ships' warp jumps: charge, flash, streak, rift (renderer/warp_fx.rs).
    warp_fx: warp_fx::WarpFx,
    nodes: Buffer,
    marks: Buffer,
    ranges: Buffer,
    range_ib: Buffer,
    projectiles: Buffer,
    effects: Buffer,
    shockwaves: Buffer,
    stains: Buffer,
    puffs: Buffer,
    beams: Buffer,
    welds: Buffer,
    shields: Buffer,
    shield_hits: Buffer,
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
    /// The ore veins under every field, one static triangle list.
    vein_vb: Buffer,
    vein_count: u32,
    patch_ib: Buffer,
    patch_index_count: u32,

    overview: Image,
    tiles: Image,
    tile_index: Image,
    /// The fog of war, from the sim's grid to the field the shaders read (fog_field.rs).
    fog: fog_field::FogField,
    noise: Image,
    terrain_materials: Image,
    ground_cover: Image,
    /// Sun, sky and weather (sky.rs).
    sky: crate::sky::Sky,
    /// The nuclear blasts' half-size march and its composite (nuke_volume.rs).
    nuke_volume: nuke_volume::NukeVolume,
    /// Cone weapons' wakes as shells of light (wake_shell.rs, wake_fx.rs).
    wake_shells: wake_shell::WakeShells,
    pad_footprints: Image,
    hull_plans: Image,
    font: Image,
    font_uploaded: bool,

    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    timers: gpu_timers::GpuTimers,
    /// A copy of the next frame shown, when asked for (Mark Issue).
    capture: capture::Capture,
    garbage: Vec<Buffer>,

    pool: Arc<Pool>,
    tile_cache: TileCache,
    node_scratch: Vec<TerrainNode>,
    upload_scratch: Vec<TerrainUpload>,
    map_info: mc_map::MapInfo,
    palette: [[f32; 4]; 4],
    team_colors: [[f32; 4]; crate::gpu_consts::owner::COLORS as usize],
    slot_count: u32,
    static_count: u32,
    dynamic_count: u32,
    sim_units: u32,
    /// The Behemoth's AEB charge on the clock, for its coils' light (`titan_charge`).
    titan_charge: titan_charge::TitanCharge,
    /// Which units plant ground stakes, for the ground punched where each strikes.
    stake_fx: stake_fx::StakeFx,
    projectile_count: u32,
    /// Ground stains in the buffer, in the order written: the sim's scorch, blast craters
    /// (impact_craters.rs). Drawn a run at a time, so a lost
    /// device's breadcrumbs say which kind was on the GPU.
    stain_runs: [u32; 2],
    pad_count: u32,
    /// The ore fields on the ground and the mines' reaches they dim under.
    ore: ore_fields::OreFields,
    effect_cursor: usize,
    shockwave_cursor: usize,
    /// When each shockwave slot's wave is over, so the tone map visits only live ones.
    shockwave_ends: [f32; MAX_SHOCKWAVES],
    puff_cursor: usize,
    shield_count: u32,
    hull_shield_count: u32,
    /// Per model (blueprint): its first draw slot and how many (one per LOD).
    model_draws: Vec<[u32; 2]>,
    /// Draw slots of the models wearing a live hull field this frame: the hull
    /// passes draw only these instead of every visible entity. Empty with hull
    /// fields up means a unit could not be matched: draw everything as before.
    hull_draws: Vec<u32>,
    shield_hit_cursor: usize,
    work_beams: work_beams::WorkBeams,
    /// Flown path of each missile or energy slug, keyed by this tick's head.
    trail_paths: HashMap<[u32; 3], TrailPath>,
    /// Hitscan and rail paths that are still fading.
    fade_beams: Vec<FadeBeam>,
    /// Hitscan shots fired this tick whose impact has not been seen yet.
    pending_rail: Vec<PendingRail>,
    track_cursor: usize,
    /// Track marks written so far, capped at the ring's size: how many to draw.
    track_count: u32,
    prints: titan_fx::Prints,
    scatter: Scatter,
    blueprints: Arc<Blueprints>,
    /// Per blueprint: where its tracks touch the ground, if it has any.
    treads: Vec<Option<Treads>>,
    /// Per blueprint: the legs of a walker, for the dust and prints its feet leave.
    legs: Vec<Option<Legs>>,
    /// Per blueprint: hovercraft raise a downwash instead of track marks.
    hover: Vec<bool>,
    vtol: aircraft_trails::VtolPods,
    /// Per blueprint: where smoke and flame stand on the hull over its burn marks.
    burn_sites: Vec<BurnSite>,
    /// Render-clock seconds between the last two sim ticks.
    tick_seconds: f32,
    last_tick_time: f32,
    fog_enabled: bool,
    /// Survival: how awake the Precursor facility is (`RenderFrame::precursor_activity`).
    precursor_activity: f32,
    /// How the ground and sea are drawn: the map's climate, or its regions' (`set_map_look`).
    look: mc_data::weather::MapLook,
    /// Where the build grid is drawn around: pointer xy, radius, taken lot count.
    build_cursor: [f32; 4],
    build_blocked: [[f32; 4]; BUILD_BLOCKED_MAX],
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
                face: [0.0; 4],
                surface: models::pattern::NONE,
            });
        }
        lod.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Model {
        key: key.to_owned(),
        lods: [lod.clone(), lod.clone(), lod],
        far: None,
        turret_pivot: [0.0; 3],
        spinner_pivot: [0.0; 3],
        spinner_scans: false,
        bounds_radius: (radius * radius + height * height).sqrt(),
        surface_reach: (radius * radius + height * height).sqrt(),
        dust_line: height * 0.62,
        treads: None,
        legs: None,
        arm_pivot: None,
        arm_boom: false,
        recoil: None,
        fold: None,
        breech: None,
        charge_gear: None,
        fold_wrist: None,
        neck: None,
        shield_emitter: None,
        mount: None,
        houses: Vec::new(),
        cells: Vec::new(),
        spins: Vec::new(),
        hover: false,
        pit: None,
        excavation: None,
        star_core: None,
        beam_core: None,
        exhausts: Vec::new(),
        lifts: Vec::new(),
        discharge: None,
        vtol: None,
    }
}

impl Renderer {
    pub fn new(target: Target, scene: SceneDesc) -> Result<Renderer, GpuError> {
        let mut renderer = Self::prepare(target, scene, &|_, _| {})?;
        renderer.attach()?;
        Ok(renderer)
    }

    /// Builds everything but the window's swapchain, so it can run on another
    /// thread while an older renderer still owns the window; `attach` finishes
    /// the job on the thread that presents. `progress` hears the step under way
    /// and how much of the build is done, 0..1.
    pub fn prepare(
        target: Target,
        scene: SceneDesc,
        progress: &dyn Fn(&'static str, f32),
    ) -> Result<Renderer, GpuError> {
        Self::prepare_staged(target, scene, &[], progress)
    }

    /// [`Self::new`] with `staged` props standing among the map's, after them: a prop
    /// shot (`--unit-shot` of a prop's model key) stages them and hides all but its
    /// subject through `RenderFrame::props_dead`, bits from the map's prop count on.
    pub fn new_staged(
        target: Target,
        scene: SceneDesc,
        staged: &[Prop],
    ) -> Result<Renderer, GpuError> {
        let mut renderer = Self::prepare_staged(target, scene, staged, &|_, _| {})?;
        renderer.attach()?;
        Ok(renderer)
    }

    fn prepare_staged(
        target: Target,
        scene: SceneDesc,
        staged: &[Prop],
        progress: &dyn Fn(&'static str, f32),
    ) -> Result<Renderer, GpuError> {
        let clock = std::time::Instant::now();
        let last = std::cell::Cell::new(("Waking the graphics card", clock));
        // Shares are measured build time on the RTX 3080 Ti.
        let step = |name: &'static str, done: f32| {
            let (was, since) = last.replace((name, std::time::Instant::now()));
            // At info, so a report of a build that failed ends on the step it failed in.
            log::info!(
                "renderer build: {was} {:.0} ms; next: {}",
                since.elapsed().as_secs_f32() * 1000.0,
                if name.is_empty() { "done" } else { name }
            );
            progress(name, done);
        };
        progress("Waking the graphics card", 0.0);
        let (gpu, surface, width, height, vsync) = match &target {
            Target::Window {
                display,
                window,
                width,
                height,
                vsync,
            } => {
                let extensions = ash_window::enumerate_required_extensions(*display)?;
                let gpu = Gpu::new(extensions)?;
                // SAFETY: `Target::Window` carries the handles of the app's live window, which
                // it keeps open while it draws with this renderer (`App` drops its renderer
                // before its window); `gpu` was made with the extensions this display needs.
                let surface = unsafe {
                    ash_window::create_surface(&gpu.entry, &gpu.instance, *display, *window, None)
                }?;
                (gpu, Some(surface), *width, *height, *vsync)
            }
            Target::Headless { width, height } => (Gpu::new(&[])?, None, *width, *height, false),
        };
        let (width, height) = (width.max(16), height.max(16));

        let present_format = match surface {
            Some(surface) => swapchain::surface_format(&gpu, surface)?,
            None => vk::Format::R8G8B8A8_SRGB,
        };
        let present_layout = if surface.is_some() {
            vk::ImageLayout::PRESENT_SRC_KHR
        } else {
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL
        };
        step("Compiling shaders", 0.08);
        let passes = Passes::new(&gpu, present_format, present_layout)?;
        let layouts = Layouts::new(&gpu)?;
        let pipelines = Pipelines::new(&gpu, &layouts, &passes)?;

        // Models: one per blueprint (each is fitted to its blueprint's size), then one per prop kind.
        // Every loadout and kit of a refittable unit draws its base's model, which carries each
        // module's pieces; the look bits in its `ModelInfo` say which are on show.
        step("Building unit models", 0.2);
        let bps = &scene.blueprints;
        let mut model_list: Vec<(Model, u32)> = Vec::new();
        // For each blueprint: the model it draws (an index into `model_list`) and its look.
        let mut drawn_as: Vec<(usize, u32, u32)> = Vec::new();
        let mut treads: Vec<Option<Treads>> = Vec::new();
        let mut legs: Vec<Option<Legs>> = Vec::new();
        let mut hover: Vec<bool> = Vec::new();
        let mut vtol = aircraft_trails::VtolPods::default();
        let mut excavations: Vec<Option<(models::Excavation, models::Pit)>> = Vec::new();
        let mut stars: Vec<Option<[f32; 4]>> = Vec::new();
        let mut beam_cores: Vec<Option<[f32; 4]>> = Vec::new();
        let mut burn_sites: Vec<BurnSite> = Vec::new();
        let mut pad_layers: Vec<Vec<u8>> = Vec::new();
        let mut hull_layers: Vec<(Vec<u8>, f32)> = Vec::new();
        for bp in &bps.units {
            progress(
                "Building unit models",
                0.2 + 0.3 * bp.id.index() as f32 / bps.units.len() as f32,
            );
            let base = bps.base_of(bp.id);
            let tier_icon = |icon: u32| (icon & !0xFF00) | (bp.tech as u32) << 8;
            if base != bp.id {
                // Built already: the base comes first in id order.
                let (at, icon, _) = drawn_as[base.index()];
                drawn_as.push((at, tier_icon(icon), bps.look(bp.id)));
                pad_layers.push(pad_layers[base.index()].clone());
                hull_layers.push(hull_layers[base.index()].clone());
                treads.push(treads[base.index()]);
                legs.push(legs[base.index()]);
                hover.push(hover[base.index()]);
                vtol.0.push(vtol.0[base.index()]);
                excavations.push(excavations[base.index()].clone());
                stars.push(stars[base.index()]);
                burn_sites.push(burn_sites[base.index()].clone());
                continue;
            }
            let (radius, height) = (bp.radius.to_f32(), bp.height.to_f32());
            let module_keys: Vec<&str> = bps.refit_set(bp.id).map_or(Vec::new(), |set| {
                set.slots
                    .iter()
                    .flat_map(|s| &s.modules)
                    .map(|m| m.key.as_str())
                    .collect()
            });
            let model =
                models::build_model_fitted(&bp.visual.mesh, radius, height, bp.tech, &module_keys)
                    .unwrap_or_else(|| {
                        log::warn!("no model for mesh key {:?}; using a box", bp.visual.mesh);
                        fallback_model(&bp.visual.mesh, radius, height)
                    });
            let icon = bp.visual.icon as u32
                | (bp.tech as u32) << 8
                | (bp.is_mobile() as u32) << 16
                | (model.hover as u32) << 17
                | (bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air) as u32)
                    << 18
                | (bp.motion.is_some_and(|m| m.hover) as u32) << 19
                | ((bp.visual.mesh == "assault_air") as u32) << 20
                | ((bp.visual.mesh == "rotor_gunship") as u32) << 21
                // retired: 1 << 22 (the Osprey's hold doors)
                // A ship: rides the swell, not the ground (`entity.wgsl`).
                | if bp.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Naval) {
                    crate::gpu_consts::icon::NAVAL
                } else {
                    0
                }
                // Transport flight pitch; Bastion also has ramp/gear parts (`entity.wgsl`).
                | (bp.transport.is_some() as u32) << 24
                | ((bp.visual.mesh == "light_transport") as u32) << 25
                // A spacecraft: kept clean whatever its tech (`entity.wgsl` field dirt).
                | if bp.is_capital_ship() {
                    crate::gpu_consts::icon::CAPITAL
                } else {
                    0
                }
                // Its spinner looks about instead of turning round (`Model::spinner_scans`).
                | (model.spinner_scans as u32) << 27
                // It stands on the seabed; its spire reaches the surface (`entity.wgsl`).
                | if bp.seabed {
                    crate::gpu_consts::icon::SEABED
                } else {
                    0
                };
            let pad = if bp.poured_lot() {
                let half = bp.footprint.0.max(bp.footprint.1) as f32 * (BUILD_CELL_M as f32 * 0.5);
                models::bake_pad_footprint(&model.lods[0], half)
            } else {
                vec![0u8; (models::PAD_FOOTPRINT_RES * models::PAD_FOOTPRINT_RES * 2) as usize]
            };
            pad_layers.push(pad);
            let plan_h = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(0.0f32, f32::max)
                .max(0.5);
            let plan_half = models::hull_plan_half(&model.lods[0]);
            hull_layers.push((
                models::bake_hull_plan(&model.lods[0], plan_half, plan_h),
                plan_half,
            ));
            treads.push(model.treads);
            legs.push(model.legs);
            hover.push(model.hover);
            vtol.0.push(model.vtol);
            excavations.push(model.excavation.clone().zip(model.pit));
            stars.push(model.star_core);
            beam_cores.push(model.beam_core);
            burn_sites.push(BurnSite {
                grid: models::burns::BurnGrid::bake(&model.lods[0]),
                // As `entity.wgsl` hands them to the surface shader, so the marks agree.
                reach: model.surface_reach.max(1.0),
                bounds: model.bounds_radius.max(1.0),
                height: plan_h.max(1.0),
                turret_pivot: Vec3::from(model.turret_pivot),
            });
            drawn_as.push((model_list.len(), icon, bps.look(bp.id)));
            model_list.push((model, icon));
        }
        step("Shaping terrain props", 0.5);
        let prop_base = drawn_as.len() as u32;
        // Props have a fourth draw slot, their far level (`lod::FAR`).
        let first_prop_model = model_list.len();
        for kind in PropKind::ALL {
            let key = models::prop_model_key(kind.raw());
            let model = models::build_model(key).unwrap_or_else(|| fallback_model(key, 4.0, 8.0));
            drawn_as.push((model_list.len(), 13, 0));
            model_list.push((model, 13));
        }
        // Trees and rocks lead the prop kinds; their slots get a vertex stage of their
        // own (entity.wgsl `vs_prop`).
        let plain_props = PropKind::ALL
            .iter()
            .take_while(|k| k.is_tree() || k.is_rock())
            .count();
        // Then the rock pieces dressing the cliffs (`cliff_rocks`), drawn as props.
        let cliff_base = drawn_as.len() as u32;
        for key in models::cliffs::KEYS {
            let model = models::build_model(key).unwrap_or_else(|| fallback_model(key, 16.0, 24.0));
            let icon = 13 | crate::gpu_consts::icon::CLIFF;
            drawn_as.push((model_list.len(), icon, 0));
            model_list.push((model, icon));
        }
        let mut vertices: Vec<MeshVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut slots: Vec<DrawSlot> = Vec::new();
        let mut infos: Vec<ModelInfo> = Vec::new();
        // Which model slots carry charge coils (`titan_charge`).
        let mut coil_models: Vec<bool> = Vec::new();
        // Each model slot's exhaust ports (`heat_haze`).
        let mut exhaust_models: Vec<Vec<models::Exhaust>> = Vec::new();
        // Each model slot's plasma lift bells (`lift_fx`).
        let mut lift_models: Vec<Vec<models::Lift>> = Vec::new();
        // Each model slot's held charge (`reactor_fx`).
        let mut discharge_models: Vec<Option<models::Discharge>> = Vec::new();
        // Per blueprint: its ground stakes' scale, if it plants any (`stake_fx`).
        let mut stake_scales: Vec<Option<f32>> = Vec::new();
        let mut model_draws: Vec<[u32; 2]> = Vec::new();
        let mut first_slot: Vec<u32> = Vec::new();
        for (at, (model, _)) in model_list.iter().enumerate() {
            first_slot.push(slots.len() as u32);
            for lod in model.lods.iter().chain(&model.far) {
                slots.push(DrawSlot {
                    index_count: lod.indices.len() as u32,
                    first_index: indices.len() as u32,
                    vertex_offset: vertices.len() as i32,
                    pad: 0,
                });
                let first = vertices.len();
                vertices.extend_from_slice(&lod.vertices);
                models::shell::pack(&mut vertices[first..]);
                indices.extend_from_slice(&lod.indices);
            }
            // A prop without a far level of its own draws its coarse one there.
            if at >= first_prop_model && model.far.is_none() {
                slots.push(slots[slots.len() - 1]);
            }
        }
        let prop_slots = first_slot[first_prop_model]..first_slot[first_prop_model + plain_props];
        for &(at, icon, look) in &drawn_as {
            let (model, _) = &model_list[at];
            model_draws.push([first_slot[at], model.lods.len() as u32]);
            exhaust_models.push(model.exhausts.clone());
            lift_models.push(model.lifts.clone());
            discharge_models.push(model.discharge.clone());
            stake_scales.push(models::stakes::stake_scale(model));
            coil_models.push(model.lods[0].vertices.iter().any(|v| {
                (models::pattern::COIL..=models::pattern::COIL_TURN_BACK)
                    .contains(&(v.surface & 0xFF))
            }));
            let icon = &icon;
            let height = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(0.0f32, f32::max);
            let plan_half = hull_layers
                .get(infos.len())
                .map(|(_, h)| *h)
                .unwrap_or(model.bounds_radius.max(1.0));
            let plan_box = hull_layers
                .get(infos.len())
                .and_then(|(tex, h)| models::hull_plan_box(tex, *h))
                .unwrap_or([0.0, 0.0, model.bounds_radius, model.bounds_radius]);
            infos.push(ModelInfo {
                slot: first_slot[at],
                icon: *icon,
                bounds_radius: model.bounds_radius,
                height,
                plan_half,
                modules: look,
                pit: model.pit.map_or([0.0; 2], |p| [p.open, p.radius]),
                pit_feed: model
                    .pit
                    .map_or([0.0; 4], |p| [p.rack[0], p.rack[1], p.section, 0.0]),
                shield_emitter: model
                    .shield_emitter
                    .map_or([0.0; 4], |e| [e[0], e[1], e[2], 1.0]),
                surface: [
                    model.surface_reach,
                    model.dust_line,
                    model.legs.map_or(0.0, |l| l.crouch),
                    model.neck.map_or(0.0, |n| n[1]),
                ],
                // w: x of a walker's neck (`Model::neck`).
                turret_pivot: [
                    model.turret_pivot[0],
                    model.turret_pivot[1],
                    model.turret_pivot[2],
                    model.neck.map_or(0.0, |n| n[0]),
                ],
                spinner_pivot: [
                    model.spinner_pivot[0],
                    model.spinner_pivot[1],
                    model.spinner_pivot[2],
                    model.pit.map_or(0.0, |p| p.stroke),
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
                arm_pivot: model.arm_pivot.map_or([0.0; 4], |p| {
                    [p[0], p[1], p[2], if model.arm_boom { 2.0 } else { 1.0 }]
                }),
                recoil: model.recoil.unwrap_or([0.0; 4]),
                fold: model.fold.unwrap_or([0.0; 4]),
                fold_wrist: model.fold_wrist.unwrap_or([0.0; 4]),
                mount: model.mount.unwrap_or([0.0; 4]),
                houses: std::array::from_fn(|i| {
                    model
                        .houses
                        .get(i)
                        .map_or([0.0; 4], |h| [h.pivot[0], h.pivot[1], h.pivot[2], h.travel])
                }),
                house_weapon: std::array::from_fn(|i| {
                    model.houses.get(i).map_or(0.0, |h| h.weapon as f32 + 1.0)
                }),
                houses_high: std::array::from_fn(|i| {
                    model
                        .houses
                        .get(4 + i)
                        .map_or([0.0; 4], |h| [h.pivot[0], h.pivot[1], h.pivot[2], h.travel])
                }),
                house_weapon_high: std::array::from_fn(|i| {
                    model
                        .houses
                        .get(4 + i)
                        .map_or(0.0, |h| h.weapon as f32 + 1.0)
                }),
                // The rig, and in [3].w the ship's cruise in metres a tick: its drives open
                // out as it nears that (`entity.wgsl`, the nozzle's flare).
                capital: models::capital_rig(&model.key).map_or([[0.0; 4]; 7], |mut rig| {
                    let cruise = bps.units.get(infos.len()).and_then(|bp| bp.motion);
                    rig[3][3] =
                        cruise.map_or(6.0, |m| m.speed.to_f32()) / mc_core::TICKS_PER_SECOND as f32;
                    rig
                }),
                crawl: model
                    .legs
                    .and_then(|l| l.crawl)
                    .map_or([[0.0; 4]; models::CRAWL_SLOTS], |c| c.gpu()),
                leg_hock: model
                    .legs
                    .and_then(|l| l.hock)
                    .map_or([0.0; 4], |(h, follow)| [h[0], h[1], h[2], follow]),
                breech: model.breech.unwrap_or([0.0; 4]),
                charge_gear: model.charge_gear.unwrap_or([0.0; 4]),
                leg_sway: model
                    .legs
                    .map_or([0.0; 4], |l| [l.sway[0], l.sway[1], l.sway[2], 0.0]),
                plan_box,
                vtol: model.vtol.map_or([[0.0; 4]; 2], |v| v.gpu()),
                cells: models::CellBlock::gpu(&model.cells).0,
                cell_grid: models::CellBlock::gpu(&model.cells).1,
                spin: model
                    .spins
                    .iter()
                    .find(|(need, until, _)| {
                        let has = |tag: u32| tag != 0 && look & (1 << (tag - 1)) != 0;
                        (*need == 0 || has(*need)) && !has(*until)
                    })
                    // x carries which pieces turn about it (their module and `until` tags): the
                    // axis runs along x, so the shader needs only y and z of the point.
                    .map_or([0.0; 4], |(need, until, p)| {
                        [(need | until << 6) as f32, p[1], p[2], 1.0]
                    }),
            });
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

        step("Scattering the map's props", 0.6);
        // Static entities: the map's props.
        let info = scene.map.info().clone();
        let cover = ground_cover::ground_cover(&scene.map);
        let map_size = glam::Vec2::from(scene.map.info().size_metres().to_f32());
        let tile_cache = TileCache::new(
            scene.map.clone(),
            crate::cliff_blocks::CliffBlocks::new(&cover, map_size),
        );
        let kinds: Vec<u16> = PropKind::ALL.iter().map(|k| k.raw()).collect();
        let statics_data: Vec<UnitInstance> = scene
            .map
            .props()
            .iter()
            .chain(staged)
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
                // A span of wires is raised to its pivot and pitched to meet the
                // ground at the next tower, stretched so it still reaches it.
                let (arm_pitch, packed) = match p.kind.span() {
                    Some((length, pivot)) => {
                        let scale = p.scale_milli as f32 / 1000.0;
                        let reach = length as f32 * scale;
                        let next = glam::Vec2::from(xy) + glam::Vec2::from_angle(heading) * reach;
                        let pitch = (tile_cache.overview_height(next) - pos[2]).atan2(reach);
                        let stretched = p.scale_milli as f32 / pitch.cos();
                        let raise = pivot as f32 * scale;
                        ([pitch, pitch, -raise, 0.0], stretched.round() as u32)
                    }
                    // A tree stands at a height of its own (`fallen_trees::height_stretch`).
                    None if kind < fallen_trees::TREE_KINDS => (
                        [0.0, 0.0, 0.0, fallen_trees::height_stretch(i as u32)],
                        p.scale_milli as u32,
                    ),
                    None => ([0.0; 4], p.scale_milli as u32),
                };
                // City dressing that nothing hits (cars, lights) shows its wear
                // as health: a burnt-out car. Structures' damage is `city_look`.
                let dressing = mc_map::city::structure(p.kind).is_some_and(|s| s.health == 0);
                let health = if dressing {
                    1.0 - p.wear_milli as f32 / 1000.0
                } else {
                    1.0
                };
                UnitInstance {
                    prev_pos: pos,
                    prev_heading: heading,
                    pos,
                    heading,
                    blueprint: prop_base + kind,
                    owner_flags: KIND_PROP,
                    health,
                    build: 1.0,
                    turret_yaw: 0.0,
                    radius: 4.0,
                    unit_id: i as u32,
                    packed,
                    gait: [0.0; 3],
                    upgrade: 0.0,
                    arm_pitch,
                    prev_turret_yaw: 0.0,
                    weld: [0.0; 3],
                    recoil: 0.0,
                    prev_recoil: 0.0,
                    weld_first: 0,
                    weld_count: 0,
                    deploy: 0.0,
                    prev_deploy: 0.0,
                    _pad2: [0.0; 2],
                    refit_modules: 0,
                    status: [0; 3],
                    mount: [0.0; 4],
                    spin_recoil: [0.0; 4],
                    fx: [0.0; 4],
                    drive_swing: [0.0; 2],
                    twin_spin: [0.0; 2],
                }
            })
            .collect();
        let mut statics_data = statics_data;
        let cliff_rocks = {
            use crate::keep::{kept, Kept};
            static KEPT: Kept<Vec<UnitInstance>> = std::sync::Mutex::new(Vec::new());
            let first = statics_data.len();
            statics_data.extend(kept(&KEPT, scene.map.content_id(), || {
                cliff_rocks::cliff_rocks(&scene.map, cliff_base, first as u32)
            }));
            cliff_rocks::CliffRocks::new(&statics_data, first)
        };
        // Under every city structure, the rubble it would leave (city_fx.rs).
        let (heaps, heap_list) = {
            let rubble = PropKind::ALL
                .iter()
                .position(|k| *k == PropKind::CityRubble)
                .unwrap_or(0) as u32;
            city_fx::heaps(
                &scene.map,
                statics_data.len() as u32,
                prop_base + rubble,
                |at| tile_cache.overview_height(at),
            )
        };
        statics_data.extend(heaps);
        let city_fx = city_fx::CityFx::new(&gpu, &scene.map, staged.len(), heap_list)?;
        let ore = ore_fields::OreFields::new(scene.map.ore_regions());
        let vein_mesh = ore_vein_mesh(scene.map.ore_regions());
        let static_count = statics_data.len() as u32;

        use vk::BufferUsageFlags as U;
        let storage = U::STORAGE_BUFFER;
        let total_entities = static_count as u64 + MAX_DYNAMIC as u64;
        let globals = gpu.host_buffer(size_of::<Globals>() as u64, U::UNIFORM_BUFFER)?;
        let dynamic = gpu.host_buffer((MAX_DYNAMIC * size_of::<UnitInstance>()) as u64, storage)?;
        let statics = gpu.buffer_with_data(bytemuck::cast_slice(&statics_data), storage)?;
        let model_table = gpu.buffer_with_data(bytemuck::cast_slice(&infos), storage)?;
        let slot_table = gpu.buffer_with_data(bytemuck::cast_slice(&slots), storage)?;
        let cull = cull_lists::CullLists::new(
            &gpu,
            slot_count,
            total_entities,
            MAX_DYNAMIC as u64,
            active_draws::ActiveDraws::new(
                model_draws.clone(),
                prop_base,
                prop_slots,
                slot_count - 1,
                &statics_data,
            ),
        )?;
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
        let shockwaves =
            gpu.host_buffer((MAX_SHOCKWAVES * size_of::<GpuShockwave>()) as u64, storage)?;
        shockwaves.write(0, &vec![0u8; shockwaves.size as usize]);
        let welds = gpu.host_buffer(
            (MAX_CONSTRUCTION_WELDS * size_of::<GpuWeld>()) as u64,
            storage,
        )?;
        welds.write(0, &vec![0u8; welds.size as usize]);
        let effect_barriers = gpu.host_buffer(
            (16 + MAX_SHIELDS * size_of::<EffectBarrier>()) as u64,
            storage,
        )?;
        effect_barriers.write(0, &vec![0u8; effect_barriers.size as usize]);
        let houses = gpu.host_buffer(
            (MAX_HOUSES * size_of::<mc_sim::mirror::HousePose>()) as u64,
            storage,
        )?;
        houses.write(0, &vec![0u8; houses.size as usize]);
        let light_list_bytes =
            (crate::lights::MAX_LIGHTS * size_of::<crate::lights::GpuLight>()) as u64;
        let light_grid_bytes = ((crate::lights::CLUSTERS + crate::lights::MAX_INDICES) * 4) as u64;
        let light_list = gpu.device_buffer(light_list_bytes, storage)?;
        let light_grid = gpu.device_buffer(light_grid_bytes, storage)?;
        let light_stage = gpu.host_buffer(light_list_bytes + light_grid_bytes, U::TRANSFER_SRC)?;
        let shields = gpu.host_buffer((MAX_SHIELDS * size_of::<GpuShield>()) as u64, storage)?;
        shields.write(0, &vec![0u8; shields.size as usize]);
        let shield_hits =
            gpu.host_buffer((MAX_SHIELD_HITS * size_of::<ShieldHit>()) as u64, storage)?;
        shield_hits.write(0, &vec![0u8; shield_hits.size as usize]);
        let stains = gpu.host_buffer((MAX_STAINS * size_of::<StainInstance>()) as u64, storage)?;
        let puffs = gpu.host_buffer((MAX_PUFFS * size_of::<Puff>()) as u64, storage)?;
        puffs.write(0, &vec![0u8; puffs.size as usize]);
        let beams = gpu.host_buffer(
            (work_beams::MAX_BEAMS * size_of::<work_beams::GpuBeam>()) as u64,
            storage,
        )?;
        let track_marks = gpu.host_buffer(
            ((MAX_TRACK_MARKS + MAX_PRINTS) * size_of::<TrackMark>()) as u64,
            storage,
        )?;
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
        let vein_count = vein_mesh.len() as u32;
        let empty = [MeshVertex::zeroed()];
        let vein_vb = gpu.buffer_with_data(
            bytemuck::cast_slice(if vein_mesh.is_empty() {
                &empty[..]
            } else {
                &vein_mesh
            }),
            U::VERTEX_BUFFER,
        )?;
        let patch_ib = gpu.buffer_with_data(bytemuck::cast_slice(&patch_i), U::INDEX_BUFFER)?;

        step("Weaving terrain textures", 0.7);
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
        let fog = fog_field::FogField::new(&gpu, size)?;
        let noise = {
            let data = textures::noise_map();
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
            textures::flatten_noise_mips(&mut chain);
            for (level, (_, pixels)) in chain.iter().enumerate() {
                gpu.upload_image(&image, 0, level as u32 + 1, None, pixels, false)?;
            }
            image
        };
        let material_layers = textures::terrain_materials();
        let terrain_materials = gpu.image(&ImageDesc {
            width: textures::SIZE as u32,
            height: textures::SIZE as u32,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: sampled,
            layers: material_layers.len() as u32,
            mips: 10,
            array: true,
        })?;
        for (layer, (pixels, cutout)) in material_layers.iter().enumerate() {
            gpu.upload_image(
                &terrain_materials,
                layer as u32,
                0,
                None,
                pixels,
                layer == 0,
            )?;
            for (mip, (_, data)) in textures::terrain_mips(pixels, *cutout).iter().enumerate() {
                gpu.upload_image(
                    &terrain_materials,
                    layer as u32,
                    mip as u32 + 1,
                    None,
                    data,
                    false,
                )?;
            }
        }
        step("Laying ground cover", 0.8);
        let ground_cover = gpu.image(&ImageDesc {
            width: cover.width,
            height: cover.height,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: sampled,
            layers: 3,
            mips: 1,
            array: true,
        })?;
        gpu.upload_image(&ground_cover, 0, 0, None, &cover.texels, true)?;
        gpu.upload_image(&ground_cover, 1, 0, None, &cover.ways, false)?;
        gpu.upload_image(&ground_cover, 2, 0, None, &cover.streets, false)?;
        let pad_res = models::PAD_FOOTPRINT_RES;
        let pad_footprints = gpu.image(&ImageDesc {
            width: pad_res,
            height: pad_res,
            format: vk::Format::R8G8_UNORM,
            usage: sampled,
            layers: pad_layers.len().max(1) as u32,
            mips: 1,
            array: true,
        })?;
        if pad_layers.is_empty() {
            gpu.upload_image(
                &pad_footprints,
                0,
                0,
                None,
                &vec![0u8; (pad_res * pad_res * 2) as usize],
                true,
            )?;
        } else {
            for (i, layer) in pad_layers.iter().enumerate() {
                gpu.upload_image(&pad_footprints, i as u32, 0, None, layer, i == 0)?;
            }
        }
        let hull_plans = gpu.image(&ImageDesc {
            width: pad_res,
            height: pad_res,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: sampled,
            layers: hull_layers.len().max(1) as u32,
            mips: 1,
            array: true,
        })?;
        if hull_layers.is_empty() {
            gpu.upload_image(
                &hull_plans,
                0,
                0,
                None,
                &vec![0u8; (pad_res * pad_res * 4) as usize],
                true,
            )?;
        } else {
            for (i, (layer, _)) in hull_layers.iter().enumerate() {
                gpu.upload_image(&hull_plans, i as u32, 0, None, layer, i == 0)?;
            }
        }
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

        step("Forming the sky", 0.88);
        let sky = crate::sky::Sky::new(
            &gpu,
            &layouts,
            &passes,
            glam::Vec2::from(size),
            info.water_level.to_f32(),
            (size[0].to_bits() as u64) << 32 | size[1].to_bits() as u64,
            |xy| tile_cache.overview_height(xy),
        )?;

        let nuke_volume = nuke_volume::NukeVolume::new(&gpu, &layouts, &passes, sky.noise_view())?;
        let wake_shells = wake_shell::WakeShells::new(&gpu, &layouts, &passes)?;
        let post = post::Post::new(&gpu, passes.present, layouts.screen)?;
        let shafts = shafts::Shafts::new(&gpu, &layouts, &passes)?;

        step("Wiring the passes together", 0.96);
        // Descriptor sets: the pool holds exactly the sets allocated below.
        let mut sets = SetPool::new(
            &gpu,
            &[
                (SCENE_SET, 1),
                (CULL_SET, 1),
                // screen, hdr, a bloom set a level, two glass, water, hull
                (SCREEN_SET, 6 + BLOOM_LEVELS as u32),
                // nodes, marks, ranges, shockwaves, sprites, stains, puffs, shields, sea
                (PASS_SET, 9),
            ],
        )?;
        let scene_set = sets.alloc(&gpu, layouts.scene_set, SCENE_SET)?;
        let cull_set = sets.alloc(&gpu, layouts.cull_set, CULL_SET)?;
        let screen_set = sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?;
        let nodes_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let marks_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let ranges_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let shockwaves_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let sprites_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let stains_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let puffs_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let shields_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let hdr_set = sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?;
        let bloom_sets = (0..BLOOM_LEVELS)
            .map(|_| sets.alloc(&gpu, layouts.screen_set, SCREEN_SET))
            .collect::<Result<Vec<_>, _>>()?;
        let glass_sets = [
            sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?,
            sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?,
        ];
        let water_set = sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?;
        let hull_set = sets.alloc(&gpu, layouts.screen_set, SCREEN_SET)?;
        // What the water reads of the effects on it (renderer/water_fx.rs), in both bindings of a pass set.
        let sea_fx = gpu.host_buffer(water_fx::SEA_FX_BYTES as u64, storage)?;
        let sea_set = sets.alloc(&gpu, layouts.pass_set, PASS_SET)?;
        let descriptor_pool = sets.into_raw();

        let write_buffers =
            |set: vk::DescriptorSet, first: u32, ty: vk::DescriptorType, buffers: &[&Buffer]| {
                for (i, b) in buffers.iter().enumerate() {
                    let info = [b.info()];
                    let write = [vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(first + i as u32)
                        .descriptor_type(ty)
                        .buffer_info(&info)];
                    // SAFETY: `set` is a fresh set from `sets` that no command buffer uses
                    // yet, `b` is a live buffer of this device, and `write`/`info` live to the
                    // end of the call.
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
                // SAFETY: `set` is a fresh set from `sets` that no command buffer uses yet,
                // `view` is a live view of this device, and `write`/`info` live to the end of
                // the call.
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
            // SAFETY: `set` is a fresh set from `sets` that no command buffer uses yet,
            // `sampler` is a live sampler of this device, and `write`/`info` live to the end of
            // the call.
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
            &[&dynamic, &statics, &model_table, &cull.visible],
        );
        write_buffers(
            scene_set,
            31,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&cull.sway],
        );
        write_buffers(scene_set, 15, vk::DescriptorType::STORAGE_BUFFER, &[&welds]);
        for (binding, image) in [
            (5, &overview),
            (6, &tiles),
            (7, &tile_index),
            (9, &noise),
            (16, &pad_footprints),
            (17, &hull_plans),
            (18, &terrain_materials),
            (20, &ground_cover),
        ] {
            write_image(scene_set, binding, image.view, read);
        }
        let quality = SceneQuality::from_env();
        let shadow =
            shadow_map::ShadowMap::new(&gpu, passes.shadow, scene_set, quality.shadow_size)?;
        write_image(scene_set, 21, sky.weather_view(), vk::ImageLayout::GENERAL);
        write_image(scene_set, 23, sky.flow_view(), vk::ImageLayout::GENERAL);
        write_image(scene_set, 24, sky.floor_view(), read);
        write_image(scene_set, 27, sky.shade_view(), vk::ImageLayout::GENERAL);
        write_image(scene_set, 8, fog.view(), vk::ImageLayout::GENERAL);
        let mut gtao = gtao::Gtao::new(&gpu, &globals)?;
        gtao.enabled = quality.ambient_occlusion;
        let terrain_lit = terrain_lit::TerrainLit::new(&gpu, layouts.screen_set)?;
        let foundations =
            foundations::Foundations::new(&gpu, &layouts, &passes, scene.map.clone())?;
        let mut grass = grass::Grass::new(
            &gpu,
            layouts.scene_set,
            passes.scene,
            &stains,
            &track_marks,
            foundations.cells(),
        )?;
        grass.set_density(quality.grass_density);
        let adjacency_links = adjacency_links::AdjacencyLinks::new(&gpu, &layouts, &passes)?;
        write_image(scene_set, 30, gtao.ao_view(), vk::ImageLayout::GENERAL);
        write_buffers(
            scene_set,
            22,
            vk::DescriptorType::UNIFORM_BUFFER,
            &[sky.atmosphere_buffer()],
        );
        for (i, s) in samplers.iter().enumerate() {
            write_sampler(scene_set, 12 + i as u32, *s);
        }
        write_buffers(cull_set, 0, vk::DescriptorType::UNIFORM_BUFFER, &[&globals]);
        write_buffers(
            cull_set,
            1,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&dynamic, &statics, &model_table, &slot_table]
                .into_iter()
                .chain(cull.bindings())
                .chain([&props_dead, &effect_barriers, &cull.sway])
                .collect::<Vec<_>>(),
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
            shockwaves_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&shockwaves, &shockwaves],
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
        write_buffers(
            shields_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&shields, &shield_hits],
        );
        write_buffers(
            scene_set,
            19,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&effect_barriers],
        );
        write_buffers(
            scene_set,
            28,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&houses],
        );
        let craters = craters::Craters::new(&gpu)?;
        // A plant's heat sinks vent steam as well as shimmering (`reactor_fx`).
        let plant_vents: Vec<Vec<models::Exhaust>> = exhaust_models
            .iter()
            .zip(&discharge_models)
            .map(|(e, d)| if d.is_some() { e.clone() } else { Vec::new() })
            .collect();
        let heat_haze = heat_haze::HeatHaze::new(&gpu, exhaust_models)?;
        let lens_flares = lens_flare::LensFlares::new(&gpu)?;
        write_buffers(
            scene_set,
            29,
            vk::DescriptorType::STORAGE_BUFFER,
            &[craters.buffer()],
        );
        let ground_melt = ground_melt::GroundMelt::new(&gpu)?;
        write_buffers(
            scene_set,
            32,
            vk::DescriptorType::STORAGE_BUFFER,
            &[ground_melt.buffer()],
        );
        write_buffers(
            scene_set,
            33,
            vk::DescriptorType::STORAGE_BUFFER,
            &[city_fx.buffer()],
        );
        write_buffers(
            scene_set,
            25,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&light_list, &light_grid],
        );
        write_buffers(
            sea_set,
            0,
            vk::DescriptorType::STORAGE_BUFFER,
            &[&sea_fx, &sea_fx],
        );
        for set in std::iter::once(&screen_set)
            .chain([&hdr_set])
            .chain(&bloom_sets)
            .chain(&glass_sets)
            .chain([&water_set, &hull_set])
        {
            write_image(*set, 1, font.view, read);
            write_sampler(*set, 2, samplers[1]);
            write_buffers(*set, 4, vk::DescriptorType::STORAGE_BUFFER, &[&shockwaves]);
            write_buffers(*set, 5, vk::DescriptorType::UNIFORM_BUFFER, &[&globals]);
            write_buffers(
                *set,
                6,
                vk::DescriptorType::STORAGE_BUFFER,
                &[&effect_barriers],
            );
            write_buffers(
                *set,
                8,
                vk::DescriptorType::STORAGE_BUFFER,
                &[heat_haze.buffer()],
            );
            write_buffers(
                *set,
                9,
                vk::DescriptorType::STORAGE_BUFFER,
                &[lens_flares.buffer()],
            );
        }

        // SAFETY: the pool is this device's and used only from the thread that owns the `Gpu`;
        // the allocate info lives to the end of the call.
        let cmd = unsafe {
            gpu.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(gpu.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }?[0];
        // SAFETY: the device is alive and the create info lives to the end of the call.
        let fence = unsafe {
            gpu.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }?;
        // SAFETY: the device is alive and the create info lives to the end of the call.
        let image_available = unsafe {
            gpu.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }?;
        // SAFETY: the device is alive and the create info lives to the end of the call.
        let render_finished = unsafe {
            gpu.device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
        }?;
        let timers = gpu_timers::GpuTimers::new(&gpu)?;

        let faction = &bps.factions[0];
        let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        let mut team_colors = [[1.0; 4]; crate::gpu_consts::owner::COLORS as usize];
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
            glass: Vec::new(),
            glass_fbs: Vec::new(),
            glass_sets,
            hdr_set,
            refract: placeholder(&gpu)?,
            refract_fb: vk::Framebuffer::null(),
            water_set,
            hull_depth: placeholder(&gpu)?,
            hull_depth_fb: vk::Framebuffer::null(),
            prepass_fb: vk::Framebuffer::null(),
            prepass: std::env::var("MERIDIAN_PREPASS").map_or(true, |v| v != "0"),
            shafts,
            quality,
            gtao,
            terrain_lit,
            grass,
            foundations,
            adjacency_links,
            hull_set,
            present_format,
            width,
            height,
            scene_width: width,
            scene_height: height,
            render_scale: 1.0,
            antialiasing: Antialiasing::Off,
            post,
            passes,
            layouts,
            pipelines,
            shadow,
            scene_fb: vk::Framebuffer::null(),
            present_fbs: Vec::new(),
            descriptor_pool,
            scene_set,
            cull_set,
            screen_set,
            nodes_set,
            marks_set,
            ranges_set,
            shockwaves_set,
            sprites_set,
            stains_set,
            puffs_set,
            shields_set,
            samplers,
            globals,
            dynamic,
            statics,
            model_table,
            slot_table,
            cull,
            props_dead,
            prop_instances: statics_data,
            cliff_rocks,
            tree_model_base: prop_base,
            previous_dead: Vec::new(),
            burning_trees: Vec::new(),
            fallen_trees: Default::default(),
            tree_blasts: Default::default(),
            water_fx: water_fx::WaterFx::new(sea_fx, sea_set),
            wreck_fx: wreck_fx::WreckFx::default(),
            wreck_finish: wreck_finish::WreckFinish::default(),
            hull_crash_fx: capital_crash_fx::HullCrashFx::default(),
            impact_craters: impact_craters::ImpactCraters::default(),
            bore_fx: bore_fx::BoreFx::default(),
            plasma_fx: plasma_fx::PlasmaFx::new(beam_cores),
            regency_mine_fx: regency_mine_fx::RegencyMineFx::new(excavations),
            star_core_fx: star_core_fx::StarCoreFx::new(stars),
            giant_fx: titan_fx::GiantFx::default(),
            heavy_rail: heavy_rail_fx::HeavyRailFx::default(),
            emp_fx: stun_fx::EmpFx::default(),
            great_gun: great_gun_fx::GreatGunFx::default(),
            nuke_fx: nuke_fx::NukeFx::default(),
            craters,
            ground_melt,
            city_fx,
            heat_haze,
            lens_flares,
            lift_fx: lift_fx::LiftFx::new(lift_models),
            reactor_fx: reactor_fx::ReactorFx::new(discharge_models, plant_vents),
            capital_fx: capital_fx::CapitalFx::default(),
            warp_fx: warp_fx::WarpFx::default(),
            nodes,
            marks,
            ranges,
            range_ib,
            projectiles,
            effects,
            shockwaves,
            stains,
            puffs,
            beams,
            welds,
            shields,
            shield_hits,
            effect_barriers,
            live_effect_barriers: Vec::new(),
            houses,
            lights: crate::lights::Lights::new(&scene.blueprints),
            light_list,
            light_grid,
            light_stage,
            light_copy: (0, 0),
            effect_origin: None,
            effect_outbound: false,
            effect_settings: mc_data::EffectSettings::default(),
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
            vein_vb,
            vein_count,
            patch_ib,
            patch_index_count: patch_i.len() as u32,
            overview,
            tiles,
            tile_index,
            fog,
            noise,
            terrain_materials,
            ground_cover,
            sky,
            nuke_volume,
            wake_shells,
            pad_footprints,
            hull_plans,
            font,
            font_uploaded: false,
            cmd,
            fence,
            image_available,
            render_finished,
            timers,
            capture: Default::default(),
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
                rgba(faction.shield_color),
            ],
            team_colors,
            slot_count,
            static_count,
            dynamic_count: 0,
            sim_units: 0,
            titan_charge: titan_charge::TitanCharge::new(coil_models),
            stake_fx: stake_fx::StakeFx::new(stake_scales),
            projectile_count: 0,
            stain_runs: [0; 2],
            pad_count: 0,
            ore,
            effect_cursor: 0,
            shockwave_cursor: 0,
            shockwave_ends: [f32::NEG_INFINITY; MAX_SHOCKWAVES],
            puff_cursor: 0,
            shield_count: 0,
            hull_shield_count: 0,
            model_draws,
            hull_draws: Vec::new(),
            shield_hit_cursor: 0,
            work_beams: Default::default(),
            trail_paths: HashMap::new(),
            fade_beams: Vec::new(),
            pending_rail: Vec::new(),
            track_cursor: 0,
            track_count: 0,
            prints: titan_fx::Prints::default(),
            scatter: Scatter(0x9E37_79B9),
            blueprints: scene.blueprints.clone(),
            treads,
            legs,
            hover,
            vtol,
            burn_sites,
            tick_seconds: 0.1,
            last_tick_time: 0.0,
            fog_enabled: false,
            precursor_activity: 0.0,
            look: map_look::initial(),
            build_cursor: [0.0; 4],
            build_blocked: [[0.0; 4]; BUILD_BLOCKED_MAX],
            last_time: 0.0,
            stats: FrameStats::default(),
            gpu,
        };
        // Headless shots and tests: MERIDIAN_RENDER_SCALE=1.5, MERIDIAN_AA=off|smaa. The game sets both from its settings (`set_render_quality`).
        let env = |key| std::env::var(key).ok().and_then(|v| v.parse::<f32>().ok());
        renderer.render_scale = env("MERIDIAN_RENDER_SCALE").map_or(1.0, |s| s.clamp(0.5, 2.0));
        match std::env::var("MERIDIAN_AA").ok().as_deref() {
            Some("off") => renderer.antialiasing = Antialiasing::Off,
            Some("smaa") => renderer.antialiasing = Antialiasing::Smaa,
            _ => {}
        }
        step("", 1.0);
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

    /// The build grid shows within `radius` metres of `cursor`, with the lots in
    /// `blocked` (min x, min y, max x, max y) drawn as taken. Drawn only while
    /// `FrameInput::build_grid` is set.
    /// How strongly ore fields show, 0..1: faint in play, full while a core
    /// mine is being placed.
    pub fn set_ore_highlight(&mut self, k: f32) {
        self.ore.set_highlight(k);
    }

    /// The mines in sight whose reach the ore under dims (`ore_fields.rs`).
    pub fn set_ore_claims(&mut self, claims: &[OreClaim]) {
        self.ore.set_claims(claims);
    }

    pub fn set_build_grid(&mut self, cursor: glam::Vec2, radius: f32, blocked: &[[f32; 4]]) {
        let n = blocked.len().min(BUILD_BLOCKED_MAX);
        self.build_cursor = [cursor.x, cursor.y, radius, n as f32];
        self.build_blocked[..n].copy_from_slice(&blocked[..n]);
    }

    pub fn device_name(&self) -> &str {
        &self.gpu.device_name
    }

    /// The device, for choosing a graphics preset to suit it.
    pub fn adapter(&self) -> crate::gpu::Adapter {
        crate::gpu::Adapter {
            name: self.gpu.device_name.clone(),
            kind: self.gpu.kind,
            vendor_id: self.gpu.vendor_id,
            vram_mib: self.gpu.vram_mib,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Cursor ray against the terrain (overview resolution).
    pub fn pick_ground(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        self.tile_cache.pick(origin, dir)
    }

    /// Ground stains of every kind in the buffer, ahead of the pads.
    fn stain_count(&self) -> u32 {
        self.stain_runs.iter().sum()
    }

    /// Sets how staged prop `index` (`Self::new_staged`) looks hurt: a
    /// `gpu_consts::city_look` word, as the sim's damage would set it.
    pub fn set_staged_look(&mut self, index: usize, word: u32) {
        self.city_fx.stage_look(index, word);
    }

    pub fn ground_height(&self, xy: glam::Vec2) -> f32 {
        self.tile_cache.overview_height(xy)
    }

    /// How high a standing city structure rises over the ground at `xy`, metres
    /// (0 where none stands): what a line of fire must also clear (city_fx.rs).
    pub fn building_top(&self, xy: glam::Vec2) -> f32 {
        self.city_fx.standing_top(xy)
    }

    /// Cursor ray against what the player sees: the terrain, or the water's
    /// surface where the ray reaches the sea before the seabed.
    pub fn pick_surface(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        let hit = self.pick_ground(origin, dir)?;
        let water = self.map_info.water_level.to_f32();
        if hit.z >= water || dir.z >= 0.0 || origin.z <= water {
            return Some(hit);
        }
        Some(origin + dir * ((water - origin.z) / dir.z))
    }

    /// The ground, or the water's surface over the sea.
    pub fn surface_height(&self, xy: glam::Vec2) -> f32 {
        self.ground_height(xy)
            .max(self.map_info.water_level.to_f32())
    }

    /// Takes the window: builds the swapchain and every size-dependent target.
    /// A renderer from `prepare` needs this once before its first frame.
    pub fn attach(&mut self) -> Result<(), GpuError> {
        self.create_size_dependent()
    }

    /// Gives the window up so another renderer can `attach` to it: the
    /// swapchain goes now, the rest when this is dropped. Renders nothing after.
    pub fn release_window(&mut self) {
        self.gpu.wait_idle();
        if let Output::Window(sc) = &mut self.output {
            // SAFETY: the device went idle just above, so nothing still uses the framebuffers,
            // views or chain; each handle is this device's and drained or nulled here, so
            // `Drop` does not destroy it again.
            unsafe {
                for fb in self.present_fbs.drain(..) {
                    self.gpu.device.destroy_framebuffer(fb, None);
                }
                for v in sc.views.drain(..) {
                    self.gpu.device.destroy_image_view(v, None);
                }
                if let Some(f) = &self.gpu.swapchain_fn {
                    f.destroy_swapchain(sc.swapchain, None);
                }
            }
            sc.swapchain = vk::SwapchainKHR::null();
        }
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
        // SAFETY: the device went idle just above, so no command buffer in flight uses these
        // framebuffers; each is drained or nulled as it goes, so nothing destroys it twice.
        unsafe {
            for fb in self.present_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            // Nulled as they go, so a failure below cannot leave `Drop` a dead handle.
            if self.scene_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(std::mem::take(&mut self.scene_fb), None);
            }
            if self.refract_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(std::mem::take(&mut self.refract_fb), None);
            }
            if self.hull_depth_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(std::mem::take(&mut self.hull_depth_fb), None);
            }
            if self.prepass_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(std::mem::take(&mut self.prepass_fb), None);
            }
            for fb in self.bloom_fbs.drain(..).chain(self.glass_fbs.drain(..)) {
                device.destroy_framebuffer(fb, None);
            }
        }
        for image in self.bloom.drain(..).chain(self.glass.drain(..)) {
            self.gpu.destroy_image(image);
        }
        let mut present_views: Vec<vk::ImageView> = Vec::new();
        match &mut self.output {
            Output::Window(sc) => {
                (self.width, self.height) = swapchain::rebuild(
                    &self.gpu,
                    sc.surface,
                    self.present_format,
                    sc.vsync,
                    (self.width, self.height),
                    &mut sc.swapchain,
                    &mut sc.views,
                )?;
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

        // Past 16k a side the device may refuse the image.
        let scaled = |n: u32| ((n as f32 * self.render_scale).round() as u32).clamp(1, 16384);
        self.scene_width = scaled(self.width);
        self.scene_height = scaled(self.height);
        let (sw, sh) = (self.scene_width, self.scene_height);
        let attachment = |format, usage| ImageDesc {
            width: sw,
            height: sh,
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
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ))?;
        self.gpu
            .destroy_image(std::mem::replace(&mut self.hdr, hdr));
        self.gpu
            .destroy_image(std::mem::replace(&mut self.depth, depth));
        // The march finds its footprint in the depth it reads, so it can
        // follow the output: supersampling does not multiply the clouds' cost.
        self.sky.resize(
            &self.gpu,
            self.width,
            self.height,
            self.depth.view,
            self.quality.cloud_divisor,
        )?;
        let clouds = self
            .sky
            .cloud_targets()
            .expect("the sky's targets are made in its resize");
        self.nuke_volume
            .resize(&self.gpu, self.width, self.height, self.depth.view, clouds)?;
        self.post.resize(
            &self.gpu,
            self.antialiasing,
            (sw, sh),
            (self.width, self.height),
        )?;
        self.gtao.resize(&self.gpu, (sw, sh), self.depth.view)?;
        self.terrain_lit
            .resize(&self.gpu, (sw, sh), self.depth.view, self.passes.bloom_down)?;
        self.shafts.resize(&self.gpu, (sw, sh), self.depth.view)?;
        // SAFETY: the device went idle at the top of this function, so no command buffer in
        // flight reads `scene_set`; `ao_view` is GTAO's live view, just remade by its `resize`,
        // and `info`/`write` live to the end of the call.
        unsafe {
            let info = [vk::DescriptorImageInfo::default()
                .image_view(self.gtao.ao_view())
                .image_layout(vk::ImageLayout::GENERAL)];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(self.scene_set)
                .dst_binding(30)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&info)];
            self.gpu.device.update_descriptor_sets(&write, &[]);
        }
        let refract = self.gpu.image(&attachment(
            HDR_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ))?;
        self.gpu
            .destroy_image(std::mem::replace(&mut self.refract, refract));
        let hull_depth = self.gpu.image(&attachment(
            DEPTH_FORMAT,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        ))?;
        self.gpu
            .destroy_image(std::mem::replace(&mut self.hull_depth, hull_depth));

        let device = &self.gpu.device;
        let views = [self.hdr.view, self.depth.view];
        // SAFETY: `hdr` and `depth` are live views of this device, made just above at the scene
        // size, in the formats the scene pass's two attachments use; the create info lives to
        // the end of the call.
        self.scene_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.passes.scene)
                    .attachments(&views)
                    .width(sw)
                    .height(sh)
                    .layers(1),
                None,
            )
        }?;
        // SAFETY: `refract` is a live HDR view at the scene size, matching the bloom pass's one
        // colour attachment; the create info lives to the end of the call.
        self.refract_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.passes.bloom_down)
                    .attachments(&[self.refract.view])
                    .width(sw)
                    .height(sh)
                    .layers(1),
                None,
            )
        }?;
        // SAFETY: `depth` is a live `DEPTH_FORMAT` view at the scene size, matching the depth-
        // only shadow pass; the create info lives to the end of the call.
        self.prepass_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.passes.shadow)
                    .attachments(&[self.depth.view])
                    .width(sw)
                    .height(sh)
                    .layers(1),
                None,
            )
        }?;
        // SAFETY: `hull_depth` is a live `DEPTH_FORMAT` view at the scene size, matching the
        // depth-only shadow pass; the create info lives to the end of the call.
        self.hull_depth_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.passes.shadow)
                    .attachments(&[self.hull_depth.view])
                    .width(sw)
                    .height(sh)
                    .layers(1),
                None,
            )
        }?;
        for view in present_views {
            let views = [view];
            // SAFETY: `view` is a live output image view (swapchain or headless) at the output
            // size in `present_format`, the format the present pass was made for; the create
            // info lives to the end of the call.
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
            let (w, h) = ((sw >> (level + 1)).max(1), (sh >> (level + 1)).max(1));
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
            // SAFETY: `image` was just made at this size in `HDR_FORMAT`, the bloom pass's
            // attachment format; the create info lives to the end of the call.
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
        for _ in 0..2 {
            let (w, h) = ((sw / 4).max(1), (sh / 4).max(1));
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
            // SAFETY: `image` was just made at this size in `HDR_FORMAT`, the bloom pass's
            // attachment format; the create info lives to the end of the call.
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
            self.glass.push(image);
            self.glass_fbs.push(fb);
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
            // SAFETY: the closure runs only in this function, after the device went idle, so no
            // command buffer in flight uses the sets; views and sets are this device's and
            // `write`/`info` live to the end of the call.
            unsafe { device.update_descriptor_sets(&write, &[]) };
        };
        for set in std::iter::once(&self.screen_set)
            .chain([&self.hdr_set])
            .chain(&self.bloom_sets)
            .chain(&self.glass_sets)
            .chain([&self.water_set])
        {
            let info = [vk::DescriptorImageInfo::default()
                .image_view(self.depth.view)
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(*set)
                .dst_binding(7)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&info)];
            // SAFETY: the device went idle at the top of this function, so no command buffer in
            // flight uses the set; the depth view is live and `write`/`info` live to the end of
            // the call.
            unsafe {
                device.update_descriptor_sets(&write, &[]);
            }
        }
        write_view(self.screen_set, 0, self.hdr.view);
        write_view(self.screen_set, 3, self.bloom[0].view);
        write_view(self.hdr_set, 0, self.hdr.view);
        write_view(self.hdr_set, 3, self.hdr.view);
        write_view(self.water_set, 0, self.refract.view);
        write_view(self.water_set, 3, self.refract.view);
        // SAFETY: the device went idle at the top of this function, so no command buffer in
        // flight uses `hull_set`; the hull depth view was just made and `write`/`info` live to
        // the end of the call.
        unsafe {
            let info = [vk::DescriptorImageInfo::default()
                .image_view(self.hull_depth.view)
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
            let write = [vk::WriteDescriptorSet::default()
                .dst_set(self.hull_set)
                .dst_binding(7)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&info)];
            device.update_descriptor_sets(&write, &[]);
        }
        for (set, image) in self.bloom_sets.iter().zip(&self.bloom) {
            write_view(*set, 0, image.view);
            write_view(*set, 3, self.hdr.view);
        }
        for (set, image) in self.glass_sets.iter().zip(&self.glass) {
            write_view(*set, 0, image.view);
            write_view(*set, 3, self.hdr.view);
        }
        self.timers.invalidate();
        Ok(())
    }

    /// Copies a new tick's mirror into GPU memory. Bulk copies only.
    /// The weather to play the match in (a map's `MapConfig`, or the skirmish
    /// choice). Starts the sky over.
    pub fn set_weather(&mut self, weather: mc_data::weather::Weather) {
        self.sky.set_weather(weather);
    }

    /// When in the day the match is played (24-hour clock): the sun's path,
    /// dusk colours, moonlight at night.
    pub fn set_hour(&mut self, hour: f32) {
        self.sky.set_hour(hour);
    }

    /// The weather of each of the map's regions, region 0 first (a map's
    /// `MapConfig::weathers`): one weather on a map without regions, as `set_weather`.
    /// The wind is the first's over the whole map. The regions are the map look's
    /// (`set_map_look`); a region given no weather plays the first.
    pub fn set_weathers(&mut self, weathers: &[mc_data::weather::Weather]) {
        self.sky.set_weathers(weathers);
    }

    /// Parks a raging storm over `at` (the test range's "storm overhead"),
    /// or lets the weather run by itself again.
    pub fn park_storm(&mut self, at: Option<glam::Vec2>) {
        self.sky.park_storm(at);
    }

    /// How hard it is raining where the camera looks, 0 to 1, for the rain's sound.
    pub fn rain_here(&self) -> f32 {
        self.sky.rain_here()
    }

    /// The clock the shaders animate by (`Globals::camera.w`), as of the last frame:
    /// what `shore::next_breaker` reads, so a wave is heard as it is seen to break.
    pub fn time(&self) -> f32 {
        self.last_time
    }

    /// The point the breakers at `xy` are worked out for (`shore::surf_point`): its
    /// region's own, on a map with regions.
    pub fn surf_point(&self, xy: glam::Vec2) -> glam::Vec2 {
        crate::shore::surf_point(self.look.walls().region_at(xy.x, xy.y), xy)
    }

    /// Lightning since the last call, for thunder.
    pub fn take_thunder(&mut self) -> Vec<crate::sky::Thunder> {
        self.sky.take_thunder()
    }

    /// A blast that throws the clouds outward, `reach` metres, `strength` 1 a
    /// big explosion and 3 a commander's reactor. Deaths and heavy impacts do
    /// this already; this is for anything bigger (a nuke) that wants its own.
    pub fn cloud_blast(&mut self, at: Vec3, reach: f32, strength: f32) {
        self.sky.blast(at, reach, strength, self.last_time);
        self.tree_blasts
            .record(at, self.last_time, reach, strength.min(1.0), false);
    }

    /// Big blasts throw the clouds about (sky.rs).
    fn stir_clouds(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::UnitDied { pos, blueprint, .. }
            | SimEvent::AircraftCrashed { pos, blueprint } => {
                let bp = self.blueprints.unit(*blueprint);
                let r = bp.radius.to_f32();
                // A commander's reactor tears a hole kilometres wide. Anything
                // else reaches a few times its own size; how much of that gets
                // up into the cloud is the sky's call (`Sky::blast`).
                let (reach, strength) = if bp.key.contains("commander") {
                    (1200.0, 3.0)
                } else {
                    ((r * 18.0).clamp(40.0, 300.0), (r / 6.0).clamp(0.3, 1.5))
                };
                if r >= 2.5 || bp.key.contains("commander") {
                    self.sky
                        .blast(Vec3::from(pos.to_f32()), reach, strength, time);
                }
            }
            SimEvent::Impact { pos, splash, .. } => {
                let splash = splash.to_f32();
                if splash >= 12.0 {
                    self.sky.blast(
                        Vec3::from(pos.to_f32()),
                        (splash * 6.0).min(400.0),
                        (splash / 20.0).min(1.5),
                        time,
                    );
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn sky_mut(&mut self) -> &mut crate::sky::Sky {
        &mut self.sky
    }

    fn upload_sim(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let part = mc_core::perf_span!("cpu.upload.instances");
        // The sim's tables bound the mirror, so this never cuts; it only keeps a mirror
        // that broke that promise out of the buffer's tail.
        let units = &frame.units[..frame.units.len().min(MAX_SIM_ENTITIES)];
        if units.len() < frame.units.len() {
            log::error!(
                "render mirror has {} entities; the renderer holds {}",
                frame.units.len(),
                units.len()
            );
        }
        let patched = self.titan_charge.patch(units, time);
        let patched = self.capital_fx.swing.patch(&self.blueprints, patched);
        self.dynamic.write(0, bytemuck::cast_slice(&patched));
        self.sim_units = units.len() as u32;
        // One per unit at most (`MAX_HOUSES`): never cut.
        let houses = &frame.houses[..frame.houses.len().min(MAX_HOUSES)];
        if !houses.is_empty() {
            self.houses.write(0, bytemuck::cast_slice(houses));
        }
        drop(part);
        let part = mc_core::perf_span!("cpu.upload.lists");
        self.note_gun_hulls(units, houses);
        self.stake_strikes(units, time);
        self.upload_welds(frame);
        self.upload_shields(frame, camera.eye());
        self.lights.tick(frame, &self.blueprints);

        let projectiles = &frame.projectiles[..frame.projectiles.len().min(MAX_PROJECTILES)];
        self.projectiles.write(0, bytemuck::cast_slice(projectiles));
        self.projectile_count = projectiles.len() as u32;

        self.upload_beams(frame, time);

        let stains = &frame.stains[..frame.stains.len().min(MAX_STAINS)];
        self.stains.write(0, bytemuck::cast_slice(stains));
        // Craters where blasts struck (impact_craters.rs) are drawn as stains too, after
        // the sim's.
        // A world with no scorch at all in its first seconds is a new one (the backdrop
        // restaged): its ground is whole. Later, a clean map is only one nothing has burnt yet;
        // a warhead leaves no sim scorch, so its crater must not be wiped with it.
        if frame.stains.is_empty() && frame.tick < 50 {
            self.wreck_fx.clear();
            self.wreck_finish.clear();
            self.hull_crash_fx.clear();
            self.impact_craters.clear();
            self.bore_fx.clear();
            self.plasma_fx.clear();
            self.nuke_fx.clear();
            self.craters.clear();
            self.ground_melt.clear();
        }
        self.impact_craters.land(time);
        let craters = self.impact_craters.craters();
        let craters = &craters[..craters.len().min(MAX_STAINS - stains.len())];
        if !craters.is_empty() {
            self.stains.write(
                std::mem::size_of_val(stains) as u64,
                bytemuck::cast_slice(craters),
            );
        }
        let crater_count = craters.len();
        let stained = stains.len() + crater_count;
        self.stain_runs = [stains.len() as u32, crater_count as u32];
        let pads = self.structure_pads(units, &frame.pads, MAX_STAINS.saturating_sub(stained));
        if !pads.is_empty() {
            self.stains.write(
                (stained * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&pads),
            );
        }
        self.pad_count = pads.len() as u32;
        let used = stained + pads.len();
        self.ore.upload(&self.stains, used, MAX_STAINS, time);

        let mut dead = self.cliff_rocks.dead_props(&frame.props_dead).to_vec();
        dead.resize(self.props_dead.size as usize / 4, 0);
        self.city_fx.hide_heaps(&frame.props_dead, &mut dead);
        self.props_dead.write(0, bytemuck::cast_slice(&dead));
        self.city_fx.set_looks(&frame.city, frame.tick);

        // Units glide from the last tick's place to this one's over the coming
        // tick, so what a tick reports is timed against that stretch.
        let since = time - self.last_tick_time;
        if since > 0.0 && since < 1.0 {
            self.tick_seconds = self.tick_seconds * 0.7 + since * 0.3;
        }
        self.last_tick_time = time;

        drop(part);
        let part = mc_core::perf_span!("cpu.upload.unit_fx");
        self.ground_contact(units, time, camera);
        self.storm_beams(time);
        self.storms_tick(time);
        self.mine_blows(units, time, camera);
        self.aircraft_trails(units, time, camera);
        self.heat_haze.note(units, self.tick_seconds);
        self.aircraft_crash_trails(units, time, camera);
        self.damage_smoke(units, time, camera);
        self.wreck_smoke(units, time, camera);
        self.wreck_finish(units, &frame.events, time, camera);
        drop(part);
        let part = mc_core::perf_span!("cpu.upload.events");
        self.construction(frame, time, camera);
        // A blast is known before the trees it kills are looked at (`nuke_fx::tree_fate`).
        for event in &frame.events {
            if matches!(event, SimEvent::NuclearDetonation { .. }) {
                self.nuke_event(event, time);
            }
        }
        self.tree_fires(frame, time, camera);
        self.city_events(frame, time, camera);
        self.trample_trees(frame, time);
        self.clear_lots(frame, time);
        for event in &frame.events {
            self.effects_of(event, time);
            self.stir_clouds(event, time);
            self.impact_crater(event, time);
        }
        drop(part);
        let part = mc_core::perf_span!("cpu.upload.sky");
        self.sky.set_units(units.iter().map(|u| {
            let bp = self
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16));
            bp.transport
                .is_none()
                .then_some(glam::Vec2::new(u.pos[0], u.pos[1]))
        }));
        let flyers: Vec<_> = units
            .iter()
            .filter(|u| {
                stirs_clouds(
                    u,
                    self.blueprints
                        .unit(mc_data::BlueprintId(u.blueprint as u16)),
                )
            })
            .map(|u| {
                let r = self
                    .blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16))
                    .radius
                    .to_f32();
                (Vec3::from(u.prev_pos), Vec3::from(u.pos), r)
            })
            .collect();
        self.sky.set_flyers(flyers.into_iter());
        drop(part);
        let _part = mc_core::perf_span!("cpu.upload.weapon_fx");
        self.ground_fires(&frame.fires, time);
        self.flush_rail_misses(time);
        self.rail_wakes(projectiles, time, camera);
        self.write_fade_beams(time);
        self.write_plasma_fx(units, time);
        self.regency_guns_tick(units, time);
        self.regency_trails(projectiles, time);
        self.gravitic_tick(projectiles, time);
        self.excavation_tick(units, time, camera);
        self.star_core_tick(units, time, camera);
        self.lance_core_tick(units, time, camera);
        self.bolt_rifle_tick(units, &frame.houses, time);
        self.arc_howitzer_tick(units, &frame.houses, time);
        self.write_bore_strokes(time);
        self.heavy_rail_tick(units, &frame.houses, projectiles, time);
        self.emp_tick(frame, units, time, camera);
        self.great_gun_tick(projectiles, time);
        self.missile_trails(projectiles, time, camera);
        self.nuke_tick(frame, time, camera);
        self.write_regency_trails(time);
        self.stream_bursts(projectiles, time, camera);
        self.sea_tick(units, projectiles, time, camera);
        self.warp_tick(frame, time, camera);
    }

    /// The rounds of a stream gun's shot (`Weapon::rounds`) have no sim impact of their
    /// own. Each that lands on what its shot hit bursts there: a small orange pop and a
    /// few sparks, as its shot does; a Pinched jet's pours in as plasma (`pinched_pour`).
    fn stream_bursts(&mut self, projectiles: &[ProjectileInstance], time: f32, camera: &Camera) {
        let reach = camera.distance * 2.5 + 300.0;
        let focus = camera.focus.truncate();
        for p in projectiles {
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            if p._pad[1] < 0.5 || ends <= 0.0 {
                continue;
            }
            let at = Vec3::from(p.pos);
            let start = time + ends * self.tick_seconds;
            if regency_guns_fx::drawn_look(p) == 1 {
                self.pinched_pour(at, p.size, start);
                continue;
            }
            let size = p.size.max(0.3);
            // Red rounds (`Weapon::red`, `_pad[0]` above one) pop red.
            let kind = if p._pad[0] > 1.5 { 8.0 } else { 1.0 };
            self.push_effect(at.to_array(), start, size * 5.5, 0.18, kind, 0.7);
            if camera.distance > 1400.0 || at.truncate().distance(focus) > reach {
                continue;
            }
            for _ in 0..3 {
                let vel = self.scatter.upward(0.3) * (14.0 + self.scatter.unit() * 18.0);
                let life = 0.2 + self.scatter.unit() * 0.15;
                self.push_puff(PUFF_SPARK, at, vel, start, life, (0.24, 0.06));
            }
        }
    }

    /// Reclaim beams have a beginning and an end the sim does not tell of: it only lists
    /// who is at work each tick. A beam new to the list starts empty and fills from the
    /// target; one that has left it stops tearing bits loose and lets those in flight arrive.
    fn upload_beams(&mut self, frame: &RenderFrame, time: f32) {
        let all = self.work_beams.tick(frame, time);
        self.beams.write(0, bytemuck::cast_slice(&all));
    }

    fn upload_welds(&mut self, frame: &RenderFrame) {
        let mut gpu: Vec<GpuWeld> = frame
            .welds
            .iter()
            .take(MAX_CONSTRUCTION_WELDS)
            .map(|w| GpuWeld {
                local: w.local,
                fade: w.fade,
            })
            .collect();
        if gpu.is_empty() {
            gpu.push(GpuWeld {
                local: [0.0; 3],
                fade: 0.0,
            });
        }
        self.welds.write(0, bytemuck::cast_slice(&gpu));
    }

    /// Everything that shines this frame, into scene set bindings 25 and 26.
    fn upload_lights(&mut self, time: f32, alpha: f32, camera: &Camera) {
        // The nearest few hundred burning trees light the ground; past that it is a glow.
        let focus = camera.focus;
        let mut lit: Vec<&BurningTree> = self.burning_trees.iter().collect();
        if lit.len() > 256 {
            lit.sort_by(|a, b| {
                Vec3::from(a.instance.pos)
                    .distance_squared(focus)
                    .total_cmp(&Vec3::from(b.instance.pos).distance_squared(focus))
            });
            lit.truncate(256);
        }
        for tree in lit {
            let at = Vec3::from(tree.instance.pos) + Vec3::Z * tree.height * 0.3;
            self.lights.tree_fire(at, time - tree.start);
        }
        for (at, size, age) in self.city_fx.fire_lights(time, focus) {
            self.lights.building_fire(at, size, age);
        }
        for b in &self.fade_beams {
            let k = 1.0 - ((time - b.start) / b.life.max(0.01)).clamp(0.0, 1.0);
            // A rail slug's path is white-hot and thin: a faint white light, never blue.
            // A laser shot and a gravity crush's tether light red.
            let color = match b.kind {
                FADE_LASER | fade_beam::GRAVITY_TETHER => Vec3::new(1.0, 0.08, 0.04),
                FADE_RAIL => Vec3::new(1.0, 0.96, 0.9) * 0.3,
                _ => Vec3::new(0.3, 0.62, 1.0),
            };
            self.lights.beam(
                b.from,
                b.to,
                color * 90.0 * k * b.width.clamp(0.3, 3.0),
                10.0 + 4.0 * b.width,
            );
        }
        self.capital_lights(time, alpha);
        self.warp_lights(time);
        self.regency_guns_lights(time);
        self.gravitic_lights(time);
        self.star_core_lights(time);
        self.lance_core_lights(time);
        self.heavy_rail_lights(time);
        self.emp_lights(time);
        self.bolt_rifle_lights(time);
        self.arc_howitzer_lights(time);
        let dark = self.sky.darkness();
        let (list, grid) = self.lights.build(time, alpha, camera, dark);
        let list_bytes: &[u8] = bytemuck::cast_slice(list);
        let grid_bytes: &[u8] = bytemuck::cast_slice(grid);
        self.light_stage.write(0, list_bytes);
        self.light_stage.write(self.light_list.size, grid_bytes);
        // An empty grid means the GPU's copy is already right (all zero: no lights
        // now or last frame), so nothing is sent.
        self.light_copy = (list_bytes.len() as u64, grid_bytes.len() as u64);
    }

    /// Copies this frame's lights to the GPU's own buffers, before any pass reads them.
    fn record_light_copy(&mut self, cmd: vk::CommandBuffer) {
        let device = &self.gpu.device;
        let (list, grid) = std::mem::take(&mut self.light_copy);
        // SAFETY: `cmd` is recording (called from `render` between begin and end).
        // `light_stage` (TRANSFER_SRC) holds the list at 0 and the grid at `light_list.size`:
        // the list is capped at `MAX_LIGHTS`, the size of `light_list`, and the stage's
        // bounds-checked write keeps the grid within `light_grid`'s size. The GPU is done
        // with the previous frame's copy, since `render` waited on the frame fence.
        unsafe {
            if list > 0 {
                device.cmd_copy_buffer(
                    cmd,
                    self.light_stage.buffer,
                    self.light_list.buffer,
                    &[vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: list,
                    }],
                );
            }
            if grid > 0 {
                device.cmd_copy_buffer(
                    cmd,
                    self.light_stage.buffer,
                    self.light_grid.buffer,
                    &[vk::BufferCopy {
                        src_offset: self.light_list.size,
                        dst_offset: 0,
                        size: grid,
                    }],
                );
            }
            if list > 0 || grid > 0 {
                let barrier = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)];
                device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &barrier,
                    &[],
                    &[],
                );
            }
        }
    }

    fn push_shield_hit(&mut self, pos: [f32; 3], start: f32, strength: f32) {
        let hit = ShieldHit {
            pos,
            start,
            strength,
            _pad: [0.0; 3],
        };
        self.shield_hits.write(
            (self.shield_hit_cursor * size_of::<ShieldHit>()) as u64,
            bytemuck::bytes_of(&hit),
        );
        self.shield_hit_cursor = (self.shield_hit_cursor + 1) % MAX_SHIELD_HITS;
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
        let origin = self.effect_origin.unwrap_or(Vec3::from(pos));
        let outbound = self.effect_outbound;
        if !outbound
            && self
                .live_effect_barriers
                .iter()
                .any(|b| b.crosses(origin, Vec3::from(pos)))
        {
            return;
        }
        self.lights.effect(pos, start, radius, life, kind);
        let e = Effect {
            // w 0: no shield clips it (sprites.wgsl).
            origin: origin.extend(if outbound { 0.0 } else { 1.0 }).to_array(),
            pos,
            start,
            params: [radius, life, kind, ring],
        };
        self.effects.write(
            (self.effect_cursor * size_of::<Effect>()) as u64,
            bytemuck::bytes_of(&e),
        );
        self.effect_cursor = (self.effect_cursor + 1) % EFFECT_RING;
    }

    /// An expanding 3D pressure sphere. `radius` is the reach in metres.
    /// `color` is the weapon colour (0 blue, 1 orange): energy waves are blue, guns stay dust.
    /// `axis` is the barrel for a muzzle blast (the bright ring sits perpendicular
    /// to it); zero for a blast that opens the same in every direction.
    fn push_shockwave(
        &mut self,
        pos: [f32; 3],
        start: f32,
        radius: f32,
        life: f32,
        strength: f32,
        color: f32,
        axis: Vec3,
    ) {
        let center = Vec3::from(pos);
        let origin = self.effect_origin.unwrap_or(center);
        if self
            .live_effect_barriers
            .iter()
            .any(|b| b.crosses(origin, center))
        {
            return;
        }
        self.tree_blasts
            .record(center, start, radius, strength, axis != Vec3::ZERO);
        let e = GpuShockwave {
            pos,
            start,
            params: [radius, life, color, strength.clamp(0.0, 1.0)],
            axis: axis.normalize_or_zero().to_array(),
            _pad: 0.0,
            tint: self
                .effect_settings
                .shockwave_color
                .map_or([0.0; 4], |rgb| [rgb[0], rgb[1], rgb[2], 1.0]),
        };
        self.shockwaves.write(
            (self.shockwave_cursor * size_of::<GpuShockwave>()) as u64,
            bytemuck::bytes_of(&e),
        );
        self.shockwave_ends[self.shockwave_cursor] = start + life.max(0.0);
        self.shockwave_cursor = (self.shockwave_cursor + 1) % MAX_SHOCKWAVES;
        let previous_origin = self.effect_origin.replace(center);
        self.shockwave_ground_dust(center, start, radius, life, strength, axis);
        self.effect_origin = previous_origin;
    }

    /// Schedule once at birth, rather than emitting every rendered frame.
    /// Three staggered bands produce a swept patch of dust, not a dotted ring.
    fn shockwave_ground_dust(
        &mut self,
        center: Vec3,
        start: f32,
        radius: f32,
        life: f32,
        strength: f32,
        axis: Vec3,
    ) {
        if radius < 8.0 || strength < 0.15 || life <= 0.0 {
            return;
        }
        let map_size = Vec3::from(
            self.map_info
                .size_metres()
                .extend(mc_core::Fx::ZERO)
                .to_f32(),
        );
        let water = self.map_info.water_level.to_f32();
        let phase = self.scatter.unit() * std::f32::consts::TAU;
        for band in 0..3 {
            for sector in 0..24 {
                let angle = phase
                    + (sector as f32 + band as f32 * 0.38 + self.scatter.signed() * 0.28)
                        * std::f32::consts::TAU
                        / 24.0;
                let outward = Vec3::new(angle.cos(), angle.sin(), 0.0);
                let reach = radius * (0.20 + band as f32 * 0.23 + self.scatter.signed() * 0.06);
                let mut ground = center + outward * reach;
                if ground.x < 0.0
                    || ground.y < 0.0
                    || ground.x > map_size.x
                    || ground.y > map_size.y
                {
                    continue;
                }
                ground.z = self.ground_height(ground.truncate());
                let Some((arrival, pressure)) =
                    shockwave_ground_arrival(center, ground, radius, axis, water)
                else {
                    continue;
                };
                let power = (pressure * strength.clamp(0.0, 1.0)).sqrt();
                let size = (radius * 0.14).clamp(2.0, 12.0) * power;
                let born = start + arrival * life;
                let drift = outward * ((7.0 + self.scatter.unit() * 8.0) * power)
                    + Vec3::Z * (1.4 + self.scatter.unit() * 2.2);
                let duration = 2.0 + self.scatter.unit() * 1.0;
                let spread = size * (2.8 + self.scatter.unit() * 1.1);
                self.push_puff(
                    PUFF_SHOCK_DUST,
                    ground + Vec3::Z * (0.45 + size * 0.18),
                    drift,
                    born,
                    duration,
                    (size, spread),
                );
                if (sector + band * 2) % 12 == 0 {
                    self.push_puff(
                        PUFF_SHOCK_SMOKE,
                        ground + Vec3::Z * 0.8,
                        drift * 0.45 + Vec3::Z * 1.8,
                        born + 0.06,
                        duration + 0.6,
                        (size * 0.85, spread * 0.85),
                    );
                }
            }
        }
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
        self.push_puff_with_motion(kind, pos, vel, start, life, size, Vec3::ZERO);
    }

    // Ion ribbons use appearance.xyz for emitter velocity; their vel stays an
    // independent aft axis so forward flight cannot reverse the plume.
    fn push_puff_with_motion(
        &mut self,
        kind: f32,
        pos: Vec3,
        vel: Vec3,
        start: f32,
        life: f32,
        size: (f32, f32),
        motion: Vec3,
    ) {
        let origin = self.effect_origin.unwrap_or(pos);
        let outbound = self.effect_outbound;
        if !outbound
            && self
                .live_effect_barriers
                .iter()
                .any(|b| b.crosses(origin, pos))
        {
            return;
        }
        // An origin this high marks a puff no shield clips (puffs.wgsl `fs_puff`).
        let origin = if outbound {
            Vec3::new(pos.x, pos.y, PUFF_UNCLIPPED_Z)
        } else {
            origin
        };
        let dusty = kind == PUFF_DUST
            || kind == PUFF_SMOKE
            || kind == PUFF_SHOCK_DUST
            || kind == PUFF_SHOCK_SMOKE;
        let opacity = if dusty {
            self.effect_settings.dust_visibility
        } else {
            1.0
        };
        let life = life
            * if dusty {
                self.effect_settings.dust_lifetime
            } else {
                1.0
            };
        if opacity <= 0.0 || life <= 0.0 {
            return;
        }
        // A blast or flak puff's x is its heat (blast_fx.rs, flak_fx.rs); a blast's y above
        // a half burns red (laser_fx.rs). A smoke tube's x, when given, is its strength,
        // below zero for black smoke (gravitic_fx.rs `seeker_smoke`).
        let appearance =
            if kind == PUFF_ION || kind == blast_fx::PUFF_BLAST || kind == flak_fx::PUFF_FLAK {
                [motion.x, motion.y, motion.z, 1.0]
            } else if kind == nuke_fx::PUFF_STRATEGIC_TRAIL && motion.x != 0.0 {
                [-1.0, -1.0, -1.0, motion.x]
            } else if dusty && motion != Vec3::ZERO {
                // Dust or smoke of a colour of its own: a city's masonry (city_fx.rs).
                [
                    motion.x,
                    motion.y,
                    motion.z,
                    self.effect_settings.dust_brightness,
                ]
            } else if dusty {
                let rgb = self.effect_settings.dust_color.unwrap_or([-1.0; 3]);
                [rgb[0], rgb[1], rgb[2], self.effect_settings.dust_brightness]
            } else {
                [-1.0, -1.0, -1.0, 1.0]
            };
        let seed = self.scatter.unit();
        let p = Puff {
            appearance,
            origin: origin.to_array(),
            opacity,
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
        self.puff_cursor = (self.puff_cursor + 1) % PUFF_RING;
    }

    fn push_mark(&mut self, mark: TrackMark) {
        self.track_marks.write(
            (self.track_cursor * size_of::<TrackMark>()) as u64,
            bytemuck::bytes_of(&mark),
        );
        self.track_cursor = (self.track_cursor + 1) % MAX_TRACK_MARKS;
        self.track_count = (self.track_count + 1).min(MAX_TRACK_MARKS as u32);
    }

    /// The flame is a draped wave in the reserved effect slots. Smoke stays in
    /// the puff tail so a later blast cannot erase a column that is still rising.
    fn ground_fires(&mut self, fires: &[FireInstance], time: f32) {
        let shown = fires.len().min(MAX_GROUND_FIRES);
        let first = fires.len() - shown;
        let mut waves = vec![Effect::zeroed(); MAX_GROUND_FIRES];
        let mut smoke = vec![Puff::zeroed(); GROUND_FIRE_SLOTS];
        for (n, fire) in fires[first..].iter().enumerate() {
            let duration = fire.duration.max(0.05);
            let age = (fire.elapsed / duration).clamp(0.0, 0.999);
            let pos = Vec3::from(fire.pos);
            waves[n] = Effect {
                origin: pos.extend(1.0).to_array(),
                pos: fire.pos,
                start: time - age * duration,
                // Kind 7: a napalm puddle. The shader tears the shape and boils the heat in place.
                params: [fire.radius, duration, 7.0, 0.0],
            };
            if n * 2 + 1 >= GROUND_FIRE_SLOTS {
                continue;
            }
            let seed = ground_hash(fire.pos[0] + fire.pos[1], fire.pos[2].to_bits(), 1);
            let start = time - fire.elapsed;
            let cycle = 3.2;
            let phase = ((time - start) / cycle).floor().max(0.0);
            let smoke_start = start + phase * cycle;
            let base = (fire.radius * 0.45).clamp(3.0, 8.0);
            for i in 0..2u32 {
                let ang = ground_hash(seed, i, 7) * std::f32::consts::TAU;
                let dist = fire.radius * ground_hash(seed, i, 8).sqrt() * 0.65;
                let at = pos + Vec3::new(ang.cos() * dist, ang.sin() * dist, 0.5);
                smoke[n * 2 + i as usize] = Puff {
                    appearance: [-1.0, -1.0, -1.0, 1.0],
                    origin: at.to_array(),
                    opacity: 1.0,
                    pos: at.to_array(),
                    start: smoke_start,
                    vel: [ang.cos() * 0.3, ang.sin() * 0.3, 1.6],
                    life: cycle,
                    params: [base * 0.35, base * 1.1, PUFF_SMOKE, ground_hash(seed, i, 9)],
                };
            }
        }
        self.effects.write(
            (EFFECT_RING * size_of::<Effect>()) as u64,
            bytemuck::cast_slice(&waves),
        );
        self.puffs.write(
            (PUFF_RING * size_of::<Puff>()) as u64,
            bytemuck::cast_slice(&smoke),
        );
    }

    /// Newly destroyed trees beside a blast or anywhere along a bore channel ignite.
    /// Reclaim and construction clearing therefore never light a forest on fire.
    /// These are bounded presentation emitters; they do not change simulation state.
    fn tree_fires(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        self.burning_trees.retain(|tree| time - tree.start < 42.0);
        let impacts: Vec<(Vec3, f32)> = frame
            .events
            .iter()
            .filter_map(|event| {
                if let SimEvent::Impact {
                    pos,
                    splash,
                    on_shield,
                    ..
                } = event
                {
                    if !on_shield && splash.to_f32() > 0.0 {
                        return Some((Vec3::from(pos.to_f32()), splash.to_f32()));
                    }
                }
                None
            })
            .collect();
        for (word, &dead) in frame.props_dead.iter().enumerate() {
            // On first upload, old destruction is history, not a new forest fire.
            let mut changed = dead & !self.previous_dead.get(word).copied().unwrap_or(dead);
            while changed != 0 {
                let bit = changed.trailing_zeros();
                changed &= changed - 1;
                let index = word * 32 + bit as usize;
                let Some(&instance) = self.prop_instances.get(index) else {
                    continue;
                };
                let kind = instance.blueprint.wrapping_sub(self.tree_model_base);
                if kind >= fallen_trees::TREE_KINDS || self.burning_trees.len() >= MAX_BURNING_TREES
                {
                    continue;
                }
                let at = Vec3::from(instance.pos);
                // A nuclear blast throws the near trees flat and sets the rest alight.
                match self.nuke_fx.tree_fate(at.truncate(), time) {
                    Some(nuke_fx::TreeFate::Flattened { away, at: start }) => {
                        self.blow_down_tree(index as u32, away, start);
                        continue;
                    }
                    Some(nuke_fx::TreeFate::Gone) => continue,
                    Some(nuke_fx::TreeFate::Burning) => {
                        let height = fallen_trees::tree_height(kind, &instance);
                        // Lit by the flash, not all in the same instant.
                        let start = time - self.scatter.unit() * 1.5;
                        self.burning_trees.push(BurningTree {
                            instance,
                            start,
                            height,
                        });
                        continue;
                    }
                    None => {}
                }
                let blasted = impacts.iter().any(|(center, radius)| {
                    at.truncate().distance(center.truncate()) <= radius + 2.0
                });
                let seared = frame.events.iter().any(|event| {
                    let SimEvent::BoreDischarge {
                        from, to, width, ..
                    } = event
                    else {
                        return false;
                    };
                    let from = glam::Vec2::from(from.xy().to_f32());
                    let to = glam::Vec2::from(to.xy().to_f32());
                    let segment = to - from;
                    let t = ((at.truncate() - from).dot(segment)
                        / segment.length_squared().max(0.001))
                    .clamp(0.0, 1.0);
                    at.truncate().distance(from + segment * t) <= width.to_f32().max(4.0) + 0.1
                });
                if !blasted && !seared && !self.wake_seared(at.truncate(), time) {
                    continue;
                }
                let height = fallen_trees::tree_height(kind, &instance);
                self.burning_trees.push(BurningTree {
                    instance,
                    start: time,
                    height,
                });
            }
        }
        self.previous_dead.clone_from(&frame.props_dead);
        // A burning forest shares a budget: each tree is drawn from less often as more burn.
        let share = (160.0 / self.burning_trees.len().max(1) as f32).min(1.0);
        for i in 0..self.burning_trees.len() {
            let tree = self.burning_trees[i];
            let age = time - tree.start;
            let mut at = Vec3::from(tree.instance.pos);
            if at.distance(camera.focus) > camera.distance * 2.5 + 250.0 {
                continue;
            }
            if share < 1.0 && self.scatter.unit() > share {
                continue;
            }
            at.z = self.ground_height(at.truncate());
            let h = tree.height;
            let strength = (1.0 - age / 30.0).clamp(0.0, 1.0);
            if strength > 0.0 {
                for _ in 0..4 {
                    let angle = self.scatter.unit() * std::f32::consts::TAU;
                    let flame = at
                        + Vec3::new(
                            angle.cos() * h * 0.30,
                            angle.sin() * h * 0.30,
                            h * (0.40 + self.scatter.unit() * 0.40),
                        );
                    let rise = Vec3::new(0.6, 0.25, 3.0 + strength * 3.0);
                    self.push_puff(
                        PUFF_TREE_FIRE,
                        flame,
                        rise,
                        time,
                        1.05,
                        (h * 0.12 * strength, h * 0.27 * strength),
                    );
                }
            }
            // Emission is throttled independently of the rendered frame rate.
            if self.scatter.unit() < 0.5 {
                let smoke = at + Vec3::Z * h * (0.60 + strength * 0.2);
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    smoke,
                    (self.sky.wind_heading() * 1.3).extend(7.0),
                    time,
                    6.5,
                    (h * 0.12, h * 0.55),
                );
            }
            if strength > 0.2 && self.scatter.unit() < 0.18 {
                let vel = Vec3::new(1.0, 0.4, 8.0 + self.scatter.unit() * 8.0);
                self.push_puff(
                    PUFF_SPARK,
                    at + Vec3::Z * h * 0.7,
                    vel,
                    time,
                    1.0,
                    (0.16, 0.025),
                );
            }
        }
    }

    /// A hurt unit smokes from its burn marks. Nothing on it burns: a first mark
    /// only wisps, a unit near death pours thick smoke from every mark. The marks
    /// are the ones the unit shader paints (`models::burns`), so the smoke rises
    /// from the scorch and not from somewhere near it; it is left in the world,
    /// so a moving unit trails it, and the wind carries it off.
    fn damage_smoke(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        let hidden = KIND_WRECK
            | KIND_GHOST
            | KIND_PROP
            | STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        let reach = camera.distance * 2.5 + 300.0;
        let wind = self.sky.wind_heading();
        for u in units {
            if u.owner_flags & hidden != 0 || u.build < 1.0 || u.health >= 0.9 {
                continue;
            }
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            if to.distance(camera.focus) > reach {
                continue;
            }
            let Some(site) = self.burn_sites.get(u.blueprint as usize) else {
                continue;
            };
            let marks = models::burns::burn_marks(u.unit_id, u.health, site.reach, site.height);
            let hurt = 1.0 - u.health.clamp(0.0, 1.0);
            // From a wisp at the first mark to a heavy column past half health.
            let heavy = ((hurt - 0.3) / 0.45).clamp(0.0, 1.0);
            let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let moving =
                ((to - from).length() / self.tick_seconds.max(0.02) / 10.0).clamp(0.0, 1.0);
            // Gathered first: standing a puff on the hull reads the site, emitting needs the whole renderer.
            let mut puffs: Vec<(Vec3, f32, f32)> = Vec::new();
            for mark in marks {
                if mark.grow < 0.2 {
                    continue;
                }
                // The burnt-through middle of the mark: the smoke comes from anywhere on it.
                let core = mark.radius * mark.grow * 0.4;
                let scatter = &mut self.scatter;
                let mut stand = |spread: f32| -> Option<(Vec3, f32)> {
                    let angle = scatter.unit() * std::f32::consts::TAU;
                    let r = core * spread * scatter.unit().sqrt();
                    let (local, on_turret) = site.grid.surface(
                        mark.centre[0] + angle.cos() * r,
                        mark.centre[1] + angle.sin() * r,
                    )?;
                    let mut local = Vec3::from(local);
                    if on_turret {
                        let (s, c) = u.turret_yaw.sin_cos();
                        let d = local - site.turret_pivot;
                        local = site.turret_pivot
                            + Vec3::new(d.x * c - d.y * s, d.x * s + d.y * c, d.z);
                    }
                    let t = scatter.unit();
                    let (s, c) = (u.prev_heading + turn * t).sin_cos();
                    let at = from.lerp(to, t)
                        + Vec3::new(
                            local.x * c - local.y * s,
                            local.x * s + local.y * c,
                            local.z,
                        );
                    Some((at, time + t * self.tick_seconds))
                };
                let smoke = (core * 0.6).clamp(0.5, 5.0) * (0.8 + 0.35 * heavy);
                // A fast unit lays a second puff each tick, or the trail breaks into dots;
                // a badly hurt one pours out more.
                for _ in 0..1 + (moving > 0.5) as usize + (heavy * mark.grow > 0.5) as usize {
                    if let Some((at, start)) = stand(0.5) {
                        puffs.push((at, start, smoke));
                    }
                }
            }
            let thick = 0.65 + 0.35 * heavy;
            for (at, start, size) in puffs {
                // A dived boat, or the part of a hull under the water: no smoke there.
                if self.under_sea(at) {
                    continue;
                }
                if self.scatter.unit() > (0.3 + 0.5 * thick) * thick.max(0.6) {
                    continue;
                }
                let gust = 1.1 + 0.6 * heavy + self.scatter.signed() * 0.4;
                let drift = (wind * gust
                    + glam::Vec2::new(self.scatter.signed(), self.scatter.signed()) * 0.5)
                    .extend(1.5 + size * 0.8);
                let life =
                    (2.4 + size * 1.2 + 2.0 * heavy).min(9.0) * (0.8 + 0.4 * self.scatter.unit());
                let grown = size * (2.2 + 1.2 * thick);
                self.push_puff(
                    PUFF_TREE_SMOKE,
                    at + Vec3::Z * size * 0.4,
                    drift,
                    start,
                    life,
                    (size * 0.6, grown),
                );
            }
        }
    }

    /// A falling wreck trails smoke. A burning hull keeps a flame on it, and
    /// only occasionally a wisp of smoke leaving that flame. A spent casing in the air
    /// (`UnitBlueprint::scrap`) is cold metal and trails nothing.
    fn aircraft_crash_trails(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        self.forget_falling_hulls(time);
        for u in units {
            let falling = u.owner_flags & KIND_WRECK != 0
                && u.packed == mc_sim::mirror::WRECK_FALLING
                && !self
                    .blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16))
                    .scrap;
            let burning = u.owner_flags & (KIND_WRECK | STATE_RADAR) == 0
                && u.packed & mc_sim::mirror::UNIT_BURNING != 0;
            if !falling && !burning {
                continue;
            }
            // A capital hull burns along its length (capital_crash_fx.rs).
            if falling
                && self
                    .blueprints
                    .unit(mc_data::BlueprintId(u.blueprint as u16))
                    .is_capital_ship()
            {
                if Vec3::from(u.pos).distance(camera.focus) <= camera.distance * 3.0 + 800.0 {
                    self.capital_falling(u, time);
                }
                continue;
            }
            let from = Vec3::from(u.prev_pos);
            let to = Vec3::from(u.pos);
            if to.distance(camera.focus) > camera.distance * 3.0 + 400.0 {
                continue;
            }
            let samples = (from.distance(to) / 2.0).ceil().clamp(1.0, 16.0) as usize;
            let r = u.radius;
            for i in 0..samples {
                let t = (i as f32 + 0.5) / samples as f32;
                let start = time + t * self.tick_seconds;
                if self.under_sea(from.lerp(to, t)) {
                    continue;
                }
                if falling {
                    let at = from.lerp(to, t);
                    let drift = Vec3::new(1.2, 0.4, 2.5);
                    self.push_puff(PUFF_SMOKE, at, drift, start, 4.5, (r * 0.38, r * 1.8));
                    self.push_puff(PUFF_FIRE, at, drift, start, 0.55, (r * 0.32, r * 0.5));
                    let vel = self.scatter.upward(0.1) * 9.0;
                    self.push_puff(PUFF_SPARK, at, vel, start, 0.8, (0.3, 0.04));
                    continue;
                }
                // Napalm on a live hull: flames on the body. Smoke is the
                // exception, a wisp that leaves the fire.
                let spread = (r * 0.35).clamp(0.45, 3.2);
                let at = from.lerp(to, t)
                    + Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        0.55 + self.scatter.unit() * 0.7,
                    ) * spread;
                let rise = Vec3::new(
                    self.scatter.signed() * 0.35,
                    self.scatter.signed() * 0.35,
                    1.6 + self.scatter.unit() * 1.4,
                );
                let flame_life = 0.9 + self.scatter.unit() * 0.35;
                self.push_puff(
                    PUFF_FIRE,
                    at,
                    rise,
                    start,
                    flame_life,
                    ((r * 0.22).clamp(0.45, 1.6), (r * 0.5).clamp(0.8, 2.4)),
                );
                if self.scatter.unit() < 0.22 {
                    let trail = Vec3::new(
                        self.scatter.signed() * 1.5,
                        self.scatter.signed() * 1.5,
                        3.2 + self.scatter.unit() * 2.2,
                    );
                    let smoke_life = 2.2 + self.scatter.unit() * 0.7;
                    self.push_puff(
                        PUFF_SMOKE,
                        at + Vec3::Z * 0.45,
                        trail,
                        start + 0.08,
                        smoke_life,
                        ((r * 0.14).clamp(0.3, 0.9), (r * 0.7).clamp(0.8, 2.2)),
                    );
                }
                if self.scatter.unit() < 0.4 {
                    let vel = self.scatter.upward(0.2) * (5.0 + self.scatter.unit() * 7.0);
                    self.push_puff(PUFF_SPARK, at, vel, start, 0.5, (0.18, 0.04));
                }
            }
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
            .filter(|p| p.color & PROJECTILE_BEAM != 0)
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
            // A Regency refit has no welding: its nanites are drawn round it as a site's
            // are (beams.wgsl `BEAM_NANITE_SITE`).
            .filter(|u| u.status[1] & mc_sim::mirror::UNIT_NANITE == 0)
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

    /// Crackle at the slug's head so the sheath reads around it, not only behind it.
    fn emit_plasma_head(&mut self, at: Vec3, dir: Vec3, start: f32, plasma: f32) {
        for _ in 0..4 {
            let spray = (dir * 0.45
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.85)
                .normalize_or_zero();
            let life = 0.1 + self.scatter.unit() * 0.08;
            let speed = 5.0 + self.scatter.unit() * 9.0;
            self.push_puff(
                PUFF_BOLT,
                at + spray * plasma * 0.12,
                spray * speed,
                start,
                life,
                (0.28 + plasma * 0.04, 0.07),
            );
        }
    }

    /// Keep lingering hitscans in the projectile buffer so they fade on the GPU
    /// after the tick that spawned them.
    fn write_fade_beams(&mut self, time: f32) {
        self.fade_beams.retain(|b| time < b.start + b.life);
        let size = size_of::<ProjectileInstance>();
        for beam in &self.fade_beams {
            let i = self.projectile_count as usize;
            if i >= MAX_PROJECTILES {
                break;
            }
            let inst = ProjectileInstance {
                prev_pos: beam.from.to_array(),
                color: PROJECTILE_FADE_BEAM | beam.kind,
                pos: beam.to.to_array(),
                size: beam.width,
                wake: beam.start,
                plasma: beam.life,
                _pad: [0.0; 2],
                aim: [0.0; 4],
                prev_aim: [0.0; 4],
            };
            self.projectiles
                .write((i * size) as u64, bytemuck::bytes_of(&inst));
            self.projectile_count += 1;
        }
    }

    /// A hitscan shot's beam, muzzle to `to`: a hot core that is gone almost at once,
    /// inside the ionised channel it leaves hanging a moment longer.
    fn rail_beam(&mut self, from: Vec3, to: Vec3, width: f32, hot: bool, time: f32) {
        if !hot {
            self.fade_beams.push(FadeBeam {
                from,
                to,
                start: time,
                life: 0.14,
                width: width * 1.8,
                kind: FADE_PLAIN,
            });
            self.fade_beams.push(FadeBeam {
                from,
                to,
                start: time,
                life: 0.55,
                width,
                kind: FADE_PLAIN,
            });
            return;
        }
        // A rail slug is there the moment it is fired: its whole path flashes white-hot
        // and fades, and the air it tore leaves a thin trail of white vapour that hangs
        // and drifts off on the wind.
        // Thin: a slug's path, not a beam weapon.
        let line = (width * 0.3).clamp(0.25, 0.7);
        self.fade_beams.push(FadeBeam {
            from,
            to,
            start: time,
            life: 0.1,
            width: line * 1.6,
            kind: FADE_RAIL,
        });
        self.fade_beams.push(FadeBeam {
            from,
            to,
            start: time,
            life: 0.6,
            width: line * 0.6,
            kind: FADE_RAIL,
        });
        let length = from.distance(to);
        let wind = self.sky.wind_heading();
        let n = ((length / 7.0) as usize).clamp(2, 70);
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let at = from + (to - from) * t;
            let drift = (wind * (1.0 + self.scatter.unit())).extend(0.3);
            let life = 1.4 + self.scatter.unit() * 1.2;
            let grow = width * (1.6 + self.scatter.unit());
            self.push_puff(
                water_fx::PUFF_STEAM,
                at,
                drift,
                time + t * 0.02,
                life,
                (width * 0.5, grow),
            );
        }
    }

    /// Draws the beam for the hitscan shot that struck `at`, if one was fired this tick.
    fn rail_hit(&mut self, at: Vec3, time: f32) {
        let mut best: Option<(usize, f32)> = None;
        for (i, shot) in self.pending_rail.iter().enumerate() {
            if let Some(score) = beam_score(shot.muzzle, shot.dir, shot.range, at) {
                if score < best.map_or(56.0, |b| b.1) {
                    best = Some((i, score));
                }
            }
        }
        if let Some((i, _)) = best {
            let shot = self.pending_rail.swap_remove(i);
            self.rail_beam(shot.muzzle, at, shot.width, shot.hot, time);
        }
    }

    /// Hitscan that struck nothing this tick: the beam still runs out to range.
    fn flush_rail_misses(&mut self, time: f32) {
        for shot in std::mem::take(&mut self.pending_rail) {
            self.rail_beam(
                shot.muzzle,
                shot.muzzle + shot.dir * shot.range,
                shot.width,
                shot.hot,
                time,
            );
        }
    }

    /// The flashes, smoke and debris of one sim event.
    fn effects_of(&mut self, event: &SimEvent, time: f32) {
        let (origin, blueprint) = match event {
            SimEvent::ShotFired { pos, blueprint, .. }
            | SimEvent::MissileIgnited { pos, blueprint, .. }
            | SimEvent::Impact { pos, blueprint, .. }
            | SimEvent::UnitDied { pos, blueprint, .. }
            | SimEvent::AircraftCrashed { pos, blueprint, .. }
            | SimEvent::Reclaimed { pos, blueprint, .. } => {
                (Some(Vec3::from(pos.to_f32())), Some(*blueprint))
            }
            SimEvent::ShieldBroken { pos, .. } => (Some(Vec3::from(pos.to_f32())), None),
            SimEvent::MissileLased { to, .. } => (Some(Vec3::from(to.to_f32())), None),
            _ => (None, None),
        };
        let previous = (self.effect_origin, self.effect_settings);
        self.effect_origin = origin;
        self.effect_settings = blueprint
            .and_then(|id| self.blueprints.units.get(id.0 as usize))
            .map_or_else(mc_data::EffectSettings::default, |bp| bp.visual.effects);
        if let SimEvent::ShotFired {
            blueprint, weapon, ..
        }
        | SimEvent::Impact {
            blueprint, weapon, ..
        } = event
        {
            if self.blueprints.unit(*blueprint).weapons[*weapon as usize]
                .bore
                .is_some()
            {
                self.effect_settings.shockwave_color = Some([0.24, 0.62, 1.0]);
            }
        }
        if !self.heavy_rail_event(event, time)
            && !self.great_gun_event(event, time)
            && !self.bolt_rifle_event(event, time)
            && !self.arc_howitzer_event(event, time)
        {
            self.effects_of_inner(event, time);
        }
        (self.effect_origin, self.effect_settings) = previous;
    }

    /// A weapon charging before it fires (`Weapon::charge_ticks`): a knot of its colour
    /// growing at each muzzle over the charge, lighting what is round it, with sparks
    /// drawn in along the barrel toward the mouth. The sim names the hull and weapon
    /// only; where the gun is and how its house is turned come from the last tick's
    /// mirror (`note_gun_hulls`), or failing that the muzzle in the hull's frame.
    fn weapon_charging(
        &mut self,
        at: Vec3,
        blueprint: mc_data::BlueprintId,
        weapon: u8,
        time: f32,
    ) {
        let blueprints = self.blueprints.clone();
        let w = &blueprints.unit(blueprint).weapons[weapon as usize];
        let seconds = w.charge_ticks as f32 * self.tick_seconds.max(0.02);
        if seconds < 0.15 {
            return;
        }
        let tint = if w.color == mc_data::WeaponColor::Blue {
            0.0
        } else {
            1.0
        };
        // The hull: the nearest of that blueprint to where the sim says it is.
        let hull = self
            .water_fx
            .guns
            .iter()
            .filter(|g| g.blueprint == blueprint.0 as u32)
            .min_by(|a, b| {
                let (da, db) = (
                    a.pos.truncate().distance_squared(at.truncate()),
                    b.pos.truncate().distance_squared(at.truncate()),
                );
                da.total_cmp(&db)
            });
        let heading = hull.map_or(0.0, |g| g.heading);
        let base = hull.map_or(at - Vec3::Z * w.muzzle.z.to_f32(), |g| g.pos);
        // Only the first `MAX_HOUSES` weapons turn on houses of their own.
        let pose = hull
            .and_then(|g| g.house)
            .filter(|_| (weapon as usize) < mc_data::MAX_HOUSES)
            .map(|h| h.pose[weapon as usize]);
        // A turret gun's aim is the unit's own: its turret's yaw, and the first gun's
        // pitch about its trunnion (a mortar laid up at a steep lob).
        let turret = hull
            .filter(|_| pose.is_none() && weapon == 0 && w.turret_turn > 0 && w.pivot.is_some())
            .map(|g| g.turret);
        let (yaw, pitch) = pose
            .map(|p| (p[1], p[3]))
            .or(turret.map(|t| (t[0], t[1])))
            .unwrap_or((0.0, 0.0));
        let aimed = pose.is_some() || turret.is_some();
        let pivot = w.pivot.map_or(Vec3::ZERO, |p| Vec3::from(p.to_f32()));
        let rot_z = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
        };
        let rot_xz = |v: Vec3, a: f32| {
            let (s, c) = a.sin_cos();
            Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
        };
        // A house turns about its pivot by its yaw off the hull; what is in it pitches there too.
        let placed = |local: Vec3| {
            let on_hull = if pose.is_some() {
                pivot + rot_z(rot_xz(local - pivot, pitch), yaw)
            } else if turret.is_some() {
                // The turret turns about the unit's middle, the gun about its trunnion.
                rot_z(pivot + rot_xz(local - pivot, pitch), yaw)
            } else {
                local
            };
            base + rot_z(on_hull, heading)
        };
        // Down the bore: a turret gun's rests laid from its trunnion up to its muzzle.
        let reach = Vec3::from(w.muzzle.to_f32()) - pivot;
        let bore = if turret.is_some() {
            Vec3::new(reach.x, 0.0, reach.z).normalize_or(Vec3::X)
        } else {
            Vec3::X
        };
        let dir = rot_z(
            if aimed {
                rot_z(rot_xz(bore, pitch), yaw)
            } else {
                Vec3::X
            },
            heading,
        );
        let power = w.damage.to_f32().max(1.0).sqrt();
        let full = (0.6 + power * 0.05).min(3.5) * w.flash.max(0.5);
        let steps = ((seconds / 0.16).ceil() as usize).clamp(3, 24);
        let mouths: Vec<Vec3> = if w.muzzles.is_empty() {
            vec![Vec3::from(w.muzzle.to_f32())]
        } else {
            w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect()
        };
        for local in mouths {
            // An aft house's muzzles are given as if it faced forward, mirrored (as the sim does).
            let local = if w.rear {
                Vec3::new(-local.x, -local.y, local.z)
            } else {
                local
            };
            let mouth = placed(local);
            for k in 0..steps {
                let f = k as f32 / (steps - 1).max(1) as f32;
                let start = time + f * (seconds - 0.1);
                // Slow to build, then quick: most of the glow comes in the last second.
                let r = full * (0.15 + 0.85 * f * f);
                self.push_effect(
                    (mouth + dir * 0.3).to_array(),
                    start,
                    r,
                    0.2 + seconds / steps as f32,
                    tint,
                    0.0,
                );
                for _ in 0..1 + (f * 2.0) as usize {
                    let back = 1.5 + self.scatter.unit() * (4.0 + power * 0.08);
                    let off = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * (0.3 + f * 0.5);
                    let life = 0.18 + self.scatter.unit() * 0.12;
                    let from = mouth - dir * back + off;
                    let when = start + self.scatter.unit() * 0.1;
                    self.push_puff(
                        PUFF_BOLT,
                        from,
                        dir * (back / life) * 0.8,
                        when,
                        life,
                        (0.14 + f * 0.12, 0.05),
                    );
                }
            }
        }
    }

    fn effects_of_inner(&mut self, event: &SimEvent, time: f32) {
        if self.sea_effects_of(event, time) {
            return;
        }
        self.giant_event(event, time);
        self.titan_charge
            .note(event, self.tick_seconds, &self.blueprints);
        match event {
            SimEvent::BoreDischarge {
                from,
                to,
                width,
                after,
                blueprint,
                weapon,
                ..
            } => {
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let (splash, cool) = (w.splash.to_f32(), w.bore.map_or(10.0, |b| b.cool));
                let storm = w.bore.and_then(|b| b.storm);
                let blast = w.bore.filter(|b| b.blast > 0.0);
                self.bore_discharge(
                    Vec3::from(from.to_f32()),
                    Vec3::from(to.to_f32()),
                    width.to_f32(),
                    splash,
                    cool,
                    after.to_f32(),
                    time,
                );
                // A bore big enough to gut a base lands like one (`titan_fx`); one with a
                // `blast` of its own raises its fireball (`bore_fx`).
                let landed = time + after.to_f32() * self.tick_seconds;
                if let Some(b) = blast {
                    self.bore_blast(Vec3::from(to.to_f32()), b.blast, b.blast_time, landed);
                }
                if splash >= titan_fx::CATACLYSM_SPLASH {
                    self.cataclysm(Vec3::from(to.to_f32()), splash, landed);
                }
                if let Some(storm) = storm {
                    let seconds = storm.ticks as f32 * 0.1;
                    self.discharge_storm(
                        Vec3::from(to.to_f32()),
                        storm.radius.to_f32(),
                        seconds,
                        landed,
                    );
                    self.feed_storm(
                        Vec3::from(from.to_f32()),
                        Vec3::from(to.to_f32()),
                        blueprint.0 as u32,
                        *weapon,
                        landed,
                        landed + seconds,
                        storm.radius.to_f32(),
                    );
                }
            }
            SimEvent::NuclearDetonation { .. }
            | SimEvent::NuclearLaunch { .. }
            | SimEvent::InterceptorLaunch { .. }
            | SimEvent::WarheadIntercepted { .. } => self.nuke_event(event, time),
            SimEvent::ShellDischarge {
                from,
                to,
                after,
                blueprint,
                weapon,
            } => {
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let (splash, melt) = (w.splash.to_f32(), w.melt);
                let to = Vec3::from(to.to_f32());
                self.shell_discharge(Vec3::from(from.to_f32()), to, splash, after.to_f32(), time);
                self.shell_melt(to, splash * melt, after.to_f32(), time);
            }
            SimEvent::WeaponCharging {
                unit,
                pos,
                owner,
                blueprint,
                weapon,
            } => {
                // A squeezed plasma gun gathers its charge in front of the bore
                // (`regency_guns_fx`), not as an ordinary gun's glow nor as a thrown
                // charge held at its unpitched muzzle (the Springald lobs, but its
                // charge rides its barrel).
                if !self.regency_charging(
                    unit.0,
                    *blueprint,
                    *weapon,
                    Vec3::from(pos.to_f32()),
                    time,
                ) {
                    self.plasma_charging(
                        unit.0,
                        *owner,
                        *blueprint,
                        *weapon,
                        Vec3::from(pos.to_f32()),
                        time,
                    );
                    self.weapon_charging(Vec3::from(pos.to_f32()), *blueprint, *weapon, time);
                }
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let seconds = w.charge_ticks as f32 * self.tick_seconds.max(0.02);
                if w.bore.is_some() && seconds >= titan_fx::GIANT_CHARGE {
                    let (at, id) = (Vec3::from(pos.to_f32()), blueprint.0 as u32);
                    self.bore_charge(at, id, seconds, time);
                }
            }
            SimEvent::MissileLased {
                from,
                to,
                killed,
                blueprint,
            } => {
                // A faction whose look is the gravity crush draws that (`crush_fx`), not a laser.
                if !self.missile_crushed(*blueprint, from, to, *killed, time) {
                    self.missile_lased(from, to, *killed, time);
                }
            }
            SimEvent::ClusterSplit {
                pos, vel, count, ..
            } => {
                let vel = Vec3::from(vel.to_f32()) / self.tick_seconds.max(0.02);
                self.cluster_split(Vec3::from(pos.to_f32()), vel, *count, time);
            }
            SimEvent::MissileIgnited {
                pos,
                vel,
                blueprint,
                weapon,
            } => {
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let size =
                    (0.3 + w.damage.to_f32().sqrt() * 0.045 + w.splash.to_f32() * 0.07) * w.tracer;
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                let tail = Vec3::from(pos.to_f32()) - dir * missile_half_length(size, w.caliber);
                // The motor lights in the open: a white-hot blast, larger than a tube launch.
                self.push_shockwave(tail.to_array(), time, 84.0, 0.72, 1.0, 1.0, -dir);
                self.push_shockwave(tail.to_array(), time, 40.0, 0.34, 1.0, 1.0, -dir);
                self.push_effect(tail.to_array(), time, 18.0, 0.26, 2.0, 0.2);
                self.push_effect(tail.to_array(), time, 7.5, 0.14, 2.0, 0.95);
                self.push_puff(PUFF_FIREBALL, tail, -dir * 9.0, time, 0.48, (3.4, 6.2));
                self.push_puff(PUFF_FIRE, tail, -dir * 30.0, time, 0.3, (1.6, 2.8));
                for i in 0..10 {
                    let spray = -dir * (14.0 + 6.0 * i as f32)
                        + Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.signed(),
                        ) * 6.0;
                    let kind = if i % 3 == 0 {
                        PUFF_FIREBALL
                    } else if i % 3 == 1 {
                        PUFF_FIRE
                    } else {
                        PUFF_SMOKE
                    };
                    self.push_puff(
                        kind,
                        tail,
                        spray,
                        time + 0.01 * i as f32,
                        0.55 + 0.12 * i as f32,
                        (1.1, 2.4 + 0.28 * i as f32),
                    );
                }
                for _ in 0..14 {
                    let spray = (-dir
                        + Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.signed(),
                        ) * 0.7)
                        .normalize_or_zero();
                    let speed = 48.0 + self.scatter.unit() * 60.0;
                    self.push_puff(PUFF_SPARK, tail, spray * speed, time, 0.45, (0.28, 0.1));
                }
            }
            SimEvent::ShotFired {
                pos,
                vel,
                travel,
                color,
                owner,
                blueprint,
                weapon,
            } => {
                if self.blueprints.unit(*blueprint).weapons[*weapon as usize].beam {
                    // A held beam: one steady stream, not a shot a tick (`plasma_fx`).
                    self.beam_fired(
                        *owner,
                        *blueprint,
                        *weapon,
                        Vec3::from(pos.to_f32()),
                        Vec3::from(travel.to_f32()),
                        Vec3::from(vel.to_f32()).normalize_or_zero(),
                        time,
                    );
                    return;
                }
                if self.blueprints.unit(*blueprint).weapons[*weapon as usize]
                    .cone
                    .is_some()
                {
                    // A cone weapon's wake rolls out over the ground (`wake_fx`).
                    let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                    let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::X);
                    self.wake_fired(*blueprint, *weapon, at, dir, time);
                    return;
                }
                // A Regency plasma gun's own firing: a thrown charge leaving the claw
                // (`plasma_fx`); a direct-fire plasma gun's is all its own (`regency_guns_fx`).
                {
                    let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                    let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                    self.plasma_thrown(*owner, *blueprint, *weapon, at, time);
                    let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                    let gap = mc_sim::mirror::round_gap(w) * self.tick_seconds;
                    if self.regency_fired(*blueprint, *weapon, at, dir, gap, time) {
                        return;
                    }
                    // A Gravitic Seeker leaves its cell with no flame or smoke (`gravitic_fx`).
                    if self.seeker_fired(*blueprint, *weapon, at, dir, time) {
                        return;
                    }
                }
                let unit = self.blueprints.unit(*blueprint);
                let weapon = &unit.weapons[*weapon as usize];
                if unit
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
                    && weapon.trajectory == mc_data::Trajectory::Ballistic
                    && !weapon.missile
                {
                    // Gravity drops have no muzzle flash, propellant smoke, or sparks.
                    return;
                }
                if weapon.boost_ticks > 0 {
                    // The booster lights in the cell and drives the missile up out of its
                    // mouth, a body length above where it stood (`launch_fx.rs`).
                    let mouth = Vec3::from(pos.to_f32()) + Vec3::Z * (weapon.caliber / 0.28);
                    let power = weapon.damage.to_f32().max(1.0).sqrt() * 0.5;
                    self.cell_launch(mouth, Vec3::Z, power, time);
                    return;
                }
                if weapon.cold_launch_ticks > 0 {
                    let at = Vec3::from(pos.to_f32());
                    // Pneumatic ejection: pressure wave only, no rocket flash.
                    self.push_shockwave(at.to_array(), time, 18.0, 0.35, 0.55, 0.0, Vec3::Z);
                    return;
                }
                if weapon.missile && weapon.vertical_launch {
                    // The motor lights in the cell (`launch_fx.rs`).
                    let at = Vec3::from(pos.to_f32()) - Vec3::from(travel.to_f32());
                    let dir = Vec3::from(vel.to_f32()).normalize_or(Vec3::Z);
                    self.cell_launch(at, dir, weapon.damage.to_f32().max(1.0).sqrt(), time);
                    return;
                }
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let flash = weapon.flash;
                let bore = weapon.bore.is_some();
                let shockwave = weapon.shockwave;
                // A thrown charge leaves the claw with a snap of its own (`plasma_thrown`), not a
                // gun's pressure wave; its `shockwave` is for where it lands.
                let thrown = weapon.curve.0 > 0;
                let missile = weapon.missile;
                let caliber = weapon.caliber;
                let bolts = weapon.bolts;
                let rounds = weapon.rounds;
                let round_gap = mc_sim::mirror::round_gap(weapon) * self.tick_seconds;
                let casings = weapon.casings;
                // A gun off to the left of the hull throws its casings out to its left.
                let outboard = if weapon.muzzle.y.to_f32() > 0.5 {
                    -1.0
                } else {
                    1.0
                };
                // An aircraft's casings leave at its speed and fall away behind it.
                let flying = unit
                    .motion
                    .filter(|m| m.layer == mc_data::MoveLayer::Air)
                    .map(|m| m.speed.to_f32());
                // The gun as it is drawn: a unit glides up to its place over the tick
                // after, so it is a tick's travel short of the sim's muzzle, and each
                // later round leaves from as far on as it has got by then.
                let travel = Vec3::from(travel.to_f32());
                let gap_ticks = mc_sim::mirror::round_gap(weapon);
                let round_at = |at: Vec3, k: u8| {
                    at + Vec3::from(mc_sim::mirror::round_shift(travel.to_array(), k, gap_ticks))
                };
                let at = Vec3::from(pos.to_f32()) - travel;
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                let shell = *color == mc_data::WeaponColor::Orange;
                // A gun whose tracers run red (`Weapon::red`) flashes red too: effect kind 8.
                let tint = if shell && weapon.red > 0.5 {
                    8.0
                } else {
                    *color as u32 as f32
                };
                // An ARC rail gun flashes white, not powder orange.
                let rail = (weapon.rail || weapon.hitscan) && shell;
                let flak = weapon.flak;
                let tint = if rail { rail_fx::RAIL_FLASH } else { tint };
                // An electric bore's shot is its discharge (`bore_fx`), not a rail's beam.
                if weapon.hitscan && weapon.bore.is_none() {
                    self.pending_rail.push(PendingRail {
                        muzzle: at,
                        dir,
                        range: weapon.range_max.to_f32(),
                        width: 0.45 + power * 0.022 * flash,
                        hot: shell,
                    });
                }
                if !rail {
                    // A rail gun's flash is its own (`rail_muzzle`).
                    self.push_effect(
                        at.to_array(),
                        time,
                        (1.2 + power * 0.42) * flash,
                        if shell { 0.09 } else { 0.12 },
                        tint,
                        0.0,
                    );
                }
                // The rounds a stream gun's shot is drawn as (`Weapon::rounds`) each
                // leave with a flash of their own, as they leave in the mirror.
                for k in 1..rounds {
                    let size = 0.85 + 0.15 * self.scatter.unit();
                    self.push_effect(
                        round_at(at + travel, k).to_array(),
                        time + k as f32 * round_gap,
                        (1.2 + power * 0.42) * flash * size,
                        if shell { 0.07 } else { 0.1 },
                        tint,
                        0.0,
                    );
                }
                if casings > 0.0 {
                    let breech = casing_fx::Breech {
                        at: at + travel - dir * casings,
                        dir,
                        outboard,
                        travel,
                        gap_ticks,
                        flying,
                        size: 0.3 + power * 0.03,
                        reach: casings,
                    };
                    self.casings_thrown(&breech, rounds, round_gap, time);
                }
                if !shell {
                    // A hotter knot at the bore, with a small ring, so an energy

                    // projector reads as a discharge rather than a gun flash.
                    self.push_effect(
                        (at + dir * 1.1).to_array(),
                        time,
                        (0.55 + power * 0.16) * flash,
                        0.08,
                        *color as u32 as f32,
                        0.4,
                    );
                }
                if bolts > 0 {
                    self.push_effect(
                        (at + dir * 1.6).to_array(),
                        time,
                        (0.7 + power * 0.16) * flash,
                        0.14,
                        *color as u32 as f32,
                        0.9,
                    );
                    for _ in 0..bolts {
                        let spray = (dir * 1.6
                            + Vec3::new(
                                self.scatter.signed(),
                                self.scatter.signed(),
                                self.scatter.signed(),
                            ) * 0.35)
                            .normalize_or_zero();
                        let (speed, life) = (
                            22.0 + self.scatter.unit() * 18.0,
                            0.12 + self.scatter.unit() * 0.1,
                        );
                        self.push_puff(
                            PUFF_BOLT,
                            at + dir * 0.4,
                            spray * speed,
                            time,
                            life,
                            (0.22, 0.05),
                        );
                    }
                }
                if shockwave > 0.0 && !thrown {
                    // Much bigger than the gun: a howitzer's wave dwarfs the bunker it sits on.
                    let life = if bore {
                        0.8
                    } else {
                        (0.48 + power * 0.01).min(0.95)
                    };
                    self.push_shockwave(
                        (at + dir * 2.0).to_array(),
                        time,
                        // The bore launches a small tracer: a distinct pressure front,
                        // sized independently of the much more powerful discharge.
                        if bore {
                            (12.0 + power * 0.35) * shockwave
                        } else {
                            (22.0 + power * 2.0) * shockwave
                        },
                        life,
                        shockwave.min(1.0),
                        *color as u32 as f32,
                        dir,
                    );
                }
                if missile {
                    // The motor lights in the tube (`launch_fx.rs`).
                    self.tube_launch(at, dir, power, caliber, time);
                    return;
                }
                if !shell {
                    return;
                }
                if rail {
                    self.rail_muzzle(at, dir, (1.2 + power * 0.42) * flash, time);
                }
                if flak {
                    // Flak's own report: a smoke ring punched out ahead (`flak_fx`).
                    self.flak_muzzle(at, dir, power, time);
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
                target_motion,
                splash,
                color,
                after,
                on_unit,
                on_shield,
                on_structure: _,
                blueprint,
                weapon,
            } => {
                let (beam, plasma) = {
                    let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                    (w.beam, w.plasma_shot().is_some())
                };
                // A Gravitic Seeker's strike is all its own (`gravitic_fx`); on a shield it is
                // the shield's hit, as any shot's (below).
                if !*on_shield {
                    let start = time + after.to_f32() * self.tick_seconds;
                    let at = Vec3::from(pos.to_f32());
                    if self.seeker_struck(*blueprint, *weapon, at, *splash, *on_unit, start) {
                        return;
                    }
                }
                if beam {
                    // A held beam's strike: it glasses the ground, no shell's blast (`plasma_fx`).
                    let at = Vec3::from(pos.to_f32());
                    self.beam_struck(*blueprint, *weapon, at, *on_unit || *on_shield, time);
                    return;
                }
                if plasma {
                    let start = time + after.to_f32() * self.tick_seconds;
                    let at = Vec3::from(pos.to_f32());
                    self.plasma_landed(*blueprint, *weapon, at, *on_unit || *on_shield, start);
                    // A direct-fire plasma gun's strike is all its own: no shell's blast.
                    // On a shield it is the shield's hit, as any shot's (below).
                    if !*on_shield && self.regency_landed(*blueprint, *weapon, at, *on_unit, start)
                    {
                        return;
                    }
                }
                let weapon = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                // A blue hitscan gun (the commander's rail cannon) lands with the heavy blue
                // bloom. Projectile rail guns fire hot slugs and land like shells.
                // An electric bore lands as its discharge (`bore_fx`) draws it.
                let bore = weapon.bore.is_some();
                let rail = weapon.hitscan && !bore && *color == mc_data::WeaponColor::Blue;
                // A fire bomb's napalm wave. An incendiary gun's rounds still burst like
                // shells: the fire they leave is the patch (`fires`), not the hit.
                let incendiary = weapon.burn_ticks > 0 && weapon.rounds <= 1;
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let impact = weapon.impact;
                let shockwave = weapon.shockwave;
                let bolts = weapon.bolts;
                let red = weapon.red;
                let slug = weapon.rail;
                let flak = weapon.flak;
                let ion_blast = weapon.ion_blast;
                // A hitscan shot is there the moment it is fired: no flight to wait out. It lands
                // at the start of the tick, where the target is drawn then, not where it ends up.
                // A bore's strike is timed and placed with its discharge (`BoreDischarge`).
                let hitscan = weapon.hitscan && !bore;
                let at = Vec3::from(pos.to_f32())
                    - if hitscan {
                        Vec3::from(target_motion.to_f32())
                    } else {
                        Vec3::ZERO
                    };
                let start = if hitscan {
                    time
                } else {
                    time + after.to_f32() * self.tick_seconds
                };
                if hitscan {
                    self.rail_hit(at, time);
                }
                if *on_shield {
                    // The glass itself is the hit: a tight bloom at the strike, then
                    // the hex plates crackle. No fragments thrown off a live dome.
                    let heavy = if bolts > 0 { 1.35 } else { 1.0 };
                    let strength = ((0.45 + power * 0.22) * heavy).min(5.5);
                    let cool = ((strength - 0.55) / 4.0).clamp(0.0, 1.0);
                    self.push_shield_hit(at.to_array(), start, strength);
                    self.push_effect(
                        at.to_array(),
                        start,
                        (0.42 + power * 0.08) * impact.max(0.5),
                        0.11,
                        0.0,
                        0.62 + cool * 0.28,
                    );
                    return;
                }
                let splash = splash.to_f32();
                if ion_blast > 0.0 {
                    // An AEB warhead goes off as the electric bore's blast (`bore_fx`), burning
                    // about a second for every 7.5 m of it, as the Fulgur's does.
                    self.bore_blast(at, ion_blast, ion_blast / 7.5, start);
                    return;
                }
                if flak {
                    let burst = flak_fx::FlakBurst {
                        at,
                        motion: Vec3::from(target_motion.to_f32()) / self.tick_seconds.max(0.001),
                        splash,
                        impact,
                        direct: *on_unit,
                    };
                    self.flak_burst(&burst, start);
                    return;
                }
                self.shell_impact(&impact_fx::ShellImpact {
                    at,
                    start,
                    splash,
                    impact,
                    power,
                    shockwave,
                    bolts,
                    color: *color,
                    red,
                    white_hot: hitscan || slug,
                    rail,
                    incendiary,
                    on_unit: *on_unit,
                });
            }
            SimEvent::ShieldBroken { pos, radius, .. } => {
                let at = Vec3::from(pos.to_f32());
                let r = radius.to_f32();
                // Domes are flattened: z runs to `dome_height`, not the radius.
                let flat = Vec3::new(1.0, 1.0, mc_data::dome_height_f32(r) / r.max(0.001));
                let apex = at + Vec3::Z * r * flat.z * 0.92;
                // The membrane peel is the read. These are the pop: a pole flash,
                // hex ripples across the remaining glass, plates shedding from the
                // receding lip, and a pressure wave on the ground — not a disc
                // that milks the view, and not a spray of fragments into the dirt.
                self.push_effect(
                    apex.to_array(),
                    time,
                    (18.0 + r * 0.12).min(42.0),
                    0.32,
                    0.0,
                    0.95,
                );
                self.push_effect(
                    at.to_array(),
                    time,
                    (10.0 + r * 0.06).min(22.0),
                    0.22,
                    0.0,
                    0.7,
                );
                self.push_shockwave(at.to_array(), time, r * 1.45, 0.95, 1.0, 0.0, Vec3::ZERO);
                self.push_shield_hit(apex.to_array(), time, 5.4);
                let ripples = 5;
                for i in 0..ripples {
                    let a = (i as f32 + 0.5) * std::f32::consts::TAU / ripples as f32;
                    let hit = at + Vec3::new(a.cos(), a.sin(), 0.55) * flat * r * 0.72;
                    self.push_shield_hit(hit.to_array(), time, 3.6);
                }
                let n = (16.0 + r * 0.1).min(32.0) as u32;
                for i in 0..n {
                    let peel = (i as f32 + 0.5) / n as f32;
                    let inc = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
                    // Lip travels rim → pole with the peel. Stay off the dirt so
                    // a plate is not born underground and culled on its first frame.
                    let polar = 0.92 - peel * 0.70;
                    let z = (1.0 - polar * polar).max(0.0).sqrt();
                    let phi = i as f32 * inc;
                    let nrm = Vec3::new(phi.cos() * polar, phi.sin() * polar, z);
                    let mut spawn = at + nrm * flat * r;
                    spawn.z = spawn.z.max(at.z + 1.6);
                    let vel = nrm * (3.2 + self.scatter.unit() * 4.0)
                        + Vec3::Z * (-1.8 - self.scatter.unit() * 2.2);
                    let puff_start = time + peel * 0.38 + self.scatter.unit() * 0.05;
                    let life = 0.85 + self.scatter.unit() * 0.45;
                    let plate = 2.4 + r * 0.016 + self.scatter.unit() * 1.6;
                    self.push_puff(
                        PUFF_SHARD,
                        spawn,
                        vel,
                        puff_start,
                        life,
                        (plate, plate * 0.55),
                    );
                }
                let bolts = (6.0 + r * 0.04).min(12.0) as u32;
                for _ in 0..bolts {
                    let a = self.scatter.unit() * std::f32::consts::TAU;
                    let polar = 0.35 + self.scatter.unit() * 0.4;
                    let z = (1.0 - polar * polar).max(0.0).sqrt();
                    let nrm = Vec3::new(a.cos() * polar, a.sin() * polar, z);
                    let spawn = at + nrm * r * 0.92;
                    let vel = nrm * (6.0 + self.scatter.unit() * 8.0);
                    let life = 0.16 + self.scatter.unit() * 0.12;
                    self.push_puff(PUFF_BOLT, spawn, vel, time, life, (0.32 + r * 0.002, 0.05));
                }
            }
            SimEvent::UnitDied {
                pos,
                blueprint,
                airborne: true,
                ..
            } => {
                let bp = self.blueprints.unit(*blueprint);
                let at = Vec3::from(pos.to_f32());
                if bp.is_capital_ship() {
                    self.capital_air_death(at, blueprint.0 as u32, time);
                } else {
                    self.air_blast(at, bp.radius.to_f32(), time);
                }
            }
            SimEvent::AircraftCrashed { pos, blueprint }
                if self.blueprints.unit(*blueprint).is_capital_ship() =>
            {
                self.capital_crash(Vec3::from(pos.to_f32()), blueprint.0 as u32, time);
            }
            SimEvent::UnitDied { pos, blueprint, .. }
            | SimEvent::AircraftCrashed { pos, blueprint } => {
                // A unit going up is an event, not a big impact (blast_fx.rs).
                let at = Vec3::from(pos.to_f32());
                let complete = !matches!(
                    event,
                    SimEvent::UnitDied {
                        complete: false,
                        ..
                    }
                );
                let bp = self.blueprints.unit(*blueprint);
                let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
                let blast = bp.death_blast.filter(|_| complete);
                if bp.has(mc_data::cat::COMMANDER) {
                    // A commander goes up as a small nuclear blast: the sim's
                    // `NuclearDetonation` draws it (nuke_fx.rs).
                    return;
                }
                // A Regency power generator's burning star breaks free and goes supernova:
                // that is how its blast looks.
                if self.star_nova(at, time) {
                    return;
                }
                // Only a finished plant goes up, as the sim's blast does.
                if let Some(db) = blast {
                    // A volatile plant: the same detonation, sized to its blast. The
                    // fireball's puffs keep the commander's proportion to the blast,
                    // not the building's footprint.
                    let blast = db.radius.to_f32();
                    self.reactor_death(*blueprint, at, h, blast, time);
                    return;
                }
                self.unit_blast(at, r, h, time);
            }
            SimEvent::Reclaimed {
                pos,
                blueprint,
                wreck,
            } => self.reclaimed_flare(Vec3::from(pos.to_f32()), *blueprint, *wreck, time),
            _ => {}
        }
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
            .filter(|f| f.len() == self.fog.grid_len());
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
            while !bytes.len().is_multiple_of(4) {
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
            while !bytes.len().is_multiple_of(4) {
                bytes.push(0);
            }
            copies.push((bytes.len() as u64, self.fog.grid(), 0, None));
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
        if let Some(scopes) = self.timers.read(&self.gpu.device) {
            self.stats.gpu_passes.clear();
            for s in scopes.iter().filter(|s| s.depth == 0) {
                self.stats.gpu_passes.push((s.name, s.ms));
            }
            self.stats.gpu_scopes = scopes;
        }
    }

    /// Counts triangles and shader invocations per GPU scope from the next
    /// frame on (`MERIDIAN_GPU_STATS=1` starts with them on).
    pub fn set_gpu_stats(&mut self, on: bool) {
        self.timers.set_stats(on);
    }

    /// Copies the next frame the window shows, HUD and all; `take_capture`
    /// hands it over a frame or two later. Headless targets use `read_pixels`.
    pub fn capture_next_frame(&mut self) {
        self.capture.request();
    }

    /// The copy asked for, once it is ready; an error when the window cannot be copied.
    pub fn take_capture(&mut self) -> Option<Result<Shot, &'static str>> {
        if self.capture.refused() {
            return Some(Err("this window's surface cannot be copied"));
        }
        self.capture.take().map(Ok)
    }

    /// Headless only: the last rendered frame as tightly packed RGBA8.
    pub fn read_pixels(&mut self) -> Option<Vec<u8>> {
        // SAFETY: the fence is this device's and was created signalled or submitted, so the
        // wait ends.
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
        // SAFETY: the device went idle on the first line of `drop`, so nothing on the GPU uses
        // these objects; each is this device's and destroyed once, here (framebuffers already
        // nulled or drained by `create_size_dependent` are null, which is a no-op), children
        // before the surface.
        unsafe {
            for fb in self.present_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            device.destroy_framebuffer(self.scene_fb, None);
            device.destroy_framebuffer(self.refract_fb, None);
            device.destroy_framebuffer(self.hull_depth_fb, None);
            device.destroy_framebuffer(self.prepass_fb, None);
            for fb in self.bloom_fbs.drain(..).chain(self.glass_fbs.drain(..)) {
                device.destroy_framebuffer(fb, None);
            }
            self.timers.destroy(&self.gpu);
            self.capture.destroy(&self.gpu);
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
        self.sky.destroy(&self.gpu);
        self.nuke_volume.destroy(&self.gpu);
        self.wake_shells.destroy(&self.gpu);
        self.post.destroy(&self.gpu);
        self.gtao.destroy(&self.gpu);
        self.terrain_lit.destroy(&self.gpu);
        self.fog.destroy(&self.gpu);
        self.grass.destroy(&self.gpu);
        self.foundations.destroy(&self.gpu);
        self.adjacency_links.destroy(&self.gpu);
        self.shafts.destroy(&self.gpu);
        self.craters.destroy(&self.gpu);
        self.ground_melt.destroy(&self.gpu);
        self.city_fx.destroy(&self.gpu);
        self.heat_haze.destroy(&self.gpu);
        self.lens_flares.destroy(&self.gpu);
        self.cull.destroy(&self.gpu);
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
            &mut self.props_dead,
            &mut self.nodes,
            &mut self.marks,
            &mut self.ranges,
            &mut self.range_ib,
            &mut self.projectiles,
            &mut self.effects,
            &mut self.shockwaves,
            &mut self.stains,
            &mut self.puffs,
            &mut self.water_fx.buffer,
            &mut self.beams,
            &mut self.welds,
            &mut self.effect_barriers,
            &mut self.light_list,
            &mut self.light_grid,
            &mut self.light_stage,
            &mut self.shields,
            &mut self.shield_hits,
            &mut self.track_marks,
            &mut self.houses,
            &mut self.overlay_vb,
            &mut self.mesh_vb,
            &mut self.mesh_ib,
            &mut self.quad_vb,
            &mut self.quad_ib,
            &mut self.grid_vb,
            &mut self.grid_ib,
            &mut self.patch_vb,
            &mut self.patch_ib,
            &mut self.vein_vb,
        ] {
            gpu.destroy_buffer(std::mem::replace(b, placeholder()));
        }
        for b in self.garbage.drain(..) {
            gpu.destroy_buffer(b);
        }
        for image in [
            &self.hdr,
            &self.depth,
            &self.refract,
            &self.hull_depth,
            &self.overview,
            &self.tiles,
            &self.tile_index,
            &self.noise,
            &self.terrain_materials,
            &self.ground_cover,
            &self.pad_footprints,
            &self.hull_plans,
            &self.font,
        ]
        .into_iter()
        .chain(&self.bloom)
        .chain(&self.glass)
        {
            gpu.destroy_image_ref(image);
        }
        self.shadow.destroy(gpu);
        if let Output::Headless { image, readback } = &mut self.output {
            gpu.destroy_image_ref(image);
            gpu.destroy_buffer(std::mem::replace(readback, placeholder()));
        }
        // `self.gpu` drops last and destroys the device and instance.
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    use glam::Vec2;

    fn save_environment_frame(renderer: &mut Renderer, path: &std::path::Path) {
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = b"P6\n960 720\n255\n".to_vec();
        for pixel in pixels.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, ppm).unwrap();
    }

    /// Real Vulkan pipeline check, with a captured frame for visual inspection.
    #[test]
    #[ignore = "requires Vulkan and maps/crosswater.mcmap"]
    fn forest_fire_lifecycle_and_render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/crosswater.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let start = Vec2::from(map.start_positions()[0].to_f32());
        let index = map
            .props()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind.is_tree())
            .min_by(|(_, a), (_, b)| {
                Vec2::from(a.pos.to_f32())
                    .distance_squared(start)
                    .total_cmp(&Vec2::from(b.pos.to_f32()).distance_squared(start))
            })
            .unwrap()
            .0;
        let xy = map.props()[index].pos;
        let blueprint = blueprints.id_of("aster_t1_bomber").unwrap();
        let mut renderer = Renderer::new(
            Target::Headless {
                width: 960,
                height: 720,
            },
            SceneDesc {
                map: map.clone(),
                blueprints,
                pool: Arc::new(Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        let mut camera = Camera::new(
            Vec2::from(map.info().size_metres().to_f32()),
            Vec2::new(960.0, 720.0),
        );
        camera.focus = Vec3::new(
            xy.x.to_f32(),
            xy.y.to_f32(),
            renderer.ground_height(Vec2::from(xy.to_f32())) + 8.0,
        );
        camera.distance = 48.0;
        camera.tilt = 0.48;
        let mut frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        let overlay = Overlay::default();
        for _ in 0..24 {
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: 0.0,
                    alpha: 1.0,
                    sim: Some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                    icons: true,
                })
                .unwrap();
        }
        save_environment_frame(&mut renderer, &root.join("artifacts/terrain-v2/tree.ppm"));
        renderer.tree_fires(&frame, 0.0, &camera);
        frame.props_dead[index / 32] |= 1 << (index % 32);
        renderer.tree_fires(&frame, 0.1, &camera);
        assert!(
            renderer.burning_trees.is_empty(),
            "reclaim must not ignite trees"
        );
        frame.props_dead[index / 32] = 0;
        renderer.tree_fires(&frame, 0.2, &camera);
        frame.props_dead[index / 32] |= 1 << (index % 32);
        frame.events.push(SimEvent::Impact {
            pos: xy.extend(mc_core::Fx::from_int(camera.focus.z as i32 - 8)),
            target_motion: mc_core::FxVec3::ZERO,
            splash: mc_core::Fx::from_int(12),
            color: mc_data::WeaponColor::Orange,
            after: mc_core::Fx::ZERO,
            on_unit: false,
            on_shield: true,
            on_structure: None,
            blueprint,
            weapon: 0,
        });
        renderer.tree_fires(&frame, 0.3, &camera);
        assert!(
            renderer.burning_trees.is_empty(),
            "shield interception must not ignite trees"
        );
        renderer.previous_dead[index / 32] = 0;
        if let SimEvent::Impact { on_shield, .. } = &mut frame.events[0] {
            *on_shield = false;
        }
        renderer.tree_fires(&frame, 0.4, &camera);
        assert_eq!(renderer.burning_trees.len(), 1);
        frame.events.clear();
        let overlay = Overlay::default();
        for step in 0..=110 {
            let time = 0.5 + step as f32 * 0.1;
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time,
                    alpha: 1.0,
                    sim: Some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                    icons: true,
                })
                .unwrap();
        }
        assert_eq!(
            renderer.burning_trees.len(),
            1,
            "dead bits must not restart fire"
        );
        assert_eq!(renderer.dynamic_count, 1, "keep the charred tree visible");
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = b"P6\n960 720\n255\n".to_vec();
        for pixel in pixels.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::create_dir_all(root.join("artifacts/terrain-v2")).unwrap();
        std::fs::write(root.join("artifacts/terrain-v2/fire.ppm"), ppm).unwrap();
        renderer.tree_fires(&frame, 50.0, &camera);
        assert!(
            renderer.burning_trees.is_empty(),
            "burned tree must eventually expire"
        );
        for width in [0, 7] {
            renderer.previous_dead[index / 32] = 0;
            renderer.burning_trees.clear();
            let at = xy.extend(mc_core::Fx::from_int(camera.focus.z as i32));
            frame.events = vec![SimEvent::BoreDischarge {
                from: at
                    - mc_core::FxVec3::new(
                        mc_core::Fx::from_int(100),
                        mc_core::Fx::ZERO,
                        mc_core::Fx::ZERO,
                    ),
                to: at
                    + mc_core::FxVec3::new(
                        mc_core::Fx::from_int(100),
                        mc_core::Fx::ZERO,
                        mc_core::Fx::ZERO,
                    ),
                width: mc_core::Fx::from_int(width),
                after: mc_core::Fx::ZERO,
                owner: 0,
                blueprint,
                weapon: 0,
            }];
            renderer.tree_fires(&frame, 51.0, &camera);
            assert_eq!(
                renderer.burning_trees.len(),
                1,
                "bore width {width} must ignite the middle of its path"
            );
        }
        frame.events.clear();
        for (name, x, y, distance, tilt) in [
            ("rock", 3264.0, 2304.0, 330.0, 0.0),
            ("rock-close", 3264.0, 2304.0, 90.0, 0.25),
            ("grass", 12260.0, 12430.0, 35.0, 0.25),
        ] {
            camera.focus = Vec3::new(x, y, renderer.ground_height(Vec2::new(x, y)));
            camera.distance = distance;
            camera.tilt = tilt;
            for step in 0..24 {
                renderer
                    .render(&FrameInput {
                        camera: &camera,
                        time: 50.0 + step as f32 * 0.1,
                        alpha: 1.0,
                        sim: Some(&frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &overlay,
                        build_grid: false,
                        icons: true,
                    })
                    .unwrap();
            }
            save_environment_frame(
                &mut renderer,
                &root.join(format!("artifacts/terrain-v2/{name}.ppm")),
            );
        }
    }
}

#[cfg(test)]
mod shockwave_tests {

    /// Captures the real Vulkan effect at several ages and across a live dome.
    #[test]
    #[ignore = "requires Vulkan and maps/crosswater.mcmap"]
    fn shockwave_shield_color_render() {
        use super::*;
        use glam::Vec2;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/crosswater.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let mut renderer = Renderer::new(
            Target::Headless {
                width: 960,
                height: 720,
            },
            SceneDesc {
                map: map.clone(),
                blueprints,
                pool: Arc::new(Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        renderer.fog_enabled = false;
        let xy = Vec2::new(12360.0, 12380.0);
        let mut camera = Camera::new(
            Vec2::from(map.info().size_metres().to_f32()),
            Vec2::new(960.0, 720.0),
        );
        camera.focus = xy.extend(renderer.ground_height(xy) + 18.0);
        camera.distance = 235.0;
        camera.tilt = std::env::var("MC_EFFECT_TEST_TILT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.12);
        let mut frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        let overlay = Overlay::default();
        let draw = |renderer: &mut Renderer, frame: &RenderFrame, time| {
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time,
                    alpha: 1.0,
                    sim: Some(frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                    icons: true,
                })
                .unwrap();
        };
        for _ in 0..24 {
            draw(&mut renderer, &frame, 0.0);
        }
        let output = root.join(
            std::env::var("MC_EFFECT_TEST_OUTPUT")
                .unwrap_or_else(|_| "artifacts/shockwave-shields".into()),
        );
        std::fs::create_dir_all(&output).unwrap();
        let save = |renderer: &mut Renderer, name: &str| {
            let pixels = renderer.read_pixels().unwrap();
            let mut ppm = b"P6\n960 720\n255\n".to_vec();
            for pixel in pixels.as_chunks::<4>().0 {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(output.join(format!("{name}.ppm")), ppm).unwrap();
            pixels
        };
        let center = camera.focus + Vec3::new(-34.0, 0.0, -5.0);
        for (case, tint, shielded, dust_color, opacity, brightness) in [
            ("cyan", [0.16, 0.62, 1.0], false, None, 1.2, 1.0),
            ("amber", [1.0, 0.3, 0.07], false, None, 1.2, 1.0),
            ("shield", [0.16, 0.62, 1.0], true, None, 1.2, 1.0),
            (
                "dust-rust",
                [0.16, 0.62, 1.0],
                false,
                Some([0.7, 0.22, 0.08]),
                1.2,
                1.0,
            ),
            (
                "dust-faint",
                [0.16, 0.62, 1.0],
                false,
                Some([0.7, 0.22, 0.08]),
                0.3,
                1.0,
            ),
            (
                "dust-bright",
                [0.16, 0.62, 1.0],
                false,
                Some([0.7, 0.22, 0.08]),
                1.2,
                2.0,
            ),
        ] {
            renderer.scatter = Scatter(0x9E37_79B9);
            renderer
                .shockwaves
                .write(0, &vec![0; renderer.shockwaves.size as usize]);
            renderer
                .puffs
                .write(0, &vec![0; renderer.puffs.size as usize]);
            frame.shields.clear();
            if shielded {
                let at = camera.focus + Vec3::new(34.0, 0.0, -18.0);
                frame.shields.push(mc_sim::mirror::ShieldInstance {
                    pos: at.to_array(),
                    radius: 32.0,
                    prev_open: 1.0,
                    open: 1.0,
                    health: 1.0,
                    packed: 2 << 16,
                    unit_id: 1,
                    projector: 10.0,
                    height: 20.0,
                    prev_radius: 0.0,
                });
            }
            draw(&mut renderer, &frame, 10.0);
            renderer.effect_settings = mc_data::EffectSettings {
                dust_visibility: opacity,
                dust_lifetime: 1.5,
                shockwave_color: Some(tint),
                dust_color,
                dust_brightness: brightness,
                shield_projector: None,
            };
            renderer.push_shockwave(center.to_array(), 10.0, 112.0, 1.25, 1.0, 0.0, Vec3::ZERO);
            for (step, age) in [0.12, 0.28, 0.48, 0.72, 1.15, 2.2, 4.5]
                .into_iter()
                .enumerate()
            {
                draw(&mut renderer, &frame, 10.0 + age);
                save(&mut renderer, &format!("{case}-{step}"));
            }
            if std::env::var_os("MC_EFFECT_TEST_ANIMATION").is_some() {
                for step in 0..60 {
                    draw(&mut renderer, &frame, 10.0 + step as f32 / 20.0);
                    save(&mut renderer, &format!("{case}-anim-{step:02}"));
                }
            }
        }
        let cursor = renderer.puff_cursor;
        renderer.effect_settings.dust_visibility = 0.0;
        renderer.push_puff(PUFF_SHOCK_DUST, center, Vec3::Z, 20.0, 2.0, (5.0, 10.0));
        assert_eq!(
            renderer.puff_cursor, cursor,
            "zero visibility must disable emission"
        );
        renderer.effect_settings.dust_visibility = 1.0;
        renderer.effect_settings.dust_lifetime = 0.0;
        renderer.push_puff(PUFF_SMOKE, center, Vec3::Z, 20.0, 2.0, (5.0, 10.0));
        assert_eq!(
            renderer.puff_cursor, cursor,
            "zero lifetime must disable emission"
        );
    }

    use super::*;

    #[test]
    fn shockwave_dust_begins_when_the_expanding_front_reaches_ground() {
        let center = Vec3::new(100.0, 100.0, 15.0);
        let mut previous = 0.0;
        for offset in [5.0, 20.0, 40.0, 60.0] {
            let ground = Vec3::new(100.0 + offset, 100.0, 3.0);
            let (age, pressure) =
                shockwave_ground_arrival(center, ground, 80.0, Vec3::ZERO, 0.0).unwrap();
            let reached = 80.0 * (1.0 - (1.0 - age).powi(2));
            assert!((reached - center.distance(ground)).abs() < 0.0001);
            assert!(age > previous && age < 1.0);
            assert!(pressure > 0.0 && pressure <= 1.0);
            previous = age;
        }
    }

    #[test]
    fn shockwave_dust_requires_dry_ground_within_the_directional_front() {
        let center = Vec3::new(0.0, 0.0, 12.0);
        assert!(
            shockwave_ground_arrival(center, Vec3::new(20.0, 0.0, 2.0), 60.0, Vec3::X, 0.0)
                .is_some()
        );
        assert!(
            shockwave_ground_arrival(center, Vec3::new(-20.0, 0.0, 2.0), 60.0, Vec3::X, 0.0)
                .is_none()
        );
        assert!(shockwave_ground_arrival(
            center,
            Vec3::new(20.0, 0.0, -2.0),
            60.0,
            Vec3::ZERO,
            0.0
        )
        .is_none());
        assert!(shockwave_ground_arrival(
            Vec3::new(0.0, 0.0, 100.0),
            Vec3::new(20.0, 0.0, 2.0),
            60.0,
            Vec3::ZERO,
            0.0
        )
        .is_none());
    }
}

#[cfg(test)]
mod glass_tests {
    use super::*;

    /// Overlay glass shows the scene blurred and darkened, and leaves the rest alone.
    /// `GLASS_DUMP` names a PPM to write the frame to.
    #[test]
    #[ignore = "requires Vulkan and maps/crosswater.mcmap"]
    fn glass_blurs_the_scene_behind_it() {
        use glam::Vec2;
        let (w, h) = (640u32, 360u32);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/crosswater.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let mut renderer = Renderer::new(
            Target::Headless {
                width: w,
                height: h,
            },
            SceneDesc {
                map: map.clone(),
                blueprints,
                pool: Arc::new(Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        renderer.fog_enabled = false;
        let mut camera = Camera::new(
            Vec2::from(map.info().size_metres().to_f32()),
            Vec2::new(w as f32, h as f32),
        );
        let xy = Vec2::new(12200.0, 12150.0);
        camera.focus = xy.extend(renderer.ground_height(xy));
        camera.distance = 600.0;
        let frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        let mut draw = |overlay: &Overlay| {
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: 1.0,
                    alpha: 1.0,
                    sim: Some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay,
                    build_grid: false,
                    icons: true,
                })
                .unwrap();
            renderer.read_pixels().unwrap()
        };
        let plain = draw(&Overlay::default());
        let mut overlay = Overlay::default();
        overlay.blur_rect(0.0, 0.0, w as f32 / 2.0, h as f32, [0.0; 4]);
        overlay.blur_rect(
            w as f32 * 0.75,
            0.0,
            w as f32 / 4.0,
            h as f32,
            [0.0, 0.0, 0.0, 0.55],
        );
        let glass = draw(&overlay);
        if let Ok(path) = std::env::var("GLASS_DUMP") {
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            for pixel in glass.as_chunks::<4>().0 {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(path, ppm).unwrap();
        }
        // Mean luminance and mean difference between horizontal neighbours over columns `x0..x1`.
        let stats = |px: &[u8], x0: u32, x1: u32| {
            let luma = |x: u32, y: u32| {
                let at = ((y * w + x) * 4) as usize;
                px[at] as f32 * 0.3 + px[at + 1] as f32 * 0.59 + px[at + 2] as f32 * 0.11
            };
            let (mut sum, mut detail, mut count) = (0.0, 0.0, 0.0);
            for y in 20..h - 20 {
                for x in x0 + 20..x1 - 20 {
                    sum += luma(x, y);
                    detail += (luma(x + 1, y) - luma(x, y)).abs();
                    count += 1.0;
                }
            }
            (sum / count, detail / count)
        };
        let (half, quarter) = (w / 2, w / 4);
        let (plain_mean, plain_detail) = stats(&plain, 0, half);
        let (blur_mean, blur_detail) = stats(&glass, 0, half);
        eprintln!(
            "plain {plain_mean:.1}/{plain_detail:.2}, blurred {blur_mean:.1}/{blur_detail:.2}"
        );
        assert!(
            plain_detail > 0.2,
            "the scene has too little detail to tell"
        );
        assert!(blur_detail < plain_detail * 0.5, "glass did not blur");
        assert!(
            (blur_mean - plain_mean).abs() < plain_mean * 0.15,
            "glass changed the brightness"
        );
        // Outside the glass nothing changes (but for streaming noise between two frames).
        let (open, shut) = (
            stats(&plain, half, 3 * quarter),
            stats(&glass, half, 3 * quarter),
        );
        assert!(
            (open.0 - shut.0).abs() < 0.5 && (open.1 - shut.1).abs() < 0.05,
            "{open:?} vs {shut:?}"
        );
        let (dark_mean, _) = stats(&glass, 3 * quarter, w);
        let (under_mean, _) = stats(&plain, 3 * quarter, w);
        assert!(
            dark_mean < under_mean * 0.75,
            "tinted glass is not darker: {dark_mean} vs {under_mean}"
        );
    }
}

/// The ore under every field as solid geometry: a lumpy body at the field's
/// heart and lodes running out from it toward the outline, rising and sinking,
/// each swelling into nodules along the way. Vertex `pos` is xy in metres and
/// z as metres below the surface (negative); `vs_vein` hangs it under the
/// terrain. Deterministic, from the outlines alone.
fn ore_vein_mesh(regions: &[mc_map::OreRegion]) -> Vec<MeshVertex> {
    use glam::{Vec2, Vec3};
    let mut out = Vec::new();
    let vertex = |pos: Vec3, normal: Vec3| {
        let mut v = MeshVertex::zeroed();
        v.pos = pos.into();
        v.normal = normal.into();
        v
    };
    let hash = |a: u32, b: u32| {
        let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        (h & 0xFFFF) as f32 / 65535.0
    };
    // An ellipsoid of radii `r`, lumpy with `seed`.
    let blob = |out: &mut Vec<MeshVertex>, c: Vec3, r: Vec3, seed: u32| {
        const LAT: usize = 7;
        const LON: usize = 12;
        let at = |i: usize, j: usize| {
            let th = i as f32 / LAT as f32 * std::f32::consts::PI;
            let ph = j as f32 / LON as f32 * std::f32::consts::TAU;
            let n = Vec3::new(th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos());
            let bump = 0.8 + 0.4 * hash(seed, (i * LON + j % LON) as u32);
            (c + n * r * bump, n)
        };
        for i in 0..LAT {
            for j in 0..LON {
                let (a, na) = at(i, j);
                let (b, nb) = at(i + 1, j);
                let (cc, nc) = at(i + 1, j + 1);
                let (d, nd) = at(i, j + 1);
                out.extend([vertex(a, na), vertex(b, nb), vertex(cc, nc)]);
                out.extend([vertex(a, na), vertex(cc, nc), vertex(d, nd)]);
            }
        }
    };
    // A tube through `line`, radius per point.
    let tube = |out: &mut Vec<MeshVertex>, line: &[(Vec3, f32)]| {
        const SIDES: usize = 9;
        let ring = |k: usize| {
            let (p, r) = line[k];
            let dir = if k + 1 < line.len() {
                line[k + 1].0 - p
            } else {
                p - line[k - 1].0
            };
            let dir = dir.normalize_or_zero();
            let side = dir.cross(Vec3::Z).normalize_or(Vec3::X);
            let up = side.cross(dir);
            (0..=SIDES)
                .map(|i| {
                    let a = i as f32 / SIDES as f32 * std::f32::consts::TAU;
                    let n = side * a.cos() + up * a.sin();
                    (p + n * r, n)
                })
                .collect::<Vec<_>>()
        };
        for k in 0..line.len() - 1 {
            let (a, b) = (ring(k), ring(k + 1));
            for i in 0..SIDES {
                let (p0, n0) = a[i];
                let (p1, n1) = a[i + 1];
                let (q0, m0) = b[i];
                let (q1, m1) = b[i + 1];
                out.extend([vertex(p0, n0), vertex(q0, m0), vertex(q1, m1)]);
                out.extend([vertex(p0, n0), vertex(q1, m1), vertex(p1, n1)]);
            }
        }
    };
    for (index, region) in regions.iter().enumerate() {
        let pts: Vec<Vec2> = region
            .points
            .iter()
            .map(|p| Vec2::from(p.to_f32()))
            .collect();
        if pts.len() < 3 {
            continue;
        }
        let seed = index as u32 * 977 + 13;
        let centre = Vec2::from(region.centre().to_f32());
        let depth = region.depth().to_f32();
        let heart = centre.extend(-depth);
        blob(&mut out, heart, Vec3::new(24.0, 19.0, 14.0), seed);
        let lodes = 4 + (hash(seed, 1) * 3.0) as usize;
        for v in 0..lodes {
            let corner = pts
                [(v * pts.len() / lodes + (hash(seed, 2 + v as u32) * 3.0) as usize) % pts.len()];
            let end = centre + (corner - centre) * (0.7 + 0.25 * hash(seed, 20 + v as u32));
            let rise = (hash(seed, 40 + v as u32) - 0.5) * 90.0;
            let side = (end - centre).perp().normalize_or_zero();
            let steps = 10;
            let line: Vec<(Vec3, f32)> = (0..=steps)
                .map(|k| {
                    let t = k as f32 / steps as f32;
                    let wander = (hash(seed, 100 + v as u32 * 16 + k as u32) - 0.5)
                        * 30.0
                        * (t * (1.0 - t) * 4.0);
                    let xy = centre.lerp(end, t) + side * wander;
                    let z = -depth
                        - rise * t
                        - (t * std::f32::consts::PI * 1.5 + v as f32).sin() * 10.0;
                    (xy.extend(z), 11.0 * (1.0 - t).powf(0.8) + 2.5)
                })
                .collect();
            tube(&mut out, &line);
            // Nodules where the lode swells.
            for k in [4usize, 7] {
                if hash(seed, 300 + v as u32 * 4 + k as u32) > 0.35 {
                    let (p, r) = line[k];
                    blob(
                        &mut out,
                        p,
                        Vec3::splat(r * 1.6),
                        seed + 500 + v as u32 * 8 + k as u32,
                    );
                }
            }
        }
    }
    out
}

/// Capital transports pass through cloud without changing its density or flow.
fn stirs_clouds(u: &UnitInstance, bp: &mc_data::UnitBlueprint) -> bool {
    u.owner_flags & (KIND_WRECK | STATE_RADAR) == 0
        && u.build >= 1.0
        && bp
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
        && bp.transport.is_none()
        && Vec3::from(u.prev_pos).distance(Vec3::from(u.pos)) > 0.3
}

#[cfg(test)]
mod capital_cloud_tests {
    use super::*;
    #[test]
    fn capital_hulls_never_clear_clouds_at_rest_or_underway() {
        let data = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let bp = data.unit(data.id_of("aster_t3_lift_ship").unwrap());
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = [1000.0, 1000.0, 420.0];
        unit.prev_pos = unit.pos;
        unit.build = 1.0;
        assert!(!stirs_clouds(&unit, bp));
        unit.pos[0] += 8.0;
        assert!(!stirs_clouds(&unit, bp));
        unit.build = 0.5;
        assert!(!stirs_clouds(&unit, bp));
        unit.build = 1.0;
        unit.owner_flags = KIND_WRECK;
        assert!(!stirs_clouds(&unit, bp));
    }
}
