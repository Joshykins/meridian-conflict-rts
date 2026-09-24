// Local lights: weapon flashes, blasts, fires, shots in flight, beams, lamps
// on hulls (lights.rs). build.rs appends this after bindings.wgsl.
//
// The CPU bins every light into clusters: a 32x18 grid over the screen, times
// 24 slices of distance from the eye (logarithmic, 4 m to 40 km). A shaded
// point finds its cluster and walks only the lights listed there, so a pixel
// far from any fire pays one grid read.

struct Light {
    pos: vec3<f32>,
    // Metres at which the light has faded to nothing.
    range: f32,
    // Linear RGB times intensity: irradiance at one metre, in the sun's units.
    color: vec3<f32>,
    kind: u32,
    // Spot and pair: unit direction. Line: the segment from `pos`.
    axis: vec3<f32>,
    cos_outer: f32,
    cos_inner: f32,
    // Pair: metres from the middle to each lamp.
    pair: f32,
    // Radius of the glowing source: keeps the falloff finite up close.
    size: f32,
    // Share of the light a spot lamp spills outside its cone.
    spill: f32,
}

const LIGHT_POINT: u32 = 0u;
const LIGHT_SPOT: u32 = 1u;
const LIGHT_LINE: u32 = 2u;
const LIGHT_PAIR: u32 = 3u;

const LIGHT_TILES_X: u32 = 32u;
const LIGHT_TILES_Y: u32 = 18u;
const LIGHT_SLICES: u32 = 24u;
const LIGHT_CLUSTERS: u32 = 13824u; // 32 * 18 * 24
const LIGHT_NEAR: f32 = 4.0;
// LIGHT_SLICES / ln(40000 / LIGHT_NEAR)
const LIGHT_SLICE_SCALE: f32 = 2.6057;

@group(0) @binding(25) var<storage, read> lights: array<Light>;
// LIGHT_CLUSTERS words of (first index << 8 | count), then the light indices.
@group(0) @binding(26) var<storage, read> light_grid: array<u32>;

fn light_cluster(world: vec3<f32>) -> u32 {
    let clip = globals.view_proj * vec4<f32>(world, 1.0);
    if clip.w <= 0.0 {
        return 0xFFFFFFFFu;
    }
    let uv = clamp(clip.xy / clip.w * 0.5 + 0.5, vec2<f32>(0.0), vec2<f32>(0.9999));
    let tile = vec2<u32>(uv * vec2<f32>(f32(LIGHT_TILES_X), f32(LIGHT_TILES_Y)));
    let d = max(distance(world, globals.camera.xyz), LIGHT_NEAR);
    let slice = min(u32(log(d / LIGHT_NEAR) * LIGHT_SLICE_SCALE), LIGHT_SLICES - 1u);
    return (slice * LIGHT_TILES_Y + tile.y) * LIGHT_TILES_X + tile.x;
}

// Light arriving at `world` from one light: xyz toward the light, w unused;
// the returned colour is the irradiance on a face turned to it.
struct Arrival {
    l: vec3<f32>,
    e: vec3<f32>,
}

fn light_arrival(li: Light, world: vec3<f32>) -> Arrival {
    var out: Arrival;
    out.e = vec3<f32>(0.0);
    out.l = vec3<f32>(0.0, 0.0, 1.0);
    var to = li.pos - world;
    if li.kind == LIGHT_LINE {
        let t = clamp(dot(-to, li.axis) / max(dot(li.axis, li.axis), 1e-4), 0.0, 1.0);
        to += li.axis * t;
    }
    let d2 = dot(to, to);
    let r2 = li.range * li.range;
    if d2 >= r2 {
        return out;
    }
    let l = to * inverseSqrt(max(d2, 1e-6));
    // Inverse square, windowed so it reaches exactly zero at `range`.
    let q = d2 / r2;
    let window = clamp(1.0 - q * q, 0.0, 1.0);
    var e = window * window / (d2 + li.size * li.size);
    if li.kind == LIGHT_SPOT || li.kind == LIGHT_PAIR {
        let outward = -l;
        var cone = smoothstep(li.cos_outer, li.cos_inner, dot(outward, li.axis));
        if li.kind == LIGHT_PAIR {
            // Two beams side by side: the pools overlap in the middle and part
            // at the edges, the way a vehicle's lamps light the road.
            let side = normalize(cross(li.axis, vec3<f32>(0.0, 0.0, 1.0)) + vec3<f32>(1e-5, 0.0, 0.0));
            let along = max(dot(-to, li.axis), 0.0);
            let lateral = dot(-to, side);
            let spread = sqrt(max(1.0 - li.cos_outer * li.cos_outer, 0.0)) / max(li.cos_outer, 0.1);
            let w = along * spread * 0.55 + 0.25;
            let a = (lateral - li.pair) / w;
            let b = (lateral + li.pair) / w;
            cone *= 0.3 + 0.7 * max(exp(-a * a), exp(-b * b));
        }
        e *= max(cone, li.spill);
    }
    out.l = l;
    out.e = li.color * e;
    return out;
}

// Local light reflected toward the eye by a PBR surface.
fn local_lights(m: Pbr, world: vec3<f32>, n: vec3<f32>, v: vec3<f32>) -> vec3<f32> {
    let cluster = light_cluster(world);
    if cluster == 0xFFFFFFFFu {
        return vec3<f32>(0.0);
    }
    let word = light_grid[cluster];
    let count = word & 0xFFu;
    if count == 0u {
        return vec3<f32>(0.0);
    }
    let first = LIGHT_CLUSTERS + (word >> 8u);
    let n_dot_v = max(dot(n, v), 0.001);
    let rough = clamp(m.roughness, 0.08, 1.0);
    let a = rough * rough;
    let f0 = mix(vec3<f32>(0.04), m.albedo, m.metallic);
    let diffuse = (1.0 - m.metallic) * m.albedo / PI;
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        let arrival = light_arrival(lights[light_grid[first + i]], world);
        let n_dot_l = dot(n, arrival.l);
        if n_dot_l <= 0.0 || all(arrival.e == vec3<f32>(0.0)) {
            continue;
        }
        let h = normalize(v + arrival.l);
        let f = f0 + (1.0 - f0) * pow(clamp(1.0 - dot(h, v), 0.0, 1.0), 5.0);
        let spec = d_ggx(max(dot(n, h), 0.0), a) * g_smith(n_dot_v, n_dot_l, rough) * f
            / (4.0 * n_dot_v * max(n_dot_l, 0.001));
        sum += ((1.0 - f) * diffuse + spec) * arrival.e * n_dot_l;
    }
    return sum;
}

// Local light falling on a soft, scattering thing (smoke, dust): no facing,
// just how much arrives, for the vertex shader of a billboard.
fn local_light_volume(world: vec3<f32>) -> vec3<f32> {
    let cluster = light_cluster(world);
    if cluster == 0xFFFFFFFFu {
        return vec3<f32>(0.0);
    }
    let word = light_grid[cluster];
    let count = word & 0xFFu;
    let first = LIGHT_CLUSTERS + (word >> 8u);
    var sum = vec3<f32>(0.0);
    for (var i = 0u; i < count; i++) {
        sum += light_arrival(lights[light_grid[first + i]], world).e;
    }
    return sum;
}
