// Screen-space passes: the sky behind everything, tone mapping from the HDR
// scene target to the swapchain, and the 2D overlay (UI, text, profiler).

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var font: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var bloom_chain: texture_2d<f32>;

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

// Weight that stops one very bright texel from flickering through the whole chain.
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

@fragment
fn fs_tonemap(in: FullOut) -> @location(0) vec4<f32> {
    let hdr = textureSampleLevel(scene, linear_sampler, in.uv, 0.0).rgb;
    let bloom = textureSampleLevel(bloom_chain, linear_sampler, in.uv, 0.0).rgb;
    let color = tonemap((hdr + bloom * 0.22) * push.a.x);
    let d = in.uv - vec2<f32>(0.5);
    let vignette = 1.0 - dot(d, d) * push.a.y;
    return vec4<f32>(color * vignette, 1.0);
}

struct OverlayIn {
    // Pixels from the top-left corner.
    @location(0) pos: vec2<f32>,
    // Overlay atlas coordinates; x < 0 means a solid fill.
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
    }
    return color;
}
