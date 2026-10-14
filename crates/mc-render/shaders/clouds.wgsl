//!use bindings
//!use cloud_shape
// The sky and its clouds (sky.rs).
//
//   fs_sky        end of the scene pass: the sky and sun wherever nothing was drawn
//   fs_march      half size: rays through the cloud layer and the mist banks
//                 under it, stopped by the scene's depth. rgb light scattered
//                 toward the eye, a what gets through
//   fs_resolve    the march folded into last frame's clouds, carried to where
//                 they are now on screen: a new start offset every frame averages
//                 out instead of shimmering or showing its pattern
//   fs_composite  in scene_over, before the icons: the clouds laid over the
//                 picture (a see-through hologram of them in the middle of the
//                 screen), and lightning strokes
//
// The clouds are the weather map (cover, storm, churn: clouds_sim.wgsl) shaped
// by a tiling Perlin-Worley texture drifting with the wind. Where the player
// looks, and over what they have selected, they thin away so the battle shows.

@group(1) @binding(0) var scene_depth: texture_depth_2d;
@group(1) @binding(1) var cloud_noise: texture_3d<f32>;
// rgb and alpha as four halves in x and y, z the distance to the cloud the
// texel saw (0: none), so the resolve can carry each texel back by its own depth,
// w how far the texel marched (to the scene, 1e9 over the sky), so the composite
// takes a pixel's clouds only from texels at its own depth.
@group(1) @binding(2) var cloud_march: texture_2d<u32>;
// The accumulated clouds: last frame's (read by the resolve) and this frame's
// (written by the resolve, read by the composite).
@group(1) @binding(3) var cloud_history: texture_2d<f32>;
@group(1) @binding(4) var cloud_now: texture_2d<f32>;
// This frame's part of the march (`fs_march`), `MARCH_STRIDE` times smaller each way
// than `cloud_march`.
@group(1) @binding(6) var cloud_march_part: texture_2d<u32>;
// The column map (cloud_shape.wgsl), near and far: written by `cs_columns`, read by
// everything after.
@group(1) @binding(7) var cloud_columns_out: texture_storage_2d_array<rgba16float, write>;
@group(1) @binding(8) var cloud_columns: texture_2d_array<f32>;
@group(1) @binding(11) var cloud_columns_far_out: texture_storage_2d_array<rgba16float, write>;
@group(1) @binding(12) var cloud_columns_far: texture_2d_array<f32>;
// The nearest (x) and farthest (y) any march texel within two of each one went
// (`cs_reach`), for the composite's `upsample`.
@group(1) @binding(13) var cloud_reach_out: texture_storage_2d<rg32float, write>;
@group(1) @binding(14) var cloud_reach: texture_2d<f32>;
// The light volume (cloud_shape.wgsl `sun_through`): written by `cs_light`.
@group(1) @binding(9) var cloud_light_out: texture_storage_3d<rgba16float, write>;
@group(1) @binding(10) var cloud_light: texture_3d<f32>;

struct FullOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullOut {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullOut;
    // Depth 0 is infinitely far in reversed Z: the sky pass tests against it.
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn unproject(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    return h.xyz / h.w;
}

fn view_ray(uv: vec2<f32>) -> vec3<f32> {
    return normalize(unproject(uv, 0.01) - globals.camera.xyz);
}

// ---- Sky -----------------------------------------------------------------------

@fragment
fn fs_sky(in: FullOut) -> @location(0) vec4<f32> {
    let d = view_ray(in.uv);
    var color = sky_radiance(d);
    let mu = dot(d, globals.sun.xyz);
    let dark = atmos.horizon_color.w;
    // The disk, and the bright aureole right round it. The moon's disk is a
    // pale silver, not the blue of the light it is standing in for.
    let disk = mix(atmos.sun_color.rgb, vec3<f32>(dot(atmos.sun_color.rgb, vec3<f32>(0.3, 0.45, 0.25))) * vec3<f32>(1.0, 0.98, 0.93), dark);
    color += disk * atmos.sky_color.w * smoothstep(0.99985, 0.99995, mu);
    color += atmos.sun_color.rgb * 0.35 * mix(1.0, 0.25, dark) * pow(max(mu, 0.0), 900.0);
    // Derivatives before the branch: a pixel's size on the sky.
    let px = max(length(fwidth(d)), 1e-5) * 0.8;
    if dark > 0.0 && d.z > -0.02 {
        color += night_sky(d, mu, px) * dark;
    }
    return vec4<f32>(min(color, vec3<f32>(60.0)), 1.0);
}

// Value noise on the sky's sphere: smooth, no seams, no poles.
fn sky_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = p - i;
    let u = f * f * (3.0 - 2.0 * f);
    let n = i.xy + i.z * vec2<f32>(17.31, 41.7);
    let a = mix(hash21(n), hash21(n + vec2<f32>(1.0, 0.0)), u.x);
    let b = mix(hash21(n + vec2<f32>(0.0, 1.0)), hash21(n + vec2<f32>(1.0, 1.0)), u.x);
    let c = mix(hash21(n + vec2<f32>(17.31, 41.7)), hash21(n + vec2<f32>(18.31, 41.7)), u.x);
    let e = mix(hash21(n + vec2<f32>(17.31, 42.7)), hash21(n + vec2<f32>(18.31, 42.7)), u.x);
    return mix(mix(a, b, u.y), mix(c, e, u.y), u.z);
}

// One layer of stars: at most one per cell of a cube-face grid, jittered in
// its cell and drawn as a point the size of a pixel whatever the resolution.
// `px` is the angular size of a pixel. Most are faint, a handful bright.
fn star_layer(d: vec3<f32>, px: f32, cells: f32, share: f32, bright: f32, salt: f32) -> vec3<f32> {
    let a = abs(d);
    var uv: vec2<f32>;
    var face: f32;
    if a.x >= a.y && a.x >= a.z {
        uv = d.yz / a.x;
        face = select(0.0, 1.0, d.x < 0.0);
    } else if a.y >= a.z {
        uv = d.xz / a.y;
        face = select(2.0, 3.0, d.y < 0.0);
    } else {
        uv = d.xy / a.z;
        face = select(4.0, 5.0, d.z < 0.0);
    }
    let g = (uv * 0.5 + 0.5) * cells;
    let cell = floor(g);
    let id = cell + vec2<f32>(face * 1031.0 + salt, face * 577.0 - salt);
    let h = hash21(id);
    if h > share {
        return vec3<f32>(0.0);
    }
    let r = h / share;
    // Kept off the cell's edges so a star's glow never needs the neighbour.
    let at = vec2<f32>(hash21(id + 7.13), hash21(id + 3.71)) * 0.5 + 0.25;
    // Grid units to radians round here: a face's cell grows away from its middle.
    let span = 2.0 / (cells * max(max(a.x, a.y), a.z)) * dot(d, d);
    let off = (g - cell - at) * span / px;
    // Brightness spread like real magnitudes: many at the limit of sight.
    let flux = bright * (0.06 + 0.94 * pow(1.0 - r, 7.0));
    let core = exp(-dot(off, off) * 1.8);
    // Colour from the star's temperature: mostly white, some blue, some amber.
    let t = hash21(id + 11.9);
    let tint = mix(mix(vec3<f32>(0.72, 0.82, 1.0), vec3<f32>(1.0, 0.98, 0.95), smoothstep(0.0, 0.35, t)),
        vec3<f32>(1.0, 0.8, 0.58), smoothstep(0.7, 1.0, t));
    // Scintillation: the air near the horizon makes stars flicker, not the zenith.
    let air = 1.0 - smoothstep(0.0, 0.5, d.z);
    let time = atmos.weather.w;
    let flicker = 1.0 + (sin(time * (6.0 + t * 9.0) + h * 900.0) * 0.5 + sin(time * (13.0 + r * 7.0) + t * 300.0) * 0.3) * (0.12 + 0.5 * air);
    return tint * flux * core * flicker;
}

// The night sky: the band of the galaxy and its dust, then the stars, both
// dimmed by the air near the horizon and washed out round the moon.
fn night_sky(d: vec3<f32>, mu: f32, px: f32) -> vec3<f32> {
    // Starlight lost in the air along the way: the lowest few degrees are bare.
    let up = max(d.z, 0.0);
    let clear = exp(-0.1 / (up + 0.04)) * smoothstep(-0.02, 0.04, d.z);
    // Near the moon its glare hides all but the brightest.
    let glare = 1.0 - 0.85 * smoothstep(0.9, 0.995, mu);
    // The galaxy: a band round a great circle, clumped and split by dust.
    let pole = normalize(vec3<f32>(0.35, -0.62, 0.7));
    let across = dot(d, pole);
    let q = d * 6.0;
    let clumps = sky_noise(q) * 0.6 + sky_noise(q * 2.3 + 5.1) * 0.3 + sky_noise(q * 5.7 + 1.7) * 0.1;
    let band = exp(-across * across / 0.028) * smoothstep(0.3, 0.75, clumps);
    let dust = exp(-(across - 0.03) * (across - 0.03) / 0.003) * smoothstep(0.35, 0.7, sky_noise(q * 1.7 + 9.3));
    let galaxy = band * (1.0 - 0.8 * dust);
    var sky = vec3<f32>(0.62, 0.66, 0.8) * galaxy * 0.08;
    // Stars: a sparse bright layer, and a fine faint one crowded into the band.
    var stars = star_layer(d, px, 160.0, 0.05, 7.0, 0.0);
    stars += star_layer(d, px, 420.0, 0.05 + 0.25 * galaxy, 1.1, 91.0);
    sky += stars * glare;
    return sky * clear;
}

// ---- Clouds --------------------------------------------------------------------

// 1 where clouds are drawn in full, 0 where they have been thinned away:
// round the selection (but never a storm: it is opaque, and a Nimbus's trail of
// them must not vanish round the bomber that laid it), and right in front of the
// eye so flying through the layer never whites out the screen. (The middle of the screen is see-through
// in the composite instead, a hologram of the cloud.)
// A bubble of clear air round the eye (`clearing`), as big as the view is close:
// down among the clouds the battle stays in sight; from high up it is too small to
// reach them. Down among the clouds it clears the cloud near the eye, so the view
// is never a whiteout, but stops short of the ground being looked at: the cloud
// there is at the height aircraft fly, and they fly through it (a bubble reaching
// the ground cleared every cloud round the battle). From high above it shrinks back
// and the layer is seen whole. The same for every sample of a frame: worked out
// once a ray.
fn eye_bubble() -> f32 {
    let eye = globals.camera.xyz;
    let above = eye.z - cloud_floor(eye.xy);
    let among = 1.0 - smoothstep(atmos.layer.y + 300.0, atmos.layer.y + 2500.0, above);
    // From above it is only big enough that a tower top reaching the eye does
    // not white out the screen, and its edge is ragged: a clean sphere up to
    // 1200 m across sliced every tall cloud near the camera into a smooth wall
    // with a hard rim, which showed as bands and streaks as the view moved.
    return mix(clamp(atmos.view.z * 0.12, 250.0, 600.0), max(atmos.view.z * 0.45, 120.0), among);
}

// How much of the cloud at `p` is kept (1 all): thinned over the selection and in
// the bubble round the eye, `bubble` across (`eye_bubble`).
fn clearing(p: vec3<f32>, bubble: f32) -> f32 {
    var keep = 1.0;
    let n = u32(atmos.counts.x);
    for (var i = 0u; i < n; i++) {
        let c = atmos.clears[i];
        let d = distance(p.xy, c.xy);
        keep *= mix(1.0, smoothstep(c.z * 0.55, c.z * 1.5, d), c.w);
    }
    if n > 0u && keep < 1.0 {
        // A Nimbus's seeded cloud is marked by a storm below zero (clouds_sim.wgsl).
        let storm = weather_at(p.xy).y;
        keep = mix(keep, 1.0, max(smoothstep(0.25, 0.55, storm), smoothstep(0.05, 0.3, -storm)));
    }
    let eye = globals.camera.xyz;
    let far = distance(p, eye);
    // Past the most its ragged edge reaches: no noise to look up.
    if far >= bubble * 1.35 {
        return keep;
    }
    let rag = grad_noise2(p.xy + vec2<f32>(p.z * 0.7, -p.z * 0.4) - atmos.wind.xy, bubble * 0.6) - 0.5;
    keep *= smoothstep(bubble * 0.4, bubble, far + rag * bubble * 0.7);
    return keep;
}

// The column map (cloud_shape.wgsl `bake_column`): the near map over the map and its
// margin (z 0), the far one round it (z 1). Every row, or every so manyth from the
// frame number's (sky/columns.rs `STAGGER`), as the dispatch's height says.
@compute @workgroup_size(8, 8)
fn cs_columns(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let far = id.z == 1u;
    let near_size = textureDimensions(cloud_columns_out).xy;
    var size = near_size;
    if far {
        size = textureDimensions(cloud_columns_far_out).xy;
    }
    let stride = max(near_size.y / (groups.y * 8u), 1u);
    let texel = vec2<u32>(id.x, id.y * stride + u32(atmos.frame.x) % stride);
    if any(texel >= size) {
        return;
    }
    let rect = columns_rect(far);
    let xy = rect.xy + (vec2<f32>(texel) + 0.5) / vec2<f32>(size) * rect.zw;
    let column = bake_column(xy);
    for (var k = 0; k < COLUMN_LAYERS; k++) {
        if far {
            textureStore(cloud_columns_far_out, vec2<i32>(texel), k, column[k]);
        } else {
            textureStore(cloud_columns_out, vec2<i32>(texel), k, column[k]);
        }
    }
}

// The light volume (cloud_shape.wgsl `sun_through`): every slice, or every so manyth
// (sky/light.rs `STAGGER`) from the frame number's, as the dispatch's depth says.
@compute @workgroup_size(8, 8)
fn cs_light(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let size = textureDimensions(cloud_light_out);
    let stride = max(size.z / groups.z, 1u);
    let slice = id.z * stride + u32(atmos.frame.x) % stride;
    if any(id.xy >= size.xy) || slice >= size.z {
        return;
    }
    let rect = columns_rect(false);
    let xy = rect.xy + (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size.xy) * rect.zw;
    let span = light_span();
    let up = f32(slice) / f32(size.z - 1u);
    let p = vec3<f32>(xy, cloud_floor(xy) + span.x + up * up * span.y);
    textureStore(cloud_light_out, vec3<i32>(vec2<i32>(id.xy), i32(slice)), vec4<f32>(exp(-sun_taps(p) * LIGHT_OCTAVES), 1.0));
}

// The clouds' shade on everything under them (sky.rs `shade`, read by
// `cloud_shadow`): per texel of the map, the sunlight left after a ray from
// below the layer crosses the same clouds the march draws. The shade was once
// the weather map's cover, smoothed: dark wherever the air was cloudy at all,
// under the billows' holes and the thin shreds as much as under the heaps, so
// the land went dark with nothing drawn overhead.
@group(1) @binding(5) var cloud_shade_out: texture_storage_2d<rgba16float, read_write>;

// How far under the cloud floor's base the shade is taken: below the lowest
// base a cloud can wander down to.
const SHADE_BELOW: f32 = 300.0;
// Taps through fair-weather cloud, then on up through storm towers.
const SHADE_STEPS: i32 = 10;
const SHADE_STORM_STEPS: i32 = 5;
// A new start offset every frame, folded into what the texel had: the taps
// are tens of metres apart and a fixed offset would band the shade.
const SHADE_BLEND: f32 = 0.25;
// Erosion drawn this many times its size: the shade's texels are coarse.
const SHADE_DETAIL: f32 = 2.0;

@compute @workgroup_size(8, 8)
fn cs_shade(@builtin(global_invocation_id) invocation: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let size = textureDimensions(cloud_shade_out);
    // A quarter of the texels a frame, each of a 2x2 block in turn (the clouds move
    // little in four frames), dispatching only those: skipping lanes in a full-size
    // dispatch still pays for inactive lanes on SIMD GPUs. The first frames fill
    // all (sky/shade.rs `record_shade`), as the dispatch's size says.
    let frame = u32(atmos.frame.x);
    let stagger = groups.x * 8u < size.x;
    let stride = select(1u, 2u, stagger);
    let phase = frame % 4u;
    let offset = select(vec2<u32>(0u), vec2<u32>(phase & 1u, phase >> 1u), stagger);
    let id = vec3<u32>(invocation.xy * stride + offset, 0u);
    if any(id.xy >= size) {
        return;
    }
    let xy = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size) * atmos.weather.yz;
    let floor = cloud_floor(xy);
    let start = vec3<f32>(xy, floor + atmos.layer.x - SHADE_BELOW);
    // The same slant `cloud_shadow` casts along.
    let s = globals.sun.xyz;
    let d = normalize(vec3<f32>(s.xy, max(s.z, 0.2)));
    let deck = atmos.layer.y - atmos.layer.x;
    let fair_top = floor + atmos.layer.y + deck * 2.6 * atmos.regions.z + deck * (1.0 + TOP_LUMP) + 200.0;
    let storm_top = floor + atmos.layer.z + (atmos.layer.z - atmos.layer.x) * TOP_LUMP * 0.5;
    let fair_len = (fair_top - start.z) / d.z;
    let storm_len = max(storm_top - fair_top, 0.0) / d.z;
    let jitter = fract(hash21(vec2<f32>(id.xy) + 0.5) + atmos.frame.x * 0.618034);
    var tau = 0.0;
    let dt = fair_len / f32(SHADE_STEPS);
    for (var k = 0; k < SHADE_STEPS; k++) {
        let p = start + d * (dt * (f32(k) + jitter));
        tau += cloud_at(p, SHADE_DETAIL).sigma * dt;
    }
    // Above the fair tops only a storm's towers reach.
    let ds = storm_len / f32(SHADE_STORM_STEPS);
    for (var k = 0; k < SHADE_STORM_STEPS; k++) {
        let p = start + d * (fair_len + ds * (f32(k) + jitter));
        let at = column_at(p.xy);
        let c0 = column_base(at);
        if c0.w < 0.02 {
            continue;
        }
        tau += cloud_shape(p, at, c0, SHADE_DETAIL, false).sigma * ds;
    }
    let lit = exp(-tau);
    let texel = vec2<i32>(id.xy);
    let old = textureLoad(cloud_shade_out, texel);
    // Alpha 0: never written (sky.rs clears it so), take this frame whole.
    // Four frames elapsed between these samples; keep the same response time.
    let weight = select(SHADE_BLEND, 1.0 - pow(1.0 - SHADE_BLEND, 4.0), stagger);
    let blend = select(weight, 1.0, old.a < 0.5);
    textureStore(cloud_shade_out, texel, vec4<f32>(mix(old.r, lit, blend), 0.0, 0.0, 1.0));
}

// Interleaved gradient noise: a per-pixel start offset for the march.
// A start offset with no pattern across the screen (a per-texel hash), stepped
// by the golden ratio each frame so every texel walks its whole range over a
// few frames; fs_resolve averages them. A patterned offset (interleaved
// gradient noise) showed through thin cloud as a lattice.
fn ign(p: vec2<f32>) -> f32 {
    return fract(hash21(floor(p) + vec2<f32>(0.5, 0.5)) + atmos.frame.x * 0.618034);
}

// How far along the ray the cloud the march saw lies, for the resolve.
var<private> march_depth: f32;
// How far the march went: to the scene, or 1e9 over the sky (the composite's upsample).
var<private> march_reach: f32;

// The finest the march's stride inside cloud need be, metres (`march`).
const MARCH_FINE: f32 = 50.0;

// One texel of each `MARCH_STRIDE`-square block is marched a frame, each in turn,
// and `fs_fill` fills the rest from the marched ones round them; the resolve folds
// each new march into what the texel had.
const MARCH_STRIDE: i32 = i32(CLOUD_MARCH_STRIDE);

// Which texel of each block is marched this frame: in a Bayer order, so frames one
// after another march texels far apart and the blocks never show.
fn march_offset() -> vec2<i32> {
    var k = i32(atmos.frame.x) % (MARCH_STRIDE * MARCH_STRIDE);
    var at = vec2<i32>(0);
    var half = MARCH_STRIDE / 2;
    while half > 0 {
        let q = k & 3;
        at += vec2<i32>(select(0, half, q == 1 || q == 2), select(0, half, q == 1 || q == 3));
        k = k >> 2u;
        half = half / 2;
    }
    return at;
}

// Whether march texel `px` is marched this frame.
fn marched_now(px: vec2<i32>) -> bool {
    return all((px - march_offset() + MARCH_STRIDE) % MARCH_STRIDE == vec2<i32>(0));
}

// Drawn `MARCH_STRIDE` times smaller each way than the march (`cloud_march_part`):
// each texel marches the ray of the texel of its block marched this frame.
@fragment
fn fs_march(part: FullOut) -> @location(0) vec4<u32> {
    let full = vec2<f32>(textureDimensions(cloud_history));
    var in = part;
    let at = floor(part.clip.xy) * f32(MARCH_STRIDE) + vec2<f32>(march_offset()) + 0.5;
    in.clip = vec4<f32>(at, part.clip.zw);
    in.uv = in.clip.xy / full;
    march_depth = 0.0;
    march_reach = 1.0e9;
    var c = march(in);
    c = with_mist(c, in);
    // A NaN or inf let through here is carried by the resolve into its
    // neighbours every frame, until the screen is white: a bad texel is clear sky.
    if !finite4(c) || !finite4(vec4<f32>(march_depth)) {
        c = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        march_depth = 0.0;
    }
    return vec4<u32>(pack2x16float(c.rg), pack2x16float(c.ba), bitcast<u32>(march_depth), bitcast<u32>(march_reach));
}

fn march_texel(px: vec2<i32>) -> vec4<f32> {
    let v = textureLoad(cloud_march, px, 0);
    return vec4<f32>(unpack2x16float(v.x), unpack2x16float(v.y));
}

// ---- Mist ----------------------------------------------------------------------
// Low cloud banks (sky.rs `mist_amount`): mist lying in the hollows and over the
// water, deepest where the land dips below its surroundings (the smoothed
// `cloud_floor`), so a valley fills while the ridges either side stand out of it.
// Torn into banks by the billow texture drifting slowly on the wind, thickest
// from dusk to dawn. Thin: looking down from the strategic view through its few
// tens of metres the battle shows through it; a low eye looking along it sees banks.

// Extinction in the thickest bank, per metre.
const MIST_SIGMA: f32 = 0.003;
// Size of the banks: the billow texture's repeat, metres.
const MIST_PERIOD: f32 = 2300.0;
// How much deeper than `atmos.mist.y` a hollow's mist may pool, metres.
const MIST_POOL: f32 = 90.0;
// How much deeper autumn's banks lie than elsewhere, a share.
const AUTUMN_MIST_DEEPER: f32 = 0.25;
// Steps along a ray through the mist's slab, at most.
const MIST_STEPS: i32 = 40;

// The mist's extinction at `p`, per metre. Where it may lie and how much the
// ground holds come from the column map (cloud_shape.wgsl `bake_column`).
fn mist_at(p: vec3<f32>) -> f32 {
    let at = column_at(p.xy);
    if at.map < 0 {
        return 0.0;
    }
    let c5 = column_layer(at, 5);
    let land = c5.x;
    let fall = c5.z;
    // Autumn karst keeps a little mist in its valleys all day, wisps lying a
    // little deeper than elsewhere: the ground still shows through it.
    let amount = max(atmos.mist.x, 0.45 * fall);
    let depth = atmos.mist.y * (1.0 + AUTUMN_MIST_DEEPER * fall);
    // Above the highest a bank can stand here: nothing more to look up.
    if p.z >= max(land, globals.map.z) + depth * 1.5 || c5.w <= 0.0 {
        return 0.0;
    }
    let q = p.xy - atmos.wind.xy * 0.35;
    let n = textureSampleLevel(cloud_noise, repeat_sampler, vec3<f32>(turn(q, TURN_OCTAVE) / MIST_PERIOD, 0.37 + p.z / 900.0), 0.0);
    let bank = n.x * 0.75 + n.y * 0.25;
    // The bank's top rides up and down with it, so the banks are lumpy on top.
    let inside = c5.y + depth * (0.5 + bank) - p.z;
    if inside <= 0.0 {
        return 0.0;
    }
    // Banks with clear air between them: at the thickest a bank covers a third or
    // so of the low ground, not all of it.
    let dens = clamp((bank - (1.0 - 0.6 * amount)) / 0.25, 0.0, 1.0);
    // Soft on top, thickest low down.
    return dens * c5.w * smoothstep(0.0, 30.0, inside) * MIST_SIGMA;
}

struct Mist {
    light: vec3<f32>,
    through: f32,
    // How far along the ray the mist the march saw lies (0 for none).
    depth: f32,
}

// The mist along the ray from `eye`, as far as `t_scene`.
fn mist_march(eye: vec3<f32>, rd: vec3<f32>, t_scene: f32, jitter: f32) -> Mist {
    var out: Mist;
    out.through = 1.0;
    // (Autumn keeps its mist whatever the weather: `mist_at`.)
    let fall = select(0.0, 1.0, autumn());
    if atmos.mist.x <= 0.01 && fall <= 0.0 {
        return out;
    }
    // The slab the banks can lie in: from the sea or the lowest land to the
    // deepest a bank can stand over the highest of the cloud floor.
    let z0 = min(globals.map.z, atmos.frame.w) - 2.0;
    let z1 = max(globals.map.z, atmos.shape.w) + atmos.mist.y * (1.0 + AUTUMN_MIST_DEEPER * fall) * 1.5;
    var t0 = 0.0;
    var t1 = t_scene;
    if abs(rd.z) > 1e-5 {
        let a = (z0 - eye.z) / rd.z;
        let b = (z1 - eye.z) / rd.z;
        t0 = max(min(a, b), 0.0);
        t1 = min(max(a, b), t1);
    } else if eye.z > z1 || eye.z < z0 {
        return out;
    }
    // Banks further off than this are the haze's.
    t1 = min(t1, t0 + 12000.0);
    if t1 <= t0 {
        return out;
    }
    let steps = clamp((t1 - t0) / 40.0, 6.0, f32(MIST_STEPS));
    let dt = (t1 - t0) / steps;
    let sun = globals.sun.xyz;
    let mu = dot(rd, sun);
    // Light scattered many times in it: white, lit as the land under it is, by
    // the sun on its top and the sky. Then a glow looking toward the sun.
    let sun_light = atmos.sun_color.rgb * atmos.sun_color.w;
    let diffuse = (sun_light * max(sun.z, 0.0) / PI + atmos.sky_color.rgb) * 0.9;
    let glow = sun_light * phase_hg(mu, 0.6) * 1.5;
    var t = t0 + dt * jitter;
    var light = vec3<f32>(0.0);
    var through = 1.0;
    var depth_sum = 0.0;
    var weight_sum = 0.0;
    let bubble = eye_bubble();
    for (var i = 0; i < MIST_STEPS; i++) {
        if t >= t1 || through < 0.02 {
            break;
        }
        let p = eye + rd * t;
        var sigma = mist_at(p);
        if sigma > 0.0 {
            // Where the player looks through, thinned like the clouds.
            sigma *= mix(0.15, 1.0, clearing(p, bubble));
            // The sun's share is shaded by the clouds above.
            let shade = cloud_shadow(p);
            let lit = diffuse * mix(0.6, 1.0, shade) + glow * shade;
            let absorb = exp(-sigma * dt);
            let w = through * (1.0 - absorb);
            light += lit * w;
            depth_sum += t * w;
            weight_sum += w;
            through *= absorb;
        }
        t += dt;
    }
    if weight_sum > 0.0 {
        out.depth = depth_sum / weight_sum;
        let covered = 1.0 - through;
        light = apply_haze(light / max(covered, 1e-3), eye + rd * out.depth, eye) * covered;
    }
    out.light = light;
    out.through = through;
    return out;
}

// The march's clouds `cloud` with the mist laid in front of or behind them,
// whichever the ray meets first; `march_depth` becomes the two's mean depth.
fn with_mist(cloud: vec4<f32>, in: FullOut) -> vec4<f32> {
    let eye = globals.camera.xyz;
    let m = mist_march(eye, view_ray(in.uv), march_reach, ign(in.clip.xy + vec2<f32>(17.0, 31.0)));
    if m.through >= 1.0 {
        return cloud;
    }
    var out: vec4<f32>;
    if march_depth <= 0.0 || m.depth < march_depth {
        out = vec4<f32>(m.light + m.through * cloud.rgb, m.through * cloud.a);
    } else {
        out = vec4<f32>(cloud.rgb + cloud.a * m.light, cloud.a * m.through);
    }
    let cw = 1.0 - cloud.a;
    let mw = 1.0 - m.through;
    march_depth = (march_depth * cw + m.depth * mw) / max(cw + mw, 1e-4);
    return out;
}

// Nuclear fireballs light the cloud round them in their own colour: white-hot in the
// flash, then yellow and orange, sinking to a dull red as the fire goes out
// (`Atmosphere::fires`, sky/fires.rs). Their light on the cloud at `p`: from the ball's
// surface out, falling off over a few hundred metres (further for a bigger blast).
fn fire_glow(p: vec3<f32>) -> vec3<f32> {
    var glow = vec3<f32>(0.0);
    let count = min(u32(atmos.fire_count.x), CLOUD_FIRES_SLOTS);
    for (var i = 0u; i < count; i++) {
        let f = atmos.fires[i * 2u];
        let out = max(distance(p, f.xyz) - f.w * 0.85, 0.0);
        let reach = 200.0 + f.w * 0.5;
        glow += atmos.fires[i * 2u + 1u].rgb * exp(-out / reach);
    }
    return glow;
}

fn march(in: FullOut) -> vec4<f32> {
    // The farthest of the full-size depths at this texel's corners.
    let full = vec2<f32>(textureDimensions(scene_depth));
    let size = vec2<f32>(textureDimensions(cloud_history));
    // Full-size pixels per march texel.
    let scale = round(full / size);
    let px = vec2<i32>(floor(in.uv * full - scale * 0.5 + 0.5));
    let step = vec2<i32>(max(scale - 1.0, vec2<f32>(0.0)));
    let dims = vec2<i32>(full) - vec2<i32>(1);
    var depth = 1.0;
    for (var i = 0; i < 4; i++) {
        depth = min(depth, textureLoad(scene_depth, min(px + vec2<i32>(i & 1, i >> 1u) * step, dims), 0));
    }
    let eye = globals.camera.xyz;
    let rd = view_ray(in.uv);
    // A march texel's angle across. (Not `fwidth`: neighbouring fragments of the half
    // march stand for texels two apart, `fs_march`.)
    let across = view_ray(in.uv + vec2<f32>(1.0 / size.x, 0.0)) - rd;
    let down = view_ray(in.uv + vec2<f32>(0.0, 1.0 / size.y)) - rd;
    let texel_angle = max(length(abs(across) + abs(down)), 1e-6);
    var t_scene = 1.0e9;
    if depth > 0.0 {
        t_scene = distance(unproject(in.uv, depth), eye);
    }
    march_reach = t_scene;
    // Where the ray is inside the layer, from the lowest storm base to the highest top.
    let z0 = atmos.frame.w + atmos.layer.x - 300.0;
    let z1 = atmos.shape.w + atmos.layer.z + (atmos.layer.z - atmos.layer.x) * TOP_LUMP * 0.5;
    // At most 50 km of layer along the ray, counted from where it enters: the
    // strategic view over an 80 km map looks down from well over 100 km, and
    // a cap counted from the eye cut every ray off before it reached the layer.
    var t0 = 0.0;
    var t1 = 50000.0;
    if abs(rd.z) > 1e-5 {
        let a = (z0 - eye.z) / rd.z;
        let b = (z1 - eye.z) / rd.z;
        t0 = max(min(a, b), 0.0);
        t1 = min(max(a, b), t0 + t1);
    } else if eye.z < z0 || eye.z > z1 {
        return with_rain(vec4<f32>(0.0, 0.0, 0.0, 1.0), eye, rd, t_scene, in.clip.xy);
    }
    t1 = min(t1, t_scene);
    if t1 <= t0 {
        return with_rain(vec4<f32>(0.0, 0.0, 0.0, 1.0), eye, rd, t_scene, in.clip.xy);
    }

    let sun = globals.sun.xyz;
    let mu = dot(rd, sun);
    // Water droplets' scattering (`phase_mie`): the narrow bright peak round the
    // sun, the silver lining, and Draine's broad bulk with a little back-glow; the
    // light scattered again, softer.
    let phase = phase_mie(mu, CLOUD_DROPLET_UM, 1.0);
    let phase_soft = phase_mie(mu, CLOUD_DROPLET_UM, 0.5);
    let span = t1 - t0;
    // Steps grow with distance: fine where clouds are near and large on screen.
    // Not rounded: a whole number of steps would jump as the camera moves.
    // Looking down from the strategic view the ray crosses the layer steeply:
    // at 420 m a step a kilometre of cloud got three samples.
    let steps = clamp(span / mix(50.0, 100.0, smoothstep(2000.0, 30000.0, t0)), 16.0, 120.0);
    let dt = span / steps;
    // Inside cloud a finer stride, but never much finer than a march texel is
    // wide where the ray enters: from the strategic view a texel spans some
    // 170 m, and a quarter stride (25 m) spent dozens of fully lit samples a
    // texel crossing a deck's soft top, detail no one could see. Half the coarse
    // stride at the finest, and never over `MARCH_FINE`: a low eye looking out to
    // the horizon crosses tens of kilometres of layer, its coarse stride hundreds
    // of metres, and half that stepped over the erosion's lumps (tens of metres)
    // here on one frame and there on the next, every edge a spray of speckles.
    let fine_dt = clamp(t0 * texel_angle, min(dt * 0.5, MARCH_FINE), dt);
    let light_lod = smoothstep(25.0, 45.0, t0 * texel_angle);
    let jitter = ign(in.clip.xy);
    var t = t0 + dt * jitter;
    var through = 1.0;
    var light = vec3<f32>(0.0);
    var depth_sum = 0.0;
    var weight_sum = 0.0;
    let bubble = eye_bubble();
    // The erosion's smallest lumps (about a sixteenth of its period) kept to
    // two march texels or more where the ray enters the cloud.
    let detail = max(1.0, t0 * texel_angle * 32.0 / DETAIL_PERIOD);
    let flash_count = u32(atmos.counts.y);
    // Coarse steps through clear air; on finding cloud, back up and go on at the
    // fine stride until it has been clear for a while. At the coarse stride alone
    // a thin part was hit on some frames and missed on others, and the clouds'
    // edges crawled. Found is the cloud's body before the erosion carves it
    // (`CloudSample::body`), not only what is left: a fringe the erosion has worn
    // thinner than a coarse step lies inside the body, and stepping coarsely
    // through it found it on one frame and missed it the next.
    var fine = 0;
    for (var i = 0; i < 240; i++) {
        if t >= t1 || through < 0.015 {
            break;
        }
        // The fine stride follows the footprint where the ray is now, not only
        // where it entered: a ray grazing the layer at a low camera angle runs on
        // for kilometres, its texel ever wider, and spent most of its steps
        // there at the entry's stride.
        let stride = select(dt, max(fine_dt, min(t * texel_angle, dt)), fine > 0);
        let p = eye + rd * t;
        let at = column_at(p.xy);
        let c0 = column_base(at);
        // Open sky, or over or under the column's cloud: nothing more to look up.
        if c0.y <= 0.0 || p.z <= c0.x || p.z >= c0.x + c0.y * (1.0 + TOP_LUMP * 0.5) {
            fine = max(fine - 1, 0);
            t += stride;
            continue;
        }
        var c = cloud_shape(p, at, c0, detail, true);
        if (c.sigma > 0.0 || c.body > 0.0) && fine == 0 {
            fine = 8;
            t = max(t - dt * 0.75, t0);
            continue;
        }
        if c.sigma <= 0.0 {
            fine = select(max(fine - 1, 0), 8, c.body > 0.0);
        } else {
            fine = 8;
            // Where the player looks through, the cloud keeps its shape and its
            // light but lets the ground show: thinned, not removed.
            let keep = clearing(p, bubble);
            let sigma_full = c.sigma;
            c.sigma = sigma_full * mix(0.025, 1.0, keep * keep);
            // Light from the sun: through the coarse cloud toward it (`sun_through`,
            // mostly from the light volume), past a short tap with the fine erosion,
            // about one of its lumps toward the sun, so each lump shades the crease
            // behind it. Lit from the coarse cloud alone the cauliflower changed the
            // outline but never its own light: smooth cotton gradients. Not on a
            // deck's or a sheet's flat top, which it turned into one even fleece
            // from edge to edge; and faded out zoomed out, where a march texel is
            // tens of metres wide and the lumps are smaller than it.
            let short = DETAIL_PERIOD * detail * 0.06;
            let past = short * 2.0 * (1.0 - light_lod);
            var sunward = sun_through(p + sun * past, through > 0.3);
            // Deep in the cloud, where hardly any sunlight comes straight through, or
            // behind enough of it that little of what is lit here shows, the crease
            // would darken nothing anyone could see: not looked up.
            if light_lod < 1.0 && sunward.y > 0.05 && through > 0.4 {
                let fade = (1.0 - light_lod) * (1.0 - 0.8 * c.flat);
                let crease = cloud_shape(p + sun * short, at, c0, detail, false).sigma * short * 2.0 * fade;
                sunward *= exp(-crease * LIGHT_OCTAVES);
            }
            // Beer's law with the light scattered many times approximated by
            // ever-softer octaves (Hillaire 2016), and the dark "powder" rims.
            let sunlit = sunward.x * (phase + mie_peak(CLOUD_DROPLET_UM) * phase_soft) + sunward.y * 0.3 * phase_soft + sunward.z * 0.25 * 0.0796;
            // Powder darkens thin cloud only looking toward the sun (its
            // in-scatter has not built up yet there). From above, with the sun
            // behind the eye, it greyed every rim and softened the outline.
            let powder = mix(1.0, 1.0 - exp(-c.sigma * 90.0), smoothstep(-0.2, 0.5, mu));
            let direct = atmos.sun_color.rgb * sunlit * mix(0.55, 1.0, powder) * 2.6 * mix(1.0, atmos.sun_color.w, 0.8);
            // Sky light from above, dimmer low in the cloud and inside storms;
            // the land's light from below on the base.
            // The column above hides the sky from a point low in the cloud: a
            // small cloud's base stays bright, a big heap's belly goes grey.
            let hidden = exp(-c.above * 0.0028 * mix(0.35, 1.0, c.firm));
            let sky = atmos.sky_color.rgb * mix(0.3, 1.15, hidden) * mix(0.55, 1.0, c.height) * (1.0 - c.storm * 0.55)
                + atmos.ground_color.rgb * 0.35 * (1.0 - c.height);
            let albedo = 1.0 - c.grey;
            var glow = vec3<f32>(0.0);
            for (var f = 0u; f < flash_count; f++) {
                let fl = atmos.flashes[f];
                let r = distance(p, fl.xyz);
                glow += vec3<f32>(0.62, 0.7, 1.0) * fl.w * 26.0 * exp(-r / 520.0);
            }
            if atmos.fire_count.x > 0.0 {
                glow += fire_glow(p);
            }
            // Shell bursts, flak and explosions under or in the cloud (sky.rs `set_glows`).
            for (var g = 0u; g < 8u; g++) {
                let col = atmos.glows[g * 2u + 1u];
                if col.w <= 0.0 {
                    break;
                }
                let at = atmos.glows[g * 2u];
                let d = p - at.xyz;
                // Inverse-square from a soft core, cut off a few cores out so a flash
                // lights the cloud over it, not the whole deck.
                let d2 = dot(d, d);
                glow += col.rgb / (1.0 + d2 / (at.w * at.w)) * exp(-sqrt(d2) / (at.w * 6.0));
            }
            let source = ((direct + sky) * albedo + glow) * c.sigma;
            let absorb = exp(-c.sigma * stride);
            light += through * source * (1.0 - absorb) / c.sigma;
            let w = through * (1.0 - absorb);
            depth_sum += t * w;
            weight_sum += w;
            through *= absorb;
        }
        t += stride;
    }
    if weight_sum > 0.0 {
        // Air between the eye and the cloud: far clouds fade into the sky.
        march_depth = depth_sum / weight_sum;
        let at = eye + rd * march_depth;
        let covered = 1.0 - through;
        let hazed = apply_haze(light / max(covered, 1e-3), at, eye);
        light = hazed * covered;
    }
    // Checked before the cap: NVIDIA's min() turns a NaN into the 40, a white texel.
    if !finite4(vec4<f32>(light, through)) {
        light = vec3<f32>(0.0);
        through = 1.0;
    }
    return with_rain(vec4<f32>(min(light, vec3<f32>(40.0)), through), eye, rd, t_scene, in.clip.xy);
}

// Rain under the clouds: grey veils hanging from storm bases and heavy decks
// (the weather map's rain, `w`), streaked and falling. `cloud` is what the
// march found above; the rain goes behind it seen from above, in front of it
// seen from beneath.
fn with_rain(cloud: vec4<f32>, eye: vec3<f32>, rd: vec3<f32>, t_scene: f32, pixel: vec2<f32>) -> vec4<f32> {
    if atmos.regions.w <= 0.0 {
        return cloud;
    }
    // The highest the rain can hang from; each sample checks its own floor.
    let top = atmos.shape.w + atmos.layer.x - 60.0;
    var ta = 0.0;
    var tb = min(t_scene, 30000.0);
    if eye.z > top {
        if rd.z > -1e-4 {
            return cloud;
        }
        ta = (top - eye.z) / rd.z;
    } else if rd.z > 1e-4 {
        tb = min(tb, (top - eye.z) / rd.z);
    }
    if tb <= ta {
        return cloud;
    }
    // Nothing falling anywhere along it: done.
    let r0 = column_layer(column_at((eye + rd * ta).xy), 4).z;
    let r1 = column_layer(column_at((eye + rd * tb).xy), 4).z;
    let rm = column_layer(column_at((eye + rd * (ta + tb) * 0.5).xy), 4).z;
    if max(r0, max(r1, rm)) < 0.01 {
        return cloud;
    }
    let steps = 8.0;
    let dt = (tb - ta) / steps;
    var t = ta + dt * ign(pixel + vec2<f32>(37.0, 11.0));
    var through = 1.0;
    var light = vec3<f32>(0.0);
    let time = atmos.weather.w;
    for (var i = 0; i < 8; i++) {
        let p = eye + rd * t;
        let at = column_at(p.xy);
        let c4 = column_layer(at, 4);
        let r = select(0.0, c4.z, at.map >= 0);
        if r > 0.005 {
            // Shafts: coarse noise in plan, stretched tall, drifting down.
            let q = p.xy - atmos.wind.xy * 1.1;
            let streak = textureSampleLevel(cloud_noise, repeat_sampler,
                vec3<f32>(q / 260.0, (p.z + time * 55.0) / 1800.0), 0.0).z;
            let under = column_layer(at, 5).x + atmos.layer.x - 60.0;
            let hang = smoothstep(under, under - 250.0, p.z);
            var sigma = r * 0.00055 * (0.35 + 1.3 * streak) * hang;
            // Lit grey by the sky, a little by the sun through the cloud above.
            var source = atmos.sky_color.rgb * 1.1 + atmos.sun_color.rgb * 0.035;
            let cold = c4.w;
            if cold > 0.0 {
                // Polar snow comes down in thicker, whiter veils than rain.
                sigma *= 1.0 + 0.6 * cold;
                source = mix(source, atmos.sky_color.rgb * 1.6 + atmos.sun_color.rgb * 0.08, cold);
            }
            let absorb = exp(-sigma * dt);
            light += through * source * (1.0 - absorb);
            through *= absorb;
        }
        t += dt;
    }
    if eye.z > top {
        return vec4<f32>(cloud.rgb + cloud.a * light, cloud.a * through);
    }
    return vec4<f32>(light + through * cloud.rgb, through * cloud.a);
}

// ---- Falling rain, close up -------------------------------------------------------
// Streaks where the weather map says it rains, in three nested layers of tiles
// fixed to the ground (small ones close up, big ones further out), each drop
// wrapping round its tile as the camera moves, so the rain stays put in the
// world and pans past like everything else. The layer that suits the zoom is
// drawn; its neighbours fade in and out as the camera goes in and out.
// Drawn after the clouds: from under the deck every drop is nearer than any
// cloud, so the clouds only cover the drops when the eye is up at or over
// the deck's base, looking down through it.

const RAIN_DROPS: u32 = 15000u;
const RAIN_LAYERS: u32 = 3u;

struct RainOut {
    @builtin(position) clip: vec4<f32>,
    // x along the streak (0 head, 1 tail), y across it (-1 to 1), z strength,
    // w how far the clouds on screen lie between the eye and the drop (0 to 1).
    @location(0) streak: vec4<f32>,
    // 1 for a snowflake (`snowfall_at`): streak.xy then run across a round flake.
    @location(1) snow: f32,
}

@vertex
fn vs_rain(@builtin(vertex_index) vertex: u32, @builtin(instance_index) drop: u32) -> RainOut {
    var out: RainOut;
    out.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
    if atmos.regions.w <= 0.0 {
        return out;
    }
    let eye = globals.camera.xyz;
    let focus = vec3<f32>(atmos.view.xy, terrain_height(atmos.view.xy));
    let dist = distance(eye, focus);
    // Tile size of this drop's layer: 60, 240, 960 m.
    let layer = drop % RAIN_LAYERS;
    let span = 60.0 * exp2(2.0 * f32(layer));
    // Each layer is best when its tile is about as wide as the view is far;
    // the next one in takes over well before its small tile shows as a patch.
    let fit = log2(span / max(dist * 0.9, 1.0)) * 0.5;
    var weight = exp(-fit * fit * 6.0);
    if layer == 0u {
        weight = select(weight, 1.0, fit > 0.0);
    }
    weight *= 1.0 - smoothstep(2800.0, 4500.0, dist);
    if weight < 0.02 {
        return out;
    }
    let id = f32(drop / RAIN_LAYERS) + f32(layer) * 7919.0;
    let h = vec4<f32>(hash11(id * 1.37 + 0.1), hash11(id * 2.71 + 0.3), hash11(id * 0.93 + 0.7), hash11(id * 3.17 + 0.9));
    // Where in its tile, taken round to a tile centred a little toward the
    // camera from the focus: the ground at the bottom of the screen is nearer
    // the eye than the focus is, and the drops that cover it hang nearer still.
    let toward = eye.xy - focus.xy;
    let centre = focus.xy + toward * min(0.35, span * 0.3 / max(length(toward), 1.0));
    let local = fract(vec2<f32>(h.x, h.y) - centre / span) - 0.5;
    var xy = centre + local * span;
    // Falls about its column's height in a second and a half, whatever the layer,
    // so on screen close and far rain fall alike: brisk, not drifting.
    let column = span * 0.75;
    var fall = column / mix(1.2, 1.8, h.w);
    var slant = atmos.wind.zw * 0.022;
    // Where it is cold enough it falls as snow (`snowfall_at`; where rain and snow
    // country meet, some of each): a flake comes down a fifth as fast as a drop,
    // carried further by the wind and wandering as it goes.
    let snow = snowfall_at(xy) > h.y;
    if snow {
        fall *= 0.2;
        slant *= 3.0;
        // In a polar storm it is a blizzard: the wind drives the snow almost flat.
        slant *= 1.0 + 1.6 * polar_within(xy, REGIONS_SKY_BLEND_M) * clamp(weather_at(xy).y, 0.0, 1.0);
    }
    let phase = fract(h.z - atmos.weather.w * fall / column);
    let ground = terrain_height(xy);
    // Phase runs down as the drop falls, so it is carried downwind as it goes.
    xy -= slant * column * (phase - 0.5);
    if snow {
        let t = atmos.weather.w;
        xy += vec2<f32>(sin(t * 0.9 + h.z * 40.0), cos(t * 0.7 + h.w * 40.0)) * span * 0.012;
    }
    let z = ground + phase * column;
    let rain = weather_at(xy).w;
    // Light rain is fewer drops, not fainter ones.
    if rain < 0.02 || h.x * 0.85 > rain * 1.3 + 0.08 {
        return out;
    }
    let edge = 1.0 - smoothstep(0.36, 0.5, max(abs(local.x), abs(local.y)));
    let head = vec3<f32>(xy, z);
    let vel = normalize(vec3<f32>(slant, -1.0));
    // A camera exposure's worth of motion.
    let tail = head - vel * fall * 0.04;
    let corner = vertex % 6u;
    let along = select(0.0, 1.0, corner == 1u || corner == 2u || corner == 4u);
    let side = select(-1.0, 1.0, corner == 2u || corner == 4u || corner == 5u);
    // A thin quad from head to tail, a pixel and a bit wide on screen.
    let ch = globals.view_proj * vec4<f32>(head, 1.0);
    let ct = globals.view_proj * vec4<f32>(tail, 1.0);
    if ch.w <= 0.1 || ct.w <= 0.1 {
        return out;
    }
    let sh = ch.xy / ch.w * globals.viewport.xy;
    let st = ct.xy / ct.w * globals.viewport.xy;
    let dir = normalize(st - sh + vec2<f32>(0.0, 1e-3));
    let c = mix(ch, ct, along);
    let px = vec2<f32>(-dir.y, dir.x) * side * 0.75;
    out.clip = vec4<f32>(c.xy + px * 2.0 * globals.viewport.zw * c.w, c.zw);
    // Under the deck's base the drop is in front of every cloud; up in it or over it,
    // the eye looks down through the clouds to the drop.
    let base = cloud_floor(eye.xy) + atmos.layer.x - 60.0;
    let behind = smoothstep(base - 60.0, base + 120.0, eye.z);
    out.streak = vec4<f32>(along, side, min(rain * 1.6, 1.0) * mix(0.55, 1.0, h.y) * weight * edge, behind);
    if snow {
        // A flake: a round dot a few pixels across, not a streak.
        let size = mix(1.3, 2.4, h.w);
        let corner_px = vec2<f32>(side, along * 2.0 - 1.0) * size;
        out.clip = vec4<f32>(ch.xy + corner_px * 2.0 * globals.viewport.zw * ch.w, ch.zw);
        out.snow = 1.0;
    }
    return out;
}

@fragment
fn fs_rain(in: RainOut) -> @location(0) vec4<f32> {
    if globals.camera.z < globals.map.z {
        discard;
    }
    let edge = 1.0 - abs(in.streak.y);
    let fade = smoothstep(0.0, 0.15, in.streak.x) * (1.0 - smoothstep(0.55, 1.0, in.streak.x));
    var a = edge * fade * in.streak.z * 0.42;
    var color = atmos.sky_color.rgb * 1.8 + atmos.sun_color.rgb * 0.08;
    if in.snow > 0.5 {
        // Soft, round and whiter than rain's grey streak.
        let r = length(vec2<f32>(in.streak.x * 2.0 - 1.0, in.streak.y));
        a = (1.0 - smoothstep(0.3, 1.0, r)) * in.streak.z * 0.7;
        color = atmos.sky_color.rgb * 2.4 + atmos.sun_color.rgb * 0.3;
    }
    if in.streak.w > 0.0 {
        let uv = in.clip.xy / vec2<f32>(textureDimensions(scene_depth));
        let cover = cloud_cover(uv, textureSampleLevel(cloud_now, clamp_sampler, uv, 0.0).a);
        a *= 1.0 - cover * in.streak.w;
    }
    return vec4<f32>(color * a, a);
}

// ---- Carried over from frame to frame --------------------------------------------

// A cubic of the Mitchell-Netravali family (B, C) from nine texels in five bilinear
// taps (the corners are dropped: they weigh next to nothing). Negative lobes can
// overshoot, so the caller clamps.
fn sample_cubic(tex: texture_2d<f32>, uv: vec2<f32>, b: f32, c: f32) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(tex));
    let at = uv * size;
    let centre = floor(at - 0.5) + 0.5;
    let f = at - centre;
    // The kernel at 1 + f, f, 1 - f and 2 - f texels out.
    let near = vec3<f32>(12.0 - 9.0 * b - 6.0 * c, -18.0 + 12.0 * b + 6.0 * c, 6.0 - 2.0 * b) / 6.0;
    let far = vec4<f32>(-b - 6.0 * c, 6.0 * b + 30.0 * c, -12.0 * b - 48.0 * c, 8.0 * b + 24.0 * c) / 6.0;
    let x0 = 1.0 + f;
    let x2 = 1.0 - f;
    let x3 = 2.0 - f;
    let w0 = ((far.x * x0 + far.y) * x0 + far.z) * x0 + far.w;
    let w1 = (near.x * f + near.y) * f * f + near.z;
    let w2 = (near.x * x2 + near.y) * x2 * x2 + near.z;
    let w3 = ((far.x * x3 + far.y) * x3 + far.z) * x3 + far.w;
    let w12 = w1 + w2;
    let o12 = w2 / w12;
    let p0 = (centre - 1.0) / size;
    let p3 = (centre + 2.0) / size;
    let p12 = (centre + o12) / size;
    var sum = textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p0.y), 0.0) * (w12.x * w0.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p0.x, p12.y), 0.0) * (w0.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p12.y), 0.0) * (w12.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p3.x, p12.y), 0.0) * (w3.x * w12.y);
    sum += textureSampleLevel(tex, clamp_sampler, vec2<f32>(p12.x, p3.y), 0.0) * (w12.x * w3.y);
    let weight = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
    return sum / weight;
}

@fragment
fn fs_resolve(in: FullOut) -> @location(0) vec4<f32> {
    let px = vec2<i32>(in.clip.xy);
    // Marched this frame (`marched_now`), or filled in from the marched round it.
    let marched = marched_now(px);
    let last = vec2<i32>(textureDimensions(cloud_march)) - vec2<i32>(1);
    let now = march_texel(px);
    if atmos.frame.y < 0.5 {
        return now;
    }
    // Where this texel's cloud was on screen last frame: back along the ray
    // as far as the cloud it saw. One plane for every texel (the middle of the
    // layer) put everything above or below it in the wrong place as the view
    // panned, the clamp threw that history away, and the clouds went back to
    // one frame's grain and crawled. Clear texels use the middle of the layer.
    let eye = globals.camera.xyz;
    let d = view_ray(in.uv);
    var t = bitcast<f32>(textureLoad(cloud_march, px, 0).z);
    if t <= 0.0 {
        let mid = cloud_floor(eye.xy + d.xy * 1000.0) + mix(atmos.layer.x, atmos.layer.y, 0.5);
        t = 20000.0;
        if abs(d.z) > 1e-4 && (mid - eye.z) / d.z > 0.0 {
            t = (mid - eye.z) / d.z;
        }
    }
    let clip = atmos.prev_view_proj * vec4<f32>(eye + d * t, 1.0);
    if clip.w <= 0.0 {
        return now;
    }
    let uv = vec2<f32>(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return now;
    }
    // Keep the history inside what the neighbourhood holds now (its mean give
    // or take its spread), so a cloud that moved or a view that swung leaves
    // no ghost or streak behind.
    // Only neighbours that marched about as far: a swaying treetop's texel,
    // clamped among the sky's, kept the sky's cloud and rain from the frames
    // the tree leant out of it, a grey halo round the crown.
    let reach = bitcast<f32>(textureLoad(cloud_march, px, 0).w);
    var sum = vec4<f32>(0.0);
    var sq = vec4<f32>(0.0);
    var lo = now;
    var hi = now;
    var n = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let at = clamp(px + vec2<i32>(x, y), vec2<i32>(0), last);
            let there = bitcast<f32>(textureLoad(cloud_march, at, 0).w);
            if abs(log2(max(there, 0.1) / max(reach, 0.1))) > 0.3 {
                continue;
            }
            let c = march_texel(at);
            sum += c;
            sq += c * c;
            lo = min(lo, c);
            hi = max(hi, c);
            n += 1.0;
        }
    }
    if !finite4(sum) || !finite4(sq) {
        return select(vec4<f32>(0.0, 0.0, 0.0, 1.0), now, finite4(now));
    }
    let mean = sum / n;
    let spread = sqrt(max(sq / n - mean * mean, vec4<f32>(0.0)));
    // With the view still, the history is the better estimate: clip it only
    // loosely (tight clipping to a noisy neighbourhood re-injects the noise)
    // and average over more frames. While it moves, clip harder and lean a
    // little more on the new frame, or history resampled every frame blurs
    // and trails; carried by its own depth it lands true, so not so hard that
    // panning shows one frame's grain.
    let moved = length((uv - in.uv) * vec2<f32>(textureDimensions(cloud_march)));
    let motion = smoothstep(0.3, 3.0, moved);
    let k = mix(3.0, 2.0, motion);
    // Read back without softening it: a still view (whose reprojection is off
    // by float error only) takes its own texel, a moving one a Catmull-Rom
    // sample, since bilinear history resampled every frame softens as it pans.
    var history: vec4<f32>;
    if moved < 0.25 {
        history = textureLoad(cloud_history, px, 0);
    } else {
        history = sample_cubic(cloud_history, uv, 0.0, 0.5);
    }
    // Never carry a bad value on: it spreads a texel a frame through the
    // neighbourhood clamp and the sharp resample, and whites out the view.
    if !finite4(history) || !finite4(now) {
        return select(vec4<f32>(0.0, 0.0, 0.0, 1.0), now, finite4(now));
    }
    let before = clamp(
        history,
        mix(mean - spread * k, max(lo, mean - spread * k), motion),
        mix(mean + spread * k, min(hi, mean + spread * k), motion),
    );
    // A filled texel brings nothing new (it is its neighbours' blend): it keeps its
    // history, clamped as above; a marched one counts for every frame it was not, so
    // the clouds keep the pace they settle and follow at.
    let gain = f32(MARCH_STRIDE * MARCH_STRIDE);
    return mix(before, now, min(mix(0.06, 0.14, motion) * select(0.0, gain, marched), 1.0));
}

// The march at full size (`cloud_march`, read by the resolve, the composite and
// `past_clouds`): this frame's marched texels from `cloud_march_part`, and between
// them a blend of the four marched round each, weighted by how near, from those
// that went about as far (as the resolve's neighbourhood does), so a treetop's
// texel is not filled with the sky's.
@fragment
fn fs_fill(in: FullOut) -> @location(0) vec4<u32> {
    let px = vec2<i32>(in.clip.xy);
    let last = vec2<i32>(textureDimensions(cloud_march_part)) - vec2<i32>(1);
    // Where this texel sits among the blocks' marched texels: the one up and to
    // the left (part coordinates), and how far on toward the next, 0 to 1.
    let rel = px - march_offset() + MARCH_STRIDE;
    let cell = rel / MARCH_STRIDE - vec2<i32>(1);
    let frac = vec2<f32>(rel % MARCH_STRIDE) / f32(MARCH_STRIDE);
    if all(frac == vec2<f32>(0.0)) {
        return textureLoad(cloud_march_part, clamp(cell, vec2<i32>(0), last), 0);
    }
    // How far this texel's march would have gone: to the nearest scene at its
    // corners, as `march` finds it.
    let full = vec2<f32>(textureDimensions(scene_depth));
    let size = vec2<f32>(textureDimensions(cloud_history));
    let scale = round(full / size);
    let corner = vec2<i32>(floor(in.uv * full - scale * 0.5 + 0.5));
    let step = vec2<i32>(max(scale - 1.0, vec2<f32>(0.0)));
    let dims = vec2<i32>(full) - vec2<i32>(1);
    var depth = 1.0;
    for (var i = 0; i < 4; i++) {
        depth = min(depth, textureLoad(scene_depth, min(corner + vec2<i32>(i & 1, i >> 1u) * step, dims), 0));
    }
    var reach = 1.0e9;
    if depth > 0.0 {
        reach = distance(unproject(in.uv, depth), globals.camera.xyz);
    }
    var sum = vec4<f32>(0.0);
    var depth_sum = 0.0;
    var depth_weight = 0.0;
    var weight = 0.0;
    var nearest = vec4<u32>(0u);
    var apart = 1.0e9;
    for (var i = 0; i < 4; i++) {
        let side = vec2<i32>(i & 1, i >> 1u);
        let q = cell + side;
        if any(q < vec2<i32>(0)) || any(q > last) {
            continue;
        }
        let v = textureLoad(cloud_march_part, q, 0);
        let off = abs(log2(max(bitcast<f32>(v.w), 0.1) / max(reach, 0.1)));
        if off < apart {
            apart = off;
            nearest = v;
        }
        let k = mix(1.0 - frac, frac, vec2<f32>(side));
        let w = k.x * k.y;
        if off > 0.3 || w <= 0.0 {
            continue;
        }
        sum += vec4<f32>(unpack2x16float(v.x), unpack2x16float(v.y)) * w;
        let d = bitcast<f32>(v.z);
        if d > 0.0 {
            depth_sum += d * w;
            depth_weight += w;
        }
        weight += w;
    }
    if weight <= 0.0 {
        return vec4<u32>(nearest.xyz, bitcast<u32>(reach));
    }
    let c = sum / weight;
    let d = select(0.0, depth_sum / max(depth_weight, 1e-6), depth_weight > weight * 0.5);
    return vec4<u32>(pack2x16float(c.rg), pack2x16float(c.ba), bitcast<u32>(d), bitcast<u32>(reach));
}

// `cloud_reach`: over the five by five march texels round each, which hold every
// texel the composite's cubic reads for a pixel nearest that one.
@compute @workgroup_size(8, 8)
fn cs_reach(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(cloud_march);
    if any(id.xy >= size) {
        return;
    }
    let last = vec2<i32>(size) - vec2<i32>(1);
    var near = 1.0e30;
    var far = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let at = clamp(vec2<i32>(id.xy) + vec2<i32>(x, y), vec2<i32>(0), last);
            let there = bitcast<f32>(textureLoad(cloud_march, at, 0).w);
            near = min(near, there);
            far = max(far, there);
        }
    }
    textureStore(cloud_reach_out, vec2<i32>(id.xy), vec4<f32>(near, far, 0.0, 0.0));
}

// ---- Laid over the picture ------------------------------------------------------

// A stroke's kinks, `k` of `n` along it: the same shape for its whole life.
fn bolt_point(top: vec3<f32>, ground: vec3<f32>, seed: f32, k: i32, n: i32) -> vec3<f32> {
    let f = f32(k) / f32(n);
    var p = mix(top, ground, f);
    if k > 0 && k < n {
        let len = distance(top, ground);
        let jag = vec2<f32>(hash11(seed + f32(k) * 1.37) - 0.5, hash11(seed * 1.9 + f32(k) * 2.11) - 0.5);
        p += vec3<f32>(jag * len * 0.16 * sin(f * PI), 0.0);
    }
    return p;
}

fn to_screen(p: vec3<f32>) -> vec3<f32> {
    let c = globals.view_proj * vec4<f32>(p, 1.0);
    let w = max(c.w, 1e-3);
    return vec3<f32>((c.x / w * 0.5 + 0.5) * globals.viewport.x, (0.5 - c.y / w * 0.5) * globals.viewport.y, c.z / w);
}

// `upsample` where every texel round the pixel marched as far as it did: a cubic
// between a B-spline and Catmull-Rom (Mitchell-Netravali B 0.4, C 0.3), kept inside
// the four texels round the pixel so its lobes cannot ring. The B-spline alone
// blurred a texel and a half on top of the march's own coarse size, every cloud's
// edge soft as cotton wool; Catmull-Rom alone drew a lone dark texel in a storm as
// a square. `centre` is the texel left of and above the pixel.
fn upsample_whole(uv: vec2<f32>, centre: vec2<f32>, last: vec2<i32>) -> vec4<f32> {
    var lo = textureLoad(cloud_now, clamp(vec2<i32>(centre), vec2<i32>(0), last), 0);
    var hi = lo;
    for (var i = 1; i < 4; i++) {
        let c = textureLoad(cloud_now, clamp(vec2<i32>(centre) + vec2<i32>(i & 1, i >> 1u), vec2<i32>(0), last), 0);
        lo = min(lo, c);
        hi = max(hi, c);
    }
    return clamp(sample_cubic(cloud_now, uv, 0.4, 0.3), lo, hi);
}

// The accumulated clouds at a full-size pixel whose scene depth is `depth`. The
// march texels are several pixels wide, and one straddling a treetop and the
// sky marched to the sky (the farthest of its corners): its cloud and rain,
// spread by the cubic filter, greyed the leaves beside it and blurred every
// crown against the sky. Where the texels round it went as far as the pixel,
// a cubic (`upsample_whole`); across a jump, each of the four nearest counts only
// as far as it marched to this pixel's own depth, and where none did, the nearest
// wins (none at all for a pixel nearer than all of them).
fn upsample(uv: vec2<f32>, depth: f32) -> vec4<f32> {
    let size = vec2<f32>(textureDimensions(cloud_now));
    let at = uv * size - 0.5;
    let centre = floor(at);
    var reach = 1.0e9;
    if depth > 0.0 {
        reach = distance(unproject(uv, depth), globals.camera.xyz);
    }
    let last = vec2<i32>(size) - vec2<i32>(1);
    // All of a piece (`cloud_reach`: every texel round went within a ninth of an
    // octave of the pixel's own reach, as the test below would find texel by texel).
    let range = textureLoad(cloud_reach, clamp(vec2<i32>(uv * size), vec2<i32>(0), last), 0).xy;
    let spread = abs(log2(max(range, vec2<f32>(0.1)) / max(reach, 0.1)));
    if max(spread.x, spread.y) < 0.11 {
        return upsample_whole(uv, centre, last);
    }
    // Across a jump in depth: the four texels round the pixel, blended by how near
    // each is, each counting only as far as it marched to this pixel's own depth.
    // (A cubic over sixteen here cost a third of the whole composite: every pixel
    // in a block of them along the horizon waited on it.)
    let f = at - centre;
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    var nearest = 1.0e30;
    var nearest_at = vec2<i32>(0);
    for (var i = 0; i < 4; i++) {
        let k = vec2<i32>(i & 1, i >> 1u);
        let px = clamp(vec2<i32>(centre) + k, vec2<i32>(0), last);
        let there = bitcast<f32>(textureLoad(cloud_march, px, 0).w);
        // Log distance apart: a quarter of the way nearer or farther halves it.
        let apart = abs(log2(max(there, 0.1) / max(reach, 0.1)));
        if apart < nearest {
            nearest = apart;
            nearest_at = px;
        }
        let side = mix(1.0 - f, f, vec2<f32>(k));
        let w = side.x * side.y * exp2(-apart * apart * 12.0);
        sum += textureLoad(cloud_now, px, 0) * w;
        weight += w;
    }
    if weight < 1e-3 {
        // Nearer than every texel's march (a twig against the sky): the
        // cloud they saw is all behind it.
        if reach < bitcast<f32>(textureLoad(cloud_march, nearest_at, 0).w) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        return textureLoad(cloud_now, nearest_at, 0);
    }
    return sum / weight;
}

@fragment
fn fs_composite(in: FullOut) -> @location(0) vec4<f32> {
    // From under the sea the sky shows only through the surface (water.wgsl `under_sea`).
    if globals.camera.z < globals.map.z {
        discard;
    }
    // The accumulated clouds, whose texels are several pixels wide, read through a
    // cubic (`upsample`): smooth across texels, so a ragged edge is not a row of
    // squares (a small tent showed each texel as one at low camera angles).
    let texel = 1.0 / vec2<f32>(textureDimensions(cloud_now));
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0);
    var cloud = upsample(in.uv, scene);
    var light = cloud.rgb;
    var cover = 1.0 - cloud.a;

    let ray = screen_ray(in.uv);
    var holo = cloud_holo(in.uv, ray);
    // The hologram sees through the cloud deck between the eye and the battle,
    // not the mist lying on the ground under it: where what the march met lies
    // low over the land, well under the deck's base, it is mist, and the
    // hologram leaves it be (else every tower standing out of it, and the
    // edge of every bank, was drawn in its cyan rim).
    if holo > 0.0 {
        let at_texel = clamp(vec2<i32>(in.uv * vec2<f32>(textureDimensions(cloud_march))), vec2<i32>(0),
            vec2<i32>(textureDimensions(cloud_march)) - vec2<i32>(1));
        let met = bitcast<f32>(textureLoad(cloud_march, at_texel, 0).z);
        if met > 0.0 {
            let at = globals.camera.xyz + ray * met;
            let over = at.z - cloud_floor(at.xy);
            holo *= smoothstep(0.35 * atmos.layer.x, 0.7 * atmos.layer.x, over);
        }
    }
    let e = texel * 3.0;
    let ax = textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(e.x, 0.0), 0.0).a
        - textureSampleLevel(cloud_now, clamp_sampler, in.uv - vec2<f32>(e.x, 0.0), 0.0).a;
    let ay = textureSampleLevel(cloud_now, clamp_sampler, in.uv + vec2<f32>(0.0, e.y), 0.0).a
        - textureSampleLevel(cloud_now, clamp_sampler, in.uv - vec2<f32>(0.0, e.y), 0.0).a;
    let rim = smoothstep(0.1, 0.45, abs(ax) + abs(ay));
    let level = cover * 5.0;
    let band = abs(fract(level + 0.5) - 0.5);
    // Soft, a few pixels wide: a hairline would crawl as the cloud drifts.
    let px = max(fwidth(level), 1e-4);
    let contour = (1.0 - smoothstep(px * 0.5, px * 3.0, band)) * smoothstep(0.05, 0.2, cover);
    let veil = cloud_battle_veil(ray);
    cover *= veil;
    light *= veil;
    if holo > 0.0 {
        let cyan = vec3<f32>(0.35, 0.85, 1.0);
        let ghost = light * 0.14 + cyan * (rim * 0.6 + contour * 0.2) * 0.45;
        light = mix(light, ghost, holo);
        cover = mix(cover, cover * 0.14, holo);
    }
    return vec4<f32>(light, cover);
}

// Lightning strokes over the picture, added to it (drawn only while a flash lasts,
// sky.rs `draw_composite`). In the composite itself the strokes' loop, never run
// without a flash, still held so many registers that the composite ran at a third
// of the pace.
@fragment
fn fs_bolts(in: FullOut) -> @location(0) vec4<f32> {
    if globals.camera.z < globals.map.z {
        discard;
    }
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0);
    let cloud = textureSampleLevel(cloud_now, clamp_sampler, in.uv, 0.0);
    var light = vec3<f32>(0.0);
    // Output pixels, as `to_screen` gives, so a bolt is as thin at any render scale.
    let pixel = in.clip.xy / globals.scene.z;
    let flash_count = u32(atmos.counts.y);
    for (var b = 0u; b < flash_count; b++) {
        let bolt = atmos.bolts[b];
        if bolt.w <= 0.0 {
            continue;
        }
        // From the cloud's base under the flash down to the ground it strikes.
        let ground = vec3<f32>(bolt.xy, terrain_height(bolt.xy));
        let top = vec3<f32>(atmos.flashes[b].xy, cloud_floor(atmos.flashes[b].xy) + atmos.layer.x);
        var near = 1.0e6;
        var at_z = 0.0;
        var prev = to_screen(bolt_point(top, ground, bolt.z, 0, 14));
        for (var k = 1; k <= 14; k++) {
            let next = to_screen(bolt_point(top, ground, bolt.z, k, 14));
            let seg = next.xy - prev.xy;
            let f = clamp(dot(pixel - prev.xy, seg) / max(dot(seg, seg), 1e-4), 0.0, 1.0);
            let d = distance(pixel, prev.xy + seg * f);
            if d < near && prev.z > 0.0 && next.z > 0.0 {
                near = d;
                at_z = mix(prev.z, next.z, f);
            }
            prev = next;
        }
        // Hidden behind nearer ground or hulls.
        if at_z + 1e-7 < scene {
            continue;
        }
        let core = exp(-near * near / 2.2);
        let halo = exp(-near / 14.0) * 0.12 + exp(-near / 90.0) * 0.02;
        light += vec3<f32>(0.8, 0.86, 1.0) * bolt.w * (core * 26.0 + halo * 4.0) * mix(0.35, 1.0, cloud.a);
    }
    return vec4<f32>(light, 0.0);
}
