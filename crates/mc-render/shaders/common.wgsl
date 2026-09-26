// Shared by every shader (build.rs prepends this file).
// Conventions: world is metres, Z up. Clip space is WebGPU-style (naga flips Y
// for Vulkan); depth is reversed-Z, 1 at the near plane and 0 at infinity.

// Mirrors mc_map::BUILD_CELL_M.
const BUILD_CELL_M: f32 = 12.0;
// Pad footprint atlas: lot UV covered by each layer, and metres encoded
// around the form edge. Must match `models::footprint`.
const PAD_FOOTPRINT_REACH: f32 = 1.12;
const PAD_SDF_RANGE: f32 = 8.0;
// Hull-plan atlas: same texel encoding as a pad, no pour. G/B are the
// local-Z span of the mesh in that texel, encoded by `ModelInfo.height`.
const HULL_PLAN_REACH: f32 = 1.12;
const HULL_PLAN_RANGE: f32 = 8.0;

//!rust crate::renderer::Globals
struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    shadow_view_proj: mat4x4<f32>,
    // xyz eye position, w time in seconds
    camera: vec4<f32>,
    // xyz unit vector toward the sun, w interpolation factor between the last two sim ticks
    sun: vec4<f32>,
    // Output width, height, 1/width, 1/height. Pixel sizes (lines, icons, beams)
    // are output pixels, the same at any render scale; a fragment's clip.xy is a
    // scene pixel, so screen uv from it uses `scene`.
    viewport: vec4<f32>,
    frustum: array<vec4<f32>, 6>,
    // map size x, y, water level, shadow strength (fades out with zoom)
    map: vec4<f32>,
    // min_z, z span of the u16 range, tiles_w, tiles_h
    height: vec4<f32>,
    // projection scale (pixels per metre at 1 m), icon below px, lod1 below px, lod2 below px
    lod: vec4<f32>,
    // dynamic entity count, static entity count, draw slot count, flags (1 = fog on)
    counts: vec4<u32>,
    // Faction palette: plating, accent, glow.
    plating: vec4<f32>,
    accent: vec4<f32>,
    glow: vec4<f32>,
    team_colors: array<vec4<f32>, 8>,
    // Build grid: pointer xy, radius it shows within, taken lot count.
    build_cursor: vec4<f32>,
    // Taken lots (structures and plans) near the pointer: min xy, max xy.
    // Size mirrors renderer::BUILD_BLOCKED_MAX.
    build_blocked: array<vec4<f32>, 48>,
    // Scene width, height (the output times the render scale), render scale, and how
    // far the selection's see-through is in, 0..1 (sky.rs; the Precursor cutaway).
    scene: vec4<f32>,
    // x how many tree_blasts are in use (renderer/tree_wind.rs); yz the camera's focus;
    // w how awake a survival map's Precursor facility is, 0.15..1 (0 elsewhere: the
    // light as authored, and no cutaway).

    tree_wind: vec4<f32>,

    // Per blast: xyz where, w when it went off; then range, force at a 10 m tree's top.
    // Size mirrors tree_wind::TREE_BLASTS * 2.
    tree_blasts: array<vec4<f32>, 48>,
    // The sun's shadow cascades, near to far (renderer/shadow_cascades.rs);
    // `shadow_view_proj` is the first. Per cascade: metres per texel, metres of depth.
    shadow_cascades: array<mat4x4<f32>, 3>,
    shadow_info: array<vec4<f32>, 3>,
    // The faction's shield colour (faction.ron `shield_color`), rgb.
    shield: vec4<f32>,
    // Nuclear blasts drawn as volumes (renderer/nuke_fx.rs, nuke.wgsl): four vec4 each.
    // Size mirrors nuke_fx::NUKE_SLOTS * 4.
    nukes: array<vec4<f32>, 256>,
    // x the flash whiting the view out, y the scene dimmed after it, z blasts in use,
    // w missiles in use.
    nuke_view: vec4<f32>,
    // Strategic missiles: nose xyz and kind | owner << 4 | plume metres << 8, then axis
    // xyz and nose heat. Size mirrors nuke_fx::MISSILE_SLOTS * 2.
    strategic: array<vec4<f32>, 128>,
    // x the map's climate (mc_data::weather::Climate): 0 temperate, 1 tropical
    // (`tropical()` in bindings.wgsl); y 1 while grass is grown (renderer/grass.rs);
    // z how far from the eye it grows (`grass::reach`); w spare.
    climate: vec4<f32>,
    // Prop detail (renderer `PropDetail`): x the smallest a prop is drawn at (pixels
    // of radius), y scales the LOD thresholds for props, z the smallest a prop casts
    // a shadow at (pixels), w spare.
    detail: vec4<f32>,
}


// Mirrors mc_sim::mirror::UnitInstance (192 bytes).
//!rust mc_sim::mirror::UnitInstance
struct Entity {
    prev_pos: vec3<f32>,
    prev_heading: f32,
    pos: vec3<f32>,
    heading: f32,
    blueprint: u32,
    owner_flags: u32,
    health: f32,
    build: f32,
    turret_yaw: f32,
    radius: f32,
    unit_id: u32,
    // Props: scale in thousandths. Units: kills, fire state, rank. Wrecks: falling/sinking.
    packed: u32,
    // x ground covered in metres (wrapping), y what this tick added, z what the tick before added.
    // A spent casing in the air: x, y its age in seconds last tick and this (`casing_carry`).
    gait: vec3<f32>,
    // 0, or how far along the unit's refit is
    upgrade: f32,
    // Pitch of the gun arm last tick and this, then of the build arm (radians, up positive).
    arm_pitch: vec4<f32>,
    prev_turret_yaw: f32,
    // Local-space print origin while someone is working (three f32s: vec3 would pad).
    weld: array<f32, 3>,
    // How far the barrel is kicked back: 1 the instant it fires, 0 at rest.
    recoil: f32,
    prev_recoil: f32,
    // Range into the construction-weld buffer: every origin still lighting this site.
    weld_first: u32,
    weld_count: u32,
    // How far a siege gun is planted: 0 packed, 1 ready to fire.
    deploy: f32,
    prev_deploy: f32,
    _pad2: vec2<f32>,
    // While a refit is under way: the look bits of the loadout being fitted. Zero otherwise.
    refit_modules: u32,
    // `mirror::UnitInstance::status`: see the `UNIT_*` constants for its bits. A spent casing
    // in the air: [2] is one more than its thrower's index in the dynamic entities, or zero.
    status: array<u32, 3>,
    // A mounted turret: yaw off the torso last tick and this, pitch last tick and this.
    mount: vec4<f32>,
    // Rotary barrels turned last tick and this, then the mounted tube's kick last tick and this.
    spin_recoil: vec4<f32>,
}

// One per blueprint / prop kind.
//!rust crate::renderer::ModelInfo
struct ModelInfo {
    // First draw slot; LOD n is slot + n.
    slot: u32,
    // icon shape | tech << 8 | (1 << 16 when the model is a mobile unit)
    icon: u32,
    bounds_radius: f32,
    height: f32,
    // Metres of the hull-plan atlas: [-1, 1] in plan UV is this square.
    plan_half: f32,
    // The refit modules on show, one look bit each (`Blueprints::look`).
    modules: u32,
    // A pit the model digs into the ground: the height of its opening and its radius there.
    // Zero for none. What is inside and below the opening is drawn at the opening's depth.
    pit: vec2<f32>,
    turret_pivot: vec4<f32>,
    // w: how far a pile driver (part 9, `models::part::RAM`) is hauled up on its beat.
    spinner_pivot: vec4<f32>,
    // A walker's left leg at rest: hip (w: ground covered by one cycle, 0 for no legs),
    // knee (w: how high a foot lifts), ankle (w: share of the cycle a foot is planted).
    leg_hip: vec4<f32>,
    leg_knee: vec4<f32>,
    leg_ankle: vec4<f32>,
    // The left elbow that forearms pitch about (the right one is its mirror image).
    // w: 0 none, 1 elbow-only, 2 two-bone boom (shoulder is turret_pivot).
    arm_pivot: vec4<f32>,
    // Rest-space barrel axis (xyz) and how far recoiling verts kick back (w, metres). Zero if none.
    recoil: vec4<f32>,
    // Hinge (xyz) of folding gear (`rig::FOLD`) and how far it swings back stowed (w, radians).
    fold: vec4<f32>,
    // Trunnion (xyz) of a mounted turret (`rig::MOUNT`) and its tube's kick-back (w, metres).
    mount: vec4<f32>,
    // The rotary barrels' axis for this loadout: y and z of a point on it (it runs along x), w 1
    // when present. x: the module tag | `until` tag << 6 of the barrels that turn about it.
    spin: vec4<f32>,
    // Wrist (xyz) of the head on the folding gear (`rig::FOLD_HEAD`) and how far it folds
    // back stowed (w, radians). Zero if none.
    fold_wrist: vec4<f32>,
    // A pit's pipe feed (`models::Pit`): where the next section waits (xy), the section's
    // length (z), and how far the rig rises onto its stilts in water (w).
    pit_feed: vec4<f32>,
    // x: the model's size for its surface, guns and arms at rest (`Model::surface_reach`);
    // y: the height its running gear's dust reaches (`Model::dust_line`);
    // z: how far a walker's hips sink in full stride (`Legs::crouch`);
    // w: height of a walker's neck (`rig::HEAD`), zero for none; its x is turret_pivot.w.
    surface: vec4<f32>,
    // Gun houses of their own (limbs 11..15): pivot (xyz) and kick-back travel (w).
    houses: array<vec4<f32>, 4>,
    // Which weapon each house is bound to, plus one; zero for no house in that slot.
    house_weapon: vec4<f32>,
    // A spacecraft's rig (`models::capital::CapitalRig::gpu`), zero for any other model:
    // [0] fore legs hinge x, |y|, z, stow way (+1 foot swings aft, -1 forward; 0: no gear);
    // [1] aft legs the same; [2] bay door hinges |y|: fore inner, outer, aft inner, outer;
    // [3] door hinge z, fore leg size, aft leg size (1: a 36 m leg), unused;
    // [4] stern drives: mouth x, z, |y| of the inner and outer pair (x 0: none);
    // [5] lift jets: fore x, |y|, aft x, |y| (0: none); [6] lift jet mouth z, drive size
    // (1: a 17 m deep bell), belly ramp hinge x, z (z 0: no ramp).
    capital: array<vec4<f32>, 7>,
    // Houses 4..8 (`rig::HOUSE_HIGH`), as `houses` and `house_weapon`.
    houses_high: array<vec4<f32>, 4>,
    house_weapon_high: vec4<f32>,
    // Where a personal (hull) shield is thrown from, bind pose (`Model::shield_emitter`);
    // w 1 when the model says, zero for the default (the top of the hull over the middle).
    shield_emitter: vec4<f32>,
    // A many-legged walker (`models::Crawl::gpu`), zero for any other model: [0] x pair
    // count, y the tail's root height, z its top (`rig::TAIL`); then per pair hip (w: where
    // in the cycle its left foot lifts), knee, foot tip; w of [0] the tail's joint count.
    // Then [13..19) the tail's joints two to a vec4 (x, z, x, z), [19] the pincer's
    // shoulder (w 1 when it has pincers), [20] its jaw hinge. Size mirrors CRAWL_SLOTS.
    crawl: array<vec4<f32>, 21>,
    // A reverse-kneed walker's left hock at rest (xyz, between the knee and the ankle) and how
    // much of the leg's swing the tarsus below it follows (w). All zero for any other model.
    leg_hock: vec4<f32>,
}

// Mirrors mc_sim::mirror::HousePose (192 bytes): per weapon yaw off the hull last tick and
// this, pitch last tick and this; then each weapon's kick-back last tick and this, two per weapon.
//!rust mc_sim::mirror::HousePose
struct HousePose {
    pose: array<vec4<f32>, 8>,
    kick: array<vec4<f32>, 4>,
}

const KIND_WRECK: u32 = 0x80000000u;
const KIND_PROP: u32 = 0x40000000u;
const KIND_GHOST: u32 = 0x20000000u;
const FLAG_UNDER_CONSTRUCTION: u32 = 0x100u;
const FLAG_IN_FACTORY: u32 = 0x200u;
const FLAG_BUILDING: u32 = 0x400u;
const FLAG_MOVING: u32 = 0x800u;
const FLAG_UPGRADE: u32 = 0x2000u;
const STATE_RADAR: u32 = 0x2000000u;
const STATE_UNIDENTIFIED: u32 = 0x4000000u;
const STATE_UNPOWERED: u32 = 0x8000000u;
const STATE_CHARGING: u32 = 0x10000000u;
// Entity `status[0]` (`mirror::UNIT_PAUSED`): the player paused this unit's work.
const UNIT_PAUSED: u32 = 0x200u;


// Top bit of a visible-list index: the entity lives in the dynamic buffer.
const DYNAMIC_BIT: u32 = 0x80000000u;
const NOT_VISIBLE: u32 = 0xFFFFFFFFu;

const PI: f32 = 3.14159265;

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    var d = b - a;
    d = d - floor((d + PI) / (2.0 * PI)) * 2.0 * PI;
    return a + d * t;
}

fn rot_z(v: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

fn hash11(n: f32) -> f32 {
    return fract(sin(n * 12.9898) * 43758.5453);
}

fn hash21(p: vec2<f32>) -> f32 {
    // Stable at large world coordinates. A `sin` hash of the lattice locks
    // into a visible grid once `xy` is tens of kilometres.
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Fold world metres so the lattice stays inside a few thousand cells: f32 still
// has sub-cell precision, and the repeat is kilometres — past a playable map.
// Folded in cells, not metres: a negative point folded in metres was pushed up
// by the whole span, and at a span of 10^8 m f32 lost metres of it.
const LATTICE_CELLS: f32 = 8192.0;
fn noise_lattice(xy: vec2<f32>, cell: f32) -> vec2<f32> {
    let p = xy / cell;
    return p - LATTICE_CELLS * floor(p / LATTICE_CELLS);
}

// A lattice corner taken round the fold, so the corner past the last cell is
// the first one again. Hashed as it was, the noise jumped along a straight
// line wherever its input crossed zero (the clouds' drift puts that on the map).
fn lattice_corner(i: vec2<f32>) -> vec2<f32> {
    return select(i, i - LATTICE_CELLS, i >= vec2<f32>(LATTICE_CELLS));
}

// Value noise of `xy` with one feature every `cell` metres. A pure function of
// position, so it never tiles with the noise texture.
fn value_noise2(xy: vec2<f32>, cell: f32) -> f32 {
    let p = noise_lattice(xy, cell);
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(lattice_corner(i + vec2<f32>(1.0, 0.0))), u.x),
        mix(hash21(lattice_corner(i + vec2<f32>(0.0, 1.0))), hash21(lattice_corner(i + vec2<f32>(1.0, 1.0))), u.x),
        u.y
    );
}

fn ihash21(ix: i32, iy: i32) -> u32 {
    var n = u32(ix) * 1597334677u ^ u32(iy) * 3812015801u;
    n ^= n >> 16u;
    n = n * 2246822519u;
    n ^= n >> 13u;
    n = n * 3266489917u;
    n ^= n >> 16u;
    return n;
}

fn grad_vec(i: vec2<f32>) -> vec2<f32> {
    let h = ihash21(i32(i.x) & 8191, i32(i.y) & 8191);
    let a = f32(h >> 8u) * (6.2831853 / 16777216.0);
    return vec2<f32>(cos(a), sin(a));
}

// Gradient noise, [0, 1]. Quintic fade, so cell edges do not crease into lines
// the way a value-noise lattice does.
fn grad_noise2(xy: vec2<f32>, cell: f32) -> f32 {
    let p = noise_lattice(xy, cell);
    let i = floor(p);
    let f = p - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let v00 = dot(grad_vec(i), f);
    let v10 = dot(grad_vec(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let v01 = dot(grad_vec(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let v11 = dot(grad_vec(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));
    return clamp(mix(mix(v00, v10, u.x), mix(v01, v11, u.x), u.y) * 0.70710678 * 0.5 + 0.5, 0.0, 1.0);
}

// ACES filmic curve (Narkowicz fit).
fn tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

struct Pbr {
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,
    emissive: vec3<f32>,
}

fn d_ggx(n_dot_h: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

fn g_smith(n_dot_v: f32, n_dot_l: f32, rough: f32) -> f32 {
    let k = (rough + 1.0) * (rough + 1.0) / 8.0;
    return (n_dot_v / (n_dot_v * (1.0 - k) + k)) * (n_dot_l / (n_dot_l * (1.0 - k) + k));
}

// Specular anti-aliasing (Kaplanyan and Hoffman 2016): where the normal turns
// faster than a pixel can hold, the highlight widens instead of sparkling.
// Returns the roughness to shade with.
fn specular_aa(n: vec3<f32>, roughness: f32) -> f32 {
    let dx = dpdx(n);
    let dy = dpdy(n);
    let variance = 0.25 * (dot(dx, dx) + dot(dy, dy));
    let kernel = min(2.0 * variance, 0.18);
    return sqrt(clamp(roughness * roughness + kernel, 0.0, 1.0));
}

// The split-sum environment BRDF, fitted analytically (Karis 2014, "Physically
// Based Shading on Mobile"): the specular reflectance averaged over the lobe is
// `f0 * x + y`. `x + y` is the share of light a single bounce off the
// microfacets returns; the rest bounces again (`energy_compensation`).
fn env_brdf(rough: f32, n_dot_v: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = rough * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

// Single-scatter GGX loses the light that bounces between microfacets, so rough
// metal goes dull; scale the lobe back up by what was lost (Filament).
fn energy_compensation(f0: vec3<f32>, ab: vec2<f32>) -> vec3<f32> {
    return 1.0 + f0 * (1.0 / max(ab.x + ab.y, 0.05) - 1.0);
}

// Cook-Torrance with one sun and a sky/ground hemisphere for ambient. `sun_rgb`
// is the sun's light where it reaches the ground, `sky_rgb` the sky's
// irradiance from straight up, `ground_rgb` light bounced up off the land, and
// `sky_vis` how much of the sky the point can see (1 open ground, less in a gully).
fn shade_pbr_env(m: Pbr, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32,
    sun_rgb: vec3<f32>, sky_rgb: vec3<f32>, ground_rgb: vec3<f32>, sky_vis: f32) -> vec3<f32> {
    // What a glossy face mirrors: the sky's colour above, the land's below.
    let r = reflect(-v, n);
    let sky_refl = mix(ground_rgb, sky_rgb * 1.25, clamp(r.z * 0.5 + 0.5, 0.0, 1.0)) * mix(0.35, 1.0, sky_vis);
    return shade_pbr_refl(m, n, v, l, shadow, sun_rgb, sky_rgb, ground_rgb, sky_vis, sky_refl);
}

// The same, given what the face mirrors (`env_reflection` in bindings.wgsl looks
// up the sky and the clouds for it).
fn shade_pbr_refl(m: Pbr, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32,
    sun_rgb: vec3<f32>, sky_rgb: vec3<f32>, ground_rgb: vec3<f32>, sky_vis: f32, sky_refl: vec3<f32>) -> vec3<f32> {
    let h = normalize(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 0.001);
    let n_dot_h = max(dot(n, h), 0.0);
    let f0 = mix(vec3<f32>(0.04), m.albedo, m.metallic);
    // The bases are clamped: a dot product a hair over one would make them negative, and `pow` of that is a NaN.
    let f = f0 + (1.0 - f0) * pow(clamp(1.0 - dot(h, v), 0.0, 1.0), 5.0);
    let rough = clamp(m.roughness, 0.06, 1.0);
    let ab = env_brdf(rough, n_dot_v);
    let spec = d_ggx(n_dot_h, rough * rough) * g_smith(n_dot_v, n_dot_l, rough) * f / (4.0 * n_dot_v * max(n_dot_l, 0.001))
        * energy_compensation(f0, ab);
    let diffuse = (1.0 - f) * (1.0 - m.metallic) * m.albedo / PI;
    let direct = (diffuse + spec) * sun_rgb * n_dot_l * shadow;

    // Sky from above and a little more from the sun's side of the sky, the
    // land's bounce from below; a hollow sees less of both.
    let up = n.z * 0.5 + 0.5;
    let toward_sun = clamp(dot(n.xy, l.xy) * 0.5 + 0.5, 0.0, 1.0);
    let sky = (sky_rgb * up * (0.8 + 0.4 * toward_sun) + ground_rgb * (1.0 - up)) * sky_vis;
    // Split-sum ambient with multiple scattering (Fdez-Aguera 2019): what the
    // mirror image contributes, plus the light that bounces between microfacets
    // before leaving, so rough metal keeps its brightness. The diffuse gets what
    // the specular did not take.
    let ss = f0 * ab.x + ab.y;
    let e_ss = ab.x + ab.y;
    let f_avg = f0 + (1.0 - f0) / 21.0;
    let f_ms = ss * f_avg / (1.0 - (1.0 - e_ss) * f_avg);
    let spec_amb = ss + f_ms * (1.0 - e_ss);
    let ambient = (1.0 - m.metallic) * m.albedo * (1.0 - spec_amb) * sky + spec_amb * sky_refl;
    return direct + ambient + m.emissive;
}

// Atmosphere, clouds and weather (sky.rs `Atmosphere`, bindings 22).
//!rust crate::sky::Atmosphere
struct Atmosphere {
    // The sun's light at the ground, after the air it came through; w: how
    // much of it the sky dome and clouds show (1 by day, low under the moon).
    sun_color: vec4<f32>,
    // Sky irradiance on a face looking straight up; w: sun disk brightness.
    sky_color: vec4<f32>,
    // The sky just above the horizon.
    horizon_color: vec4<f32>,
    // Light the land bounces back up.
    ground_color: vec4<f32>,
    // xy how far the wind has carried the air (m), zw wind velocity (m/s).
    wind: vec4<f32>,
    // Heights above the cloud floor (`cloud_floor`): base, fair-weather top,
    // storm top; w how much of the sky the air fills (1.2 fair).
    layer: vec4<f32>,
    // Weather map: metres per texel, map width, map height, seconds.
    weather: vec4<f32>,
    // Clearing where the player looks: xy focus, radius, strength.
    view: vec4<f32>,
    // How many clear zones, lightning flashes, disturbers and storms are in use.
    counts: vec4<f32>,
    // Extra clear zones (selection): xy, radius, strength.
    clears: array<vec4<f32>, 8>,
    // Lightning inside a cloud: xyz, w brightness now.
    flashes: array<vec4<f32>, 4>,
    // A flash's stroke to the ground: xy where it lands, z seed, w brightness (0: none).
    bolts: array<vec4<f32>, 4>,
    // Last frame's view-projection, for carrying the clouds' history forward.
    prev_view_proj: mat4x4<f32>,
    // x frame number (wraps at 1024), y 1 when last frame's clouds can be reused,
    // z how far the see-through middle of the screen is in (0 with nothing
    // selected), w the cloud floor's lowest point.
    frame: vec4<f32>,
    // The map's weather: x how towering the clouds are (0-1), y how big the
    // cloud masses (1 usual), z how readily heavy cloud rains (0-1), w the
    // cloud floor's highest point.
    shape: vec4<f32>,
    // Explosions and weapon flashes lighting the clouds (sky.rs `set_glows`), brightest
    // first: per glow xyz and soft radius, then its colour at the middle (w 1 in use).
    glows: array<vec4<f32>, 16>,
    // Storms wheeling round their eye (sky.rs `conjure_storm` with a spin): per vortex
    // xy of the eye, radius, how far round the eye has turned (radians); then x 1 in use,
    // y how far into clearing the air about it once it has rained out.
    vortex: array<vec4<f32>, 4>,
}

// How much faster a wheeling storm's eye turns than its rim (sky.rs `VORTEX_SHEAR`).
const VORTEX_SHEAR: f32 = 1.2;
// How far a wheeling storm draws the air round it in as it turns: per radian turned,
// the air now here came from this much further out.
const VORTEX_INFLOW: f32 = 0.07;

// Where the air now at `xy` was before the wheeling storms in `vortex` turned it and
// drew it in: the weather and the clouds' billows are looked up there, so the whole
// cloud round a vortex wheels, the eye faster than the rim, and the cloud about it is
// pulled in, twisting, as it goes. Past twice a vortex's radius the air is left alone.
fn vortex_warp_in(vortex: array<vec4<f32>, 4>, xy: vec2<f32>) -> vec2<f32> {
    var q = xy;
    for (var i = 0u; i < 2u; i++) {
        let v = vortex[i * 2u];
        if vortex[i * 2u + 1u].x <= 0.0 {
            continue;
        }
        let rel = q - v.xy;
        let d = length(rel) / v.z;
        if d >= 2.0 {
            continue;
        }
        let turn = v.w / (1.0 + VORTEX_SHEAR * d) * (1.0 - smoothstep(1.2, 2.0, d));
        let c = cos(-turn);
        let s = sin(-turn);
        let pull = exp(VORTEX_INFLOW * turn);
        q = v.xy + vec2<f32>(rel.x * c - rel.y * s, rel.x * s + rel.y * c) * pull;
    }
    return q;
}

// How far into a wheeling storm's reach `xy` is: 1 within its radius and a half, easing
// to 0 at twice it.
fn vortex_reach_in(vortex: array<vec4<f32>, 4>, xy: vec2<f32>) -> f32 {
    var r = 0.0;
    for (var i = 0u; i < 2u; i++) {
        let v = vortex[i * 2u];
        if vortex[i * 2u + 1u].x > 0.0 {
            r = max(r, 1.0 - smoothstep(1.5, 2.0, distance(xy, v.xy) / v.z));
        }
    }
    return r;
}

// Rayleigh scattering of sea-level air per metre, and its scale height.
const RAYLEIGH: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const RAYLEIGH_H: f32 = 8000.0;
// Haze (Mie): scattering per metre at sea level and its scale height.
const MIE: f32 = 8.0e-6;
const MIE_H: f32 = 1400.0;

fn phase_rayleigh(mu: f32) -> f32 {
    return 0.0596831 * (1.0 + mu * mu);
}

fn phase_hg(mu: f32, g: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * mu;
    return 0.0795775 * (1.0 - g * g) / (d * sqrt(d));
}

// Mass of air an exponential layer of scale height `h` puts between `a` and
// `b`, as metres of sea-level air. Exact for a flat world.
fn air_column(a: vec3<f32>, b: vec3<f32>, h: f32) -> f32 {
    let dist = distance(a, b);
    let za = max(a.z, -50.0);
    let zb = max(b.z, -50.0);
    let dz = zb - za;
    var mean = exp(-za / h);
    if abs(dz) > 1.0 {
        mean = h * (exp(-za / h) - exp(-zb / h)) / dz;
    }
    return mean * dist;
}

// The weather the air mass itself carries, without storms or anything that
// disturbed it: x cloud cover (0 clear, 1 overcast column), y how convective
// it is. `drift` is how far the wind has carried the air; `scale` biases cover.
// Fields of cloud cells, then the cells, `s` times their usual size, warped so
// they drift into banks rather than sit on a lattice.
fn climate_field(p: vec2<f32>, s: f32) -> f32 {
    let warp = vec2<f32>(grad_noise2(p, 5200.0 * s), grad_noise2(p + 911.0, 4700.0 * s)) * 1900.0 * s;
    let q = p + warp;
    return grad_noise2(q, 3600.0 * s) * 0.55 + grad_noise2(q + 71.0, 1500.0 * s) * 0.3 + grad_noise2(q - 233.0, 640.0 * s) * 0.15;
}

// Shared by the weather simulation (its rest state) and the sky beyond the map.
// `size` scales the cloud masses (1 usual); some stretches of sky grow them
// bigger still, so now and then a vast bank drifts over.
fn cloud_climate(xy: vec2<f32>, drift: vec2<f32>, scale: f32, size: f32) -> vec2<f32> {
    let p = xy - drift;
    let vast = smoothstep(0.45, 0.8, grad_noise2(p + 5711.0, 26000.0));
    // Two fixed sizes of cloud mass, blended: never one whose size varies from
    // place to place. Noise at `p / size(p)` is stretched in proportion to the
    // distance from the origin, and `p` grows with the drift, so that drew
    // streaks fanning across the map and seams that crawled as the wind blew.
    let s = max(size, 0.3);
    var field = 0.0;
    if vast < 1.0 {
        field += climate_field(p, s * 0.8) * (1.0 - vast);
    }
    if vast > 0.0 {
        field += climate_field(p + 3301.0, s * 2.1) * vast;
    }
    // Fronts tens of kilometres across.
    let front = grad_noise2(p * 0.5 + 3170.0, 9000.0);
    let cover = smoothstep(0.50, 0.72, field + (front - 0.5) * 0.55 + (scale - 1.0) * 0.25);
    let convect = smoothstep(0.55, 0.85, front) * cover;
    return vec2<f32>(cover, convect);
}

// Shared shockwave envelope and radial bands. Width has a pixel floor so a
// pressure front stays legible at gameplay zoom without filling its interior.
fn shockwave_fade(age: f32) -> f32 {
    let remaining = max(1.0 - age, 0.0);
    let expansion = 1.0 - remaining * remaining;
    // The same pressure spreads over an increasing surface area. Both the
    // visible front and refraction thin out well before the wave expires.
    let dilution = 1.0 / (1.0 + 3.5 * expansion * expansion);
    return smoothstep(0.0, 0.045, age) * pow(remaining, 1.25) * dilution;
}

fn shockwave_bands(facing: f32, radius_px: f32) -> vec3<f32> {
    let inset = 1.0 - sqrt(max(1.0 - facing * facing, 0.0));
    let width = clamp(1.8 / max(radius_px, 1.0), 0.007, 0.05);
    let edge = smoothstep(0.0, width * 0.7, inset);
    let core = exp(-pow(abs((inset - width * 1.8) / width), 2.0)) * edge;
    let vapor = exp(-pow(abs((inset - width * 4.0) / (width * 3.0)), 2.0)) * edge;
    // Compression followed by a weaker opposite bend, both continuous at the edge.
    let bend = core - vapor * 0.35;
    return vec3<f32>(core, vapor, bend);
}

// Compact, live physical barriers shared by all blast passes.
//!rust crate::renderer::EffectBarrier
struct EffectBarrier {
    center: vec3<f32>,
    radius: f32,
    inverse_axes: vec3<f32>,
    min_z: f32,
}
struct EffectBarriers {
    header: vec4<u32>,
    entries: array<EffectBarrier>,
}
fn barrier_crosses(source: vec3<f32>, to: vec3<f32>, barrier: EffectBarrier) -> bool {
    let q = (source - barrier.center) * barrier.inverse_axes;
    let v = (to - source) * barrier.inverse_axes;
    let a = dot(v, v);
    let c = dot(q, q) - 1.0;
    if a < 0.0000001 || (c < -0.0001 && dot(q + v, q + v) < 0.9999) { return false; }
    let b = dot(q, v);
    let disc = b * b - a * c;
    if disc <= 0.0 { return false; }
    let roots = vec2<f32>((-b - sqrt(disc)) / a, (-b + sqrt(disc)) / a);
    for (var i = 0u; i < 2u; i++) {
        let t = roots[i];
        if t >= -0.0001 && t <= 1.0 && (t > 0.0001 || b < 0.0)
            && (source + (to - source) * t).z >= barrier.min_z - 0.1 { return true; }
    }
    return false;
}

// Shield domes and the hits rippling over them (renderer `GpuShield`, `ShieldHit`),
// read by the entity and shield passes.
//!rust crate::renderer::GpuShield
struct Shield {
    pos: vec3<f32>,
    radius: f32,
    prev_open: f32,
    open: f32,
    health: f32,
    packed: u32,
    unit_id: u32,
    projector: f32,
    height: f32,
    overlap: u32,
    contact_n: u32,
    // `radius` last tick while an upgraded dome swells out; zero when the same.
    prev_radius: f32,
    _pad1: u32,
    _pad2: u32,
    contacts: array<u32, 16>,
}

//!rust crate::renderer::ShieldHit
struct ShieldHit {
    pos: vec3<f32>,
    start: f32,
    strength: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

// Blast fronts (renderer `GpuShockwave`), read by the shockwave and screen passes.
//!rust crate::renderer::GpuShockwave
struct Shockwave {
    pos: vec3<f32>,
    start: f32,
    // x reach in metres, y lifetime seconds, z weapon colour (0 blue, 1 orange), w how hard the front is
    params: vec4<f32>,
    // Barrel direction for a muzzle blast; zero for an isotropic sphere.
    axis: vec3<f32>,
    _pad: f32,
    tint: vec4<f32>,
}

// A mark on the ground (the renderer's stains buffer): the sim's scorch marks,
// wreck craters and molten ground, then the structures' lots (ground.wgsl draws
// them; grass_gen.wgsl keeps grass off lots and chars it on scorches).
//!rust mc_sim::mirror::StainInstance
struct Stain {
    pos: vec2<f32>,
    radius: f32,
    strength_seed: u32,
}

// One stretch of track marks: both tracks of a vehicle between two points of
// its path. Written once into a ring; fades out with age.
//!rust crate::renderer::TrackMark
struct TrackMark {
    start_xy: vec2<f32>,
    end_xy: vec2<f32>,
    // Centre line to the middle of each track, and one track's width.
    half_gauge: f32,
    width: f32,
    start: f32,
    life: f32,
}
