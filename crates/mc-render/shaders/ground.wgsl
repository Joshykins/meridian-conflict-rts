//!use bindings
// Things painted over the terrain: crater/scorch stains and the water surface.
// Stains are decals only; the ground under them is never deformed.

// Mirrors mc_sim::mirror::StainInstance.
struct Stain {
    pos: vec2<f32>,
    radius: f32,
    strength_seed: u32,
}

@group(1) @binding(0) var<storage, read> stains: array<Stain>;

struct StainOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) strength_seed: u32,
}

// `grid` spans [-1, 1] on a small mesh so the decal follows the ground.
@vertex
fn vs_stain(@location(0) grid: vec2<f32>, @builtin(instance_index) instance: u32) -> StainOut {
    let s = stains[instance];
    let seed = f32(s.strength_seed >> 8u);
    let spun = rot_z(vec3<f32>(grid, 0.0), seed * 0.37).xy;
    let xy = s.pos + spun * s.radius * 1.35;
    let ground = terrain_height(xy);
    let world = vec3<f32>(xy, ground);
    var out: StainOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    // Pull toward the camera a little instead of lifting: no floating at glancing angles.
    out.clip.z += 0.00002 * out.clip.w + 0.02;
    let px = s.radius * globals.lod.x / max(out.clip.w, 1.0);
    if px < 0.75 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    }
    out.uv = grid;
    out.world = world;
    out.strength_seed = s.strength_seed;
    return out;
}

@fragment
fn fs_stain(in: StainOut) -> @location(0) vec4<f32> {
    let strength = f32(in.strength_seed & 0xFFu) / 255.0;
    let seed = f32(in.strength_seed >> 8u);
    let d = length(in.uv);
    // Ragged edge: the noise texture perturbs the radius.
    let n = textureSample(noise_map, repeat_sampler, in.world.xy / 23.0 + vec2<f32>(seed * 0.013)).ba;
    let edge = d + (n.x - 0.5) * 0.55;
    let mask = (1.0 - smoothstep(0.35, 1.0, edge)) * (0.75 + n.y * 0.5);
    let alpha = clamp(mask * (0.35 + strength * 1.1), 0.0, 0.93);
    if alpha < 0.01 {
        discard;
    }
    let char_color = apply_fog_of_war(vec3<f32>(0.025, 0.022, 0.02), in.world.xy);
    return vec4<f32>(apply_haze(char_color, in.world, globals.camera.xyz), alpha);
}

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vs_water(@builtin(vertex_index) index: u32) -> WaterOut {
    // Two triangles covering the map, extended outward so the sea reaches the horizon.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[index];
    let xy = c * globals.map.xy;
    var out: WaterOut;
    out.world = vec3<f32>(xy, globals.map.z);
    out.clip = globals.view_proj * vec4<f32>(out.world, 1.0);
    return out;
}

@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    let xy = in.world.xy;
    let depth = globals.map.z - terrain_height(xy);
    if depth <= 0.0 {
        discard;
    }
    let time = globals.camera.w;
    let eye = globals.camera.xyz;
    let v = normalize(eye - in.world);
    let dist = distance(eye, in.world);

    let calm = clamp(1.0 - dist / 6000.0, 0.0, 1.0);
    let w1 = textureSample(noise_map, repeat_sampler, xy / 41.0 + vec2<f32>(time * 0.021, time * 0.013)).xy - 0.5;
    let w2 = textureSample(noise_map, repeat_sampler, xy / 13.0 - vec2<f32>(time * 0.034, time * 0.027)).xy - 0.5;
    let n = normalize(vec3<f32>((w1 * 0.5 + w2 * 0.3) * calm, 1.0));

    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    let r = reflect(-v, n);
    let sky = mix(vec3<f32>(0.5, 0.64, 0.86), vec3<f32>(0.2, 0.38, 0.72), clamp(r.z, 0.0, 1.0));
    let sun = pow(max(dot(r, globals.sun.xyz), 0.0), 400.0) * 40.0 * calm;
    let body = mix(vec3<f32>(0.05, 0.22, 0.26), vec3<f32>(0.01, 0.05, 0.12), clamp(depth / 35.0, 0.0, 1.0));
    var color = mix(body, sky, fresnel) + vec3<f32>(1.0, 0.95, 0.85) * sun;

    // Foam where the water meets the shore.
    let foam_noise = textureSample(noise_map, repeat_sampler, xy / 7.0 + vec2<f32>(time * 0.05, 0.0)).b;
    let foam = (1.0 - smoothstep(0.0, 1.6, depth)) * smoothstep(0.35, 0.7, foam_noise) * calm;
    color = mix(color, vec3<f32>(0.9), foam * 0.8);

    color = apply_fog_of_war(color, xy);
    color = apply_haze(color, in.world, eye);
    let alpha = clamp(depth / 5.0, 0.0, 1.0) * mix(0.72, 1.0, fresnel) + foam * 0.3;
    return vec4<f32>(color, clamp(alpha, 0.0, 1.0));
}
