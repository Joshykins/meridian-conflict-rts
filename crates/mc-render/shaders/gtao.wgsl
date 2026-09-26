// Ground-truth ambient occlusion (Jimenez et al. 2016, "Practical Realtime
// Strategies for Accurate Indirect Occlusion", after Intel's XeGTAO), from the
// depth pre-pass, before anything is shaded (renderer/gtao.rs).
//
// Half the scene's size. Each texel walks two slices of the view in screen
// space, finds the highest horizon on each side within `radius` metres and
// integrates the sky the normal can see between them. The slice angles and
// step offsets follow a 4x4 pattern, which the 4x4 blur then averages out
// exactly, weighted by distance so it does not bleed across silhouettes.
// Only the ambient light is darkened by it (`screen_ao` in bindings.wgsl).
//
// Pixels here are rows of the depth image, the same as a fragment's clip.xy
// in the scene pass; world positions come back the way water.wgsl's do.

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var raw_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var raw_in: texture_2d<f32>;
@group(0) @binding(4) var ao_out: texture_storage_2d<rgba8unorm, write>;

struct GtaoPush {
    // x radius (m), y how strongly (exponent on the visibility), z the largest
    // search on screen (full-size pixels), w 1 on, 0 off (all open).
    a: vec4<f32>,
}

var<immediate> gtao: GtaoPush;

const GTAO_SLICES: i32 = 2;
const GTAO_STEPS: i32 = 6;

fn gtao_world(pix: vec2<f32>, depth: f32, size: vec2<f32>) -> vec3<f32> {
    let uv = pix / size;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    return h.xyz / h.w;
}

fn gtao_depth(pix: vec2<i32>, size: vec2<i32>) -> f32 {
    return textureLoad(depth_tex, clamp(pix, vec2<i32>(0), size - 1), 0);
}

// A 4x4 ordered pattern in [0, 1): each of the 16 texels of a block takes a different value.
fn bayer4(p: vec2<u32>) -> f32 {
    let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return (m[(p.y & 3u) * 4u + (p.x & 3u)] + 0.5) / 16.0;
}

@compute @workgroup_size(8, 8)
fn cs_gtao(@builtin(global_invocation_id) id: vec3<u32>) {
    let out_size = textureDimensions(raw_out);
    if id.x >= out_size.x || id.y >= out_size.y {
        return;
    }
    let size_i = vec2<i32>(textureDimensions(depth_tex));
    let size = vec2<f32>(size_i);
    let pix = min(vec2<i32>(id.xy) * 2, size_i - 1);
    let depth = gtao_depth(pix, size_i);
    // The sky (depth 0 in reversed Z) is open; so is everything with GTAO off.
    if depth <= 0.0 || gtao.a.w < 0.5 {
        textureStore(raw_out, id.xy, vec4<f32>(1.0, 0.0, 0.0, 0.0));
        return;
    }
    let centre = vec2<f32>(pix) + 0.5;
    let p = gtao_world(centre, depth, size);
    let eye = globals.camera.xyz;
    let dist = distance(p, eye);
    let v = (eye - p) / max(dist, 1e-3);

    // The surface's normal from its neighbours' depths, taking the nearer side
    // on each axis so an edge does not bend it.
    let pl = gtao_world(centre + vec2<f32>(-1.0, 0.0), gtao_depth(pix + vec2<i32>(-1, 0), size_i), size);
    let pr = gtao_world(centre + vec2<f32>(1.0, 0.0), gtao_depth(pix + vec2<i32>(1, 0), size_i), size);
    let pu = gtao_world(centre + vec2<f32>(0.0, -1.0), gtao_depth(pix + vec2<i32>(0, -1), size_i), size);
    let pd = gtao_world(centre + vec2<f32>(0.0, 1.0), gtao_depth(pix + vec2<i32>(0, 1), size_i), size);
    let dx = select(p - pl, pr - p, length(pr - p) < length(p - pl));
    let dy = select(p - pu, pd - p, length(pd - p) < length(p - pu));
    var n = normalize(cross(dx, dy));
    if dot(n, v) < 0.0 {
        n = -n;
    }

    // Wider as the view pulls back, so a base still grounds its buildings from afar.
    let radius = max(gtao.a.x, dist * 0.012);
    // How many full-size pixels the radius covers here.
    let screen_r = radius * globals.lod.x * globals.scene.z / dist;
    if screen_r < 1.0 {
        textureStore(raw_out, id.xy, vec4<f32>(1.0, dist, 0.0, 0.0));
        return;
    }
    let r_px = min(screen_r, gtao.a.z);
    // Samples past 0.6 of the radius count for less, and none past it.
    let falloff_range = 0.615 * radius;
    let falloff_mul = -1.0 / falloff_range;
    let falloff_add = (radius - falloff_range) / falloff_range + 1.0;

    let noise_slice = bayer4(id.xy);
    let noise_step = bayer4(id.xy / 4u + vec2<u32>(id.y & 3u, id.x & 3u));
    var visibility = 0.0;
    for (var slice = 0; slice < GTAO_SLICES; slice++) {
        let phi = (f32(slice) + noise_slice) * PI / f32(GTAO_SLICES);
        let omega = vec2<f32>(cos(phi), sin(phi));
        // The slice's direction in the world, across the view at this depth.
        let across = gtao_world(centre + omega * 4.0, depth, size) - p;
        let ortho = normalize(across - v * dot(across, v));
        let axis = normalize(cross(ortho, v));
        let proj_n = n - axis * dot(n, axis);
        let proj_len = length(proj_n);
        let sign_n = select(-1.0, 1.0, dot(ortho, proj_n) >= 0.0);
        let cos_n = clamp(dot(proj_n, v) / max(proj_len, 1e-5), 0.0, 1.0);
        let n_ang = sign_n * acos(cos_n);
        let low0 = cos(n_ang + PI * 0.5);
        let low1 = cos(n_ang - PI * 0.5);
        var hcos0 = low0;
        var hcos1 = low1;
        for (var s = 0; s < GTAO_STEPS; s++) {
            var t = (f32(s) + noise_step) / f32(GTAO_STEPS);
            // More samples close in, where contact shadows are.
            t *= t;
            let off = omega * max(t * r_px, f32(s) + 1.0);
            let q0 = centre + off;
            let q1 = centre - off;
            let d0 = gtao_depth(vec2<i32>(floor(q0)), size_i);
            let d1 = gtao_depth(vec2<i32>(floor(q1)), size_i);
            let s0 = gtao_world(floor(q0) + 0.5, d0, size) - p;
            let s1 = gtao_world(floor(q1) + 0.5, d1, size) - p;
            let l0 = length(s0);
            let l1 = length(s1);
            let w0 = select(clamp(l0 * falloff_mul + falloff_add, 0.0, 1.0), 0.0, d0 <= 0.0);
            let w1 = select(clamp(l1 * falloff_mul + falloff_add, 0.0, 1.0), 0.0, d1 <= 0.0);
            let c0 = mix(low0, dot(s0 / max(l0, 1e-4), v), w0);
            let c1 = mix(low1, dot(s1 / max(l1, 1e-4), v), w1);
            hcos0 = max(hcos0, c0);
            hcos1 = max(hcos1, c1);
        }
        var h0 = -acos(clamp(hcos1, -1.0, 1.0));
        var h1 = acos(clamp(hcos0, -1.0, 1.0));
        h0 = n_ang + clamp(h0 - n_ang, -PI * 0.5, PI * 0.5);
        h1 = n_ang + clamp(h1 - n_ang, -PI * 0.5, PI * 0.5);
        let sin_n = sin(n_ang);
        let arc0 = (cos_n + 2.0 * h0 * sin_n - cos(2.0 * h0 - n_ang)) * 0.25;
        let arc1 = (cos_n + 2.0 * h1 * sin_n - cos(2.0 * h1 - n_ang)) * 0.25;
        visibility += proj_len * (arc0 + arc1);
    }
    visibility = clamp(visibility / f32(GTAO_SLICES), 0.0, 1.0);
    // Fade in as the radius grows past a couple of pixels, so zooming out does not pop.
    visibility = mix(1.0, visibility, smoothstep(1.0, 3.0, screen_r));
    textureStore(raw_out, id.xy, vec4<f32>(pow(visibility, gtao.a.y), dist, 0.0, 0.0));
}

// 4x4 blur over the pattern, each tap weighted by how near its distance is to this texel's.
@compute @workgroup_size(8, 8)
fn cs_gtao_blur(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(ao_out);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let here = textureLoad(raw_in, vec2<i32>(id.xy), 0);
    if here.y <= 0.0 {
        textureStore(ao_out, id.xy, vec4<f32>(here.x));
        return;
    }
    let max_i = vec2<i32>(size) - 1;
    var sum = 0.0;
    var total = 0.0;
    for (var y = -1; y <= 2; y++) {
        for (var x = -1; x <= 2; x++) {
            let t = textureLoad(raw_in, clamp(vec2<i32>(id.xy) + vec2<i32>(x, y), vec2<i32>(0), max_i), 0);
            let w = clamp(1.0 - abs(t.y - here.y) / (here.y * 0.02 + 0.2), 0.0, 1.0);
            sum += t.x * w;
            total += w;
        }
    }
    textureStore(ao_out, id.xy, vec4<f32>(sum / max(total, 1e-4)));
}
