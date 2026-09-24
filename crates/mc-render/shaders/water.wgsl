//!use bindings
// The sea: one oversized quad on the water plane, drawn after the opaque
// scene has been copied (`Renderer::refract`). Everything is in the fragment
// shader:
//
// - The surface is a height field of drifting noise octaves plus Gerstner
//   swell, differentiated at the pixel's footprint so ripples filter out
//   instead of aliasing; what filters out widens the sun's highlight instead.
// - What lies under the water (seabed, the drowned part of a hull) is the
//   copied scene, bent by the surface and absorbed along the real path the
//   light takes through the water, red first. Shallows turn turquoise over
//   sand and deep water is only the colour light scatters back.
// - Caustics are drawn on that seabed, at its real position.
// - Reflections march the copied scene (hulls, cliffs) and fall back to a
//   height-field walk of the coast and then the sky.
// - Foam: surf that rolls in over the real bathymetry, rings where hulls and
//   structures stand in the water, and whitecaps on the swell crests.
// - What happens on the water (renderer/water_fx.rs, `sea_fx`): rings spreading
//   from splashes and blasts with their foam, the flash of a blast caught by the
//   water round it, and the wakes of moving hulls laid along the path they took.
//
// A tessellated mesh does not pay from the play camera — 40 cm of lift is less
// than a pixel — and it overdrew badly on a projected grid.

@group(2) @binding(0) var sea_under: texture_2d<f32>;
@group(2) @binding(2) var sea_sampler: sampler;
@group(2) @binding(7) var sea_depth: texture_depth_2d;

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vs_water(@builtin(vertex_index) index: u32) -> WaterOut {
    // Two triangles, larger than the map so the sea reaches the horizon.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let c = corners[index];
    let xy = (c - 0.5) * globals.map.xy * 2.3 + globals.map.xy * 0.5;
    var out: WaterOut;
    out.world = vec3<f32>(xy, globals.map.z);
    out.clip = globals.view_proj * vec4<f32>(out.world, 1.0);
    return out;
}

// ---------------------------------------------------------------- swell

// Spectrum spans orbit swell down to close chop. Each train fades once its
// wavelength is a couple of pixels, or the sea becomes a stamped grid.
const WATER_WAVES: u32 = 6u;

// Direction, wavelength, amplitude. Wind is east-north-east. Wavelengths
// are incommensurate and the headings fan out so crests do not lock.
fn water_train(i: u32) -> vec4<f32> {
    switch i {
        case 0u: { return vec4<f32>(0.91, 0.41, 860.0, 2.4); }
        case 1u: { return vec4<f32>(0.28, 0.96, 310.0, 1.35); }
        case 2u: { return vec4<f32>(0.95, -0.32, 118.0, 0.55); }
        case 3u: { return vec4<f32>(0.42, 0.91, 47.0, 0.26); }
        case 4u: { return vec4<f32>(0.88, 0.47, 19.6, 0.11); }
        default: { return vec4<f32>(-0.22, 0.98, 8.8, 0.05); }
    }
}

struct Swell {
    slope: vec2<f32>,
    // -1 in a trough, 1 on a crest; weighted toward the trains still visible.
    peak: f32,
}

fn swell(xy: vec2<f32>, time: f32, amp: f32, pixel: f32) -> Swell {
    var slope = vec2<f32>(0.0);
    var peak = 0.0;
    var peak_w = 0.0;
    for (var i = 0u; i < WATER_WAVES; i++) {
        let w = water_train(i);
        let fade = smoothstep(2.0, 7.5, w.z / max(pixel, 0.001));
        if fade < 0.02 {
            continue;
        }
        let dir = normalize(w.xy);
        let k = 6.283185 / w.z;
        let a = w.w * amp * fade;
        let phase = k * dot(dir, xy) - sqrt(9.81 * k) * time;
        slope += dir * (k * a * cos(phase));
        let weight = 0.45 + 0.55 * fade;
        peak += sin(phase) * weight;
        peak_w += weight;
    }
    var out: Swell;
    out.slope = slope;
    out.peak = peak / max(peak_w, 0.001);
    return out;
}

// ---------------------------------------------------------------- ripples

const RIPPLE_OCTAVES: u32 = 7u;

// Cell size in metres, steepness, drift in m/s, heading of the drift.
fn ripple_octave(i: u32) -> vec4<f32> {
    switch i {
        case 0u: { return vec4<f32>(420.0, 0.070, 7.0, 0.35); }
        case 1u: { return vec4<f32>(130.0, 0.080, 4.2, 0.95); }
        case 2u: { return vec4<f32>(44.0, 0.100, 2.6, -0.25); }
        case 3u: { return vec4<f32>(15.5, 0.110, 1.6, 0.55); }
        case 4u: { return vec4<f32>(5.6, 0.105, 1.05, 0.15); }
        case 5u: { return vec4<f32>(2.1, 0.095, 0.62, 0.40); }
        default: { return vec4<f32>(0.85, 0.080, 0.38, 0.70); }
    }
}

// How much of an octave a pixel `pixel` metres across can still show.
fn ripple_fade(cell: f32, pixel: f32) -> f32 {
    return smoothstep(2.2, 6.0, cell / max(pixel, 0.0001));
}

// Height of the fine surface at `xy` in metres, octaves the pixel cannot
// resolve left out. A slow warp from the broad octaves keeps the finer ones
// from reading as a texture sliding across the sea.
fn ripple_height(xy: vec2<f32>, time: f32, pixel: f32, calm: f32) -> f32 {
    let warp = vec2<f32>(
        grad_noise2(xy + vec2<f32>(time * 1.9, -time * 1.3), 61.0),
        grad_noise2(xy.yx + vec2<f32>(97.0 - time * 1.1, 13.0 + time * 1.7), 73.0),
    ) - 0.5;
    var h = 0.0;
    for (var i = 0u; i < RIPPLE_OCTAVES; i++) {
        let o = ripple_octave(i);
        let fade = ripple_fade(o.x, pixel);
        if fade <= 0.0 {
            continue;
        }
        let a = o.w + f32(i) * 1.7;
        let r = vec2<f32>(cos(a), sin(a));
        // Rotate the lattice per octave so no two share an axis. The wind
        // ripples (the finer octaves) are drawn out across it, longer than wide.
        var p = vec2<f32>(xy.x * r.x - xy.y * r.y, xy.x * r.y + xy.y * r.x);
        if i >= 3u {
            let wind = vec2<f32>(0.91, 0.41);
            let along = dot(p, wind);
            p += wind * along * -0.45;
        }
        let drift = vec2<f32>(cos(o.w), sin(o.w)) * o.z * time;
        let n = grad_noise2(p + drift + warp * o.x * 0.9, o.x);
        // Sharpen crests a little: water peaks, troughs are round.
        let crest = n * n * (3.0 - 2.0 * n);
        h += (crest - 0.5) * o.x * o.y * fade * mix(1.0, 0.45, calm * f32(i < 3u));
    }
    return h;
}

// Slope the octaves the pixel filtered away would have had: it spreads the
// sun's highlight instead of vanishing.
fn ripple_lost_slope(pixel: f32) -> f32 {
    var lost = 0.0;
    for (var i = 0u; i < RIPPLE_OCTAVES; i++) {
        let o = ripple_octave(i);
        let s = o.y * 1.6;
        lost += s * s * (1.0 - ripple_fade(o.x, pixel));
    }
    return lost;
}

// ---------------------------------------------------------------- the scene below

fn sea_uv(p: vec3<f32>) -> vec3<f32> {
    let c = globals.view_proj * vec4<f32>(p, 1.0);
    let ndc = c.xyz / c.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
}

fn sea_depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(sea_depth));
    let px = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(sea_depth, px, 0);
}

// World position of the scene at `uv`, given its depth. Reversed-Z: 0 is the
// cleared far plane, which the caller treats as open ocean.
fn sea_world(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, max(depth, 0.0000001), 1.0);
    return h.xyz / h.w;
}

fn sea_scene(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(sea_under, sea_sampler, uv, 0.0).rgb;
}

// ---------------------------------------------------------------- light

fn sky_along(r: vec3<f32>) -> vec3<f32> {
    let d = normalize(r);
    // A little under the drawn sky: the water's own tint and the eye's
    // adaptation keep a lake from turning to mirror-silver.
    var sky = sky_radiance(d) * 0.75;
    // The real weather overhead, where the reflected ray would meet the cloud layer.
    if d.z > 0.02 {
        let eye = globals.camera.xyz;
        let mid = cloud_floor(eye.xy) + mix(atmos.layer.x, atmos.layer.y, 0.4);
        let at = eye.xy + d.xy * max(mid - eye.z, 0.0) / d.z;
        let w = weather_at(at);
        let cover = smoothstep(0.1, 0.6, w.x - (grad_noise2(at - atmos.wind.xy, 400.0) - 0.5) * 0.3);
        let cloud = mix(atmos.sun_color.rgb * 0.34 + atmos.sky_color.rgb * 0.7, atmos.sky_color.rgb * 0.45, w.y);
        sky = mix(sky, cloud, cover * smoothstep(0.02, 0.2, d.z));
    }
    let sun = pow(max(dot(d, globals.sun.xyz), 0.0), 520.0);
    sky += atmos.sun_color.rgb * sun * 1.3;
    return sky;
}

// March the reflected ray across the heightfield so cliffs and beaches come
// back where the screen has no picture of them. Far water and rays that dive
// skip the walk.
fn shore_reflect(origin: vec3<f32>, r: vec3<f32>, dist: f32) -> vec3<f32> {
    var col = sky_along(r);
    if dist > 3500.0 || r.z < 0.0 {
        return col;
    }
    let size = globals.map.xy;
    var t = 8.0;
    for (var i = 0; i < 4; i++) {
        let p = origin + r * t;
        if p.x < 0.0 || p.y < 0.0 || p.x > size.x || p.y > size.y {
            break;
        }
        let h = terrain_height(p.xy);
        if h > p.z + 0.3 {
            let n = terrain_normal(p.xy, 16.0);
            let alt = h - globals.map.z;
            var albedo = mix(vec3<f32>(0.07, 0.15, 0.045), vec3<f32>(0.28, 0.24, 0.16), clamp((1.0 - n.z) * 3.0, 0.0, 1.0));
            albedo = mix(albedo, vec3<f32>(0.46, 0.40, 0.28), clamp((4.0 - alt) / 7.0, 0.0, 1.0));
            let land = albedo * (0.3 + 0.9 * max(dot(n, globals.sun.xyz), 0.0));
            col = mix(col, land, (1.0 - clamp(t / 160.0, 0.0, 1.0)) * 0.9);
            break;
        }
        t += 14.0 + f32(i) * 18.0;
    }
    return col;
}

// Screen-space reflection: walk the reflected ray over the copied scene and
// take the first thing standing above the water that it passes behind.
// xyz colour, w how sure (0 = nothing found, use the fallback).
fn screen_reflect(origin: vec3<f32>, r: vec3<f32>, dist: f32, jitter: f32) -> vec4<f32> {
    if r.z <= 0.0 || dist > 2400.0 {
        return vec4<f32>(0.0);
    }
    let eye = globals.camera.xyz;
    let water = globals.map.z;
    let reach = clamp(dist * 0.35, 30.0, 260.0);
    let steps = 20;
    var prev_t = 0.0;
    for (var i = 1; i <= steps; i++) {
        let f = (f32(i) - jitter) / f32(steps);
        let t = reach * f * f + 0.3;
        let p = origin + r * t;
        let s = sea_uv(p);
        if s.x <= 0.0 || s.x >= 1.0 || s.y <= 0.0 || s.y >= 1.0 || s.z <= 0.0 {
            break;
        }
        let d = sea_depth_at(s.xy);
        if d > s.z {
            // The scene is nearer than the ray here: the ray passed behind it.
            let hit = sea_world(s.xy, d);
            let gap = distance(eye, p) - distance(eye, hit);
            let tolerance = (t - prev_t) * 1.5 + 0.6 + dist * 0.004;
            if gap < tolerance && hit.z > water - 0.2 {
                // Refine between the last two samples.
                var lo = prev_t;
                var hi = t;
                for (var k = 0; k < 4; k++) {
                    let mid = (lo + hi) * 0.5;
                    let m = sea_uv(origin + r * mid);
                    if sea_depth_at(m.xy) > m.z {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let q = sea_uv(origin + r * hi);
                let edge = min(min(q.x, 1.0 - q.x), min(q.y, 1.0 - q.y));
                let sure = smoothstep(0.0, 0.08, edge) * (1.0 - smoothstep(0.6, 1.0, hi / reach));
                return vec4<f32>(sea_scene(q.xy), sure);
            }
            if gap >= tolerance {
                // Passed behind something much nearer the camera: occluded, not a reflection.
                break;
            }
        }
        prev_t = t;
    }
    return vec4<f32>(0.0);
}

// Light focused by the surface, crawling on whatever lies under it.
fn caustics(p: vec2<f32>, time: f32, pixel: f32) -> f32 {
    let visible = smoothstep(0.15, 0.6, 1.0 / max(pixel, 0.001));
    if visible <= 0.0 {
        return 0.0;
    }
    let warp = vec2<f32>(
        grad_noise2(p + vec2<f32>(time * 0.7, time * 0.4), 9.0),
        grad_noise2(p + vec2<f32>(41.0 - time * 0.5, 17.0 + time * 0.6), 9.0),
    ) - 0.5;
    let q = p + warp * 5.0;
    let a = grad_noise2(q + vec2<f32>(time * 1.1, time * 0.5), 3.4);
    let b = grad_noise2(q.yx + vec2<f32>(-time * 0.8, time * 0.9) + 23.0, 2.6);
    let fine = pow(1.0 - abs(a - b), 9.0);
    let c = grad_noise2(q + vec2<f32>(-time * 0.6, time * 0.4) + 71.0, 7.5);
    let d = grad_noise2(q.yx + vec2<f32>(time * 0.5, -time * 0.7) + 5.0, 6.1);
    let broad = pow(1.0 - abs(c - d), 7.0);
    return (fine * 0.75 + broad * 0.5) * visible;
}

// Bubbly foam texture, 0-1; `cover` 0-1 is how much of the pixel it should fill.
fn foam_lace(p: vec2<f32>, time: f32, pixel: f32, cover: f32) -> f32 {
    if cover <= 0.001 {
        return 0.0;
    }
    let big = grad_noise2(p + vec2<f32>(time * 0.35, -time * 0.22), 4.8);
    let mid = grad_noise2(p * 1.3 + vec2<f32>(-time * 0.3, time * 0.25) + 37.0, 1.7);
    let fine = grad_noise2(p * 1.7 + vec2<f32>(time * 0.2, time * 0.4) + 91.0, 0.55);
    let fine_w = smoothstep(0.6, 2.0, 0.55 / max(pixel, 0.001));
    let mid_w = smoothstep(0.6, 2.0, 1.7 / max(pixel, 0.001));
    let pattern = big * 0.5 + mix(0.5, mid, mid_w) * 0.32 + mix(0.5, fine, fine_w) * 0.18;
    // Where the texture is too fine to see, fade to its average instead of flickering.
    let soft = mix(0.35, 0.06, fine_w);
    let edge = 1.0 - cover;
    return smoothstep(edge - soft, edge + soft, pattern) * smoothstep(0.0, 0.15, cover);
}

fn hash_pixel(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

// ---------------------------------------------------------------- what happens on the water

// A ring spreading from a splash or blast (renderer/water_fx.rs).
struct SeaRipple {
    // xy the centre; z where its flash is: over the water, or under it for a torpedo.
    pos: vec3<f32>,
    start: f32,
    // x size in metres, y life in seconds, z flash (negative: an energy blast's blue), w foam.
    params: vec4<f32>,
}

// A hull or a torpedo moving through the water.
struct SeaWake {
    // xy where it is this frame, zw its heading.
    at: vec4<f32>,
    // x speed m/s, y half length, z half beam, w kind (0 a hull on the surface, 1 dived, 2 a torpedo).
    shape: vec4<f32>,
    // A circle round all it touches (xy, radius), then how strong it is.
    bound: vec4<f32>,
    // Bow, stern, then the path the stern took, newest first (a torpedo: its head,
    // then the end of its line): xy, the speed it had there, the water's age in seconds.
    trail: array<vec4<f32>, 8>,
}

struct SeaFxList {
    // x rings, y wakes.
    counts: vec4<u32>,
    ripples: array<SeaRipple, 64>,
    wakes: array<SeaWake, 48>,
}

@group(1) @binding(0) var<storage, read> sea_fx: SeaFxList;

struct SeaStir {
    // Added to the surface's height gradient.
    slope: vec2<f32>,
    foam: f32,
    // Slope variance the pixel is too coarse to show: it widens highlights.
    rough: f32,
}

// Rings, foam and wakes at `xy`. Each entry is a circle test away from being
// skipped, and the lists are short and sorted nearest the camera.
fn sea_stir(xy: vec2<f32>, time: f32, pixel: f32) -> SeaStir {
    var out: SeaStir;
    out.slope = vec2<f32>(0.0);
    out.foam = 0.0;
    out.rough = 0.0;
    let rings = min(sea_fx.counts.x, 64u);
    for (var i = 0u; i < rings; i++) {
        let e = sea_fx.ripples[i];
        let t = time - e.start;
        let size = e.params.x;
        let life = e.params.y;
        if t < 0.0 || t > life {
            continue;
        }
        let d = xy - e.pos.xy;
        // A few crests running out, longer and slower-dying for a bigger blast;
        // the train stretches as it goes, as real ripples disperse.
        let speed = 3.5 + sqrt(size) * 2.0;
        let lambda = clamp(0.7 + size * 0.3, 0.9, 7.0) * (1.0 + 0.3 * t);
        let front = size * 0.25 + speed * t;
        let reach = front + lambda * 2.0;
        if dot(d, d) > reach * reach {
            continue;
        }
        let r = length(d);
        let x = r - front;
        let spread = size * (0.4 + 0.3 * sqrt(t));
        // Well inside the train the water has calmed: only the foam is left there.
        let waves = x > -lambda * 4.0;
        if waves {
            let k = 6.283185 / lambda;
            let env = select(exp(-x * x / (lambda * lambda * 0.36)), exp(-x * x / (lambda * lambda * 6.0)), x < 0.0);
            let amp = size * 0.012 * exp(-t * 3.0 / life) * inverseSqrt(1.0 + r / size) * env;
            let shown = smoothstep(1.2, 4.0, lambda / max(pixel, 0.001));
            let dir = d / max(r, 0.001);
            out.slope += dir * (amp * k * cos(k * x) * shown);
            out.rough += amp * k * amp * k * (1.0 - shown) * 0.5;
        }
        let foam = e.params.w;
        if foam > 0.0 && (r < spread * 1.3 || (waves && t < 1.8)) {
            // White water where it came down, spreading and thinning; a rim on the first crest early on.
            let broken = grad_noise2(xy * 1.3 + e.pos.xy, max(size * 0.25, 1.0)) - 0.5;
            // Never solid: the lace has room to open holes in it, and the noise tears its edge.
            let centre = (1.0 - smoothstep(0.2, 1.0, r / spread + broken * 0.9))
                * (1.0 - smoothstep(0.15, 1.0, t / life)) * 0.72;
            let rim = exp(-x * x / (lambda * lambda * 0.2)) * (1.0 - smoothstep(0.2, 1.8, t)) * 0.7;
            out.foam = max(out.foam, max(centre, rim) * foam);
            // Churned in the middle while it boils.
            let churn = (1.0 - smoothstep(0.3, 1.0, r / spread)) * exp(-t * 1.5) * foam;
            out.rough += churn * 0.04;
        }
    }
    // Wakes are laid down along the path each hull took: the arms and the white
    // water spread out and fade where the water was stirred, so a wake grows out
    // from the stern as a hull gets going and is left lying when it stops.
    var churn = 0.0;
    let wakes = min(sea_fx.counts.y, 48u);
    for (var i = 0u; i < wakes; i++) {
        // Read field by field: copying the whole record would put its path in
        // local memory, which the loop below then indexes.
        let bound = sea_fx.wakes[i].bound;
        let rel = xy - bound.xy;
        if dot(rel, rel) > bound.z * bound.z {
            continue;
        }
        let shape = sea_fx.wakes[i].shape;
        let at = sea_fx.wakes[i].at;
        let speed = shape.x;
        let half_length = shape.y;
        let half_beam = shape.z;
        let kind = shape.w;
        let strength = bound.w;
        let torpedo = kind > 1.5;
        let fwd = at.zw;
        if kind < 0.5 {
            // White water pushed up round the stem and along the fore part of the
            // hull: it rides with the hull, as strong as it is going fast.
            let right = vec2<f32>(fwd.y, -fwd.x);
            let local = xy - at.xy;
            let b = half_length - dot(local, fwd);
            let side = abs(dot(local, right));
            let hug = side - half_beam * (0.35 + clamp(b / half_length, 0.0, 1.0) * 0.85);
            let hug_w = 0.6 + half_beam * 0.3;
            let bow = exp(-hug * hug / (hug_w * hug_w)) * (1.0 - smoothstep(0.0, half_length * 1.3, b))
                * smoothstep(-half_beam * 1.5, 0.0, b) * smoothstep(1.0, 8.0, speed);
            out.foam = max(out.foam, bow * 0.9 * strength);
        }
        // Point 0 is the bow and point 1 the stern: the arms start at the stem,
        // the churned water behind the stern.
        let life = select(9.0, 4.0, torpedo);
        var c = sea_fx.wakes[i].trail[0];
        for (var j = 0u; j < 7u; j++) {
            let a = c;
            c = sea_fx.wakes[i].trail[j + 1u];
            // A stretch made standing still, or padding past the end of the path.
            if a.z <= 0.0 && c.z <= 0.0 {
                continue;
            }
            let ab = c.xy - a.xy;
            let u = clamp(dot(xy - a.xy, ab) / max(dot(ab, ab), 0.0001), 0.0, 1.0);
            let off = xy - a.xy - ab * u;
            let d = length(off);
            let age = mix(a.w, c.w, u);
            let spd = mix(a.z, c.z, u);
            if torpedo {
                // A faint line of its air coming up behind it (none yet where the
                // age is still negative), spreading and breaking into specks as it goes.
                let width = 0.35 + max(age, 0.0) * 0.35;
                let specks = smoothstep(0.35, 0.75, grad_noise2(xy + vec2<f32>(time * 0.25, 0.0), 1.6));
                let s = exp(-d * d / (width * width)) * (1.0 - smoothstep(life * 0.3, life, age))
                    * smoothstep(0.0, 0.5, age) * smoothstep(1.0, 7.0, spd) * mix(0.35, 1.0, specks);
                churn = max(churn, s * strength);
                continue;
            }
            // The Kelvin arms: each stretch of the path sends a crest out to
            // either side, narrower behind a fast boat, dying as it spreads.
            let open = mix(0.34, 0.2, smoothstep(14.0, 40.0, spd));
            let arm = d - (half_beam * 0.7 + age * spd * open);
            let arm_w = 0.5 + age * 1.1 + half_beam * 0.2;
            if abs(arm) < arm_w * 3.0 {
                let env = exp(-arm * arm / (arm_w * arm_w)) * (1.0 - smoothstep(0.0, 4.5, age))
                    * smoothstep(2.0, 10.0, spd) * strength;
                let lambda = clamp(spd * 0.18, 1.2, 6.0);
                let k = 6.283185 / lambda;
                // Short crests feathering along the arm, fixed where they were made.
                let along = dot(xy - a.xy, normalize(ab + vec2<f32>(1e-5, 0.0)));
                let phase = k * (along * 0.8 - d * 1.2);
                let amp = (0.05 + half_beam * 0.03) * min(spd / 12.0, 2.0) * env;
                let shown = smoothstep(1.2, 4.0, lambda / max(pixel, 0.001));
                out.slope += (off / max(d, 0.001)) * (amp * k * cos(phase) * shown);
                out.rough += amp * k * amp * k * (1.0 - shown);
                let breaking = mix(0.25, 0.8, smoothstep(12.0, 40.0, spd));
                out.foam = max(out.foam, env * breaking * (1.0 - smoothstep(0.3, 2.5, age)));
            }
            if j == 0u {
                continue;
            }
            // Churned white water behind the stern, widening as it goes.
            let width = half_beam * 1.1 + age * 1.2;
            if d > width * 3.0 {
                continue;
            }
            let s = exp(-d * d / (width * width)) * (1.0 - smoothstep(life * 0.1, life, age))
                * smoothstep(1.0, 7.0, spd);
            churn = max(churn, s * strength * 0.85);
        }
    }
    if churn > 0.01 {
        // Broken up along its length: foam comes up in patches, not as a painted line.
        let patchy = 0.55 + 0.45 * grad_noise2(xy + vec2<f32>(time * 0.3, 0.0), 6.0);
        out.foam = max(out.foam, churn * patchy);
        out.rough += churn * 0.03;
    }
    return out;
}

// Light a blast throws on the water round it for a moment: a glint off the
// rippled surface toward the eye, and for one under the water a glow seen
// through it, absorbed with the path. It falls off softly and the ripples break
// it up, so it never reads as a disc.
fn sea_flash(world: vec3<f32>, n: vec3<f32>, v: vec3<f32>, rough: f32, fresnel: f32, time: f32, pixel: f32) -> vec3<f32> {
    var light = vec3<f32>(0.0);
    let water = globals.map.z;
    let rings = min(sea_fx.counts.x, 64u);
    for (var i = 0u; i < rings; i++) {
        let e = sea_fx.ripples[i];
        let flash = e.params.z;
        let t = time - e.start;
        if flash == 0.0 || t < 0.0 || t > 0.6 {
            continue;
        }
        let under = e.pos.z < water;
        let pulse = abs(flash) * smoothstep(0.0, 0.02, t) * exp(-t / 0.08);
        var tint = select(vec3<f32>(1.0, 0.45, 0.13), vec3<f32>(0.3, 0.65, 1.0), flash < 0.0);
        if under {
            // Seen through a few metres of water: whiter, the red a little lost.
            tint = select(vec3<f32>(1.0, 0.7, 0.36), vec3<f32>(0.45, 0.75, 1.0), flash < 0.0);
        }
        let to = e.pos - world;
        let d2 = dot(to, to);
        let reach = e.params.x * select(1.3, 0.45, under) + select(3.0, 2.0, under);
        // Under the water the glow is held in close; over it, the glint reaches further out.
        let near = select(reach * reach / (d2 + reach * reach), exp(-d2 / (reach * reach)), under);
        if near * pulse < 0.004 {
            continue;
        }
        let l = to * inverseSqrt(max(d2, 0.0001));
        if !under {
            let h = normalize(v + l);
            let n_dot_h = max(dot(n, h), 0.0);
            let a2 = max(rough * rough, 0.01);
            let dd = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
            let glint = min(a2 / (PI * dd * dd) * max(dot(n, l), 0.0) * 0.05, 3.0);
            light += tint * pulse * near * (glint * fresnel * 4.0 + 0.03);
        } else {
            // Through the water: redder light is lost first, and the surface
            // bends it into a web of bright lines, as it does the sun on the seabed.
            let through = exp(-vec3<f32>(0.34, 0.075, 0.058) * sqrt(d2) * 0.1);
            let web = caustics(world.xy * 0.7 + e.pos.yx, time * 2.0, pixel * 0.7);
            let bunch = (0.25 + 1.6 * web) * (0.7 + 0.6 * clamp(0.5 + dot(n.xy, -l.xy) * 6.0, 0.0, 1.0));
            light += tint * pulse * near * through * bunch * (1.0 - fresnel) * 0.45;
        }
    }
    return light;
}

// ---------------------------------------------------------------- the surface

@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    let xy = in.world.xy;
    let world = in.world;
    let water = globals.map.z;
    let time = globals.camera.w;
    let eye = globals.camera.xyz;
    let v = normalize(eye - world);
    let dist = distance(eye, world);
    let uv = in.clip.xy / globals.scene.xy;

    // Footprint of this pixel on the water, in metres.
    let pixel = max(length(dpdx(xy)), length(dpdy(xy)));
    // Bathymetry from the height field: it moves the surf and calms the
    // shallows the same whatever is drawn over the seabed.
    let depth = water_depth(xy);
    let depth_aa = max(fwidth(depth), 0.06);
    if depth <= 0.02 {
        discard;
    }

    // What is behind this pixel if there were no water.
    let behind_d = sea_depth_at(uv);
    let open = behind_d <= 0.0000002;
    let behind = sea_world(uv, behind_d);

    // Surface normal. The swell calms toward the shore; the ripples keep on.
    let amp = smoothstep(0.2, 5.0, depth);
    let calm = 1.0 - smoothstep(1.0, 12.0, depth);
    let sw = swell(xy, time, amp, pixel);
    let e = max(pixel * 0.75, 0.04);
    let h0 = ripple_height(xy, time, pixel, calm);
    let hx = ripple_height(xy + vec2<f32>(e, 0.0), time, pixel, calm);
    let hy = ripple_height(xy + vec2<f32>(0.0, e), time, pixel, calm);
    let ripple_slope = vec2<f32>(hx - h0, hy - h0) / e;
    // Rings, wakes and foam from what is happening on the water.
    var stir: SeaStir;
    if dist < 6000.0 {
        stir = sea_stir(xy, time, pixel);
    }
    let n = normalize(vec3<f32>(-ripple_slope - sw.slope * 0.12 - stir.slope, 1.0));
    let n_dot_v = clamp(dot(n, v), 0.0001, 1.0);

    // Schlick with the water's 2% at normal incidence.
    let fresnel = 0.02 + 0.98 * pow(1.0 - n_dot_v, 5.0);
    let shadow = sun_shadow(world, vec3<f32>(0.0, 0.0, 1.0));

    // ---- refraction
    // Bend the view by the surface slope, more for a thicker column of water
    // behind, and never onto something standing in front of the water.
    var column = 60.0;
    if !open {
        column = distance(world, behind);
    }
    let bend_m = n.xy * min(column, 6.0) * 0.55;
    let bent_clip = globals.view_proj * vec4<f32>(world + vec3<f32>(bend_m, 0.0), 1.0);
    var ruv = vec2<f32>(bent_clip.x / bent_clip.w * 0.5 + 0.5, 0.5 - bent_clip.y / bent_clip.w * 0.5);
    var rd = sea_depth_at(ruv);
    var under = sea_world(ruv, rd);
    if rd > in.clip.z || (rd > 0.0000002 && under.z > water + 0.05) {
        ruv = uv;
        rd = behind_d;
        under = behind;
    }
    let seen_open = rd <= 0.0000002;
    // Path of light through the water: down from the surface to what is lit
    // under it, then back up to the eye.
    var view_path = 80.0;
    var sink = 40.0;
    // 1 where what is under the water stands off the seabed: a hull, not the ground.
    var hull = 0.0;
    if !seen_open {
        view_path = max(distance(world, under), 0.0);
        sink = max(water - under.z, 0.0);
        hull = smoothstep(0.5, 1.5, under.z - terrain_height(under.xy)) * smoothstep(0.1, 0.8, sink);
    }
    // Long paths count for less than they would: a stylised clarity, so the
    // bottom of deep water (a wreck field, a commander on the seabed) still
    // shows faintly instead of drowning in the water's own blue.
    let raw_path = view_path + sink * 0.9;
    let path = select(raw_path / (1.0 + raw_path / 55.0), raw_path, seen_open);
    // Clear, lightly green coastal water: red is gone in a few metres, and the
    // seabed reads through a dozen metres or so of it.
    let absorb = vec3<f32>(0.17, 0.032, 0.025);
    let through = exp(-absorb * path);
    // Light the water scatters back to the eye, lit by the sun, dimmer in shadow.
    let sun_in = max(globals.sun.z, 0.0);
    let lit = mix(0.45, 1.0, shadow) * (0.55 + 0.45 * sun_in);
    let deep_scatter = vec3<f32>(0.0045, 0.020, 0.036) * lit;
    let shallow_scatter = vec3<f32>(0.010, 0.050, 0.052) * lit;
    // Over a hull the water keeps the colour of the depth it stands in, so a
    // dived boat does not show as a patch of shallow-water green.
    let scatter = mix(shallow_scatter, deep_scatter, smoothstep(2.0, 30.0, mix(sink, max(sink, depth), hull)));
    var below = vec3<f32>(0.0);
    if !seen_open {
        below = sea_scene(ruv);
        // A hull under the water (a dived boat, a ship's keel, a wreck, a
        // commander walking the bottom) was lit by the full sun in the copy.
        // Under the surface it gets only what light comes down: dimmer than the
        // open seabed and dimmer with depth, but its shape and detail still
        // read through clear water. The seabed keeps its light and caustics.
        let gloom = mix(0.6, 0.42, smoothstep(2.0, 30.0, sink));
        below = mix(below, below * gloom, hull);
        // Caustics are sunlight: brighter shallow, gone in shadow and at depth.
        let focus = caustics(under.xy, time, pixel) * exp(-sink * 0.16) * (1.0 - hull);
        below *= 1.0 + focus * 2.4 * shadow * smoothstep(0.05, 0.6, sink);
    }
    // The copied scene is already fogged; only what the water adds is fogged here.
    let seen_through = below * through * (1.0 - fresnel);

    // ---- reflection
    let r = reflect(-v, n);
    var reflected = shore_reflect(world, r, dist);
    let ssr = screen_reflect(world, r, dist, hash_pixel(in.clip.xy));
    reflected = mix(reflected, ssr.rgb, ssr.a);

    // Sun: a sharp GGX highlight, widened by the ripples this pixel cannot show.
    let l = globals.sun.xyz;
    let h = normalize(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let rough = sqrt(0.004 + ripple_lost_slope(pixel) * 0.5 + stir.rough);
    let a2 = rough * rough;
    let n_dot_h = max(dot(n, h), 0.0);
    let dd = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    let ggx = a2 / (PI * dd * dd);
    let f_sun = 0.02 + 0.98 * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    let spec = min(ggx * f_sun * n_dot_l / (4.0 * n_dot_v * max(n_dot_l, 0.05) + 0.001), 60.0);
    let sun_color = vec3<f32>(1.0, 0.95, 0.85) * 2.7;

    var color = scatter * (vec3<f32>(1.0) - through) * (1.0 - fresnel)
        + reflected * fresnel + sun_color * spec * shadow;

    // ---- foam
    // Surf: bands that roll in over the real bathymetry and break on the beach.
    let breakup = grad_noise2(xy + vec2<f32>(time * 1.3, time * 0.6), max(9.0, pixel * 2.5));
    let wash_phase = depth * 1.35 - time * 1.1 + breakup * 3.5;
    let roll = pow(0.5 + 0.5 * sin(wash_phase), 6.0) * (1.0 - smoothstep(0.6, 4.5, depth));
    let edge = 1.0 - smoothstep(0.0, max(0.35, min(depth_aa * 1.1, 7.0)), depth);
    var cover = clamp(edge * 0.8 + roll * 0.6, 0.0, 1.0);
    // Where hulls, piles and foundations stand in the water: the water surface
    // is close to something that pierces it.
    if !open && dist < 1600.0 {
        let near = max(water - behind.z, 0.0);
        // Only for something standing off the ground: the seabed has its own surf.
        let standing = step(terrain_height(behind.xy) + 0.8, behind.z);
        cover = max(cover, (1.0 - smoothstep(0.0, 0.9, near + view_path * 0.15)) * step(behind.z, water + 0.05) * standing);
        let reach_px = clamp(1.4 / max(pixel, 0.001), 2.0, 14.0);
        var ring = 0.0;
        for (var k = 0; k < 8; k++) {
            let a = f32(k) * 0.785398 + hash_pixel(in.clip.xy + 7.0) * 0.785;
            let o = vec2<f32>(cos(a), sin(a)) * reach_px / globals.scene.xy;
            let d = sea_depth_at(uv + o);
            if d > in.clip.z {
                // Something in front of the water here: does it stand in it, near this point?
                let p = sea_world(uv + o, d);
                let across = length(p.xy - xy);
                // A hull, not the ground: beaches and terrain-mesh LOD error are not rings.
                if p.z < water + 2.5 && across < 2.6 && p.z > terrain_height(p.xy) + 0.8 {
                    ring = max(ring, 1.0 - across / 2.6);
                }
            }
        }
        cover = max(cover, ring * 0.9);
    }
    cover = max(cover, stir.foam);
    // Whitecaps on the steepest crests of the open sea.
    let gusts = smoothstep(0.62, 0.85, grad_noise2(xy + vec2<f32>(time * 4.0, time * 1.5), 190.0));
    cover = max(cover, smoothstep(0.6, 0.97, sw.peak) * amp * 0.28 * gusts * (1.0 - calm));
    let foam = foam_lace(xy, time, pixel, cover * 0.8);
    let foam_color = vec3<f32>(0.80, 0.86, 0.88) * (0.35 + 0.65 * shadow) * (0.55 + 0.45 * sun_in);
    color = mix(color, foam_color, foam * 0.92);
    if dist < 6000.0 {
        color += sea_flash(world, n, v, rough, fresnel, time, pixel) * (1.0 - foam * 0.6);
    }
    // Lamps, fires and blasts (lights.rs): long glints across the ripples, and
    // their light on foam and murky shallows.
    var lit_water: Pbr;
    lit_water.albedo = mix(vec3<f32>(0.018, 0.03, 0.034), vec3<f32>(0.7, 0.75, 0.78), foam);
    lit_water.metallic = 0.0;
    lit_water.roughness = max(rough, 0.14);
    lit_water.emissive = vec3<f32>(0.0);
    color += min(local_lights(lit_water, world, n, v), vec3<f32>(24.0));
    if (globals.counts.w & 2u) != 0u {
        // The build grid while a structure is being placed, on the surface where it would stand.
        color = build_grid_overlay(color, xy, dist);
    }
    color = apply_fog_of_war(color, xy) + seen_through * (1.0 - foam * 0.92);
    color = apply_haze(color, world, eye);
    // Shore pixels blend out through the height-field edge, so the coastline
    // stays soft where the terrain mesh and height field disagree.
    let alpha = smoothstep(0.015, 0.12, depth);
    return vec4<f32>(color, alpha);
}

fn water_depth(xy: vec2<f32>) -> f32 {
    let size = globals.map.xy;
    if xy.x < 0.0 || xy.y < 0.0 || xy.x >= size.x || xy.y >= size.y {
        // Continue the edge bathymetry beyond the playable map; a constant
        // offshore depth draws a visible rectangular colour seam from orbit.
        let edge = clamp(xy, vec2<f32>(0.0), size - vec2<f32>(1.0));
        return max(globals.map.z - terrain_height(edge), 0.03) + distance(xy, edge) * 0.035;
    }
    return globals.map.z - terrain_height(xy);
}
