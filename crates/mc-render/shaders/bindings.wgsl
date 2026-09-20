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

// Two lookups of the tiling noise, rotated and scaled so their periods never
// line up. The blend always mixes both, on a scale close to one tile, so a
// single square repeat cannot sit on the ground as a grid.
fn noise_varied(xy: vec2<f32>, period: f32) -> vec4<f32> {
    let uv = xy / period;
    let uv2 = vec2<f32>(uv.x * 0.8 + uv.y * 0.6, -uv.x * 0.6 + uv.y * 0.8) * 1.618034 + vec2<f32>(0.31, 0.67);
    let a = textureSample(noise_map, repeat_sampler, uv);
    let b = textureSample(noise_map, repeat_sampler, uv2);
    let w = value_noise2(xy, period * 1.9);
    return mix(a, b, w);
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
        return 1.0;
    }
    let biased = world + n * 0.35;
    let clip = globals.shadow_view_proj * vec4<f32>(biased, 1.0);
    let uv = vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5);
    if uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || clip.z <= 0.0 || clip.z >= 1.0 {
        return 1.0;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, clip.z - 0.0015);
        }
    }
    return mix(1.0, lit / 9.0, strength);
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
