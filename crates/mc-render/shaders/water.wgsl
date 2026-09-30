//!use bindings
//!use shore
// The sea: one oversized quad on the water plane, drawn after the opaque
// scene has been copied (`Renderer::refract`). Everything is in the fragment
// shader:
//
// - The surface is a sum of wave trains, each running at its own speed, from a
//   150 m swell to half-metre chop, differentiated at the pixel's footprint so
//   waves too fine for it filter out; what filters out widens the sun's
//   highlight instead. Whitecaps break where the trains pile up in the gusts.
// - What lies under the water (seabed, the drowned part of a hull) is the
//   copied scene, bent by the surface and absorbed along the real path the
//   light takes through the water, red first. Shallows turn turquoise over
//   sand and deep water is only the colour light scatters back.
// - Caustics are drawn on that seabed, at its real position.
// - Reflections march the copied scene (hulls, cliffs) and fall back to a
//   height-field walk of the coast and then the sky.
// - The shore (shore.wgsl): breakers that roll in over the real bathymetry,
//   break and run in as white water; the wash up the sand is the terrain's.
// - Foam rings where hulls and structures stand in the water.
// - What happens on the water (renderer/water_fx.rs, `sea_fx`): rings spreading
//   from splashes and blasts with their foam, the flash of a blast caught by the
//   water round it, and the wakes of moving hulls laid along the path they took.
// - With the eye under the water the quad becomes the whole screen and draws
//   the view from inside the sea (`under_sea`).
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
    var out: WaterOut;
    if globals.camera.z < globals.map.z {
        // The eye is under the water: the whole screen, in front of everything
        // (reversed-Z: 1 is the near plane), for `under_sea`.
        out.world = vec3<f32>(0.0);
        out.clip = vec4<f32>(c.x * 2.0 - 1.0, c.y * 2.0 - 1.0, 1.0, 1.0);
        return out;
    }
    let xy = (c - 0.5) * globals.map.xy * 2.3 + globals.map.xy * 0.5;
    out.world = vec3<f32>(xy, globals.map.z);
    out.clip = globals.view_proj * vec4<f32>(out.world, 1.0);
    return out;
}

// ---------------------------------------------------------------- waves

// Downwind, as the longest trains run.
const SEA_WIND: vec2<f32> = vec2<f32>(0.91, 0.41);

// The sea is a sum of wave trains from a 150 m swell down to half-metre chop,
// each running at its own speed (deep water: sqrt(g k)), so crests form, pass
// through each other and break up as real water does instead of a pattern
// sliding over it. Headings fan out round the wind, the short ones most.
const SEA_TRAINS: u32 = 20u;

struct SeaWaves {
    slope: vec2<f32>,
    // The mid-scale waves' height here in units of their spread: past 2 on the steepest crests.
    crest: f32,
    // The long swell's height, -1 in a trough to 1 on a crest (for the view from far off).
    swell: f32,
    // Slope variance of the trains the pixel is too coarse to show.
    lost: f32,
}

// `long` scales the swell (calm in the shallows), `chop` the short waves (gusts).
fn sea_waves(xy: vec2<f32>, time: f32, pixel: f32, long: f32, chop: f32) -> SeaWaves {
    var out: SeaWaves;
    out.slope = vec2<f32>(0.0);
    out.lost = 0.0;
    var crest = 0.0;
    var crest_var = 0.0;
    var swell = 0.0;
    var swell_w = 0.0;
    let wind = atan2(SEA_WIND.y, SEA_WIND.x);
    // Four broad fields drifting downwind. Each train takes its strength and a
    // bend in its crests from its own blend of them: endless plane waves summed
    // tile into a lattice the eye picks out, where real trains come and go in
    // patches and their crests wander.
    let drift = SEA_WIND * time * 1.5;
    let fields = vec4<f32>(
        grad_noise2(xy - drift, 910.0),
        grad_noise2(xy.yx + vec2<f32>(311.0, -127.0) - drift * 0.7, 530.0),
        grad_noise2(xy + vec2<f32>(-743.0, 219.0) - drift * 1.3, 290.0),
        grad_noise2(xy.yx + vec2<f32>(97.0, 613.0) - drift * 0.5, 170.0),
    ) - 0.5;
    for (var i = 0u; i < SEA_TRAINS; i++) {
        let fi = f32(i);
        let lambda = 150.0 * pow(0.735, fi);
        let k = 6.283185 / lambda;
        // Steepness (k times height): gentle swell, steeper wind waves.
        let steep = mix(0.03, 0.075, smoothstep(1.0, 9.0, fi)) * select(1.0, long, i < 6u) * select(1.0, chop, i > 11u);
        let fade = smoothstep(1.5, 5.0, lambda / max(pixel, 0.001));
        out.lost += steep * steep * 0.3 * (1.0 - fade);
        if fade <= 0.0 {
            continue;
        }
        let spread = mix(0.8, 2.3, fi / f32(SEA_TRAINS - 1u));
        // This train's blend of the fields: one for its strength, another for its bend.
        let mix_a = vec4<f32>(sin(fi * 1.7), cos(fi * 2.3), sin(fi * 0.9 + 1.0), cos(fi * 3.1 + 2.0));
        let mix_b = vec4<f32>(cos(fi * 1.3 + 0.5), sin(fi * 2.9), cos(fi * 0.7 + 2.0), sin(fi * 1.9 + 1.0));
        let lot = clamp(0.5 + dot(fields, mix_a) * 1.6, 0.0, 1.0);
        let strength = mix(0.2, 1.55, lot * lot * (3.0 - 2.0 * lot));
        let bend = dot(fields, mix_b) * 2.2;
        let a = wind + (fract(fi * 0.618034 + 0.13) * 2.0 - 1.0) * spread;
        let dir = vec2<f32>(cos(a), sin(a));
        // In cycles, folded before the multiply so kilometres of map and hours of
        // play keep the phase precise.
        let along = dot(dir, xy) / lambda;
        let cycles = sqrt(9.81 * k) / 6.283185 * time;
        let phase = 6.283185 * (fract(along) - fract(cycles) + fract(fi * 0.3713)) + bend * 6.283185;
        let s = sin(phase);
        // exp(sin - 1): sharp crests, broad troughs, as wind waves stand.
        let e = exp(s - 1.0);
        let amp = steep / k * fade * strength;
        out.slope += dir * (amp * k * e * cos(phase));
        if lambda > 3.0 && lambda < 70.0 {
            crest += amp * (e - 0.466);
            crest_var += amp * amp * 0.0915;
        }
        if lambda >= 40.0 {
            swell += s * fade;
            swell_w += fade;
        }
    }
    out.crest = crest * inverseSqrt(max(crest_var, 1e-8));
    out.swell = swell / max(swell_w, 0.001);
    return out;
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
fn screen_reflect(origin: vec3<f32>, r: vec3<f32>, dist: f32) -> vec4<f32> {
    if r.z <= 0.0 || dist > 2400.0 {
        return vec4<f32>(0.0);
    }
    let eye = globals.camera.xyz;
    let water = globals.map.z;
    let reach = clamp(dist * 0.35, 30.0, 260.0);
    let steps = 20;
    var prev_t = 0.0;
    for (var i = 1; i <= steps; i++) {
        // Steps at fixed places: a per-pixel jitter with nothing to average it over
        // frames left the reflections grainy.
        let f = (f32(i) - 0.5) / f32(steps);
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

// ---------------------------------------------------------------- what happens on the water

// A ring spreading from a splash or blast (renderer/water_fx.rs).
struct SeaRipple {
    // xy the centre; z where its flash is: over the water, or under it for a torpedo.
    pos: vec3<f32>,
    start: f32,
    // x size in metres, y life in seconds, z flash (negative: an energy blast's blue),
    // w foam (negative: the flash's light only, no ring).
    params: vec4<f32>,
}

// A big gun's muzzle blast pressing the sea flat in a fan in front of it (renderer/water_fx.rs).
struct SeaBlast {
    // xy where the muzzles stand over the water, zw the way they fired.
    at: vec4<f32>,
    // x reach downrange in metres, y start, z life in seconds, w strength.
    params: vec4<f32>,
}

// A hull or a torpedo moving through the water.
struct SeaWake {
    // xy where it is this frame, zw its heading.
    at: vec4<f32>,
    // x speed m/s, y half length, z half beam, w kind (0 a hull on the surface, 1 dived,
    // 2 and up a torpedo: 2 plus its `mc_data::TorpedoLook`; y is then its line's life).
    shape: vec4<f32>,
    // A circle round all it touches (xy, radius), then how strong it is.
    bound: vec4<f32>,
    // Bow, stern, then the path the stern took, newest first (a torpedo: its head,
    // then its line back to the tubes in the first 8): xy, the speed it had there, the water's age in seconds.
    trail: array<vec4<f32>, 12>,
    // The path the bow took, newest first (the live bow, then where it was): xy,
    // the speed it had there, the water's age. The Kelvin arms spread from it, so a
    // turning hull leaves its wedge curving through the water behind it.
    arms: array<vec4<f32>, 12>,
}

struct SeaFxList {
    // x rings, y wakes, z muzzle blasts.
    counts: vec4<u32>,
    ripples: array<SeaRipple, 64>,
    wakes: array<SeaWake, 48>,
    blasts: array<SeaBlast, 16>,
}

@group(1) @binding(0) var<storage, read> sea_fx: SeaFxList;

struct SeaStir {
    // Added to the surface's height gradient.
    slope: vec2<f32>,
    foam: f32,
    // Slope variance the pixel is too coarse to show: it widens highlights.
    rough: f32,
    // Water full of air, churned by a hull: paler and lit from within.
    aerate: f32,
    // Water pressed flat by a muzzle blast: its ripples and swell are gone for a moment.
    flat: f32,
    // A Regency plasma torpedo's drive glowing red up through the water.
    ember: f32,
}

// Noise that fades to its mean as its cells get down to a few pixels, so a
// pattern never goes finer than the screen can show. `cell` must be the cell as
// sampled (no scaling of `p`), and must not change across the surface: with world
// coordinates in the thousands, a cell that drifts shifts the pattern's phase by
// hundreds of cells a metre and it aliases into stripes.
fn soft_noise(p: vec2<f32>, cell: f32, pixel: f32) -> f32 {
    let shown = smoothstep(1.5, 4.0, cell / max(pixel, 0.001));
    if shown <= 0.0 {
        return 0.5;
    }
    return mix(0.5, grad_noise2(p, cell), shown);
}

// Rings, foam and wakes at `xy`. Each entry is a circle test away from being
// skipped, and the lists are short and sorted nearest the camera.
fn sea_stir(xy: vec2<f32>, time: f32, pixel: f32) -> SeaStir {
    var out: SeaStir;
    out.slope = vec2<f32>(0.0);
    out.foam = 0.0;
    out.rough = 0.0;
    out.aerate = 0.0;
    out.flat = 0.0;
    out.ember = 0.0;
    let rings = min(sea_fx.counts.x, 64u);
    for (var i = 0u; i < rings; i++) {
        let e = sea_fx.ripples[i];
        let t = time - e.start;
        let size = e.params.x;
        let life = e.params.y;
        if t < 0.0 || t > life || e.params.w < 0.0 {
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
    // Muzzle blasts: the sea pressed flat and dark in a fan out in front of the guns,
    // its front running out fast as a band of ruffled water with a sheet of spray
    // breaking white along it, heaviest downrange. One soft front, no wave train.
    let blasts = min(sea_fx.counts.z, 16u);
    for (var i = 0u; i < blasts; i++) {
        let e = sea_fx.blasts[i];
        let t = time - e.params.y;
        let life = e.params.z;
        let reach = e.params.x;
        if t < 0.0 || t > life {
            continue;
        }
        let d = xy - e.at.xy;
        if dot(d, d) > reach * reach * 1.4 {
            continue;
        }
        let way = e.at.zw;
        let along = dot(d, way);
        let across = dot(d, vec2<f32>(-way.y, way.x));
        // The fan: reach downrange, a third of it behind the muzzles, widening as it goes.
        let long = select(reach * 0.15, reach, along > 0.0);
        let wide = reach * (0.2 + 0.75 * sqrt(clamp(along / reach, 0.0, 1.0)));
        // The edge wobbles: the blast does not press the sea out in a clean curve.
        let wobble = (soft_noise(xy + e.at.yx, max(reach * 0.25, 2.5), pixel) - 0.5) * 0.25;
        let rr = length(vec2<f32>(along / long, across / wide)) + wobble;
        let front = 1.0 - exp(-t * 6.0);
        let strength = e.params.w;
        // Pressed flat inside the front, easing back after a second.
        let flat = (1.0 - smoothstep(front - 0.2, front + 0.02, rr)) * (1.0 - smoothstep(0.35, 1.9, t)) * strength;
        out.flat = max(out.flat, clamp(flat, 0.0, 1.0));
        // The front: a band of ruffled water with the spray sheet breaking along it.
        let band = max(reach * (0.07 + 0.05 * t), pixel * 2.0) / max(mix(long, wide, 0.5), 1.0);
        let x = rr - front;
        let ring = exp(-x * x / (band * band));
        let fade = 1.0 - smoothstep(0.2, life, t);
        let r = length(d);
        let out_dir = d / max(r, 0.001);
        let band_m = band * mix(long, wide, 0.5);
        let rise = band_m * 0.14 * strength * (1.0 - smoothstep(0.3, 1.5, t));
        out.slope += out_dir * (-2.0 * x / (band * band)) * ring * rise / max(mix(long, wide, 0.5), 1.0);
        out.rough += ring * 0.08 * fade * strength;
        let downrange = 0.15 + 0.85 * smoothstep(-0.1, 0.8, along / reach);
        let torn = soft_noise(xy + vec2<f32>(53.0, 11.0) - way * t * 7.0, max(reach * 0.06, 0.8), pixel);
        let spray = ring * downrange * mix(0.15, 1.1, torn) * (1.0 - smoothstep(0.15, life * 0.8, t));
        // Spray that fell back lies as a thin lace over the pressed water.
        let lace = (1.0 - smoothstep(front - 0.3, front, rr)) * smoothstep(0.3, 0.9, t) * fade * 0.35
            * smoothstep(0.45, 0.7, torn) * downrange;
        out.foam = max(out.foam, clamp((spray + lace) * strength, 0.0, 1.0));
    }
    // Wakes are laid down along the path each hull took: the arms and the white
    // water spread out and fade where the water was stirred, so a wake grows out
    // from the stern as a hull gets going and is left lying when it stops.
    var churn = 0.0;
    var torpedo_air = 0.0;
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
        if torpedo {
            // The line its run leaves on the water, by its look (`mc_data::TorpedoLook`):
            // 0 air: a bright seam of bubbles over a wider pale band, breaking into
            //   specks as it ages;
            // 1 a plasma drive: a narrow glassy line of steam, few bubbles, and a red
            //   glow under the water behind the head;
            // 2 an interceptor: a thin fizzing line snaking behind it, soon gone;
            // 3 heavy: twin seams from its two screws over a broad band that lies long;
            // 4 a pump-jet: its air let go in gulps, a dotted line.
            // Never thinner than a pixel and a half, so it still reads from high up.
            let look = u32(kind - 1.5);
            let life = shape.y;
            let heavy = look == 3u;
            let sprint = look == 2u;
            let plasma = look == 1u;
            var c = sea_fx.wakes[i].trail[0];
            // Metres back along the line from the head, for the gulps and the snaking.
            var run = 0.0;
            for (var j = 0u; j < 11u; j++) {
                let a = c;
                c = sea_fx.wakes[i].trail[j + 1u];
                let ab = c.xy - a.xy;
                let len = length(ab);
                let run_a = run;
                run += len;
                // Padding past the end of the path.
                if a.z <= 0.0 && c.z <= 0.0 {
                    continue;
                }
                let raw = dot(xy - a.xy, ab) / max(len * len, 0.0001);
                let u = clamp(raw, 0.0, 1.0);
                let rel = xy - a.xy - ab * u;
                var d = length(rel);
                // Across the line, signed: square to the stretch, not round its ends.
                let across = dot(rel, vec2<f32>(-ab.y, ab.x)) / max(len, 0.001);
                let back = run_a + u * len;
                // (Negative while its air is still on the way up.)
                let age = mix(a.w, c.w, u);
                let spd = mix(a.z, c.z, u);
                var true_w = 0.6 + max(age, 0.0) * 0.45;
                if sprint {
                    // Snakes as it steers: the line weaves across its path.
                    d = abs(across - sin(back * 0.21 + f32(i)) * 1.1);
                    true_w = 0.3 + max(age, 0.0) * 0.3;
                } else if heavy {
                    true_w = 0.9 + max(age, 0.0) * 0.5;
                } else if plasma {
                    true_w = 0.45 + max(age, 0.0) * 0.3;
                }
                let width = max(true_w, pixel * 1.5);
                let band = width * select(2.6, 3.4, heavy);
                if d > band * 2.0 {
                    continue;
                }
                let fade = (1.0 - smoothstep(life * 0.35, life, age)) * smoothstep(-0.2, 0.4, age)
                    * smoothstep(1.0, 6.0, spd) * sqrt(true_w / width) * strength;
                let specks = smoothstep(0.3, 0.7, soft_noise(xy + vec2<f32>(time * 0.25, 0.0), 1.3, pixel));
                var seam = exp(-d * d / (width * width));
                if heavy && raw >= 0.0 && raw <= 1.0 {
                    // Two screws: two seams either side of the line, merging as they
                    // spread. Only along a stretch: round its ends they drew rings.
                    let apart = 0.9 + max(age, 0.0) * 0.25;
                    let e = abs(across) - apart;
                    seam = max(seam * 0.55, exp(-e * e / (width * width * 0.5)));
                }
                if look == 4u {
                    // Let go in gulps every few metres: dots that spread into rings of air.
                    let gulp = fract(back / 9.0);
                    seam *= smoothstep(0.0, 0.12, gulp) * (1.0 - smoothstep(0.35, 0.55, gulp));
                }
                seam *= mix(select(0.4, 0.8, plasma), 1.0, specks) * mix(0.55, 0.9, exp(-max(age, 0.0) / 2.5));
                // Steam, not air: a glassy line, only a little white.
                let white = select(1.0, 0.45, plasma) * select(1.0, 1.15, sprint);
                churn = max(churn, seam * fade * white);
                torpedo_air = max(torpedo_air, exp(-d * d / (band * band)) * fade * select(1.0, 0.6, look == 4u));
                if plasma {
                    // The drive's glow under the water, hot behind the head and dying fast.
                    let hot = exp(-d * d / (band * band * 0.6)) * exp(-max(age, 0.0) / 1.2) * fade;
                    out.ember = max(out.ember, hot);
                }
            }
            continue;
        }
        // A hull. Everything is sized by its beam, so a skiff leaves a skiff's wake
        // and the Leviathan a battleship's; nothing in it is a repeating wave train,
        // so nothing can hatch or alias: the arms are one soft crest each, torn into
        // feathers by noise, and every noise fades to its mean before it gets down
        // to a few pixels.
        let right = vec2<f32>(fwd.y, -fwd.x);
        let local = xy - at.xy;
        let cell = max(half_beam * 0.5, 1.0);
        let drift = vec2<f32>(time * 0.35, -time * 0.2);
        if kind < 0.5 {
            // The bow wave: water heaped up against the stem and the fore part of
            // the hull, standing higher and further off the faster it goes, and
            // breaking white along its crest. It rides with the hull.
            let stand = clamp(speed / 14.0, 0.0, 1.25);
            let b = half_length - dot(local, fwd);
            let side = dot(local, right);
            // Narrow at the stem, the hull's full beam a third of the way back.
            let flare = half_beam * (0.3 + 0.75 * sqrt(clamp(b / (half_length * 0.6), 0.0, 1.0)));
            let bw = max((0.4 + half_beam * 0.3) * (0.45 + 0.6 * stand), pixel * 1.5);
            let crest = abs(side) - flare - bw * 0.35;
            let reach = (1.0 - smoothstep(half_length * 0.12, half_length * (0.45 + 0.4 * stand), b))
                * smoothstep(-half_beam * 0.7, -half_beam * 0.05, b) * smoothstep(1.0, 7.0, speed) * strength;
            if reach > 0.001 && abs(crest) < bw * 3.0 {
                let heap = exp(-crest * crest / (bw * bw)) * reach;
                let rise = bw * 0.12 * stand;
                out.slope += right * sign(side) * (-2.0 * crest / (bw * bw)) * rise * heap;
                let torn = soft_noise(xy + vec2<f32>(71.0, 29.0) + drift * 1.5, cell * 0.4, pixel);
                out.foam = max(out.foam, heap * stand * mix(0.35, 1.05, torn));
                out.aerate = max(out.aerate, heap * min(stand, 1.0) * 0.7);
                out.rough += heap * 0.03;
            }
        }
        // The arms spread from the path the bow took, newest first; the white water
        // lies along the stern's. Each stretch sends its arms out square to itself;
        // a stretch's round ends only count on the outside of a bend, between the
        // two stretches (elsewhere they would ring every point with arms of their
        // own), and never ahead of the stem or behind where the hull got going.
        let life = 9.0;
        let arm_life = min(4.5 + half_beam * 0.2, 7.5);
        let stern = sea_fx.wakes[i].trail[1].xy;
        let breaking = mix(0.45, 0.95, smoothstep(10.0, 40.0, speed + half_beam));
        var crest = 0.0;
        var crest_slope = vec2<f32>(0.0);
        var crest_foam = 0.0;
        var crest_air = 0.0;
        var white = 0.0;
        var fresh = 0.0;
        var prev_raw = 0.5;
        // Metres the bow has run since it passed the stretch's newer end.
        var run = 0.0;
        var c = sea_fx.wakes[i].arms[0];
        for (var j = 0u; j < 11u; j++) {
            let a = c;
            c = sea_fx.wakes[i].arms[j + 1u];
            let ab = c.xy - a.xy;
            let len2 = dot(ab, ab);
            let len = sqrt(len2);
            let run_a = run;
            run += len;
            // A stretch made standing still, or padding past the end of the path.
            if (a.z <= 0.0 && c.z <= 0.0) || len2 < 0.01 {
                prev_raw = 0.5;
                continue;
            }
            let raw = dot(xy - a.xy, ab) / len2;
            let u = clamp(raw, 0.0, 1.0);
            let off = xy - a.xy - ab * u;
            let d2 = dot(off, off);
            let age = max(mix(a.w, c.w, u), 0.0);
            let spd = mix(a.z, c.z, u);
            let last = j == 10u || all(sea_fx.wakes[i].arms[min(j + 2u, 11u)].xy == c.xy);
            // Metres past an end of the whole path, where the arms give out.
            var over = 0.0;
            var arms = true;
            if raw < 0.0 {
                if j == 0u {
                    over = -raw * len;
                } else {
                    arms = prev_raw > 1.0;
                }
            }
            if raw > 1.0 {
                if last {
                    over = (raw - 1.0) * len;
                } else {
                    arms = false;
                }
            }
            prev_raw = raw;
            if !arms {
                continue;
            }
            // The Kelvin arm: one soft crest at the wedge's angle (narrower behind a
            // fast boat), widening and dying as it spreads, opened by how far the bow
            // has run since it passed here. The wedge opens from the stem, so by the
            // stern of a long hull it is already well out.
            let d = sqrt(d2);
            let open = mix(0.34, 0.2, smoothstep(14.0, 40.0, spd));
            let arm = d - (half_beam * 0.35 + (run_a + u * len) * open);
            let arm_w = max(0.5 + age * (0.6 + half_beam * 0.03) + half_beam * 0.2, pixel * 1.5);
            if abs(arm) < arm_w * 3.0 && over < arm_w * 2.0 {
                let env = exp(-arm * arm / (arm_w * arm_w)) * (1.0 - smoothstep(0.0, arm_life, age))
                    * smoothstep(2.0, 10.0, spd) * (1.0 - smoothstep(0.0, arm_w * 1.5, over)) * strength;
                let rise = arm_w * 0.16 * min(spd / 12.0, 1.5);
                let s = (-2.0 * arm / (arm_w * arm_w)) * rise * env;
                // Where two stretches' arms cross, the stronger one leads.
                if abs(s) > abs(crest) {
                    crest = s;
                    crest_slope = (off / max(d, 0.001)) * s;
                }
                // White only on its inner, breaking side, and only while young.
                let young = 1.0 - smoothstep(0.3, 2.5 + half_beam * 0.15, age);
                let inner = exp(-(arm + arm_w * 0.25) * (arm + arm_w * 0.25) / (arm_w * arm_w * 0.7));
                crest_foam = max(crest_foam, inner * env * breaking * young);
                crest_air = max(crest_air, env * young);
            }
        }
        // Churned white water behind the stern, widening as it goes: point 1 of the
        // trail is the stern, then where it was.
        c = sea_fx.wakes[i].trail[1];
        for (var j = 1u; j < 11u; j++) {
            let a = c;
            c = sea_fx.wakes[i].trail[j + 1u];
            let ab = c.xy - a.xy;
            let len2 = dot(ab, ab);
            if (a.z <= 0.0 && c.z <= 0.0) || len2 < 0.01 {
                continue;
            }
            let u = clamp(dot(xy - a.xy, ab) / len2, 0.0, 1.0);
            let off = xy - a.xy - ab * u;
            let d2 = dot(off, off);
            let age = max(mix(a.w, c.w, u), 0.0);
            let spd = mix(a.z, c.z, u);
            let width = half_beam * 0.75 + age * (0.8 + half_beam * 0.06);
            if d2 > width * width * 9.0 {
                continue;
            }
            var w = exp(-d2 / (width * width)) * (1.0 - smoothstep(life * 0.1, life, age))
                * smoothstep(1.0, 7.0, spd);
            if j == 1u {
                // None ahead of the stern: the round end of the churn stays under the hull.
                w *= 1.0 - smoothstep(-half_beam * 0.3, half_beam * 0.5, dot(xy - stern, fwd));
            }
            white = max(white, w);
            fresh = max(fresh, w * exp(-age / (1.2 + half_beam * 0.08)));
        }
        if crest_air > 0.002 {
            // Feathered: the crests' height wanders slowly along them (a streaky slope
            // would glint in stripes), and their white comes and goes in uneven tufts.
            // Round noise, not streaks slanting off the track: those lined up into
            // hatching wherever the camera looked along them. Every cell here is fixed
            // per hull (see `soft_noise`).
            let feather = soft_noise(xy + drift, cell * 1.1, pixel);
            let tufts = soft_noise(xy + vec2<f32>(13.0, 41.0) - drift, cell * 1.3, pixel) * 0.45
                + soft_noise(xy + vec2<f32>(3.0, 61.0) + drift, cell * 0.3, pixel) * 0.2 + feather * 0.35;
            let feathers = smoothstep(0.27, 0.64, tufts);
            out.slope += crest_slope * mix(0.45, 1.2, feather);
            out.rough += crest_air * 0.025;
            out.foam = max(out.foam, crest_foam * mix(0.2, 1.15, feathers));
            out.aerate = max(out.aerate, crest_air * 0.3);
        }
        if white > 0.01 {
            // Boiling: dense and white right behind the screws, torn into patches and
            // thinning as it spreads, the patches turning over as the water churns.
            let big = soft_noise(xy + drift, max(half_beam * 0.9, 2.0), pixel);
            let small = soft_noise(xy + vec2<f32>(17.0, 17.0) - drift * 1.1, cell * 0.32, pixel);
            let boil = big * 0.6 + small * 0.4;
            let w = white * strength;
            // Solid right behind the screws, then patches, then threads.
            let patches = smoothstep(0.38, 0.72, boil);
            out.foam = max(out.foam, clamp(w * (0.15 + 0.7 * patches) + fresh * strength * 0.3, 0.0, 1.0));
            out.aerate = max(out.aerate, w * mix(0.55, 1.0, big));
            // Lumpy, broken water: rough enough to scatter the light, no pattern to glint.
            out.rough += w * 0.05;
        }
    }
    if churn > 0.01 || torpedo_air > 0.01 {
        // Broken up along its length: foam comes up in patches, not as a painted line.
        let patchy = 0.7 + 0.3 * soft_noise(xy + vec2<f32>(time * 0.3, 0.0), 6.0, pixel);
        out.foam = max(out.foam, churn * patchy);
        out.aerate = max(out.aerate, torpedo_air * 0.5);
        out.rough += churn * 0.03 + torpedo_air * 0.02;
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

// ---------------------------------------------------------------- the sea from far off

struct FarSea {
    // Change to the water's brightness, about 0: slicks lighter, gusts darker.
    tone: f32,
    // Roughness the ruffled water adds.
    rough: f32,
    // Whitecap cover, 0..1.
    foam: f32,
}

// What the eye reads of open water from high up, where every ripple is under a
// pixel: lanes of slick and ruffled water drawn out down the wind, gusts
// darkening the water as they run across it, and whitecaps breaking and
// fading, thickest in the gusts. Everything drifts downwind fast enough to see
// moving from a strategic height.
fn far_sea(xy: vec2<f32>, time: f32, pixel: f32) -> FarSea {
    var out: FarSea;
    let wind = normalize(SEA_WIND);
    // The wind's frame: `a` downwind, `c` across it.
    let a = dot(xy, wind);
    let c = dot(xy, vec2<f32>(-wind.y, wind.x));
    // Lanes, seven times longer than wide, drifting and slowly re-forming.
    let lane_a = soft_noise(vec2<f32>((a - time * 6.0) * 0.14, c + time * 0.7), 150.0, pixel);
    let lane_b = soft_noise(vec2<f32>((a - time * 9.0) * 0.2 + 311.0, c - 97.0 - time * 0.4), 55.0, pixel);
    let ruffle = (lane_a - 0.5) * 2.6 + (lane_b - 0.5) * 1.4;
    // Gusts: patches of ruffled water running downwind faster than the lanes.
    let bend = grad_noise2(xy + vec2<f32>(517.0, -211.0), 1400.0) - 0.5;
    let g = grad_noise2(vec2<f32>(a - time * 22.0 + bend * 700.0, c * 1.5), 640.0) * 0.65
        + grad_noise2(vec2<f32>(a - time * 27.0 - 800.0, c * 1.3 + 400.0), 240.0) * 0.35;
    let gust = smoothstep(0.46, 0.64, g);
    out.tone = -ruffle * 0.26 - gust * 0.26;
    out.rough = gust * 0.012 + max(ruffle, 0.0) * 0.004;
    // Whitecaps: a cap breaks in a cell, flares and fades; cells drift with the
    // wind. Under a few pixels a cell is only its mean cover.
    let cell = 16.0;
    let p = vec2<f32>(a - time * 7.0, c) / cell;
    let id = floor(p);
    let f = p - id - 0.5;
    let h = hash21(id + vec2<f32>(17.0, -3.0));
    let chance = 0.35 * gust * gust;
    let life = fract(time / 6.5 + h * 13.7);
    let flare = smoothstep(0.0, 0.06, life) * (1.0 - smoothstep(0.06, 0.55, life));
    // A streak of spume blown out downwind of where it broke, off the cell's middle.
    let off = f - (vec2<f32>(hash21(id + 5.0), hash21(id + 9.0)) - 0.5) * 0.4;
    let trail = select(off.x * 0.45, off.x * 1.8, off.x > 0.0);
    let shape = (1.0 - smoothstep(0.03, 0.2, length(vec2<f32>(trail, off.y * 2.6)))) * 0.8;
    let cap = flare * shape * step(hash21(id - 31.0), chance);
    let shown = smoothstep(1.5, 4.0, cell / max(pixel, 0.001));
    out.foam = mix(chance * 0.02, cap, shown);
    return out;
}

// How light goes through this map's water: absorbed per metre, red first, and
// the colour it scatters back from a column `column` metres deep, lit by `lit`.
struct Optics {
    absorb: vec3<f32>,
    scatter: vec3<f32>,
}

fn water_optics(column: f32, lit: f32) -> Optics {
    var o: Optics;
    if tropical() {
        // Bahamas water: very clear, so sand shows through turquoise over the
        // banks, cyan-teal at 10-20 m, and sapphire in the deep channels.
        o.absorb = TROPIC_ABSORB;
        let deep_hue = mix(TROPIC_AZURE, TROPIC_DEEP, smoothstep(TROPIC_SCATTER_DEPTHS.y, TROPIC_SCATTER_DEPTHS.z, column));
        o.scatter = mix(TROPIC_SHALLOW, deep_hue, smoothstep(TROPIC_SCATTER_DEPTHS.x, TROPIC_SCATTER_DEPTHS.y, column)) * lit;
    } else if desert() {
        // A canyon reservoir (Lake Powell, Lake Mead): clear and very saturated,
        // jade over the pale shallows, teal-blue, then cobalt down the old channel.
        o.absorb = DESERT_ABSORB;
        let deep_hue = mix(DESERT_TEAL, DESERT_DEEP, smoothstep(DESERT_SCATTER_DEPTHS.y, DESERT_SCATTER_DEPTHS.z, column));
        o.scatter = mix(DESERT_JADE, deep_hue, smoothstep(DESERT_SCATTER_DEPTHS.x, DESERT_SCATTER_DEPTHS.y, column)) * lit;
    } else {
        // Clear, lightly green coastal water: red is gone in a few metres, and
        // the seabed reads through a dozen metres or so of it.
        o.absorb = vec3<f32>(0.17, 0.032, 0.025);
        o.scatter = mix(vec3<f32>(0.010, 0.050, 0.052), vec3<f32>(0.0045, 0.020, 0.036), smoothstep(2.0, 30.0, column)) * lit;
    }
    return o;
}

@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    if globals.camera.z < globals.map.z {
        return under_sea(in.clip);
    }
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
    if depth <= 0.02 {
        discard;
    }

    // What is behind this pixel if there were no water.
    let behind_d = sea_depth_at(uv);
    let open = behind_d <= 0.0000002;
    let behind = sea_world(uv, behind_d);

    // ---- the surface
    // The swell calms toward the shore, where the breakers take over; the
    // short waves come and go in gusts running downwind.
    let amp = smoothstep(0.2, 5.0, depth);
    let calm = 1.0 - smoothstep(1.0, 12.0, depth);
    // 1 where this pixel is metres across over open water: the view from a
    // strategic height, where the far sea's lanes, gusts and swell take over.
    let far = smoothstep(1.5, 6.0, pixel) * smoothstep(4.0, 16.0, depth);
    let gust = grad_noise2(xy - SEA_WIND * time * 5.0 + vec2<f32>(37.0, 11.0), 120.0);
    let chop = mix(0.45, 1.35, smoothstep(0.3, 0.72, gust));
    let waves = sea_waves(xy, time, pixel, amp, chop);
    var far_fx: FarSea;
    if far > 0.0 {
        far_fx = far_sea(xy, time, pixel);
    }
    // Rings, wakes and foam from what is happening on the water.
    var stir: SeaStir;
    if dist < 6000.0 {
        stir = sea_stir(xy, time, pixel);
    }
    // Breakers rolling in on the shore (shore.wgsl).
    var breakers: Surf;
    if depth < SURF_REACH_DEPTH {
        breakers = surf(xy, shore_at(xy, depth), time, pixel);
    }
    // A muzzle blast presses the waves flat for a moment.
    let unpressed = 1.0 - stir.flat;
    let n = normalize(vec3<f32>(-(waves.slope * unpressed + stir.slope + breakers.slope), 1.0));
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
    // Light the water scatters back to the eye, lit by the sun, dimmer in shadow.
    let sun_in = max(globals.sun.z, 0.0);
    let lit = mix(0.45, 1.0, shadow) * (0.55 + 0.45 * sun_in);
    // Over a hull the water keeps the colour of the depth it stands in, so a
    // dived boat does not show as a patch of shallow-water green.
    let optics = water_optics(mix(sink, max(sink, depth), hull), lit);
    let through = exp(-optics.absorb * path);
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
    // A wave's back tipped away from the eye would send the reflected ray into
    // the sea; the water in front of it is what it sees then, which is the sky
    // just above the horizon, not the haze under it.
    var r = reflect(-v, n);
    r = normalize(vec3<f32>(r.xy, max(abs(r.z), 0.01)));
    var reflected = shore_reflect(world, r, dist);
    let ssr = screen_reflect(world, r, dist);
    reflected = mix(reflected, ssr.rgb, ssr.a);

    // Sun: a sharp GGX highlight, widened by the waves this pixel cannot show.
    let l = globals.sun.xyz;
    let h = normalize(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let rough = sqrt(0.003 + waves.lost * unpressed + stir.rough + far_fx.rough * far);
    let a2 = rough * rough;
    let n_dot_h = max(dot(n, h), 0.0);
    let dd = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    let ggx = a2 / (PI * dd * dd);
    let f_sun = 0.02 + 0.98 * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    let spec = min(ggx * f_sun * n_dot_l / (4.0 * n_dot_v * max(n_dot_l, 0.05) + 0.001), 60.0);
    let sun_color = vec3<f32>(1.0, 0.95, 0.85) * 2.7;

    var color = optics.scatter * (vec3<f32>(1.0) - through) * (1.0 - fresnel)
        + reflected * fresnel + sun_color * spec * shadow;
    // Light through the thin water of a breaker's rearing face glows the water's
    // own colour. (Not every crest's: lit where the open sea's trains stack up,
    // they drew a regular grid of pale blotches across it from high up.)
    let glow = breakers.face * 1.6;
    color += optics.scatter * glow * (0.4 + 0.6 * sun_in) * 1.6;
    // Pressed flat, the water shows darker: no waves catching the sky.
    color *= 1.0 - 0.5 * stir.flat;
    // From far off the lanes and gusts, and the long swell's crests catching the light.
    color *= 1.0 + (far_fx.tone * select(1.0, 0.55, desert()) + waves.swell * 0.06) * far;

    // ---- foam
    var cover = breakers.foam;
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
            let a = f32(k) * 0.785398 + 0.39;
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
    // Whitecaps where the waves pile up steepest, in the gusts; out in open
    // water only. Under a few pixels a cap is only its average.
    let caps_shown = smoothstep(1.2, 3.5, 3.0 / max(pixel, 0.001));
    let caps = smoothstep(2.1, 3.2, waves.crest) * smoothstep(0.5, 0.85, gust) * amp * (1.0 - calm);
    cover = max(cover, caps * caps_shown * 0.6);
    // A sheltered lake: few whitecaps, calmer lanes.
    let sheltered = select(1.0, 0.25, desert());
    let far_caps = far_fx.foam * far * sheltered;
    let foam = foam_lace(xy, time, pixel, cover * 0.85);
    let foam_color = vec3<f32>(0.80, 0.86, 0.88) * (0.35 + 0.65 * shadow) * (0.55 + 0.45 * sun_in);
    // Water a hull has churned full of air: paler and greener, lit from within,
    // and it hides what is under it.
    let aerate = clamp(stir.aerate + breakers.lip * 0.5, 0.0, 1.0) * 0.5;
    color = mix(color, foam_color * vec3<f32>(0.42, 0.7, 0.68), aerate);
    color = mix(color, foam_color, foam * 0.92);
    // A breaking lip is solid white water.
    let lip = clamp(breakers.lip, 0.0, 1.0);
    color = mix(color, foam_color * 1.08, lip * 0.95);
    color = mix(color, foam_color, far_caps * 0.9);
    let white = max(foam, lip);
    if dist < 6000.0 {
        color += sea_flash(world, n, v, rough, fresnel, time, pixel) * (1.0 - white * 0.6);
    }
    color += vec3<f32>(0.9, 0.12, 0.05) * stir.ember * (1.0 - fresnel) * 0.9;
    // Lamps, fires and blasts (lights.rs): long glints across the waves, and
    // their light on foam and murky shallows.
    var lit_water: Pbr;
    lit_water.albedo = mix(vec3<f32>(0.018, 0.03, 0.034), vec3<f32>(0.7, 0.75, 0.78), white);
    lit_water.metallic = 0.0;
    lit_water.roughness = max(rough, 0.14);
    lit_water.emissive = vec3<f32>(0.0);
    color += min(local_lights(lit_water, world, n, v), vec3<f32>(24.0));
    if (globals.counts.w & 2u) != 0u {
        // The build grid while a structure is being placed, on the surface where it would stand.
        color = build_grid_overlay(color, xy, dist);
    }
    color = apply_fog_of_war(color, xy) + seen_through * (1.0 - white * 0.92) * (1.0 - aerate);
    color = apply_haze(color, world, eye);
    // Shore pixels blend out through the height-field edge, so the coastline
    // stays soft where the terrain mesh and height field disagree.
    let alpha = smoothstep(0.015, 0.12, depth);
    return vec4<f32>(color, alpha);
}

// ---------------------------------------------------------------- under the sea

// The free camera under the water: the whole view is drawn here, over the
// opaque scene. What it sees is dimmed and tinted by the water in between; the
// surface overhead shows the sky through a round window (Snell's) and beyond
// it mirrors the water below; shafts of sunlight slant down through it.
fn under_sea(clip: vec4<f32>) -> vec4<f32> {
    let uv = clip.xy / globals.scene.xy;
    let eye = globals.camera.xyz;
    let water = globals.map.z;
    let time = globals.camera.w;
    let dir = normalize(sea_world(uv, 0.001) - eye);
    let scene_d = sea_depth_at(uv);
    var t_scene = 1e9;
    if scene_d > 0.0000002 {
        t_scene = distance(sea_world(uv, scene_d), eye);
    }
    var t_top = 1e9;
    if dir.z > 0.0001 {
        t_top = (water - eye.z) / dir.z;
    }
    let t = min(t_scene, t_top);
    let end = eye + dir * min(t, 400.0);
    // How far a pixel reaches there, for the waves' and caustics' filtering.
    let pixel = max(max(length(dpdx(end)), length(dpdy(end))), 0.002);
    let sun_in = max(globals.sun.z, 0.0);
    let deep = max(water - eye.z, 0.0);
    // The water's own glow: the light scattered toward the eye, brightest near
    // the surface and dimming with depth, strongest looking up toward the sun.
    let optics = water_optics(12.0, 0.55 + 0.45 * sun_in);
    let toward_sun = pow(max(dot(dir, globals.sun.xyz), 0.0), 3.0);
    // Bright overhead, dark looking down into the deep.
    let fog_color = optics.scatter * (4.0 + 4.0 * toward_sun) * mix(0.3, 1.5, clamp(dir.z * 0.6 + 0.5, 0.0, 1.0))
        * exp(-optics.absorb * deep * 0.5);
    // The water is clearer than the colour absorbed in it suggests: past 60 m
    // or so nothing shows but the water itself.
    let extinction = optics.absorb * 0.4 + vec3<f32>(0.006);
    let through = exp(-extinction * min(t, 400.0));

    var color: vec3<f32>;
    if t_top < t_scene {
        // ---- the surface from below
        let p = eye + dir * t_top;
        let waves = sea_waves(p.xy, time, pixel, 1.0, 1.0);
        var stir: SeaStir;
        if t_top < 600.0 {
            stir = sea_stir(p.xy, time, pixel);
        }
        let n = normalize(vec3<f32>(-(waves.slope + stir.slope), 1.0));
        // From water into air: past about 48.6 degrees from overhead the light
        // cannot leave, and the surface is a mirror of the water under it.
        let out = refract(dir, -n, 1.33);
        // The mirror shows the seabed and whatever is under the water, shimmering
        // with the waves: found by projecting where the reflected ray meets the bed.
        let down = reflect(dir, -n);
        let bed_at = p + down * ((water - terrain_height(p.xy)) / max(-down.z, 0.05));
        let seen = sea_uv(bed_at);
        var mirror = optics.scatter * 7.0;
        if seen.z > 0.0 && all(seen.xy > vec2<f32>(0.0)) && all(seen.xy < vec2<f32>(1.0)) && sea_depth_at(seen.xy) > 0.0000002 {
            let far_off = distance(p, bed_at);
            let lit_bed = sea_scene(seen.xy) * 0.6;
            let lost = exp(-extinction * min(far_off, 400.0));
            mirror = lit_bed * lost + optics.scatter * 7.0 * (vec3<f32>(1.0) - lost);
        }
        // The underside of the waves catches the light unevenly.
        mirror *= 0.75 + 0.6 * caustics(p.xy * 0.5, time * 0.8, pixel);
        if dot(out, out) < 0.001 {
            color = mirror;
        } else {
            let cos_t = max(dot(out, n), 0.0);
            let f = 0.02 + 0.98 * pow(1.0 - cos_t, 5.0);
            // The sky, with the sun's glare where it shines down.
            var sky = sky_along(out) * 1.1;
            // At the window's rim the view is squeezed flat onto the horizon.
            sky *= smoothstep(0.0, 0.25, cos_t) * 0.8 + 0.2;
            color = mix(sky, mirror, f);
        }
        // Foam and churned water on top block the sky, grey-white from below.
        let foam = clamp(stir.foam + stir.aerate * 0.5, 0.0, 1.0);
        color = mix(color, fog_color * 1.6 + vec3<f32>(0.05, 0.07, 0.07) * sun_in, foam * 0.8);
    } else if scene_d > 0.0000002 {
        // ---- the seabed, a hull
        let hit = eye + dir * t_scene;
        var below = sea_scene(uv);
        let sink = max(water - hit.z, 0.0);
        let hull = smoothstep(0.5, 1.5, hit.z - terrain_height(hit.xy));
        // Only the light that came down through the water reaches it.
        below *= mix(0.95, 0.5, smoothstep(2.0, 40.0, sink)) * mix(1.0, 0.8, hull);
        let focus = caustics(hit.xy, time, pixel) * exp(-sink * 0.08) * (1.0 - hull * 0.6);
        below *= 1.0 + focus * 2.2 * sun_in;
        color = below;
    } else {
        color = fog_color;
    }
    color = color * through + fog_color * (vec3<f32>(1.0) - through);
    // Shafts of sunlight slanting down from the surface: brighter where the
    // waves overhead focused it, fading into the depth.
    if sun_in > 0.05 {
        var shafts = 0.0;
        let reach = min(t, 70.0);
        let slant = globals.sun.xy / max(globals.sun.z, 0.2);
        for (var i = 0; i < 8; i++) {
            let s = (f32(i) + 0.5) / 8.0 * reach;
            let q = eye + dir * s;
            let down = max(water - q.z, 0.0);
            // Where the ray down to this point met the surface.
            let at = q.xy + slant * down;
            let beam = caustics(at * 0.12, time * 0.35, 0.1);
            shafts += beam * exp(-down * 0.06) * exp(-s * 0.035);
        }
        color += optics.scatter * shafts / 8.0 * reach * 0.18 * sun_in * (0.6 + 0.8 * toward_sun);
    }
    color = apply_fog_of_war(color, end.xy);
    return vec4<f32>(color, 1.0);
}

// The tropical sea (`tropical()`): absorption per metre, the colour scattered
// back over the shallows and the deep, and the depths it turns between.
const TROPIC_ABSORB: vec3<f32> = vec3<f32>(0.30, 0.042, 0.026);
const TROPIC_SHALLOW: vec3<f32> = vec3<f32>(0.012, 0.075, 0.080);
// Over the drop-off: bright azure, before the channels' sapphire.
const TROPIC_AZURE: vec3<f32> = vec3<f32>(0.004, 0.042, 0.105);
const TROPIC_DEEP: vec3<f32> = vec3<f32>(0.0015, 0.011, 0.068);
// Shallow to azure over x..y metres of water, azure to deep over y..z.
const TROPIC_SCATTER_DEPTHS: vec3<f32> = vec3<f32>(7.0, 20.0, 50.0);

// The canyon reservoir (`desert()`): the same, greener in the shallows.
const DESERT_ABSORB: vec3<f32> = vec3<f32>(0.40, 0.068, 0.095);
const DESERT_JADE: vec3<f32> = vec3<f32>(0.006, 0.050, 0.036);
const DESERT_TEAL: vec3<f32> = vec3<f32>(0.002, 0.026, 0.050);
const DESERT_DEEP: vec3<f32> = vec3<f32>(0.001, 0.007, 0.034);
const DESERT_SCATTER_DEPTHS: vec3<f32> = vec3<f32>(3.0, 12.0, 35.0);

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
