// Shared by every shader (build.rs prepends this file).
// Conventions: world is metres, Z up. Clip space is WebGPU-style (naga flips Y
// for Vulkan); depth is reversed-Z, 1 at the near plane and 0 at infinity.

struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    shadow_view_proj: mat4x4<f32>,
    // xyz eye position, w time in seconds
    camera: vec4<f32>,
    // xyz unit vector toward the sun, w interpolation factor between the last two sim ticks
    sun: vec4<f32>,
    // width, height, 1/width, 1/height
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
}

// Mirrors mc_sim::mirror::UnitInstance (64 bytes).
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
    scale: u32,
}

// One per blueprint / prop kind.
struct ModelInfo {
    // First draw slot; LOD n is slot + n.
    slot: u32,
    // icon shape | tech << 8 | (1 << 16 when the model is a mobile unit)
    icon: u32,
    bounds_radius: f32,
    height: f32,
    turret_pivot: vec4<f32>,
    spinner_pivot: vec4<f32>,
}

const KIND_WRECK: u32 = 0x80000000u;
const KIND_PROP: u32 = 0x40000000u;
const KIND_GHOST: u32 = 0x20000000u;
const FLAG_UNDER_CONSTRUCTION: u32 = 0x100u;
const FLAG_IN_FACTORY: u32 = 0x200u;
const FLAG_BUILDING: u32 = 0x400u;
const FLAG_MOVING: u32 = 0x800u;
const FLAG_UPGRADE: u32 = 0x2000u;

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
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
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

// Atmospheric haze: far terrain fades toward the horizon colour.
fn apply_haze(color: vec3<f32>, world: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let dist = distance(world, eye);
    // Looking straight down there is little air in the way; toward the horizon, a lot.
    let grazing = 1.0 - abs(normalize(world - eye).z);
    let amount = (1.0 - exp(-dist * 0.000012)) * grazing * grazing;
    return mix(color, vec3<f32>(0.55, 0.66, 0.8), clamp(amount, 0.0, 0.6));
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

// Cook-Torrance with one sun and a sky/ground hemisphere for ambient.
fn shade_pbr(m: Pbr, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32) -> vec3<f32> {
    let h = normalize(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 0.001);
    let n_dot_h = max(dot(n, h), 0.0);
    let f0 = mix(vec3<f32>(0.04), m.albedo, m.metallic);
    let f = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    let rough = clamp(m.roughness, 0.06, 1.0);
    let spec = d_ggx(n_dot_h, rough * rough) * g_smith(n_dot_v, n_dot_l, rough) * f / (4.0 * n_dot_v * max(n_dot_l, 0.001));
    let diffuse = (1.0 - f) * (1.0 - m.metallic) * m.albedo / PI;
    let sun_color = vec3<f32>(1.0, 0.95, 0.86) * 3.4;
    let direct = (diffuse + spec) * sun_color * n_dot_l * shadow;

    let sky = mix(vec3<f32>(0.20, 0.19, 0.17), vec3<f32>(0.42, 0.55, 0.78), n.z * 0.5 + 0.5);
    let f_amb = f0 + (max(vec3<f32>(1.0 - rough), f0) - f0) * pow(1.0 - n_dot_v, 5.0);
    let r = reflect(-v, n);
    let sky_refl = mix(vec3<f32>(0.22, 0.21, 0.2), vec3<f32>(0.5, 0.64, 0.86), clamp(r.z * 0.5 + 0.5, 0.0, 1.0));
    let ambient = (1.0 - m.metallic) * m.albedo * sky * 0.6 + f_amb * sky_refl * (1.0 - rough * 0.7) * 0.8;
    return direct + ambient + m.emissive;
}
