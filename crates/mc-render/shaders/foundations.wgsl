//!use bindings
// Foundation walls where a structure's lot was levelled into the ground
// (renderer/foundations.rs): one block per 8 m cell of the step, its top flat
// across it, its sides down into the ground. Dressed stone in courses under a
// pale coping, grimy at the foot. A new lot's walls rise out of the ground
// while it settles (`settle::SECONDS`).

//!rust crate::renderer::foundations::FoundationCell
struct FoundationCell {
    origin: vec2<f32>,
    bottom: f32,
    start: f32,
    top: vec4<f32>,
    faces: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct FoundationPush {
    pass_kind: u32,
    unused: u32,
}

@group(1) @binding(0) var<storage, read> cells: array<FoundationCell>;
var<immediate> push: FoundationPush;

const CELL_M: f32 = 8.0;
// Courses of dressed stone: their height, and each block's length.
const COURSE_M: f32 = 0.9;
const BLOCK_M: f32 = 1.8;
// The stone coping along the top of a face; the rest of the top is packed gravel.
const COPING_M: f32 = 0.7;

struct VsOut {
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Where in its cell the point is, 0..1 each way, and the cell's `faces`.
    @location(2) cell_uv: vec2<f32>,
    @location(3) @interpolate(flat) faces: u32,
}

// The cell's corners, in `FoundationCell::top` order.
fn corner(k: u32) -> vec2<f32> {
    return vec2<f32>(f32(k & 1u), f32(k >> 1u));
}

// Top: two triangles split along (0,0)-(1,1) as the terrain's cells are. Then per
// side (-x, +x, -y, +y) its two corners, low along the side first.
const TOP_CORNERS = array<u32, 6>(0u, 1u, 3u, 0u, 3u, 2u);
const SIDE_A = array<u32, 4>(0u, 1u, 0u, 2u);
const SIDE_B = array<u32, 4>(2u, 3u, 1u, 3u);
const SIDE_NORMAL = array<vec2<f32>, 4>(
    vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, -1.0), vec2<f32>(0.0, 1.0),
);
// Per side quad vertex: which corner (0 a, 1 b) and whether it is at the top.
const QUAD_B = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
const QUAD_TOP = array<u32, 6>(1u, 1u, 0u, 1u, 0u, 0u);

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let c = cells[ii];
    var out: VsOut;
    // As the lot settles, the wall rises from the ground that is there to its full height.
    let t = clamp((globals.camera.w - c.start) / SETTLE_SECONDS, 0.0, 1.0);
    let rise = t * t * (3.0 - 2.0 * t);
    var k: u32;
    var at_top = true;
    var n = vec3<f32>(0.0, 0.0, 1.0);
    if vi < 6u {
        k = TOP_CORNERS[vi];
    } else {
        let side = (vi - 6u) / 6u;
        let q = (vi - 6u) % 6u;
        if (c.faces & (1u << side)) == 0u || rise <= 0.001 {
            // Against another wall cell: nothing to see.
            out.clip = vec4<f32>(0.0);
            return out;
        }
        k = select(SIDE_A[side], SIDE_B[side], QUAD_B[q] == 1u);
        at_top = QUAD_TOP[q] == 1u;
        n = vec3<f32>(SIDE_NORMAL[side], 0.0);
    }
    if rise <= 0.001 {
        out.clip = vec4<f32>(0.0);
        return out;
    }
    let xy = c.origin + corner(k) * CELL_M;
    var z = c.bottom;
    if at_top {
        z = mix(terrain_height(xy), c.top[k], rise);
    }
    let world = vec3<f32>(xy, z);
    if (push.pass_kind & PASS_KIND_MASK) == PASS_SHADOW {
        out.clip = globals.shadow_cascades[push.pass_kind >> PASS_CASCADE_SHIFT] * vec4<f32>(world, 1.0);
    } else {
        out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    }
    out.world = world;
    out.normal = n;
    out.cell_uv = corner(k);
    out.faces = c.faces;
    return out;
}

@fragment
fn fs_shadow() {
}

// Soft dark line `width` metres wide at each integer of `v`, faded out once a
// pixel (`px` in units of `v`) is too coarse to show it.
fn joint(v: f32, width: f32, px: f32) -> f32 {
    let d = min(fract(v), 1.0 - fract(v));
    return (1.0 - smoothstep(width * 0.5, width * 0.5 + px, d)) * (1.0 - smoothstep(0.15, 0.4, px));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let eye = globals.camera.xyz;
    let n = normalize(in.normal);
    let p = in.world;
    let px = max(length(fwidth(p)), 0.004);
    let grain = noise_varied(p.xy + p.z * vec2<f32>(0.37, 0.61), 3.0);
    var albedo: vec3<f32>;
    var rough = 0.85;
    if n.z > 0.5 {
        // Packed gravel fill, and a stone coping along each edge that has a face.
        let m = in.cell_uv * CELL_M;
        let big = 1e6;
        let to_face = min(
            min(select(big, m.x, (in.faces & 1u) != 0u), select(big, CELL_M - m.x, (in.faces & 2u) != 0u)),
            min(select(big, m.y, (in.faces & 4u) != 0u), select(big, CELL_M - m.y, (in.faces & 8u) != 0u)),
        );
        let clods = noise_varied(p.xy, 0.9);
        let gravel = vec3<f32>(0.13, 0.115, 0.09) * (0.7 + 0.5 * grain.r) * (0.8 + 0.4 * clods.g);
        var coping = vec3<f32>(0.3, 0.29, 0.27) * (0.9 + 0.2 * grain.g);
        coping *= 1.0 - 0.35 * joint(dot(p.xy, vec2<f32>(1.0)) / BLOCK_M, 0.03, px / BLOCK_M);
        albedo = mix(coping, gravel, smoothstep(COPING_M - px, COPING_M + px, to_face));
        rough = 0.9;
    } else {
        // Dressed stone laid in courses, each course's joints staggered half a block,
        // each block a shade of its own.
        let along = dot(p.xy, vec2<f32>(-n.y, n.x));
        let course = floor(p.z / COURSE_M);
        let run = along / BLOCK_M + course * 0.5;
        let block = hash21(vec2<f32>(floor(run), course));
        albedo = vec3<f32>(0.34, 0.33, 0.30) * (0.82 + 0.3 * block) * (0.88 + 0.24 * grain.g);
        albedo = mix(albedo, albedo * vec3<f32>(1.08, 0.98, 0.9), step(0.8, block));
        let lines = max(joint(p.z / COURSE_M, 0.04, px / COURSE_M), joint(run, 0.04 * COURSE_M / BLOCK_M, px / BLOCK_M));
        albedo *= 1.0 - 0.45 * lines;
        // Just under the coping it stays clean; lower down, runs of grime.
        let streak = smoothstep(0.55, 0.85, noise_varied(vec2<f32>(along * 0.35, p.z * 0.04), 9.0).b);
        albedo *= 1.0 - 0.25 * streak;
        // Dirt splashed up from the ground at its foot.
        let ground = terrain_height(p.xy + n.xy * 0.6);
        let foot = 1.0 - smoothstep(0.0, 1.6, p.z - ground);
        albedo = mix(albedo, albedo * vec3<f32>(0.55, 0.5, 0.42), foot);
        rough = 0.9;
    }
    // Rain darkens and wets it as it does the ground.
    let soaked = weather_at(p.xy).w;
    albedo *= 1.0 - 0.3 * soaked;
    rough = mix(rough, 0.45, soaked * 0.75);

    var m: Pbr;
    m.albedo = albedo;
    m.metallic = 0.0;
    m.roughness = rough;
    m.emissive = vec3<f32>(0.0);
    let v = normalize(eye - p);
    let shadow = sun_shadow(p, n);
    // A face down in a cut sees less of the sky than the coping does.
    let sky_vis = select(0.75, 0.95, n.z > 0.5) * screen_ao(in.clip.xy);
    var color = shade_pbr_vis(m, n, v, globals.sun.xyz, shadow, sky_vis);
    color += albedo * lightning_light(p, n) * 0.35;
    color += local_lights(m, p, n, v);
    color = apply_fog_of_war(color, p.xy);
    color = apply_haze(color, p, eye);
    return vec4<f32>(color, 1.0);
}
