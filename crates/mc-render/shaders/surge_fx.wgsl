//!use bindings
// A surge shell's wake (renderer/surge_fx.rs, surge_fx/pass.rs), drawn as two meshes, one
// instance of each a surge:
//
// - **The front**, the swell: a low ripple standing on the ring where the front stands,
//   all round the crater. Not a wall: a long shallow back of bent violet air rising to a
//   thin pale rose crest no taller than a man, then a short steep face to a bright line
//   along its foot. It thins away as it reaches its radius.
// - **The burn** behind it: the disc it has rolled over draped on the ground, veined with
//   cracks of light. Just behind the front they burn white and rose; they settle to
//   crimson embers pulsing slowly, faint rings of violet running out across them, and the
//   ground between is darkened as if scorched. At the end it dies down.
//
// Both are light laid over the scene premultiplied: mostly light added, a little of what
// is behind dimmed.

//!rust crate::renderer::surge_fx::pass::GpuSurge
struct SurgeWake {
    // The crater, and the renderer time the shell landed.
    centre: vec3<f32>,
    start: f32,
    // Metres out it rolls, and metres a second.
    radius: f32,
    speed: f32,
    // Seconds the ground burns once the front has rolled out.
    linger: f32,
    // Sets each surge's noise apart.
    seed: f32,
    // How hard it lands: the front's height and light.
    power: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(1) @binding(0) var<storage, read> surges: array<SurgeWake>;

const WHITE: vec3<f32> = vec3<f32>(1.0, 0.93, 0.95);
const ROSE: vec3<f32> = vec3<f32>(1.0, 0.45, 0.5);
const CRIMSON: vec3<f32> = vec3<f32>(0.75, 0.03, 0.06);
const EMBER: vec3<f32> = vec3<f32>(0.32, 0.01, 0.03);
const VIOLET: vec3<f32> = vec3<f32>(0.55, 0.16, 1.0);
// Metres between the burn's cracks, coarse and fine.
const CRACK: f32 = 7.0;
const CRACK_FINE: f32 = 2.6;
// Metres apart the aftershock rings run, and their pace against the front's.
const SHOCK_GAP: f32 = 26.0;
const SHOCK_PACE: f32 = 0.4;
// The swell: metres of back behind its crest, of face ahead of it, and where along its
// section the crest stands.
const BACK: f32 = 12.0;
const FACE: f32 = 1.8;
const CREST: f32 = 0.82;

struct SurgeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // The front: round the ring (0..1), and across its section (0 back foot, 1 face foot).
    @location(2) at: vec2<f32>,
    @location(3) @interpolate(flat) surge: u32,
}

fn hidden() -> SurgeOut {
    var out: SurgeOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    return out;
}

fn ground_at(xy: vec2<f32>) -> f32 {
    return max(terrain_height(xy), globals.map.z);
}

// Seconds since the shell landed.
fn age_of(s: SurgeWake) -> f32 {
    return globals.camera.w - s.start;
}

// Seconds the front takes to roll out.
fn roll_of(s: SurgeWake) -> f32 {
    return s.radius / max(s.speed, 1.0);
}

// Metres out the front stands now: past its radius it eases on a little as it thins.
fn front_of(s: SurgeWake) -> f32 {
    let age = age_of(s);
    let over = clamp(age - roll_of(s), 0.0, SURGE_FX_SPENT);
    return min(age * s.speed, s.radius) + s.speed * over * (0.5 - 0.25 * over / SURGE_FX_SPENT);
}

// The corner of quad `quad` that vertex `corner` (0..6) of it stands on, as steps out
// (`x`) and round (`y`).
fn grid_corner(quad: u32, corner: u32, around: u32) -> vec2<u32> {
    var di = 0u;
    var dj = 0u;
    switch corner {
        case 1u, 3u: {
            dj = 1u;
        }
        case 2u, 5u: {
            di = 1u;
        }
        case 4u: {
            di = 1u;
            dj = 1u;
        }
        default: {}
    }
    return vec2<u32>(quad / around + di, quad % around + dj);
}

// The swell's height `s` across its section, 0 at its back foot to 1 at its face's: a
// long low rise up its back to the crest, a short fall down its face.
fn swell(s: f32) -> f32 {
    let up = sin(0.5 * PI * clamp(s / CREST, 0.0, 1.0));
    let down = cos(0.5 * PI * clamp((s - CREST) / (1.0 - CREST), 0.0, 1.0));
    return select(down, up * up, s < CREST);
}

@vertex
fn vs_surge_front(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let age = age_of(s);
    let front = front_of(s);
    if age < 0.0 || age > roll_of(s) + SURGE_FX_SPENT || front < 2.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_SEGMENTS);
    let across = f32(g.x) / f32(SURGE_FX_SECTION);
    let round = f32(g.y) / f32(SURGE_FX_SEGMENTS);
    let a = round * 2.0 * PI;
    let out_xy = vec2<f32>(cos(a), sin(a));
    // Close in, the swell is short: it grows to its full length over its first 40 m.
    let grown = smoothstep(0.0, 40.0, front);
    let back = BACK * (0.3 + 0.7 * grown);
    let behind = front - back * (1.0 - across / CREST);
    let ahead = front + FACE * (across - CREST) / (1.0 - CREST);
    let r = max(select(ahead, behind, across < CREST), 0.0);
    let xy = s.centre.xy + out_xy * r;
    let tall = (1.2 + 0.7 * s.power) * (0.5 + 0.5 * grown);
    let h = tall * swell(across);
    let p = vec3<f32>(xy, ground_at(xy) + 0.15 + h);
    var out: SurgeOut;
    out.surge = instance;
    // Leaning back over its long back, out over its face.
    let lean = select(-0.6, tall / back, across < CREST);
    out.normal = normalize(vec3<f32>(out_xy * lean, 1.0));
    out.world = p;
    out.at = vec2<f32>(round, across);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@vertex
fn vs_surge_ground(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> SurgeOut {
    let s = surges[instance];
    let age = age_of(s);
    let front = min(age * s.speed, s.radius);
    if age < 0.0 || age > roll_of(s) + s.linger || front < 2.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, SURGE_FX_SEGMENTS);
    let u = f32(g.x) / f32(SURGE_FX_RINGS);
    let round = f32(g.y) / f32(SURGE_FX_SEGMENTS);
    let a = round * 2.0 * PI;
    // Rows even over the disc's area: as many out at its rim as its middle has room for.
    let r = front * sqrt(u);
    let xy = s.centre.xy + vec2<f32>(cos(a), sin(a)) * r;
    let p = vec3<f32>(xy, ground_at(xy) + 0.3);
    var out: SurgeOut;
    out.surge = instance;
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.world = p;
    out.at = vec2<f32>(round, u);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

// Cracks of a cellular pattern `cell` metres across: 1 on a crack, falling to 0 between,
// softened where they would be finer than a pixel, so far off they glow rather than
// sparkle. Its seed sets one surge's cracks apart from another's.
fn cracks(xy: vec2<f32>, cell: f32, seed: f32) -> f32 {
    let p = xy / cell + vec2<f32>(seed, seed * 1.7);
    let i = floor(p);
    let f = p - i;
    var d1 = 8.0;
    var d2 = 8.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y));
            let c = i + o;
            let jitter = vec2<f32>(hash21(c), hash21(c + vec2<f32>(17.3, 5.1)));
            let d = length(o + jitter - f);
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let gap = d2 - d1;
    let width = max(0.07, fwidth(gap) * 1.5);
    let line = 1.0 - smoothstep(0.0, width, gap);
    let fine = clamp(length(fwidth(p)) * 2.0, 0.0, 1.0);
    return mix(line, 0.05, fine);
}

// How much it is seen edge on, 0 square on to 1 edge on.
fn edge_of(in: SurgeOut) -> f32 {
    let view = normalize(globals.camera.xyz - in.world);
    return 1.0 - abs(dot(normalize(in.normal), view));
}

fn fs_surge_front_lit(in: SurgeOut) -> vec4<f32> {
    let s = surges[in.surge];
    let age = age_of(s);
    let across = in.at.y;
    // Spent, it thins away from its back forward; it comes up in a blink.
    let spent = clamp((age - roll_of(s)) / SURGE_FX_SPENT, 0.0, 1.0);
    let fade = smoothstep(0.0, 0.05, age) * (1.0 - spent);
    // Light running round the ring in lumps, drawn out along it, finer the wider it is.
    let span = max(8.0, 2.0 * PI * front_of(s) / 9.0);
    let warp = value_noise2(vec2<f32>(in.at.x * span * 0.4, age * 0.8) + s.seed, 1.0);
    let lumps = value_noise2(vec2<f32>(in.at.x * span + warp * 2.0, across * 2.0 - age * 1.5) + s.seed * 1.3, 1.0);
    // A thin crest line, and a brighter line along its foot ahead.
    let crest = exp(-pow((across - CREST) / 0.05, 2.0));
    let foot = exp(-pow((across - 1.0) / 0.04, 2.0));
    // Its back a faint haze of bent violet air, thickest just under the crest.
    let body = smoothstep(0.0, CREST, across) * (1.0 - smoothstep(CREST, 1.0, across));
    let edge = edge_of(in);
    let hot = 0.6 + 0.6 * smoothstep(0.45, 0.8, lumps);
    let light = (mix(ROSE, WHITE, 0.5 * crest) * crest * 1.4 + ROSE * foot * 0.9) * hot
        + VIOLET * body * (0.08 + 0.35 * pow(edge, 2.0)) * (0.5 + lumps);
    let cover = clamp(0.06 * body + 0.1 * crest, 0.0, 0.2);
    // Its back is eaten away first as it thins.
    let alive = smoothstep(-0.05, 0.05, across + 0.3 * lumps - spent * 1.2);
    return vec4<f32>(light * s.power, cover) * alive * fade;
}

fn fs_surge_ground_lit(in: SurgeOut) -> vec4<f32> {
    let s = surges[in.surge];
    let age = age_of(s);
    let d = in.world.xy - s.centre.xy;
    let r = length(d);
    // Seconds since the front passed here; nothing ahead of it (masked at the end, not
    // discarded: the cracks take derivatives).
    let reached = age - r / max(s.speed, 1.0);
    let passed = max(reached, 0.0);
    let total = roll_of(s) + s.linger;
    let dying = 1.0 - smoothstep(total - SURGE_FX_FADE, total, age);
    let fresh = exp(-passed / SURGE_FX_FRESH);
    // Cracks wandering over the ground, coarse and fine, bent so they are not cells.
    let bend = vec2<f32>(value_noise2(in.world.xy + s.seed * 7.0, 9.0),
        value_noise2(in.world.xy + s.seed * 3.0 + 40.0, 9.0)) * 4.0;
    let coarse = cracks(in.world.xy + bend, CRACK, s.seed);
    let fine = cracks(in.world.xy + bend * 0.6, CRACK_FINE, s.seed + 3.1);
    // Only some of them open: patches of cracked ground between quiet ones, and each crack
    // broken along its length, so it wanders and stops rather than walling cells in.
    let patches = smoothstep(0.42, 0.72, value_noise2(in.world.xy + s.seed * 5.0, 22.0));
    let runs = smoothstep(0.35, 0.6, value_noise2(in.world.xy + s.seed * 9.0, 3.5));
    let veins = (coarse * runs + 0.4 * fine * smoothstep(0.5, 0.75, runs)) * patches;
    // Each patch pulses slowly, out of step with its neighbours.
    let phase = value_noise2(in.world.xy + s.seed * 11.0, 14.0) * 2.0 * PI;
    let pulse = 0.6 + 0.4 * sin(age * 2.1 + phase);
    // Rings of violet running out across it behind the front, the swell's aftershocks.
    let shock_r = r - (age * s.speed * SHOCK_PACE);
    // Broken up round the ring, so they read as a ripple through the cracks, not a target.
    let torn = smoothstep(0.35, 0.75, value_noise2(vec2<f32>(atan2(d.y, d.x) * 12.0, shock_r * 0.05) + s.seed, 1.0));
    let shock = pow(0.5 + 0.5 * cos(shock_r / SHOCK_GAP * 2.0 * PI), 10.0)
        * smoothstep(0.0, 0.6, passed) * torn;
    // Fresh, the cracks white and rose with a wash of heat between them; settled, crimson
    // embers; at the rim, softened so the disc's edge does not show.
    let rim = 1.0 - smoothstep(s.radius * 0.88, s.radius, r);
    let ember = mix(EMBER, CRIMSON, pulse) * veins * 0.9;
    let hot = mix(CRIMSON, mix(ROSE, WHITE, fresh * fresh), fresh) * (veins * 1.3 + 0.02) * fresh;
    let light = (ember + hot + VIOLET * shock * veins * 0.8) * rim;
    let cover = 0.16 * (1.0 - 0.5 * fresh) * rim;
    return vec4<f32>(light, cover) * dying * step(0.0, reached);
}

// The two, seen through the water from under it (bindings.wgsl `under_sea_seen`).
@fragment
fn fs_surge_front(in: SurgeOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_surge_front_lit(in), in.clip);
}

@fragment
fn fs_surge_ground(in: SurgeOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_surge_ground_lit(in), in.clip);
}
