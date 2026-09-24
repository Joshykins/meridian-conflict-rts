// The weather over the map (sky.rs): a cloud-cover field the wind carries,
// with storms, and wakes where aircraft, blasts and big hulls stir it.
//
// Two passes a frame over the whole map:
//   cs_advect  (state, flow) -> (state', flow')  carried along by the wind and
//              by whatever was pushed into motion
//   cs_force   (state', flow') -> (state, flow)  drawn back toward the air mass
//              and its storms, then disturbed
// `state`: x cloud cover, y storm, z churn (turbulence), w rain (0-1).
// `flow`: xy local wind on top of the prevailing one (m/s), z the height of the
// last wake through here (m), w how fresh that wake is (0-1). A wake is drawn
// only near its own height: an aircraft cuts a streak through the cloud it
// flies in, not a hole through the whole layer.
// Also cs_noise, run once: the tiling 3D noise the clouds are built from.

@group(0) @binding(0) var<uniform> atmos: Atmosphere;
@group(0) @binding(1) var state_in: texture_2d<f32>;
@group(0) @binding(2) var flow_in: texture_2d<f32>;
@group(0) @binding(3) var state_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var flow_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var linear_clamp: sampler;
@group(0) @binding(6) var<storage, read> disturbers: array<Disturber>;
@group(0) @binding(7) var<storage, read> storms: array<vec4<f32>>;

// Mirrors sky.rs `Disturber`.
struct Disturber {
    // xy where it is now, radius (m), kind
    a: vec4<f32>,
    // xy velocity (m/s), strength, age (s)
    b: vec4<f32>,
    // z height it moves at (m)
    c: vec4<f32>,
}

const KIND_WAKE: f32 = 0.0;
const KIND_BLAST: f32 = 1.0;
// Fastest the stirred air may move on top of the prevailing wind, m/s.
const FLOW_MAX: f32 = 12.0;

struct SimPush {
    // Seconds this step covers.
    dt: f32,
    // 1: start from the air mass alone (first frame, or after a jump).
    reset: u32,
}
var<immediate> push: SimPush;

fn texel_world(id: vec2<u32>) -> vec2<f32> {
    let size = vec2<f32>(textureDimensions(state_out));
    return (vec2<f32>(id) + 0.5) / size * atmos.weather.yz;
}

// The weather the air would have by itself: the air mass plus the storms
// sky.rs is running (xy centre, radius, strength).
fn rest_state(xy: vec2<f32>) -> vec2<f32> {
    var w = cloud_climate(xy, atmos.wind.xy, atmos.layer.w, atmos.shape.y);
    let n = u32(atmos.counts.w);
    for (var i = 0u; i < n; i++) {
        let s = storms[i];
        let d = distance(xy, s.xy) / s.z;
        if d > 1.6 { continue; }
        // A storm is a mass of cloud, ragged at its rim, with a cluster of
        // towering cells inside it rather than one smooth column.
        let local = xy - atmos.wind.xy + s.xy * 0.37;
        let rim = grad_noise2(local, s.z * 0.35);
        let body = (1.0 - smoothstep(0.35, 1.25, d + (rim - 0.5) * 0.7)) * s.w;
        let cells = smoothstep(0.35, 0.72, grad_noise2(local + 311.0, s.z * 0.22) * 0.7 + grad_noise2(local - 97.0, s.z * 0.09) * 0.3);
        let core = (1.0 - smoothstep(0.0, 0.85, d + (rim - 0.5) * 0.4)) * s.w * mix(0.35, 1.0, cells);
        w.x = max(w.x, body);
        w.y = max(w.y, core);
    }
    return w;
}

// How hard cloud this thick rains, before it has had time to start.
fn rain_from(cover: f32, storm: f32) -> f32 {
    // Cover tops out at 1: a full deck rains only in heavy (overcast) weather.
    let heavy = smoothstep(0.35, 0.8, storm) + 0.6 * smoothstep(0.9, 1.0, cover) * smoothstep(1.3, 1.8, atmos.layer.w)
        // Wet weather (rain set past 0.6) lets any thick cloud shower, not just storms.
        + 0.8 * smoothstep(0.6, 1.0, atmos.shape.z) * smoothstep(0.55, 0.95, cover);
    return clamp(heavy * atmos.shape.z * 1.2, 0.0, 1.0);
}

@compute @workgroup_size(8, 8)
fn cs_advect(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(state_out);
    if id.x >= dims.x || id.y >= dims.y {
        return;
    }
    let size = vec2<f32>(dims);
    let uv = (vec2<f32>(id.xy) + 0.5) / size;
    let here = textureSampleLevel(flow_in, linear_clamp, uv, 0.0);
    // Back along the wind: where the air now here was a moment ago. The rest
    // state drifts with the same prevailing wind, so the two stay in step.
    let back = (atmos.wind.zw + here.xy) * push.dt / atmos.weather.yz;
    let src = uv - back;
    let state = textureSampleLevel(state_in, linear_clamp, src, 0.0);
    let flow = textureSampleLevel(flow_in, linear_clamp, src, 0.0);
    textureStore(state_out, vec2<i32>(id.xy), state);
    textureStore(flow_out, vec2<i32>(id.xy), flow);
}

@compute @workgroup_size(8, 8)
fn cs_force(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(state_out);
    if id.x >= dims.x || id.y >= dims.y {
        return;
    }
    let xy = texel_world(id.xy);
    let rest = rest_state(xy);
    if push.reset != 0u {
        textureStore(state_out, vec2<i32>(id.xy), vec4<f32>(rest, 0.0, rain_from(rest.x, rest.y)));
        textureStore(flow_out, vec2<i32>(id.xy), vec4<f32>(0.0));
        return;
    }
    let dt = push.dt;
    var state = textureLoad(state_in, vec2<i32>(id.xy), 0);
    var flow = textureLoad(flow_in, vec2<i32>(id.xy), 0);

    // Drawn back toward the air mass: quickly where the air is calm, slowly
    // where it was churned, so a wake lingers and then heals over.
    let heal = mix(1.0 / 25.0, 1.0 / 90.0, clamp(state.z, 0.0, 1.0));
    state.x += (rest.x - state.x) * (1.0 - exp(-dt * heal));
    state.y += (rest.y - state.y) * (1.0 - exp(-dt / 30.0));

    // Pushed-away air spreads the cloud it carries thin: divergence of the
    // stirred flow, from its neighbours.
    let texel = atmos.weather.x;
    let c = vec2<i32>(id.xy);
    let last = vec2<i32>(dims) - 1;
    let fx = textureLoad(flow_in, vec2<i32>(min(c.x + 1, last.x), c.y), 0).x
        - textureLoad(flow_in, vec2<i32>(max(c.x - 1, 0), c.y), 0).x;
    let fy = textureLoad(flow_in, vec2<i32>(c.x, min(c.y + 1, last.y)), 0).y
        - textureLoad(flow_in, vec2<i32>(c.x, max(c.y - 1, 0)), 0).y;
    // Capped at about 1% a second: wakes shove air sideways along every lane
    // a flight patrols, and uncapped this thinned the cloud there by a fifth a
    // second, cutting the layer into strips.
    let spread = clamp((fx + fy) / (2.0 * texel), -0.012, 0.012);
    state.x = max(state.x * exp(-max(spread, 0.0) * dt * 1.2) + max(-spread, 0.0) * dt * 0.15 * state.x, 0.0);

    let n = u32(atmos.counts.z);
    for (var i = 0u; i < n; i++) {
        let d = disturbers[i];
        let r = d.a.z;
        let kind = d.a.w;
        let rel = xy - d.a.xy;
        let dist = length(rel);
        if kind == KIND_BLAST {
            // A pressure front running out from the blast: it throws the cloud
            // outward and leaves a clear ring that closes over time.
            let age = d.b.w;
            let front = r * (0.35 + 1.4 * (1.0 - exp(-age * 1.6)));
            let band = exp(-pow((dist - front) / (r * 0.35), 2.0));
            let core = exp(-pow(dist / (front * 0.8), 2.0));
            let away = rel / max(dist, 1.0);
            let kick = d.b.z * exp(-age * 0.9);
            // Mostly a shove: the front throws cloud outward and churns it; the
            // middle thins raggedly rather than being cut out as a disc.
            let ragged = grad_noise2(xy + d.a.xy * 0.13, r * 0.35);
            flow = vec4<f32>(flow.xy + away * band * kick * 60.0 * dt, flow.zw);
            state.x *= 1.0 - clamp(core * kick * dt * 1.6 * smoothstep(0.25, 0.75, ragged), 0.0, 0.6);
            state.z = max(state.z, (core + band) * kick);
            continue;
        }
        // A body moving through the layer: distance to the stretch it swept
        // this step, so a fast jet leaves one clean line and not a dotted one.
        let sweep = d.b.xy * max(dt, 0.02) * 1.5;
        let seg = dot(sweep, sweep);
        var t = 0.0;
        if seg > 0.01 {
            t = clamp(dot(xy - (d.a.xy - sweep), sweep) / seg, 0.0, 1.0);
        }
        let near = xy - (d.a.xy - sweep + sweep * t);
        let q = length(near);
        if q > r * 7.0 { continue; }
        let body = exp(-pow(q / r, 2.0));
        let skirt = exp(-pow(q / (r * 2.2), 2.0));
        let speed = length(d.b.xy);
        let heading = d.b.xy / max(speed, 0.1);
        let side = near / max(q, 0.5);
        {
            // A wake: churned air, and a fresh streak at the aircraft's height
            // for the clouds to draw (thinned raggedly there, cloud_at).
            // Nothing is taken out of the whole column, and the air is not
            // set moving: a flight on patrol passes the same lanes again and
            // again, and any push it gave the air (a vortex shear along the
            // path, air shoved aside) carried the cloud out of those lanes
            // into long ribbons with hard edges.
            let fresh = body * d.b.z;
            if fresh > flow.w * 0.5 {
                flow.z = d.c.x;
            }
            flow.w = max(flow.w, fresh);
            // Churn broad and gentle: churn twists the billows (cloud_at), and
            // a narrow band of it sheared them into fine stripes along the path.
            // Weak rather than narrow: the wake's own radius is kept small
            // (sky.rs), and a narrower band sheared the billows into stripes.
            let broad = exp(-pow(q / (r * 5.0), 2.0));
            state.z = max(state.z, broad * d.b.z * 0.2);
        }
    }

    // Rain falls from storm cores and heavy decks, building and easing off over
    // a quarter of a minute; a wake cut through the cloud stops it there too.
    state.w += (rain_from(state.x, state.y) - state.w) * (1.0 - exp(-dt / 15.0));

    // Stirred air slows back to the prevailing wind; churn dies away.
    // Streaks fade over half a minute.
    flow = vec4<f32>(flow.xy * exp(-dt / 7.0), flow.z, flow.w * exp(-dt / 30.0));
    // Stirred air never outruns a stiff breeze. Unbounded, a flight on patrol
    // (or a hull hovering) pumped it to tens of m/s, and the cloud it carried
    // and the billows it bends (cloud_at) were sheared into long ribbons with
    // hard edges along the wake's rim.
    let gust = length(flow.xy);
    if gust > FLOW_MAX {
        flow = vec4<f32>(flow.xy * (FLOW_MAX / gust), flow.zw);
    }
    state.z *= exp(-dt / 22.0);
    state.x = clamp(state.x, 0.0, 1.5);
    textureStore(state_out, vec2<i32>(id.xy), state);
    textureStore(flow_out, vec2<i32>(id.xy), flow);
}

// ---- The cloud noise ---------------------------------------------------------
// A tiling 3D texture: x Perlin-Worley (the billows), yzw Worley at three
// frequencies (erosion of the edges). Written once at start-up.

@group(0) @binding(8) var noise_out: texture_storage_3d<rgba8unorm, write>;

fn hash33(p: vec3<u32>) -> vec3<f32> {
    var q = p * vec3<u32>(1597334673u, 3812015801u, 2798796415u);
    q = (q.x ^ q.y ^ q.z) * vec3<u32>(1597334673u, 3812015801u, 2798796415u);
    return vec3<f32>(q) * (1.0 / 4294967295.0);
}

// Distance to the nearest feature point on a lattice of `cells` per side,
// wrapping so the texture tiles. 1 at a feature, 0 far from one.
fn worley(p: vec3<f32>, cells: u32) -> f32 {
    let c = floor(p * f32(cells));
    let f = p * f32(cells) - c;
    var best = 1.0;
    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let o = vec3<f32>(f32(x), f32(y), f32(z));
                let cell = vec3<i32>(c + o);
                let n = i32(cells);
                let wrapped = vec3<u32>((cell % n + n) % n);
                let point = o + hash33(wrapped + vec3<u32>(cells * 131u));
                best = min(best, dot(point - f, point - f));
            }
        }
    }
    return 1.0 - sqrt(best);
}

fn grad3(cell: vec3<i32>, n: i32) -> vec3<f32> {
    let w = vec3<u32>((cell % n + n) % n);
    let h = hash33(w + vec3<u32>(7u, 19u, 83u));
    return normalize(h * 2.0 - 1.0 + vec3<f32>(1e-4));
}

// Tiling gradient noise, [0, 1].
fn perlin3(p: vec3<f32>, cells: u32) -> f32 {
    let q = p * f32(cells);
    let i = vec3<i32>(floor(q));
    let f = fract(q);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let n = i32(cells);
    var v = array<f32, 8>();
    for (var k = 0; k < 8; k++) {
        let o = vec3<i32>(k & 1, (k >> 1u) & 1, (k >> 2u) & 1);
        v[k] = dot(grad3(i + o, n), f - vec3<f32>(o));
    }
    let x0 = mix(mix(v[0], v[1], u.x), mix(v[2], v[3], u.x), u.y);
    let x1 = mix(mix(v[4], v[5], u.x), mix(v[6], v[7], u.x), u.y);
    return clamp(mix(x0, x1, u.z) * 0.9 + 0.5, 0.0, 1.0);
}

fn remap(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    return c + (v - a) / (b - a) * (d - c);
}

@compute @workgroup_size(4, 4, 4)
fn cs_noise(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(noise_out);
    if any(id >= dims) {
        return;
    }
    let p = (vec3<f32>(id) + 0.5) / vec3<f32>(dims);
    let perlin = perlin3(p, 4u) * 0.625 + perlin3(p, 8u) * 0.25 + perlin3(p, 16u) * 0.125;
    let w1 = worley(p, 4u) * 0.625 + worley(p, 8u) * 0.25 + worley(p, 16u) * 0.125;
    let w2 = worley(p, 8u) * 0.625 + worley(p, 16u) * 0.25 + worley(p, 32u) * 0.125;
    let w3 = worley(p, 16u) * 0.625 + worley(p, 32u) * 0.25 + worley(p, 64u) * 0.125;
    // Perlin-Worley: billowy lumps with the gaps between them carved round.
    let pw = clamp(remap(perlin, w1 - 1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
    textureStore(noise_out, vec3<i32>(id), vec4<f32>(pw, w1, w2, w3));
}
