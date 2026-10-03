// Scanned metal on the models: each faction's plate is finished with a scan of real
// metal (data/textures/metal) in the terrain material array. Prepended after
// surface.wgsl (it uses its noise) to shaders that contain the line `//!use metal`
// (entity.wgsl); regency.wgsl uses it for the Regency's worn steel, and `arc_metal`
// below is ARC's own: brushed steel, long scratches through the paint.
//
// A scan is used only as variation about its own mean (its grain, scratches and
// sheen), never its colour. It is mapped in model space by three planar projections
// blended by the face's normal, shifted per face (`seed`) so neighbouring plates
// never show the same patch; the GPU's mips take it to its mean with distance.

struct ScanAt {
    // Model space, metres, the face's normal there (unit), and the model-space step
    // to the next pixel right and down.
    local: vec3<f32>,
    n: vec3<f32>,
    dl1: vec3<f32>,
    dl2: vec3<f32>,
    // Length of a typical plate on this model (`SurfaceIn::scale`).
    scale: f32,
    // Per face, zero to one, shared with its mirror twin.
    seed: f32,
    // The scan's colour layer (its detail layer is the next one), its mean
    // luminance and roughness (what reads as "no change"), and the metres one
    // repeat covers on a model.
    layer: i32,
    lum: f32,
    rough: f32,
    tile: f32,
}

struct MetalScan {
    // Multiplies the colour; added to roughness; slope of its relief (model space);
    // how deep a scratch is here (0 none).
    tone: f32,
    rough: f32,
    slope: vec3<f32>,
    scratch: f32,
}

fn scan_sample(at: ScanAt, uv: vec2<f32>, d1: vec2<f32>, d2: vec2<f32>, u: vec3<f32>, v: vec3<f32>) -> MetalScan {
    let c = textureSampleGrad(terrain_materials, repeat_sampler, uv, at.layer, d1, d2);
    let nm = textureSampleGrad(terrain_materials, repeat_sampler, uv, at.layer + 1, d1, d2);
    let lum = dot(c.rgb, vec3<f32>(0.3, 0.59, 0.11));
    let t = nm.xy * 2.0 - 1.0;
    // A tangent normal leaning +u means the surface falls toward +u.
    return MetalScan(lum / at.lum - 1.0, c.a - at.rough, -(t.x * u + t.y * v), nm.z);
}

fn scan_mix(a: MetalScan, b: MetalScan, t: f32) -> MetalScan {
    return MetalScan(mix(a.tone, b.tone, t), mix(a.rough, b.rough, t), mix(a.slope, b.slope, t), mix(a.scratch, b.scratch, t));
}

fn scan_add(a: MetalScan, b: MetalScan, w: f32) -> MetalScan {
    return MetalScan(a.tone + b.tone * w, a.rough + b.rough * w, a.slope + b.slope * w, a.scratch + b.scratch * w);
}

// One projection of the scan, never repeating on a grid: a slow noise picks, place
// to place, which of eight offsets of the scan shows, and blends across from one to
// the next (Quilez, "texture repetition").
fn scan_axis(at: ScanAt, uv: vec2<f32>, d1: vec2<f32>, d2: vec2<f32>, u: vec3<f32>, v: vec3<f32>, axis: f32) -> MetalScan {
    let pick = surf_noise3(vec3<f32>(uv * 0.45, axis * 17.0)) * 8.0;
    let i = floor(pick);
    let f = fract(pick);
    let o0 = vec2<f32>(hash11(i * 13.7 + axis), hash11(i * 7.1 + axis + 0.5));
    let o1 = vec2<f32>(hash11((i + 1.0) * 13.7 + axis), hash11((i + 1.0) * 7.1 + axis + 0.5));
    let a = scan_sample(at, uv + o0, d1, d2, u, v);
    let b = scan_sample(at, uv + o1, d1, d2, u, v);
    return scan_mix(a, b, smoothstep(0.25, 0.75, f));
}

fn metal_scan(at: ScanAt) -> MetalScan {
    var w = pow(abs(at.n), vec3<f32>(4.0));
    w /= max(w.x + w.y + w.z, 1e-6);
    // A big model is seen from further off: its scan is laid coarser, so it still reads.
    let k = 1.0 / (at.tile * clamp(at.scale * 0.7, 1.0, 3.5));
    let shift = vec2<f32>(fract(at.seed * 7.91), fract(at.seed * 3.37));
    let p = at.local * k;
    let a = at.dl1 * k;
    let b = at.dl2 * k;
    var out = MetalScan(0.0, 0.0, vec3<f32>(0.0), 0.0);
    if w.x > 0.01 {
        out = scan_add(out, scan_axis(at, p.yz + shift, a.yz, b.yz, vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), 1.0), w.x);
    }
    if w.y > 0.01 {
        out = scan_add(out, scan_axis(at, p.xz + shift, a.xz, b.xz, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), 2.0), w.y);
    }
    if w.z > 0.01 {
        out = scan_add(out, scan_axis(at, p.xy + shift, a.xy, b.xy, vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), 3.0), w.z);
    }
    return out;
}

// ARC's scan (`metal_scan::ARC_LAYER`): brushed steel with long gouges. Its means
// (scripts/import-metal.py prints them).
const ARC_SCAN_LUM: f32 = 0.297;
const ARC_SCAN_ROUGH: f32 = 0.469;

fn arc_scan_at(local: vec3<f32>, n: vec3<f32>, dl1: vec3<f32>, dl2: vec3<f32>, scale: f32, seed: f32) -> ScanAt {
    return ScanAt(local, n, dl1, dl2, scale, seed, METAL_SCAN_ARC_LAYER, ARC_SCAN_LUM, ARC_SCAN_ROUGH, METAL_SCAN_ARC_TILE_M);
}

// Bare steel where ARC's paint is scratched through: brighter than surface.wgsl's
// scuffed edges, so a scratch a pixel wide still reads.
const ARC_BARE: vec3<f32> = vec3<f32>(0.62, 0.61, 0.6);

// ARC plate (paint over steel): the brushed grain under the paint, and long scratches
// through it to the bright metal beneath, fuller on the dark trim than on paint.
// The material, and the scan's relief as a slope in model space.
struct ArcMetal {
    m: Pbr,
    slope: vec3<f32>,
}

fn arc_metal(m_in: Pbr, at: ScanAt, dark: bool) -> ArcMetal {
    var m = m_in;
    let s = metal_scan(at);
    m.albedo *= max(1.0 + 0.6 * s.tone, 0.3);
    m.roughness = clamp(m.roughness + 0.9 * s.rough, 0.08, 1.0);
    let scratch = smoothstep(0.12, 0.6, s.scratch) * select(0.8, 1.0, dark);
    m.albedo = mix(m.albedo, ARC_BARE, scratch * 0.9);
    m.metallic = mix(m.metallic, 0.9, scratch);
    m.roughness = mix(m.roughness, 0.28, scratch);
    return ArcMetal(m, s.slope * 0.7);
}
