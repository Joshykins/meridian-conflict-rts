// Screen-space passes: the sky behind everything, tone mapping from the HDR
// scene target to the swapchain, and the 2D overlay (UI, text, profiler).

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var font: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;

struct ScreenPush {
    // Overlay: 2 / width, 2 / height. Tonemap: exposure, vignette.
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

@fragment
fn fs_tonemap(in: FullOut) -> @location(0) vec4<f32> {
    let hdr = textureSampleLevel(scene, linear_sampler, in.uv, 0.0).rgb;
    // Cheap glow: a wide 8-tap ring of the bright parts, so emitters and tracers bloom.
    let texel = 1.0 / vec2<f32>(textureDimensions(scene));
    var bloom = vec3<f32>(0.0);
    for (var i = 0; i < 8; i++) {
        let a = f32(i) * 0.7853982;
        let o = vec2<f32>(cos(a), sin(a));
        let near = textureSampleLevel(scene, linear_sampler, in.uv + o * texel * 3.0, 0.0).rgb;
        let far = textureSampleLevel(scene, linear_sampler, in.uv + o * texel * 8.0, 0.0).rgb;
        bloom += max(near - vec3<f32>(1.6), vec3<f32>(0.0)) * 0.6 + max(far - vec3<f32>(1.6), vec3<f32>(0.0)) * 0.4;
    }
    let color = tonemap((hdr + bloom * 0.09) * push.a.x);
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
