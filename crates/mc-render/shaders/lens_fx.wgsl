//!use bindings
// A gravitic lens at work, the Lodestone's (renderer/lens_fx.rs, lens_fx/pass.rs): one
// record a lens, drawn by four meshes over the scene after the water. Each bends the scene
// behind it, read from the copy the water sees through (set 2), and darkens it; the light
// is the Fallen's violet energy wound round that dark, at moderate gains. (Its trail in the
// ground is the terrain's, scars.wgsl `RIFT_TRAIL`.)
//
// - **The well** (`vs_lens_well`): a disc laid over the ground round the target, carried
//   with it, darkened, rippling in toward the middle with thin violet arms spiralling in,
//   its edge a pressed ring with violet energy crawling along it; over it a low dome of
//   bent air, seen at its edge and shimmering down over what stands in it. The strike's
//   violet ring closes over the disc; the release's pulse runs out over it; the pressed
//   ring is left fading.
// - **The grit** (`vs_lens_shard`): chips of rock lifted off the ground in the well,
//   circling slowly low down, dropping when it lets go.
// - **The air** (`vs_lens_ribbon`): a corridor of rippled air behind a running front with
//   a frayed violet wake in it, and a thread of it from the lens to the well it holds,
//   violet pulses running out along it.
// - **The charge, the front, the strike** (`vs_lens_ball`): turned to the eye, a black
//   heart with the scene behind it bent round it as round a heavy mass, a thin rim where
//   the light bends hardest, violet filaments swirling round it (`lens_energy`); drawn
//   larger far off. Rings of bent air close on the charge, and a violet one on the target
//   as the front strikes; then the dark flash throws off a ragged violet ring.
//
// Premultiplied, depth-tested, writing no depth; no `discard` (AMD).

//!rust crate::renderer::lens_fx::pass::GpuLens
struct LensWork {
    // The emitter, and the renderer time its charge began (below zero: none).
    emitter: vec3<f32>,
    charge: f32,
    // Where the front is (struck: the middle of the target), and when it was let go.
    front: vec3<f32>,
    launch: f32,
    // The ground under the target, the well's foot, and when the front struck.
    ground: vec3<f32>,
    strike: f32,
    // The well's radius, metres; seconds the charge takes; when the well let go.
    radius: f32,
    charge_time: f32,
    release: f32,
    seed: f32,
    // 0 gone, 1 full; metres the front has run.
    fade: f32,
    run: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(1) @binding(0) var<storage, read> lenses: array<LensWork>;
@group(2) @binding(0) var seen_scene: texture_2d<f32>;
@group(2) @binding(2) var seen_sampler: sampler;
@group(2) @binding(7) var seen_depth: texture_depth_2d;

// The Regency's red and violet, and the rim between them: kept low, since a bright violet
// blooms to white.
const L_RED: vec3<f32> = vec3<f32>(1.0, 0.07, 0.04);
const L_VIOLET: vec3<f32> = vec3<f32>(0.55, 0.16, 1.0);
const L_RIM: vec3<f32> = vec3<f32>(0.78, 0.11, 0.52);
// The Fallen's energy round a dark heart: violet, and lilac where it runs hottest. Kept
// to moderate gains: a bright violet line blooms to white.
const L_LILAC: vec3<f32> = vec3<f32>(0.82, 0.6, 1.0);
// The grit: dark rock.
const L_ROCK: vec3<f32> = vec3<f32>(0.07, 0.063, 0.058);

fn hidden_clip() -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, -1.0);
}

// Seconds since `when`, or a long way back when it has not happened (below zero).
fn since(when: f32) -> f32 {
    return select(-1.0e6, globals.camera.w - when, when >= 0.0);
}

// How fully the well holds: 0 before it opens, up while the strike's ring closes, 1 while
// it holds, back to 0 as it lets go.
fn well_held(s: LensWork) -> f32 {
    let a = since(s.strike);
    let open = smoothstep(LENS_FX_IMPLODE * 0.6, LENS_FX_IMPLODE + 0.3, a);
    let r = since(s.release);
    let letting = select(1.0, 1.0 - smoothstep(0.0, LENS_FX_RELEASE * 0.6, r), s.release >= 0.0);
    return open * letting * s.fade;
}

// Metres the well's disc is laid over the ground: over the grass.
const LIFT: f32 = 1.0;

fn ground_or_sea(xy: vec2<f32>) -> f32 {
    return max(terrain_height(xy), globals.map.z);
}

// The corner of quad `quad` that vertex `corner` (0..6) of it stands on, as steps along
// the grid's rows (`x`) and round it (`y`).
fn lens_grid(quad: u32, corner: u32, around: u32) -> vec2<u32> {
    var di = 0u;
    var dj = 0u;
    switch corner {
        case 1u, 3u: {
            dj = 1u;
        }
        case 2u, 5u: {
            di = 1u;
        }
        case 4u: {
            di = 1u;
            dj = 1u;
        }
        default: {}
    }
    return vec2<u32>(quad / around + di, quad % around + dj);
}

// ---------------------------------------------------------------- the bent scene

// The scene the water saw at world point `p`, and whether it may be laid there (w): not
// where what it shows stands nearer than `distance` (a hull in front of the lens), under
// the sea (the copy has no water in it) or open sky (nothing copied).
fn bent_scene(p: vec3<f32>, reach: f32) -> vec4<f32> {
    let clip = globals.view_proj * vec4<f32>(p, 1.0);
    if clip.w < 0.5 {
        return vec4<f32>(0.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(uv < vec2<f32>(0.001)) || any(uv > vec2<f32>(0.999)) {
        return vec4<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(seen_depth));
    let px = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    let depth = textureLoad(seen_depth, px, 0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    let world = h.xyz / h.w;
    let behind = distance(globals.camera.xyz, world) > reach - 1.5;
    let dry = world.z > globals.map.z + 0.05;
    let ok = select(0.0, 1.0, depth > 0.0000002 && behind && dry);
    let c = textureSampleLevel(seen_scene, seen_sampler, uv, 0.0).rgb;
    return vec4<f32>(min(max(c, vec3<f32>(0.0)), vec3<f32>(64.0)), ok);
}

// How far the scene stands behind a fragment at `clip`, metres (soft where the lens
// meets what is behind it).
fn scene_gap(clip: vec4<f32>, world: vec3<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(seen_depth));
    let uv = clip.xy / globals.scene.xy;
    let px = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    let depth = textureLoad(seen_depth, px, 0);
    if depth <= 0.0000002 {
        return 1.0e6;
    }
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    let eye = globals.camera.xyz;
    return distance(eye, h.xyz / h.w) - distance(eye, world);
}

// `bent` laid over the scene with weight `w`, darkened by `dark`, with `light` added:
// premultiplied.
fn lay(bent: vec4<f32>, w: f32, dark: f32, light: vec3<f32>) -> vec4<f32> {
    let k = clamp(w * bent.a, 0.0, 1.0);
    let d = clamp(dark, 0.0, 1.0);
    return vec4<f32>(bent.rgb * k * (1.0 - d) + light, k + d - k * d);
}

// ---------------------------------------------------------------- the well

struct WellOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Out from the middle over the radius; 1 on the dome, 0 on the ground.
    @location(2) at: vec2<f32>,
    @location(3) @interpolate(flat) lens: u32,
}

@vertex
fn vs_lens_well(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> WellOut {
    let s = lenses[instance];
    var out: WellOut;
    out.lens = instance;
    let r = since(s.release);
    let gone = select(false, r > LENS_FX_PRINT, s.release >= 0.0);
    if s.strike < 0.0 || s.fade <= 0.0 || gone {
        out.clip = hidden_clip();
        return out;
    }
    let quad = v / 6u;
    // The dome first, then the disc over it: where the ground shows, the disc's own
    // bending and darkening win; the dome's is left on what stands in the well and on
    // what lies past its edge.
    let dome = LENS_FX_DOME_RINGS * LENS_FX_SEGMENTS;
    let mid = s.ground.xy;
    let radius = s.radius;
    if quad >= dome {
        let g = lens_grid(quad - dome, v % 6u, LENS_FX_SEGMENTS);
        let out_r = f32(g.x) / f32(LENS_FX_RINGS) * LENS_FX_DISC_REACH;
        let angle = f32(g.y) / f32(LENS_FX_SEGMENTS) * 2.0 * PI;
        let xy = mid + vec2<f32>(cos(angle), sin(angle)) * out_r * radius;
        // Laid over the grass, not among its blades; the fragment finds the ground it
        // covers from the scene's depth (`well_ground`).
        let p = vec3<f32>(xy, ground_or_sea(xy) + LIFT);
        out.world = p;
        out.normal = vec3<f32>(0.0, 0.0, 1.0);
        out.at = vec2<f32>(out_r, 0.0);
        out.clip = globals.view_proj * vec4<f32>(p, 1.0);
        return out;
    }
    // The dome: only while it holds and as it lets go.
    if select(false, r > LENS_FX_RELEASE, s.release >= 0.0) {
        out.clip = hidden_clip();
        return out;
    }
    let g = lens_grid(quad, v % 6u, LENS_FX_SEGMENTS);
    let up = f32(g.x) / f32(LENS_FX_DOME_RINGS) * 0.5 * PI;
    let angle = f32(g.y) / f32(LENS_FX_SEGMENTS) * 2.0 * PI;
    let around = vec2<f32>(cos(angle), sin(angle));
    let tall = LENS_FX_DOME_HEIGHT * radius * max(well_held(s), 0.05);
    let xy = mid + around * cos(up) * radius;
    let foot = ground_or_sea(xy);
    let top = s.ground.z + tall;
    let z = mix(foot, max(top, foot), sin(up));
    let p = vec3<f32>(xy, z);
    out.world = p;
    out.normal = normalize(vec3<f32>(around * cos(up) / radius, sin(up) / max(tall, 0.5)));
    out.at = vec2<f32>(cos(up), 1.0);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

// The scene at a fragment, as a world point, or `fallback` where it is open sky.
fn scene_point(clip: vec4<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(seen_depth));
    let uv = clip.xy / globals.scene.xy;
    let px = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    let depth = textureLoad(seen_depth, px, 0);
    if depth <= 0.0000002 {
        return fallback;
    }
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    return h.xyz / h.w;
}

fn well_ground(in: WellOut, s: LensWork) -> vec4<f32> {
    // The ground (or grass) this fragment covers; nothing standing taller than the grass.
    let world = scene_point(in.clip, in.world);
    let stand = world.z - ground_or_sea(world.xy);
    let keep = 1.0 - smoothstep(LIFT, LIFT + 0.8, stand);
    let rr = length(world.xy - s.ground.xy) / s.radius;
    let px = max(fwidth(rr), 0.0005);
    let t = globals.camera.w;
    let a = since(s.strike);
    let r = since(s.release);
    let released = s.release >= 0.0;
    let held = well_held(s);
    let off = world.xy - s.ground.xy;
    let dir = off / max(length(off), 0.001);
    let metres = s.radius / 30.0;
    // Waves running in toward the middle, a few metres apart, and a slow churn.
    let phase = rr * s.radius / 6.0 + t * 0.7;
    let body = smoothstep(0.05, 0.3, rr) * (1.0 - smoothstep(0.8, 1.0, rr));
    let churn = value_noise2(world.xy * 0.3 + vec2<f32>(s.seed, t * 0.4), 1.0) - 0.5;
    let wave = sin(phase * 2.0 * PI);
    var bend = dir * (wave * 0.8 + churn * 0.5) * body * held;
    // Each wave's trough a shade darker, so the waves read running in over even ground.
    var dark = 0.07 * (0.5 - 0.5 * wave) * body * held;
    var light = vec3<f32>(0.0);
    // The strike's ring closing in over the ground.
    if a >= 0.0 && a < LENS_FX_IMPLODE {
        let f = a / LENS_FX_IMPLODE;
        let ring = 1.2 * (1.0 - f * f);
        let x = (rr - ring) / 0.07;
        let g = exp(-x * x);
        bend -= dir * 2.4 * metres * x * g;
        dark += 0.4 * g;
        light += L_VIOLET * 0.35 * g * (0.5 + 0.5 * f);
    }
    // The push after the dark flash: bent air running out over the ground to the edge.
    let after = a - LENS_FX_IMPLODE - 0.05;
    if after >= 0.0 && after < 0.35 {
        let f = after / 0.35;
        let ring = 0.15 + 0.9 * sqrt(f);
        let x = (rr - ring) / 0.08;
        let g = exp(-x * x) * (1.0 - f);
        bend += dir * 1.8 * metres * x * g;
        dark += 0.15 * g;
    }
    // The release's soft pulse running out past the edge.
    if released && r >= 0.0 && r < LENS_FX_RELEASE {
        let f = r / LENS_FX_RELEASE;
        let ring = 1.0 + 0.45 * sqrt(f);
        let x = (rr - ring) / 0.08;
        let g = exp(-x * x) * (1.0 - f);
        bend += dir * 1.3 * metres * x * g;
        dark += 0.12 * g;
    }
    let seen = bent_scene(world + vec3<f32>(bend, 0.0), distance(globals.camera.xyz, world));
    let w = clamp(length(bend) / 0.06, 0.0, 1.0);
    // Darkened inside, a shallow trough pressed just inside the edge.
    let inside = 1.0 - smoothstep(1.0 - px * 1.5, 1.0 + px * 1.5, rr);
    let print = select(held, (1.0 - smoothstep(LENS_FX_RELEASE, LENS_FX_PRINT, r)) * s.fade, released);
    let trough = exp(-pow((rr - 0.955) / max(0.03, px * 2.0), 2.0));
    // The ground pushed up just outside the trough, catching a little more light.
    let lip = exp(-pow((rr - 1.025) / max(0.02, px * 2.0), 2.0));
    let pressed = smoothstep(LENS_FX_IMPLODE, LENS_FX_IMPLODE + 0.15, a);
    let press = pressed * max(print, held);
    dark += 0.14 * inside * held + 0.45 * trough * press;
    light += vec3<f32>(0.035, 0.03, 0.022) * lip * press;
    // The thin violet line at the edge, energy crawling along it.
    let edge_w = px * 1.6 + 0.006;
    let edge = 1.0 - smoothstep(0.0, edge_w, abs(rr - 1.0));
    let ang = atan2(off.y, off.x);
    let crawl = 0.45 + 0.55 * smoothstep(0.35, 0.8, value_noise2(vec2<f32>(ang * 5.0 + t * 1.7, t * 0.9) + s.seed, 1.0));
    light += (L_RIM * 0.04 + L_VIOLET * 0.22 * crawl) * edge * held;
    // Violet energy spiralling in over the ground toward the middle, in thin arms.
    let arms = pow(0.5 + 0.5 * sin(ang * 3.0 + log(max(rr, 0.02)) * 4.0 + t * 2.4 + s.seed), 10.0);
    let thread = smoothstep(0.4, 0.75, value_noise2(world.xy * 0.18 + vec2<f32>(t * 0.6, s.seed), 1.0));
    light += L_VIOLET * 0.09 * arms * (0.3 + 0.7 * thread) * body * held;
    return lay(seen, w, dark, light) * keep;
}

fn well_dome(in: WellOut, s: LensWork, front: bool) -> vec4<f32> {
    // Its far side is not drawn: the near side bends what stands in it.
    if !front {
        return vec4<f32>(0.0);
    }
    let held = well_held(s);
    let t = globals.camera.w;
    let n = normalize(in.normal);
    let view = normalize(globals.camera.xyz - in.world);
    let edge = 1.0 - abs(dot(n, view));
    // Pulled toward the middle at its edge, and shimmering down over what stands in it.
    let shimmer = sin(in.world.z * 1.7 + t * 5.5 + in.world.x * 0.21 + in.world.y * 0.17);
    let bend = -n * pow(edge, 2.0) * 0.9 - vec3<f32>(0.0, 0.0, 0.18 * shimmer * (1.0 - in.at.x * 0.5));
    let seen = bent_scene(in.world + bend * held, distance(globals.camera.xyz, in.world));
    let w = clamp(length(bend * held) / 0.08, 0.0, 1.0) * (0.35 + 0.65 * edge);
    let dark = 0.16 * pow(edge, 3.0) * held;
    let light = L_RIM * 0.006 * pow(edge, 6.0) * held;
    return lay(seen, w, dark, light);
}

@fragment
fn fs_lens_well(in: WellOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let s = lenses[in.lens];
    var c: vec4<f32>;
    if in.at.y > 0.5 {
        c = well_dome(in, s, front);
    } else {
        c = well_ground(in, s);
    }
    return under_sea_seen(c, in.clip);
}

// ---------------------------------------------------------------- the grit

struct ShardOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) glow: f32,
    @location(2) @interpolate(flat) fade: f32,
}

fn shard_hash(s: LensWork, i: u32, k: f32) -> f32 {
    return hash11(s.seed * 13.17 + f32(i) * 7.31 + k * 3.97);
}

// How high shard `i` circles at renderer time `t`, lifted once the well opens.
fn shard_height(s: LensWork, i: u32, t: f32) -> f32 {
    let a = t - s.strike;
    let late = 0.4 * shard_hash(s, i, 6.0);
    let lift = smoothstep(LENS_FX_IMPLODE + 0.1 + late, LENS_FX_IMPLODE + 1.3 + late, a);
    let high = 0.5 + 3.4 * pow(shard_hash(s, i, 4.0), 2.0);
    let bob = 0.3 * sin(t * 1.1 + shard_hash(s, i, 2.0) * 6.28);
    return (high + bob) * lift;
}

fn turn3(v: vec3<f32>, a: f32, b: f32) -> vec3<f32> {
    let ca = cos(a);
    let sa = sin(a);
    let cb = cos(b);
    let sb = sin(b);
    let v1 = vec3<f32>(v.x * ca - v.y * sa, v.x * sa + v.y * ca, v.z);
    return vec3<f32>(v1.x, v1.y * cb - v1.z * sb, v1.y * sb + v1.z * cb);
}

@vertex
fn vs_lens_shard(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> ShardOut {
    let s = lenses[instance];
    var out: ShardOut;
    let t = globals.camera.w;
    let i = v / 24u;
    let face = (v % 24u) / 3u;
    let corner = v % 3u;
    let r = since(s.release);
    let released = s.release >= 0.0;
    // Fallen and settled, it sinks out of sight.
    let fall_from = shard_height(s, i, select(t, s.release, released));
    let fall_time = sqrt(2.0 * fall_from / 14.0);
    let after = select(-1.0, r - fall_time, released);
    if s.strike < 0.0 || s.fade <= 0.0 || after > 0.6 || t - s.strike < LENS_FX_IMPLODE {
        out.clip = hidden_clip();
        return out;
    }
    var height = fall_from;
    if released {
        height = max(fall_from - 7.0 * r * r, 0.0);
    }
    let a = max(t - s.strike - LENS_FX_IMPLODE, 0.0);
    let out_r = (0.18 + 0.74 * shard_hash(s, i, 1.0)) * s.radius;
    let speed = (0.22 + 0.2 * shard_hash(s, i, 3.0)) / (0.45 + out_r / s.radius);
    // Circling while it holds; let go, it keeps the way it was going a moment.
    let spun = select(a, a - max(r, 0.0) * 0.5, released);
    let angle = shard_hash(s, i, 2.0) * 6.28 + speed * spun;
    let xy = s.ground.xy + vec2<f32>(cos(angle), sin(angle)) * out_r;
    let size = (0.3 + 0.75 * pow(shard_hash(s, i, 5.0), 2.0)) * (1.0 - smoothstep(0.0, 0.6, after));
    let shape = vec3<f32>(1.0, 0.6 + 0.5 * shard_hash(s, i, 7.0), 0.45 + 0.35 * shard_hash(s, i, 8.0));
    let sx = select(1.0, -1.0, (face & 1u) != 0u);
    let sy = select(1.0, -1.0, (face & 2u) != 0u);
    let sz = select(1.0, -1.0, (face & 4u) != 0u);
    var local = vec3<f32>(sx, 0.0, 0.0);
    if corner == 1u {
        local = vec3<f32>(0.0, sy, 0.0);
    } else if corner == 2u {
        local = vec3<f32>(0.0, 0.0, sz);
    }
    let tumble = select(1.0, 0.0, released && after > 0.0);
    let ta = (t * (0.6 + shard_hash(s, i, 9.0)) + shard_hash(s, i, 10.0) * 6.28) * tumble;
    let tb = (t * (0.4 + shard_hash(s, i, 11.0)) + shard_hash(s, i, 12.0) * 6.28) * tumble;
    let p = vec3<f32>(xy, ground_or_sea(xy) + height + size * 0.5)
        + turn3(local * shape * size, ta, tb);
    out.normal = turn3(normalize(vec3<f32>(sx, sy, sz) / shape), ta, tb);
    out.glow = well_held(s);
    out.fade = s.fade;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_lens_shard(in: ShardOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let sun = max(dot(n, globals.sun.xyz), 0.0);
    // Dark rock in the sun, a trace of the well's red under it.
    let rock = L_ROCK * (0.3 + 1.6 * sun) + L_RIM * 0.004 * max(-n.z, 0.0) * in.glow;
    return under_sea_seen(vec4<f32>(rock, 1.0) * in.fade, in.clip);
}

// ---------------------------------------------------------------- the air

struct RibbonOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // Along (0 at its head), across (-1..1); metres along; how strong.
    @location(1) at: vec4<f32>,
    @location(2) @interpolate(flat) axis: vec3<f32>,
    @location(3) @interpolate(flat) side: vec3<f32>,
    @location(4) @interpolate(flat) lens: u32,
    @location(5) @interpolate(flat) kind: u32,
}

@vertex
fn vs_lens_ribbon(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> RibbonOut {
    let s = lenses[instance];
    var out: RibbonOut;
    out.lens = instance;
    let per = LENS_FX_RIBBON_QUADS * 6u;
    let kind = v / per;
    out.kind = kind;
    let g = lens_grid((v % per) / 6u, v % 6u, 1u);
    let u = f32(g.x) / f32(LENS_FX_RIBBON_QUADS);
    let across = f32(g.y) * 2.0 - 1.0;
    let to_front = s.front - s.emitter;
    let span = length(to_front);
    let ahead = to_front / max(span, 0.001);
    var head = s.front;
    var length_m = 0.0;
    var half = 0.0;
    var strength = 0.0;
    if kind == 0u {
        // Behind a running front, drawn in after it strikes.
        let a = since(s.strike);
        let gone = select(0.0, smoothstep(0.0, 0.3, a), s.strike >= 0.0);
        length_m = min(s.run, LENS_FX_CORRIDOR) * (1.0 - gone);
        half = mix(2.8, 1.0, u);
        strength = (1.0 - gone) * s.fade * select(0.0, 1.0, s.launch >= 0.0);
    } else {
        // The thread back to the lens while the well holds, from the emitter.
        head = s.emitter;
        length_m = span;
        half = 0.9;
        strength = well_held(s) * smoothstep(LENS_FX_IMPLODE, LENS_FX_IMPLODE + LENS_FX_FLASH, since(s.strike));
    }
    if strength <= 0.0 || length_m < 1.0 || span < 1.0 {
        out.clip = hidden_clip();
        return out;
    }
    let dir = select(-ahead, ahead, kind == 1u);
    let p0 = head + dir * (u * length_m);
    let side = normalize(cross(dir, globals.camera.xyz - p0) + vec3<f32>(0.0, 0.0, 1e-5));
    let p = p0 + side * across * half;
    out.world = p;
    out.at = vec4<f32>(u, across, u * length_m, strength);
    out.axis = dir;
    out.side = side;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_lens_ribbon(in: RibbonOut) -> @location(0) vec4<f32> {
    let s = lenses[in.lens];
    let t = globals.camera.w;
    let u = in.at.x;
    let across = in.at.y;
    let m = in.at.z;
    let strength = in.at.w;
    let soft = 1.0 - across * across;
    var bend = vec3<f32>(0.0);
    var dark = 0.0;
    var light = vec3<f32>(0.0);
    if in.kind == 0u {
        // Rippled air: bands across the corridor, settling behind the head.
        let n1 = value_noise2(vec2<f32>(m * 0.45 - t * 2.0, across * 1.3) + s.seed, 1.0) - 0.5;
        let n2 = value_noise2(vec2<f32>(m * 0.3 + 11.0, across * 1.7 - t * 1.5) + s.seed, 1.0) - 0.5;
        let bands = sin(m * 1.1 - t * 25.0);
        let amp = 1.1 * pow(1.0 - u, 1.4) * soft * strength;
        bend = (in.axis * (n1 + 0.35 * bands) + in.side * n2) * amp;
        // A violet wake streaming off the front's energy, frayed, gone a few tens of metres
        // back.
        let fray = smoothstep(0.35, 0.75, value_noise2(vec2<f32>(m * 0.25 - t * 6.0, across * 2.0) + s.seed, 1.0));
        let lead = smoothstep(0.0, 0.12, u);
        light = L_VIOLET * 0.2 * lead * pow(1.0 - u, 3.0) * pow(soft, 3.0) * (0.25 + 0.75 * fray) * strength;
    } else {
        // A faint thread: pulses running out to the well.
        let pulse = pow(0.5 + 0.5 * sin(m * 0.08 - t * 7.0), 6.0);
        let n = value_noise2(vec2<f32>(m * 0.5 - t * 3.0, across) + s.seed, 1.0) - 0.5;
        bend = in.side * n * (0.25 + 0.35 * pulse) * soft * strength;
        light = (L_RIM * 0.008 + L_VIOLET * 0.07) * pulse * pow(soft, 6.0) * strength;
    }
    let seen = bent_scene(in.world + bend, distance(globals.camera.xyz, in.world));
    let w = clamp(length(bend) / 0.06, 0.0, 1.0);
    return under_sea_seen(lay(seen, w, dark, light), in.clip);
}

// ---------------------------------------------------------------- the charge, the front, the strike

// The violet energy wound round a heart (`BallOut::energy`), at `r` metres from its middle:
// filaments of it swirling round, hottest close in, a faint violet haze between them out to
// its outer radius; and the violet on a ring of bent air closing in (`shape.w`). Light only,
// round the dark heart, never a ball of its own.
fn lens_energy(in: BallOut, r: f32, px: f32) -> vec3<f32> {
    let strength = in.energy.x;
    let inner = in.energy.y;
    let outer = max(in.energy.z, inner + 0.1);
    var light = vec3<f32>(0.0);
    if strength > 0.0 && inner > 0.0 {
        let t = globals.camera.w;
        let span = outer - inner;
        let out = max(r - inner, 0.0) / span;
        // Wound round: the further out, the further behind it turns.
        let turn = t * 5.0 + in.seed - out * 2.6;
        let c = cos(turn);
        let sn = sin(turn);
        let q = vec2<f32>(in.q.x * c - in.q.y * sn, in.q.x * sn + in.q.y * c) / inner;
        let n1 = value_noise2(q * 1.6 + vec2<f32>(in.seed, t * 1.3), 1.0);
        let n2 = value_noise2(q * 3.1 - vec2<f32>(t * 2.1, in.seed), 1.0);
        let ridge = pow(1.0 - abs(n1 - 0.5) * 2.0, 10.0) + 0.6 * pow(1.0 - abs(n2 - 0.5) * 2.0, 12.0);
        // Ragged, not a disc: it reaches out further some ways than others.
        let ragged = 0.55 + 0.9 * value_noise2(normalize(in.q + vec2<f32>(1e-4, 0.0)) * 1.7 + vec2<f32>(t * 0.9, in.seed), 1.0);
        let o2 = out / ragged;
        // Soft at the heart's edge, gone by the outer radius.
        let env = smoothstep(inner * 0.97, inner * 1.12 + px, r) * exp(-o2 * 3.4) * (1.0 - smoothstep(0.6, 1.0, o2));
        let hot = exp(-out * 6.0);
        let col = mix(L_VIOLET, L_LILAC, 0.4 * hot * ridge);
        light += col * strength * env * (0.12 + 1.1 * ridge);
    }
    let ring = in.shape.w;
    if ring >= 0.0 && in.energy.w > 0.0 {
        let width = max(0.08 * ring, 0.45) + px;
        let x = (r - ring) / width;
        let wobble = 0.6 + 0.4 * value_noise2(in.q * 0.7 + vec2<f32>(globals.camera.w * 6.0, in.seed), 1.0);
        light += L_VIOLET * 0.5 * in.energy.w * exp(-x * x) * wobble;
    }
    return light;
}

struct BallOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    // The billboard's metres out from its middle.
    @location(1) q: vec2<f32>,
    @location(2) @interpolate(flat) middle: vec3<f32>,
    @location(3) @interpolate(flat) right: vec3<f32>,
    @location(4) @interpolate(flat) up: vec3<f32>,
    // The black heart's radius, how hard the light bends (the radius it is seen at),
    // the reach of the bend, and a ring of bent air's radius (below zero: none).
    @location(5) @interpolate(flat) shape: vec4<f32>,
    // The ring's strength, the rim's, how black the heart is, the billboard's half size.
    @location(6) @interpolate(flat) look: vec4<f32>,
    // The violet energy wound round the heart: how strong, its inner and outer radius,
    // and the ring's violet (the strike's closing ring).
    @location(7) @interpolate(flat) energy: vec4<f32>,
    @location(8) @interpolate(flat) seed: f32,
}

@vertex
fn vs_lens_ball(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> BallOut {
    let s = lenses[instance];
    var out: BallOut;
    let ball = v / 6u;
    var middle = s.front;
    // Heart, bend, reach, ring; ring strength, rim, black, half size.
    var shape = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    var look = vec4<f32>(0.0, 0.35, 1.0, 0.0);
    var energy = vec4<f32>(0.0);
    var show = false;
    // Far off, the charge and the front are drawn larger, so they read at a game's zoom
    // as a knot of violet round a dark heart, not a speck.
    let far = clamp(distance(globals.camera.xyz, s.front) / 220.0, 1.0, 4.0);
    if ball == 0u {
        let c = since(s.charge);
        if s.launch < 0.0 && s.charge >= 0.0 && c >= 0.0 && c < s.charge_time + 0.4 {
            // The pinpoint swelling, rings of bent air closing in on it one after another.
            let p = clamp(c / s.charge_time, 0.0, 1.0);
            let heart = 0.1 + 0.9 * p * p;
            let ring = 12.0 * (1.0 - fract(c / 0.42));
            let big = min(far, 2.0);
            shape = vec4<f32>(heart, heart * 1.4, heart * 3.2 + 1.2, ring) * vec4<f32>(big, big, big, 1.0);
            look = vec4<f32>(0.25 + 0.75 * p, 0.25 + 0.35 * p, 1.0, 12.5 * big);
            energy = vec4<f32>(0.15 + 0.85 * p * p, heart * big, (heart * 2.6 + 0.8) * big, 0.35 * p);
            middle = s.emitter;
            show = true;
        } else if s.launch >= 0.0 && since(s.strike) < 0.0 {
            // The front, grown out of the pinpoint as it leaves.
            let grow = smoothstep(0.0, 0.12, since(s.launch));
            let heart = mix(1.0, 1.3, grow) * far;
            shape = vec4<f32>(heart, heart * 2.1, 10.0 * far, -1.0);
            look = vec4<f32>(0.0, 0.5, 1.0, 10.5 * far);
            energy = vec4<f32>(1.0, heart, heart * 3.4, 0.0);
            show = true;
        }
    } else if ball == 2u {
        // The snap as it lets go: a ring of bent air thrown out from the emitter.
        let a = since(s.launch);
        if s.launch >= 0.0 && a >= 0.0 && a < 0.3 {
            let f = a / 0.3;
            shape = vec4<f32>(0.0, 0.0, 0.0, 1.0 + 15.0 * sqrt(f));
            look = vec4<f32>(-1.8 * (1.0 - f), 0.0, 0.0, 18.0);
            middle = s.emitter;
            show = true;
        }
    } else {
        let a = since(s.strike);
        if s.strike >= 0.0 && a >= 0.0 && a < LENS_FX_IMPLODE + LENS_FX_FLASH {
            if a < LENS_FX_IMPLODE {
                // The front at the target, a ring of bent air snapping in onto it.
                let f = a / LENS_FX_IMPLODE;
                let ring = s.radius * 0.7 * (1.0 - f * f);
                shape = vec4<f32>(1.3, 2.7, 10.0, ring);
                look = vec4<f32>(2.2, 0.5, 1.0, max(s.radius * 0.7 + 4.0, 10.5));
                energy = vec4<f32>(1.0, 1.3, 4.5, 1.0 - 0.5 * f);
            } else {
                // The dark flash: the heart thrown open and gone.
                let f = (a - LENS_FX_IMPLODE) / LENS_FX_FLASH;
                let heart = mix(1.3, 7.5, sqrt(f));
                shape = vec4<f32>(heart, heart * 1.3 * (1.0 - f), heart * 2.4, -1.0);
                look = vec4<f32>(0.0, 0.3 * (1.0 - f), 1.0 - f, heart * 2.5);
                // The energy thrown off it in a ragged violet ring as it opens.
                energy = vec4<f32>(1.4 * (1.0 - f) * (1.0 - f), heart, heart * 1.9, 0.0);
            }
            show = true;
        }
    }
    if !show || s.fade <= 0.0 {
        out.clip = hidden_clip();
        return out;
    }
    let to_eye = globals.camera.xyz - middle;
    let view = to_eye / max(length(to_eye), 0.001);
    var right = cross(vec3<f32>(0.0, 0.0, 1.0), view);
    if length(right) < 0.01 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    let up = cross(view, right);
    var corner = vec2<f32>(-1.0, -1.0);
    switch v % 6u {
        case 1u: {
            corner = vec2<f32>(1.0, -1.0);
        }
        case 2u, 4u: {
            corner = vec2<f32>(1.0, 1.0);
        }
        case 5u: {
            corner = vec2<f32>(-1.0, 1.0);
        }
        default: {}
    }
    let q = corner * look.w;
    // Laid a little toward the eye, so the ground it runs over cuts less of it.
    let toward = select(min(look.w * 0.5, 4.0), 2.5, ball == 0u && s.launch < 0.0);
    let p = middle + right * q.x + up * q.y + view * toward;
    out.world = p;
    out.q = q;
    out.middle = middle;
    out.right = right;
    out.up = up;
    out.shape = shape;
    out.look = vec4<f32>(look.xyz * s.fade, look.w);
    out.energy = vec4<f32>(energy.x * s.fade, energy.yz, energy.w * s.fade);
    out.seed = s.seed + f32(ball) * 5.3;
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@fragment
fn fs_lens_ball(in: BallOut) -> @location(0) vec4<f32> {
    let r = length(in.q);
    let px = max(fwidth(r), 0.001);
    let gap = scene_gap(in.clip, in.world);
    let dir = in.q / max(r, 0.0001);
    let heart = in.shape.x;
    let bend = in.shape.y;
    let reach = in.shape.z;
    let ring = in.shape.w;
    // As round a heavy mass: what lies behind is seen pushed out from the heart, hardest
    // close in, fading to nothing at the reach.
    let fall = 1.0 - smoothstep(0.5 * reach, max(reach, 0.01), r);
    var src = in.q - dir * (bend * bend / max(r, max(heart * 0.85, 0.05))) * fall;
    // A ring of bent air: what is behind it gathered in to its line.
    if ring >= 0.0 {
        let width = max(0.16 * ring, 0.7);
        let x = (r - ring) / width;
        src -= dir * in.look.x * width * x * exp(-x * x) * smoothstep(0.3, 2.5, ring);
    }
    let p = in.middle + in.right * src.x + in.up * src.y;
    let seen = bent_scene(p, distance(globals.camera.xyz, in.middle));
    let half = in.look.w;
    let w = clamp(length(src - in.q) / 0.05, 0.0, 1.0) * (1.0 - smoothstep(0.85, 1.0, r / half));
    // The black heart, soft by a pixel, and the thin rim where the light bends hardest.
    let black = (1.0 - smoothstep(heart - px, heart + px, r)) * in.look.z;
    // A shadow round the heart, deepest at its edge: the light that passed close is lost.
    let shade = exp(-max(r - heart, 0.0) / max(0.9 * heart, 0.3)) * 0.55 * in.look.z;
    let rim = exp(-pow((r - heart * 1.06) / (0.04 * heart + px), 2.0)) * in.look.y;
    var light = L_RIM * rim * 0.05 + lens_energy(in, r, px) * (1.0 - black);
    // Soft where it meets the ground or a hull behind it.
    let soft = clamp(gap / 2.0, 0.0, 1.0);
    // The bend softens where it meets what is behind it; the heart stays whole.
    let heart_dark = max(black, shade * select(0.0, 1.0, heart > 0.0) * soft);
    return under_sea_seen(lay(seen, w * soft, heart_dark, light), in.clip);
}
