// A capital ship's hull as it goes into warp and comes out (renderer/warp_fx.rs draws the
// light round it). Prepended to shaders with `//!use warp_hull` (entity.wgsl).
//
// `Entity::fx.xy` is the stretch last tick and this (`mc_sim::mirror::warp`): 0 whole, 1 a
// streak of light. Going in it rises, over the one tick the ship is still drawn where it
// left: its tail stays put and its nose shoots ahead along its heading, the hull thinning
// into a needle of light. Coming out it falls: the streak lies back along the way it came
// and collapses forward into the hull. A dampened jump (`WARP_STATUS_DAMPED`) stutters:
// the streak catches and slips, and the hull is torn sideways in jagged bands.

// A full streak reaches this many hull radii ahead of (or behind) the ship.
const WARP_STREAK_RADII: f32 = 16.0;
// What is left of the hull's width and height at full stretch.
const WARP_THIN: f32 = 0.08;

// How far into the streak the hull is drawn this frame, eased, and 1 when it is going in
// (the stretch rising), 0 coming out.
fn warp_state(fx: vec4<f32>, t: f32, damped: bool, seed: f32, time: f32) -> vec2<f32> {
    let s = clamp(mix(fx.x, fx.y, t), 0.0, 1.0);
    var e = s * s;
    if damped {
        // The drive catches and slips: held back, then let go, a dozen times a second.
        let beat = floor(time * 13.0 + seed * 7.0);
        e *= 0.55 + 0.45 * hash11(beat + seed * 91.0);
    }
    return vec2<f32>(e, select(0.0, 1.0, fx.y > fx.x));
}

// `world`, a hull vertex at `origin` facing `fwd` with `left` and `up`, pulled out along
// the streak. `x` is the vertex's model-space fore-and-aft, `reach` the hull's radius.
fn warp_stretch(world: vec3<f32>, origin: vec3<f32>, fwd: vec3<f32>, left: vec3<f32>, up: vec3<f32>,
                x: f32, reach: f32, state: vec2<f32>, damped: bool, seed: f32, time: f32) -> vec3<f32> {
    let e = state.x;
    if e <= 0.0 {
        return world;
    }
    // 0 at the stern, 1 at the nose.
    let u = clamp((x + reach) / (2.0 * reach), 0.0, 1.0);
    let length = WARP_STREAK_RADII * reach * e;
    // Going in, the nose runs ahead and the stern stays; coming out, the stern trails back.
    let along = select(-pow(1.0 - u, 1.6), pow(u, 1.6), state.y > 0.5) * length;
    let rel = world - origin;
    let ax = dot(rel, fwd);
    let thin = mix(1.0, WARP_THIN, smoothstep(0.0, 0.3, e));
    var out = origin + fwd * (ax + along) + (rel - fwd * ax) * thin;
    if damped {
        // Torn in bands along the streak, each thrown aside its own way and re-thrown often.
        let band = floor(u * 11.0) + floor(time * 17.0) * 13.0 + seed * 57.0;
        let side = hash11(band) - 0.5;
        let lift = hash11(band + 3.7) - 0.5;
        out += (left * side + up * lift * 0.6) * reach * 0.7 * e;
    }
    return out;
}

// A hull's colour as it goes into or comes out of a streak: it whitens into the light of
// the streak from the stretched end, cold blue-white, or sickly violet and red when a
// dampener has it. `warp` is `warp_state`, then 1 when dampened, then the vertex's 0..1
// stern-to-nose place.
fn warp_hull_light(color: vec3<f32>, warp: vec4<f32>, time: f32, seed: f32) -> vec3<f32> {
    let e = warp.x;
    if e <= 0.0 {
        return color;
    }
    let damped = warp.z > 0.5;
    // The stretched end is the brightest: the nose going in, the stern coming out.
    let lead = select(1.0 - warp.w, warp.w, warp.y > 0.5);
    let white = select(vec3<f32>(0.78, 0.9, 1.0), vec3<f32>(0.66, 0.36, 1.0), damped);
    let tint = select(vec3<f32>(0.25, 0.55, 1.0), vec3<f32>(0.5, 0.06, 0.9), damped);
    var hot = mix(tint, white, lead) * (2.0 + 9.0 * e * (0.3 + 0.7 * lead));
    if damped {
        // Red flickers through the torn bands.
        let flick = hash11(floor(time * 23.0) + floor(warp.w * 9.0) * 7.0 + seed * 31.0);
        hot = mix(hot, vec3<f32>(1.0, 0.1, 0.18) * 8.0 * e, step(0.85, flick));
    }
    return mix(color, hot, smoothstep(0.0, 0.3, e));
}
