// A capital ship's hull as its drive charges, as it goes into warp and as it comes out
// (renderer/warp_fx.rs draws the light round it). Prepended to shaders with
// `//!use warp_hull` (entity.wgsl).
//
// While the drive spools, `Entity::fx.xy` is minus its charge (0 to -1), last tick and this:
// arcs crawl over the plating, thickening as it fills (`warp_charge_arcs`).
//
// In a jump `Entity::fx.xy` is the stretch last tick and this (`mc_sim::mirror::warp`):
// 0 whole, 1 a streak of light. Going in it rises, over the one tick the ship is still
// drawn where it left: its tail stays put and its nose shoots ahead along its heading, the
// hull thinning into a needle of light. Coming out it falls: the streak lies back along the way it came
// and collapses forward into the hull. A dampened jump (`WARP_STATUS_DAMPED`) stutters:
// the streak catches and slips, and the hull is torn sideways in jagged bands.

// The Regency's drive (`WARP_STATUS_NANITE`) does not stretch the hull: it comes apart. The
// hull is cut into cells of a few metres, each leaving at its own moment (the stern first
// going in, the nose first coming back) and its own speed along the jump, so the triangles
// that span two cells are drawn as slivers streaming out; the shards darken to black with
// red heat at the edge of the break, and the swarm re-condenses in violet (`warp_unmake`).

// Metres across a cell of a hull coming apart (the vertex and fragment shaders cut it alike).
const WARP_CELL: f32 = 6.0;

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
                local: vec3<f32>, reach: f32, state: vec2<f32>, damped: bool, nanite: bool,
                seed: f32, time: f32) -> vec3<f32> {
    let e = state.x;
    if e <= 0.0 {
        return world;
    }
    let x = local.x;
    if nanite {
        return warp_unmake(world, origin, fwd, left, up, local, reach, e, state.y > 0.5, damped, seed, time);
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

// How far the cell of `local` has come apart, 0 whole to 1 gone, when the hull is `s` of the
// way through (0 whole, 1 all swarm). Going in the stern leaves first, coming back the nose
// is whole first; each cell's own noise keeps the front ragged.
fn warp_cell(local: vec3<f32>, u: f32, s: f32, seed: f32) -> vec2<f32> {
    let q = floor(local / WARP_CELL);
    let h = hash11(dot(q, vec3<f32>(1.0, 7.3, 13.1)) + seed * 50.0);
    let th = mix(h, 1.0 - u, 0.55) * 0.72;
    return vec2<f32>(clamp((s - th) / 0.28, 0.0, 1.0), h);
}

// A Regency hull coming apart (`s` through the swarm, `going` in): each cell flies off along
// the jump, forward as it leaves and back along the way it came as it returns, at its own
// speed, thrown a little sideways.
fn warp_unmake(world: vec3<f32>, origin: vec3<f32>, fwd: vec3<f32>, left: vec3<f32>, up: vec3<f32>,
               local: vec3<f32>, reach: f32, e: f32, going: bool, damped: bool, seed: f32,
               time: f32) -> vec3<f32> {
    let s = sqrt(e);
    let u = clamp((local.x + reach) / (2.0 * reach), 0.0, 1.0);
    let c = warp_cell(local, u, s, seed);
    let p = c.x * c.x * (3.0 - 2.0 * c.x);
    if p <= 0.0 {
        return world;
    }
    let dir = select(-1.0, 1.0, going);
    let speed = 3.0 + 13.0 * hash11(c.y * 91.0 + seed * 7.0);
    var out = world + fwd * dir * p * p * speed * reach * 0.5;
    let spread = vec2<f32>(hash11(c.y * 17.0 + 3.0) - 0.5, hash11(c.y * 29.0 + 5.0) - 0.5);
    out += (left * spread.x + up * spread.y * 0.6) * reach * 1.1 * p;
    if damped {
        // Torn, the swarm shudders: each cell re-thrown a dozen times a second.
        let beat = floor(time * 13.0) * 7.0 + c.y * 40.0;
        out += (left * (hash11(beat) - 0.5) + up * (hash11(beat + 3.7) - 0.5)) * reach * 0.25 * p;
    }
    return out;
}

// A hull's colour as it goes into or comes out of a streak: it whitens into the light of
// the streak from the stretched end, cold blue-white, or sickly violet and red when a
// dampener has it. `warp` is `warp_state`, then 1 when dampened, then the vertex's 0..1
// stern-to-nose place.
fn warp_hull_light(color: vec3<f32>, warp: vec4<f32>, local: vec3<f32>, time: f32, seed: f32) -> vec3<f32> {
    let e = warp.x;
    if e <= 0.0 {
        return color;
    }
    let damped = (u32(warp.z) & 1u) != 0u;
    if warp.z > 1.5 {
        return warp_swarm_light(color, local, warp.w, e, warp.y > 0.5, damped, time, seed);
    }
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

// A Regency hull as it comes apart or condenses: the plating going to black soot where the
// swarm has it, a seam of red heat at the edge of the break (violet where it is reforming),
// and sparse red glints in the dark. Nothing here is white: the swarm eats the light.
fn warp_swarm_light(color: vec3<f32>, local: vec3<f32>, u: f32, e: f32, going: bool,
                    damped: bool, time: f32, seed: f32) -> vec3<f32> {
    let c = warp_cell(local, u, sqrt(e), seed);
    let p = c.x;
    if p <= 0.0 {
        return color;
    }
    let soot = vec3<f32>(0.010, 0.008, 0.012);
    let seam = smoothstep(0.0, 0.2, p) * (1.0 - smoothstep(0.3, 0.9, p));
    let red = vec3<f32>(1.0, 0.1, 0.05);
    let violet = vec3<f32>(0.55, 0.2, 1.0);
    var heat = select(violet, red, going) * (0.4 + 1.8 * seam) * seam;
    if damped {
        // A torn jump flares and gutters.
        heat *= 0.3 + 1.4 * hash11(floor(time * 19.0) + c.y * 60.0 + seed * 11.0);
    }
    // Glints in the soot, each cell's own.
    let glint = step(0.93, hash11(c.y * 53.0 + floor(time * 9.0))) * 1.6 * p;
    return mix(color, soot, smoothstep(0.0, 0.45, p)) + heat + red * glint * (1.0 - p * 0.5);
}

// How full a spooling drive's charge is, 0 to 1, between ticks: `Entity::fx.xy` carry it
// below zero while the drive spools (`mc_sim::mirror::warp`), the stretch at or above it.
fn warp_charge(fx: vec4<f32>, t: f32) -> f32 {
    return clamp(-mix(fx.x, fx.y, t), 0.0, 1.0);
}

// A drive charging, as light to add over the hull's own plating: arcs like an EMP's
// (emp.wgsl) in the drive's blue-white, a few at first and thickening and brightening as
// the charge fills; in its last third they gather aft round the drives and at the nose,
// and long bolts leap across the plating. The hull's light runs aft in waves, quicker as
// it fills. (A charge is never dampened: a dampener snags the jump only as it goes.)
// `u` is the point's place from stern (0) to nose (1), the rest as `emp_arcs`.
fn warp_charge_arcs(local: vec3<f32>, reach: f32, px: f32, charge: f32, u: f32, nanite: bool,
                    time: f32, seed: f32) -> vec3<f32> {
    if charge <= 0.0 {
        return vec3<f32>(0.0);
    }
    // The Regency's drive burns red through the plating, violet at the hot centre.
    let arc = select(vec3<f32>(0.3, 0.6, 1.0), vec3<f32>(1.0, 0.1, 0.06), nanite);
    let core = select(vec3<f32>(0.85, 0.94, 1.0), vec3<f32>(0.75, 0.4, 1.0), nanite);
    // Finer than a stun's arcs: more of them, each smaller.
    let cell = clamp(reach * 0.08, 0.6, 12.0);
    let q = local / cell;
    let p = px / cell;
    // Late on, the arcs leave the middle of the hull for the drives aft and the nose.
    let gather = smoothstep(0.55, 1.0, charge);
    let ends = max(1.0 - smoothstep(0.0, 0.3, u), 0.7 * smoothstep(0.72, 1.0, u));
    let weight = mix(1.0, 0.3 + 1.7 * ends, gather);
    // (The noise the patches come from seldom climbs past 0.8: below a cover of 0.2 there
    // are next to none, at 0.3 a fair crawl.)
    let cover = min(0.15 + (0.04 + 0.2 * charge) * weight, 0.5);
    let a = emp_arc_layer(q, p, time, seed + 0.21, 0.25, cover);
    let b = emp_arc_layer(q * 1.7 + 5.0, p * 1.7, time, seed + 0.53, 0.75, cover - 0.04);
    var line = max(a.x, b.x * 0.7);
    var hot = max(a.y, b.y);
    // The last third: long bolts, cells four times the size, leaping across the plating.
    let leap = smoothstep(0.65, 1.0, charge);
    if leap > 0.0 {
        let c = emp_arc_layer(q * 0.25 + 23.0, p * 0.25, time * 0.8, seed + 0.9, 0.1, 0.18 + 0.14 * leap);
        line = max(line, c.x * 1.3);
        hot = max(hot, c.y * 1.5);
    }
    let light = arc * line * 3.0 + core * hot * 5.5;
    // Waves of light running aft to the drives, quicker and brighter as it fills.
    let wave = fract(u * 1.5 + time * (0.4 + 2.2 * charge));
    let sheen = pow(wave, 10.0) * 0.18 * charge * charge;
    let strength = smoothstep(0.0, 0.2, charge) * (0.45 + 1.1 * charge);
    return (light + arc * sheen) * strength;
}
