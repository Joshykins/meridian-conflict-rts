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
use crate::ground_cover;
use crate::textures;
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use mc_data::{cat, Blueprints};
use mc_jobs::Pool;
use mc_map::{MapFile, PropKind, BUILD_CELL_M, TILE_SAMPLES};
use mc_sim::mirror::{
    FireInstance, ProjectileInstance, RenderFrame, SimEvent, StainInstance, UnitInstance, KIND_GHOST, KIND_PROP,
    KIND_WRECK, MAX_CONSTRUCTION_WELDS, PROJECTILE_BEAM, PROJECTILE_ENDS_SHIFT,
    PROJECTILE_FADE_BEAM, PROJECTILE_FRESH, PROJECTILE_MISSILE, PROJECTILE_COLD, PROJECTILE_SMOKE, PROJECTILE_TRAIL,
    PROJECTILE_APOGEE, PROJECTILE_SKIM,
    STATE_RADAR,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::collections::HashMap;
use std::sync::Arc;

mod clearing;
mod fallen_trees;
mod tree_wind;
mod mine_fx;
mod survival_fx;
mod water_fx;
mod wreck_fx;
mod bore_fx;
mod capital_fx;

pub const MAX_DYNAMIC: usize = mc_sim::tables::MAX_UNITS + mc_sim::tables::MAX_WRECKS + 512;
/// Units with gun houses of their own whose poses fit the houses buffer (`mirror::HousePose`).
pub const MAX_HOUSES: usize = 2048;
pub const MAX_MARKS: usize = 4096;
pub const MAX_EFFECTS: usize = 2048;
/// Expanding 3D pressure spheres. The oldest are overwritten.
pub const MAX_SHOCKWAVES: usize = 64;
/// Must match the missile mesh dimensions in sprites.wgsl.
fn missile_half_length(size: f32) -> f32 { (size * 1.4).clamp(1.4, 4.8) }

/// UV-sphere tessellation for a shockwave shell. Must match `shockwaves.wgsl`.
const SHOCKWAVE_LAT: u32 = 32;
const SHOCKWAVE_LON: u32 = 64;
/// Rings of short-lived particles and of track marks; the oldest are overwritten.
pub const MAX_PUFFS: usize = 8192;
/// Tail of the puff buffer. Smoke over a fire is rewritten here every frame,
/// so a carpet cannot wrap the ring and erase a column halfway through.
const GROUND_FIRE_SLOTS: usize = 512;
const PUFF_RING: usize = MAX_PUFFS - GROUND_FIRE_SLOTS;
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
    pub team_colors: [[f32; 3]; 8],
}

/// Selection / hover marker on an entity of the current render frame.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Mark {
    /// Index into `RenderFrame::units`.
    pub unit_index: u32,
    /// Bit 0: hovered (else selected). Bit 1: enemy.
    pub kind: u32,
    /// Construction fill, zero to one. Negative: the unit is not building, so
    /// the bar under health stays off.
    pub work: f32,
    /// Shield fill, zero to one. Negative: the unit has no bubble, so the
    /// line above health stays off.
    pub shield: f32,
}

/// `Mark` as the ring shader reads it, with the hull of a long ship (`icons.wgsl`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct GpuMark {
    unit_index: u32,
    kind: u32,
    work: f32,
    shield: f32,
    /// Half length and width of a long hull, metres; zero for a round ring.
    hull: [f32; 2],
    _pad: [f32; 2],
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
    build_cursor: [f32; 4],
    build_blocked: [[f32; 4]; BUILD_BLOCKED_MAX],
    /// The 3D scene's size in pixels, the render scale, and 1 when FXAA is on.
    /// `viewport` stays the output's size: pixel widths are output pixels.
    scene: [f32; 4],
    /// x how many of `tree_blasts` are in use (tree_wind.rs).
    tree_wind: [f32; 4],
    tree_blasts: [[f32; 4]; tree_wind::TREE_BLASTS * 2],
}

/// Lots the build grid shows as taken, at most.
pub const BUILD_BLOCKED_MAX: usize = 48;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ModelInfo {
    slot: u32,
    icon: u32,
    bounds_radius: f32,
    height: f32,
    /// Metres of the hull-plan atlas: [-1, 1] in plan UV is this square.
    plan_half: f32,
    /// The refit modules on show (`Blueprints::look`), one bit each.
    modules: u32,
    /// A pit dug into the ground (`Model::pit`): its opening's height and radius. Zero for none.
    pit: [f32; 2],
    turret_pivot: [f32; 4],
    /// w: how far a `part::RAM` pile driver is hauled up (`models::Pit::stroke`).
    spinner_pivot: [f32; 4],
    /// `Legs`: hip and the stride, knee and the lift, ankle and the stance. All zero for a model without legs.
    leg_hip: [f32; 4],
    leg_knee: [f32; 4],
    leg_ankle: [f32; 4],
    /// The left elbow forearms pitch about; w is one when the model has one.
    arm_pivot: [f32; 4],
    /// Rest-space barrel axis and recoil travel; zero if the tube does not slide.
    recoil: [f32; 4],
    /// Hinge of the folding gear and its stowed angle; zero if there is none.
    fold: [f32; 4],
    /// Trunnion of a mounted turret and its tube's kick-back; zero if there is none.
    mount: [f32; 4],
    /// The rotary barrels' axis for this loadout (a point on it; it runs along x), w 1 when there is one.
    spin: [f32; 4],
    /// Wrist of the head on the folding gear and its stowed angle; zero if there is none.
    fold_wrist: [f32; 4],
    /// A pit's pipe feed (`models::Pit`): where the next section waits (xy), the section's
    /// length, and how far the rig rises onto stilts in water. Zero for none.
    pit_feed: [f32; 4],
    /// How the surface shader sizes the model (`Model::surface_reach`), the height
    /// its field dust reaches (`Model::dust_line`), how far a walker's hips sink in
    /// stride (`Legs::crouch`) and its neck's height (`Model::neck`, zero for none).
    surface: [f32; 4],
    /// Gun houses of their own (`rig::HOUSE_FIRST + i`): pivot and kick-back travel.
    houses: [[f32; 4]; 4],
    /// Which weapon each house is bound to, plus one; zero for no house in that slot.
    house_weapon: [f32; 4],
    /// A spacecraft's rig for `entity.wgsl` (`models::capital_rig`): gear legs, bay doors,
    /// drives, lift jets, ramp. All zero for any other model.
    capital: [[f32; 4]; 7],
}

const _: () = assert!(std::mem::size_of::<ModelInfo>() == 432);

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
    origin: [f32; 4],
    pos: [f32; 3],
    start: f32,
    params: [f32; 4],
}

/// GPU shockwave. `axis` is the barrel direction for a muzzle blast; zero
/// for an isotropic sphere (impact, death, a dome going).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuShockwave {
    pos: [f32; 3],
    start: f32,
    params: [f32; 4],
    axis: [f32; 3],
    _pad: f32,
    tint: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<GpuShockwave>() == 64);

/// Mirrors `Puff` in shaders/puffs.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Puff {
    origin: [f32; 3],
    opacity: f32,
    pos: [f32; 3],
    start: f32,
    vel: [f32; 3],
    life: f32,
    /// Size at birth, size at the end, kind, seed.
    params: [f32; 4],
    /// Custom dust RGB (negative means natural color), brightness.
    appearance: [f32; 4],
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

/// A shatter gun's fading hitscan: muzzle to the burst, lingering after the tick.
struct FadeBeam {
    from: Vec3,
    to: Vec3,
    start: f32,
    life: f32,
    width: f32,
    /// Orange intercept laser. Shatter beams stay blue.
    laser: bool,
}

/// A shatter shot waiting for its burst so the beam can run muzzle to split.
struct PendingShatter {
    effects: mc_data::EffectSettings,
    muzzle: Vec3,
    dir: Vec3,
    range: f32,
    bolts: u8,
    splash: f32,
    width: f32,
    impact: f32,
    shockwave: f32,
    color: f32,
}

/// A hitscan shot waiting for its impact so the beam can run muzzle to hit.
struct PendingRail {
    muzzle: Vec3,
    dir: Vec3,
    range: f32,
    width: f32,
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

fn is_shatter_gun(weapon: &mc_data::Weapon) -> bool {
    weapon.sounds.fire.as_deref() == Some("aster_shatter")
}

/// Put the split visibly before the impact, including very short shots.
fn shatter_airburst(muzzle: Vec3, target: Vec3, fallback: Vec3, splash: f32) -> (Vec3, Vec3) {
    let distance = muzzle.distance(target);
    // Keep the beam on the fired barrel axis. Only the fragments lead the
    // moving target; bending the beam toward their future arrivals breaks aim.
    let forward = fallback.normalize_or_zero();
    let stand_off = (splash * 1.35).max(24.0).min(distance * 0.4);
    (muzzle + forward * (distance - stand_off), forward)
}

const SHATTER_FRAGMENT_MIN_LIFE: f32 = 0.16;
const SHATTER_FRAGMENT_MAX_LIFE: f32 = 0.33;

/// Impact positions are sampled against the tick-end hull, while the renderer
/// interpolates that hull from its previous position. Put the target on the
/// same render timeline, then lead by each fragment's actual travel time.
fn shatter_target_at(impact: Vec3, motion: Vec3, after: f32, tick_seconds: f32, delay: f32) -> Vec3 {
    impact + motion * (after - 1.0 + delay / tick_seconds.max(0.001))
}

/// The emplacement's impact 1.8 is the reference size; lighter guns also
/// get thinner fragments and shorter debris throws, not just smaller flashes.
fn shatter_detail_scale(impact: f32) -> f32 {
    (impact / 1.8).clamp(0.35, 1.5)
}

fn shatter_fragment_count(bolts: u8, miss: bool) -> u8 {
    if miss { (bolts / 2).max(3) } else { bolts.max(3).saturating_mul(2) }
}

fn trail_key(p: Vec3) -> [u32; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// GPU shield record. Wider than the sim's `ShieldInstance`: overlap and a
/// short hull list are packed here so the fragment shader never walks every
/// unit, or every other dome, unless those actually meet this one.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuShield {
    pos: [f32; 3],
    radius: f32,
    prev_open: f32,
    open: f32,
    health: f32,
    packed: u32,
    unit_id: u32,
    projector: f32,
    height: f32,
    /// 1 when another same-team dome overlaps this one.
    overlap: u32,
    contact_n: u32,
    _pad: [u32; 3],
    contacts: [u32; SHIELD_CONTACTS],
}

const _: () = assert!(std::mem::size_of::<GpuShield>() == 128);

/// Mirrors `ShieldHit` in shaders/shields.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShieldHit {
    pos: [f32; 3],
    start: f32,
    strength: f32,
    _pad: [f32; 3],
}

const _: () = assert!(std::mem::size_of::<ShieldHit>() == 32);

/// When a pressure sphere reaches a dry ground sample, and its directional
/// strength there. Inverts the shader's radius = reach * (1 - (1-age)^2).
fn shockwave_ground_arrival(
    center: Vec3, ground: Vec3, radius: f32, axis: Vec3, water: f32,
) -> Option<(f32, f32)> {
    if radius <= 0.2 || ground.z < water + 0.2 { return None; }
    let delta = ground - center;
    let fraction = delta.length() / radius;
    // By the last part of its reach the front is too weak to lift fresh dust.
    if fraction >= 0.86 { return None; }
    let directional = if axis.length_squared() > 0.25 {
        let t = ((delta.normalize_or_zero().dot(axis.normalize()) + 0.45) / 1.1).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    } else { 1.0 };
    if directional < 0.08 { return None; }
    Some((1.0 - (1.0 - fraction).sqrt(), directional * (1.0 - fraction * 0.65)))
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
const PUFF_SPLINTER: f32 = 13.0;
/// A directed plasma bolt thrown when a shatter beam splits.
const PUFF_PLASMA_BOLT: f32 = 14.0;
/// Torn blue-white explosion lobes at an airburst and fragment strikes.
const PUFF_SHATTER_BLAST: f32 = 15.0;
const PUFF_TREE_SMOKE: f32 = 16.0;
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
/// What `damage_fires` needs to know about a blueprint's model.
#[derive(Clone)]
struct BurnSite {
    grid: models::burns::BurnGrid,
    reach: f32,
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

const PASS_NAMES: [&str; 4] = ["cull", "shadow", "scene", "present"];

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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EffectBarrier {
    center: [f32; 3],
    radius: f32,
    inverse_axes: [f32; 3],
    min_z: f32,
}
impl EffectBarrier {
    fn crosses(&self, from: Vec3, to: Vec3) -> bool {
        let axes = Vec3::from(self.inverse_axes);
        let q = (from - Vec3::from(self.center)) * axes;
        let v = (to - from) * axes;
        let a = v.length_squared();
        let c = q.length_squared() - 1.0;
        if a < 0.0000001 || (c < -0.0001 && (q + v).length_squared() < 0.9999) { return false; }
        let b = q.dot(v);
        let disc = b * b - a * c;
        if disc <= 0.0 { return false; }
        for t in [(-b - disc.sqrt()) / a, (-b + disc.sqrt()) / a] {
            // A surface impact can emit back out, but cannot emit into the field.
            if t >= -0.0001 && t <= 1.0 && (t > 0.0001 || b < 0.0)
                && (from + (to - from) * t).z >= self.min_z - 0.1 { return true; }
        }
        false
    }
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
    fxaa: bool,
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
    hull_set: vk::DescriptorSet,
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
    vis: Buffer,
    counters: Buffer,
    commands: Buffer,
    visible: Buffer,
    props_dead: Buffer,
    prop_instances: Vec<UnitInstance>,
    tree_model_base: u32,
    previous_dead: Vec<u32>,
    burning_trees: Vec<BurningTree>,
    fallen_trees: fallen_trees::FallenTrees,
    tree_blasts: tree_wind::TreeBlasts,
    /// Rings, flashes and wakes on the sea, and the set the water draws with.
    water_fx: water_fx::WaterFx,
    /// Craters round wrecks and the smoke off them (renderer/wreck_fx.rs).
    wreck_fx: wreck_fx::WreckFx,
    /// Electric bore lightning and the molten ground it leaves (renderer/bore_fx.rs).
    bore_fx: bore_fx::BoreFx,
    /// Capital ships' drives, lift jets and lamps (renderer/capital_fx.rs).
    capital_fx: capital_fx::CapitalFx,
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
    fog: Image,
    fog_dims: (u32, u32),
    noise: Image,
    panel: Image,
    terrain_materials: Image,
    ground_cover: Image,
    /// Sun, sky and weather (sky.rs).
    sky: crate::sky::Sky,
    pad_footprints: Image,
    hull_plans: Image,
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
    /// Blueprint of each sim unit uploaded, for per-unit lookups (selection rings).
    unit_blueprints: Vec<u16>,
    projectile_count: u32,
    stain_count: u32,
    pad_count: u32,
    /// Ore fields as ground splats, written after stains and pads: every
    /// field's corners first, then the tiles that cover the fields.
    deposit_splats: Vec<StainInstance>,
    /// How many entries at the front of `deposit_splats` are corners, not tiles.
    ore_corners: usize,
    /// 0..1: how strongly ore fields show. Faint normally; full while a mine is
    /// placed. Eases toward the goal a little every frame.
    ore_highlight: f32,
    ore_highlight_goal: f32,
    /// Seconds, for the veins' drifting glints.
    vein_time: f32,
    /// Corner count of every ore field, in order, and which are being mined
    /// by a mine the viewer has seen.
    ore_regions: Vec<usize>,
    ore_tapped: Vec<bool>,
    deposit_count: u32,
    deposit_first: u32,
    effect_cursor: usize,
    shockwave_cursor: usize,
    puff_cursor: usize,
    shield_count: u32,
    hull_shield_count: u32,
    shield_hit_cursor: usize,
    beam_count: u32,
    /// Beams that are on, by the unit they come from.
    beams_live: HashMap<u32, GpuBeam>,
    /// Beams that have shut off and are emptying out.
    beams_ended: Vec<GpuBeam>,
    /// Flown path of each missile or energy slug, keyed by this tick's head.
    trail_paths: HashMap<[u32; 3], TrailPath>,
    /// Shatter hitscan beams that are still fading.
    fade_beams: Vec<FadeBeam>,
    /// Shatter shots this tick whose burst has not been drawn yet.
    pending_shatter: Vec<PendingShatter>,
    /// Hitscan shots fired this tick whose impact has not been seen yet.
    pending_rail: Vec<PendingRail>,
    track_cursor: usize,
    /// Track marks written so far, capped at the ring's size: how many to draw.
    track_count: u32,
    scatter: Scatter,
    blueprints: Arc<Blueprints>,
    /// Per blueprint: where its tracks touch the ground, if it has any.
    treads: Vec<Option<Treads>>,
    /// Per blueprint: the legs of a walker, for the dust and prints its feet leave.
    legs: Vec<Option<Legs>>,
    /// Per blueprint: hovercraft raise a downwash instead of track marks.
    hover: Vec<bool>,
    /// Per blueprint: where smoke and flame stand on the hull over its burn marks.
    burn_sites: Vec<BurnSite>,
    /// Render-clock seconds between the last two sim ticks.
    tick_seconds: f32,
    last_tick_time: f32,
    fog_enabled: bool,
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
        turret_pivot: [0.0; 3],
        spinner_pivot: [0.0; 3],
        bounds_radius: (radius * radius + height * height).sqrt(),
        surface_reach: (radius * radius + height * height).sqrt(),
        dust_line: height * 0.62,
        treads: None,
        legs: None,
        arm_pivot: None,
        arm_boom: false,
        recoil: None,
        fold: None,
        fold_wrist: None,
        neck: None,
        mount: None,
        houses: Vec::new(),
        spins: Vec::new(),
        hover: false,
        pit: None,
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
        let clock = std::time::Instant::now();
        let last = std::cell::Cell::new(("Waking the graphics card", clock));
        // Shares are measured build time on the RTX 3080 Ti.
        let step = |name: &'static str, done: f32| {
            let (was, since) = last.replace((name, std::time::Instant::now()));
            log::debug!("renderer build: {was} {:.0} ms", since.elapsed().as_secs_f32() * 1000.0);
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
        let mut burn_sites: Vec<BurnSite> = Vec::new();
        let mut pad_layers: Vec<Vec<u8>> = Vec::new();
        let mut hull_layers: Vec<(Vec<u8>, f32)> = Vec::new();
        for bp in &bps.units {
            progress("Building unit models", 0.2 + 0.3 * bp.id.index() as f32 / bps.units.len() as f32);
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
                burn_sites.push(burn_sites[base.index()].clone());
                continue;
            }
            let (radius, height) = (bp.radius.to_f32(), bp.height.to_f32());
            let module_keys: Vec<&str> = bps.refit_set(bp.id).map_or(Vec::new(), |set| {
                set.slots.iter().flat_map(|s| &s.modules).map(|m| m.key.as_str()).collect()
            });
            let model = models::build_model_fitted(&bp.visual.mesh, radius, height, bp.tech, &module_keys)
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
                | ((bp.visual.mesh == "reclaim_carrier") as u32) << 22
                // A ship: rides the swell, not the ground (`entity.wgsl`).
                | (bp.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Naval) as u32) << 23
                // Transport flight pitch; Bastion also has ramp/gear parts (`entity.wgsl`).
                | (bp.transport.is_some() as u32) << 24
                | ((bp.visual.mesh == "light_transport") as u32) << 25;
            let pad = if bp.is_structure() && !bp.has(cat::WALL) {
                let half = bp.footprint.0.max(bp.footprint.1) as f32 * (BUILD_CELL_M as f32 * 0.5);
                models::bake_pad_footprint(&model.lods[0], half)
            } else {
                vec![0u8; (models::PAD_FOOTPRINT_RES * models::PAD_FOOTPRINT_RES) as usize]
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
            burn_sites.push(BurnSite {
                grid: models::burns::BurnGrid::bake(&model.lods[0]),
                // As `entity.wgsl` hands them to the surface shader, so the marks agree.
                reach: model.surface_reach.max(1.0),
                height: plan_h.max(1.0),
                turret_pivot: Vec3::from(model.turret_pivot),
            });
            drawn_as.push((model_list.len(), icon, bps.look(bp.id)));
            model_list.push((model, icon));
        }
        step("Shaping terrain props", 0.5);
        let prop_base = drawn_as.len() as u32;
        for kind in PropKind::ALL {
            let key = models::prop_model_key(kind.raw());
            let model = models::build_model(key).unwrap_or_else(|| fallback_model(key, 4.0, 8.0));
            drawn_as.push((model_list.len(), 13, 0));
            model_list.push((model, 13));
        }
        let mut vertices: Vec<MeshVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut slots: Vec<DrawSlot> = Vec::new();
        let mut infos: Vec<ModelInfo> = Vec::new();
        let mut first_slot: Vec<u32> = Vec::new();
        for (model, _) in &model_list {
            first_slot.push(slots.len() as u32);
            for lod in &model.lods {
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
        }
        for &(at, icon, look) in &drawn_as {
            let (model, _) = &model_list[at];
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
            infos.push(ModelInfo {
                slot: first_slot[at],
                icon: *icon,
                bounds_radius: model.bounds_radius,
                height,
                plan_half,
                modules: look,
                pit: model.pit.map_or([0.0; 2], |p| [p.open, p.radius]),
                pit_feed: model.pit.map_or([0.0; 4], |p| [p.rack[0], p.rack[1], p.section, p.afloat_lift]),
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
                    model.houses.get(i).map_or([0.0; 4], |h| [h.pivot[0], h.pivot[1], h.pivot[2], h.travel])
                }),
                house_weapon: std::array::from_fn(|i| model.houses.get(i).map_or(0.0, |h| h.weapon as f32 + 1.0)),
                capital: models::capital_rig(&model.key).unwrap_or([[0.0; 4]; 7]),
                spin: model
                    .spins
                    .iter()
                    .find(|(need, until, _)| {
                        let has = |tag: u32| tag != 0 && look & (1 << (tag - 1)) != 0;
                        (*need == 0 || has(*need)) && !has(*until)
                    })
                    // x carries which pieces turn about it (their module and `until` tags): the
                    // axis runs along x, so the shader needs only y and z of the point.
                    .map_or([0.0; 4], |(need, until, p)| [(need | until << 6) as f32, p[1], p[2], 1.0]),
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
                    recoil: 0.0,
                    prev_recoil: 0.0,
                    weld_first: 0,
                    weld_count: 0,
                    deploy: 0.0,
                    prev_deploy: 0.0,
                    _pad2: [0.0; 2],
                    refit_modules: 0,
                    _pad3: [0; 3],
                    mount: [0.0; 4],
                    spin_recoil: [0.0; 4],
                }
            })
            .collect();
        let rounded: Vec<mc_map::OreRegion> = scene.map.ore_regions().iter().map(rounded_ore).collect();
        let (deposit_splats, ore_corners) = ore_splats(&rounded);
        let ore_regions: Vec<usize> = rounded.iter().map(|r| r.points.len()).collect();
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
        let vis = gpu.device_buffer(total_entities * 4, storage)?;
        let counters = gpu.device_buffer(slot_count as u64 * 4, storage)?;
        let commands = gpu.device_buffer(slot_count as u64 * 20, storage | U::INDIRECT_BUFFER)?;
        // Room for every unit twice: its model and its strategic icon.
        let visible = gpu.device_buffer((total_entities + MAX_DYNAMIC as u64) * 4, storage)?;
        let props_dead = gpu.host_buffer((static_count as u64).div_ceil(32).max(1) * 4, storage)?;
        props_dead.write(0, &vec![0u8; props_dead.size as usize]);
        let nodes = gpu.host_buffer((MAX_NODES * size_of::<TerrainNode>()) as u64, storage)?;
        let marks = gpu.host_buffer((MAX_MARKS * size_of::<GpuMark>()) as u64, storage)?;
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
        let effect_barriers = gpu.host_buffer((16 + MAX_SHIELDS * size_of::<EffectBarrier>()) as u64, storage)?;
        effect_barriers.write(0, &vec![0u8; effect_barriers.size as usize]);
        let houses = gpu.host_buffer((MAX_HOUSES * size_of::<mc_sim::mirror::HousePose>()) as u64, storage)?;
        houses.write(0, &vec![0u8; houses.size as usize]);
        let light_list_bytes = (crate::lights::MAX_LIGHTS * size_of::<crate::lights::GpuLight>()) as u64;
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
        let vein_count = vein_mesh.len() as u32;
        let empty = [MeshVertex::zeroed()];
        let vein_vb = gpu.buffer_with_data(
            bytemuck::cast_slice(if vein_mesh.is_empty() { &empty[..] } else { &vein_mesh }),
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
            gpu.upload_image(&terrain_materials, layer as u32, 0, None, pixels, layer == 0)?;
            for (mip, (_, data)) in textures::terrain_mips(pixels, *cutout).iter().enumerate() {
                gpu.upload_image(&terrain_materials, layer as u32, mip as u32 + 1, None, data, false)?;
            }
        }
        step("Laying ground cover", 0.8);
        let cover = ground_cover::ground_cover(&scene.map);
        let ground_cover = gpu.image(&ImageDesc {
            width: cover.width,
            height: cover.height,
            format: vk::Format::R8G8B8A8_UNORM,
            usage: sampled,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&ground_cover, 0, 0, None, &cover.texels, true)?;
        let pad_res = models::PAD_FOOTPRINT_RES;
        let pad_footprints = gpu.image(&ImageDesc {
            width: pad_res,
            height: pad_res,
            format: vk::Format::R8_UNORM,
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
                &vec![0u8; (pad_res * pad_res) as usize],
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

        step("Wiring the passes together", 0.96);
        // Descriptor sets.
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 16,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 100,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 64,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: 16,
            },
        ];
        let descriptor_pool = unsafe {
            gpu.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(28)
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
        let shockwaves_set = alloc(layouts.pass_set)?;
        let sprites_set = alloc(layouts.pass_set)?;
        let stains_set = alloc(layouts.pass_set)?;
        let puffs_set = alloc(layouts.pass_set)?;
        let shields_set = alloc(layouts.pass_set)?;
        let hdr_set = alloc(layouts.screen_set)?;
        let bloom_sets = (0..BLOOM_LEVELS)
            .map(|_| alloc(layouts.screen_set))
            .collect::<Result<Vec<_>, _>>()?;
        let glass_sets = [alloc(layouts.screen_set)?, alloc(layouts.screen_set)?];
        let water_set = alloc(layouts.screen_set)?;
        let hull_set = alloc(layouts.screen_set)?;
        // What the water reads of the effects on it (renderer/water_fx.rs), in both bindings of a pass set.
        let sea_fx = gpu.host_buffer(water_fx::SEA_FX_BYTES as u64, storage)?;
        let sea_set = alloc(layouts.pass_set)?;

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
        write_buffers(scene_set, 15, vk::DescriptorType::STORAGE_BUFFER, &[&welds]);
        for (binding, image) in [
            (5, &overview),
            (6, &tiles),
            (7, &tile_index),
            (8, &fog),
            (9, &noise),
            (10, &panel),
            (16, &pad_footprints),
            (17, &hull_plans),
            (18, &terrain_materials),
            (20, &ground_cover),
        ] {
            write_image(scene_set, binding, image.view, read);
        }
        write_image(
            scene_set,
            11,
            shadow.view,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
        );
        write_image(scene_set, 21, sky.weather_view(), vk::ImageLayout::GENERAL);
        write_image(scene_set, 23, sky.flow_view(), vk::ImageLayout::GENERAL);
        write_image(scene_set, 24, sky.floor_view(), read);
        write_image(scene_set, 27, sky.shade_view(), vk::ImageLayout::GENERAL);
        write_buffers(scene_set, 22, vk::DescriptorType::UNIFORM_BUFFER, &[sky.atmosphere_buffer()]);
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
        write_buffers(scene_set, 19, vk::DescriptorType::STORAGE_BUFFER, &[&effect_barriers]);
        write_buffers(scene_set, 28, vk::DescriptorType::STORAGE_BUFFER, &[&houses]);
        write_buffers(scene_set, 25, vk::DescriptorType::STORAGE_BUFFER, &[&light_list, &light_grid]);
        write_buffers(sea_set, 0, vk::DescriptorType::STORAGE_BUFFER, &[&sea_fx, &sea_fx]);
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
            write_buffers(*set, 6, vk::DescriptorType::STORAGE_BUFFER, &[&effect_barriers]);
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
            glass: Vec::new(),
            glass_fbs: Vec::new(),
            glass_sets,
            hdr_set,
            refract: placeholder(&gpu)?,
            refract_fb: vk::Framebuffer::null(),
            water_set,
            hull_depth: placeholder(&gpu)?,
            hull_depth_fb: vk::Framebuffer::null(),
            hull_set,
            present_format,
            width,
            height,
            scene_width: width,
            scene_height: height,
            render_scale: 1.0,
            fxaa: false,
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
            vis,
            counters,
            commands,
            visible,
            props_dead,
            prop_instances: statics_data,
            tree_model_base: prop_base,
            previous_dead: Vec::new(),
            burning_trees: Vec::new(),
            fallen_trees: Default::default(),
            tree_blasts: Default::default(),
            water_fx: water_fx::WaterFx::new(sea_fx, sea_set),
            wreck_fx: wreck_fx::WreckFx::default(),
            bore_fx: bore_fx::BoreFx::default(),
            capital_fx: capital_fx::CapitalFx::default(),
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
            fog_dims,
            noise,
            panel,
            terrain_materials,
            ground_cover,
            sky,
            pad_footprints,
            hull_plans,
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
            unit_blueprints: Vec::new(),
            projectile_count: 0,
            stain_count: 0,
            pad_count: 0,
            deposit_splats,
            ore_corners,
            ore_highlight: 0.0,
            ore_highlight_goal: 0.0,
            vein_time: 0.0,
            ore_tapped: vec![false; ore_regions.len()],
            ore_regions,
            deposit_count: 0,
            deposit_first: 0,
            effect_cursor: 0,
            shockwave_cursor: 0,
            puff_cursor: 0,
            shield_count: 0,
            hull_shield_count: 0,
            shield_hit_cursor: 0,
            beam_count: 0,
            beams_live: Default::default(),
            beams_ended: Vec::new(),
            trail_paths: HashMap::new(),
            fade_beams: Vec::new(),
            pending_shatter: Vec::new(),
            pending_rail: Vec::new(),
            track_cursor: 0,
            track_count: 0,
            scatter: Scatter(0x9E37_79B9),
            blueprints: scene.blueprints.clone(),
            treads,
            legs,
            hover,
            burn_sites,
            tick_seconds: 0.1,
            last_tick_time: 0.0,
            fog_enabled: false,
            build_cursor: [0.0; 4],
            build_blocked: [[0.0; 4]; BUILD_BLOCKED_MAX],
            last_time: 0.0,
            stats: FrameStats::default(),
            gpu,
        };
        // Headless shots and tests: MERIDIAN_RENDER_SCALE=1.5, MERIDIAN_FXAA=1.
        // The game sets both from its settings (`set_render_quality`).
        let env = |key| std::env::var(key).ok().and_then(|v| v.parse::<f32>().ok());
        renderer.render_scale = env("MERIDIAN_RENDER_SCALE").map_or(1.0, |s| s.clamp(0.5, 2.0));
        renderer.fxaa = env("MERIDIAN_FXAA").is_some_and(|v| v > 0.0);
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
        self.ore_highlight_goal = k.clamp(0.0, 1.0);
    }

    /// Which ore fields (in map order) a mine the viewer has seen is working;
    /// they take another colour from far away.
    pub fn set_ore_tapped(&mut self, tapped: &[bool]) {
        for (t, &v) in self.ore_tapped.iter_mut().zip(tapped) {
            *t = v;
        }
    }

    pub fn set_build_grid(&mut self, cursor: glam::Vec2, radius: f32, blocked: &[[f32; 4]]) {
        let n = blocked.len().min(BUILD_BLOCKED_MAX);
        self.build_cursor = [cursor.x, cursor.y, radius, n as f32];
        self.build_blocked[..n].copy_from_slice(&blocked[..n]);
    }

    pub fn device_name(&self) -> &str {
        &self.gpu.device_name
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Supersampling (a scale over 1), or a cheaper scene (under 1), and FXAA
    /// on the result. Rebuilds the size-dependent targets when the scale changes.
    pub fn set_render_quality(&mut self, scale: f32, fxaa: bool) -> Result<(), GpuError> {
        let scale = if scale.is_finite() { scale.clamp(0.5, 2.0) } else { 1.0 };
        self.fxaa = fxaa;
        if scale == self.render_scale {
            return Ok(());
        }
        self.render_scale = scale;
        self.create_size_dependent()
    }

    /// Cursor ray against the terrain (overview resolution).
    pub fn pick_ground(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        self.tile_cache.pick(origin, dir)
    }

    pub fn ground_height(&self, xy: glam::Vec2) -> f32 {
        self.tile_cache.overview_height(xy)
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
        self.ground_height(xy).max(self.map_info.water_level.to_f32())
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
        unsafe {
            for fb in self.present_fbs.drain(..) {
                device.destroy_framebuffer(fb, None);
            }
            if self.scene_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(self.scene_fb, None);
            }
            if self.refract_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(self.refract_fb, None);
            }
            if self.hull_depth_fb != vk::Framebuffer::null() {
                device.destroy_framebuffer(self.hull_depth_fb, None);
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
        self.sky.resize(&self.gpu, self.width, self.height, self.depth.view)?;
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
                (sw >> (level + 1)).max(1),
                (sh >> (level + 1)).max(1),
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
            let write = [vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(7)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&info)];
            unsafe { device.update_descriptor_sets(&write, &[]); }
        }
        write_view(self.screen_set, 0, self.hdr.view);
        write_view(self.screen_set, 3, self.bloom[0].view);
        write_view(self.hdr_set, 0, self.hdr.view);
        write_view(self.hdr_set, 3, self.hdr.view);
        write_view(self.water_set, 0, self.refract.view);
        write_view(self.water_set, 3, self.refract.view);
        unsafe {
            let info = [vk::DescriptorImageInfo::default()
                .image_view(self.hull_depth.view)
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
            let write = [vk::WriteDescriptorSet::default().dst_set(self.hull_set).dst_binding(7)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&info)];
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
        self.queries_valid = false;
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

    /// Parks a raging storm over `at` (the test range's "storm overhead"),
    /// or lets the weather run by itself again.
    pub fn park_storm(&mut self, at: Option<glam::Vec2>) {
        self.sky.park_storm(at);
    }

    /// How hard it is raining where the camera looks, 0 to 1, for the rain's sound.
    pub fn rain_here(&self) -> f32 {
        self.sky.rain_here()
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
        self.tree_blasts.record(at, self.last_time, reach, strength.min(1.0), false);
    }

    /// Big blasts throw the clouds about (sky.rs).
    fn stir_clouds(&mut self, event: &SimEvent, time: f32) {
        match event {
            SimEvent::UnitDied { pos, blueprint, .. } | SimEvent::AircraftCrashed { pos, blueprint } => {
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
                    self.sky.blast(Vec3::from(pos.to_f32()), reach, strength, time);
                }
            }
            SimEvent::Impact { pos, splash, .. } => {
                let splash = splash.to_f32();
                if splash >= 12.0 {
                    self.sky.blast(Vec3::from(pos.to_f32()), (splash * 6.0).min(400.0), (splash / 20.0).min(1.5), time);
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn sky_mut(&mut self) -> &mut crate::sky::Sky {
        &mut self.sky
    }

    /// Half length and width of the hull of sim unit `index` when it is long enough
    /// (a capital ship) for its selection ring to follow it; zero otherwise.
    fn long_hull(&self, index: u32) -> [f32; 2] {
        let Some(&blueprint) = self.unit_blueprints.get(index as usize) else {
            return [0.0; 2];
        };
        let bp = self.blueprints.unit(mc_data::BlueprintId(blueprint));
        let (x, y) = (bp.hull.0.to_f32(), bp.hull.1.to_f32());
        if bp.is_mobile() && y > 0.0 && x > y * 1.4 {
            [x, y]
        } else {
            [0.0; 2]
        }
    }

    fn upload_sim(&mut self, frame: &RenderFrame, time: f32, camera: &Camera) {
        let units = &frame.units[..frame.units.len().min(MAX_DYNAMIC - 768)];
        if units.len() < frame.units.len() {
            log::error!(
                "render mirror has {} entities; the renderer holds {}",
                frame.units.len(),
                units.len()
            );
        }
        self.dynamic.write(0, bytemuck::cast_slice(units));
        self.sim_units = units.len() as u32;
        self.unit_blueprints.clear();
        self.unit_blueprints.extend(units.iter().map(|u| u.blueprint as u16));
        let houses = &frame.houses[..frame.houses.len().min(MAX_HOUSES)];
        if !houses.is_empty() {
            self.houses.write(0, bytemuck::cast_slice(houses));
        }
        self.note_gun_hulls(units, houses);
        self.upload_welds(frame);
        self.upload_shields(frame, camera.eye());
        self.lights.tick(frame, &self.blueprints);

        let projectiles = &frame.projectiles[..frame.projectiles.len().min(MAX_PROJECTILES)];
        self.projectiles.write(0, bytemuck::cast_slice(projectiles));
        self.projectile_count = projectiles.len() as u32;

        self.upload_beams(frame, time);

        let stains = &frame.stains[..frame.stains.len().min(MAX_STAINS)];
        self.stains.write(0, bytemuck::cast_slice(stains));
        // Craters round wrecks (wreck_fx.rs) are drawn as stains too, after the sim's.
        // A world with no scorch at all is a new one (the backdrop restaged): its ground is whole.
        if frame.stains.is_empty() {
            self.wreck_fx.clear();
            self.bore_fx.clear();
        }
        let craters = self.wreck_fx.craters();
        let craters = &craters[..craters.len().min(MAX_STAINS - stains.len())];
        if !craters.is_empty() {
            self.stains.write(
                (stains.len() * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(craters),
            );
        }
        let crater_count = craters.len();
        // Molten ground left by electric bores (bore_fx.rs), cooling frame by frame.
        let molten = self.bore_fx.molten_stains(time);
        let molten = &molten[..molten.len().min(MAX_STAINS - stains.len() - crater_count)];
        if !molten.is_empty() {
            self.stains.write(
                ((stains.len() + crater_count) * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(molten),
            );
        }
        let stained = stains.len() + crater_count + molten.len();
        self.stain_count = stained as u32;
        let pads = self.structure_pads(units, &frame.pads, MAX_STAINS.saturating_sub(stained));
        if !pads.is_empty() {
            self.stains.write(
                (stained * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&pads),
            );
        }
        self.pad_count = pads.len() as u32;
        let used = stained + pads.len();
        // All or nothing: a tile without its field's corners would draw garbage.
        let n = if self.deposit_splats.len() <= MAX_STAINS.saturating_sub(used) {
            self.deposit_splats.len()
        } else {
            0
        };
        if n > 0 {
            // Corners carry the highlight in their unused radius, plus 2 on a
            // field a seen mine is working.
            self.ore_highlight += (self.ore_highlight_goal - self.ore_highlight) * 0.18;
            self.vein_time += 1.0 / 60.0;
            let mut at = 0;
            for (i, &n) in self.ore_regions.iter().enumerate() {
                let tapped = if self.ore_tapped.get(i).copied().unwrap_or(false) { 2.0 } else { 0.0 };
                for c in &mut self.deposit_splats[at..at + n] {
                    c.radius = self.ore_highlight + tapped;
                }
                at += n;
            }
            self.stains.write(
                (used * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&self.deposit_splats[..n]),
            );
        }
        self.deposit_first = (used + self.ore_corners) as u32;
        self.deposit_count = n.saturating_sub(self.ore_corners) as u32;

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
        self.mine_blows(units, time, camera);
        self.aircraft_trails(units, time, camera);
        self.aircraft_crash_trails(units, time, camera);
        self.damage_smoke(units, time, camera);
        self.wreck_smoke(units, time, camera);
        self.construction(frame, time, camera);
        self.tree_fires(frame, time, camera);
        self.trample_trees(frame, time);
        self.clear_lots(frame, time);
        for event in &frame.events {
            self.effects_of(event, time);
            self.stir_clouds(event, time);
        }
        self.sky.set_units(units.iter().map(|u| {
            let bp = self.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16));
            bp.transport.is_none().then_some(glam::Vec2::new(u.pos[0],u.pos[1]))
        }));
        let flyers: Vec<_> = units
            .iter()
            .filter(|u| stirs_clouds(u, self.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16))))
            .map(|u| {
                let r = self.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16)).radius.to_f32();
                (Vec3::from(u.prev_pos), Vec3::from(u.pos), r)
            })
            .collect();
        self.sky.set_flyers(flyers.into_iter());
        self.ground_fires(&frame.fires, time);
        self.flush_shatter_misses(time);
        self.flush_rail_misses(time);
        self.write_fade_beams(time);
        self.write_bore_strokes(time);
        self.missile_trails(projectiles, time, camera);
        self.stream_bursts(projectiles, time, camera);
        self.sea_tick(units, projectiles, time, camera);
    }

    /// The rounds of a stream gun's shot (`Weapon::rounds`) have no sim impact of their
    /// own. Each that lands on what its shot hit bursts there: a small orange pop and a
    /// few sparks, as its shot does.
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

    fn upload_shields(&mut self, frame: &RenderFrame, eye: Vec3) {
        let n = frame.shields.len().min(MAX_SHIELDS);
        let src = &frame.shields[..n];
        self.live_effect_barriers.clear();
        for s in src {
            if s.open < 200.0 / 255.0 || s.health <= 0.0
                || s.packed & ((1 << 24) | (1 << 26)) != 0 || s.radius <= 0.01 { continue; }
            let hull = s.packed & (1 << 25) != 0;
            let mut center = s.pos;
            if hull { center[2] += s.height * 0.5; }
            self.live_effect_barriers.push(EffectBarrier {
                center, radius: s.radius,
                inverse_axes: [1.0 / s.radius, 1.0 / s.radius,
                    if hull { 2.0 / s.height.max(0.1) } else { 1.0 / s.radius }],
                min_z: s.pos[2],
            });
        }
        self.effect_barriers.write(0, bytemuck::cast_slice(&[self.live_effect_barriers.len() as u32, 0, 0, 0]));
        self.effect_barriers.write(16, bytemuck::cast_slice(&self.live_effect_barriers));
        let units = &frame.units[..self.sim_units as usize];
        let skip_hull = KIND_GHOST | KIND_PROP | (mc_sim::tables::flag::IN_FACTORY as u32) << 8;
        let mut gpu = Vec::with_capacity(n);
        for (i, s) in src.iter().enumerate() {
            let team = (s.packed >> 8) & 255;
            let hull = (s.packed >> 25) & 1 == 1;
            let mut overlap = 0u32;
            // Hull wraps stay their own membrane: they do not fuse with a dome. Nor does a veil.
            if !hull && s.packed & mc_sim::mirror::SHIELD_VEIL == 0 {
                for (j, other) in src.iter().enumerate() {
                    if i == j
                        || ((other.packed >> 8) & 255) != team
                        || (other.packed >> 25) & 1 == 1
                        || other.packed & mc_sim::mirror::SHIELD_VEIL != 0
                    {
                        continue;
                    }
                    let dx = s.pos[0] - other.pos[0];
                    let dy = s.pos[1] - other.pos[1];
                    let dz = s.pos[2] - other.pos[2];
                    let r = s.radius + other.radius + SHIELD_PAD * 2.0;
                    if dx * dx + dy * dy + dz * dz <= r * r {
                        overlap = 1;
                        break;
                    }
                }
            }
            let mut contacts = [0u32; SHIELD_CONTACTS];
            let mut scores = [f32::MAX; SHIELD_CONTACTS];
            let mut contact_n = 0u32;
            let shell = s.radius;
            for (ei, e) in units.iter().enumerate() {
                if e.unit_id == s.unit_id || e.radius < 0.4 || e.owner_flags & skip_hull != 0 {
                    continue;
                }
                let reach = e.radius * 2.2 + 4.0;
                let ox = e.pos[0] - s.pos[0];
                let oy = e.pos[1] - s.pos[1];
                let oz = e.pos[2] - s.pos[2];
                let d2 = ox * ox + oy * oy + oz * oz;
                let lo = shell - reach;
                let hi = shell + reach;
                if d2 < lo.max(0.0) * lo.max(0.0) || d2 > hi * hi {
                    continue;
                }
                let ex = e.pos[0] - eye.x;
                let ey = e.pos[1] - eye.y;
                let ez = e.pos[2] - eye.z;
                let score = ex * ex + ey * ey + ez * ez;
                if contact_n < SHIELD_CONTACTS as u32 {
                    let k = contact_n as usize;
                    contacts[k] = ei as u32;
                    scores[k] = score;
                    contact_n += 1;
                    continue;
                }
                let mut worst = 0usize;
                for k in 1..SHIELD_CONTACTS {
                    if scores[k] > scores[worst] {
                        worst = k;
                    }
                }
                if score < scores[worst] {
                    contacts[worst] = ei as u32;
                    scores[worst] = score;
                }
            }
            gpu.push(GpuShield {
                pos: s.pos,
                radius: s.radius,
                prev_open: s.prev_open,
                open: s.open,
                health: s.health,
                packed: s.packed,
                unit_id: s.unit_id,
                projector: s.projector,
                height: s.height,
                overlap,
                contact_n,
                _pad: [0; 3],
                contacts,
            });
        }
        if n > 0 {
            self.shields.write(0, bytemuck::cast_slice(&gpu));
        }
        let prev = self.shield_count as usize;
        if n < prev {
            let zeros = vec![GpuShield::zeroed(); prev - n];
            self.shields.write(
                (n * size_of::<GpuShield>()) as u64,
                bytemuck::cast_slice(&zeros),
            );
        }
        self.shield_count = n as u32;
        self.hull_shield_count = src.iter().filter(|s| (s.packed >> 25) & 1 == 1).count() as u32;
    }

    /// Everything that shines this frame, into scene set bindings 25 and 26.
    fn upload_lights(&mut self, time: f32, alpha: f32, camera: &Camera) {
        for tree in &self.burning_trees {
            let at = Vec3::from(tree.instance.pos) + Vec3::Z * tree.height * 0.3;
            self.lights.tree_fire(at, time - tree.start);
        }
        for b in &self.fade_beams {
            let k = 1.0 - ((time - b.start) / b.life.max(0.01)).clamp(0.0, 1.0);
            let color = if b.laser { Vec3::new(1.0, 0.42, 0.07) } else { Vec3::new(0.3, 0.62, 1.0) };
            self.lights.beam(b.from, b.to, color * 90.0 * k * b.width.clamp(0.3, 3.0), 10.0 + 4.0 * b.width);
        }
        self.capital_lights(time, alpha);
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
        unsafe {
            if list > 0 {
                device.cmd_copy_buffer(cmd, self.light_stage.buffer, self.light_list.buffer,
                    &[vk::BufferCopy { src_offset: 0, dst_offset: 0, size: list }]);
            }
            if grid > 0 {
                device.cmd_copy_buffer(cmd, self.light_stage.buffer, self.light_grid.buffer,
                    &[vk::BufferCopy { src_offset: self.light_list.size, dst_offset: 0, size: grid }]);
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
        if self.live_effect_barriers.iter().any(|b| b.crosses(origin, Vec3::from(pos))) { return; }
        self.lights.effect(pos, start, radius, life, kind);
        let e = Effect {
            origin: origin.extend(1.0).to_array(),
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
        if self.live_effect_barriers.iter().any(|b| b.crosses(origin, center)) { return; }
        self.tree_blasts.record(center, start, radius, strength, axis != Vec3::ZERO);
        let e = GpuShockwave {
            pos,
            start,
            params: [radius, life, color, strength.clamp(0.0, 1.0)],
            axis: axis.normalize_or_zero().to_array(),
            _pad: 0.0,
            tint: self.effect_settings.shockwave_color.map_or([0.0; 4], |rgb| [rgb[0], rgb[1], rgb[2], 1.0]),
        };
        self.shockwaves.write(
            (self.shockwave_cursor * size_of::<GpuShockwave>()) as u64,
            bytemuck::bytes_of(&e),
        );
        self.shockwave_cursor = (self.shockwave_cursor + 1) % MAX_SHOCKWAVES;
        let previous_origin = self.effect_origin.replace(center);
        self.shockwave_ground_dust(center, start, radius, life, strength, axis);
        self.effect_origin = previous_origin;

    }

    /// Schedule once at birth, rather than emitting every rendered frame.
    /// Three staggered bands produce a swept patch of dust, not a dotted ring.
    fn shockwave_ground_dust(
        &mut self, center: Vec3, start: f32, radius: f32, life: f32,
        strength: f32, axis: Vec3,
    ) {
        if radius < 8.0 || strength < 0.15 || life <= 0.0 { return; }
        let map_size = Vec3::from(self.map_info.size_metres().extend(mc_core::Fx::ZERO).to_f32());
        let water = self.map_info.water_level.to_f32();
        let phase = self.scatter.unit() * std::f32::consts::TAU;
        for band in 0..3 {
            for sector in 0..24 {
                let angle = phase + (sector as f32 + band as f32 * 0.38
                    + self.scatter.signed() * 0.28) * std::f32::consts::TAU / 24.0;
                let outward = Vec3::new(angle.cos(), angle.sin(), 0.0);
                let reach = radius * (0.20 + band as f32 * 0.23 + self.scatter.signed() * 0.06);
                let mut ground = center + outward * reach;
                if ground.x < 0.0 || ground.y < 0.0
                    || ground.x > map_size.x || ground.y > map_size.y { continue; }
                ground.z = self.ground_height(ground.truncate());
                let Some((arrival, pressure)) =
                    shockwave_ground_arrival(center, ground, radius, axis, water)
                else { continue; };
                let power = (pressure * strength.clamp(0.0, 1.0)).sqrt();
                let size = (radius * 0.14).clamp(2.0, 12.0) * power;
                let born = start + arrival * life;
                let drift = outward * ((7.0 + self.scatter.unit() * 8.0) * power)
                    + Vec3::Z * (1.4 + self.scatter.unit() * 2.2);
                let duration = 2.0 + self.scatter.unit() * 1.0;
                let spread = size * (2.8 + self.scatter.unit() * 1.1);
                self.push_puff(PUFF_SHOCK_DUST, ground + Vec3::Z * (0.45 + size * 0.18), drift,
                    born, duration, (size, spread));
                if (sector + band * 2) % 12 == 0 {
                    self.push_puff(PUFF_SHOCK_SMOKE, ground + Vec3::Z * 0.8,
                        drift * 0.45 + Vec3::Z * 1.8, born + 0.06,
                        duration + 0.6, (size * 0.85, spread * 0.85));
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
        &mut self, kind: f32, pos: Vec3, vel: Vec3, start: f32,
        life: f32, size: (f32, f32), motion: Vec3,
    ) {
        let origin = self.effect_origin.unwrap_or(pos);
        if self.live_effect_barriers.iter().any(|b| b.crosses(origin, pos)) { return; }
        let dusty = kind == PUFF_DUST || kind == PUFF_SMOKE || kind == PUFF_SHOCK_DUST || kind == PUFF_SHOCK_SMOKE;
        let opacity = if dusty { self.effect_settings.dust_visibility } else { 1.0 };
        let life = life * if dusty { self.effect_settings.dust_lifetime } else { 1.0 };
        if opacity <= 0.0 || life <= 0.0 { return; }
        let appearance = if kind == PUFF_ION {
            [motion.x, motion.y, motion.z, 1.0]
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
                    u.blueprint as u16,
                    ghost,
                    false,
                ),
            });
        }
        pads
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
        let impacts: Vec<(Vec3, f32)> = frame.events.iter().filter_map(|event| {
            if let SimEvent::Impact { pos, splash, on_shield, .. } = event {
                if !on_shield && splash.to_f32() > 0.0 {
                    return Some((Vec3::from(pos.to_f32()), splash.to_f32()));
                }
            }
            None
        }).collect();
        for (word, &dead) in frame.props_dead.iter().enumerate() {
            // On first upload, old destruction is history, not a new forest fire.
            let mut changed = dead & !self.previous_dead.get(word).copied().unwrap_or(dead);
            while changed != 0 {
                let bit = changed.trailing_zeros();
                changed &= changed - 1;
                let index = word * 32 + bit as usize;
                let Some(&instance) = self.prop_instances.get(index) else { continue };
                let kind = instance.blueprint.wrapping_sub(self.tree_model_base);
                if kind >= 4 || self.burning_trees.len() >= 256 { continue; }
                let at = Vec3::from(instance.pos);
                let blasted = impacts.iter().any(|(center, radius)| {
                    at.truncate().distance(center.truncate()) <= radius + 2.0
                });
                let seared = frame.events.iter().any(|event| {
                    let SimEvent::BoreDischarge { from, to, width, .. } = event else { return false };
                    let from = glam::Vec2::from(from.xy().to_f32());
                    let to = glam::Vec2::from(to.xy().to_f32());
                    let segment = to - from;
                    let t = ((at.truncate() - from).dot(segment) / segment.length_squared().max(0.001)).clamp(0.0, 1.0);
                    at.truncate().distance(from + segment * t) <= width.to_f32().max(4.0) + 0.1
                });
                if !blasted && !seared { continue; }
                let height = [12.0, 14.0, 18.0, 9.0][kind as usize] * instance._pad as f32 * 0.001;
                self.burning_trees.push(BurningTree { instance, start: time, height });
            }
        }
        self.previous_dead.clone_from(&frame.props_dead);
        for i in 0..self.burning_trees.len() {
            let tree = self.burning_trees[i];
            let age = time - tree.start;
            let mut at = Vec3::from(tree.instance.pos);
            if at.distance(camera.focus) > camera.distance * 2.5 + 250.0 { continue; }
            at.z = self.ground_height(at.truncate());
            let h = tree.height;
            let strength = (1.0 - age / 30.0).clamp(0.0, 1.0);
            if strength > 0.0 {
                for _ in 0..4 {
                    let angle = self.scatter.unit() * std::f32::consts::TAU;
                    let flame = at + Vec3::new(angle.cos() * h * 0.30,
                        angle.sin() * h * 0.30, h * (0.40 + self.scatter.unit() * 0.40));
                    let rise = Vec3::new(0.6, 0.25, 3.0 + strength * 3.0);
                    self.push_puff(PUFF_TREE_FIRE, flame, rise, time, 1.05,
                        (h * 0.12 * strength, h * 0.27 * strength));
                }
            }
            // Emission is throttled independently of the rendered frame rate.
            if self.scatter.unit() < 0.5 {
                let smoke = at + Vec3::Z * h * (0.60 + strength * 0.2);
                self.push_puff(PUFF_TREE_SMOKE, smoke, (self.sky.wind_heading() * 1.3).extend(7.0),
                    time, 6.5, (h * 0.12, h * 0.55));
            }
            if strength > 0.2 && self.scatter.unit() < 0.18 {
                let vel = Vec3::new(1.0, 0.4, 8.0 + self.scatter.unit() * 8.0);
                self.push_puff(PUFF_SPARK, at + Vec3::Z * h * 0.7, vel,
                    time, 1.0, (0.16, 0.025));
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
            let moving = ((to - from).length() / self.tick_seconds.max(0.02) / 10.0).clamp(0.0, 1.0);
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
                    let (local, on_turret) = site
                        .grid
                        .surface(mark.centre[0] + angle.cos() * r, mark.centre[1] + angle.sin() * r)?;
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
                        + Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, local.z);
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
                let life = (2.4 + size * 1.2 + 2.0 * heavy).min(9.0)
                    * (0.8 + 0.4 * self.scatter.unit());
                let grown = size * (2.2 + 1.2 * thick);
                self.push_puff(PUFF_TREE_SMOKE, at + Vec3::Z * size * 0.4, drift, start, life, (size * 0.6, grown));
            }
        }
    }

    /// A falling wreck trails smoke. A burning hull keeps a flame on it, and
    /// only occasionally a wisp of smoke leaving that flame.
    fn aircraft_crash_trails(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        for u in units {
            let falling =
                u.owner_flags & KIND_WRECK != 0 && u._pad == mc_sim::mirror::WRECK_FALLING;
            let burning = u.owner_flags & (KIND_WRECK | STATE_RADAR) == 0
                && u._pad & mc_sim::mirror::UNIT_BURNING != 0;
            if !falling && !burning {
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

    /// Soft white smoke expands and fades along the actual flown
    /// path. No emitter is generated for parked, hidden or unfinished aircraft.
    fn aircraft_trails(&mut self, units: &[UnitInstance], time: f32, _camera: &Camera) {
        let hidden = KIND_WRECK
            | STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        for u in units {
            if u.owner_flags & hidden != 0 || u.build < 1.0 {
                continue;
            }
            let bp = self
                .blueprints
                .unit(mc_data::BlueprintId(u.blueprint as u16));
            let ports = crate::models::aircraft_exhausts(&bp.visual.mesh);
            let nacelles = crate::models::vtol_nacelles(&bp.visual.mesh);
            let carrier = bp.visual.mesh == "reclaim_carrier";
            let fans = carrier || bp.visual.mesh == "reclaim_drone";
            let hover_flight = bp.motion.is_some_and(|m| m.hover);
            let assault = bp.visual.mesh == "assault_air";
            let capital = bp.is_capital_ship();
            let transport_flight = bp.transport.is_some();
            if ports.is_empty() {
                continue;
            }
            let from = Vec3::from(u.prev_pos);
            let to = Vec3::from(u.pos);
            let distance = from.distance(to);
            if capital {
                self.capital_drives(u, time, _camera.focus.truncate());
                continue;
            }
            if transport_flight && to.z <= self.ground_height(to.truncate()) + 1.0 {
                continue;
            }
            // A hovering VTOL's engines run whether it moves or not; a jet that is
            // still is parked, and leaves nothing.
            let still = distance < 0.08;
            if (still && !hover_flight) || distance > 40.0 {
                continue;
            }
            let pitch = (to.z - from.z)
                .atan2((to - from).truncate().length().max(2.0))
                .clamp(-0.2, 0.2);
            let samples = if still {
                1
            } else {
                (distance / 1.6).ceil().clamp(1.0, 16.0) as usize
            };
            let delta = (u.heading - u.prev_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            for i in 0..samples {
                let t = (i as f32 + 0.5) / samples as f32;
                let yaw = u.prev_heading + delta * t;
                let bank = u._pad2[0] + (u._pad2[1] - u._pad2[0]) * t;
                let horizontal = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
                let mut pitch = if hover_flight {
                    -((to - from).dot(horizontal) * 0.035).clamp(-0.12, 0.12)
                } else {
                    pitch
                };
                if assault || transport_flight {
                    pitch = u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * t;
                }
                let forward = horizontal * pitch.cos() + Vec3::Z * pitch.sin();
                let up = Vec3::Z * pitch.cos() - horizontal * pitch.sin();
                let left = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
                let rolled_left = left * bank.cos() + up * bank.sin();
                let rolled_up = up * bank.cos() - left * bank.sin();
                for port in ports {
                    let mut port = Vec3::from(*port);
                    let mut nozzle = -forward;
                    if let Some(pivots) = nacelles {
                        // The pod tilts about its pivot as the entity shader tilts it:
                        // stood up to hover, laid down to cruise, rolled a little into a
                        // sideslip (not the carrier's). The nozzle is its aft end.
                        let travel = to - from;
                        let local_speed = travel.dot(horizontal);
                        let lateral = travel.dot(left);
                        let tilt = if carrier {
                            let cruise = (local_speed * 0.62 - travel.z * 1.1).clamp(0.0, 1.0);
                            1.5708 + (0.08 - 1.5708) * cruise
                        } else {
                            (1.5708 - local_speed * 0.18 - travel.z * 0.06).clamp(0.35, 2.5)
                        };
                        let roll = if carrier { 0.0 } else { (lateral * 0.12).clamp(-0.45, 0.45) };
                        let front = (port.x - pivots[0][0]).abs() < (port.x - pivots[1][0]).abs();
                        let pv = pivots[if front { 0 } else { 1 }];
                        let pivot = Vec3::new(pv[0], port.y, pv[2]);
                        let d = port - pivot;
                        let d = Vec3::new(
                            d.x * tilt.cos() - d.z * tilt.sin(),
                            d.y,
                            d.x * tilt.sin() + d.z * tilt.cos(),
                        );
                        port = pivot
                            + Vec3::new(
                                d.x,
                                d.y * roll.cos() - d.z * roll.sin(),
                                d.y * roll.sin() + d.z * roll.cos(),
                            );
                        let local = Vec3::new(-tilt.cos(), 0.0, -tilt.sin());
                        nozzle = forward * local.x + rolled_up * local.z;
                    }
                    let at = from.lerp(to, t)
                        + forward * port.x
                        + rolled_left * port.y
                        + rolled_up * port.z;
                    let start = time + t * self.tick_seconds;
                    if hover_flight {
                        // The camera looks down on a hovering VTOL, so a jet pointing
                        // straight down is hidden by its own pod: what shows is what
                        // reaches out past the pod's rim, and the wash on the ground.
                        let carried = (to - from) / self.tick_seconds.max(0.02);
                        if fans {
                            // A lift fan's field: a wide blue glow in the wash under the
                            // duct, hanging a moment where it was thrown.
                            let size = if carrier { (2.4, 3.4) } else { (0.55, 0.8) };
                            let reach = if carrier { 1.1 } else { 0.35 };
                            let drift = nozzle * if carrier { 6.0 } else { 3.0 } + carried * 0.6;
                            self.push_puff(PUFF_PLASMA, at + nozzle * reach, drift, start, 0.3, size);
                        } else {
                            // A vector-thrust jet: a long flame cone out of the nozzle
                            // (a little of it peeling off as soot) and the white-hot bloom
                            // at the mouth that lights past the pod's rim.
                            let drift = nozzle * (20.0 + self.scatter.unit() * 6.0)
                                + rolled_left * (self.scatter.signed() * 0.5)
                                + carried * 0.8;
                            self.push_puff(PUFF_FIRE, at + nozzle * 0.3, drift, start, 0.22, (0.8, 1.3));
                            let bloom = nozzle * 3.0 + carried * 0.9;
                            self.push_puff(PUFF_SPARK, at + nozzle * 0.4, bloom, start, 0.16, (1.3, 0.6));
                        }
                        if still || fans {
                            continue;
                        }
                        // Under way, the jets leave a thin haze behind them.
                        let drift = nozzle * 0.5
                            + rolled_left * (self.scatter.signed() * 1.2)
                            + rolled_up * (self.scatter.signed() * 0.6);
                        let life = 1.7 + self.scatter.unit() * 0.4;
                        self.push_puff(PUFF_CONTRAIL, at, drift, start, life, (0.6, 3.6));
                        continue;
                    }
                    let drift = nozzle * 0.5
                        + rolled_left * (self.scatter.signed() * 1.5)
                        + rolled_up * (self.scatter.signed() * 0.8);
                    let life = 2.6 + self.scatter.unit() * 0.4;
                    self.push_puff(PUFF_CONTRAIL, at, drift, start, life, (1.0, 6.0));
                }
            }
        }
    }

    /// Track marks and dust for the tracked vehicles that moved this tick,
    /// the prints a walker leaves as each foot comes down, and the downwash
    /// a hovercraft keeps under itself, near enough to the camera for any of
    /// them to be seen.
    fn ground_contact(&mut self, units: &[UnitInstance], time: f32, camera: &Camera) {
        const FLAG_MOVING: u32 = (mc_sim::tables::flag::MOVING as u32) << 8;
        if camera.distance > 2500.0 {
            return;
        }
        let reach = camera.distance * 2.5 + 300.0;
        let dust = camera.distance < 900.0;
        let focus = camera.focus.truncate();
        let water = self.map_info.water_level.to_f32();
        let hidden_aircraft = STATE_RADAR
            | ((mc_sim::tables::flag::IN_FACTORY | mc_sim::tables::flag::UNDER_CONSTRUCTION)
                as u32)
                << 8;
        let previous = (self.effect_origin, self.effect_settings);
        for u in units {
            self.effect_origin = Some(Vec3::from(u.pos));
            self.effect_settings = self.blueprints.units.get(u.blueprint as usize)
                .map_or_else(mc_data::EffectSettings::default, |bp| bp.visual.effects);
            if u.owner_flags & (KIND_WRECK | STATE_RADAR) != 0 {
                continue;
            }
            let moving = u.owner_flags & FLAG_MOVING != 0;
            let close = Vec3::from(u.pos).truncate().distance(focus) <= reach;
            if dust
                && close
                && self
                    .hover
                    .get(u.blueprint as usize)
                    .copied()
                    .unwrap_or(false)
                && u.pos[2] >= water
            {
                self.hover_downwash(u, time, moving);
            }
            if dust && close && u.build >= 1.0 && u.owner_flags & hidden_aircraft == 0 {
                let bp = self.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16));
                let hovering = bp
                    .motion
                    .is_some_and(|m| m.layer == mc_data::MoveLayer::Air && m.hover);
                let parked_transport = bp.transport.is_some()
                    && u.pos[2] <= self.ground_height(glam::Vec2::new(u.pos[0],u.pos[1])) + 1.0;
                if hovering && !parked_transport {
                    let radius = bp.radius.to_f32();
                    self.air_downwash(u, radius, time);
                }
            }
            // On a lift ship's ramp or deck it leaves no prints and throws no dirt.
            if !moving || u._pad3[0] & mc_sim::mirror::UNIT_ON_DECK != 0 {
                continue;
            }
            if let Some(legs) = self.legs.get(u.blueprint as usize).copied().flatten() {
                self.footfall(u, &legs, time, close, dust && close);
                continue;
            }
            let Some(treads) = self.treads.get(u.blueprint as usize).copied().flatten() else {
                continue;
            };
            let (from, to) = (Vec3::from(u.prev_pos), Vec3::from(u.pos));
            let moved = to.truncate().distance(from.truncate());
            // Afloat (a Mason riding the surface, sea under it) the tracks touch
            // nothing: no prints, no dust. Its wake comes from the water effects.
            if !(0.05..=40.0).contains(&moved)
                || to.truncate().distance(focus) > reach
                || to.z < water
                || self.ground_height(to.truncate()) < water - 0.15
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
        (self.effect_origin, self.effect_settings) = previous;
    }

    /// Dust under a hovercraft: a cushion at rest, thrown back when it moves.
    fn hover_downwash(&mut self, u: &UnitInstance, time: f32, moving: bool) {
        let at = Vec3::from(u.pos);
        let forward = Vec3::new(u.heading.cos(), u.heading.sin(), 0.0);
        let left = Vec3::new(-forward.y, forward.x, 0.0);
        let r = u.radius * 0.55;
        let n = if moving { 3 } else { 1 };
        let speed = if moving {
            Vec3::from(u.pos)
                .truncate()
                .distance(Vec3::from(u.prev_pos).truncate())
                / self.tick_seconds.max(0.02)
        } else {
            0.0
        };
        for _ in 0..n {
            let k = self.scatter.unit();
            let pos = at
                + forward * (self.scatter.signed() * r * 0.85)
                + left * (self.scatter.signed() * r)
                + Vec3::Z * 0.12;
            let vel = forward * (-speed * 0.16)
                + left * (self.scatter.signed() * if moving { 1.1 } else { 0.35 })
                + Vec3::Z * (0.45 + self.scatter.unit() * 0.7);
            let life = 0.65 + self.scatter.unit() * 0.45;
            self.push_puff(
                PUFF_DUST,
                pos,
                vel,
                time + k * self.tick_seconds,
                life,
                (0.4, 1.35 + speed * 0.02),
            );
        }
    }

    /// The wash of a hovering aircraft's lift on the ground under it: a ring of
    /// dust driven outward, spray over water, stronger the lower it hangs. The
    /// Osprey works at 22 m and raises a storm; the Kestrel at 65 m only stirs
    /// the grass. Nothing from high up.
    fn air_downwash(&mut self, u: &UnitInstance, radius: f32, time: f32) {
        let at = Vec3::from(u.pos);
        let ground = self.ground_height(at.truncate());
        let water = self.map_info.water_level.to_f32();
        let surface = ground.max(water);
        let height = at.z - surface;
        let strength = (1.0 - height / 95.0).clamp(0.0, 1.0);
        if strength <= 0.05 {
            return;
        }
        let wet = ground < water - 0.2;
        let scale = (radius / 8.0).clamp(0.5, 1.5);
        let ring = radius * (0.7 + height * 0.025);
        let n = 3 + (strength * 6.0) as usize;
        for _ in 0..n {
            let a = self.scatter.unit() * std::f32::consts::TAU;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            let r = ring * (0.35 + self.scatter.unit() * 0.75);
            let pos = Vec3::new(at.x, at.y, surface) + out * r + Vec3::Z * if wet { 0.5 } else { 0.35 };
            // Driven out along the ground, rolling up a little at the ring's edge.
            let vel = out * (4.0 + 10.0 * strength + self.scatter.unit() * 3.0)
                + Vec3::Z * (0.5 + self.scatter.unit() * 1.2 * strength);
            let life = 1.2 + self.scatter.unit() * 0.8 + strength * 0.8;
            let kind = if wet { water_fx::PUFF_SPRAY } else { PUFF_DUST };
            let start = time + self.scatter.unit() * self.tick_seconds;
            self.push_puff(
                kind,
                pos,
                vel,
                start,
                life,
                ((1.0 + 1.2 * strength) * scale, (3.0 + 3.5 * strength) * scale),
            );
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

    /// A reactor going up, a commander's or a power plant's: a flash that whites the
    /// screen out, a fireball that climbs into a cloud on a stalk, a shock front across
    /// the ground out to the edge of the blast (`blast` metres), and a column of smoke
    /// that stands for a long time. `r` is the size of the fireball's puffs; everything
    /// else scales with the blast, the commander's 140 m being the reference.
    fn reactor_death(&mut self, at: Vec3, r: f32, h: f32, blast: f32, time: f32) {
        let core = at + Vec3::Z * h * 0.5;
        const BLAST_REF: f32 = 140.0;
        let s = blast / BLAST_REF;
        // Fewer, not only smaller, puffs for a small plant: a row of them going up
        // must not eat the puff budget.
        let n = |count: u32| ((count as f32 * s.sqrt().clamp(0.35, 1.0)).ceil()) as u32;
                // The flash: twice, the second broader and slower, so it blinds and then lingers.
        self.push_effect(core.to_array(), time, 420.0 * s, 0.55, 4.0, 0.0);
        self.push_effect(core.to_array(), time + 0.05, 260.0 * s, 1.9, 4.0, 1.0);
        // The shock front along the ground, and a second behind it.
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.05,
            blast * 1.25,
            1.1,
            2.0,
            1.0,
        );
        self.push_effect(
            (at + Vec3::Z * 2.0).to_array(),
            time + 0.35,
            blast * 0.9,
            1.4,
            2.0,
            1.0,
        );
        // The fireball, rolling upward.
        for i in 0..7 {
            let rise = i as f32 * 7.0 * s;
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
        for i in 0..n(26) {
            let off = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                self.scatter.unit(),
            ) * r
                * 2.2;
            let vel = self.scatter.upward(0.3) * (14.0 + self.scatter.unit() * 22.0) * s.sqrt();
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
        let stalk = n(22);
        for i in 0..stalk {
            let z = (4.0 + i as f32 * 3.2 * 22.0 / stalk as f32) * s;
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.7
                + Vec3::Z * z;
            let kind = if i % 3 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 5.0 + self.scatter.unit() * 3.0;
            let climb = Vec3::Z * (6.0 + self.scatter.unit() * 5.0) * s;
            self.push_puff(
                kind,
                at + off,
                climb,
                time + 0.3 + z / 40.0,
                life,
                (r * 1.4, r * 3.2),
            );
        }
        let cap = n(28);
        for i in 0..cap {
            let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / cap as f32;
            let ring = Vec3::new(a.cos(), a.sin(), 0.0) * r * (2.0 + self.scatter.unit() * 2.4);
            let top = at + ring + Vec3::Z * (72.0 + self.scatter.signed() * 7.0) * s;
            let roll = (ring.normalize_or_zero() * (5.0 + self.scatter.unit() * 5.0)
                + Vec3::Z * (2.0 + self.scatter.unit() * 3.0))
                * s;
            let kind = if i % 4 == 0 {
                PUFF_FIREBALL
            } else {
                PUFF_SMOKE
            };
            let life = 6.0 + self.scatter.unit() * 3.5;
            let start = time + 2.0 * s.sqrt() + self.scatter.unit() * 0.5;
            self.push_puff(kind, top, roll, start, life, (r * 2.4, r * 6.0));
        }
        // Dust driven out flat ahead of the shock, all the way to the edge of the blast.
        for ring in 0..3 {
            let around = n(30);
            for i in 0..around {
                let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / around as f32;
                let out = Vec3::new(a.cos(), a.sin(), 0.03);
                let from = blast * (0.12 + 0.3 * ring as f32);
                let life = 2.2 + self.scatter.unit() * 1.6;
                let push = out * (60.0 + self.scatter.unit() * 30.0) * s.sqrt();
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
        for _ in 0..n(110) {
            let vel = self.scatter.upward(0.1) * (25.0 + self.scatter.unit() * 70.0) * s.sqrt();
            let life = 1.0 + self.scatter.unit() * 2.2;
            let (start, size) = (
                time + self.scatter.unit() * 0.1,
                0.5 + self.scatter.unit() * 0.5,
            );
            self.push_puff(PUFF_SPARK, core, vel, start, life, (size, 0.1));
        }
        for _ in 0..n(40) {
            let vel = self.scatter.upward(0.3) * (18.0 + self.scatter.unit() * 35.0) * s.sqrt();
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

    /// Missile smoke, white conventional wakes, and blue energy-slug trails.
    /// Paths are recorded even when the camera is too far to draw them, so
    /// zooming in lights the trail already flown, not a fresh ribbon from here.
    fn missile_trails(&mut self, projectiles: &[ProjectileInstance], time: f32, camera: &Camera) {
        let close = camera.distance <= 2200.0;
        let reach = camera.distance * 2.8 + 360.0;
        let focus = camera.focus.truncate();
        let n = if camera.distance > 900.0 { 4 } else { 6 };
        let mut was = std::mem::take(&mut self.trail_paths);
        let mut now = HashMap::with_capacity(was.len());
        for p in projectiles {
            if p.color & (PROJECTILE_BEAM | PROJECTILE_FADE_BEAM) != 0 {
                continue;
            }
            if p.color & PROJECTILE_COLD != 0 {
                continue;
            }
            let missile = p.color & PROJECTILE_MISSILE != 0;
            let arc = p.color & PROJECTILE_TRAIL != 0;
            let smoke = p.color & PROJECTILE_SMOKE != 0;
            if !missile && !arc && !smoke {
                continue;
            }
            let (from, to) = (Vec3::from(p.prev_pos), Vec3::from(p.pos));
            let ends = ((p.color >> PROJECTILE_ENDS_SHIFT) & 0xFF) as f32 / 255.0;
            let duration = if ends > 0.0 {
                ends * self.tick_seconds
            } else {
                self.tick_seconds
            };
            let mut path = was.remove(&trail_key(from)).unwrap_or_else(|| TrailPath {
                points: vec![(from, time)],
                spawned: 0,
            });
            path.points.push((to, time + duration));
            // Keep only what the ribbon still shows: older puffs have died.
            let keep = if arc || smoke {
                (p.wake + 0.55).max(1.45)
            } else {
                4.2
            };
            while path.points.len() >= 2 && time - path.points[1].1 > keep {
                path.points.remove(0);
                path.spawned = path.spawned.saturating_sub(1);
            }
            let in_view = close && to.truncate().distance(focus) <= reach;
            let skim = missile && p.color & PROJECTILE_SKIM != 0;
            let apogee = missile && p.color & PROJECTILE_APOGEE != 0;
            // A high-arc missile's motor is out once it is over the top: no trail, no
            // exhaust; the nose glows coming down instead (`arc_missile_flight`).
            let falling = apogee && to.z < from.z;
            if in_view && skim {
                self.skimmer_over_sea(from, to, time, duration, p);
            }
            if in_view && apogee {
                self.arc_missile_flight(from, to, time, duration, p, falling);
            }
            if falling {
                // Nothing to lay: keep the cursor on the head so the trail never catches up later.
                path.spawned = path.points.len();
            }
            if in_view && !falling {
                let fresh = p.color & PROJECTILE_FRESH != 0;
                let puffs = if fresh && path.points.len() <= 2 {
                    6
                } else {
                    n
                };
                let first = if path.spawned == 0 {
                    0
                } else {
                    path.spawned - 1
                };
                let last = path.points.len().saturating_sub(1);
                for i in first..last {
                    let (a, t_a) = path.points[i];
                    let (b, t_b) = path.points[i + 1];
                    let (seg_time, seg_dur) = if i + 1 == last {
                        (time, duration)
                    } else {
                        (t_a, (t_b - t_a).max(0.001))
                    };
                    if seg_time + keep < time {
                        continue;
                    }
                    let engine = missile && i + 1 == last;
                    self.emit_trail_segment(a, b, seg_time, seg_dur, puffs, arc, engine, p);
                }
                path.spawned = path.points.len();
                if arc && p.plasma > 0.0 {
                    let dir = (to - from).normalize_or_zero();
                    self.emit_plasma_head(to, dir, time + duration, p.plasma);
                }
            }
            now.insert(trail_key(to), path);
        }
        self.trail_paths = now;
    }

    /// A sea skimmer (`PROJECTILE_SKIM`) running in low: within a few metres of the sea,
    /// its exhaust tears a line of spray off the water under it.
    fn skimmer_over_sea(&mut self, from: Vec3, to: Vec3, time: f32, duration: f32, p: &ProjectileInstance) {
        let Some(water) = self.at_sea(to, 6.0) else {
            return;
        };
        let dir = (to - from).normalize_or_zero();
        let side = Vec3::new(-dir.y, dir.x, 0.0);
        let low = (1.0 - (to.z - water) / 6.0).clamp(0.2, 1.0);
        let s = (0.5 + p.size * 0.3) * low;
        for i in 0..3 {
            let along = (i as f32 + 0.5) / 3.0;
            let at = from.lerp(to, along);
            let at = Vec3::new(at.x, at.y, water + 0.25);
            let start = time + along * duration;
            self.push_puff(water_fx::PUFF_SPRAY, at, dir * 3.0 + Vec3::Z * 0.8, start, 0.55, (s, s * 2.6));
            for sign in [-1.0f32, 1.0] {
                let vel = side * sign * (3.0 + self.scatter.unit() * 3.0) + dir * 4.0 + Vec3::Z * (1.5 + self.scatter.unit() * 2.0) * low;
                self.push_puff(water_fx::PUFF_DROPLET, at + side * sign * 0.6, vel, start, 1.2, (0.14, 0.32));
            }
        }
    }

    /// A high-arc missile (`PROJECTILE_APOGEE`): boosting up, a long white-hot exhaust
    /// behind it (the trail's column is `emit_trail_segment`'s); falling, the motor is
    /// out and the air it falls through heats the nose, brighter the lower it gets,
    /// streaking fire back off it.
    fn arc_missile_flight(&mut self, from: Vec3, to: Vec3, time: f32, duration: f32, p: &ProjectileInstance, falling: bool) {
        let dir = (to - from).normalize_or_zero();
        let half = missile_half_length(p.size);
        let when = time + duration;
        if !falling {
            // Fire is additive and whites out when it stacks, so the flame is small and short
            // and the body of the exhaust is the fireball behind it.
            let tail = to - dir * half;
            self.push_puff(PUFF_FIRE, tail, -dir * 22.0, when, 0.18, (0.5 + p.size * 0.12, 1.1 + p.size * 0.25));
            self.push_puff(PUFF_FIREBALL, tail - dir * 2.0, -dir * 9.0, when, 0.3, (0.8 + p.size * 0.2, 2.0 + p.size * 0.4));
            self.push_effect((tail - dir).to_array(), when, 1.6 + p.size * 0.5, 0.25, 1.0, 0.0);
            return;
        }
        let floor = self.ground_height(to.truncate()).max(self.sea_level());
        let heat = (1.0 - (to.z - floor) / 700.0).clamp(0.15, 1.0);
        let nose = to + dir * half * 0.8;
        self.push_effect(nose.to_array(), when, (1.0 + p.size * 0.5) * (0.5 + 1.5 * heat), 0.3, 6.0, 0.0);
        for i in 0..3 {
            let back = (i as f32 + 0.5) * (1.5 + heat * 2.5);
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()) * 0.3;
            let s = 0.4 + heat * 0.7;
            self.push_puff(PUFF_FIRE, nose - dir * back + off, -dir * 12.0 * heat, when, 0.22, (s, s * (2.0 + heat * 1.5)));
        }
    }

    fn emit_trail_segment(
        &mut self,
        from: Vec3,
        to: Vec3,
        time: f32,
        duration: f32,
        n: u32,
        arc: bool,
        engine: bool,
        p: &ProjectileInstance,
    ) {
        let dir = (to - from).normalize_or_zero();
        let step = (to - from).length();
        let tail = if p.color & PROJECTILE_MISSILE != 0 { missile_half_length(p.size) } else { 0.12 };
        // A heavy missile trail (one with a `wake`) is a solid column, not a dotted line.
        // A sea skimmer keeps a thin one however heavy: it is fast and low, not a booster.
        let heavy = p.color & PROJECTILE_MISSILE != 0 && p.wake > 0.0 && p.color & PROJECTILE_SKIM == 0;
        let n = if heavy { n.max(1) * 3 } else { n.max(1) };
        for i in 0..n {
            let along = (i as f32 + 0.5) / n as f32;
            let at = from.lerp(to, along) - dir * tail;
            // Tangent for the ribbon; the puff hangs where it was born.
            // Light it only once the shot has passed this stretch, or the
            // wake pops in a whole tick ahead of the interpolating slug.
            let vel = dir * (step / n as f32);
            let start = time + ((i as f32 + 1.0) / n as f32) * duration;
            if p.color & (PROJECTILE_SMOKE | PROJECTILE_MISSILE) != 0 {
                let life = if p.wake > 0.0 { p.wake } else { 1.2 };
                // A missile given a `wake` hangs a heavy, billowing column that long; a
                // booster climbing to its apogee a bigger one still.
                let boost = p.color & PROJECTILE_APOGEE != 0;
                let (size, grow) = if heavy && boost {
                    ((1.2 + p.size * 0.4).min(2.8), 3.5)
                } else if heavy {
                    ((1.3 + p.size * 0.45).min(3.2), 4.0)
                } else {
                    ((0.35 + p.size * 0.2).min(0.65), 1.5)
                };
                self.push_puff(PUFF_BOMB_TRAIL, at, vel, start, life, (size, size * grow));
            } else if arc {
                let life = if p.wake > 0.0 {
                    p.wake + self.scatter.unit() * 0.08
                } else {
                    0.85 + self.scatter.unit() * 0.25
                };
                // A small direct-fire slug (the Bulwark) keeps a thin, dim thread.
                let faint = p.wake > 0.0 && p.size < 1.0;
                let s = if faint {
                    (0.08 + p.size * 0.05).min(0.16)
                } else {
                    (0.62 + p.size * 0.34).min(1.55)
                };
                let end = if faint { -s * 1.4 } else { s * 2.4 };
                self.push_puff(PUFF_ARC, at, vel, start, life, (s, end));
                if p.plasma > 0.0 {
                    self.emit_plasma_around(at, dir, vel, start, p.plasma);
                }
            } else {
                let life = 3.4 + self.scatter.unit() * 0.6;
                self.push_puff(PUFF_TRAIL, at, vel, start, life, (0.36, 1.05));
            }
        }
        if engine {
            self.push_puff(
                PUFF_FIRE,
                to - dir * (tail + 0.35),
                -dir * 5.0,
                time + duration,
                0.28,
                (0.22, 0.55),
            );
        }
    }

    /// Extra plasma around an energy slug: soft discs beside the wake, not instead of it.
    fn emit_plasma_around(&mut self, at: Vec3, dir: Vec3, vel: Vec3, start: f32, plasma: f32) {
        let mut perp = dir.cross(Vec3::Z);
        if perp.length_squared() < 0.04 {
            perp = dir.cross(Vec3::X);
        }
        let perp = perp.normalize_or_zero();
        let up = dir.cross(perp).normalize_or_zero();
        for k in 0..6 {
            let a = (k as f32 + self.scatter.unit() * 0.18) * std::f32::consts::TAU / 6.0;
            let r = plasma * (0.22 + self.scatter.unit() * 0.16);
            let off = (perp * a.cos() + up * a.sin()) * r;
            let life = 0.2 + self.scatter.unit() * 0.1;
            let s = plasma * (0.28 + self.scatter.unit() * 0.1);
            self.push_puff(PUFF_PLASMA, at + off, vel * 0.2, start, life, (s, s * 1.65));
        }
        let knot = plasma * 0.4;
        let knot_life = 0.16 + self.scatter.unit() * 0.06;
        self.push_puff(
            PUFF_PLASMA,
            at,
            vel * 0.12,
            start,
            knot_life,
            (knot, knot * 1.55),
        );
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

    /// Hitscan that never found a burst this tick: the beam still runs to range
    /// and dumps a thinner split, so a miss is seen as a shot, not a blank muzzle.
    fn flush_shatter_misses(&mut self, time: f32) {
        let pending = std::mem::take(&mut self.pending_shatter);
        for shot in pending {
            let burst = shot.muzzle + shot.dir * shot.range;
            self.spawn_shatter_split(&shot, burst, Vec3::ZERO, time, time, true);
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
                color: if beam.laser {
                    PROJECTILE_FADE_BEAM | 1
                } else {
                    PROJECTILE_FADE_BEAM
                },
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

    fn take_pending_shatter(&mut self, burst: Vec3) -> Option<PendingShatter> {
        let mut best_i = None;
        let mut best_score = 56.0_f32;
        for (i, shot) in self.pending_shatter.iter().enumerate() {
            let Some(score) = beam_score(shot.muzzle, shot.dir, shot.range, burst) else {
                continue;
            };
            if score < best_score {
                best_score = score;
                best_i = Some(i);
            }
        }
        best_i.map(|i| self.pending_shatter.swap_remove(i))
    }

    /// A hitscan shot's beam, muzzle to `to`: a hot core that is gone almost at once,
    /// inside the ionised channel it leaves hanging a moment longer.
    fn rail_beam(&mut self, from: Vec3, to: Vec3, width: f32, time: f32) {
        self.fade_beams.push(FadeBeam { from, to, start: time, life: 0.14, width: width * 1.8, laser: false });
        self.fade_beams.push(FadeBeam { from, to, start: time, life: 0.55, width, laser: false });
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
            self.rail_beam(shot.muzzle, at, shot.width, time);
        }
    }

    /// Hitscan that struck nothing this tick: the beam still runs out to range.
    fn flush_rail_misses(&mut self, time: f32) {
        for shot in std::mem::take(&mut self.pending_rail) {
            self.rail_beam(shot.muzzle, shot.muzzle + shot.dir * shot.range, shot.width, time);
        }
    }

    /// Beam from the muzzle to the burst, then a cone of plasma bolts continuing
    /// forward — the shot that exploded just short of the target.
    fn shatter_burst(&mut self, burst: Vec3, motion: Vec3, after: f32, beam_start: f32, split_start: f32) {
        let shot = self.take_pending_shatter(burst).unwrap_or(PendingShatter {
            effects: self.effect_settings,
            muzzle: burst,
            dir: Vec3::Z,
            range: 0.0,
            bolts: 6,
            splash: 26.0,
            width: 1.8,
            impact: 1.2,
            shockwave: 0.8,
            color: 0.0,
        });
        let target = shatter_target_at(burst, motion, after, self.tick_seconds, 0.0);
        let velocity = motion / self.tick_seconds.max(0.001);
        self.spawn_shatter_split(&shot, target, velocity, beam_start, split_start, false);
    }

    fn spawn_shatter_split(
        &mut self, shot: &PendingShatter, burst: Vec3, target_velocity: Vec3,
        beam_start: f32, split_start: f32, miss: bool,
    ) {
        let previous = (self.effect_origin, self.effect_settings);
        self.effect_origin = Some(burst);
        self.effect_settings = shot.effects;
        self.spawn_shatter_split_inner(shot, burst, target_velocity, beam_start, split_start, miss);
        (self.effect_origin, self.effect_settings) = previous;
    }

    fn spawn_shatter_split_inner(
        &mut self,
        shot: &PendingShatter,
        burst: Vec3,
        target_velocity: Vec3,
        beam_start: f32,
        split_start: f32,
        miss: bool,
    ) {
        let target = burst;
        let (burst, forward) = shatter_airburst(shot.muzzle, target, shot.dir, shot.splash);
        let has_beam = shot.muzzle.distance(burst) > 2.0;
        if has_beam {
            self.fade_beams.push(FadeBeam {
                from: shot.muzzle,
                to: burst,
                start: beam_start,
                life: 0.2,
                width: shot.width,
                laser: false,
            });
        }
        let scale = if miss { 0.45 } else { 1.0 };
        let detail_scale = shatter_detail_scale(shot.impact) * scale;
        self.push_effect(
            burst.to_array(),
            split_start,
            (12.0 * shot.impact) * scale,
            0.26,
            shot.color,
            0.0,
        );
        self.push_effect(
            burst.to_array(),
            split_start,
            (5.0 * shot.impact) * scale,
            0.12,
            shot.color,
            0.0,
        );
        // A ragged expanding plasma explosion marks the split, with bright
        // lobes and debris that remain legible after the thin beam fades.
        for _ in 0..shot.bolts.clamp(3, 9) {
            let dir = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()).normalize_or_zero();
            self.push_puff(PUFF_SHATTER_BLAST, burst + dir * shot.impact, dir * (36.0 * detail_scale),
                split_start, 0.34, (2.2 * shot.impact * scale, 6.0 * shot.impact * scale));
        }
        if shot.shockwave > 0.0 && !miss {
            self.push_shockwave(
                burst.to_array(),
                split_start,
                (14.0 + shot.impact * 6.0) * shot.shockwave,
                0.42,
                (shot.shockwave * 0.75).min(1.0),
                shot.color,
                shot.dir,
            );
        }
        let n = shatter_fragment_count(shot.bolts, miss);
        let side = forward.cross(if forward.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z }).normalize_or_zero();
        let up = side.cross(forward).normalize_or_zero();
        for i in 0..n {
            // Fill the target volume, with a guaranteed central strike and
            // irregular outer detonations instead of a uniform radial star.
            let angle = i as f32 * 2.399963 + self.scatter.signed() * 0.22;
            let radius = if i == 0 { 0.0 } else { (i as f32 / (n - 1) as f32).sqrt() * shot.splash };
            let depth = if i == 0 { 0.0 } else { self.scatter.signed() * shot.splash * 0.65 };
            let life = SHATTER_FRAGMENT_MIN_LIFE
                + self.scatter.unit() * (SHATTER_FRAGMENT_MAX_LIFE - SHATTER_FRAGMENT_MIN_LIFE);
            let mut end = target + target_velocity * life
                + (side * angle.cos() + up * angle.sin()) * radius + forward * depth;
            end.z = end.z.max(self.ground_height(end.truncate()) + 0.5);
            self.push_puff(PUFF_PLASMA_BOLT, burst, (end - burst) / life, split_start, life, (0.65 * detail_scale, 0.24 * detail_scale));
            if !miss {
                let arrival = split_start + life;
                let size = shot.impact * (6.0 + self.scatter.unit() * 4.5);
                self.push_effect(end.to_array(), arrival, size, 0.3, shot.color, 0.0);
                for _ in 0..3 {
                    let dir = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed());
                    self.push_puff(PUFF_SHATTER_BLAST, end + dir * size * 0.15, dir * (20.0 * detail_scale),
                        arrival, 0.32, (size * 0.18, size * 0.48));
                }
                // Every fragment detonation carries its own blue pressure front.
                if shot.shockwave > 0.0 {
                    self.push_shockwave(end.to_array(), arrival, size * 1.8, 0.38,
                        shot.shockwave.min(1.0), shot.color, Vec3::ZERO);
                }
                self.push_effect(end.to_array(), arrival, size * 0.38, 0.08, shot.color, 0.0);
                for _ in 0..5 {
                    let velocity = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()) * (34.0 * detail_scale);
                    self.push_puff(PUFF_BOLT, end, velocity, arrival, 0.22, (0.35 * detail_scale, 0.04 * detail_scale));
                }
            }
        }
        let sparks = if miss { 6 } else { shot.bolts.max(3) + 2 };
        for _ in 0..sparks {
            let spray = (shot.dir * 0.7
                + Vec3::new(
                    self.scatter.signed(),
                    self.scatter.signed(),
                    self.scatter.signed(),
                ) * 0.55)
                .normalize_or_zero();
            let speed = (18.0 + self.scatter.unit() * 28.0) * detail_scale;
            let life = 0.1 + self.scatter.unit() * 0.1;
            self.push_puff(
                PUFF_BOLT,
                burst,
                spray * speed,
                split_start,
                life,
                (0.28 * detail_scale, 0.06 * detail_scale),
            );
        }
        if !miss {
            for _ in 0..6 {
                let spray = (shot.dir * 0.35
                    + Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.signed(),
                    ) * 0.7)
                    .normalize_or_zero();
                let speed = (22.0 + self.scatter.unit() * 30.0) * detail_scale;
                self.push_puff(
                    PUFF_SPLINTER,
                    burst,
                    spray * speed,
                    split_start,
                    0.2,
                    (0.55 * detail_scale, 0.1 * detail_scale),
                );
            }
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
            | SimEvent::Reclaimed { pos, blueprint, .. } => (Some(Vec3::from(pos.to_f32())), Some(*blueprint)),
            SimEvent::ShieldBroken { pos, .. } => (Some(Vec3::from(pos.to_f32())), None),
            SimEvent::MissileLased { to, .. } => (Some(Vec3::from(to.to_f32())), None),
            _ => (None, None),
        };
        let previous = (self.effect_origin, self.effect_settings);
        self.effect_origin = origin;
        self.effect_settings = blueprint.and_then(|id| self.blueprints.units.get(id.0 as usize))
            .map_or_else(mc_data::EffectSettings::default, |bp| bp.visual.effects);
        if let SimEvent::ShotFired { blueprint, weapon, .. }
            | SimEvent::Impact { blueprint, weapon, .. } = event
        {
            if self.blueprints.unit(*blueprint).weapons[*weapon as usize].bore.is_some() {
                self.effect_settings.shockwave_color = Some([0.24, 0.62, 1.0]);
            }
        }
        self.effects_of_inner(event, time);
        (self.effect_origin, self.effect_settings) = previous;
    }

    /// A weapon charging before it fires (`Weapon::charge_ticks`): a knot of its colour
    /// growing at each muzzle over the charge, lighting what is round it, with sparks
    /// drawn in along the barrel toward the mouth. The sim names the hull and weapon
    /// only; where the gun is and how its house is turned come from the last tick's
    /// mirror (`note_gun_hulls`), or failing that the muzzle in the hull's frame.
    fn weapon_charging(&mut self, at: Vec3, blueprint: mc_data::BlueprintId, weapon: u8, time: f32) {
        let blueprints = self.blueprints.clone();
        let w = &blueprints.unit(blueprint).weapons[weapon as usize];
        let seconds = w.charge_ticks as f32 * self.tick_seconds.max(0.02);
        if seconds < 0.15 {
            return;
        }
        let tint = if w.color == mc_data::WeaponColor::Blue { 0.0 } else { 1.0 };
        // The hull: the nearest of that blueprint to where the sim says it is.
        let hull = self
            .water_fx
            .guns
            .iter()
            .filter(|g| g.blueprint == blueprint.0 as u32)
            .min_by(|a, b| {
                let (da, db) = (a.pos.truncate().distance_squared(at.truncate()), b.pos.truncate().distance_squared(at.truncate()));
                da.total_cmp(&db)
            });
        let heading = hull.map_or(0.0, |g| g.heading);
        let base = hull.map_or(at - Vec3::Z * w.muzzle.z.to_f32(), |g| g.pos);
        let pose = hull.and_then(|g| g.house).map(|h| h.pose[weapon as usize]);
        let (yaw, pitch) = pose.map_or((0.0, 0.0), |p| (p[1], p[3]));
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
            let on_hull = if pose.is_some() { pivot + rot_z(rot_xz(local - pivot, pitch), yaw) } else { local };
            base + rot_z(on_hull, heading)
        };
        let dir = rot_z(if pose.is_some() { rot_z(rot_xz(Vec3::X, pitch), yaw) } else { Vec3::X }, heading);
        let power = w.damage.to_f32().max(1.0).sqrt();
        let full = (0.6 + power * 0.05).min(3.5) * w.flash.max(0.5);
        let steps = ((seconds / 0.16).ceil() as usize).clamp(3, 24);
        let mouths: Vec<Vec3> = if w.muzzles.is_empty() {
            vec![Vec3::from(w.muzzle.to_f32())]
        } else {
            w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect()
        };
        for local in mouths {
            let mouth = placed(local);
            for k in 0..steps {
                let f = k as f32 / (steps - 1).max(1) as f32;
                let start = time + f * (seconds - 0.1);
                // Slow to build, then quick: most of the glow comes in the last second.
                let r = full * (0.15 + 0.85 * f * f);
                self.push_effect((mouth + dir * 0.3).to_array(), start, r, 0.2 + seconds / steps as f32, tint, 0.0);
                for _ in 0..1 + (f * 2.0) as usize {
                    let back = 1.5 + self.scatter.unit() * (4.0 + power * 0.08);
                    let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), self.scatter.signed()) * (0.3 + f * 0.5);
                    let life = 0.18 + self.scatter.unit() * 0.12;
                    let from = mouth - dir * back + off;
                    let when = start + self.scatter.unit() * 0.1;
                    self.push_puff(PUFF_BOLT, from, dir * (back / life) * 0.8, when, life, (0.14 + f * 0.12, 0.05));
                }
            }
        }
    }

    fn effects_of_inner(&mut self, event: &SimEvent, time: f32) {
        if self.sea_effects_of(event, time) {
            return;
        }
        match event {
            SimEvent::BoreDischarge { from, to, width, after, blueprint, weapon, .. } => {
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let (splash, cool) = (w.splash.to_f32(), w.bore.map_or(10.0, |b| b.cool));
                self.bore_discharge(
                    Vec3::from(from.to_f32()),
                    Vec3::from(to.to_f32()),
                    width.to_f32(),
                    splash,
                    cool,
                    after.to_f32(),
                    time,
                );
            }
            SimEvent::WeaponCharging { pos, blueprint, weapon, .. } => {
                self.weapon_charging(Vec3::from(pos.to_f32()), *blueprint, *weapon, time);
            }
            SimEvent::MissileLased { from, to, killed } => {
                let origin = Vec3::from(from.to_f32());
                let at = Vec3::from(to.to_f32());
                self.fade_beams.push(FadeBeam {
                    from: origin,
                    to: at,
                    start: time,
                    life: 0.16,
                    width: 0.22,
                    laser: true,
                });
                let back = (origin - at).normalize_or_zero();
                if !*killed {
                    // Still burning: a hot knot and sparks kicking back along the beam.
                    self.push_effect(at.to_array(), time, 1.15, 0.07, 1.0, 0.35);
                    for _ in 0..3 {
                        let spray = (back
                            + Vec3::new(
                                self.scatter.signed(),
                                self.scatter.signed(),
                                self.scatter.signed(),
                            ) * 0.55)
                            .normalize_or_zero();
                        let speed = 16.0 + self.scatter.unit() * 22.0;
                        self.push_puff(PUFF_SPARK, at, spray * speed, time, 0.16, (0.16, 0.04));
                    }
                }
                if *killed {
                    // A defense kill, not a shell hit: a white star, an orange ring,
                    // and the casing thrown off the beam.
                    if let Some(beam) = self.fade_beams.last_mut() {
                        beam.life = 0.32;
                        beam.width = 0.34;
                    }
                    self.push_effect(at.to_array(), time, 26.0, 0.42, 6.0, 0.0);
                    self.push_shockwave(at.to_array(), time, 18.0, 0.34, 0.85, 1.0, Vec3::ZERO);
                    let axis = if back.length_squared() > 1e-4 { back } else { Vec3::Z };
                    let side = axis.cross(if axis.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z }).normalize_or_zero();
                    let up = side.cross(axis).normalize_or_zero();
                    for i in 0..10 {
                        let ang = i as f32 * 2.399963;
                        let dir = (side * ang.cos() + up * ang.sin() + axis * self.scatter.signed() * 0.35)
                            .normalize_or_zero();
                        let speed = 28.0 + self.scatter.unit() * 36.0;
                        self.push_puff(PUFF_SPARK, at, dir * speed, time, 0.32, (0.28, 0.07));
                    }
                    for _ in 0..8 {
                        let dir = (side * self.scatter.signed() + up * self.scatter.signed() + axis * self.scatter.signed())
                            .normalize_or_zero();
                        let speed = 16.0 + self.scatter.unit() * 22.0;
                        self.push_puff(PUFF_SHARD, at, dir * speed, time, 0.45, (0.42, 0.1));
                    }
                    self.push_puff(PUFF_SMOKE, at, axis * 4.0 + Vec3::Z * 1.5, time, 0.8, (0.7, 1.6));
                }
            }
            SimEvent::AircraftLaunched { pos, heading, .. } => {
                // Fired out of a launch tunnel: the catapult's flash in the portal, a gout of
                // dust and exhaust blown out along the lane, and grit off the ground either side.
                let a = heading.0 as f32 * (std::f32::consts::TAU / 65536.0);
                let dir = Vec3::new(a.cos(), a.sin(), 0.0);
                let side = Vec3::new(-dir.y, dir.x, 0.0);
                let mouth = Vec3::from(pos.to_f32()) + Vec3::Z * 2.0;
                // The event comes as the catapult fires at the back of the tunnel; the
                // aircraft is at the mouth a moment later.
                let time = time + 0.35;
                self.push_effect(mouth.to_array(), time, 9.0, 0.16, 2.0, 0.6);
                self.push_shockwave(mouth.to_array(), time, 36.0, 0.4, 0.6, 1.0, dir);
                for i in 0..10 {
                    let spread = self.scatter.signed();
                    let vel = dir * (22.0 + 4.0 * i as f32) + side * spread * 9.0 + Vec3::Z * (3.0 + self.scatter.unit() * 5.0);
                    let kind = if i % 3 == 0 { PUFF_SMOKE } else { PUFF_DUST };
                    self.push_puff(kind, mouth, vel, time + 0.015 * i as f32, 1.1 + 0.12 * i as f32, (2.2, 6.0 + 0.4 * i as f32));
                }
                for s in [-1.0, 1.0] {
                    for i in 0..3 {
                        let vel = side * s * (14.0 + 5.0 * i as f32) + dir * 6.0 + Vec3::Z * 2.0;
                        self.push_puff(PUFF_DUST, mouth - Vec3::Z * 1.5, vel, time + 0.03 * i as f32, 1.4, (1.8, 4.5));
                    }
                }
            }
            SimEvent::MissileIgnited { pos, vel, blueprint, weapon } => {
                let w = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                let size = (0.3 + w.damage.to_f32().sqrt() * 0.045 + w.splash.to_f32() * 0.07) * w.tracer;
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                let tail = Vec3::from(pos.to_f32()) - dir * missile_half_length(size);
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
                    let kind = if i % 3 == 0 { PUFF_FIREBALL } else if i % 3 == 1 { PUFF_FIRE } else { PUFF_SMOKE };
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
                blueprint,
                weapon,
                ..
            } => {
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
                if weapon.cold_launch_ticks > 0 {
                    let at = Vec3::from(pos.to_f32());
                    // Pneumatic ejection: pressure wave only, no rocket flash.
                    self.push_shockwave(at.to_array(), time, 18.0, 0.35, 0.55, 0.0, Vec3::Z);
                    return;
                }
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let flash = weapon.flash;
                let bore = weapon.bore.is_some();
                let shockwave = weapon.shockwave;
                let missile = weapon.missile;
                let bolts = weapon.bolts;
                let rounds = weapon.rounds;
                let round_gap = mc_sim::mirror::round_gap(weapon) * self.tick_seconds;
                let casings = weapon.casings;
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
                let round_at = |at: Vec3, k: u8| at + Vec3::from(mc_sim::mirror::round_shift(travel.to_array(), k, gap_ticks));
                let at = Vec3::from(pos.to_f32()) - travel;
                let dir = Vec3::from(vel.to_f32()).normalize_or_zero();
                let shell = *color == mc_data::WeaponColor::Orange;
                // A gun whose tracers run red (`Weapon::red`) flashes red too: effect kind 8.
                let tint = if shell && weapon.red > 0.5 { 8.0 } else { *color as u32 as f32 };
                let shatter = is_shatter_gun(weapon);
                if weapon.hitscan && !shatter {
                    self.pending_rail.push(PendingRail {
                        muzzle: at,
                        dir,
                        range: weapon.range_max.to_f32(),
                        width: 0.45 + power * 0.022 * flash,
                    });
                }
                if shatter {
                    self.pending_shatter.push(PendingShatter {
                        effects: self.effect_settings,
                        muzzle: at,
                        dir,
                        range: weapon.range_max.to_f32(),
                        bolts,
                        splash: weapon.splash.to_f32(),
                        width: (0.55 + flash * 0.13) * shatter_detail_scale(weapon.impact),
                        impact: weapon.impact,
                        shockwave,
                        color: *color as u32 as f32,
                    });
                }
                self.push_effect(
                    at.to_array(),
                    time,
                    (1.2 + power * 0.42) * flash,
                    if shell { 0.09 } else { 0.12 },
                    tint,
                    0.0,
                );
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
                    // One casing per round, out of the breech: to the gun's right from
                    // the ground, straight down out of an aircraft.
                    let right = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
                    let breech = at + travel - dir * casings;
                    // They leave at the gun's own speed, then the air takes it off them.
                    let carried = travel / self.tick_seconds.max(0.01);
                    let size = 0.3 + power * 0.03;
                    for k in 0..rounds.max(1) {
                        let jitter = Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.signed(),
                        );
                        let (vel, life) = match flying {
                            Some(speed) => {
                                let ahead = Vec3::new(dir.x, dir.y, 0.0).normalize_or_zero() * speed;
                                let drop = Vec3::Z * -(6.0 + 3.0 * self.scatter.unit());
                                (ahead + drop + right * jitter.x * 2.0 + jitter * 1.2, 1.6)
                            }
                            None => {
                                let throw = right * (4.0 + 3.0 * self.scatter.unit())
                                    + Vec3::Z * (3.0 + 2.0 * self.scatter.unit())
                                    - dir * 1.0;
                                (carried + throw + jitter * 1.1, 2.4 + 0.6 * self.scatter.unit())
                            }
                        };
                        let (from, start) = (round_at(breech, k), time + k as f32 * round_gap);
                        self.push_puff(PUFF_CASING, from, vel, start, life, (size, size));
                        self.casing_splash(from, vel, start, life, k % 2 == 0);
                    }
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
                        if shatter { 0.0 } else { 0.4 },
                    );
                }
                if bolts > 0 {
                    self.push_effect(
                        (at + dir * 1.6).to_array(),
                        time,
                        (0.7 + power * 0.16) * flash,
                        0.14,
                        *color as u32 as f32,
                        if shatter { 0.0 } else { 0.9 },
                    );
                    // Shatter bolts are the split before the target, not a muzzle spray.
                    if !shatter {
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
                }
                if shockwave > 0.0 {
                    // Much bigger than the gun: a howitzer's wave dwarfs the bunker it sits on.
                    let life = if bore {
                        0.8
                    } else if shatter {
                        0.72
                    } else {
                        (0.48 + power * 0.01).min(0.95)
                    };
                    self.push_shockwave(
                        (at + dir * 2.0).to_array(),
                        time,
                        // The bore launches a small tracer: a distinct pressure front,
                        // sized independently of the much more powerful discharge.
                        if bore { (12.0 + power * 0.35) * shockwave }
                        else { (22.0 + power * 2.0) * shockwave },
                        life,
                        shockwave.min(1.0),
                        *color as u32 as f32,
                        dir,
                    );
                }
                if missile {
                    // Exhaust out the back of the tube as the motor lights.
                    for i in 0..4 {
                        let push = -dir * (3.0 + 3.5 * i as f32)
                            + Vec3::new(
                                self.scatter.signed(),
                                self.scatter.signed(),
                                0.4 + self.scatter.unit(),
                            );
                        self.push_puff(
                            PUFF_SMOKE,
                            at - dir * 0.3,
                            push,
                            time + 0.015 * i as f32,
                            0.9 + 0.2 * i as f32,
                            (0.4, 1.5),
                        );
                    }
                    self.push_puff(PUFF_FIRE, at, dir * 6.0, time, 0.22, (0.25, 0.55));
                    return;
                }
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
                target_motion,
                splash,
                color,
                after,
                on_unit,
                on_shield,
                blueprint,
                weapon,
            } => {
                let weapon = &self.blueprints.unit(*blueprint).weapons[*weapon as usize];
                // The Bulwark's rail and every hitscan gun land with the heavy blue bloom.
                let rail = self.blueprints.unit(*blueprint).key == "aster_t2_tank" || weapon.hitscan;
                let incendiary = weapon.burn_ticks > 0;
                let power = weapon.damage.to_f32().max(1.0).sqrt();
                let impact = weapon.impact;
                let shockwave = weapon.shockwave;
                let bolts = weapon.bolts;
                let red = weapon.red;
                let shatter = is_shatter_gun(weapon);
                // A hitscan shot is there the moment it is fired: no flight to wait out. It lands
                // at the start of the tick, where the target is drawn then, not where it ends up.
                let hitscan = weapon.hitscan && !shatter;
                let at = Vec3::from(pos.to_f32())
                    - if hitscan { Vec3::from(target_motion.to_f32()) } else { Vec3::ZERO };
                let start = if hitscan { time } else { time + after.to_f32() * self.tick_seconds };
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
                    if shatter {
                        // The shell absorbed the shot; no airburst fragments beyond it,
                        // but the beam still runs from the muzzle to the glass.
                        if let Some(shot) = self.take_pending_shatter(at) {
                            if shot.muzzle.distance(at) > 2.0 {
                                self.fade_beams.push(FadeBeam {
                                    from: shot.muzzle,
                                    to: at,
                                    start: time,
                                    life: 0.2,
                                    width: shot.width,
                                    laser: false,
                                });
                            }
                        }
                    }
                    return;
                }
                let splash = splash.to_f32();
                if shatter {
                    self.shatter_burst(at, Vec3::from(target_motion.to_f32()), after.to_f32(), time, start);
                    return;
                }
                let shell = *color == mc_data::WeaponColor::Orange;
                let tint = if shell && red > 0.5 { 8.0 } else { *color as u32 as f32 };
                let core = (1.6 + power * 0.28 + splash * 0.15) * impact;
                let snap = bolts > 0 || (shockwave > 0.0 && splash <= 0.0);
                let (life, ring) = if splash > 0.0 {
                    ((0.38 + splash * 0.01) * impact.min(2.0), 1.0)
                } else if snap {
                    (0.2, 1.0)
                } else {
                    (0.22, 0.0)
                };
                // A modest ball of light above the crater — the wide blast sits on the ground.
                self.push_effect(
                    at.to_array(),
                    start,
                    core,
                    if rail {
                        0.55
                    } else {
                        (life * 0.7).max(if snap { 0.12 } else { 0.2 })
                    },
                    tint,
                    // Above 1 only for the Bulwark: the flash shader reads the extra as a brighter, bluer core.
                    if rail { 1.8 } else { ring },
                );
                if rail {
                    // A second, slower blue bloom, then lobes and bolts around the core.
                    self.push_effect(
                        (at + Vec3::Z * 0.6).to_array(),
                        start + 0.04,
                        core * 0.62,
                        0.85,
                        *color as u32 as f32,
                        1.8,
                    );
                    for _ in 0..7 {
                        let dir = Vec3::new(
                            self.scatter.signed(),
                            self.scatter.signed(),
                            self.scatter.unit() * 0.8,
                        )
                        .normalize_or_zero();
                        let speed = 10.0 + self.scatter.unit() * 14.0;
                        let life = 0.42 + self.scatter.unit() * 0.18;
                        self.push_puff(
                            PUFF_SHATTER_BLAST,
                            at + dir * core * 0.18,
                            dir * speed,
                            start,
                            life,
                            (core * 0.16, core * 0.42),
                        );
                    }
                    for _ in 0..8 {
                        let vel = self.scatter.upward(0.2) * (18.0 + self.scatter.unit() * 26.0);
                        let life = 0.28 + self.scatter.unit() * 0.22;
                        self.push_puff(
                            PUFF_BOLT,
                            at + Vec3::Z * 0.3,
                            vel,
                            start,
                            life,
                            (0.45, 0.08),
                        );
                    }
                }
                if splash > 0.0 {
                    let draped = (8.0 + splash * 1.15) * impact.min(2.4);
                    self.push_effect(
                        at.to_array(),
                        start,
                        draped,
                        (life * 0.7).max(0.22),
                        5.0,
                        tint,
                    );
                    if splash > 16.0 {
                        // A second beat of fire, not another sheet of haze.
                        self.push_effect(
                            (at + Vec3::Z * 2.2).to_array(),
                            start + 0.06,
                            core * 1.15,
                            0.28,
                            2.0,
                            0.55,
                        );
                    }
                }
                if shockwave > 0.0 {
                    self.push_shockwave(
                        at.to_array(),
                        start,
                        if splash > 0.0 {
                            (18.0 + splash * 2.1) * shockwave
                        } else {
                            (16.0 + power * 2.6) * shockwave
                        },
                        if splash > 0.0 {
                            (0.7 + splash * 0.018).min(2.0)
                        } else {
                            0.55
                        },
                        shockwave.min(1.0),
                        *color as u32 as f32,
                        Vec3::ZERO,
                    );
                }
                if !shell && splash <= 0.0 {
                    for _ in 0..bolts {
                        let vel = self.scatter.upward(0.08) * (16.0 + self.scatter.unit() * 22.0);
                        let life = 0.16 + self.scatter.unit() * 0.12;
                        self.push_puff(
                            PUFF_BOLT,
                            at + Vec3::Z * 0.2,
                            vel,
                            start,
                            life,
                            (0.22, 0.05),
                        );
                    }
                    return;
                }
                // Sparks off armour or a burst of earth off the ground, and the smoke that hangs after.
                // A splash shot (energy or shell) still throws debris: the ground takes the hit.
                let big = (splash * 0.12) as usize;
                let (sparks, clods) = if *on_unit {
                    (12 + big, 2 + big / 2)
                } else {
                    (5 + big, 8 + big)
                };
                let reach = 1.0 + splash * 0.12;
                for _ in 0..sparks {
                    let vel =
                        self.scatter.upward(0.15) * (8.0 + self.scatter.unit() * 14.0) * reach;
                    let life = 0.25 + self.scatter.unit() * 0.35 + splash * 0.008;
                    self.push_puff(
                        PUFF_SPARK,
                        at + Vec3::Z * 0.2,
                        vel,
                        start,
                        life,
                        (0.2 + splash * 0.004, 0.05),
                    );
                }
                for _ in 0..clods {
                    let vel = self.scatter.upward(0.45) * (6.0 + self.scatter.unit() * 9.0) * reach;
                    let life = 0.7 + self.scatter.unit() * 0.6 + splash * 0.012;
                    self.push_puff(
                        PUFF_CLOD,
                        at + Vec3::Z * 0.2,
                        vel,
                        start,
                        life,
                        (0.12 + power * 0.025 + splash * 0.006, 0.08),
                    );
                }
                // Napalm stays as the ground wave. A normal blast still throws a short flame.
                if !(incendiary && splash > 0.0) {
                    let smokes = 2 + (splash > 0.0) as usize * 2 + big / 3;
                    for i in 0..smokes {
                        let vel = self.scatter.upward(0.5)
                            * (1.5 + self.scatter.unit() * 2.5 + splash * 0.12);
                        let kind = if *on_unit || i % 2 == 0 { PUFF_SMOKE } else { PUFF_DUST };
                        let life = 0.55 + self.scatter.unit() * 0.4 + splash * 0.008;
                        self.push_puff(
                            kind,
                            at + Vec3::Z * 0.4,
                            vel,
                            start + 0.03,
                            life,
                            (
                                0.55 + power * 0.05 + splash * 0.03,
                                1.2 + power * 0.1 + splash * 0.08,
                            ),
                        );
                    }
                    if splash > 0.0 {
                        let fires = 2 + big / 3;
                        for i in 0..fires {
                            let vel = self.scatter.upward(0.35)
                                * (3.0 + self.scatter.unit() * 4.0 + splash * 0.05);
                            let life = 0.4 + self.scatter.unit() * 0.25 + splash * 0.004;
                            self.push_puff(
                                PUFF_FIRE,
                                at + Vec3::Z * 0.55,
                                vel,
                                start + i as f32 * 0.015,
                                life,
                                (0.7 + splash * 0.04, 1.6 + splash * 0.06),
                            );
                        }
                    }
                }
            }
            SimEvent::ShieldBroken { pos, radius, .. } => {
                let at = Vec3::from(pos.to_f32());
                let r = radius.to_f32();
                let apex = at + Vec3::Z * r * 0.92;
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
                    let hit = at + Vec3::new(a.cos(), a.sin(), 0.55) * r * 0.72;
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
                    let mut spawn = at + nrm * r;
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
                // Lingering fire and smoke follow the falling hull, not the death point.
                let at = Vec3::from(pos.to_f32());
                let r = self.blueprints.unit(*blueprint).radius.to_f32();
                self.push_effect(at.to_array(), time, r * 2.8, 0.22, 1.0, 0.0);
                self.push_effect(at.to_array(), time, r * 4.0, 0.6, 2.0, 0.8);
                self.push_shockwave(at.to_array(), time, r * 5.0, 0.5, 0.65, 0.0, Vec3::ZERO);
                for _ in 0..24 {
                    let vel = self.scatter.upward(0.1) * (12.0 + r * 2.0);
                    self.push_puff(PUFF_SPARK, at, vel, time, 0.9, (0.35, 0.05));
                }
                for _ in 0..8 {
                    let vel = self.scatter.upward(0.2) * 15.0;
                    self.push_puff(PUFF_CLOD, at, vel, time, 1.3, (0.4, 0.2));
                }
            }
            SimEvent::UnitDied { pos, blueprint, .. }
            | SimEvent::AircraftCrashed { pos, blueprint } => {
                // A unit going up is an event, not a big impact: the detonation, secondary blasts
                // walking across the hull, burning fragments thrown wide, a ring of dust along the
                // ground, and fire that turns into a column of black smoke over the wreck.
                let at = Vec3::from(pos.to_f32());
                let bp = self.blueprints.unit(*blueprint);
                let (r, h) = (bp.radius.to_f32(), bp.height.to_f32());
                if bp.has(mc_data::cat::COMMANDER) {
                    self.reactor_death(at, r, h, 140.0, time);
                    return;
                }
                if let Some(db) = bp.death_blast {
                    // A volatile plant: the same detonation, sized to its blast. The
                    // fireball's puffs keep the commander's proportion to the blast,
                    // not the building's footprint.
                    let blast = db.radius.to_f32();
                    self.reactor_death(at, blast * 0.046, h, blast, time);
                    return;
                }
                let core = at + Vec3::Z * h * 0.45;
                self.push_effect(core.to_array(), time, r * 2.6, 0.22, 1.0, 0.0);
                self.push_effect(core.to_array(), time, r * 5.8, 0.85, 2.0, 1.0);
                self.push_effect(at.to_array(), time, r * 4.2, 0.45, 5.0, 1.0);
                self.push_shockwave(
                    at.to_array(),
                    time,
                    22.0 + r * 7.0,
                    0.7,
                    0.85,
                    1.0,
                    Vec3::ZERO,
                );
                for i in 0..5 {
                    let off = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit() * 0.6,
                    ) * r
                        * 0.85;
                    let delay = 0.06 + 0.09 * i as f32 + self.scatter.unit() * 0.05;
                    let size = r * (1.8 + self.scatter.unit() * 1.2);
                    self.push_effect((core + off).to_array(), time + delay, size, 0.55, 2.0, 0.35);
                }
                for _ in 0..40 {
                    let vel = self.scatter.upward(0.1)
                        * (12.0 + self.scatter.unit() * 28.0)
                        * (0.85 + r * 0.07);
                    let life = 0.55 + self.scatter.unit() * 1.2;
                    self.push_puff(PUFF_SPARK, core, vel, time, life, (0.22 + r * 0.04, 0.05));
                }
                for _ in 0..16 {
                    let vel = self.scatter.upward(0.32) * (8.0 + self.scatter.unit() * 14.0);
                    let life = 1.0 + self.scatter.unit() * 0.9;
                    self.push_puff(PUFF_CLOD, core, vel, time, life, (0.22 + r * 0.05, 0.14));
                }
                // The dust ring: pushed out flat from the foot of the blast.
                for i in 0..18 {
                    let a = (i as f32 + self.scatter.unit()) * std::f32::consts::TAU / 18.0;
                    let out = Vec3::new(a.cos(), a.sin(), 0.06);
                    let life = 1.4 + self.scatter.unit() * 0.9;
                    self.push_puff(
                        PUFF_DUST,
                        at + out * r * 0.7 + Vec3::Z * 0.4,
                        out * (12.0 + r * 1.6),
                        time + 0.03,
                        life,
                        (r * 0.4, r * 1.25),
                    );
                }
                // The fireball, then fire licking out of the wreck for a few seconds, smoke above it.
                for i in 0..12 {
                    let off = Vec3::new(
                        self.scatter.signed(),
                        self.scatter.signed(),
                        self.scatter.unit(),
                    ) * r
                        * 0.5;
                    let vel = self.scatter.upward(0.45) * (4.0 + self.scatter.unit() * 6.5);
                    let life = 1.1 + self.scatter.unit() * 0.8;
                    self.push_puff(
                        PUFF_FIRE,
                        core + off,
                        vel,
                        time + 0.02 * i as f32,
                        life,
                        (r * 0.6, r * 1.7),
                    );
                }
                for i in 0..18 {
                    let off =
                        Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.45
                            + Vec3::Z * h * 0.3;
                    let vel = Vec3::new(
                        self.scatter.signed() * 0.7,
                        self.scatter.signed() * 0.7,
                        2.6 + self.scatter.unit() * 1.8,
                    );
                    let start = time + 0.4 + i as f32 * 0.22 + self.scatter.unit() * 0.12;
                    let life = 1.5 + self.scatter.unit() * 0.7;
                    self.push_puff(PUFF_FIRE, at + off, vel, start, life, (r * 0.28, r * 0.95));
                }
                for i in 0..16 {
                    let off =
                        Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.45
                            + Vec3::Z * h * 0.5;
                    let vel = Vec3::new(
                        self.scatter.signed() * 0.9 + 0.9,
                        self.scatter.signed() * 0.9,
                        3.4 + self.scatter.unit() * 2.2,
                    );
                    let start = time + 0.12 + i as f32 * 0.28;
                    let life = 3.4 + self.scatter.unit() * 1.6;
                    self.push_puff(PUFF_SMOKE, at + off, vel, start, life, (r * 0.5, r * 2.1));
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
        let trees: Vec<_> = self.burning_trees.iter().map(|tree| {
            let mut instance = tree.instance;
            instance.health = (1.0 - (input.time - tree.start) / 9.0).clamp(0.0, 1.0);
            // Last few seconds: the burned crown crumbles into its own smoke.
            let collapse = ((input.time - tree.start - 34.0) / 8.0).clamp(0.0, 1.0);
            instance._pad = ((instance._pad as f32 * (1.0 - collapse)).max(1.0)) as u32;
            instance
        }).collect();
        self.dynamic.write((self.dynamic_count as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(&trees));
        self.dynamic_count += trees.len() as u32;
        self.land_fallen_trees(input.time);
        self.upload_sea_fx(input.time, input.alpha, camera);
        let fallen = self.fallen_tree_instances(input.time);
        let fallen = &fallen[..fallen.len().min(MAX_DYNAMIC.saturating_sub(self.dynamic_count as usize))];
        self.dynamic.write((self.dynamic_count as usize * size_of::<UnitInstance>()) as u64,
            bytemuck::cast_slice(fallen));
        self.dynamic_count += fallen.len() as u32;

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

        let marks: Vec<GpuMark> = input
            .marks
            .iter()
            .take(MAX_MARKS)
            .filter(|m| m.unit_index < self.sim_units)
            .map(|m| GpuMark {
                unit_index: m.unit_index | 0x8000_0000,
                kind: m.kind,
                work: m.work,
                shield: m.shield,
                hull: self.long_hull(m.unit_index),
                _pad: [0.0; 2],
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
        let glass = input.overlay.has_glass();

        // Sun, and a shadow box around what the camera is looking at.
        let sun = self.sky.light_direction();
        let selected: Vec<u32> = input
            .marks
            .iter()
            .filter(|m| m.kind & 1 == 0)
            .map(|m| m.unit_index)
            .collect();
        self.sky.update(&crate::sky::SkyFrame {
            camera,
            time: input.time,
            alpha: input.alpha.clamp(0.0, 1.0),
            tick_seconds: self.tick_seconds,
            selected: &selected,
        });
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
        let (tree_blast_count, tree_blasts) = self.tree_blasts.upload(input.time, &camera.frustum());
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
            // Projected radii, not diameters: keep full detail down to about
            // 72 px across, and reserve the coarse silhouettes for under 24 px.
            lod: [camera.projection_scale(), icon_px, 36.0, 12.0],
            counts: [
                self.dynamic_count,
                self.static_count,
                self.slot_count,
                self.fog_enabled as u32 | (input.build_grid as u32) << 1,
            ],
            plating: self.palette[0],
            accent: self.palette[1],
            glow: self.palette[2],
            team_colors: self.team_colors,
            build_cursor: self.build_cursor,
            scene: [
                self.scene_width as f32,
                self.scene_height as f32,
                self.render_scale,
                self.fxaa as u32 as f32,
            ],
            build_blocked: self.build_blocked,
            tree_wind: [tree_blast_count as f32, 0.0, 0.0, 0.0],
            tree_blasts,
        };
        self.globals.write(0, bytemuck::bytes_of(&globals));
        self.last_time = input.time;
        self.upload_lights(input.time, input.alpha.clamp(0.0, 1.0), camera);

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
        self.record_light_copy(cmd);
        self.sky.record_sim(&self.gpu, cmd);
        self.sky.record_shade(&self.gpu, cmd, self.scene_set);

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
            bind_pass_set(self.shields_set);
            push(pass_kind, self.shield_count);
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
                        width: self.scene_width,
                        height: self.scene_height,
                    },
                })
                .clear_values(&clear);
            device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
            set_viewport(self.scene_width, self.scene_height);
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
                // Ore fields are ground, under the foundations and the scorch marks.
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
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.missile);
            bind_pass_set(self.sprites_set);
            // Eight-sided casing, nose, rear cap, and four fins.
            device.cmd_draw(cmd, 120, self.projectile_count, 0, 0);

            // The sky wherever nothing else was drawn.
            self.sky.draw_sky(&self.gpu, cmd);

            // The sea reads what is under and around it: end the scene here,
            // copy it, and carry on drawing into the same targets.
            device.cmd_end_render_pass(cmd);
            if self.hull_shield_count > 0 {
                // The hull fields' outermost skin, into their own depth target.
                let clear = [vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue { depth: 0.0, stencil: 0 },
                }];
                device.cmd_begin_render_pass(
                    cmd,
                    &vk::RenderPassBeginInfo::default()
                        .render_pass(self.passes.shadow)
                        .framebuffer(self.hull_depth_fb)
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: vk::Extent2D { width: self.scene_width, height: self.scene_height },
                        })
                        .clear_values(&clear),
                    vk::SubpassContents::INLINE,
                );
                set_viewport(self.scene_width, self.scene_height);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.scene,
                    0,
                    &[self.scene_set],
                    &[],
                );
                draw_entities(self.pipelines.hull_shield_depth, 2);
                device.cmd_end_render_pass(cmd);
            }
            // The clouds, at half size, stopped by the scene's depth.
            self.sky.record_march(&self.gpu, cmd, self.scene_set);
            set_viewport(self.scene_width, self.scene_height);
            let area = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: self.scene_width,
                    height: self.scene_height,
                },
            };
            device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes.bloom_down)
                    .framebuffer(self.refract_fb)
                    .render_area(area),
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.refract_copy);
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.screen,
                0,
                &[self.hdr_set],
                &[],
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
            device.cmd_end_render_pass(cmd);
            device.cmd_begin_render_pass(
                cmd,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes.scene_over)
                    .framebuffer(self.scene_fb)
                    .render_area(area),
                vk::SubpassContents::INLINE,
            );
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.layouts.water,
                0,
                &[self.scene_set, self.water_fx.set, self.water_set],
                &[],
            );
            // The copy's layout left push constants undefined; restore the entities' values.
            push(0, self.shield_count);
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.water);
            device.cmd_draw(cmd, 6, 1, 0, 0);

            // Shockwaves after the sea: drawn before it, the surface painted over
            // every front that crossed open water.
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.shockwave);
            bind_pass_set(self.shockwaves_set);
            device.cmd_draw(cmd, SHOCKWAVE_LAT * SHOCKWAVE_LON * 6, MAX_SHOCKWAVES as u32, 0, 0);

            if ranges_long + ranges_short > 0 {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.range,
                );
                bind_pass_set(self.ranges_set);
                device.cmd_bind_index_buffer(cmd, self.range_ib.buffer, 0, vk::IndexType::UINT32);
                // Four instances a ring: its reach, its dead zone, and a part ring's two edges.
                for (first, count, segments) in [
                    (0, ranges_long, RANGE_SEGMENTS[0]),
                    (ranges_long, ranges_short, RANGE_SEGMENTS[1]),
                ] {
                    if count > 0 {
                        push(ranges.len() as u32, segments);
                        device.cmd_draw_indexed(cmd, segments * 6, count * 4, 0, 0, first * 4);
                    }
                }
            }
            draw_quads(self.pipelines.ring, self.marks_set, marks.len() as u32);
            if self.shield_count > self.hull_shield_count {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.shield,
                );
                bind_pass_set(self.shields_set);
                // One fullscreen triangle traces every dome. Instancing a cube
                // per bubble re-solved the same union on overlapping pixels.
                push(self.shield_count, 0);
                device.cmd_draw(cmd, 3, 1, 0, 0);
            }
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
            // Hull fields last, so their glow lies over the effects inside them.
            if self.hull_shield_count > 0 {
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.hull_shield);
                device.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts.water,
                    1,
                    &[self.shields_set, self.hull_set],
                    &[],
                );
                push(2, self.shield_count);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.mesh_vb.buffer], &[0]);
                device.cmd_bind_index_buffer(cmd, self.mesh_ib.buffer, 0, vk::IndexType::UINT32);
                device.cmd_draw_indexed_indirect(cmd, self.commands.buffer, 0, model_slots, 20);
            }

            // Ore veins glow through the ground, and through trees and units,
            // while the mine survey is up.
            if self.vein_count > 0 && self.ore_highlight > 0.01 {
                // Set 0 (scene) is bound already; the veins read nothing else.
                device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines.vein);
                device.cmd_bind_vertex_buffers(cmd, 0, &[self.vein_vb.buffer], &[0]);
                device.cmd_push_constants(
                    cmd,
                    self.layouts.scene,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&[self.ore_highlight, self.vein_time]),
                );
                device.cmd_draw(cmd, self.vein_count, 1, 0, 0);
            }
            // Rain close up, then the clouds over everything in the world; icons and bars stay on top.
            self.sky.draw_rain(&self.gpu, cmd, self.scene_set);
            self.sky.draw_composite(&self.gpu, cmd, self.scene_set);

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

        // Overlay glass: the tone-mapped picture at quarter size, blurred across, then down.
        if glass {
            unsafe {
                let (w, h) = (self.glass[0].width, self.glass[0].height);
                let glass_pass = |pipeline: vk::Pipeline, target: usize, source: vk::DescriptorSet, a: [f32; 2]| {
                    let begin = vk::RenderPassBeginInfo::default()
                        .render_pass(self.passes.bloom_down)
                        .framebuffer(self.glass_fbs[target])
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: vk::Extent2D { width: w, height: h },
                        });
                    device.cmd_begin_render_pass(cmd, &begin, vk::SubpassContents::INLINE);
                    set_viewport(w, h);
                    device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
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
                glass_pass(self.pipelines.glass_source, 0, self.screen_set, TONEMAP);
                glass_pass(self.pipelines.glass_blur, 1, self.glass_sets[0], [1.0, 0.0]);
                glass_pass(self.pipelines.glass_blur, 0, self.glass_sets[1], [0.0, 1.0]);
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
                bytemuck::bytes_of(&TONEMAP),
            );
            device.cmd_draw(cmd, 3, 1, 0, 0);
            if !overlay.is_empty() {
                device.cmd_bind_pipeline(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines.overlay,
                );
                if glass {
                    // The blurred picture where the tone mapper had the scene; same atlas.
                    device.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layouts.screen,
                        0,
                        &[self.glass_sets[0]],
                        &[],
                    );
                }
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
            device.destroy_framebuffer(self.refract_fb, None);
            device.destroy_framebuffer(self.hull_depth_fb, None);
            device.destroy_framebuffer(self.shadow_fb, None);
            for fb in self.bloom_fbs.drain(..).chain(self.glass_fbs.drain(..)) {
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
        self.sky.destroy(&self.gpu);
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
            &self.shadow,
            &self.overview,
            &self.tiles,
            &self.tile_index,
            &self.fog,
            &self.noise,
            &self.panel,
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
        if let Output::Headless { image, readback } = &mut self.output {
            gpu.destroy_image_ref(image);
            gpu.destroy_buffer(std::mem::replace(readback, placeholder()));
        }
        // `self.gpu` drops last and destroys the device and instance.
    }
}

#[cfg(test)]
mod shatter_tests {
    use super::*;

    /// Exercise the real Vulkan emission path, including every delayed wave.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn shatter_fragment_shockwaves_render() {
        use glam::Vec2;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let mut renderer = Renderer::new(Target::Headless { width: 960, height: 720 }, SceneDesc {
            map: map.clone(), blueprints: blueprints.clone(), pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; 8],
        }).unwrap();
        renderer.fog_enabled = false;
        let xy = Vec2::new(12360.0, 12380.0);
        let target = xy.extend(renderer.ground_height(xy) + 100.0);
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(960.0, 720.0));
        camera.focus = target;
        camera.distance = 255.0;
        camera.tilt = 0.2;
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let overlay = Overlay::default();
        let draw = |renderer: &mut Renderer, time| {
            renderer.render(&FrameInput { camera: &camera, time, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        };
        let output = root.join("artifacts/shatter-update");
        std::fs::create_dir_all(&output).unwrap();
        for (index, key) in ["aster_t3_shatter", "aster_t3_mobile_aa"].iter().enumerate() {
            let unit = blueprints.unit(blueprints.id_of(key).unwrap());
            let weapon = &unit.weapons[0];
            let time = 10.0 + index as f32 * 3.0;
            for _ in 0..24 { draw(&mut renderer, time); }
            let muzzle = target + Vec3::new(-140.0, 0.0, -70.0);
            let shot = PendingShatter {
                effects: unit.visual.effects, muzzle, dir: (target - muzzle).normalize(),
                range: weapon.range_max.to_f32(), bolts: weapon.bolts,
                splash: weapon.splash.to_f32(), width: 0.7,
                impact: weapon.impact, shockwave: weapon.shockwave, color: 0.0,
            };
            let before = renderer.shockwave_cursor;
            renderer.spawn_shatter_split(&shot, target, Vec3::Y * 25.0, time, time, false);
            assert_eq!((renderer.shockwave_cursor + MAX_SHOCKWAVES - before) % MAX_SHOCKWAVES,
                1 + shatter_fragment_count(weapon.bolts, false) as usize);
            assert_eq!(shot.effects.shockwave_color, Some([0.16, 0.48, 1.0]));
            for (name, age) in [("split", 0.10), ("bursts", 0.32), ("waves", 0.48)] {
                draw(&mut renderer, time + age);
                let pixels = renderer.read_pixels().unwrap();
                let mut ppm = b"P6\n960 720\n255\n".to_vec();
                for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
                std::fs::write(output.join(format!("{key}-{name}.ppm")), ppm).unwrap();
            }
        }
    }



    #[test]
    fn shatter_beam_stays_on_the_barrel_axis_when_target_moves() {
        let muzzle = Vec3::new(8.0, 0.0, 11.0);
        let axis = Vec3::new(1.0, 0.0, 1.0).normalize();
        let target = muzzle + axis * 150.0 + Vec3::Y * 40.0;
        let (split, forward) = shatter_airburst(muzzle, target, axis, 60.0);
        assert!((split - muzzle).normalize().distance(axis) < 0.001);
        assert!(forward.distance(axis) < 0.001);
        assert!((split - muzzle).length() < target.distance(muzzle));
    }

    #[test]
    fn shatter_explosions_lead_crossing_and_climbing_aircraft() {
        let impact = Vec3::new(100.0, 100.0, 80.0);
        // A 170 m/s crossing aircraft climbing at 20 m/s. A 0.3 s fragment
        // arrives 0.225 s past the tick-end hull after render interpolation.
        let motion = Vec3::new(0.0, 17.0, 2.0);
        let arrival = shatter_target_at(impact, motion, 0.25, 0.1, 0.3);
        assert!(arrival.distance(Vec3::new(100.0, 138.25, 84.5)) < 0.001);
        for life in [SHATTER_FRAGMENT_MIN_LIFE, SHATTER_FRAGMENT_MAX_LIFE] {
            let end = shatter_target_at(impact, motion, 0.25, 0.1, life);
            let target_at_split = shatter_target_at(impact, motion, 0.25, 0.1, 0.0);
            assert!((end - target_at_split).distance(motion * (life / 0.1)) < 0.001);
        }
        assert_eq!(shatter_target_at(impact, Vec3::ZERO, 0.25, 0.1, 0.3), impact);
        // Slower render-clock ticks must reduce the amount of lead.
        let slow = shatter_target_at(impact, motion, 0.25, 0.2, 0.3);
        assert!(slow.distance(Vec3::new(100.0, 112.75, 81.5)) < 0.001);
        let retreat = shatter_target_at(impact, -motion, 0.25, 0.1, 0.3);
        assert!(retreat.distance(Vec3::new(100.0, 61.75, 75.5)) < 0.001);
    }

    #[test]
    fn sunder_shatter_keeps_lighter_effects_and_firepower() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let bp = mc_data::Blueprints::load(&root).unwrap();
        let mobile = &bp.unit(bp.id_of("aster_t3_mobile_aa").unwrap()).weapons[0];
        let static_gun = &bp.unit(bp.id_of("aster_t3_shatter").unwrap()).weapons[0];
        assert!(is_shatter_gun(mobile) && is_shatter_gun(static_gun));
        assert_eq!(shatter_fragment_count(mobile.bolts, false), 14);
        assert_eq!(shatter_fragment_count(static_gun.bolts, false), 24);
        assert!(mobile.impact < static_gun.impact * 0.6);
        assert!(mobile.flash < static_gun.flash * 0.6);
        assert!(mobile.shockwave < static_gun.shockwave * 0.6);
        assert_eq!(shatter_detail_scale(static_gun.impact), 1.0);
        assert!(shatter_detail_scale(mobile.impact) < 0.6);
        assert_eq!(mobile.damage.to_f32(), 875.0);
        assert_eq!(static_gun.damage.to_f32(), 1500.0);
        assert_eq!(static_gun.splash.to_f32(), 60.0);
        assert_eq!(mobile.range_max.to_f32(), 440.0);
        assert_eq!(mobile.splash.to_f32(), 42.0);
    }

    #[test]
    fn airburst_leaves_room_for_fragments_without_crossing_the_muzzle() {
        let muzzle = Vec3::new(8.0, 0.0, 11.0);
        for distance in [0.0, 1.0, 12.0, 60.0, 620.0] {
            for direction in [Vec3::X, Vec3::Z, Vec3::new(1.0, 1.0, 0.5).normalize()] {
                let target = muzzle + direction * distance;
                let (burst, forward) = shatter_airburst(muzzle, target, direction, 38.0);
                assert!(burst.is_finite() && forward.is_finite());
                assert!((burst - muzzle).dot(direction) >= -0.001);
                if distance > 0.0 {
                    assert!(burst.distance(target) > 0.0);
                    assert!(burst.distance(muzzle) < distance);
                }
                if distance >= 60.0 {
                    assert!(burst.distance(target) >= 23.99);
                }
            }
        }
    }
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    use glam::Vec2;

    fn save_environment_frame(renderer: &mut Renderer, path: &std::path::Path) {
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = b"P6\n960 720\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, ppm).unwrap();
    }

    /// Real Vulkan pipeline check, with a captured frame for visual inspection.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn forest_fire_lifecycle_and_render() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let start = Vec2::from(map.start_positions()[0].to_f32());
        let index = map.props().iter().enumerate().filter(|(_, p)| p.kind.is_tree())
            .min_by(|(_, a), (_, b)| {
                Vec2::from(a.pos.to_f32()).distance_squared(start)
                    .total_cmp(&Vec2::from(b.pos.to_f32()).distance_squared(start))
            }).unwrap().0;
        let xy = map.props()[index].pos;
        let blueprint = blueprints.id_of("aster_t1_bomber").unwrap();
        let mut renderer = Renderer::new(Target::Headless { width: 960, height: 720 }, SceneDesc {
            map: map.clone(), blueprints, pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; 8],
        }).unwrap();
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(960.0, 720.0));
        camera.focus = Vec3::new(xy.x.to_f32(), xy.y.to_f32(), renderer.ground_height(Vec2::from(xy.to_f32())) + 8.0);
        camera.distance = 48.0;
        camera.tilt = 0.48;
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let overlay = Overlay::default();
        for _ in 0..24 {
            renderer.render(&FrameInput { camera: &camera, time: 0.0, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        }
        save_environment_frame(&mut renderer, &root.join("artifacts/terrain-v2/tree.ppm"));
        renderer.tree_fires(&frame, 0.0, &camera);
        frame.props_dead[index / 32] |= 1 << (index % 32);
        renderer.tree_fires(&frame, 0.1, &camera);
        assert!(renderer.burning_trees.is_empty(), "reclaim must not ignite trees");
        frame.props_dead[index / 32] = 0;
        renderer.tree_fires(&frame, 0.2, &camera);
        frame.props_dead[index / 32] |= 1 << (index % 32);
        frame.events.push(SimEvent::Impact {
            pos: xy.extend(mc_core::Fx::from_int(camera.focus.z as i32 - 8)),
            target_motion: mc_core::FxVec3::ZERO,
            splash: mc_core::Fx::from_int(12), color: mc_data::WeaponColor::Orange,
            after: mc_core::Fx::ZERO, on_unit: false, on_shield: true,
            blueprint, weapon: 0,
        });
        renderer.tree_fires(&frame, 0.3, &camera);
        assert!(renderer.burning_trees.is_empty(), "shield interception must not ignite trees");
        renderer.previous_dead[index / 32] = 0;
        if let SimEvent::Impact { on_shield, .. } = &mut frame.events[0] { *on_shield = false; }
        renderer.tree_fires(&frame, 0.4, &camera);
        assert_eq!(renderer.burning_trees.len(), 1);
        frame.events.clear();
        let overlay = Overlay::default();
        for step in 0..=110 {
            let time = 0.5 + step as f32 * 0.1;
            renderer.render(&FrameInput { camera: &camera, time, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        }
        assert_eq!(renderer.burning_trees.len(), 1, "dead bits must not restart fire");
        assert_eq!(renderer.dynamic_count, 1, "keep the charred tree visible");
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = b"P6\n960 720\n255\n".to_vec();
        for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::create_dir_all(root.join("artifacts/terrain-v2")).unwrap();
        std::fs::write(root.join("artifacts/terrain-v2/fire.ppm"), ppm).unwrap();
        renderer.tree_fires(&frame, 50.0, &camera);
        assert!(renderer.burning_trees.is_empty(), "burned tree must eventually expire");
        for width in [0, 7] {
            renderer.previous_dead[index / 32] = 0;
            renderer.burning_trees.clear();
            let at = xy.extend(mc_core::Fx::from_int(camera.focus.z as i32));
            frame.events = vec![SimEvent::BoreDischarge {
                from: at - mc_core::FxVec3::new(mc_core::Fx::from_int(100), mc_core::Fx::ZERO, mc_core::Fx::ZERO),
                to: at + mc_core::FxVec3::new(mc_core::Fx::from_int(100), mc_core::Fx::ZERO, mc_core::Fx::ZERO),
                width: mc_core::Fx::from_int(width), after: mc_core::Fx::ZERO,
                owner: 0, blueprint, weapon: 0,
            }];
            renderer.tree_fires(&frame, 51.0, &camera);
            assert_eq!(renderer.burning_trees.len(), 1, "bore width {width} must ignite the middle of its path");
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
                renderer.render(&FrameInput { camera: &camera, time: 50.0 + step as f32 * 0.1, alpha: 1.0,
                    sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                    overlay: &overlay, build_grid: false }).unwrap();
            }
            save_environment_frame(&mut renderer, &root.join(format!("artifacts/terrain-v2/{name}.ppm")));
        }

    }
}


#[cfg(test)]
mod shockwave_tests {

    /// Captures the real Vulkan effect at several ages and across a live dome.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn shockwave_shield_color_render() {
        use super::*;
        use glam::Vec2;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let mut definitions = Blueprints::load(&root.join("data")).unwrap();
        let id = definitions.id_of("aster_t3_shatter").unwrap();
        definitions.units[id.0 as usize].visual.effects = mc_data::EffectSettings {
            dust_visibility: 1.2, dust_lifetime: 1.5, shockwave_color: Some([0.16, 0.62, 1.0]),
            ..Default::default()
        };
        let blueprints = Arc::new(definitions);
        let mut renderer = Renderer::new(Target::Headless { width: 960, height: 720 }, SceneDesc {
            map: map.clone(), blueprints, pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; 8],
        }).unwrap();
        renderer.fog_enabled = false;
        let xy = Vec2::new(12360.0, 12380.0);
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(960.0, 720.0));
        camera.focus = xy.extend(renderer.ground_height(xy) + 18.0);
        camera.distance = 235.0;
        camera.tilt = std::env::var("MC_EFFECT_TEST_TILT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.12);
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let overlay = Overlay::default();
        let draw = |renderer: &mut Renderer, frame: &RenderFrame, time| {
            renderer.render(&FrameInput { camera: &camera, time, alpha: 1.0,
                sim: Some(frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        };
        for _ in 0..24 { draw(&mut renderer, &frame, 0.0); }
        let output = root.join(std::env::var("MC_EFFECT_TEST_OUTPUT").unwrap_or_else(|_| "artifacts/shockwave-shields".into()));
        std::fs::create_dir_all(&output).unwrap();
        let save = |renderer: &mut Renderer, name: &str| {
            let pixels = renderer.read_pixels().unwrap();
            let mut ppm = b"P6\n960 720\n255\n".to_vec();
            for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
            std::fs::write(output.join(format!("{name}.ppm")), ppm).unwrap();
            pixels
        };
        let center = camera.focus + Vec3::new(-34.0, 0.0, -5.0);
        for (case, tint, shielded, dust_color, opacity, brightness) in [
            ("cyan", [0.16, 0.62, 1.0], false, None, 1.2, 1.0),
            ("amber", [1.0, 0.3, 0.07], false, None, 1.2, 1.0),
            ("shield", [0.16, 0.62, 1.0], true, None, 1.2, 1.0),
            ("dust-rust", [0.16, 0.62, 1.0], false, Some([0.7, 0.22, 0.08]), 1.2, 1.0),
            ("dust-faint", [0.16, 0.62, 1.0], false, Some([0.7, 0.22, 0.08]), 0.3, 1.0),
            ("dust-bright", [0.16, 0.62, 1.0], false, Some([0.7, 0.22, 0.08]), 1.2, 2.0),
        ] {
            renderer.scatter = Scatter(0x9E37_79B9);
            renderer.shockwaves.write(0, &vec![0; renderer.shockwaves.size as usize]);
            renderer.puffs.write(0, &vec![0; renderer.puffs.size as usize]);
            frame.shields.clear();
            if shielded {
                let at = camera.focus + Vec3::new(34.0, 0.0, -18.0);
                frame.shields.push(mc_sim::mirror::ShieldInstance {
                    pos: at.to_array(), radius: 32.0, prev_open: 1.0, open: 1.0, health: 1.0,
                    packed: 2 << 16, unit_id: 1, projector: 10.0, height: 20.0, _pad: 0.0,
                });
            }
            draw(&mut renderer, &frame, 10.0);
            renderer.effect_settings = mc_data::EffectSettings {
                dust_visibility: opacity, dust_lifetime: 1.5, shockwave_color: Some(tint),
                dust_color, dust_brightness: brightness,
            };
            renderer.push_shockwave(center.to_array(), 10.0, 112.0, 1.25, 1.0, 0.0, Vec3::ZERO);
            for (step, age) in [0.12, 0.28, 0.48, 0.72, 1.15, 2.2, 4.5].into_iter().enumerate() {
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
        assert_eq!(renderer.puff_cursor, cursor, "zero visibility must disable emission");
        renderer.effect_settings.dust_visibility = 1.0;
        renderer.effect_settings.dust_lifetime = 0.0;
        renderer.push_puff(PUFF_SMOKE, center, Vec3::Z, 20.0, 2.0, (5.0, 10.0));
        assert_eq!(renderer.puff_cursor, cursor, "zero lifetime must disable emission");
    }

    #[test]
    fn shockwave_barriers_stop_crossings_but_allow_shared_interior_and_outward_sparks() {
        let b = super::EffectBarrier { center: [0.0; 3], radius: 10.0, inverse_axes: [0.1; 3], min_z: 0.0 };
        let p = |x, z| glam::Vec3::new(x, 0.0, z);
        assert!(b.crosses(p(-20.0, 1.0), p(0.0, 1.0)));
        assert!(b.crosses(p(-20.0, 1.0), p(20.0, 1.0)));
        assert!(b.crosses(p(0.0, 1.0), p(20.0, 1.0)));
        assert!(!b.crosses(p(-2.0, 1.0), p(2.0, 1.0)));
        assert!(!b.crosses(p(-20.0, 12.0), p(20.0, 12.0)));
        assert!(!b.crosses(p(-20.0, -2.0), p(20.0, -2.0)));
        assert!(b.crosses(p(-10.0, 0.0), p(0.0, 0.0)));
        assert!(!b.crosses(p(-10.0, 0.0), p(-20.0, 0.0)));
        let hull = super::EffectBarrier { center: [0.0, 0.0, 3.0], radius: 4.0,
            inverse_axes: [0.25, 0.25, 1.0 / 3.0], min_z: 0.0 };
        assert!(hull.crosses(p(-8.0, 3.0), p(0.0, 3.0)));
        assert!(!hull.crosses(p(-8.0, 7.0), p(8.0, 7.0)));
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
        assert!(shockwave_ground_arrival(center, Vec3::new(20.0, 0.0, 2.0),
            60.0, Vec3::X, 0.0).is_some());
        assert!(shockwave_ground_arrival(center, Vec3::new(-20.0, 0.0, 2.0),
            60.0, Vec3::X, 0.0).is_none());
        assert!(shockwave_ground_arrival(center, Vec3::new(20.0, 0.0, -2.0),
            60.0, Vec3::ZERO, 0.0).is_none());
        assert!(shockwave_ground_arrival(Vec3::new(0.0, 0.0, 100.0),
            Vec3::new(20.0, 0.0, 2.0), 60.0, Vec3::ZERO, 0.0).is_none());
    }
}

#[cfg(test)]
mod glass_tests {
    use super::*;

    /// Overlay glass shows the scene blurred and darkened, and leaves the rest alone.
    /// `GLASS_DUMP` names a PPM to write the frame to.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn glass_blurs_the_scene_behind_it() {
        use glam::Vec2;
        let (w, h) = (640u32, 360u32);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let mut renderer = Renderer::new(Target::Headless { width: w, height: h }, SceneDesc {
            map: map.clone(), blueprints, pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; 8],
        }).unwrap();
        renderer.fog_enabled = false;
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(w as f32, h as f32));
        let xy = Vec2::new(12200.0, 12150.0);
        camera.focus = xy.extend(renderer.ground_height(xy));
        camera.distance = 600.0;
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let mut draw = |overlay: &Overlay| {
            renderer.render(&FrameInput { camera: &camera, time: 1.0, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay, build_grid: false }).unwrap();
            renderer.read_pixels().unwrap()
        };
        let plain = draw(&Overlay::default());
        let mut overlay = Overlay::default();
        overlay.blur_rect(0.0, 0.0, w as f32 / 2.0, h as f32, [0.0; 4]);
        overlay.blur_rect(w as f32 * 0.75, 0.0, w as f32 / 4.0, h as f32, [0.0, 0.0, 0.0, 0.55]);
        let glass = draw(&overlay);
        if let Ok(path) = std::env::var("GLASS_DUMP") {
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            for pixel in glass.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
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
        eprintln!("plain {plain_mean:.1}/{plain_detail:.2}, blurred {blur_mean:.1}/{blur_detail:.2}");
        assert!(plain_detail > 0.2, "the scene has too little detail to tell");
        assert!(blur_detail < plain_detail * 0.5, "glass did not blur");
        assert!((blur_mean - plain_mean).abs() < plain_mean * 0.15, "glass changed the brightness");
        // Outside the glass nothing changes (but for streaming noise between two frames).
        let (open, shut) = (stats(&plain, half, 3 * quarter), stats(&glass, half, 3 * quarter));
        assert!((open.0 - shut.0).abs() < 0.5 && (open.1 - shut.1).abs() < 0.05, "{open:?} vs {shut:?}");
        let (dark_mean, _) = stats(&glass, 3 * quarter, w);
        let (under_mean, _) = stats(&plain, 3 * quarter, w);
        assert!(dark_mean < under_mean * 0.75, "tinted glass is not darker: {dark_mean} vs {under_mean}");
    }
}

/// Edge of the square decal tiles an ore field is drawn with, metres: six
/// patch quads of one terrain cell each.
const ORE_TILE_M: f32 = 48.0;

/// Ore fields for the ground pass. Every field's corners go first (only `pos`
/// is used); then one tile per 48 m square that touches a field, whose
/// `strength_seed` packs the corner count (low 8 bits) and how far back from
/// the tile its field's first corner is (the rest), so the shader can find the
/// outline whatever offset the block is uploaded at. Returns the entries and
/// how many of them are corners.
/// A field's outline with its corners rounded off (Chaikin corner cutting),
/// for the drawn rim only: the few corners of a map polygon read as a hard,
/// hand-cut shape. The rounded line lies just inside the real one; the sim
/// keeps the real corners. At most 64 corners, the rim shader walks them all.
fn rounded_ore(region: &mc_map::OreRegion) -> mc_map::OreRegion {
    let mut pts: Vec<[f32; 2]> = region.points.iter().map(|p| p.to_f32()).collect();
    for _ in 0..3 {
        if pts.len() < 3 || pts.len() * 2 > 64 {
            break;
        }
        let n = pts.len();
        pts = (0..n)
            .flat_map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                [at(0.25), at(0.75)]
            })
            .collect();
    }
    let fx = |v: f32| mc_core::Fx::from_f32(v);
    mc_map::OreRegion { points: pts.into_iter().map(|p| mc_core::FxVec2::new(fx(p[0]), fx(p[1]))).collect() }
}

fn ore_splats(regions: &[mc_map::OreRegion]) -> (Vec<StainInstance>, usize) {
    let mut corners = Vec::new();
    let mut firsts = Vec::new();
    for r in regions {
        firsts.push(corners.len());
        corners.extend(r.points.iter().map(|p| StainInstance {
            pos: p.to_f32(),
            radius: 0.0,
            strength_seed: 0,
        }));
    }
    let corner_count = corners.len();
    let mut tiles = Vec::new();
    for (r, &first) in regions.iter().zip(&firsts) {
        let (lo, hi) = r.bounds();
        let (lo, hi) = (lo.to_f32(), hi.to_f32());
        // One tile of margin for the rim and the ragged edge.
        let tx0 = ((lo[0] - 8.0) / ORE_TILE_M).floor() as i32;
        let ty0 = ((lo[1] - 8.0) / ORE_TILE_M).floor() as i32;
        let tx1 = ((hi[0] + 8.0) / ORE_TILE_M).floor() as i32;
        let ty1 = ((hi[1] + 8.0) / ORE_TILE_M).floor() as i32;
        let pts: Vec<[f32; 2]> = r.points.iter().map(|p| p.to_f32()).collect();
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let centre = [
                    (tx as f32 + 0.5) * ORE_TILE_M,
                    (ty as f32 + 0.5) * ORE_TILE_M,
                ];
                // Skip tiles wholly outside the outline (plus the margin).
                if polygon_distance(&pts, centre) > ORE_TILE_M * 0.72 + 8.0 {
                    continue;
                }
                let index = corner_count + tiles.len();
                let back = (index - first) as u32;
                tiles.push(StainInstance {
                    pos: centre,
                    radius: ORE_TILE_M * 0.5,
                    strength_seed: pts.len() as u32 | back << 8,
                });
            }
        }
    }
    corners.extend(tiles);
    (corners, corner_count)
}

/// Signed distance from `p` to the polygon `pts`, negative inside.
fn polygon_distance(pts: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let w = [p[0] - a[0], p[1] - a[1]];
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-6)).clamp(0.0, 1.0);
        let q = [w[0] - e[0] * t, w[1] - e[1] * t];
        d = d.min(q[0] * q[0] + q[1] * q[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + e[0] * (p[1] - a[1]) / e[1] {
            inside = !inside;
        }
    }
    if inside {
        -d.sqrt()
    } else {
        d.sqrt()
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
    let mut blob = |out: &mut Vec<MeshVertex>, c: Vec3, r: Vec3, seed: u32| {
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
            let dir = if k + 1 < line.len() { line[k + 1].0 - p } else { p - line[k - 1].0 };
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
        let pts: Vec<Vec2> = region.points.iter().map(|p| Vec2::from(p.to_f32())).collect();
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
            let corner = pts[(v * pts.len() / lodes + (hash(seed, 2 + v as u32) * 3.0) as usize) % pts.len()];
            let end = centre + (corner - centre) * (0.7 + 0.25 * hash(seed, 20 + v as u32));
            let rise = (hash(seed, 40 + v as u32) - 0.5) * 90.0;
            let side = (end - centre).perp().normalize_or_zero();
            let steps = 10;
            let line: Vec<(Vec3, f32)> = (0..=steps)
                .map(|k| {
                    let t = k as f32 / steps as f32;
                    let wander = (hash(seed, 100 + v as u32 * 16 + k as u32) - 0.5) * 30.0 * (t * (1.0 - t) * 4.0);
                    let xy = centre.lerp(end, t) + side * wander;
                    let z = -depth - rise * t - (t * std::f32::consts::PI * 1.5 + v as f32).sin() * 10.0;
                    (xy.extend(z), 11.0 * (1.0 - t).powf(0.8) + 2.5)
                })
                .collect();
            tube(&mut out, &line);
            // Nodules where the lode swells.
            for k in [4usize, 7] {
                if hash(seed, 300 + v as u32 * 4 + k as u32) > 0.35 {
                    let (p, r) = line[k];
                    blob(&mut out, p, Vec3::splat(r * 1.6), seed + 500 + v as u32 * 8 + k as u32);
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
        && bp.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
        && bp.transport.is_none()
        && Vec3::from(u.prev_pos).distance(Vec3::from(u.pos)) > 0.3
}

#[cfg(test)]
mod capital_cloud_tests {
    use super::*;
    #[test]
    fn capital_hulls_never_clear_clouds_at_rest_or_underway() {
        let data = mc_data::Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        let bp = data.unit(data.id_of("aster_t2_lift_ship").unwrap());
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = [1000.0,1000.0,420.0];
        unit.prev_pos = unit.pos;
        unit.build = 1.0;
        assert!(!stirs_clouds(&unit,bp));
        unit.pos[0] += 8.0;
        assert!(!stirs_clouds(&unit,bp));
        unit.build = 0.5;
        assert!(!stirs_clouds(&unit,bp));
        unit.build = 1.0;
        unit.owner_flags = KIND_WRECK;
        assert!(!stirs_clouds(&unit,bp));
    }
}
