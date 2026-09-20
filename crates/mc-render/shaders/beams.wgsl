//!use bindings
// Reclaim beams. The sim lists who is taking what apart each tick; everything
// seen here is made from that on the GPU: a cone of light that grips the target
// and narrows into the emitter, the glow where it bites, and the torn-off bits
// that stream back up it, heating from red through orange to white as they go.
// Premultiplied: the white core only adds light, but the orange and the red
// also cover what is behind them, or over grass they would wash out to yellow.

// Mirrors the renderer's GpuBeam: mc_sim::reclaim::BeamInstance, and when the beam came on and went off.
struct Beam {
    emitter: vec3<f32>,
    // 0 reclaim. 1 is kept for construction.
    kind: u32,
    to_prev: vec3<f32>,
    radius: f32,
    to: vec3<f32>,
    height: f32,
    // Seconds, on the clock of globals.camera.w. `end` is negative while the beam is on.
    start: f32,
    end: f32,
    pad: vec2<f32>,
}

@group(1) @binding(1) var<storage, read> beams: array<Beam>;

// Quads per beam: the ribbon, the two glows, and the bits.
const SLOTS: u32 = 32u;
const SHAPE_RIBBON: f32 = 0.0;
const SHAPE_GLOW: f32 = 1.0;
const SHAPE_BIT: f32 = 2.0;

const WHITE: vec3<f32> = vec3<f32>(1.0, 0.92, 0.78);
const ORANGE: vec3<f32> = vec3<f32>(1.0, 0.42, 0.07);
const RED: vec3<f32> = vec3<f32>(0.9, 0.07, 0.02);

struct BeamOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // x shape, y metres from the target along the beam (ribbon) or heat 0..1 (bit, glow), z half-width in metres (ribbon) or seed
    @location(1) state: vec3<f32>,
    @location(2) level: f32,
}

fn hash(n: f32) -> f32 {
    return fract(sin(n * 12.9898) * 43758.547);
}

fn hidden() -> BeamOut {
    var out: BeamOut;
    out.clip = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    out.uv = vec2<f32>(0.0);
    out.state = vec3<f32>(0.0);
    out.level = 0.0;
    return out;
}

// A quad of `half_px` pixels round a world point, stretched `stretch` times along the screen direction `dir`.
fn billboard(world: vec3<f32>, corner: vec2<f32>, half_px: f32, dir: vec2<f32>, stretch: f32) -> vec4<f32> {
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    let side = vec2<f32>(-dir.y, dir.x);
    let offset = (dir * corner.x * stretch + side * corner.y) * half_px * globals.viewport.zw;
    return vec4<f32>((c.xy / c.w + offset) * c.w, c.z, c.w);
}

@vertex
fn vs_beam(@location(0) corner: vec2<f32>, @builtin(instance_index) instance: u32) -> BeamOut {
    let b = beams[instance / SLOTS];
    let slot = instance % SLOTS;
    let time = globals.camera.w;
    let foot = mix(b.to_prev, b.to, globals.sun.w);
    let grip = foot + vec3<f32>(0.0, 0.0, b.height * 0.55);
    let span = b.emitter - grip;
    let len = max(length(span), 0.01);
    let axis = span / len;

    let a = globals.view_proj * vec4<f32>(grip, 1.0);
    let e = globals.view_proj * vec4<f32>(b.emitter, 1.0);
    if a.w <= 0.01 || e.w <= 0.01 {
        return hidden();
    }
    var dir = (e.xy / e.w - a.xy / a.w) * globals.viewport.xy;
    let dir_len = length(dir);
    if dir_len > 0.001 {
        dir = dir / dir_len;
    } else {
        dir = vec2<f32>(0.0, 1.0);
    }
    let side = vec2<f32>(-dir.y, dir.x);
    var out: BeamOut;
    out.uv = corner;
    // The light comes up quickly when the beam comes on and dies as quickly when it goes off.
    // What is on its way up it is not cut short: see the bits below.
    var power = smoothstep(0.0, 0.18, time - b.start);
    if b.end >= 0.0 {
        power = power * (1.0 - smoothstep(0.0, 0.25, time - b.end));
    }
    out.level = power;

    if slot == 0u {
        // The ribbon: wide where it grips the target, narrow at the emitter.
        let at_emitter = corner.x > 0.0;
        let half_m = select(clamp(b.radius * 0.45, 0.9, 5.0), 0.55, at_emitter);
        let p = select(a, e, at_emitter);
        let half_px = max(half_m * globals.lod.x / max(p.w, 1.0), 1.6);
        let ndc = p.xy / p.w + side * corner.y * half_px * globals.viewport.zw;
        out.clip = vec4<f32>(ndc * p.w, p.z, p.w);
        // Half-width as drawn, in metres: from far off the ribbon is kept a few pixels wide.
        out.state = vec3<f32>(SHAPE_RIBBON, select(0.0, len, at_emitter), half_px * max(p.w, 1.0) / globals.lod.x);
        return out;
    }
    if slot <= 2u {
        // Where it bites, and the emitter it all goes into.
        let at_emitter = slot == 2u;
        let world = select(grip, b.emitter, at_emitter);
        let radius = select(clamp(b.radius * 1.15, 2.0, 12.0), 1.5, at_emitter);
        let w = select(a.w, e.w, at_emitter);
        let flicker = 0.75 + 0.25 * sin(time * 31.0 + f32(instance)) * sin(time * 17.3);
        out.clip = billboard(world, corner, max(radius * globals.lod.x / max(w, 1.0), 2.5), vec2<f32>(1.0, 0.0), 1.0);
        out.state = vec3<f32>(SHAPE_GLOW, select(0.0, 1.0, at_emitter), 0.0);
        out.level = power * flicker * select(0.55, 1.6, at_emitter);
        return out;
    }

    // A bit of the target on its way up the beam. Seeded by the emitter, so it does not jump when the list of beams changes.
    let seed = f32(slot) * 7.31 + b.emitter.x * 0.37 + b.emitter.y * 0.73;
    let trip = clamp(len / 42.0, 0.55, 2.2) * (0.8 + 0.5 * hash(seed + 1.0));
    let turns = time / trip + hash(seed + 2.0);
    let phase = fract(turns);
    // A bit exists only if it tore loose while the beam was on: a new beam fills from the
    // target, and one that has shut off empties into the emitter. Nothing pops in or out mid-air.
    let born = time - phase * trip;
    if born < b.start || (b.end >= 0.0 && born > b.end) {
        return hidden();
    }
    // Slow to tear loose, then faster and faster.
    let along = pow(phase, 1.7);
    var b1 = cross(axis, vec3<f32>(0.0, 0.0, 1.0));
    if dot(b1, b1) < 1e-4 {
        b1 = vec3<f32>(1.0, 0.0, 0.0);
    }
    b1 = normalize(b1);
    let b2 = cross(axis, b1);
    let swing = select(-1.0, 1.0, hash(seed + 3.0) > 0.5);
    let angle = hash(seed + 4.0) * 6.2832 + along * 3.4 * swing;
    let off = b.radius * (0.25 + 0.75 * hash(seed + 5.0)) * pow(1.0 - along, 1.4);
    let start = (hash(seed + 6.0) - 0.5) * b.radius;
    let world = grip + axis * (start + (len - start) * along) + (b1 * cos(angle) + b2 * sin(angle)) * off;
    let size = mix(1.3, 0.35, along) * (0.6 + 0.9 * hash(seed + 7.0)) * clamp(b.radius / 7.0, 0.7, 2.2);
    let c = globals.view_proj * vec4<f32>(world, 1.0);
    let half_px = max(size * globals.lod.x / max(c.w, 1.0), 2.0);
    // Drawn out along its flight the faster it goes.
    out.clip = billboard(world, corner, half_px, dir, 1.0 + along * 2.0);
    out.state = vec3<f32>(SHAPE_BIT, along, hash(seed + 8.0));
    out.level = smoothstep(0.0, 0.1, phase) * (1.0 - smoothstep(0.9, 1.0, phase));
    return out;
}

@fragment
fn fs_beam(in: BeamOut) -> @location(0) vec4<f32> {
    let time = globals.camera.w;
    let run = in.state.y;
    // Sampled for every shape: a texture is read in uniform control flow.
    let n = textureSample(noise_map, repeat_sampler, vec2<f32>(run * 0.035 + time * 0.9, in.uv.y * 0.11 + time * 0.07)).b;
    if in.state.x < 0.5 {
        // Across the ribbon in metres: a thin white core, orange about it, red to the edge.
        let half_m = max(in.state.z, 0.05);
        let y = abs(in.uv.y) * half_m;
        // Pulses run from the target to the emitter, like the bits do.
        let pulse = 0.7 + 0.3 * sin(run * 0.9 + time * 14.0);
        // Kept dim enough outside the core that the tone mapper leaves orange orange and red red.
        let core = exp(-y * y / 0.02) * (0.8 + 0.4 * n);
        let body = exp(-y * y / (0.16 * half_m * half_m + 0.06)) * (0.45 + 0.8 * n) * pulse;
        let edge = pow(max(1.0 - abs(in.uv.y), 0.0), 1.5) * (0.35 + 0.65 * n);
        let color = WHITE * core * 5.0 + ORANGE * body * 1.3 + RED * edge * 0.8;
        return vec4<f32>(color * in.level, clamp(body * 0.7 + edge * 0.55, 0.0, 0.85) * in.level);
    }
    if in.state.x < 1.5 {
        let d = length(in.uv);
        if d > 1.0 {
            discard;
        }
        let fall = pow(1.0 - d, 2.2);
        let hot = mix(mix(RED, ORANGE, fall), WHITE, in.state.y * fall);
        return vec4<f32>(hot * fall * 2.0 * in.level, clamp(fall * 0.5 * in.level, 0.0, 0.6));
    }
    // A shard, not a spark: a hard-edged, crooked plate that tumbles as it goes.
    let spin = in.state.z * 6.2832 + time * (2.5 + in.state.z * 6.0) * select(-1.0, 1.0, in.state.z > 0.5);
    let p = vec2<f32>(in.uv.x * cos(spin) - in.uv.y * sin(spin), in.uv.x * sin(spin) + in.uv.y * cos(spin));
    let skew = p.y + p.x * (in.state.z - 0.5) * 0.9;
    let d = max(abs(p.x) * (1.45 + in.state.z * 0.5), abs(skew) * 2.3);
    if d > 1.0 {
        discard;
    }
    let heat = in.state.y;
    let color = mix(mix(RED, ORANGE, smoothstep(0.0, 0.45, heat)), WHITE, smoothstep(0.55, 1.0, heat));
    // Its torn edge runs hotter than its face.
    let solid = (1.0 - smoothstep(0.85, 1.0, d)) * (0.7 + 0.6 * smoothstep(0.45, 0.9, d));
    // Dull red as it tears loose; white hot only on the last of the way in.
    let glow = 0.85 + 0.5 * smoothstep(0.2, 0.6, heat) + 4.0 * smoothstep(0.75, 1.0, heat);
    return vec4<f32>(color * solid * glow * in.level, solid * in.level * 0.95);
}
