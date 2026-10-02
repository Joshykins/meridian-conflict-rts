//!use bindings
// A cone weapon's wake as a shell of light (renderer/wake_shell.rs, wake_fx.rs): from the
// muzzle it opens as a cone across the fan, swells and closes in a rounded dome at the
// front, half of it standing above the ground. It is light and nothing else: faint where
// it is seen square on, bright where it is seen edge on, threaded with thin streaks that
// stretch as it grows, and white-hot where it meets the ground. Brightest toward the
// front, faint back at the muzzle. One instance a wake, a mesh of `WAKE_SHELL_ALONG` by
// `WAKE_SHELL_AROUND` quads, laid over the scene premultiplied: mostly light added, a
// little of what is behind dimmed.

//!rust crate::renderer::wake_shell::GpuWakeShell
struct WakeShell {
    // The muzzle, and the renderer time the wake left it.
    apex: vec3<f32>,
    start: f32,
    // Which way it rolls over the ground (unit length), and the tangent of the fan's
    // half-angle.
    ahead: vec2<f32>,
    spread: f32,
    // Metres out the front stands now.
    front: f32,
    // The ground's height under the muzzle and under the front.
    base: vec2<f32>,
    // The shell's height over its half-width.
    rise: f32,
    // 0 gone, 1 full.
    fade: f32,
}

@group(1) @binding(0) var<storage, read> shells: array<WakeShell>;

const RED: vec3<f32> = vec3<f32>(1.0, 0.02, 0.06);
const HOT: vec3<f32> = vec3<f32>(1.0, 0.35, 0.45);
const VIOLET: vec3<f32> = vec3<f32>(0.7, 0.2, 1.0);

struct ShellOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // How far out (0 muzzle, 1 front), and round (0 one side on the ground, 1 the other).
    @location(2) at: vec2<f32>,
    @location(3) @interpolate(flat) shell: u32,
}

// The shell's half-width `u` of the way out, in fan half-widths at the front.
fn shell_width(u: f32) -> f32 {
    let cap = WAKE_SHELL_CAP;
    let close = select(1.0, sqrt(max(1.0 - pow((u - cap) / (1.0 - cap), 2.0), 0.0)), u > cap);
    return u * (1.0 + WAKE_SHELL_SWELL * smoothstep(0.45, 0.85, u)) * close;
}

// The point `u` of the way out and `a` (0..1) of the way round.
fn shell_point(s: WakeShell, u: f32, a: f32) -> vec3<f32> {
    let side = vec2<f32>(-s.ahead.y, s.ahead.x);
    let half = s.front * s.spread * shell_width(u);
    let phi = a * PI;
    let floor = mix(s.apex.z, mix(s.base.x, s.base.y, u), smoothstep(0.0, 0.2, u));
    let xy = s.apex.xy + s.ahead * (s.front * u) + side * (half * cos(phi));
    return vec3<f32>(xy, floor + s.rise * half * sin(phi));
}

@vertex
fn vs_wake_shell(@builtin(vertex_index) v: u32, @builtin(instance_index) instance: u32) -> ShellOut {
    let s = shells[instance];
    var out: ShellOut;
    out.shell = instance;
    if s.fade <= 0.0 || s.front < 1.0 {
        out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
        return out;
    }
    let quad = v / 6u;
    let corner = v % 6u;
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
    let t = f32(quad / WAKE_SHELL_AROUND + di) / f32(WAKE_SHELL_ALONG);
    // Closer together toward the front, where the dome bends.
    let u = 1.0 - pow(1.0 - t, 1.6);
    let a = f32(quad % WAKE_SHELL_AROUND + dj) / f32(WAKE_SHELL_AROUND);
    let p = shell_point(s, u, a);
    // The normal of the stretched half-ellipse round the line ahead, taken from the shape
    // itself, so it stays true at the dome's nose where the mesh closes to a point.
    let side = vec2<f32>(-s.ahead.y, s.ahead.x);
    let slope = (shell_width(min(u + 0.005, 1.0)) - shell_width(max(u - 0.005, 0.0)))
        / (min(u + 0.005, 1.0) - max(u - 0.005, 0.0));
    let phi = a * PI;
    let flare = clamp(-s.spread * slope, -40.0, 40.0);
    out.normal = normalize(vec3<f32>(side * cos(phi) + s.ahead * flare, sin(phi) / s.rise));
    out.world = p;
    out.at = vec2<f32>(u, a);
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

fn fs_wake_shell_lit(in: ShellOut) -> vec4<f32> {
    let s = shells[in.shell];
    let eye = globals.camera.xyz;
    let view = normalize(eye - in.world);
    let facing = abs(dot(normalize(in.normal), view));
    let edge = 1.0 - facing;
    let u = in.at.x;
    let a = in.at.y;
    let age = globals.camera.w - s.start;
    let seed = fract(s.start * 0.137) * 61.0;

    // Brighter toward the front; the cone back to the muzzle only a breath.
    let out = smoothstep(0.1, 0.95, u);
    // Threads of light: long fine ones running out from the muzzle, bending a little, a
    // second set across them on the slant, and faint ripples round the front.
    let q1 = vec2<f32>(u * 5.0 - age * 0.5, a * 55.0 + sin(u * 6.0 + seed) * 1.5) + seed;
    let q2 = vec2<f32>(u * 7.0 + a * 9.0 - age * 0.3, a * 33.0 - u * 4.0) + seed * 1.7;
    let q3 = vec2<f32>(u * 45.0 - age * 2.0, a * 6.0) + seed * 2.3;
    let wisp = streaks(q1, 22.0) + 0.55 * streaks(q2, 24.0) + 0.35 * out * streaks(q3, 18.0);
    // White-hot where it meets the ground (or the water).
    let ground = max(terrain_height(in.world.xy), globals.map.z);
    let touch = exp(-max(in.world.z - ground, 0.0) / 0.5) * out * out;

    // Its nose, seen head on, would read as a hole: it glows of itself.
    let nose = smoothstep(WAKE_SHELL_CAP, 1.0, u);
    let light = RED * (0.03 * out + 0.3 * nose + wisp * (0.1 + 0.6 * out)
            + pow(edge, 4.0) * out * 0.7)
        + HOT * (touch * 1.2 + pow(edge, 16.0) * out * 0.35)
        + VIOLET * pow(edge, 20.0) * out * 0.25;
    // It dims a little of what is behind it, so it reads red over green grass and not orange.
    let cover = clamp(0.1 * out + 0.1 * nose + 0.35 * pow(edge, 3.0) * out + 0.1 * wisp * out, 0.0, 0.5);
    return vec4<f32>(light * 0.7, cover) * s.fade;
}

// fs_wake_shell, seen through the water from under it (bindings.wgsl `under_sea_seen`).
@fragment
fn fs_wake_shell(in: ShellOut) -> @location(0) vec4<f32> {
    return under_sea_seen(fs_wake_shell_lit(in), in.clip);
}
