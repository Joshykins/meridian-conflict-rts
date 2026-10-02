//!use bindings
// A cone weapon's wake (renderer/wake_shell.rs, wake_fx.rs), drawn as two meshes, one
// instance of each a wake:
//
// - **The front**, the wave: a rolling crest arched over the fan where the front stands,
//   its top leading its feet. It is plasma turning over on itself: white-hot on its
//   leading face, pink-hot over the top, red down its back, in hot cells that roll forward
//   over the crest. Run out, it swells, cools and is eaten through.
// - **The trail** it leaves: a shell from the muzzle out to the crest. Each stretch is hot
//   when the front has just passed it and cools after, pink to red to a deep ember with a
//   violet rim, sinking a little and eaten through in hard-edged holes (no mist,
//   docs/STYLE.md), so it dies back from the muzzle behind the front.
//
// Both are light laid over the scene premultiplied: mostly light added, a little of what
// is behind dimmed so it reads red over green grass and not orange.

//!rust crate::renderer::wake_shell::GpuWakeShell
struct WakeShell {
    // The muzzle, and the renderer time the wake left it.
    apex: vec3<f32>,
    start: f32,
    // Which way it rolls over the ground (unit length), and the tangent of the fan's
    // half-angle.
    ahead: vec2<f32>,
    spread: f32,
    // Metres out the front stands now, and metres a second it rolls.
    front: f32,
    speed: f32,
    // The shell's height over its half-width.
    rise: f32,
    // 0 gone, 1 full.
    fade: f32,
    // Seconds since the front ran out; below 0 while it rolls.
    spent: f32,
}

@group(1) @binding(0) var<storage, read> shells: array<WakeShell>;

const WHITE: vec3<f32> = vec3<f32>(1.0, 0.93, 0.88);
const HOT: vec3<f32> = vec3<f32>(1.0, 0.42, 0.16);
const RED: vec3<f32> = vec3<f32>(1.0, 0.06, 0.05);
const EMBER: vec3<f32> = vec3<f32>(0.34, 0.01, 0.03);
const VIOLET: vec3<f32> = vec3<f32>(0.6, 0.18, 1.0);

struct ShellOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // The front: round its arch (0..1) and round its crest (0..1, 0 its leading face).
    // The trail: how far out (0 muzzle, 1 front) and round (0..1).
    @location(2) at: vec2<f32>,
    // The trail: metres from the muzzle, and seconds since the front passed.
    @location(3) passed: vec2<f32>,
    @location(4) @interpolate(flat) shell: u32,
}

// Plasma by heat, 0 spent to 1 fresh: a deep ember, red, pink-hot, white at the top
// (plasma_puffs.wgsl's bursts and wakes cool through the same).
fn wake_heat(h: f32) -> vec3<f32> {
    let c = mix(EMBER, RED, smoothstep(0.0, 0.35, h));
    let d = mix(c, HOT, smoothstep(0.4, 0.75, h));
    return mix(d, WHITE, smoothstep(0.78, 1.0, h));
}

// The corner of quad `quad` that vertex `corner` (0..6) of it stands on, as steps along
// the grid's rows (`x`) and round it (`y`).
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

fn side_of(s: WakeShell) -> vec2<f32> {
    return vec2<f32>(-s.ahead.y, s.ahead.x);
}

// The front's half-width, and how far its feet trail its top.
fn front_half(s: WakeShell) -> f32 {
    return s.front * s.spread * WAKE_SHELL_WIDTH;
}

fn ground_at(xy: vec2<f32>) -> f32 {
    return max(terrain_height(xy), globals.map.z);
}

fn hidden() -> ShellOut {
    var out: ShellOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    return out;
}

// How far a front that has run out has broken up, 0..1.
fn broken(s: WakeShell) -> f32 {
    return clamp(s.spent / WAKE_SHELL_BREAK, 0.0, 1.0);
}

@vertex
fn vs_wake_front(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> ShellOut {
    let s = shells[instance];
    if s.fade <= 0.0 || s.front < 1.0 || s.spent > WAKE_SHELL_BREAK {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, WAKE_SHELL_TUBE);
    let a = f32(g.x) / f32(WAKE_SHELL_ARCH);
    let t = f32(g.y) / f32(WAKE_SHELL_TUBE);
    let phi = a * PI;
    let theta = t * 2.0 * PI;
    let side = side_of(s);
    let half = front_half(s);
    let b = broken(s);
    // The crest's line: arched over the fan, its top leading its feet.
    let along = s.front - WAKE_SHELL_LAG * half * (1.0 - sin(phi));
    let line_xy = s.apex.xy + s.ahead * along + side * (half * cos(phi));
    let line = vec3<f32>(line_xy, ground_at(line_xy) + s.rise * half * sin(phi));
    let out_dir = normalize(vec3<f32>(side * cos(phi), sin(phi) / s.rise));
    let ahead = vec3<f32>(s.ahead, 0.0);
    // Round the crest: a teardrop, short ahead, drawn out behind; it swells as it breaks.
    let swell = 1.0 + 0.7 * b;
    let thick = (0.12 * half + 1.2) * swell;
    let reach = select(0.6 * half + 4.0, 0.08 * half + 1.0, cos(theta) > 0.0) * swell;
    let p = line + ahead * (cos(theta) * reach) + out_dir * (sin(theta) * thick)
        + vec3<f32>(0.0, 0.0, 3.0 * b * b);
    var out: ShellOut;
    out.shell = instance;
    out.normal = normalize(ahead * (cos(theta) / reach) + out_dir * (sin(theta) / thick));
    out.world = p;
    out.at = vec2<f32>(a, t);
    out.passed = vec2<f32>(along, 0.0);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

@vertex
fn vs_wake_trail(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> ShellOut {
    let s = shells[instance];
    if s.fade <= 0.0 || s.front < 1.0 {
        return hidden();
    }
    let g = grid_corner(v / 6u, v % 6u, WAKE_SHELL_AROUND);
    let u = f32(g.x) / f32(WAKE_SHELL_ALONG);
    let a = f32(g.y) / f32(WAKE_SHELL_AROUND);
    let phi = a * PI;
    let side = side_of(s);
    let half_front = front_half(s);
    // Out to the crest's line, the fan's own width swelling to the crest's near it.
    let along = u * (s.front - WAKE_SHELL_LAG * half_front * (1.0 - sin(phi)));
    let half = u * half_front * mix(1.0 / WAKE_SHELL_WIDTH, 1.0, smoothstep(0.3, 1.0, u));
    let age = globals.camera.w - s.start;
    let passed = max(age - along / s.speed, 0.0);
    // As it cools it sinks a little, and its heat lifts it off the ground.
    let rise = s.rise * (1.0 - 0.35 * smoothstep(0.0, WAKE_SHELL_LINGER, passed));
    let xy = s.apex.xy + s.ahead * along + side * (half * cos(phi));
    let floor = mix(s.apex.z, ground_at(xy), smoothstep(0.0, 20.0, along)) + 1.5 * passed;
    let p = vec3<f32>(xy, floor + rise * half * sin(phi));
    var out: ShellOut;
    out.shell = instance;
    out.normal = normalize(vec3<f32>(side * cos(phi), sin(phi) / rise) - vec3<f32>(s.ahead, 0.0) * s.spread);
    out.world = p;
    out.at = vec2<f32>(u, a);
    out.passed = vec2<f32>(along, passed);
    out.clip = globals.view_proj * vec4<f32>(p, 1.0);
    return out;
}

// Thin bright lines where noise crosses its middle, `sharp` the thinner; softened where
// they would be finer than a pixel, so far off they glow rather than sparkle.
fn streaks(q: vec2<f32>, sharp: f32) -> f32 {
    let r = 1.0 - abs(2.0 * value_noise2(q, 1.0) - 1.0);
    let fine = clamp(length(fwidth(q)) * 1.5, 0.0, 1.0);
    return mix(pow(r, sharp), 0.12, fine);
}

// How much it is seen edge on, 0 square on to 1 edge on.
fn edge_of(in: ShellOut) -> f32 {
    let view = normalize(globals.camera.xyz - in.world);
    return 1.0 - abs(dot(normalize(in.normal), view));
}

// White-hot where it meets the ground (or the water).
fn touching(world: vec3<f32>) -> f32 {
    return exp(-max(world.z - ground_at(world.xy), 0.0) / 0.6);
}

fn fs_wake_front_lit(in: ShellOut) -> vec4<f32> {
    let s = shells[in.shell];
    let edge = edge_of(in);
    let age = globals.camera.w - s.start;
    let seed = fract(s.start * 0.137) * 61.0;
    let b = broken(s);
    let a = in.at.x;
    let lead = cos(in.at.y * 2.0 * PI);
    // Plasma rolling forward over the crest: lumps of it, drawn out the way it turns, and
    // fine flow lines through them, finer the wider the arch.
    let span = 4.0 + 0.25 * front_half(s);
    let roll = in.at.y * 6.0 - age * 2.4;
    let warp = value_noise2(vec2<f32>(a * span * 0.5, roll * 0.5) + seed, 1.0);
    let lumps = value_noise2(vec2<f32>(a * span + warp * 1.5, roll * 0.6 + warp) + seed * 1.3, 1.0);
    let flow = streaks(vec2<f32>(a * span * 5.0 + warp * 1.2, roll * 0.15) + seed * 0.7, 10.0);
    // Its leading edge a white-hot band, pink-hot behind it, red over its body, cooler
    // down its back; hot spots where the lumps run hot; cooling as it breaks.
    let face = smoothstep(0.35, 0.95, lead);
    let back = smoothstep(0.1, -1.0, lead);
    let touch = touching(in.world);
    let hot_spot = smoothstep(0.62, 0.85, lumps);
    let heat = clamp(0.2 + 0.5 * face + 0.35 * hot_spot * (0.4 + face) + 0.12 * flow
        - 0.2 * back + 0.35 * touch - 0.9 * b, 0.0, 1.0);
    // Its back thins away into the trail; its body is lumpy, not even.
    let body = (1.0 - 0.9 * back * back) * (0.3 + 0.7 * smoothstep(0.25, 0.75, lumps));
    // Broken up, it is eaten through from its back in.
    let grain = value_noise2(vec2<f32>(a * span * 1.3, in.at.y * 9.0) + seed * 2.1, 1.0);
    let alive = smoothstep(-0.03, 0.03, grain * 0.6 + 0.45 + 0.2 * lead - 1.2 * b);
    let glow = 0.2 + 0.6 * pow(edge, 3.0) + 0.45 * flow + 0.6 * face * face;
    let light = wake_heat(heat) * glow * body * (1.0 + 0.8 * touch)
        + VIOLET * pow(edge, 6.0) * back * 0.5;
    let cover = clamp((0.04 + 0.22 * pow(edge, 3.0)) * body, 0.0, 0.4);
    return vec4<f32>(light * 0.8, cover) * alive * s.fade;
}

fn fs_wake_trail_lit(in: ShellOut) -> vec4<f32> {
    let s = shells[in.shell];
    let edge = edge_of(in);
    let age = globals.camera.w - s.start;
    let seed = fract(s.start * 0.137) * 61.0;
    let u = in.at.x;
    let a = in.at.y;
    let along = in.passed.x;
    let passed = in.passed.y;
    // Hot where the front has just been, cooling behind it.
    let heat = exp(-passed / WAKE_SHELL_COOL);
    // Threads of light laid where it passed, so they stay put as it cools: long fine ones
    // running out from the muzzle, bending a little, and a second set across them.
    let q1 = vec2<f32>(along * 0.06 - age * 0.15, a * 55.0 + sin(along * 0.05 + seed) * 1.5) + seed;
    let q2 = vec2<f32>(along * 0.09 + a * 9.0, a * 33.0 - along * 0.05 + age * 0.4) + seed * 1.7;
    let wisp = streaks(q1, 20.0) + 0.55 * streaks(q2, 24.0);
    // Eaten through in hard-edged holes as it cools, from the muzzle out behind the front.
    let grain = value_noise2(vec2<f32>(along * 0.08, a * 14.0) + seed * 3.1, 1.0) * 0.6
        + value_noise2(vec2<f32>(along * 0.25, a * 40.0) + seed * 1.9, 1.0) * 0.4;
    let alive = smoothstep(-0.03, 0.03, grain * 0.75 + 0.3 - passed / WAKE_SHELL_LINGER);
    // A breath back at the muzzle, more toward the front.
    let out = smoothstep(0.0, 0.5, u);
    // Its foot burns on the ground, white where the front has just been, then an ember line.
    let touch = touching(in.world) * out;
    let lit = 0.04 + wisp * (0.12 + 0.5 * heat) + pow(edge, 4.0) * 0.6 + touch * (0.4 + 1.2 * heat);
    let tone = wake_heat(clamp(heat * 0.85 + 0.2 * wisp + 0.35 * touch * heat, 0.0, 1.0));
    let light = tone * lit * out + VIOLET * pow(edge, 8.0) * (1.0 - heat) * out * 0.3;
    let cover = clamp((0.08 + 0.3 * pow(edge, 3.0) + 0.08 * wisp) * out, 0.0, 0.45);
    return vec4<f32>(light * 0.7, cover) * alive * s.fade;
}

// The two, seen through the water from under it (bindings.wgsl `under_sea_seen`).
@fragment
fn fs_wake_front(in: ShellOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_wake_front_lit(in), in.clip);
}

@fragment
fn fs_wake_trail(in: ShellOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_wake_trail_lit(in), in.clip);
}
