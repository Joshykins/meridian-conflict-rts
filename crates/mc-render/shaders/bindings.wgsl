// Descriptor set 0 of every graphics pipeline. build.rs inserts this file after
// common.wgsl into shaders that contain the line `//!use bindings`.

@group(0) @binding(0) var<uniform> globals: Globals;

// The map's climate is tropical (`Globals::climate`): bright coral sand, lush green,
// turquoise shallows. Temperate when neither this nor desert(). What is drawn at a
// place asks `climate_at` instead: on a map with regions this is region 0's climate
// only.
fn tropical() -> bool {
    return globals.climate.x > 0.5 && globals.climate.x < 1.5;
}

// The map's climate is canyon-country desert (`Globals::climate` 2): red-rock
// strata by height, the reservoir's bathtub ring, red sand and pale caliche
// dotted with dark shrubs, jade-to-cobalt lake water.
fn desert() -> bool {
    return globals.climate.x > 1.5;
}

// How much of each climate's look the ground or sea at `xy` takes, 0-1: x tropical,
// y desert (temperate what is left). On a map of one climate, exactly `tropical()`
// and `desert()`. On a map with regions (regions.wgsl) each region has its own
// (`Globals::region_climate`), and across a wall they hand over within `half` metres
// either side of it: a ruled line from any height, so `half` is at least a pixel or
// two (`px`, the metres a pixel covers there). Anything drawn at a place asks this
// (or `climate_at`, `desert_at`, `tropical_at`) instead of `tropical()` and `desert()`.
fn climate_within(xy: vec2<f32>, half: f32) -> vec2<f32> {
    if !has_regions() {
        return vec2<f32>(select(0.0, 1.0, tropical()), select(0.0, 1.0, desert()));
    }
    let shares = region_shares(xy, half);
    if !shares.mixed {
        return globals.region_climate[shares.region].xy;
    }
    var climate = vec2<f32>(0.0);
    let regions = u32(atmos.regions.y);
    for (var r = 0u; r < regions; r++) {
        if shares.w[r] > 0.0 {
            climate += globals.region_climate[r].xy * shares.w[r];
        }
    }
    return climate;
}

fn climate_at(xy: vec2<f32>, px: f32) -> vec2<f32> {
    return climate_within(xy, max(REGIONS_BLEND_M, px * 1.5));
}

// How much of the desert's look `xy` takes: `f32(desert())` on a map of one climate.
fn desert_at(xy: vec2<f32>, px: f32) -> f32 {
    return climate_at(xy, px).y;
}

// How much of the tropical look `xy` takes: `f32(tropical())` on a map of one climate.
fn tropical_at(xy: vec2<f32>, px: f32) -> f32 {
    return climate_at(xy, px).x;
}

// How much of what falls from the clouds over `xy` is snow rather than rain, 0-1: in
// a temperate region of a map with regions, if the map carries a snow layer (the
// alpine part of such a map); nowhere on any other map. mc_data's `MapLook::snows_at`
// is the same.
fn snowfall_at(xy: vec2<f32>) -> f32 {
    if !has_regions() {
        return 0.0;
    }
    let climate = climate_within(xy, REGIONS_SKY_BLEND_M);
    return (1.0 - climate.x - climate.y) * ground_snow_at(xy).z;
}

// The foot of a climate wall: a line of Precursor light on the ground and the sea
// along every wall between regions, the cold blue-white of their working parts
// (entity.wgsl `MAT_GLOW_PRECURSOR`). A bright core a few metres wide, never thinner
// than a pixel or so, so it reads as a ruled line from the strategic view; a soft
// glow some 35 m either side; slow pulses of brighter light running along the wall
// from its first point to its last.
// Light to add at `xy`, where a pixel covers `px` metres. Only where `has_regions()`.
fn wall_seam(xy: vec2<f32>, px: f32) -> vec3<f32> {
    let probe = region_probe(xy);
    let d = probe.wall;
    if d > 40.0 + px {
        return vec3<f32>(0.0);
    }
    let time = globals.camera.w;
    let half = max(1.7, px * 0.6);
    let soft = max(px * 0.75, 0.25);
    let core = 1.0 - smoothstep(half - soft, half + soft, d);
    // Hottest along its middle, so up close it is a beam of light and not a painted band.
    let heart = 1.0 - smoothstep(0.0, half, d);
    let halo = 1.0 - smoothstep(0.0, 36.0, d);
    // As `precursor_pulse` (surface.wgsl): the light breathes, and bands of it travel.
    let breath = 0.85 + 0.15 * sin(time * 0.75);
    // (A band's leading edge eased over a few metres: cut off, it flickered as it moved.)
    let f = fract(probe.along / 520.0 - time * 0.07);
    let band = pow(f, 8.0) * (1.0 - smoothstep(0.975, 1.0, f));
    let light = vec3<f32>(0.45, 0.78, 1.0);
    return light * breath * (core * (0.8 + 1.4 * heart * heart + 1.6 * band) + halo * halo * (0.16 + 0.2 * band));
}

// Metres the desert's rock beds lie lower on this map than in Vermilion Gorge
// (`MapConfig::strata_lift`): added to a height above the water wherever it picks a
// bed, its soil, its shrubs or the reservoir's ring (desert.wgsl), never where the
// water itself matters (the beach, the wet line).
fn strata_lift() -> f32 {
    return globals.map_look.x;
}
@group(0) @binding(1) var<storage, read> dynamic_entities: array<Entity>;
@group(0) @binding(2) var<storage, read> static_entities: array<Entity>;
@group(0) @binding(3) var<storage, read> models: array<ModelInfo>;
@group(0) @binding(4) var<storage, read> visible: array<u32>;
@group(0) @binding(5) var height_overview: texture_2d<f32>;
@group(0) @binding(6) var height_tiles: texture_2d_array<f32>;
@group(0) @binding(7) var tile_index: texture_2d<u32>;
// The fog of war, smoothed from the sim's grid (fog_field.wgsl); read through `fog_at`.
@group(0) @binding(8) var fog_map: texture_2d<f32>;
@group(0) @binding(9) var noise_map: texture_2d<f32>;
// retired: 10 (the tiled plate map)
@group(0) @binding(11) var shadow_map: texture_depth_2d_array;
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
    var z = globals.height.x + h * globals.height.y;
    // A new structure's lot eases to its level: the tiles keep the old ground
    // until it is done (terrain.rs `TileCache::apply_edits`).
    let settling = u32(globals.settle.x);
    for (var i = 0u; i < settling; i++) {
        let r = globals.settling[2u * i];
        if all(p >= r.xy) && all(p <= r.zw) {
            let s = globals.settling[2u * i + 1u];
            z = mix(z, s.x, s.y);
        }
    }
    return z;
}

fn terrain_normal(xy: vec2<f32>, step: f32) -> vec3<f32> {
    let hx = terrain_height(xy + vec2<f32>(step, 0.0)) - terrain_height(xy - vec2<f32>(step, 0.0));
    let hy = terrain_height(xy + vec2<f32>(0.0, step)) - terrain_height(xy - vec2<f32>(0.0, step));
    return normalize(vec3<f32>(-hx, -hy, 2.0 * step));
}

// Whether `part` is one of a joining wall's pieces (`models::wall`, `gpu_consts::wall`).
fn wall_piece(part: u32) -> bool {
    return part >= WALL_PART_FIRST && part < WALL_PART_FIRST + 4u * WALL_CASES;
}

// Whether the wall piece a vertex of `part` belongs to is the one its quarter's
// neighbours (`status[2]`) call for. Always true for anything that is not a wall piece.
fn wall_piece_shown(part: u32, joins: u32) -> bool {
    if !wall_piece(part) {
        return true;
    }
    let k = part - WALL_PART_FIRST;
    let q = k / WALL_CASES;
    let a = ((joins >> (2u * q)) & 1u) != 0u;
    let b = ((joins >> ((2u * q + 2u) & 7u)) & 1u) != 0u;
    let corner = ((joins >> (2u * q + 1u)) & 1u) != 0u;
    var want = WALL_CAP;
    if a && b {
        want = select(WALL_JOIN, WALL_FULL, corner);
    } else if a {
        want = WALL_RUN_A;
    } else if b {
        want = WALL_RUN_B;
    }
    return k % WALL_CASES == want;
}

// How far a joining wall's vertex at `local` (plan, in the section's frame) rises to
// follow the ground: sections on a slope meet on the ground between them, not in a step.
fn wall_follow_ground(origin: vec2<f32>, heading: f32, local: vec2<f32>) -> f32 {
    let c = cos(heading);
    let s = sin(heading);
    let world = origin + vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
    return terrain_height(world) - terrain_height(origin);
}

// Where `world` falls in shadow cascade `i`: uv, depth to compare, and how near
// the map's edge (0 centre, 1 edge). Offsets scale with the cascade's texel.
fn shadow_coord(i: u32, world: vec3<f32>, n: vec3<f32>) -> vec4<f32> {
    let info = globals.shadow_info[i];
    let biased = world + n * (info.x * 1.5);
    let clip = globals.shadow_cascades[i] * vec4<f32>(biased, 1.0);
    let edge = max(abs(clip.x), abs(clip.y));
    let z = clip.z - info.x * 1.2 / info.y;
    return vec4<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5, z, select(edge, 2.0, clip.z <= 0.0 || clip.z >= 1.0));
}

// 3x3 bilinear PCF in one cascade.
fn shadow_pcf(i: u32, c: vec4<f32>) -> f32 {
    if globals.detail.w > 0.5 {
        // Hardware bilinear comparison still softens edges without nine filter taps.
        return textureSampleCompareLevel(shadow_map, shadow_sampler, c.xy, i, c.z);
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(shadow_map));
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, c.xy + vec2<f32>(f32(x), f32(y)) * texel, i, c.z);
        }
    }
    return lit / 9.0;
}

// 1 lit, 0 shadowed. The nearest cascade that holds the point, blended into the
// next across its outer rim; fades out with `globals.map.w` when zoomed far out.
fn sun_shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let strength = globals.map.w;
    if strength <= 0.0 {
        return cloud_shadow(world);
    }
    var lit = 1.0;
    for (var i = 0u; i < 3u; i++) {
        let c = shadow_coord(i, world, n);
        if c.w >= 0.97 {
            continue;
        }
        lit = shadow_pcf(i, c);
        let rim = smoothstep(0.82, 0.97, c.w);
        if rim > 0.0 {
            var beyond = 1.0;
            if i < 2u {
                let next = shadow_coord(i + 1u, world, n);
                if next.w < 0.97 {
                    beyond = shadow_pcf(i + 1u, next);
                }
            }
            lit = mix(lit, beyond, rim);
        }
        break;
    }
    return mix(1.0, lit, strength) * cloud_shadow(world);
}

// x: visible now, y: explored. Both 1 when fog is off.
fn fog_at(xy: vec2<f32>) -> vec2<f32> {
    if (globals.counts.w & 1u) == 0u {
        return vec2<f32>(1.0);
    }
    let uv = xy / (vec2<f32>(textureDimensions(fog_map)) * (FOG_CELL_M / f32(FOG_FIELD_SCALE)));
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
// Layer 1, from the map's ways layer: r = how trodden the ground is, g, b =
// the trail's heading as cos 2a, sin 2a (0.5 = 0).
@group(0) @binding(20) var ground_cover: texture_2d_array<f32>;

fn ground_cover_at(xy: vec2<f32>) -> vec2<f32> {
    let uv = xy / globals.map.xy;
    return textureSampleLevel(ground_cover, clamp_sampler, uv, 0, 0.0).rg;
}

// How trodden the ground is, 0-1: 1 on a trail's floor (canyon_trail).
fn ground_way_at(xy: vec2<f32>) -> f32 {
    let uv = xy / globals.map.xy;
    return textureSampleLevel(ground_cover, clamp_sampler, uv, 1, 0.0).r;
}

// How much of a crag the ground at `xy` is, 0-1: steep ground, softened
// (`cliff_blocks::crag_field`; rock.wgsl `crag_relief`).
fn ground_crag_at(xy: vec2<f32>) -> f32 {
    let uv = xy / globals.map.xy;
    return textureSampleLevel(ground_cover, clamp_sampler, uv, 1, 0.0).a;
}

// Which way the trail at `xy` runs: a unit vector (either way along it).
fn ground_way_heading(xy: vec2<f32>) -> vec2<f32> {
    let uv = xy / globals.map.xy;
    let t = textureSampleLevel(ground_cover, clamp_sampler, uv, 1, 0.0).gb * 2.0 - 1.0;
    // Halve the doubled angle.
    let a = 0.5 * atan2(t.y, t.x);
    return vec2<f32>(cos(a), sin(a));
}

// x: glacier ice, y: lying snow, from the map's snow layer; z: 1 on a map
// that has one (the layer's snow is stored from 1/255 up), else 0.
fn ground_snow_at(xy: vec2<f32>) -> vec3<f32> {
    let uv = xy / globals.map.xy;
    let t = textureSampleLevel(ground_cover, clamp_sampler, uv, 0, 0.0).ba;
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
    if globals.detail.w > 0.5 {
        // Keep every material and its normal/roughness; save the three rotated
        // anti-tiling patches and relief trace on the lower quality presets.
        var out: TerrainPatch;
        out.color = textureSampleGrad(terrain_materials, repeat_sampler, uv, layer, dx, dy);
        let n = textureSampleGrad(terrain_materials, repeat_sampler, uv, layer + 1, dx, dy);
        let packed = n.xy * 2.0 - 1.0;
        out.normal = vec4<f32>(packed, sqrt(max(1.0 - dot(packed, packed), 0.0)), n.a);
        out.height = n.b;
        return out;
    }
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
// The sunlit haze's glow at the game's exposure (the shafts pass takes it
// away again where the air is shaded).
const HAZE_GLOW: f32 = 3.2;

// The weather at a map point; past the map's edge, the air mass alone. Round a
// wheeling storm, the weather that has turned round to here (`vortex_warp_in`).
fn weather_at(at: vec2<f32>) -> vec4<f32> {
    let xy = vortex_warp_in(atmos.vortex, at);
    let size = atmos.weather.yz;
    let uv = xy / size;
    let edge = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let inside = textureSampleLevel(cloud_weather, clamp_sampler, uv, 0.0);
    if edge >= 0.02 {
        return inside;
    }
    let outside = vec4<f32>(air_mass_at(xy), 0.0, 0.0);
    return mix(outside, inside, smoothstep(0.0, 0.02, edge));
}

// The clouds' shade (clouds.wgsl `cs_shade`): r the sunlight they let through
// to a plane under the layer, per texel of the map. 1 everywhere with the
// clouds off.
@group(0) @binding(27) var cloud_shade: texture_2d<f32>;
// Ambient occlusion from the depth pre-pass, half the scene's size (renderer/gtao.rs).
@group(0) @binding(30) var ao_map: texture_2d<f32>;

// How much of the sky and bounce light reaches this pixel past what stands
// around it (1 open). `clip` is the fragment's position; only the ambient
// light is scaled by it.
fn screen_ao(clip: vec2<f32>) -> f32 {
    return textureSampleLevel(ao_map, clamp_sampler, clip / globals.scene.xy, 0.0).r;
}

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
    let up = clamp(d.z, 0.0, 1.0);
    // Kasten-Young air mass: how many zenith columns of air lie along `d`.
    let zenith_deg = degrees(acos(up));
    let mass = min(1.0 / (up + 0.50572 * pow(max(96.07995 - zenith_deg, 0.01), -1.6364)), 38.0);
    var sky = air_light(dot(d, globals.sun.xyz), mass);
    if d.z < 0.0 {
        // Below the horizon: the haze over far land.
        sky = mix(sky, atmos.horizon_color.rgb * 0.55 + atmos.ground_color.rgb * 0.6, clamp(-d.z * 3.0, 0.0, 1.0));
    }
    return sky;
}

// The light of `mass` zenith columns of air seen at `mu` (cosine) from the sun:
// the sky along a ray, or (at the horizon's mass) enough of the low air that
// nothing shows through it.
fn air_light(mu: f32, mass: f32) -> vec3<f32> {
    return air_light_g(mu, mass, 0.78);
}

// `air_light` with the Mie forward glow's sharpness `g`.
fn air_light_g(mu: f32, mass: f32, g: f32) -> vec3<f32> {
    let r = RAYLEIGH * RAYLEIGH_H * mass;
    let m = vec3<f32>(MIE * MIE_H * mass);
    let ext = r + m * 1.1;
    let scatter = r * phase_rayleigh(mu) + m * phase_hg(mu, g);
    // At night the scene's key light is a day-for-night moon, far brighter
    // than the real one; the dome it lights stays near black (`sun_color.w`).
    var sky = atmos.sky_sun.rgb * scatter / max(ext, vec3<f32>(1e-6)) * (1.0 - exp(-ext)) * SKY_GAIN * atmos.sun_color.w;
    // Light scattered more than once fills the shadowed side a little.
    sky += atmos.sky_color.rgb * 0.18 * (1.0 - exp(-ext * 2.0));
    return sky;
}

// What a glossy face at `p` sees mirrored along `r`: the sky's own colour
// (`sky_radiance`), the undersides of the clouds where the ray meets their
// layer (their shade map says where they are thick), the land below the
// horizon. Rough faces see it blurred toward the plain sky/ground average.
// `sky_vis` (occlusion) dims it the way it dims the sky light.
fn env_reflection(p: vec3<f32>, r: vec3<f32>, rough: f32, sky_vis: f32) -> vec3<f32> {
    let ground = atmos.ground_color.rgb;
    var sky = sky_radiance(normalize(vec3<f32>(r.xy, max(r.z, 0.02))));
    if r.z > 0.03 {
        let base = cloud_floor(p.xy) + atmos.layer.x;
        let t = max(base - p.z, 0.0) / r.z;
        let q = p.xy + r.xy * t;
        let uv = q / atmos.weather.yz;
        let shade = textureSampleLevel(cloud_shade, clamp_sampler, uv, 0.0).r;
        let cover = 1.0 - smoothstep(0.35, 0.95, shade);
        // A cloud's underside: sunlit white where it is thin, grey where it is thick.
        // A low sun lights the clouds' sides and tops, not their bellies.
        let belly = smoothstep(0.1, 0.6, globals.sun.z);
        let under = atmos.sun_color.rgb * mix(0.55, 0.2, cover) * atmos.sun_color.w * belly + atmos.sky_color.rgb * 0.6;
        // Far along the ray the clouds blur into the haze at the horizon.
        sky = mix(sky, under, cover * (1.0 - smoothstep(4000.0, 20000.0, t)));
    }
    let sharp = mix(ground * 0.9, sky, smoothstep(-0.12, 0.08, r.z));
    let blurred = mix(ground, atmos.sky_color.rgb * 1.25, clamp(r.z * 0.5 + 0.5, 0.0, 1.0));
    return mix(sharp, blurred, smoothstep(0.25, 0.75, rough)) * mix(0.35, 1.0, sky_vis);
}

// The low air, where the weather and the dust are: denser than the sky's
// air above it, blue from its molecules, and only a scale height of this deep,
// so a low eye looking far across the land sees a lot of it (distant hills go
// blue) while an eye high over the battle looks down through little of it.
const AERIAL_H: f32 = 500.0;
// Its Rayleigh scattering against sea-level air's.
const AERIAL: f32 = 1.0;
// The haze's glow against the drawn sky at the horizon.
const HAZE_SKY: f32 = 0.5;
// The haze toward a sun near the horizon: how sharply it glows forward (the sky's
// own Mie phase is 0.78), and how bright it is against `HAZE_SKY`.
const LOW_SUN_HAZE_G: f32 = 0.55;
const LOW_SUN_HAZE: f32 = 0.6;
// Damp air under a closed deck or in rain (`atmos.view.w` 1): grey haze per
// metre of that low air, on top.
const DAMP_MIE: f32 = 3.5e-5;

// Aerial perspective: light lost and gained on the way from `world` to the
// eye through the same air the sky is made of.
fn apply_haze(color: vec3<f32>, world: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    // Desert air is dry and clear: far less haze, so what is left is mostly
    // the air's own blue, the blue-violet that fills a canyon's depths.
    // (Across a climate wall the air changes over a few hundred metres, not on the line.)
    let dry = climate_within(world.xy, REGIONS_SKY_BLEND_M).y;
    var dry_air = vec2<f32>(1.0);
    if dry >= 1.0 {
        dry_air = vec2<f32>(0.7, 0.4);
    } else if dry > 0.0 {
        dry_air = mix(dry_air, vec2<f32>(0.7, 0.4), dry);
    }
    let damp = atmos.view.w;
    let low = air_column(eye, world, AERIAL_H);
    let column_r = (air_column(eye, world, RAYLEIGH_H) * HAZE_SCALE + low * AERIAL) * dry_air.x;
    let column_m = air_column(eye, world, MIE_H) * HAZE_SCALE * dry_air.y;
    let tau = RAYLEIGH * column_r + vec3<f32>(MIE * 1.1 * column_m + low * DAMP_MIE * damp);
    let through = exp(-tau);
    // Enough of this air glows like the sky at the horizon seen the same way
    // from the sun (single scattering in a thick layer of it), so far land
    // fades toward the sky behind it: blue on the way, paler at the end; a
    // little under the drawn sky, which is lifted for the look. Not
    // normalize(): a point at the eye (the clouds' march, down among them,
    // finds cloud right at it) made a NaN that drew as a white texel.
    let d = (world - eye) / max(length(world - eye), 1e-3);
    // Toward a low sun the haze's forward glow is softened, or near land
    // vanishes under a white veil at golden hour.
    let low_sun = 1.0 - smoothstep(0.12, 0.55, globals.sun.z);
    let g = mix(0.78, LOW_SUN_HAZE_G, low_sun);
    let clear = air_light_g(dot(d, globals.sun.xyz), 38.0, g) * HAZE_SKY * mix(1.0, LOW_SUN_HAZE, low_sun);
    // Under a closed deck the air is lit by the cloud's grey light from all
    // round instead: dimmer and greyer, but still cool, not slate.
    let deck = vec3<f32>(dot(clear, vec3<f32>(0.3, 0.5, 0.2))) * vec3<f32>(0.76, 0.9, 1.04) * 0.75;
    let glow = mix(clear, deck, damp * 0.75);
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

// Craters big blasts leave in the ground (renderer/craters.rs), shaded by terrain.wgsl.
struct Crater {
    // x, y, radius (metres), the time it was made.
    at: vec4<f32>,
    // heat (0-1), seconds it takes to cool, pool share of the radius, seed.
    look: vec4<f32>,
}
struct CraterList {
    // x how many are in use.
    count: vec4<u32>,
    // Size mirrors craters::MAX_CRATERS.
    items: array<Crater, 48>,
}
@group(0) @binding(29) var<storage, read> ground_craters: CraterList;

// Heat in the ground (renderer/ground_melt.rs): the layout is `MELT_FIELD_*`.
@group(0) @binding(32) var<storage, read> ground_melt: array<u32>;

// The slot (plus one) of the melt field's tile at `tile`, 0 if it has none. The
// renderer's `first_entry` hashes the same way.
fn melt_slot(tile: vec2<i32>) -> u32 {
    let key = ((u32(tile.x) & 0xFFFFu) << 16u) | (u32(tile.y) & 0xFFFFu);
    var e = (key * 0x9E3779B1u) >> (32u - MELT_FIELD_SLOT_BITS);
    for (var i = 0u; i < MELT_FIELD_PROBES; i++) {
        let at = MELT_FIELD_TABLE + e * 2u;
        let slot = ground_melt[at + 1u];
        if slot == 0u || ground_melt[at] == key {
            return slot;
        }
        e = (e + 1u) & (MELT_FIELD_SLOTS - 1u);
    }
    return 0u;
}

// One cell of the melt field, given the slot (plus one) of its tile: x heat (1
// white-hot), y glass, z scorch.
fn melt_cell(slot: u32, local: vec2<i32>) -> vec3<f32> {
    if slot == 0u {
        return vec3<f32>(0.0);
    }
    let i = MELT_FIELD_ATLAS + (slot - 1u) * MELT_FIELD_TILE * MELT_FIELD_TILE
        + u32(local.y) * MELT_FIELD_TILE + u32(local.x);
    let v = unpack4x8unorm(ground_melt[i]).xyz;
    return vec3<f32>(v.x * MELT_FIELD_HEAT_MAX, v.y, v.z);
}

// The melt field at `xy`, blended between the four nearest cells: x heat, y glass,
// z scorch. Zero wherever nothing has heated the ground.
fn melt_sample(xy: vec2<f32>) -> vec3<f32> {
    if ground_melt[0] == 0u {
        return vec3<f32>(0.0);
    }
    let p = xy / MELT_FIELD_CELL - 0.5;
    let c = vec2<i32>(floor(p));
    let f = p - floor(p);
    let size = i32(MELT_FIELD_TILE);
    let t0 = vec2<i32>(floor(vec2<f32>(c) / f32(size)));
    let t1 = vec2<i32>(floor(vec2<f32>(c + 1) / f32(size)));
    var v = array<vec3<f32>, 4>();
    if all(t0 == t1) {
        // All four in one tile, the usual case: one lookup.
        let slot = melt_slot(t0);
        if slot == 0u {
            return vec3<f32>(0.0);
        }
        let l = c - t0 * size;
        v[0] = melt_cell(slot, l);
        v[1] = melt_cell(slot, l + vec2<i32>(1, 0));
        v[2] = melt_cell(slot, l + vec2<i32>(0, 1));
        v[3] = melt_cell(slot, l + vec2<i32>(1, 1));
    } else {
        for (var k = 0; k < 4; k++) {
            let cell = c + vec2<i32>(k & 1, k >> 1u);
            let tile = vec2<i32>(floor(vec2<f32>(cell) / f32(size)));
            v[k] = melt_cell(melt_slot(tile), cell - tile * size);
        }
    }
    return mix(mix(v[0], v[1], f.x), mix(v[2], v[3], f.x), f.y);
}

// ---------------------------------------------------------------- under the sea

// Light lost per metre of the water, red first (water.wgsl `water_optics`): clear
// green coastal water, the Bahamas' very clear water (tropical), or a canyon
// reservoir's (desert). `climate` is `climate_at` where the water is.
fn sea_absorb(climate: vec2<f32>) -> vec3<f32> {
    let temperate = 1.0 - climate.x - climate.y;
    var absorb = vec3<f32>(0.0);
    if climate.x > 0.0 {
        absorb += vec3<f32>(0.30, 0.042, 0.026) * climate.x;
    }
    if climate.y > 0.0 {
        absorb += vec3<f32>(0.40, 0.068, 0.095) * climate.y;
    }
    if temperate > 0.0 {
        absorb += vec3<f32>(0.17, 0.032, 0.025) * temperate;
    }
    return absorb;
}

// Light lost per metre along a view from under the water the eye is in: clearer
// than the absorption says, so the seabed and a fight read out to 100 m or so.
fn under_sea_extinction() -> vec3<f32> {
    return sea_absorb(climate_at(globals.camera.xy, 0.0)) * 0.2 + vec3<f32>(0.004);
}

// With the free camera under the water (water.wgsl `under_sea`), what is drawn
// after the water is seen through it: the pixel at `clip` (its depth tells where
// it is) is dimmed by the water between it and the eye (xyz), and nothing above
// the surface shows (w 0): spray, smoke and the sky reach the eye only as the
// surface shows them. Everything is kept whole with the eye above the water.
fn under_sea_veil(clip: vec4<f32>) -> vec4<f32> {
    let water = globals.map.z;
    let eye = globals.camera.xyz;
    if eye.z >= water {
        return vec4<f32>(1.0);
    }
    let uv = clip.xy / globals.scene.xy;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, max(clip.z, 0.0000001), 1.0);
    let world = h.xyz / h.w;
    let range = distance(eye, world);
    // Nor what is right at the lens: a burst of air a few metres off filled the view white.
    let keep = (1.0 - smoothstep(water - 0.3, water + 0.3, world.z)) * smoothstep(1.5, 12.0, range);
    let through = exp(-under_sea_extinction() * min(range, 400.0));
    return vec4<f32>(through, keep);
}

// A colour drawn after the water, seen through it (`under_sea_veil`).
fn under_sea_seen(color: vec4<f32>, clip: vec4<f32>) -> vec4<f32> {
    let veil = under_sea_veil(clip);
    return vec4<f32>(color.rgb * veil.xyz * veil.w, color.a * veil.w * dot(veil.xyz, vec3<f32>(0.2, 0.5, 0.3)));
}
