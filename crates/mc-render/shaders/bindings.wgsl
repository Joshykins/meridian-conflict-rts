// Descriptor set 0 of every graphics pipeline. build.rs inserts this file after
// common.wgsl into shaders that contain the line `//!use bindings`.

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> dynamic_entities: array<Entity>;
@group(0) @binding(2) var<storage, read> static_entities: array<Entity>;
@group(0) @binding(3) var<storage, read> models: array<ModelInfo>;
@group(0) @binding(4) var<storage, read> visible: array<u32>;
@group(0) @binding(5) var height_overview: texture_2d<f32>;
@group(0) @binding(6) var height_tiles: texture_2d_array<f32>;
@group(0) @binding(7) var tile_index: texture_2d<u32>;
@group(0) @binding(8) var fog_map: texture_2d<f32>;
@group(0) @binding(9) var noise_map: texture_2d<f32>;
@group(0) @binding(10) var panel_map: texture_2d<f32>;
@group(0) @binding(11) var shadow_map: texture_depth_2d;
@group(0) @binding(12) var repeat_sampler: sampler;
@group(0) @binding(13) var clamp_sampler: sampler;
@group(0) @binding(14) var shadow_sampler: sampler_comparison;
// One R8 SDF layer per unit blueprint: the structure's ground plan.
@group(0) @binding(16) var pad_footprints: texture_2d_array<f32>;
// One RGBA8 layer per unit blueprint: mesh plan SDF + local-Z span.
@group(0) @binding(17) var hull_plans: texture_2d_array<f32>;

// World metres → UV of a tiling texture, folded into 64 periods so the
// fractional part stays precise on an 80 km map. 64 is an integer, so the fold
// itself is a seamless wrap.
fn tile_uv(xy: vec2<f32>, period: f32) -> vec2<f32> {
    let span = period * 64.0;
    return (xy - span * floor(xy / span)) / period;
}

// Two lookups of the tiling noise, rotated and scaled so their periods never
// line up. Both samples stay in the mix: a value-noise weight was going to 0
// or 1 and leaving a lone wrap seam as a line on the ground.
fn noise_varied(xy: vec2<f32>, period: f32) -> vec4<f32> {
    let uv = tile_uv(xy, period);
    let uv2 = vec2<f32>(uv.x * 0.8 + uv.y * 0.6, -uv.x * 0.6 + uv.y * 0.8) * 1.618034 + vec2<f32>(0.31, 0.67);
    let a = textureSample(noise_map, repeat_sampler, uv);
    let b = textureSample(noise_map, repeat_sampler, uv2);
    return mix(a, b, 0.5);
}

fn load_entity(index: u32) -> Entity {
    if (index & DYNAMIC_BIT) != 0u {
        return dynamic_entities[index & ~DYNAMIC_BIT];
    }
    return static_entities[index];
}

// Height of the terrain surface. Uses the streamed full-resolution tile when
// it is resident, otherwise the always-resident overview.
fn terrain_height(xy: vec2<f32>) -> f32 {
    let size = globals.map.xy;
    let p = clamp(xy, vec2<f32>(0.0), size - vec2<f32>(0.01));
    let cell = p / 8.0;
    let tile = floor(cell / 256.0);
    let layer = textureLoad(tile_index, vec2<i32>(tile), 0).r;
    var h: f32;
    if layer > 0u {
        let local = cell - tile * 256.0;
        let uv = (local + vec2<f32>(0.5)) / 257.0;
        h = textureSampleLevel(height_tiles, clamp_sampler, uv, i32(layer) - 1, 0.0).r;
    } else {
        let dims = vec2<f32>(textureDimensions(height_overview));
        let uv = (cell / 4.0 + vec2<f32>(0.5)) / dims;
        h = textureSampleLevel(height_overview, clamp_sampler, uv, 0.0).r;
    }
    return globals.height.x + h * globals.height.y;
}

fn terrain_normal(xy: vec2<f32>, step: f32) -> vec3<f32> {
    let hx = terrain_height(xy + vec2<f32>(step, 0.0)) - terrain_height(xy - vec2<f32>(step, 0.0));
    let hy = terrain_height(xy + vec2<f32>(0.0, step)) - terrain_height(xy - vec2<f32>(0.0, step));
    return normalize(vec3<f32>(-hx, -hy, 2.0 * step));
}

// 1 lit, 0 shadowed. 3x3 PCF; fades out with `globals.map.w` when zoomed far out.
fn sun_shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let strength = globals.map.w;
    if strength <= 0.0 {
        return cloud_shadow(world);
    }
    let biased = world + n * 0.35;
    let clip = globals.shadow_view_proj * vec4<f32>(biased, 1.0);
    let uv = vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5);
    if uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || clip.z <= 0.0 || clip.z >= 1.0 {
        return cloud_shadow(world);
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, clip.z - 0.0015);
        }
    }
    return mix(1.0, lit / 9.0, strength) * cloud_shadow(world);
}

// x: visible now, y: explored. Both 1 when fog is off.
fn fog_at(xy: vec2<f32>) -> vec2<f32> {
    if (globals.counts.w & 1u) == 0u {
        return vec2<f32>(1.0);
    }
    let uv = xy / (vec2<f32>(textureDimensions(fog_map)) * 64.0);
    return textureSampleLevel(fog_map, clamp_sampler, uv, 0.0).rg;
}

fn apply_fog_of_war(color: vec3<f32>, xy: vec2<f32>) -> vec3<f32> {
    let f = fog_at(xy);
    let seen = mix(0.12, 0.45, f.y);
    return color * mix(seen, 1.0, f.x);
}

// Ground materials as layer pairs (textures::GROUND): linear albedo with
// roughness in alpha, then tangent normal XY, scanned height and occlusion.
// The constants are colour layers; the detail layer is the next one.
@group(0) @binding(18) var terrain_materials: texture_2d_array<f32>;
const MAT_ROCK_FACE: i32 = 0;
const MAT_LEAFY_GRASS: i32 = 2;
const MAT_MEADOW: i32 = 4;
const MAT_MOSSY_GRASS: i32 = 6;
const MAT_FOREST_FLOOR: i32 = 8;
const MAT_SCREE: i32 = 10;
const MAT_DRY_DIRT: i32 = 12;
const MAT_HIGH_ROCK: i32 = 14;
const MAT_SAND: i32 = 16;
const MAT_MUD: i32 = 18;
// First layer after the ground materials: foliage and bark (foliage.rs).
const FOLIAGE_BASE: i32 = 20;

// r: canopy closure overhead, g: how much of it is conifer. Built from the
// map's trees (ground_cover.rs); one texel covers map size / dimensions.
// b, a: glacier ice and lying snow from the map's snow layer.
@group(0) @binding(20) var ground_cover: texture_2d<f32>;

fn ground_cover_at(xy: vec2<f32>) -> vec2<f32> {
    let uv = xy / globals.map.xy;
    return textureSampleLevel(ground_cover, clamp_sampler, uv, 0.0).rg;
}

// x: glacier ice, y: lying snow, from the map's snow layer; z: 1 on a map
// that has one (the layer's snow is stored from 1/255 up), else 0.
fn ground_snow_at(xy: vec2<f32>) -> vec3<f32> {
    let uv = xy / globals.map.xy;
    let t = textureSampleLevel(ground_cover, clamp_sampler, uv, 0.0).ba;
    let layered = step(0.5 / 255.0, t.y);
    return vec3<f32>(t.x, max(t.y - 1.0 / 255.0, 0.0) * (255.0 / 254.0), layered);
}

struct SurfaceDetail {
    color: vec3<f32>,
    normal: vec3<f32>,
    roughness: f32,
    ao: f32,
    height: f32,
}

struct TerrainPatch {
    color: vec4<f32>,
    normal: vec4<f32>,
    height: f32,
}

fn turn_uv(p: vec2<f32>, cs: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cs.x * p.x - cs.y * p.y, cs.y * p.x + cs.x * p.y);
}

// Each triangle corner owns one randomly rotated, translated scan. Shared
// corners retain the same transform across triangle edges. Explicit gradients
// keep the random UV offsets out of mip selection, so no blurred tile seams.
fn terrain_patch(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, id: vec2<f32>,
    layer: i32, ray: vec2<f32>, relief: f32) -> TerrainPatch {
    let angle = hash21(id + vec2<f32>(73.0, 19.0)) * 6.2831853;
    let cs = vec2<f32>(cos(angle), sin(angle));
    let anchor = vec2<f32>(id.x + id.y * 0.5, id.y * 0.8660254) * 1.8;
    let offset = vec2<f32>(hash21(id + 13.7), hash21(id + 87.1)) * 7.0;
    var at = turn_uv(uv - anchor, cs) + offset;
    let gx = turn_uv(dx, cs);
    let gy = turn_uv(dy, cs);
    let h_layer = layer + 1;
    // A bounded relief trace is only used where the texels are visible. All
    // three material channels follow the same hit, preserving cavity edges.
    let delta = turn_uv(ray, cs) * relief;
    if relief > 0.0001 {
        var depth = 0.0;
        for (var i = 0; i < 5; i++) {
            let height = textureSampleGrad(terrain_materials, repeat_sampler, at, h_layer, gx, gy).b;
            let step = max((1.0 - height - depth) * 0.5, 0.0);
            at -= delta * step;
            depth += step;
        }
    }
    var out: TerrainPatch;
    out.color = textureSampleGrad(terrain_materials, repeat_sampler, at, layer, gx, gy);
    let n = textureSampleGrad(terrain_materials, repeat_sampler, at, h_layer, gx, gy);
    let packed = n.xy * 2.0 - 1.0;
    let nxy = turn_uv(packed, vec2<f32>(cs.x, -cs.y));
    out.normal = vec4<f32>(nxy, sqrt(max(1.0 - dot(packed, packed), 0.0)), n.a);
    out.height = n.b;
    return out;
}

fn terrain_projection(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, layer: i32,
    ray: vec2<f32>, relief: f32) -> TerrainPatch {
    let lattice = vec2<f32>(uv.x - uv.y * 0.57735027, uv.y * 1.1547005) / 1.8;
    let cell = floor(lattice);
    let f = fract(lattice);
    var ids = array<vec2<f32>, 3>(cell, cell + vec2<f32>(1.0, 0.0), cell + vec2<f32>(0.0, 1.0));
    var bary = vec3<f32>(1.0 - f.x - f.y, f.x, f.y);
    if f.x + f.y > 1.0 {
        ids = array<vec2<f32>, 3>(cell + vec2<f32>(1.0), cell + vec2<f32>(0.0, 1.0), cell + vec2<f32>(1.0, 0.0));
        bary = vec3<f32>(f.x + f.y - 1.0, 1.0 - f.x, 1.0 - f.y);
    }
    let a = terrain_patch(uv, dx, dy, ids[0], layer, ray, relief);
    let b = terrain_patch(uv, dx, dy, ids[1], layer, ray, relief);
    let c = terrain_patch(uv, dx, dy, ids[2], layer, ray, relief);
    // Height-aware weights keep raised stone and tufts intact instead of
    // washing three unrelated photos into a flat, low-contrast average.
    let height = vec3<f32>(a.height, b.height, c.height);
    var w = pow(max(bary, vec3<f32>(0.0)), vec3<f32>(3.0)) * exp2(height * 5.0);
    w /= max(dot(w, vec3<f32>(1.0)), 0.00001);
    var out: TerrainPatch;
    out.color = a.color * w.x + b.color * w.y + c.color * w.z;
    out.normal = a.normal * w.x + b.normal * w.y + c.normal * w.z;
    out.height = dot(height, w);
    return out;
}

fn terrain_surface(p: vec3<f32>, base_n: vec3<f32>, period: f32, layer: i32, strength: f32) -> SurfaceDetail {
    return terrain_surface_grad(p, base_n, period, layer, strength, dpdx(p), dpdy(p));
}

// terrain_surface with the screen derivatives of `p` supplied, so it can be
// called under a branch.
fn terrain_surface_grad(p: vec3<f32>, base_n: vec3<f32>, period: f32, layer: i32, strength: f32,
    dpx: vec3<f32>, dpy: vec3<f32>) -> SurfaceDetail {
    var w = pow(abs(base_n), vec3<f32>(4.0));
    w /= max(dot(w, vec3<f32>(1.0)), 0.0001);
    let uv = p / period;
    let dx = dpx / period;
    let dy = dpy / period;
    let view = normalize(globals.camera.xyz - p);
    let tangent_view = view - base_n * dot(view, base_n);
    let ray = tangent_view / max(dot(view, base_n), 0.28);
    let range = distance(globals.camera.xyz, p);
    let relief = select(0.60, 0.07, layer == 2) / period * (1.0 - smoothstep(65.0, 220.0, range));
    var out: SurfaceDetail;
    var gradient = vec3<f32>(0.0);
    // Skip irrelevant projections; the gradients were computed before branching.
    if w.x > 0.004 {
        let a = terrain_projection(uv.yz, dx.yz, dy.yz, layer, ray.yz, relief);
        let g = a.normal.xy / max(a.normal.z, 0.30);
        out.color += a.color.rgb * w.x; out.roughness += a.color.a * w.x;
        out.ao += a.normal.a * w.x; out.height += a.height * w.x;
        gradient += vec3<f32>(0.0, g.x, g.y) * w.x;
    }
    if w.y > 0.004 {
        let a = terrain_projection(uv.xz, dx.xz, dy.xz, layer, ray.xz, relief);
        let g = a.normal.xy / max(a.normal.z, 0.30);
        out.color += a.color.rgb * w.y; out.roughness += a.color.a * w.y;
        out.ao += a.normal.a * w.y; out.height += a.height * w.y;
        gradient += vec3<f32>(g.x, 0.0, g.y) * w.y;
    }
    if w.z > 0.004 {
        let a = terrain_projection(uv.xy, dx.xy, dy.xy, layer, ray.xy, relief);
        let g = a.normal.xy / max(a.normal.z, 0.30);
        out.color += a.color.rgb * w.z; out.roughness += a.color.a * w.z;
        out.ao += a.normal.a * w.z; out.height += a.height * w.z;
        gradient += vec3<f32>(g, 0.0) * w.z;
    }
    out.normal = normalize(base_n + (gradient - base_n * dot(gradient, base_n)) * strength);
    return out;
}

@group(0) @binding(19) var<storage, read> effect_barriers: EffectBarriers;
@group(0) @binding(28) var<storage, read> houses: array<HousePose>;
fn effect_blocked(source: vec3<f32>, to: vec3<f32>) -> bool {
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        if barrier_crosses(source, to, effect_barriers.entries[i]) { return true; }
    }
    return false;
}
fn effect_billboard_world(center: vec3<f32>, corner: vec2<f32>, diameter: f32) -> vec3<f32> {
    let right = normalize(vec3<f32>(globals.view_proj[0].x, globals.view_proj[1].x, globals.view_proj[2].x));
    let up = normalize(vec3<f32>(globals.view_proj[0].y, globals.view_proj[1].y, globals.view_proj[2].y));
    return center + (right * corner.x + up * corner.y) * diameter * 0.5;
}

// The 12 m build grid while a structure is being placed, near the pointer only.
// Lots already taken (structures, plans) are drawn red. Shared by the terrain
// and the water, so the grid is on whichever surface a structure would stand on.
fn build_grid_overlay(color: vec3<f32>, xy: vec2<f32>, dist: f32) -> vec3<f32> {
    let c = globals.build_cursor;
    let near = 1.0 - smoothstep(c.z * 0.45, c.z, distance(xy, c.xy));
    if near <= 0.0 {
        return color;
    }
    let g = abs(fract(xy / BUILD_CELL_M + 0.5) - 0.5) * BUILD_CELL_M;
    let width = max(dist * 0.0012, 0.12);
    let line = 1.0 - smoothstep(0.0, width, min(g.x, g.y));
    var taken = false;
    for (var i = 0u; i < u32(c.w); i++) {
        let r = globals.build_blocked[i];
        if all(xy >= r.xy - width) && all(xy <= r.zw + width) {
            taken = true;
            break;
        }
    }
    if taken {
        let red = vec3<f32>(1.0, 0.24, 0.18);
        let tinted = mix(color, red * 0.6, 0.12 * near);
        return mix(tinted, red * 1.6, line * 0.5 * near);
    }
    return mix(color, vec3<f32>(0.55, 0.85, 1.0) * 1.6, line * 0.225 * near);
}

// ---- Atmosphere and clouds (sky.rs) -----------------------------------------

// The weather simulation's state: x cloud cover, y storm, z churn, w spare.
// Kept in GENERAL layout; the compute pass rewrites it every frame.
@group(0) @binding(21) var cloud_weather: texture_2d<f32>;
@group(0) @binding(22) var<uniform> atmos: Atmosphere;
// The weather's stirred-up flow: xy local wind (m/s), z height of the last
// aircraft wake through here, w how fresh it is.
@group(0) @binding(23) var cloud_flow: texture_2d<f32>;
// The land smoothed over a few hundred metres (the sea where there is none):
// cloud heights are measured from it (sky.rs).
@group(0) @binding(24) var cloud_floor_map: texture_2d<f32>;

fn cloud_floor(xy: vec2<f32>) -> f32 {
    let size = atmos.weather.yz;
    let floor = textureSampleLevel(cloud_floor_map, clamp_sampler, xy / size, 0.0).r;
    // Past the map's edge the edge row would run on for ever, pulling the
    // layer into straight streaks: ease down to the lowest floor (the sea).
    let out = length(max(max(-xy, xy - size), vec2<f32>(0.0)));
    return mix(floor, atmos.frame.w, smoothstep(0.0, 2500.0, out));
}

fn flow_at(xy: vec2<f32>) -> vec4<f32> {
    let uv = xy / atmos.weather.yz;
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    return textureSampleLevel(cloud_flow, clamp_sampler, uv, 0.0);
}

// How strongly the air scatters, against the real thing: a battlefield seen
// from a few kilometres wants some depth, not a whiteout.
const HAZE_SCALE: f32 = 0.35;
// Sky brightness against the lit ground at the game's exposure (sky.rs SKY_GAIN).
const SKY_GAIN: f32 = 5.0;
// The haze's own glow, kept near physical so land far below stays land.
const HAZE_GLOW: f32 = 3.2;

// The weather at a map point; past the map's edge, the air mass alone.
fn weather_at(xy: vec2<f32>) -> vec4<f32> {
    let size = atmos.weather.yz;
    let uv = xy / size;
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let inside = textureSampleLevel(cloud_weather, clamp_sampler, uv, 0.0);
    if edge >= 0.02 {
        return inside;
    }
    let outside = vec4<f32>(cloud_climate(xy, atmos.wind.xy, atmos.layer.w, atmos.shape.y), 0.0, 0.0);
    return mix(outside, inside, smoothstep(0.0, 0.02, edge));
}

// The clouds' shade (clouds.wgsl `cs_shade`): r the sunlight they let through
// to a plane under the layer, per texel of the map. 1 everywhere with the
// clouds off.
@group(0) @binding(27) var cloud_shade: texture_2d<f32>;

// Light the clouds let through to `world`, 1 under open sky: the shade the
// drawn clouds cast, carried down the sun's slant from under the layer.
fn cloud_shadow(world: vec3<f32>) -> f32 {
    let s = globals.sun.xyz;
    let under = cloud_floor(world.xy) + atmos.layer.x - 300.0;
    let xy = world.xy + s.xy / max(s.z, 0.2) * max(under - world.z, 0.0);
    let size = atmos.weather.yz;
    let uv = xy / size;
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    // Never black: light scattered through and under the cloud still arrives.
    let shade = max(textureSampleLevel(cloud_shade, clamp_sampler, uv, 0.0).r, 0.3);
    if edge >= 0.01 {
        return shade;
    }
    // Past the map's edge, where there is no shade map: the air mass's cover.
    return mix(cloud_shadow_coarse(world), shade, smoothstep(-0.01, 0.01, edge));
}

// The cover's shade, from the weather alone: only past the map's edge.
fn cloud_shadow_coarse(world: vec3<f32>) -> f32 {
    let s = globals.sun.xyz;
    let mid = cloud_floor(world.xy) + mix(atmos.layer.x, atmos.layer.y, 0.45);
    let xy = world.xy + s.xy / max(s.z, 0.2) * max(mid - world.z, 0.0);
    let w = weather_at(xy);
    // Edges broken up the way the drawn clouds' are: one fetch of the tiling noise.
    let ragged = textureSampleLevel(noise_map, repeat_sampler, tile_uv(xy - atmos.wind.xy, 900.0), 0.0).r;
    let cover = smoothstep(0.08, 0.55, w.x - (ragged - 0.5) * 0.35);
    let depth = cover * (1.6 + w.y * 3.0);
    // Never black: light scattered through and under the cloud still arrives.
    return mix(1.0, max(exp(-depth), 0.3), smoothstep(0.0, 0.08, w.x));
}

// Single scattering of sunlight in a flat, exponential atmosphere, seen
// along `d` from near sea level. Blue overhead, pale at the horizon, a warm
// glow round the sun. No sun disk (the sky pass adds one).
fn sky_radiance(d: vec3<f32>) -> vec3<f32> {
    let sun = globals.sun.xyz;
    let mu = dot(d, sun);
    let up = clamp(d.z, 0.0, 1.0);
    // Kasten-Young air mass: how many zenith columns of air lie along `d`.
    let zenith_deg = degrees(acos(up));
    let mass = min(1.0 / (up + 0.50572 * pow(max(96.07995 - zenith_deg, 0.01), -1.6364)), 38.0);
    let r = RAYLEIGH * RAYLEIGH_H * mass;
    let m = vec3<f32>(MIE * MIE_H * mass);
    let ext = r + m * 1.1;
    let scatter = r * phase_rayleigh(mu) + m * phase_hg(mu, 0.78);
    // At night the scene's key light is a day-for-night moon, far brighter
    // than the real one; the dome it lights stays near black (`sun_color.w`).
    var sky = atmos.sun_color.rgb * scatter / max(ext, vec3<f32>(1e-6)) * (1.0 - exp(-ext)) * SKY_GAIN * atmos.sun_color.w;
    // Light scattered more than once fills the shadowed side a little.
    sky += atmos.sky_color.rgb * 0.18 * (1.0 - exp(-ext * 2.0));
    if d.z < 0.0 {
        // Below the horizon: the haze over far land.
        sky = mix(sky, atmos.horizon_color.rgb * 0.55 + atmos.ground_color.rgb * 0.6, clamp(-d.z * 3.0, 0.0, 1.0));
    }
    return sky;
}

// Aerial perspective: light lost and gained on the way from `world` to the
// eye through the same air the sky is made of.
fn apply_haze(color: vec3<f32>, world: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let column_r = air_column(eye, world, RAYLEIGH_H) * HAZE_SCALE;
    let column_m = air_column(eye, world, MIE_H) * HAZE_SCALE;
    let tau = RAYLEIGH * column_r + vec3<f32>(MIE * 1.1 * column_m);
    let through = exp(-tau);
    let d = normalize(world - eye);
    let mu = dot(d, globals.sun.xyz);
    // Sunlight scattered toward the eye by that same air, blue from the
    // molecules, grey-white round the sun from the haze.
    let scatter = RAYLEIGH * column_r * phase_rayleigh(mu) + vec3<f32>(MIE * column_m * phase_hg(mu, 0.7));
    let glow = atmos.sun_color.rgb * scatter / max(tau, vec3<f32>(1e-7)) * HAZE_GLOW + atmos.sky_color.rgb * 0.35;
    return color * through + glow * (1.0 - through);
}

// Lightning's light where it lands on the ground or a hull: bluish white,
// falling off over a few kilometres.
fn lightning_light(world: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    let count = u32(atmos.counts.y);
    for (var i = 0u; i < count; i++) {
        let f = atmos.flashes[i];
        if f.w <= 0.0 { continue; }
        let to = f.xyz - world;
        let d2 = dot(to, to);
        let facing = max(dot(n, to * inverseSqrt(max(d2, 1.0))), 0.0) * 0.7 + 0.3;
        light += vec3<f32>(0.78, 0.84, 1.0) * f.w * facing * 2.4e6 / (d2 + 1.5e6);
    }
    return light;
}

// Cook-Torrance under this sky, sun and weather. `shadow` already carries the
// clouds (sun_shadow does). `sky_vis` darkens the sky light in hollows.
fn shade_pbr_vis(m: Pbr, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32, sky_vis: f32) -> vec3<f32> {
    return shade_pbr_env(m, n, v, l, shadow, atmos.sun_color.rgb, atmos.sky_color.rgb,
        atmos.ground_color.rgb, sky_vis);
}

fn shade_pbr(m: Pbr, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, shadow: f32) -> vec3<f32> {
    return shade_pbr_vis(m, n, v, l, shadow, 1.0);
}
