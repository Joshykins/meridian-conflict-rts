// Screen-space passes: the sky behind everything, tone mapping source the HDR
// scene target to the swapchain, and the 2D overlay (UI, text, profiler).

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var font: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var bloom_chain: texture_2d<f32>;
@group(0) @binding(4) var<storage, read> waves: array<Shockwave>;
@group(0) @binding(5) var<uniform> wave_globals: Globals;
@group(0) @binding(6) var<storage, read> effect_barriers: EffectBarriers;
@group(0) @binding(7) var scene_depth: texture_depth_2d;
fn effect_blocked(source: vec3<f32>, to: vec3<f32>) -> bool {
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        if barrier_crosses(source, to, effect_barriers.entries[i]) { return true; }
    }
    return false;
}

// Mirrors the GPU shockwave record. A painted ring looked like a range circle;
// the front now bends the scene instead.
struct Shockwave {
    pos: vec3<f32>,
    start: f32,
    params: vec4<f32>,
    axis: vec3<f32>,
    _pad: f32,
    tint: vec4<f32>,
}

struct ScreenPush {
    // Overlay: 2 / width, 2 / height. Tonemap: exposure, vignette.
    // Bloom down: 1 for the first level, which keeps only the bright parts.
    // Bloom up: filter radius in source texels, weight of the level.
    a: vec2<f32>,
}

var<immediate> push: ScreenPush;

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullOut {
    // One triangle that covers the viewport.
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

// Bloom: the bright parts of the scene are taken down a chain of half-size
// images and added back up it, so light spreads smoothly around an emitter
// however many pixels it covers. `scene` is the level being read.

// What is left of a colour above the bloom threshold, with a soft knee.
fn bright_part(c: vec3<f32>) -> vec3<f32> {
    let threshold = 1.9;
    let knee = 0.8;
    let level = max(c.r, max(c.g, c.b));
    let soft = clamp(level - threshold + knee, 0.0, 2.0 * knee);
    let keep = max(soft * soft / (4.0 * knee), level - threshold) / max(level, 1e-4);
    return c * keep;
}

// Weight that stops one very bright texel source flickering through the whole chain.
fn firefly_weight(c: vec3<f32>) -> f32 {
    return 1.0 / (1.0 + dot(c, vec3<f32>(0.2126, 0.7152, 0.0722)));
}

// One tap of the downsample. The first level keeps only the bright part, and
// first makes the colour sane: one NaN or infinity in the scene would spread
// through every level. `max` comes first because, given a NaN, it returns the
// other operand, where a `select` on a comparison may compile to a multiply that keeps it.
fn bloom_tap(uv: vec2<f32>, first: bool) -> vec3<f32> {
    let v = textureSampleLevel(scene, linear_sampler, uv, 0.0).rgb;
    if first {
        return bright_part(min(max(v, vec3<f32>(0.0)), vec3<f32>(64.0)));
    }
    return v;
}

// Average of four taps; on the first level weighted down by its brightness.
fn bloom_box(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>, weight: f32, first: bool) -> vec4<f32> {
    let color = (a + b + c + d) * 0.25;
    var w = weight;
    if first {
        w = weight * firefly_weight(color);
    }
    return vec4<f32>(color * w, w);
}

@fragment
fn fs_bloom_down(in: FullOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(scene));
    let first = push.a.x > 0.5;
    // Thirteen taps: a centre box and four overlapping corner boxes.
    let a = bloom_tap(in.uv + vec2<f32>(-2.0, -2.0) * t, first);
    let b = bloom_tap(in.uv + vec2<f32>(0.0, -2.0) * t, first);
    let c = bloom_tap(in.uv + vec2<f32>(2.0, -2.0) * t, first);
    let d = bloom_tap(in.uv + vec2<f32>(-1.0, -1.0) * t, first);
    let e = bloom_tap(in.uv + vec2<f32>(1.0, -1.0) * t, first);
    let f = bloom_tap(in.uv + vec2<f32>(-2.0, 0.0) * t, first);
    let g = bloom_tap(in.uv, first);
    let h = bloom_tap(in.uv + vec2<f32>(2.0, 0.0) * t, first);
    let i = bloom_tap(in.uv + vec2<f32>(-1.0, 1.0) * t, first);
    let j = bloom_tap(in.uv + vec2<f32>(1.0, 1.0) * t, first);
    let k = bloom_tap(in.uv + vec2<f32>(-2.0, 2.0) * t, first);
    let l = bloom_tap(in.uv + vec2<f32>(0.0, 2.0) * t, first);
    let m = bloom_tap(in.uv + vec2<f32>(2.0, 2.0) * t, first);
    let sum = bloom_box(d, e, i, j, 0.5, first) + bloom_box(a, b, f, g, 0.125, first) + bloom_box(b, c, g, h, 0.125, first)
        + bloom_box(f, g, k, l, 0.125, first) + bloom_box(g, h, l, m, 0.125, first);
    return vec4<f32>(sum.rgb / max(sum.a, 1e-5), 1.0);
}

// Blended additively onto the next larger level.
@fragment
fn fs_bloom_up(in: FullOut) -> @location(0) vec4<f32> {
    let t = push.a.x / vec2<f32>(textureDimensions(scene));
    var sum = vec3<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let w = f32((2 - abs(x)) * (2 - abs(y))) / 16.0;
            sum += textureSampleLevel(scene, linear_sampler, in.uv + vec2<f32>(f32(x), f32(y)) * t, 0.0).rgb * w;
        }
    }
    return vec4<f32>(sum * push.a.y, 1.0);
}

// The water's copy of the opaque scene (`Renderer::refract`).
@fragment
fn fs_refract_copy(in: FullOut) -> @location(0) vec4<f32> {
    // The opaque scene as the water will see it under itself. NaNs become black,
    // or one bad pixel bleeds through every bent sample of the sea.
    let c = textureLoad(scene, vec2<i32>(in.clip.xy), 0).rgb;
    return vec4<f32>(min(max(c, vec3<f32>(0.0)), vec3<f32>(64.0)), 1.0);
}

// Glass: a quarter-size, strongly blurred copy of the finished picture, which
// overlay panels show through (`Overlay::blur_rect`). Only run on frames that
// have such a panel.

// Tone maps the scene as `fs_tonemap` does, four bilinear taps averaging the
// 4x4 texels under each quarter-size texel. Taps are tone mapped one by one, so
// a lone bright pixel does not smear into a bright blot. Push: exposure, vignette.
@fragment
fn fs_glass_source(in: FullOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(scene));
    let bloom = textureSampleLevel(bloom_chain, linear_sampler, in.uv, 0.0).rgb * 0.22;
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let o = vec2<f32>(f32(i & 1) * 2.0 - 1.0, f32(i >> 1u) * 2.0 - 1.0);
        let hdr = min(max(textureSampleLevel(scene, linear_sampler, in.uv + o * t, 0.0).rgb, vec3<f32>(0.0)), vec3<f32>(64.0));
        sum += tonemap((hdr + bloom) * push.a.x);
    }
    let d = in.uv - vec2<f32>(0.5);
    return vec4<f32>(sum * 0.25 * (1.0 - dot(d, d) * push.a.y), 1.0);
}

// One direction of a separable Gaussian (sigma 4 texels: 16 screen pixels at
// any resolution). Push: the step between taps, in texels.
@fragment
fn fs_glass_blur(in: FullOut) -> @location(0) vec4<f32> {
    let step = push.a / vec2<f32>(textureDimensions(scene));
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var i = -9; i <= 9; i++) {
        let w = exp(-f32(i * i) / 32.0);
        sum += textureSampleLevel(scene, linear_sampler, in.uv + step * f32(i), 0.0).rgb * w;
        total += w;
    }
    return vec4<f32>(sum / total, 1.0);
}

fn sphere_hits(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, r: f32) -> vec2<f32> {
    let oc = ro - c;
    let b = dot(oc, rd);
    let disc = b * b - dot(oc, oc) + r * r;
    if disc < 0.0 {
        return vec2<f32>(-1.0);
    }
    let s = sqrt(max(disc, 0.0));
    return vec2<f32>(-b - s, -b + s);
}

fn world_from_uv(uv: vec2<f32>) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = wave_globals.inv_view_proj * vec4<f32>(ndc, 0.01, 1.0);
    return h.xyz / max(h.w, 1e-5);
}

// Screen offset and a whisper of lip light source every live pressure sphere.
fn wave_bend(uv: vec2<f32>) -> vec3<f32> {
    let eye = wave_globals.camera.xyz;
    let rd = normalize(world_from_uv(uv) - eye);
    let dims = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    let depth = textureLoad(scene_depth, pixel, 0);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = wave_globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    let scene_distance = length(h.xyz / h.w - eye);
    var offset = vec2<f32>(0.0);
    var lip = 0.0;
    for (var i = 0u; i < 64u; i++) {
        let e = waves[i];
        let age = (wave_globals.camera.w - e.start) / max(e.params.y, 0.001);
        if age < 0.0 || age >= 1.0 || e.params.x <= 0.0 {
            continue;
        }
        let grow = 1.0 - (1.0 - age) * (1.0 - age);
        let reach = e.params.x * grow;
        if reach < 0.4 {
            continue;
        }
        let ts = sphere_hits(eye, rd, e.pos, reach);
        var t = ts.x;
        if t <= 0.001 {
            t = ts.y;
        }
        if t <= 0.001 {
            continue;
        }
        if t > scene_distance { continue; }
        let p = eye + rd * t;
        if effect_blocked(e.pos, p) { continue; }
        let n = (p - e.pos) / max(reach, 0.001);
        let facing = abs(dot(n, -rd));
        let radius_px = reach * wave_globals.lod.x / max(length(eye - e.pos), 1.0);
        let bands = shockwave_bands(facing, radius_px);
        let fade = shockwave_fade(age);
        let axis_len = length(e.axis);
        var directional = 1.0;
        if axis_len > 0.5 {
            directional = smoothstep(-0.45, 0.65, dot(n, e.axis / axis_len));
        }
        let body_bend = 0.38 * facing * sqrt(max(1.0 - facing * facing, 0.0));
        let amp = (bands.z + body_bend) * fade * (0.4 + 0.6 * e.params.w) * directional;
        let clip = wave_globals.view_proj * vec4<f32>(e.pos, 1.0);
        if clip.w < 0.15 {
            continue;
        }
        let cuv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
        var away = (uv - cuv) * wave_globals.viewport.xy;
        let len = length(away);
        if len > 1e-5 {
            away = away / len;
        }
        // A few pixels of push at the front — compressed air, not a drawn ring.
        offset += away * amp * 4.5 * wave_globals.viewport.zw;
        // Refraction only: no painted circular lip.
    }
    return vec3<f32>(offset, min(lip, 0.12));
}

// Render scale and FXAA. The scene may be larger than the output (supersampled)
// or smaller (a cheaper frame); the tone mapper resamples it here. FXAA works
// in output pixels, on what the player sees.

fn scene_hdr(uv: vec2<f32>) -> vec3<f32> {
    return min(max(textureSampleLevel(scene, linear_sampler, uv, 0.0).rgb, vec3<f32>(0.0)), vec3<f32>(64.0));
}

// Perceptual brightness after exposure, near what the tone map makes of it.
fn fxaa_luma(uv: vec2<f32>) -> f32 {
    let l = dot(scene_hdr(uv), vec3<f32>(0.2126, 0.7152, 0.0722)) * push.a.x;
    return sqrt(l / (1.0 + l));
}

// FXAA 3.11, quality preset: where to sample so a stair-stepped edge blends
// across itself. `px` is one output pixel in uv.
fn fxaa_uv(uv: vec2<f32>, px: vec2<f32>) -> vec2<f32> {
    let m = fxaa_luma(uv);
    let n = fxaa_luma(uv + vec2<f32>(0.0, -px.y));
    let s = fxaa_luma(uv + vec2<f32>(0.0, px.y));
    let e = fxaa_luma(uv + vec2<f32>(px.x, 0.0));
    let w = fxaa_luma(uv + vec2<f32>(-px.x, 0.0));
    let hi = max(m, max(max(n, s), max(e, w)));
    let lo = min(m, min(min(n, s), min(e, w)));
    let range = hi - lo;
    if range < max(0.0312, hi * 0.125) {
        return uv;
    }
    let nw = fxaa_luma(uv - px);
    let se = fxaa_luma(uv + px);
    let ne = fxaa_luma(uv + vec2<f32>(px.x, -px.y));
    let sw = fxaa_luma(uv + vec2<f32>(-px.x, px.y));
    // An edge that runs across the screen changes brightness down it.
    let across = abs(nw + sw - 2.0 * w) + abs(n + s - 2.0 * m) * 2.0 + abs(ne + se - 2.0 * e);
    let down = abs(nw + ne - 2.0 * n) + abs(w + e - 2.0 * m) * 2.0 + abs(sw + se - 2.0 * s);
    let horizontal = across >= down;
    var l1 = w;
    var l2 = e;
    var step = px.x;
    var along = vec2<f32>(0.0, px.y);
    if horizontal {
        l1 = n;
        l2 = s;
        step = px.y;
        along = vec2<f32>(px.x, 0.0);
    }
    // Step toward the side of the edge that differs most from this pixel.
    let g1 = l1 - m;
    let g2 = l2 - m;
    let gradient = 0.25 * max(abs(g1), abs(g2));
    var edge_luma = 0.5 * (l2 + m);
    if abs(g1) >= abs(g2) {
        step = -step;
        edge_luma = 0.5 * (l1 + m);
    }
    var on_edge = uv + vec2<f32>(step * 0.5, 0.0);
    if horizontal {
        on_edge = uv + vec2<f32>(0.0, step * 0.5);
    }
    // Walk both ways along the edge to its ends.
    var p1 = on_edge - along;
    var p2 = on_edge + along;
    var end1 = fxaa_luma(p1) - edge_luma;
    var end2 = fxaa_luma(p2) - edge_luma;
    let strides = array<f32, 11>(1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 2.0, 2.0, 4.0, 8.0);
    for (var i = 0; i < 11; i++) {
        let done1 = abs(end1) >= gradient;
        let done2 = abs(end2) >= gradient;
        if done1 && done2 {
            break;
        }
        if !done1 {
            p1 -= along * strides[i];
            end1 = fxaa_luma(p1) - edge_luma;
        }
        if !done2 {
            p2 += along * strides[i];
            end2 = fxaa_luma(p2) - edge_luma;
        }
    }
    var d1 = uv.y - p1.y;
    var d2 = p2.y - uv.y;
    if horizontal {
        d1 = uv.x - p1.x;
        d2 = p2.x - uv.x;
    }
    let nearer_end = select(end2, end1, d1 < d2);
    var shift = 0.0;
    // Only blend when this pixel is on the side of the edge its nearer end says.
    if (nearer_end < 0.0) != (m < edge_luma) {
        shift = 0.5 - min(d1, d2) / max(d1 + d2, 1e-6);
    }
    // Lone bright or dark pixels: blend by how much they stand out of the area.
    let area = (2.0 * (n + s + e + w) + nw + ne + sw + se) / 12.0;
    let sub = clamp(abs(area - m) / range, 0.0, 1.0);
    let sub_shape = (3.0 - 2.0 * sub) * sub * sub;
    shift = max(shift, sub_shape * sub_shape * 0.75);
    if horizontal {
        return uv + vec2<f32>(0.0, shift * step);
    }
    return uv + vec2<f32>(shift * step, 0.0);
}

// The scene at an output pixel, tone mapped. When supersampled, four taps
// cover the scene pixels under it, each tone mapped before they are averaged
// so one very bright scene pixel does not whiten the whole output pixel.
fn resolve(uv: vec2<f32>, bloom: vec3<f32>) -> vec3<f32> {
    let scale = wave_globals.scene.z;
    if scale <= 1.0 {
        return tonemap((scene_hdr(uv) + bloom) * push.a.x);
    }
    let t = 0.25 * scale / wave_globals.scene.xy;
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let o = vec2<f32>(f32(i & 1) * 2.0 - 1.0, f32(i >> 1u) * 2.0 - 1.0) * t;
        sum += tonemap((scene_hdr(uv + o) + bloom) * push.a.x);
    }
    return sum * 0.25;
}

@fragment
fn fs_tonemap(in: FullOut) -> @location(0) vec4<f32> {
    let bend = wave_bend(in.uv);
    let o = bend.xy;
    var uv = in.uv;
    if wave_globals.scene.w > 0.5 {
        uv = fxaa_uv(in.uv, wave_globals.viewport.zw);
    }
    let bloom = textureSampleLevel(bloom_chain, linear_sampler, uv + o, 0.0).rgb * 0.22;
    var color = resolve(uv + o, bloom);
    if dot(o, o) > 0.0 {
        // A hair of chromatic split so the warp reads on even ground.
        color.r = tonemap((scene_hdr(uv + o * 1.08) + bloom) * push.a.x).r;
        color.b = tonemap((scene_hdr(uv + o * 0.92) + bloom) * push.a.x).b;
    }
    color += vec3<f32>(0.92, 0.93, 0.9) * bend.z;
    let d = in.uv - vec2<f32>(0.5);
    let vignette = 1.0 - dot(d, d) * push.a.y;
    return vec4<f32>(color * vignette, 1.0);
}

struct OverlayIn {
    // Pixels source the top-left corner.
    @location(0) pos: vec2<f32>,
    // Overlay atlas coordinates; x < 0 means a solid fill, x < -1.5 glass.
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct OverlayOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_overlay(in: OverlayIn) -> OverlayOut {
    var out: OverlayOut;
    out.clip = vec4<f32>(in.pos.x * push.a.x - 1.0, 1.0 - in.pos.y * push.a.y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_overlay(in: OverlayOut) -> @location(0) vec4<f32> {
    var color = in.color;
    if in.uv.x >= 0.0 {
        // Glyphs are white with coverage in alpha; images carry their own colour.
        color *= textureSampleLevel(font, linear_sampler, in.uv, 0.0);
    } else if in.uv.x < -1.5 {
        // Glass: on frames with any, `scene` is bound to the blurred picture.
        // The colour tints it by its alpha; uv.y is how opaque the panel itself
        // is (1 unless it is fading in or out).
        let blurred = textureSampleLevel(scene, linear_sampler, in.clip.xy * push.a * 0.5, 0.0).rgb;
        return vec4<f32>(mix(blurred, color.rgb, color.a), in.uv.y);
    }
    return color;
}
