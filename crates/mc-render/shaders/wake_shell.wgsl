//!use bindings
// A cone weapon's wake (renderer/wake_shell.rs, wake_fx.rs), drawn as two meshes, one
// instance of each a wake:
//
// - **The front**, the wave: a wall of plasma standing on the arc where the front stands,
//   across the whole fan and falling away at its two ends, its top curling forward. It is
//   plasma turning over on itself: white-hot along its leading face and its crest, orange
//   over its body, red down its back, in hot cells that roll up and over the crest. Run
//   out, it rolls on a little way, slowing, slumps and is eaten through.
// - **The trail** leading up to it: the fan laid low over the ground from the muzzle out
//   to the wall, rising up the wall's back, in long threads of light running out to the
//   wall. Each stretch is hot when the front has just passed it and cools after, orange to
//   red to a deep ember, and is eaten through in hard-edged holes (no mist,
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
    // The wall's height over the middle of the fan, metres.
    height: f32,
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
    // The front: along its arc (0..1) and round its section (0..1, 0 its foot, a quarter
    // its leading face, a half its top).
    // The trail: how far out (0 muzzle, 1 the wall) and across (0..1).
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

// Out over the ground from the muzzle, `c` of the way from the fan's middle (0) to one
// edge (±1).
fn radial(s: WakeShell, c: f32) -> vec2<f32> {
    let psi = c * atan(s.spread * WAKE_SHELL_WIDTH);
    let cs = cos(psi);
    let sn = sin(psi);
    return vec2<f32>(s.ahead.x * cs - s.ahead.y * sn, s.ahead.x * sn + s.ahead.y * cs);
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

// The wall's height `c` along its arc: full over the middle, falling away to nothing at
// its two ends, slumping as it breaks up.
fn wall_tall(s: WakeShell, c: f32) -> f32 {
    let taper = sqrt(max(1.0 - pow(abs(c), 4.0), 0.0));
    return s.height * taper * (1.0 - 0.75 * broken(s)) + 0.3;
}

// How deep the wall's back runs down behind its foot.
fn wall_back(tall: f32) -> f32 {
    return 0.45 * tall + 2.0;
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
    let c = 2.0 * a - 1.0;
    let theta = t * 2.0 * PI;
    let out_xy = radial(s, c);
    let foot = s.apex.xy + out_xy * s.front;
    let tall = wall_tall(s, c);
    // Its section, a wave's face: steep and thin ahead, sloping long down its back, its top
    // curling forward over its foot.
    let face = 0.06 * tall + 0.4;
    let back = wall_back(tall);
    let rise = 0.5 * (1.0 - cos(theta));
    let depth = select(back, face, sin(theta) > 0.0);
    let fwd = sin(theta) * depth + 0.22 * tall * rise * rise;
    let xy = foot + out_xy * fwd;
    let p = vec3<f32>(xy, ground_at(foot) + tall * rise);
    let n2 = vec2<f32>(0.5 * tall * sin(theta), -depth * cos(theta));
    var out: ShellOut;
    out.shell = instance;
    out.normal = normalize(vec3<f32>(out_xy * n2.x, n2.y));
    out.world = p;
    out.at = vec2<f32>(a, t);
    out.passed = vec2<f32>(rise, 0.0);
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
    let c = 2.0 * a - 1.0;
    let out_xy = radial(s, c);
    // Laid low over the fan out to the wall, rising up the last of the way to meet its
    // back; the rows bunch toward the wall, where it is busiest.
    let along = (1.0 - (1.0 - u) * (1.0 - u)) * s.front;
    let tall = wall_tall(s, c);
    let ramp = smoothstep(s.front - 2.5 * wall_back(tall), s.front, along);
    let age = globals.camera.w - s.start;
    let passed = max(age - along / s.speed, 0.0);
    let xy = s.apex.xy + out_xy * along;
    // Its heat lifts it a little off the ground as it cools.
    let floor = mix(s.apex.z, ground_at(xy), smoothstep(0.0, 20.0, along)) + 0.3 + 0.8 * passed;
    let p = vec3<f32>(xy, floor + 0.3 * tall * ramp * ramp);
    var out: ShellOut;
    out.shell = instance;
    out.normal = normalize(vec3<f32>(-out_xy * 0.6 * ramp, 1.0));
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
    let lead = sin(in.at.y * 2.0 * PI);
    let up = in.passed.x;
    // Plasma rolling up its face and over its crest: lumps of it, drawn out the way it
    // turns, and fine flow lines through them, finer the wider the arc.
    let span = 6.0 + 0.4 * s.front * s.spread;
    let roll = in.at.y * 6.0 - age * 2.4;
    let warp = value_noise2(vec2<f32>(a * span * 0.5, roll * 0.5) + seed, 1.0);
    let lumps = value_noise2(vec2<f32>(a * span + warp * 1.5, roll * 0.6 + warp) + seed * 1.3, 1.0);
    let flow = streaks(vec2<f32>(a * span * 5.0 + warp * 1.2, roll * 0.15) + seed * 0.7, 10.0);
    // Its leading face and crest white-hot, orange over its body, red down its back; hot
    // spots where the lumps run hot; cooling as it breaks.
    let face = smoothstep(0.3, 0.95, lead);
    let back = smoothstep(0.1, -1.0, lead);
    let crest = smoothstep(0.75, 1.0, up);
    let touch = touching(in.world);
    let hot_spot = smoothstep(0.62, 0.85, lumps);
    let heat = clamp(0.12 + 0.3 * face + 0.35 * crest + 0.35 * hot_spot * (0.4 + face)
        + 0.12 * flow - 0.25 * back + 0.3 * touch - 0.9 * b, 0.0, 1.0);
    // Its back thins away into the trail; its body is lumpy, not even.
    let body = (1.0 - 0.85 * back * back) * (0.35 + 0.65 * smoothstep(0.25, 0.75, lumps));
    // Broken up, it is eaten through from its crest and back down.
    let grain = value_noise2(vec2<f32>(a * span * 1.3, in.at.y * 9.0) + seed * 2.1, 1.0);
    let alive = smoothstep(-0.03, 0.03, grain * 0.6 + 0.5 + 0.2 * lead - 0.3 * up - 1.1 * b);
    let glow = 0.2 + 0.5 * pow(edge, 3.0) + 0.45 * flow + 0.35 * face * face + 0.4 * crest;
    let light = wake_heat(heat) * glow * body * (1.0 + 0.8 * touch)
        + VIOLET * pow(edge, 6.0) * back * 0.5;
    let cover = clamp((0.06 + 0.22 * pow(edge, 3.0)) * body, 0.0, 0.4);
    return vec4<f32>(light * 0.8, cover) * alive * s.fade;
}

fn fs_wake_trail_lit(in: ShellOut) -> vec4<f32> {
    let s = shells[in.shell];
    let age = globals.camera.w - s.start;
    let seed = fract(s.start * 0.137) * 61.0;
    let u = in.at.x;
    let c = 2.0 * in.at.y - 1.0;
    let along = in.passed.x;
    let passed = in.passed.y;
    // Hot where the front has just been, cooling behind it.
    let heat = exp(-passed / WAKE_SHELL_COOL);
    // Long threads of light running out to the wall, streaming out after it: fine ones
    // close across the fan, and broader ones between them.
    let out_flow = along * 0.025 - age * s.speed * 0.02;
    let q1 = vec2<f32>(c * 26.0 + sin(along * 0.03 + seed) * 0.6, out_flow) + seed;
    let q2 = vec2<f32>(c * 11.0 - along * 0.004, out_flow * 1.6) + seed * 1.7;
    let threads = streaks(q1, 14.0) + 0.6 * streaks(q2, 6.0);
    // Eaten through in hard-edged holes, long ones down the threads, as it cools, from the
    // muzzle out behind the front.
    let grain = value_noise2(vec2<f32>(c * 9.0, along * 0.04) + seed * 3.1, 1.0) * 0.6
        + value_noise2(vec2<f32>(c * 30.0, along * 0.12) + seed * 1.9, 1.0) * 0.4;
    let alive = smoothstep(-0.03, 0.03, grain * 0.75 + 0.3 - passed / WAKE_SHELL_LINGER);
    // A breath back at the muzzle, building out to the wall, and thinning at the fan's
    // edges.
    let near_wall = smoothstep(0.0, 1.0, u);
    let sides = 1.0 - smoothstep(0.75, 1.0, abs(c));
    let fill = smoothstep(0.0, 0.3, u) * sides;
    let lit = (0.015 + threads * (0.12 + 0.7 * heat)) * (0.4 + 0.6 * near_wall)
        + 0.12 * heat * near_wall * near_wall;
    let tone = wake_heat(clamp(heat * 0.6 + 0.25 * threads * heat, 0.0, 1.0));
    let light = tone * lit * fill;
    let cover = clamp((0.03 + 0.1 * threads) * heat * fill, 0.0, 0.3);
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
