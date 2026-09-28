//!use bindings
// Light shafts: sunlight the haze scatters toward the eye, taken away again
// wherever the air along the view is in shadow, so shafts of lit air show
// between the clouds' shadows and behind hills and hulls.
//
// The scene's haze (`apply_haze`) treats all the air along the view as
// equally sunlit. This pass walks the view ray from the eye to the scene's
// depth and compares the light at each step (the clouds' shade, and the sun's
// shadow cascades where they reach) with the air's around the focus: darker
// stretches take in-scattered sunlight away, brighter ones (a gap in a dark
// deck) add it, so an even overcast is left as it was. The walk runs at half
// the scene's size into a target of its own (renderer/shafts.rs); the
// composite applies it in `scene_over` with reverse-subtract blending, before
// the clouds are laid over the picture.
//
// Set 1: 0 the scene's depth, 1 the walk's target.

@group(1) @binding(0) var shaft_depth: texture_depth_2d;
@group(1) @binding(1) var shaft_march: texture_2d<f32>;

// How much denser the air's shafts read than the haze they come from: the
// view rarely crosses more than a kilometre of air, which on its own hardly shows.
const SHAFT_GAIN: f32 = 5.0;
const SHAFT_STEPS: i32 = 12;
// The furthest the walk goes, for the sky and far land.
const SHAFT_REACH: f32 = 9000.0;

struct ShaftOut {
    @builtin(position) clip: vec4<f32>,
}

@vertex
fn vs_shafts(@builtin(vertex_index) index: u32) -> ShaftOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: ShaftOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// One comparison in the nearest cascade that holds the point; lit outside all three.
fn shaft_map(x: vec3<f32>) -> f32 {
    var lit = 1.0;
    if globals.map.w > 0.0 {
        for (var i = 0u; i < 3u; i++) {
            let c = shadow_coord(i, x, globals.sun.xyz);
            if c.w < 0.97 {
                lit = mix(1.0, textureSampleCompareLevel(shadow_map, shadow_sampler, c.xy, i, c.z), globals.map.w);
                break;
            }
        }
    }
    return lit;
}

// Sunlight the clouds let through at `x`, 0-1. cloud_shadow never goes under
// 0.3 (light scattered under the deck): here the full range.
fn shaft_cloud(x: vec3<f32>) -> f32 {
    return clamp((cloud_shadow(x) - 0.3) / 0.7, 0.0, 1.0);
}

@fragment
fn fs_shafts_march(in: ShaftOut) -> @location(0) vec4<f32> {
    if globals.sun.z <= 0.02 || atmos.sun_color.w <= 0.0 {
        return vec4<f32>(0.0);
    }
    // This target is half the scene's size: the scene pixel under this texel.
    let pix = in.clip.xy * 2.0;
    let uv = pix / globals.scene.xy;
    let size = vec2<i32>(textureDimensions(shaft_depth));
    let depth = textureLoad(shaft_depth, clamp(vec2<i32>(pix), vec2<i32>(0), size - 1), 0);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    let end = h.xyz / h.w;
    let eye = globals.camera.xyz;
    let span = end - eye;
    let full = length(span);
    let dir = span / max(full, 1e-3);
    let reach = min(full, SHAFT_REACH);
    let mu = dot(dir, globals.sun.xyz);
    let phase_r = phase_rayleigh(mu);
    let phase_m = phase_hg(mu, 0.7);
    let dt = reach / f32(SHAFT_STEPS);
    // Interleaved gradient noise: the steps start at a different offset in each pixel.
    let jitter = fract(52.9829189 * fract(dot(in.clip.xy, vec2<f32>(0.06711056, 0.00583715))));
    // The haze already holds the light of air as sunny as it is around the focus
    // on average: under an even deck that air is left alone, and only what is
    // brighter (gaps) or darker (thicker cloud, hills, hulls) than it shows.
    let focus = vec3<f32>(globals.tree_wind.yz, terrain_height(globals.tree_wind.yz) + 60.0);
    var around = shaft_cloud(focus);
    for (var k = 0; k < 4; k++) {
        let a = f32(k) * 1.5708 + 0.4;
        around += shaft_cloud(focus + vec3<f32>(cos(a), sin(a), 0.0) * 450.0);
    }
    let reference = max(around / 5.0, 0.08);
    var lost = vec3<f32>(0.0);
    var through = vec3<f32>(1.0);
    for (var s = 0; s < SHAFT_STEPS; s++) {
        let x = eye + dir * ((f32(s) + jitter) * dt);
        let z = max(x.z, -50.0);
        let air_r = exp(-z / RAYLEIGH_H) * HAZE_SCALE;
        let air_m = exp(-z / MIE_H) * HAZE_SCALE;
        let scatter = RAYLEIGH * air_r * phase_r + vec3<f32>(MIE * air_m * phase_m);
        // Past 1 (a gap in a dark deck) it adds light rather than taking it away.
        let shade = 1.0 - min(shaft_map(x) * shaft_cloud(x) / reference, 3.0);
        lost += through * scatter * shade * dt;
        through *= exp(-(RAYLEIGH * air_r + vec3<f32>(MIE * 1.1 * air_m)) * dt);
    }
    let removed = atmos.sun_color.rgb * atmos.sun_color.w * lost * HAZE_GLOW * SHAFT_GAIN;
    return vec4<f32>(removed, 1.0);
}

// Full size, in `scene_over`: the walk's result, taken off the picture.
@fragment
fn fs_shafts_composite(in: ShaftOut) -> @location(0) vec4<f32> {
    let removed = textureSampleLevel(shaft_march, clamp_sampler, in.clip.xy / globals.scene.xy, 0.0).rgb;
    // Negative where a gap lets more sun through than around it: reverse-subtract adds it.
    return vec4<f32>(removed, 0.0);
}
