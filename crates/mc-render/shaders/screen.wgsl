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
@group(0) @binding(8) var<storage, read> haze: HeatPlumes;
@group(0) @binding(9) var<storage, read> flares: LensFlares;
fn effect_blocked(source: vec3<f32>, to: vec3<f32>) -> bool {
    for (var i = 0u; i < effect_barriers.header.x; i++) {
        if barrier_crosses(source, to, effect_barriers.entries[i]) { return true; }
    }
    return false;
}

// Mirrors the GPU shockwave record. A painted ring looked like a range circle;
// the front now bends the scene instead.
struct ScreenPush {
    // Overlay: 2 / width, 2 / height. Tonemap: exposure, vignette.
    // Bloom down: 1 for the first level, which keeps only the bright parts.
    // Bloom up: filter radius in source texels, weight of the level.
    a: vec2<f32>,
    // Tonemap: which of the 64 shockwave slots are live, as bits (two words).
    b: vec2<f32>,
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

// How far from the eye the scene at `uv` lies, metres.
fn scene_distance_at(uv: vec2<f32>, eye: vec3<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    let depth = textureLoad(scene_depth, pixel, 0);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = wave_globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    return length(h.xyz / h.w - eye);
}

// Screen offset and a whisper of lip light source every live pressure sphere.
fn wave_bend(uv: vec2<f32>) -> vec3<f32> {
    if bitcast<u32>(push.b.x) == 0u && bitcast<u32>(push.b.y) == 0u {
        return vec3<f32>(0.0);
    }
    let eye = wave_globals.camera.xyz;
    let rd = normalize(world_from_uv(uv) - eye);
    let scene_distance = scene_distance_at(uv, eye);
    var offset = vec2<f32>(0.0);
    var lip = 0.0;
    // Only the live slots (the tone map's push), lowest first.
    var live = vec2<u32>(bitcast<u32>(push.b.x), bitcast<u32>(push.b.y));
    loop {
        var i = 0u;
        if live.x != 0u {
            i = firstTrailingBit(live.x);
            live.x &= live.x - 1u;
        } else if live.y != 0u {
            i = 32u + firstTrailingBit(live.y);
            live.y &= live.y - 1u;
        } else {
            break;
        }
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

// Hot air rising off an engine's exhaust (renderer/heat_haze.rs): a column from the
// port's mouth that widens as it rises and thins out at the top.
//!rust crate::renderer::heat_haze::GpuHeatPlume
struct HeatPlume {
    port: vec3<f32>,
    radius: f32,
    axis: vec3<f32>,
    height: f32,
    // 0 cold, 1 flat out.
    strength: f32,
    seed: f32,
    _pad0: f32,
    _pad1: f32,
}
struct HeatPlumes {
    // x: plumes in use.
    header: vec4<u32>,
    entries: array<HeatPlume>,
}

// Where the scene seen through the plumes at `uv` comes from: a small offset of rising,
// churning noise (xy, uv units), and how far off the nearest plume it went through lies
// (z, metres; zero with no offset). Shimmer, not glass: a pixel or two, no colour split.
fn haze_bend(uv: vec2<f32>) -> vec3<f32> {
    let count = min(haze.header.x, HAZE_MAX_PLUMES);
    if count == 0u {
        return vec3<f32>(0.0);
    }
    let eye = wave_globals.camera.xyz;
    let rd = normalize(world_from_uv(uv) - eye);
    let time = wave_globals.camera.w;
    var scene_distance = -1.0;
    var offset = vec2<f32>(0.0);
    var nearest = 1.0e9;
    for (var i = 0u; i < count; i++) {
        let p = haze.entries[i];
        // Where the view ray passes closest to the column's axis.
        let w = eye - p.port;
        let b = dot(rd, p.axis);
        let d = dot(rd, w);
        let e = dot(p.axis, w);
        let den = max(1.0 - b * b, 1e-4);
        let s = clamp((e - b * d) / den, 0.0, p.height);
        let on_axis = p.port + p.axis * s;
        let t = dot(on_axis - eye, rd);
        if t <= 0.5 {
            continue;
        }
        let rise = s / p.height;
        let reach = p.radius * (1.0 + 1.8 * rise);
        let across = on_axis - (eye + rd * t);
        let off = length(across) / reach;
        if off >= 1.0 {
            continue;
        }
        if scene_distance < 0.0 {
            scene_distance = scene_distance_at(uv, eye);
        }
        if t > scene_distance {
            continue;
        }
        // Soft across the column; comes on over the mouth and thins out going up.
        let core = (1.0 - off * off) * (1.0 - off * off);
        let fade = smoothstep(0.0, 0.1, rise) * pow(1.0 - rise, 1.6);
        // Noise in the column's own metres, carried up with the gas as it churns.
        let side = normalize(cross(p.axis, rd) + vec3<f32>(1e-5, 0.0, 0.0));
        let lateral = dot(across, side);
        let speed = 2.4 + 2.6 * p.strength;
        let cell = p.radius * 0.55;
        let q = vec2<f32>(lateral, s - time * speed) + vec2<f32>(p.seed * 37.0, p.seed * 11.0);
        let churn = vec2<f32>(time * 0.9, -time * speed * 0.6);
        let n = vec2<f32>(
            value_noise2(q, cell) + 0.5 * value_noise2(q * 1.9 + churn, cell) - 0.75,
            value_noise2(q + vec2<f32>(17.3, 5.1), cell) + 0.5 * value_noise2(q * 2.3 - churn, cell) - 0.75,
        );
        // A few centimetres of bend where the column is thick, as pixels at its distance.
        let px_per_m = wave_globals.lod.x / max(t, 1.0);
        let amount = core * fade * (0.25 + 0.75 * p.strength);
        let px = min(0.15 * p.radius * px_per_m, HAZE_MAX_PX) * amount;
        offset += n * 2.0 * px * vec2<f32>(0.7, 1.0) * wave_globals.viewport.zw;
        nearest = min(nearest, t);
    }
    if dot(offset, offset) == 0.0 {
        return vec3<f32>(0.0);
    }
    return vec3<f32>(offset, nearest);
}

// Lens flares on bright points (renderer/lens_flare.rs): what a small, very bright light
// does in the camera's glass. A soft core, a star of thin spikes (one lens, so every
// star turns the same way), a long flat streak across it, and faint ghosts strung on
// the line from the light through the middle of the picture. Hidden, softly, where
// the scene stands nearer than the light.
//!rust crate::renderer::lens_flare::GpuLensFlare
struct LensFlare {
    // 0..1 across and down.
    at: vec2<f32>,
    // How far the spikes reach, output pixels.
    radius: f32,
    // Metres from the eye.
    distance: f32,
    // Colour times brightness.
    color: vec3<f32>,
    ghosts: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}
struct LensFlares {
    // x: flares in use.
    header: vec4<u32>,
    entries: array<LensFlare>,
}

// How much of a flare's light gets past the scene: five taps round its middle, each
// open where the scene there lies beyond the light.
fn flare_seen(f: LensFlare) -> f32 {
    let eye = wave_globals.camera.xyz;
    let step = 2.5 * wave_globals.viewport.zw;
    let slack = 4.0 + f.distance * 0.02;
    var seen = 0.0;
    for (var i = 0; i < 5; i++) {
        let o = vec2<f32>(f32(i == 1) - f32(i == 2), f32(i == 3) - f32(i == 4)) * step;
        seen += select(0.0, 0.2, scene_distance_at(f.at + o, eye) + slack > f.distance);
    }
    return seen;
}

// A spike along `dir` through the light: hair-thin, bright at the root and fading out
// to its tip at `reach` pixels.
fn flare_spike(p: vec2<f32>, dir: vec2<f32>, reach: f32, width: f32) -> f32 {
    let along = abs(dot(p, dir));
    let across = abs(p.x * dir.y - p.y * dir.x);
    let fall = max(1.0 - along / reach, 0.0);
    return exp(-across / width) * fall * fall * fall;
}

fn lens_flare(uv: vec2<f32>) -> vec3<f32> {
    let count = min(flares.header.x, LENS_MAX_FLARES);
    var light = vec3<f32>(0.0);
    let px = wave_globals.viewport.xy;
    let middle = vec2<f32>(0.5);
    // The star's turn, the same for every flare: it is the lens's.
    let a = vec2<f32>(0.966, 0.259);
    let b = vec2<f32>(-a.y, a.x);
    let c = normalize(a + b);
    let d = vec2<f32>(-c.y, c.x);
    for (var i = 0u; i < count; i++) {
        let f = flares.entries[i];
        let p = (uv - f.at) * px;
        let r = length(p);
        let reach = f.radius;
        // Ghosts: soft discs on the line from the light through the middle, in the
        // lens's own tints, bigger the further they are thrown.
        var ghost = vec3<f32>(0.0);
        if f.ghosts > 0.0 {
            let axis = middle - f.at;
            for (var g = 0; g < 3; g++) {
                let t = array<f32, 3>(0.55, 1.25, 1.7)[g];
                let size = reach * array<f32, 3>(0.06, 0.12, 0.08)[g];
                let tint = array<vec3<f32>, 3>(
                    vec3<f32>(0.5, 0.8, 1.0),
                    vec3<f32>(0.7, 1.0, 0.6),
                    vec3<f32>(1.0, 0.6, 0.9),
                )[g];
                let q = length((uv - (f.at + axis * t)) * px) / max(size, 1.0);
                if q < 1.0 {
                    // A disc with a slightly brighter rim, as a lens element throws it.
                    ghost += tint * (smoothstep(1.0, 0.85, q) * (0.35 + 0.65 * q * q));
                }
            }
        }
        let near = r < reach * 1.6;
        if !near && dot(ghost, ghost) == 0.0 {
            continue;
        }
        let seen = flare_seen(f);
        if seen <= 0.0 {
            continue;
        }
        var shape = 0.0;
        if near {
            let core = exp(-r * r / (reach * reach * 0.004));
            let halo = exp(-r / (reach * 0.12)) * 0.25;
            let spikes = flare_spike(p, a, reach, 0.7) + flare_spike(p, b, reach, 0.7)
                + 0.45 * (flare_spike(p, c, reach * 0.55, 0.6) + flare_spike(p, d, reach * 0.55, 0.6));
            // The flat streak: twice the spikes' reach, thin, tinted cool.
            let streak = flare_spike(p, vec2<f32>(1.0, 0.0), reach * 1.6, 0.9) * 0.35;
            shape = core * 3.0 + halo + spikes;
            light += f.color * seen * shape + mix(f.color, vec3<f32>(0.6, 0.75, 1.0) * max(f.color.r, max(f.color.g, f.color.b)), 0.5) * seen * streak;
        }
        light += ghost * f.ghosts * seen * 0.018 * max(f.color.r, max(f.color.g, f.color.b));
    }
    return light;
}

// Render scale. The scene may be larger than the output (supersampled) or
// smaller (a cheaper frame); the tone mapper resamples it here.

fn scene_hdr(uv: vec2<f32>) -> vec3<f32> {
    return min(max(textureSampleLevel(scene, linear_sampler, uv, 0.0).rgb, vec3<f32>(0.0)), vec3<f32>(64.0));
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

fn tonemapped(in: FullOut) -> vec3<f32> {
    let bend = wave_bend(in.uv);
    let uv = in.uv;
    var o = bend.xy;
    let heat = haze_bend(uv);
    // Hot air bends only what lies behind it: a nearer hull is not dragged into the plume.
    if heat.z > 0.0 && scene_distance_at(uv + o + heat.xy, wave_globals.camera.xyz) > heat.z {
        o += heat.xy;
    }
    let bloom = textureSampleLevel(bloom_chain, linear_sampler, uv + o, 0.0).rgb * 0.22 + lens_flare(uv);
    var color = resolve(uv + o, bloom);
    if dot(bend.xy, bend.xy) > 0.0 {
        // A hair of chromatic split so the warp reads on even ground.
        color.r = tonemap((scene_hdr(uv + o * 1.08) + bloom) * push.a.x).r;
        color.b = tonemap((scene_hdr(uv + o * 0.92) + bloom) * push.a.x).b;
    }
    color += vec3<f32>(0.92, 0.93, 0.9) * bend.z;
    // A nuclear flash: the view goes white and comes back slowly, the eye still
    // dazzled (renderer/nuke_fx.rs).
    let nuke = wave_globals.nuke_view;
    color *= 1.0 - nuke.y;
    color = mix(color, vec3<f32>(1.0, 0.985, 0.95), clamp(nuke.x, 0.0, 1.0));
    let d = in.uv - vec2<f32>(0.5);
    let vignette = 1.0 - dot(d, d) * push.a.y;
    return color * vignette;
}

@fragment
fn fs_tonemap(in: FullOut) -> @location(0) vec4<f32> {
    return vec4<f32>(tonemapped(in), 1.0);
}

// The same into a UNORM image, sRGB-encoded, for SMAA and FSR (post.wgsl).
@fragment
fn fs_tonemap_ldr(in: FullOut) -> @location(0) vec4<f32> {
    let c = clamp(tonemapped(in), vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return vec4<f32>(select(hi, lo, c <= vec3<f32>(0.0031308)), 1.0);
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
